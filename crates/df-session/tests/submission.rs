#![cfg(not(target_arch = "wasm32"))]

use df_observe::OperationContext;
use df_session::inbox::{ActorInput, AdmissionRefusal, AdmissionSequence, Reducer, bounded_inbox};
use df_session::submission::*;
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Barrier, Mutex};
use std::time::Duration;

use df_model::checkpoint::*;
use df_types::{
    BuildIdentity, MemberId, OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId,
    SessionRevision,
};

fn label(value: &str) -> RevisionLabel {
    RevisionLabel::new(Some(value)).unwrap()
}
fn entity(value: u8) -> EntityId {
    EntityId::from_bytes(&[value; 16]).unwrap()
}
fn member(value: u8) -> MemberId {
    MemberId::from_bytes(&[value; 16]).unwrap()
}
fn revision(epoch: u64, sequence: u64) -> SessionRevision {
    SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), sequence)
}
fn basis() -> Basis {
    Basis {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        revision: revision(2, 8),
    }
}
fn content() -> ContentReference {
    ContentReference {
        package: label("fixture-package-1"),
        entry: label("fixture-entry-1"),
    }
}
fn rule() -> RuleReference {
    RuleReference {
        catalog: label("fixture-catalog-1"),
        source: label("fixture-source-1"),
        entry: label("fixture-entry-1"),
        clause: label("fixture-clause-1"),
    }
}
fn pins() -> CheckpointPins {
    CheckpointPins {
        rules: RulesPins {
            mode: RulesMode::Standard2024,
            ruleset: label("fixture-rules-1"),
            catalog: label("fixture-catalog-1"),
            catalog_digest: ContentDigest([1; 32]),
            source_manifest: label("fixture-sources-1"),
            source_manifest_digest: ContentDigest([2; 32]),
            handler: label("fixture-handler-1"),
            handler_digest: ContentDigest([3; 32]),
        },
        content: ContentPins {
            content: label("fixture-content-1"),
            content_digest: ContentDigest([4; 32]),
            package: label("fixture-package-1"),
            package_digest: ContentDigest([5; 32]),
        },
        build: BuildIdentity::new(
            Some("fixture-source-1"),
            Some("fixture-native-1"),
            Some("fixture-wasm-1"),
            Some("fixture-config-1"),
            Some("fixture-content-1"),
        )
        .unwrap(),
    }
}
fn resource_constraints() -> Vec<ResourceConstraint> {
    vec![ResourceConstraint {
        owner: entity(4),
        resource: label("fixture-resource-1"),
        minimum: 0,
        maximum: 8,
        source: rule(),
    }]
}
fn limits() -> CheckpointLimits {
    CheckpointLimits {
        maximum_records: 100,
        maximum_text_bytes: 256,
        maximum_total_text_bytes: 1024,
        maximum_retained_bytes: 1024 * 1024,
    }
}
fn state() -> GameState {
    GameState {
        mode: ExecutionMode::Replay,
        logical_time: LogicalTime {
            ticks: 120,
            ticks_per_second: 10,
        },
        members: vec![MembershipLink {
            member: member(3),
            character: Some(entity(4)),
        }],
        entities: vec![WorldEntity {
            id: entity(4),
            definition: content(),
            location: None,
            position: Some(Position { x: 0, y: 0, z: 0 }),
            identity_revision: label("fixture-entity-1"),
        }],
        characters: vec![CharacterState {
            entity: entity(4),
            build: content(),
            owner: member(3),
            choices: vec![],
        }],
        resources: vec![ResourceState {
            owner: entity(4),
            resource: label("fixture-resource-1"),
            value: 4,
            minimum: 0,
            maximum: 8,
            source: rule(),
        }],
        inventory: vec![],
        facts: vec![],
        draws: vec![],
        decisions: vec![],
        pending: vec![],
        intents: vec![],
        timers: vec![],
        active_effects: vec![],
        knowledge: vec![],
        beliefs: vec![],
        memories: vec![],
        schedules: vec![],
        threats: vec![],
        relationships: vec![],
        conversations: vec![],
        obligations: vec![],
        narrative: NarrativeState {
            definition: content(),
            active_beats: vec![],
            completed_beats: vec![],
            open_threads: vec![],
            accepted_facts: vec![],
            remaining_budget: 0,
        },
        encounters: vec![],
        activity: vec![],
        tempo: TempoState {
            policy: content(),
            presentation_ticks: 0,
            intensity: 0,
            inertia: 0,
            fatigue: vec![],
        },
        presentation: vec![],
        continuity: continuity(),
    }
}

fn continuity() -> ContinuityState {
    ContinuityState {
        creation: vec![],
        simulation: vec![],
        catch_up: None,
        environment: vec![],
        travel: vec![],
        witnesses: vec![],
        rumors: vec![],
        journal: vec![],
        summaries: vec![],
        retrieval: vec![],
        retrieved: vec![],
        consolidation: vec![],
        npcs: vec![],
        hooks: vec![],
        arcs: vec![],
        remote: None,
        presence: vec![],
        audio: None,
        private_offers: vec![],
        knowledge_cues: vec![],
        moments: vec![],
        demands: vec![],
        asset_jobs: vec![],
        asset_dependencies: vec![],
        canonical_packs: vec![],
        shots: vec![],
        prefetch: None,
        scenes: vec![],
        item_origins: vec![],
        bookends: vec![],
        exports: vec![],
        critical_cues: vec![],
        content_candidates: vec![],
        content_admissions: vec![],
        recovery: RecoveryState {
            origin: None,
            retired_epochs: vec![],
            lost_ranges: vec![],
            suppression_generation: 0,
            redacted_records: vec![],
            unavailable_sources: vec![],
        },
    }
}

const WAIT: Duration = Duration::from_secs(5);
const RECEIPT_BYTES: usize = 64 * 1024;

fn checkpoint_at(basis: Basis, state: GameState) -> Checkpoint {
    let rules = [rule()];
    let content_entries = [content()];
    let resource_constraints = resource_constraints();
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        basis,
        pins(),
        state,
        ReferenceInventory {
            rules: &rules,
            content: &content_entries,
            resources: &resource_constraints,
            assets: &[],
        },
        limits(),
    )
    .unwrap()
}
fn initial() -> Checkpoint {
    checkpoint_at(basis(), state())
}
fn context() -> OperationContext {
    OperationContext {
        trace_parent: String::new(),
        build: "session-fixture".to_owned(),
    }
}

// Fixture identity values label trusted scope data; they do not issue authority.
#[derive(Clone, Copy)]
enum KeyCapture {
    Bounded,
    Fail,
    Oversize,
    Overflow,
    Unrepresentable,
}
struct ScopeKey {
    tenant: u8,
    session: SessionId,
    principal: u8,
    namespace: Vec<u8>,
    recovery_epoch: RecoveryEpoch,
    operation: OperationId,
    fingerprint_version: u8,
    fingerprint: u64,
    reported_heap: Option<usize>,
}
impl ScopeKey {
    fn matches_scope(&self, scope: &Scope) -> bool {
        self.tenant == scope.tenant
            && self.session == scope.basis.session
            && self.principal == scope.principal
            && self.namespace == scope.namespace
            && self.recovery_epoch == scope.basis.revision.epoch()
            && self.operation == scope.operation
            && self.fingerprint_version == scope.fingerprint_version
            && self.fingerprint == scope.fingerprint
    }
}
impl PartialEq for ScopeKey {
    fn eq(&self, other: &Self) -> bool {
        self.tenant == other.tenant
            && self.session == other.session
            && self.principal == other.principal
            && self.namespace == other.namespace
            && self.recovery_epoch == other.recovery_epoch
            && self.operation == other.operation
            && self.fingerprint_version == other.fingerprint_version
            && self.fingerprint == other.fingerprint
    }
}
impl Eq for ScopeKey {}
impl ActorInput for ScopeKey {
    fn retained_heap_bytes(&self) -> Option<usize> {
        self.reported_heap
    }
}
#[derive(Clone)]
struct Scope {
    basis: Basis,
    operation: OperationId,
    fingerprint: u64,
    fingerprint_version: u8,
    tenant: u8,
    principal: u8,
    namespace: Vec<u8>,
    key_capture: KeyCapture,
    fence: u64,
    lookup_only: bool,
}
impl ActorInput for Scope {
    fn retained_heap_bytes(&self) -> Option<usize> {
        Some(self.namespace.capacity())
    }
}
impl OperationScope for Scope {
    type UncertaintyKey = ScopeKey;
    fn capture_uncertainty_key(
        &self,
        maximum_retained_bytes: usize,
    ) -> Result<Self::UncertaintyKey, RepositoryError> {
        let inline = std::mem::size_of::<ScopeKey>();
        if inline
            .checked_add(self.namespace.len())
            .is_none_or(|n| n > maximum_retained_bytes)
            || matches!(self.key_capture, KeyCapture::Fail)
        {
            return Err(RepositoryError::Capacity);
        }
        let mut namespace = Vec::new();
        namespace
            .try_reserve_exact(self.namespace.len())
            .map_err(|_| RepositoryError::Capacity)?;
        namespace.extend_from_slice(&self.namespace);
        let actual_heap = namespace.capacity();
        if inline
            .checked_add(actual_heap)
            .is_none_or(|n| n > maximum_retained_bytes)
        {
            return Err(RepositoryError::Capacity);
        }
        let reported_heap = match self.key_capture {
            KeyCapture::Bounded | KeyCapture::Fail => Some(actual_heap),
            // Deliberately malformed consumer accounting probes caller fail-closed
            // validation without allocating huge or unrepresentable buffers.
            KeyCapture::Oversize => maximum_retained_bytes.checked_add(1),
            KeyCapture::Overflow => Some(usize::MAX),
            KeyCapture::Unrepresentable => None,
        };
        Ok(ScopeKey {
            tenant: self.tenant,
            session: self.basis.session,
            principal: self.principal,
            namespace,
            recovery_epoch: self.basis.revision.epoch(),
            operation: self.operation,
            fingerprint_version: self.fingerprint_version,
            fingerprint: self.fingerprint,
            reported_heap,
        })
    }

    fn session(&self) -> SessionId {
        self.basis.session
    }
    fn operation(&self) -> OperationId {
        self.operation
    }
    fn validate_input(&self, input: &GameInput) -> Result<(), RepositoryError> {
        let (basis, operation) = match input {
            GameInput::Game(x) => (x.basis, Some(x.operation)),
            GameInput::Host(x) => (x.basis, Some(x.operation)),
            GameInput::Job(x) => (x.basis, Some(x.operation)),
            GameInput::Timer(x) => (x.basis, None),
            GameInput::Presentation(x) => (x.basis, None),
        };
        if basis != self.basis || operation.is_some_and(|op| op != self.operation) {
            return Err(RepositoryError::InputBinding);
        }
        Ok(())
    }
    fn is_lookup_only(&self) -> bool {
        self.lookup_only
    }
}
fn operation(value: u8) -> OperationId {
    OperationId::from_bytes(&[value; 16]).unwrap()
}
fn scope(value: u8) -> Scope {
    Scope {
        basis: basis(),
        operation: operation(value),
        fingerprint: u64::from(value),
        fingerprint_version: 1,
        tenant: 1,
        principal: 1,
        namespace: vec![],
        key_capture: KeyCapture::Bounded,
        fence: 1,
        lookup_only: false,
    }
}
fn input(scope: &Scope) -> GameInput {
    GameInput::Host(HostInput {
        basis: scope.basis,
        operation: scope.operation,
        host: member(3),
        command: HostCommand::RequestCheckpoint,
    })
}
fn owned(scope: Scope) -> (OwnedInput<Scope>, Receiver<SubmissionOutcome>) {
    let input = input(&scope);
    OwnedInput::new(context(), scope, input)
}

