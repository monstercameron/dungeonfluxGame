//! Synthetic admitted owner inputs exercise actual public selection; no provider/source issuance.
use df_media::continuity::*;
use df_model::checkpoint::*;
use df_types::{OperationId, RevisionLabel};

fn label(value: &str) -> RevisionLabel {
    fixture::label(value)
}
fn fact(value: u8) -> FactId {
    FactId::from_bytes(&[value; 16]).unwrap()
}
fn operation(value: u8) -> OperationId {
    OperationId::from_bytes(&[value; 16]).unwrap()
}
fn limits() -> ContinuityLimits {
    ContinuityLimits {
        checkpoint: fixture::limits(),
        maximum_prepared_records: 16,
        maximum_prepared_bytes: 1024,
    }
}

struct Fixture {
    pins: CheckpointPins,
    rules: Vec<RuleReference>,
    contents: Vec<ContentReference>,
    resources: Vec<ResourceConstraint>,
    assets: Vec<AssetReference>,
}
impl Fixture {
    fn new() -> Self {
        Self {
            pins: fixture::pins(),
            rules: vec![fixture::rule()],
            contents: vec![fixture::content()],
            resources: fixture::resource_constraints(),
            assets: [
                AssetKind::Image,
                AssetKind::Video,
                AssetKind::Audio,
                AssetKind::TacticalGeometry,
            ]
            .into_iter()
            .enumerate()
            .map(|(index, kind)| AssetReference {
                key: label(&format!("immutable-asset-{index}")),
                digest: ContentDigest([index as u8 + 80; 32]),
                byte_length: 4,
                kind,
            })
            .collect(),
        }
    }
    fn inventory(&self) -> ReferenceInventory<'_> {
        ReferenceInventory {
            rules: &self.rules,
            content: &self.contents,
            resources: &self.resources,
            assets: &self.assets,
        }
    }
    fn context<'a>(&'a self, current: &'a Checkpoint) -> ContinuityContext<'a> {
        ContinuityContext {
            current,
            expected: current.basis(),
            pins: &self.pins,
            inventory: self.inventory(),
        }
    }
    fn checkpoint(&self, state: GameState) -> Checkpoint {
        Checkpoint::new(
            CHECKPOINT_SCHEMA,
            fixture::basis(),
            self.pins.clone(),
            state,
            self.inventory(),
            fixture::limits(),
        )
        .unwrap()
    }
    fn state(&self) -> GameState {
        let mut state = fixture::state();
        for id in [5, 6, 7] {
            state.entities.push(WorldEntity {
                id: fixture::entity(id),
                definition: fixture::content(),
                location: None,
                position: None,
                identity_revision: label("entity-v2"),
            });
        }
        for id in [30, 31, 32] {
            state.facts.push(GameFact {
                id: fact(id),
                revision: state_revision(),
                operation: operation(id),
                ordinal: 0,
                cause: None,
                audience: AudienceScope::Shared,
                value: FactValue::ContentEvent {
                    definition: fixture::content(),
                    subjects: vec![fixture::entity(id - 25)],
                },
            });
            state.decisions.push(AcceptedDecision {
                operation: operation(id),
                revision: state_revision(),
                facts: vec![fact(id)],
                draws: vec![],
                effects: vec![],
                source_policy: label("accepted-source-policy"),
                semantic_output: None,
            });
        }
        state.continuity.canonical_packs.push(CanonicalPack {
            revision: label("pack-v2"),
            digest: ContentDigest([70; 32]),
            bible: VisualBible {
                revision: label("bible-v2"),
                definition: fixture::content(),
                palette: vec![label("amber")],
                style: "Approved amber style".into(),
                references: self.assets[..3].to_vec(),
            },
            identities: [6, 7]
                .into_iter()
                .map(|id| EntityIdentityRevision {
                    entity: fixture::entity(id),
                    revision: label("entity-v2"),
                    character_appearance: None,
                    source_facts: vec![fact(id + 25)],
                    appearances: self.assets[..2].to_vec(),
                    voice: Some(self.assets[2].clone()),
                    sound: vec![],
                })
                .collect(),
        });
        state.continuity.scenes.push(SceneIdentityRevision {
            scene: fixture::entity(5),
            revision: label("scene-v3"),
            source_facts: vec![fact(30)],
            geometry: self.assets[3].clone(),
            canonical_pack: label("pack-v2"),
        });
        state.continuity.item_origins.push(ItemOrigin {
            item: fixture::entity(6),
            award_operation: operation(31),
            source_fact: fact(31),
            definition: fixture::content(),
            identity_revision: label("entity-v2"),
        });
        state.continuity.npcs.push(NpcState {
            entity: fixture::entity(7),
            role: fixture::content(),
            personality: fixture::content(),
            motivations: vec![],
            goals: vec![],
            needs: vec![],
            fears: vec![],
            known_facts: vec![],
            beliefs: vec![],
            secrets: vec![],
        });
        state
    }
    fn binding<'a>(
        &self,
        current: &'a Checkpoint,
        target: IdentityTarget,
        audience: &'a AudienceScope,
    ) -> ContinuityBinding<'a> {
        let pack = &current.state().continuity.canonical_packs[0];
        let (revision, facts, scene, item) = match target {
            IdentityTarget::Scene(_) => {
                let scene = &current.state().continuity.scenes[0];
                (
                    &scene.revision,
                    scene.source_facts.as_slice(),
                    Some(scene),
                    None,
                )
            }
            IdentityTarget::Item(_) => {
                let identity = &pack.identities[0];
                (
                    &identity.revision,
                    identity.source_facts.as_slice(),
                    None,
                    Some(&current.state().continuity.item_origins[0]),
                )
            }
            IdentityTarget::Npc(_) => {
                let identity = &pack.identities[1];
                (
                    &identity.revision,
                    identity.source_facts.as_slice(),
                    None,
                    None,
                )
            }
        };
        ContinuityBinding {
            basis: current.basis(),
            source: current.pins().content.content_digest,
            mode: current.state().mode,
            target,
            revision,
            pack_revision: &pack.revision,
            pack_digest: pack.digest,
            bible_revision: &pack.bible.revision,
            facts,
            audience,
            scene,
            item,
        }
    }
    fn bytes(&self) -> Vec<AdmittedPreparedBytes<'_>> {
        self.assets
            .iter()
            .map(|asset| AdmittedPreparedBytes {
                asset,
                bytes: &[80, 81, 82, 83],
            })
            .collect()
    }
}
fn state_revision() -> df_types::SessionRevision {
    fixture::basis().revision
}
fn targets() -> [IdentityTarget; 3] {
    [
        IdentityTarget::Scene(fixture::entity(5)),
        IdentityTarget::Item(fixture::entity(6)),
        IdentityTarget::Npc(fixture::entity(7)),
    ]
}

