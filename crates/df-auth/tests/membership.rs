use df_auth::membership::{
    MembershipAuthority, MembershipError, MembershipRecord, MembershipRequest,
    authorize_membership, revalidate_membership,
};

// Synthetic domain identities belong only to this adapter fixture. Production
// consumers supply their actual domain types; integers cannot enter the API.
#[derive(Eq, PartialEq)]
struct Principal(u8);
#[derive(Eq, PartialEq)]
struct Tenant(u8);
#[derive(Eq, PartialEq)]
struct Campaign(u8);
#[derive(Eq, PartialEq)]
enum Role {
    Player,
    SharedDisplay,
}
#[derive(Eq, PartialEq)]
struct Revision(u64);
#[derive(Debug, Eq, PartialEq)]
enum SourceError {
    Unavailable,
}

struct Authority {
    record: Option<MembershipRecord<Self>>,
    unavailable: bool,
    reads: usize,
    lookups: Vec<(u8, u8, u8, bool)>,
}

impl MembershipAuthority for Authority {
    type Principal = Principal;
    type Tenant = Tenant;
    type Campaign = Campaign;
    type Role = Role;
    type Revision = Revision;
    type Error = SourceError;

    fn read_current(
        &mut self,
        request: MembershipRequest<'_, Self>,
    ) -> Result<Option<MembershipRecord<Self>>, Self::Error> {
        self.reads += 1;
        self.lookups.push((
            request.principal.0,
            request.tenant.0,
            request.campaign.0,
            *request.role == Role::Player,
        ));
        if self.unavailable {
            return Err(SourceError::Unavailable);
        }
        // Copy the authority's current facts rather than request-selected facts.
        Ok(self.record.as_ref().map(|record| MembershipRecord {
            principal: Principal(record.principal.0),
            tenant: Tenant(record.tenant.0),
            campaign: Campaign(record.campaign.0),
            role: if record.role == Role::Player {
                Role::Player
            } else {
                Role::SharedDisplay
            },
            revision: Revision(record.revision.0),
            active: record.active,
            expires_at: record.expires_at,
        }))
    }
}

fn authority() -> Authority {
    Authority {
        record: Some(MembershipRecord {
            principal: Principal(2),
            tenant: Tenant(4),
            campaign: Campaign(7),
            role: Role::Player,
            revision: Revision(9),
            active: true,
            expires_at: Some(100),
        }),
        unavailable: false,
        reads: 0,
        lookups: Vec::new(),
    }
}

fn request() -> MembershipRequest<'static, Authority> {
    MembershipRequest {
        principal: &Principal(2),
        tenant: &Tenant(4),
        campaign: &Campaign(7),
        role: &Role::Player,
    }
}

fn denial(
    authority: &mut Authority,
    request: MembershipRequest<'_, Authority>,
    now: u64,
) -> MembershipError<SourceError> {
    match authorize_membership(authority, request, now) {
        Ok(_) => panic!("invalid grant must not issue a capability"),
        Err(error) => error,
    }
}

#[test]
fn current_matching_scope_issues_exact_checked_basis_and_rereads() {
    let mut source = authority();
    let checked = authorize_membership(&mut source, request(), 99).unwrap();
    assert!(checked.request().principal == &Principal(2));
    assert!(checked.request().tenant == &Tenant(4));
    assert!(checked.request().campaign == &Campaign(7));
    assert!(*checked.request().role == Role::Player);
    assert!(checked.revision() == &Revision(9));
    assert_eq!(revalidate_membership(&mut source, &checked, 99), Ok(()));
    assert_eq!(source.reads, 2);
    assert_eq!(source.lookups, [(2, 4, 7, true), (2, 4, 7, true)]);
    assert!(source.record.as_ref().unwrap().revision == Revision(9));
}

#[test]
fn each_supplied_scope_and_role_is_checked_against_authority() {
    let mut source = authority();
    assert_eq!(
        denial(
            &mut source,
            MembershipRequest {
                principal: &Principal(3),
                ..request()
            },
            99
        ),
        MembershipError::WrongPrincipal,
    );
    assert_eq!(
        denial(
            &mut source,
            MembershipRequest {
                tenant: &Tenant(5),
                ..request()
            },
            99
        ),
        MembershipError::WrongTenant,
    );
    assert_eq!(
        denial(
            &mut source,
            MembershipRequest {
                campaign: &Campaign(8),
                ..request()
            },
            99
        ),
        MembershipError::WrongCampaign,
    );
    assert_eq!(
        denial(
            &mut source,
            MembershipRequest {
                role: &Role::SharedDisplay,
                ..request()
            },
            99
        ),
        MembershipError::WrongRole,
    );
    assert_eq!(source.reads, 4);
    let record = source.record.as_ref().unwrap();
    assert!(record.principal == Principal(2));
    assert!(record.tenant == Tenant(4));
    assert!(record.campaign == Campaign(7));
    assert!(record.role == Role::Player);
}

