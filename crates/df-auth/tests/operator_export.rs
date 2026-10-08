use df_auth::membership::{
    MembershipAuthority, MembershipError, MembershipRecord, MembershipRequest, authorize_membership,
};
use df_auth::permissions::*;
use df_auth::rotation::CredentialFence;
use df_types::SessionId;
use std::{cell::Cell, rc::Rc};

type Scope = PermissionScope<u64, u64>;
type RefusalCase = (fn(&mut Authority), PermissionRefusal);

#[derive(Eq, PartialEq)]
enum CampaignRole {
    Host,
}

struct HostMembership;

impl MembershipAuthority for HostMembership {
    type Principal = u64;
    type Tenant = u64;
    type Campaign = u64;
    type Role = CampaignRole;
    type Revision = u64;
    type Error = ();

    fn read_current(
        &mut self,
        _request: MembershipRequest<'_, Self>,
    ) -> Result<Option<MembershipRecord<Self>>, Self::Error> {
        Ok(Some(MembershipRecord {
            principal: 1,
            tenant: 3,
            campaign: 4,
            role: CampaignRole::Host,
            revision: 1,
            active: true,
            expires_at: Some(100),
        }))
    }
}
const PERMISSIONS: [OperatorPermission; 11] = [
    OperatorPermission::SupportInspect,
    OperatorPermission::TelemetryQuery,
    OperatorPermission::TelemetryPin,
    OperatorPermission::TelemetryExport,
    OperatorPermission::DebugMutation,
    OperatorPermission::Refund,
    OperatorPermission::Deletion,
    OperatorPermission::Restore,
    OperatorPermission::RightsUnlock,
    OperatorPermission::PrivateCapture,
    OperatorPermission::BreakGlassAccess,
];