#[test]
fn scene_item_and_npc_select_exact_current_canonical_versions() {
    let fixture = Fixture::new();
    let audience = AudienceScope::Shared;
    for mode in [
        ExecutionMode::Live,
        ExecutionMode::PreparedOnly,
        ExecutionMode::Replay,
    ] {
        let mut state = fixture.state();
        state.mode = mode;
        let checkpoint = fixture.checkpoint(state);
        let before = checkpoint.clone();
        for target in targets() {
            let binding = fixture.binding(&checkpoint, target, &audience);
            let offers = [PreparedRepresentation {
                binding,
                tier: PreparedTier::Still,
                asset: &fixture.assets[0],
            }];
            let selected = select_continuity(
                fixture.context(&checkpoint),
                binding,
                &[PreparedTier::Still],
                &offers,
                &fixture.bytes(),
                limits(),
            )
            .unwrap();
            assert_eq!(selected.binding, binding);
            assert_eq!(
                selected.pack,
                &checkpoint.state().continuity.canonical_packs[0]
            );
            assert_eq!(selected.bible.revision, label("bible-v2"));
            assert_eq!(selected.bytes, &[80, 81, 82, 83]);
            assert!(!selected.fallback);
            assert_eq!(
                selected.identity.is_some(),
                !matches!(target, IdentityTarget::Scene(_))
            );
        }
        assert_eq!(checkpoint, before);
    }
}

