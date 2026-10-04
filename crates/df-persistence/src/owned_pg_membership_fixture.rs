//! Fixture-only actual DB membership producer. The production issuer remains unqualified.
//! Its owned connection uses the injected existing runtime, joined driver and finite deadlines.
use crate::native_connection::{OwnedConnection, actor_block_on};
use crate::native_scope::AuthKeyMapping;
use df_auth::membership::{
    MembershipAuthority, MembershipCapability, MembershipRecord, MembershipRequest,
};
use df_observe::OperationContext;
use df_session::submission::RepositoryError;
use std::time::Duration;
use tokio::runtime::Handle;
use tokio::time::{Instant, timeout_at};
use tokio_postgres::Row;

#[derive(Eq, PartialEq)]
pub(crate) struct FixturePrincipal(pub(crate) [u8; 16]);
#[derive(Eq, PartialEq)]
pub(crate) struct FixtureTenant(pub(crate) [u8; 16]);
#[derive(Eq, PartialEq)]
pub(crate) struct FixtureCampaign(pub(crate) [u8; 16]);
#[derive(Eq, PartialEq)]
pub(crate) struct FixtureRole(pub(crate) Vec<u8>);
#[derive(Eq, PartialEq)]
pub(crate) struct FixtureRevision(pub(crate) Vec<u8>);

pub(crate) struct FixtureMembershipAuthority {
    pub(crate) runtime: Handle,
    pub(crate) connection: OwnedConnection,
    pub(crate) query_bound: Duration,
    pub(crate) context: OperationContext,
}
impl MembershipAuthority for FixtureMembershipAuthority {
    type Principal = FixturePrincipal;
    type Tenant = FixtureTenant;
    type Campaign = FixtureCampaign;
    type Role = FixtureRole;
    type Revision = FixtureRevision;
    type Error = RepositoryError;

    fn read_current(
        &mut self,
        request: MembershipRequest<'_, Self>,
    ) -> Result<Option<MembershipRecord<Self>>, Self::Error> {
        let runtime = self.runtime.clone();
        actor_block_on(&runtime, self.read_bounded(request))?
    }
}
impl FixtureMembershipAuthority {
    async fn read_bounded(
        &mut self,
        request: MembershipRequest<'_, Self>,
    ) -> Result<Option<MembershipRecord<Self>>, RepositoryError> {
        let mut span = df_observe::begin(&self.context, "persistence.fixture_membership_current");
        let result = self.read_current_row(request).await;
        span.finish_unmeasured(match &result {
            Ok(Some(_)) => "current_membership",
            Ok(None) => "membership_missing",
            Err(_) => "membership_unavailable",
        });
        result
    }

    async fn read_current_row(
        &mut self,
        request: MembershipRequest<'_, Self>,
    ) -> Result<Option<MembershipRecord<Self>>, RepositoryError> {
        if self.query_bound.is_zero() || request.role.0.is_empty() || request.role.0.len() > 128 {
            return Err(RepositoryError::Capacity);
        }
        let deadline = Instant::now()
            .checked_add(self.query_bound)
            .ok_or(RepositoryError::Capacity)?;
        let client = self.connection.take_client()?;
        let result = timeout_at(deadline, client.query_opt(
            "SELECT * FROM df_fixture_authority.read_membership($1::bytea, $2::bytea, $3::bytea, $4::bytea)",
            &[&request.tenant.0.as_slice(), &request.principal.0.as_slice(),
              &request.campaign.0.as_slice(), &request.role.0.as_slice()])).await;
        let row = match result {
            Ok(Ok(row)) => row,
            Ok(Err(_)) | Err(_) => {
                let mut span =
                    df_observe::begin(&self.context, "persistence.fixture_membership_discard");
                let closed = self.connection.discard().await;
                span.finish_unmeasured(if closed.is_ok() {
                    "driver_joined"
                } else {
                    "driver_join_pending"
                });
                return Err(RepositoryError::Unavailable);
            }
        };
        if self.connection.return_client(client).is_err() {
            self.connection.discard().await?;
            return Err(RepositoryError::Unavailable);
        }
        row.as_ref().map(decode_current_record).transpose()
    }

    pub(crate) fn into_owned_connection(self) -> OwnedConnection {
        self.connection
    }
}
fn decode_identity(row: &Row, column: &str) -> Result<[u8; 16], RepositoryError> {
    let bytes: Vec<u8> = row
        .try_get(column)
        .map_err(|_| RepositoryError::Unavailable)?;
    let bytes: [u8; 16] = bytes.try_into().map_err(|_| RepositoryError::Unavailable)?;
    if bytes == [0; 16] {
        return Err(RepositoryError::Unavailable);
    }
    Ok(bytes)
}
fn decode_label(row: &Row, column: &str) -> Result<Vec<u8>, RepositoryError> {
    let bytes: Vec<u8> = row
        .try_get(column)
        .map_err(|_| RepositoryError::Unavailable)?;
    if bytes.is_empty() || bytes.len() > 128 {
        return Err(RepositoryError::Unavailable);
    }
    Ok(bytes)
}
fn decode_current_record(
    row: &Row,
) -> Result<MembershipRecord<FixtureMembershipAuthority>, RepositoryError> {
    let expires: i64 = row
        .try_get("expires_unix")
        .map_err(|_| RepositoryError::Unavailable)?;
    Ok(MembershipRecord {
        principal: FixturePrincipal(decode_identity(row, "principal_id")?),
        tenant: FixtureTenant(decode_identity(row, "tenant_id")?),
        campaign: FixtureCampaign(decode_identity(row, "campaign_id")?),
        role: FixtureRole(decode_label(row, "effective_role")?),
        revision: FixtureRevision(decode_label(row, "access_revision")?),
        active: row
            .try_get("active")
            .map_err(|_| RepositoryError::Unavailable)?,
        expires_at: Some(u64::try_from(expires).map_err(|_| RepositoryError::Unavailable)?),
    })
}

pub(crate) fn fixture_key_mapping() -> AuthKeyMapping<FixtureMembershipAuthority> {
    fn tenant(value: &FixtureTenant) -> Result<[u8; 16], RepositoryError> {
        Ok(value.0)
    }
    fn principal(value: &FixturePrincipal) -> Result<[u8; 16], RepositoryError> {
        Ok(value.0)
    }
    fn campaign(value: &FixtureCampaign) -> Result<Vec<u8>, RepositoryError> {
        Ok(value.0.to_vec())
    }
    fn role(value: &FixtureRole) -> Result<Vec<u8>, RepositoryError> {
        Ok(value.0.clone())
    }
    fn revision(value: &FixtureRevision) -> Result<Vec<u8>, RepositoryError> {
        Ok(value.0.clone())
    }
    fn capacity(value: &MembershipCapability<FixtureMembershipAuthority>) -> Option<usize> {
        value
            .request()
            .role
            .0
            .capacity()
            .checked_add(value.revision().0.capacity())
    }
    AuthKeyMapping {
        tenant_bytes: tenant,
        principal_bytes: principal,
        campaign_bytes: campaign,
        role_bytes: role,
        revision_bytes: revision,
        retained_capability_heap_bytes: capacity,
    }
}
