use std::cell::{Cell, RefCell};

use df_encounter::challenge::{
    AuthoredChallengeReference, ChallengeBudget, ChallengeMode, ChallengeReferenceKind,
    ChallengeSourceGap, EncounterDirector, EncounterError, EncounterRequest, EncounterSourceOwner,
    SourceAdmission,
};
use df_encounter::participants::{ParticipantError, ParticipantOwner, ParticipantStatus};
use df_model::checkpoint::{
    Basis, Checkpoint, CheckpointPins, ContentReference, EntityId, ReferenceInventory,
    RuleReference,
};

include!("objectives/fixture.rs");

fn definitions() -> Vec<ContentReference> {
    vec![content()]
}

fn reference() -> AuthoredChallengeReference {
    AuthoredChallengeReference {
        definition: content(),
        source: rule(),
    }
}

fn reference_with_entry(entry: &str) -> AuthoredChallengeReference {
    let mut authored = reference();
    authored.definition.entry = label(entry);
    authored.source.entry = label(entry);
    authored
}

fn current() -> Checkpoint {
    checkpoint(state()).unwrap()
}

fn inventory<'a>(
    rules: &'a [RuleReference],
    contents: &'a [ContentReference],
) -> ReferenceInventory<'a> {
    ReferenceInventory {
        rules,
        content: contents,
        resources: &[],
        assets: &[],
    }
}

#[derive(Default)]
struct Authority {
    basis_calls: Cell<usize>,
    basis_allowed: Cell<Option<bool>>,
    participant_calls: Cell<usize>,
    participant_order: RefCell<Vec<EntityId>>,
    participant_result: Cell<Option<ParticipantStatus>>,
    source_calls: Cell<usize>,
    source_purposes: RefCell<Vec<ChallengeReferenceKind>>,
    expected_mode: Cell<Option<ChallengeMode>>,
    expected_admissions: RefCell<
        Vec<(
            ChallengeMode,
            ChallengeReferenceKind,
            AuthoredChallengeReference,
        )>,
    >,
    source_result: Cell<Option<SourceAdmission>>,
    source_result_at: Cell<Option<(usize, SourceAdmission)>>,
    source_error_at: Cell<Option<usize>>,
}

impl ParticipantOwner for Authority {
    type Actor = EntityId;
    type Basis = Basis;
    type Error = ();

    fn validate_basis(&self, proposal: &Basis, current: &Basis) -> Result<(), Self::Error> {
        self.basis_calls.set(self.basis_calls.get() + 1);
        if self.basis_allowed.get() == Some(false) || proposal != current {
            return Err(());
        }
        Ok(())
    }

    fn participant_status(
        &self,
        _current: &Basis,
        actor: &EntityId,
    ) -> Result<ParticipantStatus, Self::Error> {
        self.participant_calls.set(self.participant_calls.get() + 1);
        self.participant_order.borrow_mut().push(*actor);
        Ok(self.participant_result.get().unwrap_or_else(|| {
            if *actor == entity(4) || *actor == entity(5) {
                ParticipantStatus::Available
            } else {
                ParticipantStatus::Unknown
            }
        }))
    }
}

impl EncounterSourceOwner for Authority {
    fn admit_reference(
        &self,
        current: &Checkpoint,
        mode: ChallengeMode,
        purpose: ChallengeReferenceKind,
        authored: &AuthoredChallengeReference,
    ) -> Result<SourceAdmission, Self::Error> {
        assert_eq!(current.pins(), &pins());
        let call = self.source_calls.get();
        let expected = self.expected_admissions.borrow();
        if expected.is_empty() {
            assert_eq!(authored, &reference());
        } else {
            let (expected_mode, expected_purpose, expected_reference) =
                &expected[call % expected.len()];
            assert_eq!(mode, *expected_mode);
            assert_eq!(purpose, *expected_purpose);
            assert_eq!(authored, expected_reference);
        }
        if let Some(expected_mode) = self.expected_mode.get() {
            assert_eq!(mode, expected_mode);
        }
        self.source_purposes.borrow_mut().push(purpose);
        self.source_calls.set(call + 1);
        if current.basis() != basis() || self.source_error_at.get() == Some(call) {
            return Err(());
        }
        if let Some((index, result)) = self.source_result_at.get()
            && index == call
        {
            return Ok(result);
        }
        Ok(self
            .source_result
            .get()
            .unwrap_or(SourceAdmission::Admitted))
    }
}