#[test]
fn prepared_tiers_and_fallback_preserve_identity_bible_and_references() {
    let fixture = Fixture::new();
    let checkpoint = fixture.checkpoint(fixture.state());
    let audience = AudienceScope::Shared;
    for target in targets() {
        let binding = fixture.binding(&checkpoint, target, &audience);
        let offers = [
            PreparedRepresentation {
                binding,
                tier: PreparedTier::Still,
                asset: &fixture.assets[0],
            },
            PreparedRepresentation {
                binding,
                tier: PreparedTier::Audio,
                asset: &fixture.assets[2],
            },
        ];
        let tiers = [
            PreparedTier::Video,
            PreparedTier::Still,
            PreparedTier::Audio,
        ];
        let bytes = fixture.bytes();
        let selected = select_continuity(
            fixture.context(&checkpoint),
            binding,
            &tiers,
            &offers,
            &bytes,
            limits(),
        )
        .unwrap();
        assert!(selected.fallback);
        assert_eq!(selected.representation.tier, PreparedTier::Still);
        assert_eq!(selected.binding, binding);
        assert!(std::ptr::eq(selected.bible, &selected.pack.bible));
        let audio = select_continuity(
            fixture.context(&checkpoint),
            binding,
            &tiers,
            &offers[1..],
            &bytes,
            limits(),
        )
        .unwrap();
        assert_eq!(audio.representation.asset, &fixture.assets[2]);
        assert!(audio.fallback);
        let mismatched = [PreparedRepresentation {
            binding: ContinuityBinding {
                pack_digest: ContentDigest([99; 32]),
                ..binding
            },
            ..offers[0]
        }];
        assert_eq!(
            select_continuity(
                fixture.context(&checkpoint),
                binding,
                &tiers,
                &mismatched,
                &bytes,
                limits()
            )
            .unwrap_err(),
            ContinuityError::PreparedMismatch
        );
    }
    let binding = fixture.binding(&checkpoint, targets()[0], &audience);
    let offers = [PreparedRepresentation {
        binding,
        tier: PreparedTier::FlatScene,
        asset: &fixture.assets[3],
    }];
    assert_eq!(
        select_continuity(
            fixture.context(&checkpoint),
            binding,
            &[PreparedTier::FlatScene],
            &offers,
            &fixture.bytes(),
            limits()
        )
        .unwrap()
        .representation
        .asset,
        &checkpoint.state().continuity.scenes[0].geometry
    );
}

