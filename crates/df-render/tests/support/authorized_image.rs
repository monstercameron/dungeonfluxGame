use df_assets::{
    AccessFailure, AssetManifest, AssetMetadataStore, AssetReadAuthority, AssetResolver,
    AssetStore, AuthorizedBinding, ByteRange, ChunkOutcome, DurableObject, MetadataFailure,
    NativeFileStore, Publication, PublicationStatus, PublishedBinding, RangeError, publish,
};
use df_client::cache::{AssetCache, CacheError, CacheKey, CacheLimits, CacheScope};
use df_model::checkpoint::{AssetKind, AssetReference, ContentDigest};
use df_observe::OperationContext;
use df_render::{DecodedImage, SceneOwner};
use df_types::{
    ClientBindingId, OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision,
};
use sha2::{Digest, Sha256};
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

// Local deterministic fixture encoding, not a wire DTO or advertised production codec.
pub const SOURCE: &[u8] = b"RGBA\x01\0\0\0\x01\0\0\0\x10\x20\x30\xff";

pub fn label(text: &str) -> RevisionLabel {
    RevisionLabel::new(Some(text)).unwrap()
}
pub fn revision(epoch: u64, sequence: u64) -> SessionRevision {
    SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), sequence)
}
pub fn owner() -> SceneOwner {
    (
        SessionId::from_bytes(&[1; 16]).unwrap(),
        RunId::from_bytes(&[2; 16]).unwrap(),
        ClientBindingId::from_bytes(&[3; 16]).unwrap(),
    )
}
pub fn scope() -> CacheScope {
    let (session, run, binding) = owner();
    CacheScope {
        session,
        run,
        binding,
    }
}
pub fn context() -> OperationContext {
    OperationContext {
        trace_parent: String::new(),
        build: "wave29-current-render-consumer".to_owned(),
    }
}

pub struct Authority {
    pub published: RefCell<Option<PublishedBinding<AssetReference>>>,
    pub revoked: Cell<bool>,
    pub basis: Cell<u64>,
    pub calls: Cell<usize>,
    pub revoke_at: Cell<Option<usize>>,
}

