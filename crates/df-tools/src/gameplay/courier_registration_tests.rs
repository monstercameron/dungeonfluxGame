use super::*;
use crate::gameplay::wire;
use df_engine::effect_emission::EffectEmissionError;
use df_persistence::local_demo_scope::{LocalDemoRole, PLAYER};

fn command(
    current: &Checkpoint,
    member: MemberId,
    operation: u8,
    entry: &str,
    choices: Vec<(RevisionLabel, RevisionLabel)>,
) -> GameInput {
    GameInput::Game(CommandInput {
        basis: current.basis(),
        operation: OperationId::from_bytes(&[operation; 16]).unwrap(),
        member,
        observed_revision: current.basis().revision,
        command: GameCommand::ProposeAction {
            actor: if member.as_bytes() == &PLAYER {
                journey::room_entity().unwrap()
            } else {
                journey::player_entity(member, current).unwrap()
            },
            action: model::content(entry).unwrap(),
            targets: vec![],
            choices,
        },
    })
}

fn build(name: &str) -> Vec<(RevisionLabel, RevisionLabel)> {
    [
        ("name", journey::hex(name.as_bytes())),
        ("species", "dwarf".to_owned()),
        ("class", "fighter-1".to_owned()),
        ("background", "soldier".to_owned()),
        ("array", "stalwart".to_owned()),
        ("alignment", "lawful-good".to_owned()),
        ("language-1", "dwarvish".to_owned()),
        ("language-2", "elvish".to_owned()),
        ("skill-1", "perception".to_owned()),
        ("skill-2", "survival".to_owned()),
        ("gaming-set", "dice".to_owned()),
        ("equipment", "fighter-a-soldier-a".to_owned()),
        (
            "style-masteries",
            "defense-greatsword-flail-javelin".to_owned(),
        ),
        ("allied-ties", "join-order".to_owned()),
    ]
    .into_iter()
    .map(|(key, value)| (model::label(key).unwrap(), model::label(&value).unwrap()))
    .collect()
}

fn opening() -> (Checkpoint, MemberId, MemberId) {
    let mut current = journey::initial().unwrap();
    let bootstrap = MemberId::from_bytes(&PLAYER).unwrap();
    for operation in [1, 2] {
        current = journey::stage_join(
            &current,
            &command(&current, bootstrap, operation, "join-room", vec![]),
        )
        .unwrap();
    }
    let members = current
        .state()
        .members
        .iter()
        .filter(|link| link.member != bootstrap)
        .map(|link| link.member)
        .collect::<Vec<_>>();
    let (first, second) = (members[0], members[1]);
    for (member, operation, name) in [(first, 3, "Brynn"), (second, 4, "Vale")] {
        current = model::stage_registered_journey(
            &current,
            &command(&current, member, operation, "create-character", build(name)),
        )
        .unwrap();
    }
    current = model::stage_registered_journey(
        &current,
        &command(&current, first, 5, "begin-story", vec![]),
    )
    .unwrap();
    (current, first, second)
}

fn pending(current: &Checkpoint, member: MemberId) -> Checkpoint {
    model::stage_registered_journey(current, &command(current, member, 6, "ask-courier", vec![]))
        .unwrap()
}

fn assert_registration_refused(
    current: &Checkpoint,
    staged: &Checkpoint,
    source_member: MemberId,
    refusal: CourierError,
) {
    let original = current.clone();
    let candidate = staged.clone();
    let effect = &staged.state().intents[0];
    let GameInput::Game(source) = command(current, source_member, 6, "ask-courier", vec![]) else {
        panic!("source command")
    };
    assert_eq!(
        inspect_candidate(current, staged, &source),
        Err(EffectInspectionError::Registration {
            effect: effect.id,
            kind: effect.kind,
            refusal,
        })
    );
    assert_eq!(current, &original);
    assert_eq!(staged, &candidate);
    assert_eq!(effect.status, DurableStatus::Pending);
    assert_eq!(
        staged.state().decisions.len(),
        current.state().decisions.len() + 1
    );
}