#[test]
fn late_stale_wrong_digest_or_ambiguous_pack_cannot_revert_current_identity() {
    let fixture = Fixture::new();
    let checkpoint = fixture.checkpoint(fixture.state());
    let audience = AudienceScope::Shared;
    let old = label("old-v1");
    for target in targets() {
        let binding = fixture.binding(&checkpoint, target, &audience);
        for (changed, expected) in [
            (
                ContinuityBinding {
                    revision: &old,
                    ..binding
                },
                ContinuityError::StaleBinding,
            ),
            (
                ContinuityBinding {
                    pack_digest: ContentDigest([99; 32]),
                    ..binding
                },
                ContinuityError::PackMismatch,
            ),
            (
                ContinuityBinding {
                    bible_revision: &old,
                    ..binding
                },
                ContinuityError::PackMismatch,
            ),
            (
                ContinuityBinding {
                    source: ContentDigest([99; 32]),
                    ..binding
                },
                ContinuityError::StaleBinding,
            ),
            (
                ContinuityBinding {
                    basis: Basis {
                        revision: binding.basis.revision.next_sequence().unwrap(),
                        ..binding.basis
                    },
                    ..binding
                },
                ContinuityError::StaleBinding,
            ),
        ] {
            assert_eq!(
                select_continuity(
                    fixture.context(&checkpoint),
                    changed,
                    &[PreparedTier::Still],
                    &[],
                    &[],
                    limits()
                )
                .unwrap_err(),
                expected
            );
        }
        let mut state = checkpoint.state().clone();
        state
            .continuity
            .canonical_packs
            .push(state.continuity.canonical_packs[0].clone());
        let ambiguous = fixture.checkpoint(state);
        assert_eq!(
            select_continuity(
                fixture.context(&ambiguous),
                binding,
                &[PreparedTier::Still],
                &[],
                &[],
                limits()
            )
            .unwrap_err(),
            ContinuityError::AmbiguousIdentity
        );
    }
    let binding = fixture.binding(&checkpoint, targets()[0], &audience);
    let mut state = checkpoint.state().clone();
    state
        .continuity
        .scenes
        .push(state.continuity.scenes[0].clone());
    let ambiguous = fixture.checkpoint(state);
    assert_eq!(
        select_continuity(
            fixture.context(&ambiguous),
            binding,
            &[PreparedTier::Still],
            &[],
            &[],
            limits()
        )
        .unwrap_err(),
        ContinuityError::AmbiguousIdentity
    );
    let mut state = checkpoint.state().clone();
    let mut old_pack = state.continuity.canonical_packs[0].clone();
    old_pack.revision = old;
    state.continuity.canonical_packs.push(old_pack);
    let historical = fixture.checkpoint(state);
    let before = historical.clone();
    assert_eq!(
        select_continuity(
            fixture.context(&historical),
            binding,
            &[PreparedTier::Still],
            &[],
            &[],
            limits()
        )
        .unwrap_err(),
        ContinuityError::Unavailable
    );
    assert_eq!(historical, before);
    let scene_binding = fixture.binding(&checkpoint, targets()[0], &audience);
    let mut wrong_scene = scene_binding.scene.unwrap().clone();
    wrong_scene.geometry.digest = ContentDigest([99; 32]);
    assert_eq!(
        select_continuity(
            fixture.context(&checkpoint),
            ContinuityBinding {
                scene: Some(&wrong_scene),
                ..scene_binding
            },
            &[PreparedTier::Still],
            &[],
            &[],
            limits()
        )
        .unwrap_err(),
        ContinuityError::StaleBinding
    );
    let item_binding = fixture.binding(&checkpoint, targets()[1], &audience);
    let mut wrong_item = item_binding.item.unwrap().clone();
    wrong_item.award_operation = operation(99);
    assert_eq!(
        select_continuity(
            fixture.context(&checkpoint),
            ContinuityBinding {
                item: Some(&wrong_item),
                ..item_binding
            },
            &[PreparedTier::Still],
            &[],
            &[],
            limits()
        )
        .unwrap_err(),
        ContinuityError::StaleBinding
    );
    let npc_binding = fixture.binding(&checkpoint, targets()[2], &audience);
    let mut evolved_state = checkpoint.state().clone();
    evolved_state
        .entities
        .iter_mut()
        .find(|entity| entity.id == fixture::entity(7))
        .unwrap()
        .identity_revision = label("npc-v3");
    let mut new_pack = evolved_state.continuity.canonical_packs[0].clone();
    new_pack.revision = label("pack-v3");
    new_pack.digest = ContentDigest([73; 32]);
    new_pack.identities[1].revision = label("npc-v3");
    evolved_state.continuity.canonical_packs.push(new_pack);
    let evolved = fixture.checkpoint(evolved_state);
    assert_eq!(
        select_continuity(
            fixture.context(&evolved),
            npc_binding,
            &[PreparedTier::Still],
            &[],
            &[],
            limits()
        )
        .unwrap_err(),
        ContinuityError::StaleBinding
    );
    assert_eq!(
        evolved.state().continuity.canonical_packs[0],
        checkpoint.state().continuity.canonical_packs[0]
    );
    let pack = &evolved.state().continuity.canonical_packs[1];
    let current_binding = ContinuityBinding {
        revision: &pack.identities[1].revision,
        pack_revision: &pack.revision,
        pack_digest: pack.digest,
        ..npc_binding
    };
    let offer = PreparedRepresentation {
        binding: current_binding,
        tier: PreparedTier::Still,
        asset: &fixture.assets[0],
    };
    assert!(
        select_continuity(
            fixture.context(&evolved),
            current_binding,
            &[PreparedTier::Still],
            &[offer],
            &fixture.bytes(),
            limits()
        )
        .is_ok()
    );
}

