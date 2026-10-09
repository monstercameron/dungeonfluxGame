use std::{cell::Cell, time::Duration};

use df_ai::lookup::{LookupError, PreparedRead, ReadResult, lookup_prepared, lookup_replay};
use df_model::checkpoint::{
    AssetKind, AssetReference, AssetRequestKey, AudienceScope, Basis, ContentDigest, ExecutionMode,
    JobId, RecordId,
};
use df_provider_api::{
    CheckedRequest, InvalidRequestLimits, RequestBinding, RequestError, RequestIdentity,
    RequestIdentityField, RequestLimit, RequestLimits, RequestOwnerState, RequestUsage,
};
use df_types::{
    MemberId, OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision, Usage,
    UsageUnit,
};

fn label(value: &str) -> RevisionLabel {
    RevisionLabel::new(Some(value)).unwrap()
}

fn binding(mode: ExecutionMode) -> RequestBinding<AssetRequestKey> {
    RequestBinding {
        identity: RequestIdentity {
            basis: Basis {
                session: SessionId::from_bytes(&[1; 16]).unwrap(),
                run: RunId::from_bytes(&[2; 16]).unwrap(),
                revision: SessionRevision::new(RecoveryEpoch::new(3).unwrap(), 4),
            },
            job: JobId::from_bytes(&[5; 16]).unwrap(),
            operation: OperationId::from_bytes(&[6; 16]).unwrap(),
            generation: 7,
        },
        semantic_basis: AssetRequestKey {
            schema: 1,
            source: ContentDigest([8; 32]),
            moment: RecordId::from_bytes(&[9; 16]).unwrap(),
            identity: label("identity-1"),
            style: label("style-1"),
            voice: Some(label("voice-1")),
            provider: label("fixture-provider"),
            model: label("fixture-model"),
            format: label("fixture-format"),
            references: vec![AssetReference {
                key: label("reference-1"),
                digest: ContentDigest([10; 32]),
                byte_length: 4,
                kind: AssetKind::Image,
            }],
            audience: AudienceScope::Members(vec![MemberId::from_bytes(&[11; 16]).unwrap()]),
            parameters: label("parameters-1"),
        },
        mode,
        deadline: Duration::from_secs(2),
    }
}

fn owner(current: &RequestBinding<AssetRequestKey>) -> RequestOwnerState<'_, AssetRequestKey> {
    RequestOwnerState {
        current: Some(current),
        elapsed: Duration::ZERO,
        cancelled: false,
    }
}

fn limits() -> RequestLimits {
    RequestLimits::new(
        4,
        3,
        Duration::from_nanos(5),
        Usage::new(2, UsageUnit::Token),
    )
    .unwrap()
}

fn work() -> RequestUsage {
    RequestUsage::new(3, Duration::from_nanos(5), Usage::new(2, UsageUnit::Token))
}

#[test]
fn exact_caps_own_payload_and_preserve_the_canonical_binding_and_counter_values() {
    for mode in [
        ExecutionMode::Live,
        ExecutionMode::PreparedOnly,
        ExecutionMode::Replay,
    ] {
        let current = binding(mode);
        let candidate = binding(mode);
        let mut input = b"data".to_vec();
        let request =
            CheckedRequest::new(candidate, &input, work(), limits(), owner(&current)).unwrap();
        input.fill(0);

        assert_eq!(request.payload(), b"data");
        assert_eq!(request.binding().identity, current.identity);
        assert_eq!(request.binding().semantic_basis, current.semantic_basis);
        assert_eq!(request.binding().mode, mode);
        assert_eq!(request.binding().deadline, Duration::from_secs(2));
        assert_eq!(request.usage().tokens(), 3);
        assert_eq!(request.usage().duration(), Duration::from_nanos(5));
        assert_eq!(request.usage().usage(), Usage::new(2, UsageUnit::Token));
        assert_eq!(request.limits().max_bytes(), 4);
        assert_eq!(request.limits().max_tokens(), 3);
        assert_eq!(request.limits().max_duration(), Duration::from_nanos(5));
        assert_eq!(
            request.limits().max_usage(),
            Usage::new(2, UsageUnit::Token)
        );
        assert_eq!(request.validate_current(owner(&current)), Ok(()));

        let (admitted, bytes, counters) = request.into_parts();
        assert_eq!(admitted.identity, current.identity);
        assert_eq!(admitted.semantic_basis, current.semantic_basis);
        assert_eq!(admitted.mode, mode);
        assert_eq!(admitted.deadline, current.deadline);
        assert_eq!(bytes, b"data");
        assert_eq!(counters, work());
    }
}

