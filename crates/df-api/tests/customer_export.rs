#![cfg(not(target_arch = "wasm32"))]

//! Native-only cases for the existing auth and commerce policy boundary.
//! The controlled authority is a test fixture, not an authenticator or production
//! grant issuer. Commerce settlement facts never create its separate export grant.

use df_auth::permissions::{
    AccessBasis, CurrentAccess, CurrentExport, CurrentOperator, ExportGrant, ExportItem,
    ExportRequest, OperatorRequest, PermissionAuthority, PermissionError, PermissionRefusal,
    PermissionScope, publish_export,
};
use df_auth::rotation::CredentialFence;
use df_commerce::{
    Access, ObservationAuthority, PaidInvoice, PaymentOutcome, SubscriptionObservation,
    SubscriptionState, effective_access, observe_subscription,
};
use df_types::{Currency, Money, SessionId};

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
const MEDIA: u64 = 12;
const SOURCE_VERSION: u64 = 13;
const ACCESS_VERSION: u64 = 14;
const RIGHTS_VERSION: u64 = 15;

type Scope = PermissionScope<u64, u64>;

fn scope(tenant: u64) -> Scope {
    scope_with_session(tenant, 16)
}

fn scope_with_session(tenant: u64, session_byte: u8) -> Scope {
    Scope {
        tenant,
        campaign: CAMPAIGN,
        session: SessionId::from_bytes(&[session_byte; 16]).unwrap(),
    }
}

fn currency() -> Currency {
    Currency::parse("USD").unwrap()
}

fn settled_entitlement() -> SubscriptionState {
    let pending = SubscriptionState {
        currency: currency(),
        revision: 1,
        object_revision: 1,
        access: Access::PendingInitial,
        paid_through: 0,
        grace_until: None,
    };
    let observation = SubscriptionObservation {
        authority: ObservationAuthority::VerifiedLatest,
        expected_revision: 1,
        object_revision: 2,
        outcome: PaymentOutcome::Settled(PaidInvoice {
            period_start: 90,
            paid_through: 200,
            selected_allowance: Money::new(currency(), 20),
            allowance_already_recorded: false,
        }),
    };
    let transition = observe_subscription(pending, observation, 100).unwrap();
    assert_eq!(transition.next.access, Access::Active);
    assert_eq!(effective_access(transition.next, 100), Access::Active);
    transition.next
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ItemKind {
    Fact,
    Media,
}

struct CurrentGrantAuthority {
    grant_scope: Scope,
    credential_fence: CredentialFence<u64>,
    binding_generation: u64,
    binding_active: bool,
    credential_unexpired: bool,
    binding_unexpired: bool,
    grant_present: bool,
    grant_principal: u64,
    grant_recipient: u64,
    grant_audience: u64,
    grant_export_identity: u64,
    grant_item: ItemKind,
    source_version: u64,
    access_version: u64,
    rights_version: u64,
    active: bool,
    expires_at: u64,
    consented: bool,
    contributor_rights: bool,
    source_rights: bool,
    now: u64,
    export_enabled: bool,
}

impl Default for CurrentGrantAuthority {
    fn default() -> Self {
        Self {
            grant_scope: scope(TENANT),
            credential_fence: CredentialFence::new(GENERATION),
            binding_generation: GENERATION,
            binding_active: true,
            credential_unexpired: true,
            binding_unexpired: true,
            grant_present: false,
            grant_principal: PRINCIPAL,
            grant_recipient: RECIPIENT,
            grant_audience: AUDIENCE,
            grant_export_identity: EXPORT_IDENTITY,
            grant_item: ItemKind::Fact,
            source_version: SOURCE_VERSION,
            access_version: ACCESS_VERSION,
            rights_version: RIGHTS_VERSION,
            active: true,
            expires_at: 200,
            consented: true,
            contributor_rights: true,
            source_rights: true,
            now: 100,
            export_enabled: true,
        }
    }
}

impl CurrentGrantAuthority {
    fn access(&self) -> CurrentAccess<'_, Self> {
        CurrentAccess {
            principal: &PRINCIPAL,
            credential: &CREDENTIAL,
            fence: &self.credential_fence,
            credential_unexpired: self.credential_unexpired,
            binding_generation: &self.binding_generation,
            binding_active: self.binding_active,
            binding_unexpired: self.binding_unexpired,
        }
    }
}

