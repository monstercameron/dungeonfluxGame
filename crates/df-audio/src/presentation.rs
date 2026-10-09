//! Checked local playback labels. These values grant no source access or audience authority.
use df_assets::AssetManifest;
use df_types::{ClientBindingId, OperationId, RevisionLabel, RunId, SessionId, SessionRevision};

use crate::QueueError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaybackBasis {
    session: SessionId,
    run: RunId,
    revision: SessionRevision,
}

impl PlaybackBasis {
    pub fn new(session: SessionId, run: RunId, revision: SessionRevision) -> Self {
        Self {
            session,
            run,
            revision,
        }
    }
    pub fn session(self) -> SessionId {
        self.session
    }
    pub fn run(self) -> RunId {
        self.run
    }
    pub fn revision(self) -> SessionRevision {
        self.revision
    }
}

/// Opaque correlation of one already-permitted cue, with generation ordered only within its job.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaybackCue {
    basis: PlaybackBasis,
    job: [u8; 16],
    operation: OperationId,
    generation: u64,
}

impl PlaybackCue {
    pub fn new(
        basis: PlaybackBasis,
        job: [u8; 16],
        operation: OperationId,
        generation: u64,
    ) -> Result<Self, QueueError> {
        if job == [0; 16] || generation == 0 {
            return Err(QueueError::InvalidGeneration);
        }
        Ok(Self {
            basis,
            job,
            operation,
            generation,
        })
    }
    pub fn basis(self) -> PlaybackBasis {
        self.basis
    }
    pub fn job(self) -> [u8; 16] {
        self.job
    }
    pub fn operation(self) -> OperationId {
        self.operation
    }
    pub fn generation(self) -> u64 {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlaybackDestination {
    PublicRoom,
    PrivateListener,
}

/// Equality/freshness labels for one selected output. No canonical audience or member list.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaybackOutput {
    lease_id: [u8; 16],
    binding: ClientBindingId,
    generation: u64,
    destination: PlaybackDestination,
    device_unlocked: bool,
}

impl PlaybackOutput {
    pub fn new(
        lease_id: [u8; 16],
        binding: ClientBindingId,
        generation: u64,
        destination: PlaybackDestination,
        device_unlocked: bool,
    ) -> Result<Self, QueueError> {
        if lease_id == [0; 16] || generation == 0 {
            return Err(QueueError::InvalidLease);
        }
        Ok(Self {
            lease_id,
            binding,
            generation,
            destination,
            device_unlocked,
        })
    }
    pub fn lease_id(self) -> [u8; 16] {
        self.lease_id
    }
    pub fn binding(self) -> ClientBindingId {
        self.binding
    }
    pub fn generation(self) -> u64 {
        self.generation
    }
    pub fn destination(self) -> PlaybackDestination {
        self.destination
    }
    pub fn device_unlocked(self) -> bool {
        self.device_unlocked
    }
}

/// Immutable source binding for already-authorized decoded Audio-kind PCM only.
/// The native adapter checks kind before constructing this projection; a browser decoder/
/// mounted shell must already have its current source authorization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcmSource {
    key: RevisionLabel,
    manifest: AssetManifest,
}

impl PcmSource {
    pub fn new(key: RevisionLabel, manifest: AssetManifest) -> Result<Self, QueueError> {
        if manifest.byte_len == 0 {
            return Err(QueueError::WrongAsset);
        }
        Ok(Self { key, manifest })
    }
    pub fn key(&self) -> &RevisionLabel {
        &self.key
    }
    pub fn manifest(&self) -> AssetManifest {
        self.manifest
    }
}
