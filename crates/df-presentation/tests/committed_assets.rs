#![cfg(not(target_arch = "wasm32"))]

#[path = "../../df-session/tests/support/fixture_model.rs"]
pub mod fixture_model;

use df_assets::{
    AccessFailure, AssetManifest, AssetMetadataStore, AssetReadAuthority, AssetResolver,
    AssetStore, AuthorizedBinding, ByteRange, ChunkOutcome, DurableObject, MetadataFailure,
    NativeFileStore, Publication, PublicationStatus, PublishedBinding, RangeError, publish,
};
use df_model::checkpoint::*;
use df_observe::OperationContext;
use df_presentation::moment_selection::{
    MomentAlternative, MomentContext, MomentDisposition, MomentError, MomentLimits,
    PermittedMoment, select_committed_moment_plan,
};
use df_types::{OperationId, RevisionLabel};
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

const BYTES: &[u8] = b"owned complete presentation bytes";

fn limits() -> MomentLimits {
    MomentLimits {
        maximum_alternatives: 8,
        maximum_items: 1024,
        maximum_shots: 8,
        maximum_demands: 8,
        maximum_duration_ticks: 100,
        maximum_reference_bytes: 1024,
        maximum_demand_bytes: 1024,
    }
}

fn record(value: u8) -> RecordId {
    RecordId::from_bytes(&[value; 16]).unwrap()
}

fn reference() -> AssetReference {
    AssetReference {
        key: fixture_model::label("presentation-complete-v1"),
        digest: ContentDigest(Sha256::digest(BYTES).into()),
        byte_length: BYTES.len() as u64,
        kind: AssetKind::Image,
    }
}

struct CurrentConsumer {
    checkpoint: Checkpoint,
    entities: [EntityId; 1],
    content: [ContentReference; 1],
    assets: [AssetReference; 1],
}

impl CurrentConsumer {
    fn new() -> Self {
        Self::with_recovery(None)
    }

    fn with_recovery(failure: Option<u8>) -> Self {
        // Reuse the current accepted canonical fixture, not a copy of a historical GameState.
        let mut state = fixture_model::state();
        let pins = fixture_model::pins();
        let moment = NarrativeMoment {
            id: record(31),
            location: fixture_model::entity(4),
            characters: vec![fixture_model::entity(4)],
            facts: vec![],
            attributed_claims: vec![],
            audience: AudienceScope::Shared,
            semantic_focus: fixture_model::content(),
            identity_revision: fixture_model::label("fixture-entity-1"),
        };
        let presentation = PresentationDemand {
            id: record(32),
            definition: fixture_model::content(),
            audience: AudienceScope::Shared,
            causal_facts: vec![],
            source_revision: fixture_model::basis().revision,
        };
        let assets = [reference()];
        let shot = ShotPlan {
            id: record(33),
            moment: moment.id,
            subjects: moment.characters.clone(),
            audience: AudienceScope::Shared,
            duration_ticks: 20,
            definition: fixture_model::content(),
            references: assets.to_vec(),
            performance: PerformanceHint {
                definition: fixture_model::content(),
                voice: None,
                emphasis_facts: vec![],
            },
        };
        let demand = AssetDemand {
            id: record(34),
            basis: fixture_model::basis(),
            key: AssetRequestKey {
                schema: CHECKPOINT_SCHEMA,
                source: pins.content.content_digest,
                moment: moment.id,
                identity: moment.identity_revision.clone(),
                style: fixture_model::label("fixture-style-1"),
                voice: None,
                provider: fixture_model::label("fixture-no-dispatch"),
                model: fixture_model::label("fixture-model-1"),
                format: fixture_model::label("fixture-image-1"),
                references: assets.to_vec(),
                audience: AudienceScope::Shared,
                parameters: fixture_model::label("fixture-parameters-1"),
            },
            priority: DemandPriority::InteractionCritical,
            mode: state.mode,
            expires: LogicalTime {
                ticks: 140,
                ticks_per_second: 10,
            },
            budget_reservation: fixture_model::label("fixture-no-live-spend"),
            maximum_bytes: 128,
            policy: fixture_model::content(),
        };
        state.continuity.moments.push(moment);
        state.presentation.push(presentation);
        let mut reference_free = shot.clone();
        reference_free.id = record(35);
        reference_free.references.clear();
        state.continuity.shots.push(shot);
        state.continuity.shots.push(reference_free);
        state.continuity.demands.push(demand);
        match failure {
            Some(0) => state.continuity.recovery.redacted_records.push(record(99)),
            Some(_) => state
                .continuity
                .recovery
                .unavailable_sources
                .push(pins.content.package.clone()),
            None => {}
        }
        let entities = [fixture_model::entity(4)];
        let content = [fixture_model::content()];
        let rules = [fixture_model::rule()];
        let resources = fixture_model::resource_constraints();
        let mut checkpoint_limits = fixture_model::limits();
        checkpoint_limits.maximum_records = 512;
        checkpoint_limits.maximum_total_text_bytes = 16 * 1024;
        let checkpoint = Checkpoint::new(
            CHECKPOINT_SCHEMA,
            fixture_model::basis(),
            pins,
            state,
            ReferenceInventory {
                rules: &rules,
                content: &content,
                resources: &resources,
                assets: &assets,
            },
            checkpoint_limits,
        )
        .unwrap();
        Self {
            checkpoint,
            entities,
            content,
            assets,
        }
    }

