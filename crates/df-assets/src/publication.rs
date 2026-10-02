use df_observe::OperationContext;
use df_types::OperationId;
use std::io::Read;

/// Expected identity of one complete, unmodified byte sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssetManifest {
    pub byte_len: u64,
    pub sha256: [u8; 32],
}

/// One publication attempt. The metadata owner supplies its own version and metadata types.
#[derive(Clone, Debug)]
pub struct Publication<V, M> {
    pub operation: OperationId,
    pub version: V,
    pub metadata: M,
    pub bytes: AssetManifest,
}

/// A byte object confirmed by the byte store. Its digest is a private storage key, not access.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DurableObject {
    digest: [u8; 32],
    byte_len: u64,
}

impl DurableObject {
    /// Reconstruct a storage key from retained metadata. This does not prove backing exists;
    /// the byte store must confirm the object before any use or idempotent publication.
    pub fn from_manifest(manifest: AssetManifest) -> Self {
        Self {
            digest: manifest.sha256,
            byte_len: manifest.byte_len,
        }
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    pub fn byte_len(&self) -> u64 {
        self.byte_len
    }
}

/// Exact metadata binding returned by the visibility authority.
#[derive(Clone, Debug)]
pub struct PublishedBinding<M> {
    pub metadata: M,
    pub bytes: AssetManifest,
    pub object: DurableObject,
}

/// Result of the metadata owner's atomic immutable-version comparison and commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationStatus {
    Published,
    AlreadyPublished,
    VersionConflict,
}

/// The metadata owner must preserve an ambiguous commit as unknown.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataFailure {
    Unavailable,
    Unknown,
    UnsupportedManifest,
}

/// Byte I/O failures never count as a publication receipt.
#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
    Capacity,
    StagingConflict,
    BackingMissing,
    BackingIntegrity,
}

impl From<std::io::Error> for StoreError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Refusal or uncertain failure before a confirmed publication receipt.
#[derive(Debug)]
pub enum PublicationError {
    Incomplete,
    HashMismatch,
    VersionConflict,
    Store(StoreError),
    Metadata(MetadataFailure),
}

impl From<StoreError> for PublicationError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

/// Receipt for one metadata-confirmed immutable version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicationReceipt<V> {
    pub version: V,
    pub status: PublicationStatus,
}

/// Consumer-owned byte port. Staging has no publication visibility.
pub trait AssetStore {
    type Staged;

    fn stage(
        &self,
        operation: OperationId,
        reader: &mut dyn Read,
    ) -> Result<Self::Staged, StoreError>;
    fn check_expected(&self, expected: AssetManifest) -> Result<(), StoreError>;
    fn staged_operation(&self, staged: &Self::Staged) -> Result<OperationId, StoreError>;
    fn staged_len(&self, staged: &Self::Staged) -> Result<u64, StoreError>;
    fn matches_published<M: Eq>(
        &self,
        staged: &Self::Staged,
        published: &PublishedBinding<M>,
    ) -> Result<bool, StoreError>;
    fn verify_and_promote(
        &self,
        staged: &Self::Staged,
        expected: AssetManifest,
    ) -> Result<DurableObject, PublicationError>;
}

/// Consumer-owned metadata visibility port. `publish_immutable` compares and commits atomically.
/// Its native implementation must recheck current rights and suppression before private
/// publication; an unavailable decision cannot produce a successful receipt.
pub trait AssetMetadataStore {
    type Version: Clone + Eq;
    type Metadata: Clone + Eq;

    fn lookup(
        &self,
        version: &Self::Version,
    ) -> Result<Option<PublishedBinding<Self::Metadata>>, MetadataFailure>;
    fn publish_immutable(
        &self,
        publication: &Publication<Self::Version, Self::Metadata>,
        object: DurableObject,
    ) -> Result<PublicationStatus, MetadataFailure>;
}

/// Publish only after complete staged bytes have been verified and made durable.
/// An exact retry uses the existing binding; a changed version binding is refused.
pub fn publish<S, M>(
    context: &OperationContext,
    publication: &Publication<M::Version, M::Metadata>,
    staged: &S::Staged,
    byte_store: &S,
    metadata_store: &M,
) -> Result<PublicationReceipt<M::Version>, PublicationError>
where
    S: AssetStore,
    M: AssetMetadataStore,
{
    let mut span = df_observe::begin(context, "asset.publish");
    let result = publish_inner(publication, staged, byte_store, metadata_store);
    let status = match &result {
        Ok(receipt) if receipt.status == PublicationStatus::Published => "published",
        Ok(_) => "already_published",
        Err(PublicationError::Incomplete) => "incomplete",
        Err(PublicationError::HashMismatch) => "hash_mismatch",
        Err(PublicationError::VersionConflict) => "version_conflict",
        Err(PublicationError::Store(_)) => "byte_store_failure",
        Err(PublicationError::Metadata(MetadataFailure::Unknown)) => "metadata_unknown",
        Err(PublicationError::Metadata(_)) => "metadata_failure",
    };
    if result.is_ok() {
        match usize::try_from(publication.bytes.byte_len) {
            Ok(bytes) => span.finish(status, bytes),
            Err(_) => span.finish_unmeasured(status),
        }
    } else {
        span.finish_unmeasured(status);
    }
    result
}

fn publish_inner<S, M>(
    publication: &Publication<M::Version, M::Metadata>,
    staged: &S::Staged,
    byte_store: &S,
    metadata_store: &M,
) -> Result<PublicationReceipt<M::Version>, PublicationError>
where
    S: AssetStore,
    M: AssetMetadataStore,
{
    byte_store
        .check_expected(publication.bytes)
        .map_err(PublicationError::Store)?;
    if byte_store
        .staged_operation(staged)
        .map_err(PublicationError::Store)?
        != publication.operation
    {
        return Err(PublicationError::Store(StoreError::StagingConflict));
    }
    let length = byte_store
        .staged_len(staged)
        .map_err(PublicationError::Store)?;
    if length != publication.bytes.byte_len {
        return Err(PublicationError::Incomplete);
    }

    if let Some(previous) = metadata_store
        .lookup(&publication.version)
        .map_err(PublicationError::Metadata)?
    {
        if previous.metadata != publication.metadata
            || previous.bytes != publication.bytes
            || !byte_store
                .matches_published(staged, &previous)
                .map_err(PublicationError::Store)?
        {
            return Err(PublicationError::VersionConflict);
        }
        return commit(publication, previous.object, metadata_store);
    }

    let object = byte_store.verify_and_promote(staged, publication.bytes)?;
    commit(publication, object, metadata_store)
}

fn commit<M: AssetMetadataStore>(
    publication: &Publication<M::Version, M::Metadata>,
    object: DurableObject,
    metadata_store: &M,
) -> Result<PublicationReceipt<M::Version>, PublicationError> {
    match metadata_store
        .publish_immutable(publication, object)
        .map_err(PublicationError::Metadata)?
    {
        PublicationStatus::VersionConflict => Err(PublicationError::VersionConflict),
        status => Ok(PublicationReceipt {
            version: publication.version.clone(),
            status,
        }),
    }
}
