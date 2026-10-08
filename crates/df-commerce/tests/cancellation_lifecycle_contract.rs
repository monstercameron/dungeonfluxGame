//! Executable, native-only boundary witness for the D04 design decision.
//!
//! Lifecycle observations below are test-owned policy inputs. They do not verify a
//! gateway event, publish an entitlement, write a ledger, or acknowledge erasure.

#![cfg(not(target_arch = "wasm32"))]

use df_auth::membership::{
    MembershipAuthority, MembershipError, MembershipRecord, MembershipRequest,
    authorize_membership, revalidate_membership,
};
use df_auth::permissions::{
    AccessBasis, CurrentAccess, CurrentExport, CurrentOperator, ExportGrant, ExportItem,
    ExportRequest, OperatorGrant, OperatorPermission, OperatorRequest, PermissionAuthority,
    PermissionError, PermissionRefusal, PermissionScope, invoke_operator, publish_export,
};
use df_auth::rotation::CredentialFence;
use df_commerce::{
    Access, ObservationAuthority, PaidInvoice, PaymentOutcome, SpendConsent, SpendCounter,
    SpendOperationObservation, SpendOperationStatus, SpendRefusal, SpendRequest, SpendSnapshot,
    SubscriptionObservation, SubscriptionRefusal, SubscriptionState, effective_access,
    observe_subscription, propose_spend,
};
use df_types::{Currency, Money, OperationId, SessionId};

const PRINCIPAL: u64 = 2;
const PAYER: u64 = 3;
const TENANT: u64 = 4;
const CAMPAIGN: u64 = 5;
const CREDENTIAL: u64 = 6;
const GENERATION: u64 = 7;
const RECIPIENT: u64 = 8;
const AUDIENCE: u64 = 9;
const EXPORT_IDENTITY: u64 = 10;
const FACT: u64 = 11;
const SOURCE_VERSION: u64 = 12;
const ACCESS_VERSION: u64 = 13;
const RIGHTS_VERSION: u64 = 14;

type Scope = PermissionScope<u64, u64>;

fn scope() -> Scope {
    Scope {
        tenant: TENANT,
        campaign: CAMPAIGN,
        session: SessionId::from_bytes(&[15; 16]).unwrap(),
    }
}

fn currency() -> Currency {
    Currency::parse("USD").unwrap()
}

fn pending_subscription() -> SubscriptionState {
    SubscriptionState {
        currency: currency(),
        revision: 1,
        object_revision: 1,
        access: Access::PendingInitial,
        paid_through: 0,
        grace_until: None,
    }
}

fn settled_observation() -> SubscriptionObservation {
    SubscriptionObservation {
        authority: ObservationAuthority::VerifiedLatest,
        expected_revision: 1,
        object_revision: 2,
        outcome: PaymentOutcome::Settled(PaidInvoice {
            period_start: 90,
            paid_through: 200,
            selected_allowance: Money::new(currency(), 20),
            allowance_already_recorded: false,
        }),
    }
}

