use std::time::Duration;

use df_model::checkpoint::{
    AssetRequestKey, AudienceScope, Basis, ContentDigest, ExecutionMode, JobId, RecordId,
};
use df_provider_api::{
    BudgetMutation, CheckedRequest, ProviderBillingClass, ProviderLiabilityDisposition,
    ProviderNextAction, ProviderOutcomeError, ProviderResultClass, ProviderRetryPolicy,
    RequestBinding, RequestIdentity, RequestLimits, RequestOwnerState, RequestUsage,
};
use df_providers::{
    CandidateRouteId, FalQueueObservation, ProviderAttemptIdentity, ProviderFailureClass,
    ProviderImageMetadata, ProviderRequestId, ReconciliationInput, ReconciliationLookup,
    classify_fal_status_response, reconcile_provider_attempt, reconciliation_capability,
    validate_elevenlabs_audio,
};
use df_types::{
    OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision, Usage, UsageUnit,
};

fn operation(value: u8) -> OperationId {
    OperationId::from_bytes(&[value; 16]).unwrap()
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
            operation: operation(6),
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

fn fal_attempt(request_id: ProviderRequestId) -> ProviderAttemptIdentity {
    ProviderAttemptIdentity::new(
        operation(6),
        CandidateRouteId::FalFluxSchnellDisposableImage,
        Some(request_id),
    )
}

fn completed_fal(request_id: &ProviderRequestId) -> FalQueueObservation {
    classify_fal_status_response(
        200,
        request_id,
        Some(request_id.as_str()),
        Some("COMPLETED"),
        None,
        None,
        Some(1.25),
    )
    .unwrap()
}

fn fal_image() -> ProviderImageMetadata {
    ProviderImageMetadata {
        url: "https://v3.fal.media/files/fixture.jpg".to_owned(),
        content_type: "image/jpeg".to_owned(),
        width: 1024,
        height: 768,
    }
}

#[test]
fn absent_submit_response_and_not_found_preserve_unknown_liability() {
    let fixture = fixture();
    let request_id = ProviderRequestId::new("fal-request-7").unwrap();
    let not_found =
        classify_fal_status_response(404, &request_id, None, None, None, None, None).unwrap();
    let pending = classify_fal_status_response(
        200,
        &request_id,
        Some(request_id.as_str()),
        Some("IN_QUEUE"),
        None,
        None,
        None,
    )
    .unwrap();
    for supplier in [
        ReconciliationInput::UnknownSupplierOutcome,
        ReconciliationInput::FalStatus(&not_found),
        ReconciliationInput::FalStatus(&pending),
    ] {
        let decision = reconcile_provider_attempt(
            &fixture.request,
            &fal_attempt(request_id.clone()),
            owner(&fixture.current),
            supplier,
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
}

#[test]
fn classified_http_failure_cancellation_and_deadline_preserve_unknown_liability() {
    let fixture = fixture();
    let attempt = fal_attempt(ProviderRequestId::new("fal-request-7").unwrap());
    let cases = [
        (
            ProviderFailureClass::Unknown,
            owner(&fixture.current),
            ProviderResultClass::Failed(ProviderFailureClass::Unknown),
            false,
        ),
        (
            ProviderFailureClass::Cancelled,
            RequestOwnerState {
                current: Some(&fixture.current),
                elapsed: Duration::ZERO,
                cancelled: true,
            },
            ProviderResultClass::Failed(ProviderFailureClass::Cancelled),
            true,
        ),
        (
            ProviderFailureClass::Deadline,
            RequestOwnerState {
                current: Some(&fixture.current),
                elapsed: Duration::from_secs(2),
                cancelled: false,
            },
            ProviderResultClass::Failed(ProviderFailureClass::Deadline),
            true,
        ),
    ];

    for (failure, owner, expected_result, owner_rejected) in cases {
        let decision = reconcile_provider_attempt(
            &fixture.request,
            &attempt,
            owner,
            ReconciliationInput::ClassifiedFailure(failure),
            ProviderBillingClass::MissingOrAmbiguous,
            &BudgetMutation::<(), (), ()>::Unknown,
            retry_policy(),
        )
        .unwrap();

        assert_eq!(decision.result, expected_result);
        assert_eq!(
            decision.liability,
            ProviderLiabilityDisposition::RetainWorstCase
        );
        assert_eq!(decision.next, ProviderNextAction::ReconcileSameOperation);
        assert_eq!(decision.current_request.is_err(), owner_rejected);
    }
}

#[test]
fn operation_and_fal_request_identity_are_bound_to_the_stored_attempt() {
    let fixture = fixture();
    let expected = ProviderRequestId::new("fal-request-7").unwrap();
    let other = ProviderRequestId::new("other-request-8").unwrap();
    let mismatch = completed_fal(&other);
    let decision = reconcile_provider_attempt(
        &fixture.request,
        &fal_attempt(expected.clone()),
        owner(&fixture.current),
        ReconciliationInput::FalStatus(&mismatch),
        ProviderBillingClass::MissingOrAmbiguous,
        &BudgetMutation::<(), (), ()>::Unknown,
        retry_policy(),
    )
    .unwrap();
    assert_eq!(decision.result, ProviderResultClass::Incomplete);

    let matching = completed_fal(&expected);
    let wrong_route = ProviderAttemptIdentity::new(
        operation(6),
        CandidateRouteId::ElevenFlashV25Tts,
        Some(ProviderRequestId::new("fal-request-7").unwrap()),
    );
    let wrong_route_decision = reconcile_provider_attempt(
        &fixture.request,
        &wrong_route,
        owner(&fixture.current),
        ReconciliationInput::FalStatus(&matching),
        ProviderBillingClass::MissingOrAmbiguous,
        &BudgetMutation::<(), (), ()>::Unknown,
        retry_policy(),
    )
    .unwrap();
    assert_eq!(wrong_route_decision.result, ProviderResultClass::Incomplete);

    let cross_operation = ProviderAttemptIdentity::new(
        operation(99),
        CandidateRouteId::FalFluxSchnellDisposableImage,
        Some(expected),
    );
    assert_eq!(
        reconcile_provider_attempt(
            &fixture.request,
            &cross_operation,
            owner(&fixture.current),
            ReconciliationInput::ClassifiedFailure(ProviderFailureClass::Unknown),
            ProviderBillingClass::MissingOrAmbiguous,
            &BudgetMutation::<(), (), ()>::Unknown,
            retry_policy(),
        ),
        Err(ProviderOutcomeError::OperationMismatch)
    );
}

#[test]
fn eleven_complete_stream_is_output_evidence_but_partial_or_unbound_id_is_unknown() {
    let fixture = fixture();
    let stream_id = ProviderRequestId::new("eleven-request-9").unwrap();
    let audio = validate_elevenlabs_audio(
        200,
        "audio/mpeg",
        true,
        512,
        Some(stream_id.as_str()),
        None,
        Some("250"),
    )
    .unwrap();
    let attempt = ProviderAttemptIdentity::new(
        operation(6),
        CandidateRouteId::ElevenFlashV25Tts,
        Some(stream_id.clone()),
    );
    let complete = reconcile_provider_attempt(
        &fixture.request,
        &attempt,
        owner(&fixture.current),
        ReconciliationInput::ElevenLabsStream(&audio),
        ProviderBillingClass::MissingOrAmbiguous,
        &BudgetMutation::<(), (), ()>::Unknown,
        retry_policy(),
    )
    .unwrap();
    assert_eq!(complete.result, ProviderResultClass::Complete);
    assert_eq!(
        complete.liability,
        ProviderLiabilityDisposition::RetainWorstCase
    );
    assert_eq!(complete.next, ProviderNextAction::ReconcileSameOperation);

    let unbound =
        ProviderAttemptIdentity::new(operation(6), CandidateRouteId::ElevenFlashV25Tts, None);
    let unbound_decision = reconcile_provider_attempt(
        &fixture.request,
        &unbound,
        owner(&fixture.current),
        ReconciliationInput::ElevenLabsStream(&audio),
        ProviderBillingClass::MissingOrAmbiguous,
        &BudgetMutation::<(), (), ()>::Unknown,
        retry_policy(),
    )
    .unwrap();
    assert_eq!(unbound_decision.result, ProviderResultClass::Incomplete);

    assert!(validate_elevenlabs_audio(200, "audio/mpeg", false, 512, None, None, None).is_err());
}

#[test]
fn fal_completed_status_alone_is_not_complete_output() {
    let fixture = fixture();
    let request_id = ProviderRequestId::new("fal-request-7").unwrap();
    let status = completed_fal(&request_id);
    let decision = reconcile_provider_attempt(
        &fixture.request,
        &fal_attempt(request_id.clone()),
        owner(&fixture.current),
        ReconciliationInput::FalStatus(&status),
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

    let invalid_images = [ProviderImageMetadata {
        content_type: "text/html".to_owned(),
        ..fal_image()
    }];
    let invalid_result = reconcile_provider_attempt(
        &fixture.request,
        &fal_attempt(request_id),
        owner(&fixture.current),
        ReconciliationInput::FalResult {
            observation: &status,
            images: &invalid_images,
        },
        ProviderBillingClass::MissingOrAmbiguous,
        &BudgetMutation::<(), (), ()>::Unknown,
        retry_policy(),
    )
    .unwrap();
    assert_eq!(invalid_result.result, ProviderResultClass::Incomplete);
}

#[test]
fn fal_validated_same_id_result_is_complete_but_needs_verified_settlement() {
    let fixture = fixture();
    let request_id = ProviderRequestId::new("fal-request-7").unwrap();
    let status = completed_fal(&request_id);
    let images = [fal_image()];
    let attempt = fal_attempt(request_id);

    let pending_settlement = reconcile_provider_attempt(
        &fixture.request,
        &attempt,
        owner(&fixture.current),
        ReconciliationInput::FalResult {
            observation: &status,
            images: &images,
        },
        ProviderBillingClass::VerifiedUnused,
        &BudgetMutation::<(), (), ()>::Unknown,
        retry_policy(),
    )
    .unwrap();
    assert_eq!(pending_settlement.result, ProviderResultClass::Complete);
    assert_eq!(
        pending_settlement.liability,
        ProviderLiabilityDisposition::RetainWorstCase
    );
    assert_eq!(
        pending_settlement.next,
        ProviderNextAction::ReconcileSameOperation
    );

    let committed_settlement = reconcile_provider_attempt(
        &fixture.request,
        &attempt,
        owner(&fixture.current),
        ReconciliationInput::FalResult {
            observation: &status,
            images: &images,
        },
        ProviderBillingClass::VerifiedUnused,
        &BudgetMutation::<(), (), ()>::Committed {
            result: (),
            replayed: false,
        },
        retry_policy(),
    )
    .unwrap();
    assert_eq!(
        committed_settlement.liability,
        ProviderLiabilityDisposition::KnownUnused
    );
    assert_eq!(committed_settlement.next, ProviderNextAction::Complete);

    let committed_without_billing = reconcile_provider_attempt(
        &fixture.request,
        &attempt,
        owner(&fixture.current),
        ReconciliationInput::FalResult {
            observation: &status,
            images: &images,
        },
        ProviderBillingClass::MissingOrAmbiguous,
        &BudgetMutation::<(), (), ()>::Committed {
            result: (),
            replayed: false,
        },
        retry_policy(),
    )
    .unwrap();
    assert_eq!(
        committed_without_billing.liability,
        ProviderLiabilityDisposition::RetainWorstCase
    );
    assert_eq!(
        committed_without_billing.next,
        ProviderNextAction::ReconcileSameOperation
    );
}

#[test]
fn routes_do_not_claim_submission_idempotency() {
    let eleven = reconciliation_capability(CandidateRouteId::ElevenFlashV25Tts);
    let fal = reconciliation_capability(CandidateRouteId::FalFluxSchnellDisposableImage);
    assert_eq!(eleven.lookup(), ReconciliationLookup::ResponseMetadataOnly);
    assert_eq!(fal.lookup(), ReconciliationLookup::CapturedRequestId);
    assert!(!eleven.submission_idempotency_documented());
    assert!(!fal.submission_idempotency_documented());
}
