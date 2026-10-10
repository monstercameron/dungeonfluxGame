#[path = "../../df-session/tests/support/fixture_model.rs"]
mod fixture_model;
mod support;

use df_model::checkpoint::{
    AcceptedDecision, AssetDemand, AssetKind, AssetReference, AssetRequestKey, AudienceScope,
    Basis, Checkpoint, CheckpointPins, ContentDigest, DemandPriority, ExecutionMode, FactValue,
    GameFact, LogicalTime, NarrativeMoment, PerformanceHint, PresentationDemand,
    ReferenceInventory, ShotPlan,
};
use df_presentation::moment_selection::{
    MomentAlternative, MomentContext, MomentDisposition, MomentError, MomentLimits,
    PermittedMoment, select_committed_moment_plan, select_moment_plan,
};
use support::{Fixture, entity, fact, label, limits, now, record};

#[test]
fn selected_records_are_borrowed_exactly_without_changing_inputs() {
    let fixture = Fixture::new();
    let original_shots = fixture.shots.clone();
    let original_demands = fixture.demands.clone();
    let alternatives = [fixture.alternative()];
    let selected = select_moment_plan(
        fixture.context(),
        now(),
        Some(fixture.permitted()),
        &alternatives,
        limits(),
    )
    .unwrap();
    assert_eq!(selected.disposition, MomentDisposition::Selected);
    assert!(std::ptr::eq(selected.selected.unwrap(), &alternatives[0]));
    assert_eq!(fixture.shots, original_shots);
    assert_eq!(fixture.demands, original_demands);
}

#[test]
fn selected_visual_and_performance_descriptors_keep_the_approved_source_references() {
    let mut fixture = Fixture::new();
    let shot_definition = fixture.content[0].clone();
    let performance_definition = fixture.content[0].clone();
    let voice = fixture.assets[0].clone();
    fixture.shots[0].definition = shot_definition.clone();
    fixture.shots[0].performance.definition = performance_definition.clone();
    fixture.shots[0].performance.voice = Some(voice.clone());
    let original_shot = fixture.shots[0].clone();
    let original_moment = fixture.moment.clone();
    let alternatives = [fixture.alternative()];

    let result = select_moment_plan(
        fixture.context(),
        now(),
        Some(fixture.permitted()),
        &alternatives,
        limits(),
    )
    .unwrap();

    let selected = result.selected.unwrap();
    assert_eq!(selected.context.moment, &original_moment);
    assert!(std::ptr::eq(&selected.shots[0], &fixture.shots[0]));
    assert_eq!(selected.shots[0], original_shot);
    assert_eq!(selected.shots[0].definition, shot_definition);
    assert_eq!(selected.shots[0].references, fixture.assets[..1]);
    assert_eq!(
        selected.shots[0].performance.definition,
        performance_definition
    );
    assert_eq!(selected.shots[0].performance.voice.as_ref(), Some(&voice));
    assert_eq!(
        selected.shots[0].performance.emphasis_facts,
        original_moment.facts
    );
}

