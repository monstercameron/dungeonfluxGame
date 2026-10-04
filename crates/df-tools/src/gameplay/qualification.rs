//! Finite private consumer of generated requests against the actual local actor and PG.
//! Enabled only by the explicit qualification flag; no HTTP route or gameplay UI control.
use super::{Service, actor, hex};
use df_persistence::local_demo_scope::TENANT;
use df_protocol::common as rpc;
use prost::Message;
use sha2::{Digest, Sha256};
use std::{io, time::Duration};
use tokio::sync::oneshot;
use tonic::Request;

const TRIGGER: &str = "/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/real-next-scene-20261004/qualification-start-01";
const REPORT: &str = "/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/real-next-scene-20261004/authority-acceptance-report-01.json";
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
        Some(rpc::submit_action_response::Outcome::CommittedDecision(receipt)) => Ok(*receipt),
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
    display: [u8; 32],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (reply, wait) = oneshot::channel();
    service
        .actor
        .try_submit(actor::Call::QualificationInputs { reply })
        .map_err(|_| io::Error::other("qualification inputs admission failed"))?;
    let snapshot = tokio::time::timeout(Duration::from_secs(5), wait).await???;
    required(snapshot.joins.len() == 2)?;
    let (proof, grant, member) = &snapshot.joins[0];
    let counts=client.query_one("SELECT (SELECT count(*) FROM df_local_demo.room_grants) AS joins,(SELECT count(*) FROM df_local_demo.grants) AS grants,(SELECT count(*) FROM df_game.operations) AS operations",&[]).await?;
    let before_counts = (
        counts.get::<_, i64>("joins"),
        counts.get::<_, i64>("grants"),
        counts.get::<_, i64>("operations"),
    );
    required(before_counts.0 == 2 && before_counts.1 == 4)?;
    let replay = rpc::room_service_server::RoomService::join(
        service,
        Request::new(rpc::JoinRoomRequest::decode(
            proof.encode_to_vec().as_slice(),
        )?),
    )
    .await?
    .into_inner();
    required(
        matches!(replay.outcome,Some(rpc::join_room_response::Outcome::Joined(joined)) if joined.local_binding==hex(grant)
        && joined.client_binding_id.as_ref().and_then(|id|id.value.as_ref())==Some(&member.to_vec())
        && joined.receipt.as_ref().is_some_and(|receipt|receipt.replayed && receipt.operation_id==proof.operation_id)),
    )?;
    let mut changed = proof.clone();
    changed.join_secret[0] ^= 1;
    let conflict = rpc::room_service_server::RoomService::join(service, Request::new(changed))
        .await?
        .into_inner();
    required(
        matches!(conflict.outcome,Some(rpc::join_room_response::Outcome::Refused(value)) if value.code==rpc::RejectionCode::OperationConflict as i32),
    )?;
    let counts=client.query_one("SELECT (SELECT count(*) FROM df_local_demo.room_grants) AS joins,(SELECT count(*) FROM df_local_demo.grants) AS grants,(SELECT count(*) FROM df_game.operations) AS operations",&[]).await?;
    required(
        before_counts
            == (
                counts.get::<_, i64>("joins"),
                counts.get::<_, i64>("grants"),
                counts.get::<_, i64>("operations"),
            ),
    )?;
    let inputs = snapshot.actions;
    let creations = inputs
        .iter()
        .filter(|(_, request)| {
            request.action_kind == rpc::GameplayActionKind::CreateCharacter as i32
        })
        .collect::<Vec<_>>();
    required(creations.len() == 2 && creations[0].0 != creations[1].0)?;
    for (credential, original) in &creations {
        let operation = original
            .operation_id
            .as_ref()
            .and_then(|id| id.value.as_ref())
            .ok_or_else(|| io::Error::other("creation identity absent"))?;
        let before = durable(client, operation).await?;
        let receipt = committed(submit(service, original, *credential).await?)?;
        required(
            receipt.replayed
                && receipt.operation_id == original.operation_id
                && matches!(receipt.outcome,Some(rpc::decision_receipt::Outcome::Accepted(ref accepted)) if accepted.character.is_some())
                && durable(client, operation).await? == before,
        )?;
    }
    let (player, original) = inputs
        .iter()
        .find(|(_, request)| {
            request.action_kind == rpc::GameplayActionKind::GreatswordAttack as i32
        })
        .ok_or_else(|| io::Error::other("recorded real player attack absent"))?;
    let operation = original
        .operation_id
        .as_ref()
        .and_then(|id| id.value.as_ref())
        .ok_or_else(|| io::Error::other("attack identity absent"))?;
    let before = durable(client, operation).await?;
    required(before.fingerprint == Sha256::digest(original.encode_to_vec()).as_slice())?;
    let accepted = committed(submit(service, original, *player).await?)?;
    required(
        accepted.replayed
            && matches!(accepted.outcome,Some(rpc::decision_receipt::Outcome::Accepted(ref value)) if !value.combat.is_empty()),
    )?;
    let mut conflict = original.clone();
    conflict.graze = !conflict.graze;
    observed(
        submit(service, &conflict, *player).await?,
        rpc::RejectionCode::OperationConflict,
    )?;
    required(durable(client, operation).await? == before)?;
    let mut stale = creations[0].1.clone();
    stale.operation_id = Some(rpc::OperationId {
        value: Some(vec![0x95; 16]),
    });
    let rejected = committed(submit(service, &stale, creations[0].0).await?)?;
    required(
        matches!(rejected.outcome,Some(rpc::decision_receipt::Outcome::Rejected(ref value)) if value.code==rpc::RejectionCode::StaleOffer as i32),
    )?;
    let replay = committed(submit(service, &stale, creations[0].0).await?)?;
    required(
        replay.replayed
            && replay.outcome == rejected.outcome
            && replay.revision == rejected.revision,
    )?;
    required(
        submit(service, original, display)
            .await
            .is_err_and(|status| status.code() == tonic::Code::PermissionDenied),
    )?;
    required(
        submit(service, original, [0; 32])
            .await
            .is_err_and(|status| status.code() == tonic::Code::PermissionDenied),
    )?;
    let mut watch = rpc::WatchViewRequest {
        session_id: original.session_id.clone(),
        run_id: original.run_id.clone(),
        client_binding_id: Some(rpc::ClientBindingId {
            value: Some(vec![0x72; 16]),
        }),
        after_revision: None,
    };
    let (_, public) = service.view(display, watch.clone()).await?;
    required(
        matches!(&public.audience,Some(rpc::view_message::Audience::Display(view)) if view.journey.as_ref().is_some_and(|journey|journey.creation.is_none() && journey.own_character.is_none())),
    )?;
    required(!String::from_utf8_lossy(&public.encode_to_vec()).contains("addressed to Vell"))?;
    watch.client_binding_id = Some(rpc::ClientBindingId {
        value: Some(vec![0x61; 16]),
    });
    required(
        service
            .view(display, watch)
            .await
            .is_err_and(|status| status.code() == tonic::Code::PermissionDenied),
    )?;
    required(durable(client, operation).await? == before)?;
    required(client.execute("UPDATE df_game.operations SET receipt=NULL WHERE tenant_id=$1 AND session_id=$2 AND operation_id=$3",&[&TENANT.as_slice(),&[0x41_u8;16].as_slice(),&operation.as_slice()]).await?==1)?;
    observed(
        submit(service, original, *player).await?,
        rpc::RejectionCode::OperationExpired,
    )?;
    Ok(())
}
pub(super) async fn run(
    service: Service,
    display: [u8; 32],
    mut cancelled: tokio::sync::oneshot::Receiver<()>,
    database: tokio_postgres::Config,
) -> Result<(), io::Error> {
    for _ in 0..1040 {
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
                exercise(&service, &client, display),
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
                    "{{\"pass\":{passed},\"actor_calls_limit\":14,\"transport\":\"generated protobuf Service consumer; browser transport assessed separately\",\"retention_qualification\":\"last; expired row remains retained\"}}\n"
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
