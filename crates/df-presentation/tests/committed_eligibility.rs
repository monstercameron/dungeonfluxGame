#[path = "../src/committed_eligibility.rs"]
mod committed_eligibility;
#[path = "../../df-session/tests/support/fixture_model.rs"]
pub mod fixture_model;

use committed_eligibility::*;
use df_media::continuity::*;
use df_model::checkpoint::*;
use df_presentation::critical::*;
use df_presentation::moment_selection::*;
use fixture_model as fixture;

fn fact(value: u8) -> FactId {
    FactId::from_bytes(&[value; 16]).unwrap()
}
fn record(value: u8) -> RecordId {
    RecordId::from_bytes(&[value; 16]).unwrap()
}
fn operation(value: u8) -> df_types::OperationId {
    df_types::OperationId::from_bytes(&[value; 16]).unwrap()
}
fn resolution() -> ResolutionId {
    ResolutionId::from_bytes(&[50; 16]).unwrap()
}

struct Fixture {
    current: Checkpoint,
    rules: Vec<RuleReference>,
    content: Vec<ContentReference>,
    resources: Vec<ResourceConstraint>,
    assets: Vec<AssetReference>,
    entities: Vec<EntityId>,
}
impl Fixture {
    fn new(edit: impl FnOnce(&mut GameState)) -> Self {
        let rules = vec![fixture::rule()];
        let content = vec![fixture::content()];
        let resources = fixture::resource_constraints();
        let assets = vec![
            AssetReference {
                key: fixture::label("prepared-scene-still"),
                digest: ContentDigest([80; 32]),
                byte_length: 4,
                kind: AssetKind::Image,
            },
            AssetReference {
                key: fixture::label("exact-scene-geometry"),
                digest: ContentDigest([81; 32]),
                byte_length: 4,
                kind: AssetKind::TacticalGeometry,
            },
        ];
        let mut state = fixture::state();
        state.mode = ExecutionMode::PreparedOnly;
        let mut scene = state.entities[0].clone();
        scene.id = fixture::entity(5);
        state.entities.push(scene);
        for (id, revision) in [
            (30, fixture::revision(2, 7)),
            (31, fixture::basis().revision),
        ] {
            state.facts.push(GameFact {
                id: fact(id),
                revision,
                operation: operation(id),
                ordinal: 0,
                cause: None,
                audience: AudienceScope::Shared,
                value: FactValue::ContentEvent {
                    definition: fixture::content(),
                    subjects: vec![fixture::entity(4), fixture::entity(5)],
                },
            });
            state.decisions.push(AcceptedDecision {
                operation: operation(id),
                revision,
                facts: vec![fact(id)],
                draws: vec![],
                effects: vec![],
                source_policy: fixture::label("source-resolved-outcome"),
                semantic_output: None,
            });
        }
        state.continuity.canonical_packs.push(CanonicalPack {
            revision: fixture::label("pack-v1"),
            digest: ContentDigest([70; 32]),
            bible: VisualBible {
                revision: fixture::label("bible-v1"),
                definition: fixture::content(),
                palette: vec![],
                style: String::new(),
                references: vec![assets[0].clone()],
            },
            identities: vec![],
        });
        state.continuity.scenes.push(SceneIdentityRevision {
            scene: fixture::entity(5),
            revision: fixture::label("scene-v1"),
            source_facts: vec![fact(30)],
            geometry: assets[1].clone(),
            canonical_pack: fixture::label("pack-v1"),
        });
        state.continuity.moments.push(NarrativeMoment {
            id: record(40),
            location: fixture::entity(5),
            characters: vec![fixture::entity(4)],
            facts: vec![fact(30), fact(31)],
            attributed_claims: vec![],
            audience: AudienceScope::Shared,
            semantic_focus: fixture::content(),
            identity_revision: fixture::label("scene-v1"),
        });
        state.presentation.push(PresentationDemand {
            id: record(41),
            definition: fixture::content(),
            audience: AudienceScope::Shared,
            causal_facts: vec![fact(31)],
            source_revision: fixture::basis().revision,
        });
        state.continuity.shots.push(ShotPlan {
            id: record(42),
            moment: record(40),
            subjects: vec![fixture::entity(4)],
            audience: AudienceScope::Shared,
            duration_ticks: 5,
            definition: fixture::content(),
            references: vec![assets[0].clone()],
            performance: PerformanceHint {
                definition: fixture::content(),
                voice: None,
                emphasis_facts: vec![fact(31)],
            },
        });
        state.continuity.critical_cues.push(CriticalCueEligibility {
            id: record(43),
            fact: fact(31),
            resolution: resolution(),
            audience: AudienceScope::Shared,
            ready_assets: vec![assets[0].clone()],
            policy: fixture::content(),
            maximum_duration_ticks: 20,
        });
        edit(&mut state);
        let current = Checkpoint::new(
            CHECKPOINT_SCHEMA,
            fixture::basis(),
            fixture::pins(),
            state,
            ReferenceInventory {
                rules: &rules,
                content: &content,
                resources: &resources,
                assets: &assets,
            },
            Self::checkpoint_limits(),
        )
        .unwrap();
        Self {
            current,
            rules,
            content,
            resources,
            assets,
            entities: vec![fixture::entity(4), fixture::entity(5)],
        }
    }
    fn checkpoint_limits() -> CheckpointLimits {
        let mut limits = fixture::limits();
        limits.maximum_records = 512;
        limits.maximum_total_text_bytes = 16 * 1024;
        limits
    }
    fn inventory(&self) -> ReferenceInventory<'_> {
        ReferenceInventory {
            rules: &self.rules,
            content: &self.content,
            resources: &self.resources,
            assets: &self.assets,
        }
    }
    fn critical(&self) -> CriticalContext<'_> {
        CriticalContext {
            basis: self.current.basis(),
            pins: self.current.pins(),
            cue: &self.current.state().continuity.critical_cues[0],
        }
    }
    fn moment(&self) -> MomentContext<'_> {
        MomentContext {
            basis: self.current.basis(),
            pins: self.current.pins(),
            mode: self.current.state().mode,
            moment: &self.current.state().continuity.moments[0],
            presentation: &self.current.state().presentation[0],
        }
    }
    fn alternative(&self) -> MomentAlternative<'_> {
        MomentAlternative {
            context: self.moment(),
            shots: &self.current.state().continuity.shots,
            demands: &[],
        }
    }
    fn binding(&self) -> ContinuityBinding<'_> {
        let pack = &self.current.state().continuity.canonical_packs[0];
        let scene = &self.current.state().continuity.scenes[0];
        ContinuityBinding {
            basis: self.current.basis(),
            source: self.current.pins().content.content_digest,
            mode: self.current.state().mode,
            target: IdentityTarget::Scene(scene.scene),
            revision: &scene.revision,
            pack_revision: &pack.revision,
            pack_digest: pack.digest,
            bible_revision: &pack.bible.revision,
            facts: &scene.source_facts,
            audience: &self.critical().cue.audience,
            scene: Some(scene),
            item: None,
        }
    }
    fn prepared(&self) -> PreparedRepresentation<'_> {
        PreparedRepresentation {
            binding: self.binding(),
            tier: PreparedTier::Still,
            asset: &self.assets[0],
        }
    }
    fn bytes(&self) -> AdmittedPreparedBytes<'_> {
        AdmittedPreparedBytes {
            asset: &self.assets[0],
            bytes: &[80, 81, 82, 83],
        }
    }
    fn request<'a>(
        &'a self,
        alternatives: &'a [MomentAlternative<'a>],
        prepared: &'a [PreparedRepresentation<'a>],
        bytes: &'a [AdmittedPreparedBytes<'a>],
    ) -> EligibilityRequest<'a> {
        EligibilityRequest {
            critical: CriticalCueRequest {
                context: self.critical(),
                now: LogicalTime {
                    ticks: 120,
                    ticks_per_second: 10,
                },
                duration_ticks: 10,
                limits: CriticalCueLimits {
                    maximum_items: 128,
                    maximum_asset_bytes: 64,
                    maximum_duration_ticks: 30,
                },
            },
            outcome: Some(ResolvedOutcomeAdmission {
                context: self.critical(),
                fact: &self.current.state().facts[1],
                decision: self
                    .current
                    .state()
                    .decisions
                    .iter()
                    .find(|decision| decision.operation == operation(31))
                    .unwrap(),
                resolution: resolution(),
                source: &self.rules[0],
                outcome: ResolvedOutcome::Succeeded,
            }),
            critical_permission: Some(PermittedCriticalCue {
                context: self.critical(),
                fact: &self.current.state().facts[1],
                content: &self.content,
                assets: &self.assets,
                cadence: CueCadence::Allowed,
                expires: LogicalTime {
                    ticks: 140,
                    ticks_per_second: 10,
                },
            }),
            moment: self.moment(),
            moment_permission: Some(PermittedMoment {
                context: self.moment(),
                facts: &self.current.state().continuity.moments[0].facts,
                attributed_claims: &[],
                entities: &self.entities,
                content: &self.content,
                assets: &self.assets,
            }),
            alternatives,
            continuity: self.binding(),
            tiers: &[PreparedTier::Still],
            prepared,
            admitted_bytes: bytes,
        }
    }
    fn limits() -> EligibilityLimits {
        EligibilityLimits {
            moment: MomentLimits {
                maximum_alternatives: 8,
                maximum_items: 1024,
                maximum_shots: 8,
                maximum_demands: 8,
                maximum_duration_ticks: 100,
                maximum_reference_bytes: 1024,
                maximum_demand_bytes: 1024,
            },
            continuity: ContinuityLimits {
                checkpoint: Self::checkpoint_limits(),
                maximum_prepared_records: 16,
                maximum_prepared_bytes: 1024,
            },
        }
    }
}

