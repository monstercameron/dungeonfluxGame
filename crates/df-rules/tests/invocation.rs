// Cross-module compiled-handler sentinels; none establishes a source mechanics golden.
use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_model::commands::{CommandLimits, validate_client_command};
use df_rules::{
    DispatchError, DispatchRegistry, HandlerRegistration, InvocationError, RulesCommandHandler,
};
use std::cell::Cell;
include!("fixtures.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureRejection {
    UnsupportedMechanic,
}

struct FixtureHandler {
    supplied_pins: CheckpointPins,
    candidate: Checkpoint,
    reject: bool,
    calls: Cell<usize>,
}

impl RulesCommandHandler for FixtureHandler {
    type Rejection = FixtureRejection;

    fn pins(&self) -> &CheckpointPins {
        &self.supplied_pins
    }

    fn stage(&self, _: &GameInput, _: &Checkpoint) -> Result<Checkpoint, Self::Rejection> {
        self.calls.set(self.calls.get() + 1);
        if self.reject {
            Err(FixtureRejection::UnsupportedMechanic)
        } else {
            Ok(self.candidate.clone())
        }
    }
}

fn input() -> GameInput {
    GameInput::Game(CommandInput {
        basis: basis(),
        observed_revision: basis().revision,
        operation: OperationId::from_bytes(&[6; 16]).unwrap(),
        member: member(3),
        command: GameCommand::Speak {
            speaker: entity(4),
            text: "fixture".into(),
            conversation: None,
        },
    })
}

fn candidate_with(
    current: &Checkpoint,
    candidate_basis: Basis,
    candidate_pins: CheckpointPins,
    decision: bool,
) -> Checkpoint {
    let mut supplied = current.state().clone();
    if decision {
        supplied.decisions.push(AcceptedDecision {
            operation: OperationId::from_bytes(&[6; 16]).unwrap(),
            revision: candidate_basis.revision,
            facts: vec![],
            draws: vec![],
            effects: vec![],
            source_policy: label("fixture-source-policy"),
            semantic_output: None,
        });
    }
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        candidate_basis,
        candidate_pins,
        supplied,
        ReferenceInventory {
            rules: &[rule()],
            content: &[content()],
            resources: &resource_constraints(),
            assets: &[],
        },
        limits(),
    )
    .unwrap()
}

fn fixture(current: &Checkpoint) -> FixtureHandler {
    let mut next = current.basis();
    next.revision = next.revision.next_sequence().unwrap();
    FixtureHandler {
        supplied_pins: current.pins().clone(),
        candidate: candidate_with(current, next, current.pins().clone(), true),
        reject: false,
        calls: Cell::new(0),
    }
}

fn registry<'a>(
    pins: &'a CheckpointPins,
    entries: &'a [CatalogEntry<'a, RuleReference>],
    registrations: &'a [HandlerRegistration<'a, FixtureHandler>],
) -> DispatchRegistry<'a, FixtureHandler> {
    let catalog = CatalogSnapshot::from_published(
        &pins.rules.catalog,
        pins,
        b"synthetic-source-index",
        entries,
        CatalogLimits {
            max_complete_bytes: 64,
            max_entries: 4,
            max_item_bytes: 64,
            max_total_item_bytes: 128,
        },
    )
    .unwrap();
    DispatchRegistry::from_catalog(catalog, registrations, 4).unwrap()
}

