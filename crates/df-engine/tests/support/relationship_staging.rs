use std::cell::Cell;

use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_engine::command_entry::{
    CommandEntryContext, CommandEntryLimits, CommandRejection, decide_registered_command,
};
use df_engine::obligation_fulfillment::{
    AcceptedFulfillmentCause, FulfillmentAction, FulfillmentError, FulfillmentLimits,
    FulfillmentRegistration, FulfillmentSourceOwner, ObligationFulfillmentHandler,
};
use df_engine::relationship_staging::*;
use df_interaction::debts::DebtAction;
use df_interaction::reactions::*;
use df_model::checkpoint::*;
use df_model::commands::CommandLimits;
use df_rules::{DispatchRegistry, HandlerRegistration, RulesCommandInput};
use df_types::OperationId;

use crate::fixture_model as model;

pub fn content(entry: &str) -> ContentReference {
    ContentReference {
        package: model::content().package,
        entry: model::label(entry),
    }
}
pub fn record(value: u8) -> RecordId {
    RecordId::from_bytes(&[value; 16]).unwrap()
}
pub fn fact(value: u8) -> FactId {
    FactId::from_bytes(&[value; 16]).unwrap()
}
pub fn operation(value: u8) -> OperationId {
    OperationId::from_bytes(&[value; 16]).unwrap()
}