#[test]
fn committed_source_admitted_fact_gates_critical_and_exact_lineage_gates_identity_continuity() {
    let fixture = Fixture::new(|_| {});
    let before = fixture.current.clone();
    let alternatives = [fixture.alternative()];
    let prepared = [fixture.prepared()];
    let bytes = [fixture.bytes()];
    let selected = select_committed_eligibility(
        &fixture.current,
        fixture.inventory(),
        fixture.request(&alternatives, &prepared, &bytes),
        Fixture::limits(),
    );
    assert!(std::ptr::eq(
        selected.critical.unwrap(),
        fixture.critical().cue
    ));
    assert_eq!(
        selected.moment.unwrap().disposition,
        MomentDisposition::Selected
    );
    let continuity = selected.continuity.unwrap();
    assert_eq!(continuity.binding.facts, &[fact(30)]);
    assert_eq!(continuity.bytes, &[80, 81, 82, 83]);
    assert_eq!(continuity.pack.revision, fixture::label("pack-v1"));
    assert_eq!(fixture.current, before);
}

#[test]
fn failed_or_missing_source_admission_suppresses_celebration_without_erasing_valid_continuity() {
    let fixture = Fixture::new(|_| {});
    let alternatives = [fixture.alternative()];
    let prepared = [fixture.prepared()];
    let bytes = [fixture.bytes()];
    for failed in [true, false] {
        let mut request = fixture.request(&alternatives, &prepared, &bytes);
        if failed {
            request.outcome.as_mut().unwrap().outcome = ResolvedOutcome::Failed;
        } else {
            request.outcome = None;
        }
        let selected = select_committed_eligibility(
            &fixture.current,
            fixture.inventory(),
            request,
            Fixture::limits(),
        );
        assert_eq!(
            selected.critical,
            Err(if failed {
                CriticalCueError::OutcomeNotSuccessful
            } else {
                CriticalCueError::MissingOutcomeAdmission
            })
        );
        assert!(selected.continuity.is_ok());
    }
}

