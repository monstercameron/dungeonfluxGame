#![cfg(not(target_arch = "wasm32"))]

#[path = "support/engine_threat.rs"]
mod engine_threat;

use df_knowledge::perception::{ObserverScope, PerceptionError};
use df_model::checkpoint::*;
use df_narrative::{AdmittedThreatEvidence, ThreatRelevanceError, select_threat_relevance};
use df_session::submission::{RepositoryError, SubmissionOutcome};
use df_types::{BuildIdentity, RecoveryEpoch, SessionRevision};
use df_world::{DueSelectionError, DueSelectionLimits, DueSelectionRequest, select_due_events};
use engine_threat::{Fixture, Options, exercise, fact, limits, model, selected};
use std::time::Duration;

#[test]
fn registered_engine_session_commit_publishes_only_permitted_accepted_threat_evidence() {
    let fixture = Fixture::new();
    let initial = fixture.initial();
    let original = initial.clone();
    let result = exercise(
        &fixture,
        &initial,
        Options {
            retry: true,
            ..Options::default()
        },
    );
    let [
        SubmissionOutcome::Confirmed(first),
        SubmissionOutcome::Confirmed(retry),
    ] = result.outcomes.as_slice()
    else {
        panic!("confirmed exact retry")
    };
    assert_eq!(first, retry);
    assert_eq!(first.decision().source_policy, fixture.registration.policy);
    assert!(first.decision().facts.is_empty());
    assert!(first.decision().draws.is_empty());
    assert_eq!(result.source_calls, 1);
    assert_eq!(initial, original);
    assert_eq!(
        result.current.basis().revision,
        initial.basis().revision.next_sequence().unwrap()
    );
    assert_eq!(result.current.state().facts, initial.state().facts);
    assert_eq!(result.current.state().threats, initial.state().threats);
    assert_eq!(
        result.current.state().logical_time,
        initial.state().logical_time
    );
    assert_eq!(result.current.state().draws, initial.state().draws);
    assert_eq!(result.current.state().knowledge, initial.state().knowledge);
    assert_eq!(
        result.current.state().continuity.environment[0].change_facts,
        vec![fact(30), fact(31), fact(32)]
    );
    for id in [fact(31), fact(32)] {
        let cause = result
            .current
            .state()
            .facts
            .iter()
            .find(|cause| cause.id == id)
            .unwrap();
        assert!(
            result
                .current
                .state()
                .decisions
                .iter()
                .any(|decision| decision.operation == cause.operation
                    && decision.revision == cause.revision
                    && decision.facts.contains(&id)
                    && decision.source_policy == model::label("accepted-storm-source"))
        );
    }
    let observed = result.observed.borrow();
    assert_eq!(observed.durable, result.current);
    assert_eq!(observed.shared, vec![vec![fact(32)]]);
    assert_eq!(observed.private, vec![vec![fact(31), fact(32)]]);
    assert_eq!(
        observed.events,
        vec![
            "lookup",
            "commit",
            "publish-threat-evidence",
            "wake",
            "lookup",
            "reload"
        ]
    );
    assert_eq!(result.recovered.as_ref(), Some(&result.current));
    assert_eq!(result.recovered_shared, vec![fact(32)]);
    assert_eq!(result.recovery_error, None);
}

#[test]
fn real_session_reload_rejects_changed_full_build_pins_before_recovered_consumption() {
    let fixture = Fixture::new();
    let initial = fixture.initial();
    let result = exercise(
        &fixture,
        &initial,
        Options {
            changed_recovery_pins: true,
            ..Options::default()
        },
    );
    assert!(matches!(
        result.outcomes.as_slice(),
        [SubmissionOutcome::Confirmed(_)]
    ));
    assert_eq!(
        result.recovery_error,
        Some(RepositoryError::InvalidCandidate)
    );
    assert!(result.recovered.is_none());
    assert!(result.recovered_shared.is_empty());
    assert_eq!(result.observed.borrow().shared, vec![vec![fact(32)]]);
    assert_eq!(result.current.state().threats, initial.state().threats);
    assert_eq!(result.source_calls, 1);
}