#[test]
fn committed_projection_accepts_checkpoint_records_and_refuses_uncommitted_shots() {
    let basis: Basis = fixture_model::basis();
    let pins: CheckpointPins = fixture_model::pins();
    let content = fixture_model::content();
    let moment_id = record(70);
    let fact_id = fact(71);
    let still = AssetReference {
        key: label("committed-still"),
        digest: ContentDigest([72; 32]),
        byte_length: 4,
        kind: AssetKind::Image,
    };
    let voice = AssetReference {
        key: label("committed-voice"),
        digest: ContentDigest([73; 32]),
        byte_length: 5,
        kind: AssetKind::Audio,
    };
    let assets = [still.clone(), voice.clone()];
    let mut state = fixture_model::state();
    state.mode = ExecutionMode::PreparedOnly;
    let operation = fixture_model::operation();
    state.facts.push(GameFact {
        id: fact_id,
        revision: basis.revision,
        operation,
        ordinal: 0,
        cause: None,
        audience: AudienceScope::Shared,
        value: FactValue::ContentEvent {
            definition: content.clone(),
            subjects: vec![entity(4)],
        },
    });
    state.decisions.push(AcceptedDecision {
        operation,
        revision: basis.revision,
        facts: vec![fact_id],
        draws: vec![],
        effects: vec![],
        source_policy: label("committed-source-policy"),
        semantic_output: None,
    });
    let moment = NarrativeMoment {
        id: moment_id,
        location: entity(4),
        characters: vec![entity(4)],
        facts: vec![fact_id],
        attributed_claims: vec![],
        audience: AudienceScope::Shared,
        semantic_focus: content.clone(),
        identity_revision: label("committed-identity"),
    };
    let presentation = PresentationDemand {
        id: record(74),
        definition: content.clone(),
        audience: AudienceScope::Shared,
        causal_facts: vec![fact_id],
        source_revision: basis.revision,
    };
    let shot = ShotPlan {
        id: record(75),
        moment: moment_id,
        subjects: moment.characters.clone(),
        audience: moment.audience.clone(),
        duration_ticks: 5,
        definition: content.clone(),
        references: vec![still.clone()],
        performance: PerformanceHint {
            definition: content.clone(),
            voice: Some(voice.clone()),
            emphasis_facts: vec![fact_id],
        },
    };
    let demand = AssetDemand {
        id: record(76),
        basis,
        key: AssetRequestKey {
            schema: df_model::checkpoint::CHECKPOINT_SCHEMA,
            source: pins.content.content_digest,
            moment: moment_id,
            identity: moment.identity_revision.clone(),
            style: label("committed-style"),
            voice: Some(voice.key.clone()),
            provider: label("prepared-only"),
            model: label("fixture-model"),
            format: label("still-format"),
            references: vec![still],
            audience: AudienceScope::Shared,
            parameters: label("committed-parameters"),
        },
        priority: DemandPriority::InteractionCritical,
        mode: ExecutionMode::PreparedOnly,
        expires: LogicalTime {
            ticks: 100,
            ticks_per_second: 10,
        },
        budget_reservation: label("no-live-spend"),
        maximum_bytes: 16,
        policy: content.clone(),
    };
    state.continuity.moments.push(moment);
    state.presentation.push(presentation);
    state.continuity.shots.push(shot);
    state.continuity.demands.push(demand);
    let rules = [fixture_model::rule()];
    let content_inventory = [content.clone()];
    let resources = fixture_model::resource_constraints();
    let checkpoint = Checkpoint::new(
        df_model::checkpoint::CHECKPOINT_SCHEMA,
        basis,
        pins,
        state,
        ReferenceInventory {
            rules: &rules,
            content: &content_inventory,
            resources: &resources,
            assets: &assets,
        },
        fixture_model::limits(),
    )
    .unwrap();
    let current = MomentContext {
        basis: checkpoint.basis(),
        pins: checkpoint.pins(),
        mode: checkpoint.state().mode,
        moment: &checkpoint.state().continuity.moments[0],
        presentation: &checkpoint.state().presentation[0],
    };
    let facts = [fact_id];
    let claims = [];
    let entities = [entity(4)];
    let permitted = PermittedMoment {
        context: current,
        facts: &facts,
        attributed_claims: &claims,
        entities: &entities,
        content: &content_inventory,
        assets: &assets,
    };
    let alternative = MomentAlternative {
        context: current,
        shots: &checkpoint.state().continuity.shots,
        demands: &checkpoint.state().continuity.demands,
    };
    let alternatives = [alternative];
    let selected = select_committed_moment_plan(
        &checkpoint,
        current,
        LogicalTime {
            ticks: 90,
            ticks_per_second: 10,
        },
        Some(permitted),
        &alternatives,
        MomentLimits {
            maximum_alternatives: 8,
            maximum_items: 1024,
            maximum_shots: 8,
            maximum_demands: 8,
            maximum_duration_ticks: 100,
            maximum_reference_bytes: 1024,
            maximum_demand_bytes: 1024,
        },
    )
    .unwrap();
    assert_eq!(selected.disposition, MomentDisposition::Selected);
    assert!(std::ptr::eq(selected.selected.unwrap(), &alternatives[0]));
    assert_eq!(selected.selected.unwrap().context.moment.facts, facts);
    assert_eq!(
        selected.selected.unwrap().shots[0].performance.voice,
        Some(voice)
    );

    let mut uncommitted_shot = checkpoint.state().continuity.shots[0].clone();
    uncommitted_shot.id = record(77);
    let uncommitted_shots = [uncommitted_shot];
    let uncommitted = [MomentAlternative {
        context: current,
        shots: &uncommitted_shots,
        demands: &checkpoint.state().continuity.demands,
    }];
    assert!(matches!(
        select_committed_moment_plan(
            &checkpoint,
            current,
            LogicalTime {
                ticks: 90,
                ticks_per_second: 10,
            },
            Some(permitted),
            &uncommitted,
            MomentLimits {
                maximum_alternatives: 8,
                maximum_items: 1024,
                maximum_shots: 8,
                maximum_demands: 8,
                maximum_duration_ticks: 100,
                maximum_reference_bytes: 1024,
                maximum_demand_bytes: 1024,
            },
        ),
        Err(MomentError::InvalidCurrentContext)
    ));
}

