use std::{fmt::Debug, time::Duration};

use df_model::checkpoint::{Basis, ContentDigest, ExecutionMode, JobId};
use df_provider_api::{
    CheckedModalityRequest, RequestBasis, RequestBinding, RequestError, RequestIdentity,
    RequestIdentityField, RequestLimit, RequestLimits, RequestModality, RequestOwnerState,
    RequestUsage,
};
use df_types::{
    OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision, Usage, UsageUnit,
};

#[derive(Clone, Eq, PartialEq)]
struct PrivateContext(String);

fn label(value: &str) -> RevisionLabel {
    RevisionLabel::new(Some(value)).expect("valid fixture revision")
}

fn identity(value: u8) -> RequestIdentity {
    RequestIdentity {
        basis: Basis {
            session: SessionId::from_bytes(&[value; 16]).expect("valid session id"),
            run: RunId::from_bytes(&[value + 1; 16]).expect("valid run id"),
            revision: SessionRevision::new(
                RecoveryEpoch::new(u64::from(value)).expect("valid recovery epoch"),
                u64::from(value) + 1,
            ),
        },
        job: JobId::from_bytes(&[value + 2; 16]).expect("valid job id"),
        operation: OperationId::from_bytes(&[value + 3; 16]).expect("valid operation id"),
        generation: u64::from(value) + 4,
    }
}

fn basis(modality: RequestModality, value: u8) -> RequestBasis<PrivateContext> {
    RequestBasis {
        modality,
        source: ContentDigest([value; 32]),
        rights_revision: label(&format!("rights-{value}")),
        output_format_revision: label(&format!("format-{value}")),
        semantic: PrivateContext(format!("private-semantic-context-{value}")),
    }
}

fn binding(
    mode: ExecutionMode,
    value: u8,
    semantic_basis: RequestBasis<PrivateContext>,
) -> RequestBinding<RequestBasis<PrivateContext>> {
    RequestBinding {
        identity: identity(value),
        semantic_basis,
        mode,
        deadline: Duration::from_millis(500 + u64::from(value)),
    }
}

fn limits(unit: UsageUnit, value: u128) -> RequestLimits {
    RequestLimits::new(
        64 + usize::from(value as u8),
        20 + value,
        Duration::from_millis(80 + value as u64),
        Usage::new(10 + value, unit),
    )
    .expect("positive byte cap")
}

fn usage(unit: UsageUnit, value: u128) -> RequestUsage {
    RequestUsage::new(
        2 + value,
        Duration::from_millis(10 + value as u64),
        Usage::new(1 + value, unit),
    )
}

fn owner<'a>(
    current: &'a RequestBinding<RequestBasis<PrivateContext>>,
    elapsed: Duration,
) -> RequestOwnerState<'a, RequestBasis<PrivateContext>> {
    RequestOwnerState {
        current: Some(current),
        elapsed,
        cancelled: false,
    }
}

fn assert_private_debug<T: Debug>(value: &T) {
    let debug = format!("{value:?}");
    for secret in [
        "private-payload-secret",
        "private-rights-secret",
        "private-semantic-context",
    ] {
        assert!(!debug.contains(secret), "debug leaked {secret}");
    }
}

