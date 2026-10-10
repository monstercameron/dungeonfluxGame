use df_auth::membership::{
    MembershipAuthority, MembershipError, MembershipRecord, MembershipRequest,
    authorize_membership, revalidate_membership,
};
use df_auth::renewal::renew_membership;
use df_auth::rotation::{
    CredentialFence, CurrentPublicationState, PublicationAuthority, PublicationError,
    PublicationLease, PublicationRefusal, PublicationScope, publish_current,
};
use df_types::{ClientBindingId, SessionId};
use std::convert::Infallible;

#[derive(Clone, Eq, PartialEq)]
struct Principal(u8);
#[derive(Clone, Eq, PartialEq)]
struct Tenant(u8);
#[derive(Clone, Eq, PartialEq)]
struct Campaign(u8);
#[derive(Clone, Eq, PartialEq)]
enum Role {
    Player,
    SharedDisplay,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Revision(u8);
#[derive(Debug, Eq, PartialEq)]
enum SourceError {
    Unavailable,
}

struct MembershipSource {
    record: Option<MembershipRecord<Self>>,
    unavailable: bool,
    reads: usize,
}

impl MembershipAuthority for MembershipSource {
    type Principal = Principal;
    type Tenant = Tenant;
    type Campaign = Campaign;
    type Role = Role;
    type Revision = Revision;
    type Error = SourceError;

    fn read_current(
        &mut self,
        _request: MembershipRequest<'_, Self>,
    ) -> Result<Option<MembershipRecord<Self>>, Self::Error> {
        self.reads += 1;
        if self.unavailable {
            return Err(SourceError::Unavailable);
        }
        Ok(self.record.as_ref().map(|record| MembershipRecord {
            principal: record.principal.clone(),
            tenant: record.tenant.clone(),
            campaign: record.campaign.clone(),
            role: record.role.clone(),
            revision: record.revision.clone(),
            active: record.active,
            expires_at: record.expires_at,
        }))
    }
}

fn membership_source() -> MembershipSource {
    MembershipSource {
        record: Some(MembershipRecord {
            principal: Principal(1),
            tenant: Tenant(2),
            campaign: Campaign(3),
            role: Role::Player,
            revision: Revision(4),
            active: true,
            expires_at: Some(10),
        }),
        unavailable: false,
        reads: 0,
    }
}

fn request() -> MembershipRequest<'static, MembershipSource> {
    MembershipRequest {
        principal: &Principal(1),
        tenant: &Tenant(2),
        campaign: &Campaign(3),
        role: &Role::Player,
    }
}

fn expected_session() -> SessionId {
    SessionId::from_bytes(&[1; 16]).expect("fixture session identity is valid")
}

fn expected_binding() -> ClientBindingId {
    ClientBindingId::from_bytes(&[2; 16]).expect("fixture binding identity is valid")
}

#[test]
fn renewal_returns_a_fresh_immutable_snapshot_for_the_same_current_revision() {
    let mut source = membership_source();
    let original = authorize_membership(&mut source, request(), 5).unwrap();
    source.record.as_mut().unwrap().expires_at = Some(20);

    let renewed = renew_membership(&mut source, &original, 12).unwrap();

    assert_eq!(original.revision(), &Revision(4));
    assert_eq!(renewed.revision(), &Revision(4));
    assert_eq!(source.reads, 2);
    assert_eq!(revalidate_membership(&mut source, &original, 12), Ok(()));
    assert_eq!(source.reads, 3);
}

#[test]
fn changed_version_renews_to_current_snapshot_while_old_snapshot_stays_stale() {
    let mut source = membership_source();
    let original = authorize_membership(&mut source, request(), 5).unwrap();
    source.record.as_mut().unwrap().revision = Revision(5);

    let renewed = renew_membership(&mut source, &original, 5).unwrap();
    assert_eq!(renewed.revision(), &Revision(5));
    assert_eq!(
        revalidate_membership(&mut source, &original, 5),
        Err(MembershipError::StaleRevision)
    );
    assert_eq!(original.revision(), &Revision(4));
}

#[test]
fn revocation_expiry_missing_and_source_failure_remain_distinct() {
    let mut source = membership_source();
    let original = authorize_membership(&mut source, request(), 5).unwrap();

    source.record.as_mut().unwrap().active = false;
    assert_eq!(
        renew_membership(&mut source, &original, 5)
            .err()
            .expect("revoked record refuses renewal"),
        MembershipError::Revoked
    );
    source.record.as_mut().unwrap().active = true;
    assert_eq!(
        renew_membership(&mut source, &original, 10)
            .err()
            .expect("expiry boundary refuses renewal"),
        MembershipError::Expired
    );
    source.record = None;
    assert_eq!(
        renew_membership(&mut source, &original, 5)
            .err()
            .expect("missing record refuses renewal"),
        MembershipError::Missing
    );
    source.unavailable = true;
    assert_eq!(
        renew_membership(&mut source, &original, 5)
            .err()
            .expect("source failure refuses renewal"),
        MembershipError::Source(SourceError::Unavailable)
    );
}

