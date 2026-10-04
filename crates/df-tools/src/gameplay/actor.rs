use df_model::checkpoint::{Checkpoint, GameInput};
use df_observe::OperationContext;
use df_persistence::local_demo_scope::{
    LocalDemoAuthority, LocalDemoRole, LocalDemoScopeIssuer, LocalRejectedLookup,
};
use df_persistence::{NativeCodecLimits, NativeScope, PostgresRepository};
use df_protocol::common as rpc;
use df_session::inbox::{ActorInput, AdmissionSequence, Reducer};
use df_session::submission::{
    DeliveryError, DurableOwner, OwnedInput, PublicationOwner, RepositoryError, SubmissionOutcome,
};
use prost::Message;
use sha2::{Digest, Sha256};
use tokio::sync::{oneshot, watch};

use super::{model, wire};

pub(super) type Audit = (usize, usize, usize, [u8; 32]);
pub(super) enum Call {
    Audit {
        reply: oneshot::Sender<Result<Audit, tonic::Status>>,
    },
    Submit {
        credential: [u8; 32],
        request: rpc::SubmitActionRequest,
        reply: oneshot::Sender<Result<rpc::SubmitActionResponse, tonic::Status>>,
    },
    View {
        credential: [u8; 32],
        request: rpc::WatchViewRequest,
        reply: oneshot::Sender<Result<(LocalDemoRole, rpc::ViewMessage), tonic::Status>>,
    },
}
impl ActorInput for Call {
    fn retained_heap_bytes(&self) -> Option<usize> {
        fn bytes(value: &Option<Vec<u8>>) -> usize {
            value.as_ref().map_or(0, Vec::capacity)
        }
        match self {
            Self::Audit { .. } => Some(0),
            Self::Submit { request, .. } => request
                .offer_id
                .capacity()
                .checked_add(request.session_id.as_ref().map_or(0, |id| bytes(&id.value)))?
                .checked_add(request.run_id.as_ref().map_or(0, |id| bytes(&id.value)))?
                .checked_add(
                    request
                        .operation_id
                        .as_ref()
                        .map_or(0, |id| bytes(&id.value)),
                ),
            Self::View { request, .. } => request
                .session_id
                .as_ref()
                .map_or(0, |id| bytes(&id.value))
                .checked_add(request.run_id.as_ref().map_or(0, |id| bytes(&id.value)))?
                .checked_add(
                    request
                        .client_binding_id
                        .as_ref()
                        .map_or(0, |id| bytes(&id.value)),
                ),
        }
    }
}

pub(super) struct Publication(pub watch::Sender<Checkpoint>);
impl PublicationOwner<NativeScope<LocalDemoAuthority>> for Publication {
    fn publish_committed(
        &mut self,
        _: &NativeScope<LocalDemoAuthority>,
        checkpoint: &Checkpoint,
    ) -> Result<(), DeliveryError> {
        self.0.send_replace(checkpoint.clone());
        Ok(())
    }
    fn wake_committed_intents(
        &mut self,
        _: &NativeScope<LocalDemoAuthority>,
    ) -> Result<(), DeliveryError> {
        Ok(())
    }
}
pub(super) type Owner =
    DurableOwner<PostgresRepository<LocalDemoAuthority>, model::HarborEngine, Publication>;
