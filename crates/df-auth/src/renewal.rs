//! Current-version membership capability renewal and revocation policy.
//!
//! Renewal rereads the same authoritative membership scope and returns a new
//! snapshot at the current revision. The original snapshot is immutable and fails
//! revalidation after its revision becomes stale. Revocation is performed by the
//! authority that owns the grant: an inactive or missing grant denies renewal, and
//! a changed audience version fences existing [`crate::rotation::PublicationLease`]
//! values at publication. This module does not mutate a store or close a stream.

pub use crate::membership::renew_membership;
