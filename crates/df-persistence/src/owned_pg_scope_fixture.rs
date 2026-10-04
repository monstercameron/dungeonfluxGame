//! Fixture-only scope construction from actual checked DB membership and root-owned proof row.
//! The fixture admin issues only its registered canonical input; runtime callers cannot seed rows.
use crate::native_scope::{DatabaseBindingVerifier, NativeScope};
use crate::owned_pg_membership_fixture::{FixtureMembershipAuthority, fixture_key_mapping};
use df_auth::membership::MembershipCapability;
use df_model::checkpoint::{CheckpointPins, ExecutionMode, GameInput};
use df_session::submission::RepositoryError;
use df_types::{OperationId, RecoveryEpoch, SessionId};
use tokio_postgres::Row;

pub(crate) struct FixtureBoundInput {
    pub(crate) input: GameInput,
    pub(crate) pins: CheckpointPins,
}

/// `proof` is selected by the registered fixture administrator from its private scope_proofs
/// row, never a runtime/client-provided Row. The full proof is reverified by DB on every call.
pub(crate) fn fixture_scope(
    capability: MembershipCapability<FixtureMembershipAuthority>,
    proof: &Row,
    bound: FixtureBoundInput,
) -> Result<NativeScope<FixtureMembershipAuthority>, RepositoryError> {
    fn bytes(row: &Row, column: &str, length: usize) -> Result<Vec<u8>, RepositoryError> {
        let value: Vec<u8> = row
            .try_get(column)
            .map_err(|_| RepositoryError::Unauthorized)?;
        if value.len() != length {
            return Err(RepositoryError::Unauthorized);
        }
        Ok(value)
    }
    fn identity(row: &Row, column: &str) -> Result<[u8; 16], RepositoryError> {
        let value: [u8; 16] = bytes(row, column, 16)?
            .try_into()
            .map_err(|_| RepositoryError::Unauthorized)?;
        if value == [0; 16] {
            return Err(RepositoryError::Unauthorized);
        }
        Ok(value)
    }
    let binding = bytes(proof, "binding", 32)?;
    let tenant = identity(proof, "tenant_id")?;
    let principal = identity(proof, "principal_id")?;
    let campaign = identity(proof, "campaign_id")?;
    let effective_role: Vec<u8> = proof
        .try_get("effective_role")
        .map_err(|_| RepositoryError::Unauthorized)?;
    let revision: Vec<u8> = proof
        .try_get("access_revision")
        .map_err(|_| RepositoryError::Unauthorized)?;
    let request = capability.request();
    if tenant != request.tenant.0
        || principal != request.principal.0
        || campaign != request.campaign.0
        || effective_role != request.role.0
        || revision != capability.revision().0
    {
        return Err(RepositoryError::Unauthorized);
    }
    let session = SessionId::from_bytes(&identity(proof, "session_id")?)
        .map_err(|_| RepositoryError::Unauthorized)?;
    let operation = OperationId::from_bytes(&identity(proof, "operation_id")?)
        .map_err(|_| RepositoryError::Unauthorized)?;
    let namespace: Vec<u8> = proof
        .try_get("command_namespace")
        .map_err(|_| RepositoryError::Unauthorized)?;
    if namespace.is_empty() || namespace.len() > 128 {
        return Err(RepositoryError::Unauthorized);
    }
    let epoch: String = proof
        .try_get("recovery_epoch")
        .map_err(|_| RepositoryError::Unauthorized)?;
    let epoch = RecoveryEpoch::new(
        crate::revision_codec::decode_unsigned_number(&epoch)
            .map_err(|_| RepositoryError::Unauthorized)?,
    )
    .map_err(|_| RepositoryError::Unauthorized)?;
    let fingerprint_version: i32 = proof
        .try_get("fingerprint_version")
        .map_err(|_| RepositoryError::Unauthorized)?;
    if fingerprint_version != 1 {
        return Err(RepositoryError::Unauthorized);
    }
    let fingerprint = bytes(proof, "canonical_fingerprint", 32)?
        .try_into()
        .map_err(|_| RepositoryError::Unauthorized)?;
    let mode = match proof
        .try_get::<_, i16>("execution_mode")
        .map_err(|_| RepositoryError::Unauthorized)?
    {
        1 => ExecutionMode::Live,
        2 => ExecutionMode::PreparedOnly,
        3 => ExecutionMode::Replay,
        _ => return Err(RepositoryError::Unauthorized),
    };
    Ok(NativeScope {
        capability,
        mapping: fixture_key_mapping(),
        binding,
        tenant,
        principal,
        session,
        operation,
        namespace,
        epoch,
        fingerprint,
        owner_fence: identity(proof, "owner_fence")?,
        lookup_only: proof
            .try_get("lookup_only")
            .map_err(|_| RepositoryError::Unauthorized)?,
        bound_input: bound.input,
        pins: bound.pins,
        mode,
    })
}

pub(crate) async fn fixture_verifier(
    client: &tokio_postgres::Client,
    deadline: tokio::time::Instant,
) -> Result<DatabaseBindingVerifier, RepositoryError> {
    let statement = tokio::time::timeout_at(
        deadline,
        client.prepare("SELECT * FROM df_fixture_authority.bind_scope($1::bytea, $2::bytea)"),
    )
    .await
    .map_err(|_| RepositoryError::Unavailable)?
    .map_err(|_| RepositoryError::Unavailable)?;
    Ok(DatabaseBindingVerifier {
        statement: Some(statement),
        maximum_binding_bytes: 32,
        maximum_authority_value_bytes: 128,
        maximum_namespace_bytes: 128,
    })
}
