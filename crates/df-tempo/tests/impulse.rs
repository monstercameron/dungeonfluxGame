//! Executable B-C-df-tempo-D02: sudden events cannot bypass intensity caps.
//! Coefficients are source-bound fixture policy, not production calibration or device safety.
mod tempo_fixture;

use std::time::Duration;

use df_engine::director_staging::*;
use df_model::checkpoint::*;
use df_tempo::elapsed::*;
use df_tempo::impulse::*;
use df_types::MemberId;
use df_world::{DueSelectionLimits, DueSelectionRequest};
use tempo_fixture::*;

fn impulse_policy(current: &Checkpoint) -> ImpulsePolicy {
    ImpulsePolicy {
        definition: current.state().tempo.policy.clone(),
        minimum_intensity: 0,
        maximum_intensity: 50,
        maximum_absolute_inertia: 20,
        recovery_units_per_tick: 1,
        rise_units_per_tick: 2,
        fall_units_per_tick: 3,
        deadband_units: 2,
        named_impulses: vec![NamedImpulseRule {
            event: content(),
            inertia_units: 40,
            rise_units_per_tick: 10,
            fall_units_per_tick: 10,
        }],
    }
}

fn input(current: &Checkpoint, target: i64, shock: bool) -> PermittedTempoInput {
    PermittedTempoInput {
        expected_basis: current.basis(),
        target_intensity: target,
        impulse: shock.then_some(ImpulseEvent {
            fact: fact_id(),
            at_presentation_ticks: current.state().tempo.presentation_ticks,
        }),
    }
}

fn impulse_limits() -> ImpulseLimits {
    ImpulseLimits {
        elapsed: limits(),
        maximum_named_impulses: 4,
    }
}

fn advance(current: &Checkpoint, target: i64, shock: bool, ticks: u64) -> ImpulseAdvance {
    advance_impulse(
        current,
        current.pins(),
        request(current, Duration::from_millis(ticks * 100), false),
        Some(&policy(current)),
        Some(&impulse_policy(current)),
        input(current, target, shock),
        impulse_limits(),
    )
    .unwrap()
}

fn staged(current: &Checkpoint, advance: ImpulseAdvance) -> Checkpoint {
    let mut state = current.state().clone();
    state.tempo = advance.state;
    checkpoint_from(state)
}

#[test]
fn public_named_shock_extremes_and_repeats_stay_within_intensity_and_inertia_caps() {
    let mut current = checkpoint_for(ExecutionMode::Live, vec![(content(), 7)], 40);
    let original = current.clone();
    let first = advance(&current, 31, true, 10);
    assert_eq!(first.state.intensity, 50);
    assert_eq!(first.state.inertia, 10);
    assert_eq!(first.state.fatigue, current.state().tempo.fatigue);
    assert_eq!(first, advance(&current, 31, true, 10));
    assert_eq!(current, original);
    for _ in 0..8 {
        let result = advance(&current, 31, true, 10);
        let curve = result.curve.unwrap();
        assert_eq!(curve.ticks_per_second(), 10);
        for tick in curve.from_ticks()..=curve.through_ticks() {
            assert!((0..=50).contains(&curve.intensity_at(tick).unwrap()));
        }
        assert_eq!(curve.intensity_at(curve.through_ticks() + 1), None);
        assert_eq!(curve.intensity_at(curve.from_ticks() - 1), None);
        assert!((0..=50).contains(&result.state.intensity));
        assert!((-20..=20).contains(&result.state.inertia));
        current = staged(&current, result);
    }
    let mut negative = impulse_policy(&current);
    negative.named_impulses[0].inertia_units = i64::MIN;
    let result = advance_impulse(
        &current,
        current.pins(),
        request(&current, Duration::from_secs(1), false),
        Some(&policy(&current)),
        Some(&negative),
        input(&current, 0, true),
        impulse_limits(),
    )
    .unwrap();
    assert_eq!(result.state.intensity, 0);
    assert_eq!(result.state.inertia, -10);

    // Full signed intensity range and maximum interval/rates still use constant-space arithmetic.
    let mut extreme_state = original.state().clone();
    extreme_state.tempo.intensity = i64::MIN;
    extreme_state.tempo.inertia = 0;
    extreme_state.tempo.presentation_ticks = 0;
    let extreme = checkpoint_from(extreme_state);
    let mut extreme_policy = impulse_policy(&extreme);
    extreme_policy.minimum_intensity = i64::MIN;
    extreme_policy.maximum_intensity = i64::MAX;
    extreme_policy.maximum_absolute_inertia = i64::MAX;
    extreme_policy.rise_units_per_tick = u64::MAX;
    extreme_policy.fall_units_per_tick = u64::MAX;
    extreme_policy.recovery_units_per_tick = u64::MAX;
    extreme_policy.named_impulses[0].inertia_units = i64::MAX;
    extreme_policy.named_impulses[0].rise_units_per_tick = u64::MAX;
    let mut time_policy = policy(&extreme);
    time_policy.ticks_per_second = 1;
    time_policy.maximum_elapsed_ticks = u64::MAX;
    let result = advance_impulse(
        &extreme,
        extreme.pins(),
        request(&extreme, Duration::from_secs(u64::MAX), false),
        Some(&time_policy),
        Some(&extreme_policy),
        input(&extreme, i64::MAX, true),
        impulse_limits(),
    )
    .unwrap();
    assert_eq!(result.state.intensity, i64::MAX);
    assert_eq!(result.state.inertia, 0);
    let curve = result.curve.unwrap();
    assert_eq!(curve.intensity_at(0), Some(i64::MIN));
    assert_eq!(curve.intensity_at(u64::MAX / 2), Some(-1));
    assert_eq!(curve.intensity_at(u64::MAX), Some(i64::MAX));
    let mut falling_state = extreme.state().clone();
    falling_state.tempo.intensity = i64::MAX;
    let falling = checkpoint_from(falling_state);
    let result = advance_impulse(
        &falling,
        falling.pins(),
        request(&falling, Duration::from_secs(u64::MAX), false),
        Some(&time_policy),
        Some(&extreme_policy),
        input(&falling, i64::MIN, false),
        impulse_limits(),
    )
    .unwrap();
    assert_eq!(result.state.intensity, i64::MIN);
    assert_eq!(result.curve.unwrap().intensity_at(u64::MAX / 2), Some(0));
}