#[test]
fn hidden_source_or_audience_mismatch_cannot_change_public_selection() {
    let fixture = Fixture::new();
    let audience = AudienceScope::Shared;
    for target in targets() {
        let mut state = fixture.state();
        for fact in &mut state.facts {
            fact.audience = AudienceScope::Members(vec![fixture::member(3)]);
        }
        let checkpoint = fixture.checkpoint(state);
        let binding = fixture.binding(&checkpoint, target, &audience);
        let offers = [PreparedRepresentation {
            binding,
            tier: PreparedTier::Still,
            asset: &fixture.assets[0],
        }];
        assert_eq!(
            select_continuity(
                fixture.context(&checkpoint),
                binding,
                &[PreparedTier::Still],
                &offers,
                &fixture.bytes(),
                limits()
            )
            .unwrap_err(),
            ContinuityError::AudienceUnavailable
        );
        let member = AudienceScope::Members(vec![fixture::member(3)]);
        let permitted = fixture.binding(&checkpoint, target, &member);
        let offers = [PreparedRepresentation {
            binding: permitted,
            ..offers[0]
        }];
        assert!(
            select_continuity(
                fixture.context(&checkpoint),
                permitted,
                &[PreparedTier::Still],
                &offers,
                &fixture.bytes(),
                limits()
            )
            .is_ok()
        );
        let wrong_offer = [PreparedRepresentation {
            binding,
            ..offers[0]
        }];
        assert_eq!(
            select_continuity(
                fixture.context(&checkpoint),
                permitted,
                &[PreparedTier::Still],
                &wrong_offer,
                &fixture.bytes(),
                limits()
            )
            .unwrap_err(),
            ContinuityError::PreparedMismatch
        );
    }
    let mut state = fixture.state();
    state.decisions.clear();
    let checkpoint = fixture.checkpoint(state);
    let binding = fixture.binding(&checkpoint, targets()[2], &audience);
    assert_eq!(
        select_continuity(
            fixture.context(&checkpoint),
            binding,
            &[PreparedTier::Still],
            &[],
            &[],
            limits()
        )
        .unwrap_err(),
        ContinuityError::UncommittedFact
    );
}