fn budget() -> ChallengeBudget {
    ChallengeBudget {
        max_participants: 1,
        max_objectives: 1,
        max_escape_references: 1,
        max_consequence_references: 1,
        max_comparisons: 100,
        max_input_bytes: 1_000_000,
        max_output_bytes: 1_000_000,
    }
}

fn request<'a>(
    current: &'a Checkpoint,
    pins: &'a CheckpointPins,
    inventory: ReferenceInventory<'a>,
    mode: ChallengeMode,
    authored: &'a AuthoredChallengeReference,
    participants: &'a [EntityId],
) -> EncounterRequest<'a> {
    EncounterRequest {
        current,
        expected_basis: basis(),
        admitted_pins: pins,
        inventory,
        mode,
        definition: authored,
        participants,
        objectives: std::slice::from_ref(authored),
        escape_condition: Some(authored),
        consequences: std::slice::from_ref(authored),
        budget: budget(),
    }
}

#[test]
fn all_modes_share_the_same_bounded_source_addressed_contract_and_preserve_order() {
    let current = current();
    let before = current.clone();
    let authored = reference();
    let rules = [rule()];
    let contents = definitions();
    let actors = [entity(4)];
    let modes = [
        ChallengeMode::Combat,
        ChallengeMode::Social,
        ChallengeMode::Chase,
        ChallengeMode::Hazard,
        ChallengeMode::Puzzle,
        ChallengeMode::Survival,
    ];

    for mode in modes {
        let owner = Authority::default();
        owner.expected_mode.set(Some(mode));
        let plan = EncounterDirector::propose(
            &owner,
            request(
                &current,
                current.pins(),
                inventory(&rules, &contents),
                mode,
                &authored,
                &actors,
            ),
        )
        .unwrap();
        assert_eq!(plan.mode, mode);
        assert_eq!(plan.basis, basis());
        assert_eq!(plan.pins, *current.pins());
        assert_eq!(plan.definition, authored);
        assert_eq!(plan.participants.as_slice(), &actors);
        assert_eq!(plan.objectives[0].authored, authored);
        assert_eq!(plan.escape_condition.as_ref().unwrap().authored, authored);
        assert_eq!(plan.consequences[0].authored, authored);
        assert!(plan.source_gaps.is_empty());
        assert_eq!(owner.source_calls.get(), 4);
        assert_eq!(owner.participant_calls.get(), 1);
        EncounterDirector::validate(
            &owner,
            &current,
            &plan,
            basis(),
            current.pins(),
            inventory(&rules, &contents),
            budget(),
        )
        .unwrap();
        assert_eq!(owner.source_calls.get(), 8);
        let expected_purposes = [
            ChallengeReferenceKind::Definition,
            ChallengeReferenceKind::Objective,
            ChallengeReferenceKind::EscapeCondition,
            ChallengeReferenceKind::Consequence,
            ChallengeReferenceKind::Definition,
            ChallengeReferenceKind::Objective,
            ChallengeReferenceKind::EscapeCondition,
            ChallengeReferenceKind::Consequence,
        ];
        assert_eq!(
            owner.source_purposes.borrow().as_slice(),
            expected_purposes.as_slice()
        );
    }

    assert_eq!(current, before);
}

#[test]
fn exact_count_capacity_succeeds_and_count_overflow_refuses_before_callbacks() {
    let current = current();
    let authored = reference();
    let rules = [rule()];
    let contents = definitions();
    let actors = [entity(4)];
    let owner = Authority::default();

    let mut exact = budget();
    exact.max_participants = 1;
    exact.max_objectives = 1;
    exact.max_escape_references = 1;
    exact.max_consequence_references = 1;
    let mut exact_request = request(
        &current,
        current.pins(),
        inventory(&rules, &contents),
        ChallengeMode::Combat,
        &authored,
        &actors,
    );
    exact_request.budget = exact;
    assert!(EncounterDirector::propose(&owner, exact_request).is_ok());

    let mut too_small = exact;
    too_small.max_consequence_references = 0;
    let mut limited_request = request(
        &current,
        current.pins(),
        inventory(&rules, &contents),
        ChallengeMode::Combat,
        &authored,
        &actors,
    );
    limited_request.budget = too_small;
    let calls_before = owner.source_calls.get();
    assert_eq!(
        EncounterDirector::propose(&owner, limited_request),
        Err(EncounterError::Capacity)
    );
    assert_eq!(owner.source_calls.get(), calls_before);
}

