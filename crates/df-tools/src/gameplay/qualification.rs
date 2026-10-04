//! Finite private consumer of generated requests against the actual local actor and PG.
//! Enabled only by the explicit qualification flag; no HTTP route or gameplay UI control.
use super::{Service, actor, hex, model};
use df_persistence::local_demo_scope::{PLAYER, TENANT};
use df_protocol::common as rpc;
use prost::Message;
use sha2::{Digest, Sha256};
use std::{io, time::Duration};
use tokio::sync::oneshot;
use tonic::Request;

const TRIGGER: &str = "/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/real-gameplay-demo-20261004/qualification-start-01";
const REPORT: &str = "/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/real-gameplay-demo-20261004/authority-acceptance-report-01.json";
fn required(condition: bool) -> Result<(), io::Error> {
    if condition {
        Ok(())
    } else {
        Err(io::Error::other("finite authority assertion failed"))
    }
}
fn request<T>(value: T, credential: [u8; 32]) -> Result<Request<T>, io::Error> {
    let mut result = Request::new(value);
    result.metadata_mut().insert(
        "x-df-local-binding",
        hex(&credential)
            .parse()
            .map_err(|_| io::Error::other("qualification binding invalid"))?,
    );
    Ok(result)
}
async fn submit(
    service: &Service,
    body: &rpc::SubmitActionRequest,
    credential: [u8; 32],
) -> Result<rpc::SubmitActionResponse, tonic::Status> {
    // Use actual generated protobuf encoding/decoding before the same native service.
    let decoded = rpc::SubmitActionRequest::decode(body.encode_to_vec().as_slice())
        .map_err(|_| tonic::Status::internal("qualification request invalid"))?;
    let request = request(decoded, credential)
        .map_err(|_| tonic::Status::internal("qualification binding invalid"))?;
    rpc::action_service_server::ActionService::submit(service, request)
        .await
        .map(|response| response.into_inner())
}
fn committed(response: rpc::SubmitActionResponse) -> Result<rpc::DecisionReceipt, io::Error> {
    match response.outcome {
        Some(rpc::submit_action_response::Outcome::CommittedDecision(receipt)) => Ok(receipt),
        _ => Err(io::Error::other("qualified decision missing")),
    }
}
fn observed(
    response: rpc::SubmitActionResponse,
    code: rpc::RejectionCode,
) -> Result<(), io::Error> {
    required(
        matches!(response.outcome,Some(rpc::submit_action_response::Outcome::OperationObservation(value)) if value.code==code as i32),
    )
}
async fn audit(service: &Service) -> Result<(usize, usize, usize, [u8; 32]), io::Error> {
    let (reply, wait) = oneshot::channel();
    service
        .actor
        .try_submit(actor::Call::Audit { reply })
        .map_err(|_| io::Error::other("qualification audit admission failed"))?;
    tokio::time::timeout(Duration::from_secs(5), wait)
        .await
        .map_err(|_| io::Error::other("qualification audit timeout"))?
        .map_err(|_| io::Error::other("qualification audit owner stopped"))?
        .map_err(|_| io::Error::other("qualification audit refused"))
}
#[derive(PartialEq)]
struct DurableSnapshot {
    operations: i64,
    facts: i64,
    checkpoints: i64,
    sequence: String,
    checkpoint: [u8; 32],
    receipt: [u8; 32],
    fingerprint: [u8; 32],
}
async fn durable(
    client: &tokio_postgres::Client,
    operation: &[u8],
) -> Result<DurableSnapshot, Box<dyn std::error::Error + Send + Sync>> {
    let row=client.query_one("SELECT (SELECT count(*) FROM df_game.operations) AS operations,(SELECT count(*) FROM df_game.facts) AS facts,(SELECT count(*) FROM df_game.checkpoints) AS checkpoints,s.in_epoch_sequence::text AS sequence,CASE WHEN octet_length(c.complete_envelope)<=1048576 THEN c.complete_envelope END AS envelope,CASE WHEN octet_length(o.receipt)<=4096 THEN o.receipt END AS receipt,o.canonical_fingerprint FROM df_game.sessions s JOIN df_game.checkpoints c ON (c.tenant_id,c.session_id,c.recovery_epoch,c.in_epoch_sequence)=(s.tenant_id,s.session_id,s.recovery_epoch,s.in_epoch_sequence) JOIN df_game.operations o ON o.tenant_id=s.tenant_id AND o.session_id=s.session_id WHERE s.tenant_id=$1 AND s.session_id=$2 AND o.operation_id=$3",&[&TENANT.as_slice(),&[0x41_u8;16].as_slice(),&operation]).await?;
    let envelope: Vec<u8> = row.try_get("envelope")?;
    let receipt: Vec<u8> = row.try_get("receipt")?;
    let fingerprint: Vec<u8> = row.try_get("canonical_fingerprint")?;
    Ok(DurableSnapshot {
        operations: row.try_get("operations")?,
        facts: row.try_get("facts")?,
        checkpoints: row.try_get("checkpoints")?,
        sequence: row.try_get("sequence")?,
        checkpoint: Sha256::digest(envelope).into(),
        receipt: Sha256::digest(receipt).into(),
        fingerprint: fingerprint
            .try_into()
            .map_err(|_| io::Error::other("qualified fingerprint malformed"))?,
    })
}
async fn exercise(
    service: &Service,
    client: &tokio_postgres::Client,
    player: [u8; 32],
    display: [u8; 32],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let row=client.query_one("SELECT operation_id FROM df_game.operations WHERE tenant_id=$1 AND session_id=$2 AND principal_id=$3",&[&TENANT.as_slice(),&[0x41_u8;16].as_slice(),&PLAYER.as_slice()]).await?;
    let operation: Vec<u8> = row.try_get("operation_id")?;
    required(operation.len() == 16)?;
    let mut original = rpc::SubmitActionRequest {
        session_id: Some(rpc::SessionId {
            value: Some(vec![0x41; 16]),
        }),
        run_id: Some(rpc::RunId {
            value: Some(vec![0x42; 16]),
        }),
        observed_revision: Some(rpc::SessionRevision {
            epoch: Some(rpc::RecoveryEpoch { value: Some(1) }),
            sequence: Some(0),
        }),
        operation_id: Some(rpc::OperationId {
            value: Some(operation.clone()),
        }),
        offer_id: model::OFFER.to_owned(),
        action_kind: rpc::GameplayActionKind::ExamineHarborSeal as i32,
    };
    let before = durable(client, &operation).await?;
    required(
        before.operations == 1
            && before.facts == 2
            && before.checkpoints == 2
            && before.sequence == "1"
            && before.fingerprint == Sha256::digest(original.encode_to_vec()).as_slice(),
    )?;
    let first_audit = audit(service).await?;
    required(
        first_audit.0 == 1
            && first_audit.1 == 2
            && first_audit.2 == 1
            && first_audit.3 == before.checkpoint,
    )?;
    let saved = committed(submit(service, &original, player).await?)?;
    required(
        saved.replayed
            && saved.operation_id == original.operation_id
            && saved.session_id == original.session_id
            && saved.run_id == original.run_id
            && saved.revision.as_ref().and_then(|r| r.sequence) == Some(1)
            && matches!(
                saved.outcome,
                Some(rpc::decision_receipt::Outcome::Accepted(_))
            ),
    )?;
    let mut changed = original.clone();
    changed.offer_id.push_str("-changed");
    observed(
        submit(service, &changed, player).await?,
        rpc::RejectionCode::OperationConflict,
    )?;
    let mut stale = original.clone();
    stale.operation_id = Some(rpc::OperationId {
        value: Some(vec![0x95; 16]),
    });
    let rejection = committed(submit(service, &stale, player).await?)?;
    required(
        matches!(rejection.outcome,Some(rpc::decision_receipt::Outcome::Rejected(ref r)) if r.code==rpc::RejectionCode::StaleOffer as i32),
    )?;
    let replayed = committed(submit(service, &stale, player).await?)?;
    required(
        replayed.replayed
            && replayed.outcome == rejection.outcome
            && replayed.revision == rejection.revision,
    )?;
    required(
        submit(service, &original, display)
            .await
            .is_err_and(|status| status.code() == tonic::Code::PermissionDenied),
    )?;
    required(
        submit(service, &original, [0; 32])
            .await
            .is_err_and(|status| status.code() == tonic::Code::PermissionDenied),
    )?;
    let mut watch = rpc::WatchViewRequest {
        session_id: original.session_id.clone(),
        run_id: original.run_id.clone(),
        client_binding_id: Some(rpc::ClientBindingId {
            value: Some(vec![0x71; 16]),
        }),
        after_revision: None,
    };
    required(
        service
            .view(display, watch.clone())
            .await
            .is_err_and(|status| status.code() == tonic::Code::PermissionDenied),
    )?;
    watch.client_binding_id = Some(rpc::ClientBindingId {
        value: Some(vec![0x72; 16]),
    });
    let (_, display_view) = service.view(display, watch).await?;
    required(matches!(
        display_view.audience,
        Some(rpc::view_message::Audience::Display(_))
    ))?;
    required(audit(service).await? == first_audit && durable(client, &operation).await? == before)?;
    let row=client.query_one("SELECT count(*) AS count FROM df_local_demo.rejected_operations WHERE typed_receipt IS NOT NULL",&[]).await?;
    required(row.try_get::<_, i64>("count")? == 1)?;
    // Controlled qualification of retention comes last, after captured gameplay and reconnects.
    required(client.execute("UPDATE df_game.operations SET receipt=NULL WHERE tenant_id=$1 AND session_id=$2 AND operation_id=$3",&[&TENANT.as_slice(),&[0x41_u8;16].as_slice(),&operation]).await?==1)?;
    observed(
        submit(service, &original, player).await?,
        rpc::RejectionCode::OperationExpired,
    )?;
    required(client.execute("INSERT INTO df_game.retired_namespaces (tenant_id,session_id,principal_id,command_namespace,recovery_epoch) VALUES ($1::bytea,$2::bytea,$3::bytea,$4::bytea,1)",&[&TENANT.as_slice(),&[0x41_u8;16].as_slice(),&PLAYER.as_slice(),&b"harbor-ability-check-v1".as_slice()]).await?==1)?;
    original.operation_id = Some(rpc::OperationId {
        value: Some(vec![0x96; 16]),
    });
    observed(
        submit(service, &original, player).await?,
        rpc::RejectionCode::OperationExpired,
    )?;
    Ok(())
}
pub(super) async fn run(
    service: Service,
    player: [u8; 32],
    display: [u8; 32],
    mut cancelled: tokio::sync::oneshot::Receiver<()>,
    database: tokio_postgres::Config,
) -> Result<(), io::Error> {
    for _ in 0..480 {
        if let Ok(metadata) = std::fs::symlink_metadata(TRIGGER) {
            required(
                metadata.is_file() && metadata.len() == 3 && std::fs::read(TRIGGER)? == b"run",
            )?;
            let (client, connection) = tokio::time::timeout(
                Duration::from_secs(2),
                database.connect(tokio_postgres::NoTls),
            )
            .await
            .map_err(|_| io::Error::other("qualification connection timeout"))?
            .map_err(io::Error::other)?;
            let driver = tokio::spawn(connection);
            let result = tokio::time::timeout(
                Duration::from_secs(30),
                exercise(&service, &client, player, display),
            )
            .await;
            drop(client);
            let joined = driver
                .await
                .map_err(io::Error::other)?
                .map_err(io::Error::other);
            let passed = matches!(result, Ok(Ok(()))) && joined.is_ok();
            std::fs::write(
                REPORT,
                format!(
                    "{{\"pass\":{passed},\"actor_calls_limit\":12,\"transport\":\"generated protobuf Service consumer; browser transport assessed separately\",\"retention_qualification\":\"last; expired row remains retained\"}}\n"
                ),
            )?;
            required(passed)?;
            return Ok(());
        }
        tokio::select! {
            _ = &mut cancelled => {
                if std::fs::symlink_metadata(TRIGGER).is_err() {
                    return Ok(());
                }
            },
            _ = tokio::time::sleep(Duration::from_millis(250)) => {},
        }
    }
    Err(io::Error::other("qualification trigger deadline"))
}