#[test]
fn unavailable_unverified_or_wrong_tier_assets_return_typed_refusal() {
    let fixture = Fixture::new();
    let checkpoint = fixture.checkpoint(fixture.state());
    let audience = AudienceScope::Shared;
    for target in targets() {
        let binding = fixture.binding(&checkpoint, target, &audience);
        let offer = PreparedRepresentation {
            binding,
            tier: PreparedTier::Still,
            asset: &fixture.assets[0],
        };
        assert_eq!(
            select_continuity(
                fixture.context(&checkpoint),
                binding,
                &[PreparedTier::Still],
                &[],
                &[],
                limits()
            )
            .unwrap_err(),
            ContinuityError::Unavailable
        );
        assert_eq!(
            select_continuity(
                fixture.context(&checkpoint),
                binding,
                &[PreparedTier::Still],
                &[offer],
                &[],
                limits()
            )
            .unwrap_err(),
            ContinuityError::BytesUnavailable
        );
        for bytes in [&[][..], &[1, 2][..]] {
            assert_eq!(
                select_continuity(
                    fixture.context(&checkpoint),
                    binding,
                    &[PreparedTier::Still],
                    &[offer],
                    &[AdmittedPreparedBytes {
                        asset: offer.asset,
                        bytes
                    }],
                    limits()
                )
                .unwrap_err(),
                ContinuityError::BytesUnavailable
            );
        }
        let wrong = PreparedRepresentation {
            tier: PreparedTier::Video,
            ..offer
        };
        assert_eq!(
            select_continuity(
                fixture.context(&checkpoint),
                binding,
                &[PreparedTier::Video],
                &[wrong],
                &fixture.bytes(),
                limits()
            )
            .unwrap_err(),
            ContinuityError::UnsupportedTier
        );
        let mut reference = fixture.assets[0].clone();
        reference.digest = ContentDigest([99; 32]);
        let wrong = PreparedRepresentation {
            asset: &reference,
            ..offer
        };
        assert_eq!(
            select_continuity(
                fixture.context(&checkpoint),
                binding,
                &[PreparedTier::Still],
                &[wrong],
                &fixture.bytes(),
                limits()
            )
            .unwrap_err(),
            ContinuityError::ReferenceUnavailable
        );
    }
}

#[test]
fn capacity_duplicates_and_missing_policy_refuse_preserving_geometry_mechanics_and_checkpoint() {
    let fixture = Fixture::new();
    let checkpoint = fixture.checkpoint(fixture.state());
    let before = checkpoint.clone();
    let audience = AudienceScope::Shared;
    for target in targets() {
        let binding = fixture.binding(&checkpoint, target, &audience);
        let offer = PreparedRepresentation {
            binding,
            tier: PreparedTier::Still,
            asset: &fixture.assets[0],
        };
        for (tiers, expected) in [
            (&[][..], ContinuityError::MissingPolicy),
            (
                &[PreparedTier::Still, PreparedTier::Still][..],
                ContinuityError::Duplicate,
            ),
        ] {
            assert_eq!(
                select_continuity(
                    fixture.context(&checkpoint),
                    binding,
                    tiers,
                    &[offer],
                    &fixture.bytes(),
                    limits()
                )
                .unwrap_err(),
                expected
            );
        }
        let mut bounds = limits();
        bounds.maximum_prepared_records = 1;
        assert_eq!(
            select_continuity(
                fixture.context(&checkpoint),
                binding,
                &[PreparedTier::Still],
                &[offer],
                &fixture.bytes(),
                bounds
            )
            .unwrap_err(),
            ContinuityError::Capacity
        );
        bounds = limits();
        bounds.maximum_prepared_bytes = 3;
        assert_eq!(
            select_continuity(
                fixture.context(&checkpoint),
                binding,
                &[PreparedTier::Still],
                &[offer],
                &fixture.bytes(),
                bounds
            )
            .unwrap_err(),
            ContinuityError::Capacity
        );
        bounds = limits();
        bounds.maximum_prepared_records = 0;
        assert_eq!(
            select_continuity(
                fixture.context(&checkpoint),
                binding,
                &[PreparedTier::Still],
                &[],
                &[],
                bounds
            )
            .unwrap_err(),
            ContinuityError::InvalidLimits
        );
        assert_eq!(
            select_continuity(
                fixture.context(&checkpoint),
                binding,
                &[PreparedTier::Still],
                &[offer, offer],
                &fixture.bytes(),
                limits()
            )
            .unwrap_err(),
            ContinuityError::Duplicate
        );
        let duplicate_facts = [binding.facts[0], binding.facts[0]];
        assert_eq!(
            select_continuity(
                fixture.context(&checkpoint),
                ContinuityBinding {
                    facts: &duplicate_facts,
                    ..binding
                },
                &[PreparedTier::Still],
                &[],
                &[],
                limits()
            )
            .unwrap_err(),
            ContinuityError::Duplicate
        );
        assert_eq!(
            select_continuity(
                fixture.context(&checkpoint),
                ContinuityBinding {
                    facts: &[],
                    ..binding
                },
                &[PreparedTier::Still],
                &[],
                &[],
                limits()
            )
            .unwrap_err(),
            ContinuityError::MissingFacts
        );
    }
    assert_eq!(checkpoint, before);
    assert_eq!(
        checkpoint.state().continuity.scenes[0].geometry,
        fixture.assets[3]
    );
    assert_eq!(checkpoint.state().resources, before.state().resources);
    assert_eq!(checkpoint.state().inventory, before.state().inventory);
    assert_eq!(checkpoint.state().decisions, before.state().decisions);
}