#[test]
fn current_accepted_source_policy_withdrawal_refuses_registered_stage_before_commit() {
    let fixture = Fixture::new();
    let initial = fixture.initial();
    let mut state = initial.state().clone();
    state
        .decisions
        .iter_mut()
        .find(|decision| decision.facts.contains(&fact(32)))
        .unwrap()
        .source_policy = model::label("withdrawn-storm-source");
    let withdrawn = fixture.checkpoint(initial.basis(), state);
    let result = exercise(&fixture, &withdrawn, Options::default());
    assert!(matches!(
        result.outcomes.as_slice(),
        [SubmissionOutcome::Refused(_)]
    ));
    assert_eq!(result.current, withdrawn);
    assert_eq!(result.observed.borrow().events, vec!["lookup"]);
    assert!(result.observed.borrow().shared.is_empty());
    assert_eq!(result.source_calls, 1);
}

#[test]
fn source_mapping_access_and_known_commit_refusals_publish_nothing_and_draw_nothing() {
    let fixture = Fixture::new();
    let initial = fixture.initial();
    for options in [
        Options {
            refuse_source: true,
            ..Options::default()
        },
        Options {
            missing_producer: true,
            ..Options::default()
        },
        Options {
            missing_selector: true,
            ..Options::default()
        },
        Options {
            fail_commit: true,
            ..Options::default()
        },
        Options {
            revoke_access: true,
            ..Options::default()
        },
    ] {
        let result = exercise(&fixture, &initial, options);
        assert!(matches!(
            result.outcomes.as_slice(),
            [SubmissionOutcome::Refused(_)]
        ));
        assert_eq!(result.current, initial);
        let observed = result.observed.borrow();
        assert_eq!(observed.durable, initial);
        assert!(observed.shared.is_empty());
        assert!(observed.private.is_empty());
        assert!(!observed.events.contains(&"publish-threat-evidence"));
        assert!(!observed.events.contains(&"wake"));
        assert!(result.recovered.is_none());
        assert_eq!(result.current.state().draws, initial.state().draws);
        assert_eq!(
            result.source_calls,
            usize::from(options.refuse_source || options.fail_commit)
        );
        if options.fail_commit {
            assert_eq!(
                result.outcomes,
                vec![SubmissionOutcome::Refused(RepositoryError::Unavailable)]
            );
            assert_eq!(observed.events, vec!["lookup", "commit"]);
        } else {
            assert_eq!(observed.events, vec!["lookup"]);
        }
    }
}

#[test]
fn explicitly_admitted_empty_change_commits_only_operation_and_keeps_threat_time_and_facts() {
    let fixture = Fixture::new();
    let initial = fixture.initial();
    let result = exercise(
        &fixture,
        &initial,
        Options {
            no_changes: true,
            ..Options::default()
        },
    );
    assert!(matches!(
        result.outcomes.as_slice(),
        [SubmissionOutcome::Confirmed(_)]
    ));
    let mut expected = initial.state().clone();
    expected.decisions = result.current.state().decisions.clone();
    assert_eq!(result.current.state(), &expected);
    assert_eq!(result.source_calls, 1);
    assert_eq!(result.recovered.as_ref(), Some(&result.current));
}

