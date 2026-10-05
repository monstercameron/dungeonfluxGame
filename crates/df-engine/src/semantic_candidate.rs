//! Admitted semantic pending responses enter the existing registered engine command path.
use crate::command_entry::CommandEntryContext;
use crate::pending_resumption::{ResumeError, ResumeFence, ResumeLimits, stage_pending_response};
use df_intent::candidate::{CandidateError, CandidateOwner, validate_semantic_candidate};
use df_model::checkpoint::{Checkpoint, ReferenceInventory};
use df_rules::preconditions::PreconditionedCommandHandler;
use df_rules::{DispatchRegistry, RulesCommandHandler, RulesCommandInput};
use df_types::RevisionLabel;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticResponseError<R> {
    Candidate(CandidateError),
    Resume(ResumeError<R>),
}

/// Native attempt admission, independent of classifier output and client observations.
/// These existing owner/fence/bound contracts are not a persisted intent or plan schema.
pub struct SemanticResponseAdmission<'a> {
    pub owner: CandidateOwner<'a>,
    pub fence: ResumeFence,
    pub limits: ResumeLimits,
}

/// Admits a typed model suggestion, then stages it through the real pending command entry.
///
/// The native session supplies the authenticated member, admitted operation and current owner
/// basis/pins independently of the suggestion. Only currently offered choice/reaction responses
/// and roll requests are supported. Speech, general actions, host commands and native completions
/// cannot become commands through this entry; actor control or ambiguous input confirmation is
/// never inferred. This operation does not classify text or implement compound action plans.
///
/// Admission delegates to Intent's canonical validator; pending source selection, generation/
/// cancellation checks and exact registered handler dispatch delegate to Engine's existing entry.
/// The registry retains its source-reviewed precondition dependencies. The supplied native draw
/// slice reaches that handler unchanged; this entry cannot invent outcomes or consume durable dice.
/// All refusals return no checkpoint and leave the current snapshot unchanged.
///
/// The session must lookup the exact scoped operation before calling this function. Already
/// committed, conflicting, in-progress or uncertain operations must retain their session outcome.
/// A successful checkpoint is detached: owner fencing, atomic commit and publication remain with
/// the durable session. No source mapping, authentication, source rights or provider qualification
/// is granted by passing these borrowed contexts.
pub fn stage_semantic_response<H: RulesCommandHandler>(
    input: RulesCommandInput<'_>,
    current: &Checkpoint,
    context: CommandEntryContext<'_>,
    admission: SemanticResponseAdmission<'_>,
    registry: &DispatchRegistry<'_, PreconditionedCommandHandler<'_, H>>,
    selector: &RevisionLabel,
) -> Result<Checkpoint, SemanticResponseError<H::Rejection>> {
    validate_semantic_candidate(
        input.command,
        current,
        admission.owner,
        ReferenceInventory {
            rules: context.inventory.rules,
            content: context.inventory.content,
            resources: context.inventory.resources,
            assets: context.inventory.assets,
        },
        context.limits.command,
    )
    .map_err(SemanticResponseError::Candidate)?;
    stage_pending_response(
        input,
        current,
        context,
        admission.fence,
        admission.limits,
        registry,
        selector,
    )
    .map_err(SemanticResponseError::Resume)
}
