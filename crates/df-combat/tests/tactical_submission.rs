use df_combat::candidates::{CandidateContext, CandidateLimits};
use df_combat::current_response::{
    CurrentResponseRanking, CurrentResponseRankingError, ResponseUtility, stage_preferred_response,
};
use df_combat::ranking::{
    KnowledgeObserver, RankingObservation, UtilityContribution, UtilityEvidence, UtilityLimits,
    UtilityPolicy, UtilityRankingError,
};
use df_combat::registered_candidates::{
    RegisteredCandidateLimits, RegisteredCandidateRequest, enumerate_registered_responses,
};
use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_engine::combat_staging::{
    TacticalResponseAdmission, TacticalResponseError, stage_tactical_response,
};
use df_engine::command_entry::{CommandEntryContext, CommandEntryLimits, CommandRejection};
use df_engine::pending_resumption::{ResumeError, ResumeFence, ResumeLimits};
use df_model::commands::CommandLimits;
use df_rules::preconditions::{
    CurrentRuleContext, PreconditionLimits, PreconditionedCommandHandler, PreconditionedRejection,
    RuleDependency, RulePreconditions,
};
use df_rules::{
    DispatchError, DispatchRegistry, HandlerRegistration, InvocationError, RulesCommandHandler,
    RulesCommandInput,
};
use std::cell::{Cell, RefCell};

include!("support/combat-I02-response-fixture.rs");

const BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureRefusal {
    Unsupported,
}

// Synthetic registered continuation only; no attack, resource cost or D&D outcome is invented.
struct Continuation {
    pins: CheckpointPins,
    policy: RevisionLabel,
    calls: Cell<usize>,
    observed: RefCell<Option<GameInput>>,
    draw_pointer: Cell<*const ActualDraw>,
    refuse: bool,
}

impl Continuation {
    fn new(policy: &str) -> Self {
        Self {
            pins: pins(),
            policy: label(policy),
            calls: Cell::new(0),
            observed: RefCell::new(None),
            draw_pointer: Cell::new(std::ptr::null()),
            refuse: false,
        }
    }
}

impl RulesCommandHandler for Continuation {
    type Rejection = FixtureRefusal;

    fn pins(&self) -> &CheckpointPins {
        &self.pins
    }

    fn stage(
        &self,
        input: RulesCommandInput<'_>,
        current: &Checkpoint,
    ) -> Result<Checkpoint, FixtureRefusal> {
        self.calls.set(self.calls.get() + 1);
        *self.observed.borrow_mut() = Some(input.command.clone());
        self.draw_pointer.set(input.supplied_draws.as_ptr());
        assert!(input.supplied_draws.is_empty());
        if self.refuse {
            return Err(FixtureRefusal::Unsupported);
        }
        let GameInput::Game(command) = input.command else {
            panic!("fixture supports canonical responses");
        };
        let mut next = current.basis();
        next.revision = next.revision.next_sequence().unwrap();
        let mut state = current.state().clone();
        state.pending.clear();
        state.decisions.push(AcceptedDecision {
            operation: command.operation,
            revision: next.revision,
            facts: vec![],
            draws: vec![],
            effects: vec![],
            source_policy: self.policy.clone(),
            semantic_output: None,
        });
        Ok(Checkpoint::new(
            CHECKPOINT_SCHEMA,
            next,
            current.pins().clone(),
            state,
            ReferenceInventory {
                rules: &[rule()],
                content: &[content()],
                resources: &resource_constraints(),
                assets: &[],
            },
            limits(),
        )
        .unwrap())
    }
}