#[test]
fn recovered_consumer_rechecks_current_audience_membership_and_source_admission() {
    let fixture = Fixture::new();
    let result = exercise(&fixture, &fixture.initial(), Options::default());
    let recovered = result.recovered.unwrap();
    let original = recovered.clone();
    let private = [fact(31)];
    let mapping = [AdmittedThreatEvidence {
        expected: &recovered.state().threats[0],
        evidence: &private,
    }];
    let mut state = recovered.state().clone();
    state
        .facts
        .iter_mut()
        .find(|cause| cause.id == fact(31))
        .unwrap()
        .audience = AudienceScope::Host;
    let withdrawn = fixture.checkpoint(recovered.basis(), state.clone());
    let selection = select_threat_relevance(
        &withdrawn,
        fixture.request(
            &withdrawn,
            Some(&mapping),
            ObserverScope::Member(model::member(3)),
        ),
        limits(),
    )
    .unwrap();
    assert!(selection.facts().is_empty());
    assert_eq!(
        selected(&fixture, &withdrawn, ObserverScope::Shared).unwrap(),
        vec![fact(32)]
    );
    state.members.clear();
    state.characters.clear();
    let removed = fixture.checkpoint(recovered.basis(), state);
    assert_eq!(
        selected(&fixture, &removed, ObserverScope::Member(model::member(3))).unwrap_err(),
        ThreatRelevanceError::Perception(PerceptionError::ObserverUnavailable)
    );
    let shared = [fact(32)];
    let mapping = [AdmittedThreatEvidence {
        expected: &recovered.state().threats[0],
        evidence: &shared,
    }];
    let narrowed = [fixture.contents[0].clone(), fixture.contents[1].clone()];
    let mut request = fixture.request(&recovered, Some(&mapping), ObserverScope::Shared);
    request.admitted_content = &narrowed;
    assert_eq!(
        select_threat_relevance(&recovered, request, limits()).unwrap_err(),
        ThreatRelevanceError::InvalidAdmission
    );
    let mut request = fixture.request(&recovered, Some(&mapping), ObserverScope::Shared);
    request.expected_policy = &fixture.contents[1];
    assert_eq!(
        select_threat_relevance(&recovered, request, limits()).unwrap_err(),
        ThreatRelevanceError::InvalidAdmission
    );
    assert_eq!(recovered, original);
}

#[test]
fn committed_recovered_consumer_refuses_stale_basis_epoch_pins_and_clock_mapping() {
    let fixture = Fixture::new();
    let initial = fixture.initial();
    let result = exercise(&fixture, &initial, Options::default());
    let recovered = result.recovered.unwrap();
    let original = recovered.clone();
    let evidence = [fact(32)];
    let mapping = [AdmittedThreatEvidence {
        expected: &recovered.state().threats[0],
        evidence: &evidence,
    }];
    for basis in [
        initial.basis(),
        Basis {
            revision: SessionRevision::new(RecoveryEpoch::new(99).unwrap(), 9),
            ..recovered.basis()
        },
    ] {
        let mut request = fixture.request(&recovered, Some(&mapping), ObserverScope::Shared);
        request.expected_basis = basis;
        assert_eq!(
            select_threat_relevance(&recovered, request, limits()).unwrap_err(),
            ThreatRelevanceError::Checkpoint(CheckpointError::StaleBasis)
        );
    }
    for component in 0..3 {
        let mut pins = recovered.pins().clone();
        let expected = match component {
            0 => {
                pins.rules.source_manifest_digest = ContentDigest([99; 32]);
                CheckpointError::RulesMismatch
            }
            1 => {
                pins.content.package_digest = ContentDigest([99; 32]);
                CheckpointError::ContentMismatch
            }
            2 => {
                pins.build = BuildIdentity::new(
                    Some("changed-source"),
                    Some("fixture-native-1"),
                    Some("fixture-wasm-1"),
                    Some("fixture-config-1"),
                    Some("fixture-content-1"),
                )
                .unwrap();
                CheckpointError::BuildMismatch
            }
            _ => unreachable!(),
        };
        let mut request = fixture.request(&recovered, Some(&mapping), ObserverScope::Shared);
        request.admitted_pins = &pins;
        assert_eq!(
            select_threat_relevance(&recovered, request, limits()).unwrap_err(),
            ThreatRelevanceError::Checkpoint(expected)
        );
    }
    let mut changed = recovered.state().clone();
    changed.threats[0].progress += 1;
    let changed = fixture.checkpoint(recovered.basis(), changed);
    assert_eq!(
        select_threat_relevance(
            &changed,
            fixture.request(&changed, Some(&mapping), ObserverScope::Shared),
            limits()
        )
        .unwrap_err(),
        ThreatRelevanceError::InvalidAdmission
    );
    assert_eq!(recovered, original);
}

