// Native synthetic storm fixture using the existing environmental Engine handler.
// This qualifies retained threat evidence consumption, not authored clock progression,
// PostgreSQL durability, geometry bytes, or production campaign/source authorization.
// The shared fixture also supplies checkpoint/accepted/accepted_from/operation/action helpers.
// This consumer builds the storm checkpoint and stages acceptance through the real handler,
// so those generic supplied-candidate constructors intentionally remain unused here.
#[expect(
    dead_code,
    reason = "shared fixture's generic checkpoint/accepted/accepted_from/operation/action helpers are unused by the source-admitted storm consumer"
)]
#[path = "../../../df-engine/tests/support/fixture_model.rs"]
pub mod model;

use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_engine::command_entry::*;
use df_engine::world_staging::*;
use df_knowledge::perception::{ObserverScope, PerceptionLimits};
use df_model::checkpoint::*;
use df_model::commands::CommandLimits;
use df_narrative::*;
use df_observe::OperationContext;
use df_rules::{DispatchRegistry, HandlerRegistration};
use df_session::inbox::{ActorInput, bounded_inbox};
use df_session::submission::*;
use df_types::{OperationId, RecoveryEpoch, SessionId};
use df_world::{AdmittedEnvironmentalChange, EnvironmentalDeltaLimits};
use std::cell::{Cell, RefCell};
use std::mem::size_of;
use std::rc::Rc;

pub fn fact(value: u8) -> FactId {
    FactId::from_bytes(&[value; 16]).unwrap()
}

fn operation(value: u8) -> OperationId {
    OperationId::from_bytes(&[value; 16]).unwrap()
}

fn content(entry: &str) -> ContentReference {
    ContentReference {
        entry: model::label(entry),
        ..model::content()
    }
}

pub struct Fixture {
    pub pins: CheckpointPins,
    pub registration: EnvironmentalRegistration,
    pub contents: Vec<ContentReference>,
    rules: Vec<RuleReference>,
    resources: Vec<ResourceConstraint>,
    assets: Vec<AssetReference>,
}

impl Fixture {
    pub fn new() -> Self {
        Self {
            pins: model::pins(),
            registration: EnvironmentalRegistration {
                action: content("apply-authored-storm"),
                environment: content("authored-storm"),
                source: model::rule(),
                policy: model::label("compiled-storm-application"),
            },
            contents: vec![
                model::content(),
                content("apply-authored-storm"),
                content("authored-storm"),
            ],
            rules: vec![model::rule()],
            resources: model::resource_constraints(),
            assets: vec![AssetReference {
                key: model::label("exact-tactical-geometry"),
                digest: ContentDigest([6; 32]),
                byte_length: 128,
                kind: AssetKind::TacticalGeometry,
            }],
        }
    }

