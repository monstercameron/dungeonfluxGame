use std::future::Future;

use df_types::{OperationId, RecoveryEpoch, SessionId, SessionRevision};

/// Original server-owned operation namespace. These values confer no authorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperationKey {
    session: SessionId,
    operation: OperationId,
    epoch: RecoveryEpoch,
}

impl OperationKey {
    pub fn new(session: SessionId, operation: OperationId, epoch: RecoveryEpoch) -> Self {
        Self {
            session,
            operation,
            epoch,
        }
    }

    pub fn session(self) -> SessionId {
        self.session
    }

    pub fn operation(self) -> OperationId {
        self.operation
    }

    pub fn epoch(self) -> RecoveryEpoch {
        self.epoch
    }
}

/// An authorized lookup's observation; absence never establishes a decision.
/// The owning receipt type retains its accepted/rejected and pending-work semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LookupOutcome<Receipt> {
    Committed {
        revision: SessionRevision,
        receipt: Receipt,
    },
    InProgress,
    NotRecorded,
    ExpiredOrIndeterminate,
}

/// Caller-owned read boundary. Its owner authorizes and correlates the response
/// with this exact key and maps its actual receipt type without issuing mutations.
/// The borrowed future owns its request lifetime and need not be `Send` on WASM.
pub trait ReceiptLookup {
    type Receipt;
    type Error;

    fn lookup(
        &mut self,
        key: OperationKey,
    ) -> impl Future<Output = Result<LookupOutcome<Self::Receipt>, Self::Error>>;
}

/// Presentation state for one original operation, independent of game authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationResolution<Receipt> {
    Uncertain,
    InProgress,
    NotRecorded,
    ExpiredOrIndeterminate,
    Committed {
        revision: SessionRevision,
        receipt: Receipt,
    },
    NamespaceRetired {
        current_epoch: RecoveryEpoch,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionError<LookupError> {
    Lookup(LookupError),
    WrongEpoch {
        expected: RecoveryEpoch,
        actual: RecoveryEpoch,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetirementError {
    AlreadyResolved,
    NotNewer {
        current: RecoveryEpoch,
        supplied: RecoveryEpoch,
    },
}

/// Owns one ambiguously submitted intent until a server receipt or explicit
/// namespace retirement. It has no mutation, operation replacement or retry API.
/// Dropping a lookup future stops only that wait; committed server work continues.
pub struct UncertainOperation<Intent, Receipt> {
    key: OperationKey,
    intent: Intent,
    resolution: OperationResolution<Receipt>,
}

impl<Intent, Receipt> UncertainOperation<Intent, Receipt> {
    /// Takes the exact original canonical intent and key after an ambiguous submit.
    /// The caller retains authorization and request-fingerprint responsibilities.
    pub fn new(key: OperationKey, intent: Intent) -> Self {
        Self {
            key,
            intent,
            resolution: OperationResolution::Uncertain,
        }
    }

    pub fn key(&self) -> OperationKey {
        self.key
    }

    pub fn intent(&self) -> &Intent {
        &self.intent
    }

    pub fn resolution(&self) -> &OperationResolution<Receipt> {
        &self.resolution
    }

    /// Executes only a read of the original operation. Unresolved outcomes retain
    /// the intent; cancellation or lookup failure leaves it explicitly uncertain.
    /// Exclusive borrowing prevents an old in-flight completion changing a newer
    /// owner state. The caller must cancel/drop its wait before changing this owner.
    pub async fn resolve<Lookup>(
        &mut self,
        lookup: &mut Lookup,
    ) -> Result<&OperationResolution<Receipt>, ResolutionError<Lookup::Error>>
    where
        Lookup: ReceiptLookup<Receipt = Receipt>,
    {
        if matches!(
            self.resolution,
            OperationResolution::Committed { .. } | OperationResolution::NamespaceRetired { .. }
        ) {
            return Ok(&self.resolution);
        }

        self.resolution = OperationResolution::Uncertain;
        let outcome = lookup
            .lookup(self.key)
            .await
            .map_err(ResolutionError::Lookup)?;
        self.resolution = match outcome {
            LookupOutcome::Committed { revision, receipt } => {
                if revision.epoch() != self.key.epoch {
                    return Err(ResolutionError::WrongEpoch {
                        expected: self.key.epoch,
                        actual: revision.epoch(),
                    });
                }
                OperationResolution::Committed { revision, receipt }
            }
            LookupOutcome::InProgress => OperationResolution::InProgress,
            LookupOutcome::NotRecorded => OperationResolution::NotRecorded,
            LookupOutcome::ExpiredOrIndeterminate => OperationResolution::ExpiredOrIndeterminate,
        };
        Ok(&self.resolution)
    }

    /// Records a caller-verified recovery advance without issuing an epoch or
    /// recreating the original intent in the new namespace. Retirement is permanent.
    pub fn retire_namespace(&mut self, supplied: RecoveryEpoch) -> Result<(), RetirementError> {
        if matches!(self.resolution, OperationResolution::Committed { .. }) {
            return Err(RetirementError::AlreadyResolved);
        }
        let current = match self.resolution {
            OperationResolution::NamespaceRetired { current_epoch } => current_epoch,
            _ => self.key.epoch,
        };
        if supplied <= current {
            return Err(RetirementError::NotNewer { current, supplied });
        }
        self.resolution = OperationResolution::NamespaceRetired {
            current_epoch: supplied,
        };
        Ok(())
    }
}