#[test]
fn tentative_critical_fact_and_unaccepted_identity_fact_are_independently_refused() {
    let fixture = Fixture::new(|_| {});
    let alternatives = [fixture.alternative()];
    let prepared = [fixture.prepared()];
    let bytes = [fixture.bytes()];
    let mut tentative = fixture.current.state().facts[1].clone();
    tentative.id = fact(99);
    let mut request = fixture.request(&alternatives, &prepared, &bytes);
    request.outcome.as_mut().unwrap().fact = &tentative;
    let selected = select_committed_eligibility(
        &fixture.current,
        fixture.inventory(),
        request,
        Fixture::limits(),
    );
    assert_eq!(
        selected.critical,
        Err(CriticalCueError::InvalidOutcomeAdmission)
    );
    assert!(selected.continuity.is_ok());
    // Canonical facts need not all be accepted decisions; the actual continuity owner checks this.
    let unaccepted = Fixture::new(|state| {
        state.decisions.remove(0);
    });
    let alternatives = [unaccepted.alternative()];
    let prepared = [unaccepted.prepared()];
    let bytes = [unaccepted.bytes()];
    let selected = select_committed_eligibility(
        &unaccepted.current,
        unaccepted.inventory(),
        unaccepted.request(&alternatives, &prepared, &bytes),
        Fixture::limits(),
    );
    assert!(selected.critical.is_ok());
    assert!(matches!(
        selected.continuity,
        Err(ContinuityError::UncommittedFact)
    ));
}