fn waiting(reaction: bool) -> Checkpoint {
    let mut state = state();
    state.facts.push(fact(7, 0));
    state.knowledge.push(KnowledgeGrant {
        observer: member(3),
        fact: FactId::from_bytes(&[7; 16]).unwrap(),
        source: FactId::from_bytes(&[7; 16]).unwrap(),
    });
    let belief = RecordId::from_bytes(&[18; 16]).unwrap();
    state.beliefs.push(AttributedClaim {
        id: belief,
        holder: entity(4),
        subject: entity(4),
        claim: "private-tactical-belief".to_owned(),
        evidence: vec![FactId::from_bytes(&[7; 16]).unwrap()],
        audience: AudienceScope::Host,
        source: content(),
    });
    state.continuity.npcs.push(NpcState {
        entity: entity(4),
        personality: content(),
        motivations: vec![],
        known_facts: vec![],
        beliefs: vec![belief],
        secrets: vec![],
    });
    let mut pending = pending();
    let PendingInput::Reaction { remaining } = &mut pending.next else {
        panic!("fixture reaction");
    };
    let mut second = remaining.first().unwrap().clone();
    second.offer = label("fixture-offer-2");
    second.options = vec![label("fixture-option-2")];
    remaining.push(second);
    if !reaction {
        pending.next = PendingInput::Choice {
            remaining: remaining.clone(),
        };
    }
    state.pending.push(pending);
    checkpoint(state).unwrap()
}

fn command_limits() -> CommandLimits {
    CommandLimits {
        maximum_records: 16,
        maximum_text_bytes: 64,
        maximum_retained_bytes: 16384,
    }
}

#[derive(Clone, Copy)]
enum Submission {
    Selected,
    UnknownOption,
    ForeignMember,
    ForeignOperation,
    StaleGeneration,
    Cancelled,
    UnknownSelector,
}

#[derive(Debug, Eq, PartialEq)]
enum ConsumerError {
    Ranking(CurrentResponseRankingError<FixtureRefusal>),
    Engine(TacticalResponseError<FixtureRefusal>),
}

