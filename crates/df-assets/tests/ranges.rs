#![cfg(not(target_arch = "wasm32"))]

use df_assets::{
    AccessFailure, AssetManifest, AssetMetadataStore, AssetReadAuthority, AssetReadStore,
    AssetStore, AuthorizedBinding, ByteRange, ChunkOutcome, DurableObject, MAX_CHUNK_BYTES,
    MetadataFailure, NativeFileStore, Publication, PublicationStatus, PublishedBinding, RangeError,
    StoreError, open_range, publish,
};
use df_observe::OperationContext;
use df_types::OperationId;
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::fs;
use std::io::{self, Cursor, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);

// Fixture-only identity. Production supplies df-auth's verified caller boundary.
struct Caller(u8);

#[derive(Clone, Eq, PartialEq)]
enum Purpose {
    Playback,
    Export,
}

struct Authority {
    binding: RefCell<Option<PublishedBinding<&'static str>>>,
    basis: Cell<[u64; 3]>,
    cap: Cell<usize>,
    denied: Cell<bool>,
    unavailable: Cell<bool>,
    calls: Cell<usize>,
    deny_on_call: Cell<Option<usize>>,
    permitted_range: Cell<Option<ByteRange>>,
}

impl Authority {
    fn new() -> Self {
        Self {
            binding: RefCell::new(None),
            basis: Cell::new([1, 1, 1]),
            cap: Cell::new(4),
            denied: Cell::new(false),
            unavailable: Cell::new(false),
            calls: Cell::new(0),
            deny_on_call: Cell::new(None),
            permitted_range: Cell::new(None),
        }
    }
}

impl AssetMetadataStore for Authority {
    type Version = u8;
    type Metadata = &'static str;

    fn lookup(
        &self,
        version: &u8,
    ) -> Result<Option<PublishedBinding<Self::Metadata>>, MetadataFailure> {
        Ok(if *version == 1 {
            self.binding.borrow().clone()
        } else {
            None
        })
    }