#[test]
fn source_gaps_are_typed_refusals_and_never_successful_partial_plans() {
    let current = current();
    let authored = reference();
    let rules = [rule()];
    let contents = definitions();
    let actors = [entity(4)];

    for reason in [
        SourceAdmission::Denied,
        SourceAdmission::Unavailable,
        SourceAdmission::Unsupported,
    ] {
        let owner = Authority {
            source_result: Cell::new(Some(reason)),
            ..Authority::default()
        };
        let result = EncounterDirector::propose(
            &owner,
            request(
                &current,
                current.pins(),
                inventory(&rules, &contents),
                ChallengeMode::Puzzle,
                &authored,
                &actors,
            ),
        );
        assert_eq!(
            result,
            Err(EncounterError::SourceGap(ChallengeSourceGap {
                purpose: ChallengeReferenceKind::Definition,
                index: 0,
                reason,
            }))
        );
        assert_eq!(owner.source_calls.get(), 1);
    }
}

#[test]
fn source_callbacks_wait_for_bounds_and_inventory_admission() {
    let current = current();
    let authored = reference();
    let rules = [rule()];
    let contents = definitions();
    let actors = [entity(4)];
    let owner = Authority::default();

    let mut bounded = budget();
    bounded.max_input_bytes = 1;
    let mut byte_request = request(
        &current,
        current.pins(),
        inventory(&rules, &contents),
        ChallengeMode::Hazard,
        &authored,
        &actors,
    );
    byte_request.budget = bounded;
    assert_eq!(
        EncounterDirector::propose(&owner, byte_request),
        Err(EncounterError::Capacity)
    );
    assert_eq!(owner.source_calls.get(), 0);

    let mut output_bounded = budget();
    output_bounded.max_output_bytes = 1;
    let mut output_request = request(
        &current,
        current.pins(),
        inventory(&rules, &contents),
        ChallengeMode::Hazard,
        &authored,
        &actors,
    );
    output_request.budget = output_bounded;
    assert_eq!(
        EncounterDirector::propose(&owner, output_request),
        Err(EncounterError::Capacity)
    );
    assert_eq!(owner.source_calls.get(), 0);

    let mut work_bounded = budget();
    work_bounded.max_comparisons = 0;
    let mut work_request = request(
        &current,
        current.pins(),
        inventory(&rules, &contents),
        ChallengeMode::Hazard,
        &authored,
        &actors,
    );
    work_request.budget = work_bounded;
    assert_eq!(
        EncounterDirector::propose(&owner, work_request),
        Err(EncounterError::Capacity)
    );
    assert_eq!(owner.source_calls.get(), 0);

    let empty_contents: [ContentReference; 0] = [];
    assert_eq!(
        EncounterDirector::propose(
            &owner,
            request(
                &current,
                current.pins(),
                inventory(&rules, &empty_contents),
                ChallengeMode::Hazard,
                &authored,
                &actors,
            ),
        ),
        Err(EncounterError::UnadmittedContent)
    );
    assert_eq!(owner.source_calls.get(), 0);
}

