use df_experience::pacing::*;
use df_model::checkpoint::{Checkpoint, CheckpointError, ExecutionMode};
use df_types::{MemberId, RunId, SessionId};
use fixture::*;
use std::time::Duration;

// Canonical checkpoint fixture; recommendations are consumed through the public crate boundary.
mod fixture {
    use df_model::checkpoint::*;
    use df_types::{
        BuildIdentity, MemberId, OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId,
        SessionRevision,
    };

    pub(crate) fn label(value: &str) -> RevisionLabel {
        RevisionLabel::new(Some(value)).unwrap()
    }
    pub(crate) fn content() -> ContentReference {
        ContentReference {
            package: label("fixture-package-1"),
            entry: label("fixture-entry-1"),
        }
    }
    pub(crate) fn revision(sequence: u64) -> SessionRevision {
        SessionRevision::new(RecoveryEpoch::new(2).unwrap(), sequence)
    }
    pub(crate) fn basis() -> Basis {
        Basis {
            session: SessionId::from_bytes(&[1; 16]).unwrap(),
            run: RunId::from_bytes(&[2; 16]).unwrap(),
            revision: revision(8),
        }
    }
    pub(crate) fn fact_id() -> FactId {
        FactId::from_bytes(&[5; 16]).unwrap()
    }
    pub(crate) fn pins() -> CheckpointPins {
        CheckpointPins {
            rules: RulesPins {
                mode: RulesMode::Standard2024,
                ruleset: label("fixture-rules-1"),
                catalog: label("fixture-catalog-1"),
                catalog_digest: ContentDigest([1; 32]),
                source_manifest: label("fixture-sources-1"),
                source_manifest_digest: ContentDigest([2; 32]),
                handler: label("fixture-handler-1"),
                handler_digest: ContentDigest([3; 32]),
            },
            content: ContentPins {
                content: label("fixture-content-1"),
                content_digest: ContentDigest([4; 32]),
                package: label("fixture-package-1"),
                package_digest: ContentDigest([5; 32]),
            },
            build: BuildIdentity::new(
                Some("fixture-source-1"),
                Some("fixture-native-1"),
                Some("fixture-wasm-1"),
                Some("fixture-config-1"),
                Some("fixture-content-1"),
            )
            .unwrap(),
        }
    }
    pub(crate) fn continuity() -> ContinuityState {
        ContinuityState {
            creation: vec![],
            simulation: vec![],
            catch_up: None,
            environment: vec![],
            travel: vec![],
            witnesses: vec![],
            rumors: vec![],
            journal: vec![],
            summaries: vec![],
            retrieval: vec![],
            retrieved: vec![],
            consolidation: vec![],
            npcs: vec![],
            hooks: vec![],
            arcs: vec![],
            remote: None,
            presence: vec![],
            audio: None,
            private_offers: vec![],
            knowledge_cues: vec![],
            moments: vec![],
            demands: vec![],
            asset_jobs: vec![],
            asset_dependencies: vec![],
            canonical_packs: vec![],
            shots: vec![],
            prefetch: None,
            scenes: vec![],
            item_origins: vec![],
            bookends: vec![],
            exports: vec![],
            critical_cues: vec![],
            content_candidates: vec![],
            content_admissions: vec![],
            recovery: RecoveryState {
                origin: None,
                retired_epochs: vec![],
                lost_ranges: vec![],
                suppression_generation: 0,
                redacted_records: vec![],
                unavailable_sources: vec![],
            },
        }
    }