#[test]
fn controlled_elapsed_slew_decay_and_deadband_are_exact_and_bounded() {
    let current = checkpoint_for(ExecutionMode::Live, vec![], 40);
    let deadband = advance(&current, 31, false, 10);
    assert_eq!(deadband.state.intensity, 31);
    assert_eq!(deadband.state.inertia, 0);
    let ordinary = advance(&current, 50, false, 1);
    assert_eq!(ordinary.state.intensity, 33);
    assert_eq!(ordinary.state.inertia, -1);
    let shock = advance(&current, 31, true, 1);
    assert_eq!(shock.state.intensity, 41);
    assert_eq!(shock.state.inertia, 19);
    let shocked = staged(&current, shock);
    let release = advance(&shocked, 0, false, 10);
    assert_eq!(release.state.intensity, 19);
    assert_eq!(release.state.inertia, 9);
    let released = staged(&shocked, release);
    let recovery = advance(&released, 0, false, 10);
    assert_eq!(recovery.state.intensity, 9);
    assert_eq!(recovery.state.inertia, 0);
    let recovered = staged(&released, recovery);
    assert_eq!(advance(&recovered, 0, false, 10).state.intensity, 0);
    let unchanged = advance(&current, 50, false, 0);
    assert_eq!(unchanged.state, current.state().tempo);
    assert_eq!(unchanged.curve, None);
    assert_eq!(unchanged.disposition, ElapsedDisposition::Unchanged);

    // The new public consumer follows the actual existing engine director staging boundary.
    let directors = compose_director_candidates(
        &current,
        current.pins(),
        DueSelectionRequest {
            expected_basis: current.basis(),
            target_time: current.state().logical_time,
            paused: false,
            deadline_remaining: Duration::from_secs(1),
            policy: &content(),
        },
        DirectorCandidates {
            interaction: None,
            narrative: None,
        },
        DirectorLimits {
            maximum_checkpoint_bytes: 1024 * 1024,
            maximum_pass_bytes: 5 * 1024 * 1024,
            maximum_relationships: 10,
            world: DueSelectionLimits {
                queue_events: 10,
                selected_events: 10,
                output_bytes: 1024 * 1024,
            },
        },
    )
    .unwrap();
    let DirectorStaging::Staged(directors) = directors else {
        panic!("empty world reaches tempo")
    };
    let selected = directors.candidate();
    let proposal = advance(selected, 31, true, 10);
    let mut candidate = selected.state().clone();
    candidate.tempo = proposal.state;
    assert_ne!(candidate.tempo, selected.state().tempo);
    candidate.tempo = selected.state().tempo.clone();
    assert_eq!(candidate, *selected.state());
}