#[test]
fn native_registered_source_preserves_exact_candidate_and_private_terminal_route() {
    let (current, first, second) = opening();
    let original = current.clone();
    let input = command(&current, first, 6, "ask-courier", vec![]);
    let uninspected = journey::stage(&current, &input).unwrap();
    let staged = model::stage_registered_journey(&current, &input).unwrap();
    assert_eq!(staged, uninspected);
    assert_eq!(current, original);
    let effect = &staged.state().intents[0];
    assert_eq!(effect.status, DurableStatus::Pending);
    assert_eq!(recipient(&staged, effect), Ok(first));
    assert_eq!(saved_response(&staged, first), Ok(None));
    assert_eq!(saved_response(&staged, second), Ok(None));

    // Only after successful read-only registration do we exercise the existing terminal pair.
    let completion = execute(&staged, effect).unwrap();
    let completed =
        stage_completion(&staged, &completion, completion_operation(effect).unwrap()).unwrap();
    assert_eq!(
        completed.state().intents[0].status,
        DurableStatus::Completed
    );
    assert_eq!(saved_response(&completed, first), Ok(Some(RESPONSE)));
    assert_eq!(saved_response(&completed, second), Ok(None));
    assert_eq!(completed.state().facts, staged.state().facts);
    assert_eq!(completed.state().draws, staged.state().draws);
    assert_eq!(completed.state().resources, staged.state().resources);
    assert_eq!(completed.state().narrative, staged.state().narrative);
    assert_eq!(completed.state().logical_time, staged.state().logical_time);
    let display = wire::journey_view(
        &completed,
        LocalDemoRole::Display,
        MemberId::from_bytes(&df_persistence::local_demo_scope::DISPLAY).unwrap(),
    )
    .unwrap();
    use prost::Message;
    assert!(
        !display
            .encode_to_vec()
            .windows(RESPONSE.len())
            .any(|bytes| bytes == RESPONSE.as_bytes())
    );
    assert_eq!(staged, uninspected);
}

#[test]
fn actual_native_staging_refuses_canonical_unregistered_modes_before_returning_candidate() {
    let (current, first, _) = opening();
    for mode in [ExecutionMode::Live, ExecutionMode::Replay] {
        let mut state = current.state().clone();
        state.mode = mode;
        let current = model::checkpoint(current.basis(), state).unwrap();
        let original = current.clone();
        let input = command(&current, first, 6, "ask-courier", vec![]);
        let staged = journey::stage(&current, &input).unwrap();
        assert_registration_refused(&current, &staged, first, CourierError::Unavailable);
        assert_eq!(
            model::stage_registered_journey(&current, &input),
            Err(RepositoryError::InvalidCandidate)
        );
        assert_eq!(current, original);
    }
}

#[test]
fn canonical_unregistered_kind_and_definition_do_not_admit_source_effects() {
    let (current, first, _) = opening();
    let staged = pending(&current, first);
    for case in 0..2 {
        let mut state = staged.state().clone();
        if case == 0 {
            state.intents[0].kind = EffectKind::RunMedia;
        } else {
            state.intents[0].definition = model::content("escort-courier").unwrap();
        }
        let candidate = model::checkpoint(staged.basis(), state).unwrap();
        assert_registration_refused(&current, &candidate, first, CourierError::Unavailable);
    }
}

#[test]
fn canonical_wrong_source_recipient_generation_and_slot_do_not_start_a_terminal_route() {
    let (current, first, second) = opening();
    let staged = pending(&current, first);
    for case in 0..6 {
        let mut state = staged.state().clone();
        let effect = state.intents[0].clone();
        let source = state
            .facts
            .iter_mut()
            .find(|fact| {
                fact.operation == effect.operation
                    && matches!(&fact.value, FactValue::ContentEvent { definition, .. }
                    if *definition == model::content("private-courier-note").unwrap())
            })
            .unwrap();
        let expected = match case {
            0 => {
                state.intents[0].generation = 2;
                CourierError::Stale
            }
            1 => {
                state.intents[0].slot = 1;
                CourierError::Stale
            }
            2 => {
                source.audience = AudienceScope::Shared;
                CourierError::Source
            }
            3 => {
                source.audience = AudienceScope::Members(vec![first, second]);
                CourierError::Source
            }
            4 => {
                source.value = FactValue::ContentEvent {
                    definition: model::content("escort-courier").unwrap(),
                    subjects: vec![],
                };
                CourierError::Source
            }
            5 => {
                state
                    .decisions
                    .iter_mut()
                    .find(|decision| decision.operation == effect.operation)
                    .unwrap()
                    .source_policy = model::label("unregistered-courier-source").unwrap();
                CourierError::Source
            }
            _ => unreachable!(),
        };
        let candidate = model::checkpoint(staged.basis(), state).unwrap();
        assert_registration_refused(&current, &candidate, first, expected);
    }
}

