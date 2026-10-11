//! Source-bound Design example; policy numbers and accepted records are synthetic.
//! Real canonical checkpoint/budget APIs run here; no production director or durable commit is claimed.
#![cfg(not(target_arch = "wasm32"))]

use df_content::narrative::{NarrativeBudgetPolicy, NarrativeBudgetRule};
use df_model::checkpoint::*;
use df_narrative::{
    NarrativeBudgetChange, NarrativeBudgetError, NarrativeBudgetLimits, NarrativeBudgetOutcome,
    NarrativeBudgetProposal, NarrativeBudgetRequest, stage_candidate_narrative_budget,
    stage_checkpoint_narrative_budget,
};
use df_types::{OperationId, RevisionLabel};

#[expect(
    dead_code,
    reason = "shared canonical checkpoint fixture has other consumers"
)]
#[path = "../../df-engine/tests/support/fixture_model.rs"]
mod model;

fn content(entry: &str) -> ContentReference {
    ContentReference {
        package: model::pins().content.package,
        entry: model::label(entry),
    }
}

fn fact_id(value: u8) -> FactId {
    FactId::from_bytes(&[value; 16]).unwrap()
}

struct Fixture {
    references: Vec<ContentReference>,
    rules: Vec<RuleReference>,
    resources: Vec<ResourceConstraint>,
    checkpoint: Checkpoint,
    policy: NarrativeBudgetPolicy<ContentReference>,
    source_policy: RevisionLabel,
}

impl Fixture {
    fn new(balance: u64) -> Self {
        let references = [
            model::content(),
            content("admitted-budget-policy"),
            content("strong-opportunity"),
            content("approved-free-play"),
            content("unsupported-event"),
        ]
        .to_vec();
        let policy = NarrativeBudgetPolicy {
            definition: references[1].clone(),
            maximum: 10,
            strong_events: vec![NarrativeBudgetRule {
                event: references[2].clone(),
                units: 3,
            }],
            free_play_events: vec![NarrativeBudgetRule {
                event: references[3].clone(),
                units: 4,
            }],
        };
        let source_policy = model::label("admitted-event-policy");
        let rules = vec![model::rule()];
        let resources = model::resource_constraints();
        let mut state = model::state();
        state.narrative.remaining_budget = balance;
        for (id, event) in [
            (20, &references[2]),
            (21, &references[3]),
            (22, &references[4]),
        ] {
            append_receipt(&mut state, model::basis(), id, event, &source_policy);
        }
        let checkpoint = Checkpoint::new(
            CHECKPOINT_SCHEMA,
            model::basis(),
            model::pins(),
            state,
            ReferenceInventory {
                rules: &rules,
                content: &references,
                resources: &resources,
                assets: &[],
            },
            model::limits(),
        )
        .unwrap();
        Self {
            references,
            rules,
            resources,
            checkpoint,
            policy,
            source_policy,
        }
    }

    fn inventory(&self) -> ReferenceInventory<'_> {
        ReferenceInventory {
            rules: &self.rules,
            content: &self.references,
            resources: &self.resources,
            assets: &[],
        }
    }

    fn limits() -> NarrativeBudgetLimits {
        NarrativeBudgetLimits {
            records: 100,
            work: 4096,
        }
    }

    fn request<'a>(
        &'a self,
        current: &'a Checkpoint,
        change: NarrativeBudgetChange,
    ) -> NarrativeBudgetRequest<'a> {
        NarrativeBudgetRequest {
            expected_basis: current.basis(),
            admitted_pins: current.pins(),
            policy: &self.policy,
            expected_policy: &self.policy,
            source_policy: &self.source_policy,
            change,
            inventory: self.inventory(),
            checkpoint_limits: model::limits(),
        }
    }

    fn rebuild(&self, basis: Basis, state: GameState) -> Checkpoint {
        Checkpoint::new(
            self.checkpoint.schema(),
            basis,
            self.checkpoint.pins().clone(),
            state,
            self.inventory(),
            model::limits(),
        )
        .unwrap()
    }

    fn stage(
        &self,
        current: &Checkpoint,
        change: NarrativeBudgetChange,
    ) -> Result<NarrativeBudgetProposal, NarrativeBudgetError> {
        stage_checkpoint_narrative_budget(current, self.request(current, change), Self::limits())
    }

    fn fresh_candidate(&self) -> Checkpoint {
        let mut next = self.checkpoint.basis();
        next.revision = next.revision.next_sequence().unwrap();
        let mut state = self.checkpoint.state().clone();
        append_receipt(
            &mut state,
            next,
            23,
            &self.references[2],
            &self.source_policy,
        );
        state.narrative.accepted_facts.push(fact_id(23));
        self.rebuild(next, state)
    }
}