    pub(crate) fn checkpoint_limits() -> CheckpointLimits {
        CheckpointLimits {
            maximum_records: 100,
            maximum_text_bytes: 256,
            maximum_total_text_bytes: 1024,
            maximum_retained_bytes: 1024 * 1024,
        }
    }
    pub(crate) fn checkpoint_for(
        mode: ExecutionMode,
        fatigue: Vec<(ContentReference, u64)>,
        presentation_ticks: u64,
    ) -> Checkpoint {
        let member = MemberId::from_bytes(&[3; 16]).unwrap();
        let operation = OperationId::from_bytes(&[4; 16]).unwrap();
        let state = GameState {
            mode,
            logical_time: LogicalTime {
                ticks: 9,
                ticks_per_second: 10,
            },
            members: vec![MembershipLink {
                member,
                character: None,
            }],
            entities: vec![],
            characters: vec![],
            resources: vec![],
            inventory: vec![],
            facts: vec![GameFact {
                id: fact_id(),
                revision: revision(8),
                operation,
                ordinal: 0,
                cause: None,
                audience: AudienceScope::Shared,
                value: FactValue::ContentEvent {
                    definition: content(),
                    subjects: vec![],
                },
            }],
            draws: vec![],
            decisions: vec![AcceptedDecision {
                operation,
                revision: revision(8),
                facts: vec![fact_id()],
                draws: vec![],
                effects: vec![],
                source_policy: label("fixture-policy-1"),
                semantic_output: None,
            }],
            pending: vec![],
            intents: vec![],
            timers: vec![],
            active_effects: vec![],
            knowledge: vec![],
            beliefs: vec![],
            memories: vec![],
            schedules: vec![],
            threats: vec![],
            relationships: vec![],
            conversations: vec![],
            obligations: vec![],
            narrative: NarrativeState {
                definition: content(),
                active_beats: vec![],
                completed_beats: vec![],
                open_threads: vec![],
                accepted_facts: vec![],
                remaining_budget: 0,
            },
            encounters: vec![],
            activity: vec![ActivityWindow {
                member,
                started: LogicalTime {
                    ticks: 1,
                    ticks_per_second: 10,
                },
                ends: LogicalTime {
                    ticks: 9,
                    ticks_per_second: 10,
                },
                observed_facts: vec![fact_id()],
                spotlight_opt_in: false,
            }],
            tempo: TempoState {
                policy: content(),
                presentation_ticks,
                intensity: 31,
                inertia: -2,
                fatigue,
            },
            presentation: vec![],
            continuity: continuity(),
        };
        checkpoint_from(state)
    }
    pub(crate) fn checkpoint_from(state: GameState) -> Checkpoint {
        let mut contents = vec![content()];
        for (definition, _) in &state.tempo.fatigue {
            if !contents.contains(definition) {
                contents.push(definition.clone());
            }
        }
        Checkpoint::new(
            CHECKPOINT_SCHEMA,
            basis(),
            pins(),
            state,
            ReferenceInventory {
                rules: &[],
                content: &contents,
                resources: &[],
                assets: &[],
            },
            checkpoint_limits(),
        )
        .unwrap()
    }
}

fn member() -> MemberId {
    MemberId::from_bytes(&[3; 16]).unwrap()
}
fn policy() -> PacingPolicy {
    PacingPolicy {
        definition: content(),
        rest_after: Duration::from_secs(1),
        lower_intensity_at: 50,
        lower_after_fatigue: 5,
    }
}
fn preferences(current: &Checkpoint) -> PacingPreferences {
    PacingPreferences {
        basis: current.basis(),
        member: member(),
        allow_suggestions: true,
        reduced_motion: false,
        quiet: false,
    }
}
fn request(current: &Checkpoint) -> PacingRequest<'_> {
    PacingRequest {
        expected_basis: current.basis(),
        member: member(),
        expected_policy: &current.state().tempo.policy,
        observed_logical_time: current.state().logical_time,
        observed_presentation_ticks: current.state().tempo.presentation_ticks,
        paused: false,
    }
}
fn limits() -> PacingLimits {
    PacingLimits {
        maximum_checkpoint_bytes: 1024 * 1024,
        maximum_activity_windows: 4,
        maximum_observed_facts: 4,
        maximum_fatigue_entries: 4,
    }
}
fn run(current: &Checkpoint, policy: &PacingPolicy, pref: PacingPreferences) -> PacingOutcome {
    recommend(
        Some(current),
        current.pins(),
        request(current),
        Some(policy),
        Some(pref),
        limits(),
    )
    .unwrap()
}
fn candidate(outcome: PacingOutcome) -> PacingRecommendation {
    match outcome {
        PacingOutcome::Recommendation(value) => value,
        other => panic!("{other:?}"),
    }
}
fn no_change(current: &Checkpoint, reason: NoInterventionReason) -> PacingOutcome {
    PacingOutcome::NoIntervention {
        basis: current.basis(),
        reason,
    }
}

