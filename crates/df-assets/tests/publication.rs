#![cfg(not(target_arch = "wasm32"))]

use df_assets::{
    AssetManifest, AssetMetadataStore, AssetStore, DurableObject, MetadataFailure, NativeFileStore,
    Publication, PublicationError, PublicationStatus, PublishedBinding, StoreError, publish,
};
use df_observe::OperationContext;
use df_types::OperationId;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

static TEST_ROOT: AtomicUsize = AtomicUsize::new(0);

fn root() -> PathBuf {
    let temporary = PathBuf::from(std::env::var_os("TMPDIR").expect("owned TMPDIR is required"));
    loop {
        let number = TEST_ROOT.fetch_add(1, Ordering::Relaxed);
        let path = temporary.join(format!("df-assets-test-{}-{number}", std::process::id()));
        if fs::create_dir(&path).is_ok() {
            return path;
        }
    }
}

fn operation(seed: u8) -> OperationId {
    OperationId::from_bytes(&[seed; 16]).unwrap()
}

fn expected(bytes: &[u8]) -> AssetManifest {
    AssetManifest {
        byte_len: bytes.len() as u64,
        sha256: Sha256::digest(bytes).into(),
    }
}

fn publication(
    operation: OperationId,
    version: u8,
    metadata: &'static str,
    bytes: AssetManifest,
) -> Publication<u8, &'static str> {
    Publication {
        operation,
        version,
        metadata,
        bytes,
    }
}

fn context() -> OperationContext {
    OperationContext {
        trace_parent: String::new(),
        build: "asset-test".to_owned(),
    }
}

struct Metadata {
    bytes: Arc<NativeFileStore>,
    records: Mutex<BTreeMap<u8, PublishedBinding<&'static str>>>,
    fail_next: Mutex<Option<MetadataFailure>>,
    publish_calls: AtomicUsize,
}

impl Metadata {
    fn new(bytes: Arc<NativeFileStore>) -> Self {
        Self {
            bytes,
            records: Mutex::new(BTreeMap::new()),
            fail_next: Mutex::new(None),
            publish_calls: AtomicUsize::new(0),
        }
    }

    fn count(&self) -> usize {
        self.records.lock().unwrap().len()
    }
}

impl AssetMetadataStore for Metadata {
    type Version = u8;
    type Metadata = &'static str;

    fn lookup(
        &self,
        version: &u8,
    ) -> Result<Option<PublishedBinding<&'static str>>, MetadataFailure> {
        Ok(self.records.lock().unwrap().get(version).cloned())
    }

    fn publish_immutable(
        &self,
        publication: &Publication<u8, &'static str>,
        object: DurableObject,
    ) -> Result<PublicationStatus, MetadataFailure> {
        self.publish_calls.fetch_add(1, Ordering::Relaxed);
        self.bytes
            .confirm(object)
            .expect("metadata cannot publish before durable bytes are complete");
        let mut records = self.records.lock().unwrap();
        let status = match records.get(&publication.version) {
            Some(existing)
                if existing.metadata == publication.metadata
                    && existing.bytes == publication.bytes
                    && existing.object == object =>
            {
                PublicationStatus::AlreadyPublished
            }
            Some(_) => PublicationStatus::VersionConflict,
            None => {
                records.insert(
                    publication.version,
                    PublishedBinding {
                        metadata: publication.metadata,
                        bytes: publication.bytes,
                        object,
                    },
                );
                PublicationStatus::Published
            }
        };
        if let Some(failure) = self.fail_next.lock().unwrap().take() {
            if failure == MetadataFailure::Unavailable {
                records.remove(&publication.version);
            }
            return Err(failure);
        }
        Ok(status)
    }
}

#[test]
fn staged_partial_and_wrong_hash_never_reach_metadata() {
    let bytes = Arc::new(NativeFileStore::new(root(), 1024).unwrap());
    let metadata = Metadata::new(bytes.clone());
    let complete = b"asset-v1";
    let short = bytes.stage(operation(1), &mut &complete[..3]).unwrap();
    let candidate = publication(operation(1), 1, "image", expected(complete));
    assert!(matches!(
        publish(&context(), &candidate, &short, bytes.as_ref(), &metadata),
        Err(PublicationError::Incomplete)
    ));
    assert_eq!(metadata.publish_calls.load(Ordering::Relaxed), 0);
    assert_eq!(metadata.count(), 0);

    assert!(matches!(
        publish(
            &context(),
            &publication(operation(9), 1, "image", expected(complete)),
            &short,
            bytes.as_ref(),
            &metadata,
        ),
        Err(PublicationError::Store(StoreError::StagingConflict))
    ));
    assert_eq!(metadata.publish_calls.load(Ordering::Relaxed), 0);

    let wrong = bytes.stage(operation(2), &mut &complete[..]).unwrap();
    let wrong_manifest = expected(b"asset-w0");
    assert_eq!(wrong_manifest.byte_len, candidate.bytes.byte_len);
    assert!(matches!(
        publish(
            &context(),
            &publication(operation(2), 2, "image", wrong_manifest),
            &wrong,
            bytes.as_ref(),
            &metadata,
        ),
        Err(PublicationError::HashMismatch)
    ));
    assert_eq!(metadata.publish_calls.load(Ordering::Relaxed), 0);
    assert_eq!(metadata.count(), 0);
}

