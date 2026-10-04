//! Supplied source-handler and publication-owner fixtures exercise the real canonical boundary.
//! They do not qualify 2024 creation mechanics, provider bytes/format, spend or native commits.
use super::fixture_model as fixture;
use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_engine::character_reference::*;
use df_engine::command_entry::{CommandEntryContext, CommandEntryLimits, CommandRejection};
use df_model::checkpoint::*;
use df_model::commands::{CommandError, CommandLimits};
use df_rules::{DispatchRegistry, HandlerRegistration, RulesCommandHandler, RulesCommandInput};
use df_types::{OperationId, RevisionLabel};
use std::cell::Cell;

struct AcceptedCreation {
    candidate: Checkpoint,
    calls: Cell<usize>,
    rejected: bool,
}
impl RulesCommandHandler for AcceptedCreation {
    type Rejection = ();
    fn pins(&self) -> &CheckpointPins {
        self.candidate.pins()
    }
    fn stage(&self, _: RulesCommandInput<'_>, _: &Checkpoint) -> Result<Checkpoint, ()> {
        self.calls.set(self.calls.get() + 1);
        if self.rejected {
            Err(())
        } else {
            Ok(self.candidate.clone())
        }
    }
}
fn label(value: &str) -> RevisionLabel {
    fixture::label(value)
}
fn record(value: u8) -> RecordId {
    RecordId::from_bytes(&[value; 16]).unwrap()
}
fn job_id() -> JobId {
    JobId::from_bytes(&[60; 16]).unwrap()
}
fn asset() -> AssetReference {
    AssetReference {
        key: label("published-sheet-v1"),
        digest: ContentDigest([81; 32]),
        byte_length: 128,
        kind: AssetKind::Image,
    }
}
fn bounds() -> ReferenceSheetLimits {
    ReferenceSheetLimits {
        checkpoint: fixture::limits(),
        maximum_owned_bytes: 8 * 1024 * 1024,
    }
}
fn draft() -> CharacterDraft {
    CharacterDraft {
        entity: fixture::entity(4),
        member: fixture::member(3),
        ancestry: Some(fixture::content()),
        background: None,
        classes: vec![fixture::content()],
        choices: vec![],
        phase: CreationPhase::Selecting,
    }
}
fn initial_pack() -> CanonicalPack {
    CanonicalPack {
        revision: label("appearance-pack-v1"),
        digest: ContentDigest([71; 32]),
        bible: VisualBible {
            revision: label("lantern-style-v1"),
            definition: fixture::content(),
            palette: vec![label("warm-amber")],
            style: "Painterly amber lantern light".into(),
            references: vec![],
        },
        identities: vec![EntityIdentityRevision {
            entity: fixture::entity(4),
            revision: label("fixture-entity-1"),
            character_appearance: Some(CharacterAppearance {
                features: "Round face, dark curls, broad shoulders".into(),
                outfit: "Green cloak and brass clasps".into(),
                outfit_revision: label("cloak-v1"),
            }),
            source_facts: vec![],
            appearances: vec![],
            voice: None,
            sound: vec![],
        }],
    }
}
fn make(
    basis: Basis,
    state: GameState,
    assets: &[AssetReference],
) -> Result<Checkpoint, CheckpointError> {
    let rules = [fixture::rule()];
    let contents = [fixture::content()];
    let resources = fixture::resource_constraints();
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        basis,
        fixture::pins(),
        state,
        ReferenceInventory {
            rules: &rules,
            content: &contents,
            resources: &resources,
            assets,
        },
        fixture::limits(),
    )
}
fn current() -> Checkpoint {
    let mut state = fixture::state();
    state.characters.clear();
    state.continuity.creation.push(draft());
    state.continuity.canonical_packs.push(initial_pack());
    make(fixture::basis(), state, &[]).unwrap()
}
fn input() -> GameInput {
    GameInput::Game(CommandInput {
        basis: fixture::basis(),
        observed_revision: fixture::basis().revision,
        operation: fixture::operation(),
        member: fixture::member(3),
        command: GameCommand::SubmitCharacterDraft { draft: draft() },
    })
}
fn accepted(current: &Checkpoint) -> Checkpoint {
    let mut state = current.state().clone();
    state.continuity.creation.first_mut().unwrap().phase = CreationPhase::Accepted;
    state.characters.push(CharacterState {
        entity: fixture::entity(4),
        build: fixture::content(),
        owner: fixture::member(3),
        choices: vec![],
    });
    fixture::accepted_from(state)
}
fn plan(accepted: &Checkpoint) -> CharacterReferencePlan {
    let moment = record(50);
    CharacterReferencePlan {
        moment: NarrativeMoment {
            id: moment,
            location: fixture::entity(4),
            characters: vec![fixture::entity(4)],
            facts: vec![],
            attributed_claims: vec![],
            audience: AudienceScope::Members(vec![fixture::member(3)]),
            semantic_focus: fixture::content(),
            identity_revision: label("fixture-entity-1"),
        },
        demand: AssetDemand {
            id: record(51),
            basis: accepted.basis(),
            key: AssetRequestKey {
                schema: CHECKPOINT_SCHEMA,
                source: accepted.pins().content.content_digest,
                moment,
                identity: label("fixture-entity-1"),
                style: label("lantern-style-v1"),
                voice: None,
                provider: label("fixture-prepared-provider"),
                model: label("fixture-four-view-model-v1"),
                format: label("png-v1"),
                references: vec![],
                audience: AudienceScope::Members(vec![fixture::member(3)]),
                parameters: label("cloak-v1"),
            },
            priority: DemandPriority::Optional,
            mode: accepted.state().mode,
            expires: LogicalTime {
                ticks: 240,
                ticks_per_second: 10,
            },
            budget_reservation: label("prepared-no-new-spend"),
            maximum_bytes: 1024,
            policy: fixture::content(),
        },
        intent: DurableIntent {
            id: EffectId::from_bytes(&[52; 16]).unwrap(),
            basis: accepted.basis(),
            operation: fixture::operation(),
            slot: 0,
            kind: EffectKind::RunMedia,
            job: Some(job_id()),
            timer: None,
            generation: 2,
            status: DurableStatus::Pending,
            definition: fixture::content(),
        },
    }
}
fn registered(
    current: &Checkpoint,
    input: &GameInput,
    handler: &AcceptedCreation,
    plan: &CharacterReferencePlan,
    limits: ReferenceSheetLimits,
) -> Result<CharacterReferenceStaging, CommandRejection<()>> {
    let pins = fixture::pins();
    let rules = [fixture::rule()];
    let contents = [fixture::content()];
    let resources = fixture::resource_constraints();
    let selector = label("fixture-supplied-creation-selector");
    let entries = [CatalogEntry::new(
        &rules[0],
        b"supplied synthetic creation boundary",
    )];
    let catalog = CatalogSnapshot::from_published(
        &pins.rules.catalog,
        &pins,
        b"complete synthetic source",
        &entries,
        CatalogLimits {
            max_complete_bytes: 128,
            max_entries: 4,
            max_item_bytes: 128,
            max_total_item_bytes: 512,
        },
    )
    .unwrap();
    let registrations = [HandlerRegistration::new(&selector, &rules[0], handler)];
    let registry = DispatchRegistry::from_catalog(catalog, &registrations, 1).unwrap();
    decide_registered_character_reference(
        RulesCommandInput {
            command: input,
            supplied_draws: &[],
        },
        current,
        CharacterReferenceContext {
            command: CommandEntryContext {
                current_basis: current.basis(),
                admitted_pins: &pins,
                inventory: ReferenceInventory {
                    rules: &rules,
                    content: &contents,
                    resources: &resources,
                    assets: &[],
                },
                limits: CommandEntryLimits {
                    command: CommandLimits {
                        maximum_records: 16,
                        maximum_text_bytes: 256,
                        maximum_retained_bytes: 16 * 1024,
                    },
                    maximum_staged_bytes: 1024 * 1024,
                },
            },
            reference_limits: limits,
        },
        &registry,
        &selector,
        &rules[0],
        plan,
    )
}
fn queue() -> (Checkpoint, CharacterReferencePlan) {
    let current = current();
    let accepted = accepted(&current);
    let plan = plan(&accepted);
    let handler = AcceptedCreation {
        candidate: accepted,
        calls: Cell::new(0),
        rejected: false,
    };
    let output = registered(&current, &input(), &handler, &plan, bounds()).unwrap();
    assert_eq!(output.reference, ReferenceSheetAdmission::Queued);
    assert_eq!(handler.calls.get(), 1);
    (output.checkpoint, plan)
}
fn event(
    current: &Checkpoint,
    event: ReferenceSheetEvent<'_>,
    assets: &[AssetReference],
) -> Result<Checkpoint, ReferenceSheetError> {
    let rules = [fixture::rule()];
    let contents = [fixture::content()];
    let resources = fixture::resource_constraints();
    stage_character_reference_event(
        current,
        current.basis(),
        current.pins(),
        event,
        ReferenceInventory {
            rules: &rules,
            content: &contents,
            resources: &resources,
            assets,
        },
        bounds(),
    )
}
fn started(current: &Checkpoint, plan: &CharacterReferencePlan) -> Checkpoint {
    event(
        current,
        ReferenceSheetEvent::Started {
            basis: plan.intent.basis,
            operation: plan.intent.operation,
            job: job_id(),
            generation: 2,
        },
        &[],
    )
    .unwrap()
}
fn completed(plan: &CharacterReferencePlan, outcome: JobOutcome) -> JobCompletion {
    JobCompletion {
        basis: plan.intent.basis,
        operation: plan.intent.operation,
        job: job_id(),
        generation: 2,
        outcome,
    }
}
fn published_pack() -> CanonicalPack {
    let mut pack = initial_pack();
    pack.revision = label("published-pack-v2");
    pack.digest = ContentDigest([72; 32]);
    pack.identities
        .first_mut()
        .unwrap()
        .appearances
        .push(asset());
    pack
}
fn ready() -> (
    Checkpoint,
    CharacterReferencePlan,
    JobCompletion,
    CanonicalPack,
) {
    let (queued, plan) = queue();
    let running = started(&queued, &plan);
    let completion = completed(
        &plan,
        JobOutcome::Media {
            asset: asset(),
            demand: plan.demand.id,
        },
    );
    let pack = published_pack();
    let ready = event(
        &running,
        ReferenceSheetEvent::Completed {
            completion: &completion,
            canonical_pack: Some(&pack),
        },
        &[asset()],
    )
    .unwrap();
    (ready, plan, completion, pack)
}

