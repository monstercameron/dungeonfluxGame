//! Current provider request -> canonical recording admission -> outcome consumer.
//! These finite in-memory owners prove local EOF/disposal and current publication
//! fencing, not vendor sockets, durable storage, rights or supplier billing.

use std::{cell::Cell, time::Duration};

use df_ai::admission::{
    CompleteRecord, RecordAdmissionError, RecordEvent, RecordIdentity, RecordIdentityField,
    RecordInputError, RecordLimit, RecordLimits, RecordingPublisher, admit_complete_record,
};
use df_model::checkpoint::{
    AssetRequestKey, AudienceScope, Basis, ContentDigest, ExecutionMode, JobId, RecordId,
};
use df_provider_api::{
    BudgetMutation, CheckedRequest, ProviderAttemptObservation, ProviderBillingClass,
    ProviderFailureClass, ProviderLiabilityDisposition, ProviderNextAction, ProviderResultClass,
    ProviderRetryPolicy, RequestBinding, RequestError, RequestIdentity, RequestLimits,
    RequestOwnerState, RequestUsage, classify_provider_outcome,
};
use df_types::{
    OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision, Usage, UsageUnit,
};

type StreamBasis = (RequestIdentity, ExecutionMode);
type Event = RecordEvent<AssetRequestKey, StreamBasis>;
type Admission = Result<(), RecordAdmissionError<PublicationError>>;

const DIGEST: [u8; 32] = [
    0xbe, 0xf5, 0x7e, 0xc7, 0xf5, 0x3a, 0x6d, 0x40, 0xbe, 0xb6, 0x40, 0xa7, 0x80, 0xa6, 0x39, 0xc8,
    0x3b, 0xc2, 0x9a, 0xc8, 0xa9, 0x81, 0x6f, 0x1f, 0xc6, 0xc5, 0xc6, 0xdc, 0xd9, 0x3c, 0x47, 0x21,
];

fn operation(value: u8) -> OperationId {
    OperationId::from_bytes(&[value; 16]).unwrap()
}

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
            operation: operation(6),
            generation: 7,
        },
        semantic_basis: AssetRequestKey {
            schema: 1,
            source: ContentDigest([8; 32]),
            moment: RecordId::from_bytes(&[9; 16]).unwrap(),
            identity: label("identity"),
            style: label("style"),
            voice: None,
            provider: label("finite-fixture"),
            model: label("fixture-model"),
            format: label("fixture-format"),
            references: vec![],
            audience: AudienceScope::Host,
            parameters: label("fixture-parameters"),
        },
        mode,
        deadline: Duration::from_secs(2),
    }
}

#[derive(Default)]
struct Owner {
    cancelled: Cell<bool>,
    elapsed: Cell<Duration>,
    unavailable: Cell<bool>,
}

impl Owner {
    fn observe<'a>(
        &self,
        current: &'a RequestBinding<AssetRequestKey>,
    ) -> RequestOwnerState<'a, AssetRequestKey> {
        RequestOwnerState {
            current: (!self.unavailable.get()).then_some(current),
            elapsed: self.elapsed.get(),
            cancelled: self.cancelled.get(),
        }
    }

    fn stop(&self, action: Stop, current: &RequestBinding<AssetRequestKey>) {
        match action {
            Stop::Cancel => self.cancelled.set(true),
            Stop::Deadline => self.elapsed.set(current.deadline),
            Stop::Unavailable => self.unavailable.set(true),
        }
    }
}

fn request(current: &RequestBinding<AssetRequestKey>) -> CheckedRequest<AssetRequestKey> {
    CheckedRequest::new(
        binding(current.mode),
        b"data",
        RequestUsage::new(1, Duration::ZERO, Usage::new(1, UsageUnit::Token)),
        RequestLimits::new(4, 1, Duration::ZERO, Usage::new(1, UsageUnit::Token)).unwrap(),
        Owner::default().observe(current),
    )
    .unwrap()
}

fn identity(
    request: &CheckedRequest<AssetRequestKey>,
) -> RecordIdentity<AssetRequestKey, StreamBasis> {
    let admitted = request.binding();
    RecordIdentity {
        key: admitted.semantic_basis.clone(),
        basis: (admitted.identity, admitted.mode),
        operation: admitted.identity.operation,
    }
}

fn complete(request: &CheckedRequest<AssetRequestKey>) -> Event {
    RecordEvent::Complete {
        identity: identity(request),
        byte_length: 6,
        sha256: DIGEST,
    }
}

