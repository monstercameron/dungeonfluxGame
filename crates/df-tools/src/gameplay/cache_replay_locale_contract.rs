//! Executable Design contract for the registered prepared-only courier consumer.
use super::*;
use df_ai::lookup::{LookupError, lookup_replay};
use df_types::{BuildIdentity, LocaleTag};

fn authored(current: &Checkpoint) -> AuthoredRecording {
    AuthoredRecording {
        key: model::content(DEFINITION).unwrap(),
        basis: RecordingBasis {
            pins: current.pins().clone(),
            policy: model::content(POLICY).unwrap(),
            model: model::label(MODEL).unwrap(),
            source_locale: LocaleTag::parse(SOURCE_LOCALE).unwrap(),
        },
    }
}

fn complete_events(
    effect: &DurableIntent,
    text: &str,
) -> [Result<RecordEvent<JobId, Basis>, RecordInputError>; 2] {
    [
        Ok(RecordEvent::Chunk(text.as_bytes().to_vec())),
        Ok(RecordEvent::Complete {
            identity: RecordIdentity {
                key: effect.job.unwrap(),
                basis: effect.basis,
                operation: effect.operation,
            },
            byte_length: text.len() as u64,
            sha256: Sha256::digest(text.as_bytes()).into(),
        }),
    ]
}

fn withdraw_source(current: &Checkpoint, case: u8) -> Checkpoint {
    let mut state = current.state().clone();
    match case {
        0 => {
            state
                .facts
                .iter_mut()
                .find(|fact| {
                    fact.operation == intent(current).operation
                        && matches!(fact.value, FactValue::ContentEvent { .. })
                })
                .unwrap()
                .audience = AudienceScope::Shared;
        }
        1 => {
            state
                .facts
                .iter_mut()
                .find(|fact| {
                    fact.operation == intent(current).operation
                        && matches!(fact.value, FactValue::ContentEvent { .. })
                })
                .unwrap()
                .audience = AudienceScope::Host
        }
        2 => {
            state
                .facts
                .iter_mut()
                .find(|fact| {
                    fact.operation == intent(current).operation
                        && matches!(fact.value, FactValue::ContentEvent { .. })
                })
                .unwrap()
                .value = FactValue::ContentEvent {
                definition: model::content("escort-courier").unwrap(),
                subjects: vec![],
            }
        }
        3 => {
            state
                .decisions
                .iter_mut()
                .find(|decision| decision.operation == intent(current).operation)
                .unwrap()
                .source_policy = model::label("unadmitted-source-policy").unwrap()
        }
        _ => unreachable!(),
    }
    model::checkpoint(current.basis(), state).unwrap()
}

#[test]
fn exact_prepared_entry_is_borrowed_and_complete_before_candidate_admission() {
    let (current, first, second) = pending();
    let original = current.clone();
    let recording = authored(&current);
    let text = lookup_prepared(&recording, &recording.key, &recording.basis).unwrap();
    assert_eq!(*text, RESPONSE);
    assert_eq!(recording.basis.source_locale.as_str(), "en");
    let completion = admit_record(
        &current,
        intent(&current),
        complete_events(intent(&current), text),
    )
    .unwrap();
    assert_eq!(completion, execute(&current, intent(&current)).unwrap());
    assert_eq!(current, original);
    assert!(player_clue(&current, first).is_empty());
    assert!(player_clue(&current, second).is_empty());
    let completed = stage_completion(
        &current,
        &completion,
        completion_operation(intent(&current)).unwrap(),
    )
    .unwrap();
    assert_eq!(player_clue(&completed, first), RESPONSE);
    assert!(player_clue(&completed, second).is_empty());
}

