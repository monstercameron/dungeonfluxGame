//! Bounded decoded presentation resources, separate from production playback.
//!
//! The queue owns PCM and preserves supplied frame offsets. Canonical media identities,
//! output leases and immutable asset references do not authorize audio by themselves.
//! No provider bytes, decoder, browser node or game rule is implemented here.
//! A mounted playback adapter must stop its actual nodes before confirming stop,
//! and explicitly dispose this owner before unmounting. Queue callbacks report
//! resource lifetime facts; they never attest that a speaker was audible.

mod pcm;
mod queue;

pub use pcm::{PcmBuffer, PcmFormat, PcmRefusal};
pub use queue::{
    AudioQueue, AudioReceipt, BufferCompletion, BufferTicket, Cancellation, EnqueueRefusal,
    MissingPlaybackPrerequisite, PcmView, QueueError, QueueLimits, QueueSnapshot, QueueState,
    REQUIRED_PLAYBACK_PREREQUISITES, Replacement,
};

pub mod browser_playback;