fn proposed(current: &Checkpoint, scope: &Scope) -> Checkpoint {
    let mut basis = current.basis();
    basis.revision = basis.revision.next_sequence().unwrap();
    let mut state = current.state().clone();
    let fact = FactId::from_bytes(scope.operation.as_bytes()).unwrap();
    let effect = EffectId::from_bytes(scope.operation.as_bytes()).unwrap();
    state.facts.push(GameFact {
        id: fact,
        revision: basis.revision,
        operation: scope.operation,
        ordinal: 0,
        cause: None,
        audience: AudienceScope::Shared,
        value: FactValue::ContentEvent {
            definition: content(),
            subjects: vec![entity(4)],
        },
    });
    state.intents.push(DurableIntent {
        id: effect,
        basis,
        operation: scope.operation,
        slot: 0,
        kind: EffectKind::PublishPresentation,
        job: None,
        timer: None,
        generation: 1,
        status: DurableStatus::Pending,
        definition: content(),
    });
    state.decisions.push(AcceptedDecision {
        operation: scope.operation,
        revision: basis.revision,
        facts: vec![fact],
        draws: vec![],
        effects: vec![effect],
        source_policy: label("fixture-decision"),
        semantic_output: Some("fixture accepted checkpoint request".to_owned()),
    });
    checkpoint_at(basis, state)
}
fn receipt(checkpoint: &Checkpoint, operation: OperationId) -> DecisionReceipt {
    let decision = checkpoint
        .state()
        .decisions
        .iter()
        .find(|d| d.operation == operation)
        .unwrap()
        .clone();
    DecisionReceipt::new(
        Basis {
            revision: decision.revision,
            ..checkpoint.basis()
        },
        decision,
        RECEIPT_BYTES,
    )
    .unwrap()
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Failure {
    None,
    BeforeCommit,
    LostCommittedAck,
    UnknownNotCommitted,
    BadAck,
}
#[derive(Clone, Copy)]
enum LookupBehavior {
    InProgress,
    Expired,
    Unavailable,
    Malformed,
    TooLarge,
}
struct Database {
    checkpoint: Checkpoint,
    receipts: HashMap<OperationId, (u64, DecisionReceipt)>,
    scoped_receipts: Vec<(ScopeKey, DecisionReceipt)>,
    events: Vec<&'static str>,
    commit_calls: usize,
    engine_calls: usize,
    fence: u64,
    db_now: u64,
    lease_until: u64,
    failure: Failure,
    engine_failure: bool,
    lookup: HashMap<OperationId, LookupBehavior>,
    reload_failure: bool,
    reconnect_failure: bool,
    reconnect_calls: usize,
    reload_override: Option<Checkpoint>,
}
fn database() -> Arc<Mutex<Database>> {
    Arc::new(Mutex::new(Database {
        checkpoint: initial(),
        receipts: HashMap::new(),
        scoped_receipts: vec![],
        events: vec![],
        commit_calls: 0,
        engine_calls: 0,
        fence: 1,
        db_now: 10,
        lease_until: 11,
        failure: Failure::None,
        engine_failure: false,
        lookup: HashMap::new(),
        reload_failure: false,
        reconnect_failure: false,
        reconnect_calls: 0,
        reload_override: None,
    }))
}
struct Repository {
    database: Arc<Mutex<Database>>,
    before_ack: Option<(Sender<()>, Receiver<()>)>,
    force_duplicate_race: bool,
}
impl SessionRepository for Repository {
    type Scope = Scope;
    fn recover_connection(&mut self, _: &OperationContext) -> Result<(), RepositoryError> {
        let mut db = self.database.lock().unwrap();
        db.reconnect_calls += 1;
        if db.reconnect_failure {
            Err(RepositoryError::Unavailable)
        } else {
            Ok(())
        }
    }
    fn lookup_operation(
        &mut self,
        scope: &Scope,
        _: &OperationContext,
    ) -> Result<OperationLookup, RepositoryError> {
        let mut db = self.database.lock().unwrap();
        db.events.push("lookup");
        // Scripted retained histories allow independently scoped keys sharing one
        // OperationId. These are controlled port observations, not native DB proof.
        if let Some((_, receipt)) = db
            .scoped_receipts
            .iter()
            .find(|(key, _)| key.matches_scope(scope))
        {
            return Ok(OperationLookup::Committed(receipt.clone()));
        }
        if scope.session() != db.checkpoint.basis().session {
            return Err(RepositoryError::Unauthorized);
        }
        if let Some(behavior) = db.lookup.get(&scope.operation) {
            return match behavior {
                LookupBehavior::InProgress => Ok(OperationLookup::InProgress),
                LookupBehavior::Expired => Ok(OperationLookup::ExpiredOrIndeterminate),
                LookupBehavior::Unavailable => Err(RepositoryError::Unavailable),
                LookupBehavior::Malformed | LookupBehavior::TooLarge => {
                    let supplied = proposed(&db.checkpoint, scope);
                    let mut result = receipt(&supplied, scope.operation).decision().clone();
                    match behavior {
                        LookupBehavior::Malformed => result.operation = operation(99),
                        LookupBehavior::TooLarge => {
                            result.semantic_output = Some("private".repeat(RECEIPT_BYTES))
                        }
                        _ => unreachable!(),
                    }
                    Ok(OperationLookup::Committed(
                        DecisionReceipt::new(supplied.basis(), result, 1024 * 1024).unwrap(),
                    ))
                }
            };
        }
        Ok(match db.receipts.get(&scope.operation) {
            Some((fingerprint, _)) if *fingerprint != scope.fingerprint => {
                OperationLookup::Conflict
            }
            Some((_, value)) => OperationLookup::Committed(value.clone()),
            None => OperationLookup::NotRecorded,
        })
    }
    fn commit_decision(
        &mut self,
        scope: &Scope,
        candidate: &Checkpoint,
        expected: Basis,
        _: &OperationContext,
    ) -> Result<CommitOutcome, RepositoryError> {
        let mut db = self.database.lock().unwrap();
        db.commit_calls += 1;
        db.events.push("transaction");
        if self.force_duplicate_race {
            let winner = proposed(&db.checkpoint, scope);
            let mut winner_state = winner.state().clone();
            winner_state.decisions.last_mut().unwrap().semantic_output =
                Some("exact race winner".to_owned());
            let winner = checkpoint_at(winner.basis(), winner_state);
            let result = receipt(&winner, scope.operation);
            db.checkpoint = winner;
            db.receipts
                .insert(scope.operation, (scope.fingerprint, result));
            self.force_duplicate_race = false;
        }
        if let Some((fingerprint, value)) = db.receipts.get(&scope.operation) {
            return if *fingerprint == scope.fingerprint {
                Ok(CommitOutcome::PreviouslyCommitted(value.clone()))
            } else {
                Err(RepositoryError::OperationConflict)
            };
        }
        if expected.revision.epoch() != db.checkpoint.basis().revision.epoch() {
            return Err(RepositoryError::StaleEpoch);
        }
        if scope.fence != db.fence {
            return Err(RepositoryError::StaleFence);
        }
        if db.db_now >= db.lease_until {
            return Err(RepositoryError::ExpiredOwner);
        }
        if expected != db.checkpoint.basis() {
            return Err(RepositoryError::RevisionConflict);
        }
        // Simulated transaction stages the WHOLE real domain checkpoint privately.
        // This controlled owner demonstrates outcomes; it does not claim PostgreSQL durability.
        db.events.push("stage_checkpoint_facts_receipt_intents");
        if db.failure == Failure::BeforeCommit {
            db.events.push("rollback");
            return Err(RepositoryError::Unavailable);
        }
        if db.failure == Failure::UnknownNotCommitted {
            return Ok(CommitOutcome::Indeterminate);
        }
        let result = receipt(candidate, scope.operation);
        db.checkpoint = candidate.clone();
        db.receipts
            .insert(scope.operation, (scope.fingerprint, result.clone()));
        db.events.push("commit");
        let failure = db.failure;
        drop(db);
        if let Some((entered, proceed)) = self.before_ack.take() {
            entered.send(()).unwrap();
            if proceed.recv_timeout(WAIT).is_err() {
                return Ok(CommitOutcome::Indeterminate);
            }
        }
        if failure == Failure::LostCommittedAck {
            return Ok(CommitOutcome::Indeterminate);
        }
        if failure == Failure::BadAck {
            let mut decision = result.decision().clone();
            decision.operation = operation(99);
            return Ok(CommitOutcome::Confirmed(
                DecisionReceipt::new(result.basis(), decision, RECEIPT_BYTES).unwrap(),
            ));
        }
        self.database.lock().unwrap().events.push("ack");
        Ok(CommitOutcome::Confirmed(result))
    }
    fn load_current(
        &mut self,
        _: &Scope,
        _: &OperationContext,
    ) -> Result<Checkpoint, RepositoryError> {
        let mut db = self.database.lock().unwrap();
        db.events.push("load");
        if db.reload_failure {
            return Err(RepositoryError::Unavailable);
        }
        Ok(db
            .reload_override
            .as_ref()
            .unwrap_or(&db.checkpoint)
            .clone())
    }
}
struct Engine {
    database: Arc<Mutex<Database>>,
}
impl SessionEngine<Scope> for Engine {
    fn decide(
        &mut self,
        current: &Checkpoint,
        scope: &Scope,
        input: &GameInput,
    ) -> Result<Checkpoint, RepositoryError> {
        {
            let mut db = self.database.lock().unwrap();
            db.engine_calls += 1;
            if db.engine_failure {
                return Err(RepositoryError::InvalidCandidate);
            }
        }
        let rules = [rule()];
        let content_entries = [content()];
        let constraints = resource_constraints();
        df_model::commands::validate_client_command(
            input,
            current,
            ReferenceInventory {
                rules: &rules,
                content: &content_entries,
                resources: &constraints,
                assets: &[],
            },
            df_model::commands::CommandLimits {
                maximum_records: 100,
                maximum_text_bytes: 256,
                maximum_retained_bytes: 1024 * 1024,
            },
        )
        .map_err(|_| RepositoryError::InvalidCandidate)?;
        Ok(proposed(current, scope))
    }
    fn validate_recovery(&mut self, checkpoint: &Checkpoint) -> Result<(), RepositoryError> {
        checkpoint
            .validate_resume(checkpoint.basis(), &pins())
            .map(|_| ())
            .map_err(|_| RepositoryError::InvalidCandidate)
    }
}
struct Publication {
    database: Arc<Mutex<Database>>,
    fail: bool,
}
impl PublicationOwner<Scope> for Publication {
    fn publish_committed(
        &mut self,
        _: &Scope,
        checkpoint: &Checkpoint,
    ) -> Result<(), DeliveryError> {
        let mut db = self.database.lock().unwrap();
        assert_eq!(checkpoint, &db.checkpoint);
        assert!(db.events.contains(&"ack"));
        db.events.push("publish");
        if self.fail {
            Err(DeliveryError::Unavailable)
        } else {
            Ok(())
        }
    }
    fn wake_committed_intents(&mut self, _: &Scope) -> Result<(), DeliveryError> {
        let mut db = self.database.lock().unwrap();
        assert!(db.events.contains(&"ack"));
        db.events.push("wake");
        if self.fail {
            Err(DeliveryError::Unavailable)
        } else {
            Ok(())
        }
    }
}
type Owner = DurableOwner<Repository, Engine, Publication>;
fn owner_configured(
    db: &Arc<Mutex<Database>>,
    before_ack: Option<(Sender<()>, Receiver<()>)>,
    force_duplicate_race: bool,
    publication_failure: bool,
) -> Owner {
    DurableOwner::new(
        Repository {
            database: Arc::clone(db),
            before_ack,
            force_duplicate_race,
        },
        Engine {
            database: Arc::clone(db),
        },
        Publication {
            database: Arc::clone(db),
            fail: publication_failure,
        },
        initial(),
        RECEIPT_BYTES,
    )
    .unwrap()
}
fn owner(db: &Arc<Mutex<Database>>) -> Owner {
    owner_configured(db, None, false, false)
}
fn submit(owner: &mut Owner, scope: Scope) -> SubmissionOutcome {
    let (input, wait) = owned(scope);
    owner.reduce(AdmissionSequence(1), input);
    wait.recv_timeout(WAIT).unwrap()
}
fn assert_confirmed(outcome: SubmissionOutcome) -> DecisionReceipt {
    match outcome {
        SubmissionOutcome::Confirmed(receipt) => receipt,
        other => panic!("expected durable confirmation: {other:?}"),
    }
}

#[test]
fn actual_inbox_admission_and_commit_do_not_release_success_before_ack() {
    let db = database();
    let (entered, commit_entered) = mpsc::channel();
    let (proceed, release_ack) = mpsc::channel();
    let mut owner = owner_configured(&db, Some((entered, release_ack)), false, false);
    let (handle, actor) = bounded_inbox();
    let (input, wait) = owned(scope(6));
    assert!(handle.try_submit(input).is_ok());
    assert_eq!(wait.try_recv(), Err(TryRecvError::Empty));
    handle.stop().unwrap();
    std::thread::scope(|threads| {
        let running = threads.spawn(|| actor.run(&mut owner));
        commit_entered.recv_timeout(WAIT).unwrap();
        assert_eq!(wait.try_recv(), Err(TryRecvError::Empty));
        assert!(!db.lock().unwrap().events.contains(&"publish"));
        proceed.send(()).unwrap();
        let receipt = assert_confirmed(wait.recv_timeout(WAIT).unwrap());
        assert_eq!(
            receipt.basis().revision,
            basis().revision.next_sequence().unwrap()
        );
        let diagnostic = format!("{receipt:?}");
        assert!(diagnostic.contains("operation"));
        assert!(diagnostic.contains("revision"));
        assert!(diagnostic.contains("retained_bytes"));
        assert!(!diagnostic.contains("fixture accepted checkpoint request"));
        assert!(!diagnostic.contains("fixture-decision"));
        assert!(!diagnostic.contains("semantic_output"));
        running.join().unwrap().unwrap();
    });
    let db = db.lock().unwrap();
    assert_eq!(
        db.events,
        [
            "lookup",
            "transaction",
            "stage_checkpoint_facts_receipt_intents",
            "commit",
            "ack",
            "publish",
            "wake"
        ]
    );
    assert_eq!(db.checkpoint.state().facts.len(), 1);
    assert_eq!(db.checkpoint.state().decisions.len(), 1);
    assert_eq!(db.checkpoint.state().intents.len(), 1);
}

#[test]
fn precommit_failure_preserves_all_row_families_and_emits_no_confirmation_or_effect() {
    let db = database();
    db.lock().unwrap().failure = Failure::BeforeCommit;
    let mut owner = owner(&db);
    assert_eq!(
        submit(&mut owner, scope(6)),
        SubmissionOutcome::Refused(RepositoryError::Unavailable)
    );
    let db = db.lock().unwrap();
    assert_eq!(db.checkpoint, initial());
    assert!(db.receipts.is_empty());
    assert_eq!(owner.checkpoint(), &initial());
    assert!(!db.events.contains(&"publish"));
    assert!(!db.events.contains(&"wake"));
}

#[test]
fn lost_commit_ack_is_lookup_only_and_resolves_same_receipt_without_reduction() {
    let db = database();
    db.lock().unwrap().failure = Failure::LostCommittedAck;
    let mut owner = owner(&db);
    assert_eq!(
        submit(&mut owner, scope(6)),
        SubmissionOutcome::LookupRequired
    );
    assert_eq!(owner.checkpoint(), &initial());
    let mut retry = scope(6);
    retry.lookup_only = true;
    let receipt = assert_confirmed(submit(&mut owner, retry));
    assert_eq!(receipt, db.lock().unwrap().receipts[&operation(6)].1);
    let db = db.lock().unwrap();
    assert_eq!(db.commit_calls, 1);
    assert_eq!(db.engine_calls, 1);
    assert!(!db.events.contains(&"publish"));
    assert!(!db.events.contains(&"wake"));
    assert_eq!(owner.checkpoint(), &db.checkpoint);
}

#[test]
fn uncertain_absence_never_replays_even_after_local_restart_or_new_key() {
    let db = database();
    db.lock().unwrap().failure = Failure::UnknownNotCommitted;
    let mut owner = owner(&db);
    assert_eq!(
        submit(&mut owner, scope(6)),
        SubmissionOutcome::LookupRequired
    );
    assert_eq!(
        submit(&mut owner, scope(6)),
        SubmissionOutcome::LookupRequired
    );
    assert_eq!(
        submit(&mut owner, scope(7)),
        SubmissionOutcome::LookupRequired
    );
    assert_eq!(
        owner.reload_current(&scope(6), &context()),
        Err(RepositoryError::UnresolvedCommit)
    );
    let mut restarted = crate_owner(&db);
    let mut retry = scope(6);
    retry.lookup_only = true;
    assert_eq!(
        submit(&mut restarted, retry),
        SubmissionOutcome::LookupRequired
    );
    let db = db.lock().unwrap();
    assert_eq!(db.commit_calls, 1);
    assert_eq!(db.engine_calls, 1);
    assert_eq!(db.checkpoint, initial());
    assert!(db.receipts.is_empty());
}
fn crate_owner(db: &Arc<Mutex<Database>>) -> Owner {
    owner(db)
}

#[test]
fn identical_retry_returns_receipt_before_reduction_and_conflicting_fingerprint_refuses() {
    let db = database();
    let mut owner = owner(&db);
    let committed = assert_confirmed(submit(&mut owner, scope(6)));
    db.lock().unwrap().engine_failure = true;
    let (item, wait) = owned(scope(6));
    owner.reduce(AdmissionSequence(2), item);
    assert_eq!(
        assert_confirmed(wait.recv_timeout(WAIT).unwrap()),
        committed
    );
    let mut conflict = scope(6);
    conflict.fingerprint += 1;
    assert_eq!(
        submit(&mut owner, conflict),
        SubmissionOutcome::OperationConflict
    );
    let db = db.lock().unwrap();
    assert_eq!(db.commit_calls, 1);
    assert_eq!(db.engine_calls, 1);
    assert_eq!(db.checkpoint.state().decisions.len(), 1);
}

#[test]
fn racing_previously_committed_ack_discards_candidate_and_never_publishes_it() {
    let db = database();
    let mut owner = owner_configured(&db, None, true, false);
    assert_confirmed(submit(&mut owner, scope(6)));
    let db = db.lock().unwrap();
    assert_eq!(db.checkpoint.state().decisions.len(), 1);
    assert!(db.events.contains(&"load"));
    assert!(!db.events.contains(&"publish"));
    assert!(!db.events.contains(&"wake"));
    assert_eq!(owner.checkpoint(), &db.checkpoint);
}

#[test]
fn stale_owner_expired_db_clock_and_changed_epoch_cannot_publish() {
    for refusal in [
        RepositoryError::StaleFence,
        RepositoryError::ExpiredOwner,
        RepositoryError::StaleEpoch,
    ] {
        let db = database();
        let mut owner = owner(&db);
        {
            let mut db = db.lock().unwrap();
            match refusal {
                RepositoryError::StaleFence => db.fence = 2,
                RepositoryError::ExpiredOwner => db.db_now = db.lease_until,
                RepositoryError::StaleEpoch => {
                    let mut new_basis = basis();
                    new_basis.revision = revision(3, 0);
                    db.checkpoint = checkpoint_at(new_basis, state());
                }
                _ => unreachable!(),
            }
        }
        assert_eq!(
            submit(&mut owner, scope(6)),
            SubmissionOutcome::Refused(refusal)
        );
        let db = db.lock().unwrap();
        assert!(db.receipts.is_empty());
        assert!(!db.events.contains(&"publish"));
        assert!(!db.events.contains(&"wake"));
    }
}

#[test]
fn competing_same_basis_owners_commit_one_revision_fact_result_and_intent() {
    let db = database();
    let mut first = owner(&db);
    let mut second = owner(&db);
    let barrier = Arc::new(Barrier::new(2));
    let (a, b) = std::thread::scope(|threads| {
        let gate = Arc::clone(&barrier);
        let a = threads.spawn(move || {
            gate.wait();
            submit(&mut first, scope(6))
        });
        let b = threads.spawn(move || {
            barrier.wait();
            submit(&mut second, scope(7))
        });
        (a.join().unwrap(), b.join().unwrap())
    });
    assert!(matches!(
        (&a, &b),
        (
            SubmissionOutcome::Confirmed(_),
            SubmissionOutcome::Refused(RepositoryError::RevisionConflict)
        ) | (
            SubmissionOutcome::Refused(RepositoryError::RevisionConflict),
            SubmissionOutcome::Confirmed(_)
        )
    ));
    let db = db.lock().unwrap();
    assert_eq!(db.commit_calls, 2);
    assert_eq!(
        db.checkpoint.basis().revision,
        basis().revision.next_sequence().unwrap()
    );
    assert_eq!(db.checkpoint.state().facts.len(), 1);
    assert_eq!(db.checkpoint.state().decisions.len(), 1);
    assert_eq!(db.checkpoint.state().intents.len(), 1);
    assert_eq!(db.receipts.len(), 1);
}

#[test]
fn bad_commit_ack_withholds_candidate_and_retains_uncertainty() {
    let db = database();
    db.lock().unwrap().failure = Failure::BadAck;
    let mut owner = owner(&db);
    assert_eq!(
        submit(&mut owner, scope(6)),
        SubmissionOutcome::LookupRequired
    );
    assert_eq!(owner.checkpoint(), &initial());
    assert!(!db.lock().unwrap().events.contains(&"publish"));
}

#[test]
fn dropped_receipt_waiter_does_not_cancel_owned_committed_work() {
    let db = database();
    let mut owner = owner(&db);
    let (item, waiter) = owned(scope(6));
    drop(waiter);
    owner.reduce(AdmissionSequence(1), item);
    let db = db.lock().unwrap();
    assert_eq!(db.receipts.len(), 1);
    assert_eq!(db.checkpoint.state().intents.len(), 1);
    assert!(db.events.contains(&"wake"));
}

#[test]
fn aborted_inbox_disconnects_admitted_receipts_without_running_durable_work() {
    let db = database();
    let owner = owner(&db);
    let (handle, actor) = bounded_inbox();
    let producer = handle.clone();
    let mut waiters = Vec::new();
    for value in [6, 7] {
        let (item, waiter) = owned(scope(value));
        assert!(handle.try_submit(item).is_ok());
        assert_eq!(waiter.try_recv(), Err(TryRecvError::Empty));
        waiters.push(waiter);
    }
    drop(actor);
    for waiter in waiters {
        assert_eq!(waiter.try_recv(), Err(TryRecvError::Disconnected));
    }
    let (late, late_waiter) = owned(scope(8));
    let refused = producer.try_submit(late).err().unwrap();
    assert_eq!(refused.reason, AdmissionRefusal::Closed);
    assert_eq!(late_waiter.try_recv(), Err(TryRecvError::Empty));
    drop(refused.input);
    assert_eq!(late_waiter.try_recv(), Err(TryRecvError::Disconnected));
    assert_eq!(handle.usage().unwrap().retained_items, 0);
    assert_eq!(handle.usage().unwrap().retained_bytes, 0);
    assert!(owner.is_current());
    assert_eq!(owner.checkpoint(), &initial());
    let db = db.lock().unwrap();
    assert_eq!(db.engine_calls, 0);
    assert_eq!(db.commit_calls, 0);
    assert!(db.receipts.is_empty());
    assert!(db.events.is_empty());
    assert_eq!(db.checkpoint, initial());
}

#[test]
fn postcommit_delivery_failure_preserves_confirmation_and_durable_intents() {
    let db = database();
    let mut owner = owner_configured(&db, None, false, true);
    assert_confirmed(submit(&mut owner, scope(6)));
    assert_eq!(
        db.lock().unwrap().checkpoint.state().intents[0].status,
        DurableStatus::Pending
    );
}

#[test]
fn sequence_exhaustion_never_calls_engine_or_repository_commit() {
    let db = database();
    let mut last = basis();
    last.revision = revision(2, u64::MAX);
    let checkpoint = checkpoint_at(last, state());
    let mut owner = DurableOwner::new(
        Repository {
            database: Arc::clone(&db),
            before_ack: None,
            force_duplicate_race: false,
        },
        Engine {
            database: Arc::clone(&db),
        },
        Publication {
            database: Arc::clone(&db),
            fail: false,
        },
        checkpoint,
        RECEIPT_BYTES,
    )
    .unwrap();
    let mut operation = scope(6);
    operation.basis = last;
    assert_eq!(
        submit(&mut owner, operation),
        SubmissionOutcome::Refused(RepositoryError::SequenceExhausted)
    );
    let db = db.lock().unwrap();
    assert_eq!(db.commit_calls, 0);
    assert_eq!(db.engine_calls, 0);
}

struct FixedEngine {
    candidate: Checkpoint,
}
impl SessionEngine<Scope> for FixedEngine {
    fn decide(
        &mut self,
        _: &Checkpoint,
        _: &Scope,
        _: &GameInput,
    ) -> Result<Checkpoint, RepositoryError> {
        Ok(self.candidate.clone())
    }
    fn validate_recovery(&mut self, checkpoint: &Checkpoint) -> Result<(), RepositoryError> {
        checkpoint
            .validate_resume(checkpoint.basis(), &pins())
            .map(|_| ())
            .map_err(|_| RepositoryError::InvalidCandidate)
    }
}

#[test]
fn engine_cannot_issue_epoch_skip_sequence_change_run_or_omit_operation_result() {
    for changed in 0..4 {
        let db = database();
        let mut candidate_basis = basis();
        candidate_basis.revision = basis().revision.next_sequence().unwrap();
        match changed {
            0 => candidate_basis.revision = revision(3, 0),
            1 => candidate_basis.revision = revision(2, basis().revision.sequence() + 2),
            2 => candidate_basis.run = RunId::from_bytes(&[90; 16]).unwrap(),
            3 => {}
            _ => unreachable!(),
        }
        let mut owner = DurableOwner::new(
            Repository {
                database: Arc::clone(&db),
                before_ack: None,
                force_duplicate_race: false,
            },
            FixedEngine {
                candidate: checkpoint_at(candidate_basis, state()),
            },
            Publication {
                database: Arc::clone(&db),
                fail: false,
            },
            initial(),
            RECEIPT_BYTES,
        )
        .unwrap();
        let (item, wait) = owned(scope(6));
        owner.reduce(AdmissionSequence(1), item);
        assert_eq!(
            wait.recv_timeout(WAIT).unwrap(),
            SubmissionOutcome::Refused(RepositoryError::InvalidCandidate)
        );
        let db = db.lock().unwrap();
        assert_eq!(db.commit_calls, 0);
        assert!(db.receipts.is_empty());
    }
}

struct CountedCandidateEngine {
    database: Arc<Mutex<Database>>,
    fixed: FixedEngine,
}
impl SessionEngine<Scope> for CountedCandidateEngine {
    fn decide(
        &mut self,
        current: &Checkpoint,
        scope: &Scope,
        input: &GameInput,
    ) -> Result<Checkpoint, RepositoryError> {
        self.database.lock().unwrap().engine_calls += 1;
        self.fixed.decide(current, scope, input)
    }
    fn validate_recovery(&mut self, checkpoint: &Checkpoint) -> Result<(), RepositoryError> {
        self.fixed.validate_recovery(checkpoint)
    }
}
fn candidate_owner(
    db: &Arc<Mutex<Database>>,
    current: Checkpoint,
    candidate: Checkpoint,
) -> DurableOwner<Repository, CountedCandidateEngine, Publication> {
    DurableOwner::new(
        Repository {
            database: Arc::clone(db),
            before_ack: None,
            force_duplicate_race: false,
        },
        CountedCandidateEngine {
            database: Arc::clone(db),
            fixed: FixedEngine { candidate },
        },
        Publication {
            database: Arc::clone(db),
            fail: false,
        },
        current,
        RECEIPT_BYTES,
    )
    .unwrap()
}
fn history_checkpoint() -> Checkpoint {
    let accepted = proposed(&initial(), &scope(70));
    let mut state = accepted.state().clone();
    // Retained canonical records can include history outside a receipt's projection.
    let mut fact = state.facts[0].clone();
    fact.id = FactId::from_bytes(&[71; 16]).unwrap();
    fact.ordinal = 1;
    state.facts.push(fact);
    let draw = ActualDraw {
        operation: operation(70),
        ordinal: 0,
        resolution: ResolutionId::from_bytes(&[72; 16]).unwrap(),
        window: WindowId::from_bytes(&[73; 16]).unwrap(),
        sides: 20,
        value: 7,
        source: rule(),
    };
    state.draws.push(draw.clone());
    state.draws.push(ActualDraw {
        ordinal: 1,
        value: 8,
        ..draw
    });
    state.decisions[0].draws.push(0);
    checkpoint_at(accepted.basis(), state)
}
fn assert_history_candidate_refused(current: &Checkpoint, candidate: Checkpoint, scope: Scope) {
    // Both checkpoints have already passed Checkpoint::new's canonical validation.
    let db = database();
    db.lock().unwrap().checkpoint = current.clone();
    let mut owner = candidate_owner(&db, current.clone(), candidate);
    let (item, wait) = owned(scope);
    owner.reduce(AdmissionSequence(1), item);
    assert_eq!(
        wait.recv_timeout(WAIT).unwrap(),
        SubmissionOutcome::Refused(RepositoryError::InvalidCandidate)
    );
    assert!(owner.is_current());
    let db = db.lock().unwrap();
    assert_eq!(db.engine_calls, 1);
    assert_eq!(db.commit_calls, 0);
    assert_eq!(db.checkpoint, *current);
    assert!(db.receipts.is_empty());
    assert!(db.scoped_receipts.is_empty());
    assert_eq!(db.events, vec!["lookup"]);
}

#[test]
fn session_rejects_additional_decisions_at_current_or_historical_revision() {
    let current = history_checkpoint();
    let mut request = scope(74);
    request.basis = current.basis();
    let candidate = proposed(&current, &request);
    for revision in [candidate.basis().revision, current.basis().revision] {
        let mut state = candidate.state().clone();
        state.decisions.push(AcceptedDecision {
            operation: operation(75),
            revision,
            facts: vec![],
            draws: vec![],
            effects: vec![],
            source_policy: label("fixture-extra-decision"),
            semantic_output: None,
        });
        assert_history_candidate_refused(
            &current,
            checkpoint_at(candidate.basis(), state),
            request.clone(),
        );
    }
}

#[test]
fn session_rejects_rewritten_deleted_or_reordered_decision_history() {
    let current = history_checkpoint();
    let mut request = scope(74);
    request.basis = current.basis();
    let candidate = proposed(&current, &request);
    for changed in 0..3 {
        let mut state = candidate.state().clone();
        match changed {
            0 => state.decisions[0].semantic_output = Some("rewritten history".to_owned()),
            1 => {
                state.decisions.remove(0);
            }
            2 => state.decisions.swap(0, 1),
            _ => unreachable!(),
        }
        assert_history_candidate_refused(
            &current,
            checkpoint_at(candidate.basis(), state),
            request.clone(),
        );
    }
}

#[test]
fn session_rejects_rewritten_or_deleted_fact_history_with_decisions_unchanged() {
    let current = history_checkpoint();
    let mut request = scope(74);
    request.basis = current.basis();
    let candidate = proposed(&current, &request);
    for changed in 0..2 {
        let mut state = candidate.state().clone();
        match changed {
            0 => state.facts[0].audience = AudienceScope::Host,
            1 => {
                state.facts.remove(1);
            }
            _ => unreachable!(),
        }
        assert_eq!(state.decisions, candidate.state().decisions);
        assert_history_candidate_refused(
            &current,
            checkpoint_at(candidate.basis(), state),
            request.clone(),
        );
    }
}

#[test]
fn session_rejects_rewritten_or_deleted_draw_history_with_decisions_unchanged() {
    let current = history_checkpoint();
    let mut request = scope(74);
    request.basis = current.basis();
    let candidate = proposed(&current, &request);
    for changed in 0..2 {
        let mut state = candidate.state().clone();
        match changed {
            0 => state.draws[0].value = 9,
            1 => {
                state.draws.remove(1);
            }
            _ => unreachable!(),
        }
        assert_eq!(state.decisions, candidate.state().decisions);
        assert_history_candidate_refused(
            &current,
            checkpoint_at(candidate.basis(), state),
            request.clone(),
        );
    }
}

#[test]
fn session_accepts_one_operation_with_mutable_state_and_retries_without_reduction() {
    for failure in [Failure::None, Failure::LostCommittedAck] {
        let current = history_checkpoint();
        let mut request = scope(74);
        request.basis = current.basis();
        let candidate = proposed(&current, &request);
        let mut state = candidate.state().clone();
        state.resources[0].value = 3;
        state.intents[0].status = DurableStatus::Completed;
        state.draws.push(ActualDraw {
            operation: request.operation,
            ordinal: 0,
            resolution: ResolutionId::from_bytes(&[76; 16]).unwrap(),
            window: WindowId::from_bytes(&[77; 16]).unwrap(),
            sides: 20,
            value: 10,
            source: rule(),
        });
        state.decisions.last_mut().unwrap().draws.push(0);
        let candidate = checkpoint_at(candidate.basis(), state);
        let expected = receipt(&candidate, request.operation);
        let db = database();
        {
            let mut db = db.lock().unwrap();
            db.checkpoint = current.clone();
            db.failure = failure;
        }
        let mut owner = candidate_owner(&db, current, candidate.clone());
        let (item, wait) = owned(request.clone());
        owner.reduce(AdmissionSequence(1), item);
        let first = wait.recv_timeout(WAIT).unwrap();
        if failure == Failure::LostCommittedAck {
            assert_eq!(first, SubmissionOutcome::LookupRequired);
            assert!(!owner.is_current());
        } else {
            assert_eq!(assert_confirmed(first), expected);
            assert!(owner.is_current());
        }
        let before_retry = db.lock().unwrap().events.clone();
        let (item, wait) = owned(request);
        owner.reduce(AdmissionSequence(2), item);
        assert_eq!(assert_confirmed(wait.recv_timeout(WAIT).unwrap()), expected);
        assert!(owner.is_current());
        let db = db.lock().unwrap();
        assert_eq!(db.checkpoint, candidate);
        assert_eq!(db.engine_calls, 1);
        assert_eq!(db.commit_calls, 1);
        assert_eq!(db.receipts.len(), 1);
        let expected_retry = if failure == Failure::LostCommittedAck {
            vec!["lookup", "load"]
        } else {
            vec!["lookup"]
        };
        assert_eq!(&db.events[before_retry.len()..], expected_retry.as_slice());
        let deliveries = if failure == Failure::LostCommittedAck {
            0
        } else {
            1
        };
        assert_eq!(
            db.events
                .iter()
                .filter(|event| **event == "publish")
                .count(),
            deliveries
        );
        assert_eq!(
            db.events.iter().filter(|event| **event == "wake").count(),
            deliveries
        );
    }
}

fn alternate_intent_content() -> ContentReference {
    ContentReference {
        entry: label("fixture-alternate-intent"),
        ..content()
    }
}
fn intent_checkpoint_at(basis: Basis, state: GameState) -> Checkpoint {
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        basis,
        pins(),
        state,
        ReferenceInventory {
            rules: &[rule()],
            content: &[content(), alternate_intent_content()],
            resources: &resource_constraints(),
            assets: &[],
        },
        limits(),
    )
    .unwrap()
}
fn retained_intent_checkpoint() -> Checkpoint {
    let current = history_checkpoint();
    let mut state = current.state().clone();
    // Canonical history can retain effects outside a receipt's projected effect list.
    state.intents.push(DurableIntent {
        id: EffectId::from_bytes(&[80; 16]).unwrap(),
        basis: current.basis(),
        operation: operation(80),
        slot: 0,
        kind: EffectKind::RunAi,
        job: Some(JobId::from_bytes(&[81; 16]).unwrap()),
        timer: None,
        generation: 3,
        status: DurableStatus::SentUnknown,
        definition: content(),
    });
    for value in [84, 85] {
        state.timers.push(OwnedTimer {
            id: TimerId::from_bytes(&[value; 16]).unwrap(),
            basis: current.basis(),
            generation: 2,
            due: state.logical_time,
            source: rule(),
            status: DurableStatus::Pending,
        });
    }
    state.intents.push(DurableIntent {
        id: EffectId::from_bytes(&[82; 16]).unwrap(),
        basis: current.basis(),
        operation: operation(82),
        slot: 0,
        kind: EffectKind::ArmTimer,
        job: None,
        timer: Some(TimerId::from_bytes(&[84; 16]).unwrap()),
        generation: 2,
        status: DurableStatus::Pending,
        definition: content(),
    });
    intent_checkpoint_at(current.basis(), state)
}

