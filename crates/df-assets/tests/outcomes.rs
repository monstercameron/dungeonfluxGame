#![cfg(not(target_arch = "wasm32"))]

use df_assets::{
    AccessFailure, AssetManifest, AssetMetadataStore, AssetReadAuthority, AssetResolutionError,
    AssetStore, AssetUnavailable, AuthorizedBinding, ByteRange, ChunkOutcome, DurableObject,
    MetadataFailure, NativeFileStore, Publication, PublicationStatus, PublishedBinding, RangeError,
    SelectedAsset, StaleAsset, StoreError, publish,
};
use df_model::checkpoint::{
    AssetKind, AssetReference, AssetRequestKey, AudienceScope, ContentDigest, RecordId,
};
use df_observe::OperationContext;
use df_types::{OperationId, RevisionLabel};
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static ROOT_NUMBER: AtomicUsize = AtomicUsize::new(0);

// Local access fixtures exercise the real consumer-owned authority port; these are not grants.
struct Caller(Cell<u8>);

#[derive(Clone, Eq, PartialEq)]
enum Purpose {
    Playback,
    Export,
}

struct Authority {
    binding: RefCell<Option<PublishedBinding<AssetReference>>>,
    basis: Cell<[u64; 3]>,
    rights_unavailable: Cell<bool>,
    allow_playback: Cell<bool>,
    calls: Cell<usize>,
    stale_on_call: Cell<Option<usize>>,
}

impl AssetMetadataStore for Authority {
    type Version = RevisionLabel;
    type Metadata = AssetReference;

    fn lookup(
        &self,
        version: &RevisionLabel,
    ) -> Result<Option<PublishedBinding<AssetReference>>, MetadataFailure> {
        Ok(self
            .binding
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
        let mut binding = self.binding.borrow_mut();
        if let Some(previous) = binding.as_ref() {
            return Ok(
                if previous.metadata == publication.metadata
                    && previous.bytes == publication.bytes
                    && previous.object == object
                {
                    PublicationStatus::AlreadyPublished
                } else {
                    PublicationStatus::VersionConflict
                },
            );
        }
        *binding = Some(PublishedBinding {
            metadata: publication.metadata.clone(),
            bytes: publication.bytes,
            object,
        });
        Ok(PublicationStatus::Published)
    }
}

impl AssetReadAuthority for Authority {
    type Caller = Caller;
    type Purpose = Purpose;
    type Basis = [u64; 3];