#[test]
fn shared_projection_refuses_private_critical_or_identity_cause() {
    for index in [0, 1] {
        let fixture = Fixture::new(|state| {
            state.facts[index].audience = AudienceScope::Members(vec![fixture::member(3)])
        });
        let alternatives = [fixture.alternative()];
        let prepared = [fixture.prepared()];
        let bytes = [fixture.bytes()];
        let selected = select_committed_eligibility(
            &fixture.current,
            fixture.inventory(),
            fixture.request(&alternatives, &prepared, &bytes),
            Fixture::limits(),
        );
        if index == 1 {
            assert_eq!(selected.critical, Err(CriticalCueError::AudienceDenied));
        } else {
            assert!(matches!(
                selected.continuity,
                Err(ContinuityError::AudienceUnavailable)
            ));
        }
    }
}

#[test]
fn current_filtered_permission_is_required_and_does_not_replace_committed_provenance() {
    let fixture = Fixture::new(|_| {});
    let alternatives = [fixture.alternative()];
    let prepared = [fixture.prepared()];
    let bytes = [fixture.bytes()];
    let mut request = fixture.request(&alternatives, &prepared, &bytes);
    request.critical_permission = None;
    request.moment_permission = None;
    let selected = select_committed_eligibility(
        &fixture.current,
        fixture.inventory(),
        request,
        Fixture::limits(),
    );
    assert_eq!(selected.critical, Err(CriticalCueError::MissingPermission));
    assert_eq!(
        selected.moment.unwrap().disposition,
        MomentDisposition::Unavailable
    );
    assert!(selected.continuity.is_ok());
}

#[test]
fn stale_binding_and_old_prepared_offer_are_not_reused_as_current_identity() {
    let fixture = Fixture::new(|_| {});
    let alternatives = [fixture.alternative()];
    let mut prepared = [fixture.prepared()];
    let bytes = [fixture.bytes()];
    prepared[0].binding.source = ContentDigest([99; 32]);
    let selected = select_committed_eligibility(
        &fixture.current,
        fixture.inventory(),
        fixture.request(&alternatives, &prepared, &bytes),
        Fixture::limits(),
    );
    assert!(selected.critical.is_ok());
    assert!(matches!(
        selected.continuity,
        Err(ContinuityError::PreparedMismatch)
    ));
    let prepared = [fixture.prepared()];
    let mut request = fixture.request(&alternatives, &prepared, &bytes);
    request.critical.context.basis.revision = fixture::revision(2, 7);
    let selected = select_committed_eligibility(
        &fixture.current,
        fixture.inventory(),
        request,
        Fixture::limits(),
    );
    assert!(matches!(
        selected.critical,
        Err(CriticalCueError::Checkpoint(CheckpointError::StaleBasis))
    ));
    assert!(matches!(
        selected.continuity,
        Err(ContinuityError::Checkpoint(CheckpointError::StaleBasis))
    ));
}