    pub fn inventory(&self) -> ReferenceInventory<'_> {
        ReferenceInventory {
            rules: &self.rules,
            content: &self.contents,
            resources: &self.resources,
            assets: &self.assets,
        }
    }

    pub fn checkpoint(&self, basis: Basis, state: GameState) -> Checkpoint {
        Checkpoint::new(
            CHECKPOINT_SCHEMA,
            basis,
            self.pins.clone(),
            state,
            self.inventory(),
            model::limits(),
        )
        .unwrap()
    }

    pub fn initial(&self) -> Checkpoint {
        let mut state = model::state();
        let mut location = state.entities[0].clone();
        location.id = model::entity(5);
        state.entities.push(location);
        state.facts = [
            (
                30,
                model::revision(2, 7),
                AudienceScope::Shared,
                model::content(),
            ),
            (
                31,
                model::basis().revision,
                AudienceScope::Members(vec![model::member(3)]),
                self.registration.environment.clone(),
            ),
            (
                32,
                model::basis().revision,
                AudienceScope::Shared,
                self.registration.environment.clone(),
            ),
        ]
        .into_iter()
        .map(|(id, revision, audience, definition)| GameFact {
            id: fact(id),
            revision,
            operation: operation(id),
            ordinal: 0,
            cause: if id == 30 { None } else { Some(fact(30)) },
            audience,
            value: FactValue::ContentEvent {
                definition,
                subjects: vec![model::entity(5)],
            },
        })
        .collect();
        state.decisions = state
            .facts
            .iter()
            .map(|cause| AcceptedDecision {
                operation: cause.operation,
                revision: cause.revision,
                facts: vec![cause.id],
                draws: vec![],
                effects: vec![],
                source_policy: model::label("accepted-storm-source"),
                semantic_output: None,
            })
            .collect();
        state.threats.push(ThreatClock {
            id: RecordId::from_bytes(&[40; 16]).unwrap(),
            definition: self.registration.environment.clone(),
            progress: 2,
            capacity: 10,
        });
        state.continuity.environment.push(EnvironmentalState {
            location: model::entity(5),
            definition: model::content(),
            change_facts: vec![fact(30)],
        });
        state.continuity.canonical_packs.push(CanonicalPack {
            revision: model::label("geometry-pack"),
            digest: ContentDigest([7; 32]),
            bible: VisualBible {
                revision: model::label("geometry-bible"),
                definition: model::content(),
                palette: vec![],
                style: String::new(),
                references: vec![],
            },
            identities: vec![],
        });
        state.continuity.scenes.push(SceneIdentityRevision {
            scene: model::entity(5),
            revision: model::label("current-scene"),
            source_facts: vec![fact(30)],
            geometry: self.assets[0].clone(),
            canonical_pack: model::label("geometry-pack"),
        });
        self.checkpoint(model::basis(), state)
    }

    pub fn input(&self, basis: Basis) -> GameInput {
        GameInput::Game(CommandInput {
            basis,
            observed_revision: basis.revision,
            operation: operation(50),
            member: model::member(3),
            command: GameCommand::ProposeAction {
                actor: model::entity(4),
                action: self.registration.action.clone(),
                targets: vec![model::entity(5)],
                choices: vec![],
            },
        })
    }

    pub fn request<'a>(
        &'a self,
        current: &'a Checkpoint,
        mappings: Option<&'a [AdmittedThreatEvidence<'a>]>,
        observer: ObserverScope,
    ) -> ThreatRelevanceRequest<'a> {
        ThreatRelevanceRequest {
            expected_basis: current.basis(),
            admitted_pins: &self.pins,
            observer,
            policy: &self.contents[0],
            expected_policy: &self.contents[0],
            admitted_content: &self.contents,
            mappings,
        }
    }
}

pub fn limits() -> ThreatRelevanceLimits {
    ThreatRelevanceLimits {
        maximum_checkpoint_bytes: 1024 * 1024,
        maximum_candidates: 8,
        maximum_evidence_records: 8,
        maximum_clock_records: 8,
        maximum_content_records: 8,
        maximum_reference_bytes: 256,
        maximum_work: 4096,
        maximum_output_bytes: 4096,
        perception: PerceptionLimits {
            maximum_scan_records: 32,
            maximum_member_comparisons: 32,
            maximum_selected_facts: 16,
        },
    }
}

pub fn selected(
    fixture: &Fixture,
    current: &Checkpoint,
    observer: ObserverScope,
) -> Result<Vec<FactId>, ThreatRelevanceError> {
    let private = [fact(31)];
    let shared = [fact(32)];
    let mappings = [
        AdmittedThreatEvidence {
            expected: &current.state().threats[0],
            evidence: &private,
        },
        AdmittedThreatEvidence {
            expected: &current.state().threats[0],
            evidence: &shared,
        },
    ];
    select_threat_relevance(
        current,
        fixture.request(current, Some(&mappings), observer),
        limits(),
    )
    .map(|selection| selection.facts().iter().map(|fact| fact.id).collect())
}

