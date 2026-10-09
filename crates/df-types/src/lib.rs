//! Pure caller-supplied identity, recovery ordering, exact units, and nonsecret provenance.
//! Valid values confer no authentication, permissions, issuance, or build approval.
//!
//! Different identity kinds cannot be substituted:
//! ```compile_fail
//! use df_types::{MemberId, SessionId};
//! fn session_only(_: SessionId) {}
//! session_only(MemberId::from_bytes(&[1; 16]).unwrap());
//! ```
//! Text constructors preserve the same kind boundary:
//! ```compile_fail
//! use df_types::{MemberId, SessionId};
//! fn session_only(_: SessionId) {}
//! session_only(MemberId::from_hex("01010101010101010101010101010101").unwrap());
//! ```
mod identity;
mod locale;
mod money;
mod provenance;
mod revision;

pub use identity::{
    ClientBindingId, IdentityError, MemberId, OperationId, PaidInvoiceId, PriceVersion, RunId,
    SessionId, SubscriptionId, TextIdentityError,
};
pub use locale::{LocaleTag, LocaleTagError};
pub use money::{Currency, LiabilityRate, Money, MoneyError, Usage, UsageUnit};
pub use provenance::{
    BuildIdentity, BuildIdentityError, BuildRevision, RevisionLabel, RevisionLabelError,
};
pub use revision::{RecoveryEpoch, RevisionError, SessionRevision};
