/// Current membership facts remain owned by the authoritative native adapter.
///
/// Associated identities and roles must be the adapter's real domain types. The
/// principal comes from authentication, not a payer, trace, or client role label.
/// Implementations read one consistent current grant; source errors fail closed.
pub trait MembershipAuthority: Sized {
    type Principal: Eq;
    type Tenant: Eq;
    type Campaign: Eq;
    type Role: Eq;
    type Revision: Eq;
    type Error;

    fn read_current(
        &mut self,
        request: MembershipRequest<'_, Self>,
    ) -> Result<Option<MembershipRecord<Self>>, Self::Error>;
}

/// Exact requested scope and effective role; construction grants no permission.
pub struct MembershipRequest<'a, A: MembershipAuthority> {
    pub principal: &'a A::Principal,
    pub tenant: &'a A::Tenant,
    pub campaign: &'a A::Campaign,
    pub role: &'a A::Role,
}

/// One authoritative snapshot, including its access revision and current status.
///
/// The adapter owns role-to-capability mapping and timestamp units. `expires_at`
/// uses the same clock as the caller's `now`; equality means expired. Returning
/// this record is a trust boundary, not a way for request fields to issue grants.
pub struct MembershipRecord<A: MembershipAuthority> {
    pub principal: A::Principal,
    pub tenant: A::Tenant,
    pub campaign: A::Campaign,
    pub role: A::Role,
    pub revision: A::Revision,
    pub active: bool,
    pub expires_at: Option<u64>,
}

/// Typed denial facts without credential, principal, or private payload contents.
#[derive(Debug, Eq, PartialEq)]
pub enum MembershipError<E> {
    Source(E),
    Missing,
    WrongPrincipal,
    WrongTenant,
    WrongCampaign,
    WrongRole,
    Revoked,
    Expired,
    StaleRevision,
}

/// An opaque successfully checked snapshot, not a continuing publication grant.
///
/// A delivery owner must recheck current authority and publish under its own
/// transaction or serialized fence. A separate check followed by serialization
/// can race revocation. This value owns no grant store, lease, or role registry.
///
/// Caller-supplied facts cannot directly construct a checked capability:
/// ```compile_fail
/// use df_auth::membership::{MembershipAuthority, MembershipCapability, MembershipRecord};
/// fn forge<A: MembershipAuthority>(record: MembershipRecord<A>) -> MembershipCapability<A> {
///     MembershipCapability { record }
/// }
/// ```
pub struct MembershipCapability<A: MembershipAuthority> {
    record: MembershipRecord<A>,
}

impl<A: MembershipAuthority> MembershipCapability<A> {
    /// Returns the exact checked scope and effective role.
    pub fn request(&self) -> MembershipRequest<'_, A> {
        MembershipRequest {
            principal: &self.record.principal,
            tenant: &self.record.tenant,
            campaign: &self.record.campaign,
            role: &self.record.role,
        }
    }

    /// Returns the checked access basis; no revision or authority is minted.
    pub fn revision(&self) -> &A::Revision {
        &self.record.revision
    }
}

/// Reads current facts and checks every supplied scope and effective role.
///
/// No capability is produced after a missing, failed, mismatched, revoked, or
/// expired read. Source failures remain distinct from authorization refusals.
pub fn authorize_membership<A: MembershipAuthority>(
    authority: &mut A,
    request: MembershipRequest<'_, A>,
    now: u64,
) -> Result<MembershipCapability<A>, MembershipError<A::Error>> {
    // Borrow the same exact requested identities for lookup and validation. The
    // authority's returned identities must not silently replace the request.
    let record = authority
        .read_current(MembershipRequest {
            principal: request.principal,
            tenant: request.tenant,
            campaign: request.campaign,
            role: request.role,
        })
        .map_err(MembershipError::Source)?
        .ok_or(MembershipError::Missing)?;
    check_record(&record, &request, now)?;
    Ok(MembershipCapability { record })
}

/// Rereads current authority and rejects a changed access basis or permission.
///
/// Success is a current check only; it does not fence a later asynchronous action
/// or disclosure. The caller owns atomic check-and-use at its actual boundary.
pub fn revalidate_membership<A: MembershipAuthority>(
    authority: &mut A,
    capability: &MembershipCapability<A>,
    now: u64,
) -> Result<(), MembershipError<A::Error>> {
    let request = capability.request();
    let record = authority
        .read_current(MembershipRequest {
            principal: request.principal,
            tenant: request.tenant,
            campaign: request.campaign,
            role: request.role,
        })
        .map_err(MembershipError::Source)?
        .ok_or(MembershipError::Missing)?;
    check_record(&record, &request, now)?;
    if record.revision != capability.record.revision {
        return Err(MembershipError::StaleRevision);
    }
    Ok(())
}

fn check_record<A: MembershipAuthority>(
    record: &MembershipRecord<A>,
    request: &MembershipRequest<'_, A>,
    now: u64,
) -> Result<(), MembershipError<A::Error>> {
    if &record.principal != request.principal {
        return Err(MembershipError::WrongPrincipal);
    }
    if &record.tenant != request.tenant {
        return Err(MembershipError::WrongTenant);
    }
    if &record.campaign != request.campaign {
        return Err(MembershipError::WrongCampaign);
    }
    if &record.role != request.role {
        return Err(MembershipError::WrongRole);
    }
    if !record.active {
        return Err(MembershipError::Revoked);
    }
    if record.expires_at.is_some_and(|expiry| now >= expiry) {
        return Err(MembershipError::Expired);
    }
    Ok(())
}
