//! Optional macro suggestions from caller-admitted policy and current committed activity.
use std::time::Duration;

use df_model::checkpoint::{
    Basis, Checkpoint, CheckpointError, CheckpointPins, ContentReference, ExecutionMode,
    LogicalTime,
};
use df_types::MemberId;

/// Already decoded, source-admitted policy. Thresholds are caller selections, not fun measures.
/// `rest_after` measures the supplied activity window, never a mandatory gameplay deadline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PacingPolicy {
    pub definition: ContentReference,
    pub rest_after: Duration,
    pub lower_intensity_at: i64,
    pub lower_after_fatigue: u64,
}

/// Current authorized preferences supplied by the owner. IDs do not authenticate consent.
/// Macro consent is independent of spotlight consent; refusal creates no penalty.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacingPreferences {
    pub basis: Basis,
    pub member: MemberId,
    pub allow_suggestions: bool,
    pub reduced_motion: bool,
    pub quiet: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacingRequest<'a> {
    pub expected_basis: Basis,
    pub member: MemberId,
    pub expected_policy: &'a ContentReference,
    pub observed_logical_time: LogicalTime,
    pub observed_presentation_ticks: u64,
    /// Explicit owner pause decision; absence of activity never implies pause.
    pub paused: bool,
}

/// Work is bounded before traversing activity or fatigue. No defaults select production bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacingLimits {
    pub maximum_checkpoint_bytes: usize,
    pub maximum_activity_windows: usize,
    pub maximum_observed_facts: usize,
    pub maximum_fatigue_entries: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacingError {
    InvalidLimits,
    Capacity,
    ContextUnavailable,
    Binding(CheckpointError),
    LogicalTimeMismatch,
    StalePresentationAnchor,
    PolicyUnavailable,
    PolicyMismatch,
    InvalidPolicy,
    PreferencesUnavailable,
    StalePreferences,
    WrongMember,
    AmbiguousWindow,
    StaleActivityWindow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacingSuggestion {
    Rest,
    Continue,
    LowerIntensity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NoInterventionReason {
    Declined,
    NoActivity,
    Paused,
    Replay,
}

/// One owned, optional candidate. It grants no action, cue, timer, commitment or publication.
/// Consumers preserve both accessibility flags and revalidate basis/consent before delivery.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PacingRecommendation {
    pub basis: Basis,
    pub member: MemberId,
    pub policy: ContentReference,
    pub window_started: LogicalTime,
    pub window_ends: LogicalTime,
    pub observed_fact_count: usize,
    pub presentation_ticks: u64,
    pub suggestion: PacingSuggestion,
    pub reduced_motion: bool,
    pub quiet: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PacingOutcome {
    Recommendation(PacingRecommendation),
    NoIntervention {
        basis: Basis,
        reason: NoInterventionReason,
    },
}

/// Validates exact optional inputs and returns at most one macro candidate without mutation.
/// The caller admits activity/tempo to this member's audience before supplying the checkpoint;
/// this policy is not an authorization/projection service. No secret, microphone or emotional
/// inference is performed. Identical immutable inputs reproduce the candidate; durable retry,
/// current preference admission, logging and production policy decoding remain owner boundaries.
/// Replay emits no new suggestion. PreparedOnly returns data without executing it.
pub fn recommend(
    current: Option<&Checkpoint>,
    admitted_pins: &CheckpointPins,
    request: PacingRequest<'_>,
    policy: Option<&PacingPolicy>,
    preferences: Option<PacingPreferences>,
    limits: PacingLimits,
) -> Result<PacingOutcome, PacingError> {
    if limits.maximum_checkpoint_bytes == 0 {
        return Err(PacingError::InvalidLimits);
    }
    let current = current.ok_or(PacingError::ContextUnavailable)?;
    let state = current.state();
    if state.activity.len() > limits.maximum_activity_windows
        || state.tempo.fatigue.len() > limits.maximum_fatigue_entries
        || current.retained_bytes().ok_or(PacingError::Capacity)? > limits.maximum_checkpoint_bytes
    {
        return Err(PacingError::Capacity);
    }
    current
        .validate_resume(request.expected_basis, admitted_pins)
        .map_err(PacingError::Binding)?;
    if request.observed_logical_time != state.logical_time {
        return Err(PacingError::LogicalTimeMismatch);
    }
    if request.observed_presentation_ticks != state.tempo.presentation_ticks {
        return Err(PacingError::StalePresentationAnchor);
    }
    let policy = policy.ok_or(PacingError::PolicyUnavailable)?;
    if policy.definition != *request.expected_policy
        || policy.definition.package != admitted_pins.content.package
    {
        return Err(PacingError::PolicyMismatch);
    }
    if policy.rest_after.is_zero() || policy.lower_after_fatigue == 0 {
        return Err(PacingError::InvalidPolicy);
    }
    let preferences = preferences.ok_or(PacingError::PreferencesUnavailable)?;
    if preferences.basis != current.basis() {
        return Err(PacingError::StalePreferences);
    }
    if preferences.member != request.member {
        return Err(PacingError::WrongMember);
    }
    let no_intervention = |reason| PacingOutcome::NoIntervention {
        basis: current.basis(),
        reason,
    };
    if state.mode == ExecutionMode::Replay {
        return Ok(no_intervention(NoInterventionReason::Replay));
    }
    if request.paused {
        return Ok(no_intervention(NoInterventionReason::Paused));
    }
    if !preferences.allow_suggestions {
        return Ok(no_intervention(NoInterventionReason::Declined));
    }
    let mut windows = state
        .activity
        .iter()
        .filter(|window| window.member == request.member);
    let Some(window) = windows.next() else {
        return Ok(no_intervention(NoInterventionReason::NoActivity));
    };
    if windows.next().is_some() {
        return Err(PacingError::AmbiguousWindow);
    }
    if window.observed_facts.len() > limits.maximum_observed_facts {
        return Err(PacingError::Capacity);
    }
    if window.ends != state.logical_time
        || window.started.ticks_per_second != window.ends.ticks_per_second
        || window.ends.ticks_per_second == 0
        || window.started.ticks > window.ends.ticks
    {
        return Err(PacingError::StaleActivityWindow);
    }
    if window.observed_facts.is_empty() {
        return Ok(no_intervention(NoInterventionReason::NoActivity));
    }
    // Exact rational comparison avoids rounding ticks or inventing a presentation/game clock.
    let elapsed_nanos = u128::from(window.ends.ticks - window.started.ticks) * 1_000_000_000;
    let rest_nanos = policy
        .rest_after
        .as_nanos()
        .checked_mul(u128::from(window.ends.ticks_per_second))
        .ok_or(PacingError::InvalidPolicy)?;
    let suggestion = if elapsed_nanos >= rest_nanos {
        PacingSuggestion::Rest
    } else if preferences.quiet
        || state.tempo.intensity >= policy.lower_intensity_at
        || state
            .tempo
            .fatigue
            .iter()
            .any(|(_, count)| *count >= policy.lower_after_fatigue)
    {
        PacingSuggestion::LowerIntensity
    } else {
        PacingSuggestion::Continue
    };
    Ok(PacingOutcome::Recommendation(PacingRecommendation {
        basis: current.basis(),
        member: request.member,
        policy: policy.definition.clone(),
        window_started: window.started,
        window_ends: window.ends,
        observed_fact_count: window.observed_facts.len(),
        presentation_ticks: state.tempo.presentation_ticks,
        suggestion,
        reduced_motion: preferences.reduced_motion,
        quiet: preferences.quiet,
    }))
}