impl PermissionAuthority for CurrentGrantAuthority {
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
        Ok(invoke(None))
    }

    fn with_export<R>(
        &mut self,
        _request: &ExportRequest<'_, Self>,
        publish: impl FnOnce(Option<CurrentExport<'_, Self>>) -> R,
    ) -> Result<R, Self::Error> {
        let current = self.grant_present.then(|| CurrentExport {
            grant: ExportGrant {
                principal: &self.grant_principal,
                scope: &self.grant_scope,
                recipient: &self.grant_recipient,
                audience: &self.grant_audience,
                export_identity: &self.grant_export_identity,
                permitted_item: match self.grant_item {
                    ItemKind::Fact => ExportItem::Fact(&FACT),
                    ItemKind::Media => ExportItem::Media(&MEDIA),
                },
                source_version: &self.source_version,
                access_version: &self.access_version,
                rights_version: &self.rights_version,
                active: self.active,
                expires_at_seconds: self.expires_at,
                consented: self.consented,
                capture_contributor_rights: self.contributor_rights,
                source_rights_and_attribution: self.source_rights,
            },
            access: self.access(),
            now_seconds: self.now,
            export_enabled: self.export_enabled,
        });
        Ok(publish(current))
    }
}

fn access_basis() -> AccessBasis<'static, CurrentGrantAuthority> {
    AccessBasis {
        credential: &CREDENTIAL,
        credential_generation: &GENERATION,
        binding_generation: &GENERATION,
    }
}

fn export_request<'a>(
    scope: &'a Scope,
    principal: &'static u64,
    item: ItemKind,
) -> ExportRequest<'a, CurrentGrantAuthority> {
    ExportRequest {
        principal,
        scope,
        recipient: &RECIPIENT,
        audience: &AUDIENCE,
        export_identity: &EXPORT_IDENTITY,
        item: match item {
            ItemKind::Fact => ExportItem::Fact(&FACT),
            ItemKind::Media => ExportItem::Media(&MEDIA),
        },
        source_version: &SOURCE_VERSION,
        access_version: &ACCESS_VERSION,
        rights_version: &RIGHTS_VERSION,
        access: access_basis(),
    }
}

fn publish_result(
    authority: &mut CurrentGrantAuthority,
    request: &ExportRequest<'_, CurrentGrantAuthority>,
) -> (Result<(), PermissionError<(), ()>>, usize) {
    let mut publication_calls = 0;
    let result = publish_export(authority, request, || {
        publication_calls += 1;
        Ok::<_, ()>(())
    });
    (result, publication_calls)
}

#[test]
fn settled_payment_and_active_entitlement_do_not_issue_private_export_grant() {
    let entitlement = settled_entitlement();
    assert_eq!(effective_access(entitlement, 100), Access::Active);

    let mut authority = CurrentGrantAuthority::default();
    let current_scope = scope(TENANT);
    let request = export_request(&current_scope, &PRINCIPAL, ItemKind::Fact);
    assert_eq!(
        publish_result(&mut authority, &request),
        (
            Err(PermissionError::Refused(PermissionRefusal::MissingGrant)),
            0
        )
    );
}

#[test]
fn current_export_grant_allows_only_its_exact_fact_or_media_item() {
    let _entitlement = settled_entitlement();
    let mut authority = CurrentGrantAuthority {
        grant_present: true,
        ..CurrentGrantAuthority::default()
    };
    let current_scope = scope(TENANT);
    let fact = export_request(&current_scope, &PRINCIPAL, ItemKind::Fact);
    assert_eq!(publish_result(&mut authority, &fact), (Ok(()), 1));

    authority.grant_item = ItemKind::Media;
    let media = export_request(&current_scope, &PRINCIPAL, ItemKind::Media);
    assert_eq!(publish_result(&mut authority, &media), (Ok(()), 1));
    assert_eq!(
        publish_result(&mut authority, &fact),
        (
            Err(PermissionError::Refused(PermissionRefusal::OutsideRange)),
            0
        )
    );
}

#[test]
fn payer_cannot_use_another_principals_current_private_export_grant() {
    let _entitlement = settled_entitlement();
    let mut authority = CurrentGrantAuthority {
        grant_present: true,
        ..CurrentGrantAuthority::default()
    };
    let current_scope = scope(TENANT);
    let request = export_request(&current_scope, &PAYER, ItemKind::Fact);

    assert_eq!(
        publish_result(&mut authority, &request),
        (
            Err(PermissionError::Refused(PermissionRefusal::WrongPrincipal)),
            0
        )
    );
}