#[test]
fn absence_revocation_expiry_and_source_failure_are_distinct() {
    let mut source = authority();
    source.unavailable = true;
    assert_eq!(
        denial(&mut source, request(), 0),
        MembershipError::Source(SourceError::Unavailable)
    );
    source.unavailable = false;
    assert_eq!(
        denial(&mut source, request(), 100),
        MembershipError::Expired
    );
    source.record.as_mut().unwrap().active = false;
    assert_eq!(denial(&mut source, request(), 0), MembershipError::Revoked);
    source.record = None;
    assert_eq!(denial(&mut source, request(), 0), MembershipError::Missing);
    assert_eq!(source.reads, 4);
}

#[test]
fn current_revision_role_and_status_are_rechecked_after_issuance() {
    let mut source = authority();
    let checked = authorize_membership(&mut source, request(), 0).unwrap();
    source.record.as_mut().unwrap().revision = Revision(10);
    assert_eq!(
        revalidate_membership(&mut source, &checked, 0),
        Err(MembershipError::StaleRevision)
    );
    source.record.as_mut().unwrap().revision = Revision(8);
    assert_eq!(
        revalidate_membership(&mut source, &checked, 0),
        Err(MembershipError::StaleRevision)
    );
    source.record.as_mut().unwrap().revision = Revision(9);
    source.record.as_mut().unwrap().role = Role::SharedDisplay;
    assert_eq!(
        revalidate_membership(&mut source, &checked, 0),
        Err(MembershipError::WrongRole)
    );
    source.record.as_mut().unwrap().role = Role::Player;
    source.record.as_mut().unwrap().active = false;
    assert_eq!(
        revalidate_membership(&mut source, &checked, 0),
        Err(MembershipError::Revoked)
    );
    source.record.as_mut().unwrap().active = true;
    assert_eq!(
        revalidate_membership(&mut source, &checked, 100),
        Err(MembershipError::Expired)
    );
    source.unavailable = true;
    assert_eq!(
        revalidate_membership(&mut source, &checked, 0),
        Err(MembershipError::Source(SourceError::Unavailable))
    );
    source.unavailable = false;
    source.record = None;
    assert_eq!(
        revalidate_membership(&mut source, &checked, 0),
        Err(MembershipError::Missing)
    );
    assert_eq!(source.reads, 8);
}

#[test]
fn revalidation_rejects_authority_returning_a_different_identity_scope() {
    let mut source = authority();
    let checked = authorize_membership(&mut source, request(), 0).unwrap();
    source.record.as_mut().unwrap().principal = Principal(3);
    assert_eq!(
        revalidate_membership(&mut source, &checked, 0),
        Err(MembershipError::WrongPrincipal)
    );
    source.record.as_mut().unwrap().principal = Principal(2);
    source.record.as_mut().unwrap().tenant = Tenant(5);
    assert_eq!(
        revalidate_membership(&mut source, &checked, 0),
        Err(MembershipError::WrongTenant)
    );
    source.record.as_mut().unwrap().tenant = Tenant(4);
    source.record.as_mut().unwrap().campaign = Campaign(8);
    assert_eq!(
        revalidate_membership(&mut source, &checked, 0),
        Err(MembershipError::WrongCampaign)
    );
    assert_eq!(source.reads, 4);
}

#[test]
fn caller_clock_boundaries_preserve_unlimited_and_expiring_grants() {
    let mut source = authority();
    source.record.as_mut().unwrap().expires_at = None;
    let checked = authorize_membership(&mut source, request(), u64::MAX).unwrap();
    assert_eq!(
        revalidate_membership(&mut source, &checked, u64::MAX),
        Ok(())
    );
    source.record.as_mut().unwrap().expires_at = Some(u64::MAX);
    assert!(authorize_membership(&mut source, request(), u64::MAX - 1).is_ok());
    assert_eq!(
        denial(&mut source, request(), u64::MAX),
        MembershipError::Expired
    );
    source.record.as_mut().unwrap().expires_at = Some(0);
    assert_eq!(denial(&mut source, request(), 0), MembershipError::Expired);
}