#[test]
fn fabricated_uncommitted_moment_is_refused_without_changing_valid_critical_or_identity_records() {
    let fixture = Fixture::new(|_| {});
    let before = fixture.current.clone();
    let mut moment = fixture.current.state().continuity.moments[0].clone();
    moment.id = record(99);
    let context = MomentContext {
        moment: &moment,
        ..fixture.moment()
    };
    let alternatives = [MomentAlternative {
        context,
        ..fixture.alternative()
    }];
    let prepared = [fixture.prepared()];
    let bytes = [fixture.bytes()];
    let mut request = fixture.request(&alternatives, &prepared, &bytes);
    request.moment = context;
    request.moment_permission.as_mut().unwrap().context = context;
    let selected = select_committed_eligibility(
        &fixture.current,
        fixture.inventory(),
        request,
        Fixture::limits(),
    );
    assert!(matches!(
        selected.moment,
        Err(MomentError::InvalidCurrentContext)
    ));
    assert!(selected.critical.is_ok());
    assert!(selected.continuity.is_ok());
    assert_eq!(fixture.current, before);
}

#[test]
fn replay_and_cadence_suppress_new_celebration_while_retaining_prepared_continuity() {
    for replay in [true, false] {
        let fixture = Fixture::new(|state| {
            if replay {
                state.mode = ExecutionMode::Replay;
            }
        });
        let alternatives = [fixture.alternative()];
        let prepared = [fixture.prepared()];
        let bytes = [fixture.bytes()];
        let mut request = fixture.request(&alternatives, &prepared, &bytes);
        request.critical_permission.as_mut().unwrap().cadence = CueCadence::Suppressed;
        let selected = select_committed_eligibility(
            &fixture.current,
            fixture.inventory(),
            request,
            Fixture::limits(),
        );
        assert_eq!(
            selected.critical,
            Err(if replay {
                CriticalCueError::ReplaySuppressed
            } else {
                CriticalCueError::Suppressed
            })
        );
        assert!(selected.continuity.is_ok());
    }
}

#[test]
fn incomplete_prepared_bytes_and_capacity_fail_without_mutation_or_provider_work() {
    let fixture = Fixture::new(|_| {});
    let before = fixture.current.clone();
    let alternatives = [fixture.alternative()];
    let prepared = [fixture.prepared()];
    let bytes = [AdmittedPreparedBytes {
        asset: &fixture.assets[0],
        bytes: &[80, 81],
    }];
    let selected = select_committed_eligibility(
        &fixture.current,
        fixture.inventory(),
        fixture.request(&alternatives, &prepared, &bytes),
        Fixture::limits(),
    );
    assert!(matches!(
        selected.continuity,
        Err(ContinuityError::BytesUnavailable)
    ));
    assert!(selected.critical.is_ok());
    let bytes = [fixture.bytes()];
    let mut bound = Fixture::limits();
    bound.continuity.maximum_prepared_records = 1;
    let selected = select_committed_eligibility(
        &fixture.current,
        fixture.inventory(),
        fixture.request(&alternatives, &prepared, &bytes),
        bound,
    );
    assert!(selected.continuity.is_ok());
    let excess = [fixture.prepared(), fixture.prepared()];
    let mut bound = Fixture::limits();
    bound.continuity.maximum_prepared_records = 1;
    let selected = select_committed_eligibility(
        &fixture.current,
        fixture.inventory(),
        fixture.request(&alternatives, &excess, &bytes),
        bound,
    );
    assert!(matches!(
        selected.continuity,
        Err(ContinuityError::Capacity)
    ));
    assert_eq!(fixture.current, before);
}
