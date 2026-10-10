use std::time::Duration;

use df_model::checkpoint::{
    AssetRequestKey, AudienceScope, Basis, ContentDigest, ExecutionMode, JobId, RecordId,
};
use df_provider_api::{
    BudgetMutation, CheckedRequest, ProviderBillingClass, ProviderLiabilityDisposition,
    ProviderNextAction, ProviderResultClass, ProviderRetryPolicy, RequestBinding, RequestIdentity,
    RequestLimits, RequestOwnerState, RequestUsage,
};
use df_providers::{
    CandidateRouteId, FalQueueObservation, ProviderRequestId, ReconciliationInput,
    ReconciliationLookup, classify_fal_status_response, reconcile_provider_attempt,
    reconciliation_capability,
};
use df_types::{OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision};
use df_types::{Usage, UsageUnit};

fn operation() -> OperationId {
    OperationId::from_bytes(&[6; 16]).unwrap()
}

fn binding() -> RequestBinding<AssetRequestKey> {
    RequestBinding {
        identity: RequestIdentity {
            basis: Basis {
                session: SessionId::from_bytes(&[1; 16]).unwrap(),
                run: RunId::from_bytes(&[2; 16]).unwrap(),
                revision: SessionRevision::new(RecoveryEpoch::new(3).unwrap(), 4),
            },
            job: JobId::from_bytes(&[5; 16]).unwrap(),
            operation: operation(),
            generation: 7,
        },
        semantic_basis: AssetRequestKey {
            schema: 1,
            source: ContentDigest([8; 32]),
            moment: RecordId::from_bytes(&[9; 16]).unwrap(),
            identity: RevisionLabel::new(Some("identity")).unwrap(),
            style: RevisionLabel::new(Some("style")).unwrap(),
            voice: None,
            provider: RevisionLabel::new(Some("finite-fixture")).unwrap(),
            model: RevisionLabel::new(Some("fixture-model")).unwrap(),
            format: RevisionLabel::new(Some("fixture-format")).unwrap(),
            references: vec![],
            audience: AudienceScope::Host,
            parameters: RevisionLabel::new(Some("fixture-parameters")).unwrap(),
        },
        mode: ExecutionMode::Live,
        deadline: Duration::from_secs(2),
    }
}

struct Fixture {
    current: RequestBinding<AssetRequestKey>,
    request: CheckedRequest<AssetRequestKey>,
}

fn fixture() -> Fixture {
    let current = binding();
    let request = CheckedRequest::new(
        binding(),
        b"bounded fixture",
        RequestUsage::new(1, Duration::ZERO, Usage::new(1, UsageUnit::Token)),
        RequestLimits::new(32, 1, Duration::ZERO, Usage::new(1, UsageUnit::Token)).unwrap(),
        RequestOwnerState {
            current: Some(&current),
            elapsed: Duration::ZERO,
            cancelled: false,
        },
    )
    .unwrap();
    Fixture { current, request }
}

fn owner<'a>(
    current: &'a RequestBinding<AssetRequestKey>,
) -> RequestOwnerState<'a, AssetRequestKey> {
    RequestOwnerState {
        current: Some(current),
        elapsed: Duration::ZERO,
        cancelled: false,
    }
}

fn retry_policy() -> ProviderRetryPolicy {
    ProviderRetryPolicy {
        attempts_remaining: 3,
        retry: true,
        fallback: true,
    }
}

#[test]
fn missing_submit_response_preserves_unknown_liability_without_resend() {
    let fixture = fixture();
    let decision = reconcile_provider_attempt(
        &fixture.request,
        owner(&fixture.current),
        ReconciliationInput::MissingSubmitResponse,
        ProviderBillingClass::MissingOrAmbiguous,
        &BudgetMutation::<(), (), ()>::Unknown,
        retry_policy(),
    )
    .unwrap();

    assert_eq!(decision.result, ProviderResultClass::Incomplete);
    assert_eq!(
        decision.liability,
        ProviderLiabilityDisposition::RetainWorstCase
    );
    assert_eq!(decision.next, ProviderNextAction::ReconcileSameOperation);
}

#[test]
fn same_id_fal_completion_is_output_evidence_not_billing_or_idempotency_proof() {
    let fixture = fixture();
    let expected = ProviderRequestId::new("fal-request-7").unwrap();
    let observation = classify_fal_status_response(
        200,
        &expected,
        Some("fal-request-7"),
        Some("COMPLETED"),
        None,
        None,
        Some(1.25),
    )
    .unwrap();
    assert_eq!(observation.inference_time_seconds(), Some(1.25));
    let decision = reconcile_provider_attempt(
        &fixture.request,
        owner(&fixture.current),
        ReconciliationInput::FalStatus {
            expected_request_id: Some(&expected),
            observation: &observation,
        },
        ProviderBillingClass::MissingOrAmbiguous,
        &BudgetMutation::<(), (), ()>::Unknown,
        retry_policy(),
    )
    .unwrap();

    assert_eq!(decision.result, ProviderResultClass::Complete);
    assert_eq!(
        decision.liability,
        ProviderLiabilityDisposition::RetainWorstCase
    );
    assert_eq!(decision.next, ProviderNextAction::ReconcileSameOperation);
}

#[test]
fn absent_mismatched_and_not_found_fal_observations_remain_incomplete() {
    let expected = ProviderRequestId::new("expected-1").unwrap();
    let other = ProviderRequestId::new("other-2").unwrap();
    let mismatch = FalQueueObservation::Completed {
        request_id: other,
        inference_time: Some(0.5),
    };
    let not_found =
        classify_fal_status_response(404, &expected, None, None, None, None, None).unwrap();
    for (expected_id, observation) in [
        (Some(&expected), &mismatch),
        (Some(&expected), &not_found),
        (None, &mismatch),
    ] {
        let fixture = fixture();
        let decision = reconcile_provider_attempt(
            &fixture.request,
            owner(&fixture.current),
            ReconciliationInput::FalStatus {
                expected_request_id: expected_id,
                observation,
            },
            ProviderBillingClass::MissingOrAmbiguous,
            &BudgetMutation::<(), (), ()>::Unknown,
            retry_policy(),
        )
        .unwrap();
        assert_eq!(decision.result, ProviderResultClass::Incomplete);
        assert_eq!(decision.next, ProviderNextAction::ReconcileSameOperation);
    }
}

#[test]
fn route_lookup_does_not_claim_submit_idempotency() {
    let eleven = reconciliation_capability(CandidateRouteId::ElevenFlashV25Tts);
    let fal = reconciliation_capability(CandidateRouteId::FalFluxSchnellDisposableImage);
    assert_eq!(eleven.lookup(), ReconciliationLookup::ResponseMetadataOnly);
    assert_eq!(fal.lookup(), ReconciliationLookup::CapturedRequestId);
    assert!(!eleven.submission_idempotency_documented());
    assert!(!fal.submission_idempotency_documented());
}