#[test]
fn supplied_policy_order_wins_without_an_invented_score() {
    let fixture = Fixture::new();
    let mut second_shots = fixture.shots.clone();
    second_shots[0].id = record(30);
    let alternatives = [
        MomentAlternative {
            context: fixture.context(),
            shots: &second_shots,
            demands: &fixture.demands,
        },
        fixture.alternative(),
    ];
    let result = select_moment_plan(
        fixture.context(),
        now(),
        Some(fixture.permitted()),
        &alternatives,
        limits(),
    )
    .unwrap();
    assert_eq!(result.selected.unwrap().shots[0].id, record(30));
}

#[test]
fn missing_producer_is_unavailable_in_every_execution_mode() {
    for mode in [
        ExecutionMode::Live,
        ExecutionMode::PreparedOnly,
        ExecutionMode::Replay,
    ] {
        let mut fixture = Fixture::new();
        fixture.mode = mode;
        fixture.demands[0].mode = mode;
        let alternatives = [fixture.alternative()];
        let result =
            select_moment_plan(fixture.context(), now(), None, &alternatives, limits()).unwrap();
        assert_eq!(result.disposition, MomentDisposition::Unavailable);
        assert!(result.selected.is_none());
    }
}

#[test]
fn private_fact_claim_character_location_or_policy_never_enters_a_plan() {
    for missing in 0..6 {
        let mut fixture = Fixture::new();
        match missing {
            0 => fixture.facts.clear(),
            1 => fixture.claims.clear(),
            2 => fixture.entities.retain(|id| *id != entity(9)),
            3 => fixture.entities.retain(|id| *id != entity(8)),
            4 => fixture.content.clear(),
            _ => fixture.pins.content.package = label("another-package"),
        }
        let alternatives = [fixture.alternative()];
        assert!(matches!(
            select_moment_plan(
                fixture.context(),
                now(),
                Some(fixture.permitted()),
                &alternatives,
                limits(),
            ),
            Err(MomentError::UnpermittedMoment)
        ));
    }
}

