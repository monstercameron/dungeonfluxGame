//! Typed provider-facing request/event shapes without transport or dispatch authority.

use df_types::Usage;

use crate::{ProviderFailureClass, RequestIdentity};

/// Bounded adapter progress in whole percent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderProgress(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidProviderProgress;

impl ProviderProgress {
    pub fn new(percent: u8) -> Result<Self, InvalidProviderProgress> {
        if percent > 100 {
            return Err(InvalidProviderProgress);
        }
        Ok(Self(percent))
    }

    pub fn percent(self) -> u8 {
        self.0
    }
}

/// Adapter-owned opaque correlation value. The API never imports an adapter crate.
#[derive(Clone, Eq, PartialEq)]
pub struct ProviderRequestId<Id>(pub Id);

impl<Id> std::fmt::Debug for ProviderRequestId<Id> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderRequestId")
            .finish_non_exhaustive()
    }
}

/// Every event is attributed to an adapter request and carries exact typed usage.
/// Terminal variants carry the original owner-admitted identity; this value grants
/// no send, retry, publication, or rights authority.
pub struct ProviderEvent<Id, Kind> {
    pub provider_request_id: ProviderRequestId<Id>,
    pub usage: Usage,
    pub kind: Kind,
}

/// A terminal output candidate remains separate from authorization/publication.
/// Text candidate and terminal/failure observations.
pub enum TextEventKind<Output> {
    Candidate(Output),
    Completed { identity: RequestIdentity },
    Failed(ProviderFailureClass),
}

/// Partial transcripts are explicitly nonterminal; only `Final` is transcript output.
pub enum SttEventKind<Transcript> {
    Partial(Transcript),
    Final(Transcript),
    Completed { identity: RequestIdentity },
    Failed(ProviderFailureClass),
}

/// Audio chunks carry an adapter sequence number and completion follows all chunks.
pub enum TtsEventKind<Chunk> {
    Chunk { sequence: u64, chunk: Chunk },
    Completed { identity: RequestIdentity },
    Failed(ProviderFailureClass),
}

/// Image progress is bounded; candidate output is not yet published or authorized.
pub enum ImageEventKind<Output> {
    Progress(ProviderProgress),
    Candidate(Output),
    Completed {
        identity: RequestIdentity,
        output: Output,
    },
    Failed(ProviderFailureClass),
}

/// Video acceptance and status use caller-owned opaque types; completion is terminal.
pub enum VideoEventKind<Handle, Status, Output> {
    Accepted {
        handle: Handle,
    },
    Status(Status),
    Progress(ProviderProgress),
    Completed {
        identity: RequestIdentity,
        output: Output,
    },
    Failed(ProviderFailureClass),
}

/// Sound progress and candidate output remain separate from durable publication.
pub enum SoundEventKind<Output> {
    Progress(ProviderProgress),
    Candidate(Output),
    Completed {
        identity: RequestIdentity,
        output: Output,
    },
    Failed(ProviderFailureClass),
}

pub type TextEvent<Id, Output> = ProviderEvent<Id, TextEventKind<Output>>;
pub type SttEvent<Id, Transcript> = ProviderEvent<Id, SttEventKind<Transcript>>;
pub type TtsEvent<Id, Chunk> = ProviderEvent<Id, TtsEventKind<Chunk>>;
pub type ImageEvent<Id, Output> = ProviderEvent<Id, ImageEventKind<Output>>;
pub type VideoEvent<Id, Handle, Status, Output> =
    ProviderEvent<Id, VideoEventKind<Handle, Status, Output>>;
pub type SoundEvent<Id, Output> = ProviderEvent<Id, SoundEventKind<Output>>;

/// Why a native adapter's owned stream is being closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderCloseReason {
    Completed,
    Cancelled,
    Deadline,
    OwnerDropped,
}

/// Native adapters own the stream and close it once on cancellation/deadline.
pub trait ProviderStreamOwner {
    type CloseError;

    fn close(&mut self, reason: ProviderCloseReason) -> Result<(), Self::CloseError>;
}

/// Owned close-once lease. Explicit cleanup returns its failure to the caller;
/// implicit drop cleanup forwards the result to an owner-supplied observer.
pub struct ProviderStreamLease<Owner, Observe>
where
    Owner: ProviderStreamOwner,
    Observe: FnMut(Result<(), Owner::CloseError>),
{
    owner: Owner,
    observe_drop: Observe,
    closed: bool,
}

impl<Owner, Observe> ProviderStreamLease<Owner, Observe>
where
    Owner: ProviderStreamOwner,
    Observe: FnMut(Result<(), Owner::CloseError>),
{
    pub fn new(owner: Owner, observe_drop: Observe) -> Self {
        Self {
            owner,
            observe_drop,
            closed: false,
        }
    }

    /// Closes this lease once. Subsequent calls do not repeat external cleanup.
    pub fn close(&mut self, reason: ProviderCloseReason) -> Option<Result<(), Owner::CloseError>> {
        if self.closed {
            return None;
        }
        self.closed = true;
        Some(self.owner.close(reason))
    }
}

impl<Owner, Observe> Drop for ProviderStreamLease<Owner, Observe>
where
    Owner: ProviderStreamOwner,
    Observe: FnMut(Result<(), Owner::CloseError>),
{
    fn drop(&mut self) {
        if !self.closed {
            self.closed = true;
            (self.observe_drop)(self.owner.close(ProviderCloseReason::OwnerDropped));
        }
    }
}