#[test]
fn stale_basis_anchor_policy_source_or_hidden_shock_refuses() {
    let current = checkpoint_for(ExecutionMode::Live, vec![], 40);
    let original = current.clone();
    let elapsed = policy(&current);
    let admitted = impulse_policy(&current);
    let call = |request, policy, input| {
        advance_impulse(
            &current,
            current.pins(),
            request,
            Some(&elapsed),
            Some(policy),
            input,
            impulse_limits(),
        )
    };
    let normal_request = request(&current, Duration::from_secs(1), false);
    let mut stale_request = normal_request;
    stale_request.expected_basis.revision = revision(7);
    assert!(matches!(
        call(stale_request, &admitted, input(&current, 31, true)),
        Err(ImpulseError::Elapsed(TempoAdvanceError::Binding(_)))
    ));
    stale_request = normal_request;
    stale_request.from_presentation_ticks -= 1;
    assert_eq!(
        call(stale_request, &admitted, input(&current, 31, true)),
        Err(ImpulseError::Elapsed(
            TempoAdvanceError::StalePresentationAnchor
        ))
    );
    let mut stale_input = input(&current, 31, true);
    stale_input.expected_basis.revision = revision(7);
    assert_eq!(
        call(normal_request, &admitted, stale_input),
        Err(ImpulseError::StaleInput)
    );
    stale_input = input(&current, 31, true);
    stale_input.impulse.as_mut().unwrap().at_presentation_ticks -= 1;
    assert_eq!(
        call(normal_request, &admitted, stale_input),
        Err(ImpulseError::StaleImpulseAnchor)
    );
    let mut wrong_policy = admitted.clone();
    wrong_policy.definition.entry = label("wrong-policy");
    assert_eq!(
        call(normal_request, &wrong_policy, input(&current, 31, true)),
        Err(ImpulseError::PolicyMismatch)
    );
    let mut wrong_source = admitted.clone();
    wrong_source.named_impulses[0].event.entry = label("unapproved-event");
    assert_eq!(
        call(normal_request, &wrong_source, input(&current, 31, true)),
        Err(ImpulseError::UnapprovedImpulse)
    );
    let mut unknown_input = input(&current, 31, true);
    unknown_input.impulse.as_mut().unwrap().fact = FactId::from_bytes(&[99; 16]).unwrap();
    assert_eq!(
        call(normal_request, &admitted, unknown_input),
        Err(ImpulseError::UnknownFact)
    );
    assert_eq!(current, original);

    let next = staged(&current, advance(&current, 31, true, 10));
    assert_eq!(
        advance_impulse(
            &next,
            next.pins(),
            request(&next, Duration::from_secs(1), false),
            Some(&policy(&next)),
            Some(&admitted),
            input(&current, 31, true),
            impulse_limits(),
        ),
        Err(ImpulseError::StaleImpulseAnchor)
    );
    let mut hidden_state = current.state().clone();
    hidden_state.facts[0].audience = AudienceScope::Host;
    let hidden = checkpoint_from(hidden_state);
    assert_eq!(
        advance_impulse(
            &hidden,
            hidden.pins(),
            request(&hidden, Duration::from_secs(1), false),
            Some(&elapsed),
            Some(&admitted),
            input(&hidden, 31, true),
            impulse_limits(),
        ),
        Err(ImpulseError::HiddenImpulse)
    );
    assert_eq!(
        advance(&hidden, 31, false, 10),
        advance(&current, 31, false, 10)
    );
    let mut old_state = current.state().clone();
    old_state.facts[0].revision = revision(7);
    old_state.decisions[0].revision = revision(7);
    let old = checkpoint_from(old_state);
    assert_eq!(
        advance_impulse(
            &old,
            old.pins(),
            request(&old, Duration::from_secs(1), false),
            Some(&elapsed),
            Some(&admitted),
            input(&old, 31, true),
            impulse_limits(),
        ),
        Err(ImpulseError::StaleFact)
    );
}