#[test]
fn every_modality_uses_the_real_checked_boundary_and_preserves_its_typed_basis() {
    // The unit pairings here only cover the six existing UsageUnit variants; they
    // do not establish provider billing meters for any modality.
    let cases = [
        (RequestModality::Text, UsageUnit::Token),
        (RequestModality::Stt, UsageUnit::Character),
        (RequestModality::Tts, UsageUnit::Byte),
        (RequestModality::Image, UsageUnit::AudioMillisecond),
        (RequestModality::Video, UsageUnit::VideoMillisecond),
        (RequestModality::Sound, UsageUnit::Image),
    ];

    for (index, (modality, unit)) in cases.into_iter().enumerate() {
        let value = u8::try_from(index + 1).expect("bounded fixture index");
        let basis = basis(modality, value);
        let current = binding(ExecutionMode::PreparedOnly, value, basis.clone());
        let candidate = binding(ExecutionMode::PreparedOnly, value, basis.clone());
        let mut bytes = format!("private-payload-secret-{value}").into_bytes();
        let expected_payload = bytes.clone();
        let work = usage(unit, u128::from(value));
        let request = CheckedModalityRequest::new(
            candidate,
            &bytes,
            work,
            limits(unit, u128::from(value)),
            owner(&current, Duration::from_millis(1)),
        )
        .expect("matching current binding is accepted");
        bytes.fill(0);

        assert_eq!(request.binding().semantic_basis.modality, modality);
        assert_eq!(
            request.binding().semantic_basis.source,
            ContentDigest([value; 32])
        );
        assert_eq!(
            request.binding().semantic_basis.rights_revision,
            label(&format!("rights-{value}")),
        );
        assert_eq!(
            request.binding().semantic_basis.output_format_revision,
            label(&format!("format-{value}")),
        );
        assert_eq!(request.binding().semantic_basis, current.semantic_basis);
        assert_eq!(request.binding().identity, current.identity);
        assert_eq!(request.binding().mode, current.mode);
        assert_eq!(request.binding().deadline, current.deadline);
        assert_eq!(request.payload(), expected_payload);
        assert_eq!(request.usage(), work);
        request
            .validate_current(owner(&current, Duration::from_millis(2)))
            .expect("current exact binding remains valid");
    }
}

#[test]
fn source_rights_and_output_format_changes_are_stale_basis_mismatches() {
    for changed in ["source", "rights", "format"] {
        let current_basis = basis(RequestModality::Text, 7);
        let mut candidate_basis = current_basis.clone();
        match changed {
            "source" => candidate_basis.source = ContentDigest([8; 32]),
            "rights" => candidate_basis.rights_revision = label("rights-revoked"),
            "format" => candidate_basis.output_format_revision = label("format-changed"),
            _ => unreachable!("fixture has only named metadata fields"),
        }
        let current = binding(ExecutionMode::Replay, 7, current_basis);
        let candidate = binding(ExecutionMode::Replay, 7, candidate_basis);
        assert_eq!(
            CheckedModalityRequest::new(
                candidate,
                b"private-payload-secret",
                usage(UsageUnit::Token, 1),
                limits(UsageUnit::Token, 1),
                owner(&current, Duration::ZERO),
            )
            .unwrap_err(),
            RequestError::SemanticBasisMismatch,
            "changed {changed} must be rejected by the existing equality fence",
        );
    }
}

#[test]
fn queued_envelope_rejects_a_changed_current_basis() {
    let admitted_basis = basis(RequestModality::Tts, 8);
    let admitted = binding(ExecutionMode::Live, 8, admitted_basis.clone());
    let request = CheckedModalityRequest::new(
        binding(ExecutionMode::Live, 8, admitted_basis),
        b"private-payload-secret",
        usage(UsageUnit::Character, 1),
        limits(UsageUnit::Character, 1),
        owner(&admitted, Duration::ZERO),
    )
    .expect("current basis admits request");

    let mut changed_basis = basis(RequestModality::Tts, 8);
    changed_basis.rights_revision = label("rights-changed-after-queue");
    let changed_current = binding(ExecutionMode::Live, 8, changed_basis);
    assert_eq!(
        request.validate_current(owner(&changed_current, Duration::from_millis(1))),
        Err(RequestError::SemanticBasisMismatch),
    );
}