#[test]
fn public_consumer_receives_current_basis_and_exact_optional_candidate_without_mutation() {
    let current = checkpoint_for(ExecutionMode::Live, vec![], 40);
    let original = current.clone();
    let result = candidate(run(&current, &policy(), preferences(&current)));
    assert_eq!(result.suggestion, PacingSuggestion::Continue);
    assert_eq!(result.basis, current.basis());
    assert_eq!(result.policy, content());
    assert_eq!(result.member, member());
    assert_eq!(result.window_started, current.state().activity[0].started);
    assert_eq!(result.window_ends, current.state().activity[0].ends);
    assert_eq!(result.observed_fact_count, 1);
    assert_eq!(result.presentation_ticks, 40);
    assert_eq!(current, original);
    // Macro consent cannot opt this member into spotlight or create a timer/action.
    assert!(!current.state().activity[0].spotlight_opt_in);
    assert!(current.state().timers.is_empty());
    assert!(current.state().intents.is_empty());
}

#[test]
fn caller_selected_rest_threshold_compares_exact_duration_without_rounding() {
    let current = checkpoint_for(ExecutionMode::Live, vec![], 40);
    let mut selected = policy();
    selected.rest_after = Duration::from_millis(800);
    assert_eq!(
        candidate(run(&current, &selected, preferences(&current))).suggestion,
        PacingSuggestion::Rest
    );
    selected.rest_after += Duration::from_nanos(1);
    assert_eq!(
        candidate(run(&current, &selected, preferences(&current))).suggestion,
        PacingSuggestion::Continue
    );
}

#[test]
fn explicit_quiet_and_reduced_motion_are_preserved_for_every_candidate() {
    let current = checkpoint_for(ExecutionMode::Live, vec![], 40);
    let mut pref = preferences(&current);
    pref.quiet = true;
    pref.reduced_motion = true;
    let lower = candidate(run(&current, &policy(), pref));
    assert_eq!(lower.suggestion, PacingSuggestion::LowerIntensity);
    assert!(lower.quiet && lower.reduced_motion);
    let mut selected = policy();
    selected.rest_after = Duration::from_millis(800);
    let rest = candidate(run(&current, &selected, pref));
    assert_eq!(rest.suggestion, PacingSuggestion::Rest);
    assert!(rest.quiet && rest.reduced_motion);
    pref.quiet = false;
    let ordinary = candidate(run(&current, &policy(), pref));
    assert_eq!(ordinary.suggestion, PacingSuggestion::Continue);
    assert!(ordinary.reduced_motion);
}

#[test]
fn caller_selected_fatigue_and_intensity_limits_never_reset_existing_history() {
    let current = checkpoint_for(ExecutionMode::Live, vec![(content(), 5)], 40);
    let original = current.clone();
    assert_eq!(
        candidate(run(&current, &policy(), preferences(&current))).suggestion,
        PacingSuggestion::LowerIntensity
    );
    let mut selected = policy();
    selected.lower_after_fatigue = 6;
    assert_eq!(
        candidate(run(&current, &selected, preferences(&current))).suggestion,
        PacingSuggestion::Continue
    );
    selected.lower_intensity_at = 31;
    assert_eq!(
        candidate(run(&current, &selected, preferences(&current))).suggestion,
        PacingSuggestion::LowerIntensity
    );
    assert_eq!(current, original);
}

#[test]
fn revoked_consent_and_inactivity_produce_no_compulsion_or_emotion_inference() {
    let current = checkpoint_for(ExecutionMode::Live, vec![], 40);
    let mut pref = preferences(&current);
    pref.allow_suggestions = false;
    assert_eq!(
        run(&current, &policy(), pref),
        no_change(&current, NoInterventionReason::Declined)
    );
    let mut state = current.state().clone();
    state.activity.clear();
    let missing = checkpoint_from(state);
    assert_eq!(
        run(&missing, &policy(), preferences(&missing)),
        no_change(&missing, NoInterventionReason::NoActivity)
    );
    let mut state = current.state().clone();
    state.activity[0].observed_facts.clear();
    let silent = checkpoint_from(state);
    assert_eq!(
        run(&silent, &policy(), preferences(&silent)),
        no_change(&silent, NoInterventionReason::NoActivity)
    );
}