struct SourceOwner<'a> {
    fixture: &'a Fixture,
    admitted: bool,
    calls: &'a Cell<usize>,
}
impl EnvironmentalSourceOwner for SourceOwner<'_> {
    type Refusal = RepositoryError;
    fn admit(
        &self,
        current: &Checkpoint,
        registration: &EnvironmentalRegistration,
        command: &CommandInput,
        changes: &[AdmittedEnvironmentalChange<'_>],
    ) -> Result<(), Self::Refusal> {
        self.calls.set(self.calls.get() + 1);
        if !self.admitted {
            return Err(RepositoryError::Unauthorized);
        }
        if registration != &self.fixture.registration
            || current.pins() != &self.fixture.pins
            || command.member != model::member(3)
        {
            return Err(RepositoryError::InputBinding);
        }
        let GameCommand::ProposeAction { actor, .. } = &command.command else {
            return Err(RepositoryError::InputBinding);
        };
        if !current
            .state()
            .characters
            .iter()
            .any(|character| character.entity == *actor && character.owner == command.member)
        {
            return Err(RepositoryError::Unauthorized);
        }
        for change in changes {
            if change.expected.location != model::entity(5) {
                return Err(RepositoryError::InvalidCandidate);
            }
            for id in change
                .replacement
                .change_facts
                .iter()
                .skip(change.expected.change_facts.len())
            {
                let cause = current
                    .state()
                    .facts
                    .iter()
                    .find(|cause| cause.id == *id)
                    .ok_or(RepositoryError::InvalidCandidate)?;
                let audience = if *id == fact(31) {
                    AudienceScope::Members(vec![model::member(3)])
                } else if *id == fact(32) {
                    AudienceScope::Shared
                } else {
                    return Err(RepositoryError::InvalidCandidate);
                };
                if cause.audience != audience
                    || !current.state().decisions.iter().any(|decision| {
                        decision.operation == cause.operation
                            && decision.revision == cause.revision
                            && decision.facts.contains(id)
                            && decision.source_policy == model::label("accepted-storm-source")
                    })
                {
                    return Err(RepositoryError::Unauthorized);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Eq, PartialEq)]
struct ScopeKey {
    tenant: [u8; 16],
    session: SessionId,
    principal: [u8; 16],
    namespace: [u8; 8],
    epoch: RecoveryEpoch,
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
    fn capture_uncertainty_key(&self, maximum: usize) -> Result<ScopeKey, RepositoryError> {
        if size_of::<ScopeKey>() > maximum {
            return Err(RepositoryError::Capacity);
        }
        Ok(self.key.clone())
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
        if command.basis.session != self.session()
            || command.basis.run != model::basis().run
            || command.basis.revision.epoch() != self.key.epoch
            || command.operation != self.operation()
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
            session: model::basis().session,
            principal: [3; 16],
            namespace: *b"fixture1",
            epoch: model::basis().revision.epoch(),
            operation: operation(50),
            fingerprint_version: 1,
            fingerprint: [20; 32],
        },
    }
}

struct Engine<'a, 'h, H> {
    fixture: &'a Fixture,
    registry: &'a DispatchRegistry<'h, H>,
    selector: &'a df_types::RevisionLabel,
}
impl<H: RulesCommandHandler> SessionEngine<Scope> for Engine<'_, '_, H> {
    fn decide(
        &mut self,
        current: &Checkpoint,
        operation: &Scope,
        input: &GameInput,
    ) -> Result<Checkpoint, RepositoryError> {
        operation.validate_input(input)?;
        decide_registered_command(
            RulesCommandInput {
                command: input,
                supplied_draws: &[],
            },
            current,
            CommandEntryContext {
                current_basis: current.basis(),
                admitted_pins: &self.fixture.pins,
                inventory: self.fixture.inventory(),
                limits: CommandEntryLimits {
                    command: CommandLimits {
                        maximum_records: 100,
                        maximum_text_bytes: 256,
                        maximum_retained_bytes: 1024 * 1024,
                    },
                    maximum_staged_bytes: 1024 * 1024,
                },
            },
            self.registry,
            self.selector,
            &self.fixture.registration.source,
        )
        .map_err(|_| RepositoryError::InvalidCandidate)
    }
    fn validate_recovery(&mut self, current: &Checkpoint) -> Result<(), RepositoryError> {
        current
            .validate_resume(current.basis(), &self.fixture.pins)
            .map(|_| ())
            .map_err(|_| RepositoryError::InvalidCandidate)
    }
}

pub struct Observed {
    pub durable: Checkpoint,
    pub events: Vec<&'static str>,
    pub shared: Vec<Vec<FactId>>,
    pub private: Vec<Vec<FactId>>,
    receipt: Option<DecisionReceipt>,
    fail_commit: bool,
    access: bool,
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
        if !observed.access {
            return Err(RepositoryError::Unauthorized);
        }
        Ok(match &observed.receipt {
            Some(receipt) => OperationLookup::Committed(receipt.clone()),
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
        let mut observed = self.0.borrow_mut();
        observed.events.push("commit");
        if !observed.access {
            return Err(RepositoryError::Unauthorized);
        }
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
            .find(|d| d.operation == scope.operation())
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
        let mut observed = self.0.borrow_mut();
        observed.events.push("reload");
        if !observed.access {
            return Err(RepositoryError::Unauthorized);
        }
        Ok(observed.durable.clone())
    }
}

struct Publication<'a> {
    fixture: &'a Fixture,
    observed: Rc<RefCell<Observed>>,
}
impl PublicationOwner<Scope> for Publication<'_> {
    fn publish_committed(&mut self, _: &Scope, current: &Checkpoint) -> Result<(), DeliveryError> {
        let mut observed = self.observed.borrow_mut();
        if !observed.access {
            return Err(DeliveryError::AccessRevoked);
        }
        assert_eq!(observed.durable, *current);
        assert!(observed.receipt.is_some());
        let shared = selected(self.fixture, current, ObserverScope::Shared)
            .map_err(|_| DeliveryError::Unavailable)?;
        let private = selected(
            self.fixture,
            current,
            ObserverScope::Member(model::member(3)),
        )
        .map_err(|_| DeliveryError::Unavailable)?;
        observed.shared.push(shared);
        observed.private.push(private);
        observed.events.push("publish-threat-evidence");
        Ok(())
    }
    fn wake_committed_intents(&mut self, _: &Scope) -> Result<(), DeliveryError> {
        self.observed.borrow_mut().events.push("wake");
        Ok(())
    }
}

#[derive(Clone, Copy, Default)]
pub struct Options {
    pub refuse_source: bool,
    pub missing_producer: bool,
    pub missing_selector: bool,
    pub fail_commit: bool,
    pub no_changes: bool,
    pub retry: bool,
    pub revoke_access: bool,
    pub changed_recovery_pins: bool,
}
pub struct ResultState {
    pub outcomes: Vec<SubmissionOutcome>,
    pub current: Checkpoint,
    pub observed: Rc<RefCell<Observed>>,
    pub source_calls: usize,
    pub recovered: Option<Checkpoint>,
    pub recovered_shared: Vec<FactId>,
    pub recovery_error: Option<RepositoryError>,
}

// Repository is a deterministic transaction fixture. DurableOwner and publication order are
// real df-session interfaces; successful port acknowledgement is not evidence of PostgreSQL.
pub fn exercise(fixture: &Fixture, initial: &Checkpoint, options: Options) -> ResultState {
    let calls = Cell::new(0);
    let source_owner = SourceOwner {
        fixture,
        admitted: !options.refuse_source,
        calls: &calls,
    };
    let replacement = EnvironmentalState {
        location: model::entity(5),
        definition: fixture.registration.environment.clone(),
        change_facts: vec![fact(30), fact(31), fact(32)],
    };
    let changes = [AdmittedEnvironmentalChange {
        expected: &initial.state().continuity.environment[0],
        replacement: &replacement,
        geometry: &initial.state().continuity.scenes[0],
    }];
    let handler = EnvironmentalChangeHandler {
        owner: &source_owner,
        current_basis: initial.basis(),
        admitted_pins: &fixture.pins,
        inventory: fixture.inventory(),
        registration: &fixture.registration,
        changes: if options.missing_producer {
            None
        } else if options.no_changes {
            Some(&[])
        } else {
            Some(&changes)
        },
        limits: WorldStagingLimits {
            maximum_checkpoint_bytes: 1024 * 1024,
            maximum_pass_bytes: 4 * 1024 * 1024,
            maximum_inventory_records: 32,
            checkpoint: model::limits(),
            world: EnvironmentalDeltaLimits {
                input_records: 256,
                changes: 4,
                output_bytes: 16 * 1024,
            },
        },
    };
    let selector = model::label("compiled-environment-application");
    let missing = model::label("unknown-selector");
    let entries = [CatalogEntry::new(
        &fixture.registration.source,
        b"exact synthetic environmental source",
    )];
    let catalog = CatalogSnapshot::from_published(
        &fixture.pins.rules.catalog,
        &fixture.pins,
        b"complete synthetic catalog",
        &entries,
        CatalogLimits {
            max_complete_bytes: 64,
            max_entries: 1,
            max_item_bytes: 64,
            max_total_item_bytes: 64,
        },
    )
    .unwrap();
    let registrations = [HandlerRegistration::new(
        &selector,
        &fixture.registration.source,
        &handler,
    )];
    let registry = DispatchRegistry::from_catalog(catalog, &registrations, 1).unwrap();
    let observed = Rc::new(RefCell::new(Observed {
        durable: initial.clone(),
        events: vec![],
        shared: vec![],
        private: vec![],
        receipt: None,
        fail_commit: options.fail_commit,
        access: !options.revoke_access,
    }));
    let make_engine = || Engine {
        fixture,
        registry: &registry,
        selector: if options.missing_selector {
            &missing
        } else {
            &selector
        },
    };
    let make_publication = || Publication {
        fixture,
        observed: Rc::clone(&observed),
    };
    let mut owner = DurableOwner::new(
        Repository(Rc::clone(&observed)),
        make_engine(),
        make_publication(),
        initial.clone(),
        64 * 1024,
    )
    .unwrap();
    let context = || OperationContext {
        trace_parent: String::new(),
        build: "engine-threat-fixture".to_owned(),
    };
    let (sender, actor) = bounded_inbox();
    let mut receivers = vec![];
    for _ in 0..if options.retry { 2 } else { 1 } {
        let mut input = fixture.input(initial.basis());
        if options.no_changes {
            let GameInput::Game(command) = &mut input else {
                unreachable!()
            };
            let GameCommand::ProposeAction { targets, .. } = &mut command.command else {
                unreachable!()
            };
            targets.clear();
        }
        let (item, receiver) = OwnedInput::new(context(), scope(), input);
        sender
            .try_submit(item)
            .unwrap_or_else(|_| panic!("fixture inbox admission"));
        receivers.push(receiver);
    }
    sender.stop().unwrap();
    actor.run(&mut owner).unwrap();
    let outcomes = receivers
        .into_iter()
        .map(|receiver| receiver.try_recv().unwrap())
        .collect();
    let current = owner.checkpoint().clone();
    let mut recovered = None;
    let mut recovered_shared = vec![];
    let mut recovery_error = None;
    if observed.borrow().receipt.is_some() {
        if options.changed_recovery_pins {
            let durable = observed.borrow().durable.clone();
            let mut pins = durable.pins().clone();
            pins.build = df_types::BuildIdentity::new(
                Some("changed-source"),
                Some("fixture-native-1"),
                Some("fixture-wasm-1"),
                Some("fixture-config-1"),
                Some("fixture-content-1"),
            )
            .unwrap();
            observed.borrow_mut().durable = Checkpoint::new(
                durable.schema(),
                durable.basis(),
                pins,
                durable.state().clone(),
                fixture.inventory(),
                model::limits(),
            )
            .unwrap();
        }
        // Reconstruct the real owner around its stale initial cache, then explicitly reload.
        let mut restarted = DurableOwner::new(
            Repository(Rc::clone(&observed)),
            make_engine(),
            make_publication(),
            initial.clone(),
            64 * 1024,
        )
        .unwrap();
        match restarted.reload_current(&scope(), &context()) {
            Ok(()) => {
                assert!(restarted.is_current());
                recovered_shared =
                    selected(fixture, restarted.checkpoint(), ObserverScope::Shared).unwrap();
                recovered = Some(restarted.checkpoint().clone());
            }
            Err(error) => {
                assert_eq!(restarted.checkpoint(), initial);
                recovery_error = Some(error);
            }
        }
    }
    ResultState {
        outcomes,
        current,
        observed,
        source_calls: calls.get(),
        recovered,
        recovered_shared,
        recovery_error,
    }
}