pub struct Fixture {
    pub reaction_tail: bool,
    pub registration: FulfillmentRegistration,
    pub entries: Vec<ReactionEntry>,
    pub rules: Vec<RuleReference>,
    pub content: Vec<ContentReference>,
    pub resources: Vec<ResourceConstraint>,
    pub pins: CheckpointPins,
}
impl Default for Fixture {
    fn default() -> Self {
        Self::new()
    }
}
impl Fixture {
    pub fn new() -> Self {
        Self {
            reaction_tail: false,
            registration: FulfillmentRegistration {
                obligation_definition: content("explicit-agreement"),
                cause_definition: content("accepted-delivery"),
                completion_definition: content("agreement-completed"),
                source: model::rule(),
                policy: model::label("compiled-completion-policy"),
            },
            entries: vec![ReactionEntry {
                source: content("authored-reaction"),
                event: content("accepted-delivery"),
                personality: content("authored-personality"),
                motivation: content("authored-motivation"),
                relationship_policy: content("directional-policy"),
                from_state: model::label("reserved"),
                to_state: model::label("receptive"),
            }],
            rules: vec![model::rule()],
            content: [
                "fixture-entry-1",
                "explicit-agreement",
                "accepted-delivery",
                "agreement-completed",
                "agreement-broken",
                "agreement-expired",
                "agreement-cancelled",
                "authored-reaction",
                "authored-personality",
                "authored-motivation",
                "directional-policy",
                "witness-policy",
            ]
            .into_iter()
            .map(content)
            .collect(),
            resources: model::resource_constraints(),
            pins: model::pins(),
        }
    }
    pub fn inventory(&self) -> ReferenceInventory<'_> {
        ReferenceInventory {
            rules: &self.rules,
            content: &self.content,
            resources: &self.resources,
            assets: &[],
        }
    }
    pub fn state(&self) -> GameState {
        let mut state = model::state();
        let mut npc = state.entities[0].clone();
        npc.id = model::entity(5);
        state.entities.push(npc);
        state.obligations.push(Obligation {
            id: record(40),
            obligor: model::entity(4),
            beneficiary: model::entity(5),
            definition: self.registration.obligation_definition.clone(),
            terms: self.registration.obligation_definition.clone(),
            due: None,
            agreement: ObligationAgreement {
                source: self.registration.obligation_definition.clone(),
                fact: fact(29),
                source_policy: model::label("explicit-agreement-policy"),
                at: state.logical_time,
            },
            status: ObligationStatus::Active,
            transition: None,
        });
        state.facts.push(GameFact {
            id: fact(29),
            revision: model::basis().revision,
            operation: operation(29),
            ordinal: 0,
            cause: None,
            audience: AudienceScope::Host,
            value: FactValue::ContentEvent {
                definition: self.registration.obligation_definition.clone(),
                subjects: vec![model::entity(4), model::entity(5)],
            },
        });
        state.decisions.push(AcceptedDecision {
            operation: operation(29),
            revision: model::basis().revision,
            facts: vec![fact(29)],
            draws: vec![],
            effects: vec![],
            source_policy: model::label("explicit-agreement-policy"),
            semantic_output: None,
        });
        state.facts.push(GameFact {
            id: fact(30),
            revision: model::basis().revision,
            operation: operation(30),
            ordinal: 0,
            cause: None,
            audience: AudienceScope::Host,
            value: FactValue::ContentEvent {
                definition: self.registration.cause_definition.clone(),
                subjects: vec![model::entity(4), model::entity(5)],
            },
        });
        state.decisions.push(AcceptedDecision {
            operation: operation(30),
            revision: model::basis().revision,
            facts: vec![fact(30), fact(33)],
            draws: vec![0],
            effects: vec![],
            source_policy: model::label("original-delivery-policy"),
            semantic_output: None,
        });
        // Accepted historical draw and its ordered fact/decision accounting, not a new draw input.
        state.draws.push(ActualDraw {
            operation: operation(30),
            ordinal: 0,
            resolution: ResolutionId::from_bytes(&[34; 16]).unwrap(),
            window: WindowId::from_bytes(&[35; 16]).unwrap(),
            sides: 6,
            value: 4,
            source: model::rule(),
        });
        state.facts.push(GameFact {
            id: fact(33),
            revision: model::basis().revision,
            operation: operation(30),
            ordinal: 1,
            cause: Some(fact(30)),
            audience: AudienceScope::Host,
            value: FactValue::DrawAccepted {
                operation: operation(30),
                ordinal: 0,
            },
        });
        state.relationships = vec![
            Relationship {
                subject: model::entity(5),
                object: model::entity(4),
                policy: content("directional-policy"),
                state: model::label("reserved"),
                trust: RelationshipAxisState {
                    value: model::label("reserved"),
                    provenance: RelationshipAxisProvenance::AuthoredBaseline {
                        source: content("directional-policy"),
                    },
                },
                affection: RelationshipAxisState {
                    value: model::label("reserved"),
                    provenance: RelationshipAxisProvenance::AuthoredBaseline {
                        source: content("directional-policy"),
                    },
                },
                respect: RelationshipAxisState {
                    value: model::label("reserved"),
                    provenance: RelationshipAxisProvenance::AuthoredBaseline {
                        source: content("directional-policy"),
                    },
                },
                fear: RelationshipAxisState {
                    value: model::label("reserved"),
                    provenance: RelationshipAxisProvenance::AuthoredBaseline {
                        source: content("directional-policy"),
                    },
                },
                suspicion: RelationshipAxisState {
                    value: model::label("reserved"),
                    provenance: RelationshipAxisProvenance::AuthoredBaseline {
                        source: content("directional-policy"),
                    },
                },
                debt: RelationshipAxisState {
                    value: model::label("reserved"),
                    provenance: RelationshipAxisProvenance::AuthoredBaseline {
                        source: content("directional-policy"),
                    },
                },
                familiarity: RelationshipAxisState {
                    value: model::label("reserved"),
                    provenance: RelationshipAxisProvenance::AuthoredBaseline {
                        source: content("directional-policy"),
                    },
                },
            },
            Relationship {
                subject: model::entity(4),
                object: model::entity(5),
                policy: content("directional-policy"),
                state: model::label("independent-reverse"),
                trust: RelationshipAxisState {
                    value: model::label("independent-reverse"),
                    provenance: RelationshipAxisProvenance::AuthoredBaseline {
                        source: content("directional-policy"),
                    },
                },
                affection: RelationshipAxisState {
                    value: model::label("independent-reverse"),
                    provenance: RelationshipAxisProvenance::AuthoredBaseline {
                        source: content("directional-policy"),
                    },
                },
                respect: RelationshipAxisState {
                    value: model::label("independent-reverse"),
                    provenance: RelationshipAxisProvenance::AuthoredBaseline {
                        source: content("directional-policy"),
                    },
                },
                fear: RelationshipAxisState {
                    value: model::label("independent-reverse"),
                    provenance: RelationshipAxisProvenance::AuthoredBaseline {
                        source: content("directional-policy"),
                    },
                },
                suspicion: RelationshipAxisState {
                    value: model::label("independent-reverse"),
                    provenance: RelationshipAxisProvenance::AuthoredBaseline {
                        source: content("directional-policy"),
                    },
                },
                debt: RelationshipAxisState {
                    value: model::label("independent-reverse"),
                    provenance: RelationshipAxisProvenance::AuthoredBaseline {
                        source: content("directional-policy"),
                    },
                },
                familiarity: RelationshipAxisState {
                    value: model::label("independent-reverse"),
                    provenance: RelationshipAxisProvenance::AuthoredBaseline {
                        source: content("directional-policy"),
                    },
                },
            },
        ];
        state.continuity.npcs.push(NpcState {
            entity: model::entity(5),
            personality: content("authored-personality"),
            role: content("authored-personality"),
            motivations: vec![content("authored-motivation")],
            goals: vec![],
            needs: vec![],
            fears: vec![],
            known_facts: vec![fact(30)],
            beliefs: vec![],
            secrets: vec![],
        });
        state.continuity.witnesses.push(WitnessRecord {
            id: record(31),
            observer: model::entity(5),
            fact: fact(30),
            perceived_at: state.logical_time,
            source: content("witness-policy"),
        });
        if self.reaction_tail {
            let mut second_entity = state.entities[1].clone();
            second_entity.id = model::entity(6);
            state.entities.push(second_entity);
            let mut second_npc = state.continuity.npcs[0].clone();
            second_npc.entity = model::entity(6);
            state.continuity.npcs.push(second_npc);
            let mut second_relationship = state.relationships[0].clone();
            second_relationship.subject = model::entity(6);
            state.relationships.push(second_relationship);
            let mut second_witness = state.continuity.witnesses[0].clone();
            second_witness.id = record(32);
            second_witness.observer = model::entity(6);
            state.continuity.witnesses.push(second_witness);
        }
        state
    }
    pub fn checkpoint(&self, state: GameState) -> Checkpoint {
        self.try_checkpoint(state).unwrap()
    }
    pub fn try_checkpoint(&self, state: GameState) -> Result<Checkpoint, CheckpointError> {
        Checkpoint::new(
            CHECKPOINT_SCHEMA,
            model::basis(),
            self.pins.clone(),
            state,
            self.inventory(),
            model::limits(),
        )
    }
    pub fn current(&self) -> Checkpoint {
        self.checkpoint(self.state())
    }
    pub fn input(&self, current: &Checkpoint) -> GameInput {
        GameInput::Game(CommandInput {
            basis: current.basis(),
            observed_revision: current.basis().revision,
            operation: operation(50),
            member: model::member(3),
            command: GameCommand::ProposeAction {
                actor: model::entity(4),
                action: self.registration.obligation_definition.clone(),
                targets: vec![model::entity(5)],
                choices: vec![],
            },
        })
    }
    pub fn request(&self, current: &Checkpoint) -> ReactionRequest {
        ReactionRequest {
            expected_basis: current.basis(),
            npc: model::entity(5),
            target: model::entity(4),
            event: fact(30),
            witness: record(31),
        }
    }
    pub fn requests(&self, current: &Checkpoint) -> Vec<ReactionRequest> {
        let mut requests = vec![self.request(current)];
        if self.reaction_tail {
            requests.push(ReactionRequest {
                expected_basis: current.basis(),
                npc: model::entity(6),
                target: model::entity(4),
                event: fact(99),
                witness: record(32),
            });
        }
        requests
    }
    pub fn inner<'a>(
        &'a self,
        owner: &'a SourceOwner,
        current: &Checkpoint,
    ) -> ObligationFulfillmentHandler<'a, SourceOwner> {
        ObligationFulfillmentHandler {
            owner,
            current_basis: current.basis(),
            admitted_pins: &self.pins,
            inventory: self.inventory(),
            registration: &self.registration,
            obligation: record(40),
            action: DebtAction::Fulfill,
            action_definition: &self.registration.completion_definition,
            cause: AcceptedFulfillmentCause {
                fact: fact(30),
                operation: operation(30),
                revision: model::basis().revision,
            },
            action_fact: fact(50),
            limits: FulfillmentLimits {
                maximum_checkpoint_bytes: 1024 * 1024,
                maximum_pass_bytes: 4 * 1024 * 1024,
                maximum_inventory_records: 64,
                checkpoint: model::limits(),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceRefusal {
    Withdrawn,
    Binding,
    Control,
}

/// Exact synthetic native source/admission view. This fixture proves checks, not real source rights.
pub struct SourceOwner {
    pub expected: FulfillmentRegistration,
    pub entries: Vec<ReactionEntry>,
    pub pins: CheckpointPins,
    pub withdrawn: Cell<bool>,
    pub controlled: Cell<bool>,
    pub application_calls: Cell<usize>,
    pub entry_calls: Cell<usize>,
    pub command_calls: Cell<usize>,
    pub withdraw_after_entry: Cell<Option<usize>>,
}
impl SourceOwner {
    pub fn new(fixture: &Fixture) -> Self {
        Self {
            expected: fixture.registration.clone(),
            entries: fixture.entries.clone(),
            pins: fixture.pins.clone(),
            withdrawn: Cell::new(false),
            controlled: Cell::new(true),
            application_calls: Cell::new(0),
            entry_calls: Cell::new(0),
            command_calls: Cell::new(0),
            withdraw_after_entry: Cell::new(None),
        }
    }
    fn validate_control(
        &self,
        current: &Checkpoint,
        command: &CommandInput,
    ) -> Result<(), SourceRefusal> {
        if self.withdrawn.get() {
            return Err(SourceRefusal::Withdrawn);
        }
        if current.pins() != &self.pins {
            return Err(SourceRefusal::Binding);
        }
        if !self.controlled.get()
            || command.member != model::member(3)
            || !current.state().characters.iter().any(|character| {
                character.entity == model::entity(4) && character.owner == command.member
            })
        {
            return Err(SourceRefusal::Control);
        }
        Ok(())
    }
}
impl FulfillmentSourceOwner for SourceOwner {
    type Refusal = SourceRefusal;
    fn admit(
        &self,
        current: &Checkpoint,
        registration: &FulfillmentRegistration,
        action: FulfillmentAction<'_>,
        command: &CommandInput,
        _: &Obligation,
        _: &GameFact,
    ) -> Result<(), Self::Refusal> {
        self.command_calls.set(self.command_calls.get() + 1);
        self.validate_control(current, command)?;
        if registration != &self.expected
            || action.kind != DebtAction::Fulfill
            || action.definition != &registration.completion_definition
        {
            return Err(SourceRefusal::Binding);
        }
        Ok(())
    }
}
impl ReactionSourceOwner for SourceOwner {
    fn validate_entry(
        &self,
        basis: Basis,
        pins: &ContentPins,
        entry: &ReactionEntry,
    ) -> Result<(), ReactionSourceRefusal> {
        let calls = self.entry_calls.get() + 1;
        self.entry_calls.set(calls);
        if self.withdrawn.get()
            || self
                .withdraw_after_entry
                .get()
                .is_some_and(|limit| calls >= limit)
        {
            return Err(ReactionSourceRefusal::AccessDenied);
        }
        if basis != model::basis() || pins != &self.pins.content {
            return Err(ReactionSourceRefusal::StaleAdmission);
        }
        if !self.entries.contains(entry) {
            return Err(ReactionSourceRefusal::NotAdmitted);
        }
        Ok(())
    }
}
impl RelationshipApplicationOwner for SourceOwner {
    type Refusal = SourceRefusal;
    fn admit_application(
        &self,
        current: &Checkpoint,
        command: &CommandInput,
        requests: &[ReactionRequest],
        entries: &[ReactionEntry],
    ) -> Result<(), Self::Refusal> {
        self.application_calls.set(self.application_calls.get() + 1);
        self.validate_control(current, command)?;
        if entries != self.entries
            || requests.iter().any(|request| {
                request.expected_basis != current.basis()
                    || ![model::entity(5), model::entity(6)].contains(&request.npc)
                    || request.target != model::entity(4)
            })
        {
            return Err(SourceRefusal::Binding);
        }
        Ok(())
    }
}

pub fn limits() -> RelationshipStagingLimits {
    RelationshipStagingLimits {
        maximum_reactions: 4,
        maximum_relationships: 16,
        maximum_work: 100_000,
        maximum_inventory_records: 64,
        maximum_checkpoint_bytes: 1024 * 1024,
        maximum_pass_bytes: 8 * 1024 * 1024,
        checkpoint: model::limits(),
        reaction: ReactionLimits {
            maximum_entries: 4,
            maximum_policy_bytes: 64 * 1024,
            maximum_work: 10_000,
            maximum_proposal_bytes: 64 * 1024,
        },
    }
}

pub type Error = RelationshipStagingError<FulfillmentError<SourceRefusal>, SourceRefusal>;
pub fn stage(
    fixture: &Fixture,
    owner: &SourceOwner,
    current: &Checkpoint,
    input: &GameInput,
    requests: &[ReactionRequest],
    bounds: RelationshipStagingLimits,
) -> Result<RelationshipTransition, Error> {
    let inner = fixture.inner(owner, current);
    let handler = RelationshipReactionHandler {
        command_handler: &inner,
        owner,
        current_basis: current.basis(),
        admitted_pins: &fixture.pins,
        inventory: fixture.inventory(),
        requests,
        entries: &fixture.entries,
        limits: bounds,
    };
    handler.stage_with_outcomes(
        RulesCommandInput {
            command: input,
            supplied_draws: &[],
        },
        current,
    )
}

pub fn registered(
    fixture: &Fixture,
    owner: &SourceOwner,
    current: &Checkpoint,
    input: &GameInput,
) -> Result<Checkpoint, CommandRejection<Error>> {
    let inner = fixture.inner(owner, current);
    let requests = fixture.requests(current);
    let handler = RelationshipReactionHandler {
        command_handler: &inner,
        owner,
        current_basis: current.basis(),
        admitted_pins: &fixture.pins,
        inventory: fixture.inventory(),
        requests: &requests,
        entries: &fixture.entries,
        limits: limits(),
    };
    let selector = model::label("actual-completion-with-relationship");
    let catalog_entries = [CatalogEntry::new(
        &fixture.registration.source,
        b"complete synthetic source clause",
    )];
    let catalog = CatalogSnapshot::from_published(
        &fixture.pins.rules.catalog,
        &fixture.pins,
        b"complete synthetic catalog",
        &catalog_entries,
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
    decide_registered_command(
        RulesCommandInput {
            command: input,
            supplied_draws: &[],
        },
        current,
        CommandEntryContext {
            current_basis: current.basis(),
            admitted_pins: &fixture.pins,
            inventory: fixture.inventory(),
            limits: CommandEntryLimits {
                command: CommandLimits {
                    maximum_records: 100,
                    maximum_text_bytes: 256,
                    maximum_retained_bytes: 1024 * 1024,
                },
                maximum_staged_bytes: 1024 * 1024,
            },
        },
        &registry,
        &selector,
        &fixture.registration.source,
    )
}
