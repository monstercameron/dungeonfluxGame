use std::collections::VecDeque;
use std::future::{Future, ready};
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use df_client::receipts::{
    LookupOutcome, OperationKey, OperationResolution, ReceiptLookup, ResolutionError,
    RetirementError, UncertainOperation,
};
use df_types::{OperationId, RecoveryEpoch, SessionId, SessionRevision};

#[derive(Debug, Clone, PartialEq, Eq)]
enum ServerDecision {
    Accepted { work_pending: bool },
    Rejected { reason: Rejection },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Rejection {
    WrongTurn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TransportError {
    Unavailable,
}

struct ReadPort {
    observations: VecDeque<Result<LookupOutcome<ServerDecision>, TransportError>>,
    requested: Vec<OperationKey>,
}

impl ReadPort {
    fn new(observations: Vec<Result<LookupOutcome<ServerDecision>, TransportError>>) -> Self {
        Self {
            observations: observations.into(),
            requested: Vec::new(),
        }
    }
}

impl ReceiptLookup for ReadPort {
    type Receipt = ServerDecision;
    type Error = TransportError;

    fn lookup(
        &mut self,
        key: OperationKey,
    ) -> impl Future<Output = Result<LookupOutcome<Self::Receipt>, Self::Error>> {
        self.requested.push(key);
        ready(self.observations.pop_front().unwrap())
    }
}

fn poll_once<Output>(future: Pin<&mut impl Future<Output = Output>>) -> Poll<Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}

fn ready_result<Output>(future: impl Future<Output = Output>) -> Output {
    let mut future = std::pin::pin!(future);
    match poll_once(future.as_mut()) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("fixture must complete in one poll"),
    }
}

fn key() -> OperationKey {
    OperationKey::new(
        SessionId::from_bytes(&[1; 16]).unwrap(),
        OperationId::from_bytes(&[2; 16]).unwrap(),
        RecoveryEpoch::new(4).unwrap(),
    )
}

fn revision() -> SessionRevision {
    SessionRevision::new(key().epoch(), 7)
}

fn uncertain() -> UncertainOperation<String, ServerDecision> {
    UncertainOperation::new(key(), "original canonical intent".to_owned())
}

#[test]
fn lost_acknowledgment_looks_up_original_key_and_preserves_pending_work_receipt() {
    let mut operation = uncertain();
    let receipt = ServerDecision::Accepted { work_pending: true };
    let mut port = ReadPort::new(vec![Ok(LookupOutcome::Committed {
        revision: revision(),
        receipt: receipt.clone(),
    })]);

    assert_eq!(
        ready_result(operation.resolve(&mut port)).unwrap(),
        &OperationResolution::Committed {
            revision: revision(),
            receipt,
        }
    );
    assert_eq!(port.requested, vec![key()]);
    assert_eq!(operation.intent(), "original canonical intent");
    assert_eq!(operation.key(), key());
    ready_result(operation.resolve(&mut port)).unwrap();
    assert_eq!(port.requested, vec![key()]);
}

#[test]
fn committed_rejection_is_retained_without_resubmitting_or_reclassifying_as_failure() {
    let mut operation = uncertain();
    let receipt = ServerDecision::Rejected {
        reason: Rejection::WrongTurn,
    };
    let mut port = ReadPort::new(vec![Ok(LookupOutcome::Committed {
        revision: revision(),
        receipt: receipt.clone(),
    })]);

    ready_result(operation.resolve(&mut port)).unwrap();
    ready_result(operation.resolve(&mut port)).unwrap();
    assert_eq!(
        operation.resolution(),
        &OperationResolution::Committed {
            revision: revision(),
            receipt,
        }
    );
    assert_eq!(port.requested, vec![key()]);
    assert_eq!(
        operation.retire_namespace(RecoveryEpoch::new(5).unwrap()),
        Err(RetirementError::AlreadyResolved)
    );
}

#[test]
fn in_progress_queries_same_operation_until_server_decision_exists() {
    let mut operation = uncertain();
    let mut port = ReadPort::new(vec![
        Ok(LookupOutcome::InProgress),
        Ok(LookupOutcome::Committed {
            revision: revision(),
            receipt: ServerDecision::Accepted {
                work_pending: false,
            },
        }),
    ]);
    assert_eq!(
        ready_result(operation.resolve(&mut port)).unwrap(),
        &OperationResolution::InProgress
    );
    ready_result(operation.resolve(&mut port)).unwrap();
    assert_eq!(port.requested, vec![key(), key()]);
    assert_eq!(operation.key(), key());
}

