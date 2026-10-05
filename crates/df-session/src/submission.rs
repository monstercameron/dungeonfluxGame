//! Native session submission: only the durable repository acknowledgement releases a receipt.
//! The inbox is admission, the checkpoint is a cache, and neither confers database authority.
use std::mem::size_of;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};

use df_model::checkpoint::{AcceptedDecision, Basis, Checkpoint, GameInput};
use df_observe::OperationContext;
use df_types::{OperationId, SessionId};

use crate::inbox::{ActorInput, AdmissionSequence, Reducer};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryError {
    Unauthorized,
    InputBinding,
    OperationConflict,
    RetiredNamespace,
    StaleEpoch,
    StaleFence,
    ExpiredOwner,
    RevisionConflict,
    SequenceExhausted,
    InvalidCandidate,
    InvalidReceipt,
    UnresolvedCommit,
    Capacity,
    Unavailable,
}

/// Adapter-owned operation data from authenticated native ingress. Implementing this
/// trait supplies input binding, not authority: repository methods independently
/// verify tenant/principal/namespace/fence/access against their trusted DB boundary.
/// Lookup-only disposition must survive ambiguous submission/restart at that boundary.
pub trait OperationScope: ActorInput {
    /// Consumer-owned exact retained operation identity: native deduplication tuple
    /// (tenant, session, principal, namespace, recovery epoch, operation) AND exact
    /// fingerprint version/value. Refreshed grants/access/owner proof are excluded.
    /// Eq must compare every semantic component, not a lossy hash or OperationId.
    /// This identity is data only and never authorizes repository access.
    type UncertaintyKey: Eq + ActorInput;

    /// Fallibly own that exact identity within the total inline+heap byte bound.
    /// Preflight every allocation, use fallible bounded copying, and charge actual
    /// retained capacities. Unrepresentable/oversized/allocation failures return
    /// Capacity, never a truncated/fallback identity. No Clone contract is assumed.
    fn capture_uncertainty_key(
        &self,
        maximum_retained_bytes: usize,
    ) -> Result<Self::UncertaintyKey, RepositoryError>;

    fn session(&self) -> SessionId;
    fn operation(&self) -> OperationId;
    fn validate_input(&self, input: &GameInput) -> Result<(), RepositoryError>;
    fn is_lookup_only(&self) -> bool;
}

/// Exact committed domain result. Scope authorization and retention belong to the repository.
/// Construction validates binding and owned capacity, never asserts a database commit.
#[derive(Clone, Eq, PartialEq)]
pub struct DecisionReceipt {
    basis: Basis,
    decision: AcceptedDecision,
}

// Canonical decisions may contain private semantic text. Default diagnostics retain
// identity, revision and memory bounds without serializing the decision payload.
impl std::fmt::Debug for DecisionReceipt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DecisionReceipt")
            .field("operation", &self.decision.operation)
            .field("revision", &self.basis.revision)
            .field("retained_bytes", &self.retained_bytes())
            .finish_non_exhaustive()
    }
}

impl DecisionReceipt {
    pub fn new(
        basis: Basis,
        decision: AcceptedDecision,
        maximum_retained_bytes: usize,
    ) -> Result<Self, RepositoryError> {
        if decision.revision != basis.revision {
            return Err(RepositoryError::InvalidReceipt);
        }
        let receipt = Self { basis, decision };
        if maximum_retained_bytes == 0
            || receipt.retained_bytes().ok_or(RepositoryError::Capacity)? > maximum_retained_bytes
        {
            return Err(RepositoryError::Capacity);
        }
        Ok(receipt)
    }

    pub fn basis(&self) -> Basis {
        self.basis
    }
    pub fn decision(&self) -> &AcceptedDecision {
        &self.decision
    }

    pub fn retained_bytes(&self) -> Option<usize> {
        receipt_retained_bytes(&self.decision)
    }

    fn matches_scope<S: OperationScope>(&self, scope: &S) -> bool {
        self.basis.session == scope.session() && self.decision.operation == scope.operation()
    }
}