#[test]
fn accepted_creation_automatically_stages_one_reusable_sheet_and_preserves_mechanics() {
    let (queued, plan) = queue();
    assert_eq!(
        REFERENCE_SHEET_VIEWS,
        [
            ReferenceSheetView::FrontFullBody,
            ReferenceSheetView::SideFullBody,
            ReferenceSheetView::BackFullBody,
            ReferenceSheetView::FaceCloseUp
        ]
    );
    let expected = accepted(&current());
    assert_eq!(queued.state().characters, expected.state().characters);
    assert_eq!(queued.state().resources, expected.state().resources);
    assert_eq!(queued.state().inventory, expected.state().inventory);
    assert_eq!(queued.state().facts, expected.state().facts);
    assert_eq!(queued.state().draws, expected.state().draws);
    assert_eq!(queued.state().logical_time, expected.state().logical_time);
    assert_eq!(
        queued.state().continuity.creation,
        expected.state().continuity.creation
    );
    assert_eq!(queued.state().continuity.demands, vec![plan.demand]);
    assert_eq!(
        queued.state().continuity.asset_jobs.first().unwrap().state,
        AssetLifecycle::Queued
    );
    assert_eq!(
        queued.state().decisions.first().unwrap().effects,
        vec![plan.intent.id]
    );
    assert_eq!(queued.state().intents, vec![plan.intent]);
}
#[test]
fn original_creation_retry_never_emits_another_identity_or_resets_job_state() {
    let current = current();
    let (queued, plan) = queue();
    let rules = [fixture::rule()];
    let contents = [fixture::content()];
    let resources = fixture::resource_constraints();
    let retry = stage_accepted_character_reference(
        &current,
        queued.clone(),
        &input(),
        &plan,
        ReferenceInventory {
            rules: &rules,
            content: &contents,
            resources: &resources,
            assets: &[],
        },
        bounds(),
    );
    assert_eq!(retry.reference, ReferenceSheetAdmission::AlreadyQueued);
    assert_eq!(retry.checkpoint, queued);
    let mut changed = plan;
    changed.intent.id = EffectId::from_bytes(&[99; 16]).unwrap();
    let retry = stage_accepted_character_reference(
        &current,
        queued.clone(),
        &input(),
        &changed,
        ReferenceInventory {
            rules: &rules,
            content: &contents,
            resources: &resources,
            assets: &[],
        },
        bounds(),
    );
    assert_eq!(
        retry.reference,
        ReferenceSheetAdmission::Unavailable(ReferenceSheetError::PlanBinding)
    );
    assert_eq!(retry.checkpoint, queued);
}
#[test]
fn rejected_command_or_source_handler_never_stages_media() {
    let current = current();
    let accepted = accepted(&current);
    let plan = plan(&accepted);
    let handler = AcceptedCreation {
        candidate: accepted,
        calls: Cell::new(0),
        rejected: false,
    };
    let mut invalid = input();
    let GameInput::Game(command) = &mut invalid else {
        panic!("fixture");
    };
    command.member = fixture::member(99);
    assert!(matches!(
        registered(&current, &invalid, &handler, &plan, bounds()),
        Err(CommandRejection::Structural(CommandError::UnknownMember))
    ));
    assert_eq!(handler.calls.get(), 0);
    let rejected = AcceptedCreation {
        candidate: handler.candidate.clone(),
        calls: Cell::new(0),
        rejected: true,
    };
    assert!(registered(&current, &input(), &rejected, &plan, bounds()).is_err());
    assert!(current.state().intents.is_empty());
}
#[test]
fn unrelated_command_does_not_trigger_reference_generation() {
    let current = current();
    let accepted = accepted(&current);
    let plan = plan(&accepted);
    let handler = AcceptedCreation {
        candidate: accepted.clone(),
        calls: Cell::new(0),
        rejected: false,
    };
    let result = registered(&current, &fixture::action(), &handler, &plan, bounds()).unwrap();
    assert_eq!(
        result.reference,
        ReferenceSheetAdmission::Unavailable(ReferenceSheetError::NotCharacterCreation)
    );
    assert_eq!(result.checkpoint, accepted);
}
#[test]
fn mismatched_accepted_owner_or_choices_cannot_trigger_a_reference() {
    let current = current();
    let accepted = accepted(&current);
    let plan = plan(&accepted);
    for mismatch in 0..4 {
        let mut state = accepted.state().clone();
        match mismatch {
            0 => {
                state.continuity.creation.first_mut().unwrap().phase =
                    CreationPhase::AwaitingValidation
            }
            1 => state
                .continuity
                .creation
                .first_mut()
                .unwrap()
                .classes
                .clear(),
            2 => {
                state.members.push(MembershipLink {
                    member: fixture::member(99),
                    character: Some(fixture::entity(4)),
                });
                state.characters.first_mut().unwrap().owner = fixture::member(99);
            }
            _ => state
                .characters
                .first_mut()
                .unwrap()
                .choices
                .push(AcceptedChoice {
                    offer: label("cosmetic-mismatch-offer"),
                    selected: label("cosmetic-mismatch-option"),
                    participant: fixture::member(3),
                    source: fixture::rule(),
                }),
        }
        let supplied = make(accepted.basis(), state, &[]).unwrap();
        let handler = AcceptedCreation {
            candidate: supplied.clone(),
            calls: Cell::new(0),
            rejected: false,
        };
        let result = registered(&current, &input(), &handler, &plan, bounds()).unwrap();
        assert_eq!(
            result.reference,
            ReferenceSheetAdmission::Unavailable(ReferenceSheetError::InvalidAcceptedCreation)
        );
        assert_eq!(result.checkpoint, supplied);
    }
}
#[test]
fn cosmetic_absence_private_audience_and_capacity_are_optional_refusals() {
    let current = current();
    let accepted = accepted(&current);
    let mut missing = accepted.state().clone();
    missing
        .continuity
        .canonical_packs
        .first_mut()
        .unwrap()
        .identities
        .first_mut()
        .unwrap()
        .character_appearance = None;
    let missing = make(accepted.basis(), missing, &[]).unwrap();
    let handler = AcceptedCreation {
        candidate: missing.clone(),
        calls: Cell::new(0),
        rejected: false,
    };
    let result = registered(&current, &input(), &handler, &plan(&missing), bounds()).unwrap();
    assert_eq!(
        result.reference,
        ReferenceSheetAdmission::Unavailable(ReferenceSheetError::AppearanceUnavailable)
    );
    assert_eq!(result.checkpoint, missing);
    let handler = AcceptedCreation {
        candidate: accepted.clone(),
        calls: Cell::new(0),
        rejected: false,
    };
    let mut public = plan(&accepted);
    public.moment.audience = AudienceScope::Shared;
    public.demand.key.audience = AudienceScope::Shared;
    let result = registered(&current, &input(), &handler, &public, bounds()).unwrap();
    assert_eq!(
        result.reference,
        ReferenceSheetAdmission::Unavailable(ReferenceSheetError::PlanBinding)
    );
    assert_eq!(result.checkpoint, accepted);
    let mut limits = bounds();
    limits.maximum_owned_bytes = 1;
    let result = registered(&current, &input(), &handler, &plan(&accepted), limits).unwrap();
    assert_eq!(
        result.reference,
        ReferenceSheetAdmission::Unavailable(ReferenceSheetError::Capacity)
    );
    assert_eq!(result.checkpoint, accepted);
}
#[test]
fn cosmetic_descriptions_are_nonempty_bounded_and_accounted_in_checkpoint_heap() {
    let valid = current();
    for field in [true, false] {
        let mut state = valid.state().clone();
        let appearance = state
            .continuity
            .canonical_packs
            .first_mut()
            .unwrap()
            .identities
            .first_mut()
            .unwrap()
            .character_appearance
            .as_mut()
            .unwrap();
        if field {
            appearance.features = "  ".into();
        } else {
            appearance.outfit = "\n".into();
        }
        assert_eq!(
            make(valid.basis(), state, &[]),
            Err(CheckpointError::InvalidReference)
        );
    }
    let mut state = valid.state().clone();
    state
        .continuity
        .canonical_packs
        .first_mut()
        .unwrap()
        .identities
        .first_mut()
        .unwrap()
        .character_appearance
        .as_mut()
        .unwrap()
        .features = "a".repeat(257);
    assert_eq!(
        make(valid.basis(), state, &[]),
        Err(CheckpointError::Capacity)
    );
    let mut state = valid.state().clone();
    let baseline = make(valid.basis(), state.clone(), &[]).unwrap();
    state
        .continuity
        .canonical_packs
        .first_mut()
        .unwrap()
        .identities
        .first_mut()
        .unwrap()
        .character_appearance
        .as_mut()
        .unwrap()
        .features
        .reserve_exact(4096);
    let enlarged = make(valid.basis(), state, &[]).unwrap();
    assert!(enlarged.retained_bytes().unwrap() > baseline.retained_bytes().unwrap() + 3000);
}
#[test]
fn started_and_ready_are_fenced_idempotent_and_preserve_prior_canonical_packs() {
    let (queued, plan) = queue();
    let started = started(&queued, &plan);
    let again = event(
        &started,
        ReferenceSheetEvent::Started {
            basis: plan.intent.basis,
            operation: plan.intent.operation,
            job: job_id(),
            generation: 2,
        },
        &[],
    )
    .unwrap();
    assert_eq!(again, started);
    let (ready, _, completion, pack) = ready();
    assert_eq!(
        ready.state().continuity.asset_jobs.first().unwrap().state,
        AssetLifecycle::Ready
    );
    assert_eq!(ready.state().continuity.canonical_packs.len(), 2);
    assert_eq!(
        ready.state().continuity.canonical_packs.first().unwrap(),
        &initial_pack()
    );
    let duplicate = event(
        &ready,
        ReferenceSheetEvent::Completed {
            completion: &completion,
            canonical_pack: Some(&pack),
        },
        &[asset()],
    )
    .unwrap();
    assert_eq!(duplicate, ready);
    assert_eq!(
        ready.state().continuity.creation.first().unwrap().phase,
        CreationPhase::Accepted
    );
}
#[test]
fn obsolete_job_generation_operation_basis_and_identity_cannot_replace_current_state() {
    let (queued, plan) = queue();
    let original = queued.clone();
    for mutation in 0..4 {
        let mut completion = completed(
            &plan,
            JobOutcome::Media {
                asset: asset(),
                demand: plan.demand.id,
            },
        );
        match mutation {
            0 => completion.generation += 1,
            1 => completion.operation = OperationId::from_bytes(&[99; 16]).unwrap(),
            2 => completion.basis.revision = fixture::revision(3, 9),
            _ => completion.job = JobId::from_bytes(&[99; 16]).unwrap(),
        }
        assert!(
            event(
                &queued,
                ReferenceSheetEvent::Completed {
                    completion: &completion,
                    canonical_pack: Some(&published_pack())
                },
                &[asset()]
            )
            .is_err()
        );
        assert_eq!(queued, original);
    }
    let mut state = queued.state().clone();
    state.entities.first_mut().unwrap().identity_revision = label("new-face-v2");
    let changed = make(queued.basis(), state, &[]).unwrap();
    let completion = completed(
        &plan,
        JobOutcome::Media {
            asset: asset(),
            demand: plan.demand.id,
        },
    );
    assert_eq!(
        event(
            &changed,
            ReferenceSheetEvent::Completed {
                completion: &completion,
                canonical_pack: Some(&published_pack())
            },
            &[asset()]
        ),
        Err(ReferenceSheetError::IdentityChanged)
    );
}
#[test]
fn failure_cancel_and_stale_keep_accepted_character_and_never_repeat_unknown_dispatch() {
    for failure in [
        NativeFailure::Unavailable,
        NativeFailure::Cancelled,
        NativeFailure::Stale,
        NativeFailure::Deadline,
    ] {
        let (queued, plan) = queue();
        let running = started(&queued, &plan);
        let completion = completed(&plan, JobOutcome::Failed(failure));
        let failed = event(
            &running,
            ReferenceSheetEvent::Completed {
                completion: &completion,
                canonical_pack: None,
            },
            &[],
        )
        .unwrap();
        let expected = match failure {
            NativeFailure::Cancelled => AssetLifecycle::Cancelled,
            NativeFailure::Stale => AssetLifecycle::Stale,
            _ => AssetLifecycle::Failed,
        };
        assert_eq!(
            failed.state().continuity.asset_jobs.first().unwrap().state,
            expected
        );
        assert_eq!(
            failed
                .state()
                .continuity
                .asset_jobs
                .first()
                .unwrap()
                .dispatch,
            DurableStatus::SentUnknown
        );
        assert_eq!(failed.state().characters, running.state().characters);
        assert_eq!(failed.state().resources, running.state().resources);
        assert_eq!(
            failed.state().continuity.creation,
            running.state().continuity.creation
        );
        assert_eq!(
            failed.state().continuity.canonical_packs,
            running.state().continuity.canonical_packs
        );
        assert_eq!(
            event(
                &failed,
                ReferenceSheetEvent::Completed {
                    completion: &completion,
                    canonical_pack: None
                },
                &[]
            )
            .unwrap(),
            failed
        );
        assert_eq!(
            event(
                &failed,
                ReferenceSheetEvent::Started {
                    basis: plan.intent.basis,
                    operation: plan.intent.operation,
                    job: job_id(),
                    generation: 2
                },
                &[]
            ),
            Err(ReferenceSheetError::InvalidTransition)
        );
    }
}
#[test]
fn unqualified_oversized_wrong_kind_and_changed_cosmetic_outputs_are_refused() {
    let (queued, plan) = queue();
    let complete = completed(
        &plan,
        JobOutcome::Media {
            asset: asset(),
            demand: plan.demand.id,
        },
    );
    assert_eq!(
        event(
            &queued,
            ReferenceSheetEvent::Completed {
                completion: &complete,
                canonical_pack: Some(&published_pack())
            },
            &[]
        ),
        Err(ReferenceSheetError::OutputUnavailable)
    );
    for kind in [true, false] {
        let mut invalid = asset();
        if kind {
            invalid.kind = AssetKind::Audio;
        } else {
            invalid.byte_length = 1025;
        }
        let complete = completed(
            &plan,
            JobOutcome::Media {
                asset: invalid.clone(),
                demand: plan.demand.id,
            },
        );
        assert_eq!(
            event(
                &queued,
                ReferenceSheetEvent::Completed {
                    completion: &complete,
                    canonical_pack: Some(&published_pack())
                },
                &[invalid]
            ),
            Err(ReferenceSheetError::OutputUnavailable)
        );
    }
    let mut changed = published_pack();
    changed
        .identities
        .first_mut()
        .unwrap()
        .character_appearance
        .as_mut()
        .unwrap()
        .outfit = "New silver armor".into();
    assert_eq!(
        event(
            &queued,
            ReferenceSheetEvent::Completed {
                completion: &complete,
                canonical_pack: Some(&changed)
            },
            &[asset()]
        ),
        Err(ReferenceSheetError::OutputUnavailable)
    );
}
fn scene(current: &Checkpoint) -> (Checkpoint, AssetRequestKey) {
    let mut state = current.state().clone();
    state.continuity.moments.push(NarrativeMoment {
        id: record(77),
        location: fixture::entity(4),
        characters: vec![fixture::entity(4)],
        facts: vec![],
        attributed_claims: vec![],
        audience: AudienceScope::Members(vec![fixture::member(3)]),
        semantic_focus: fixture::content(),
        identity_revision: label("scene-snowy-ridge-v1"),
    });
    let current = make(current.basis(), state, &[asset()]).unwrap();
    let key = AssetRequestKey {
        schema: CHECKPOINT_SCHEMA,
        source: current.pins().content.content_digest,
        moment: record(77),
        identity: label("scene-snowy-ridge-v1"),
        style: label("lantern-style-v1"),
        voice: None,
        provider: label("fixture-scene-provider"),
        model: label("fixture-scene-model-v2"),
        format: label("png-v1"),
        references: vec![],
        audience: AudienceScope::Members(vec![fixture::member(3)]),
        parameters: label("snow-and-lanterns-v3"),
    };
    (current, key)
}
fn scene_key(
    current: &Checkpoint,
    key: &AssetRequestKey,
) -> Result<AssetRequestKey, ReferenceSheetError> {
    character_reference_for_scene(
        current,
        fixture::entity(4),
        &label("fixture-entity-1"),
        &label("cloak-v1"),
        &label("lantern-style-v1"),
        key,
        bounds(),
    )
}
#[test]
fn later_arbitrary_environment_request_reuses_exact_ready_sheet_once_with_original_scene_identity()
{
    let (ready, _, _, _) = ready();
    let (current, original) = scene(&ready);
    let request = scene_key(&current, &original).unwrap();
    assert_eq!(request.references, vec![asset()]);
    assert_eq!(request.identity, original.identity);
    assert_eq!(request.parameters, original.parameters);
    assert_eq!(request.model, original.model);
    assert_eq!(request.moment, original.moment);
    assert_eq!(scene_key(&current, &request).unwrap(), request);
    assert!(original.references.is_empty());
}
#[test]
fn scene_reference_reuse_refuses_missing_failed_participants_or_wider_audience() {
    let (ready, _, _, _) = ready();
    let (current, mut key) = scene(&ready);
    key.moment = record(99);
    assert_eq!(
        scene_key(&current, &key),
        Err(ReferenceSheetError::PlanBinding)
    );
    let (current, key) = scene(&ready);
    let mut public = current.state().clone();
    public.continuity.moments.last_mut().unwrap().audience = AudienceScope::Shared;
    let public = make(current.basis(), public, &[asset()]).unwrap();
    let mut widened = key.clone();
    widened.audience = AudienceScope::Shared;
    assert_eq!(
        scene_key(&public, &widened),
        Err(ReferenceSheetError::AudienceUnavailable)
    );
    let mut empty = current.state().clone();
    empty
        .continuity
        .moments
        .last_mut()
        .unwrap()
        .characters
        .clear();
    let empty = make(current.basis(), empty, &[asset()]).unwrap();
    assert_eq!(
        scene_key(&empty, &key),
        Err(ReferenceSheetError::PlanBinding)
    );
    let (queued, _) = queue();
    let (not_ready, key) = scene(&queued);
    assert_eq!(
        scene_key(&not_ready, &key),
        Err(ReferenceSheetError::OutputUnavailable)
    );
}