#[test]
fn renewal_refuses_authority_returning_another_principal_tenant_campaign_or_role() {
    let mut source = membership_source();
    let original = authorize_membership(&mut source, request(), 5).unwrap();

    source.record.as_mut().unwrap().principal = Principal(9);
    assert_eq!(
        renew_membership(&mut source, &original, 5)
            .err()
            .expect("principal mismatch refuses renewal"),
        MembershipError::WrongPrincipal
    );
    source.record.as_mut().unwrap().principal = Principal(1);
    source.record.as_mut().unwrap().tenant = Tenant(9);
    assert_eq!(
        renew_membership(&mut source, &original, 5)
            .err()
            .expect("tenant mismatch refuses renewal"),
        MembershipError::WrongTenant
    );
    source.record.as_mut().unwrap().tenant = Tenant(2);
    source.record.as_mut().unwrap().campaign = Campaign(9);
    assert_eq!(
        renew_membership(&mut source, &original, 5)
            .err()
            .expect("campaign mismatch refuses renewal"),
        MembershipError::WrongCampaign
    );
    source.record.as_mut().unwrap().campaign = Campaign(3);
    source.record.as_mut().unwrap().role = Role::SharedDisplay;
    assert_eq!(
        renew_membership(&mut source, &original, 5)
            .err()
            .expect("role mismatch refuses renewal"),
        MembershipError::WrongRole
    );
}

#[derive(Eq, PartialEq)]
struct Audience(u8);
#[derive(Eq, PartialEq)]
struct Credential(u8);
#[derive(Eq, PartialEq)]
struct PublicationPrincipal(u8);

struct PublicationSource {
    scope: PublicationScope<PublicationPrincipal, Credential, Audience>,
    fence: CredentialFence<u8>,
    binding_generation: u8,
    audience_version: Revision,
    authorized: bool,
}

impl PublicationAuthority for PublicationSource {
    type Principal = PublicationPrincipal;
    type Credential = Credential;
    type Audience = Audience;
    type CredentialGeneration = u8;
    type BindingGeneration = u8;
    type AudienceVersion = Revision;
    type Error = Infallible;

    fn with_current<R>(
        &mut self,
        _scope: &PublicationScope<Self::Principal, Self::Credential, Self::Audience>,
        invoke: impl FnOnce(Option<CurrentPublicationState<'_, Self>>) -> R,
    ) -> Result<R, Self::Error> {
        Ok(invoke(Some(CurrentPublicationState {
            scope: &self.scope,
            credential: &self.fence,
            binding_generation: &self.binding_generation,
            binding_active: true,
            credential_unexpired: true,
            binding_unexpired: true,
            audience_version: &self.audience_version,
            audience_authorized: self.authorized,
        })))
    }
}

fn publication_source() -> PublicationSource {
    PublicationSource {
        scope: PublicationScope {
            audience: Audience(6),
            principal: PublicationPrincipal(1),
            credential: Credential(2),
            session: expected_session(),
            binding: expected_binding(),
        },
        fence: CredentialFence::new(3),
        binding_generation: 4,
        audience_version: Revision(4),
        authorized: true,
    }
}

fn publication_lease() -> PublicationLease<PublicationSource> {
    PublicationLease::new(
        PublicationScope {
            audience: Audience(6),
            principal: PublicationPrincipal(1),
            credential: Credential(2),
            session: expected_session(),
            binding: expected_binding(),
        },
        3,
        4,
        Revision(4),
    )
}

#[test]
fn changed_current_access_version_fences_publication_before_producing_bytes() {
    let mut source = publication_source();
    let lease = publication_lease();
    let mut produced = false;
    let permitted = publish_current(&mut source, &lease, || {
        produced = true;
        Ok::<_, ()>(())
    });
    assert_eq!(permitted, Ok(()));
    assert!(produced);

    source.audience_version = Revision(5);
    produced = false;
    assert_eq!(
        publish_current(&mut source, &lease, || {
            produced = true;
            Ok::<_, ()>(())
        }),
        Err(PublicationError::Refused(
            PublicationRefusal::AudienceChanged
        ))
    );
    assert!(!produced);

    source.audience_version = Revision(4);
    source.authorized = false;
    assert_eq!(
        publish_current(&mut source, &lease, || Ok::<_, ()>(())),
        Err(PublicationError::Refused(
            PublicationRefusal::AudienceDenied
        ))
    );
}
