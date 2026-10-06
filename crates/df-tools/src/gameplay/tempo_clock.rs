//! Trusted native presentation-time ownership; no game-time, input or cue authority.
use std::time::{Duration, Instant};

use df_model::checkpoint::{Basis, Checkpoint, CheckpointPins, ExecutionMode};
use df_tempo::elapsed::{
    TempoAdvanceError, TempoAdvanceRequest, TempoElapsedLimits, TempoElapsedPolicy, advance_elapsed,
};

// Explicitly selected native policy units. Other timebases need a separately selected
// quantization policy; this owner never rounds a supplied Instant interval.
const NANOSECOND_TICKS_PER_SECOND: u32 = 1_000_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeClockError {
    Disposed,
    UnsupportedTimebase,
    WrongSession,
    WrongRun,
    WrongEpoch,
    StaleCheckpoint,
    SourceChanged,
    AnchorConflict,
    BackwardsInstant,
    DurationOverflow,
    AnchorOverflow,
    Elapsed(TempoAdvanceError),
}

/// One serialized native actor owns this clock and supplies captured monotonic Instants.
/// The actor authenticates its checkpoint/pause source separately. None of these values
/// proves admission, current membership, policy rights or a committed decision.
///
/// Proposed anchors are not acknowledged here: only a later authoritative checkpoint may
/// advance the observed durable anchor. Failed decisions therefore do not consume elapsed
/// presentation time. An actor must resolve an uncertain commit before sampling another input.
/// Rebind is explicit recovery of this same clock lineage, never a new run/source fallback.
pub(super) struct NativePresentationClock {
    basis: Basis,
    pins: CheckpointPins,
    policy: TempoElapsedPolicy,
    limits: TempoElapsedLimits,
    last_observed: Instant,
    origin_anchor: u64,
    active_nanoseconds: u64,
    current_anchor: u64,
    proposed_anchor: u64,
    paused: bool,
    replay: bool,
    disposed: bool,
}

impl NativePresentationClock {
    /// Start at the restored durable anchor. Process downtime is not catch-up time.
    /// Policy and limits are explicitly caller-selected; there are no production defaults.
    pub(super) fn start(
        current: &Checkpoint,
        admitted: &CheckpointPins,
        policy: &TempoElapsedPolicy,
        limits: TempoElapsedLimits,
        origin: Instant,
        paused: bool,
    ) -> Result<Self, NativeClockError> {
        if policy.ticks_per_second != NANOSECOND_TICKS_PER_SECOND {
            return Err(NativeClockError::UnsupportedTimebase);
        }
        let request = request(current, Duration::ZERO, paused);
        advance_elapsed(current, admitted, request, Some(policy), limits)
            .map_err(NativeClockError::Elapsed)?;
        let anchor = current.state().tempo.presentation_ticks;
        Ok(Self {
            basis: current.basis(),
            pins: admitted.clone(),
            policy: policy.clone(),
            limits,
            last_observed: origin,
            origin_anchor: anchor,
            active_nanoseconds: 0,
            current_anchor: anchor,
            proposed_anchor: anchor,
            paused,
            replay: current.state().mode == ExecutionMode::Replay,
            disposed: false,
        })
    }