    fn current_authorized_binding(
        &self,
        caller: &Caller,
        version: &RevisionLabel,
        purpose: &Purpose,
        range: ByteRange,
    ) -> Result<AuthorizedBinding<AssetReference, [u64; 3]>, AccessFailure> {
        let call = self.calls.get() + 1;
        self.calls.set(call);
        if self.rights_unavailable.get() {
            return Err(AccessFailure::Unavailable);
        }
        if caller.0.get() != 7 || *purpose != Purpose::Playback || !self.allow_playback.get() {
            return Err(AccessFailure::Denied);
        }
        let published = self
            .lookup(version)
            .map_err(|_| AccessFailure::Unavailable)?
            .ok_or(AccessFailure::Absent)?;
        // The real boundary receives the exact interval on every authorization check.
        assert_eq!(range, ByteRange::new(0, 8).unwrap());
        if self.stale_on_call.get() == Some(call) {
            self.basis.set([2, 1, 1]);
        }
        Ok(AuthorizedBinding {
            published,
            basis: self.basis.get(),
            max_chunk_bytes: 4,
        })
    }
}

fn label(value: &str) -> RevisionLabel {
    RevisionLabel::new(Some(value)).unwrap()
}

fn context() -> OperationContext {
    OperationContext {
        trace_parent: String::new(),
        build: "assets-i04-connected-fixture".to_owned(),
    }
}

fn request() -> AssetRequestKey {
    AssetRequestKey {
        schema: 1,
        source: ContentDigest([1; 32]),
        moment: RecordId::from_bytes(&[2; 16]).unwrap(),
        identity: label("identity-1"),
        style: label("style-1"),
        voice: None,
        provider: label("prepared"),
        model: label("prepared-1"),
        format: label("audio-1"),
        references: Vec::new(),
        audience: AudienceScope::Shared,
        parameters: label("parameters-1"),
    }
}

struct Fixture {
    root: PathBuf,
    store: NativeFileStore,
    authority: Authority,
    reference: AssetReference,
    request: AssetRequestKey,
    captured_basis: [u64; 3],
}

impl Fixture {
    fn new() -> Self {
        let temporary =
            PathBuf::from(std::env::var_os("TMPDIR").expect("registered owned TMPDIR required"));
        let root = temporary.join(format!(
            "assets-i04-connected-{}",
            ROOT_NUMBER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let store = NativeFileStore::new(&root, 16).unwrap();
        let authority = Authority {
            binding: RefCell::new(None),
            basis: Cell::new([1, 1, 1]),
            rights_unavailable: Cell::new(false),
            allow_playback: Cell::new(true),
            calls: Cell::new(0),
            stale_on_call: Cell::new(None),
        };
        let reference = AssetReference {
            key: label("asset-1"),
            digest: ContentDigest(Sha256::digest(b"abcdefgh").into()),
            byte_length: 8,
            kind: AssetKind::Audio,
        };
        let operation = OperationId::from_bytes(&[3; 16]).unwrap();
        let staged = store.stage(operation, &mut &b"abcdefgh"[..]).unwrap();
        let publication = Publication {
            operation,
            version: reference.key.clone(),
            metadata: reference.clone(),
            bytes: AssetManifest {
                byte_len: reference.byte_length,
                sha256: reference.digest.0,
            },
        };
        publish(&context(), &publication, &staged, &store, &authority).unwrap();
        Self {
            root,
            store,
            authority,
            reference,
            request: request(),
            captured_basis: [1, 1, 1],
        }
    }

    fn selected(&self) -> SelectedAsset<'_, Authority> {
        SelectedAsset::new(
            &self.request,
            &self.request,
            &self.reference,
            AssetKind::Audio,
            &self.authority,
            &self.captured_basis,
        )
        .unwrap()
    }

    fn backing(&self) -> PathBuf {
        let name = self
            .reference
            .digest
            .0
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        self.root.join("objects").join(name)
    }
}

fn deliver(
    fixture: &Fixture,
    caller: &Caller,
    purpose: &Purpose,
    output: &mut [u8],
) -> Result<ChunkOutcome, AssetResolutionError> {
    let selected = fixture.selected();
    let context = context();
    let mut stream = selected.open_native(
        &context,
        &fixture.store,
        caller,
        purpose,
        ByteRange::new(0, 8).unwrap(),
        &fixture.request,
    )?;
    stream.read_chunk(&fixture.request, output)
}

#[test]
fn canonical_selection_delivers_native_published_bytes_in_current_bounded_chunks() {
    let fixture = Fixture::new();
    let selected = fixture.selected();
    let context = context();
    let caller = Caller(Cell::new(7));
    let mut stream = selected
        .open_native(
            &context,
            &fixture.store,
            &caller,
            &Purpose::Playback,
            ByteRange::new(0, 8).unwrap(),
            &fixture.request,
        )
        .unwrap();
    let mut output = [0xee; 6];
    assert_eq!(
        stream.read_chunk(&fixture.request, &mut output).unwrap(),
        ChunkOutcome::Bytes(4)
    );
    assert_eq!(output, [b'a', b'b', b'c', b'd', 0xee, 0xee]);
    assert_eq!(
        stream.read_chunk(&fixture.request, &mut output).unwrap(),
        ChunkOutcome::Bytes(4)
    );
    assert_eq!(output, [b'e', b'f', b'g', b'h', 0xee, 0xee]);
    assert_eq!(
        stream.read_chunk(&fixture.request, &mut output).unwrap(),
        ChunkOutcome::Complete
    );
}

#[test]
fn missing_publication_or_backing_never_fabricates_playable_media_or_changes_output() {
    for missing_publication in [true, false] {
        let fixture = Fixture::new();
        if missing_publication {
            fixture.authority.binding.borrow_mut().take();
        } else {
            fs::write(fixture.root.join("disposable-cache"), b"abcdefgh").unwrap();
            fs::remove_file(fixture.backing()).unwrap();
        }
        let expected = if missing_publication {
            AssetUnavailable::NotPublished
        } else {
            AssetUnavailable::BackingMissing
        };
        let mut output = [0xee; 4];
        assert!(
            matches!(deliver(&fixture, &Caller(Cell::new(7)), &Purpose::Playback, &mut output),
            Err(AssetResolutionError::Unavailable(actual)) if actual == expected)
        );
        assert_eq!(output, [0xee; 4]);
    }
}

#[test]
fn wrong_caller_or_purpose_refuses_before_missing_backing_and_keeps_output_private() {
    let fixture = Fixture::new();
    fs::remove_file(fixture.backing()).unwrap();
    for (caller, purpose) in [
        (Caller(Cell::new(8)), Purpose::Playback),
        (Caller(Cell::new(7)), Purpose::Export),
    ] {
        let mut output = [0xee; 4];
        assert!(matches!(
            deliver(&fixture, &caller, &purpose, &mut output),
            Err(AssetResolutionError::Denied)
        ));
        assert_eq!(output, [0xee; 4]);
    }
}

#[test]
fn corrupt_truncated_or_extra_native_backing_remains_integrity_failure_with_no_output() {
    for bytes in [&b"abcdefgi"[..], &b"abcdefg"[..], &b"abcdefghi"[..]] {
        let fixture = Fixture::new();
        // Published objects are read-only; replace the fixture path to simulate corrupt backing.
        fs::remove_file(fixture.backing()).unwrap();
        fs::write(fixture.backing(), bytes).unwrap();
        let mut output = [0xee; 4];
        assert!(matches!(
            deliver(
                &fixture,
                &Caller(Cell::new(7)),
                &Purpose::Playback,
                &mut output
            ),
            Err(AssetResolutionError::Store(StoreError::BackingIntegrity))
        ));
        assert_eq!(output, [0xee; 4]);
    }
}

#[test]
fn stale_authority_token_after_a_read_refuses_next_chunk_and_cannot_be_resumed() {
    for changed in [[2, 1, 1], [1, 2, 1], [1, 1, 2]] {
        let fixture = Fixture::new();
        let selected = fixture.selected();
        let context = context();
        let caller = Caller(Cell::new(7));
        let mut stream = selected
            .open_native(
                &context,
                &fixture.store,
                &caller,
                &Purpose::Playback,
                ByteRange::new(0, 8).unwrap(),
                &fixture.request,
            )
            .unwrap();
        let mut output = [0xee; 4];
        assert_eq!(
            stream.read_chunk(&fixture.request, &mut output).unwrap(),
            ChunkOutcome::Bytes(4)
        );
        fixture.authority.basis.set(changed);
        output.fill(0xee);
        assert!(matches!(
            stream.read_chunk(&fixture.request, &mut output),
            Err(AssetResolutionError::Stale(StaleAsset::AccessBasis))
        ));
        assert_eq!(output, [0xee; 4]);
        fixture.authority.basis.set([1, 1, 1]);
        assert!(matches!(
            stream.read_chunk(&fixture.request, &mut output),
            Err(AssetResolutionError::Range(RangeError::Terminated))
        ));
        assert_eq!(output, [0xee; 4]);
    }
}

#[test]
fn rights_outage_after_a_read_is_unavailable_and_never_a_successful_tail() {
    let fixture = Fixture::new();
    let selected = fixture.selected();
    let context = context();
    let caller = Caller(Cell::new(7));
    let mut stream = selected
        .open_native(
            &context,
            &fixture.store,
            &caller,
            &Purpose::Playback,
            ByteRange::new(0, 8).unwrap(),
            &fixture.request,
        )
        .unwrap();
    let mut output = [0xee; 4];
    assert_eq!(
        stream.read_chunk(&fixture.request, &mut output).unwrap(),
        ChunkOutcome::Bytes(4)
    );
    fixture.authority.rights_unavailable.set(true);
    output.fill(0xee);
    assert!(matches!(
        stream.read_chunk(&fixture.request, &mut output),
        Err(AssetResolutionError::Unavailable(
            AssetUnavailable::CurrentAuthority
        ))
    ));
    assert_eq!(output, [0xee; 4]);
    fixture.authority.rights_unavailable.set(false);
    assert!(matches!(
        stream.read_chunk(&fixture.request, &mut output),
        Err(AssetResolutionError::Range(RangeError::Terminated))
    ));
    assert_eq!(output, [0xee; 4]);
}

#[test]
fn stale_basis_on_final_check_discards_native_bytes_already_read_into_private_scratch() {
    let fixture = Fixture::new();
    // Calls 1/2 open; call 3 checks before read; call 4 is the post-read disclosure fence.
    fixture.authority.stale_on_call.set(Some(4));
    let mut output = [0xee; 4];
    assert!(matches!(
        deliver(
            &fixture,
            &Caller(Cell::new(7)),
            &Purpose::Playback,
            &mut output
        ),
        Err(AssetResolutionError::Stale(StaleAsset::AccessBasis))
    ));
    assert_eq!(output, [0xee; 4]);
    assert_eq!(fixture.authority.calls.get(), 4);
}

#[test]
fn source_identity_and_full_request_drift_after_read_refuse_without_access_or_output_changes() {
    for dimension in 0..3 {
        let fixture = Fixture::new();
        let selected = fixture.selected();
        let context = context();
        let caller = Caller(Cell::new(7));
        let mut stream = selected
            .open_native(
                &context,
                &fixture.store,
                &caller,
                &Purpose::Playback,
                ByteRange::new(0, 8).unwrap(),
                &fixture.request,
            )
            .unwrap();
        let mut output = [0xee; 4];
        assert_eq!(
            stream.read_chunk(&fixture.request, &mut output).unwrap(),
            ChunkOutcome::Bytes(4)
        );
        let mut current = fixture.request.clone();
        let expected = match dimension {
            0 => {
                current.source = ContentDigest([9; 32]);
                StaleAsset::Source
            }
            1 => {
                current.identity = label("identity-2");
                StaleAsset::Identity
            }
            _ => {
                current.audience = AudienceScope::Host;
                StaleAsset::Request
            }
        };
        let prior_calls = fixture.authority.calls.get();
        output.fill(0xee);
        assert!(matches!(stream.read_chunk(&current, &mut output),
            Err(AssetResolutionError::Stale(actual)) if actual == expected));
        assert_eq!(output, [0xee; 4]);
        assert_eq!(fixture.authority.calls.get(), prior_calls);
        assert!(matches!(
            stream.read_chunk(&fixture.request, &mut output),
            Err(AssetResolutionError::Range(RangeError::Terminated))
        ));
        assert_eq!(output, [0xee; 4]);
    }
}

#[test]
fn mismatched_kind_refuses_selection_before_authority_or_backing() {
    let fixture = Fixture::new();
    for kind in [
        AssetKind::Image,
        AssetKind::Video,
        AssetKind::TacticalGeometry,
    ] {
        assert!(matches!(
            SelectedAsset::new(
                &fixture.request,
                &fixture.request,
                &fixture.reference,
                kind,
                &fixture.authority,
                &fixture.captured_basis
            ),
            Err(AssetResolutionError::Stale(StaleAsset::Kind))
        ));
    }
    assert_eq!(fixture.authority.calls.get(), 0);
}

#[test]
fn canonical_reference_and_malformed_manifest_refuse_through_i03_and_i02() {
    for corrupt_manifest in [false, true] {
        let fixture = Fixture::new();
        {
            let mut binding = fixture.authority.binding.borrow_mut();
            let binding = binding.as_mut().unwrap();
            if corrupt_manifest {
                binding.bytes.byte_len = 7;
            } else {
                binding.metadata.kind = AssetKind::Image;
            }
        }
        let mut output = [0xee; 4];
        let error = deliver(
            &fixture,
            &Caller(Cell::new(7)),
            &Purpose::Playback,
            &mut output,
        );
        if corrupt_manifest {
            assert!(matches!(
                error,
                Err(AssetResolutionError::Unavailable(
                    AssetUnavailable::CurrentAuthority
                ))
            ));
        } else {
            assert!(matches!(
                error,
                Err(AssetResolutionError::Stale(StaleAsset::AccessBasis))
            ));
        }
        assert_eq!(output, [0xee; 4]);
    }
}

#[test]
fn every_request_dimension_remains_part_of_source_selection() {
    let fixture = Fixture::new();
    let mut variants = Vec::new();
    let mut current = fixture.request.clone();
    current.schema = 2;
    variants.push(current);
    let mut current = fixture.request.clone();
    current.moment = RecordId::from_bytes(&[7; 16]).unwrap();
    variants.push(current);
    let mut current = fixture.request.clone();
    current.style = label("style-2");
    variants.push(current);
    let mut current = fixture.request.clone();
    current.voice = Some(label("voice-2"));
    variants.push(current);
    let mut current = fixture.request.clone();
    current.provider = label("provider-2");
    variants.push(current);
    let mut current = fixture.request.clone();
    current.model = label("model-2");
    variants.push(current);
    let mut current = fixture.request.clone();
    current.format = label("format-2");
    variants.push(current);
    let mut current = fixture.request.clone();
    current.references.push(fixture.reference.clone());
    variants.push(current);
    let mut current = fixture.request.clone();
    current.parameters = label("parameters-2");
    variants.push(current);
    for current in variants {
        assert!(matches!(
            SelectedAsset::new(
                &fixture.request,
                &current,
                &fixture.reference,
                AssetKind::Audio,
                &fixture.authority,
                &fixture.captured_basis
            ),
            Err(AssetResolutionError::Stale(StaleAsset::Request))
        ));
    }
    assert_eq!(fixture.authority.calls.get(), 0);
}

#[test]
fn wrong_current_caller_or_revoked_purpose_after_read_never_delivers_the_next_chunk() {
    for wrong_caller in [false, true] {
        let fixture = Fixture::new();
        let selected = fixture.selected();
        let context = context();
        let caller = Caller(Cell::new(7));
        let mut stream = selected
            .open_native(
                &context,
                &fixture.store,
                &caller,
                &Purpose::Playback,
                ByteRange::new(0, 8).unwrap(),
                &fixture.request,
            )
            .unwrap();
        let mut output = [0xee; 4];
        assert_eq!(
            stream.read_chunk(&fixture.request, &mut output).unwrap(),
            ChunkOutcome::Bytes(4)
        );
        if wrong_caller {
            caller.0.set(8);
        } else {
            fixture.authority.allow_playback.set(false);
        }
        output.fill(0xee);
        assert!(matches!(
            stream.read_chunk(&fixture.request, &mut output),
            Err(AssetResolutionError::Denied)
        ));
        assert_eq!(output, [0xee; 4]);
    }
}