#[test]
fn withdrawn_current_asset_or_unpermitted_subject_uses_next_approved_alternative() {
    let fixture = Fixture::new();
    let mut forbidden_shots = fixture.shots.clone();
    forbidden_shots[0].subjects.push(entity(31));
    let alternatives = [
        MomentAlternative {
            context: fixture.context(),
            shots: &forbidden_shots,
            demands: &fixture.demands,
        },
        fixture.alternative(),
    ];
    let result = select_moment_plan(
        fixture.context(),
        now(),
        Some(fixture.permitted()),
        &alternatives,
        limits(),
    )
    .unwrap();
    assert_eq!(result.rejections.unpermitted, 1);
    assert!(std::ptr::eq(result.selected.unwrap(), &alternatives[1]));
    let mut revoked = fixture.permitted();
    revoked.assets = &[];
    let result = select_moment_plan(
        fixture.context(),
        now(),
        Some(revoked),
        &alternatives,
        limits(),
    )
    .unwrap();
    assert!(result.selected.is_none());
    assert_eq!(result.rejections.unpermitted, 2);
}

#[test]
fn expired_demands_do_not_wait_or_dispatch_and_expiry_is_inclusive() {
    let fixture = Fixture::new();
    let alternatives = [fixture.alternative()];
    for ticks in [10, 11, u64::MAX] {
        let result = select_moment_plan(
            fixture.context(),
            LogicalTime {
                ticks,
                ticks_per_second: 1,
            },
            Some(fixture.permitted()),
            &alternatives,
            limits(),
        )
        .unwrap();
        assert_eq!(result.disposition, MomentDisposition::Expired);
        assert!(result.selected.is_none());
    }
}

#[test]
fn whole_nested_input_is_bounded_before_first_ready_hit_or_absent_authority() {
    let fixture = Fixture::new();
    let mut oversized = fixture.shots.clone();
    oversized[0].subjects = vec![entity(9); 4097];
    let alternatives = [
        fixture.alternative(),
        MomentAlternative {
            context: fixture.context(),
            shots: &oversized,
            demands: &fixture.demands,
        },
    ];
    for permitted in [Some(fixture.permitted()), None] {
        assert!(matches!(
            select_moment_plan(fixture.context(), now(), permitted, &alternatives, limits(),),
            Err(MomentError::Capacity)
        ));
    }
}

#[test]
fn current_audience_source_revision_and_causal_facts_are_not_reinterpreted() {
    for mismatch in 0..3 {
        let mut fixture = Fixture::new();
        match mismatch {
            0 => fixture.presentation.audience = AudienceScope::Host,
            1 => {
                fixture.presentation.source_revision =
                    fixture.basis.revision.next_sequence().unwrap()
            }
            _ => fixture.presentation.causal_facts.push(fact(32)),
        }
        let alternatives = [fixture.alternative()];
        assert!(matches!(
            select_moment_plan(
                fixture.context(),
                now(),
                Some(fixture.permitted()),
                &alternatives,
                limits(),
            ),
            Err(MomentError::InvalidCurrentContext)
        ));
    }
}

#[test]
fn prior_source_authorization_context_is_refused() {
    let fixture = Fixture::new();
    let mut stale = Fixture::new();
    stale.pins.content.content_digest = ContentDigest([42; 32]);
    let alternatives = [fixture.alternative()];
    assert!(matches!(
        select_moment_plan(
            fixture.context(),
            now(),
            Some(stale.permitted()),
            &alternatives,
            limits(),
        ),
        Err(MomentError::StalePermittedContext)
    ));
}

#[test]
fn context_replacement_never_replays_an_old_alternative() {
    let fixture = Fixture::new();
    let mut old = Fixture::new();
    old.moment.identity_revision = label("identity-old");
    let alternatives = [old.alternative(), fixture.alternative()];
    let result = select_moment_plan(
        fixture.context(),
        now(),
        Some(fixture.permitted()),
        &alternatives,
        limits(),
    )
    .unwrap();
    assert_eq!(result.rejections.stale, 1);
    assert!(std::ptr::eq(result.selected.unwrap(), &alternatives[1]));
}