// This test composes production consumers. The preview checkpoint is deliberately discarded;
// Engine must execute its own registered continuation against the current canonical checkpoint.
fn submit_ranked(
    prepared: &Checkpoint,
    current: &Checkpoint,
    preview: &Continuation,
    final_handler: &Continuation,
    observer: KnowledgeObserver,
    evidence: UtilityEvidence,
    submission: Submission,
) -> Result<Checkpoint, ConsumerError> {
    let pending = prepared.state().pending.first().unwrap();
    let reaction = matches!(pending.next, PendingInput::Reaction { .. });
    let template = CommandInput {
        basis: prepared.basis(),
        observed_revision: prepared.basis().revision,
        operation: OperationId::from_bytes(&[20; 16]).unwrap(),
        member: member(3),
        command: if reaction {
            GameCommand::SelectReaction {
                resolution: pending.id,
                window: pending.window.id,
                offer: label("ignored-client-offer"),
                option: label("ignored-client-option"),
            }
        } else {
            GameCommand::SelectChoice {
                resolution: pending.id,
                window: pending.window.id,
                offer: label("ignored-client-offer"),
                option: label("ignored-client-option"),
            }
        },
    };
    let rules = [rule()];
    let contents = [content()];
    let resources = resource_constraints();
    let inventory = || ReferenceInventory {
        rules: &rules,
        content: &contents,
        resources: &resources,
        assets: &[],
    };
    let context = || CurrentRuleContext {
        checkpoint: prepared,
        basis: prepared.basis(),
        pins: prepared.pins(),
        inventory: inventory(),
        command_limits: command_limits(),
    };
    let dependencies = [
        RuleDependency::PendingResolution(pending.id),
        RuleDependency::Resource {
            owner: entity(4),
            resource: label("fixture-resource-1"),
        },
    ];
    let preconditions = || RulePreconditions {
        prepared,
        sources: &rules,
        dependencies: &dependencies,
    };
    let precondition_limits = PreconditionLimits {
        maximum_dependencies: 8,
        maximum_comparisons: BYTES,
        maximum_checkpoint_bytes: BYTES,
    };
    let preview_wrapper = PreconditionedCommandHandler::new(
        preview,
        &rules[0],
        context(),
        preconditions(),
        precondition_limits,
    );
    let entries = [CatalogEntry::new(&rules[0], b"synthetic continuation")];
    let catalog = || {
        CatalogSnapshot::from_published(
            &prepared.pins().rules.catalog,
            prepared.pins(),
            b"synthetic publication",
            &entries,
            CatalogLimits {
                max_complete_bytes: 128,
                max_entries: 4,
                max_item_bytes: 128,
                max_total_item_bytes: 128,
            },
        )
        .unwrap()
    };
    let registrations = [HandlerRegistration::new(
        &pending.continuation,
        &rules[0],
        &preview_wrapper,
    )];
    let registry = DispatchRegistry::from_catalog(catalog(), &registrations, 1).unwrap();
    let basis = prepared.basis();
    let admitted = enumerate_registered_responses(
        context(),
        &registry,
        RegisteredCandidateRequest {
            template: &template,
            current: CandidateContext {
                basis: &basis,
                pins: prepared.pins(),
            },
            member: template.member,
            operation: template.operation,
            selector: &pending.continuation,
        },
        RegisteredCandidateLimits {
            candidates: CandidateLimits {
                max_offers: 8,
                max_identity_comparisons: 28,
                max_candidates: 8,
                max_candidate_bytes: BYTES,
            },
            maximum_checkpoint_bytes: BYTES,
            maximum_inventory_records: 8,
            maximum_rules_preparations: 8,
            maximum_staged_bytes: BYTES,
        },
    )
    .unwrap();
    let contributions = [2, 9].map(|value| {
        [UtilityContribution {
            criterion: content(),
            evidence,
            value,
        }]
    });
    let assignments: Vec<_> = admitted
        .offers()
        .iter()
        .zip(&contributions)
        .map(|(response, contributions)| ResponseUtility {
            response,
            option: response.options.first().unwrap(),
            contributions,
        })
        .collect();
    let observation = RankingObservation {
        basis: &basis,
        pins: prepared.pins(),
        observer,
        logical_time: prepared.state().logical_time,
        cause: template.operation,
        policy: &contents[0],
    };
    let selected = stage_preferred_response(
        CurrentResponseRanking {
            template: &template,
            observation: &observation,
            assignments: &assignments,
            policy: &UtilityPolicy {
                basis: &basis,
                pins: prepared.pins(),
                definition: &contents[0],
                criteria: &contents,
            },
            limits: UtilityLimits {
                candidates: 8,
                contributions: 8,
                knowledge_records: 16,
                criteria: 4,
            },
            maximum_staged_bytes: BYTES,
        },
        &admitted,
        context(),
        &registry,
        &pending.continuation,
    )
    .map_err(ConsumerError::Ranking)?
    .unwrap();
    let mut input = selected.input().clone();
    drop(selected);
    let mut fence = ResumeFence {
        admitted_generation: 5,
        current_generation: 5,
        cancel_before_admission: false,
    };
    let mut selector = pending.continuation.clone();
    let GameInput::Game(command) = &mut input else {
        panic!("canonical selected response");
    };
    match submission {
        Submission::Selected => {}
        Submission::UnknownOption => match &mut command.command {
            GameCommand::SelectChoice { option, .. }
            | GameCommand::SelectReaction { option, .. } => *option = label("unoffered-option"),
            _ => panic!("selected response"),
        },
        Submission::ForeignMember => command.member = member(99),
        Submission::ForeignOperation => {
            command.operation = OperationId::from_bytes(&[99; 16]).unwrap();
        }
        Submission::StaleGeneration => fence.current_generation = 6,
        Submission::Cancelled => fence.cancel_before_admission = true,
        Submission::UnknownSelector => selector = label("unregistered-selector"),
    }
    let wrapper = PreconditionedCommandHandler::new(
        final_handler,
        &rules[0],
        CurrentRuleContext {
            checkpoint: current,
            basis: current.basis(),
            pins: current.pins(),
            inventory: inventory(),
            command_limits: command_limits(),
        },
        preconditions(),
        precondition_limits,
    );
    let registrations = [HandlerRegistration::new(
        &pending.continuation,
        &rules[0],
        &wrapper,
    )];
    let registry = DispatchRegistry::from_catalog(catalog(), &registrations, 1).unwrap();
    let draws: [ActualDraw; 0] = [];
    let result = stage_tactical_response(
        RulesCommandInput {
            command: &input,
            supplied_draws: &draws,
        },
        current,
        CommandEntryContext {
            current_basis: current.basis(),
            admitted_pins: current.pins(),
            inventory: inventory(),
            limits: CommandEntryLimits {
                command: command_limits(),
                maximum_staged_bytes: BYTES,
            },
        },
        TacticalResponseAdmission {
            candidates: &admitted,
            member: template.member,
            operation: template.operation,
            fence,
            limits: ResumeLimits {
                maximum_checkpoint_bytes: BYTES,
            },
        },
        &registry,
        &selector,
    )
    .map_err(ConsumerError::Engine);
    if final_handler.calls.get() > 0 {
        assert_eq!(final_handler.draw_pointer.get(), draws.as_ptr());
    }
    result
}