#[test]
fn execution_mode_and_elapsed_deadline_use_existing_typed_errors() {
    let current_basis = basis(RequestModality::Image, 9);
    let current = binding(ExecutionMode::PreparedOnly, 9, current_basis.clone());
    let changed_mode = binding(ExecutionMode::Replay, 9, current_basis.clone());
    assert_eq!(
        CheckedModalityRequest::new(
            changed_mode,
            b"private-payload-secret",
            usage(UsageUnit::Image, 1),
            limits(UsageUnit::Image, 1),
            owner(&current, Duration::ZERO),
        )
        .unwrap_err(),
        RequestError::IdentityMismatch(RequestIdentityField::Mode),
    );

    let deadline = binding(ExecutionMode::PreparedOnly, 9, current_basis);
    let elapsed = deadline.deadline;
    assert_eq!(
        CheckedModalityRequest::new(
            binding(
                ExecutionMode::PreparedOnly,
                9,
                deadline.semantic_basis.clone(),
            ),
            b"private-payload-secret",
            usage(UsageUnit::Image, 1),
            limits(UsageUnit::Image, 1),
            owner(&deadline, elapsed),
        )
        .unwrap_err(),
        RequestError::DeadlineExceeded,
    );
}

#[test]
fn checked_byte_token_duration_and_unit_limits_are_not_clipped_or_relabelled() {
    let current_basis = basis(RequestModality::Sound, 12);
    let current = binding(ExecutionMode::Live, 12, current_basis.clone());
    let candidate = || binding(ExecutionMode::Live, 12, current_basis.clone());
    let caps = limits(UsageUnit::AudioMillisecond, 2);

    assert_eq!(
        CheckedModalityRequest::new(
            candidate(),
            &[b'x'; 100],
            usage(UsageUnit::AudioMillisecond, 1),
            caps,
            owner(&current, Duration::ZERO),
        )
        .unwrap_err(),
        RequestError::LimitExceeded(RequestLimit::Bytes),
    );
    assert_eq!(
        CheckedModalityRequest::new(
            candidate(),
            b"bytes",
            RequestUsage::new(
                99,
                Duration::from_millis(1),
                Usage::new(1, UsageUnit::AudioMillisecond)
            ),
            caps,
            owner(&current, Duration::ZERO),
        )
        .unwrap_err(),
        RequestError::LimitExceeded(RequestLimit::Tokens),
    );
    assert_eq!(
        CheckedModalityRequest::new(
            candidate(),
            b"bytes",
            RequestUsage::new(
                1,
                Duration::from_millis(99),
                Usage::new(1, UsageUnit::AudioMillisecond)
            ),
            caps,
            owner(&current, Duration::ZERO),
        )
        .unwrap_err(),
        RequestError::LimitExceeded(RequestLimit::Duration),
    );
    assert_eq!(
        CheckedModalityRequest::new(
            candidate(),
            b"bytes",
            RequestUsage::new(
                1,
                Duration::from_millis(1),
                Usage::new(99, UsageUnit::AudioMillisecond)
            ),
            caps,
            owner(&current, Duration::ZERO),
        )
        .unwrap_err(),
        RequestError::LimitExceeded(RequestLimit::Units),
    );
    assert_eq!(
        CheckedModalityRequest::new(
            candidate(),
            b"bytes",
            usage(UsageUnit::VideoMillisecond, 1),
            caps,
            owner(&current, Duration::ZERO),
        )
        .unwrap_err(),
        RequestError::UnitMismatch,
    );
}

#[test]
fn default_debug_does_not_expose_private_request_basis_or_payload() {
    let current_basis = RequestBasis {
        modality: RequestModality::Text,
        source: ContentDigest([42; 32]),
        rights_revision: label("private-rights-secret"),
        output_format_revision: label("format-private"),
        semantic: PrivateContext("private-semantic-context-secret".to_owned()),
    };
    let current = binding(ExecutionMode::Replay, 42, current_basis.clone());
    let request = CheckedModalityRequest::new(
        binding(ExecutionMode::Replay, 42, current_basis.clone()),
        b"private-payload-secret",
        usage(UsageUnit::Token, 1),
        limits(UsageUnit::Token, 1),
        owner(&current, Duration::ZERO),
    )
    .expect("fixture uses exact current values");

    assert_private_debug(&request);
    assert_private_debug(request.binding());
    assert_private_debug(&request.binding().semantic_basis);
}
