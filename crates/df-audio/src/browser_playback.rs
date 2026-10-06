//! Owned Web Audio execution for already-authorized decoded PCM.
//!
//! This adapter does not decode SpeechChunk bytes or authorize source content.
//! The mounted shell supplies its current output lease on every operation and pumps
//! owned ended events. There are no spawned tasks, timers, or detached promises.
//! Explicit dispose plus wait_closed is required before unmounting.
//! AudioContext scheduling/end facts are not evidence of audible speakers.

use crate::{AudioReceipt, PcmFormat, QueueError};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlaybackError {
    Queue(QueueError),
    InvalidTimeline,
    UnsupportedChannels,
    ResourceCapacity,
    Allocation,
    BrowserOperation,
    UnlockPending,
    UnlockRequired,
    Suspended,
    Closed,
    ClosePending,
}

impl From<QueueError> for PlaybackError {
    fn from(error: QueueError) -> Self {
        Self::Queue(error)
    }
}

/// Accepted cue ownership and the separate old stop/disconnect request outcome.
/// An error in stop_result does not undo admission or release the old source.
/// Retain the new receipt before handling that error; actual end/close still owns
/// retirement. Only errors returned before this outcome mean replacement was refused.
#[derive(Debug)]
#[must_use]
pub struct PlaybackReplacement {
    receipt: AudioReceipt,
    stop_result: Result<(), PlaybackError>,
}

impl PlaybackReplacement {
    pub fn receipt(&self) -> &AudioReceipt {
        &self.receipt
    }

    pub fn stop_result(&self) -> Result<(), PlaybackError> {
        self.stop_result
    }

    pub fn into_parts(self) -> (AudioReceipt, Result<(), PlaybackError>) {
        (self.receipt, self.stop_result)
    }
}

/// One immutable AudioContext time anchor. Frame offsets never use chunk arrival time.
#[derive(Clone, Copy, Debug)]
pub struct PlaybackTimeline {
    first_frame: u64,
    start_seconds: f64,
    format: PcmFormat,
}

impl PlaybackTimeline {
    pub fn new(
        first_frame: u64,
        start_seconds: f64,
        format: PcmFormat,
    ) -> Result<Self, PlaybackError> {
        if !start_seconds.is_finite() || start_seconds < 0.0 {
            return Err(PlaybackError::InvalidTimeline);
        }
        if !(1..=2).contains(&format.channels()) {
            return Err(PlaybackError::UnsupportedChannels);
        }
        Ok(Self {
            first_frame,
            start_seconds,
            format,
        })
    }

    pub fn start_seconds(self, offset_frames: u64) -> Result<f64, PlaybackError> {
        let relative = offset_frames
            .checked_sub(self.first_frame)
            .filter(|relative| *relative <= (1_u64 << 53))
            .ok_or(PlaybackError::InvalidTimeline)?;
        let seconds = self.start_seconds + relative as f64 / f64::from(self.format.sample_rate());
        if !seconds.is_finite() {
            return Err(PlaybackError::InvalidTimeline);
        }
        Ok(seconds)
    }
}

/// Relative start of one cue on its immutable AudioContext timeline.
/// The adapter validates the finite lead before retiring existing work.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaybackStart {
    pub first_frame: u64,
    pub lead_seconds: f64,
}

/// Includes queue PCM, one WebAudio buffer copy and one channel scratch allocation.
/// This is a selected finite bound, not a measured supported-device capacity.
#[derive(Clone, Copy, Debug)]
pub struct PlaybackLimits {
    pub total_sample_bytes: usize,
}

