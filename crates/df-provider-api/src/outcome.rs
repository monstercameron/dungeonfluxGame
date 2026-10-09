//! Classified provider recovery guidance from supplied native facts.
//!
//! Decision: output completion, billing certainty and new-work eligibility remain
//! separate. A missing invoice is unresolved liability even for complete output.
//! A failed settlement proves no ledger mutation, not no previous provider send.
//! Alternatives rejected: free-on-missing-invoice, automatic uncertain resend,
//! cross-mode fallback, and a second wallet or provider-status parser.
//! Unresolved native contracts: verified invoices/usage and same-attempt receipts,
//! current auth/rights/quotes, durable journal/CAS, vendor status/idempotency, stream
//! cleanup and controlled egress. This module supplies none of those authorities.

use df_model::checkpoint::ExecutionMode;
use df_types::OperationId;

use crate::{BudgetMutation, CheckedRequest, RequestError, RequestOwnerState};

/// Explicit native adapter classification, never inferred from vendor strings.
/// Capacity/unavailability may be candidates under supplied bounded policy;
/// timeout/deadline/cancellation/unknown are not evidence that a send was free.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderFailureClass {
    InvalidRequest,
    Denied,
    Capacity,
    Unavailable,
    Contract,
    Cancelled,
    Deadline,
    Unknown,
}

/// Native-validated output completion, independent of billing certainty.
/// Complete does not establish rights to deliver private output to an audience.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderResultClass {
    Complete,
    Failed(ProviderFailureClass),
    Incomplete,
}

/// Native-supplied billing facts for this exact attempt, not invoice verification.
/// VerifiedUnused requires conclusive unused liability; invoice absence cannot
/// produce it. VerifiedLiability includes billed failed/regenerated waste and
/// overruns; the native ledger must record actual liability, not truncate a quote.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderBillingClass {
    VerifiedUnused,
    VerifiedLiability,
    MissingOrAmbiguous,
}

/// Caller binds the attempt's classified result and billing facts to the admitted
/// operation. Native ownership also validates provider/attempt identity and the
/// settlement receipt; matching an OperationId here does not prove those facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderAttemptObservation {
    pub operation: OperationId,
    pub result: ProviderResultClass,
    pub billing: ProviderBillingClass,
}

/// Explicit finite policy for this operation, without default count or backoff.
/// Remaining attempts count every actually started retry/fallback branch, not
/// merely successful output. Native ownership decrements the bound atomically.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderRetryPolicy {
    pub attempts_remaining: u32,
    pub retry: bool,
    pub fallback: bool,
}

/// Guidance about existing exposure, not a counter mutation or settlement receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderLiabilityDisposition {
    KnownUnused,
    KnownLiability,
    RetainWorstCase,
}

/// No variant is a send permit. Each candidate needs a fresh native admission,
/// current source/rights/grant/quote and branch reservation before egress. An
/// identified fallback stays in the admitted mode; this never switches modes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderNextAction {
    Complete,
    Stop,
    ReconcileSameOperation,
    FreshAdmissionCandidates { retry: bool, fallback: bool },
}

/// Foreign observations never describe the requested operation or create a plan.
/// Refusal leaves the caller's prior reservations and uncertain exposure intact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderOutcomeError {
    OperationMismatch,
}

/// Complete output remains Complete while billing is unknown. Current request
/// errors block new work without erasing completed output or releasing exposure.
/// Cancellation/deadline cleanup belongs to the native stream/attempt owner; an
/// outstanding same-operation reconciliation survives loss of the caller wait.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderOutcomeDecision {
    pub operation: OperationId,
    pub result: ProviderResultClass,
    pub liability: ProviderLiabilityDisposition,
    pub next: ProviderNextAction,
    pub current_request: Result<(), RequestError>,
}

/// Classifies one existing attempt using the actual checked request and the
/// actual BudgetStore settlement/reconciliation outcome. `settlement` must be the
/// native-bound outcome for this same attempt, never reserve/claim acknowledgement.
/// Opaque receipt content, invoice authenticity and source uniqueness remain
/// native-owned; these borrowed values cannot verify them.
///
/// Only a Committed settlement plus conclusive billing facts makes liability
/// known. Pending, Unknown, Refused and Failed all leave prior exposure uncertain:
/// Failed proves no settlement commit, not an unsent or unbilled provider call.
/// Missing invoice retains worst-case liability even after an acknowledged ledger
/// operation. Reconcile the same attempt before considering another paid branch.
///
/// Positive retry/fallback guidance additionally requires successful current
/// CheckedRequest validation, its admitted Live mode, an eligible classified
/// failure and explicit remaining bounded policy. The native consumer still
/// revalidates and reserves each actual branch. No timer, I/O, parser, ledger
/// mutation, provider call, output publication or automatic cleanup runs here.
pub fn classify_provider_outcome<Semantic: Eq, Receipt, Refusal, Failure>(
    request: &CheckedRequest<Semantic>,
    owner: RequestOwnerState<'_, Semantic>,
    observation: ProviderAttemptObservation,
    settlement: &BudgetMutation<Receipt, Refusal, Failure>,
    policy: ProviderRetryPolicy,
) -> Result<ProviderOutcomeDecision, ProviderOutcomeError> {
    let binding = request.binding();
    if observation.operation != binding.identity.operation {
        return Err(ProviderOutcomeError::OperationMismatch);
    }
    let current_request = request.validate_current(owner);
    let liability = if matches!(settlement, BudgetMutation::Committed { .. }) {
        match observation.billing {
            ProviderBillingClass::VerifiedUnused => ProviderLiabilityDisposition::KnownUnused,
            ProviderBillingClass::VerifiedLiability => ProviderLiabilityDisposition::KnownLiability,
            ProviderBillingClass::MissingOrAmbiguous => {
                ProviderLiabilityDisposition::RetainWorstCase
            }
        }
    } else {
        ProviderLiabilityDisposition::RetainWorstCase
    };
    let next = if liability == ProviderLiabilityDisposition::RetainWorstCase {
        ProviderNextAction::ReconcileSameOperation
    } else {
        match observation.result {
            ProviderResultClass::Complete => ProviderNextAction::Complete,
            ProviderResultClass::Failed(
                ProviderFailureClass::Capacity | ProviderFailureClass::Unavailable,
            ) if current_request.is_ok()
                && binding.mode == ExecutionMode::Live
                && policy.attempts_remaining > 0
                && (policy.retry || policy.fallback) =>
            {
                ProviderNextAction::FreshAdmissionCandidates {
                    retry: policy.retry,
                    fallback: policy.fallback,
                }
            }
            ProviderResultClass::Failed(_) | ProviderResultClass::Incomplete => {
                ProviderNextAction::Stop
            }
        }
    };
    Ok(ProviderOutcomeDecision {
        operation: binding.identity.operation,
        result: observation.result,
        liability,
        next,
        current_request,
    })
}
