#![cfg(not(target_arch = "wasm32"))]

// External native boundary fixture: root compiles this against the actual Session I02 candidate.
// The handler returns supplied canonical checkpoints; it does not reduce game mechanics.
#[path = "support/fixture_model.rs"]
mod fixture_model;
use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_engine::command_entry::*;
use df_model::checkpoint::*;
use df_model::commands::CommandLimits;
use df_observe::OperationContext;
use df_rules::{DispatchRegistry, HandlerRegistration};
use df_session::inbox::{ActorInput, bounded_inbox};
use df_session::submission::*;
use df_types::{OperationId, RecoveryEpoch, RevisionLabel, SessionId};
use fixture_model::*;
use std::cell::RefCell;
use std::mem::size_of;
use std::rc::Rc;

struct Observed {
    durable: Checkpoint,
    receipt: Option<DecisionReceipt>,
    events: Vec<&'static str>,
    fail_commit: bool,
}
struct SuppliedHandler {
    pins: CheckpointPins,
    candidate: Checkpoint,
    observed: Rc<RefCell<Observed>>,
}
impl RulesCommandHandler for SuppliedHandler {
    type Rejection = std::convert::Infallible;
    fn pins(&self) -> &CheckpointPins {
        &self.pins
    }
    fn stage(
        &self,
        input: RulesCommandInput<'_>,
        _: &Checkpoint,
    ) -> Result<Checkpoint, std::convert::Infallible> {
        assert!(input.supplied_draws.is_empty());
        self.observed.borrow_mut().events.push("handler");
        Ok(self.candidate.clone())
    }
}
// Native composition owns the SessionEngine implementation, avoiding an engine->session edge.
struct RegistrySessionEngine<'r, 'h, H> {
    registry: &'r DispatchRegistry<'h, H>,
    pins: &'r CheckpointPins,
    selector: &'r RevisionLabel,
    source: &'r RuleReference,
}
impl<H: RulesCommandHandler> SessionEngine<Scope> for RegistrySessionEngine<'_, '_, H> {
    fn decide(
        &mut self,
        current: &Checkpoint,
        operation: &Scope,
        input: &GameInput,
    ) -> Result<Checkpoint, RepositoryError> {
        operation.validate_input(input)?;
        let rules = [rule()];
        let contents = [content()];
        let resources = resource_constraints();
        decide_registered_command(
            RulesCommandInput {
                command: input,
                supplied_draws: &[],
            },
            current,
            CommandEntryContext {
                current_basis: current.basis(),
                admitted_pins: self.pins,
                inventory: ReferenceInventory {
                    rules: &rules,
                    content: &contents,
                    resources: &resources,
                    assets: &[],
                },
                limits: CommandEntryLimits {
                    command: CommandLimits {
                        maximum_records: 8,
                        maximum_text_bytes: 32,
                        maximum_retained_bytes: 8192,
                    },
                    maximum_staged_bytes: 1024 * 1024,
                },
            },
            self.registry,
            self.selector,
            self.source,
        )
        .map_err(|_| RepositoryError::InvalidCandidate)
    }
    fn validate_recovery(&mut self, checkpoint: &Checkpoint) -> Result<(), RepositoryError> {
        checkpoint
            .validate_resume(checkpoint.basis(), self.pins)
            .map(|_| ())
            .map_err(|_| RepositoryError::InvalidCandidate)
    }
}
// Fixed-width synthetic native identity is test data, not authorization. Exact arrays retain
// every namespace/fingerprint byte without allocating or assuming a Clone operation contract.
#[derive(Eq, PartialEq)]
struct ScopeKey {
    tenant: [u8; 16],
    session: SessionId,
    principal: [u8; 16],
    namespace: [u8; 8],
    recovery_epoch: RecoveryEpoch,
    operation: OperationId,
    fingerprint_version: u32,
    fingerprint: [u8; 32],
}
impl ActorInput for ScopeKey {
    fn retained_heap_bytes(&self) -> Option<usize> {
        Some(0)
    }
}
struct Scope {
    key: ScopeKey,
}
impl ActorInput for Scope {
    fn retained_heap_bytes(&self) -> Option<usize> {
        Some(0)
    }
}
impl OperationScope for Scope {
    type UncertaintyKey = ScopeKey;
    fn capture_uncertainty_key(
        &self,
        maximum_retained_bytes: usize,
    ) -> Result<ScopeKey, RepositoryError> {
        if size_of::<ScopeKey>() > maximum_retained_bytes {
            return Err(RepositoryError::Capacity);
        }
        Ok(ScopeKey {
            tenant: self.key.tenant,
            session: self.key.session,
            principal: self.key.principal,
            namespace: self.key.namespace,
            recovery_epoch: self.key.recovery_epoch,
            operation: self.key.operation,
            fingerprint_version: self.key.fingerprint_version,
            fingerprint: self.key.fingerprint,
        })
    }
    fn session(&self) -> SessionId {
        self.key.session
    }
    fn operation(&self) -> OperationId {
        self.key.operation
    }
    fn is_lookup_only(&self) -> bool {
        false
    }
    fn validate_input(&self, input: &GameInput) -> Result<(), RepositoryError> {
        let GameInput::Game(command) = input else {
            return Err(RepositoryError::InputBinding);
        };
        if command.operation != self.key.operation
            || command.basis.session != self.key.session
            || command.basis.run != basis().run
            || command.basis.revision.epoch() != self.key.recovery_epoch
        {
            return Err(RepositoryError::InputBinding);
        }
        Ok(())
    }
}
fn scope() -> Scope {
    Scope {
        key: ScopeKey {
            tenant: [1; 16],
            session: basis().session,
            principal: [3; 16],
            namespace: *b"fixture1",
            recovery_epoch: basis().revision.epoch(),
            operation: operation(),
            fingerprint_version: 1,
            fingerprint: [20; 32],
        },
    }
}
struct Repository(Rc<RefCell<Observed>>);
impl SessionRepository for Repository {
    type Scope = Scope;
    fn lookup_operation(
        &mut self,
        _: &Scope,
        _: &OperationContext,
    ) -> Result<OperationLookup, RepositoryError> {
        let mut observed = self.0.borrow_mut();
        observed.events.push("lookup");
        Ok(match &observed.receipt {
            Some(receipt) => OperationLookup::Committed(receipt.clone()),
            None => OperationLookup::NotRecorded,
        })
    }
    fn commit_decision(
        &mut self,
        _: &Scope,
        candidate: &Checkpoint,
        expected: Basis,
        _: &OperationContext,
    ) -> Result<CommitOutcome, RepositoryError> {
        let mut observed = self.0.borrow_mut();
        observed.events.push("commit");
        if observed.fail_commit {
            return Err(RepositoryError::Unavailable);
        }
        if observed.durable.basis() != expected {
            return Err(RepositoryError::RevisionConflict);
        }
        let decision = candidate
            .state()
            .decisions
            .iter()
            .find(|d| d.operation == operation())
            .ok_or(RepositoryError::InvalidCandidate)?;
        let receipt = DecisionReceipt::new(candidate.basis(), decision.clone(), 64 * 1024)?;
        observed.durable = candidate.clone();
        observed.receipt = Some(receipt.clone());
        Ok(CommitOutcome::Confirmed(receipt))
    }
    fn load_current(
        &mut self,
        _: &Scope,
        _: &OperationContext,
    ) -> Result<Checkpoint, RepositoryError> {
        Ok(self.0.borrow().durable.clone())
    }
}
struct Publication(Rc<RefCell<Observed>>);
impl PublicationOwner<Scope> for Publication {
    fn publish_committed(
        &mut self,
        _: &Scope,
        checkpoint: &Checkpoint,
    ) -> Result<(), DeliveryError> {
        let mut observed = self.0.borrow_mut();
        assert_eq!(observed.durable, *checkpoint);
        assert!(observed.receipt.is_some());
        observed.events.push("publish");
        Ok(())
    }
    fn wake_committed_intents(&mut self, _: &Scope) -> Result<(), DeliveryError> {
        self.0.borrow_mut().events.push("wake");
        Ok(())
    }
}
fn context() -> OperationContext {
    OperationContext {
        trace_parent: String::new(),
        build: "engine-session-fixture".to_owned(),
    }
}
// One fixture owns registry/catalog bytes, serialized owner and real bounded inbox lifetime.
fn exercise(
    input: GameInput,
    unknown_selector: bool,
    fail_commit: bool,
    retry: bool,
) -> (Vec<SubmissionOutcome>, Checkpoint, Vec<&'static str>) {
    let initial = checkpoint(state()).unwrap();
    let pins = pins();
    let source = rule();
    let selector = label("qualified-test-selector");
    let missing = label("unknown-selector");
    let candidate = accepted_from(state());
    assert_eq!(candidate, accepted());
    let observed = Rc::new(RefCell::new(Observed {
        durable: initial.clone(),
        receipt: None,
        events: vec![],
        fail_commit,
    }));
    let handler = SuppliedHandler {
        pins: pins.clone(),
        candidate,
        observed: Rc::clone(&observed),
    };
    let entries = [CatalogEntry::new(&source, b"source parameters")];
    let catalog = CatalogSnapshot::from_published(
        &pins.rules.catalog,
        &pins,
        b"complete test source",
        &entries,
        CatalogLimits {
            max_complete_bytes: 64,
            max_entries: 4,
            max_item_bytes: 64,
            max_total_item_bytes: 128,
        },
    )
    .unwrap();
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry = DispatchRegistry::from_catalog(catalog, &registrations, 1).unwrap();
    let engine = RegistrySessionEngine {
        registry: &registry,
        pins: &pins,
        selector: if unknown_selector {
            &missing
        } else {
            &selector
        },
        source: &source,
    };
    let mut owner = DurableOwner::new(
        Repository(Rc::clone(&observed)),
        engine,
        Publication(Rc::clone(&observed)),
        initial,
        64 * 1024,
    )
    .unwrap();
    let (sender, actor) = bounded_inbox();
    let (item, receiver) = OwnedInput::new(context(), scope(), input.clone());
    sender
        .try_submit(item)
        .unwrap_or_else(|_| panic!("fixture admission"));
    let retry_receiver = if retry {
        let (item, receiver) = OwnedInput::new(context(), scope(), input);
        sender
            .try_submit(item)
            .unwrap_or_else(|_| panic!("fixture admission"));
        Some(receiver)
    } else {
        None
    };
    sender.stop().unwrap();
    let outcome = actor.run(&mut owner).unwrap();
    assert_eq!(outcome.reduced_inputs, if retry { 2 } else { 1 });
    let mut receipts = vec![receiver.try_recv().unwrap()];
    if let Some(receiver) = retry_receiver {
        receipts.push(receiver.try_recv().unwrap());
    }
    let current = owner.checkpoint().clone();
    let events = observed.borrow().events.clone();
    (receipts, current, events)
}
#[test]
fn canonical_staged_checkpoint_is_committed_before_publication_and_retry_never_restages() {
    let scope = scope();
    let captured = scope
        .capture_uncertainty_key(size_of::<ScopeKey>())
        .unwrap();
    assert!(captured == scope.key);
    assert!(matches!(
        scope.capture_uncertainty_key(size_of::<ScopeKey>() - 1),
        Err(RepositoryError::Capacity)
    ));
    let (receipts, current, events) = exercise(action(), false, false, true);
    let SubmissionOutcome::Confirmed(first) = &receipts[0] else {
        panic!("fixture confirmed")
    };
    let SubmissionOutcome::Confirmed(second) = &receipts[1] else {
        panic!("fixture retained receipt")
    };
    assert_eq!(first, second);
    assert_eq!(current, accepted());
    assert_eq!(
        events,
        vec!["lookup", "handler", "commit", "publish", "wake", "lookup"]
    );
}
#[test]
fn invalid_action_and_missing_registered_source_never_commit_or_publish() {
    let mut invalid = action();
    let GameInput::Game(command) = &mut invalid else {
        panic!("fixture")
    };
    let GameCommand::ProposeAction { action, .. } = &mut command.command else {
        panic!("fixture")
    };
    action.entry = label("unadmitted-action");
    for (input, unknown) in [(invalid, false), (fixture_model::action(), true)] {
        let (receipts, current, events) = exercise(input, unknown, false, false);
        assert_eq!(
            receipts,
            vec![SubmissionOutcome::Refused(
                RepositoryError::InvalidCandidate
            )]
        );
        assert_eq!(current, checkpoint(state()).unwrap());
        assert_eq!(events, vec!["lookup"]);
    }
}
#[test]
fn known_commit_failure_keeps_the_original_checkpoint_and_never_publishes_staged_result() {
    let (receipts, current, events) = exercise(action(), false, true, false);
    assert_eq!(
        receipts,
        vec![SubmissionOutcome::Refused(RepositoryError::Unavailable)]
    );
    assert_eq!(current, checkpoint(state()).unwrap());
    assert_eq!(events, vec!["lookup", "handler", "commit"]);
}