fn receipt_retained_bytes(decision: &AcceptedDecision) -> Option<usize> {
    size_of::<DecisionReceipt>()
        .checked_add(
            decision
                .facts
                .capacity()
                .checked_mul(size_of::<df_model::checkpoint::FactId>())?,
        )?
        .checked_add(decision.draws.capacity().checked_mul(size_of::<u32>())?)?
        .checked_add(
            decision
                .effects
                .capacity()
                .checked_mul(size_of::<df_model::checkpoint::EffectId>())?,
        )?
        .checked_add(decision.source_policy.retained_heap_bytes())?
        .checked_add(
            decision
                .semantic_output
                .as_ref()
                .map_or(0, String::capacity),
        )
}

#[derive(Debug, Eq, PartialEq)]
pub enum CommitOutcome {
    Confirmed(DecisionReceipt),
    PreviouslyCommitted(DecisionReceipt),
    Indeterminate,
}

#[derive(Debug, Eq, PartialEq)]
pub enum OperationLookup {
    Committed(DecisionReceipt),
    Conflict,
    InProgress,
    ExpiredOrIndeterminate,
    NotRecorded,
}

/// Consumer-owned transaction port, implemented by native PostgreSQL persistence.
/// Calls are bounded by the native runtime's explicit transaction/receipt deadlines.
/// Lookup authenticates and resolves the exact scoped key/fingerprint before current
/// command/run/owner/offer checks. NotRecorded never authorizes an uncertain replay.
/// commit_decision rechecks scope, fingerprint, owner fence, strictly live DB-clock
/// lease and exact expected Basis in ONE transaction with checkpoint, ordered facts,
/// AcceptedDecision and DurableIntent records. Known failures roll back everything;
/// a lost COMMIT response returns Indeterminate, never Unavailable or fabricated success.
/// Confirmed/PreviouslyCommitted mean actual durable acknowledgement, never queue admission.
pub trait SessionRepository {
    type Scope: OperationScope;
    /// Restore a usable owned connection before an EXACT retained uncertain retry.
    /// The session still owns its original checkpoint/key. Native implementations
    /// close/join the displaced driver before replacement and never replay a write.
    /// In-memory/already usable ports need no connection replacement.
    fn recover_connection(&mut self, _context: &OperationContext) -> Result<(), RepositoryError> {
        Ok(())
    }
    fn lookup_operation(
        &mut self,
        operation: &Self::Scope,
        context: &OperationContext,
    ) -> Result<OperationLookup, RepositoryError>;
    fn commit_decision(
        &mut self,
        operation: &Self::Scope,
        checkpoint: &Checkpoint,
        expected: Basis,
        context: &OperationContext,
    ) -> Result<CommitOutcome, RepositoryError>;
    fn load_current(
        &mut self,
        operation: &Self::Scope,
        context: &OperationContext,
    ) -> Result<Checkpoint, RepositoryError>;
}