fn paid_subscription() -> SubscriptionState {
    observe_subscription(pending_subscription(), settled_observation(), 100)
        .unwrap()
        .next
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LifecycleObservation {
    CancelAtPeriodEnd,
    CancelImmediate,
    RefundConfirmedEndingEntitlement,
    RefundUnknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CandidateReceipt {
    ProposedOnly,
    PendingReconciliation,
}

// This finite policy witness is deliberately private to the test. The native
// gateway, commerce transaction, journal, and disclosure owner remain absent.
fn governed_candidate(
    state: SubscriptionState,
    observation: LifecycleObservation,
) -> (SubscriptionState, CandidateReceipt) {
    match observation {
        LifecycleObservation::CancelAtPeriodEnd => (state, CandidateReceipt::ProposedOnly),
        LifecycleObservation::CancelImmediate
        | LifecycleObservation::RefundConfirmedEndingEntitlement => (
            SubscriptionState {
                access: Access::Restricted,
                ..state
            },
            CandidateReceipt::ProposedOnly,
        ),
        LifecycleObservation::RefundUnknown => (state, CandidateReceipt::PendingReconciliation),
    }
}

struct CurrentMembership {
    revision: u64,
    active: bool,
}

impl MembershipAuthority for CurrentMembership {
    type Principal = u64;
    type Tenant = u64;
    type Campaign = u64;
    type Role = u64;
    type Revision = u64;
    type Error = ();

    fn read_current(
        &mut self,
        _request: MembershipRequest<'_, Self>,
    ) -> Result<Option<MembershipRecord<Self>>, Self::Error> {
        Ok(Some(MembershipRecord {
            principal: PRINCIPAL,
            tenant: TENANT,
            campaign: CAMPAIGN,
            role: 1,
            revision: self.revision,
            active: self.active,
            expires_at: Some(200),
        }))
    }
}

struct CurrentPrivateAuthority {
    scope: Scope,
    credential_fence: CredentialFence<u64>,
    private_grant_present: bool,
    private_grant_active: bool,
    private_access_version: u64,
    consented: bool,
    source_rights: bool,
    export_enabled: bool,
    operator_permission: OperatorPermission,
    operator_grant_present: bool,
}

impl Default for CurrentPrivateAuthority {
    fn default() -> Self {
        Self {
            scope: scope(),
            credential_fence: CredentialFence::new(GENERATION),
            private_grant_present: false,
            private_grant_active: true,
            private_access_version: ACCESS_VERSION,
            consented: true,
            source_rights: true,
            export_enabled: true,
            operator_permission: OperatorPermission::Refund,
            operator_grant_present: false,
        }
    }
}

impl CurrentPrivateAuthority {
    fn access(&self) -> CurrentAccess<'_, Self> {
        CurrentAccess {
            principal: &PRINCIPAL,
            credential: &CREDENTIAL,
            fence: &self.credential_fence,
            credential_unexpired: true,
            binding_generation: &GENERATION,
            binding_active: true,
            binding_unexpired: true,
        }
    }
}

impl PermissionAuthority for CurrentPrivateAuthority {
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
    type Error = ();

    fn with_operator<R>(
        &mut self,
        _request: &OperatorRequest<'_, Self>,
        invoke: impl FnOnce(Option<CurrentOperator<'_, Self>>) -> R,
    ) -> Result<R, Self::Error> {
        Ok(invoke(self.operator_grant_present.then(|| {
            CurrentOperator {
                grant: OperatorGrant {
                    principal: &PRINCIPAL,
                    scope: &self.scope,
                    permission: self.operator_permission,
                    version: &1,
                    active: true,
                    issued_at_seconds: 0,
                    expires_at_seconds: 200,
                },
                access: self.access(),
                now_seconds: 100,
                independently_authenticated: true,
                mfa_verified: true,
                reason_ticket_verified: true,
                exceptional_audit_approved: false,
                private_rights_authorized: false,
                debug_enabled: false,
            }
        })))
    }

    fn with_export<R>(
        &mut self,
        _request: &ExportRequest<'_, Self>,
        publish: impl FnOnce(Option<CurrentExport<'_, Self>>) -> R,
    ) -> Result<R, Self::Error> {
        Ok(publish(self.private_grant_present.then(|| CurrentExport {
            grant: ExportGrant {
                principal: &PRINCIPAL,
                scope: &self.scope,
                recipient: &RECIPIENT,
                audience: &AUDIENCE,
                export_identity: &EXPORT_IDENTITY,
                permitted_item: ExportItem::Fact(&FACT),
                source_version: &SOURCE_VERSION,
                access_version: &self.private_access_version,
                rights_version: &RIGHTS_VERSION,
                active: self.private_grant_active,
                expires_at_seconds: 200,
                consented: self.consented,
                capture_contributor_rights: true,
                source_rights_and_attribution: self.source_rights,
            },
            access: self.access(),
            now_seconds: 100,
            export_enabled: self.export_enabled,
        })))
    }
}

fn access_basis() -> AccessBasis<'static, CurrentPrivateAuthority> {
    AccessBasis {
        credential: &CREDENTIAL,
        credential_generation: &GENERATION,
        binding_generation: &GENERATION,
    }
}

fn export_request(scope: &Scope) -> ExportRequest<'_, CurrentPrivateAuthority> {
    ExportRequest {
        principal: &PRINCIPAL,
        scope,
        recipient: &RECIPIENT,
        audience: &AUDIENCE,
        export_identity: &EXPORT_IDENTITY,
        item: ExportItem::Fact(&FACT),
        source_version: &SOURCE_VERSION,
        access_version: &ACCESS_VERSION,
        rights_version: &RIGHTS_VERSION,
        access: access_basis(),
    }
}

fn operator_request(
    scope: &Scope,
    permission: OperatorPermission,
) -> OperatorRequest<'_, CurrentPrivateAuthority> {
    OperatorRequest {
        principal: &PRINCIPAL,
        scope,
        permission,
        grant_version: &1,
        access: access_basis(),
    }
}

