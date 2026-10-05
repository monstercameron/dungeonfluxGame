//! Pure narrative proposals over canonical checkpoints; the session owner commits selected state.
mod thread_progress;
mod threat_relevance;

pub use thread_progress::{
    CheckpointProgressError, CheckpointThreadProgressProposal, ProgressError, ProgressLimits,
    ThreadCheckpointRequest, ThreadConsequenceSelection, ThreadDisposition, ThreadProgressEvidence,
    stage_checkpoint_thread_progress,
};

pub use threat_relevance::{
    AdmittedThreatEvidence, ThreatRelevanceError, ThreatRelevanceLimits, ThreatRelevanceRequest,
    ThreatRelevanceSelection, select_threat_relevance,
};