fn scope() -> Scope {
    Scope {
        tenant: 3,
        campaign: 4,
        session: SessionId::from_bytes(&[5; 16]).unwrap(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceError {
    Offline,
}

struct Authority {
    scope: Scope,
    principal: u64,
    credential: u64,
    fence: CredentialFence<u64>,
    binding: u64,
    credential_unexpired: bool,
    binding_active: bool,
    binding_unexpired: bool,
    permission: OperatorPermission,
    operator_version: u64,
    active: bool,
    issued: u64,
    expires: u64,
    now: u64,
    independently_authenticated: bool,
    mfa: bool,
    ticket: bool,
    audit: bool,
    private_rights: bool,
    debug_enabled: bool,
    recipient: u64,
    audience: u64,
    export_identity: u64,
    item: u64,
    media: bool,
    source_version: u64,
    access_version: u64,
    rights_version: u64,
    consent: bool,
    contributor_rights: bool,
    source_rights: bool,
    export_enabled: bool,
    missing: bool,
    unavailable: bool,
    inside: Rc<Cell<bool>>,
}

impl Default for Authority {
    fn default() -> Self {
        Self {
            scope: scope(),
            principal: 2,
            credential: 7,
            fence: CredentialFence::new(1),
            binding: 1,
            credential_unexpired: true,
            binding_active: true,
            binding_unexpired: true,
            permission: OperatorPermission::SupportInspect,
            operator_version: 7,
            active: true,
            issued: 0,
            expires: 100,
            now: 99,
            independently_authenticated: true,
            mfa: true,
            ticket: true,
            audit: true,
            private_rights: true,
            debug_enabled: true,
            recipient: 8,
            audience: 9,
            export_identity: 10,
            item: 13,
            media: false,
            source_version: 11,
            access_version: 12,
            rights_version: 14,
            consent: true,
            contributor_rights: true,
            source_rights: true,
            export_enabled: true,
            missing: false,
            unavailable: false,
            inside: Rc::new(Cell::new(false)),
        }
    }
}

impl Authority {
    fn access(&self) -> CurrentAccess<'_, Self> {
        CurrentAccess {
            principal: &self.principal,
            credential: &self.credential,
            fence: &self.fence,
            credential_unexpired: self.credential_unexpired,
            binding_generation: &self.binding,
            binding_active: self.binding_active,
            binding_unexpired: self.binding_unexpired,
        }
    }
    fn serialized<R>(&mut self, invoke: impl FnOnce(&Self) -> R) -> Result<R, SourceError> {
        if self.unavailable {
            return Err(SourceError::Offline);
        }
        assert!(!self.inside.replace(true));
        let result = invoke(self);
        assert!(self.inside.replace(false));
        Ok(result)
    }
}

impl PermissionAuthority for Authority {
    type Principal = u64;
    type Credential = u64;
    type CredentialGeneration = u64;
    type BindingGeneration = u64;
    type Tenant = u64;
    type Campaign = u64;
    type OperatorVersion = u64;
    type Recipient = u64;
    type Audience = u64;
    type ExportIdentity = u64;
    type Fact = u64;
    type Media = u64;
    type SourceVersion = u64;
    type AccessVersion = u64;
    type RightsVersion = u64;
    type Error = SourceError;

    fn with_operator<R>(
        &mut self,
        _request: &OperatorRequest<'_, Self>,
        invoke: impl FnOnce(Option<CurrentOperator<'_, Self>>) -> R,
    ) -> Result<R, Self::Error> {
        self.serialized(|owner| {
            invoke(if owner.missing {
                None
            } else {
                Some(CurrentOperator {
                    grant: OperatorGrant {
                        principal: &owner.principal,
                        scope: &owner.scope,
                        permission: owner.permission,
                        version: &owner.operator_version,
                        active: owner.active,
                        issued_at_seconds: owner.issued,
                        expires_at_seconds: owner.expires,
                    },
                    access: owner.access(),
                    now_seconds: owner.now,
                    independently_authenticated: owner.independently_authenticated,
                    mfa_verified: owner.mfa,
                    reason_ticket_verified: owner.ticket,
                    exceptional_audit_approved: owner.audit,
                    private_rights_authorized: owner.private_rights,
                    debug_enabled: owner.debug_enabled,
                })
            })
        })
    }

    fn with_export<R>(
        &mut self,
        _request: &ExportRequest<'_, Self>,
        publish: impl FnOnce(Option<CurrentExport<'_, Self>>) -> R,
    ) -> Result<R, Self::Error> {
        self.serialized(|owner| {
            publish(if owner.missing {
                None
            } else {
                Some(CurrentExport {
                    grant: ExportGrant {
                        principal: &owner.principal,
                        scope: &owner.scope,
                        recipient: &owner.recipient,
                        audience: &owner.audience,
                        export_identity: &owner.export_identity,
                        permitted_item: if owner.media {
                            ExportItem::Media(&owner.item)
                        } else {
                            ExportItem::Fact(&owner.item)
                        },
                        source_version: &owner.source_version,
                        access_version: &owner.access_version,
                        rights_version: &owner.rights_version,
                        active: owner.active,
                        expires_at_seconds: owner.expires,
                        consented: owner.consent,
                        capture_contributor_rights: owner.contributor_rights,
                        source_rights_and_attribution: owner.source_rights,
                    },
                    access: owner.access(),
                    now_seconds: owner.now,
                    export_enabled: owner.export_enabled,
                })
            })
        })
    }
}

fn basis() -> AccessBasis<'static, Authority> {
    AccessBasis {
        credential: &7,
        credential_generation: &1,
        binding_generation: &1,
    }
}
fn operator(scope: &Scope, permission: OperatorPermission) -> OperatorRequest<'_, Authority> {
    OperatorRequest {
        principal: &2,
        scope,
        permission,
        grant_version: &7,
        access: basis(),
    }
}
fn export(scope: &Scope) -> ExportRequest<'_, Authority> {
    ExportRequest {
        principal: &2,
        scope,
        recipient: &8,
        audience: &9,
        export_identity: &10,
        item: ExportItem::Fact(&13),
        source_version: &11,
        access_version: &12,
        rights_version: &14,
        access: basis(),
    }
}

fn refused_operator(
    owner: &mut Authority,
    request: &OperatorRequest<'_, Authority>,
    expected: PermissionRefusal,
) {
    let mut calls = 0;
    let result = invoke_operator(owner, request, || {
        calls += 1;
        Ok::<_, ()>(())
    });
    assert_eq!(result, Err(PermissionError::Refused(expected)));
    assert_eq!(calls, 0, "refused handler must never run");
    assert!(!owner.inside.get());
}
fn refused_export(
    owner: &mut Authority,
    request: &ExportRequest<'_, Authority>,
    expected: PermissionRefusal,
) {
    let mut bytes = Vec::new();
    let result = publish_export(owner, request, || {
        bytes.extend_from_slice(b"private bytes");
        Ok::<_, ()>(())
    });
    assert_eq!(result, Err(PermissionError::Refused(expected)));
    assert!(bytes.is_empty(), "refused producer must not disclose bytes");
    assert!(!owner.inside.get());
}

#[test]
fn each_operator_action_requires_its_own_current_grant_inside_the_owner_fence() {
    let scope = scope();
    for granted in PERMISSIONS {
        let mut owner = Authority {
            permission: granted,
            ..Authority::default()
        };
        for requested in PERMISSIONS {
            let request = operator(&scope, requested);
            if requested == granted {
                let inside = owner.inside.clone();
                let mut calls = 0;
                assert_eq!(
                    invoke_operator(&mut owner, &request, || {
                        assert!(inside.get());
                        calls += 1;
                        Ok::<_, ()>(requested)
                    }),
                    Ok(requested)
                );
                assert_eq!(calls, 1);
            } else {
                refused_operator(&mut owner, &request, PermissionRefusal::WrongPermission);
            }
        }
    }
}

#[test]
fn host_payer_tenant_owner_and_customer_roles_cannot_mint_support_or_export_rights() {
    let scope = scope();
    // These are distinct authenticated native subjects with current external role
    // relations; none is present in either grant owner. Payment is not consulted.
    for principal in [1, 20, 21, 22] {
        let mut owner = Authority {
            missing: true,
            ..Authority::default()
        };
        let mut request = operator(&scope, OperatorPermission::SupportInspect);
        request.principal = &principal;
        refused_operator(&mut owner, &request, PermissionRefusal::MissingGrant);
        let mut request = export(&scope);
        request.principal = &principal;
        refused_export(&mut owner, &request, PermissionRefusal::MissingGrant);
    }
    let mut owner = Authority::default();
    let mut request = operator(&scope, OperatorPermission::SupportInspect);
    request.principal = &1;
    refused_operator(&mut owner, &request, PermissionRefusal::WrongPrincipal);
}

#[test]
fn operator_refusals_cover_identity_scope_versions_mfa_ticket_and_current_access() {
    let scope = scope();
    let request = operator(&scope, OperatorPermission::SupportInspect);
    let cases: &[RefusalCase] = &[
        (|a| a.principal = 99, PermissionRefusal::WrongPrincipal),
        (|a| a.scope.tenant = 99, PermissionRefusal::WrongScope),
        (|a| a.scope.campaign = 99, PermissionRefusal::WrongScope),
        (
            |a| a.scope.session = SessionId::from_bytes(&[99; 16]).unwrap(),
            PermissionRefusal::WrongScope,
        ),
        (
            |a| a.operator_version += 1,
            PermissionRefusal::StaleOperatorVersion,
        ),
        (|a| a.active = false, PermissionRefusal::Revoked),
        (|a| a.expires = a.now, PermissionRefusal::Expired),
        (|a| a.issued = a.now + 1, PermissionRefusal::Expired),
        (
            |a| a.independently_authenticated = false,
            PermissionRefusal::OperatorAuthenticationRequired,
        ),
        (|a| a.mfa = false, PermissionRefusal::MfaRequired),
        (
            |a| a.ticket = false,
            PermissionRefusal::ReasonTicketRequired,
        ),
        (|a| a.credential = 99, PermissionRefusal::WrongCredential),
        (|a| a.fence.revoke(), PermissionRefusal::CredentialRevoked),
        (
            |a| {
                a.fence.rotate(2).unwrap();
            },
            PermissionRefusal::CredentialRotated,
        ),
        (
            |a| a.credential_unexpired = false,
            PermissionRefusal::CredentialExpired,
        ),
        (
            |a| a.binding_active = false,
            PermissionRefusal::BindingRevoked,
        ),
        (|a| a.binding += 1, PermissionRefusal::BindingReplaced),
        (
            |a| a.binding_unexpired = false,
            PermissionRefusal::BindingExpired,
        ),
    ];
    for (mutate, refusal) in cases {
        let mut owner = Authority::default();
        mutate(&mut owner);
        refused_operator(&mut owner, &request, *refusal);
    }
}

#[test]
fn debug_and_exceptional_private_access_have_separate_enablement_audit_and_rights() {
    let scope = scope();
    let mut owner = Authority {
        permission: OperatorPermission::DebugMutation,
        debug_enabled: false,
        ..Authority::default()
    };
    refused_operator(
        &mut owner,
        &operator(&scope, OperatorPermission::DebugMutation),
        PermissionRefusal::DebugDisabled,
    );
    for permission in [
        OperatorPermission::PrivateCapture,
        OperatorPermission::BreakGlassAccess,
    ] {
        let mut owner = Authority {
            permission,
            audit: false,
            ..Authority::default()
        };
        refused_operator(
            &mut owner,
            &operator(&scope, permission),
            PermissionRefusal::ExceptionalAuditRequired,
        );
        owner.audit = true;
        owner.private_rights = false;
        refused_operator(
            &mut owner,
            &operator(&scope, permission),
            PermissionRefusal::PrivateRightsRequired,
        );
    }
    let mut owner = Authority {
        permission: OperatorPermission::BreakGlassAccess,
        issued: 10,
        now: 10,
        expires: 910,
        ..Authority::default()
    };
    let request = operator(&scope, OperatorPermission::BreakGlassAccess);
    assert_eq!(
        invoke_operator(&mut owner, &request, || Ok::<_, ()>(())),
        Ok(())
    );
    owner.expires = 911;
    refused_operator(
        &mut owner,
        &request,
        PermissionRefusal::BreakGlassLifetimeExceeded,
    );
    owner.expires = 910;
    owner.now = 910;
    refused_operator(&mut owner, &request, PermissionRefusal::Expired);
}

#[test]
fn permitted_facts_and_media_reach_only_the_exact_current_recipient_inside_fence() {
    let scope = scope();
    let mut owner = Authority::default();
    let inside = owner.inside.clone();
    let mut request = export(&scope);
    let mut received = Vec::new();
    assert_eq!(
        publish_export(&mut owner, &request, || {
            assert!(inside.get());
            received.extend_from_slice(b"consented fact");
            Ok::<_, ()>(8)
        }),
        Ok(8)
    );
    request.item = ExportItem::Media(&14);
    owner.item = 14;
    owner.media = true;
    assert_eq!(
        publish_export(&mut owner, &request, || {
            assert!(inside.get());
            received.extend_from_slice(b"consented media");
            Ok::<_, ()>(8)
        }),
        Ok(8)
    );
    assert_eq!(received, b"consented factconsented media");
}

#[test]
fn every_export_scope_selection_consent_rights_version_and_access_refusal_emits_no_bytes() {
    let scope = scope();
    let request = export(&scope);
    let cases: &[RefusalCase] = &[
        (
            |a| a.export_enabled = false,
            PermissionRefusal::ExportDisabled,
        ),
        (|a| a.principal += 1, PermissionRefusal::WrongPrincipal),
        (|a| a.scope.tenant += 1, PermissionRefusal::WrongScope),
        (|a| a.scope.campaign += 1, PermissionRefusal::WrongScope),
        (
            |a| a.scope.session = SessionId::from_bytes(&[99; 16]).unwrap(),
            PermissionRefusal::WrongScope,
        ),
        (|a| a.recipient += 1, PermissionRefusal::WrongRecipient),
        (|a| a.audience += 1, PermissionRefusal::WrongAudience),
        (
            |a| a.export_identity += 1,
            PermissionRefusal::WrongExportIdentity,
        ),
        (|a| a.active = false, PermissionRefusal::Revoked),
        (|a| a.expires = a.now, PermissionRefusal::Expired),
        (
            |a| a.source_version += 1,
            PermissionRefusal::StaleSourceVersion,
        ),
        (
            |a| a.access_version += 1,
            PermissionRefusal::StaleAccessVersion,
        ),
        (
            |a| a.rights_version += 1,
            PermissionRefusal::StaleRightsVersion,
        ),
        (|a| a.consent = false, PermissionRefusal::ConsentRequired),
        (
            |a| a.contributor_rights = false,
            PermissionRefusal::RightsRequired,
        ),
        (
            |a| a.source_rights = false,
            PermissionRefusal::RightsRequired,
        ),
        (|a| a.item += 1, PermissionRefusal::OutsideRange),
        (|a| a.media = true, PermissionRefusal::OutsideRange),
        (|a| a.credential += 1, PermissionRefusal::WrongCredential),
        (|a| a.fence.revoke(), PermissionRefusal::CredentialRevoked),
        (
            |a| {
                a.fence.rotate(2).unwrap();
            },
            PermissionRefusal::CredentialRotated,
        ),
        (
            |a| a.credential_unexpired = false,
            PermissionRefusal::CredentialExpired,
        ),
        (
            |a| a.binding_active = false,
            PermissionRefusal::BindingRevoked,
        ),
        (|a| a.binding += 1, PermissionRefusal::BindingReplaced),
        (
            |a| a.binding_unexpired = false,
            PermissionRefusal::BindingExpired,
        ),
    ];
    for (mutate, refusal) in cases {
        let mut owner = Authority::default();
        mutate(&mut owner);
        refused_export(&mut owner, &request, *refusal);
    }
}

#[test]
fn revocation_after_preparation_and_between_chunks_fences_future_bytes_without_recalling_prior_bytes()
 {
    let scope = scope();
    let request = export(&scope);
    let mut owner = Authority::default();
    let prepared = b"prepared private source";
    owner.consent = false;
    let mut received = Vec::new();
    assert_eq!(
        publish_export(&mut owner, &request, || {
            received.extend_from_slice(prepared);
            Ok::<_, ()>(())
        }),
        Err(PermissionError::Refused(PermissionRefusal::ConsentRequired))
    );
    assert!(received.is_empty());
    owner.consent = true;
    assert_eq!(
        publish_export(&mut owner, &request, || {
            received.extend_from_slice(b"chunk1");
            Ok::<_, ()>(())
        }),
        Ok(())
    );
    owner.active = false;
    assert_eq!(
        publish_export(&mut owner, &request, || {
            received.extend_from_slice(b"chunk2");
            Ok::<_, ()>(())
        }),
        Err(PermissionError::Refused(PermissionRefusal::Revoked))
    );
    assert_eq!(received, b"chunk1");
}

#[test]
fn unavailable_authority_and_handler_failure_are_distinct_and_do_not_fabricate_success() {
    let scope = scope();
    let mut owner = Authority {
        unavailable: true,
        ..Authority::default()
    };
    let mut calls = 0;
    assert_eq!(
        invoke_operator(
            &mut owner,
            &operator(&scope, OperatorPermission::SupportInspect),
            || {
                calls += 1;
                Ok::<_, ()>(())
            }
        ),
        Err(PermissionError::Authority(SourceError::Offline))
    );
    assert_eq!(
        publish_export(&mut owner, &export(&scope), || {
            calls += 1;
            Ok::<_, ()>(())
        }),
        Err(PermissionError::Authority(SourceError::Offline))
    );
    assert_eq!(calls, 0);
    owner.unavailable = false;
    assert_eq!(
        invoke_operator(
            &mut owner,
            &operator(&scope, OperatorPermission::SupportInspect),
            || Err::<(), _>("strict typed handler precondition")
        ),
        Err(PermissionError::Handler(
            "strict typed handler precondition"
        ))
    );
    assert_eq!(
        publish_export(&mut owner, &export(&scope), || Err::<(), _>(
            "byte sink unavailable"
        )),
        Err(PermissionError::Handler("byte sink unavailable"))
    );
}

#[test]
fn redacted_support_does_not_create_host_or_private_audience_or_export_grants() {
    let scope = scope();
    let mut owner = Authority::default();
    let host_subject = 1;
    let support_subject = 2;
    let mut memberships = HostMembership;
    let host = authorize_membership(
        &mut memberships,
        MembershipRequest {
            principal: &host_subject,
            tenant: &scope.tenant,
            campaign: &scope.campaign,
            role: &CampaignRole::Host,
        },
        99,
    )
    .unwrap_or_else(|_| panic!("current host membership must be admitted"));
    assert_eq!(host.request().principal, &host_subject);
    let denied = authorize_membership(
        &mut memberships,
        MembershipRequest {
            principal: &support_subject,
            tenant: &scope.tenant,
            campaign: &scope.campaign,
            role: &CampaignRole::Host,
        },
        99,
    );
    assert!(matches!(denied, Err(MembershipError::WrongPrincipal)));
    let mut emitted = String::new();
    assert_eq!(
        invoke_operator(
            &mut owner,
            &operator(&scope, OperatorPermission::SupportInspect),
            || {
                emitted.push_str("receipt IDs and aggregate diagnostics");
                Ok::<_, ()>(())
            }
        ),
        Ok(())
    );
    assert!(!emitted.contains("private dialogue"));
    owner.missing = true;
    refused_export(&mut owner, &export(&scope), PermissionRefusal::MissingGrant);
    owner.missing = false;
    owner.active = false;
    refused_operator(
        &mut owner,
        &operator(&scope, OperatorPermission::SupportInspect),
        PermissionRefusal::Revoked,
    );
}