#[test]
fn each_exceeded_cap_refuses_instead_of_clipping_or_relabeling_work() {
    let current = binding(ExecutionMode::PreparedOnly);
    for (payload, usage, expected) in [
        (&b"12345"[..], work(), RequestLimit::Bytes),
        (
            &b"1234"[..],
            RequestUsage::new(4, Duration::from_nanos(5), Usage::new(2, UsageUnit::Token)),
            RequestLimit::Tokens,
        ),
        (
            &b"1234"[..],
            RequestUsage::new(3, Duration::from_nanos(6), Usage::new(2, UsageUnit::Token)),
            RequestLimit::Duration,
        ),
        (
            &b"1234"[..],
            RequestUsage::new(3, Duration::from_nanos(5), Usage::new(3, UsageUnit::Token)),
            RequestLimit::Units,
        ),
    ] {
        assert_eq!(
            CheckedRequest::new(
                binding(current.mode),
                payload,
                usage,
                limits(),
                owner(&current)
            )
            .unwrap_err(),
            RequestError::LimitExceeded(expected),
        );
    }
}

#[test]
fn mismatched_usage_kind_precedes_caps_and_cap_precedence_is_deterministic() {
    let current = binding(ExecutionMode::Replay);
    for (payload, usage, expected) in [
        (
            &b"12345"[..],
            RequestUsage::new(
                u128::MAX,
                Duration::MAX,
                Usage::new(u128::MAX, UsageUnit::Byte),
            ),
            RequestError::UnitMismatch,
        ),
        (
            &b"12345"[..],
            RequestUsage::new(
                u128::MAX,
                Duration::MAX,
                Usage::new(u128::MAX, UsageUnit::Token),
            ),
            RequestError::LimitExceeded(RequestLimit::Bytes),
        ),
        (
            &b"1234"[..],
            RequestUsage::new(
                u128::MAX,
                Duration::MAX,
                Usage::new(u128::MAX, UsageUnit::Token),
            ),
            RequestError::LimitExceeded(RequestLimit::Tokens),
        ),
        (
            &b"1234"[..],
            RequestUsage::new(3, Duration::MAX, Usage::new(u128::MAX, UsageUnit::Token)),
            RequestError::LimitExceeded(RequestLimit::Duration),
        ),
    ] {
        assert_eq!(
            CheckedRequest::new(
                binding(current.mode),
                payload,
                usage,
                limits(),
                owner(&current)
            )
            .unwrap_err(),
            expected,
        );
    }
}

#[test]
fn byte_limit_uses_actual_utf8_bytes_and_preserves_arbitrary_binary_input() {
    let current = binding(ExecutionMode::PreparedOnly);
    assert_eq!(
        CheckedRequest::new(
            binding(current.mode),
            "ééé".as_bytes(),
            work(),
            limits(),
            owner(&current)
        )
        .unwrap_err(),
        RequestError::LimitExceeded(RequestLimit::Bytes),
    );
    for payload in ["éé".as_bytes(), &[0xff, 0, 0xfe, 1][..]] {
        let request = CheckedRequest::new(
            binding(current.mode),
            payload,
            work(),
            limits(),
            owner(&current),
        )
        .unwrap();
        assert_eq!(request.payload(), payload);
    }
}