#[test]
fn recovered_consumer_missing_mapping_and_work_output_limits_return_no_partial_result() {
    let fixture = Fixture::new();
    let result = exercise(&fixture, &fixture.initial(), Options::default());
    let recovered = result.recovered.unwrap();
    let original = recovered.clone();
    assert_eq!(
        select_threat_relevance(
            &recovered,
            fixture.request(&recovered, None, ObserverScope::Shared),
            limits()
        )
        .unwrap_err(),
        ThreatRelevanceError::NotAdmitted
    );
    let empty = select_threat_relevance(
        &recovered,
        fixture.request(&recovered, Some(&[]), ObserverScope::Shared),
        limits(),
    )
    .unwrap();
    assert!(empty.facts().is_empty());
    let evidence = [fact(32)];
    let mapping = [AdmittedThreatEvidence {
        expected: &recovered.state().threats[0],
        evidence: &evidence,
    }];
    for (bounded, expected) in [
        (
            df_narrative::ThreatRelevanceLimits {
                maximum_work: 0,
                ..limits()
            },
            ThreatRelevanceError::WorkCapacity,
        ),
        (
            df_narrative::ThreatRelevanceLimits {
                maximum_output_bytes: 0,
                ..limits()
            },
            ThreatRelevanceError::OutputCapacity,
        ),
        (
            df_narrative::ThreatRelevanceLimits {
                maximum_candidates: 0,
                ..limits()
            },
            ThreatRelevanceError::InputCapacity,
        ),
    ] {
        assert_eq!(
            select_threat_relevance(
                &recovered,
                fixture.request(&recovered, Some(&mapping), ObserverScope::Shared),
                bounded
            )
            .unwrap_err(),
            expected
        );
    }
    assert_eq!(recovered, original);
}

#[test]
fn paused_world_selection_and_native_deadline_never_advance_recovered_threat_or_time() {
    let fixture = Fixture::new();
    let result = exercise(&fixture, &fixture.initial(), Options::default());
    let recovered = result.recovered.unwrap();
    let original = recovered.clone();
    let world_limits = DueSelectionLimits {
        queue_events: 8,
        selected_events: 4,
        output_bytes: 4096,
    };
    let request = |ticks, deadline_remaining| DueSelectionRequest {
        expected_basis: recovered.basis(),
        target_time: LogicalTime {
            ticks,
            ..recovered.state().logical_time
        },
        paused: true,
        deadline_remaining,
        policy: &fixture.contents[0],
    };
    let paused = select_due_events(
        &recovered,
        &fixture.pins,
        request(recovered.state().logical_time.ticks, Duration::from_secs(1)),
        world_limits,
    )
    .unwrap();
    assert_eq!(paused.proposed_time, recovered.state().logical_time);
    assert!(paused.events.is_empty());
    assert_eq!(
        select_due_events(
            &recovered,
            &fixture.pins,
            request(
                recovered.state().logical_time.ticks + 1,
                Duration::from_secs(1)
            ),
            world_limits
        )
        .unwrap_err(),
        DueSelectionError::InvalidTime
    );
    assert_eq!(
        select_due_events(
            &recovered,
            &fixture.pins,
            request(recovered.state().logical_time.ticks, Duration::ZERO),
            world_limits
        )
        .unwrap_err(),
        DueSelectionError::Deadline
    );
    assert_eq!(
        selected(&fixture, &recovered, ObserverScope::Shared).unwrap(),
        vec![fact(32)]
    );
    assert_eq!(recovered, original);
}