    fn context(&self) -> MomentContext<'_> {
        MomentContext {
            basis: self.checkpoint.basis(),
            pins: self.checkpoint.pins(),
            mode: self.checkpoint.state().mode,
            moment: &self.checkpoint.state().continuity.moments[0],
            presentation: &self.checkpoint.state().presentation[0],
        }
    }

    fn permitted(&self) -> PermittedMoment<'_> {
        // This fixed test-only inventory is intentionally not an application RightsGrant.
        PermittedMoment {
            context: self.context(),
            facts: &[],
            attributed_claims: &[],
            entities: &self.entities,
            content: &self.content,
            assets: &self.assets,
        }
    }

    fn alternative(&self) -> MomentAlternative<'_> {
        MomentAlternative {
            context: self.context(),
            shots: &self.checkpoint.state().continuity.shots,
            demands: &self.checkpoint.state().continuity.demands,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Purpose {
    Presentation,
    Export,
}

// Same test-only current authority serves publication and the actual current resolver.
#[derive(Default)]
struct Authority {
    published: RefCell<Option<PublishedBinding<AssetReference>>>,
    revoked: Cell<bool>,
    generation: Cell<u64>,
    calls: Cell<usize>,
}

impl AssetMetadataStore for Authority {
    type Version = RevisionLabel;
    type Metadata = AssetReference;

    fn lookup(
        &self,
        version: &RevisionLabel,
    ) -> Result<Option<PublishedBinding<AssetReference>>, MetadataFailure> {
        if self.revoked.get() {
            return Ok(None);
        }
        Ok(self
            .published
            .borrow()
            .as_ref()
            .filter(|binding| &binding.metadata.key == version)
            .cloned())
    }

    fn publish_immutable(
        &self,
        publication: &Publication<RevisionLabel, AssetReference>,
        object: DurableObject,
    ) -> Result<PublicationStatus, MetadataFailure> {
        if self.revoked.get() {
            return Err(MetadataFailure::Unavailable);
        }
        let mut published = self.published.borrow_mut();
        if let Some(existing) = published.as_ref() {
            return Ok(
                if existing.metadata == publication.metadata
                    && existing.bytes == publication.bytes
                    && existing.object == object
                {
                    PublicationStatus::AlreadyPublished
                } else {
                    PublicationStatus::VersionConflict
                },
            );
        }
        *published = Some(PublishedBinding {
            metadata: publication.metadata.clone(),
            bytes: publication.bytes,
            object,
        });
        Ok(PublicationStatus::Published)
    }
}

impl AssetReadAuthority for Authority {
    type Caller = u8;
    type Purpose = Purpose;
    type Basis = u64;

    fn current_authorized_binding(
        &self,
        caller: &u8,
        version: &RevisionLabel,
        purpose: &Purpose,
        _range: ByteRange,
    ) -> Result<AuthorizedBinding<AssetReference, u64>, AccessFailure> {
        self.calls.set(self.calls.get() + 1);
        if self.revoked.get() || *caller != 1 || *purpose != Purpose::Presentation {
            return Err(AccessFailure::Denied);
        }
        let published = self
            .lookup(version)
            .map_err(|_| AccessFailure::Unavailable)?
            .ok_or(AccessFailure::Absent)?;
        Ok(AuthorizedBinding {
            published,
            basis: self.generation.get(),
            max_chunk_bytes: 4,
        })
    }
}

fn owned_root() -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = PathBuf::from(std::env::var_os("TMPDIR").expect("ROOT supplies owned TMPDIR")).join(
        format!(
            "presentation-current-assets-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ),
    );
    std::fs::create_dir(&root).unwrap();
    // Root retains and owns cleanup of fixture output; no protected evidence is deleted.
    root
}

fn context() -> OperationContext {
    OperationContext {
        trace_parent: String::new(),
        build: "presentation-current-consumer".to_owned(),
    }
}

fn published() -> (PathBuf, NativeFileStore, Authority) {
    let root = owned_root();
    let store = NativeFileStore::new(&root, 128).unwrap();
    let authority = Authority::default();
    let operation = OperationId::from_bytes(&[60; 16]).unwrap();
    let staged = store.stage(operation, &mut &BYTES[..]).unwrap();
    let publication = Publication {
        operation,
        version: reference().key.clone(),
        metadata: reference(),
        bytes: AssetManifest {
            byte_len: BYTES.len() as u64,
            sha256: Sha256::digest(BYTES).into(),
        },
    };
    assert_eq!(
        publish(&context(), &publication, &staged, &store, &authority)
            .unwrap()
            .status,
        PublicationStatus::Published
    );
    (root, store, authority)
}

#[test]
fn committed_plan_selects_exact_publication_and_reads_current_bounded_bytes_after_restart() {
    let consumer = CurrentConsumer::new();
    let before = consumer.checkpoint.clone();
    let alternatives = [consumer.alternative()];
    let selected = select_committed_moment_plan(
        &consumer.checkpoint,
        consumer.context(),
        consumer.checkpoint.state().logical_time,
        Some(consumer.permitted()),
        &alternatives,
        limits(),
    )
    .unwrap();
    assert_eq!(selected.disposition, MomentDisposition::Selected);
    let reference = &selected.selected.unwrap().shots[0].references[0];
    let (root, _, authority) = published();
    let restarted = NativeFileStore::new(root, 128).unwrap();
    let current = authority.generation.get();
    let resolver = AssetResolver::new(&authority, reference, &current);
    let context = context();
    let range = ByteRange::new(0, reference.byte_length).unwrap();
    assert!(matches!(
        resolver.open_native(&context, &restarted, &2, &Purpose::Presentation, range),
        Err(RangeError::Access(AccessFailure::Denied))
    ));
    assert!(matches!(
        resolver.open_native(&context, &restarted, &1, &Purpose::Export, range),
        Err(RangeError::Access(AccessFailure::Denied))
    ));
    let mut stream = resolver
        .open_native(&context, &restarted, &1, &Purpose::Presentation, range)
        .unwrap();
    let mut output = [0; 8];
    let mut bytes = Vec::new();
    while let ChunkOutcome::Bytes(length) = stream.read_chunk(&mut output).unwrap() {
        assert!((1..=4).contains(&length));
        bytes.extend_from_slice(&output[..length]);
    }
    assert_eq!(bytes, BYTES);
    assert_eq!(consumer.checkpoint, before);
}

#[test]
fn current_revocation_stops_selected_stream_without_modifying_output_or_state() {
    let consumer = CurrentConsumer::new();
    let before = consumer.checkpoint.clone();
    let alternatives = [consumer.alternative()];
    let selected = select_committed_moment_plan(
        &consumer.checkpoint,
        consumer.context(),
        consumer.checkpoint.state().logical_time,
        Some(consumer.permitted()),
        &alternatives,
        limits(),
    )
    .unwrap();
    let reference = &selected.selected.unwrap().shots[0].references[0];
    let (_, store, authority) = published();
    let access = authority.generation.get();
    let resolver = AssetResolver::new(&authority, reference, &access);
    let context = context();
    let mut stream = resolver
        .open_native(
            &context,
            &store,
            &1,
            &Purpose::Presentation,
            ByteRange::new(0, reference.byte_length).unwrap(),
        )
        .unwrap();
    let mut output = [0; 8];
    assert_eq!(
        stream.read_chunk(&mut output).unwrap(),
        ChunkOutcome::Bytes(4)
    );
    authority.revoked.set(true);
    output.fill(0xa5);
    assert!(matches!(
        stream.read_chunk(&mut output),
        Err(RangeError::Access(AccessFailure::Denied))
    ));
    assert_eq!(output, [0xa5; 8]);
    assert!(matches!(
        stream.read_chunk(&mut output),
        Err(RangeError::Terminated)
    ));
    let mut permitted = consumer.permitted();
    permitted.assets = &[];
    let refused = select_committed_moment_plan(
        &consumer.checkpoint,
        consumer.context(),
        consumer.checkpoint.state().logical_time,
        Some(permitted),
        &alternatives,
        limits(),
    )
    .unwrap();
    assert_eq!(refused.disposition, MomentDisposition::Unavailable);
    assert_eq!(consumer.checkpoint, before);
}

#[test]
fn access_generation_change_terminates_the_already_selected_stream() {
    let consumer = CurrentConsumer::new();
    let alternatives = [consumer.alternative()];
    let selected = select_committed_moment_plan(
        &consumer.checkpoint,
        consumer.context(),
        consumer.checkpoint.state().logical_time,
        Some(consumer.permitted()),
        &alternatives,
        limits(),
    )
    .unwrap();
    let reference = &selected.selected.unwrap().shots[0].references[0];
    let (_, store, authority) = published();
    let access = authority.generation.get();
    let resolver = AssetResolver::new(&authority, reference, &access);
    let context = context();
    let mut stream = resolver
        .open_native(
            &context,
            &store,
            &1,
            &Purpose::Presentation,
            ByteRange::new(0, reference.byte_length).unwrap(),
        )
        .unwrap();
    let mut output = [0; 8];
    assert_eq!(
        stream.read_chunk(&mut output).unwrap(),
        ChunkOutcome::Bytes(4)
    );
    authority.generation.set(access + 1);
    output.fill(0xb6);
    assert!(matches!(
        stream.read_chunk(&mut output),
        Err(RangeError::Access(AccessFailure::Stale))
    ));
    assert_eq!(output, [0xb6; 8]);
    assert!(matches!(
        stream.read_chunk(&mut output),
        Err(RangeError::Terminated)
    ));
}

#[test]
fn withdrawn_imagery_selects_separately_committed_reference_free_plan_without_generation() {
    let consumer = CurrentConsumer::new();
    let before = consumer.checkpoint.clone();
    let fallback = MomentAlternative {
        context: consumer.context(),
        shots: &consumer.checkpoint.state().continuity.shots[1..],
        demands: &[],
    };
    let alternatives = [consumer.alternative(), fallback];
    let mut permitted = consumer.permitted();
    permitted.assets = &[];
    let result = select_committed_moment_plan(
        &consumer.checkpoint,
        consumer.context(),
        consumer.checkpoint.state().logical_time,
        Some(permitted),
        &alternatives,
        limits(),
    )
    .unwrap();
    assert_eq!(result.disposition, MomentDisposition::Selected);
    assert_eq!(result.rejections.unpermitted, 1);
    let selected = result.selected.unwrap();
    assert_eq!(selected.shots[0].id, record(35));
    assert!(selected.shots[0].references.is_empty());
    assert!(selected.demands.is_empty());
    assert_eq!(consumer.checkpoint, before);
}

#[test]
fn tentative_context_or_shot_is_refused_and_preserves_checkpoint() {
    let consumer = CurrentConsumer::new();
    let before = consumer.checkpoint.clone();
    let alternatives = [consumer.alternative()];
    let mut context = consumer.context();
    context.basis.revision = context.basis.revision.next_sequence().unwrap();
    assert!(matches!(
        select_committed_moment_plan(
            &consumer.checkpoint,
            context,
            consumer.checkpoint.state().logical_time,
            Some(consumer.permitted()),
            &alternatives,
            limits(),
        ),
        Err(MomentError::Checkpoint(CheckpointError::StaleBasis))
    ));
    let mut changed = consumer.checkpoint.state().continuity.shots.clone();
    changed[0].duration_ticks += 1;
    let tentative = [MomentAlternative {
        context: consumer.context(),
        shots: &changed,
        demands: &consumer.checkpoint.state().continuity.demands,
    }];
    assert!(matches!(
        select_committed_moment_plan(
            &consumer.checkpoint,
            consumer.context(),
            consumer.checkpoint.state().logical_time,
            Some(consumer.permitted()),
            &tentative,
            limits(),
        ),
        Err(MomentError::InvalidCurrentContext)
    ));
    assert_eq!(consumer.checkpoint, before);
}

#[test]
fn canonical_redaction_and_unavailable_source_guards_refuse_even_with_permitted_inventory() {
    for failure in [0, 1] {
        let consumer = CurrentConsumer::with_recovery(Some(failure));
        let alternatives = [consumer.alternative()];
        let result = select_committed_moment_plan(
            &consumer.checkpoint,
            consumer.context(),
            consumer.checkpoint.state().logical_time,
            Some(consumer.permitted()),
            &alternatives,
            limits(),
        );
        let expected = if failure == 0 {
            CheckpointError::RedactedCheckpoint
        } else {
            CheckpointError::UnavailableCheckpoint
        };
        assert_eq!(result.err().unwrap(), MomentError::Checkpoint(expected));
    }
}

#[test]
fn missing_current_producer_and_expired_committed_demand_are_nonblocking_unavailable() {
    let consumer = CurrentConsumer::new();
    let alternatives = [consumer.alternative()];
    let missing = select_committed_moment_plan(
        &consumer.checkpoint,
        consumer.context(),
        consumer.checkpoint.state().logical_time,
        None,
        &alternatives,
        limits(),
    )
    .unwrap();
    assert_eq!(missing.disposition, MomentDisposition::Unavailable);
    assert!(missing.selected.is_none());
    let expired = select_committed_moment_plan(
        &consumer.checkpoint,
        consumer.context(),
        LogicalTime {
            ticks: 140,
            ticks_per_second: 10,
        },
        Some(consumer.permitted()),
        &alternatives,
        limits(),
    )
    .unwrap();
    assert_eq!(expired.disposition, MomentDisposition::Expired);
    assert!(expired.selected.is_none());
}