#[test]
fn stale_pins_duplicate_and_inadmissible_participants_refuse_without_partial_plans() {
    let current = current();
    let authored = reference();
    let rules = [rule()];
    let contents = definitions();
    let actors = [entity(4)];
    let owner = Authority::default();

    let mut stale_pins = current.pins().clone();
    stale_pins.content.content = label("other-content-pin");
    let result = EncounterDirector::propose(
        &owner,
        request(
            &current,
            &stale_pins,
            inventory(&rules, &contents),
            ChallengeMode::Social,
            &authored,
            &actors,
        ),
    );
    assert!(matches!(result, Err(EncounterError::Checkpoint(_))));
    assert_eq!(owner.source_calls.get(), 0);

    let duplicate_actors = [entity(4), entity(4)];
    let mut duplicate_request = request(
        &current,
        current.pins(),
        inventory(&rules, &contents),
        ChallengeMode::Social,
        &authored,
        &duplicate_actors,
    );
    duplicate_request.budget.max_participants = 2;
    assert!(matches!(
        EncounterDirector::propose(&owner, duplicate_request),
        Err(EncounterError::Participant(_))
    ));
    assert_eq!(owner.source_calls.get(), 0);

    for status in [
        ParticipantStatus::Unknown,
        ParticipantStatus::Unavailable,
        ParticipantStatus::Undisclosed,
    ] {
        let refusing_owner = Authority {
            participant_result: Cell::new(Some(status)),
            ..Authority::default()
        };
        assert!(matches!(
            EncounterDirector::propose(
                &refusing_owner,
                request(
                    &current,
                    current.pins(),
                    inventory(&rules, &contents),
                    ChallengeMode::Social,
                    &authored,
                    &actors,
                ),
            ),
            Err(EncounterError::Participant(_))
        ));
        assert_eq!(refusing_owner.source_calls.get(), 0);
    }
}

#[test]
fn validation_counts_content_and_rule_inventory_before_owner_callbacks() {
    let current = current();
    let before = current.clone();
    let authored = reference();
    let rules = [rule()];
    let contents = definitions();
    let actors = [entity(4)];
    let proposal_owner = Authority::default();
    let plan = EncounterDirector::propose(
        &proposal_owner,
        request(
            &current,
            current.pins(),
            inventory(&rules, &contents),
            ChallengeMode::Chase,
            &authored,
            &actors,
        ),
    )
    .unwrap();

    let expanded_contents = vec![content(); 1_000];
    let expanded_rules = vec![rule(); 1_000];
    let mut limited = budget();
    limited.max_comparisons = 10_000;
    limited.max_input_bytes = 10_000;
    for expanded_inventory in [
        inventory(&rules, &expanded_contents),
        inventory(&expanded_rules, &contents),
    ] {
        let validation_owner = Authority::default();
        assert_eq!(
            EncounterDirector::validate(
                &validation_owner,
                &current,
                &plan,
                basis(),
                current.pins(),
                expanded_inventory,
                limited,
            ),
            Err(EncounterError::Capacity)
        );
        assert_eq!(validation_owner.basis_calls.get(), 0);
        assert_eq!(validation_owner.participant_calls.get(), 0);
        assert_eq!(validation_owner.source_calls.get(), 0);
        assert_eq!(current, before);
    }
}

#[test]
fn committed_history_does_not_change_a_staged_challenge_or_checkpoint() {
    let committed = fact(7, 0);
    let mut game_state = state();
    game_state.facts.push(committed.clone());
    game_state.decisions.push(AcceptedDecision {
        operation: committed.operation,
        revision: committed.revision,
        facts: vec![committed.id],
        draws: vec![],
        effects: vec![],
        source_policy: label("fixture-committed-source-policy"),
        semantic_output: None,
    });
    let current = checkpoint(game_state).unwrap();
    let before = current.clone();
    let authored = reference();
    let rules = [rule()];
    let contents = definitions();
    let actors = [entity(4)];
    let owner = Authority::default();
    let plan = EncounterDirector::propose(
        &owner,
        request(
            &current,
            current.pins(),
            inventory(&rules, &contents),
            ChallengeMode::Social,
            &authored,
            &actors,
        ),
    )
    .unwrap();
    assert_eq!(plan.basis, basis());
    assert!(plan.source_gaps.is_empty());
    EncounterDirector::validate(
        &owner,
        &current,
        &plan,
        basis(),
        current.pins(),
        inventory(&rules, &contents),
        budget(),
    )
    .unwrap();
    assert_eq!(current, before);
}

