//! Admission and shutdown policy facts; this module does not own or shut down runtime services.
use df_session::effects::{
    DispatchKnowledge, DispatchLookup, DispatchRecord, EffectRepositoryError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AdmissionMode {
    Serving,
    Incident,
    ShuttingDown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AdmissionRefusal {
    Incident,
    ShuttingDown,
    JournalMissing,
    JournalPending,
    JournalUnavailable,
    PossibleSendUnresolved,
    UnresolvedClaimWithoutIdentity,
    ConflictingRecordedIdentity,
}

/// A hold carries both identities when a journal result conflicts with a claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AdmissionHold<A, R, K> {
    pub(crate) reason: AdmissionRefusal,
    pub(crate) latest_record: Option<DispatchRecord<A, R, K>>,
    pub(crate) attempted_record: Option<DispatchRecord<A, R, K>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum AdmissionDecision<A, R, K> {
    Admitted,
    Held(AdmissionHold<A, R, K>),
}

/// The caller supplies its latest authenticated journal/repository observation.
/// Missing or ambiguous evidence closes affected admissions, including new work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AdmissionPolicy {
    mode: AdmissionMode,
}

impl Default for AdmissionPolicy {
    fn default() -> Self {
        Self {
            mode: AdmissionMode::Serving,
        }
    }
}

impl AdmissionPolicy {
    pub(crate) const fn mode(self) -> AdmissionMode {
        self.mode
    }

    pub(crate) fn enter_incident(&mut self) {
        if self.mode == AdmissionMode::Serving {
            self.mode = AdmissionMode::Incident;
        }
    }

    pub(crate) fn begin_shutdown(&mut self) {
        self.mode = AdmissionMode::ShuttingDown;
    }

    pub(crate) fn admit_new_work<A: PartialEq, R: PartialEq, K: PartialEq>(
        &self,
        latest: Result<DispatchLookup<A, R, K>, EffectRepositoryError>,
        attempted_identity: Option<DispatchRecord<A, R, K>>,
    ) -> AdmissionDecision<A, R, K> {
        self.admit_with_latest(latest, attempted_identity)
    }

    /// Recovery has the same fail-closed journal gate as new admission. This
    /// decision grants no provider retransmission or egress permit.
    pub(crate) fn admit_recovery<A: PartialEq, R: PartialEq, K: PartialEq>(
        &self,
        latest: Result<DispatchLookup<A, R, K>, EffectRepositoryError>,
        attempted_identity: Option<DispatchRecord<A, R, K>>,
    ) -> AdmissionDecision<A, R, K> {
        self.admit_with_latest(latest, attempted_identity)
    }

    fn admit_with_latest<A: PartialEq, R: PartialEq, K: PartialEq>(
        &self,
        latest: Result<DispatchLookup<A, R, K>, EffectRepositoryError>,
        attempted_identity: Option<DispatchRecord<A, R, K>>,
    ) -> AdmissionDecision<A, R, K> {
        let decision = classify_dispatch(latest, attempted_identity);
        let refusal = match self.mode {
            AdmissionMode::Serving => return decision,
            AdmissionMode::Incident => AdmissionRefusal::Incident,
            AdmissionMode::ShuttingDown => AdmissionRefusal::ShuttingDown,
        };
        match decision {
            AdmissionDecision::Admitted => AdmissionDecision::Held(AdmissionHold {
                reason: refusal,
                latest_record: None,
                attempted_record: None,
            }),
            AdmissionDecision::Held(mut hold) => {
                hold.reason = refusal;
                AdmissionDecision::Held(hold)
            }
        }
    }
}

pub(crate) fn classify_dispatch<A: PartialEq, R: PartialEq, K: PartialEq>(
    latest: Result<DispatchLookup<A, R, K>, EffectRepositoryError>,
    attempted_identity: Option<DispatchRecord<A, R, K>>,
) -> AdmissionDecision<A, R, K> {
    match latest {
        Ok(DispatchLookup::Recorded(record)) => {
            let terminal = matches!(
                record.knowledge,
                DispatchKnowledge::Completed | DispatchKnowledge::VerifiedUnsentCanceled
            );
            let conflicting_attempt = attempted_identity.as_ref().is_some_and(|attempted| {
                attempted.attempt_id != record.attempt_id
                    || attempted.reservation_id != record.reservation_id
                    || attempted.provider_key != record.provider_key
            });
            if terminal && !conflicting_attempt {
                AdmissionDecision::Admitted
            } else {
                AdmissionDecision::Held(AdmissionHold {
                    reason: if conflicting_attempt {
                        AdmissionRefusal::ConflictingRecordedIdentity
                    } else {
                        AdmissionRefusal::PossibleSendUnresolved
                    },
                    latest_record: Some(record),
                    attempted_record: attempted_identity,
                })
            }
        }
        Ok(DispatchLookup::Missing) => AdmissionDecision::Held(AdmissionHold {
            reason: AdmissionRefusal::JournalMissing,
            latest_record: None,
            attempted_record: attempted_identity,
        }),
        Ok(DispatchLookup::Pending) => AdmissionDecision::Held(AdmissionHold {
            reason: AdmissionRefusal::JournalPending,
            latest_record: None,
            attempted_record: attempted_identity,
        }),
        Err(EffectRepositoryError::UnresolvedClaim) => AdmissionDecision::Held(AdmissionHold {
            reason: if attempted_identity.is_some() {
                AdmissionRefusal::PossibleSendUnresolved
            } else {
                AdmissionRefusal::UnresolvedClaimWithoutIdentity
            },
            latest_record: None,
            attempted_record: attempted_identity,
        }),
        Err(_) => AdmissionDecision::Held(AdmissionHold {
            reason: AdmissionRefusal::JournalUnavailable,
            latest_record: None,
            attempted_record: attempted_identity,
        }),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DrainStep {
    StopProducers,
    DrainAcceptedInputs,
    CloseRepositoryOnActorThread,
    JoinActorWhileRuntimeAlive,
    ShutdownTelemetry,
}

impl DrainStep {
    const ORDER: [Self; 5] = [
        Self::StopProducers,
        Self::DrainAcceptedInputs,
        Self::CloseRepositoryOnActorThread,
        Self::JoinActorWhileRuntimeAlive,
        Self::ShutdownTelemetry,
    ];
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DrainRefusal {
    WrongStep {
        expected: DrainStep,
        received: DrainStep,
    },
    StepFailed(DrainStep),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DrainStatus {
    Incomplete { next: DrainStep },
    HeldPossibleSends { count: usize },
    Complete,
}

pub(crate) const MAX_RETAINED_POSSIBLE_SENDS: usize = 256;

/// The caller retains ownership of a record rejected by the bounded policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RetainRefusal<A, R, K> {
    Capacity(DispatchRecord<A, R, K>),
    Allocation(DispatchRecord<A, R, K>),
}

/// Failed steps remain owned and retryable. Possible-send records survive every incomplete
/// result so a deadline or close failure cannot hide their exact attempt/provider identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DrainPolicy<A, R, K> {
    completed_steps: usize,
    possible_sends: Vec<DispatchRecord<A, R, K>>,
}

impl<A, R, K> Default for DrainPolicy<A, R, K> {
    fn default() -> Self {
        Self {
            completed_steps: 0,
            possible_sends: Vec::new(),
        }
    }
}

impl<A: PartialEq, R: PartialEq, K: PartialEq> DrainPolicy<A, R, K> {
    pub(crate) fn status(&self) -> DrainStatus {
        if self.completed_steps < DrainStep::ORDER.len() {
            DrainStatus::Incomplete {
                next: DrainStep::ORDER[self.completed_steps],
            }
        } else if self.possible_sends.is_empty() {
            DrainStatus::Complete
        } else {
            DrainStatus::HeldPossibleSends {
                count: self.possible_sends.len(),
            }
        }
    }

    pub(crate) fn retain_possible_send(
        &mut self,
        record: DispatchRecord<A, R, K>,
    ) -> Result<(), RetainRefusal<A, R, K>> {
        if self.possible_sends.iter().any(|existing| {
            existing.attempt_id == record.attempt_id
                && existing.reservation_id == record.reservation_id
                && existing.provider_key == record.provider_key
        }) {
            return Ok(());
        }
        if self.possible_sends.len() == MAX_RETAINED_POSSIBLE_SENDS {
            return Err(RetainRefusal::Capacity(record));
        }
        if self.possible_sends.try_reserve(1).is_err() {
            return Err(RetainRefusal::Allocation(record));
        }
        self.possible_sends.push(record);
        Ok(())
    }

    pub(crate) fn possible_sends(&self) -> &[DispatchRecord<A, R, K>] {
        &self.possible_sends
    }

    /// Advance only after the owning component reports success. A failure, including repository
    /// close or telemetry deadline, leaves the failed step and retained operation identities.
    pub(crate) fn observe(
        &mut self,
        step: DrainStep,
        succeeded: bool,
    ) -> Result<DrainStatus, DrainRefusal> {
        if self.completed_steps == DrainStep::ORDER.len() {
            return Err(DrainRefusal::WrongStep {
                expected: DrainStep::ShutdownTelemetry,
                received: step,
            });
        }
        let expected = DrainStep::ORDER[self.completed_steps];
        if step != expected {
            return Err(DrainRefusal::WrongStep {
                expected,
                received: step,
            });
        }
        if !succeeded {
            return Err(DrainRefusal::StepFailed(step));
        }
        self.completed_steps += 1;
        Ok(self.status())
    }
}