// These explicitly synthetic records go through the real Model consistency validator.
// A valid record is not proof that a production rules executor emitted or persisted it.
fn append_receipt(
    state: &mut GameState,
    basis: Basis,
    id: u8,
    event: &ContentReference,
    source_policy: &RevisionLabel,
) {
    let operation = OperationId::from_bytes(&[id; 16]).unwrap();
    let fact = fact_id(id);
    state.facts.push(GameFact {
        id: fact,
        revision: basis.revision,
        operation,
        ordinal: 0,
        cause: None,
        audience: AudienceScope::Shared,
        value: FactValue::ContentEvent {
            definition: event.clone(),
            subjects: vec![],
        },
    });
    state.decisions.push(AcceptedDecision {
        operation,
        revision: basis.revision,
        facts: vec![fact],
        draws: vec![],
        effects: vec![],
        source_policy: source_policy.clone(),
        semantic_output: None,
    });
}

#[test]
fn strong_intervention_spends_exact_source_units_without_mutating_current() {
    let fixture = Fixture::new(5);
    let before = fixture.checkpoint.clone();
    let proposed = fixture
        .stage(
            &before,
            NarrativeBudgetChange::StrongIntervention(fact_id(20)),
        )
        .unwrap();
    assert_eq!(
        proposed.outcome,
        NarrativeBudgetOutcome::Applied {
            before: 5,
            after: 2
        }
    );
    let mut expected = before.state().clone();
    expected.narrative.remaining_budget = 2;
    expected.narrative.accepted_facts.push(fact_id(20));
    assert_eq!(proposed.checkpoint.state(), &expected);
    assert_eq!(proposed.checkpoint.basis(), before.basis());
    assert_eq!(proposed.checkpoint.pins(), before.pins());
    assert_eq!(fixture.checkpoint, before);
}

#[test]
fn approved_free_play_is_capped_and_does_not_advance_world_or_resources() {
    let fixture = Fixture::new(9);
    let before = fixture.checkpoint.clone();
    let proposed = fixture
        .stage(
            &before,
            NarrativeBudgetChange::ApprovedFreePlay(fact_id(21)),
        )
        .unwrap();
    assert_eq!(
        proposed.outcome,
        NarrativeBudgetOutcome::Applied {
            before: 9,
            after: 10
        }
    );
    let mut expected = before.state().clone();
    expected.narrative.remaining_budget = 10;
    expected.narrative.accepted_facts.push(fact_id(21));
    assert_eq!(proposed.checkpoint.state(), &expected);
    assert_eq!(fixture.checkpoint, before);
}

#[test]
fn exhaustion_refuses_and_does_not_consume_the_receipt() {
    let fixture = Fixture::new(2);
    let before = fixture.checkpoint.clone();
    assert_eq!(
        fixture
            .stage(
                &before,
                NarrativeBudgetChange::StrongIntervention(fact_id(20))
            )
            .unwrap_err(),
        NarrativeBudgetError::InsufficientBudget
    );
    assert_eq!(fixture.checkpoint, before);
}