impl PlaybackLimits {
    pub fn required_sample_bytes(
        self,
        queue_bytes: usize,
        dispatched_bytes: usize,
        format: PcmFormat,
    ) -> Result<usize, PlaybackError> {
        if self.total_sample_bytes == 0 || self.total_sample_bytes > 32 * 1024 * 1024 {
            return Err(PlaybackError::ResourceCapacity);
        }
        if !(1..=2).contains(&format.channels()) {
            return Err(PlaybackError::UnsupportedChannels);
        }
        let required = queue_bytes
            .checked_add(dispatched_bytes)
            .and_then(|bytes| bytes.checked_add(dispatched_bytes / usize::from(format.channels())))
            .filter(|bytes| *bytes <= self.total_sample_bytes)
            .ok_or(PlaybackError::ResourceCapacity)?;
        Ok(required)
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use df_assets::AssetManifest;
    use df_media::speech::{SpeechIdentity, SpeechStopped};
    use df_model::checkpoint::{AssetReference, AudioOutputLease, Basis};
    use wasm_bindgen::{JsCast, closure::Closure};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{
        AudioBuffer, AudioBufferSourceNode, AudioContext, AudioContextState,
        AudioScheduledSourceNode, Event,
    };

    use super::{
        PlaybackError, PlaybackLimits, PlaybackReplacement, PlaybackStart, PlaybackTimeline,
    };
    use crate::{
        AudioQueue, AudioReceipt, BufferCompletion, BufferTicket, EnqueueRefusal, PcmBuffer,
        PcmFormat, QueueError, QueueSnapshot, QueueState,
    };

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Terminal {
        Pending,
        EndedCurrent,
        EndedRetired,
    }

    struct Source {
        node: AudioBufferSourceNode,
        _buffer: AudioBuffer,
        ticket: BufferTicket,
        _ended: Closure<dyn FnMut(Event)>,
        terminal: Rc<Cell<Terminal>>,
        stopped: bool,
        started: bool,
        start_seconds: f64,
        copied_bytes: usize,
    }

    impl Drop for Source {
        fn drop(&mut self) {
            // A JS property must not outlive its Rust closure, including failure/unwind paths.
            let scheduled: &AudioScheduledSourceNode = self.node.as_ref();
            scheduled.set_onended(None);
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum PlaybackState {
        UnlockRequired,
        UnlockPending,
        Ready,
        Scheduled,
        Suspended,
        Closing,
        Closed,
    }

    #[derive(Clone, Copy, Debug)]
    pub struct PlaybackSnapshot {
        pub state: PlaybackState,
        pub queue: QueueSnapshot,
        pub sources: usize,
        pub ended_callbacks: usize,
        pub pending_promises: usize,
        pub retained_sample_bytes: usize,
        pub peak_sample_bytes: usize,
        pub completed_buffers: u64,
        pub stopped_buffers: u64,
        pub scheduled_start_seconds: Option<f64>,
        pub scheduling_lateness_seconds: f64,
        pub unlock_refused: bool,
    }

    /// One mounted scope owns one context, queue, source and ended callback.
    /// Stop retains its exact source/PCM until real ended or successful context close.
    /// Because AudioQueue permits one dispatch, a tail is scheduled only after end;
    /// lateness is reported explicitly and gapless multi-buffer streaming is not claimed.
    pub struct BrowserPlayback {
        context: AudioContext,
        queue: AudioQueue,
        output: AudioOutputLease,
        current_output: Rc<RefCell<Option<AudioOutputLease>>>,
        limits: PlaybackLimits,
        timeline: Option<PlaybackTimeline>,
        receipt: Option<AudioReceipt>,
        source: Option<Source>,
        unlock: Option<JsFuture>,
        closing: Option<JsFuture>,
        unlocked: bool,
        unlock_refused: bool,
        disposed: bool,
        peak_sample_bytes: usize,
        completed_buffers: u64,
        stopped_buffers: u64,
        scheduling_lateness_seconds: f64,
    }

    impl BrowserPlayback {
        pub fn new(
            queue: AudioQueue,
            output: AudioOutputLease,
            limits: PlaybackLimits,
        ) -> Result<Self, PlaybackError> {
            if queue.snapshot().state != QueueState::Idle {
                return Err(PlaybackError::Queue(QueueError::Busy));
            }
            limits.required_sample_bytes(
                queue.snapshot().sample_bytes,
                0,
                PcmFormat::new(1, 1)?,
            )?;
            let context = AudioContext::new().map_err(|_| PlaybackError::BrowserOperation)?;
            Ok(Self {
                context,
                queue,
                current_output: Rc::new(RefCell::new(Some(output.clone()))),
                output,
                limits,
                timeline: None,
                receipt: None,
                source: None,
                unlock: None,
                closing: None,
                unlocked: false,
                unlock_refused: false,
                disposed: false,
                peak_sample_bytes: 0,
                completed_buffers: 0,
                stopped_buffers: 0,
                scheduling_lateness_seconds: 0.0,
            })
        }

        /// Call synchronously inside a real user gesture. A running browser context
        /// alone is not substituted for the explicit local unlock operation.
        pub fn request_unlock(&mut self, lease: &AudioOutputLease) -> Result<(), PlaybackError> {
            self.check_scope(lease)?;
            if self.unlock.is_some() {
                return Err(PlaybackError::UnlockPending);
            }
            let promise = self
                .context
                .resume()
                .map_err(|_| PlaybackError::BrowserOperation)?;
            self.unlocked = false;
            self.unlock = Some(JsFuture::from(promise));
            Ok(())
        }

        /// The future stays in this owner if a caller drops its wait future.
        pub async fn wait_unlocked(&mut self) -> Result<(), PlaybackError> {
            let result = self
                .unlock
                .as_mut()
                .ok_or(PlaybackError::UnlockRequired)?
                .await;
            self.unlock = None;
            if result.is_err() {
                self.unlock_refused = true;
                return Err(PlaybackError::BrowserOperation);
            }
            if self.disposed || self.context.state() == AudioContextState::Closed {
                return Err(PlaybackError::Closed);
            }
            if self.context.state() != AudioContextState::Running {
                return Err(PlaybackError::Suspended);
            }
            self.unlocked = true;
            self.unlock_refused = false;
            Ok(())
        }

        pub fn replace(
            &mut self,
            lease: &AudioOutputLease,
            basis: Basis,
            identity: SpeechIdentity,
            asset: AssetReference,
            format: PcmFormat,
            start: PlaybackStart,
        ) -> Result<PlaybackReplacement, PlaybackError> {
            self.check_ready(lease)?;
            if !start.lead_seconds.is_finite() || !(0.0..=2.0).contains(&start.lead_seconds) {
                return Err(PlaybackError::InvalidTimeline);
            }
            let timeline = PlaybackTimeline::new(
                start.first_frame,
                self.context.current_time() + start.lead_seconds,
                format,
            )?;
            let replacement =
                self.queue
                    .replace(lease, basis, identity, asset, format, start.first_frame)?;
            self.receipt = Some(replacement.receipt.clone());
            self.timeline = Some(timeline);
            let stop_result = if replacement.retired.stop_required.is_some() {
                self.stop_source()
            } else {
                Ok(())
            };
            Ok(PlaybackReplacement {
                receipt: replacement.receipt,
                stop_result,
            })
        }

        /// Caller revalidates its source/cache/decode authorization immediately before
        /// admission. No encoded speech is accepted or guessed to be f32 PCM.
        pub fn enqueue(
            &mut self,
            receipt: &AudioReceipt,
            lease: &AudioOutputLease,
            manifest: AssetManifest,
            sequence: u64,
            offset_frames: u64,
            buffer: PcmBuffer,
        ) -> Result<(), EnqueueRefusal> {
            if let Err(error) = self.check_scope(lease) {
                return Err(EnqueueRefusal {
                    reason: match error {
                        PlaybackError::Queue(error) => error,
                        _ => QueueError::Disposed,
                    },
                    buffer,
                });
            }
            let prospective = self
                .queue
                .snapshot()
                .sample_bytes
                .checked_add(buffer.sample_bytes())
                .and_then(|bytes| {
                    bytes.checked_add(self.source.as_ref().map_or(0, |source| source.copied_bytes))
                });
            if prospective.is_none_or(|bytes| bytes > self.limits.total_sample_bytes) {
                return Err(EnqueueRefusal {
                    reason: QueueError::SampleCapacity,
                    buffer,
                });
            }
            self.queue
                .enqueue(receipt, lease, manifest, sequence, offset_frames, buffer)
        }

        pub fn close(
            &mut self,
            receipt: &AudioReceipt,
            lease: &AudioOutputLease,
            chunk_count: u64,
            end_frame: u64,
        ) -> Result<QueueState, PlaybackError> {
            self.check_scope(lease)?;
            Ok(self.queue.close(receipt, lease, chunk_count, end_frame)?)
        }

        /// Apply one actual callback and then schedule at most one current buffer.
        /// The shell owns pump cadence; duplicate/stale source flags cannot release
        /// a newer dispatch because each source owns a distinct flag and exact ticket.
        pub fn pump(&mut self, lease: &AudioOutputLease) -> Result<(), PlaybackError> {
            self.check_scope(lease)?;
            self.collect_ended()?;
            self.check_ready(lease)?;
            if self.source.is_some() {
                return Ok(());
            }
            let receipt = self
                .receipt
                .clone()
                .ok_or(PlaybackError::Queue(QueueError::StaleReceipt))?;
            if matches!(
                self.queue.snapshot().state,
                QueueState::Drained | QueueState::Cancelled
            ) {
                return Ok(());
            }
            let Some(ticket) = self.queue.begin(&receipt, lease)? else {
                return Ok(());
            };
            if let Err(error) = self.schedule(ticket.clone()) {
                // A failed creation/start cancels this cue. If disconnect itself
                // failed, retain the exact allocated source until close succeeds.
                let cancellation = self.queue.cancel(&receipt, lease)?;
                if self.source.is_some() {
                    self.stop_source()?;
                } else if let Some(stop) = cancellation.stop_required {
                    self.queue.confirm_stopped(&stop)?;
                }
                return Err(error);
            }
            Ok(())
        }

        fn schedule(&mut self, ticket: BufferTicket) -> Result<(), PlaybackError> {
            let view = self.queue.dispatched(&ticket)?;
            let format = view.buffer.format();
            let required = self.limits.required_sample_bytes(
                self.queue.snapshot().sample_bytes,
                view.buffer.sample_bytes(),
                format,
            )?;
            let start_seconds = self
                .timeline
                .ok_or(PlaybackError::InvalidTimeline)?
                .start_seconds(view.offset_frames)?;
            let frames =
                u32::try_from(view.buffer.frames()).map_err(|_| PlaybackError::ResourceCapacity)?;
            let mut channel = Vec::new();
            channel
                .try_reserve_exact(frames as usize)
                .map_err(|_| PlaybackError::Allocation)?;
            let scratch_bytes = channel
                .capacity()
                .checked_mul(size_of::<f32>())
                .ok_or(PlaybackError::ResourceCapacity)?;
            let actual_required = self
                .queue
                .snapshot()
                .sample_bytes
                .checked_add(view.buffer.sample_bytes())
                .and_then(|bytes| bytes.checked_add(scratch_bytes))
                .filter(|bytes| *bytes <= self.limits.total_sample_bytes)
                .ok_or(PlaybackError::ResourceCapacity)?;
            self.peak_sample_bytes = self.peak_sample_bytes.max(required).max(actual_required);
            channel.resize(frames as usize, 0.0);
            let buffer = self
                .context
                .create_buffer(
                    u32::from(format.channels()),
                    frames,
                    format.sample_rate() as f32,
                )
                .map_err(|_| PlaybackError::BrowserOperation)?;
            for channel_number in 0..format.channels() {
                for (output, frame) in channel.iter_mut().zip(
                    view.buffer
                        .samples()
                        .chunks_exact(usize::from(format.channels())),
                ) {
                    *output = *frame
                        .get(usize::from(channel_number))
                        .ok_or(PlaybackError::UnsupportedChannels)?;
                }
                buffer
                    .copy_to_channel(&channel, i32::from(channel_number))
                    .map_err(|_| PlaybackError::BrowserOperation)?;
            }
            let node = self
                .context
                .create_buffer_source()
                .map_err(|_| PlaybackError::BrowserOperation)?;
            node.set_buffer(Some(&buffer));
            let terminal = Rc::new(Cell::new(Terminal::Pending));
            let ended_flag = Rc::clone(&terminal);
            let current = Rc::clone(&self.current_output);
            let admitted_output = self.output.clone();
            let ended = Closure::wrap(Box::new(move |_event: Event| {
                let current = current.borrow();
                ended_flag.set(if current.as_ref() == Some(&admitted_output) {
                    Terminal::EndedCurrent
                } else {
                    Terminal::EndedRetired
                });
            }) as Box<dyn FnMut(Event)>);
            let scheduled: &AudioScheduledSourceNode = node.as_ref();
            scheduled.set_onended(Some(ended.as_ref().unchecked_ref()));
            if node
                .connect_with_audio_node(&self.context.destination())
                .is_err()
            {
                scheduled.set_onended(None);
                return Err(PlaybackError::BrowserOperation);
            }
            self.source = Some(Source {
                node,
                _buffer: buffer,
                ticket,
                _ended: ended,
                terminal,
                stopped: false,
                started: false,
                start_seconds,
                copied_bytes: view.buffer.sample_bytes(),
            });
            let source = self
                .source
                .as_mut()
                .ok_or(PlaybackError::BrowserOperation)?;
            if source.node.start_with_when(start_seconds).is_err() {
                source
                    .node
                    .disconnect()
                    .map_err(|_| PlaybackError::BrowserOperation)?;
                self.source = None;
                return Err(PlaybackError::BrowserOperation);
            }
            source.started = true;
            self.scheduling_lateness_seconds =
                (self.context.current_time() - start_seconds).max(0.0);
            Ok(())
        }

        fn collect_ended(&mut self) -> Result<(), PlaybackError> {
            let Some(source) = &self.source else {
                return Ok(());
            };
            if source.terminal.get() == Terminal::Pending {
                return Ok(());
            }
            source
                .node
                .disconnect()
                .map_err(|_| PlaybackError::BrowserOperation)?;
            let retired = source.stopped || source.terminal.get() == Terminal::EndedRetired;
            let completion = if retired {
                self.queue.confirm_stopped(&source.ticket)?
            } else {
                self.queue.complete(&source.ticket)?
            };
            match completion {
                BufferCompletion::Current(_) => self.completed_buffers += 1,
                BufferCompletion::Retired(_) => self.stopped_buffers += 1,
            }
            source.node.set_buffer(None);
            self.source = None;
            Ok(())
        }

        fn stop_source(&mut self) -> Result<(), PlaybackError> {
            if let Some(source) = &mut self.source {
                if source.started && !source.stopped {
                    let scheduled: &AudioScheduledSourceNode = source.node.as_ref();
                    scheduled
                        .stop()
                        .map_err(|_| PlaybackError::BrowserOperation)?;
                }
                source.stopped = true;
                source
                    .node
                    .disconnect()
                    .map_err(|_| PlaybackError::BrowserOperation)?;
            }
            Ok(())
        }

        pub fn cancel(
            &mut self,
            receipt: &AudioReceipt,
            lease: &AudioOutputLease,
        ) -> Result<(), PlaybackError> {
            self.check_scope(lease)?;
            self.queue.cancel(receipt, lease)?;
            self.stop_source()
        }

        /// The media event owner retains the AudioReceipt returned by replace.
        /// Never select this player's newer current receipt for a delayed stop event.
        pub fn cancel_media(
            &mut self,
            receipt: &AudioReceipt,
            lease: &AudioOutputLease,
            stopped: &SpeechStopped,
        ) -> Result<(), PlaybackError> {
            self.check_scope(lease)?;
            self.queue.cancel_media(receipt, lease, stopped)?;
            self.stop_source()
        }

        /// Revocation is permanent for this context/queue owner. A new lease needs
        /// a new owner; late old ended events can only retire the old dispatch.
        pub fn revoke(&mut self) -> Result<(), PlaybackError> {
            *self.current_output.borrow_mut() = None;
            self.dispose()
        }

        /// Admission closes synchronously even if stop or close fails. The exact
        /// old source remains counted until ended or successful close resolution.
        pub fn dispose(&mut self) -> Result<(), PlaybackError> {
            self.disposed = true;
            *self.current_output.borrow_mut() = None;
            self.queue.dispose();
            let stop_result = self.stop_source();
            if self.closing.is_none() && self.context.state() != AudioContextState::Closed {
                let promise = self
                    .context
                    .close()
                    .map_err(|_| PlaybackError::BrowserOperation)?;
                self.closing = Some(JsFuture::from(promise));
            }
            stop_result
        }

        pub async fn wait_closed(&mut self) -> Result<(), PlaybackError> {
            if !self.disposed {
                return Err(PlaybackError::ClosePending);
            }
            if let Some(closing) = self.closing.as_mut() {
                let result = closing.await;
                self.closing = None;
                result.map_err(|_| PlaybackError::BrowserOperation)?;
            }
            if self.context.state() != AudioContextState::Closed {
                return Err(PlaybackError::ClosePending);
            }
            if let Some(source) = &self.source {
                source
                    .node
                    .disconnect()
                    .map_err(|_| PlaybackError::BrowserOperation)?;
                self.queue.confirm_stopped(&source.ticket)?;
                self.stopped_buffers += 1;
                source.node.set_buffer(None);
                self.source = None;
            }
            // Closing settles a previously requested resume, but its observer remains
            // owned until awaited; do not silently drop pending callback ownership.
            if self.unlock.is_some() {
                match self.wait_unlocked().await {
                    Ok(()) | Err(PlaybackError::Closed) => {}
                    Err(PlaybackError::BrowserOperation) => {
                        // A resume rejection caused by closing is observable separately.
                        self.unlock_refused = true;
                    }
                    Err(error) => return Err(error),
                }
            }
            Ok(())
        }

        pub fn snapshot(&self) -> PlaybackSnapshot {
            let context_state = self.context.state();
            let state = if context_state == AudioContextState::Closed
                && self.closing.is_none()
                && self.unlock.is_none()
                && self.source.is_none()
                && self.queue.snapshot().state == QueueState::Disposed
            {
                PlaybackState::Closed
            } else if self.disposed {
                PlaybackState::Closing
            } else if self.unlock.is_some() {
                PlaybackState::UnlockPending
            } else if !self.unlocked {
                PlaybackState::UnlockRequired
            } else if context_state != AudioContextState::Running {
                PlaybackState::Suspended
            } else if self.source.is_some() {
                PlaybackState::Scheduled
            } else {
                PlaybackState::Ready
            };
            PlaybackSnapshot {
                state,
                queue: self.queue.snapshot(),
                sources: usize::from(self.source.is_some()),
                ended_callbacks: usize::from(self.source.is_some()),
                pending_promises: usize::from(self.unlock.is_some())
                    + usize::from(self.closing.is_some()),
                retained_sample_bytes: self.queue.snapshot().sample_bytes
                    + self.source.as_ref().map_or(0, |source| source.copied_bytes),
                peak_sample_bytes: self.peak_sample_bytes,
                completed_buffers: self.completed_buffers,
                stopped_buffers: self.stopped_buffers,
                scheduled_start_seconds: self.source.as_ref().map(|source| source.start_seconds),
                scheduling_lateness_seconds: self.scheduling_lateness_seconds,
                unlock_refused: self.unlock_refused,
            }
        }

        fn check_scope(&self, lease: &AudioOutputLease) -> Result<(), PlaybackError> {
            if self.disposed {
                return Err(PlaybackError::Closed);
            }
            if lease != &self.output || self.current_output.borrow().as_ref() != Some(lease) {
                return Err(PlaybackError::Queue(QueueError::WrongLease));
            }
            Ok(())
        }

        fn check_ready(&self, lease: &AudioOutputLease) -> Result<(), PlaybackError> {
            self.check_scope(lease)?;
            if self.unlock.is_some() {
                return Err(PlaybackError::UnlockPending);
            }
            if !self.unlocked {
                return Err(PlaybackError::UnlockRequired);
            }
            if self.context.state() != AudioContextState::Running {
                return Err(PlaybackError::Suspended);
            }
            Ok(())
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use browser::{BrowserPlayback, PlaybackSnapshot, PlaybackState};