fn retained_job_completion(request: &Scope, job: &DurableIntent) -> GameInput {
    GameInput::Job(JobCompletion {
        basis: request.basis,
        operation: request.operation,
        job: job.job.unwrap(),
        generation: job.generation,
        outcome: JobOutcome::Ai {
            semantic_output: "fixture completed job".to_owned(),
            policy: content(),
            model: label("fixture-model-1"),
        },
    })
}

#[test]
fn session_refuses_each_retained_intent_binding_change_without_commit_or_delivery() {
    let current = retained_intent_checkpoint();
    let mut request = scope(74);
    request.basis = current.basis();
    let candidate = proposed(&current, &request);
    let completion = retained_job_completion(&request, &current.state().intents[1]);
    for changed in 0..11 {
        let mut state = candidate.state().clone();
        match changed {
            0 => state.intents[1].basis.revision = basis().revision,
            1 => state.intents[1].operation = operation(86),
            2 => state.intents[1].slot = 1,
            3 => state.intents[1].kind = EffectKind::RunMedia,
            4 => state.intents[1].job = Some(JobId::from_bytes(&[83; 16]).unwrap()),
            5 => state.intents[2].timer = Some(TimerId::from_bytes(&[85; 16]).unwrap()),
            6 => state.intents[1].generation = 4,
            7 => state.intents[1].definition = alternate_intent_content(),
            8 => state.intents[1].id = EffectId::from_bytes(&[87; 16]).unwrap(),
            9 => {
                state.intents.remove(1);
            }
            10 => {
                state.intents.remove(2);
                state.intents.remove(1);
            }
            _ => unreachable!(),
        }
        assert_eq!(state.decisions, candidate.state().decisions);
        assert_eq!(state.facts, candidate.state().facts);
        assert_eq!(state.draws, candidate.state().draws);
        // Every malformed transition remains a valid canonical checkpoint; only
        // the retained-intent transition policy can reject it at this boundary.
        let malformed = intent_checkpoint_at(candidate.basis(), state);
        let db = database();
        db.lock().unwrap().checkpoint = current.clone();
        let mut owner = candidate_owner(&db, current.clone(), malformed);
        let (item, wait) = OwnedInput::new(context(), request.clone(), completion.clone());
        owner.reduce(AdmissionSequence(1), item);
        assert_eq!(
            wait.recv_timeout(WAIT).unwrap(),
            SubmissionOutcome::Refused(RepositoryError::InvalidCandidate)
        );
        assert!(owner.is_current());
        assert_eq!(owner.checkpoint(), &current);
        let db = db.lock().unwrap();
        assert_eq!(db.engine_calls, 1);
        assert_eq!(db.commit_calls, 0);
        assert_eq!(db.checkpoint, current);
        assert!(db.receipts.is_empty());
        assert!(db.scoped_receipts.is_empty());
        assert_eq!(db.events, vec!["lookup"]);
    }
}