#[test]
fn absent_optional_context_policy_and_preferences_are_distinct_refusals() {
    let current = checkpoint_for(ExecutionMode::Live, vec![], 40);
    let selected = policy();
    let pref = preferences(&current);
    assert_eq!(
        recommend(
            None,
            current.pins(),
            request(&current),
            Some(&selected),
            Some(pref),
            limits()
        ),
        Err(PacingError::ContextUnavailable)
    );
    assert_eq!(
        recommend(
            Some(&current),
            current.pins(),
            request(&current),
            None,
            Some(pref),
            limits()
        ),
        Err(PacingError::PolicyUnavailable)
    );
    assert_eq!(
        recommend(
            Some(&current),
            current.pins(),
            request(&current),
            Some(&selected),
            None,
            limits()
        ),
        Err(PacingError::PreferencesUnavailable)
    );
}

#[test]
fn stale_scope_pins_clocks_policy_and_preferences_cannot_suggest() {
    let current = checkpoint_for(ExecutionMode::Live, vec![], 40);
    let selected = policy();
    let pref = preferences(&current);
    let check = |input| {
        recommend(
            Some(&current),
            current.pins(),
            input,
            Some(&selected),
            Some(pref),
            limits(),
        )
    };
    let mut input = request(&current);
    input.expected_basis.revision = revision(7);
    assert_eq!(
        check(input),
        Err(PacingError::Binding(CheckpointError::StaleBasis))
    );
    input = request(&current);
    input.expected_basis.session = SessionId::from_bytes(&[8; 16]).unwrap();
    assert_eq!(
        check(input),
        Err(PacingError::Binding(CheckpointError::WrongSession))
    );
    input = request(&current);
    input.expected_basis.run = RunId::from_bytes(&[8; 16]).unwrap();
    assert_eq!(
        check(input),
        Err(PacingError::Binding(CheckpointError::WrongRun))
    );
    input = request(&current);
    input.observed_logical_time.ticks += 1;
    assert_eq!(check(input), Err(PacingError::LogicalTimeMismatch));
    input = request(&current);
    input.observed_presentation_ticks += 1;
    assert_eq!(check(input), Err(PacingError::StalePresentationAnchor));
    let mut stale_pins = current.pins().clone();
    stale_pins.content.package_digest.0[0] ^= 1;
    assert_eq!(
        recommend(
            Some(&current),
            &stale_pins,
            request(&current),
            Some(&selected),
            Some(pref),
            limits()
        ),
        Err(PacingError::Binding(CheckpointError::ContentMismatch))
    );
    let mut stale = selected.clone();
    stale.definition.entry = label("other-policy");
    assert_eq!(
        recommend(
            Some(&current),
            current.pins(),
            request(&current),
            Some(&stale),
            Some(pref),
            limits()
        ),
        Err(PacingError::PolicyMismatch)
    );
    stale = selected.clone();
    stale.definition.package = label("other-package");
    assert_eq!(
        recommend(
            Some(&current),
            current.pins(),
            request(&current),
            Some(&stale),
            Some(pref),
            limits()
        ),
        Err(PacingError::PolicyMismatch)
    );
    let mut old_pref = pref;
    old_pref.basis.revision = revision(7);
    assert_eq!(
        recommend(
            Some(&current),
            current.pins(),
            request(&current),
            Some(&selected),
            Some(old_pref),
            limits()
        ),
        Err(PacingError::StalePreferences)
    );
    old_pref = pref;
    old_pref.member = MemberId::from_bytes(&[8; 16]).unwrap();
    assert_eq!(
        recommend(
            Some(&current),
            current.pins(),
            request(&current),
            Some(&selected),
            Some(old_pref),
            limits()
        ),
        Err(PacingError::WrongMember)
    );
}

#[test]
fn exact_input_capacities_are_admitted_and_excess_returns_no_partial_candidate() {
    let current = checkpoint_for(ExecutionMode::Live, vec![(content(), 5)], 40);
    let original = current.clone();
    let selected = policy();
    let pref = preferences(&current);
    let check = |bounds| {
        recommend(
            Some(&current),
            current.pins(),
            request(&current),
            Some(&selected),
            Some(pref),
            bounds,
        )
    };
    let exact = PacingLimits {
        maximum_checkpoint_bytes: current.retained_bytes().unwrap(),
        maximum_activity_windows: 1,
        maximum_observed_facts: 1,
        maximum_fatigue_entries: 1,
    };
    assert!(check(exact).is_ok());
    let mut bounds = exact;
    bounds.maximum_checkpoint_bytes = 0;
    assert_eq!(check(bounds), Err(PacingError::InvalidLimits));
    bounds = exact;
    bounds.maximum_checkpoint_bytes -= 1;
    assert_eq!(check(bounds), Err(PacingError::Capacity));
    bounds = exact;
    bounds.maximum_activity_windows = 0;
    assert_eq!(check(bounds), Err(PacingError::Capacity));
    bounds = exact;
    bounds.maximum_observed_facts = 0;
    assert_eq!(check(bounds), Err(PacingError::Capacity));
    bounds = exact;
    bounds.maximum_fatigue_entries = 0;
    assert_eq!(check(bounds), Err(PacingError::Capacity));
    assert_eq!(current, original);
}