#[test]
fn canonical_bootstrap_room_and_switched_joined_recipient_are_refused_at_source_admission() {
    let (current, first, second) = opening();
    let staged = pending(&current, first);
    let bootstrap = MemberId::from_bytes(&PLAYER).unwrap();
    assert_eq!(
        current
            .state()
            .members
            .iter()
            .find(|link| link.member == bootstrap)
            .unwrap()
            .character,
        Some(journey::room_entity().unwrap())
    );
    for member in [bootstrap, second] {
        let mut state = staged.state().clone();
        let effect = state.intents[0].clone();
        state
            .facts
            .iter_mut()
            .find(|fact| {
                fact.operation == effect.operation
                    && matches!(&fact.value, FactValue::ContentEvent { definition, .. }
                    if *definition == model::content("private-courier-note").unwrap())
            })
            .unwrap()
            .audience = AudienceScope::Members(vec![member]);
        let candidate = model::checkpoint(staged.basis(), state).unwrap();
        // Recovery still follows committed source; new admission also binds the actual caller.
        assert_eq!(
            recipient(&candidate, &candidate.state().intents[0]),
            Ok(member)
        );
        assert_eq!(candidate.state().members, current.state().members);
        assert_registration_refused(&current, &candidate, first, CourierError::Source);
    }
}

#[test]
fn matching_bootstrap_source_member_with_actual_room_character_is_not_a_joined_participant() {
    let (current, first, _) = opening();
    let staged = pending(&current, first);
    let bootstrap = MemberId::from_bytes(&PLAYER).unwrap();
    let mut state = staged.state().clone();
    let operation = state.intents[0].operation;
    state
        .facts
        .iter_mut()
        .find(|fact| {
            fact.operation == operation
                && matches!(&fact.value, FactValue::ContentEvent { definition, .. }
                if *definition == model::content("private-courier-note").unwrap())
        })
        .unwrap()
        .audience = AudienceScope::Members(vec![bootstrap]);
    let candidate = model::checkpoint(staged.basis(), state).unwrap();
    assert_eq!(
        recipient(&candidate, &candidate.state().intents[0]),
        Ok(bootstrap)
    );
    assert_eq!(candidate.state().members, current.state().members);
    assert_registration_refused(&current, &candidate, bootstrap, CourierError::Source);
}

#[test]
fn canonical_nonpending_stale_and_oversized_batches_are_refused_by_declaration() {
    let (current, first, _) = opening();
    let staged = pending(&current, first);
    let original = current.clone();
    for case in 0..3 {
        let mut state = staged.state().clone();
        let operation = state.intents[0].operation;
        let expected = match case {
            0 => {
                state.intents[0].status = DurableStatus::Completed;
                EffectEmissionError::NotPending
            }
            1 => {
                state.intents[0].basis = current.basis();
                EffectEmissionError::StaleEffect
            }
            2 => {
                let mut extra = state.intents[0].clone();
                extra.id = EffectId::from_bytes(&[99; 16]).unwrap();
                extra.job = Some(JobId::from_bytes(&[99; 16]).unwrap());
                extra.slot = 1;
                state
                    .decisions
                    .iter_mut()
                    .find(|decision| decision.operation == operation)
                    .unwrap()
                    .effects
                    .push(extra.id);
                state.intents.push(extra);
                EffectEmissionError::Capacity
            }
            _ => unreachable!(),
        };
        let candidate = model::checkpoint(staged.basis(), state).unwrap();
        let before_inspection = candidate.clone();
        let GameInput::Game(source) = command(&current, first, 6, "ask-courier", vec![]) else {
            panic!("source command")
        };
        assert_eq!(
            inspect_candidate(&current, &candidate, &source),
            Err(EffectInspectionError::Declaration(expected))
        );
        assert_eq!(candidate, before_inspection);
        assert_eq!(current, original);
    }
}

#[test]
fn no_effect_native_journey_candidates_remain_exact_in_each_execution_mode() {
    let (current, first, _) = opening();
    for mode in [
        ExecutionMode::PreparedOnly,
        ExecutionMode::Live,
        ExecutionMode::Replay,
    ] {
        let mut state = current.state().clone();
        state.mode = mode;
        let current = model::checkpoint(current.basis(), state).unwrap();
        let original = current.clone();
        let input = command(&current, first, 6, "escort-courier", vec![]);
        let uninspected = journey::stage(&current, &input).unwrap();
        let inspected = model::stage_registered_journey(&current, &input).unwrap();
        assert_eq!(inspected, uninspected);
        assert!(inspected.state().intents.is_empty());
        assert_eq!(current, original);
    }
}
