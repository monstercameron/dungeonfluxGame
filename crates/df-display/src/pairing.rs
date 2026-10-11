//! Passive display pairing and separately authorized Host-controls presentation.

use df_auth::rotation::{
    PublicationAuthority, PublicationError, PublicationLease, publish_current,
};
use df_model::checkpoint::AudienceScope;

/// Presentation selected by the current audience owner, never an authorization grant.
/// Retaining this value cannot authorize a later command or publication.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayPairingMode {
    Passive,
    HostControls,
}

/// Pairing refusals preserve the existing auth and synchronous publisher errors.
#[derive(Debug, Eq, PartialEq)]
pub enum DisplayPairingError<A, P> {
    PrivateAudience,
    Publication(PublicationError<A, P>),
}

/// Produce a paired display presentation inside the existing current auth fence.
///
/// Shared audiences remain passive; only a separately authorized current Host
/// audience selects Host controls. Device, invitation and display role are not
/// inputs to this decision. Member-private audiences never invoke the producer.
///
/// The owner must supply an already audience-filtered producer. Host controls
/// remain advertised input intents, not adjudication, operator access or a private
/// data bypass. Commands still pass their existing auth/rules/session boundaries.
///
/// The callback must complete synchronously without awaiting, reentering the
/// authority or queuing unguarded later I/O. Revocation and replacement are checked
/// by `publish_current` for each use; neither this mode nor the retained lease is
/// a continuing permission. Refusals never invoke the producer.
pub fn publish_paired_display<A, R, E>(
    authority: &mut A,
    lease: &PublicationLease<A>,
    publish: impl FnOnce(DisplayPairingMode) -> Result<R, E>,
) -> Result<R, DisplayPairingError<A::Error, E>>
where
    A: PublicationAuthority<Audience = AudienceScope>,
{
    let mode = match &lease.scope().audience {
        AudienceScope::Shared => DisplayPairingMode::Passive,
        AudienceScope::Host => DisplayPairingMode::HostControls,
        AudienceScope::Members(_) => return Err(DisplayPairingError::PrivateAudience),
    };
    publish_current(authority, lease, || publish(mode)).map_err(DisplayPairingError::Publication)
}