#[test]
fn session_accepts_status_only_job_completion_with_reordered_retained_intents() {
    for failure in [Failure::None, Failure::LostCommittedAck] {
        let current = retained_intent_checkpoint();
        let job = current.state().intents[1].clone();
        let mut request = scope(74);
        request.basis = current.basis();
        let completion = retained_job_completion(&request, &job);
        let candidate = proposed(&current, &request);
        let mut state = candidate.state().clone();
        state.intents[1].status = DurableStatus::Completed;
        state.intents.reverse();
        let candidate = intent_checkpoint_at(candidate.basis(), state);
        let expected = receipt(&candidate, request.operation);
        let db = database();
        {
            let mut db = db.lock().unwrap();
            db.checkpoint = current.clone();
            db.failure = failure;
        }
        let mut owner = candidate_owner(&db, current, candidate.clone());
        // This is the actual Session Job input boundary with a controlled engine
        // candidate. The native courier owns completion staging and its own proof.
        let (item, wait) = OwnedInput::new(context(), request.clone(), completion.clone());
        owner.reduce(AdmissionSequence(1), item);
        let first = wait.recv_timeout(WAIT).unwrap();
        if failure == Failure::LostCommittedAck {
            assert_eq!(first, SubmissionOutcome::LookupRequired);
            assert!(!owner.is_current());
        } else {
            assert_eq!(assert_confirmed(first), expected);
            assert!(owner.is_current());
            assert_eq!(owner.checkpoint(), &candidate);
        }
        let before_retry = db.lock().unwrap().events.clone();
        let (item, wait) = OwnedInput::new(context(), request, completion);
        owner.reduce(AdmissionSequence(2), item);
        assert_eq!(assert_confirmed(wait.recv_timeout(WAIT).unwrap()), expected);
        assert!(owner.is_current());
        assert_eq!(owner.checkpoint(), &candidate);
        let mut retained = owner
            .checkpoint()
            .state()
            .intents
            .iter()
            .find(|intent| intent.id == job.id)
            .unwrap()
            .clone();
        assert_eq!(retained.status, DurableStatus::Completed);
        retained.status = job.status;
        assert_eq!(retained, job);
        let db = db.lock().unwrap();
        assert_eq!(db.checkpoint, candidate);
        assert_eq!(db.engine_calls, 1);
        assert_eq!(db.commit_calls, 1);
        assert_eq!(db.receipts.len(), 1);
        let expected_retry = if failure == Failure::LostCommittedAck {
            vec!["lookup", "load"]
        } else {
            vec!["lookup"]
        };
        assert_eq!(&db.events[before_retry.len()..], expected_retry.as_slice());
        let deliveries = if failure == Failure::LostCommittedAck {
            0
        } else {
            1
        };
        assert_eq!(
            db.events
                .iter()
                .filter(|event| **event == "publish")
                .count(),
            deliveries
        );
        assert_eq!(
            db.events.iter().filter(|event| **event == "wake").count(),
            deliveries
        );
    }
}

