//! Reconciliation facts for the two selected supplier routes.
//!
//! Supplier request IDs are lookup handles, not application `OperationId`s.
//! Neither selected route documents duplicate-safe submission: fal documents
//! status/result lookup only after a request ID is captured, while the selected
//! ElevenLabs stream exposes response metadata without a status lookup. A
//! timeout, missing response, absent status, 404, cancellation, or incomplete
//! stream therefore cannot establish that submission was absent or safe to
//! repeat. This module classifies facts only; it never sends, retries, refunds,
//! or verifies billing.

use df_provider_api::{
    BudgetMutation, CheckedRequest, ProviderAttemptObservation, ProviderBillingClass,
    ProviderFailureClass, ProviderOutcomeDecision, ProviderOutcomeError, ProviderResultClass,
    ProviderRetryPolicy, RequestOwnerState, classify_provider_outcome,
};

use crate::{CandidateRouteId, ElevenLabsAudioObservation, FalQueueObservation, ProviderRequestId};

/// Documented status lookup capability, independent of submit idempotency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReconciliationLookup {
    ResponseMetadataOnly,
    CapturedRequestId,
}

/// Current documented reconciliation capabilities for a selected route.
///
/// `submission_idempotency_documented` is false for both selected routes based
/// on the dated source decision. A positive lookup does not change that fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReconciliationCapability {
    lookup: ReconciliationLookup,
    submission_idempotency_documented: bool,
}

impl ReconciliationCapability {
    pub const fn lookup(self) -> ReconciliationLookup {
        self.lookup
    }

    pub const fn submission_idempotency_documented(self) -> bool {
        self.submission_idempotency_documented
    }
}

/// Read-only supplier evidence presented to the application attempt boundary.
pub enum ReconciliationInput<'a> {
    /// Submit response was lost, absent, or otherwise unusable.
    MissingSubmitResponse,
    /// A clean, validated ElevenLabs stream response; its optional request ID
    /// remains metadata because this route has no documented status lookup.
    ElevenLabsStream(&'a ElevenLabsAudioObservation),
    /// FAL status for the caller's previously captured request ID, if one exists.
    FalStatus {
        expected_request_id: Option<&'a ProviderRequestId>,
        observation: &'a FalQueueObservation,
    },
}

/// Describes lookup and submission idempotency separately for one route.
pub const fn reconciliation_capability(route: CandidateRouteId) -> ReconciliationCapability {
    let lookup = match route {
        CandidateRouteId::ElevenFlashV25Tts => ReconciliationLookup::ResponseMetadataOnly,
        CandidateRouteId::FalFluxSchnellDisposableImage => ReconciliationLookup::CapturedRequestId,
    };
    ReconciliationCapability {
        lookup,
        submission_idempotency_documented: false,
    }
}

/// Applies supplier result facts through the canonical provider outcome policy.
///
/// `billing` and `settlement` must come from the caller's verified same-attempt
/// authority; supplier status, inference time, response headers, and invoice
/// absence cannot establish them. Missing/ambiguous supplier facts map to an
/// incomplete result, so even conclusive billing cannot turn absence into a
/// retry/fallback candidate. This function performs no I/O or mutation.
pub fn reconcile_provider_attempt<Semantic: Eq, Receipt, Refusal, Failure>(
    request: &CheckedRequest<Semantic>,
    owner: RequestOwnerState<'_, Semantic>,
    supplier: ReconciliationInput<'_>,
    billing: ProviderBillingClass,
    settlement: &BudgetMutation<Receipt, Refusal, Failure>,
    policy: ProviderRetryPolicy,
) -> Result<ProviderOutcomeDecision, ProviderOutcomeError> {
    let result = match supplier {
        ReconciliationInput::MissingSubmitResponse => ProviderResultClass::Incomplete,
        ReconciliationInput::ElevenLabsStream(_) => ProviderResultClass::Complete,
        ReconciliationInput::FalStatus {
            expected_request_id: Some(expected),
            observation: FalQueueObservation::Pending(observed),
        } if expected == observed => ProviderResultClass::Incomplete,
        ReconciliationInput::FalStatus {
            expected_request_id: Some(expected),
            observation:
                FalQueueObservation::Completed {
                    request_id: observed,
                    ..
                },
        } if expected == observed => ProviderResultClass::Complete,
        ReconciliationInput::FalStatus {
            expected_request_id: Some(expected),
            observation:
                FalQueueObservation::Failed {
                    request_id: observed,
                    ..
                },
        } if expected == observed => ProviderResultClass::Failed(ProviderFailureClass::Unknown),
        ReconciliationInput::FalStatus { .. } => ProviderResultClass::Incomplete,
    };

    let attempt = ProviderAttemptObservation {
        operation: request.binding().identity.operation,
        result,
        billing,
    };
    classify_provider_outcome(request, owner, attempt, settlement, policy)
}