/// Pure engine authority supplies a complete validated canonical checkpoint, including
/// its AcceptedDecision, facts/draws and DurableIntents. This caller never reduces rules.
pub trait SessionEngine<S: OperationScope> {
    fn decide(
        &mut self,
        current: &Checkpoint,
        operation: &S,
        input: &GameInput,
    ) -> Result<Checkpoint, RepositoryError>;
    fn validate_recovery(&mut self, checkpoint: &Checkpoint) -> Result<(), RepositoryError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryError {
    Unavailable,
    AccessRevoked,
    StaleOwner,
}

/// Native publication revalidates current scope/access/owner before each delivery.
/// Waking intents means reading registered DURABLE jobs, never executing staged bytes.
/// Failed delivery remains pending/recoverable and cannot undo an acknowledged commit.
pub trait PublicationOwner<S: OperationScope> {
    fn publish_committed(
        &mut self,
        operation: &S,
        checkpoint: &Checkpoint,
    ) -> Result<(), DeliveryError>;
    fn wake_committed_intents(&mut self, operation: &S) -> Result<(), DeliveryError>;
}

#[derive(Debug, Eq, PartialEq)]
pub enum SubmissionOutcome {
    Confirmed(DecisionReceipt),
    OperationConflict,
    LookupRequired,
    ExpiredOrIndeterminate,
    Refused(RepositoryError),
}

/// A fresh, uncloneable capacity-one sender per inbox item. Dropping its receiver
/// cancels only receipt delivery: the session still owns admitted durable work.
pub struct OwnedInput<S> {
    context: OperationContext,
    operation: S,
    input: GameInput,
    receipt: SyncSender<SubmissionOutcome>,
}
impl<S> OwnedInput<S> {
    pub fn new(
        context: OperationContext,
        operation: S,
        input: GameInput,
    ) -> (Self, Receiver<SubmissionOutcome>) {
        let (receipt, wait) = sync_channel(1);
        (
            Self {
                context,
                operation,
                input,
                receipt,
            },
            wait,
        )
    }
}
impl<S: OperationScope> ActorInput for OwnedInput<S> {
    fn retained_heap_bytes(&self) -> Option<usize> {
        self.input
            .retained_heap_bytes()?
            .checked_add(self.operation.retained_heap_bytes()?)?
            .checked_add(self.context.trace_parent.capacity())?
            .checked_add(self.context.build.capacity())
    }
}

enum CacheState<K> {
    Current,
    ReloadRequired,
    Uncertain(K),
}

/// One serialization owner connects the accepted inbox to pure reduction and durable
/// submission. No task/thread is spawned; the runtime owns and joins the actor loop.
/// Repository/engine/publication ports remain separate owners with actual domain types.
/// maximum_receipt_bytes bounds each retained receipt AND each full uncertainty key
/// (inline size plus checked heap capacity). At most one unresolved key is retained;
/// capturing the next input temporarily owns a second bounded key. No key is cloned.
pub struct DurableOwner<R: SessionRepository, E, P> {
    repository: R,
    engine: E,
    publication: P,
    checkpoint: Checkpoint,
    cache: CacheState<<R::Scope as OperationScope>::UncertaintyKey>,
    maximum_receipt_bytes: usize,
}
impl<R, E, P> DurableOwner<R, E, P>
where
    R: SessionRepository,
    E: SessionEngine<R::Scope>,
    P: PublicationOwner<R::Scope>,
{
    pub fn new(
        repository: R,
        mut engine: E,
        publication: P,
        checkpoint: Checkpoint,
        maximum_receipt_bytes: usize,
    ) -> Result<Self, RepositoryError> {
        if maximum_receipt_bytes == 0 {
            return Err(RepositoryError::Capacity);
        }
        engine.validate_recovery(&checkpoint)?;
        Ok(Self {
            repository,
            engine,
            publication,
            checkpoint,
            cache: CacheState::Current,
            maximum_receipt_bytes,
        })
    }

    pub fn checkpoint(&self) -> &Checkpoint {
        &self.checkpoint
    }

    /// Whether the cached checkpoint may admit a new decision. A committed receipt
    /// alone does not establish this: uncertain recovery also requires validated reload.
    pub fn is_current(&self) -> bool {
        matches!(self.cache, CacheState::Current)
    }

    /// Distinguishes ambiguous admission from a known stale cache requiring reload.
    /// Only the former restricts recovery to the first exact retained key.
    pub fn has_uncertain_operation(&self) -> bool {
        matches!(self.cache, CacheState::Uncertain(_))
    }

    /// Native ingress can route a fenced retry only when its full retained identity
    /// and bound input match. This check confers no permission: the repository must
    /// independently authenticate the refreshed scope before lookup/reload.
    pub fn matches_uncertain_retry(
        &self,
        operation: &R::Scope,
        input: &GameInput,
    ) -> Result<bool, RepositoryError> {
        operation.validate_input(input)?;
        let key = self.capture_uncertainty_key(operation)?;
        Ok(matches!(&self.cache, CacheState::Uncertain(unresolved) if unresolved == &key))
    }

    /// Consumes this reducer and returns its original owned repository for explicit
    /// native shutdown. The runtime must first drain the inbox, then close/join this
    /// repository on the dedicated actor thread before joining that thread, keeping
    /// its async runtime alive. Handback itself does not drain, close, join, resolve
    /// uncertainty, or certify successful cleanup; the native close result must be
    /// observed while the returned repository remains owned.
    #[must_use = "The native owner must explicitly close and join its returned repository"]
    pub fn into_repository(self) -> R {
        self.repository
    }

    /// Explicit reload after a known CAS/fence change. It cannot clear uncertainty:
    /// that operation must first resolve to an authorized committed receipt by lookup.
    pub fn reload_current(
        &mut self,
        operation: &R::Scope,
        context: &OperationContext,
    ) -> Result<(), RepositoryError> {
        if matches!(&self.cache, CacheState::Uncertain(_)) {
            return Err(RepositoryError::UnresolvedCommit);
        }
        self.repository.recover_connection(context)?;
        self.reload(operation, context, None)
    }

    fn reload(
        &mut self,
        operation: &R::Scope,
        context: &OperationContext,
        receipt: Option<&DecisionReceipt>,
    ) -> Result<(), RepositoryError> {
        let mut span = df_observe::begin(context, "session.reload");
        let result = self.reload_scoped(operation, context, receipt);
        span.finish_unmeasured(if result.is_ok() {
            "reloaded"
        } else {
            "reload_pending"
        });
        result
    }

    fn reload_scoped(
        &mut self,
        operation: &R::Scope,
        context: &OperationContext,
        receipt: Option<&DecisionReceipt>,
    ) -> Result<(), RepositoryError> {
        let checkpoint = self.repository.load_current(operation, context)?;
        if checkpoint.basis().session != operation.session()
            || checkpoint.basis().session != self.checkpoint.basis().session
            || checkpoint.basis().revision < self.checkpoint.basis().revision
            || receipt.is_some_and(|receipt| {
                checkpoint.basis().revision < receipt.basis().revision
                    || !checkpoint.state().decisions.contains(receipt.decision())
            })
        {
            return Err(RepositoryError::InvalidCandidate);
        }
        self.engine.validate_recovery(&checkpoint)?;
        self.checkpoint = checkpoint;
        self.cache = CacheState::Current;
        Ok(())
    }

    fn check_receipt(
        &self,
        receipt: &DecisionReceipt,
        operation: &R::Scope,
    ) -> Result<(), RepositoryError> {
        if !receipt.matches_scope(operation) {
            return Err(RepositoryError::InvalidReceipt);
        }
        if receipt.retained_bytes().ok_or(RepositoryError::Capacity)? > self.maximum_receipt_bytes {
            return Err(RepositoryError::Capacity);
        }
        Ok(())
    }

    fn capture_uncertainty_key(
        &self,
        operation: &R::Scope,
    ) -> Result<<R::Scope as OperationScope>::UncertaintyKey, RepositoryError> {
        let inline = size_of::<<R::Scope as OperationScope>::UncertaintyKey>();
        if inline > self.maximum_receipt_bytes {
            return Err(RepositoryError::Capacity);
        }
        let key = operation.capture_uncertainty_key(self.maximum_receipt_bytes)?;
        let retained = inline
            .checked_add(key.retained_heap_bytes().ok_or(RepositoryError::Capacity)?)
            .ok_or(RepositoryError::Capacity)?;
        if retained > self.maximum_receipt_bytes {
            return Err(RepositoryError::Capacity);
        }
        Ok(key)
    }

    fn fence_uncertain(&mut self, operation: <R::Scope as OperationScope>::UncertaintyKey) {
        // A later lookup cannot replace the first unresolved operation. Only its
        // exact committed receipt plus validated reload can release this fence.
        if !matches!(&self.cache, CacheState::Uncertain(_)) {
            self.cache = CacheState::Uncertain(operation);
        }
    }

    fn submit(&mut self, item: &OwnedInput<R::Scope>) -> SubmissionOutcome {
        let mut span = df_observe::begin(&item.context, "session.submit");
        let result = self.submit_scoped(&item.operation, &item.input, &item.context);
        span.finish_unmeasured(match &result {
            SubmissionOutcome::Confirmed(_) => "confirmed",
            SubmissionOutcome::OperationConflict => "operation_conflict",
            SubmissionOutcome::LookupRequired => "lookup_required",
            SubmissionOutcome::ExpiredOrIndeterminate => "expired_or_indeterminate",
            SubmissionOutcome::Refused(_) => "refused",
        });
        result
    }

    fn submit_scoped(
        &mut self,
        operation: &R::Scope,
        input: &GameInput,
        context: &OperationContext,
    ) -> SubmissionOutcome {
        if let Err(error) = operation.validate_input(input) {
            return SubmissionOutcome::Refused(error);
        }
        // Capture before any repository call. A capacity failure is a known local
        // refusal and cannot erase an earlier unknown or run an untracked operation.
        let key = match self.capture_uncertainty_key(operation) {
            Ok(key) => key,
            Err(error) => return SubmissionOutcome::Refused(error),
        };
        if let CacheState::Uncertain(unresolved) = &self.cache {
            if unresolved != &key {
                return SubmissionOutcome::LookupRequired;
            }
            if let Err(error) = self.repository.recover_connection(context) {
                return SubmissionOutcome::Refused(error);
            }
        }
        match self.repository.lookup_operation(operation, context) {
            Ok(OperationLookup::Committed(receipt)) => {
                if let Err(error) = self.check_receipt(&receipt, operation) {
                    self.fence_uncertain(key);
                    return SubmissionOutcome::Refused(error);
                }
                let can_reload = match &self.cache {
                    CacheState::ReloadRequired => true,
                    CacheState::Uncertain(unresolved) => unresolved == &key,
                    CacheState::Current => false,
                };
                if can_reload {
                    // Preserve uncertainty if the bounded reload fails; the durable
                    // receipt is still truthful and no staged effects are dispatched.
                    let _reload_outcome = self.reload(operation, context, Some(&receipt));
                }
                return SubmissionOutcome::Confirmed(receipt);
            }
            Ok(OperationLookup::Conflict) => return SubmissionOutcome::OperationConflict,
            Ok(OperationLookup::InProgress) => {
                self.fence_uncertain(key);
                return SubmissionOutcome::LookupRequired;
            }
            Ok(OperationLookup::ExpiredOrIndeterminate) => {
                self.fence_uncertain(key);
                return SubmissionOutcome::ExpiredOrIndeterminate;
            }
            Ok(OperationLookup::NotRecorded) => {}
            Err(error) => {
                // Lookup failure carries no confirmed not-committed phase. It
                // must not permit another key to execute around an unknown result.
                self.fence_uncertain(key);
                return SubmissionOutcome::Refused(error);
            }
        }
        if operation.is_lookup_only() {
            self.fence_uncertain(key);
            return SubmissionOutcome::LookupRequired;
        }
        if !matches!(&self.cache, CacheState::Current) {
            return SubmissionOutcome::LookupRequired;
        }
        if operation.session() != self.checkpoint.basis().session {
            return SubmissionOutcome::Refused(RepositoryError::InputBinding);
        }
        if self.checkpoint.basis().revision.next_sequence().is_err() {
            return SubmissionOutcome::Refused(RepositoryError::SequenceExhausted);
        }
        let candidate = match self.engine.decide(&self.checkpoint, operation, input) {
            Ok(candidate) => candidate,
            Err(error) => return SubmissionOutcome::Refused(error),
        };
        if let Err(error) = validate_candidate(&self.checkpoint, &candidate, operation.operation())
        {
            return SubmissionOutcome::Refused(error);
        }
        let Some(decision) = candidate
            .state()
            .decisions
            .iter()
            .find(|d| d.operation == operation.operation())
        else {
            return SubmissionOutcome::Refused(RepositoryError::InvalidCandidate);
        };
        if receipt_retained_bytes(decision).is_none_or(|bytes| bytes > self.maximum_receipt_bytes) {
            return SubmissionOutcome::Refused(RepositoryError::Capacity);
        }
        match self.repository.commit_decision(
            operation,
            &candidate,
            self.checkpoint.basis(),
            context,
        ) {
            Ok(CommitOutcome::Confirmed(receipt)) => {
                if self.check_receipt(&receipt, operation).is_err()
                    || receipt.basis() != candidate.basis()
                    || candidate
                        .state()
                        .decisions
                        .iter()
                        .find(|d| d.operation == operation.operation())
                        != Some(receipt.decision())
                {
                    self.fence_uncertain(key);
                    return SubmissionOutcome::LookupRequired;
                }
                self.checkpoint = candidate;
                let mut span = df_observe::begin(context, "session.publication");
                span.finish_unmeasured(
                    if self
                        .publication
                        .publish_committed(operation, &self.checkpoint)
                        .is_ok()
                    {
                        "published"
                    } else {
                        "committed_publication_pending"
                    },
                );
                let mut span = df_observe::begin(context, "session.intent_wakeup");
                span.finish_unmeasured(
                    if self.publication.wake_committed_intents(operation).is_ok() {
                        "durable_intents_woken"
                    } else {
                        "durable_intents_pending"
                    },
                );
                SubmissionOutcome::Confirmed(receipt)
            }
            Ok(CommitOutcome::PreviouslyCommitted(receipt)) => {
                if let Err(error) = self.check_receipt(&receipt, operation) {
                    self.fence_uncertain(key);
                    return SubmissionOutcome::Refused(error);
                }
                self.cache = CacheState::ReloadRequired;
                let _reload_outcome = self.reload(operation, context, Some(&receipt));
                SubmissionOutcome::Confirmed(receipt)
            }
            Ok(CommitOutcome::Indeterminate) => {
                self.fence_uncertain(key);
                SubmissionOutcome::LookupRequired
            }
            Err(error) => {
                if matches!(
                    error,
                    RepositoryError::StaleEpoch
                        | RepositoryError::StaleFence
                        | RepositoryError::ExpiredOwner
                        | RepositoryError::RevisionConflict
                ) {
                    self.cache = CacheState::ReloadRequired;
                }
                SubmissionOutcome::Refused(error)
            }
        }
    }
}

fn validate_candidate(
    current: &Checkpoint,
    candidate: &Checkpoint,
    operation: OperationId,
) -> Result<(), RepositoryError> {
    let expected = current.basis();
    let next = expected
        .revision
        .next_sequence()
        .map_err(|_| RepositoryError::SequenceExhausted)?;
    let actual = candidate.basis();
    if actual.session != expected.session
        || actual.run != expected.run
        || actual.revision != next
        || current.pins() != candidate.pins()
    {
        return Err(RepositoryError::InvalidCandidate);
    }
    if !candidate
        .state()
        .decisions
        .iter()
        .any(|d| d.operation == operation && d.revision == next)
    {
        return Err(RepositoryError::InvalidCandidate);
    }
    Ok(())
}

impl<R, E, P> Reducer<OwnedInput<R::Scope>> for DurableOwner<R, E, P>
where
    R: SessionRepository,
    E: SessionEngine<R::Scope>,
    P: PublicationOwner<R::Scope>,
{
    fn reduce(&mut self, _admission: AdmissionSequence, item: OwnedInput<R::Scope>) {
        let result = self.submit(&item);
        if let Err(error) = item.receipt.try_send(result) {
            let mut span = df_observe::begin(&item.context, "session.receipt_delivery");
            span.finish_unmeasured(match error {
                std::sync::mpsc::TrySendError::Disconnected(_) => "receipt_wait_dropped",
                std::sync::mpsc::TrySendError::Full(_) => "receipt_channel_contract_failure",
            });
        }
    }
}