#[test]
fn candidate_receipt_capacity_refuses_before_any_commit() {
    let db = database();
    let capacity = receipt(&proposed(&initial(), &scope(6)), operation(6))
        .retained_bytes()
        .unwrap()
        - 1;
    assert!(capacity >= std::mem::size_of::<ScopeKey>());
    let mut owner = DurableOwner::new(
        Repository {
            database: Arc::clone(&db),
            before_ack: None,
            force_duplicate_race: false,
        },
        Engine {
            database: Arc::clone(&db),
        },
        Publication {
            database: Arc::clone(&db),
            fail: false,
        },
        initial(),
        capacity,
    )
    .unwrap();
    assert_eq!(
        submit(&mut owner, scope(6)),
        SubmissionOutcome::Refused(RepositoryError::Capacity)
    );
    let db = db.lock().unwrap();
    assert_eq!(db.engine_calls, 1);
    assert_eq!(db.commit_calls, 0);
    assert!(db.receipts.is_empty());
}

#[test]
fn mismatched_input_operation_is_rejected_before_lookup_or_engine() {
    let db = database();
    let mut owner = owner(&db);
    let (item, wait) = OwnedInput::new(context(), scope(6), input(&scope(7)));
    owner.reduce(AdmissionSequence(1), item);
    assert_eq!(
        wait.recv_timeout(WAIT).unwrap(),
        SubmissionOutcome::Refused(RepositoryError::InputBinding)
    );
    let db = db.lock().unwrap();
    assert!(db.events.is_empty());
    assert_eq!(db.engine_calls, 0);
}

#[test]
fn owned_input_charges_actual_context_and_model_spare_capacities() {
    let mut text = String::with_capacity(4096);
    text.push_str("hi");
    let mut correlation = context();
    correlation.build.reserve(1024);
    let retained_context = correlation.trace_parent.capacity() + correlation.build.capacity();
    let scope = scope(6);
    let command = GameInput::Game(CommandInput {
        basis: scope.basis,
        observed_revision: scope.basis.revision,
        operation: scope.operation,
        member: member(3),
        command: GameCommand::Speak {
            speaker: entity(4),
            text,
            conversation: None,
        },
    });
    let retained_model = command.retained_heap_bytes().unwrap();
    let (item, _waiter) = OwnedInput::new(correlation, scope, command);
    assert_eq!(
        item.retained_heap_bytes(),
        Some(retained_model + retained_context)
    );
    assert!(item.retained_heap_bytes().unwrap() >= 4096 + 1024);
}

fn assert_lookup_fences_new_key(behavior: LookupBehavior) {
    let db = database();
    db.lock().unwrap().lookup.insert(operation(6), behavior);
    let mut owner = owner(&db);
    let first = submit(&mut owner, scope(6));
    let expected = match behavior {
        LookupBehavior::InProgress => SubmissionOutcome::LookupRequired,
        LookupBehavior::Expired => SubmissionOutcome::ExpiredOrIndeterminate,
        LookupBehavior::Unavailable => SubmissionOutcome::Refused(RepositoryError::Unavailable),
        LookupBehavior::Malformed => SubmissionOutcome::Refused(RepositoryError::InvalidReceipt),
        LookupBehavior::TooLarge => SubmissionOutcome::Refused(RepositoryError::Capacity),
    };
    assert_eq!(first, expected);
    assert_eq!(
        submit(&mut owner, scope(7)),
        SubmissionOutcome::LookupRequired
    );
    let db = db.lock().unwrap();
    assert_eq!(db.engine_calls, 0);
    assert_eq!(db.commit_calls, 0);
    assert_eq!(db.events, ["lookup", "lookup"]);
    assert_eq!(db.reconnect_calls, 0);
    assert!(owner.has_uncertain_operation());
    assert_eq!(owner.checkpoint(), &initial());
    assert!(db.receipts.is_empty());
}

#[test]
fn in_progress_lookup_fences_different_not_recorded_key() {
    assert_lookup_fences_new_key(LookupBehavior::InProgress);
}
#[test]
fn expired_indeterminate_lookup_fences_different_not_recorded_key() {
    assert_lookup_fences_new_key(LookupBehavior::Expired);
}
#[test]
fn unavailable_lookup_fences_different_not_recorded_key() {
    assert_lookup_fences_new_key(LookupBehavior::Unavailable);
}
#[test]
fn malformed_committed_lookup_fences_different_not_recorded_key() {
    assert_lookup_fences_new_key(LookupBehavior::Malformed);
}
#[test]
fn oversized_committed_lookup_fences_different_not_recorded_key() {
    assert_lookup_fences_new_key(LookupBehavior::TooLarge);
}

fn seed_committed(db: &Arc<Mutex<Database>>, operation: &Scope) -> DecisionReceipt {
    let mut db = db.lock().unwrap();
    let committed = proposed(&db.checkpoint, operation);
    let result = receipt(&committed, operation.operation);
    db.checkpoint = committed;
    db.receipts
        .insert(operation.operation, (operation.fingerprint, result.clone()));
    db.lookup.remove(&operation.operation);
    result
}