#[test]
fn validation_rechecks_plan_basis_all_pin_groups_and_current_basis_before_owner_calls() {
    let current = current();
    let before = current.clone();
    let authored = reference();
    let rules = [rule()];
    let contents = definitions();
    let actors = [entity(4)];
    let plan = EncounterDirector::propose(
        &Authority::default(),
        request(
            &current,
            current.pins(),
            inventory(&rules, &contents),
            ChallengeMode::Combat,
            &authored,
            &actors,
        ),
    )
    .unwrap();

    let mut stale_basis = plan.clone();
    stale_basis.basis.revision = revision(2, 9);
    let mut stale_rules = plan.clone();
    stale_rules.pins.rules.handler = label("other-handler");
    let mut stale_content = plan.clone();
    stale_content.pins.content.content = label("other-content");
    let mut stale_build = plan.clone();
    stale_build.pins.build = BuildIdentity::new(
        Some("other-source"),
        Some("fixture-native-1"),
        Some("fixture-wasm-1"),
        Some("fixture-config-1"),
        Some("fixture-content-1"),
    )
    .unwrap();
    for (candidate, expected) in [
        (stale_basis, CheckpointError::StaleBasis),
        (stale_rules, CheckpointError::RulesMismatch),
        (stale_content, CheckpointError::ContentMismatch),
        (stale_build, CheckpointError::BuildMismatch),
    ] {
        let owner = Authority::default();
        assert_eq!(
            EncounterDirector::validate(
                &owner,
                &current,
                &candidate,
                basis(),
                current.pins(),
                inventory(&rules, &contents),
                budget(),
            ),
            Err(EncounterError::Checkpoint(expected))
        );
        assert_eq!(owner.basis_calls.get(), 0);
        assert_eq!(owner.participant_calls.get(), 0);
        assert_eq!(owner.source_calls.get(), 0);
        assert_eq!(current, before);
    }

    let mut stale_expected = basis();
    stale_expected.revision = revision(2, 9);
    let owner = Authority::default();
    assert_eq!(
        EncounterDirector::validate(
            &owner,
            &current,
            &plan,
            stale_expected,
            current.pins(),
            inventory(&rules, &contents),
            budget(),
        ),
        Err(EncounterError::Checkpoint(CheckpointError::StaleBasis))
    );
    assert_eq!(owner.basis_calls.get(), 0);
    assert_eq!(owner.participant_calls.get(), 0);
    assert_eq!(owner.source_calls.get(), 0);
    assert_eq!(current, before);
}

#[test]
fn validation_rejects_plan_gaps_missing_inventory_and_revoked_participants() {
    let current = current();
    let before = current.clone();
    let authored = reference();
    let rules = [rule()];
    let contents = definitions();
    let actors = [entity(4)];
    let plan = EncounterDirector::propose(
        &Authority::default(),
        request(
            &current,
            current.pins(),
            inventory(&rules, &contents),
            ChallengeMode::Hazard,
            &authored,
            &actors,
        ),
    )
    .unwrap();

    let mut with_gap = plan.clone();
    with_gap.source_gaps.push(ChallengeSourceGap {
        purpose: ChallengeReferenceKind::Objective,
        index: 0,
        reason: SourceAdmission::Unsupported,
    });
    let gap_owner = Authority::default();
    assert_eq!(
        EncounterDirector::validate(
            &gap_owner,
            &current,
            &with_gap,
            basis(),
            current.pins(),
            inventory(&rules, &contents),
            budget(),
        ),
        Err(EncounterError::PlanContainsSourceGaps)
    );
    assert_eq!(gap_owner.basis_calls.get(), 0);
    assert_eq!(gap_owner.participant_calls.get(), 0);
    assert_eq!(gap_owner.source_calls.get(), 0);

    let empty_content: [ContentReference; 0] = [];
    let empty_rules: [RuleReference; 0] = [];
    for (supplied, expected) in [
        (
            inventory(&rules, &empty_content),
            EncounterError::UnadmittedContent,
        ),
        (
            inventory(&empty_rules, &contents),
            EncounterError::UnadmittedSource,
        ),
    ] {
        let owner = Authority::default();
        assert_eq!(
            EncounterDirector::validate(
                &owner,
                &current,
                &plan,
                basis(),
                current.pins(),
                supplied,
                budget(),
            ),
            Err(expected)
        );
        assert_eq!(owner.basis_calls.get(), 1);
        assert_eq!(owner.participant_calls.get(), 1);
        assert_eq!(owner.source_calls.get(), 0);
        assert_eq!(current, before);
    }

    let revoked_owner = Authority::default();
    revoked_owner.basis_allowed.set(Some(false));
    assert_eq!(
        EncounterDirector::validate(
            &revoked_owner,
            &current,
            &plan,
            basis(),
            current.pins(),
            inventory(&rules, &contents),
            budget(),
        ),
        Err(EncounterError::Participant(ParticipantError::Owner(())))
    );
    assert_eq!(revoked_owner.basis_calls.get(), 1);
    assert_eq!(revoked_owner.participant_calls.get(), 0);
    assert_eq!(revoked_owner.source_calls.get(), 0);

    for status in [
        ParticipantStatus::Unknown,
        ParticipantStatus::Unavailable,
        ParticipantStatus::Undisclosed,
    ] {
        let owner = Authority {
            participant_result: Cell::new(Some(status)),
            ..Authority::default()
        };
        assert_eq!(
            EncounterDirector::validate(
                &owner,
                &current,
                &plan,
                basis(),
                current.pins(),
                inventory(&rules, &contents),
                budget(),
            ),
            Err(EncounterError::Participant(
                ParticipantError::Inadmissible { index: 0, status }
            ))
        );
        assert_eq!(owner.basis_calls.get(), 1);
        assert_eq!(owner.participant_calls.get(), 1);
        assert_eq!(owner.source_calls.get(), 0);
        assert_eq!(current, before);
    }
    assert_eq!(current, before);
}

