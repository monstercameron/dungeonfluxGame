use df_model::checkpoint::{Basis, Checkpoint, CommandInput, GameCommand, GameInput};
use df_protocol::common as rpc;
use df_session::submission::RepositoryError;
use df_types::{OperationId, SessionRevision};

use super::model;

pub(super) fn revision(value: SessionRevision) -> rpc::SessionRevision {
    rpc::SessionRevision {
        epoch: Some(rpc::RecoveryEpoch {
            value: Some(value.epoch().get()),
        }),
        sequence: Some(value.sequence()),
    }
}
pub(super) fn input(request: &rpc::SubmitActionRequest) -> Result<GameInput, tonic::Status> {
    let invalid = |_| tonic::Status::invalid_argument("required bounded action fields invalid");
    let session = df_api::session_id(request.session_id.as_ref()).map_err(invalid)?;
    let run = df_api::run_id(request.run_id.as_ref()).map_err(invalid)?;
    let observed = df_api::session_revision(request.observed_revision.as_ref()).map_err(invalid)?;
    let operation = df_api::operation_id(request.operation_id.as_ref()).map_err(invalid)?;
    if request.offer_id.len() > 64 {
        return Err(tonic::Status::invalid_argument(
            "offered action exceeds bound",
        ));
    }
    Ok(GameInput::Game(CommandInput {
        basis: Basis {
            session,
            run,
            revision: observed,
        },
        operation,
        member: model::member()
            .map_err(|_| tonic::Status::internal("configured player unavailable"))?,
        observed_revision: observed,
        command: GameCommand::ProposeAction {
            actor: model::actor()
                .map_err(|_| tonic::Status::internal("configured character unavailable"))?,
            action: model::content("inspect-seal")
                .map_err(|_| tonic::Status::internal("configured action unavailable"))?,
            targets: vec![],
            choices: vec![],
        },
    }))
}
pub(super) fn receipt(
    basis: Basis,
    operation: OperationId,
    outcome: rpc::decision_receipt::Outcome,
    replayed: bool,
) -> rpc::DecisionReceipt {
    rpc::DecisionReceipt {
        operation_id: Some(rpc::OperationId {
            value: Some(operation.as_bytes().to_vec()),
        }),
        revision: Some(revision(basis.revision)),
        session_id: Some(rpc::SessionId {
            value: Some(basis.session.as_bytes().to_vec()),
        }),
        run_id: Some(rpc::RunId {
            value: Some(basis.run.as_bytes().to_vec()),
        }),
        replayed,
        outcome: Some(outcome),
    }
}
pub(super) fn rejected(
    basis: Basis,
    operation: OperationId,
    code: rpc::RejectionCode,
) -> rpc::DecisionReceipt {
    receipt(
        basis,
        operation,
        rpc::decision_receipt::Outcome::Rejected(rpc::Rejection { code: code as i32 }),
        false,
    )
}
pub(super) fn committed(receipt: rpc::DecisionReceipt) -> rpc::SubmitActionResponse {
    rpc::SubmitActionResponse {
        outcome: Some(rpc::submit_action_response::Outcome::CommittedDecision(
            receipt,
        )),
    }
}
pub(super) fn observation(code: rpc::RejectionCode) -> rpc::SubmitActionResponse {
    rpc::SubmitActionResponse {
        outcome: Some(rpc::submit_action_response::Outcome::OperationObservation(
            rpc::OperationObservation { code: code as i32 },
        )),
    }
}
pub(super) fn check(
    current: &Checkpoint,
) -> Result<Option<rpc::AbilityCheckResult>, RepositoryError> {
    let Some(draw) = current.state().draws.first() else {
        return Ok(None);
    };
    let result = df_rules::ability_check::resolve(df_rules::ability_check::AbilityCheckInput {
        die: u8::try_from(draw.value).map_err(|_| RepositoryError::InvalidCandidate)?,
        ability_modifier: 2,
        proficiency_bonus: 2,
        difficulty_class: 15,
    })
    .map_err(|_| RepositoryError::InvalidCandidate)?;
    Ok(Some(rpc::AbilityCheckResult {
        die: u32::from(result.die),
        ability_modifier: i32::from(result.ability_modifier),
        proficiency_bonus: u32::from(result.proficiency_bonus),
        total: i32::from(result.total),
        difficulty_class: u32::from(result.difficulty_class),
        succeeded: result.succeeded,
        source_revision: df_rules::ability_check::SOURCE_REVISION.to_owned(),
    }))
}
pub(super) fn view(
    current: &Checkpoint,
    role: df_persistence::local_demo_scope::LocalDemoRole,
) -> Result<rpc::ViewMessage, RepositoryError> {
    use df_persistence::local_demo_scope::LocalDemoRole;
    let result = check(current)?;
    let public_narration = match result.as_ref() {
        None => {
            "Salt wind sweeps the harbor. Mara kneels beside a wax seal stamped with an unfamiliar crest."
        }
        Some(check) if check.succeeded => {
            "Mara studies the seal. A hidden pattern emerges, and the investigation moves forward."
        }
        Some(_) => {
            "Mara studies the damaged seal, but the saltwater has erased the decisive markings. This trail goes cold."
        }
    };
    let audience = match role {
        LocalDemoRole::Display => rpc::view_message::Audience::Display(rpc::DisplayGameplayView {
            narration: public_narration.to_owned(),
            scene_asset: "assets/ui/scenes/mara-harbor-v4.png".to_owned(),
        }),
        LocalDemoRole::Player => {
            let clue = if result.as_ref().is_some_and(|check| check.succeeded) {
                "The split anchor crest belongs to a loading-pier shipment. Its berth is hidden beneath the warehouse ledger."
            } else {
                ""
            };
            rpc::view_message::Audience::Player(rpc::PlayerGameplayView {
                offer_id: if result.is_none() {
                    model::OFFER.to_owned()
                } else {
                    String::new()
                },
                narration: public_narration.to_owned(),
                check: result,
                private_clue: clue.to_owned(),
                action_available: current.state().decisions.is_empty(),
            })
        }
    };
    Ok(rpc::ViewMessage {
        revision: Some(revision(current.basis().revision)),
        audience: Some(audience),
    })
}
