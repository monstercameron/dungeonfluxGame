use std::{cell::Cell, collections::VecDeque, time::Duration};

use df_ai::lookup::{
    AuthorizedLookupError, LookupError, PreparedRead, ReadAuthority, ReadEntry,
    lookup_authorized_prepared, lookup_authorized_replay,
};
use df_model::checkpoint::{
    AssetKind, AssetReference, AssetRequestKey, AudienceScope, Basis, ContentDigest, ExecutionMode,
    JobId, RecordId,
};
use df_provider_api::{
    CheckedRequest, RequestBinding, RequestError, RequestIdentity, RequestLimits,
    RequestOwnerState, RequestUsage,
};
use df_types::{
    MemberId, OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision, Usage,
    UsageUnit,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StorageFailure {
    Offline,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RightsFailure {
    Revoked,
}

struct StoredArtifact {
    key: AssetRequestKey,
    basis: RequestIdentity,
    bytes: Vec<u8>,
}

struct ArtifactStore {
    prepared: Option<StoredArtifact>,
    replay: Option<StoredArtifact>,
    storage_failure: bool,
    deny_authorization_at: Option<usize>,
    authorizations: Cell<usize>,
    prepared_reads: Cell<usize>,
    replay_reads: Cell<usize>,
}

impl ArtifactStore {
    fn new(prepared: Option<StoredArtifact>, replay: Option<StoredArtifact>) -> Self {
        Self {
            prepared,
            replay,
            storage_failure: false,
            deny_authorization_at: None,
            authorizations: Cell::new(0),
            prepared_reads: Cell::new(0),
            replay_reads: Cell::new(0),
        }
    }

    fn read<'a>(
        &'a self,
        entry: &'a Option<StoredArtifact>,
    ) -> Result<Option<ReadEntry<'a, AssetRequestKey, RequestIdentity, Vec<u8>>>, StorageFailure>
    {
        if self.storage_failure {
            return Err(StorageFailure::Offline);
        }
        Ok(entry
            .as_ref()
            .map(|stored| (&stored.key, &stored.basis, &stored.bytes)))
    }

    fn authorize(&self) -> Result<(), RightsFailure> {
        let call = self.authorizations.get() + 1;
        self.authorizations.set(call);
        if self.deny_authorization_at == Some(call) {
            Err(RightsFailure::Revoked)
        } else {
            Ok(())
        }
    }
}

impl PreparedRead for ArtifactStore {
    type Key = AssetRequestKey;
    type Basis = RequestIdentity;
    type Artifact = Vec<u8>;
    type Failure = StorageFailure;

    fn read_prepared(
        &self,
        _: &Self::Key,
    ) -> Result<Option<ReadEntry<'_, Self::Key, Self::Basis, Self::Artifact>>, Self::Failure> {
        self.prepared_reads.set(self.prepared_reads.get() + 1);
        self.read(&self.prepared)
    }

    fn read_replay(
        &self,
        _: &Self::Key,
    ) -> Result<Option<ReadEntry<'_, Self::Key, Self::Basis, Self::Artifact>>, Self::Failure> {
        self.replay_reads.set(self.replay_reads.get() + 1);
        self.read(&self.replay)
    }
}

impl ReadAuthority for ArtifactStore {
    type AuthorizationFailure = RightsFailure;

    fn authorize_prepared(
        &self,
        _: &Self::Key,
        _: &Self::Basis,
    ) -> Result<(), Self::AuthorizationFailure> {
        self.authorize()
    }

    fn authorize_replay(
        &self,
        _: &Self::Key,
        _: &Self::Basis,
    ) -> Result<(), Self::AuthorizationFailure> {
        self.authorize()
    }
}

#[derive(Default)]
struct FiniteLiveDispatch {
    steps: VecDeque<Vec<u8>>,
    calls: usize,
}