fn mechanical_checkpoint(mode: ExecutionMode) -> Checkpoint {
    let mut state = checkpoint_for(mode, vec![(content(), 7)], 40)
        .state()
        .clone();
    let entity = EntityId::from_bytes(&[16; 16]).unwrap();
    let member = MemberId::from_bytes(&[3; 16]).unwrap();
    let rule = RuleReference {
        catalog: pins().rules.catalog,
        source: label("fixture-source-1"),
        entry: label("fixture-rule-1"),
        clause: label("fixture-clause-1"),
    };
    state.entities.push(WorldEntity {
        id: entity,
        definition: content(),
        location: None,
        position: None,
        identity_revision: label("fixture-entity-1"),
    });
    state.resources.push(ResourceState {
        owner: entity,
        resource: label("fixture-resource-1"),
        value: 4,
        minimum: 0,
        maximum: 8,
        source: rule.clone(),
    });
    state.timers.push(OwnedTimer {
        id: TimerId::from_bytes(&[17; 16]).unwrap(),
        basis: basis(),
        generation: 1,
        due: LogicalTime {
            ticks: 10,
            ticks_per_second: 10,
        },
        source: rule.clone(),
        status: DurableStatus::Pending,
    });
    state.pending.push(PendingResolution {
        id: ResolutionId::from_bytes(&[18; 16]).unwrap(),
        basis: basis(),
        continuation: label("fixture-continuation-1"),
        window: ResolutionWindow {
            id: WindowId::from_bytes(&[19; 16]).unwrap(),
            phase: TriggerPhase::BeforeDraw,
            causal_fact: fact_id(),
            source: rule.clone(),
            timer: Some(state.timers[0].id),
        },
        next: PendingInput::Reaction {
            remaining: vec![OfferedResponse {
                participant: member,
                offer: label("fixture-offer-1"),
                options: vec![label("fixture-option-1")],
                source: rule.clone(),
            }],
        },
        choices: vec![],
        draw_ordinals: vec![],
        spent: vec![ResourceSpend {
            owner: entity,
            resource: label("fixture-resource-1"),
            amount: 1,
            source: rule.clone(),
        }],
        rulings: vec![],
    });
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        basis(),
        pins(),
        state,
        ReferenceInventory {
            rules: std::slice::from_ref(&rule),
            content: &[content()],
            resources: &[ResourceConstraint {
                owner: entity,
                resource: label("fixture-resource-1"),
                minimum: 0,
                maximum: 8,
                source: rule.clone(),
            }],
            assets: &[],
        },
        checkpoint_limits(),
    )
    .unwrap()
}

#[test]
fn pause_replay_and_prepared_modes_preserve_mechanical_state() {
    for (mode, paused, disposition) in [
        (ExecutionMode::Live, true, ElapsedDisposition::Paused),
        (ExecutionMode::Replay, false, ElapsedDisposition::Replay),
        (
            ExecutionMode::PreparedOnly,
            false,
            ElapsedDisposition::Advanced,
        ),
        (ExecutionMode::Live, false, ElapsedDisposition::Advanced),
    ] {
        let current = mechanical_checkpoint(mode);
        let original = current.clone();
        let result = advance_impulse(
            &current,
            current.pins(),
            request(
                &current,
                if paused || mode == ExecutionMode::Replay {
                    Duration::MAX
                } else {
                    Duration::from_secs(1)
                },
                paused,
            ),
            Some(&policy(&current)),
            Some(&impulse_policy(&current)),
            input(&current, 31, true),
            impulse_limits(),
        )
        .unwrap();
        assert_eq!(result.disposition, disposition);
        if paused || mode == ExecutionMode::Replay {
            assert_eq!(result.state, current.state().tempo);
            assert_eq!(result.curve, None);
        } else {
            assert_eq!(result.state.intensity, 50);
            assert!(result.curve.is_some());
        }
        let mut candidate = current.state().clone();
        candidate.tempo = result.state;
        assert_eq!(candidate.resources, current.state().resources);
        assert_eq!(candidate.pending, current.state().pending);
        assert_eq!(candidate.timers, current.state().timers);
        candidate.tempo = current.state().tempo.clone();
        assert_eq!(candidate, *current.state());
        assert_eq!(current, original);
    }
}

