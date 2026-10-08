//! Ephemeral dialogue input identities and dispositions; these are not commit receipts.
use crate::checkpoint::{Basis, CheckpointPins};
use df_types::{MemberId, OperationId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntentDisposition {
    Action,
    Question,
    Social,
    PlanOnly,
    Meta,
    Joke,
    Clarify,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputFinality {
    Partial,
    Final,
}

/// Declared discourse context is advisory and never grants mechanical authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscourseContext {
    Unspecified,
    Question,
    Social,
    PlanOnly,
    Meta,
    Joke,
    Clarify,
}

/// Native-captured input basis, without retaining private raw utterance text.
/// Complete pins and membership must be revalidated before explicit confirmation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawInputRef {
    pub input: OperationId,
    pub member: MemberId,
    pub basis: Basis,
    pub pins: CheckpointPins,
    pub finality: InputFinality,
}