#[test]
fn complete_source_basis_and_key_mismatches_are_stale_without_fallback() {
    let (current, _, _) = pending();
    let recording = authored(&current);
    for case in 0..15 {
        let mut basis = recording.basis.clone();
        let changed = model::label("different-admitted-source").unwrap();
        match case {
            0 => basis.pins.rules.mode = RulesMode::DisclosedCustom,
            1 => basis.pins.rules.ruleset = changed,
            2 => basis.pins.rules.catalog = changed,
            3 => basis.pins.rules.catalog_digest.0[0] ^= 1,
            4 => basis.pins.rules.source_manifest = changed,
            5 => basis.pins.rules.source_manifest_digest.0[0] ^= 1,
            6 => basis.pins.rules.handler = changed,
            7 => basis.pins.rules.handler_digest.0[0] ^= 1,
            8 => basis.pins.content.content = changed,
            9 => basis.pins.content.content_digest.0[0] ^= 1,
            10 => basis.pins.content.package = changed,
            11 => basis.pins.content.package_digest.0[0] ^= 1,
            12 => {
                basis.pins.build = BuildIdentity::new(
                    Some("other-source"),
                    Some("other-native"),
                    Some("other-wasm"),
                    Some("other-config"),
                    Some("other-content"),
                )
                .unwrap()
            }
            13 => basis.policy = model::content("harbor").unwrap(),
            14 => basis.model = changed,
            _ => unreachable!(),
        }
        assert_eq!(
            lookup_prepared(&recording, &recording.key, &basis),
            Err(LookupError::Stale),
            "case {case}"
        );
        assert_eq!(
            lookup_replay(&recording, &recording.key, &basis),
            Err(LookupError::Missing)
        );
    }
    assert_eq!(
        lookup_prepared(
            &recording,
            &model::content("harbor").unwrap(),
            &recording.basis
        ),
        Err(LookupError::Stale)
    );
    assert_eq!(
        *lookup_prepared(&recording, &recording.key, &recording.basis).unwrap(),
        RESPONSE
    );
}

#[test]
fn locale_case_normalization_preserves_exact_authored_source_identity() {
    let (current, _, _) = pending();
    let recording = authored(&current);
    let mut basis = recording.basis.clone();
    basis.source_locale = LocaleTag::parse("EN").unwrap();
    assert_eq!(
        *lookup_prepared(&recording, &recording.key, &basis).unwrap(),
        RESPONSE
    );
    for locale in ["en-US", "en-GB", "fr", "iw", "he"] {
        basis.source_locale = LocaleTag::parse(locale).unwrap();
        assert_eq!(
            lookup_prepared(&recording, &recording.key, &basis),
            Err(LookupError::Stale),
            "{locale}"
        );
    }
    assert_eq!(recording.basis.source_locale.as_str(), "en");
}

#[test]
fn recording_replay_missing_and_unapproved_modes_never_select_prepared_fallback() {
    let (current, _, _) = pending();
    let recording = authored(&current);
    assert_eq!(
        lookup_replay(&recording, &recording.key, &recording.basis),
        Err(LookupError::Missing)
    );
    assert!(lookup_prepared(&recording, &recording.key, &recording.basis).is_ok());
    for mode in [ExecutionMode::Replay, ExecutionMode::Live] {
        let mut state = current.state().clone();
        state.mode = mode;
        let changed = model::checkpoint(current.basis(), state).unwrap();
        let original = changed.clone();
        assert_eq!(
            execute(&changed, intent(&changed)),
            Err(CourierError::Unavailable)
        );
        assert_eq!(changed, original);
    }
}

#[test]
fn complete_prepared_bytes_cannot_publish_after_current_access_or_source_withdrawal() {
    let (current, _, _) = pending();
    let recording = authored(&current);
    let text = lookup_prepared(&recording, &recording.key, &recording.basis).unwrap();
    let completion = admit_record(
        &current,
        intent(&current),
        complete_events(intent(&current), text),
    )
    .unwrap();
    for case in 0..4 {
        let changed = withdraw_source(&current, case);
        let original = changed.clone();
        assert!(
            admit_record(
                &changed,
                intent(&changed),
                complete_events(intent(&changed), text)
            )
            .is_err(),
            "case {case}"
        );
        assert_eq!(
            stage_completion(
                &changed,
                &completion,
                completion_operation(intent(&changed)).unwrap()
            ),
            Err(CourierError::Source),
            "case {case}"
        );
        assert_eq!(changed, original);
        assert!(
            changed
                .state()
                .decisions
                .iter()
                .all(|decision| decision.operation
                    != completion_operation(intent(&changed)).unwrap())
        );
    }
}

