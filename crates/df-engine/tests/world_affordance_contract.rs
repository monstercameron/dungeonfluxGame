//! World candidates retain exact provenance through Intent and the actual registered rules port.
//! Synthetic source bytes are structural fixtures, never source rights or mechanical authority.
#[path = "support/fixture_model.rs"]
pub mod fixture_model;

use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_engine::command_entry::{
    CommandEntryContext, CommandEntryLimits, CommandRejection, decide_registered_transition,
};
use df_engine::world_staging::{
    EnvironmentalChangeHandler, EnvironmentalRegistration, EnvironmentalSourceOwner,
    WorldStagingError, WorldStagingLimits,
};
use df_intent::targets::{TargetResolutionLimits, resolve_target};
use df_model::affordance::{Affordance, AffordanceSet, TargetResolution, TargetSelection};
use df_model::checkpoint::*;
use df_model::commands::CommandLimits;
use df_model::transition::TransitionResult;
use df_rules::preconditions::{
    CurrentRuleContext, PreconditionLimits, PreconditionedCommandHandler, PreconditionedRejection,
    RuleDependency, RulePreconditions,
};
use df_rules::{
    DispatchError, DispatchRegistry, HandlerRegistration, InvocationError, RulesCommandInput,
};
use df_types::RevisionLabel;
use df_world::affordance::{
    AffordanceLookupError, AffordanceLookupLimits, AffordanceQuery, lookup_affordances,
};
use df_world::{AdmittedEnvironmentalChange, EnvironmentalDeltaLimits};
use fixture_model as fixture;
use std::cell::Cell;

const SOURCE_BYTES: &[u8] = b"opaque indexed fixture clause; no executable physics";

struct Inputs {
    pins: CheckpointPins,
    rules: Vec<RuleReference>,
    contents: Vec<ContentReference>,
    resources: Vec<ResourceConstraint>,
    registration: EnvironmentalRegistration,
}

impl Inputs {
    fn new() -> Self {
        Self {
            pins: fixture::pins(),
            rules: vec![fixture::rule()],
            contents: vec![fixture::content()],
            resources: fixture::resource_constraints(),
            registration: EnvironmentalRegistration {
                action: fixture::content(),
                environment: fixture::content(),
                source: fixture::rule(),
                policy: fixture::label("explicit-unqualified-source-policy"),
            },
        }
    }

    fn inventory(&self) -> ReferenceInventory<'_> {
        ReferenceInventory {
            rules: &self.rules,
            content: &self.contents,
            resources: &self.resources,
            assets: &[],
        }
    }

    fn checkpoint(&self, state: GameState) -> Checkpoint {
        Checkpoint::new(
            CHECKPOINT_SCHEMA,
            fixture::basis(),
            self.pins.clone(),
            state,
            self.inventory(),
            fixture::limits(),
        )
        .unwrap()
    }

    fn current(&self) -> Checkpoint {
        let mut state = fixture::state();
        for id in [5, 6] {
            state.entities.push(WorldEntity {
                id: fixture::entity(id),
                definition: fixture::content(),
                location: None,
                position: Some(Position { x: 1, y: 1, z: 0 }),
                identity_revision: fixture::label("opaque-current-target"),
            });
        }
        self.checkpoint(state)
    }
}

fn lookup_limits() -> AffordanceLookupLimits {
    AffordanceLookupLimits {
        input_records: 64,
        candidates: 8,
        output_bytes: 16384,
    }
}

fn target_limits() -> TargetResolutionLimits {
    TargetResolutionLimits {
        input_records: 64,
        candidates: 8,
        output_bytes: 16384,
    }
}

fn command_limits() -> CommandLimits {
    CommandLimits {
        maximum_records: 64,
        maximum_text_bytes: 256,
        maximum_retained_bytes: 16384,
    }
}

fn rows(current: &Checkpoint) -> Vec<Affordance> {
    current
        .state()
        .entities
        .iter()
        .filter(|target| target.id != fixture::entity(4))
        .map(|target| Affordance {
            actor: fixture::entity(4),
            action: fixture::content(),
            source: fixture::rule(),
            target: Some(target.clone()),
        })
        .collect()
}

