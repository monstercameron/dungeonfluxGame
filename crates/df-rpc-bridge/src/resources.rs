//! Passive wire accounting; h2 alone parses HTTP/2/HPACK and controls stream semantics.
use crate::{CONCURRENT_STREAMS, FRAME_BYTES, MESSAGE_BYTES, RECEIVE_BYTES, RPC_MESSAGE_BYTES};
use std::{collections::VecDeque, io, sync::Mutex, time::Duration};
use web_time::Instant;

#[cfg(any(test, target_arch = "wasm32"))]
use bytes::Bytes;
#[cfg(any(test, target_arch = "wasm32"))]
use std::sync::{Arc, MutexGuard};

/// The original callback Vec stays owned across every Bytes split or clone.
#[cfg(any(test, target_arch = "wasm32"))]
struct BrowserCallbackBacking {
    bytes: Vec<u8>,
    capacity: usize,
    metrics: Arc<ConnectionMetrics>,
}
#[cfg(any(test, target_arch = "wasm32"))]
impl AsRef<[u8]> for BrowserCallbackBacking {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}
#[cfg(any(test, target_arch = "wasm32"))]
impl Drop for BrowserCallbackBacking {
    fn drop(&mut self) {
        // Bytes drops this owner only after its final alias; free the allocation first.
        drop(std::mem::take(&mut self.bytes));
        if self
            .metrics
            .browser_callback_capacity(self.capacity, false)
            .is_err()
        {
            self.metrics.reject();
        }
    }
}

/// Credit already consumed by browser response bodies on one physical connection.
/// The coordinator owns no DATA and never changes the advertised receive windows.
#[cfg(any(test, target_arch = "wasm32"))]
#[derive(Clone)]
pub(crate) struct ReceiveCredit {
    slots: Arc<Mutex<Vec<CreditSlot>>>,
    metrics: Arc<ConnectionMetrics>,
}

#[cfg(any(test, target_arch = "wasm32"))]
struct CreditSlot {
    id: h2::StreamId,
    flow: h2::FlowControl,
    pending: usize,
}

#[cfg(any(test, target_arch = "wasm32"))]
impl ReceiveCredit {
    pub(crate) fn new(metrics: Arc<ConnectionMetrics>) -> Self {
        Self {
            slots: Arc::new(Mutex::new(Vec::with_capacity(CONCURRENT_STREAMS as usize))),
            metrics,
        }
    }