fn fact_evidence() -> UtilityEvidence {
    UtilityEvidence::Fact(FactId::from_bytes(&[7; 16]).unwrap())
}

#[test]
fn ranked_choice_and_reaction_reach_the_actual_engine_registered_handler() {
    for reaction in [false, true] {
        let current = waiting(reaction);
        let before = current.clone();
        let preview = Continuation::new("preview-policy");
        let final_handler = Continuation::new("engine-policy");
        let staged = submit_ranked(
            &current,
            &current,
            &preview,
            &final_handler,
            KnowledgeObserver::Member(member(3)),
            fact_evidence(),
            Submission::Selected,
        )
        .unwrap();
        assert_eq!(preview.calls.get(), 3);
        assert_eq!(final_handler.calls.get(), 1);
        let observed = final_handler.observed.borrow();
        let Some(GameInput::Game(command)) = observed.as_ref() else {
            panic!("engine observed the canonical selected command");
        };
        let expected = if reaction {
            GameCommand::SelectReaction {
                resolution: pending().id,
                window: pending().window.id,
                offer: label("fixture-offer-2"),
                option: label("fixture-option-2"),
            }
        } else {
            GameCommand::SelectChoice {
                resolution: pending().id,
                window: pending().window.id,
                offer: label("fixture-offer-2"),
                option: label("fixture-option-2"),
            }
        };
        assert_eq!(command.command, expected);
        assert_eq!(command.basis, current.basis());
        assert_eq!(
            staged.basis().revision,
            current.basis().revision.next_sequence().unwrap()
        );
        assert!(staged.state().pending.is_empty());
        assert_eq!(staged.state().decisions.len(), 1);
        assert_eq!(
            staged.state().decisions[0].source_policy,
            label("engine-policy")
        );
        assert_eq!(staged.state().resources, current.state().resources);
        assert_eq!(staged.state().draws, current.state().draws);
        assert_eq!(staged.state().facts, current.state().facts);
        assert_eq!(staged.state().logical_time, current.state().logical_time);
        assert_eq!(current, before);
    }
}

#[test]
fn invalid_and_cancelled_selected_proposals_never_invoke_the_engine_handler() {
    for (submission, expected) in [
        (
            Submission::UnknownOption,
            TacticalResponseError::UnadmittedResponse,
        ),
        (
            Submission::ForeignMember,
            TacticalResponseError::MemberMismatch,
        ),
        (
            Submission::ForeignOperation,
            TacticalResponseError::OperationMismatch,
        ),
        (
            Submission::StaleGeneration,
            TacticalResponseError::Resume(ResumeError::StaleGeneration),
        ),
        (
            Submission::Cancelled,
            TacticalResponseError::Resume(ResumeError::CancelledBeforeAdmission),
        ),
    ] {
        let current = waiting(true);
        let before = current.clone();
        let preview = Continuation::new("preview-policy");
        let final_handler = Continuation::new("engine-policy");
        assert_eq!(
            submit_ranked(
                &current,
                &current,
                &preview,
                &final_handler,
                KnowledgeObserver::Member(member(3)),
                fact_evidence(),
                submission,
            ),
            Err(ConsumerError::Engine(expected))
        );
        assert_eq!(preview.calls.get(), 3);
        assert_eq!(final_handler.calls.get(), 0);
        assert!(final_handler.observed.borrow().is_none());
        assert_eq!(current, before);
    }
}

