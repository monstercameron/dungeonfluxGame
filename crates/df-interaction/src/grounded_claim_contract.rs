//! Executable grounded-claim contract, mounted only by its qualification test.
//! This defines no provider, wire API, publication path or canonical truth writer.
use df_interaction::speech::{
    ExpressionClaim, ExpressionContext, ExpressionSlot, SpeechError, SpeechObservation,
};
use df_model::checkpoint::RecordId;

pub(crate) struct GroundedCandidate<'a> {
    pub(crate) claim: RecordId,
    pub(crate) text: &'a str,
    pub(crate) slots: &'a [ExpressionSlot<'a>],
}

#[derive(Clone, Copy)]
pub(crate) struct GroundedLimits {
    pub(crate) maximum_text_bytes: usize,
    pub(crate) maximum_slots: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum GroundingError {
    Speech(SpeechError),
    Capacity,
    ClaimUnavailable,
    TextMismatch,
    SlotMismatch,
}

/// Revalidate the real source/audience projection before consuming a claim.
/// This bounded example permits an exact source-reviewed expression, not arbitrary
/// model paraphrases. The returned text, attribution, intent and evidence are
/// borrowed from the admitted canonical context; caller text creates no fact.
/// Later consumers must call this boundary again after authority/state changes.
pub(crate) fn validate_grounded_claim<'context, 'data>(
    context: &'context ExpressionContext<'data>,
    current: &SpeechObservation<'_>,
    candidate: GroundedCandidate<'_>,
    limits: GroundedLimits,
) -> Result<&'context ExpressionClaim<'data>, GroundingError> {
    if candidate.text.len() > limits.maximum_text_bytes
        || candidate.slots.len() > limits.maximum_slots
    {
        return Err(GroundingError::Capacity);
    }
    context
        .validate_current(current)
        .map_err(GroundingError::Speech)?;
    let claim = context
        .claims()
        .iter()
        .find(|claim| claim.id == candidate.claim)
        .ok_or(GroundingError::ClaimUnavailable)?;
    if candidate.text != claim.text {
        return Err(GroundingError::TextMismatch);
    }
    if candidate.slots != claim.slots {
        return Err(GroundingError::SlotMismatch);
    }
    Ok(claim)
}