#[test]
fn exact_demand_key_basis_mode_expiry_and_policy_are_preserved() {
    for mismatch in 0..9 {
        let fixture = Fixture::new();
        let mut demands = fixture.demands.clone();
        match mismatch {
            0 => demands[0].key.schema = 9,
            1 => demands[0].key.source = ContentDigest([90; 32]),
            2 => demands[0].key.moment = record(90),
            3 => demands[0].key.identity = label("other-identity"),
            4 => demands[0].key.audience = AudienceScope::Host,
            5 => demands[0].mode = ExecutionMode::Live,
            6 => demands[0].basis.revision = demands[0].basis.revision.next_sequence().unwrap(),
            7 => demands[0].expires.ticks_per_second = 1000,
            _ => demands[0].policy.entry = label("unapproved-policy"),
        }
        let alternatives = [MomentAlternative {
            context: fixture.context(),
            shots: &fixture.shots,
            demands: &demands,
        }];
        let result = select_moment_plan(
            fixture.context(),
            now(),
            Some(fixture.permitted()),
            &alternatives,
            limits(),
        )
        .unwrap();
        assert!(result.selected.is_none(), "mismatch {mismatch}");
        assert_eq!(demands.len(), 1);
    }
}

#[test]
fn duplicate_shot_or_demand_id_and_duration_overflow_refuse_entire_alternative() {
    for malformed in 0..4 {
        let fixture = Fixture::new();
        let mut shots = fixture.shots.clone();
        let mut demands = fixture.demands.clone();
        match malformed {
            0 => shots.push(shots[0].clone()),
            1 => demands.push(demands[0].clone()),
            2 => shots[0].duration_ticks = 0,
            _ => {
                shots[0].duration_ticks = u64::MAX;
                let mut next = shots[0].clone();
                next.id = record(35);
                shots.push(next);
            }
        }
        let alternatives = [MomentAlternative {
            context: fixture.context(),
            shots: &shots,
            demands: &demands,
        }];
        let mut bound = limits();
        bound.maximum_duration_ticks = u64::MAX;
        let result = select_moment_plan(
            fixture.context(),
            now(),
            Some(fixture.permitted()),
            &alternatives,
            bound,
        )
        .unwrap();
        assert!(result.selected.is_none(), "malformed {malformed}");
        assert_eq!(result.rejections.invalid, 1);
    }
}

#[test]
fn byte_work_caps_preserve_exact_immutable_reference_and_demand() {
    let fixture = Fixture::new();
    let alternatives = [fixture.alternative()];
    for reference_cap in [1, 7] {
        let mut bound = limits();
        bound.maximum_reference_bytes = reference_cap;
        let result = select_moment_plan(
            fixture.context(),
            now(),
            Some(fixture.permitted()),
            &alternatives,
            bound,
        )
        .unwrap();
        assert!(result.selected.is_none());
    }
    let mut bound = limits();
    bound.maximum_demand_bytes = 63;
    assert!(
        select_moment_plan(
            fixture.context(),
            now(),
            Some(fixture.permitted()),
            &alternatives,
            bound,
        )
        .unwrap()
        .selected
        .is_none()
    );
}

#[test]
fn invalid_clock_zero_bounds_and_empty_policy_alternatives_are_explicit() {
    let fixture = Fixture::new();
    let alternatives = [fixture.alternative()];
    assert!(matches!(
        select_moment_plan(
            fixture.context(),
            LogicalTime {
                ticks: 0,
                ticks_per_second: 0
            },
            Some(fixture.permitted()),
            &alternatives,
            limits(),
        ),
        Err(MomentError::InvalidTimebase)
    ));
    let mut bound = limits();
    bound.maximum_items = 0;
    assert!(matches!(
        select_moment_plan(
            fixture.context(),
            now(),
            Some(fixture.permitted()),
            &alternatives,
            bound,
        ),
        Err(MomentError::InvalidLimits)
    ));
    assert_eq!(
        select_moment_plan(
            fixture.context(),
            now(),
            Some(fixture.permitted()),
            &[],
            limits(),
        )
        .unwrap()
        .disposition,
        MomentDisposition::Unavailable
    );
}