#[test]
fn consumed_receipt_is_idempotent() {
    for change in [
        NarrativeBudgetChange::StrongIntervention(fact_id(20)),
        NarrativeBudgetChange::ApprovedFreePlay(fact_id(21)),
    ] {
        let fixture = Fixture::new(5);
        let first = fixture.stage(&fixture.checkpoint, change).unwrap();
        let second = fixture.stage(&first.checkpoint, change).unwrap();
        assert_eq!(second.outcome, NarrativeBudgetOutcome::AlreadyConsumed);
        assert_eq!(second.checkpoint, first.checkpoint);
    }
}

#[test]
fn unsupported_missing_and_wrong_policy_receipts_refuse() {
    let fixture = Fixture::new(5);
    let before = fixture.checkpoint.clone();
    for (id, expected) in [
        (22, NarrativeBudgetError::UnsupportedEvent),
        (99, NarrativeBudgetError::Source),
    ] {
        assert_eq!(
            fixture
                .stage(
                    &before,
                    NarrativeBudgetChange::StrongIntervention(fact_id(id))
                )
                .unwrap_err(),
            expected
        );
    }
    let wrong = model::label("unadmitted-source-policy");
    let mut request = fixture.request(
        &before,
        NarrativeBudgetChange::StrongIntervention(fact_id(20)),
    );
    request.source_policy = &wrong;
    assert_eq!(
        stage_checkpoint_narrative_budget(&before, request, Fixture::limits()).unwrap_err(),
        NarrativeBudgetError::Source
    );
    assert_eq!(fixture.checkpoint, before);
}

#[test]
fn stale_basis_or_content_pins_cannot_spend() {
    let fixture = Fixture::new(5);
    let before = fixture.checkpoint.clone();
    let mut request = fixture.request(
        &before,
        NarrativeBudgetChange::StrongIntervention(fact_id(20)),
    );
    request.expected_basis.revision = model::revision(2, 7);
    assert!(matches!(
        stage_checkpoint_narrative_budget(&before, request, Fixture::limits()),
        Err(NarrativeBudgetError::Binding(_))
    ));
    let mut pins = before.pins().clone();
    pins.content.package_digest = ContentDigest([99; 32]);
    let mut request = fixture.request(
        &before,
        NarrativeBudgetChange::StrongIntervention(fact_id(20)),
    );
    request.admitted_pins = &pins;
    assert!(matches!(
        stage_checkpoint_narrative_budget(&before, request, Fixture::limits()),
        Err(NarrativeBudgetError::Binding(_))
    ));
    assert_eq!(fixture.checkpoint, before);
}

#[test]
fn invalid_or_changed_budget_policy_cannot_mint_credit() {
    let fixture = Fixture::new(5);
    let before = fixture.checkpoint.clone();
    let mut zero = fixture.policy.clone();
    zero.maximum = 0;
    let mut duplicate = fixture.policy.clone();
    duplicate.free_play_events[0].event = duplicate.strong_events[0].event.clone();
    let mut changed = fixture.policy.clone();
    changed.strong_events[0].units = 1;
    for policy in [&zero, &duplicate, &changed] {
        let mut request = fixture.request(
            &before,
            NarrativeBudgetChange::ApprovedFreePlay(fact_id(21)),
        );
        request.policy = policy;
        if policy != &changed {
            request.expected_policy = policy;
        }
        assert_eq!(
            stage_checkpoint_narrative_budget(&before, request, Fixture::limits()).unwrap_err(),
            NarrativeBudgetError::Policy
        );
    }
    let mut state = before.state().clone();
    state.narrative.remaining_budget = 11;
    let excessive = fixture.rebuild(before.basis(), state);
    assert_eq!(
        fixture
            .stage(
                &excessive,
                NarrativeBudgetChange::ApprovedFreePlay(fact_id(21))
            )
            .unwrap_err(),
        NarrativeBudgetError::InvalidBalance
    );
    assert_eq!(fixture.checkpoint, before);
}

