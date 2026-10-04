//! Concrete consumer of I01 canonical response admission and the shared rules staging pipeline.
use crate::candidates::AdmittedCandidates;
use crate::ranking::{
    KnowledgeObserver, RankingObservation, TacticalKnowledge, UtilityAssignment,
    UtilityContribution, UtilityLimits, UtilityPolicy, UtilityRankingError, rank_tactics,
};
use df_model::checkpoint::{
    Checkpoint, CheckpointError, CommandInput, GameCommand, GameInput, OfferedResponse,
};
use df_rules::current_responses::{ResponsePreparationError, prepare_current_response};
use df_rules::preconditions::{CurrentRuleContext, PreconditionedCommandHandler};
use df_rules::{DispatchRegistry, RulesCommandHandler, RulesCommandInput};
use df_types::RevisionLabel;

/// An approved option and its approved utility within one exact admitted response.
/// The current combat policy owner supplies one option per response, using permitted
/// evidence only. This consumer never invents weights or chooses an unscored option.
pub struct ResponseUtility<'a> {
    pub response: &'a OfferedResponse,
    pub option: &'a RevisionLabel,
    pub contributions: &'a [UtilityContribution],
}

pub struct CurrentResponseRanking<'a> {
    pub template: &'a CommandInput,
    pub observation: &'a RankingObservation<'a>,
    pub assignments: &'a [ResponseUtility<'a>],
    pub policy: &'a UtilityPolicy<'a>,
    pub limits: UtilityLimits,
    pub maximum_staged_bytes: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub enum CurrentResponseRankingError<R> {
    Snapshot(CheckpointError),
    ObservationMismatch,
    UnsupportedTemplate,
    UnadmittedPolicy,
    OptionMismatch,
    Ranking(UtilityRankingError),
    Preparation(ResponsePreparationError<R>),
}

/// Server-only staged choice. It deliberately has no utility/explanation/knowledge
/// accessor, formatter or serializer. Audience projection remains the API owner's job.
/// The chosen command does not contain private belief IDs, claim text or score values.
pub struct SelectedTacticalResponse {
    input: GameInput,
    staged: Checkpoint,
}

impl SelectedTacticalResponse {
    pub fn input(&self) -> &GameInput {
        &self.input
    }

    /// Transfers the exact rules-staged proposal to the engine/session owner.
    /// Taking this checkpoint never authorizes a durable commit.
    pub fn into_staged(self) -> Checkpoint {
        self.staged
    }
}

