use crate::{
    AccessFailure, AssetManifest, AssetMetadataStore, AssetReadAuthority, AuthorizedBinding,
    ByteRange, DurableObject, MetadataFailure, Publication, PublicationStatus, PublishedBinding,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::{AuthorizedRange, NativeFileStore, RangeError, open_range};
use df_model::checkpoint::AssetReference;
#[cfg(not(target_arch = "wasm32"))]
use df_observe::OperationContext;
use df_types::RevisionLabel;
#[cfg(not(target_arch = "wasm32"))]
use std::fs::File;

/// Borrowed exact-reference adapter over the existing current read authority.
/// The caller supplies the expected current basis from that authority; these supplied values
/// confer no rights. Every I02 open/chunk check resolves the original trusted caller, purpose
/// and interval through the same authority, then requires the exact canonical reference/basis.
/// No storage key, cached bytes, trace field or prior resolution can bypass that check.
/// The delivery owner must serialize final authorization and disclosure with revocation.
pub struct AssetResolver<'a, A>
where
    A: AssetReadAuthority<Version = RevisionLabel, Metadata = AssetReference>,
{
    authority: &'a A,
    requested: &'a AssetReference,
    basis: &'a A::Basis,
}

impl<'a, A> AssetResolver<'a, A>
where
    A: AssetReadAuthority<Version = RevisionLabel, Metadata = AssetReference>,
{
    pub fn new(authority: &'a A, requested: &'a AssetReference, basis: &'a A::Basis) -> Self {
        Self {
            authority,
            requested,
            basis,
        }
    }

    /// Resolve and open the exact complete immutable native object for this bounded interval.
    /// I02 verifies complete backing on the descriptor it retains and rechecks current access
    /// around every chunk. No descriptor escapes and no duplicate byte verification runs here.
    /// Missing bytes, malformed metadata, stale basis and unavailable authority fail explicitly.
    /// An existing stream pins the verified descriptor; a later open rechecks its current path.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open_native<'stream>(
        &'stream self,
        context: &'stream OperationContext,
        byte_store: &NativeFileStore,
        caller: &'stream A::Caller,
        purpose: &A::Purpose,
        range: ByteRange,
    ) -> Result<AuthorizedRange<'stream, Self, File>, RangeError> {
        open_range(
            context,
            byte_store,
            self,
            caller,
            &self.requested.key,
            purpose,
            range,
        )
    }
}

// AssetReadAuthority inherits this port. Delegation preserves the one metadata owner;
// resolution itself never invokes lookup or publication through this adapter.
impl<A> AssetMetadataStore for AssetResolver<'_, A>
where
    A: AssetReadAuthority<Version = RevisionLabel, Metadata = AssetReference>,
{
    type Version = RevisionLabel;
    type Metadata = AssetReference;

    fn lookup(
        &self,
        version: &RevisionLabel,
    ) -> Result<Option<PublishedBinding<AssetReference>>, MetadataFailure> {
        self.authority.lookup(version)
    }

    fn publish_immutable(
        &self,
        publication: &Publication<RevisionLabel, AssetReference>,
        object: DurableObject,
    ) -> Result<PublicationStatus, MetadataFailure> {
        self.authority.publish_immutable(publication, object)
    }
}

impl<A> AssetReadAuthority for AssetResolver<'_, A>
where
    A: AssetReadAuthority<Version = RevisionLabel, Metadata = AssetReference>,
{
    type Caller = A::Caller;
    type Purpose = A::Purpose;
    type Basis = A::Basis;

    fn current_authorized_binding(
        &self,
        caller: &Self::Caller,
        version: &RevisionLabel,
        purpose: &Self::Purpose,
        range: ByteRange,
    ) -> Result<AuthorizedBinding<AssetReference, Self::Basis>, AccessFailure> {
        let current = self
            .authority
            .current_authorized_binding(caller, version, purpose, range)?;
        if version != &self.requested.key
            || &current.published.metadata != self.requested
            || &current.basis != self.basis
        {
            return Err(AccessFailure::Stale);
        }
        if current.published.bytes
            != (AssetManifest {
                byte_len: self.requested.byte_length,
                sha256: self.requested.digest.0,
            })
        {
            // The authority returned malformed canonical metadata. Fail closed rather than
            // accepting valid backing for a different claimed canonical reference.
            return Err(AccessFailure::Unavailable);
        }
        // I02 validates object/manifest consistency and the native complete backing itself.
        Ok(current)
    }
}