#[test]
fn overflow_capacity_invalid_policy_and_missing_admission_refuse_without_mutation() {
    let current = checkpoint_for(ExecutionMode::Live, vec![(content(), 7)], 40);
    let original = current.clone();
    let elapsed = policy(&current);
    let admitted = impulse_policy(&current);
    let normal = request(&current, Duration::from_secs(1), false);
    let call = |request: TempoAdvanceRequest,
                elapsed_policy: Option<&TempoElapsedPolicy>,
                impulse_policy: Option<&ImpulsePolicy>,
                limits: ImpulseLimits| {
        advance_impulse(
            &current,
            current.pins(),
            request,
            elapsed_policy,
            impulse_policy,
            input(&current, 31, true),
            limits,
        )
    };
    assert_eq!(
        call(normal, Some(&elapsed), None, impulse_limits()),
        Err(ImpulseError::PolicyUnavailable)
    );
    assert_eq!(
        call(normal, None, Some(&admitted), impulse_limits()),
        Err(ImpulseError::Elapsed(TempoAdvanceError::PolicyUnavailable))
    );
    assert_eq!(
        advance_impulse(
            &current,
            current.pins(),
            normal,
            Some(&elapsed),
            Some(&admitted),
            input(&current, 51, true),
            impulse_limits()
        ),
        Err(ImpulseError::InvalidTarget)
    );
    let mut invalid = admitted.clone();
    invalid.maximum_absolute_inertia = -1;
    assert_eq!(
        call(normal, Some(&elapsed), Some(&invalid), impulse_limits()),
        Err(ImpulseError::InvalidPolicy)
    );
    invalid = admitted.clone();
    invalid.minimum_intensity = 51;
    assert_eq!(
        call(normal, Some(&elapsed), Some(&invalid), impulse_limits()),
        Err(ImpulseError::InvalidPolicy)
    );
    invalid = admitted.clone();
    invalid.maximum_intensity = 30;
    assert_eq!(
        call(normal, Some(&elapsed), Some(&invalid), impulse_limits()),
        Err(ImpulseError::InvalidState)
    );
    invalid = admitted.clone();
    invalid
        .named_impulses
        .push(invalid.named_impulses[0].clone());
    assert_eq!(
        call(normal, Some(&elapsed), Some(&invalid), impulse_limits()),
        Err(ImpulseError::DuplicateRule)
    );
    let mut bound = impulse_limits();
    bound.maximum_named_impulses = 0;
    assert_eq!(
        call(normal, Some(&elapsed), Some(&admitted), bound),
        Err(ImpulseError::Capacity)
    );
    bound = impulse_limits();
    bound.elapsed.maximum_checkpoint_bytes = 1;
    assert_eq!(
        call(normal, Some(&elapsed), Some(&admitted), bound),
        Err(ImpulseError::Elapsed(TempoAdvanceError::Capacity))
    );
    assert_eq!(
        call(
            request(&current, Duration::ZERO, false),
            Some(&elapsed),
            Some(&admitted),
            impulse_limits()
        ),
        Err(ImpulseError::ImpulseRequiresElapsed)
    );
    assert_eq!(
        call(
            request(&current, Duration::from_nanos(1), false),
            Some(&elapsed),
            Some(&admitted),
            impulse_limits()
        ),
        Err(ImpulseError::Elapsed(TempoAdvanceError::FractionalTicks))
    );
    assert_eq!(
        call(
            request(&current, Duration::from_secs(11), false),
            Some(&elapsed),
            Some(&admitted),
            impulse_limits()
        ),
        Err(ImpulseError::Elapsed(TempoAdvanceError::ElapsedLimit))
    );
    let mut mismatched_pins = current.pins().clone();
    mismatched_pins.content.package = label("wrong-package");
    assert!(matches!(
        advance_impulse(
            &current,
            &mismatched_pins,
            normal,
            Some(&elapsed),
            Some(&admitted),
            input(&current, 31, true),
            impulse_limits()
        ),
        Err(ImpulseError::Elapsed(TempoAdvanceError::Binding(_)))
    ));
    let mut overflow_state = current.state().clone();
    overflow_state.tempo.presentation_ticks = u64::MAX;
    let overflow = checkpoint_from(overflow_state);
    assert_eq!(
        advance_impulse(
            &overflow,
            overflow.pins(),
            request(&overflow, Duration::from_secs(1), false),
            Some(&elapsed),
            Some(&admitted),
            input(&overflow, 31, true),
            impulse_limits()
        ),
        Err(ImpulseError::Elapsed(TempoAdvanceError::TimeOverflow))
    );
    assert_eq!(current, original);
}