#[test]
fn zero_limits_disable_dimensions_and_maximum_typed_counters_do_not_overflow() {
    let current = binding(ExecutionMode::PreparedOnly);
    assert_eq!(
        RequestLimits::new(0, 0, Duration::ZERO, Usage::new(0, UsageUnit::Byte)),
        Err(InvalidRequestLimits),
    );
    let disabled =
        RequestLimits::new(1, 0, Duration::ZERO, Usage::new(0, UsageUnit::Byte)).unwrap();
    let zero = RequestUsage::new(0, Duration::ZERO, Usage::new(0, UsageUnit::Byte));
    assert_eq!(
        CheckedRequest::new(binding(current.mode), b"", zero, disabled, owner(&current))
            .unwrap()
            .payload(),
        b"",
    );
    for (usage, expected) in [
        (
            RequestUsage::new(1, Duration::ZERO, Usage::new(0, UsageUnit::Byte)),
            RequestLimit::Tokens,
        ),
        (
            RequestUsage::new(0, Duration::from_nanos(1), Usage::new(0, UsageUnit::Byte)),
            RequestLimit::Duration,
        ),
        (
            RequestUsage::new(0, Duration::ZERO, Usage::new(1, UsageUnit::Byte)),
            RequestLimit::Units,
        ),
    ] {
        assert_eq!(
            CheckedRequest::new(binding(current.mode), b"", usage, disabled, owner(&current))
                .unwrap_err(),
            RequestError::LimitExceeded(expected),
        );
    }
    let units = [
        UsageUnit::Token,
        UsageUnit::Character,
        UsageUnit::Byte,
        UsageUnit::AudioMillisecond,
        UsageUnit::VideoMillisecond,
        UsageUnit::Image,
    ];
    for unit in units {
        let maximum = Usage::new(u128::MAX, unit);
        let bounds = RequestLimits::new(1, u128::MAX, Duration::MAX, maximum).unwrap();
        let usage = RequestUsage::new(u128::MAX, Duration::MAX, maximum);
        let request =
            CheckedRequest::new(binding(current.mode), b"x", usage, bounds, owner(&current))
                .unwrap();
        assert_eq!(request.usage(), usage);
        for other in units {
            if other != unit {
                let mismatch = RequestUsage::new(0, Duration::ZERO, Usage::new(0, other));
                assert_eq!(
                    CheckedRequest::new(
                        binding(current.mode),
                        b"",
                        mismatch,
                        bounds,
                        owner(&current)
                    )
                    .unwrap_err(),
                    RequestError::UnitMismatch,
                );
            }
        }
    }
}

#[test]
fn invalid_generation_and_deadline_and_owner_cancellation_refuse_before_bounds() {
    let current = binding(ExecutionMode::PreparedOnly);
    let mut invalid = binding(current.mode);
    invalid.identity.generation = 0;
    assert_eq!(
        CheckedRequest::new(invalid, b"12345", work(), limits(), owner(&current)).unwrap_err(),
        RequestError::InvalidGeneration,
    );
    let mut invalid = binding(current.mode);
    invalid.deadline = Duration::ZERO;
    assert_eq!(
        CheckedRequest::new(invalid, b"12345", work(), limits(), owner(&current)).unwrap_err(),
        RequestError::InvalidDeadline,
    );
    let cancelled = RequestOwnerState {
        current: Some(&current),
        elapsed: Duration::ZERO,
        cancelled: true,
    };
    assert_eq!(
        CheckedRequest::new(binding(current.mode), b"12345", work(), limits(), cancelled)
            .unwrap_err(),
        RequestError::Cancelled,
    );
    let elapsed = RequestOwnerState {
        current: Some(&current),
        elapsed: current.deadline,
        cancelled: false,
    };
    assert_eq!(
        CheckedRequest::new(binding(current.mode), b"12345", work(), limits(), elapsed)
            .unwrap_err(),
        RequestError::DeadlineExceeded,
    );
}