#[test]
fn validation_refuses_source_owner_errors_and_all_source_gap_classes() {
    let current = current();
    let before = current.clone();
    let authored = reference();
    let rules = [rule()];
    let contents = definitions();
    let actors = [entity(4)];
    let plan = EncounterDirector::propose(
        &Authority::default(),
        request(
            &current,
            current.pins(),
            inventory(&rules, &contents),
            ChallengeMode::Survival,
            &authored,
            &actors,
        ),
    )
    .unwrap();

    let error_owner = Authority {
        source_error_at: Cell::new(Some(0)),
        ..Authority::default()
    };
    assert_eq!(
        EncounterDirector::validate(
            &error_owner,
            &current,
            &plan,
            basis(),
            current.pins(),
            inventory(&rules, &contents),
            budget(),
        ),
        Err(EncounterError::Owner(()))
    );
    assert_eq!(error_owner.basis_calls.get(), 1);
    assert_eq!(error_owner.participant_calls.get(), 1);
    assert_eq!(error_owner.source_calls.get(), 1);
    assert_eq!(current, before);

    for reason in [
        SourceAdmission::Denied,
        SourceAdmission::Unavailable,
        SourceAdmission::Unsupported,
    ] {
        let owner = Authority {
            source_result: Cell::new(Some(reason)),
            ..Authority::default()
        };
        assert_eq!(
            EncounterDirector::validate(
                &owner,
                &current,
                &plan,
                basis(),
                current.pins(),
                inventory(&rules, &contents),
                budget(),
            ),
            Err(EncounterError::SourceGap(ChallengeSourceGap {
                purpose: ChallengeReferenceKind::Definition,
                index: 0,
                reason,
            }))
        );
        assert_eq!(owner.basis_calls.get(), 1);
        assert_eq!(owner.participant_calls.get(), 1);
        assert_eq!(owner.source_calls.get(), 1);
        assert_eq!(current, before);
    }
}