fn events(request: &CheckedRequest<AssetRequestKey>) -> Vec<Result<Event, RecordInputError>> {
    vec![
        Ok(RecordEvent::Chunk(b"abc".to_vec())),
        Ok(RecordEvent::Chunk(b"def".to_vec())),
        Ok(complete(request)),
    ]
}

fn limits() -> RecordLimits {
    RecordLimits::new(6, 3, 2).unwrap()
}

#[derive(Clone, Copy)]
enum Stop {
    Cancel,
    Deadline,
    Unavailable,
}

#[derive(Default)]
struct Observations {
    polls: Cell<usize>,
    eof: Cell<bool>,
    drops: Cell<usize>,
    owner_refusal: Cell<Option<RequestError>>,
}

struct OwnedEvents<'a> {
    input: std::vec::IntoIter<Result<Event, RecordInputError>>,
    request: &'a CheckedRequest<AssetRequestKey>,
    current: &'a RequestBinding<AssetRequestKey>,
    owner: &'a Owner,
    observations: &'a Observations,
    stop_at_poll: Option<(usize, Stop)>,
}

impl Iterator for OwnedEvents<'_> {
    type Item = Result<Event, RecordInputError>;

    fn next(&mut self) -> Option<Self::Item> {
        let poll = self.observations.polls.get() + 1;
        self.observations.polls.set(poll);
        if let Some((at, action)) = self.stop_at_poll
            && at == poll
        {
            self.owner.stop(action, self.current);
        }
        if let Err(error) = self
            .request
            .validate_current(self.owner.observe(self.current))
        {
            self.observations.owner_refusal.set(Some(error));
            return Some(Err(RecordInputError::Cancelled));
        }
        let event = self.input.next();
        if event.is_none() {
            self.observations.eof.set(true);
        }
        event
    }
}

impl Drop for OwnedEvents<'_> {
    fn drop(&mut self) {
        self.observations
            .drops
            .set(self.observations.drops.get() + 1);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublicationError {
    Owner(RequestError),
    Storage,
    Unknown,
}

struct Publisher<'a> {
    request: &'a CheckedRequest<AssetRequestKey>,
    current: &'a RequestBinding<AssetRequestKey>,
    owner: &'a Owner,
    observations: &'a Observations,
    accepted: Vec<u8>,
    calls: usize,
    failure: Option<PublicationError>,
    stop_before_mutation: Option<Stop>,
}

impl<'a> Publisher<'a> {
    fn new(
        request: &'a CheckedRequest<AssetRequestKey>,
        current: &'a RequestBinding<AssetRequestKey>,
        owner: &'a Owner,
        observations: &'a Observations,
    ) -> Self {
        Self {
            request,
            current,
            owner,
            observations,
            accepted: b"previous accepted".to_vec(),
            calls: 0,
            failure: None,
            stop_before_mutation: None,
        }
    }
}

impl RecordingPublisher for Publisher<'_> {
    type Key = AssetRequestKey;
    type Basis = StreamBasis;
    type Artifact = ();
    type Error = PublicationError;

    fn publish_complete(
        &mut self,
        record: CompleteRecord<Self::Key, Self::Basis>,
    ) -> Result<(), Self::Error> {
        self.calls += 1;
        assert!(
            self.observations.eof.get(),
            "publication must follow actual EOF"
        );
        if let Some(action) = self.stop_before_mutation {
            self.owner.stop(action, self.current);
        }
        self.request
            .validate_current(self.owner.observe(self.current))
            .map_err(PublicationError::Owner)?;
        assert_eq!(record.identity().key, self.current.semantic_basis);
        assert_eq!(
            record.identity().basis,
            (self.current.identity, self.current.mode)
        );
        assert_eq!(record.identity().operation, self.current.identity.operation);
        if let Some(error) = self.failure {
            return Err(error);
        }
        assert_eq!(record.bytes(), b"abcdef");
        assert_eq!(record.sha256(), &DIGEST);
        let (_, bytes, _) = record.into_parts();
        self.accepted = bytes;
        Ok(())
    }
}