#[test]
fn record_work_and_checkpoint_byte_caps_refuse_without_mutation() {
    let fixture = Fixture::new(5);
    let before = fixture.checkpoint.clone();
    for limits in [
        NarrativeBudgetLimits {
            records: 0,
            work: 4096,
        },
        NarrativeBudgetLimits {
            records: 100,
            work: 0,
        },
    ] {
        assert_eq!(
            stage_checkpoint_narrative_budget(
                &before,
                fixture.request(
                    &before,
                    NarrativeBudgetChange::StrongIntervention(fact_id(20))
                ),
                limits,
            )
            .unwrap_err(),
            NarrativeBudgetError::Capacity
        );
    }
    let mut request = fixture.request(
        &before,
        NarrativeBudgetChange::StrongIntervention(fact_id(20)),
    );
    request.checkpoint_limits.maximum_retained_bytes = 0;
    assert_eq!(
        stage_checkpoint_narrative_budget(&before, request, Fixture::limits()).unwrap_err(),
        NarrativeBudgetError::Capacity
    );
    assert_eq!(fixture.checkpoint, before);
}

#[test]
fn fresh_candidate_composes_exactly_once_with_the_accepted_terminal_receipt() {
    let fixture = Fixture::new(5);
    let before = fixture.checkpoint.clone();
    let candidate = fixture.fresh_candidate();
    let snapshot = candidate.clone();
    let proposed = stage_candidate_narrative_budget(
        &before,
        &candidate,
        fixture.request(
            &before,
            NarrativeBudgetChange::StrongIntervention(fact_id(23)),
        ),
        Fixture::limits(),
    )
    .unwrap();
    assert_eq!(
        proposed.outcome,
        NarrativeBudgetOutcome::Applied {
            before: 5,
            after: 2
        }
    );
    let mut expected = candidate.state().clone();
    expected.narrative.remaining_budget = 2;
    assert_eq!(proposed.checkpoint.state(), &expected);
    assert_eq!(proposed.checkpoint.basis(), candidate.basis());
    assert_eq!(candidate, snapshot);
    assert_eq!(fixture.checkpoint, before);
}

#[test]
fn replay_or_changed_candidate_history_cannot_compose_again() {
    let fixture = Fixture::new(5);
    let before = fixture.checkpoint.clone();
    let candidate = fixture.fresh_candidate();
    let snapshot = candidate.clone();
    assert!(matches!(
        stage_candidate_narrative_budget(
            &before,
            &before,
            fixture.request(
                &before,
                NarrativeBudgetChange::StrongIntervention(fact_id(23))
            ),
            Fixture::limits(),
        ),
        Err(NarrativeBudgetError::Binding(_))
    ));
    let mut changed_history = candidate.state().clone();
    changed_history.facts[0].audience = AudienceScope::Host;
    let changed_history = fixture.rebuild(candidate.basis(), changed_history);
    let mut changed_balance = candidate.state().clone();
    changed_balance.narrative.remaining_budget = 2;
    let changed_balance = fixture.rebuild(candidate.basis(), changed_balance);
    for altered in [&changed_history, &changed_balance] {
        assert_eq!(
            stage_candidate_narrative_budget(
                &before,
                altered,
                fixture.request(
                    &before,
                    NarrativeBudgetChange::StrongIntervention(fact_id(23))
                ),
                Fixture::limits(),
            )
            .unwrap_err(),
            NarrativeBudgetError::Source
        );
    }
    assert_eq!(
        stage_candidate_narrative_budget(
            &before,
            &candidate,
            fixture.request(
                &before,
                NarrativeBudgetChange::StrongIntervention(fact_id(20))
            ),
            Fixture::limits(),
        )
        .unwrap_err(),
        NarrativeBudgetError::Source
    );
    assert_eq!(candidate, snapshot);
    assert_eq!(fixture.checkpoint, before);
}