#[test]
fn current_cache_discovers_peer_lost_ack_receipt_and_reloads_before_fresh_reduction() {
    let db = database();
    let mut stale_owner = owner(&db);
    let mut committing_owner = owner(&db);
    db.lock().unwrap().failure = Failure::LostCommittedAck;
    assert_eq!(
        submit(&mut committing_owner, scope(6)),
        SubmissionOutcome::LookupRequired
    );
    let stored = db.lock().unwrap().receipts[&operation(6)].1.clone();
    db.lock().unwrap().failure = Failure::None;
    db.lock().unwrap().events.clear();
    assert!(stale_owner.is_current());
    assert_eq!(stale_owner.checkpoint(), &initial());

    assert_eq!(assert_confirmed(submit(&mut stale_owner, scope(6))), stored);
    assert!(stale_owner.is_current());
    assert_eq!(stale_owner.checkpoint(), &db.lock().unwrap().checkpoint);
    {
        let db = db.lock().unwrap();
        assert_eq!(db.events, ["lookup", "load"]);
        assert_eq!(db.engine_calls, 1);
        assert_eq!(db.commit_calls, 1);
    }

    let mut fresh = scope(7);
    fresh.basis = stale_owner.checkpoint().basis();
    let next = assert_confirmed(submit(&mut stale_owner, fresh));
    assert_eq!(
        next.basis().revision,
        stored.basis().revision.next_sequence().unwrap()
    );
    let db = db.lock().unwrap();
    assert_eq!(db.engine_calls, 2);
    assert_eq!(db.commit_calls, 2);
    assert_eq!(db.receipts.len(), 2);
    assert_eq!(db.checkpoint.state().facts.len(), 2);
    assert_eq!(db.checkpoint.state().intents.len(), 2);
}

#[test]
fn newer_lookup_receipt_with_failed_reload_keeps_current_cache_fenced() {
    let db = database();
    let mut owner = owner(&db);
    let stored = seed_committed(&db, &scope(6));
    db.lock().unwrap().reload_failure = true;

    assert_eq!(assert_confirmed(submit(&mut owner, scope(6))), stored);
    assert!(!owner.is_current());
    assert!(!owner.has_uncertain_operation());
    assert_eq!(owner.checkpoint(), &initial());
    assert_eq!(
        submit(&mut owner, scope(7)),
        SubmissionOutcome::LookupRequired
    );
    {
        let db = db.lock().unwrap();
        assert_eq!(db.events, ["lookup", "load", "lookup"]);
        assert_eq!(db.engine_calls, 0);
        assert_eq!(db.commit_calls, 0);
        assert_eq!(db.receipts.len(), 1);
    }

    db.lock().unwrap().reload_failure = false;
    db.lock().unwrap().events.clear();
    assert_eq!(assert_confirmed(submit(&mut owner, scope(6))), stored);
    assert!(owner.is_current());
    assert_eq!(db.lock().unwrap().events, ["lookup", "load"]);
    let mut fresh = scope(7);
    fresh.basis = owner.checkpoint().basis();
    assert_confirmed(submit(&mut owner, fresh));
    assert_eq!(db.lock().unwrap().engine_calls, 1);
    assert_eq!(db.lock().unwrap().commit_calls, 1);
}

#[test]
fn newer_lookup_receipt_requires_nonregressing_reload_with_exact_stored_decision() {
    for invalid_reload in [initial(), proposed(&initial(), &scope(7))] {
        let db = database();
        let mut owner = owner(&db);
        let stored = seed_committed(&db, &scope(6));
        db.lock().unwrap().reload_override = Some(invalid_reload);

        assert_eq!(assert_confirmed(submit(&mut owner, scope(6))), stored);
        assert!(!owner.is_current());
        assert!(!owner.has_uncertain_operation());
        assert_eq!(owner.checkpoint(), &initial());
        assert_eq!(
            submit(&mut owner, scope(7)),
            SubmissionOutcome::LookupRequired
        );
        {
            let db = db.lock().unwrap();
            assert_eq!(db.events, ["lookup", "load", "lookup"]);
            assert_eq!(db.engine_calls, 0);
            assert_eq!(db.commit_calls, 0);
            assert_eq!(db.receipts.len(), 1);
        }

        db.lock().unwrap().reload_override = None;
        db.lock().unwrap().events.clear();
        assert_eq!(assert_confirmed(submit(&mut owner, scope(6))), stored);
        assert!(owner.is_current());
        assert_eq!(owner.checkpoint(), &db.lock().unwrap().checkpoint);
        assert_eq!(db.lock().unwrap().events, ["lookup", "load"]);
        assert_eq!(db.lock().unwrap().engine_calls, 0);
        assert_eq!(db.lock().unwrap().commit_calls, 0);
    }
}

#[test]
fn historical_and_equal_lookup_receipts_preserve_newer_current_snapshot_without_reload() {
    let db = database();
    let mut owner = owner(&db);
    let historical = assert_confirmed(submit(&mut owner, scope(6)));
    let mut fresh = scope(7);
    fresh.basis = owner.checkpoint().basis();
    let latest = assert_confirmed(submit(&mut owner, fresh.clone()));
    let checkpoint = owner.checkpoint().clone();
    db.lock().unwrap().events.clear();
    db.lock().unwrap().reload_failure = true;

    assert_eq!(assert_confirmed(submit(&mut owner, scope(6))), historical);
    assert_eq!(assert_confirmed(submit(&mut owner, fresh)), latest);
    assert!(owner.is_current());
    assert_eq!(owner.checkpoint(), &checkpoint);
    let db = db.lock().unwrap();
    assert_eq!(db.events, ["lookup", "lookup"]);
    assert_eq!(db.engine_calls, 2);
    assert_eq!(db.commit_calls, 2);
    assert_eq!(db.checkpoint, checkpoint);
}

#[test]
fn inspecting_b_unknown_and_then_b_committed_cannot_erase_unresolved_a() {
    for second in [
        LookupBehavior::InProgress,
        LookupBehavior::Expired,
        LookupBehavior::Unavailable,
        LookupBehavior::Malformed,
        LookupBehavior::TooLarge,
    ] {
        let db = database();
        db.lock()
            .unwrap()
            .lookup
            .insert(operation(6), LookupBehavior::InProgress);
        db.lock().unwrap().lookup.insert(operation(7), second);
        let mut owner = owner(&db);
        assert_eq!(
            submit(&mut owner, scope(6)),
            SubmissionOutcome::LookupRequired
        );
        let second_outcome = submit(&mut owner, scope(7));
        let expected = match second {
            LookupBehavior::InProgress => SubmissionOutcome::LookupRequired,
            LookupBehavior::Expired => SubmissionOutcome::ExpiredOrIndeterminate,
            LookupBehavior::Unavailable => SubmissionOutcome::Refused(RepositoryError::Unavailable),
            LookupBehavior::Malformed => {
                SubmissionOutcome::Refused(RepositoryError::InvalidReceipt)
            }
            LookupBehavior::TooLarge => SubmissionOutcome::Refused(RepositoryError::Capacity),
        };
        assert_eq!(second_outcome, expected);
        let retained_b = seed_committed(&db, &scope(7));
        assert_eq!(assert_confirmed(submit(&mut owner, scope(7))), retained_b);
        assert_eq!(db.lock().unwrap().events, ["lookup", "lookup", "lookup"]);
        assert!(
            owner
                .matches_uncertain_retry(&scope(6), &input(&scope(6)))
                .unwrap()
        );
        assert!(
            !owner
                .matches_uncertain_retry(&scope(7), &input(&scope(7)))
                .unwrap()
        );
        assert_eq!(
            submit(&mut owner, scope(8)),
            SubmissionOutcome::LookupRequired
        );
        let db = db.lock().unwrap();
        assert_eq!(db.engine_calls, 0);
        assert_eq!(db.commit_calls, 0);
        assert_eq!(db.events, ["lookup", "lookup", "lookup", "lookup"]);
        assert_eq!(db.reconnect_calls, 0);
        assert!(owner.has_uncertain_operation());
        assert!(!db.events.contains(&"load"));
        assert!(!db.events.contains(&"publish"));
        assert!(!db.events.contains(&"wake"));
        assert_eq!(owner.checkpoint(), &initial());
    }
}

#[test]
fn exact_a_committed_lookup_and_validated_reload_release_b_for_new_commit() {
    let db = database();
    db.lock()
        .unwrap()
        .lookup
        .insert(operation(6), LookupBehavior::InProgress);
    let mut owner = owner(&db);
    assert_eq!(
        submit(&mut owner, scope(6)),
        SubmissionOutcome::LookupRequired
    );
    assert_eq!(
        submit(&mut owner, scope(7)),
        SubmissionOutcome::LookupRequired
    );
    let a_receipt = seed_committed(&db, &scope(6));
    assert_eq!(assert_confirmed(submit(&mut owner, scope(6))), a_receipt);
    let mut b = scope(7);
    b.basis = owner.checkpoint().basis();
    let b_receipt = assert_confirmed(submit(&mut owner, b));
    assert_eq!(
        b_receipt.basis().revision,
        a_receipt.basis().revision.next_sequence().unwrap()
    );
    let db = db.lock().unwrap();
    assert_eq!(db.engine_calls, 1);
    assert_eq!(db.commit_calls, 1);
    assert_eq!(db.receipts.len(), 2);
    assert_eq!(db.checkpoint.state().decisions.len(), 2);
}

#[test]
fn exact_a_receipt_with_failed_reload_does_not_release_b() {
    let db = database();
    db.lock()
        .unwrap()
        .lookup
        .insert(operation(6), LookupBehavior::InProgress);
    let mut owner = owner(&db);
    assert_eq!(
        submit(&mut owner, scope(6)),
        SubmissionOutcome::LookupRequired
    );
    let a_receipt = seed_committed(&db, &scope(6));
    db.lock().unwrap().reload_failure = true;
    assert_eq!(assert_confirmed(submit(&mut owner, scope(6))), a_receipt);
    assert_eq!(
        submit(&mut owner, scope(7)),
        SubmissionOutcome::LookupRequired
    );
    assert_eq!(
        owner.reload_current(&scope(6), &context()),
        Err(RepositoryError::UnresolvedCommit)
    );
    assert_eq!(db.lock().unwrap().commit_calls, 0);
    db.lock().unwrap().reload_failure = false;
    assert_eq!(assert_confirmed(submit(&mut owner, scope(6))), a_receipt);
    let mut b = scope(7);
    b.basis = owner.checkpoint().basis();
    assert_confirmed(submit(&mut owner, b));
    assert_eq!(db.lock().unwrap().commit_calls, 1);
}

#[test]
fn acknowledged_a_with_loaded_snapshot_older_than_receipt_remains_fenced() {
    let db = database();
    db.lock()
        .unwrap()
        .lookup
        .insert(operation(6), LookupBehavior::InProgress);
    let mut owner = owner(&db);
    assert_eq!(
        submit(&mut owner, scope(6)),
        SubmissionOutcome::LookupRequired
    );
    let a_receipt = seed_committed(&db, &scope(6));
    db.lock().unwrap().reload_override = Some(initial());
    assert_eq!(assert_confirmed(submit(&mut owner, scope(6))), a_receipt);
    assert_eq!(
        submit(&mut owner, scope(7)),
        SubmissionOutcome::LookupRequired
    );
    assert_eq!(owner.checkpoint(), &initial());
    assert_eq!(db.lock().unwrap().commit_calls, 0);
}

#[test]
fn recovered_lookup_only_a_not_recorded_fences_new_b_without_assuming_replay_safe() {
    let db = database();
    let mut owner = owner(&db);
    let mut a = scope(6);
    a.lookup_only = true;
    assert_eq!(submit(&mut owner, a), SubmissionOutcome::LookupRequired);
    assert_eq!(
        submit(&mut owner, scope(7)),
        SubmissionOutcome::LookupRequired
    );
    assert_eq!(
        submit(&mut owner, scope(6)),
        SubmissionOutcome::LookupRequired
    );
    let db = db.lock().unwrap();
    assert_eq!(db.engine_calls, 0);
    assert_eq!(db.commit_calls, 0);
}