fn consume(
    publisher: &mut Publisher<'_>,
    input: Vec<Result<Event, RecordInputError>>,
    bounds: RecordLimits,
    stop_at_poll: Option<(usize, Stop)>,
) -> Admission {
    // This is the actual canonical admission consumer, not a second stream parser.
    let stream = OwnedEvents {
        input: input.into_iter(),
        request: publisher.request,
        current: publisher.current,
        owner: publisher.owner,
        observations: publisher.observations,
        stop_at_poll,
    };
    let expected = identity(publisher.request);
    let result = admit_complete_record(publisher, expected, bounds, stream);
    assert_eq!(
        publisher.observations.drops.get(),
        1,
        "finite local iterator disposed once"
    );
    result
}

fn classify(
    publisher: &Publisher<'_>,
    result: &Admission,
    billing: ProviderBillingClass,
) -> df_provider_api::ProviderOutcomeDecision {
    let output = match result {
        Ok(()) => ProviderResultClass::Complete,
        Err(RecordAdmissionError::MissingCompletion) => ProviderResultClass::Incomplete,
        Err(RecordAdmissionError::Input(RecordInputError::Cancelled)) => {
            let failure = if publisher.observations.owner_refusal.get()
                == Some(RequestError::DeadlineExceeded)
            {
                ProviderFailureClass::Deadline
            } else {
                ProviderFailureClass::Cancelled
            };
            ProviderResultClass::Failed(failure)
        }
        Err(RecordAdmissionError::Publication(PublicationError::Owner(
            RequestError::DeadlineExceeded,
        ))) => ProviderResultClass::Failed(ProviderFailureClass::Deadline),
        Err(RecordAdmissionError::Input(RecordInputError::Failed))
        | Err(RecordAdmissionError::Publication(
            PublicationError::Storage | PublicationError::Unknown,
        )) => ProviderResultClass::Failed(ProviderFailureClass::Unknown),
        Err(_) => ProviderResultClass::Failed(ProviderFailureClass::Contract),
    };
    classify_provider_outcome(
        publisher.request,
        publisher.owner.observe(publisher.current),
        ProviderAttemptObservation {
            operation: publisher.request.binding().identity.operation,
            result: output,
            billing,
        },
        &BudgetMutation::<(), (), ()>::Unknown,
        ProviderRetryPolicy {
            attempts_remaining: 1,
            retry: true,
            fallback: true,
        },
    )
    .unwrap()
}

fn assert_refused(publisher: &Publisher<'_>, result: &Admission) {
    assert!(result.is_err());
    assert_eq!(publisher.accepted, b"previous accepted");
    assert_ne!(
        classify(publisher, result, ProviderBillingClass::MissingOrAmbiguous).result,
        ProviderResultClass::Complete
    );
    assert_eq!(publisher.request.payload(), b"data");
}

#[test]
fn exact_stream_publishes_once_after_eof_without_changing_mode_or_billing() {
    for mode in [
        ExecutionMode::Live,
        ExecutionMode::PreparedOnly,
        ExecutionMode::Replay,
    ] {
        let current = binding(mode);
        let request = request(&current);
        let owner = Owner::default();
        let observations = Observations::default();
        let mut publisher = Publisher::new(&request, &current, &owner, &observations);
        let result = consume(&mut publisher, events(&request), limits(), None);
        assert_eq!(result, Ok(()));
        assert_eq!(observations.polls.get(), 4);
        assert!(observations.eof.get());
        assert_eq!(publisher.calls, 1);
        assert_eq!(publisher.accepted, b"abcdef");
        for billing in [
            ProviderBillingClass::MissingOrAmbiguous,
            ProviderBillingClass::VerifiedLiability,
        ] {
            let decision = classify(&publisher, &result, billing);
            assert_eq!(decision.result, ProviderResultClass::Complete);
            assert_eq!(
                decision.liability,
                ProviderLiabilityDisposition::RetainWorstCase
            );
            assert_eq!(decision.next, ProviderNextAction::ReconcileSameOperation);
        }
        assert_eq!(request.binding().identity, current.identity);
        assert_eq!(request.binding().mode, mode);
    }
}

