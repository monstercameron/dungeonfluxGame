//! Separate operator and consented fact/media export admission.
//!
//! Authority adapters verify identity, MFA, tickets, rights and durable current
//! facts. Requests and cached grants cannot establish those facts. Admission and
//! synchronous use share the adapter's fence; repeat it for each export chunk.
use crate::rotation::CredentialFence;
use df_types::SessionId;

#[derive(Eq, PartialEq)]
pub struct PermissionScope<T, C> {
    pub tenant: T,
    pub campaign: C,
    pub session: SessionId,
}

/// Host controls are membership powers, never an operator permission.
/// Support inspection permits redacted diagnostics, not a private audience.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperatorPermission {
    SupportInspect,
    TelemetryQuery,
    TelemetryPin,
    TelemetryExport,
    DebugMutation,
    Refund,
    Deletion,
    Restore,
    RightsUnlock,
    PrivateCapture,
    BreakGlassAccess,
}

/// Construction carries an expected credential/binding basis, not authority.
pub struct AccessBasis<'a, A: PermissionAuthority> {
    pub credential: &'a A::Credential,
    pub credential_generation: &'a A::CredentialGeneration,
    pub binding_generation: &'a A::BindingGeneration,
}

/// Trusted current facts borrowed within the authority's serialization fence.
pub struct CurrentAccess<'a, A: PermissionAuthority> {
    pub principal: &'a A::Principal,
    pub credential: &'a A::Credential,
    pub fence: &'a CredentialFence<A::CredentialGeneration>,
    pub credential_unexpired: bool,
    pub binding_generation: &'a A::BindingGeneration,
    pub binding_active: bool,
    pub binding_unexpired: bool,
}

pub struct OperatorRequest<'a, A: PermissionAuthority> {
    pub principal: &'a A::Principal,
    pub scope: &'a PermissionScope<A::Tenant, A::Campaign>,
    pub permission: OperatorPermission,
    pub grant_version: &'a A::OperatorVersion,
    pub access: AccessBasis<'a, A>,
}

pub struct OperatorGrant<'a, A: PermissionAuthority> {
    pub principal: &'a A::Principal,
    pub scope: &'a PermissionScope<A::Tenant, A::Campaign>,
    pub permission: OperatorPermission,
    pub version: &'a A::OperatorVersion,
    pub active: bool,
    pub issued_at_seconds: u64,
    pub expires_at_seconds: u64,
}

pub struct CurrentOperator<'a, A: PermissionAuthority> {
    pub grant: OperatorGrant<'a, A>,
    pub access: CurrentAccess<'a, A>,
    pub now_seconds: u64,
    pub independently_authenticated: bool,
    pub mfa_verified: bool,
    pub reason_ticket_verified: bool,
    pub exceptional_audit_approved: bool,
    pub private_rights_authorized: bool,
    pub debug_enabled: bool,
}

/// One exact fact or media item; the source owner resolves its permitted range.
pub enum ExportItem<'a, A: PermissionAuthority> {
    Fact(&'a A::Fact),
    Media(&'a A::Media),
}

pub struct ExportRequest<'a, A: PermissionAuthority> {
    pub principal: &'a A::Principal,
    pub scope: &'a PermissionScope<A::Tenant, A::Campaign>,
    pub recipient: &'a A::Recipient,
    pub audience: &'a A::Audience,
    pub export_identity: &'a A::ExportIdentity,
    pub item: ExportItem<'a, A>,
    pub source_version: &'a A::SourceVersion,
    pub access_version: &'a A::AccessVersion,
    pub rights_version: &'a A::RightsVersion,
    pub access: AccessBasis<'a, A>,
}

/// The exact authorized selection returned by the current rights/grant owner.
/// A request cannot supply this record to the admission function directly.
pub struct ExportGrant<'a, A: PermissionAuthority> {
    pub principal: &'a A::Principal,
    pub scope: &'a PermissionScope<A::Tenant, A::Campaign>,
    pub recipient: &'a A::Recipient,
    pub audience: &'a A::Audience,
    pub export_identity: &'a A::ExportIdentity,
    pub permitted_item: ExportItem<'a, A>,
    pub source_version: &'a A::SourceVersion,
    pub access_version: &'a A::AccessVersion,
    pub rights_version: &'a A::RightsVersion,
    pub active: bool,
    pub expires_at_seconds: u64,
    pub consented: bool,
    pub capture_contributor_rights: bool,
    pub source_rights_and_attribution: bool,
}

pub struct CurrentExport<'a, A: PermissionAuthority> {
    pub grant: ExportGrant<'a, A>,
    pub access: CurrentAccess<'a, A>,
    pub now_seconds: u64,
    pub export_enabled: bool,
}