#[test]
fn distinct_objectives_consequences_and_actors_keep_order_and_later_gaps_refuse() {
    let current = current();
    let before = current.clone();
    let mode = ChallengeMode::Puzzle;
    let definition = reference();
    let objectives = [
        reference_with_entry("objective-one"),
        reference_with_entry("objective-two"),
    ];
    let consequences = [
        reference_with_entry("consequence-one"),
        reference_with_entry("consequence-two"),
    ];
    let contents = [
        definition.definition.clone(),
        objectives[0].definition.clone(),
        objectives[1].definition.clone(),
        consequences[0].definition.clone(),
        consequences[1].definition.clone(),
    ];
    let rules = [
        definition.source.clone(),
        objectives[0].source.clone(),
        objectives[1].source.clone(),
        consequences[0].source.clone(),
        consequences[1].source.clone(),
    ];
    let actors = [entity(4), entity(5)];
    let expected_admissions = vec![
        (mode, ChallengeReferenceKind::Definition, definition.clone()),
        (
            mode,
            ChallengeReferenceKind::Objective,
            objectives[0].clone(),
        ),
        (
            mode,
            ChallengeReferenceKind::Objective,
            objectives[1].clone(),
        ),
        (
            mode,
            ChallengeReferenceKind::Consequence,
            consequences[0].clone(),
        ),
        (
            mode,
            ChallengeReferenceKind::Consequence,
            consequences[1].clone(),
        ),
    ];
    let mut bounded = budget();
    bounded.max_participants = 2;
    bounded.max_objectives = 2;
    bounded.max_escape_references = 0;
    bounded.max_consequence_references = 2;

    let owner = Authority::default();
    owner
        .expected_admissions
        .replace(expected_admissions.clone());
    let mut proposal = request(
        &current,
        current.pins(),
        inventory(&rules, &contents),
        mode,
        &definition,
        &actors,
    );
    proposal.objectives = &objectives;
    proposal.escape_condition = None;
    proposal.consequences = &consequences;
    proposal.budget = bounded;
    let plan = EncounterDirector::propose(&owner, proposal).unwrap();
    assert_eq!(plan.participants.as_slice(), actors.as_slice());
    assert_eq!(plan.objectives.len(), 2);
    assert_eq!(plan.objectives[0].authored, objectives[0]);
    assert_eq!(plan.objectives[1].authored, objectives[1]);
    assert!(plan.escape_condition.is_none());
    assert_eq!(plan.consequences.len(), 2);
    assert_eq!(plan.consequences[0].authored, consequences[0]);
    assert_eq!(plan.consequences[1].authored, consequences[1]);
    assert!(plan.source_gaps.is_empty());
    assert_eq!(owner.basis_calls.get(), 1);
    assert_eq!(owner.source_calls.get(), 5);
    assert_eq!(
        owner.participant_order.borrow().as_slice(),
        actors.as_slice()
    );

    EncounterDirector::validate(
        &owner,
        &current,
        &plan,
        basis(),
        current.pins(),
        inventory(&rules, &contents),
        bounded,
    )
    .unwrap();
    let expected_actor_order = [entity(4), entity(5), entity(4), entity(5)];
    assert_eq!(
        owner.participant_order.borrow().as_slice(),
        expected_actor_order.as_slice()
    );
    assert_eq!(owner.basis_calls.get(), 2);
    assert_eq!(owner.source_calls.get(), 10);
    assert_eq!(current, before);

    for (call, reason, purpose, index) in [
        (
            2,
            SourceAdmission::Unsupported,
            ChallengeReferenceKind::Objective,
            1,
        ),
        (
            4,
            SourceAdmission::Unavailable,
            ChallengeReferenceKind::Consequence,
            1,
        ),
    ] {
        let refusing_owner = Authority {
            source_result_at: Cell::new(Some((call, reason))),
            ..Authority::default()
        };
        refusing_owner
            .expected_admissions
            .replace(expected_admissions.clone());
        let mut request = request(
            &current,
            current.pins(),
            inventory(&rules, &contents),
            mode,
            &definition,
            &actors,
        );
        request.objectives = &objectives;
        request.escape_condition = None;
        request.consequences = &consequences;
        request.budget = bounded;
        assert_eq!(
            EncounterDirector::propose(&refusing_owner, request),
            Err(EncounterError::SourceGap(ChallengeSourceGap {
                purpose,
                index,
                reason,
            }))
        );
        assert_eq!(refusing_owner.basis_calls.get(), 1);
        assert_eq!(refusing_owner.participant_calls.get(), 2);
        assert_eq!(refusing_owner.source_calls.get(), call + 1);
        assert_eq!(current, before);

        let validation_owner = Authority {
            source_result_at: Cell::new(Some((call, reason))),
            ..Authority::default()
        };
        validation_owner
            .expected_admissions
            .replace(expected_admissions.clone());
        assert_eq!(
            EncounterDirector::validate(
                &validation_owner,
                &current,
                &plan,
                basis(),
                current.pins(),
                inventory(&rules, &contents),
                bounded,
            ),
            Err(EncounterError::SourceGap(ChallengeSourceGap {
                purpose,
                index,
                reason,
            }))
        );
        assert_eq!(validation_owner.basis_calls.get(), 1);
        assert_eq!(validation_owner.participant_calls.get(), 2);
        assert_eq!(validation_owner.source_calls.get(), call + 1);
        assert_eq!(current, before);
    }
}