#[test]
fn same_logical_asset_key_cannot_substitute_digest_length_or_kind() {
    use df_model::checkpoint::AssetKind;
    for changed in 0..3 {
        let fixture = Fixture::new();
        let mut shots = fixture.shots.clone();
        match changed {
            0 => shots[0].references[0].digest = ContentDigest([99; 32]),
            1 => shots[0].references[0].byte_length += 1,
            _ => shots[0].references[0].kind = AssetKind::Video,
        }
        let alternatives = [MomentAlternative {
            context: fixture.context(),
            shots: &shots,
            demands: &fixture.demands,
        }];
        let result = select_moment_plan(
            fixture.context(),
            now(),
            Some(fixture.permitted()),
            &alternatives,
            limits(),
        )
        .unwrap();
        assert!(result.selected.is_none());
        assert_eq!(result.rejections.unpermitted, 1);
    }
}

#[test]
fn performance_hints_never_emphasize_hidden_facts_or_import_unadmitted_voice() {
    for changed in 0..3 {
        let fixture = Fixture::new();
        let mut shots = fixture.shots.clone();
        match changed {
            0 => shots[0].performance.emphasis_facts.push(fact(99)),
            1 => shots[0].performance.definition.entry = label("hidden-policy"),
            _ => {
                let mut voice = fixture.assets[0].clone();
                voice.key = label("hidden-voice");
                shots[0].performance.voice = Some(voice);
            }
        }
        let alternatives = [MomentAlternative {
            context: fixture.context(),
            shots: &shots,
            demands: &fixture.demands,
        }];
        let result = select_moment_plan(
            fixture.context(),
            now(),
            Some(fixture.permitted()),
            &alternatives,
            limits(),
        )
        .unwrap();
        assert!(result.selected.is_none());
        assert_eq!(result.rejections.unpermitted, 1);
    }
}

#[test]
fn session_run_epoch_build_and_pin_replacement_fence_old_proposals() {
    use df_types::{RecoveryEpoch, RunId, SessionId, SessionRevision};
    for changed in 0..7 {
        let fixture = Fixture::new();
        let mut old = Fixture::new();
        match changed {
            0 => old.basis.session = SessionId::from_bytes(&[90; 16]).unwrap(),
            1 => old.basis.run = RunId::from_bytes(&[91; 16]).unwrap(),
            2 => old.basis.revision = SessionRevision::new(RecoveryEpoch::new(2).unwrap(), 4),
            3 => old.pins.rules.handler_digest = ContentDigest([92; 32]),
            4 => old.pins.content.package_digest = ContentDigest([93; 32]),
            5 => old.mode = ExecutionMode::Replay,
            _ => old.presentation.id = record(94),
        }
        let alternatives = [old.alternative()];
        let result = select_moment_plan(
            fixture.context(),
            now(),
            Some(fixture.permitted()),
            &alternatives,
            limits(),
        )
        .unwrap();
        assert!(result.selected.is_none());
        assert_eq!(result.rejections.stale, 1);
    }
}

#[test]
fn exact_caps_allow_selection_and_zero_demand_policy_does_not_create_generation() {
    let mut fixture = Fixture::new();
    fixture.demands.clear();
    let alternatives = [fixture.alternative()];
    let mut bound = limits();
    bound.maximum_demands = 0;
    bound.maximum_duration_ticks = 5;
    bound.maximum_reference_bytes = 4;
    let result = select_moment_plan(
        fixture.context(),
        now(),
        Some(fixture.permitted()),
        &alternatives,
        bound,
    )
    .unwrap();
    assert_eq!(result.disposition, MomentDisposition::Selected);
    assert!(result.selected.unwrap().demands.is_empty());
}