/// Native owners supply their actual identities and versions, without another
/// principal, role, commerce, credential or rights registry in this crate.
///
/// Each method must read current consistent facts and invoke its callback once
/// while serializing credential, binding, grant, consent and rights changes.
/// The callback must not await, reenter the owner or queue unguarded later I/O.
/// No cached membership capability or request boolean can issue a grant. Source
/// failures refuse admission. This port does not implement MFA or legal review.
pub trait PermissionAuthority: Sized {
    type Principal: Eq;
    type Credential: Eq;
    type CredentialGeneration: Ord;
    type BindingGeneration: Eq;
    type Tenant: Eq;
    type Campaign: Eq;
    type OperatorVersion: Eq;
    type Recipient: Eq;
    type Audience: Eq;
    type ExportIdentity: Eq;
    type Fact: Eq;
    type Media: Eq;
    type SourceVersion: Eq;
    type AccessVersion: Eq;
    type RightsVersion: Eq;
    type Error;

    fn with_operator<R>(
        &mut self,
        request: &OperatorRequest<'_, Self>,
        invoke: impl FnOnce(Option<CurrentOperator<'_, Self>>) -> R,
    ) -> Result<R, Self::Error>;

    fn with_export<R>(
        &mut self,
        request: &ExportRequest<'_, Self>,
        publish: impl FnOnce(Option<CurrentExport<'_, Self>>) -> R,
    ) -> Result<R, Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PermissionRefusal {
    MissingGrant,
    WrongPrincipal,
    WrongScope,
    WrongPermission,
    StaleOperatorVersion,
    Revoked,
    Expired,
    OperatorAuthenticationRequired,
    MfaRequired,
    ReasonTicketRequired,
    ExceptionalAuditRequired,
    PrivateRightsRequired,
    BreakGlassLifetimeExceeded,
    DebugDisabled,
    WrongCredential,
    CredentialRevoked,
    CredentialRotated,
    CredentialExpired,
    BindingRevoked,
    BindingReplaced,
    BindingExpired,
    ExportDisabled,
    WrongRecipient,
    WrongAudience,
    WrongExportIdentity,
    OutsideRange,
    StaleSourceVersion,
    StaleAccessVersion,
    StaleRightsVersion,
    ConsentRequired,
    RightsRequired,
}

#[derive(Debug, Eq, PartialEq)]
pub enum PermissionError<A, H> {
    Authority(A),
    Refused(PermissionRefusal),
    Handler(H),
}

/// Invoke the separately selected typed handler only after current permission.
/// The handler still owns redacted projection, typed debug preconditions and
/// session/commerce commits; a permission does not bypass those owners.
pub fn invoke_operator<A: PermissionAuthority, R, E>(
    authority: &mut A,
    request: &OperatorRequest<'_, A>,
    invoke: impl FnOnce() -> Result<R, E>,
) -> Result<R, PermissionError<A::Error, E>> {
    authority
        .with_operator(request, |current| {
            let current =
                current.ok_or(PermissionError::Refused(PermissionRefusal::MissingGrant))?;
            check_operator(request, &current).map_err(PermissionError::Refused)?;
            invoke().map_err(PermissionError::Handler)
        })
        .map_err(PermissionError::Authority)?
}

/// Recheck before each synchronous export disclosure, including after async work.
/// Refusal never invokes the producer. Previously delivered bytes cannot be
/// recalled, and no refusal reverses a committed gameplay decision.
pub fn publish_export<A: PermissionAuthority, R, E>(
    authority: &mut A,
    request: &ExportRequest<'_, A>,
    publish: impl FnOnce() -> Result<R, E>,
) -> Result<R, PermissionError<A::Error, E>> {
    authority
        .with_export(request, |current| {
            let current =
                current.ok_or(PermissionError::Refused(PermissionRefusal::MissingGrant))?;
            check_export(request, &current).map_err(PermissionError::Refused)?;
            publish().map_err(PermissionError::Handler)
        })
        .map_err(PermissionError::Authority)?
}

fn check_access<A: PermissionAuthority>(
    principal: &A::Principal,
    basis: &AccessBasis<'_, A>,
    current: &CurrentAccess<'_, A>,
) -> Result<(), PermissionRefusal> {
    if current.principal != principal {
        return Err(PermissionRefusal::WrongPrincipal);
    }
    if current.credential != basis.credential {
        return Err(PermissionRefusal::WrongCredential);
    }
    if !current.fence.is_active() {
        return Err(PermissionRefusal::CredentialRevoked);
    }
    if !current.credential_unexpired {
        return Err(PermissionRefusal::CredentialExpired);
    }
    if current.fence.generation() != basis.credential_generation {
        return Err(PermissionRefusal::CredentialRotated);
    }
    if !current.binding_active {
        return Err(PermissionRefusal::BindingRevoked);
    }
    if !current.binding_unexpired {
        return Err(PermissionRefusal::BindingExpired);
    }
    if current.binding_generation != basis.binding_generation {
        return Err(PermissionRefusal::BindingReplaced);
    }
    Ok(())
}

fn check_operator<A: PermissionAuthority>(
    request: &OperatorRequest<'_, A>,
    current: &CurrentOperator<'_, A>,
) -> Result<(), PermissionRefusal> {
    check_access(request.principal, &request.access, &current.access)?;
    let grant = &current.grant;
    if grant.principal != request.principal {
        return Err(PermissionRefusal::WrongPrincipal);
    }
    if grant.scope != request.scope {
        return Err(PermissionRefusal::WrongScope);
    }
    if grant.permission != request.permission {
        return Err(PermissionRefusal::WrongPermission);
    }
    if grant.version != request.grant_version {
        return Err(PermissionRefusal::StaleOperatorVersion);
    }
    if !grant.active {
        return Err(PermissionRefusal::Revoked);
    }
    if current.now_seconds < grant.issued_at_seconds
        || current.now_seconds >= grant.expires_at_seconds
    {
        return Err(PermissionRefusal::Expired);
    }
    if !current.independently_authenticated {
        return Err(PermissionRefusal::OperatorAuthenticationRequired);
    }
    if !current.mfa_verified {
        return Err(PermissionRefusal::MfaRequired);
    }
    if !current.reason_ticket_verified {
        return Err(PermissionRefusal::ReasonTicketRequired);
    }
    if matches!(
        request.permission,
        OperatorPermission::PrivateCapture | OperatorPermission::BreakGlassAccess
    ) {
        if !current.exceptional_audit_approved {
            return Err(PermissionRefusal::ExceptionalAuditRequired);
        }
        if !current.private_rights_authorized {
            return Err(PermissionRefusal::PrivateRightsRequired);
        }
    }
    if request.permission == OperatorPermission::BreakGlassAccess
        && grant
            .expires_at_seconds
            .saturating_sub(grant.issued_at_seconds)
            > 900
    {
        return Err(PermissionRefusal::BreakGlassLifetimeExceeded);
    }
    if request.permission == OperatorPermission::DebugMutation && !current.debug_enabled {
        return Err(PermissionRefusal::DebugDisabled);
    }
    Ok(())
}

fn check_export<A: PermissionAuthority>(
    request: &ExportRequest<'_, A>,
    current: &CurrentExport<'_, A>,
) -> Result<(), PermissionRefusal> {
    check_access(request.principal, &request.access, &current.access)?;
    let grant = &current.grant;
    if !current.export_enabled {
        return Err(PermissionRefusal::ExportDisabled);
    }
    if grant.principal != request.principal {
        return Err(PermissionRefusal::WrongPrincipal);
    }
    if grant.scope != request.scope {
        return Err(PermissionRefusal::WrongScope);
    }
    if grant.recipient != request.recipient {
        return Err(PermissionRefusal::WrongRecipient);
    }
    if grant.audience != request.audience {
        return Err(PermissionRefusal::WrongAudience);
    }
    if grant.export_identity != request.export_identity {
        return Err(PermissionRefusal::WrongExportIdentity);
    }
    if !grant.active {
        return Err(PermissionRefusal::Revoked);
    }
    if current.now_seconds >= grant.expires_at_seconds {
        return Err(PermissionRefusal::Expired);
    }
    if grant.source_version != request.source_version {
        return Err(PermissionRefusal::StaleSourceVersion);
    }
    if grant.access_version != request.access_version {
        return Err(PermissionRefusal::StaleAccessVersion);
    }
    if grant.rights_version != request.rights_version {
        return Err(PermissionRefusal::StaleRightsVersion);
    }
    if !grant.consented {
        return Err(PermissionRefusal::ConsentRequired);
    }
    if !grant.capture_contributor_rights || !grant.source_rights_and_attribution {
        return Err(PermissionRefusal::RightsRequired);
    }
    let permitted = match (&request.item, &grant.permitted_item) {
        (ExportItem::Fact(requested), ExportItem::Fact(allowed)) => requested == allowed,
        (ExportItem::Media(requested), ExportItem::Media(allowed)) => requested == allowed,
        _ => false,
    };
    if !permitted {
        return Err(PermissionRefusal::OutsideRange);
    }
    Ok(())
}