mod fixture {
    use df_model::checkpoint::*;
    use df_types::{
        BuildIdentity, MemberId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision,
    };
    pub fn label(value: &str) -> RevisionLabel {
        RevisionLabel::new(Some(value)).unwrap()
    }
    pub fn entity(value: u8) -> EntityId {
        EntityId::from_bytes(&[value; 16]).unwrap()
    }
    pub fn member(value: u8) -> MemberId {
        MemberId::from_bytes(&[value; 16]).unwrap()
    }
    pub fn revision(epoch: u64, sequence: u64) -> SessionRevision {
        SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), sequence)
    }
    pub fn basis() -> Basis {
        Basis {
            session: SessionId::from_bytes(&[1; 16]).unwrap(),
            run: RunId::from_bytes(&[2; 16]).unwrap(),
            revision: revision(2, 8),
        }
    }
    pub fn content() -> ContentReference {
        ContentReference {
            package: label("fixture-package-1"),
            entry: label("fixture-entry-1"),
        }
    }
    pub fn rule() -> RuleReference {
        RuleReference {
            catalog: label("fixture-catalog-1"),
            source: label("fixture-source-1"),
            entry: label("fixture-entry-1"),
            clause: label("fixture-clause-1"),
        }
    }
    pub fn pins() -> CheckpointPins {
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
    pub fn resource_constraints() -> Vec<ResourceConstraint> {
        vec![ResourceConstraint {
            owner: entity(4),
            resource: label("fixture-resource-1"),
            minimum: 0,
            maximum: 8,
            source: rule(),
        }]
    }
    pub fn limits() -> CheckpointLimits {
        CheckpointLimits {
            maximum_records: 100,
            maximum_text_bytes: 256,
            maximum_total_text_bytes: 1024,
            maximum_retained_bytes: 1024 * 1024,
        }
    }
    pub fn state() -> GameState {
        GameState {
            mode: ExecutionMode::Replay,
            logical_time: LogicalTime {
                ticks: 120,
                ticks_per_second: 10,
            },
            members: vec![MembershipLink {
                member: member(3),
                character: Some(entity(4)),
            }],
            entities: vec![WorldEntity {
                id: entity(4),
                definition: content(),
                location: None,
                position: Some(Position { x: 0, y: 0, z: 0 }),
                identity_revision: label("fixture-entity-1"),
            }],
            characters: vec![CharacterState {
                entity: entity(4),
                build: content(),
                owner: member(3),
                choices: vec![],
            }],
            resources: vec![ResourceState {
                owner: entity(4),
                resource: label("fixture-resource-1"),
                value: 4,
                minimum: 0,
                maximum: 8,
                source: rule(),
            }],
            inventory: vec![],
            facts: vec![],
            draws: vec![],
            decisions: vec![],
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
            activity: vec![],
            tempo: TempoState {
                policy: content(),
                presentation_ticks: 0,
                intensity: 0,
                inertia: 0,
                fatigue: vec![],
            },
            presentation: vec![],
            continuity: continuity(),
        }
    }
    pub fn continuity() -> ContinuityState {
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
}