/// Rank current supplied approved utilities, then re-stage the chosen exact option.
///
/// `admitted` must come from I01 enumerate_registered_responses, which invokes the
/// same registered source/dependency/handler pipeline before issuing its receipt.
/// Current canonical knowledge is read directly from the validated checkpoint;
/// stale detached grants/beliefs cannot be substituted. Revoked NPC known-fact or
/// belief references and member grants cause a typed failure before staging.
///
/// The owner supplies an authenticated/authorized template and observer independently
/// of utility input. Membership linkage checks attribution only and grants no actor
/// control. Entity observers must be the member's canonical linked character; a
/// server-owned autonomous NPC with no member needs its own approved native actor
/// contract, rather than an inferred member identity through this choice pipeline.
/// An empty receipt returns None. Any failure returns no selected or staged result.
/// Choice/Reaction staging explicitly borrows an empty actual-draw sequence. The
/// envelope passes unchanged into Rules; an outcome-required handler may refuse,
/// and this consumer neither fabricates outcomes nor falls back to another option.
pub fn stage_preferred_response<'a, H: RulesCommandHandler>(
    request: CurrentResponseRanking<'a>,
    admitted: &AdmittedCandidates<'a, OfferedResponse>,
    context: CurrentRuleContext<'a>,
    registry: &DispatchRegistry<'_, PreconditionedCommandHandler<'_, H>>,
    selector: &RevisionLabel,
) -> Result<Option<SelectedTacticalResponse>, CurrentResponseRankingError<H::Rejection>> {
    let current = context.checkpoint;
    current
        .validate_resume(context.basis, context.pins)
        .map_err(CurrentResponseRankingError::Snapshot)?;
    let observation = request.observation;
    if observation.basis != &context.basis
        || observation.pins != context.pins
        || observation.logical_time != current.state().logical_time
        || observation.cause != request.template.operation
    {
        return Err(CurrentResponseRankingError::ObservationMismatch);
    }
    let attributed =
        match observation.observer {
            KnowledgeObserver::Member(member) => member == request.template.member,
            KnowledgeObserver::Entity(entity) => current.state().members.iter().any(|link| {
                link.member == request.template.member && link.character == Some(entity)
            }),
        };
    if !attributed {
        return Err(CurrentResponseRankingError::ObservationMismatch);
    }
    let (resolution, window, reaction) = match &request.template.command {
        GameCommand::SelectChoice {
            resolution, window, ..
        } => (*resolution, *window, false),
        GameCommand::SelectReaction {
            resolution, window, ..
        } => (*resolution, *window, true),
        _ => return Err(CurrentResponseRankingError::UnsupportedTemplate),
    };
    if request.policy.criteria.len() > request.limits.criteria {
        return Err(CurrentResponseRankingError::Ranking(
            UtilityRankingError::PolicyCapacity,
        ));
    }
    if !context
        .inventory
        .content
        .contains(request.policy.definition)
        || request
            .policy
            .criteria
            .iter()
            .any(|criterion| !context.inventory.content.contains(criterion))
    {
        return Err(CurrentResponseRankingError::UnadmittedPolicy);
    }
    if admitted.offers().len() > request.limits.candidates {
        return Err(CurrentResponseRankingError::Ranking(
            UtilityRankingError::CandidateCapacity,
        ));
    }
    if request.assignments.len() != admitted.offers().len() {
        return Err(CurrentResponseRankingError::Ranking(
            UtilityRankingError::AssignmentMismatch,
        ));
    }
    let mut assignments = Vec::new();
    assignments
        .try_reserve_exact(request.assignments.len())
        .map_err(|_| {
            CurrentResponseRankingError::Ranking(UtilityRankingError::AllocationCapacity)
        })?;
    for (assignment, response) in request.assignments.iter().zip(admitted.offers()) {
        if !std::ptr::eq(assignment.response, *response)
            || response.participant != request.template.member
            || !response
                .options
                .iter()
                .any(|option| std::ptr::eq(option, assignment.option))
        {
            return Err(CurrentResponseRankingError::OptionMismatch);
        }
        assignments.push(UtilityAssignment {
            offer: assignment.response,
            contributions: assignment.contributions,
        });
    }
    let knowledge = TacticalKnowledge {
        basis: observation.basis,
        pins: context.pins,
        observer: observation.observer,
        grants: &current.state().knowledge,
        beliefs: &current.state().beliefs,
        npcs: &current.state().continuity.npcs,
    };
    let ranking = rank_tactics(
        observation,
        admitted,
        &assignments,
        &knowledge,
        request.policy,
        request.limits,
    )
    .map_err(CurrentResponseRankingError::Ranking)?;
    let Some(preferred) = ranking.preferred() else {
        return Ok(None);
    };
    let chosen = request
        .assignments
        .iter()
        .find(|assignment| std::ptr::eq(assignment.response, preferred.offer()))
        .ok_or(CurrentResponseRankingError::OptionMismatch)?;
    let command = if reaction {
        GameCommand::SelectReaction {
            resolution,
            window,
            offer: chosen.response.offer.clone(),
            option: chosen.option.clone(),
        }
    } else {
        GameCommand::SelectChoice {
            resolution,
            window,
            offer: chosen.response.offer.clone(),
            option: chosen.option.clone(),
        }
    };
    let input = GameInput::Game(CommandInput {
        basis: context.basis,
        observed_revision: context.basis.revision,
        operation: request.template.operation,
        member: request.template.member,
        command,
    });
    let staged = prepare_current_response(
        RulesCommandInput {
            command: &input,
            supplied_draws: &[],
        },
        context,
        registry,
        selector,
        request.maximum_staged_bytes,
    )
    .map_err(CurrentResponseRankingError::Preparation)?;
    Ok(Some(SelectedTacticalResponse { input, staged }))
}
