//! Passive wire accounting; h2 alone parses HTTP/2/HPACK and controls stream semantics.
use crate::{CONCURRENT_STREAMS, FRAME_BYTES, MESSAGE_BYTES, RECEIVE_BYTES, RPC_MESSAGE_BYTES};
use std::{io, sync::Mutex, time::Duration};
use web_time::Instant;

#[derive(Clone, Copy, Debug, Default)]
pub struct BufferSnapshot {
    pub current: usize,
    pub peak: usize,
    pub total: usize,
}
impl BufferSnapshot {
    fn add(&mut self, count: usize) {
        self.current += count;
        self.total += count;
        self.peak = self.peak.max(self.current);
    }
    fn remove(&mut self, count: usize) -> io::Result<()> {
        self.current = self
            .current
            .checked_sub(count)
            .ok_or_else(|| io::Error::other("buffer accounting underflow"))?;
        Ok(())
    }
}
/// Real observed credit: DATA bytes decrease available credit; opposite-direction
/// connection WINDOW_UPDATE increases it. Initial credit is mandated by HTTP/2.
#[derive(Clone, Copy, Debug)]
pub struct CreditSnapshot {
    pub available: usize,
    pub peak_grant: usize,
    pub data_bytes: usize,
    pub update_bytes: usize,
    pub peak_held: usize,
}
impl Default for CreditSnapshot {
    fn default() -> Self {
        Self {
            available: 65_535,
            peak_grant: 65_535,
            data_bytes: 0,
            update_bytes: 0,
            peak_held: 0,
        }
    }
}
impl CreditSnapshot {
    fn consume(&mut self, count: usize) -> io::Result<()> {
        self.available = self.available.checked_sub(count).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "peer exceeded DATA credit")
        })?;
        self.data_bytes += count;
        self.peak_held = self
            .peak_held
            .max(self.peak_grant.saturating_sub(self.available));
        Ok(())
    }
    fn grant(&mut self, count: usize) -> io::Result<()> {
        self.available = self
            .available
            .checked_add(count)
            .filter(|value| *value <= 0x7fff_ffff)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid connection WINDOW_UPDATE",
                )
            })?;
        self.update_bytes += count;
        self.peak_grant = self.peak_grant.max(self.available);
        Ok(())
    }
}
/// Snapshot excludes allocator metadata and browser-engine pre-callback allocations.
#[derive(Clone, Debug)]
pub struct ConnectionSnapshot {
    pub pipe_owners: usize,
    pub driver_failed: bool,
    pub closed: bool,
    pub received_bytes: usize,
    pub sent_bytes: usize,
    pub callback_bytes: BufferSnapshot,
    pub callback_items: BufferSnapshot,
    pub pipe_to_grpc: BufferSnapshot,
    pub pipe_to_websocket: BufferSnapshot,
    pub websocket_receive: BufferSnapshot,
    pub websocket_receive_items: BufferSnapshot,
    pub websocket_send: BufferSnapshot,
    pub receive_credit: CreditSnapshot,
    pub send_credit: CreditSnapshot,
    pub received_frames: usize,
    pub sent_frames: usize,
    pub decode_yields: usize,
    pub rejected: bool,
    pub receive_reservation_bytes: usize,
}
impl ConnectionSnapshot {
    /// Conservative payload capacity reservation, deliberately larger than current
    /// bytes: callback/pipe capacity + HTTP2 credit + bounded codecs/send/control work.
    pub fn envelope_within_limit(&self) -> bool {
        self.receive_reservation_bytes <= 8 * 1024 * 1024
    }
}
struct WireObserver {
    header: [u8; 9],
    header_used: usize,
    remaining: usize,
    kind: u8,
    stream: u32,
    update: [u8; 4],
    update_used: usize,
    preface_remaining: usize,
    frames: usize,
    rate_frames: usize,
    rate_start: Instant,
}
impl WireObserver {
    fn new(preface: bool) -> Self {
        Self {
            header: [0; 9],
            header_used: 0,
            remaining: 0,
            kind: 0,
            stream: 0,
            update: [0; 4],
            update_used: 0,
            preface_remaining: if preface { 24 } else { 0 },
            frames: 0,
            rate_frames: 0,
            rate_start: Instant::now(),
        }
    }
    fn observe(
        &mut self,
        mut bytes: &[u8],
        consumed: &mut CreditSnapshot,
        granted: &mut CreditSnapshot,
    ) -> io::Result<()> {
        let skip = self.preface_remaining.min(bytes.len());
        self.preface_remaining -= skip;
        bytes = &bytes[skip..];
        while !bytes.is_empty() {
            if self.remaining == 0 {
                let count = (9 - self.header_used).min(bytes.len());
                self.header[self.header_used..self.header_used + count]
                    .copy_from_slice(&bytes[..count]);
                self.header_used += count;
                bytes = &bytes[count..];
                if self.header_used != 9 {
                    continue;
                }
                self.header_used = 0;
                let length = ((self.header[0] as usize) << 16)
                    | ((self.header[1] as usize) << 8)
                    | self.header[2] as usize;
                if length > FRAME_BYTES {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "HTTP2 frame exceeds selected 16KiB bound",
                    ));
                }
                if self.rate_start.elapsed() >= Duration::from_millis(100) {
                    self.rate_start = Instant::now();
                    self.rate_frames = 0;
                }
                self.frames += 1;
                self.rate_frames += 1;
                if self.rate_frames > 2048 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "HTTP2 frame flood bound exceeded",
                    ));
                }
                self.kind = self.header[3];
                self.stream = u32::from_be_bytes([
                    self.header[5],
                    self.header[6],
                    self.header[7],
                    self.header[8],
                ]) & 0x7fff_ffff;
                self.update_used = 0;
                self.remaining = length;
                if self.kind == 0 {
                    consumed.consume(length)?;
                }
                if length == 0 {
                    continue;
                }
            }
            let count = self.remaining.min(bytes.len());
            if self.kind == 8
                && self.stream == 0
                && self.remaining <= 4
                && self.update_used + count <= 4
            {
                self.update[self.update_used..self.update_used + count]
                    .copy_from_slice(&bytes[..count]);
                self.update_used += count;
            }
            bytes = &bytes[count..];
            self.remaining -= count;
            if self.remaining == 0 && self.kind == 8 && self.stream == 0 && self.update_used == 4 {
                granted.grant((u32::from_be_bytes(self.update) & 0x7fff_ffff) as usize)?;
            }
        }
        Ok(())
    }
}
struct State {
    snapshot: ConnectionSnapshot,
    incoming: WireObserver,
    outgoing: WireObserver,
}
/// Owned local metrics, transferable snapshots and bounded passive frame accounting.
pub struct ConnectionMetrics {
    state: Mutex<State>,
}
impl ConnectionMetrics {
    pub fn new(browser: bool) -> Self {
        let snapshot = ConnectionSnapshot {
            pipe_owners: if browser { 0 } else { 2 },
            driver_failed: false,
            closed: false,
            received_bytes: 0,
            sent_bytes: 0,
            callback_bytes: BufferSnapshot::default(),
            callback_items: BufferSnapshot::default(),
            pipe_to_grpc: BufferSnapshot::default(),
            pipe_to_websocket: BufferSnapshot::default(),
            websocket_receive: BufferSnapshot::default(),
            websocket_receive_items: BufferSnapshot::default(),
            websocket_send: BufferSnapshot::default(),
            receive_credit: CreditSnapshot::default(),
            send_credit: CreditSnapshot::default(),
            received_frames: 0,
            sent_frames: 0,
            decode_yields: 0,
            rejected: false,
            receive_reservation_bytes: RECEIVE_BYTES * if browser { 2 } else { 3 }
                + CONCURRENT_STREAMS as usize
                    * (RPC_MESSAGE_BYTES * 2 + MESSAGE_BYTES + FRAME_BYTES)
                + MESSAGE_BYTES * 2
                + FRAME_BYTES * 10,
        };
        Self {
            state: Mutex::new(State {
                snapshot,
                incoming: WireObserver::new(!browser),
                outgoing: WireObserver::new(browser),
            }),
        }
    }
    pub fn snapshot(&self) -> io::Result<ConnectionSnapshot> {
        self.state
            .lock()
            .map(|state| state.snapshot.clone())
            .map_err(|_| io::Error::other("resource snapshot lock poisoned"))
    }
    pub(crate) fn observe(&self, incoming: bool, bytes: &[u8]) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("resource observation lock poisoned"))?;
        let State {
            snapshot,
            incoming: receive,
            outgoing: send,
        } = &mut *state;
        let result = if incoming {
            snapshot.received_bytes += bytes.len();
            receive.observe(
                bytes,
                &mut snapshot.receive_credit,
                &mut snapshot.send_credit,
            )
        } else {
            snapshot.sent_bytes += bytes.len();
            send.observe(
                bytes,
                &mut snapshot.send_credit,
                &mut snapshot.receive_credit,
            )
        };
        snapshot.received_frames = receive.frames;
        snapshot.sent_frames = send.frames;
        if result.is_err() {
            snapshot.rejected = true;
        }
        result
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn queue(&self, bytes: usize, enqueue: bool, remove_item: bool) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("queue metrics lock poisoned"))?;
        if enqueue {
            state.snapshot.callback_bytes.add(bytes);
            state.snapshot.callback_items.add(1);
        } else {
            state.snapshot.callback_bytes.remove(bytes)?;
            if remove_item {
                state.snapshot.callback_items.remove(1)?;
            }
        }
        Ok(())
    }
    pub(crate) fn socket_receive(&self, bytes: usize, holding: bool) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("WebSocket receipt metrics lock poisoned"))?;
        if holding {
            state.snapshot.websocket_receive.add(bytes);
            state.snapshot.websocket_receive_items.add(1);
        } else {
            state.snapshot.websocket_receive.remove(bytes)?;
            state.snapshot.websocket_receive_items.remove(1)?;
        }
        Ok(())
    }
    pub(crate) fn backlog(&self, bytes: usize) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("backlog metrics lock poisoned"))?;
        state.snapshot.websocket_send.total = state.snapshot.sent_bytes;
        state.snapshot.websocket_send.current = bytes;
        state.snapshot.websocket_send.peak = state.snapshot.websocket_send.peak.max(bytes);
        Ok(())
    }
    pub(crate) fn yield_decode(&self) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("decode metrics lock poisoned"))?;
        state.snapshot.decode_yields += 1;
        Ok(())
    }
    pub(crate) fn reject(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.snapshot.rejected = true;
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn driver_finished(&self, failed: bool) {
        if let Ok(mut state) = self.state.lock() {
            state.snapshot.driver_failed = failed;
        }
    }
    pub(crate) fn close(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.snapshot.closed = true;
            state.snapshot.callback_bytes.current = 0;
            state.snapshot.callback_items.current = 0;
            // Keep the last observed engine backlog; WebSocket close may continue draining.
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn pipe_drop(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.snapshot.pipe_owners = state.snapshot.pipe_owners.saturating_sub(1);
            if state.snapshot.pipe_owners == 0 {
                state.snapshot.pipe_to_grpc.current = 0;
                state.snapshot.pipe_to_websocket.current = 0;
            }
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn pipe<T>(
        &self,
        to_grpc: bool,
        operation: impl FnOnce(&mut BufferSnapshot) -> T,
    ) -> io::Result<T> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("pipe metrics lock poisoned"))?;
        Ok(operation(if to_grpc {
            &mut state.snapshot.pipe_to_grpc
        } else {
            &mut state.snapshot.pipe_to_websocket
        }))
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl BufferSnapshot {
    pub(crate) fn transferred(&mut self, count: usize, writing: bool) -> io::Result<()> {
        if writing {
            self.add(count);
            Ok(())
        } else {
            self.remove(count)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_frame_boundaries_preserve_data_credit() {
        let metrics = ConnectionMetrics::new(true);
        let frame = [0, 0, 3, 0, 0, 0, 0, 0, 1, 1, 2, 3];
        for byte in frame {
            metrics.observe(true, &[byte]).unwrap();
        }
        let snapshot = metrics.snapshot().unwrap();
        assert_eq!(snapshot.receive_credit.data_bytes, 3);
        assert_eq!(snapshot.receive_credit.available, 65532);
        assert_eq!(snapshot.received_frames, 1);
        assert!(snapshot.envelope_within_limit());
    }
    #[test]
    fn tiny_control_frame_flood_is_explicitly_rejected() {
        let metrics = ConnectionMetrics::new(true);
        let mut bytes = vec![];
        for _ in 0..2049 {
            bytes.extend([0, 0, 0, 4, 0, 0, 0, 0, 0]);
        }
        assert!(metrics.observe(true, &bytes).is_err());
        assert!(metrics.snapshot().unwrap().rejected);
    }
}