    fn publish_immutable(
        &self,
        publication: &Publication<u8, Self::Metadata>,
        object: DurableObject,
    ) -> Result<PublicationStatus, MetadataFailure> {
        *self.binding.borrow_mut() = Some(PublishedBinding {
            metadata: publication.metadata,
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
        version: &u8,
        purpose: &Purpose,
        range: ByteRange,
    ) -> Result<AuthorizedBinding<Self::Metadata, Self::Basis>, AccessFailure> {
        let call = self.calls.get() + 1;
        self.calls.set(call);
        if self.unavailable.get() {
            return Err(AccessFailure::Unavailable);
        }
        if caller.0 != 7
            || *purpose != Purpose::Playback
            || self.denied.get()
            || self.deny_on_call.get() == Some(call)
            || self
                .permitted_range
                .get()
                .is_some_and(|permitted| permitted != range)
        {
            return Err(AccessFailure::Denied);
        }
        let published = self
            .lookup(version)
            .map_err(|_| AccessFailure::Unavailable)?
            .ok_or(AccessFailure::Absent)?;
        // The adapter consumes the exact requested interval, rather than inferring it from a hash.
        assert!(range.start() < range.end());
        Ok(AuthorizedBinding {
            published,
            basis: self.basis.get(),
            max_chunk_bytes: self.cap.get(),
        })
    }
}

fn context() -> OperationContext {
    OperationContext {
        trace_parent: String::new(),
        build: "asset-range-test".to_owned(),
    }
}

struct Fixture {
    store: NativeFileStore,
    authority: Authority,
    root: PathBuf,
}

impl Fixture {
    fn new(content: &[u8]) -> Self {
        let temporary = PathBuf::from(std::env::var_os("TMPDIR").expect("owned TMPDIR required"));
        let root = temporary.join(format!(
            "df-assets-range-{}-{}",
            std::process::id(),
            NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let store = NativeFileStore::new(&root, 1024 * 1024).unwrap();
        let authority = Authority::new();
        let operation = OperationId::from_bytes(&[1; 16]).unwrap();
        let publication = Publication {
            operation,
            version: 1,
            metadata: "audio-v1",
            bytes: AssetManifest {
                byte_len: content.len() as u64,
                sha256: Sha256::digest(content).into(),
            },
        };
        let staged = store.stage(operation, &mut &content[..]).unwrap();
        publish(&context(), &publication, &staged, &store, &authority).unwrap();
        Self {
            store,
            authority,
            root,
        }
    }

    fn object_path(&self) -> PathBuf {
        let digest = self
            .authority
            .binding
            .borrow()
            .as_ref()
            .unwrap()
            .bytes
            .sha256;
        let name: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        self.root.join("objects").join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn actual_published_native_bytes_obey_interval_and_current_chunk_cap() {
    let fixture = Fixture::new(b"0123456789abcdef");
    let context = context();
    let caller = Caller(7);
    let mut stream = open_range(
        &context,
        &fixture.store,
        &fixture.authority,
        &caller,
        &1,
        &Purpose::Playback,
        ByteRange::new(3, 12).unwrap(),
    )
    .unwrap();
    let mut output = [0xee; 32];
    assert_eq!(
        stream.read_chunk(&mut output).unwrap(),
        ChunkOutcome::Bytes(4)
    );
    assert_eq!(&output[..4], b"3456");
    assert_eq!(output[4], 0xee);
    fixture.authority.cap.set(2);
    assert_eq!(
        stream.read_chunk(&mut output).unwrap(),
        ChunkOutcome::Bytes(2)
    );
    assert_eq!(&output[..2], b"78");
    assert_eq!(
        stream.read_chunk(&mut output).unwrap(),
        ChunkOutcome::Bytes(2)
    );
    assert_eq!(&output[..2], b"9a");
    assert_eq!(
        stream.read_chunk(&mut output).unwrap(),
        ChunkOutcome::Bytes(1)
    );
    assert_eq!(output[0], b'b');
    assert_eq!(
        stream.read_chunk(&mut output).unwrap(),
        ChunkOutcome::Complete
    );
}

#[test]
fn empty_reversed_overflowing_and_metadata_out_of_bounds_ranges_are_refused() {
    for (start, end) in [(0, 0), (3, 2), (u64::MAX, u64::MAX)] {
        assert!(matches!(
            ByteRange::new(start, end),
            Err(RangeError::InvalidRange)
        ));
    }
    assert!(matches!(
        ByteRange::from_start_len(u64::MAX - 1, 3),
        Err(RangeError::InvalidRange)
    ));
    let fixture = Fixture::new(b"abc");
    // Missing backing would produce another error if invalid ranges reached byte I/O.
    fs::remove_file(fixture.object_path()).unwrap();
    assert!(matches!(
        open_range(
            &context(),
            &fixture.store,
            &fixture.authority,
            &Caller(7),
            &1,
            &Purpose::Playback,
            ByteRange::new(0, 4).unwrap(),
        ),
        Err(RangeError::InvalidRange)
    ));
}

#[test]
fn caller_purpose_missing_version_and_authority_outage_fail_closed_before_bytes() {
    let fixture = Fixture::new(b"abc");
    fs::remove_file(fixture.object_path()).unwrap();
    for (caller, purpose, version, failure) in [
        (Caller(8), Purpose::Playback, 1, AccessFailure::Denied),
        (Caller(7), Purpose::Export, 1, AccessFailure::Denied),
        (Caller(7), Purpose::Playback, 2, AccessFailure::Absent),
    ] {
        assert!(matches!(
            open_range(
                &context(), &fixture.store, &fixture.authority, &caller, &version,
                &purpose, ByteRange::new(0, 2).unwrap(),
            ),
            Err(RangeError::Access(actual)) if actual == failure
        ));
    }
    fixture.authority.unavailable.set(true);
    assert!(matches!(
        open_range(
            &context(),
            &fixture.store,
            &fixture.authority,
            &Caller(7),
            &1,
            &Purpose::Playback,
            ByteRange::new(0, 2).unwrap(),
        ),
        Err(RangeError::Access(AccessFailure::Unavailable))
    ));
}

#[test]
fn authorization_change_during_open_verification_refuses_the_stream() {
    let fixture = Fixture::new(b"abc");
    fixture.authority.deny_on_call.set(Some(2));
    assert!(matches!(
        open_range(
            &context(),
            &fixture.store,
            &fixture.authority,
            &Caller(7),
            &1,
            &Purpose::Playback,
            ByteRange::new(0, 2).unwrap(),
        ),
        Err(RangeError::Access(AccessFailure::Denied))
    ));
}

#[test]
fn access_rights_and_suppression_basis_changes_fence_next_chunk_terminally() {
    for changed in [[2, 1, 1], [1, 2, 1], [1, 1, 2]] {
        let fixture = Fixture::new(b"abcdefgh");
        let context = context();
        let caller = Caller(7);
        let mut stream = open_range(
            &context,
            &fixture.store,
            &fixture.authority,
            &caller,
            &1,
            &Purpose::Playback,
            ByteRange::new(0, 8).unwrap(),
        )
        .unwrap();
        let mut output = [0xee; 4];
        assert_eq!(
            stream.read_chunk(&mut output).unwrap(),
            ChunkOutcome::Bytes(4)
        );
        fixture.authority.basis.set(changed);
        output.fill(0xee);
        assert!(matches!(
            stream.read_chunk(&mut output),
            Err(RangeError::StaleBinding)
        ));
        assert_eq!(output, [0xee; 4]);
        fixture.authority.basis.set([1, 1, 1]);
        assert!(matches!(
            stream.read_chunk(&mut output),
            Err(RangeError::Terminated)
        ));
    }
}

#[test]
fn changed_manifest_and_metadata_cannot_rebind_an_open_stream() {
    for change_metadata in [true, false] {
        let fixture = Fixture::new(b"abcdef");
        let context = context();
        let caller = Caller(7);
        let mut stream = open_range(
            &context,
            &fixture.store,
            &fixture.authority,
            &caller,
            &1,
            &Purpose::Playback,
            ByteRange::new(0, 6).unwrap(),
        )
        .unwrap();
        {
            let mut binding = fixture.authority.binding.borrow_mut();
            let binding = binding.as_mut().unwrap();
            if change_metadata {
                binding.metadata = "different-audience";
            } else {
                binding.bytes = AssetManifest {
                    byte_len: 6,
                    sha256: [9; 32],
                };
                binding.object = DurableObject::from_manifest(binding.bytes);
            }
        }
        let mut output = [0xee; 4];
        assert!(matches!(
            stream.read_chunk(&mut output),
            Err(RangeError::StaleBinding)
        ));
        assert_eq!(output, [0xee; 4]);
    }
}

struct ControlledStore<'a> {
    bytes: Vec<u8>,
    denied_during_read: Option<&'a Cell<bool>>,
    cap_during_read: Option<(&'a Cell<usize>, usize)>,
    wrong_seek: bool,
}

struct ControlledReader<'a> {
    bytes: Cursor<Vec<u8>>,
    denied_during_read: Option<&'a Cell<bool>>,
    cap_during_read: Option<(&'a Cell<usize>, usize)>,
    wrong_seek: bool,
}

impl Read for ControlledReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let read = self.bytes.read(output)?;
        if let Some(denied) = self.denied_during_read {
            denied.set(true);
        }
        if let Some((cap, limit)) = self.cap_during_read {
            cap.set(limit);
        }
        Ok(read)
    }
}

impl Seek for ControlledReader<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let actual = self.bytes.seek(position)?;
        Ok(if self.wrong_seek { actual + 1 } else { actual })
    }
}

impl<'a> AssetReadStore for ControlledStore<'a> {
    type Reader = ControlledReader<'a>;

    fn open_verified(
        &self,
        _: DurableObject,
        _: AssetManifest,
    ) -> Result<Self::Reader, StoreError> {
        // Deliberate post-open storage fault fixture, never a production integrity adapter.
        Ok(ControlledReader {
            bytes: Cursor::new(self.bytes.clone()),
            denied_during_read: self.denied_during_read,
            cap_during_read: self.cap_during_read,
            wrong_seek: self.wrong_seek,
        })
    }
}

#[test]
fn revocation_during_read_discards_even_the_final_private_chunk() {
    let fixture = Fixture::new(b"abcd");
    let store = ControlledStore {
        bytes: b"abcd".to_vec(),
        denied_during_read: Some(&fixture.authority.denied),
        cap_during_read: None,
        wrong_seek: false,
    };
    let context = context();
    let caller = Caller(7);
    let mut stream = open_range(
        &context,
        &store,
        &fixture.authority,
        &caller,
        &1,
        &Purpose::Playback,
        ByteRange::new(0, 4).unwrap(),
    )
    .unwrap();
    let mut output = [0xee; 4];
    assert!(matches!(
        stream.read_chunk(&mut output),
        Err(RangeError::Access(AccessFailure::Denied))
    ));
    assert_eq!(output, [0xee; 4]);
    assert!(matches!(
        stream.read_chunk(&mut output),
        Err(RangeError::Terminated)
    ));
}

#[test]
fn early_eof_keeps_partial_scratch_private_and_never_reports_completion() {
    let fixture = Fixture::new(b"abcdefgh");
    let store = ControlledStore {
        bytes: b"abcdef".to_vec(),
        denied_during_read: None,
        cap_during_read: None,
        wrong_seek: false,
    };
    let context = context();
    let caller = Caller(7);
    let mut stream = open_range(
        &context,
        &store,
        &fixture.authority,
        &caller,
        &1,
        &Purpose::Playback,
        ByteRange::new(0, 8).unwrap(),
    )
    .unwrap();
    let mut output = [0xee; 4];
    assert_eq!(
        stream.read_chunk(&mut output).unwrap(),
        ChunkOutcome::Bytes(4)
    );
    output.fill(0xee);
    assert!(matches!(
        stream.read_chunk(&mut output),
        Err(RangeError::Incomplete)
    ));
    assert_eq!(output, [0xee; 4]);
    assert!(matches!(
        stream.read_chunk(&mut output),
        Err(RangeError::Terminated)
    ));
}

#[test]
fn service_cap_applies_to_large_caller_buffers_and_small_buffers_remain_bounded() {
    let content = vec![3; MAX_CHUNK_BYTES + 8];
    let fixture = Fixture::new(&content);
    fixture.authority.cap.set(usize::MAX);
    let context = context();
    let caller = Caller(7);
    let mut stream = open_range(
        &context,
        &fixture.store,
        &fixture.authority,
        &caller,
        &1,
        &Purpose::Playback,
        ByteRange::new(0, content.len() as u64).unwrap(),
    )
    .unwrap();
    let mut large = vec![0xee; MAX_CHUNK_BYTES * 2];
    assert_eq!(
        stream.read_chunk(&mut large).unwrap(),
        ChunkOutcome::Bytes(MAX_CHUNK_BYTES)
    );
    assert_eq!(large[MAX_CHUNK_BYTES], 0xee);
    let mut small = [0xee; 3];
    assert_eq!(
        stream.read_chunk(&mut small).unwrap(),
        ChunkOutcome::Bytes(3)
    );
    assert_eq!(small, [3; 3]);
}

#[test]
fn empty_output_and_zero_authority_cap_are_explicit_failures() {
    let fixture = Fixture::new(b"abcd");
    fixture.authority.cap.set(0);
    assert!(matches!(
        open_range(
            &context(),
            &fixture.store,
            &fixture.authority,
            &Caller(7),
            &1,
            &Purpose::Playback,
            ByteRange::new(0, 4).unwrap(),
        ),
        Err(RangeError::InvalidChunkLimit)
    ));
    fixture.authority.cap.set(4);
    let context = context();
    let caller = Caller(7);
    let mut stream = open_range(
        &context,
        &fixture.store,
        &fixture.authority,
        &caller,
        &1,
        &Purpose::Playback,
        ByteRange::new(0, 4).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        stream.read_chunk(&mut []),
        Err(RangeError::EmptyOutput)
    ));
    assert!(matches!(
        stream.read_chunk(&mut [0; 4]),
        Err(RangeError::Terminated)
    ));
}

#[test]
fn missing_corrupt_or_mismatched_native_objects_never_open_successfully() {
    let fixture = Fixture::new(b"abcd");
    let path = fixture.object_path();
    fs::remove_file(&path).unwrap();
    assert!(matches!(
        open_range(
            &context(),
            &fixture.store,
            &fixture.authority,
            &Caller(7),
            &1,
            &Purpose::Playback,
            ByteRange::new(0, 4).unwrap(),
        ),
        Err(RangeError::Store(StoreError::BackingMissing))
    ));
    fs::write(&path, b"abce").unwrap();
    assert!(matches!(
        open_range(
            &context(),
            &fixture.store,
            &fixture.authority,
            &Caller(7),
            &1,
            &Purpose::Playback,
            ByteRange::new(0, 4).unwrap(),
        ),
        Err(RangeError::Store(StoreError::BackingIntegrity))
    ));
    fs::write(&path, b"abc").unwrap();
    assert!(matches!(
        open_range(
            &context(),
            &fixture.store,
            &fixture.authority,
            &Caller(7),
            &1,
            &Purpose::Playback,
            ByteRange::new(0, 4).unwrap(),
        ),
        Err(RangeError::Store(StoreError::BackingIntegrity))
    ));
    fixture
        .authority
        .binding
        .borrow_mut()
        .as_mut()
        .unwrap()
        .object = DurableObject::from_manifest(AssetManifest {
        byte_len: 4,
        sha256: [9; 32],
    });
    assert!(matches!(
        open_range(
            &context(),
            &fixture.store,
            &fixture.authority,
            &Caller(7),
            &1,
            &Purpose::Playback,
            ByteRange::new(0, 4).unwrap(),
        ),
        Err(RangeError::Store(StoreError::BackingIntegrity))
    ));
}

#[test]
fn verified_native_descriptor_does_not_reopen_replaced_object_path() {
    let fixture = Fixture::new(b"abcd");
    let context = context();
    let caller = Caller(7);
    let mut stream = open_range(
        &context,
        &fixture.store,
        &fixture.authority,
        &caller,
        &1,
        &Purpose::Playback,
        ByteRange::new(0, 4).unwrap(),
    )
    .unwrap();
    let path = fixture.object_path();
    fs::remove_file(&path).unwrap();
    fs::write(&path, b"wrong bytes").unwrap();
    let mut output = [0xee; 4];
    assert_eq!(
        stream.read_chunk(&mut output).unwrap(),
        ChunkOutcome::Bytes(4)
    );
    assert_eq!(&output, b"abcd");
    assert_eq!(
        stream.read_chunk(&mut output).unwrap(),
        ChunkOutcome::Complete
    );
}

#[test]
fn authority_outage_or_revocation_after_a_chunk_terminates_future_delivery() {
    for outage in [true, false] {
        let fixture = Fixture::new(b"abcdefgh");
        let context = context();
        let caller = Caller(7);
        let mut stream = open_range(
            &context,
            &fixture.store,
            &fixture.authority,
            &caller,
            &1,
            &Purpose::Playback,
            ByteRange::new(0, 8).unwrap(),
        )
        .unwrap();
        let mut output = [0xee; 4];
        assert_eq!(
            stream.read_chunk(&mut output).unwrap(),
            ChunkOutcome::Bytes(4)
        );
        fixture.authority.unavailable.set(outage);
        fixture.authority.denied.set(!outage);
        output.fill(0xee);
        let expected = if outage {
            AccessFailure::Unavailable
        } else {
            AccessFailure::Denied
        };
        assert!(matches!(
            stream.read_chunk(&mut output), Err(RangeError::Access(actual)) if actual == expected
        ));
        assert_eq!(output, [0xee; 4]);
        assert!(matches!(
            stream.read_chunk(&mut output),
            Err(RangeError::Terminated)
        ));
    }
}

#[test]
fn native_read_adapter_preserves_its_configured_object_bound() {
    let fixture = Fixture::new(b"abcd");
    let smaller_store = NativeFileStore::new(&fixture.root, 3).unwrap();
    assert!(matches!(
        open_range(
            &context(),
            &smaller_store,
            &fixture.authority,
            &Caller(7),
            &1,
            &Purpose::Playback,
            ByteRange::new(0, 4).unwrap(),
        ),
        Err(RangeError::Store(StoreError::Capacity))
    ));
}

#[test]
fn exact_last_byte_is_delivered_and_extreme_offsets_obey_metadata_bounds() {
    let fixture = Fixture::new(b"abcd");
    let context = context();
    let caller = Caller(7);
    let mut stream = open_range(
        &context,
        &fixture.store,
        &fixture.authority,
        &caller,
        &1,
        &Purpose::Playback,
        ByteRange::from_start_len(3, 1).unwrap(),
    )
    .unwrap();
    let mut output = [0xee; 4];
    assert_eq!(
        stream.read_chunk(&mut output).unwrap(),
        ChunkOutcome::Bytes(1)
    );
    assert_eq!(output, [b'd', 0xee, 0xee, 0xee]);
    assert_eq!(
        stream.read_chunk(&mut output).unwrap(),
        ChunkOutcome::Complete
    );
    assert!(matches!(
        open_range(
            &context,
            &fixture.store,
            &fixture.authority,
            &caller,
            &1,
            &Purpose::Playback,
            ByteRange::from_start_len(u64::MAX - 1, 1).unwrap(),
        ),
        Err(RangeError::InvalidRange)
    ));
}

#[test]
fn current_authority_can_deny_an_interval_even_when_bytes_and_identity_match() {
    let fixture = Fixture::new(b"abcdefgh");
    fixture
        .authority
        .permitted_range
        .set(Some(ByteRange::new(2, 6).unwrap()));
    let context = context();
    let caller = Caller(7);
    assert!(matches!(
        open_range(
            &context,
            &fixture.store,
            &fixture.authority,
            &caller,
            &1,
            &Purpose::Playback,
            ByteRange::new(0, 8).unwrap(),
        ),
        Err(RangeError::Access(AccessFailure::Denied))
    ));
    let mut stream = open_range(
        &context,
        &fixture.store,
        &fixture.authority,
        &caller,
        &1,
        &Purpose::Playback,
        ByteRange::new(2, 6).unwrap(),
    )
    .unwrap();
    let mut output = [0xee; 4];
    assert_eq!(
        stream.read_chunk(&mut output).unwrap(),
        ChunkOutcome::Bytes(4)
    );
    assert_eq!(&output, b"cdef");
}

#[test]
fn a_stricter_cap_during_io_discards_the_chunk_without_partial_success() {
    let fixture = Fixture::new(b"abcd");
    let store = ControlledStore {
        bytes: b"abcd".to_vec(),
        denied_during_read: None,
        cap_during_read: Some((&fixture.authority.cap, 2)),
        wrong_seek: false,
    };
    let context = context();
    let caller = Caller(7);
    let mut stream = open_range(
        &context,
        &store,
        &fixture.authority,
        &caller,
        &1,
        &Purpose::Playback,
        ByteRange::new(0, 4).unwrap(),
    )
    .unwrap();
    let mut output = [0xee; 4];
    assert!(matches!(
        stream.read_chunk(&mut output),
        Err(RangeError::StaleBinding)
    ));
    assert_eq!(output, [0xee; 4]);
    assert!(matches!(
        stream.read_chunk(&mut output),
        Err(RangeError::Terminated)
    ));
}

#[test]
fn a_reader_that_cannot_seek_to_the_requested_offset_is_refused() {
    let fixture = Fixture::new(b"abcd");
    let store = ControlledStore {
        bytes: b"abcd".to_vec(),
        denied_during_read: None,
        cap_during_read: None,
        wrong_seek: true,
    };
    assert!(matches!(
        open_range(
            &context(),
            &store,
            &fixture.authority,
            &Caller(7),
            &1,
            &Purpose::Playback,
            ByteRange::new(1, 4).unwrap(),
        ),
        Err(RangeError::Store(StoreError::BackingIntegrity))
    ));
}