#[test]
fn every_identity_component_and_pinned_mode_and_deadline_are_fenced() {
    let current = binding(ExecutionMode::PreparedOnly);
    for field in [
        RequestIdentityField::Session,
        RequestIdentityField::Run,
        RequestIdentityField::Revision,
        RequestIdentityField::Job,
        RequestIdentityField::Operation,
        RequestIdentityField::Generation,
        RequestIdentityField::Mode,
        RequestIdentityField::Deadline,
    ] {
        let mut candidate = binding(current.mode);
        match field {
            RequestIdentityField::Session => {
                candidate.identity.basis.session = SessionId::from_bytes(&[12; 16]).unwrap()
            }
            RequestIdentityField::Run => {
                candidate.identity.basis.run = RunId::from_bytes(&[12; 16]).unwrap()
            }
            RequestIdentityField::Revision => {
                candidate.identity.basis.revision =
                    SessionRevision::new(RecoveryEpoch::new(3).unwrap(), 5)
            }
            RequestIdentityField::Job => {
                candidate.identity.job = JobId::from_bytes(&[12; 16]).unwrap()
            }
            RequestIdentityField::Operation => {
                candidate.identity.operation = OperationId::from_bytes(&[12; 16]).unwrap()
            }
            RequestIdentityField::Generation => candidate.identity.generation = 8,
            RequestIdentityField::Mode => candidate.mode = ExecutionMode::Live,
            RequestIdentityField::Deadline => candidate.deadline = Duration::from_secs(3),
        }
        assert_eq!(
            CheckedRequest::new(candidate, b"data", work(), limits(), owner(&current)).unwrap_err(),
            RequestError::IdentityMismatch(field),
        );
    }
}

#[test]
fn semantic_basis_checks_the_entire_existing_asset_key_including_references_and_audience() {
    let current = binding(ExecutionMode::Replay);
    for component in [
        "schema",
        "source",
        "moment",
        "identity",
        "style",
        "voice",
        "provider",
        "model",
        "format",
        "reference_digest",
        "reference_length",
        "audience",
        "parameters",
    ] {
        let mut candidate = binding(current.mode);
        let key = &mut candidate.semantic_basis;
        match component {
            "schema" => key.schema = 2,
            "source" => key.source = ContentDigest([12; 32]),
            "moment" => key.moment = RecordId::from_bytes(&[12; 16]).unwrap(),
            "identity" => key.identity = label("other-identity"),
            "style" => key.style = label("other-style"),
            "voice" => key.voice = None,
            "provider" => key.provider = label("other-provider"),
            "model" => key.model = label("other-model"),
            "format" => key.format = label("other-format"),
            "reference_digest" => key.references[0].digest = ContentDigest([12; 32]),
            "reference_length" => key.references[0].byte_length = 5,
            "audience" => key.audience = AudienceScope::Host,
            "parameters" => key.parameters = label("other-parameters"),
            _ => unreachable!(),
        }
        assert_eq!(
            CheckedRequest::new(candidate, b"data", work(), limits(), owner(&current)).unwrap_err(),
            RequestError::SemanticBasisMismatch,
            "{component}",
        );
    }
}

#[test]
fn queued_request_rechecks_owner_state_and_cannot_be_revived_by_an_old_receipt_wait() {
    let current = binding(ExecutionMode::PreparedOnly);
    let request = CheckedRequest::new(
        binding(current.mode),
        b"data",
        work(),
        limits(),
        owner(&current),
    )
    .unwrap();
    let before = RequestOwnerState {
        current: Some(&current),
        elapsed: current.deadline - Duration::from_nanos(1),
        cancelled: false,
    };
    assert_eq!(request.validate_current(before), Ok(()));
    for elapsed in [current.deadline, Duration::MAX] {
        assert_eq!(
            request.validate_current(RequestOwnerState {
                current: Some(&current),
                elapsed,
                cancelled: false
            }),
            Err(RequestError::DeadlineExceeded),
        );
    }
    assert_eq!(
        request.validate_current(RequestOwnerState {
            current: Some(&current),
            elapsed: Duration::ZERO,
            cancelled: true
        }),
        Err(RequestError::Cancelled),
    );
    let mut replacement = binding(current.mode);
    replacement.identity.generation = 8;
    assert_eq!(
        request.validate_current(owner(&replacement)),
        Err(RequestError::IdentityMismatch(
            RequestIdentityField::Generation
        )),
    );
    replacement = binding(current.mode);
    replacement.semantic_basis.parameters = label("revoked-parameters");
    assert_eq!(
        request.validate_current(owner(&replacement)),
        Err(RequestError::SemanticBasisMismatch)
    );
    assert_eq!(request.binding().identity.generation, 7);
    assert_eq!(request.payload(), b"data");
}