#[test]
fn canonical_admission_then_exact_source_selection_invokes_one_compiled_handler() {
    let current = checkpoint(state()).unwrap();
    let before = current.clone();
    let request = input();
    let selector = label("fixture-selector");
    let source = rule();
    let handler = fixture(&current);
    let entries = [CatalogEntry::new(&source, b"opaque-source-parameters")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry = registry(current.pins(), &entries, &registrations);
    validate_client_command(
        &request,
        &current,
        ReferenceInventory {
            rules: &[rule()],
            content: &[content()],
            resources: &[],
            assets: &[],
        },
        CommandLimits {
            maximum_records: 8,
            maximum_text_bytes: 32,
            maximum_retained_bytes: 8192,
        },
    )
    .unwrap();

    let candidate = registry
        .stage(
            current.pins(),
            &selector,
            &source,
            &request,
            &current,
            1024 * 1024,
        )
        .unwrap();

    assert_eq!(handler.calls.get(), 1);
    assert_eq!(candidate, handler.candidate);
    assert_eq!(current, before);
    assert_eq!(candidate.state().resources, current.state().resources);
    assert!(candidate.state().draws.is_empty());
}

#[test]
fn unknown_or_unsupported_source_never_invokes_the_known_handler() {
    let current = checkpoint(state()).unwrap();
    let before = current.clone();
    let selector = label("fixture-selector");
    let source = rule();
    let handler = fixture(&current);
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry = registry(current.pins(), &entries, &registrations);
    let mut other = source.clone();
    other.clause = label("unsupported-clause");

    assert_eq!(
        registry.stage(
            current.pins(),
            &label("unknown"),
            &source,
            &input(),
            &current,
            1024 * 1024,
        ),
        Err(InvocationError::Dispatch(DispatchError::UnknownHandler))
    );
    assert_eq!(
        registry.stage(
            current.pins(),
            &selector,
            &other,
            &input(),
            &current,
            1024 * 1024,
        ),
        Err(InvocationError::Dispatch(DispatchError::UnsupportedSource))
    );
    assert_eq!(handler.calls.get(), 0);
    assert_eq!(current, before);
}

#[test]
fn registry_or_handler_pin_mismatch_refuses_before_invocation() {
    let current = checkpoint(state()).unwrap();
    let selector = label("fixture-selector");
    let source = rule();
    let mut handler = fixture(&current);
    handler.supplied_pins.rules.handler_digest = ContentDigest([90; 32]);
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry = registry(current.pins(), &entries, &registrations);
    let mut changed = current.pins().clone();
    changed.content.content_digest = ContentDigest([91; 32]);

    assert_eq!(
        registry.stage(
            &changed,
            &selector,
            &source,
            &input(),
            &current,
            1024 * 1024,
        ),
        Err(InvocationError::Dispatch(DispatchError::PinsMismatch))
    );
    assert_eq!(
        registry.stage(
            current.pins(),
            &selector,
            &source,
            &input(),
            &current,
            1024 * 1024,
        ),
        Err(InvocationError::HandlerRulesMismatch)
    );
    assert_eq!(handler.calls.get(), 0);
}

#[test]
fn typed_mechanical_rejection_returns_no_candidate_or_mutation() {
    let current = checkpoint(state()).unwrap();
    let before = current.clone();
    let selector = label("fixture-selector");
    let source = rule();
    let mut handler = fixture(&current);
    handler.reject = true;
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry = registry(current.pins(), &entries, &registrations);

    assert_eq!(
        registry.stage(
            current.pins(),
            &selector,
            &source,
            &input(),
            &current,
            1024 * 1024,
        ),
        Err(InvocationError::Handler(
            FixtureRejection::UnsupportedMechanic
        ))
    );
    assert_eq!(handler.calls.get(), 1);
    assert_eq!(current, before);
}

#[test]
fn malformed_candidate_basis_decision_pins_and_capacity_are_explicit_refusals() {
    let current = checkpoint(state()).unwrap();
    let before = current.clone();
    let selector = label("fixture-selector");
    let source = rule();
    for case in 0..4 {
        let mut handler = fixture(&current);
        let expected = match case {
            0 => {
                handler.candidate =
                    candidate_with(&current, current.basis(), current.pins().clone(), true);
                InvocationError::CandidateBasisMismatch
            }
            1 => {
                handler.candidate = candidate_with(
                    &current,
                    handler.candidate.basis(),
                    current.pins().clone(),
                    false,
                );
                InvocationError::MissingAcceptedDecision
            }
            2 => {
                let mut changed = current.pins().clone();
                changed.rules.handler_digest = ContentDigest([90; 32]);
                handler.candidate =
                    candidate_with(&current, handler.candidate.basis(), changed, true);
                InvocationError::CandidateRulesMismatch
            }
            _ => InvocationError::Capacity,
        };
        let entries = [CatalogEntry::new(&source, b"source")];
        let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
        let registry = registry(current.pins(), &entries, &registrations);
        let maximum = if case == 3 { 1 } else { 1024 * 1024 };

        assert_eq!(
            registry.stage(
                current.pins(),
                &selector,
                &source,
                &input(),
                &current,
                maximum,
            ),
            Err(expected)
        );
        assert_eq!(handler.calls.get(), 1);
        assert_eq!(current, before);
    }
}

#[test]
fn earlier_same_epoch_input_remains_admissible_but_future_input_never_invokes() {
    let current = checkpoint(state()).unwrap();
    let selector = label("fixture-selector");
    let source = rule();
    let handler = fixture(&current);
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry = registry(current.pins(), &entries, &registrations);
    let mut request = input();
    if let GameInput::Game(command) = &mut request {
        command.basis.revision = revision(2, 7);
        command.observed_revision = revision(2, 7);
    }
    assert!(
        registry
            .stage(
                current.pins(),
                &selector,
                &source,
                &request,
                &current,
                1024 * 1024,
            )
            .is_ok()
    );
    assert_eq!(handler.calls.get(), 1);
    if let GameInput::Game(command) = &mut request {
        command.basis.revision = revision(2, 9);
    }
    assert_eq!(
        registry.stage(
            current.pins(),
            &selector,
            &source,
            &request,
            &current,
            1024 * 1024,
        ),
        Err(InvocationError::InputBasisMismatch)
    );
    assert_eq!(handler.calls.get(), 1);
}

#[test]
fn zero_output_bound_refuses_before_any_handler_invocation() {
    let current = checkpoint(state()).unwrap();
    let selector = label("fixture-selector");
    let source = rule();
    let handler = fixture(&current);
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry = registry(current.pins(), &entries, &registrations);

    assert_eq!(
        registry.stage(current.pins(), &selector, &source, &input(), &current, 0,),
        Err(InvocationError::Capacity)
    );
    assert_eq!(handler.calls.get(), 0);
}

#[test]
fn previously_accepted_operation_is_never_staged_again() {
    let initial = checkpoint(state()).unwrap();
    let handler = fixture(&initial);
    let current = handler.candidate.clone();
    let selector = label("fixture-selector");
    let source = rule();
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry = registry(current.pins(), &entries, &registrations);

    assert_eq!(
        registry.stage(
            current.pins(),
            &selector,
            &source,
            &input(),
            &current,
            1024 * 1024,
        ),
        Err(InvocationError::AlreadyAccepted)
    );
    assert_eq!(handler.calls.get(), 0);
}

#[test]
fn revision_exhaustion_refuses_before_handler_invocation() {
    let initial = checkpoint(state()).unwrap();
    let handler = fixture(&initial);
    let mut exhausted = initial.basis();
    exhausted.revision = revision(2, u64::MAX);
    let current = candidate_with(&initial, exhausted, initial.pins().clone(), false);
    let selector = label("fixture-selector");
    let source = rule();
    let entries = [CatalogEntry::new(&source, b"source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry = registry(current.pins(), &entries, &registrations);

    assert_eq!(
        registry.stage(
            current.pins(),
            &selector,
            &source,
            &input(),
            &current,
            1024 * 1024,
        ),
        Err(InvocationError::Revision(
            df_types::RevisionError::SequenceOverflow
        ))
    );
    assert_eq!(handler.calls.get(), 0);
}