#[test]
fn empty_or_partial_eof_cannot_publish_or_classify_complete() {
    for count in 0..=2 {
        let current = binding(ExecutionMode::Live);
        let request = request(&current);
        let owner = Owner::default();
        let observations = Observations::default();
        let mut publisher = Publisher::new(&request, &current, &owner, &observations);
        let input = events(&request).into_iter().take(count).collect();
        let result = consume(&mut publisher, input, limits(), None);
        assert_eq!(result, Err(RecordAdmissionError::MissingCompletion));
        assert!(observations.eof.get());
        assert_eq!(publisher.calls, 0);
        assert_refused(&publisher, &result);
        let decision = classify(
            &publisher,
            &result,
            ProviderBillingClass::MissingOrAmbiguous,
        );
        assert_eq!(decision.result, ProviderResultClass::Incomplete);
        assert_eq!(
            decision.liability,
            ProviderLiabilityDisposition::RetainWorstCase
        );
        assert_eq!(decision.next, ProviderNextAction::ReconcileSameOperation);
    }
}

#[test]
fn duplicate_completion_and_trailing_even_empty_chunk_refuse_before_publication() {
    for kind in 0..3 {
        let current = binding(ExecutionMode::Live);
        let request = request(&current);
        let owner = Owner::default();
        let observations = Observations::default();
        let mut publisher = Publisher::new(&request, &current, &owner, &observations);
        let trailing = match kind {
            0 => complete(&request),
            1 => RecordEvent::Chunk(vec![]),
            _ => RecordEvent::Chunk(vec![1]),
        };
        let mut input = events(&request);
        input.push(Ok(trailing));
        let result = consume(&mut publisher, input, limits(), None);
        assert_eq!(result, Err(RecordAdmissionError::TrailingEvent));
        assert!(!observations.eof.get());
        assert_eq!(publisher.calls, 0);
        assert_refused(&publisher, &result);
    }
}

#[test]
fn input_failure_or_cancellation_before_or_after_final_never_publishes() {
    for error in [RecordInputError::Failed, RecordInputError::Cancelled] {
        for after_final in [false, true] {
            let current = binding(ExecutionMode::Live);
            let request = request(&current);
            let owner = Owner::default();
            let observations = Observations::default();
            let mut publisher = Publisher::new(&request, &current, &owner, &observations);
            let mut input = if after_final {
                events(&request)
            } else {
                vec![Ok(RecordEvent::Chunk(b"abc".to_vec()))]
            };
            input.push(Err(error));
            let result = consume(&mut publisher, input, limits(), None);
            assert_eq!(result, Err(RecordAdmissionError::Input(error)));
            assert_eq!(publisher.calls, 0);
            assert_refused(&publisher, &result);
        }
    }
}

#[test]
fn chunk_record_and_item_caps_refuse_without_replacing_previous_bytes() {
    for (bounds, count, expected) in [
        (
            RecordLimits::new(6, 2, 2).unwrap(),
            3,
            RecordLimit::ChunkBytes,
        ),
        (
            RecordLimits::new(5, 3, 2).unwrap(),
            3,
            RecordLimit::RecordBytes,
        ),
        (
            RecordLimits::new(6, 3, 1).unwrap(),
            3,
            RecordLimit::ChunkCount,
        ),
    ] {
        let current = binding(ExecutionMode::Live);
        let request = request(&current);
        let owner = Owner::default();
        let observations = Observations::default();
        let mut publisher = Publisher::new(&request, &current, &owner, &observations);
        let input = events(&request).into_iter().take(count).collect();
        let result = consume(&mut publisher, input, bounds, None);
        assert_eq!(result, Err(RecordAdmissionError::LimitExceeded(expected)));
        assert_eq!(publisher.calls, 0);
        assert_refused(&publisher, &result);
    }
    for bounds in [(0, 1, 1), (1, 0, 1), (1, 1, 0), (1, 2, 1)] {
        assert!(RecordLimits::new(bounds.0, bounds.1, bounds.2).is_err());
    }
}

#[test]
fn completion_key_basis_operation_length_and_digest_are_required() {
    for kind in 0..5 {
        let current = binding(ExecutionMode::Live);
        let request = request(&current);
        let owner = Owner::default();
        let observations = Observations::default();
        let mut publisher = Publisher::new(&request, &current, &owner, &observations);
        let mut final_identity = identity(&request);
        let mut length = 6;
        let mut digest = DIGEST;
        let expected = match kind {
            0 => {
                final_identity.key.source = ContentDigest([20; 32]);
                RecordAdmissionError::IdentityMismatch(RecordIdentityField::Key)
            }
            1 => {
                final_identity.basis.0.generation += 1;
                RecordAdmissionError::IdentityMismatch(RecordIdentityField::Basis)
            }
            2 => {
                final_identity.operation = operation(20);
                RecordAdmissionError::IdentityMismatch(RecordIdentityField::Operation)
            }
            3 => {
                length = 7;
                RecordAdmissionError::LengthMismatch
            }
            _ => {
                digest[0] ^= 1;
                RecordAdmissionError::DigestMismatch
            }
        };
        let mut input = events(&request);
        input.pop();
        input.push(Ok(RecordEvent::Complete {
            identity: final_identity,
            byte_length: length,
            sha256: digest,
        }));
        let result = consume(&mut publisher, input, limits(), None);
        assert_eq!(result, Err(expected));
        assert_eq!(publisher.calls, 0);
        assert_refused(&publisher, &result);
    }
}