    fn slots(&self) -> Result<MutexGuard<'_, Vec<CreditSlot>>, h2::Error> {
        self.slots.lock().map_err(|_| {
            self.metrics.reject();
            h2::Reason::INTERNAL_ERROR.into()
        })
    }

    pub(crate) fn register(&self, flow: h2::FlowControl) -> Result<h2::StreamId, h2::Error> {
        let id = flow.stream_id();
        let mut slots = self.slots()?;
        if slots.len() == CONCURRENT_STREAMS as usize || slots.iter().any(|slot| slot.id == id) {
            drop(slots);
            self.metrics.reject();
            return Err(h2::Reason::ENHANCE_YOUR_CALM.into());
        }
        slots.push(CreditSlot {
            id,
            flow,
            pending: 0,
        });
        Ok(id)
    }

    /// Called only when the body is polled again after delivering a DATA frame.
    pub(crate) fn consumed(&self, id: h2::StreamId, bytes: usize) -> Result<(), h2::Error> {
        if bytes == 0 {
            return Ok(());
        }
        let selection = {
            let mut slots = self.slots()?;
            let Some(slot) = slots.iter_mut().find(|slot| slot.id == id) else {
                self.metrics.reject();
                return Err(h2::Reason::INTERNAL_ERROR.into());
            };
            let Some(pending) = slot.pending.checked_add(bytes) else {
                self.metrics.reject();
                return Err(h2::Reason::INTERNAL_ERROR.into());
            };
            slot.pending = pending;
            if slot.pending >= MESSAGE_BYTES {
                Some(vec![id])
            } else if slots.iter().map(|slot| slot.pending).sum::<usize>() >= RECEIVE_BYTES / 2 {
                Some(
                    slots
                        .iter()
                        .filter(|slot| slot.pending != 0)
                        .map(|slot| slot.id)
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            }
        };
        if let Some(ids) = selection {
            self.release(&ids, false)?;
        }
        Ok(())
    }

    /// A real h2 receive Pending is a progress boundary even before a batch threshold.
    pub(crate) fn pending(&self, id: h2::StreamId) -> Result<(), h2::Error> {
        self.release(&[id], false)
    }

    /// Release consumed remainder and remove the cloned h2 handle before a body ends.
    pub(crate) fn finish(&self, id: h2::StreamId) -> Result<(), h2::Error> {
        self.release(&[id], true)
    }

    fn release(&self, ids: &[h2::StreamId], remove: bool) -> Result<(), h2::Error> {
        // Take both accounting and handles out of the mutex before h2 can wake its driver.
        let mut batch = Vec::with_capacity(ids.len());
        let mut missing = false;
        let mut poisoned = false;
        {
            let mut slots = match self.slots.lock() {
                Ok(slots) => slots,
                Err(error) => {
                    poisoned = true;
                    self.metrics.reject();
                    error.into_inner()
                }
            };
            for id in ids {
                if let Some(index) = slots.iter().position(|slot| slot.id == *id) {
                    if remove {
                        let slot = slots.swap_remove(index);
                        batch.push((slot.id, slot.flow, slot.pending));
                    } else {
                        let slot = &mut slots[index];
                        if slot.pending != 0 {
                            batch.push((
                                slot.id,
                                slot.flow.clone(),
                                std::mem::take(&mut slot.pending),
                            ));
                        }
                    }
                } else {
                    missing = true;
                }
            }
        }
        let mut first_error =
            (missing || poisoned).then(|| h2::Error::from(h2::Reason::INTERNAL_ERROR));
        if missing {
            self.metrics.reject();
        }
        for (id, mut flow, bytes) in batch {
            if bytes == 0 {
                continue;
            }
            if let Err(error) = flow.release_capacity(bytes) {
                self.metrics.reject();
                if !remove {
                    let mut slots = match self.slots.lock() {
                        Ok(slots) => slots,
                        Err(error) => {
                            self.metrics.reject();
                            if first_error.is_none() {
                                first_error = Some(h2::Reason::INTERNAL_ERROR.into());
                            }
                            error.into_inner()
                        }
                    };
                    if let Some(slot) = slots.iter_mut().find(|slot| slot.id == id) {
                        if let Some(pending) = slot.pending.checked_add(bytes) {
                            slot.pending = pending;
                        } else if first_error.is_none() {
                            first_error = Some(h2::Reason::INTERNAL_ERROR.into());
                        }
                    }
                }
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    #[cfg(all(test, not(target_arch = "wasm32")))]
    fn active(&self) -> usize {
        self.slots.lock().unwrap().len()
    }
}

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
    /// Original Rust callback Vec capacity retained by Bytes aliases. Browser only;
    /// excludes engine storage, allocator overhead, Bytes metadata and h2 buffers.
    pub browser_callback_vec_capacity: BufferSnapshot,
    pub pipe_to_grpc: BufferSnapshot,
    pub pipe_to_websocket: BufferSnapshot,
    pub websocket_receive: BufferSnapshot,
    pub websocket_receive_items: BufferSnapshot,
    pub websocket_send: BufferSnapshot,
    pub receive_credit: CreditSnapshot,
    pub send_credit: CreditSnapshot,
    pub received_frames: usize,
    pub sent_frames: usize,
    pub received_control_frames: usize,
    pub sent_control_frames: usize,
    /// Accepted incoming controls still in the rolling second, with time relative
    /// to the connection's first observation. At most 100 entries are retained.
    pub incoming_control_window: Vec<ControlFrameObservation>,
    /// The frame refused by the control-rate guard, if any.
    pub rejected_control: Option<ControlFrameObservation>,
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
#[derive(Clone, Copy, Debug)]
pub struct ControlFrameObservation {
    pub incoming: bool,
    pub kind: u8,
    pub stream: u32,
    pub elapsed_ns: u64,
}
struct TimedControl {
    at: Instant,
    frame: ControlFrameObservation,
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
    origin: Instant,
    enforce_control_rate: bool,
    control_frames: usize,
    control_window: VecDeque<TimedControl>,
    rejected_control: Option<ControlFrameObservation>,
}
impl WireObserver {
    fn new(preface: bool, enforce_control_rate: bool, now: Instant) -> Self {
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
            rate_start: now,
            origin: now,
            enforce_control_rate,
            control_frames: 0,
            control_window: VecDeque::new(),
            rejected_control: None,
        }
    }
    fn observe(
        &mut self,
        mut bytes: &[u8],
        consumed: &mut CreditSnapshot,
        granted: &mut CreditSnapshot,
        now: Instant,
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
                if now.duration_since(self.rate_start) >= Duration::from_millis(100) {
                    self.rate_start = now;
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
                if !matches!(self.kind, 0 | 1 | 9) {
                    let frame = ControlFrameObservation {
                        incoming: self.enforce_control_rate,
                        kind: self.kind,
                        stream: self.stream,
                        elapsed_ns: now.duration_since(self.origin).as_nanos() as u64,
                    };
                    if self.enforce_control_rate {
                        while self.control_window.front().is_some_and(|oldest| {
                            now.duration_since(oldest.at) >= Duration::from_secs(1)
                        }) {
                            self.control_window.pop_front();
                        }
                        if self.control_window.len() == 100 {
                            self.rejected_control = Some(frame);
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "incoming HTTP2 control frame rate exceeded",
                            ));
                        }
                        self.control_window
                            .push_back(TimedControl { at: now, frame });
                    }
                    self.control_frames += 1;
                }
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
            browser_callback_vec_capacity: BufferSnapshot::default(),
            pipe_to_grpc: BufferSnapshot::default(),
            pipe_to_websocket: BufferSnapshot::default(),
            websocket_receive: BufferSnapshot::default(),
            websocket_receive_items: BufferSnapshot::default(),
            websocket_send: BufferSnapshot::default(),
            receive_credit: CreditSnapshot::default(),
            send_credit: CreditSnapshot::default(),
            received_frames: 0,
            sent_frames: 0,
            received_control_frames: 0,
            sent_control_frames: 0,
            incoming_control_window: Vec::new(),
            rejected_control: None,
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
                incoming: WireObserver::new(!browser, true, Instant::now()),
                outgoing: WireObserver::new(browser, false, Instant::now()),
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
        self.observe_at(incoming, bytes, Instant::now())
    }
    fn observe_at(&self, incoming: bool, bytes: &[u8], now: Instant) -> io::Result<()> {
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
                now,
            )
        } else {
            snapshot.sent_bytes += bytes.len();
            send.observe(
                bytes,
                &mut snapshot.send_credit,
                &mut snapshot.receive_credit,
                now,
            )
        };
        snapshot.received_frames = receive.frames;
        snapshot.sent_frames = send.frames;
        snapshot.received_control_frames = receive.control_frames;
        snapshot.sent_control_frames = send.control_frames;
        snapshot.incoming_control_window = receive
            .control_window
            .iter()
            .map(|entry| entry.frame)
            .collect();
        snapshot.rejected_control = receive.rejected_control;
        if result.is_err() {
            snapshot.rejected = true;
        }
        result
    }
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn queue(&self, bytes: usize, enqueue: bool, remove_item: bool) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("queue metrics lock poisoned"))?;
        if enqueue {
            state.snapshot.callback_bytes.add(bytes);
            state.snapshot.callback_items.add(1);
        } else {
            let remaining_bytes = state
                .snapshot
                .callback_bytes
                .current
                .checked_sub(bytes)
                .ok_or_else(|| io::Error::other("callback byte accounting underflow"))?;
            let remaining_items = state
                .snapshot
                .callback_items
                .current
                .checked_sub(usize::from(remove_item))
                .ok_or_else(|| io::Error::other("callback item accounting underflow"))?;
            state.snapshot.callback_bytes.current = remaining_bytes;
            state.snapshot.callback_items.current = remaining_items;
        }
        Ok(())
    }
    #[cfg(any(test, target_arch = "wasm32"))]
    fn browser_callback_capacity(&self, bytes: usize, retain: bool) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("callback capacity metrics lock poisoned"))?;
        let capacity = &mut state.snapshot.browser_callback_vec_capacity;
        if retain {
            let current = capacity
                .current
                .checked_add(bytes)
                .ok_or_else(|| io::Error::other("callback capacity accounting overflow"))?;
            let total = capacity
                .total
                .checked_add(bytes)
                .ok_or_else(|| io::Error::other("callback capacity total overflow"))?;
            capacity.current = current;
            capacity.total = total;
            capacity.peak = capacity.peak.max(current);
        } else {
            capacity.remove(bytes)?;
        }
        Ok(())
    }
    /// Records capacity before Bytes owns the Vec; the final alias frees it.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub(crate) fn browser_callback_from_vec(self: &Arc<Self>, bytes: Vec<u8>) -> io::Result<Bytes> {
        let capacity = bytes.capacity();
        self.browser_callback_capacity(capacity, true)?;
        Ok(Bytes::from_owner(BrowserCallbackBacking {
            bytes,
            capacity,
            metrics: self.clone(),
        }))
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
    #[cfg(not(target_arch = "wasm32"))]
    use bytes::Bytes;
    #[cfg(not(target_arch = "wasm32"))]
    use std::task::Poll;
    fn frame(kind: u8, flags: u8, stream: u32) -> [u8; 9] {
        let id = stream.to_be_bytes();
        [0, 0, 0, kind, flags, id[0], id[1], id[2], id[3]]
    }
    #[test]
    fn browser_callback_capacity_survives_partial_reads_and_last_alias() {
        let metrics = Arc::new(ConnectionMetrics::new(true));
        let mut copied = Vec::with_capacity(128);
        copied.extend([7; 48]);
        let mut queued = metrics.browser_callback_from_vec(copied).unwrap();
        metrics.queue(queued.len(), true, false).unwrap();
        let first_read = queued.split_to(16);
        metrics.queue(first_read.len(), false, false).unwrap();
        let synchronous_alias = queued.clone();
        assert_eq!(metrics.snapshot().unwrap().callback_bytes.current, 32);
        assert_eq!(
            metrics
                .snapshot()
                .unwrap()
                .browser_callback_vec_capacity
                .current,
            128
        );
        drop(first_read);
        metrics.queue(queued.len(), false, true).unwrap();
        drop(queued);
        assert_eq!(metrics.snapshot().unwrap().callback_bytes.current, 0);
        assert_eq!(
            metrics
                .snapshot()
                .unwrap()
                .browser_callback_vec_capacity
                .current,
            128
        );
        metrics.close();
        assert_eq!(
            metrics
                .snapshot()
                .unwrap()
                .browser_callback_vec_capacity
                .current,
            128
        );
        drop(synchronous_alias);
        let settled = metrics.snapshot().unwrap();
        assert_eq!(settled.browser_callback_vec_capacity.current, 0);
        assert_eq!(settled.browser_callback_vec_capacity.peak, 128);
        assert_eq!(settled.browser_callback_vec_capacity.total, 128);
        assert_eq!(settled.callback_items.current, 0);
    }
    #[test]
    fn browser_callback_failure_drop_releases_only_owned_backings() {
        let metrics = Arc::new(ConnectionMetrics::new(true));
        let mut first = Vec::with_capacity(80);
        first.extend([1; 20]);
        let mut second = Vec::with_capacity(96);
        second.extend([2; 24]);
        let first = metrics.browser_callback_from_vec(first).unwrap();
        let second = metrics.browser_callback_from_vec(second).unwrap();
        metrics.queue(first.len(), true, false).unwrap();
        metrics.queue(second.len(), true, false).unwrap();
        assert_eq!(
            metrics
                .snapshot()
                .unwrap()
                .browser_callback_vec_capacity
                .current,
            176
        );
        metrics.reject();
        metrics.queue(first.len(), false, true).unwrap();
        drop(first);
        assert_eq!(
            metrics
                .snapshot()
                .unwrap()
                .browser_callback_vec_capacity
                .current,
            96
        );
        metrics.queue(second.len(), false, true).unwrap();
        metrics.close();
        assert_eq!(
            metrics
                .snapshot()
                .unwrap()
                .browser_callback_vec_capacity
                .current,
            96
        );
        drop(second);
        let settled = metrics.snapshot().unwrap();
        assert!(settled.rejected && settled.closed);
        assert_eq!(settled.browser_callback_vec_capacity.current, 0);
        assert_eq!(settled.browser_callback_vec_capacity.peak, 176);
        assert_eq!(settled.callback_bytes.current, 0);
        assert_eq!(settled.callback_items.current, 0);
    }
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
    #[test]
    fn rolling_control_window_counts_mixed_streams_ack_and_split_headers() {
        let metrics = ConnectionMetrics::new(true);
        let start = Instant::now();
        let kinds = [2, 3, 4, 5, 6, 7, 8, 10];
        for index in 0..100 {
            let wire = frame(
                kinds[index % kinds.len()],
                if index % 2 == 0 { 1 } else { 0 },
                index as u32,
            );
            for byte in wire {
                metrics.observe_at(true, &[byte], start).unwrap();
            }
        }
        let snapshot = metrics.snapshot().unwrap();
        assert_eq!(snapshot.received_control_frames, 100);
        assert_eq!(snapshot.incoming_control_window.len(), 100);
        assert!(!snapshot.rejected);
        let error = metrics
            .observe_at(true, &frame(8, 0, 0), start)
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        let snapshot = metrics.snapshot().unwrap();
        assert_eq!(snapshot.received_control_frames, 100);
        assert_eq!(snapshot.received_frames, 101);
        assert_eq!(snapshot.rejected_control.unwrap().kind, 8);
        assert!(snapshot.rejected);
    }
    #[test]
    fn control_window_expires_at_exact_second_not_at_fixed_bucket_boundary() {
        let metrics = ConnectionMetrics::new(true);
        let start = Instant::now();
        for _ in 0..99 {
            metrics.observe_at(true, &frame(4, 1, 0), start).unwrap();
        }
        metrics
            .observe_at(true, &frame(6, 1, 0), start + Duration::from_millis(500))
            .unwrap();
        metrics
            .observe_at(true, &frame(8, 0, 1), start + Duration::from_secs(1))
            .unwrap();
        assert_eq!(metrics.snapshot().unwrap().incoming_control_window.len(), 2);
        for _ in 0..98 {
            metrics
                .observe_at(true, &frame(4, 1, 0), start + Duration::from_secs(1))
                .unwrap();
        }
        assert!(
            metrics
                .observe_at(true, &frame(4, 1, 0), start + Duration::from_secs(1))
                .is_err()
        );
    }
    #[test]
    fn noncontrols_do_not_consume_quota_and_connections_are_independent() {
        let first = ConnectionMetrics::new(true);
        let second = ConnectionMetrics::new(true);
        let now = Instant::now();
        for kind in [0, 1, 9] {
            for _ in 0..100 {
                first.observe_at(true, &frame(kind, 0, 1), now).unwrap();
            }
        }
        for _ in 0..100 {
            first.observe_at(true, &frame(4, 1, 0), now).unwrap();
        }
        second.observe_at(true, &frame(4, 1, 0), now).unwrap();
        assert_eq!(second.snapshot().unwrap().received_control_frames, 1);
        assert!(first.observe_at(true, &frame(10, 0, 3), now).is_err());
    }
    #[test]
    fn outgoing_controls_do_not_consume_incoming_quota() {
        let metrics = ConnectionMetrics::new(false);
        let now = Instant::now();
        for _ in 0..101 {
            metrics.observe_at(false, &frame(6, 1, 0), now).unwrap();
        }
        metrics
            .observe_at(true, b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n", now)
            .unwrap();
        metrics.observe_at(true, &frame(4, 0, 0), now).unwrap();
        let snapshot = metrics.snapshot().unwrap();
        assert_eq!(snapshot.sent_control_frames, 101);
        assert_eq!(snapshot.received_control_frames, 1);
        assert!(!snapshot.rejected);
    }
    #[test]
    fn coarse_all_frame_guard_still_rejects_data_only_flood() {
        let metrics = ConnectionMetrics::new(true);
        let now = Instant::now();
        for _ in 0..2048 {
            metrics.observe_at(true, &frame(0, 0, 1), now).unwrap();
        }
        let error = metrics.observe_at(true, &frame(0, 0, 1), now).unwrap_err();
        assert_eq!(error.to_string(), "HTTP2 frame flood bound exceeded");
        assert_eq!(metrics.snapshot().unwrap().received_control_frames, 0);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn real_h2_pending_flush_recovers_partial_window_without_end_stream() {
        pending_progress_for_window(65_535).await;
        pending_progress_for_window(MESSAGE_BYTES).await;
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn pending_progress_for_window(window: usize) {
        use tokio::{io::duplex, sync::oneshot, time::timeout};

        let (client_io, server_io) = duplex(RECEIVE_BYTES);
        let (first_sent, first_seen) = oneshot::channel();
        let (first_consumed, first_released) = oneshot::channel();
        let (second_sent, second_seen) = oneshot::channel();
        let server = tokio::spawn(async move {
            let mut connection = h2::server::handshake(server_io).await.unwrap();
            let (_, mut response) = connection.accept().await.unwrap().unwrap();
            let mut send = response
                .send_response(http::Response::new(()), false)
                .unwrap();
            let producer = tokio::spawn(async move {
                async fn send_bytes(send: &mut h2::SendStream<Bytes>, mut count: usize) {
                    while count != 0 {
                        let requested = count.min(FRAME_BYTES);
                        send.reserve_capacity(requested);
                        let capacity = futures::future::poll_fn(|cx| send.poll_capacity(cx))
                            .await
                            .unwrap()
                            .unwrap();
                        let size = requested.min(capacity);
                        if size != 0 {
                            send.send_data(Bytes::from(vec![7; size]), false).unwrap();
                            count -= size;
                        }
                    }
                }
                send_bytes(&mut send, 8 * 1024).await;
                first_sent.send(()).unwrap();
                first_released.await.unwrap();
                send_bytes(&mut send, window - 8 * 1024).await;
                second_sent.send(()).unwrap();
                send_bytes(&mut send, 1).await;
                send.send_data(Bytes::new(), true).unwrap();
            });
            while let Some(result) = connection.accept().await {
                result.unwrap();
            }
            producer.await.unwrap();
        });

        let (mut request, connection) = h2::client::Builder::new()
            .initial_window_size(window as u32)
            .initial_connection_window_size(RECEIVE_BYTES as u32)
            .handshake::<_, Bytes>(client_io)
            .await
            .unwrap();
        let driver = tokio::spawn(connection);
        let (response, upload) = request.send_request(http::Request::new(()), false).unwrap();
        let mut receive = response.await.unwrap().into_body();
        let credit = ReceiveCredit::new(Arc::new(ConnectionMetrics::new(true)));
        let id = credit.register(receive.flow_control().clone()).unwrap();
        timeout(Duration::from_secs(3), first_seen)
            .await
            .unwrap()
            .unwrap();
        let first = timeout(Duration::from_secs(3), receive.data())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(first.len(), 8 * 1024);
        credit.consumed(id, first.len()).unwrap();
        credit.pending(id).unwrap();
        first_consumed.send(()).unwrap();
        timeout(Duration::from_secs(3), second_seen)
            .await
            .unwrap()
            .unwrap();

        // The peer's wire window is exhausted, though h2 reports locally
        // available credit from the earlier 8 KiB release.
        let mut received = 0;
        while received < window - 8 * 1024 {
            let bytes = timeout(Duration::from_secs(3), receive.data())
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            received += bytes.len();
            credit.consumed(id, bytes.len()).unwrap();
        }
        assert_eq!(received, window - 8 * 1024);
        assert!(receive.flow_control().available_capacity() > 0);
        let final_byte = timeout(
            Duration::from_secs(3),
            futures::future::poll_fn(|cx| match receive.poll_data(cx) {
                Poll::Pending => {
                    credit.pending(id).unwrap();
                    Poll::Pending
                }
                ready => ready,
            }),
        )
        .await
        .unwrap()
        .unwrap()
        .unwrap();
        assert_eq!(final_byte.len(), 1);
        credit.consumed(id, 1).unwrap();
        credit.finish(id).unwrap();
        assert_eq!(credit.active(), 0);
        assert_eq!(receive.flow_control().used_capacity(), 0);
        // A still-live upload reference does not retain consumed receive credit.
        drop(upload);
        drop(receive);
        driver.abort();
        server.abort();
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn real_h2_aggregate_credit_and_slot_reuse_are_bounded() {
        use tokio::{io::duplex, time::timeout};

        let (client_io, server_io) = duplex(RECEIVE_BYTES);
        let server = tokio::spawn(async move {
            let mut connection = h2::server::Builder::new()
                .max_concurrent_streams(9)
                .handshake::<_, Bytes>(server_io)
                .await
                .unwrap();
            while let Some(request) = connection.accept().await {
                let (_, mut response) = request.unwrap();
                let mut send = response
                    .send_response(http::Response::new(()), false)
                    .unwrap();
                tokio::spawn(async move {
                    let mut remaining = 64 * 1024;
                    while remaining != 0 {
                        send.reserve_capacity(FRAME_BYTES);
                        let capacity = futures::future::poll_fn(|cx| send.poll_capacity(cx))
                            .await
                            .unwrap()
                            .unwrap();
                        let size = remaining.min(FRAME_BYTES).min(capacity);
                        if size != 0 {
                            send.send_data(Bytes::from(vec![3; size]), false).unwrap();
                            remaining -= size;
                        }
                    }
                    send.send_data(Bytes::new(), true).unwrap();
                });
            }
        });
        let (mut request, connection) = h2::client::Builder::new()
            .initial_window_size(MESSAGE_BYTES as u32)
            .initial_connection_window_size(RECEIVE_BYTES as u32)
            .initial_max_send_streams(9)
            .handshake::<_, Bytes>(client_io)
            .await
            .unwrap();
        let driver = tokio::spawn(connection);
        let credit = ReceiveCredit::new(Arc::new(ConnectionMetrics::new(true)));
        let mut bodies = Vec::new();
        for _ in 0..8 {
            let (response, _) = request.send_request(http::Request::new(()), true).unwrap();
            let mut body = timeout(Duration::from_secs(3), response)
                .await
                .unwrap()
                .unwrap()
                .into_body();
            let id = credit.register(body.flow_control().clone()).unwrap();
            bodies.push((id, body));
        }
        assert_eq!(credit.active(), 8);
        let (response, _) = request.send_request(http::Request::new(()), true).unwrap();
        let mut overflow = timeout(Duration::from_secs(3), response)
            .await
            .unwrap()
            .unwrap()
            .into_body();
        assert!(credit.register(overflow.flow_control().clone()).is_err());
        assert_eq!(credit.active(), 8);
        drop(overflow);

        for (id, body) in &mut bodies {
            let mut received = 0;
            while received < 64 * 1024 {
                let bytes = timeout(Duration::from_secs(3), body.data())
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();
                received += bytes.len();
                credit.consumed(*id, bytes.len()).unwrap();
            }
        }
        assert!(
            bodies
                .iter_mut()
                .all(|(_, body)| body.flow_control().used_capacity() == 0)
        );
        let mut retained = Vec::new();
        for (id, mut body) in bodies {
            credit.finish(id).unwrap();
            // Completed response bodies may be retained without retaining a slot.
            assert_eq!(body.flow_control().used_capacity(), 0);
            retained.push(body);
        }
        assert_eq!(credit.active(), 0);
        let (response, _) = request.send_request(http::Request::new(()), true).unwrap();
        let mut reuse = timeout(Duration::from_secs(3), response)
            .await
            .unwrap()
            .unwrap()
            .into_body();
        let id = credit.register(reuse.flow_control().clone()).unwrap();
        let used = reuse.flow_control().used_capacity();
        // Inject impossible accounting to prove a failed h2 release is explicit
        // and never substitutes a successful credit return.
        assert!(credit.consumed(id, MESSAGE_BYTES).is_err());
        assert_eq!(reuse.flow_control().used_capacity(), used);
        assert!(credit.finish(id).is_err());
        assert_eq!(credit.active(), 0);
        let (response, _) = request.send_request(http::Request::new(()), true).unwrap();
        let mut poisoned_body = timeout(Duration::from_secs(3), response)
            .await
            .unwrap()
            .unwrap()
            .into_body();
        let id = credit
            .register(poisoned_body.flow_control().clone())
            .unwrap();
        let data = timeout(Duration::from_secs(3), poisoned_body.data())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        credit.consumed(id, data.len()).unwrap();
        let used = poisoned_body.flow_control().used_capacity();
        let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = credit.slots.lock().unwrap();
            panic!("intentional coordinator poison");
        }));
        assert!(poisoned.is_err());
        // Even after a poisoned lock, terminal cleanup detaches the handle,
        // releases consumed credit, and reports a typed failure.
        assert!(credit.finish(id).is_err());
        assert_eq!(
            poisoned_body.flow_control().used_capacity(),
            used - data.len()
        );
        assert_eq!(credit.slots.lock().err().unwrap().into_inner().len(), 0);
        assert!(credit.metrics.snapshot().unwrap().rejected);
        drop(retained);
        drop(reuse);
        drop(poisoned_body);
        driver.abort();
        server.abort();
    }
}