fn lookup<'a>(
    inputs: &Inputs,
    current: &'a Checkpoint,
    rows: &[Affordance],
) -> Result<AffordanceSet<'a>, AffordanceLookupError> {
    lookup_affordances(
        current,
        AffordanceQuery {
            basis: current.basis(),
            pins: &inputs.pins,
            actor: fixture::entity(4),
            action: &inputs.registration.action,
        },
        rows,
        inputs.inventory(),
        lookup_limits(),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceRefusal {
    Unqualified,
}

struct RefusingSourceOwner {
    calls: Cell<usize>,
}

impl EnvironmentalSourceOwner for RefusingSourceOwner {
    type Refusal = SourceRefusal;
    fn admit(
        &self,
        _: &Checkpoint,
        _: &EnvironmentalRegistration,
        _: &CommandInput,
        _: &[AdmittedEnvironmentalChange<'_>],
    ) -> Result<(), Self::Refusal> {
        self.calls.set(self.calls.get() + 1);
        Err(SourceRefusal::Unqualified)
    }
}

type Wrapped<'a> =
    PreconditionedCommandHandler<'a, EnvironmentalChangeHandler<'a, RefusingSourceOwner>>;

fn with_registry<T>(
    inputs: &Inputs,
    current: &Checkpoint,
    owner: &RefusingSourceOwner,
    producer_present: bool,
    inspect: impl FnOnce(&DispatchRegistry<'_, Wrapped<'_>>, &Wrapped<'_>, &RevisionLabel) -> T,
) -> T {
    let handler = EnvironmentalChangeHandler {
        owner,
        current_basis: current.basis(),
        admitted_pins: &inputs.pins,
        inventory: inputs.inventory(),
        registration: &inputs.registration,
        changes: if producer_present { Some(&[]) } else { None },
        limits: WorldStagingLimits {
            maximum_checkpoint_bytes: 1024 * 1024,
            maximum_pass_bytes: 4 * 1024 * 1024,
            maximum_inventory_records: 16,
            checkpoint: fixture::limits(),
            world: EnvironmentalDeltaLimits {
                input_records: 64,
                changes: 4,
                output_bytes: 16384,
            },
        },
    };
    let sources = [inputs.registration.source.clone()];
    let dependencies = [
        RuleDependency::Entity(fixture::entity(4)),
        RuleDependency::PendingResolutions,
    ];
    let guarded = PreconditionedCommandHandler::new(
        &handler,
        &inputs.registration.source,
        CurrentRuleContext {
            checkpoint: current,
            basis: current.basis(),
            pins: &inputs.pins,
            inventory: inputs.inventory(),
            command_limits: command_limits(),
        },
        RulePreconditions {
            prepared: current,
            sources: &sources,
            dependencies: &dependencies,
        },
        PreconditionLimits {
            maximum_dependencies: 8,
            maximum_comparisons: 1000000,
            maximum_checkpoint_bytes: 1024 * 1024,
        },
    );
    let selector = fixture::label("compiled-current-environment-clause");
    let entries = [CatalogEntry::new(&inputs.registration.source, SOURCE_BYTES)];
    let catalog = CatalogSnapshot::from_published(
        &inputs.pins.rules.catalog,
        &inputs.pins,
        b"complete synthetic publication",
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
        &inputs.registration.source,
        &guarded,
    )];
    let registry = DispatchRegistry::from_catalog(catalog, &registrations, 1).unwrap();
    inspect(&registry, &guarded, &selector)
}

#[test]
fn current_world_target_selects_exact_registered_provenance_without_mechanics() {
    let inputs = Inputs::new();
    let current = inputs.current();
    let before = current.clone();
    let set = lookup(&inputs, &current, &rows(&current)).unwrap();
    let TargetResolution::Selected(selected) = resolve_target(
        &current,
        &set,
        TargetSelection::Explicit(fixture::entity(5)),
        inputs.inventory(),
        target_limits(),
    )
    .unwrap() else {
        panic!("exact target fixture")
    };
    let owner = RefusingSourceOwner {
        calls: Cell::new(0),
    };
    with_registry(
        &inputs,
        &current,
        &owner,
        false,
        |registry, guarded, selector| {
            let (handler, provenance) = registry
                .select_with_provenance(&inputs.pins, selector, &selected.source)
                .unwrap();
            assert!(std::ptr::eq(handler, guarded));
            assert_eq!(provenance.source(), &selected.source);
            assert_eq!(provenance.selector(), selector);
            assert_eq!(provenance.rules_pins(), &inputs.pins.rules);
            assert_eq!(provenance.content_pins(), &inputs.pins.content);
            assert_eq!(provenance.build(), &inputs.pins.build);
            assert_eq!(provenance.source_bytes(), SOURCE_BYTES);
            assert_eq!(provenance.source_bytes().as_ptr(), SOURCE_BYTES.as_ptr());
        },
    );
    assert_eq!(set.basis, current.basis());
    assert!(std::ptr::eq(set.pins, current.pins()));
    assert_eq!(owner.calls.get(), 0);
    assert_eq!(current, before);
}

#[test]
fn empty_unknown_and_unsupported_source_never_create_a_rule_result() {
    let inputs = Inputs::new();
    let current = inputs.current();
    let before = current.clone();
    let empty = lookup(&inputs, &current, &[]).unwrap();
    assert_eq!(
        resolve_target(
            &current,
            &empty,
            TargetSelection::Unspecified,
            inputs.inventory(),
            target_limits()
        ),
        Ok(TargetResolution::NeedsRuling)
    );
    let mut unknown = rows(&current);
    unknown[0].source.clause = fixture::label("unadmitted-physical-guess");
    assert_eq!(
        lookup(&inputs, &current, &unknown),
        Err(AffordanceLookupError::UnknownSource)
    );
    let owner = RefusingSourceOwner {
        calls: Cell::new(0),
    };
    with_registry(&inputs, &current, &owner, false, |registry, _, selector| {
        assert!(matches!(
            registry.select_with_provenance(&inputs.pins, selector, &unknown[0].source),
            Err(DispatchError::UnsupportedSource)
        ));
        assert!(matches!(
            registry.select_with_provenance(
                &inputs.pins,
                &fixture::label("unknown-compiled-handler"),
                &inputs.registration.source
            ),
            Err(DispatchError::UnknownHandler)
        ));
    });
    assert_eq!(owner.calls.get(), 0);
    assert_eq!(current, before);
}

#[test]
fn ambiguous_targets_require_identity_and_conflicting_sources_refuse() {
    let mut inputs = Inputs::new();
    let current = inputs.current();
    let before = current.clone();
    let mut entries = rows(&current);
    entries.reverse();
    let set = lookup(&inputs, &current, &entries).unwrap();
    assert_eq!(
        resolve_target(
            &current,
            &set,
            TargetSelection::Unspecified,
            inputs.inventory(),
            target_limits()
        ),
        Ok(TargetResolution::NeedsClarification(vec![
            fixture::entity(5),
            fixture::entity(6)
        ]))
    );
    assert_eq!(
        resolve_target(
            &current,
            &set,
            TargetSelection::Explicit(fixture::entity(99)),
            inputs.inventory(),
            target_limits()
        ),
        Ok(TargetResolution::UnsupportedTarget)
    );
    let mut conflicting = entries[0].clone();
    conflicting.source.clause = fixture::label("different-source-clause");
    inputs.rules.push(conflicting.source.clone());
    entries.push(conflicting);
    assert_eq!(
        lookup(&inputs, &current, &entries),
        Err(AffordanceLookupError::AmbiguousSource)
    );
    let mut untargeted = entries[0].clone();
    untargeted.target = None;
    assert_eq!(
        lookup(&inputs, &current, &[entries[0].clone(), untargeted]),
        Err(AffordanceLookupError::ConflictingTargetPolicy)
    );
    assert_eq!(current, before);
}

#[test]
fn stale_target_and_each_complete_source_pin_refuse_before_source_owner() {
    let inputs = Inputs::new();
    let current = inputs.current();
    let before = current.clone();
    let mut stale = rows(&current);
    stale[0]
        .target
        .as_mut()
        .unwrap()
        .position
        .as_mut()
        .unwrap()
        .x += 1;
    assert_eq!(
        lookup(&inputs, &current, &stale),
        Err(AffordanceLookupError::StaleTarget)
    );
    let mut variants = Vec::new();
    let mut pins = inputs.pins.clone();
    pins.rules.handler_digest.0[0] ^= 1;
    variants.push(pins);
    let mut pins = inputs.pins.clone();
    pins.content.content_digest.0[0] ^= 1;
    variants.push(pins);
    let mut pins = inputs.pins.clone();
    pins.build = df_types::BuildIdentity::new(
        Some("other-source"),
        Some("other-native"),
        Some("other-wasm"),
        Some("other-config"),
        Some("other-content"),
    )
    .unwrap();
    variants.push(pins);
    let owner = RefusingSourceOwner {
        calls: Cell::new(0),
    };
    for pins in variants {
        assert!(matches!(
            lookup_affordances(
                &current,
                AffordanceQuery {
                    basis: current.basis(),
                    pins: &pins,
                    actor: fixture::entity(4),
                    action: &inputs.registration.action
                },
                &rows(&current),
                inputs.inventory(),
                lookup_limits()
            ),
            Err(AffordanceLookupError::Snapshot(_))
        ));
        with_registry(&inputs, &current, &owner, false, |registry, _, selector| {
            assert!(matches!(
                registry.select_with_provenance(&pins, selector, &inputs.registration.source),
                Err(DispatchError::PinsMismatch)
            ));
        });
    }
    let mut revision = current.basis();
    revision.revision = revision.revision.next_sequence().unwrap();
    let mut epoch = current.basis();
    epoch.revision = fixture::revision(3, 8);
    let mut session = current.basis();
    session.session = df_types::SessionId::from_bytes(&[31; 16]).unwrap();
    let mut run = current.basis();
    run.run = df_types::RunId::from_bytes(&[32; 16]).unwrap();
    for basis in [revision, epoch, session, run] {
        assert!(matches!(
            lookup_affordances(
                &current,
                AffordanceQuery {
                    basis,
                    pins: &inputs.pins,
                    actor: fixture::entity(4),
                    action: &inputs.registration.action
                },
                &rows(&current),
                inputs.inventory(),
                lookup_limits()
            ),
            Err(AffordanceLookupError::Snapshot(_))
        ));
    }
    assert_eq!(owner.calls.get(), 0);
    assert_eq!(current, before);
}

#[test]
fn whole_input_and_output_bounds_return_no_partial_candidate() {
    let inputs = Inputs::new();
    let current = inputs.current();
    let before = current.clone();
    let entries = rows(&current);
    let bytes = lookup(&inputs, &current, &entries)
        .unwrap()
        .retained_bytes()
        .unwrap();
    let invoke = |limits| {
        lookup_affordances(
            &current,
            AffordanceQuery {
                basis: current.basis(),
                pins: &inputs.pins,
                actor: fixture::entity(4),
                action: &inputs.registration.action,
            },
            &entries,
            inputs.inventory(),
            limits,
        )
    };
    assert!(
        invoke(AffordanceLookupLimits {
            output_bytes: bytes,
            ..lookup_limits()
        })
        .is_ok()
    );
    for limits in [
        AffordanceLookupLimits {
            output_bytes: bytes - 1,
            ..lookup_limits()
        },
        AffordanceLookupLimits {
            input_records: 1,
            ..lookup_limits()
        },
        AffordanceLookupLimits {
            candidates: 1,
            ..lookup_limits()
        },
    ] {
        assert_eq!(invoke(limits), Err(AffordanceLookupError::Capacity));
    }
    let mut all = entries.clone();
    let mut unrelated = entries[0].clone();
    unrelated.actor = fixture::entity(5);
    unrelated.source.clause = fixture::label("unadmitted-unselected-clause");
    all.push(unrelated);
    assert_eq!(
        lookup(&inputs, &current, &all),
        Err(AffordanceLookupError::UnknownSource)
    );
    assert_eq!(current, before);
}

#[test]
fn actual_registered_engine_refuses_missing_producer_and_unqualified_source() {
    let inputs = Inputs::new();
    let current = inputs.current();
    let before = current.clone();
    let rows = [Affordance {
        actor: fixture::entity(4),
        action: fixture::content(),
        source: fixture::rule(),
        target: None,
    }];
    let set = lookup(&inputs, &current, &rows).unwrap();
    let TargetResolution::Selected(selected) = resolve_target(
        &current,
        &set,
        TargetSelection::Unspecified,
        inputs.inventory(),
        target_limits(),
    )
    .unwrap() else {
        panic!("untargeted fixture")
    };
    let input = fixture::action();
    let saved = input.clone();
    for producer_present in [false, true] {
        let owner = RefusingSourceOwner {
            calls: Cell::new(0),
        };
        with_registry(
            &inputs,
            &current,
            &owner,
            producer_present,
            |registry, _, selector| {
                let result = decide_registered_transition(
                    RulesCommandInput {
                        command: &input,
                        supplied_draws: &[],
                    },
                    &current,
                    CommandEntryContext {
                        current_basis: current.basis(),
                        admitted_pins: &inputs.pins,
                        inventory: inputs.inventory(),
                        limits: CommandEntryLimits {
                            command: command_limits(),
                            maximum_staged_bytes: 1024 * 1024,
                        },
                    },
                    registry,
                    selector,
                    &selected.source,
                );
                let refusal = if producer_present {
                    WorldStagingError::Source(SourceRefusal::Unqualified)
                } else {
                    WorldStagingError::MissingProducer
                };
                assert_eq!(
                    result,
                    TransitionResult::Rejected(CommandRejection::Invocation(
                        InvocationError::Handler(PreconditionedRejection::Handler(refusal))
                    ))
                );
            },
        );
        assert_eq!(owner.calls.get(), usize::from(producer_present));
        assert_eq!(current, before);
        assert_eq!(input, saved);
    }
}

#[test]
fn duplicate_candidates_do_not_mint_decisions_facts_or_consume_resources() {
    let inputs = Inputs::new();
    let current = inputs.current();
    let before = current.clone();
    let entry = rows(&current).remove(0);
    let set = lookup(&inputs, &current, &[entry.clone(), entry.clone()]).unwrap();
    assert_eq!(set.matches, vec![entry]);
    assert!(matches!(
        resolve_target(
            &current,
            &set,
            TargetSelection::Unspecified,
            inputs.inventory(),
            target_limits()
        ),
        Ok(TargetResolution::Selected(_))
    ));
    assert_eq!(current, before);
}

#[test]
fn pending_ruling_is_not_completed_by_world_target_or_source_lookup() {
    let inputs = Inputs::new();
    let mut state = inputs.current().state().clone();
    let fact = FactId::from_bytes(&[7; 16]).unwrap();
    state.facts.push(GameFact {
        id: fact,
        revision: fixture::basis().revision,
        operation: fixture::operation(),
        ordinal: 0,
        cause: None,
        audience: AudienceScope::Shared,
        value: FactValue::ContentEvent {
            definition: fixture::content(),
            subjects: vec![fixture::entity(4)],
        },
    });
    state.pending.push(PendingResolution {
        id: ResolutionId::from_bytes(&[9; 16]).unwrap(),
        basis: fixture::basis(),
        continuation: fixture::label("explicit-ruling-continuation"),
        window: ResolutionWindow {
            id: WindowId::from_bytes(&[10; 16]).unwrap(),
            phase: TriggerPhase::BeforeDraw,
            causal_fact: fact,
            source: fixture::rule(),
            timer: None,
        },
        next: PendingInput::Ruling {
            permitted: vec![OfferedResponse {
                participant: fixture::member(3),
                offer: fixture::label("explicit-current-ruling-offer"),
                options: vec![fixture::label("explicit-current-ruling-option")],
                source: fixture::rule(),
            }],
            source: fixture::rule(),
        },
        choices: vec![],
        draw_ordinals: vec![],
        spent: vec![],
        rulings: vec![],
    });
    let current = inputs.checkpoint(state);
    let before = current.clone();
    let set = lookup(&inputs, &current, &rows(&current)).unwrap();
    let TargetResolution::Selected(selected) = resolve_target(
        &current,
        &set,
        TargetSelection::Explicit(fixture::entity(5)),
        inputs.inventory(),
        target_limits(),
    )
    .unwrap() else {
        panic!("exact target fixture")
    };
    let owner = RefusingSourceOwner {
        calls: Cell::new(0),
    };
    with_registry(&inputs, &current, &owner, false, |registry, _, selector| {
        assert!(
            registry
                .select_with_provenance(&inputs.pins, selector, &selected.source)
                .is_ok()
        );
    });
    assert_eq!(owner.calls.get(), 0);
    assert_eq!(current, before);
    assert!(matches!(
        &current.state().pending[0].next,
        PendingInput::Ruling { .. }
    ));
}