#[test]
fn diagnostic_formatting_never_calls_the_private_semantic_key_formatter() {
    #[derive(Eq, PartialEq)]
    struct PrivateKey;
    let candidate = binding(ExecutionMode::PreparedOnly);
    let current = RequestBinding {
        identity: candidate.identity,
        semantic_basis: PrivateKey,
        mode: candidate.mode,
        deadline: candidate.deadline,
    };
    let candidate = RequestBinding {
        identity: candidate.identity,
        semantic_basis: PrivateKey,
        mode: candidate.mode,
        deadline: candidate.deadline,
    };
    let state = RequestOwnerState {
        current: Some(&current),
        elapsed: Duration::ZERO,
        cancelled: false,
    };
    assert_eq!(format!("{state:?}"), "RequestOwnerState { .. }");
    assert_eq!(format!("{current:?}"), "RequestBinding { .. }");
    let bounds = RequestLimits::new(
        6,
        3,
        Duration::from_nanos(5),
        Usage::new(2, UsageUnit::Token),
    )
    .unwrap();
    let request = CheckedRequest::new(candidate, b"secret", work(), bounds, state).unwrap();
    assert_eq!(format!("{request:?}"), "CheckedRequest { .. }");
    assert_eq!(request.payload(), b"secret");
}

struct RecordingFixture {
    key: AssetRequestKey,
    basis: RequestIdentity,
    complete_bytes: [u8; 4],
    prepared_reads: Cell<usize>,
    replay_reads: Cell<usize>,
}

#[test]
fn missing_current_semantic_policy_is_an_explicit_refusal_including_after_queueing() {
    let current = binding(ExecutionMode::PreparedOnly);
    let unavailable = RequestOwnerState {
        current: None,
        elapsed: Duration::ZERO,
        cancelled: false,
    };
    assert_eq!(
        CheckedRequest::new(
            binding(current.mode),
            b"data",
            work(),
            limits(),
            unavailable
        )
        .unwrap_err(),
        RequestError::CurrentBasisUnavailable,
    );
    let request = CheckedRequest::new(
        binding(current.mode),
        b"data",
        work(),
        limits(),
        owner(&current),
    )
    .unwrap();
    assert_eq!(
        request.validate_current(RequestOwnerState {
            current: None,
            elapsed: Duration::ZERO,
            cancelled: false
        }),
        Err(RequestError::CurrentBasisUnavailable),
    );
    assert_eq!(request.payload(), b"data");
}

impl PreparedRead for RecordingFixture {
    type Key = AssetRequestKey;
    type Basis = RequestIdentity;
    type Artifact = [u8; 4];
    type Failure = ();

    fn read_prepared(
        &self,
        _: &Self::Key,
    ) -> ReadResult<'_, Self::Key, Self::Basis, Self::Artifact, Self::Failure> {
        self.prepared_reads.set(self.prepared_reads.get() + 1);
        Ok(Some((&self.key, &self.basis, &self.complete_bytes)))
    }

    fn read_replay(
        &self,
        _: &Self::Key,
    ) -> ReadResult<'_, Self::Key, Self::Basis, Self::Artifact, Self::Failure> {
        self.replay_reads.set(self.replay_reads.get() + 1);
        Ok(Some((&self.key, &self.basis, &self.complete_bytes)))
    }
}

#[test]
fn real_prepared_and_replay_lookup_consumers_reuse_the_request_key_and_basis() {
    for mode in [ExecutionMode::PreparedOnly, ExecutionMode::Replay] {
        let current = binding(mode);
        let recorded = binding(mode);
        let mut store = RecordingFixture {
            key: recorded.semantic_basis,
            basis: recorded.identity,
            complete_bytes: *b"done",
            prepared_reads: Cell::new(0),
            replay_reads: Cell::new(0),
        };
        let request =
            CheckedRequest::new(binding(mode), b"data", work(), limits(), owner(&current)).unwrap();
        request.validate_current(owner(&current)).unwrap();
        let admitted = request.binding();
        let result = match admitted.mode {
            ExecutionMode::PreparedOnly => {
                lookup_prepared(&store, &admitted.semantic_basis, &admitted.identity)
            }
            ExecutionMode::Replay => {
                lookup_replay(&store, &admitted.semantic_basis, &admitted.identity)
            }
            ExecutionMode::Live => panic!("fixture covers only admitted recorded modes"),
        };
        assert_eq!(result.unwrap(), b"done");
        assert_eq!(request.binding().mode, mode);
        store.key.source = ContentDigest([12; 32]);
        let stale = match admitted.mode {
            ExecutionMode::PreparedOnly => {
                lookup_prepared(&store, &admitted.semantic_basis, &admitted.identity)
            }
            ExecutionMode::Replay => {
                lookup_replay(&store, &admitted.semantic_basis, &admitted.identity)
            }
            ExecutionMode::Live => panic!("fixture covers only admitted recorded modes"),
        };
        assert_eq!(stale, Err(LookupError::Stale));
    }
}

