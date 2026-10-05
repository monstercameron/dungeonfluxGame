use super::*;
use crate::gameplay::{courier_ai, journey, model};
use df_model::checkpoint::*;
use df_persistence::local_demo_scope::{DISPLAY, LocalDemoRole, PLAYER};
use df_types::{MemberId, OperationId, RevisionLabel};
use prost::Message;

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
fn before_answer() -> (Checkpoint, MemberId, MemberId) {
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
    current = journey::stage(
        &current,
        &command(&current, first, 3, "create-character", build("Brynn")),
    )
    .unwrap();
    current = journey::stage(
        &current,
        &command(&current, second, 4, "create-character", build("Vale")),
    )
    .unwrap();
    current = journey::stage(
        &current,
        &command(&current, first, 5, "begin-story", vec![]),
    )
    .unwrap();
    (current, first, second)
}
fn pending() -> (Checkpoint, MemberId, MemberId) {
    let (current, first, second) = before_answer();
    (
        journey::stage(
            &current,
            &command(&current, first, 6, "ask-courier", vec![]),
        )
        .unwrap(),
        first,
        second,
    )
}

fn completed() -> (Checkpoint, MemberId, MemberId) {
    let (current, first, second) = pending();
    let intent = current.state().intents.first().unwrap();
    let completion = courier_ai::execute(&current, intent).unwrap();
    let next = courier_ai::stage_completion(
        &current,
        &completion,
        courier_ai::completion_operation(intent).unwrap(),
    )
    .unwrap();
    (next, first, second)
}

fn player(current: &Checkpoint, member: MemberId) -> rpc::PlayerGameplayView {
    let projected = journey_view(current, LocalDemoRole::Player, member).unwrap();
    let Some(rpc::view_message::Audience::Player(view)) = projected.audience else {
        panic!("player projection required")
    };
    view
}

fn display(current: &Checkpoint) -> rpc::ViewMessage {
    journey_view(
        current,
        LocalDemoRole::Display,
        MemberId::from_bytes(&DISPLAY).unwrap(),
    )
    .unwrap()
}

fn note_mut(state: &mut GameState) -> &mut GameFact {
    state.facts.iter_mut().find(|fact| matches!(
        &fact.value,
        FactValue::ContentEvent { definition, .. } if definition.entry.as_str() == "private-courier-note"
    )).unwrap()
}

#[test]
fn real_prepared_completion_projects_exact_source_to_only_its_current_recipient() {
    let (pending, first, second) = pending();
    let before = pending.clone();
    let first_pending = player(&pending, first);
    let second_pending = player(&pending, second);
    let shared_pending = display(&pending);
    assert!(first_pending.private_clue.is_empty());
    assert!(second_pending.private_clue.is_empty());
    let intent = pending.state().intents.first().unwrap();
    let completion = courier_ai::execute(&pending, intent).unwrap();
    let current = courier_ai::stage_completion(
        &pending,
        &completion,
        courier_ai::completion_operation(intent).unwrap(),
    )
    .unwrap();
    let mut first_current = player(&current, first);
    assert_eq!(first_current.private_clue, courier_ai::RESPONSE);
    first_current.private_clue.clear();
    assert_eq!(first_current, first_pending);
    assert_eq!(player(&current, second), second_pending);
    assert_eq!(display(&current).audience, shared_pending.audience);
    for encoded in [
        display(&current).encode_to_vec(),
        player(&current, second).encode_to_vec(),
    ] {
        assert!(
            !encoded
                .windows(courier_ai::RESPONSE.len())
                .any(|bytes| bytes == courier_ai::RESPONSE.as_bytes())
        );
        assert!(
            !encoded
                .windows(b"private-courier-note".len())
                .any(|bytes| bytes == b"private-courier-note")
        );
    }
    assert_eq!(pending, before);
}

#[test]
fn current_canonical_audience_removal_omits_saved_cue_even_with_a_knowledge_grant() {
    let (current, first, second) = completed();
    let original = current.clone();
    let mut expected_first = player(&current, first);
    expected_first.private_clue.clear();
    let expected_second = player(&current, second);
    let expected_display = display(&current);
    for audience in [
        AudienceScope::Host,
        AudienceScope::Members(vec![MemberId::from_bytes(&PLAYER).unwrap()]),
    ] {
        let mut state = current.state().clone();
        let note = note_mut(&mut state);
        note.audience = audience;
        let id = note.id;
        state.knowledge.push(KnowledgeGrant {
            observer: first,
            fact: id,
            source: id,
        });
        let removed = model::checkpoint(current.basis(), state).unwrap();
        assert_eq!(player(&removed, first), expected_first);
        assert_eq!(player(&removed, second), expected_second);
        assert_eq!(display(&removed), expected_display);
        // A fresh projection, including a reconnect snapshot, never uses the old cue.
        assert!(player(&removed, first).private_clue.is_empty());
    }
    assert_eq!(current, original);
}