#[test]
fn not_recorded_and_expired_remain_unresolved_and_never_replace_operation_key() {
    let mut operation = uncertain();
    let mut port = ReadPort::new(vec![
        Ok(LookupOutcome::NotRecorded),
        Ok(LookupOutcome::ExpiredOrIndeterminate),
    ]);
    assert_eq!(
        ready_result(operation.resolve(&mut port)).unwrap(),
        &OperationResolution::NotRecorded
    );
    assert_eq!(
        ready_result(operation.resolve(&mut port)).unwrap(),
        &OperationResolution::ExpiredOrIndeterminate
    );
    assert_eq!(operation.intent(), "original canonical intent");
    assert_eq!(operation.key(), key());
    assert_eq!(port.requested, vec![key(), key()]);
}

#[test]
fn lookup_failure_preserves_uncertainty_and_same_key_for_later_read() {
    let mut operation = uncertain();
    let mut port = ReadPort::new(vec![
        Err(TransportError::Unavailable),
        Ok(LookupOutcome::InProgress),
    ]);
    assert_eq!(
        ready_result(operation.resolve(&mut port)),
        Err(ResolutionError::Lookup(TransportError::Unavailable))
    );
    assert_eq!(operation.resolution(), &OperationResolution::Uncertain);
    assert_eq!(operation.intent(), "original canonical intent");
    ready_result(operation.resolve(&mut port)).unwrap();
    assert_eq!(port.requested, vec![key(), key()]);
}

struct PendingPort {
    requested: Option<OperationKey>,
    dropped: bool,
}

struct PendingLookup<'a> {
    dropped: &'a mut bool,
}

impl Future for PendingLookup<'_> {
    type Output = Result<LookupOutcome<ServerDecision>, TransportError>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

impl Drop for PendingLookup<'_> {
    fn drop(&mut self) {
        *self.dropped = true;
    }
}

impl ReceiptLookup for PendingPort {
    type Receipt = ServerDecision;
    type Error = TransportError;

    fn lookup(
        &mut self,
        key: OperationKey,
    ) -> impl Future<Output = Result<LookupOutcome<Self::Receipt>, Self::Error>> {
        self.requested = Some(key);
        PendingLookup {
            dropped: &mut self.dropped,
        }
    }
}

#[test]
fn cancelled_owned_wait_preserves_original_operation_for_later_lookup() {
    let mut operation = uncertain();
    let mut pending_port = PendingPort {
        requested: None,
        dropped: false,
    };
    {
        let mut wait = std::pin::pin!(operation.resolve(&mut pending_port));
        assert!(poll_once(wait.as_mut()).is_pending());
    }
    assert_eq!(pending_port.requested, Some(key()));
    assert!(pending_port.dropped);
    assert_eq!(operation.resolution(), &OperationResolution::Uncertain);
    assert_eq!(operation.intent(), "original canonical intent");
    let mut recovered_port = ReadPort::new(vec![Ok(LookupOutcome::InProgress)]);
    ready_result(operation.resolve(&mut recovered_port)).unwrap();
    assert_eq!(recovered_port.requested, vec![key()]);
}

#[test]
fn recovery_retires_original_namespace_without_lookup_or_new_key() {
    let mut operation = uncertain();
    let old_epoch = key().epoch();
    assert_eq!(
        operation.retire_namespace(old_epoch),
        Err(RetirementError::NotNewer {
            current: old_epoch,
            supplied: old_epoch,
        })
    );
    let restored_epoch = RecoveryEpoch::new(5).unwrap();
    operation.retire_namespace(restored_epoch).unwrap();
    let mut port = ReadPort::new(vec![]);
    assert_eq!(
        ready_result(operation.resolve(&mut port)).unwrap(),
        &OperationResolution::NamespaceRetired {
            current_epoch: restored_epoch,
        }
    );
    assert_eq!(operation.key(), key());
    assert_eq!(operation.intent(), "original canonical intent");
    assert!(port.requested.is_empty());
    assert_eq!(
        operation.retire_namespace(old_epoch),
        Err(RetirementError::NotNewer {
            current: restored_epoch,
            supplied: old_epoch,
        })
    );
}

#[test]
fn cross_epoch_committed_response_is_refused_without_fabricating_receipt() {
    let mut operation = uncertain();
    let wrong_epoch = RecoveryEpoch::new(5).unwrap();
    let mut port = ReadPort::new(vec![Ok(LookupOutcome::Committed {
        revision: SessionRevision::new(wrong_epoch, 0),
        receipt: ServerDecision::Accepted {
            work_pending: false,
        },
    })]);
    assert_eq!(
        ready_result(operation.resolve(&mut port)),
        Err(ResolutionError::WrongEpoch {
            expected: key().epoch(),
            actual: wrong_epoch,
        })
    );
    assert_eq!(operation.resolution(), &OperationResolution::Uncertain);
    assert_eq!(operation.key(), key());
}