fn disclosed_bytes(
    authority: &mut CurrentPrivateAuthority,
    request: &ExportRequest<'_, CurrentPrivateAuthority>,
) -> (Result<(), PermissionError<(), ()>>, Vec<u8>) {
    let mut bytes = Vec::new();
    let result = publish_export(authority, request, || {
        bytes.extend_from_slice(b"private fact");
        Ok::<_, ()>(())
    });
    (result, bytes)
}

#[test]
fn verified_payment_and_current_membership_do_not_issue_private_export_grants() {
    let settled = observe_subscription(pending_subscription(), settled_observation(), 100).unwrap();
    assert_eq!(settled.next.access, Access::Active);
    assert_eq!(settled.allowance, Some(Money::new(currency(), 20)));
    assert_eq!(effective_access(settled.next, 100), Access::Active);

    let mut membership = CurrentMembership {
        revision: 1,
        active: true,
    };
    let checked = authorize_membership(
        &mut membership,
        MembershipRequest {
            principal: &PRINCIPAL,
            tenant: &TENANT,
            campaign: &CAMPAIGN,
            role: &1,
        },
        100,
    )
    .unwrap();
    assert_eq!(
        revalidate_membership(&mut membership, &checked, 100),
        Ok(())
    );

    let mut authority = CurrentPrivateAuthority::default();
    let requested_scope = scope();
    let request = export_request(&requested_scope);
    assert_eq!(
        disclosed_bytes(&mut authority, &request),
        (
            Err(PermissionError::Refused(PermissionRefusal::MissingGrant)),
            Vec::new()
        )
    );
    authority.private_grant_present = true;
    assert_eq!(
        disclosed_bytes(&mut authority, &request),
        (Ok(()), b"private fact".to_vec())
    );
    let payer_request = ExportRequest {
        principal: &PAYER,
        ..export_request(&requested_scope)
    };
    assert_eq!(
        disclosed_bytes(&mut authority, &payer_request).0,
        Err(PermissionError::Refused(PermissionRefusal::WrongPrincipal))
    );
}

#[test]
fn cancellation_and_refund_candidates_preserve_decisions_and_never_expand_private_rights() {
    let paid = paid_subscription();
    let committed_decision_revision = 42;
    let mut authority = CurrentPrivateAuthority {
        private_grant_present: true,
        ..CurrentPrivateAuthority::default()
    };
    let requested_scope = scope();
    let request = export_request(&requested_scope);

    let (period_end, receipt) = governed_candidate(paid, LifecycleObservation::CancelAtPeriodEnd);
    assert_eq!(receipt, CandidateReceipt::ProposedOnly);
    assert_eq!(effective_access(period_end, 199), Access::Active);
    assert_eq!(effective_access(period_end, 200), Access::ReadOnly);
    assert_eq!(committed_decision_revision, 42);

    for observation in [
        LifecycleObservation::CancelImmediate,
        LifecycleObservation::RefundConfirmedEndingEntitlement,
    ] {
        let (restricted, receipt) = governed_candidate(paid, observation);
        assert_eq!(receipt, CandidateReceipt::ProposedOnly);
        assert_eq!(effective_access(restricted, 100), Access::Restricted);
        assert_eq!(committed_decision_revision, 42);
        authority.private_grant_active = false;
        assert_eq!(
            disclosed_bytes(&mut authority, &request),
            (
                Err(PermissionError::Refused(PermissionRefusal::Revoked)),
                Vec::new()
            )
        );
    }
    let (unknown, receipt) = governed_candidate(paid, LifecycleObservation::RefundUnknown);
    assert_eq!(unknown, paid);
    assert_eq!(receipt, CandidateReceipt::PendingReconciliation);
    assert_eq!(
        observe_subscription(
            paid,
            SubscriptionObservation {
                authority: ObservationAuthority::VerifiedLatest,
                expected_revision: paid.revision,
                object_revision: paid.object_revision + 1,
                outcome: PaymentOutcome::Unknown,
            },
            100,
        ),
        Err(SubscriptionRefusal::UnknownPayment)
    );
}