#[test]
fn durable_bytes_precede_visibility_and_old_version_survives_retry_and_conflict() {
    let path = root();
    let bytes = Arc::new(NativeFileStore::new(&path, 1024).unwrap());
    let metadata = Metadata::new(bytes.clone());
    let content = b"asset-v1";
    let first = publication(operation(3), 1, "image", expected(content));
    let staged = bytes.stage(operation(3), &mut &content[..]).unwrap();
    assert_eq!(metadata.count(), 0);
    let receipt = publish(&context(), &first, &staged, bytes.as_ref(), &metadata).unwrap();
    assert_eq!(receipt.status, PublicationStatus::Published);
    assert_eq!(metadata.count(), 1);
    let stored = metadata.lookup(&1).unwrap().unwrap();
    NativeFileStore::new(&path, 1024)
        .unwrap()
        .confirm(stored.object)
        .unwrap();

    let repeat = bytes.stage(operation(4), &mut &content[..]).unwrap();
    let repeated = publication(operation(4), 1, "image", expected(content));
    assert_eq!(
        publish(&context(), &repeated, &repeat, bytes.as_ref(), &metadata)
            .unwrap()
            .status,
        PublicationStatus::AlreadyPublished
    );
    let changed_metadata = publication(operation(4), 1, "audio", expected(content));
    assert!(matches!(
        publish(
            &context(),
            &changed_metadata,
            &repeat,
            bytes.as_ref(),
            &metadata,
        ),
        Err(PublicationError::VersionConflict)
    ));
    let changed = b"asset-v2";
    let replacement = bytes.stage(operation(5), &mut &changed[..]).unwrap();
    assert!(matches!(
        publish(
            &context(),
            &publication(operation(5), 1, "image", expected(changed)),
            &replacement,
            bytes.as_ref(),
            &metadata,
        ),
        Err(PublicationError::VersionConflict)
    ));
    assert_eq!(
        metadata.lookup(&1).unwrap().unwrap().bytes,
        expected(content)
    );
    assert_eq!(metadata.count(), 1);
}

#[test]
fn failed_or_unknown_metadata_acknowledgement_has_no_false_success() {
    let path = root();
    let bytes = Arc::new(NativeFileStore::new(&path, 1024).unwrap());
    let metadata = Metadata::new(bytes.clone());
    let content = b"committed";
    let staged = bytes.stage(operation(6), &mut &content[..]).unwrap();
    let candidate = publication(operation(6), 6, "still", expected(content));
    *metadata.fail_next.lock().unwrap() = Some(MetadataFailure::Unavailable);
    assert!(matches!(
        publish(&context(), &candidate, &staged, bytes.as_ref(), &metadata),
        Err(PublicationError::Metadata(MetadataFailure::Unavailable))
    ));
    assert_eq!(metadata.count(), 0);
    assert_eq!(
        publish(&context(), &candidate, &staged, bytes.as_ref(), &metadata)
            .unwrap()
            .status,
        PublicationStatus::Published
    );

    let next = bytes.stage(operation(7), &mut &b"new"[..]).unwrap();
    let next_candidate = publication(operation(7), 7, "still", expected(b"new"));
    *metadata.fail_next.lock().unwrap() = Some(MetadataFailure::Unknown);
    assert!(matches!(
        publish(
            &context(),
            &next_candidate,
            &next,
            bytes.as_ref(),
            &metadata
        ),
        Err(PublicationError::Metadata(MetadataFailure::Unknown))
    ));
    let reopened = NativeFileStore::new(&path, 1024).unwrap();
    let resumed = reopened.resume_staged(operation(7)).unwrap();
    assert_eq!(
        publish(&context(), &next_candidate, &resumed, &reopened, &metadata)
            .unwrap()
            .status,
        PublicationStatus::AlreadyPublished
    );
}

struct BrokenReader(bool);

impl Read for BrokenReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.0 {
            Err(io::Error::other("interrupted upload"))
        } else {
            self.0 = true;
            buffer[0] = 1;
            Ok(1)
        }
    }
}

#[test]
fn interrupted_upload_never_gets_a_staged_handle_or_metadata_call() {
    let bytes = Arc::new(NativeFileStore::new(root(), 1024).unwrap());
    let metadata = Metadata::new(bytes.clone());
    assert!(matches!(
        bytes.stage(operation(8), &mut BrokenReader(false)),
        Err(StoreError::Io(_))
    ));
    assert_eq!(metadata.count(), 0);
    assert_eq!(metadata.publish_calls.load(Ordering::Relaxed), 0);
}

