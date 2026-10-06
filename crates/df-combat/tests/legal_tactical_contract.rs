use df_combat::candidates::{
    CandidateContext, CandidateError, CandidateLimits, LegalOfferOwner, enumerate_candidates,
};
use df_combat::ranking::{
    KnowledgeObserver, RankingObservation, TacticalKnowledge, UtilityAssignment,
    UtilityContribution, UtilityEvidence, UtilityLimits, UtilityPolicy, UtilityRankingError,
    rank_tactics,
};
use df_combat::registered_candidates::{
    RegisteredCandidateLimits, RegisteredCandidateRequest, enumerate_registered_responses,
};
use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_engine::combat_staging::{
    TacticalResponseAdmission, TacticalResponseError, stage_tactical_response,
};
use df_engine::command_entry::{CommandEntryContext, CommandEntryLimits};
use df_engine::pending_resumption::{ResumeFence, ResumeLimits};
use df_model::checkpoint::*;
use df_model::commands::CommandLimits;
use df_rules::preconditions::{
    CurrentRuleContext, PreconditionLimits, PreconditionedCommandHandler, RuleDependency,
    RulePreconditions,
};
use df_rules::{DispatchRegistry, HandlerRegistration, RulesCommandHandler, RulesCommandInput};
use df_types::OperationId;
use std::cell::Cell;

#[path = "support/registered_fixture.rs"]
mod fixture;
use fixture::*;

#[derive(Clone, Copy, Debug)]
struct Offer {
    id: &'static str,
    payload_bytes: usize,
}

struct Observation<'a> {
    offered_basis: Basis,
    authorized_ids: &'a [&'static str],
}

#[derive(Default)]
struct OfferAuthority {
    observation_calls: Cell<usize>,
    legality_calls: Cell<usize>,
}

impl LegalOfferOwner for OfferAuthority {
    type Observation = Observation<'static>;
    type Offer = Offer;
    type Error = &'static str;

    fn validate_observation(
        &self,
        observation: &Self::Observation,
        current: CandidateContext<'_>,
    ) -> Result<(), Self::Error> {
        self.observation_calls.set(self.observation_calls.get() + 1);
        if observation.offered_basis == *current.basis {
            Ok(())
        } else {
            Err("observation basis mismatch")
        }
    }

    fn is_current_legal(
        &self,
        observation: &Self::Observation,
        _: CandidateContext<'_>,
        offer: &Self::Offer,
    ) -> Result<bool, Self::Error> {
        self.legality_calls.set(self.legality_calls.get() + 1);
        Ok(observation.authorized_ids.contains(&offer.id))
    }

    fn same_offer(&self, left: &Self::Offer, right: &Self::Offer) -> Result<bool, Self::Error> {
        Ok(left.id == right.id)
    }

    fn offer_bytes(&self, offer: &Self::Offer) -> Result<usize, Self::Error> {
        Ok(offer.payload_bytes)
    }
}

fn candidate_limits() -> CandidateLimits {
    CandidateLimits {
        max_offers: 8,
        max_identity_comparisons: 28,
        max_candidates: 8,
        max_candidate_bytes: 4096,
    }
}

fn tactical_knowledge<'a>(
    basis: &'a Basis,
    pins: &'a CheckpointPins,
    observer: KnowledgeObserver,
    grants: &'a [KnowledgeGrant],
    beliefs: &'a [AttributedClaim],
    npcs: &'a [NpcState],
) -> TacticalKnowledge<'a> {
    TacticalKnowledge {
        basis,
        pins,
        observer,
        grants,
        beliefs,
        npcs,
    }
}