pub(super) struct Actor {
    pub owner: Owner,
    pub issuer: LocalDemoScopeIssuer,
    pub codec: NativeCodecLimits,
    pub fenced: bool,
    pub recovery_wakeup: watch::Sender<Checkpoint>,
    pub calls_remaining: u16,
}
fn unavailable(error: RepositoryError) -> tonic::Status {
    match error {
        RepositoryError::Unauthorized => {
            tonic::Status::permission_denied("local gameplay binding denied")
        }
        _ => tonic::Status::unavailable("gameplay authority unavailable; retain your operation"),
    }
}
impl Actor {
    fn fence(&mut self) {
        self.fenced = true;
        self.recovery_wakeup
            .send_replace(self.owner.checkpoint().clone());
    }
    fn submit(
        &mut self,
        credential: [u8; 32],
        request: rpc::SubmitActionRequest,
    ) -> Result<rpc::SubmitActionResponse, tonic::Status> {
        if self.fenced {
            return Err(tonic::Status::unavailable(
                "gameplay owner requires recovery",
            ));
        }
        if self.issuer.authenticate(&credential).map_err(unavailable)? != LocalDemoRole::Player {
            return Err(tonic::Status::permission_denied(
                "display bindings cannot submit player actions",
            ));
        }
        let input = wire::input(&request)?;
        let GameInput::Game(command) = &input else {
            return Err(tonic::Status::internal("canonical command unavailable"));
        };
        let operation = command.operation;
        let current = self.owner.checkpoint().basis();
        if command.basis.session != current.session {
            return Err(tonic::Status::permission_denied(
                "local gameplay scope denied",
            ));
        }
        // Fingerprint the exact generated request; retrying requires the same original envelope.
        let fingerprint: [u8; 32] = Sha256::digest(request.encode_to_vec()).into();
        let scope = self
            .issuer
            .issue(
                &credential,
                model::random().map_err(unavailable)?,
                fingerprint,
                input.clone(),
                model::pins().map_err(unavailable)?,
            )
            .map_err(unavailable)?;
        let retained = self
            .issuer
            .rejected_operation(&scope, current, None, self.codec)
            .map_err(|error| {
                if error == RepositoryError::UnresolvedCommit {
                    self.fence();
                }
                unavailable(error)
            })?;
        match retained {
            LocalRejectedLookup::Committed(bytes) => {
                let mut receipt = rpc::DecisionReceipt::decode(bytes.as_slice())
                    .map_err(|_| tonic::Status::internal("retained receipt invalid"))?;
                receipt.replayed = true;
                return Ok(wire::committed(receipt));
            }
            LocalRejectedLookup::Conflict => {
                return Ok(wire::observation(rpc::RejectionCode::OperationConflict));
            }
            LocalRejectedLookup::Expired => {
                return Ok(wire::observation(rpc::RejectionCode::OperationExpired));
            }
            LocalRejectedLookup::NotRecorded => {
                let rejection =
                    if request.action_kind != rpc::GameplayActionKind::ExamineHarborSeal as i32 {
                        Some(rpc::RejectionCode::InvalidSelection)
                    } else if request.offer_id != model::OFFER
                        || !self.owner.checkpoint().state().decisions.is_empty()
                        || command.basis.run != current.run
                        || command.basis.revision.epoch() != current.revision.epoch()
                        || command.observed_revision > current.revision
                    {
                        Some(rpc::RejectionCode::StaleOffer)
                    } else {
                        None
                    };
                if let Some(code) = rejection {
                    let receipt = wire::rejected(current, operation, code);
                    let bytes = receipt.encode_to_vec();
                    let recorded = self
                        .issuer
                        .rejected_operation(&scope, current, Some(&bytes), self.codec)
                        .map_err(|error| {
                            if error == RepositoryError::UnresolvedCommit {
                                self.fence();
                            }
                            unavailable(error)
                        })?;
                    return match recorded {
                        LocalRejectedLookup::Committed(bytes) => {
                            rpc::DecisionReceipt::decode(bytes.as_slice())
                                .map(wire::committed)
                                .map_err(|_| tonic::Status::internal("committed rejection invalid"))
                        }
                        _ => Err(tonic::Status::unavailable(
                            "rejection commit requires recovery",
                        )),
                    };
                }
            }
            LocalRejectedLookup::Accepted => {}
        }
        let prior_revision = self.owner.checkpoint().basis().revision;
        let context = OperationContext {
            trace_parent: String::new(),
            build: crate::BUILD_ID.to_owned(),
        };
        let (item, receipt) = OwnedInput::new(context, scope, input);
        self.owner.reduce(AdmissionSequence(0), item);
        let outcome = receipt
            .try_recv()
            .map_err(|_| tonic::Status::internal("actor did not produce its outcome"))?;
        match outcome {
            SubmissionOutcome::Confirmed(committed) => {
                let check = wire::check(self.owner.checkpoint())
                    .map_err(unavailable)?
                    .ok_or_else(|| tonic::Status::internal("committed check missing"))?;
                Ok(wire::committed(wire::receipt(
                    committed.basis(),
                    operation,
                    rpc::decision_receipt::Outcome::Accepted(rpc::AcceptedAction {
                        check: Some(check),
                    }),
                    committed.basis().revision <= prior_revision,
                )))
            }
            SubmissionOutcome::OperationConflict => {
                Ok(wire::observation(rpc::RejectionCode::OperationConflict))
            }
            SubmissionOutcome::ExpiredOrIndeterminate => {
                self.fence();
                Err(tonic::Status::unavailable("operation lookup required"))
            }
            SubmissionOutcome::LookupRequired => {
                self.fence();
                Err(tonic::Status::unavailable("operation lookup required"))
            }
            SubmissionOutcome::Refused(error) => {
                if error == RepositoryError::UnresolvedCommit {
                    self.fence();
                }
                Err(unavailable(error))
            }
        }
    }
    fn view(
        &mut self,
        credential: [u8; 32],
        request: rpc::WatchViewRequest,
    ) -> Result<(LocalDemoRole, rpc::ViewMessage), tonic::Status> {
        if self.fenced {
            return Err(tonic::Status::unavailable(
                "gameplay owner requires recovery",
            ));
        }
        let role = self.issuer.authenticate(&credential).map_err(unavailable)?;
        let session = df_api::session_id(request.session_id.as_ref())
            .map_err(|_| tonic::Status::invalid_argument("required watch scope invalid"))?;
        let run = df_api::run_id(request.run_id.as_ref())
            .map_err(|_| tonic::Status::invalid_argument("required watch scope invalid"))?;
        let binding = df_api::client_binding_id(request.client_binding_id.as_ref())
            .map_err(|_| tonic::Status::invalid_argument("required watch binding invalid"))?;
        let expected_binding = match role {
            LocalDemoRole::Player => [0x71; 16],
            LocalDemoRole::Display => [0x72; 16],
        };
        let current = self.owner.checkpoint();
        if session != current.basis().session
            || run != current.basis().run
            || binding.as_bytes() != &expected_binding
        {
            return Err(tonic::Status::permission_denied("local watch scope denied"));
        }
        if let Some(revision) = request.after_revision.as_ref() {
            let observed = df_api::session_revision(Some(revision))
                .map_err(|_| tonic::Status::invalid_argument("watch revision invalid"))?;
            if observed.epoch() != current.basis().revision.epoch()
                || observed > current.basis().revision
            {
                return Err(tonic::Status::failed_precondition(
                    "watch recovery required",
                ));
            }
        }
        Ok((role, wire::view(current, role).map_err(unavailable)?))
    }
}
impl Reducer<Call> for Actor {
    fn reduce(&mut self, _: AdmissionSequence, call: Call) {
        if self.calls_remaining == 0 {
            match call {
                Call::Audit { reply } => {
                    let _ = reply.send(Err(tonic::Status::resource_exhausted(
                        "finite audit budget exhausted",
                    )));
                }
                Call::Submit { reply, .. } => {
                    let _ = reply.send(Err(tonic::Status::resource_exhausted(
                        "finite demonstration call budget exhausted",
                    )));
                }
                Call::View { reply, .. } => {
                    let _ = reply.send(Err(tonic::Status::resource_exhausted(
                        "finite demonstration call budget exhausted",
                    )));
                }
            }
            return;
        }
        self.calls_remaining -= 1;
        let context = OperationContext {
            trace_parent: String::new(),
            build: crate::BUILD_ID.to_owned(),
        };
        let mut span = df_observe::begin(&context, "gameplay.local_demo_call");
        let delivered = match call {
            Call::Audit { reply } => {
                let checkpoint = self.owner.checkpoint();
                let result = df_persistence::local_demo_scope::encode_owned_demo_checkpoint(
                    checkpoint, self.codec,
                )
                .map(|bytes| {
                    (
                        checkpoint.state().draws.len(),
                        checkpoint.state().facts.len(),
                        checkpoint.state().decisions.len(),
                        Sha256::digest(bytes).into(),
                    )
                })
                .map_err(unavailable);
                reply.send(result).is_ok()
            }
            Call::Submit {
                credential,
                request,
                reply,
            } => reply.send(self.submit(credential, request)).is_ok(),
            Call::View {
                credential,
                request,
                reply,
            } => reply.send(self.view(credential, request)).is_ok(),
        };
        span.finish_unmeasured(if delivered {
            "actor_outcome_delivered"
        } else {
            "caller_gone_outcome_retained"
        });
    }
}