#[test]
fn unsupported_selected_fact_never_becomes_clue_text_and_saved_record_is_still_validated() {
    let (current, first, second) = completed();
    let mut state = current.state().clone();
    note_mut(&mut state).value = FactValue::ContentEvent {
        definition: model::content("harbor").unwrap(),
        subjects: vec![],
    };
    let unsupported = model::checkpoint(current.basis(), state).unwrap();
    assert!(player(&unsupported, first).private_clue.is_empty());
    assert!(player(&unsupported, second).private_clue.is_empty());
    assert_eq!(display(&unsupported), display(&current));
    let mut state = current.state().clone();
    let decision = state
        .decisions
        .iter_mut()
        .find(|decision| decision.source_policy.as_str() == courier_ai::POLICY)
        .unwrap();
    decision.semantic_output = Some("Unapproved text must never reach a private view".to_owned());
    let changed = model::checkpoint(current.basis(), state).unwrap();
    assert_eq!(
        journey_view(&changed, LocalDemoRole::Player, first),
        Err(RepositoryError::InvalidCandidate)
    );
    assert!(player(&changed, second).private_clue.is_empty());
    assert_eq!(display(&changed), display(&current));
}

fn rebuild(
    current: &Checkpoint,
    pins: CheckpointPins,
    state: GameState,
    maximum_records: usize,
) -> Checkpoint {
    let recovery = model::recovery().unwrap();
    let mut limits = model::limits();
    limits.maximum_records = maximum_records;
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        current.basis(),
        pins,
        state,
        ReferenceInventory {
            rules: &recovery.rules,
            content: &recovery.content,
            resources: &recovery.resources,
            assets: &recovery.assets,
        },
        limits,
    )
    .unwrap()
}

#[test]
fn real_projection_refuses_unadmitted_current_pins_and_fact_scan_overflow() {
    let (current, first, second) = completed();
    let mut pins = current.pins().clone();
    pins.content.content_digest.0[0] ^= 1;
    let stale = rebuild(&current, pins, current.state().clone(), 4096);
    let mut state = current.state().clone();
    for ordinal in 0_u32..513 {
        let mut id = [0x7f; 16];
        id[..4].copy_from_slice(&ordinal.to_be_bytes());
        state.facts.push(GameFact {
            id: FactId::from_bytes(&id).unwrap(),
            revision: current.basis().revision,
            operation: OperationId::from_bytes(&[0x78; 16]).unwrap(),
            ordinal,
            cause: None,
            audience: AudienceScope::Shared,
            value: FactValue::ContentEvent {
                definition: model::content("harbor").unwrap(),
                subjects: vec![],
            },
        });
    }
    let overflow = rebuild(&current, current.pins().clone(), state, 4096);
    for (role, principal) in [
        (
            LocalDemoRole::Display,
            MemberId::from_bytes(&DISPLAY).unwrap(),
        ),
        (LocalDemoRole::Player, first),
        (LocalDemoRole::Player, second),
    ] {
        assert_eq!(
            journey_view(&stale, role, principal),
            Err(RepositoryError::InvalidCandidate)
        );
        assert_eq!(
            journey_view(&overflow, role, principal),
            Err(RepositoryError::Capacity)
        );
    }
}

#[test]
fn authenticated_member_selection_requires_current_membership_at_real_projection() {
    let (current, _, _) = completed();
    let unknown = MemberId::from_bytes(&[0x79; 16]).unwrap();
    assert_eq!(
        journey_view(&current, LocalDemoRole::Player, unknown),
        Err(RepositoryError::Unauthorized)
    );
}

#[test]
fn selected_reveal_still_requires_the_original_exact_single_recipient_contract() {
    let (current, first, second) = completed();
    for audience in [
        AudienceScope::Shared,
        AudienceScope::Members(vec![first, second]),
    ] {
        let mut state = current.state().clone();
        note_mut(&mut state).audience = audience;
        let changed = model::checkpoint(current.basis(), state).unwrap();
        assert_eq!(
            journey_view(&changed, LocalDemoRole::Player, first),
            Err(RepositoryError::InvalidCandidate)
        );
        assert_eq!(
            journey_view(&changed, LocalDemoRole::Player, second),
            Err(RepositoryError::InvalidCandidate)
        );
        assert_eq!(display(&changed), display(&current));
    }
}
