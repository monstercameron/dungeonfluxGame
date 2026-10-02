use df_types::OperationId;
use sha2::{Digest, Sha256};

/// Exact caller-owned recording identity. Operation IDs correlate work, not access.
///
/// The trusted caller owns all key fields, access/source generations and semantic
/// qualification. Admission compares these opaque values without constructing them.
pub struct RecordIdentity<Key, Basis> {
    pub key: Key,
    pub basis: Basis,
    pub operation: OperationId,
}

/// Caller-selected positive byte and chunk bounds for one admitted stream.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecordLimits {
    max_record_bytes: usize,
    max_chunk_bytes: usize,
    max_chunks: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidRecordLimits;

impl RecordLimits {
    /// Zero bounds or a chunk bound larger than the record bound are invalid.
    pub fn new(
        max_record_bytes: usize,
        max_chunk_bytes: usize,
        max_chunks: usize,
    ) -> Result<Self, InvalidRecordLimits> {
        if max_record_bytes == 0
            || max_chunk_bytes == 0
            || max_chunks == 0
            || max_chunk_bytes > max_record_bytes
        {
            return Err(InvalidRecordLimits);
        }
        Ok(Self {
            max_record_bytes,
            max_chunk_bytes,
            max_chunks,
        })
    }
}

/// Classified input termination; errors carry no raw provider payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordInputError {
    Cancelled,
    Failed,
}

/// Candidate stream input. A terminal declaration alone does not admit a record.
pub enum RecordEvent<Key, Basis> {
    Chunk(Vec<u8>),
    Complete {
        identity: RecordIdentity<Key, Basis>,
        byte_length: u64,
        sha256: [u8; 32],
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordIdentityField {
    Key,
    Basis,
    Operation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordLimit {
    ChunkBytes,
    RecordBytes,
    ChunkCount,
}

/// Refusal never exposes a partial record or invokes the publisher.
#[derive(Debug, Eq, PartialEq)]
pub enum RecordAdmissionError<PublicationError> {
    Input(RecordInputError),
    MissingCompletion,
    TrailingEvent,
    IdentityMismatch(RecordIdentityField),
    LimitExceeded(RecordLimit),
    LengthMismatch,
    DigestMismatch,
    AllocationUnavailable,
    Publication(PublicationError),
}

/// Verified complete bytes; only stream admission can construct this value.
///
/// This proves byte finality and identity equality, not semantic qualification,
/// authorization, durable publication or completeness of any game feature.
pub struct CompleteRecord<Key, Basis> {
    identity: RecordIdentity<Key, Basis>,
    bytes: Vec<u8>,
    sha256: [u8; 32],
}

impl<Key, Basis> CompleteRecord<Key, Basis> {
    pub fn identity(&self) -> &RecordIdentity<Key, Basis> {
        &self.identity
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    /// Transfers verified bytes and their exact identity to the publication owner.
    pub fn into_parts(self) -> (RecordIdentity<Key, Basis>, Vec<u8>, [u8; 32]) {
        (self.identity, self.bytes, self.sha256)
    }
}

/// The existing recording owner supplies exact identity and durable storage types.
///
/// Publication must atomically revalidate the captured access/source basis and
/// operation generation against current authority before making bytes reusable.
/// A refused or failed publication must preserve any previous accepted record.
/// Ambiguous durable outcomes must remain typed uncertainty in `Error`; admission
/// neither retries nor fabricates an accepted artifact after an error.
pub trait RecordingPublisher {
    type Key: Eq;
    type Basis: Eq;
    type Artifact;
    type Error;

    fn publish_complete(
        &mut self,
        record: CompleteRecord<Self::Key, Self::Basis>,
    ) -> Result<Self::Artifact, Self::Error>;
}

/// Verifies one bounded stream, then invokes the publication owner exactly once.
///
/// Chunks remain private until a matching final declaration, exact length/hash and
/// actual iterator EOF have all been observed. Any event after the declaration is
/// refused, including another declaration or an empty chunk. A cancelled or failed
/// input never publishes. The synchronous iterator and publisher remain owned by
/// the caller; this function starts no task, provider, network call or retry.
pub fn admit_complete_record<Publisher, Events>(
    publisher: &mut Publisher,
    expected: RecordIdentity<Publisher::Key, Publisher::Basis>,
    limits: RecordLimits,
    events: Events,
) -> Result<Publisher::Artifact, RecordAdmissionError<Publisher::Error>>
where
    Publisher: RecordingPublisher,
    Events: IntoIterator<
        Item = Result<RecordEvent<Publisher::Key, Publisher::Basis>, RecordInputError>,
    >,
{
    let mut events = events.into_iter();
    let mut bytes = Vec::new();
    let mut hasher = Sha256::new();
    let mut chunks = 0usize;
    loop {
        let event = events
            .next()
            .ok_or(RecordAdmissionError::MissingCompletion)?
            .map_err(RecordAdmissionError::Input)?;
        match event {
            RecordEvent::Chunk(chunk) => {
                if chunk.len() > limits.max_chunk_bytes {
                    return Err(RecordAdmissionError::LimitExceeded(RecordLimit::ChunkBytes));
                }
                if chunks >= limits.max_chunks {
                    return Err(RecordAdmissionError::LimitExceeded(RecordLimit::ChunkCount));
                }
                chunks += 1;
                let Some(total) = bytes.len().checked_add(chunk.len()) else {
                    return Err(RecordAdmissionError::LimitExceeded(
                        RecordLimit::RecordBytes,
                    ));
                };
                if total > limits.max_record_bytes {
                    return Err(RecordAdmissionError::LimitExceeded(
                        RecordLimit::RecordBytes,
                    ));
                }
                bytes
                    .try_reserve_exact(chunk.len())
                    .map_err(|_| RecordAdmissionError::AllocationUnavailable)?;
                hasher.update(&chunk);
                bytes.extend_from_slice(&chunk);
            }
            RecordEvent::Complete {
                identity,
                byte_length,
                sha256,
            } => {
                let mismatch = if identity.key != expected.key {
                    Some(RecordIdentityField::Key)
                } else if identity.basis != expected.basis {
                    Some(RecordIdentityField::Basis)
                } else if identity.operation != expected.operation {
                    Some(RecordIdentityField::Operation)
                } else {
                    None
                };
                if let Some(field) = mismatch {
                    return Err(RecordAdmissionError::IdentityMismatch(field));
                }
                if u64::try_from(bytes.len()).ok() != Some(byte_length) {
                    return Err(RecordAdmissionError::LengthMismatch);
                }
                let actual_sha256: [u8; 32] = hasher.finalize().into();
                if actual_sha256 != sha256 {
                    return Err(RecordAdmissionError::DigestMismatch);
                }
                match events.next() {
                    None => {}
                    Some(Err(error)) => return Err(RecordAdmissionError::Input(error)),
                    Some(Ok(_)) => return Err(RecordAdmissionError::TrailingEvent),
                }
                return publisher
                    .publish_complete(CompleteRecord {
                        identity: expected,
                        bytes,
                        sha256: actual_sha256,
                    })
                    .map_err(RecordAdmissionError::Publication);
            }
        }
    }
}