#[test]
fn prior_semantic_output_cannot_restore_withdrawn_access_or_source() {
    let (completed, first, _) = completed_answer();
    assert_eq!(saved_response(&completed, first).unwrap(), Some(RESPONSE));
    let operation = completion_operation(intent(&completed)).unwrap();
    for case in 0..4 {
        let restored = withdraw_source(&completed, case);
        let original = restored.clone();
        assert_eq!(
            restored
                .state()
                .decisions
                .iter()
                .find(|decision| decision.operation == operation)
                .unwrap()
                .semantic_output
                .as_deref(),
            Some(RESPONSE)
        );
        assert_eq!(
            saved_response(&restored, first),
            Err(RepositoryError::InvalidCandidate),
            "case {case}"
        );
        match wire::journey_view(&restored, LocalDemoRole::Player, first) {
            Err(error) => assert_eq!(error, RepositoryError::InvalidCandidate),
            Ok(view) => {
                let Some(crate::gameplay::rpc::view_message::Audience::Player(player)) =
                    view.audience
                else {
                    panic!("player view")
                };
                // A source no longer perceived as the private note is an empty safe view.
                assert!(player.private_clue.is_empty(), "case {case}");
            }
        }
        assert_eq!(restored, original);
    }
}

#[test]
fn display_and_other_members_receive_no_private_cached_response() {
    let (completed, first, second) = completed_answer();
    assert_eq!(saved_response(&completed, first).unwrap(), Some(RESPONSE));
    assert_eq!(saved_response(&completed, second).unwrap(), None);
    assert!(player_clue(&completed, second).is_empty());
    let display = wire::journey_view(&completed, LocalDemoRole::Display, first).unwrap();
    assert!(matches!(
        &display.audience,
        Some(crate::gameplay::rpc::view_message::Audience::Display(_))
    ));
    assert!(!format!("{display:?}").contains(RESPONSE));
    assert_eq!(
        completed,
        model::checkpoint(completed.basis(), completed.state().clone()).unwrap()
    );
}

#[test]
fn durable_receipt_retry_does_not_reexecute_or_grant_current_display_access() {
    let (current, first, _) = pending();
    let effect = intent(&current).clone();
    let input = GameInput::Job(expected_completion(&effect).unwrap());
    let operation = completion_operation(&effect).unwrap();
    let stored = Rc::new(RefCell::new(Stored {
        checkpoint: current.clone(),
        receipts: vec![],
        commit: Commit::Confirm,
        events: vec![],
        unavailable_lookups: 0,
    }));
    let (mut session, _) = owner(&stored, current);
    assert!(matches!(
        submit(&mut session, &input, operation),
        SubmissionOutcome::Confirmed(_)
    ));
    assert_eq!(
        stored
            .borrow()
            .events
            .iter()
            .filter(|event| **event == "execute")
            .count(),
        1
    );
    let revoked = withdraw_source(session.checkpoint(), 0);
    stored.borrow_mut().checkpoint = revoked.clone();
    let before_events = stored.borrow().events.clone();
    let (mut restarted, _) = owner(&stored, revoked.clone());
    assert!(matches!(
        submit(&mut restarted, &input, operation),
        SubmissionOutcome::Confirmed(_)
    ));
    assert_eq!(stored.borrow().events, before_events);
    assert_eq!(stored.borrow().receipts.len(), 1);
    assert_eq!(stored.borrow().checkpoint, revoked);
    assert_eq!(
        saved_response(restarted.checkpoint(), first),
        Err(RepositoryError::InvalidCandidate)
    );
    assert!(wire::journey_view(restarted.checkpoint(), LocalDemoRole::Player, first).is_err());
}

#[test]
fn cancellation_after_prepared_read_cannot_admit_or_stage_old_complete_bytes() {
    let (current, _, _) = pending();
    let recording = authored(&current);
    let text = lookup_prepared(&recording, &recording.key, &recording.basis).unwrap();
    let completion = execute(&current, intent(&current)).unwrap();
    let mut state = current.state().clone();
    state.intents[0].status = DurableStatus::Cancelled;
    let cancelled = model::checkpoint(current.basis(), state).unwrap();
    let original = cancelled.clone();
    assert!(
        admit_record(
            &cancelled,
            intent(&cancelled),
            complete_events(intent(&cancelled), text)
        )
        .is_err()
    );
    assert_eq!(
        stage_completion(
            &cancelled,
            &completion,
            completion_operation(intent(&cancelled)).unwrap()
        ),
        Err(CourierError::Stale)
    );
    assert_eq!(cancelled, original);
}