#[test]
fn historical_window_cannot_masquerade_as_current_activity() {
    let current = checkpoint_for(ExecutionMode::Live, vec![], 40);
    let mut state = current.state().clone();
    state.logical_time.ticks += 1;
    let later = checkpoint_from(state);
    assert_eq!(
        recommend(
            Some(&later),
            later.pins(),
            request(&later),
            Some(&policy()),
            Some(preferences(&later)),
            limits()
        ),
        Err(PacingError::StaleActivityWindow)
    );
}

#[test]
fn invalid_policy_is_refused_and_extreme_duration_has_no_default_threshold() {
    let current = checkpoint_for(ExecutionMode::Live, vec![], 40);
    let check = |selected: &PacingPolicy| {
        recommend(
            Some(&current),
            current.pins(),
            request(&current),
            Some(selected),
            Some(preferences(&current)),
            limits(),
        )
    };
    let mut selected = policy();
    selected.rest_after = Duration::ZERO;
    assert_eq!(check(&selected), Err(PacingError::InvalidPolicy));
    selected = policy();
    selected.lower_after_fatigue = 0;
    assert_eq!(check(&selected), Err(PacingError::InvalidPolicy));
    selected = policy();
    selected.rest_after = Duration::MAX;
    // The exact comparison remains representable at this fixture's ten ticks/second.
    assert!(check(&selected).is_ok());
}

#[test]
fn replay_pause_and_prepared_only_preserve_recorded_state_and_timers() {
    for mode in [
        ExecutionMode::Replay,
        ExecutionMode::PreparedOnly,
        ExecutionMode::Live,
    ] {
        let current = checkpoint_for(mode, vec![(content(), 5)], 40);
        let original = current.clone();
        let result = run(&current, &policy(), preferences(&current));
        if mode == ExecutionMode::Replay {
            assert_eq!(result, no_change(&current, NoInterventionReason::Replay));
        } else {
            assert_eq!(
                candidate(result).suggestion,
                PacingSuggestion::LowerIntensity
            );
        }
        let mut input = request(&current);
        input.paused = true;
        let paused = recommend(
            Some(&current),
            current.pins(),
            input,
            Some(&policy()),
            Some(preferences(&current)),
            limits(),
        )
        .unwrap();
        assert_eq!(
            paused,
            no_change(
                &current,
                if mode == ExecutionMode::Replay {
                    NoInterventionReason::Replay
                } else {
                    NoInterventionReason::Paused
                }
            )
        );
        assert_eq!(current, original);
    }
}

#[test]
fn identical_inputs_reproduce_candidates_but_new_recovery_basis_rejects_old_preferences() {
    let current = checkpoint_for(ExecutionMode::Live, vec![], 40);
    let pref = preferences(&current);
    assert_eq!(
        run(&current, &policy(), pref),
        run(&current, &policy(), pref)
    );
    let mut old = pref;
    old.basis.revision =
        df_types::SessionRevision::new(df_types::RecoveryEpoch::new(1).unwrap(), 8);
    assert_eq!(
        recommend(
            Some(&current),
            current.pins(),
            request(&current),
            Some(&policy()),
            Some(old),
            limits()
        ),
        Err(PacingError::StalePreferences)
    );
}

#[test]
fn multiple_retained_windows_do_not_silently_select_a_member_history() {
    let current = checkpoint_for(ExecutionMode::Live, vec![], 40);
    let mut state = current.state().clone();
    state.activity.push(state.activity[0].clone());
    let ambiguous = checkpoint_from(state);
    let original = ambiguous.clone();
    assert_eq!(
        recommend(
            Some(&ambiguous),
            ambiguous.pins(),
            request(&ambiguous),
            Some(&policy()),
            Some(preferences(&ambiguous)),
            limits()
        ),
        Err(PacingError::AmbiguousWindow)
    );
    assert_eq!(ambiguous, original);
}