impl AssetMetadataStore for Authority {
    type Version = RevisionLabel;
    type Metadata = AssetReference;
    fn lookup(
        &self,
        version: &RevisionLabel,
    ) -> Result<Option<PublishedBinding<AssetReference>>, MetadataFailure> {
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
        let mut published = self.published.borrow_mut();
        if let Some(old) = published.as_ref() {
            return Ok(
                if old.metadata == publication.metadata
                    && old.bytes == publication.bytes
                    && old.object == object
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
    type Purpose = u8;
    type Basis = u64;
    fn current_authorized_binding(
        &self,
        caller: &u8,
        version: &RevisionLabel,
        purpose: &u8,
        _range: ByteRange,
    ) -> Result<AuthorizedBinding<AssetReference, u64>, AccessFailure> {
        let calls = self.calls.get() + 1;
        self.calls.set(calls);
        if self.revoke_at.get() == Some(calls) {
            self.revoked.set(true);
        }
        if self.revoked.get() || *caller != 1 || *purpose != 1 {
            return Err(AccessFailure::Denied);
        }
        let published = self
            .lookup(version)
            .map_err(|_| AccessFailure::Unavailable)?
            .ok_or(AccessFailure::Absent)?;
        Ok(AuthorizedBinding {
            published,
            basis: self.basis.get(),
            max_chunk_bytes: 2,
        })
    }
}

pub struct Consumer {
    pub store: NativeFileStore,
    pub authority: Rc<Authority>,
    pub reference: AssetReference,
    pub key: CacheKey,
    pub bytes: Rc<RefCell<AssetCache>>,
    pub released: Rc<Cell<usize>>,
}

impl Consumer {
    pub fn new() -> Self {
        Self::with_version("current-rgba-fixture-v1")
    }

    pub fn with_version(version: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = PathBuf::from(std::env::var_os("TMPDIR").expect("root supplies owned TMPDIR"))
            .join(format!(
                "render-current-consumer-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        std::fs::create_dir(&root).unwrap();
        let store = NativeFileStore::new(root, 64).unwrap();
        let manifest = AssetManifest {
            byte_len: SOURCE.len() as u64,
            sha256: Sha256::digest(SOURCE).into(),
        };
        let reference = AssetReference {
            key: label(version),
            digest: ContentDigest(manifest.sha256),
            byte_length: manifest.byte_len,
            kind: AssetKind::Image,
        };
        let authority = Rc::new(Authority {
            published: RefCell::new(None),
            revoked: Cell::new(false),
            basis: Cell::new(1),
            calls: Cell::new(0),
            revoke_at: Cell::new(None),
        });
        let operation = OperationId::from_bytes(&[10; 16]).unwrap();
        let staged = store.stage(operation, &mut &SOURCE[..]).unwrap();
        let publication = Publication {
            operation,
            version: reference.key.clone(),
            metadata: reference.clone(),
            bytes: manifest,
        };
        assert_eq!(
            publish(
                &context(),
                &publication,
                &staged,
                &store,
                authority.as_ref()
            )
            .unwrap()
            .status,
            PublicationStatus::Published
        );
        let key = CacheKey {
            version: reference.key.clone(),
            bytes: manifest,
        };
        let mut bytes = AssetCache::new(
            scope(),
            CacheLimits {
                max_assets: 4,
                max_pending: 2,
                max_leases: 4,
                max_bytes: 64,
            },
        )
        .unwrap();
        bytes
            .apply_current(scope(), revision(1, 0), std::slice::from_ref(&key))
            .unwrap();
        Self {
            store,
            authority,
            reference,
            key,
            bytes: Rc::new(RefCell::new(bytes)),
            released: Rc::new(Cell::new(0)),
        }
    }

    pub fn read_current(&self, caller: u8, purpose: u8) -> Result<Vec<u8>, RangeError> {
        let basis = self.authority.basis.get();
        let resolver = AssetResolver::new(self.authority.as_ref(), &self.reference, &basis);
        let context = context();
        let range = ByteRange::new(0, self.reference.byte_length)?;
        let mut stream = resolver.open_native(&context, &self.store, &caller, &purpose, range)?;
        let mut output = Vec::with_capacity(SOURCE.len());
        let mut scratch = [0; 2];
        while let ChunkOutcome::Bytes(length) = stream.read_chunk(&mut scratch)? {
            output.extend_from_slice(&scratch[..length]);
        }
        Ok(output)
    }

    pub fn cache_streamed(&self) {
        let streamed = self.read_current(1, 1).unwrap();
        assert_eq!(streamed, SOURCE);
        let fetch = self.bytes.borrow_mut().fetch(&self.key).unwrap();
        self.bytes.borrow_mut().complete(&fetch, streamed).unwrap();
    }

    pub fn decoded(&self, key: CacheKey, pixel_bytes: Box<[u8]>) -> DecodedImage<CacheKey> {
        let lease = self.bytes.borrow_mut().acquire(&self.key).unwrap().unwrap();
        let basis = self.authority.basis.get();
        let guard_cache = self.bytes.clone();
        let guard_lease = lease.clone();
        let guard_authority = self.authority.clone();
        let release_cache = self.bytes.clone();
        let released = self.released.clone();
        DecodedImage::new(
            key,
            1,
            (pixel_bytes.len() / 4) as u32,
            pixel_bytes,
            move |expected| {
                if guard_lease.key() != expected
                    || guard_cache.borrow().lease_bytes(&guard_lease).is_err()
                {
                    return false;
                }
                let range = ByteRange::new(0, expected.bytes.byte_len).unwrap();
                guard_authority
                    .current_authorized_binding(&1, &expected.version, &1, range)
                    .is_ok_and(|binding| {
                        binding.basis == basis && binding.published.bytes == expected.bytes
                    })
            },
            move || {
                match release_cache.borrow_mut().release_lease(&lease) {
                    Ok(()) | Err(CacheError::StaleLease | CacheError::Closed) => {}
                    Err(error) => panic!("unexpected consumer lease release: {error:?}"),
                }
                released.set(released.get() + 1);
            },
        )
        .unwrap()
    }

    pub fn decode_fixture(&self) -> DecodedImage<CacheKey> {
        let pixels = {
            let mut cache = self.bytes.borrow_mut();
            let input = cache.get(&self.key).unwrap().unwrap();
            assert_eq!(input.get(..12), Some(&SOURCE[..12]));
            input.get(12..).unwrap().to_vec().into_boxed_slice()
        };
        self.decoded(self.key.clone(), pixels)
    }
}