#[test]
fn stale_revision_and_equal_detached_offers_never_invoke_the_engine_handler() {
    let prepared = waiting(false);
    let detached = prepared.clone();
    let mut changed_basis = prepared.basis();
    changed_basis.revision = changed_basis.revision.next_sequence().unwrap();
    let mut changed_state = prepared.state().clone();
    for pending in &mut changed_state.pending {
        pending.basis = changed_basis;
    }
    let changed = Checkpoint::new(
        CHECKPOINT_SCHEMA,
        changed_basis,
        prepared.pins().clone(),
        changed_state,
        ReferenceInventory {
            rules: &[rule()],
            content: &[content()],
            resources: &resource_constraints(),
            assets: &[],
        },
        limits(),
    )
    .unwrap();
    for (current, expected) in [
        (&changed, TacticalResponseError::StaleBasis),
        (&detached, TacticalResponseError::UnadmittedResponse),
    ] {
        let before = current.clone();
        let preview = Continuation::new("preview-policy");
        let final_handler = Continuation::new("engine-policy");
        assert_eq!(
            submit_ranked(
                &prepared,
                current,
                &preview,
                &final_handler,
                KnowledgeObserver::Member(member(3)),
                fact_evidence(),
                Submission::Selected,
            ),
            Err(ConsumerError::Engine(expected))
        );
        assert_eq!(final_handler.calls.get(), 0);
        assert_eq!(*current, before);
        assert_eq!(prepared, detached);
    }
}

#[test]
fn undisclosed_private_evidence_stops_selection_and_engine_submission() {
    let current = waiting(true);
    let before = current.clone();
    let preview = Continuation::new("preview-policy");
    let final_handler = Continuation::new("engine-policy");
    assert_eq!(
        submit_ranked(
            &current,
            &current,
            &preview,
            &final_handler,
            KnowledgeObserver::Member(member(3)),
            UtilityEvidence::Belief(RecordId::from_bytes(&[18; 16]).unwrap()),
            Submission::Selected,
        ),
        Err(ConsumerError::Ranking(
            CurrentResponseRankingError::Ranking(UtilityRankingError::UndisclosedEvidence)
        ))
    );
    // The two legal previews precede utility evaluation; neither selection nor Engine runs.
    assert_eq!(preview.calls.get(), 2);
    assert_eq!(final_handler.calls.get(), 0);
    assert_eq!(current, before);
}

#[test]
fn retained_npc_belief_selects_without_disclosing_private_evidence_to_engine() {
    let current = waiting(true);
    let before = current.clone();
    let preview = Continuation::new("preview-policy");
    let final_handler = Continuation::new("engine-policy");
    let staged = submit_ranked(
        &current,
        &current,
        &preview,
        &final_handler,
        KnowledgeObserver::Entity(entity(4)),
        UtilityEvidence::Belief(RecordId::from_bytes(&[18; 16]).unwrap()),
        Submission::Selected,
    )
    .unwrap();
    let observed = final_handler.observed.borrow();
    let command = format!("{:?}", observed.as_ref().unwrap());
    assert!(!command.contains("private-tactical-belief"));
    assert!(!command.contains("RecordId"));
    assert!(!command.contains("UtilityContribution"));
    assert_eq!(final_handler.calls.get(), 1);
    assert_eq!(staged.state().beliefs, current.state().beliefs);
    assert_eq!(current, before);
}

#[test]
fn missing_registered_selector_and_handler_refusal_have_no_fallback_or_state_change() {
    let current = waiting(true);
    let before = current.clone();
    let preview = Continuation::new("preview-policy");
    let final_handler = Continuation::new("engine-policy");
    let result = submit_ranked(
        &current,
        &current,
        &preview,
        &final_handler,
        KnowledgeObserver::Member(member(3)),
        fact_evidence(),
        Submission::UnknownSelector,
    );
    assert_eq!(
        result,
        Err(ConsumerError::Engine(TacticalResponseError::Resume(
            ResumeError::Command(CommandRejection::Invocation(InvocationError::Dispatch(
                DispatchError::UnknownHandler
            )))
        )))
    );
    assert_eq!(final_handler.calls.get(), 0);
    let preview = Continuation::new("preview-policy");
    let mut refusing = Continuation::new("engine-policy");
    refusing.refuse = true;
    let result = submit_ranked(
        &current,
        &current,
        &preview,
        &refusing,
        KnowledgeObserver::Member(member(3)),
        fact_evidence(),
        Submission::Selected,
    );
    assert_eq!(
        result,
        Err(ConsumerError::Engine(TacticalResponseError::Resume(
            ResumeError::Command(CommandRejection::Invocation(InvocationError::Handler(
                PreconditionedRejection::Handler(FixtureRefusal::Unsupported)
            )))
        )))
    );
    assert_eq!(refusing.calls.get(), 1);
    assert_eq!(current, before);
}