#[test]
fn ranking_cannot_select_an_offer_the_current_rules_owner_did_not_admit() {
    let basis = basis();
    let pins = pins();
    let observation = Observation {
        offered_basis: basis,
        authorized_ids: &["hold-position", "retreat"],
    };
    let offers = [
        Offer {
            id: "hold-position",
            payload_bytes: 8,
        },
        Offer {
            id: "invented-teleport",
            payload_bytes: 16,
        },
        Offer {
            id: "retreat",
            payload_bytes: 7,
        },
    ];
    let authority = OfferAuthority::default();
    let context = CandidateContext {
        basis: &basis,
        pins: &pins,
    };
    let admitted = enumerate_candidates(
        &authority,
        &observation,
        context,
        context,
        &offers,
        candidate_limits(),
    )
    .expect("the owner admits only its two supplied current offers");

    assert_eq!(admitted.offers().len(), 2);
    assert!(std::ptr::eq(admitted.offers()[0], &offers[0]));
    assert!(std::ptr::eq(admitted.offers()[1], &offers[2]));

    let fact_record = fact(7, 0);
    let grant = KnowledgeGrant {
        observer: member(3),
        fact: fact_record.id,
        source: fact_record.id,
    };
    let grants = [grant];
    let knowledge = tactical_knowledge(
        &basis,
        &pins,
        KnowledgeObserver::Member(member(3)),
        &grants,
        &[],
        &[],
    );
    let criterion = content();
    let criteria = [criterion.clone()];
    let policy = UtilityPolicy {
        basis: &basis,
        pins: &pins,
        definition: &criterion,
        criteria: &criteria,
    };
    let ranking_observation = RankingObservation {
        basis: &basis,
        pins: &pins,
        observer: KnowledgeObserver::Member(member(3)),
        logical_time: state().logical_time,
        cause: fact_record.operation,
        policy: &criterion,
    };
    let evidence = UtilityEvidence::Fact(fact_record.id);
    let contributions = [
        [UtilityContribution {
            criterion: criterion.clone(),
            evidence,
            value: -5,
        }],
        [UtilityContribution {
            criterion: criterion.clone(),
            evidence,
            value: 9,
        }],
    ];
    let assignments = [
        UtilityAssignment {
            offer: &offers[0],
            contributions: &contributions[0],
        },
        UtilityAssignment {
            offer: &offers[2],
            contributions: &contributions[1],
        },
    ];
    let ranked = rank_tactics(
        &ranking_observation,
        &admitted,
        &assignments,
        &knowledge,
        &policy,
        UtilityLimits {
            candidates: 2,
            contributions: 2,
            knowledge_records: 2,
            criteria: 1,
        },
    )
    .expect("the ranker accepts grounded caller-supplied policy values");

    let preferred = ranked.preferred().expect("one admitted offer is preferred");
    assert_eq!(preferred.offer().id, "retreat");
    assert!(
        admitted
            .offers()
            .iter()
            .any(|offer| std::ptr::eq(*offer, preferred.offer()))
    );
    assert!(
        !ranked
            .tactics()
            .iter()
            .any(|tactic| tactic.offer().id == "invented-teleport")
    );
    assert_eq!(ranked.tactics()[1].utility(), -5);
    assert_eq!(authority.observation_calls.get(), 1);
    assert_eq!(authority.legality_calls.get(), offers.len());
}

#[test]
fn stale_basis_and_duplicate_offer_identity_reject_the_complete_inventory() {
    let basis = basis();
    let pins = pins();
    let other_basis = Basis {
        revision: revision(2, 9),
        ..basis
    };
    let observation = Observation {
        offered_basis: basis,
        authorized_ids: &["same-id"],
    };
    let offers = [
        Offer {
            id: "same-id",
            payload_bytes: 1,
        },
        Offer {
            id: "same-id",
            payload_bytes: 99,
        },
    ];
    let authority = OfferAuthority::default();
    let stale = enumerate_candidates(
        &authority,
        &observation,
        CandidateContext {
            basis: &basis,
            pins: &pins,
        },
        CandidateContext {
            basis: &other_basis,
            pins: &pins,
        },
        &offers,
        candidate_limits(),
    );
    assert!(matches!(stale, Err(CandidateError::StaleBasis)));
    assert_eq!(authority.observation_calls.get(), 0);
    assert_eq!(authority.legality_calls.get(), 0);

    let context = CandidateContext {
        basis: &basis,
        pins: &pins,
    };
    let duplicate = enumerate_candidates(
        &authority,
        &observation,
        context,
        context,
        &offers,
        candidate_limits(),
    );
    assert_eq!(
        duplicate.err(),
        Some(CandidateError::Duplicate {
            first: 0,
            duplicate: 1,
        })
    );
    assert_eq!(authority.legality_calls.get(), 0);
}