#[test]
fn bounded_request_admission_precedes_prepared_and_replay_recording_reads() {
    for mode in [ExecutionMode::PreparedOnly, ExecutionMode::Replay] {
        for (payload, usage, expected) in [
            (&b"data"[..], work(), Ok(*b"done")),
            (
                &b"12345"[..],
                work(),
                Err(RequestError::LimitExceeded(RequestLimit::Bytes)),
            ),
            (
                &b"data"[..],
                RequestUsage::new(4, Duration::from_nanos(5), Usage::new(2, UsageUnit::Token)),
                Err(RequestError::LimitExceeded(RequestLimit::Tokens)),
            ),
            (
                &b"data"[..],
                RequestUsage::new(3, Duration::from_nanos(6), Usage::new(2, UsageUnit::Token)),
                Err(RequestError::LimitExceeded(RequestLimit::Duration)),
            ),
            (
                &b"data"[..],
                RequestUsage::new(3, Duration::from_nanos(5), Usage::new(3, UsageUnit::Token)),
                Err(RequestError::LimitExceeded(RequestLimit::Units)),
            ),
            (
                &b"data"[..],
                RequestUsage::new(3, Duration::from_nanos(5), Usage::new(2, UsageUnit::Byte)),
                Err(RequestError::UnitMismatch),
            ),
        ] {
            let current = binding(mode);
            let recorded = binding(mode);
            let store = RecordingFixture {
                key: recorded.semantic_basis,
                basis: recorded.identity,
                complete_bytes: *b"done",
                prepared_reads: Cell::new(0),
                replay_reads: Cell::new(0),
            };
            let input = payload.to_vec();
            let result: Result<[u8; 4], RequestError> = (|| {
                let request =
                    CheckedRequest::new(binding(mode), &input, usage, limits(), owner(&current))?;
                request.validate_current(owner(&current))?;
                assert_eq!(request.payload(), input);
                assert_eq!(request.usage(), usage);
                assert_eq!(request.binding().identity, current.identity);
                assert_eq!(request.binding().semantic_basis, current.semantic_basis);
                assert_eq!(request.binding().mode, mode);
                let admitted = request.binding();
                let bytes = match admitted.mode {
                    ExecutionMode::PreparedOnly => {
                        lookup_prepared(&store, &admitted.semantic_basis, &admitted.identity)
                    }
                    ExecutionMode::Replay => {
                        lookup_replay(&store, &admitted.semantic_basis, &admitted.identity)
                    }
                    ExecutionMode::Live => panic!("fixture covers only admitted recorded modes"),
                }
                .expect("current complete recording matches the admitted request");
                Ok(*bytes)
            })();

            assert_eq!(result, expected, "{mode:?}");
            let accepted_reads = usize::from(expected.is_ok());
            assert_eq!(
                store.prepared_reads.get(),
                if mode == ExecutionMode::PreparedOnly {
                    accepted_reads
                } else {
                    0
                },
            );
            assert_eq!(
                store.replay_reads.get(),
                if mode == ExecutionMode::Replay {
                    accepted_reads
                } else {
                    0
                },
            );
            assert_eq!(input, payload);
            assert_eq!(store.complete_bytes, *b"done");
            assert_eq!(store.key, current.semantic_basis);
            assert_eq!(store.basis, current.identity);
        }
    }
}