struct LocalAdapterFields<'a> {
    features: &'a str,
    outfit: &'a str,
    outfit_revision: &'a RevisionLabel,
    art_direction: &'a str,
    views: &'static [ReferenceSheetView; 4],
    references: &'a [AssetReference],
}
impl<'a> LocalAdapterFields<'a> {
    fn from_specification(specification: &'a ReferenceSheetSpecification<'a>) -> Self {
        Self {
            features: &specification.appearance.features,
            outfit: &specification.appearance.outfit,
            outfit_revision: &specification.appearance.outfit_revision,
            art_direction: &specification.visual_bible.style,
            views: specification.views,
            references: &specification.demand.key.references,
        }
    }
}
#[test]
fn actual_queued_dispatch_specification_carries_cosmetics_four_views_and_art_direction() {
    let (queued, plan) = queue();
    let specification = reference_sheet_specification(&queued, job_id(), bounds()).unwrap();
    let adapter_input = LocalAdapterFields::from_specification(&specification);
    assert_eq!(
        adapter_input.features,
        "Round face, dark curls, broad shoulders"
    );
    assert_eq!(adapter_input.outfit, "Green cloak and brass clasps");
    assert_eq!(adapter_input.outfit_revision, &plan.demand.key.parameters);
    assert_eq!(adapter_input.art_direction, "Painterly amber lantern light");
    assert_eq!(adapter_input.views, &REFERENCE_SHEET_VIEWS);
    assert_eq!(
        adapter_input.references,
        plan.demand.key.references.as_slice()
    );
    assert_eq!(specification.demand, &plan.demand);
    let running = started(&queued, &plan);
    assert!(reference_sheet_specification(&running, job_id(), bounds()).is_ok());
    let (ready, _, _, _) = ready();
    assert!(matches!(
        reference_sheet_specification(&ready, job_id(), bounds()),
        Err(ReferenceSheetError::InvalidTransition)
    ));
}
#[test]
fn failed_expired_or_replaced_identity_cannot_resolve_dispatch_or_reuse_ready_sheet() {
    let (queued, plan) = queue();
    let completion = completed(&plan, JobOutcome::Failed(NativeFailure::Unavailable));
    let failed = event(
        &queued,
        ReferenceSheetEvent::Completed {
            completion: &completion,
            canonical_pack: None,
        },
        &[],
    )
    .unwrap();
    assert!(reference_sheet_specification(&failed, job_id(), bounds()).is_err());
    let mut expired = queued.state().clone();
    expired.logical_time.ticks = plan.demand.expires.ticks;
    let expired = make(queued.basis(), expired, &[]).unwrap();
    assert!(matches!(
        reference_sheet_specification(&expired, job_id(), bounds()),
        Err(ReferenceSheetError::InvalidTransition)
    ));
    let completed = completed(
        &plan,
        JobOutcome::Media {
            asset: asset(),
            demand: plan.demand.id,
        },
    );
    assert_eq!(
        event(
            &expired,
            ReferenceSheetEvent::Completed {
                completion: &completed,
                canonical_pack: Some(&published_pack())
            },
            &[asset()]
        ),
        Err(ReferenceSheetError::OutputUnavailable)
    );
    let (ready, _, _, _) = ready();
    let (current, key) = scene(&ready);
    let mut changed = current.state().clone();
    changed.entities.first_mut().unwrap().identity_revision = label("replacement-face-v2");
    let changed = make(current.basis(), changed, &[asset()]).unwrap();
    assert_eq!(
        scene_key(&changed, &key),
        Err(ReferenceSheetError::IdentityChanged)
    );
}
#[test]
fn mismatched_scene_cache_identity_source_style_and_member_do_not_receive_sheet() {
    let (ready, _, _, _) = ready();
    let (current, original) = scene(&ready);
    for mutation in 0..3 {
        let mut key = original.clone();
        match mutation {
            0 => key.identity = label("unrelated-scene-v1"),
            1 => key.source = ContentDigest([99; 32]),
            _ => key.style = label("unrelated-style-v2"),
        }
        assert_eq!(
            scene_key(&current, &key),
            Err(ReferenceSheetError::PlanBinding)
        );
    }
    assert!(
        character_reference_for_scene(
            &current,
            fixture::entity(99),
            &label("fixture-entity-1"),
            &label("cloak-v1"),
            &label("lantern-style-v1"),
            &original,
            bounds()
        )
        .is_err()
    );
}
#[test]
fn started_generation_replacement_and_ready_failure_cannot_mutate_newer_sheet() {
    let (queued, plan) = queue();
    let mut replacement = queued.state().clone();
    replacement
        .continuity
        .asset_jobs
        .first_mut()
        .unwrap()
        .generation = 3;
    let replacement = make(queued.basis(), replacement, &[]).unwrap();
    let completion = completed(
        &plan,
        JobOutcome::Media {
            asset: asset(),
            demand: plan.demand.id,
        },
    );
    assert_eq!(
        event(
            &replacement,
            ReferenceSheetEvent::Completed {
                completion: &completion,
                canonical_pack: Some(&published_pack())
            },
            &[asset()]
        ),
        Err(ReferenceSheetError::StaleCompletion)
    );
    let (ready, plan, _, _) = ready();
    let old_failure = completed(&plan, JobOutcome::Failed(NativeFailure::Cancelled));
    assert_eq!(
        event(
            &ready,
            ReferenceSheetEvent::Completed {
                completion: &old_failure,
                canonical_pack: None
            },
            &[asset()]
        ),
        Err(ReferenceSheetError::InvalidTransition)
    );
    assert_eq!(
        ready
            .state()
            .continuity
            .asset_jobs
            .first()
            .unwrap()
            .published
            .as_ref(),
        Some(&asset())
    );
}