#[test]
fn stale_revoked_wrong_scope_and_unconsented_grants_refuse_before_publication() {
    let current_scope = scope(TENANT);
    let request = export_request(&current_scope, &PRINCIPAL, ItemKind::Fact);
    let cases = [
        (
            "missing grant",
            CurrentGrantAuthority::default(),
            PermissionRefusal::MissingGrant,
        ),
        (
            "revoked grant",
            CurrentGrantAuthority {
                grant_present: true,
                active: false,
                ..CurrentGrantAuthority::default()
            },
            PermissionRefusal::Revoked,
        ),
        (
            "expired grant",
            CurrentGrantAuthority {
                grant_present: true,
                expires_at: 100,
                ..CurrentGrantAuthority::default()
            },
            PermissionRefusal::Expired,
        ),
        (
            "wrong tenant scope",
            CurrentGrantAuthority {
                grant_present: true,
                grant_scope: scope(TENANT + 1),
                ..CurrentGrantAuthority::default()
            },
            PermissionRefusal::WrongScope,
        ),
        (
            "wrong session scope",
            CurrentGrantAuthority {
                grant_present: true,
                grant_scope: scope_with_session(TENANT, 17),
                ..CurrentGrantAuthority::default()
            },
            PermissionRefusal::WrongScope,
        ),
        (
            "wrong recipient",
            CurrentGrantAuthority {
                grant_present: true,
                grant_recipient: RECIPIENT + 1,
                ..CurrentGrantAuthority::default()
            },
            PermissionRefusal::WrongRecipient,
        ),
        (
            "wrong audience",
            CurrentGrantAuthority {
                grant_present: true,
                grant_audience: AUDIENCE + 1,
                ..CurrentGrantAuthority::default()
            },
            PermissionRefusal::WrongAudience,
        ),
        (
            "wrong export identity",
            CurrentGrantAuthority {
                grant_present: true,
                grant_export_identity: EXPORT_IDENTITY + 1,
                ..CurrentGrantAuthority::default()
            },
            PermissionRefusal::WrongExportIdentity,
        ),
        (
            "stale source revision",
            CurrentGrantAuthority {
                grant_present: true,
                source_version: SOURCE_VERSION + 1,
                ..CurrentGrantAuthority::default()
            },
            PermissionRefusal::StaleSourceVersion,
        ),
        (
            "stale access revision",
            CurrentGrantAuthority {
                grant_present: true,
                access_version: ACCESS_VERSION + 1,
                ..CurrentGrantAuthority::default()
            },
            PermissionRefusal::StaleAccessVersion,
        ),
        (
            "stale rights revision",
            CurrentGrantAuthority {
                grant_present: true,
                rights_version: RIGHTS_VERSION + 1,
                ..CurrentGrantAuthority::default()
            },
            PermissionRefusal::StaleRightsVersion,
        ),
        (
            "missing consent",
            CurrentGrantAuthority {
                grant_present: true,
                consented: false,
                ..CurrentGrantAuthority::default()
            },
            PermissionRefusal::ConsentRequired,
        ),
        (
            "missing contributor rights",
            CurrentGrantAuthority {
                grant_present: true,
                contributor_rights: false,
                ..CurrentGrantAuthority::default()
            },
            PermissionRefusal::RightsRequired,
        ),
        (
            "missing source rights",
            CurrentGrantAuthority {
                grant_present: true,
                source_rights: false,
                ..CurrentGrantAuthority::default()
            },
            PermissionRefusal::RightsRequired,
        ),
        (
            "export disabled",
            CurrentGrantAuthority {
                grant_present: true,
                export_enabled: false,
                ..CurrentGrantAuthority::default()
            },
            PermissionRefusal::ExportDisabled,
        ),
        (
            "rotated credential generation",
            {
                let mut authority = CurrentGrantAuthority {
                    grant_present: true,
                    ..CurrentGrantAuthority::default()
                };
                authority.credential_fence.rotate(GENERATION + 1).unwrap();
                authority
            },
            PermissionRefusal::CredentialRotated,
        ),
        (
            "expired credential",
            CurrentGrantAuthority {
                grant_present: true,
                credential_unexpired: false,
                ..CurrentGrantAuthority::default()
            },
            PermissionRefusal::CredentialExpired,
        ),
        (
            "revoked binding",
            CurrentGrantAuthority {
                grant_present: true,
                binding_active: false,
                ..CurrentGrantAuthority::default()
            },
            PermissionRefusal::BindingRevoked,
        ),
        (
            "expired binding",
            CurrentGrantAuthority {
                grant_present: true,
                binding_unexpired: false,
                ..CurrentGrantAuthority::default()
            },
            PermissionRefusal::BindingExpired,
        ),
        (
            "replaced binding generation",
            CurrentGrantAuthority {
                grant_present: true,
                binding_generation: GENERATION + 1,
                ..CurrentGrantAuthority::default()
            },
            PermissionRefusal::BindingReplaced,
        ),
    ];

    for (case_id, mut authority, refusal) in cases {
        let (result, calls) = publish_result(&mut authority, &request);
        assert_eq!(result, Err(PermissionError::Refused(refusal)), "{case_id}");
        assert_eq!(calls, 0, "{case_id}");
    }
}
