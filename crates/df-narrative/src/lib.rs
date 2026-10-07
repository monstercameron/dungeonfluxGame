//! Pure narrative proposals over canonical checkpoints; the session owner commits selected state.
mod beat_selection;
mod convergence;
mod thread_progress;
mod threat_relevance;

pub use convergence::{
    AdmittedConvergenceDelivery, CheckpointConvergenceProposal, CheckpointConvergenceRequest,
    ConvergenceError, ConvergenceLimits, NarrativeBudgetChange, NarrativeBudgetError,
    NarrativeBudgetLimits, NarrativeBudgetOutcome, NarrativeBudgetProposal, NarrativeBudgetRequest,
    stage_candidate_narrative_budget, stage_checkpoint_convergence,
    stage_checkpoint_narrative_budget,
};

pub use thread_progress::{
    CheckpointProgressError, CheckpointThreadProgressProposal, ProgressError, ProgressLimits,
    ThreadCheckpointRequest, ThreadConsequenceSelection, ThreadDisposition, ThreadProgressEvidence,
    stage_checkpoint_thread_progress,
};

pub use threat_relevance::{
    AdmittedThreatEvidence, ThreatRelevanceError, ThreatRelevanceLimits, ThreatRelevanceRequest,
    ThreatRelevanceSelection, select_threat_relevance,
};

pub use beat_selection::{
    AdmittedBeatAlternative, BeatCause, BeatSelectionError, BeatSelectionLimits,
    CheckpointBeatRequest, stage_checkpoint_beat_selection,
};