#[test]
fn confirmed_not_recorded_and_known_precommit_refusal_do_not_fabricate_uncertainty() {
    let db = database();
    db.lock().unwrap().failure = Failure::BeforeCommit;
    let mut owner = owner(&db);
    assert_eq!(
        submit(&mut owner, scope(6)),
        SubmissionOutcome::Refused(RepositoryError::Unavailable)
    );
    assert_eq!(db.lock().unwrap().checkpoint, initial());
    db.lock().unwrap().failure = Failure::None;
    assert_confirmed(submit(&mut owner, scope(7)));
    let db = db.lock().unwrap();
    assert_eq!(db.engine_calls, 2);
    assert_eq!(db.commit_calls, 2);
    assert!(db.receipts.contains_key(&operation(7)));
    assert!(!db.receipts.contains_key(&operation(6)));
}

fn assert_full_scope_collision_stays_fenced(dimension: &str) {
    let db = database();
    let unresolved = scope(6);
    let mut historical = unresolved.clone();
    match dimension {
        "tenant" => historical.tenant = 2,
        "session" => historical.basis.session = SessionId::from_bytes(&[70; 16]).unwrap(),
        "principal" => historical.principal = 2,
        "namespace" => historical.namespace = vec![2],
        "recovery_epoch" => historical.basis.revision = revision(1, 7),
        "fingerprint_version" => historical.fingerprint_version = 2,
        "fingerprint" => historical.fingerprint += 1,
        _ => panic!("unknown fixture dimension"),
    }
    historical.basis.run = RunId::from_bytes(&[70; 16]).unwrap();
    historical.lookup_only = true;
    let first_key = unresolved.capture_uncertainty_key(RECEIPT_BYTES).unwrap();
    let historical_key = historical.capture_uncertainty_key(RECEIPT_BYTES).unwrap();
    assert!(first_key != historical_key);
    assert_eq!(unresolved.operation(), historical.operation());
    let mut decision = receipt(&proposed(&initial(), &historical), historical.operation)
        .decision()
        .clone();
    decision.revision = historical.basis.revision;
    let historical_receipt =
        DecisionReceipt::new(historical.basis, decision, RECEIPT_BYTES).unwrap();
    {
        let mut db = db.lock().unwrap();
        db.lookup
            .insert(unresolved.operation, LookupBehavior::InProgress);
        db.scoped_receipts
            .push((historical_key, historical_receipt.clone()));
    }
    let mut owner = owner(&db);
    assert_eq!(
        submit(&mut owner, unresolved.clone()),
        SubmissionOutcome::LookupRequired
    );
    assert_eq!(
        assert_confirmed(submit(&mut owner, historical.clone())),
        historical_receipt
    );
    assert_eq!(db.lock().unwrap().events, ["lookup", "lookup"]);
    assert!(
        owner
            .matches_uncertain_retry(&unresolved, &input(&unresolved))
            .unwrap()
    );
    assert!(
        !owner
            .matches_uncertain_retry(&historical, &input(&historical))
            .unwrap()
    );
    assert!(matches!(
        db.lock().unwrap().lookup.get(&unresolved.operation),
        Some(LookupBehavior::InProgress)
    ));
    db.lock().unwrap().events.clear();
    assert_eq!(
        submit(&mut owner, scope(7)),
        SubmissionOutcome::LookupRequired,
        "another {dimension} sharing OperationId must not resolve the first full key"
    );
    let db = db.lock().unwrap();
    assert_eq!(db.events, ["lookup"]);
    assert_eq!(db.reconnect_calls, 0);
    assert!(owner.has_uncertain_operation());
    assert!(!db.events.contains(&"load"));
    assert!(!db.events.contains(&"publish"));
    assert!(!db.events.contains(&"wake"));
    assert_eq!(db.engine_calls, 0);
    assert_eq!(db.commit_calls, 0);
    assert_eq!(owner.checkpoint(), &initial());
}

#[test]
fn scoped_tenant_collision_cannot_clear_first_unknown() {
    assert_full_scope_collision_stays_fenced("tenant");
}
#[test]
fn scoped_session_collision_cannot_clear_first_unknown() {
    assert_full_scope_collision_stays_fenced("session");
}
#[test]
fn scoped_principal_collision_cannot_clear_first_unknown() {
    assert_full_scope_collision_stays_fenced("principal");
}
#[test]
fn scoped_namespace_collision_cannot_clear_first_unknown() {
    assert_full_scope_collision_stays_fenced("namespace");
}
#[test]
fn scoped_epoch_collision_cannot_clear_first_unknown() {
    assert_full_scope_collision_stays_fenced("recovery_epoch");
}
#[test]
fn scoped_fingerprint_version_collision_cannot_clear_first_unknown() {
    assert_full_scope_collision_stays_fenced("fingerprint_version");
}
#[test]
fn scoped_fingerprint_value_collision_cannot_clear_first_unknown() {
    assert_full_scope_collision_stays_fenced("fingerprint");
}

#[test]
fn refreshed_owner_proof_and_run_basis_do_not_change_exact_retained_key() {
    let db = database();
    let a = scope(6);
    db.lock()
        .unwrap()
        .lookup
        .insert(a.operation, LookupBehavior::InProgress);
    let mut owner = owner(&db);
    assert_eq!(
        submit(&mut owner, a.clone()),
        SubmissionOutcome::LookupRequired
    );
    let result = seed_committed(&db, &a);
    let mut refreshed = a.clone();
    refreshed.fence = 2;
    refreshed.basis.run = RunId::from_bytes(&[70; 16]).unwrap();
    refreshed.basis.revision = revision(2, 7);
    refreshed.lookup_only = true;
    refreshed.namespace.reserve(128);
    let first_key = a.capture_uncertainty_key(RECEIPT_BYTES).unwrap();
    let renewed_key = refreshed.capture_uncertainty_key(RECEIPT_BYTES).unwrap();
    assert!(first_key == renewed_key);
    db.lock().unwrap().fence = 2;
    assert_eq!(assert_confirmed(submit(&mut owner, refreshed)), result);
    let mut next = scope(7);
    next.basis = owner.checkpoint().basis();
    next.fence = 2;
    assert_confirmed(submit(&mut owner, next));
    let db = db.lock().unwrap();
    assert_eq!(db.engine_calls, 1);
    assert_eq!(db.commit_calls, 1);
}

#[test]
fn malformed_or_failed_key_capture_refuses_before_any_repository_call() {
    for mode in [
        KeyCapture::Fail,
        KeyCapture::Oversize,
        KeyCapture::Overflow,
        KeyCapture::Unrepresentable,
    ] {
        let db = database();
        let mut owner = owner(&db);
        let mut invalid = scope(6);
        invalid.key_capture = mode;
        assert_eq!(
            submit(&mut owner, invalid),
            SubmissionOutcome::Refused(RepositoryError::Capacity)
        );
        let db = db.lock().unwrap();
        assert!(db.events.is_empty());
        assert_eq!(db.engine_calls, 0);
        assert_eq!(db.commit_calls, 0);
        assert_eq!(owner.checkpoint(), &initial());
    }
}

#[test]
fn key_capture_failure_cannot_erase_first_unknown_or_block_exact_later_resolution() {
    let db = database();
    db.lock()
        .unwrap()
        .lookup
        .insert(operation(6), LookupBehavior::InProgress);
    let mut owner = owner(&db);
    assert_eq!(
        submit(&mut owner, scope(6)),
        SubmissionOutcome::LookupRequired
    );
    for mode in [
        KeyCapture::Fail,
        KeyCapture::Oversize,
        KeyCapture::Overflow,
        KeyCapture::Unrepresentable,
    ] {
        let mut failed = scope(7);
        failed.key_capture = mode;
        db.lock().unwrap().events.clear();
        assert_eq!(
            submit(&mut owner, failed),
            SubmissionOutcome::Refused(RepositoryError::Capacity)
        );
        assert!(db.lock().unwrap().events.is_empty());
        assert_eq!(
            submit(&mut owner, scope(8)),
            SubmissionOutcome::LookupRequired
        );
    }
    let result = seed_committed(&db, &scope(6));
    assert_eq!(assert_confirmed(submit(&mut owner, scope(6))), result);
    let mut next = scope(7);
    next.basis = owner.checkpoint().basis();
    assert_confirmed(submit(&mut owner, next));
    assert_eq!(db.lock().unwrap().commit_calls, 1);
}

#[test]
fn opaque_key_capture_charges_inline_and_actual_heap_with_exact_boundary() {
    let mut operation = scope(6);
    operation.namespace = vec![4; 128];
    let inline = std::mem::size_of::<ScopeKey>();
    assert!(matches!(
        operation.capture_uncertainty_key(inline + 127),
        Err(RepositoryError::Capacity)
    ));
    let key = operation.capture_uncertainty_key(inline + 128).unwrap();
    assert_eq!(key.retained_heap_bytes(), Some(128));
    assert_eq!(
        operation.retained_heap_bytes(),
        Some(operation.namespace.capacity())
    );
    assert_eq!(inline + key.retained_heap_bytes().unwrap(), inline + 128);
    let empty = scope(6).capture_uncertainty_key(inline).unwrap();
    assert_eq!(empty.retained_heap_bytes(), Some(0));
}

// Finite owned-driver stand-in only. Actual PostgreSQL close/join qualification is
// separately executed by the native owner with its real repository and Tokio runtime.
struct LifecycleRepository<'scope> {
    inner: Repository,
    identity: Arc<()>,
    stop: Option<Sender<()>>,
    completed: Receiver<()>,
    driver: Option<std::thread::ScopedJoinHandle<'scope, ()>>,
    fail_next_close: bool,
}
impl SessionRepository for LifecycleRepository<'_> {
    type Scope = Scope;
    fn lookup_operation(
        &mut self,
        operation: &Scope,
        context: &OperationContext,
    ) -> Result<OperationLookup, RepositoryError> {
        self.inner.lookup_operation(operation, context)
    }
    fn commit_decision(
        &mut self,
        operation: &Scope,
        checkpoint: &Checkpoint,
        expected: Basis,
        context: &OperationContext,
    ) -> Result<CommitOutcome, RepositoryError> {
        self.inner
            .commit_decision(operation, checkpoint, expected, context)
    }
    fn load_current(
        &mut self,
        operation: &Scope,
        context: &OperationContext,
    ) -> Result<Checkpoint, RepositoryError> {
        self.inner.load_current(operation, context)
    }
}
impl LifecycleRepository<'_> {
    fn close(&mut self) -> Result<(), RepositoryError> {
        self.inner
            .database
            .lock()
            .unwrap()
            .events
            .push("close_attempt");
        if self.fail_next_close {
            self.fail_next_close = false;
            self.inner
                .database
                .lock()
                .unwrap()
                .events
                .push("close_pending");
            return Err(RepositoryError::Unavailable);
        }
        if self.driver.is_none() {
            return Ok(());
        }
        if let Some(stop) = self.stop.take() {
            stop.send(()).map_err(|_| RepositoryError::Unavailable)?;
        }
        self.completed
            .recv_timeout(WAIT)
            .map_err(|_| RepositoryError::Unavailable)?;
        let driver = self.driver.take().ok_or(RepositoryError::Unavailable)?;
        driver.join().map_err(|_| RepositoryError::Unavailable)?;
        self.inner
            .database
            .lock()
            .unwrap()
            .events
            .push("driver_joined");
        Ok(())
    }
}
fn lifecycle_repository<'scope>(
    threads: &'scope std::thread::Scope<'scope, '_>,
    db: &Arc<Mutex<Database>>,
    identity: &Arc<()>,
    fail_next_close: bool,
) -> LifecycleRepository<'scope> {
    let (stop, stopped) = mpsc::channel();
    let (finished, completed) = mpsc::channel();
    let observed = Arc::clone(db);
    let driver = threads.spawn(move || {
        stopped.recv_timeout(WAIT).unwrap();
        observed.lock().unwrap().events.push("driver_exited");
        finished.send(()).unwrap();
    });
    LifecycleRepository {
        inner: Repository {
            database: Arc::clone(db),
            before_ack: None,
            force_duplicate_race: false,
        },
        identity: Arc::clone(identity),
        stop: Some(stop),
        completed,
        driver: Some(driver),
        fail_next_close,
    }
}

