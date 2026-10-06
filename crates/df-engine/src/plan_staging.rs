//! One currently legal head of an ephemeral bounded request list enters the semantic engine path.

use crate::command_entry::CommandEntryContext;
use crate::pending_resumption::{ResumeFence, ResumeLimits};
use crate::semantic_candidate::{
    SemanticResponseAdmission, SemanticResponseError, stage_semantic_response,
};
use df_intent::candidate::CandidateOwner;
use df_intent::plan::{PlanError, PlanLimits, validate_plan_steps};
use df_model::checkpoint::{ActualDraw, Checkpoint, GameInput, ReferenceInventory};
use df_rules::preconditions::PreconditionedCommandHandler;
use df_rules::{DispatchRegistry, RulesCommandHandler, RulesCommandInput};
use df_types::RevisionLabel;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlanHeadError<R> {
    Plan(PlanError),
    Semantic(SemanticResponseError<R>),
}

/// Native admission captured anew for this head, independently of the submitted request list.
/// These transient bounds and fences do not create a durable plan or continuation identity.
pub struct PlanHeadAdmission<'a> {
    pub owner: CandidateOwner<'a>,
    pub fence: ResumeFence,
    pub limits: ResumeLimits,
    pub plan_limits: PlanLimits,
}

/// Detached head proposal and the exact borrowed remainder, which has no mechanical approval.
/// A successful proposal is neither a commit receipt nor permission to execute the next request.
pub struct StagedPlanHead<'a> {
    checkpoint: Checkpoint,
    first: &'a GameInput,
    requires_revalidation: &'a [GameInput],
}

impl<'a> StagedPlanHead<'a> {
    pub fn checkpoint(&self) -> &Checkpoint {
        &self.checkpoint
    }

    pub fn first(&self) -> &'a GameInput {
        self.first
    }

    pub fn requires_revalidation(&self) -> &'a [GameInput] {
        self.requires_revalidation
    }

    /// Transfer the staged head to the session commit owner while preserving the borrowed tail.
    pub fn into_parts(self) -> (Checkpoint, &'a [GameInput]) {
        (self.checkpoint, self.requires_revalidation)
    }
}

impl std::fmt::Debug for StagedPlanHead<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StagedPlanHead")
            .field("basis", &self.checkpoint.basis())
            .field("revalidation_count", &self.requires_revalidation.len())
            .finish_non_exhaustive()
    }
}

/// Validate a bounded list and stage only its exact first borrowed choice/reaction/roll request.
///
/// The native session performs authenticated scoped operation lookup before calling this entry.
/// Committed, conflicting, in-progress and uncertain operations retain their session outcomes.
/// Intent validates list bounds, scope and duplicate/recorded operations, then current head
/// admission. The exact first `GameInput` and unchanged native draw slice subsequently enter
/// `stage_semantic_response`, including pending source resolution, generation/cancellation fencing
/// and source-registered dependency checks. No request is reconstructed from model output.
///
/// Only the returned checkpoint is staged. The tail is unchanged and requires a new invocation
/// against the newly committed current checkpoint, fresh native admission and the actual source
/// registry. Do not continue after failed or unknown commit; resolve it through session lookup.
/// Later refusal/cancellation does not roll back already committed progress. The native owner
/// remains responsible for authentication, confirmation, source rights and atomic publication.
/// General actions, arbitrary text and a persistent compound-action schema remain unsupported.
pub fn stage_plan_head<'a, H: RulesCommandHandler>(
    steps: &'a [GameInput],
    supplied_draws: &[ActualDraw],
    current: &Checkpoint,
    context: CommandEntryContext<'_>,
    admission: PlanHeadAdmission<'_>,
    registry: &DispatchRegistry<'_, PreconditionedCommandHandler<'_, H>>,
    selector: &RevisionLabel,
) -> Result<StagedPlanHead<'a>, PlanHeadError<H::Rejection>> {
    validate_plan_steps(
        steps,
        current,
        CandidateOwner {
            basis: admission.owner.basis,
            pins: admission.owner.pins,
            member: admission.owner.member,
            operation: admission.owner.operation,
        },
        ReferenceInventory {
            rules: context.inventory.rules,
            content: context.inventory.content,
            resources: context.inventory.resources,
            assets: context.inventory.assets,
        },
        admission.plan_limits,
    )
    .map_err(PlanHeadError::Plan)?;
    let (first, requires_revalidation) = steps
        .split_first()
        .ok_or(PlanHeadError::Plan(PlanError::EmptyPlan))?;
    let checkpoint = stage_semantic_response(
        RulesCommandInput {
            command: first,
            supplied_draws,
        },
        current,
        context,
        SemanticResponseAdmission {
            owner: admission.owner,
            fence: admission.fence,
            limits: admission.limits,
        },
        registry,
        selector,
    )
    .map_err(PlanHeadError::Semantic)?;
    Ok(StagedPlanHead {
        checkpoint,
        first,
        requires_revalidation,
    })
}