#[test]
fn configured_byte_ceiling_accepts_edge_and_refuses_excess_before_publication() {
    let path = root();
    let bytes = Arc::new(NativeFileStore::new(&path, 4).unwrap());
    let metadata = Metadata::new(bytes.clone());
    let edge = bytes.stage(operation(10), &mut &b"1234"[..]).unwrap();
    let candidate = publication(operation(10), 10, "still", expected(b"1234"));
    assert_eq!(
        publish(&context(), &candidate, &edge, bytes.as_ref(), &metadata)
            .unwrap()
            .status,
        PublicationStatus::Published
    );

    assert!(matches!(
        bytes.stage(operation(11), &mut &b"12345"[..]),
        Err(StoreError::Capacity)
    ));
    assert!(matches!(
        publish(
            &context(),
            &publication(operation(10), 11, "still", expected(b"12345")),
            &edge,
            bytes.as_ref(),
            &metadata,
        ),
        Err(PublicationError::Store(StoreError::Capacity))
    ));
    assert_eq!(metadata.publish_calls.load(Ordering::Relaxed), 1);
    assert_eq!(metadata.count(), 1);
    assert_eq!(fs::read_dir(path.join("staging")).unwrap().count(), 1);
}

#[test]
fn abandoned_promotion_path_does_not_block_resumed_publication() {
    let path = root();
    let bytes = Arc::new(NativeFileStore::new(&path, 1024).unwrap());
    let metadata = Metadata::new(bytes.clone());
    let content = b"complete bytes";
    bytes.stage(operation(12), &mut &content[..]).unwrap();
    let marker = path.join("promotion").join("0c".repeat(16));
    let marker_bytes = b"abandoned promotion from a previous process";
    fs::write(&marker, marker_bytes).unwrap();

    let candidate = publication(operation(12), 12, "still", expected(content));
    let restarted = NativeFileStore::new(&path, 1024).unwrap();
    let resumed = restarted.resume_staged(operation(12)).unwrap();
    assert_eq!(metadata.count(), 0);
    assert_eq!(
        publish(&context(), &candidate, &resumed, &restarted, &metadata)
            .unwrap()
            .status,
        PublicationStatus::Published
    );
    assert_eq!(fs::read(&marker).unwrap().as_slice(), marker_bytes);
    restarted
        .confirm(metadata.lookup(&12).unwrap().unwrap().object)
        .unwrap();
    assert_eq!(
        publish(&context(), &candidate, &resumed, &restarted, &metadata)
            .unwrap()
            .status,
        PublicationStatus::AlreadyPublished
    );
    assert_eq!(metadata.count(), 1);
}

#[test]
fn two_occupied_promotion_slots_survive_publication_through_next_slot() {
    let path = root();
    let bytes = Arc::new(NativeFileStore::new(&path, 1024).unwrap());
    let metadata = Metadata::new(bytes.clone());
    let content = b"complete bytes";
    let staged = bytes.stage(operation(14), &mut &content[..]).unwrap();
    let promotion = path.join("promotion");
    let operation_name = "0e".repeat(16);
    let first = promotion.join(&operation_name);
    let second = promotion.join(format!("{operation_name}-1"));
    let first_marker = b"first abandoned promotion";
    let second_marker = b"second abandoned promotion";
    fs::write(&first, first_marker).unwrap();
    fs::write(&second, second_marker).unwrap();

    let candidate = publication(operation(14), 14, "still", expected(content));
    assert_eq!(
        publish(&context(), &candidate, &staged, bytes.as_ref(), &metadata)
            .unwrap()
            .status,
        PublicationStatus::Published
    );
    assert_eq!(fs::read(&first).unwrap().as_slice(), first_marker);
    assert_eq!(fs::read(&second).unwrap().as_slice(), second_marker);
    assert!(!promotion.join(format!("{operation_name}-2")).exists());
    bytes
        .confirm(metadata.lookup(&14).unwrap().unwrap().object)
        .unwrap();
    assert_eq!(metadata.count(), 1);
}

#[test]
fn occupied_promotion_slot_bound_refuses_without_visibility_or_overwrite() {
    let path = root();
    let bytes = Arc::new(NativeFileStore::new(&path, 1024).unwrap());
    let metadata = Metadata::new(bytes.clone());
    let content = b"complete bytes";
    let staged = bytes.stage(operation(13), &mut &content[..]).unwrap();
    let promotion = path.join("promotion");
    let operation_name = "0d".repeat(16);
    let marker_bytes = b"occupied";
    let mut occupied = Vec::with_capacity(1024);
    for slot in 0..1024 {
        let name = if slot == 0 {
            operation_name.clone()
        } else {
            format!("{operation_name}-{slot}")
        };
        let marker = promotion.join(name);
        fs::write(&marker, marker_bytes).unwrap();
        occupied.push(marker);
    }

    let candidate = publication(operation(13), 13, "still", expected(content));
    assert!(matches!(
        publish(&context(), &candidate, &staged, bytes.as_ref(), &metadata),
        Err(PublicationError::Store(StoreError::StagingConflict))
    ));
    assert_eq!(metadata.publish_calls.load(Ordering::Relaxed), 0);
    assert_eq!(metadata.count(), 0);
    assert_eq!(fs::read_dir(path.join("objects")).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&promotion).unwrap().count(), 1024);
    for marker in occupied {
        assert_eq!(fs::read(marker).unwrap().as_slice(), marker_bytes);
    }
}