    /// Produce the existing pure-domain request from real supplied monotonic elapsed time.
    /// The immutable checkpoint remains unchanged. Any refusal preserves this clock too.
    ///
    /// Entering/leaving pause rebases at the supplied current durable anchor. Unaccepted
    /// cosmetic catch-up before that boundary is discarded explicitly; paused intervals
    /// never accrue into a later frame. Replay likewise retains its recorded anchor.
    pub(super) fn sample(
        &mut self,
        current: &Checkpoint,
        admitted: &CheckpointPins,
        observed: Instant,
        paused: bool,
    ) -> Result<TempoAdvanceRequest, NativeClockError> {
        self.validate_current(current, admitted)?;
        let interval = observed
            .checked_duration_since(self.last_observed)
            .ok_or(NativeClockError::BackwardsInstant)?;
        let replay = current.state().mode == ExecutionMode::Replay;
        let frozen = paused || self.paused != paused || self.replay || replay;
        let anchor = current.state().tempo.presentation_ticks;
        let (origin_anchor, active_nanoseconds, target) = if frozen {
            (anchor, 0, anchor)
        } else {
            let interval_nanoseconds = u64::try_from(interval.as_nanos())
                .map_err(|_| NativeClockError::DurationOverflow)?;
            let active = self
                .active_nanoseconds
                .checked_add(interval_nanoseconds)
                .ok_or(NativeClockError::DurationOverflow)?;
            let target = self
                .origin_anchor
                .checked_add(active)
                .ok_or(NativeClockError::AnchorOverflow)?;
            (self.origin_anchor, active, target)
        };
        let elapsed = target
            .checked_sub(anchor)
            .ok_or(NativeClockError::AnchorConflict)?;
        let proposed = request(current, Duration::from_nanos(elapsed), paused);
        // Reuse the actual owner of limits, exact time conversion and checkpoint/source
        // validation. Its detached proposal is not a commit or a public presentation plan.
        advance_elapsed(current, admitted, proposed, Some(&self.policy), self.limits)
            .map_err(NativeClockError::Elapsed)?;
        self.basis = current.basis();
        self.last_observed = observed;
        self.origin_anchor = origin_anchor;
        self.active_nanoseconds = active_nanoseconds;
        self.current_anchor = anchor;
        self.proposed_anchor = target;
        self.paused = paused;
        self.replay = replay;
        Ok(proposed)
    }

    /// Explicitly discard unaccepted cosmetic catch-up after native recovery/pause rebinding.
    /// Current acknowledged or last proposed anchor is required; identity/pins cannot change.
    /// A new run/epoch/source requires disposal and a separately authorized fresh owner.
    pub(super) fn rebind(
        &mut self,
        current: &Checkpoint,
        admitted: &CheckpointPins,
        observed: Instant,
        paused: bool,
    ) -> Result<(), NativeClockError> {
        self.validate_current(current, admitted)?;
        observed
            .checked_duration_since(self.last_observed)
            .ok_or(NativeClockError::BackwardsInstant)?;
        advance_elapsed(
            current,
            admitted,
            request(current, Duration::ZERO, paused),
            Some(&self.policy),
            self.limits,
        )
        .map_err(NativeClockError::Elapsed)?;
        let anchor = current.state().tempo.presentation_ticks;
        self.basis = current.basis();
        self.last_observed = observed;
        self.origin_anchor = anchor;
        self.active_nanoseconds = 0;
        self.current_anchor = anchor;
        self.proposed_anchor = anchor;
        self.paused = paused;
        self.replay = current.state().mode == ExecutionMode::Replay;
        Ok(())
    }

    /// Terminal and idempotent: disposal cannot be undone through sample or rebind.
    pub(super) fn dispose(&mut self) {
        self.disposed = true;
    }

    fn validate_current(
        &self,
        current: &Checkpoint,
        admitted: &CheckpointPins,
    ) -> Result<(), NativeClockError> {
        if self.disposed {
            return Err(NativeClockError::Disposed);
        }
        let basis = current.basis();
        if basis.session != self.basis.session {
            return Err(NativeClockError::WrongSession);
        }
        if basis.run != self.basis.run {
            return Err(NativeClockError::WrongRun);
        }
        if basis.revision.epoch() != self.basis.revision.epoch() {
            return Err(NativeClockError::WrongEpoch);
        }
        if basis.revision < self.basis.revision {
            return Err(NativeClockError::StaleCheckpoint);
        }
        if admitted != &self.pins || current.pins() != &self.pins {
            return Err(NativeClockError::SourceChanged);
        }
        let anchor = current.state().tempo.presentation_ticks;
        if anchor != self.current_anchor && anchor != self.proposed_anchor {
            return Err(NativeClockError::AnchorConflict);
        }
        if anchor != self.current_anchor && basis.revision == self.basis.revision {
            return Err(NativeClockError::AnchorConflict);
        }
        Ok(())
    }
}

fn request(current: &Checkpoint, elapsed: Duration, paused: bool) -> TempoAdvanceRequest {
    TempoAdvanceRequest {
        expected_basis: current.basis(),
        observed_logical_time: current.state().logical_time,
        from_presentation_ticks: current.state().tempo.presentation_ticks,
        elapsed,
        paused,
    }
}

#[cfg(test)]
#[path = "tempo_clock_tests.rs"]
mod tests;