#[test]
fn current_owner_cancel_deadline_and_unavailability_fence_each_consumption_phase() {
    for (action, expected) in [
        (Stop::Cancel, RequestError::Cancelled),
        (Stop::Deadline, RequestError::DeadlineExceeded),
        (Stop::Unavailable, RequestError::CurrentBasisUnavailable),
    ] {
        for poll in [1, 2, 4] {
            let current = binding(ExecutionMode::Live);
            let request = request(&current);
            let owner = Owner::default();
            let observations = Observations::default();
            let mut publisher = Publisher::new(&request, &current, &owner, &observations);
            let result = consume(
                &mut publisher,
                events(&request),
                limits(),
                Some((poll, action)),
            );
            assert_eq!(
                result,
                Err(RecordAdmissionError::Input(RecordInputError::Cancelled))
            );
            assert_eq!(observations.owner_refusal.get(), Some(expected));
            assert_eq!(publisher.calls, 0);
            assert_refused(&publisher, &result);
        }
    }
}

#[test]
fn stale_generation_or_semantic_basis_refuses_before_stream_read() {
    for semantic in [false, true] {
        let admitted = binding(ExecutionMode::Live);
        let request = request(&admitted);
        let mut current = binding(ExecutionMode::Live);
        if semantic {
            current.semantic_basis.format = label("changed-format");
        } else {
            current.identity.generation += 1;
        }
        let owner = Owner::default();
        let observations = Observations::default();
        let mut publisher = Publisher::new(&request, &current, &owner, &observations);
        let result = consume(&mut publisher, events(&request), limits(), None);
        assert_eq!(
            result,
            Err(RecordAdmissionError::Input(RecordInputError::Cancelled))
        );
        assert_eq!(observations.polls.get(), 1);
        assert!(observations.owner_refusal.get().is_some());
        assert_eq!(publisher.calls, 0);
        assert_refused(&publisher, &result);
    }
}

#[test]
fn publisher_rechecks_current_owner_after_eof_before_mutation() {
    for (action, expected) in [
        (Stop::Cancel, RequestError::Cancelled),
        (Stop::Deadline, RequestError::DeadlineExceeded),
        (Stop::Unavailable, RequestError::CurrentBasisUnavailable),
    ] {
        let current = binding(ExecutionMode::Live);
        let request = request(&current);
        let owner = Owner::default();
        let observations = Observations::default();
        let mut publisher = Publisher::new(&request, &current, &owner, &observations);
        publisher.stop_before_mutation = Some(action);
        let result = consume(&mut publisher, events(&request), limits(), None);
        assert_eq!(
            result,
            Err(RecordAdmissionError::Publication(PublicationError::Owner(
                expected
            )))
        );
        assert_eq!(publisher.calls, 1);
        assert!(observations.eof.get());
        assert_refused(&publisher, &result);
    }
}

#[test]
fn failed_or_unknown_publication_never_replaces_accepted_record_or_releases_liability() {
    for error in [PublicationError::Storage, PublicationError::Unknown] {
        let current = binding(ExecutionMode::Live);
        let request = request(&current);
        let owner = Owner::default();
        let observations = Observations::default();
        let mut publisher = Publisher::new(&request, &current, &owner, &observations);
        publisher.failure = Some(error);
        let result = consume(&mut publisher, events(&request), limits(), None);
        assert_eq!(result, Err(RecordAdmissionError::Publication(error)));
        assert_eq!(publisher.calls, 1);
        assert_refused(&publisher, &result);
        let decision = classify(&publisher, &result, ProviderBillingClass::VerifiedLiability);
        assert_eq!(
            decision.liability,
            ProviderLiabilityDisposition::RetainWorstCase
        );
        assert_eq!(decision.next, ProviderNextAction::ReconcileSameOperation);
    }
}
