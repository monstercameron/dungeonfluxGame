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

#[derive(Clone)]
pub(super) struct QualificationSnapshot {
    pub actions: Vec<QualificationInput>,
    pub joins: Vec<QualificationJoin>,
    pub checkpoint: Checkpoint,
    pub codec: NativeCodecLimits,
    #[cfg(test)]
    pub fenced: bool,
    #[cfg(test)]
    pub uncertain: bool,
}
pub(super) type QualificationJoin = (rpc::JoinRoomRequest, [u8; 32], [u8; 16]);
pub(super) type QualificationInput = ([u8; 32], rpc::SubmitActionRequest);
fn retain_qualification_input(
    inputs: &mut Vec<QualificationInput>,
    credential: [u8; 32],
    request: &rpc::SubmitActionRequest,
) {
    if inputs.len() < 7
        && matches!(
            rpc::GameplayActionKind::try_from(request.action_kind),
            Ok(rpc::GameplayActionKind::CreateCharacter
                | rpc::GameplayActionKind::BeginStory
                | rpc::GameplayActionKind::AskCourier
                | rpc::GameplayActionKind::DefendCourier
                | rpc::GameplayActionKind::GreatswordAttack)
        )
        && !inputs.iter().any(|(saved_credential, saved)| {
            *saved_credential == credential && saved.action_kind == request.action_kind
        })
    {
        inputs.push((credential, request.clone()));
    }
}