#[test]
fn cancelled_or_unknown_payment_cannot_release_unknown_supplier_exposure() {
    let money = |micros| Money::new(currency(), micros);
    let operation = OperationId::from_bytes(&[17; 16]).unwrap();
    let snapshot = SpendSnapshot {
        currency: currency(),
        revision: 4,
        counters: [SpendCounter {
            used: money(90),
            limit: money(100),
        }; 6],
    };
    let request = SpendRequest {
        operation,
        expected_revision: 4,
        expected_consent_revision: 2,
        maximum_supplier_liability: money(5),
    };
    let consent = SpendConsent {
        revision: 2,
        maximum: money(5),
    };
    let (cancelled, _) =
        governed_candidate(paid_subscription(), LifecycleObservation::CancelImmediate);
    assert_eq!(cancelled.access, Access::Restricted);
    assert_eq!(
        propose_spend(
            &snapshot,
            consent,
            request,
            SpendOperationObservation {
                operation,
                status: SpendOperationStatus::Unknown,
            },
        ),
        Err(SpendRefusal::UnknownOperation)
    );
    assert_eq!(
        snapshot.counters.map(|counter| counter.used),
        [money(90); 6]
    );
}

#[test]
fn deletion_suppression_and_old_access_versions_cannot_reopen_private_export() {
    let paid = paid_subscription();
    let requested_scope = scope();
    let request = export_request(&requested_scope);
    let mut authority = CurrentPrivateAuthority {
        private_grant_present: true,
        ..CurrentPrivateAuthority::default()
    };
    assert_eq!(disclosed_bytes(&mut authority, &request).0, Ok(()));

    // A test-owned current suppression fact denies before physical cleanup.
    // No durable tombstone, backup restore, or deletion receipt is simulated.
    authority.private_grant_active = false;
    authority.private_access_version += 1;
    assert_eq!(effective_access(paid, 100), Access::Active);
    assert_eq!(
        disclosed_bytes(&mut authority, &request),
        (
            Err(PermissionError::Refused(PermissionRefusal::Revoked)),
            Vec::new()
        )
    );
    authority.private_grant_active = true;
    assert_eq!(
        disclosed_bytes(&mut authority, &request).0,
        Err(PermissionError::Refused(
            PermissionRefusal::StaleAccessVersion
        ))
    );
    authority.private_access_version = ACCESS_VERSION;
    authority.consented = false;
    assert_eq!(
        disclosed_bytes(&mut authority, &request).0,
        Err(PermissionError::Refused(PermissionRefusal::ConsentRequired))
    );
    authority.consented = true;
    authority.source_rights = false;
    assert_eq!(
        disclosed_bytes(&mut authority, &request).0,
        Err(PermissionError::Refused(PermissionRefusal::RightsRequired))
    );
}

#[test]
fn refund_and_deletion_operator_permissions_are_distinct_from_private_export() {
    let requested_scope = scope();
    let mut authority = CurrentPrivateAuthority {
        operator_grant_present: true,
        ..CurrentPrivateAuthority::default()
    };
    let mut handler_calls = 0;
    assert_eq!(
        invoke_operator(
            &mut authority,
            &operator_request(&requested_scope, OperatorPermission::Refund),
            || {
                handler_calls += 1;
                Ok::<_, ()>(())
            },
        ),
        Ok(())
    );
    assert_eq!(handler_calls, 1);
    assert_eq!(
        invoke_operator(
            &mut authority,
            &operator_request(&requested_scope, OperatorPermission::Deletion),
            || {
                handler_calls += 1;
                Ok::<_, ()>(())
            },
        ),
        Err(PermissionError::Refused(PermissionRefusal::WrongPermission))
    );
    assert_eq!(handler_calls, 1);
    assert_eq!(
        disclosed_bytes(&mut authority, &export_request(&requested_scope)).0,
        Err(PermissionError::Refused(PermissionRefusal::MissingGrant))
    );
}

#[test]
fn source_unavailability_and_revocation_remain_explicit_after_paid_observation() {
    let paid = paid_subscription();
    assert_eq!(effective_access(paid, 100), Access::Active);
    let mut membership = CurrentMembership {
        revision: 1,
        active: true,
    };
    let checked = authorize_membership(
        &mut membership,
        MembershipRequest {
            principal: &PRINCIPAL,
            tenant: &TENANT,
            campaign: &CAMPAIGN,
            role: &1,
        },
        100,
    )
    .unwrap();
    membership.revision = 2;
    assert_eq!(
        revalidate_membership(&mut membership, &checked, 100),
        Err(MembershipError::StaleRevision)
    );
    membership.active = false;
    assert_eq!(
        revalidate_membership(&mut membership, &checked, 100),
        Err(MembershipError::Revoked)
    );
    let mut authority = CurrentPrivateAuthority {
        private_grant_present: true,
        export_enabled: false,
        ..CurrentPrivateAuthority::default()
    };
    let requested_scope = scope();
    assert_eq!(
        disclosed_bytes(&mut authority, &export_request(&requested_scope)).0,
        Err(PermissionError::Refused(PermissionRefusal::ExportDisabled))
    );
}