#[test]
fn drained_actor_returns_original_nonclone_repository_for_explicit_join_before_actor_exit() {
    let db = database();
    let identity = Arc::new(());
    std::thread::scope(|threads| {
        let repository = lifecycle_repository(threads, &db, &identity, false);
        let mut owner = DurableOwner::new(
            repository,
            Engine {
                database: Arc::clone(&db),
            },
            Publication {
                database: Arc::clone(&db),
                fail: false,
            },
            initial(),
            RECEIPT_BYTES,
        )
        .unwrap();
        let (handle, actor) = bounded_inbox();
        let (first, first_receipt) = owned(scope(6));
        let (retry, retry_receipt) = owned(scope(6));
        assert!(handle.try_submit(first).is_ok());
        assert!(handle.try_submit(retry).is_ok());
        handle.stop().unwrap();
        let (late, _) = owned(scope(7));
        assert!(handle.try_submit(late).is_err());
        let observed = Arc::clone(&db);
        let expected_identity = Arc::clone(&identity);
        let actor_thread = threads.spawn(move || {
            let drained = actor.run(&mut owner).unwrap();
            assert_eq!(drained.reduced_inputs, 2);
            assert_eq!(drained.last_sequence, Some(AdmissionSequence(2)));
            let mut repository = owner.into_repository();
            assert!(Arc::ptr_eq(&repository.identity, &expected_identity));
            assert!(repository.driver.is_some());
            assert_eq!(repository.completed.try_recv(), Err(TryRecvError::Empty));
            repository.close().unwrap();
            assert!(repository.driver.is_none());
            observed.lock().unwrap().events.push("actor_closed");
        });
        let first = assert_confirmed(first_receipt.recv_timeout(WAIT).unwrap());
        let retry = assert_confirmed(retry_receipt.recv_timeout(WAIT).unwrap());
        assert_eq!(first, retry);
        actor_thread.join().unwrap();
    });
    let db = db.lock().unwrap();
    assert_eq!(db.engine_calls, 1);
    assert_eq!(db.commit_calls, 1);
    assert_eq!(db.receipts.len(), 1);
    assert_eq!(db.events.iter().filter(|e| **e == "publish").count(), 1);
    assert_eq!(db.events.iter().filter(|e| **e == "wake").count(), 1);
    let retry_lookup = db.events.iter().rposition(|e| *e == "lookup").unwrap();
    let close = db
        .events
        .iter()
        .position(|e| *e == "close_attempt")
        .unwrap();
    let exited = db
        .events
        .iter()
        .position(|e| *e == "driver_exited")
        .unwrap();
    let joined = db
        .events
        .iter()
        .position(|e| *e == "driver_joined")
        .unwrap();
    let actor_closed = db.events.iter().position(|e| *e == "actor_closed").unwrap();
    assert!(retry_lookup < close && close < exited && exited < joined && joined < actor_closed);
}

#[test]
fn unknown_drain_and_failed_close_keep_receipts_unknown_and_owned_driver_retryable() {
    let db = database();
    db.lock().unwrap().failure = Failure::UnknownNotCommitted;
    let identity = Arc::new(());
    std::thread::scope(|threads| {
        let repository = lifecycle_repository(threads, &db, &identity, true);
        let mut owner = DurableOwner::new(
            repository,
            Engine {
                database: Arc::clone(&db),
            },
            Publication {
                database: Arc::clone(&db),
                fail: false,
            },
            initial(),
            RECEIPT_BYTES,
        )
        .unwrap();
        let (handle, actor) = bounded_inbox();
        let (first, first_receipt) = owned(scope(6));
        let (next, next_receipt) = owned(scope(7));
        assert!(handle.try_submit(first).is_ok());
        assert!(handle.try_submit(next).is_ok());
        handle.stop().unwrap();
        let observed = Arc::clone(&db);
        let expected_identity = Arc::clone(&identity);
        let actor_thread = threads.spawn(move || {
            let drained = actor.run(&mut owner).unwrap();
            assert_eq!(drained.reduced_inputs, 2);
            let mut repository = owner.into_repository();
            assert!(Arc::ptr_eq(&repository.identity, &expected_identity));
            assert_eq!(repository.close(), Err(RepositoryError::Unavailable));
            assert!(repository.driver.is_some());
            assert!(repository.stop.is_some());
            assert_eq!(repository.completed.try_recv(), Err(TryRecvError::Empty));
            assert!(!observed.lock().unwrap().events.contains(&"driver_joined"));
            repository.close().unwrap();
            assert!(repository.driver.is_none());
            observed.lock().unwrap().events.push("actor_closed");
        });
        assert_eq!(
            first_receipt.recv_timeout(WAIT).unwrap(),
            SubmissionOutcome::LookupRequired
        );
        assert_eq!(
            next_receipt.recv_timeout(WAIT).unwrap(),
            SubmissionOutcome::LookupRequired
        );
        actor_thread.join().unwrap();
        assert_eq!(first_receipt.try_recv(), Err(TryRecvError::Disconnected));
        assert_eq!(next_receipt.try_recv(), Err(TryRecvError::Disconnected));
    });
    let db = db.lock().unwrap();
    assert_eq!(db.engine_calls, 1);
    assert_eq!(db.commit_calls, 1);
    assert_eq!(db.checkpoint, initial());
    assert!(db.receipts.is_empty());
    assert!(!db.events.contains(&"publish"));
    assert!(!db.events.contains(&"wake"));
    assert_eq!(
        db.events.iter().filter(|e| **e == "close_attempt").count(),
        2
    );
    assert_eq!(
        db.events.iter().filter(|e| **e == "driver_joined").count(),
        1
    );
    let pending = db
        .events
        .iter()
        .position(|e| *e == "close_pending")
        .unwrap();
    let joined = db
        .events
        .iter()
        .position(|e| *e == "driver_joined")
        .unwrap();
    let actor_closed = db.events.iter().position(|e| *e == "actor_closed").unwrap();
    assert!(pending < joined && joined < actor_closed);
}

#[test]
fn exact_recovery_connection_failure_keeps_original_key_and_blocks_other_lookup() {
    let db = database();
    db.lock().unwrap().failure = Failure::LostCommittedAck;
    let mut owner = owner(&db);
    assert_eq!(
        submit(&mut owner, scope(6)),
        SubmissionOutcome::LookupRequired
    );
    assert!(!owner.is_current());
    assert!(
        owner
            .matches_uncertain_retry(&scope(6), &input(&scope(6)))
            .unwrap()
    );
    db.lock().unwrap().events.clear();
    db.lock().unwrap().reconnect_failure = true;
    assert_eq!(
        submit(&mut owner, scope(6)),
        SubmissionOutcome::Refused(RepositoryError::Unavailable)
    );
    assert_eq!(
        submit(&mut owner, scope(7)),
        SubmissionOutcome::LookupRequired
    );
    assert_eq!(owner.checkpoint(), &initial());
    assert_eq!(db.lock().unwrap().events, ["lookup"]);
    assert!(
        owner
            .matches_uncertain_retry(&scope(6), &input(&scope(6)))
            .unwrap()
    );
    assert_eq!(db.lock().unwrap().engine_calls, 1);
    assert_eq!(db.lock().unwrap().commit_calls, 1);
    assert_eq!(db.lock().unwrap().reconnect_calls, 1);
    db.lock().unwrap().reconnect_failure = false;
    db.lock().unwrap().failure = Failure::None;
    let recovered = assert_confirmed(submit(&mut owner, scope(6)));
    assert!(owner.is_current());
    assert_eq!(recovered, db.lock().unwrap().receipts[&operation(6)].1);
    let mut later = scope(7);
    later.basis = owner.checkpoint().basis();
    assert_confirmed(submit(&mut owner, later));
    assert_eq!(db.lock().unwrap().engine_calls, 2);
    assert_eq!(db.lock().unwrap().commit_calls, 2);
}

#[test]
fn valid_new_snapshot_omitting_the_exact_stored_decision_cannot_release_uncertainty() {
    let db = database();
    db.lock().unwrap().failure = Failure::LostCommittedAck;
    let mut owner = owner(&db);
    assert_eq!(
        submit(&mut owner, scope(6)),
        SubmissionOutcome::LookupRequired
    );
    let stored = db.lock().unwrap().receipts[&operation(6)].1.clone();
    db.lock().unwrap().reload_override = Some(proposed(&initial(), &scope(7)));
    assert_eq!(assert_confirmed(submit(&mut owner, scope(6))), stored);
    assert!(!owner.is_current());
    assert_eq!(owner.checkpoint(), &initial());
    db.lock().unwrap().events.clear();
    assert_eq!(
        submit(&mut owner, scope(7)),
        SubmissionOutcome::LookupRequired
    );
    assert_eq!(db.lock().unwrap().events, ["lookup"]);
    assert!(
        owner
            .matches_uncertain_retry(&scope(6), &input(&scope(6)))
            .unwrap()
    );
    db.lock().unwrap().reload_override = None;
    assert_eq!(assert_confirmed(submit(&mut owner, scope(6))), stored);
    assert!(owner.is_current());
    assert_eq!(db.lock().unwrap().engine_calls, 1);
}

#[test]
fn conflicting_other_key_cannot_recover_or_erase_first_uncertainty() {
    let db = database();
    db.lock()
        .unwrap()
        .lookup
        .insert(operation(6), LookupBehavior::InProgress);
    let mut owner = owner(&db);
    assert_eq!(
        submit(&mut owner, scope(6)),
        SubmissionOutcome::LookupRequired
    );
    let _retained = seed_committed(&db, &scope(7));
    let mut conflicting = scope(7);
    conflicting.fingerprint += 1;
    db.lock().unwrap().events.clear();
    assert_eq!(
        submit(&mut owner, conflicting),
        SubmissionOutcome::OperationConflict
    );
    assert!(
        owner
            .matches_uncertain_retry(&scope(6), &input(&scope(6)))
            .unwrap()
    );
    assert_eq!(
        submit(&mut owner, scope(8)),
        SubmissionOutcome::LookupRequired
    );
    let db = db.lock().unwrap();
    assert_eq!(db.events, ["lookup", "lookup"]);
    assert_eq!(db.reconnect_calls, 0);
    assert_eq!(db.engine_calls, 0);
    assert_eq!(db.commit_calls, 0);
    assert_eq!(owner.checkpoint(), &initial());
}

#[test]
fn independently_unauthorized_other_lookup_preserves_first_uncertainty() {
    let db = database();
    db.lock()
        .unwrap()
        .lookup
        .insert(operation(6), LookupBehavior::InProgress);
    let mut owner = owner(&db);
    assert_eq!(
        submit(&mut owner, scope(6)),
        SubmissionOutcome::LookupRequired
    );
    let mut unauthorized = scope(7);
    unauthorized.basis.session = SessionId::from_bytes(&[70; 16]).unwrap();
    db.lock().unwrap().events.clear();
    assert_eq!(
        submit(&mut owner, unauthorized),
        SubmissionOutcome::Refused(RepositoryError::Unauthorized)
    );
    assert!(
        owner
            .matches_uncertain_retry(&scope(6), &input(&scope(6)))
            .unwrap()
    );
    assert_eq!(
        submit(&mut owner, scope(8)),
        SubmissionOutcome::LookupRequired
    );
    let db = db.lock().unwrap();
    assert_eq!(db.events, ["lookup", "lookup"]);
    assert_eq!(db.reconnect_calls, 0);
    assert_eq!(db.engine_calls, 0);
    assert_eq!(db.commit_calls, 0);
    assert!(db.receipts.is_empty());
    assert_eq!(owner.checkpoint(), &initial());
}