pub(super) enum Call {
    QualificationInputs {
        reply: oneshot::Sender<Result<QualificationSnapshot, tonic::Status>>,
    },
    Join {
        request: rpc::JoinRoomRequest,
        reply: oneshot::Sender<Result<rpc::JoinRoomResponse, tonic::Status>>,
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
            Self::Join { request, .. } => request
                .room_code
                .capacity()
                .checked_add(request.join_secret.capacity())?
                .checked_add(
                    request
                        .operation_id
                        .as_ref()
                        .map_or(0, |id| bytes(&id.value)),
                ),
            Self::QualificationInputs { .. } => Some(0),
            Self::Submit { request, .. } => {
                let character_bytes = if let Some(character) = &request.character {
                    character.choices.iter().try_fold(
                        character.name.capacity().checked_add(
                            character
                                .choices
                                .capacity()
                                .checked_mul(std::mem::size_of::<rpc::JourneyChoice>())?,
                        )?,
                        |total, choice| {
                            total
                                .checked_add(choice.group_id.capacity())?
                                .checked_add(choice.option_id.capacity())
                        },
                    )?
                } else {
                    0
                };
                request
                    .offer_id
                    .capacity()
                    .checked_add(request.session_id.as_ref().map_or(0, |id| bytes(&id.value)))?
                    .checked_add(request.run_id.as_ref().map_or(0, |id| bytes(&id.value)))?
                    .checked_add(
                        request
                            .operation_id
                            .as_ref()
                            .map_or(0, |id| bytes(&id.value)),
                    )?
                    .checked_add(character_bytes)
            }
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
pub(super) struct Engine(pub Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>);
impl df_session::submission::SessionEngine<NativeScope<LocalDemoAuthority>> for Engine {
    fn decide(
        &mut self,
        current: &Checkpoint,
        scope: &NativeScope<LocalDemoAuthority>,
        input: &GameInput,
    ) -> Result<Checkpoint, RepositoryError> {
        if let Some(counter) = &self.0 {
            counter
                .fetch_update(
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                    |value| value.checked_add(1),
                )
                .map_err(|_| RepositoryError::Capacity)?;
        }
        df_session::submission::SessionEngine::decide(
            &mut model::HarborEngine,
            current,
            scope,
            input,
        )
    }
    fn validate_recovery(&mut self, checkpoint: &Checkpoint) -> Result<(), RepositoryError> {
        df_session::submission::SessionEngine::<NativeScope<LocalDemoAuthority>>::validate_recovery(
            &mut model::HarborEngine,
            checkpoint,
        )
    }
}
pub(super) type Owner = DurableOwner<PostgresRepository<LocalDemoAuthority>, Engine, Publication>;
pub(super) struct Actor {
    pub owner: Owner,
    pub bootstrap_credential: [u8; 32],
    pub issuer: LocalDemoScopeIssuer,
    pub codec: NativeCodecLimits,
    pub fenced: bool,
    pub recovery_wakeup: watch::Sender<Checkpoint>,
    pub calls_remaining: u16,
    pub qualification_inputs: Option<Vec<QualificationInput>>,
    pub qualification_joins: Option<Vec<QualificationJoin>>,
}
pub(super) fn unavailable(error: RepositoryError) -> tonic::Status {
    match error {
        RepositoryError::Unauthorized => {
            tonic::Status::permission_denied("local gameplay binding denied")
        }
        _ => tonic::Status::unavailable("gameplay authority unavailable; retain your operation"),
    }
}
impl Actor {
    pub(super) fn fence(&mut self) {
        self.fenced = true;
        self.recovery_wakeup
            .send_replace(self.owner.checkpoint().clone());
    }
    fn submit(
        &mut self,
        credential: [u8; 32],
        request: rpc::SubmitActionRequest,
    ) -> Result<rpc::SubmitActionResponse, tonic::Status> {
        if self.issuer.authenticate(&credential).map_err(unavailable)? != LocalDemoRole::Player {
            return Err(tonic::Status::permission_denied(
                "display bindings cannot submit player actions",
            ));
        }
        let principal = self.issuer.principal(&credential).map_err(unavailable)?;
        if principal == df_persistence::local_demo_scope::PLAYER {
            return Err(tonic::Status::permission_denied(
                "bootstrap scope is not a player client",
            ));
        }
        let member = df_types::MemberId::from_bytes(&principal)
            .map_err(|_| tonic::Status::permission_denied("member binding invalid"))?;
        let input = wire::journey_input(&request, member, self.owner.checkpoint())?;
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
        let context = OperationContext {
            trace_parent: String::new(),
            build: crate::BUILD_ID.to_owned(),
        };
        let recovering = self.fenced;
        let replaying_uncertain = recovering && self.owner.has_uncertain_operation();
        if recovering {
            if self.owner.has_uncertain_operation() {
                if !self
                    .owner
                    .matches_uncertain_retry(&scope, &input)
                    .map_err(unavailable)?
                {
                    return Err(tonic::Status::unavailable(
                        "exact retained operation required for recovery",
                    ));
                }
            } else {
                // Known CAS/acknowledged duplicate failures have no unknown write:
                // current database authorization plus validated reload may resume.
                self.owner
                    .reload_current(&scope, &context)
                    .map_err(unavailable)?;
                self.fenced = false;
                self.recovery_wakeup
                    .send_replace(self.owner.checkpoint().clone());
            }
        } else {
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
                    let checkpoint = self.owner.checkpoint();
                    let kind = rpc::GameplayActionKind::try_from(request.action_kind)
                        .map_err(|_| tonic::Status::invalid_argument("unknown action"))?;
                    let rejection = if command.basis.run != current.run
                        || command.basis.revision.epoch() != current.revision.epoch()
                        || command.observed_revision > current.revision
                    {
                        Some(rpc::RejectionCode::StaleOffer)
                    } else if wire::unrelated_payload(&request, kind)
                        || (kind == rpc::GameplayActionKind::CreateCharacter
                            && request.character.as_ref().is_none_or(|character| {
                                let selections = character
                                    .choices
                                    .iter()
                                    .map(|choice| {
                                        (choice.group_id.as_str(), choice.option_id.as_str())
                                    })
                                    .collect::<Vec<_>>();
                                df_rules::local_journey::validate_character(
                                    &character.name,
                                    &selections,
                                )
                                .is_err()
                            }))
                    {
                        Some(rpc::RejectionCode::InvalidSelection)
                    } else if super::journey::phase(checkpoint).map_err(unavailable)?
                        == rpc::JourneyPhase::Combat
                        && checkpoint
                            .state()
                            .encounters
                            .first()
                            .and_then(|encounter| encounter.active_turn)
                            != Some(
                                super::journey::player_entity(member, checkpoint)
                                    .map_err(unavailable)?,
                            )
                    {
                        Some(rpc::RejectionCode::WrongTurn)
                    } else if request.offer_id != super::journey::offer_id(checkpoint, kind)
                        || !super::journey::offered(checkpoint, member)
                            .map_err(unavailable)?
                            .iter()
                            .any(|(offered, _)| *offered == kind)
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
                                    .map_err(|_| {
                                        tonic::Status::internal("committed rejection invalid")
                                    })
                            }
                            _ => Err(tonic::Status::unavailable(
                                "rejection commit requires recovery",
                            )),
                        };
                    }
                }
                LocalRejectedLookup::Accepted => {}
            }
        }
        let prior_revision = self.owner.checkpoint().basis().revision;
        let (item, receipt) = OwnedInput::new(context, scope, input);
        self.owner.reduce(AdmissionSequence(0), item);
        let outcome = receipt
            .try_recv()
            .map_err(|_| tonic::Status::internal("actor did not produce its outcome"))?;
        if !self.owner.is_current() {
            self.fence();
        }
        match outcome {
            SubmissionOutcome::Confirmed(committed) => {
                if recovering && self.owner.is_current() {
                    self.fenced = false;
                    self.recovery_wakeup
                        .send_replace(self.owner.checkpoint().clone());
                }
                let accepted =
                    super::journey::accepted(committed.decision()).map_err(unavailable)?;
                if let Some(inputs) = self.qualification_inputs.as_mut() {
                    retain_qualification_input(inputs, credential, &request);
                }
                Ok(wire::committed(wire::receipt(
                    committed.basis(),
                    operation,
                    rpc::decision_receipt::Outcome::Accepted(Box::new(accepted)),
                    replaying_uncertain || committed.basis().revision <= prior_revision,
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
            LocalDemoRole::Player => self.issuer.principal(&credential).map_err(unavailable)?,
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
        let principal = self.issuer.principal(&credential).map_err(unavailable)?;
        if principal == df_persistence::local_demo_scope::PLAYER {
            return Err(tonic::Status::permission_denied(
                "bootstrap scope cannot watch",
            ));
        }
        let member = df_types::MemberId::from_bytes(&principal)
            .map_err(|_| tonic::Status::permission_denied("binding invalid"))?;
        Ok((
            role,
            wire::journey_view(current, role, member).map_err(unavailable)?,
        ))
    }
}
impl Reducer<Call> for Actor {
    fn reduce(&mut self, _: AdmissionSequence, call: Call) {
        if self.calls_remaining == 0 {
            match call {
                Call::QualificationInputs { reply } => {
                    let _ = reply.send(Err(tonic::Status::resource_exhausted(
                        "qualification budget exhausted",
                    )));
                }
                Call::Join { reply, .. } => {
                    let _ = reply.send(Err(tonic::Status::resource_exhausted(
                        "finite room budget exhausted",
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
            Call::QualificationInputs { reply } => reply
                .send(
                    self.qualification_inputs
                        .clone()
                        .map(|actions| QualificationSnapshot {
                            actions,
                            joins: self.qualification_joins.clone().unwrap_or_default(),
                            checkpoint: self.owner.checkpoint().clone(),
                            codec: self.codec,
                            #[cfg(test)]
                            fenced: self.fenced,
                            #[cfg(test)]
                            uncertain: self.owner.has_uncertain_operation(),
                        })
                        .ok_or_else(|| tonic::Status::permission_denied("qualification disabled")),
                )
                .is_ok(),
            Call::Join { request, reply } => reply.send(self.join(request)).is_ok(),
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

#[cfg(test)]
mod qualification_tests {
    use super::*;

    #[test]
    fn retained_inputs_cover_authored_verifier_and_replay_with_a_fixed_bound() {
        let mut inputs = Vec::new();
        let action = |kind| rpc::SubmitActionRequest {
            action_kind: kind as i32,
            ..Default::default()
        };
        for kind in [
            rpc::GameplayActionKind::CreateCharacter,
            rpc::GameplayActionKind::BeginStory,
            rpc::GameplayActionKind::AskCourier,
            rpc::GameplayActionKind::DefendCourier,
            rpc::GameplayActionKind::GreatswordAttack,
        ] {
            retain_qualification_input(&mut inputs, [1; 32], &action(kind));
            retain_qualification_input(&mut inputs, [1; 32], &action(kind));
        }
        retain_qualification_input(
            &mut inputs,
            [2; 32],
            &action(rpc::GameplayActionKind::CreateCharacter),
        );
        retain_qualification_input(
            &mut inputs,
            [2; 32],
            &action(rpc::GameplayActionKind::GreatswordAttack),
        );
        assert_eq!(inputs.len(), 7);
        for kind in [
            rpc::GameplayActionKind::BeginStory,
            rpc::GameplayActionKind::AskCourier,
            rpc::GameplayActionKind::DefendCourier,
            rpc::GameplayActionKind::GreatswordAttack,
        ] {
            assert!(
                inputs
                    .iter()
                    .any(|(_, request)| request.action_kind == kind as i32)
            );
        }
        for kind in [
            rpc::GameplayActionKind::CreateCharacter,
            rpc::GameplayActionKind::GreatswordAttack,
        ] {
            assert_eq!(
                inputs
                    .iter()
                    .filter(|(_, request)| request.action_kind == kind as i32)
                    .count(),
                2
            );
        }
        let retained = inputs.clone();
        retain_qualification_input(
            &mut inputs,
            [3; 32],
            &action(rpc::GameplayActionKind::BeginStory),
        );
        assert_eq!(inputs, retained);
        let mut empty = Vec::new();
        retain_qualification_input(
            &mut empty,
            [1; 32],
            &action(rpc::GameplayActionKind::EndTurn),
        );
        retain_qualification_input(
            &mut empty,
            [1; 32],
            &rpc::SubmitActionRequest {
                action_kind: i32::MAX,
                ..Default::default()
            },
        );
        assert!(empty.is_empty());
    }
}