impl FiniteLiveDispatch {
    fn new(steps: impl IntoIterator<Item = Vec<u8>>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
            calls: 0,
        }
    }

    fn dispatch<Semantic>(
        &mut self,
        request: &CheckedRequest<Semantic>,
    ) -> Result<Vec<u8>, LiveFailure> {
        if request.binding().mode != ExecutionMode::Live {
            return Err(LiveFailure::WrongMode);
        }
        self.calls += 1;
        self.steps.pop_front().ok_or(LiveFailure::Exhausted)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveFailure {
    WrongMode,
    Exhausted,
}

#[derive(Debug, Eq, PartialEq)]
enum ConsumerError {
    Owner(RequestError),
    Authority(RightsFailure),
    Lookup(LookupError<StorageFailure>),
    Live(LiveFailure),
}

fn consume(
    request: &CheckedRequest<AssetRequestKey>,
    owner: RequestOwnerState<'_, AssetRequestKey>,
    store: &ArtifactStore,
    live: &mut FiniteLiveDispatch,
) -> Result<Vec<u8>, ConsumerError> {
    request
        .validate_current(owner)
        .map_err(ConsumerError::Owner)?;

    let binding = request.binding();
    match binding.mode {
        ExecutionMode::PreparedOnly => {
            lookup_authorized_prepared(store, &binding.semantic_basis, &binding.identity)
                .cloned()
                .map_err(map_authorized_error)
        }
        ExecutionMode::Replay => {
            lookup_authorized_replay(store, &binding.semantic_basis, &binding.identity)
                .cloned()
                .map_err(map_authorized_error)
        }
        ExecutionMode::Live => live.dispatch(request).map_err(ConsumerError::Live),
    }
}

fn map_authorized_error(
    error: AuthorizedLookupError<StorageFailure, RightsFailure>,
) -> ConsumerError {
    match error {
        AuthorizedLookupError::Authority(error) => ConsumerError::Authority(error),
        AuthorizedLookupError::Lookup(error) => ConsumerError::Lookup(error),
    }
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

fn checked_request(
    mode: ExecutionMode,
) -> (
    CheckedRequest<AssetRequestKey>,
    RequestBinding<AssetRequestKey>,
) {
    let current = binding(mode);
    let request = CheckedRequest::new(
        binding(mode),
        b"payload",
        RequestUsage::new(1, Duration::from_millis(1), Usage::new(1, UsageUnit::Token)),
        RequestLimits::new(
            16,
            2,
            Duration::from_secs(1),
            Usage::new(2, UsageUnit::Token),
        )
        .unwrap(),
        owner(&current),
    )
    .unwrap();
    (request, current)
}

fn artifact(mode: ExecutionMode, bytes: &[u8]) -> StoredArtifact {
    let request = binding(mode);
    StoredArtifact {
        key: request.semantic_basis,
        basis: request.identity,
        bytes: bytes.to_vec(),
    }
}

fn stale_artifact(mode: ExecutionMode) -> StoredArtifact {
    let mut stale = artifact(mode, b"stale");
    stale.key.source = ContentDigest([12; 32]);
    stale
}

fn stale_basis_artifact(mode: ExecutionMode) -> StoredArtifact {
    let mut stale = artifact(mode, b"stale");
    stale.basis.generation += 1;
    stale
}

#[test]
fn exact_authorized_modes_read_only_their_artifact_and_live_has_a_finite_positive_control() {
    let (prepared, prepared_owner) = checked_request(ExecutionMode::PreparedOnly);
    let store = ArtifactStore::new(
        Some(artifact(ExecutionMode::PreparedOnly, b"prepared")),
        Some(artifact(ExecutionMode::Replay, b"replay")),
    );
    let mut live = FiniteLiveDispatch::new([b"live".to_vec()]);
    assert_eq!(
        consume(&prepared, owner(&prepared_owner), &store, &mut live),
        Ok(b"prepared".to_vec())
    );
    assert_eq!(prepared.binding().mode, ExecutionMode::PreparedOnly);
    assert_eq!(
        prepared.binding().semantic_basis,
        prepared_owner.semantic_basis
    );
    assert_eq!(prepared.payload(), b"payload");
    assert_eq!(store.prepared_reads.get(), 1);
    assert_eq!(store.replay_reads.get(), 0);
    assert_eq!(live.calls, 0);
    assert_eq!(live.steps.len(), 1);

    let (replay, replay_owner) = checked_request(ExecutionMode::Replay);
    assert_eq!(
        consume(&replay, owner(&replay_owner), &store, &mut live),
        Ok(b"replay".to_vec())
    );
    assert_eq!(replay.binding().mode, ExecutionMode::Replay);
    assert_eq!(replay.binding().semantic_basis, replay_owner.semantic_basis);
    assert_eq!(replay.payload(), b"payload");
    assert_eq!(store.prepared_reads.get(), 1);
    assert_eq!(store.replay_reads.get(), 1);
    assert_eq!(live.calls, 0);
    assert_eq!(live.steps.len(), 1);

    let (live_request, live_owner) = checked_request(ExecutionMode::Live);
    assert_eq!(
        consume(&live_request, owner(&live_owner), &store, &mut live),
        Ok(b"live".to_vec())
    );
    assert_eq!(live.calls, 1);
    assert!(live.steps.is_empty());
    assert_eq!(store.prepared_reads.get(), 1);
    assert_eq!(store.replay_reads.get(), 1);
}

#[test]
fn misses_stale_storage_and_authority_outcomes_never_fall_through_to_live_or_another_mode() {
    let cases = [
        (
            "prepared_missing",
            ExecutionMode::PreparedOnly,
            None,
            None,
            ConsumerError::Lookup(LookupError::Missing),
        ),
        (
            "replay_missing",
            ExecutionMode::Replay,
            Some(artifact(ExecutionMode::PreparedOnly, b"prepared")),
            None,
            ConsumerError::Lookup(LookupError::Missing),
        ),
        (
            "prepared_stale",
            ExecutionMode::PreparedOnly,
            Some(stale_artifact(ExecutionMode::PreparedOnly)),
            None,
            ConsumerError::Lookup(LookupError::Stale),
        ),
        (
            "replay_stale",
            ExecutionMode::Replay,
            None,
            Some(stale_artifact(ExecutionMode::Replay)),
            ConsumerError::Lookup(LookupError::Stale),
        ),
        (
            "replay_stale_basis",
            ExecutionMode::Replay,
            None,
            Some(stale_basis_artifact(ExecutionMode::Replay)),
            ConsumerError::Lookup(LookupError::Stale),
        ),
    ];
    for (name, mode, prepared, replay, expected) in cases {
        let (request, current) = checked_request(mode);
        let store = ArtifactStore::new(prepared, replay);
        let mut live = FiniteLiveDispatch::new([b"must-not-run".to_vec()]);
        assert_eq!(
            consume(&request, owner(&current), &store, &mut live),
            Err(expected),
            "{name}"
        );
        assert_eq!(live.calls, 0, "{name}");
        assert_eq!(live.steps.len(), 1, "{name}");
        match mode {
            ExecutionMode::PreparedOnly => {
                assert_eq!(store.prepared_reads.get(), 1, "{name}");
                assert_eq!(store.replay_reads.get(), 0, "{name}");
            }
            ExecutionMode::Replay => {
                assert_eq!(store.prepared_reads.get(), 0, "{name}");
                assert_eq!(store.replay_reads.get(), 1, "{name}");
            }
            ExecutionMode::Live => unreachable!(),
        }
    }

    for mode in [ExecutionMode::PreparedOnly, ExecutionMode::Replay] {
        let (request, current) = checked_request(mode);
        let mut store = ArtifactStore::new(
            Some(artifact(ExecutionMode::PreparedOnly, b"prepared")),
            Some(artifact(ExecutionMode::Replay, b"replay")),
        );
        store.storage_failure = true;
        let mut live = FiniteLiveDispatch::new([b"must-not-run".to_vec()]);
        assert_eq!(
            consume(&request, owner(&current), &store, &mut live),
            Err(ConsumerError::Lookup(LookupError::Unavailable(
                StorageFailure::Offline
            )))
        );
        assert_eq!(live.calls, 0);
        assert_eq!(live.steps.len(), 1);
        assert_eq!(store.prepared_reads.get() + store.replay_reads.get(), 1);

        let (request, current) = checked_request(mode);
        let mut store = ArtifactStore::new(
            Some(artifact(ExecutionMode::PreparedOnly, b"prepared")),
            Some(artifact(ExecutionMode::Replay, b"replay")),
        );
        store.deny_authorization_at = Some(1);
        let mut live = FiniteLiveDispatch::new([b"must-not-run".to_vec()]);
        assert_eq!(
            consume(&request, owner(&current), &store, &mut live),
            Err(ConsumerError::Authority(RightsFailure::Revoked))
        );
        assert_eq!(store.prepared_reads.get() + store.replay_reads.get(), 0);
        assert_eq!(live.calls, 0);

        let (request, current) = checked_request(mode);
        let mut store = ArtifactStore::new(
            Some(artifact(ExecutionMode::PreparedOnly, b"prepared")),
            Some(artifact(ExecutionMode::Replay, b"replay")),
        );
        store.deny_authorization_at = Some(2);
        let mut live = FiniteLiveDispatch::new([b"must-not-run".to_vec()]);
        assert_eq!(
            consume(&request, owner(&current), &store, &mut live),
            Err(ConsumerError::Authority(RightsFailure::Revoked))
        );
        assert_eq!(store.prepared_reads.get() + store.replay_reads.get(), 1);
        assert_eq!(store.authorizations.get(), 2);
        assert_eq!(live.calls, 0);
        assert_eq!(live.steps.len(), 1);
    }
}

#[test]
fn cancelled_expired_unqualified_and_mode_drifted_requests_refuse_before_reads_or_live_dispatch() {
    let (request, current) = checked_request(ExecutionMode::PreparedOnly);
    let cases = [
        (
            "cancelled",
            RequestOwnerState {
                current: Some(&current),
                elapsed: Duration::ZERO,
                cancelled: true,
            },
            RequestError::Cancelled,
        ),
        (
            "expired",
            RequestOwnerState {
                current: Some(&current),
                elapsed: Duration::from_secs(2),
                cancelled: false,
            },
            RequestError::DeadlineExceeded,
        ),
        (
            "unqualified",
            RequestOwnerState {
                current: None,
                elapsed: Duration::ZERO,
                cancelled: false,
            },
            RequestError::CurrentBasisUnavailable,
        ),
    ];
    for (name, owner, expected) in cases {
        let store = ArtifactStore::new(None, None);
        let mut live = FiniteLiveDispatch::new([b"must-not-run".to_vec()]);
        assert_eq!(
            consume(&request, owner, &store, &mut live),
            Err(ConsumerError::Owner(expected)),
            "{name}"
        );
        assert_eq!(store.authorizations.get(), 0, "{name}");
        assert_eq!(
            store.prepared_reads.get() + store.replay_reads.get(),
            0,
            "{name}"
        );
        assert_eq!(live.calls, 0, "{name}");
    }

    let other_mode = binding(ExecutionMode::Replay);
    let drifted = RequestOwnerState {
        current: Some(&other_mode),
        elapsed: Duration::ZERO,
        cancelled: false,
    };
    let store = ArtifactStore::new(None, None);
    let mut live = FiniteLiveDispatch::new([b"must-not-run".to_vec()]);
    assert_eq!(
        consume(&request, drifted, &store, &mut live),
        Err(ConsumerError::Owner(RequestError::IdentityMismatch(
            df_provider_api::RequestIdentityField::Mode
        )))
    );
    assert_eq!(store.authorizations.get(), 0);
    assert_eq!(store.prepared_reads.get() + store.replay_reads.get(), 0);
    assert_eq!(live.calls, 0);
}

#[test]
fn consumed_json_witness_records_observed_decisions_and_native_boundary_limits() {
    let observed = r#"{
  "decision": "PreparedOnly selects only prepared lookup; Replay selects only replay lookup; neither enters live dispatch on hit, miss, stale identity, storage failure, or rights refusal.",
  "alternatives": ["fall through to another artifact mode", "retry with live provider"],
  "observed": {
    "prepared_hit": "prepared bytes",
    "replay_hit": "replay bytes",
    "prepared_miss": "typed Missing; live calls 0",
    "replay_miss_with_prepared_entry": "typed Missing; replay reads 1; prepared reads 0; live calls 0",
    "stale": "typed Stale; live calls 0",
    "storage_failure": "typed Unavailable(Offline); live calls 0",
    "rights_denied_before_read": "typed Authority(Revoked); reads 0; live calls 0",
    "rights_revoked_after_read": "typed Authority(Revoked); reads 1; live calls 0",
    "owner_cancelled_expired_unqualified_or_mode_drifted": "typed owner refusal; reads 0; live calls 0",
    "live_positive_control": "explicit Live selects finite callback once"
  },
  "unresolved": ["native provider egress admission and supplier authority are outside this contract", "all six modality runtime integrations are not established by this test"]
}
"#;
    assert_eq!(
        observed,
        include_str!("fixtures/execution_mode_contract.json")
    );
}