#[test]
fn member_cannot_rank_an_npc_belief_without_a_grant_to_that_member() {
    let basis = basis();
    let pins = pins();
    let observation = Observation {
        offered_basis: basis,
        authorized_ids: &["canonical-choice"],
    };
    let offers = [Offer {
        id: "canonical-choice",
        payload_bytes: 4,
    }];
    let context = CandidateContext {
        basis: &basis,
        pins: &pins,
    };
    let admitted = enumerate_candidates(
        &OfferAuthority::default(),
        &observation,
        context,
        context,
        &offers,
        candidate_limits(),
    )
    .expect("the canonical offer is current and legal");

    let belief_id = RecordId::from_bytes(&[18; 16]).expect("fixed fixture ID");
    let belief = AttributedClaim {
        id: belief_id,
        holder: entity(4),
        subject: entity(4),
        claim: "fixture belief; not canonical truth".to_owned(),
        evidence: vec![],
        audience: AudienceScope::Shared,
        source: content(),
    };
    let beliefs = [belief];
    let npc = NpcState {
        entity: entity(4),
        personality: content(),
        motivations: vec![],
        known_facts: vec![],
        beliefs: vec![belief_id],
        secrets: vec![],
    };
    let npcs = [npc];
    let knowledge = tactical_knowledge(
        &basis,
        &pins,
        KnowledgeObserver::Member(member(3)),
        &[],
        &beliefs,
        &npcs,
    );
    let criterion = content();
    let criteria = [criterion.clone()];
    let policy = UtilityPolicy {
        basis: &basis,
        pins: &pins,
        definition: &criterion,
        criteria: &criteria,
    };
    let ranking_observation = RankingObservation {
        basis: &basis,
        pins: &pins,
        observer: KnowledgeObserver::Member(member(3)),
        logical_time: state().logical_time,
        cause: OperationId::from_bytes(&[6; 16]).expect("fixed fixture ID"),
        policy: &criterion,
    };
    let contribution = [UtilityContribution {
        criterion: criterion.clone(),
        evidence: UtilityEvidence::Belief(belief_id),
        value: i64::MAX,
    }];
    let assignments = [UtilityAssignment {
        offer: &offers[0],
        contributions: &contribution,
    }];

    assert_eq!(
        rank_tactics(
            &ranking_observation,
            &admitted,
            &assignments,
            &knowledge,
            &policy,
            UtilityLimits {
                candidates: 1,
                contributions: 1,
                knowledge_records: 3,
                criteria: 1,
            },
        )
        .err(),
        Some(UtilityRankingError::UndisclosedEvidence)
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HandlerRefusal {
    Rejected,
}

struct StructuralHandler {
    pins: CheckpointPins,
    calls: Cell<usize>,
}

impl RulesCommandHandler for StructuralHandler {
    type Rejection = HandlerRefusal;

    fn pins(&self) -> &CheckpointPins {
        &self.pins
    }

    fn stage(
        &self,
        input: RulesCommandInput<'_>,
        current: &Checkpoint,
    ) -> Result<Checkpoint, Self::Rejection> {
        self.calls.set(self.calls.get() + 1);
        let GameInput::Game(command) = input.command else {
            return Err(HandlerRefusal::Rejected);
        };
        let mut next_basis = current.basis();
        next_basis.revision = next_basis
            .revision
            .next_sequence()
            .map_err(|_| HandlerRefusal::Rejected)?;
        let mut next_state = current.state().clone();
        next_state.pending.clear();
        next_state.decisions.push(AcceptedDecision {
            operation: command.operation,
            revision: next_basis.revision,
            facts: vec![],
            draws: vec![],
            effects: vec![],
            source_policy: label("fixture-registered-policy"),
            semantic_output: None,
        });
        Checkpoint::new(
            CHECKPOINT_SCHEMA,
            next_basis,
            current.pins().clone(),
            next_state,
            ReferenceInventory {
                rules: &[rule()],
                content: &[content()],
                resources: &resource_constraints(),
                assets: &[],
            },
            limits(),
        )
        .map_err(|_| HandlerRefusal::Rejected)
    }
}

#[test]
fn registered_choice_preview_contains_only_source_prepared_options_for_the_member() {
    let basis = basis();
    let source = rule();
    let sources = [source.clone()];
    let contents = [content()];
    let resources = resource_constraints();
    let mut current_state = state();
    current_state.facts.push(fact(7, 0));
    current_state.members.push(MembershipLink {
        member: member(5),
        character: None,
    });
    let mut current_pending = pending();
    let PendingInput::Reaction { remaining } = &current_pending.next else {
        unreachable!("fixture pending response is a reaction");
    };
    let mut foreign_response = remaining[0].clone();
    foreign_response.participant = member(5);
    foreign_response.offer = label("foreign-member-offer");
    if let PendingInput::Reaction { remaining } = &mut current_pending.next {
        remaining.push(foreign_response);
    }
    current_state.pending.push(current_pending.clone());
    let current = checkpoint(current_state).expect("valid current pending checkpoint");
    let context = CurrentRuleContext {
        checkpoint: &current,
        basis: current.basis(),
        pins: current.pins(),
        inventory: ReferenceInventory {
            rules: &sources,
            content: &contents,
            resources: &resources,
            assets: &[],
        },
        command_limits: CommandLimits {
            maximum_records: 32,
            maximum_text_bytes: 64,
            maximum_retained_bytes: 16_384,
        },
    };
    let dependencies = [RuleDependency::PendingResolution(current_pending.id)];
    let handler = StructuralHandler {
        pins: current.pins().clone(),
        calls: Cell::new(0),
    };
    let wrapped = PreconditionedCommandHandler::new(
        &handler,
        &source,
        CurrentRuleContext {
            checkpoint: &current,
            basis: current.basis(),
            pins: current.pins(),
            inventory: ReferenceInventory {
                rules: &sources,
                content: &contents,
                resources: &resources,
                assets: &[],
            },
            command_limits: CommandLimits {
                maximum_records: 32,
                maximum_text_bytes: 64,
                maximum_retained_bytes: 16_384,
            },
        },
        RulePreconditions {
            prepared: &current,
            sources: &sources,
            dependencies: &dependencies,
        },
        PreconditionLimits {
            maximum_dependencies: 8,
            maximum_comparisons: 8 * 1024 * 1024,
            maximum_checkpoint_bytes: 1024 * 1024,
        },
    );
    let selector = current_pending.continuation;
    let registrations = [HandlerRegistration::new(&selector, &source, &wrapped)];
    let catalog_entries = [CatalogEntry::new(
        &source,
        b"synthetic source invocation fixture",
    )];
    let catalog = CatalogSnapshot::from_published(
        &current.pins().rules.catalog,
        current.pins(),
        b"synthetic source invocation fixture",
        &catalog_entries,
        CatalogLimits {
            max_complete_bytes: 1024,
            max_entries: 4,
            max_item_bytes: 512,
            max_total_item_bytes: 512,
        },
    )
    .expect("fixture catalog is bounded and pinned");
    let registry = DispatchRegistry::from_catalog(catalog, &registrations, 4)
        .expect("fixture handler matches the published catalog");
    let template = CommandInput {
        basis,
        observed_revision: basis.revision,
        operation: OperationId::from_bytes(&[6; 16]).expect("fixed fixture ID"),
        member: member(3),
        command: GameCommand::SelectReaction {
            resolution: current_pending.id,
            window: current_pending.window.id,
            offer: label("ignored-template-offer"),
            option: label("ignored-template-option"),
        },
    };

    let current_basis = current.basis();
    let admitted = enumerate_registered_responses(
        context,
        &registry,
        RegisteredCandidateRequest {
            template: &template,
            current: CandidateContext {
                basis: &current_basis,
                pins: current.pins(),
            },
            member: member(3),
            operation: template.operation,
            selector: &selector,
        },
        RegisteredCandidateLimits {
            candidates: candidate_limits(),
            maximum_checkpoint_bytes: 1024 * 1024,
            maximum_inventory_records: 16,
            maximum_rules_preparations: 8,
            maximum_staged_bytes: 1024 * 1024,
        },
    )
    .expect("the registered rules pipeline admits the exact pending member offer");

    let PendingInput::Reaction { remaining } = &current.state().pending[0].next else {
        unreachable!("fixture response kind remains a reaction");
    };
    assert_eq!(admitted.offers().len(), 1);
    assert!(std::ptr::eq(admitted.offers()[0], &remaining[0]));
    assert_eq!(
        admitted.offers()[0].options,
        vec![label("fixture-option-1")]
    );
    assert_eq!(handler.calls.get(), 1);
    assert!(current.state().decisions.is_empty());
    assert!(current.state().draws.is_empty());

    let unoffered_input = GameInput::Game(CommandInput {
        basis,
        observed_revision: basis.revision,
        operation: template.operation,
        member: member(3),
        command: GameCommand::SelectReaction {
            resolution: current_pending.id,
            window: current_pending.window.id,
            offer: label("fixture-offer-1"),
            option: label("invented-option"),
        },
    });
    let draws: [ActualDraw; 0] = [];
    let result = stage_tactical_response(
        RulesCommandInput {
            command: &unoffered_input,
            supplied_draws: &draws,
        },
        &current,
        CommandEntryContext {
            current_basis: current.basis(),
            admitted_pins: current.pins(),
            inventory: ReferenceInventory {
                rules: &sources,
                content: &contents,
                resources: &resources,
                assets: &[],
            },
            limits: CommandEntryLimits {
                command: CommandLimits {
                    maximum_records: 32,
                    maximum_text_bytes: 64,
                    maximum_retained_bytes: 16_384,
                },
                maximum_staged_bytes: 1024 * 1024,
            },
        },
        TacticalResponseAdmission {
            candidates: &admitted,
            member: member(3),
            operation: template.operation,
            fence: ResumeFence {
                admitted_generation: 1,
                current_generation: 1,
                cancel_before_admission: false,
            },
            limits: ResumeLimits {
                maximum_checkpoint_bytes: 1024 * 1024,
            },
        },
        &registry,
        &selector,
    );
    assert_eq!(result, Err(TacticalResponseError::UnadmittedResponse));
    assert_eq!(handler.calls.get(), 1);
    assert!(current.state().decisions.is_empty());
}
