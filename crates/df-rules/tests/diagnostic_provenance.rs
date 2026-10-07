// Structural provenance sentinels. The native HarborHandler test supplies real mechanics evidence.
use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_rules::{
    DispatchError, DispatchRegistry, HandlerRegistration, InvocationError, RulesCommandHandler,
    RulesCommandInput,
};
use std::cell::Cell;
include!("fixtures.rs");

fn catalog<'a>(
    pins: &'a CheckpointPins,
    entries: &'a [CatalogEntry<'a, RuleReference>],
) -> CatalogSnapshot<'a, RevisionLabel, CheckpointPins, RuleReference> {
    CatalogSnapshot::from_published(
        &pins.rules.catalog,
        pins,
        b"supplied-source-publication",
        entries,
        CatalogLimits {
            max_complete_bytes: 64,
            max_entries: 4,
            max_item_bytes: 64,
            max_total_item_bytes: 128,
        },
    )
    .unwrap()
}

#[test]
fn explanation_comes_from_the_exact_compiled_registration_and_indexed_clause() {
    let pins = pins();
    let first = rule();
    let mut second = first.clone();
    second.clause = label("second-supported-clause");
    let selector = label("compiled-selector");
    let first_handler = 1_u8;
    let second_handler = 2_u8;
    let entries = [
        CatalogEntry::new(&first, b"first-clause-source"),
        CatalogEntry::new(&second, b"second-clause-source"),
    ];
    let registrations = [
        HandlerRegistration::new(&selector, &first, &first_handler),
        HandlerRegistration::new(&selector, &second, &second_handler),
    ];
    let registry =
        DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 2).unwrap();

    for (source, handler, bytes) in [
        (&first, &first_handler, b"first-clause-source".as_slice()),
        (&second, &second_handler, b"second-clause-source".as_slice()),
    ] {
        let caller_source = source.clone();
        let caller_selector = selector.clone();
        let (selected, explanation) = registry
            .select_with_provenance(&pins, &caller_selector, &caller_source)
            .unwrap();
        assert!(std::ptr::eq(selected, handler));
        assert!(std::ptr::eq(explanation.source(), source));
        assert!(std::ptr::eq(explanation.selector(), &selector));
        assert!(std::ptr::eq(explanation.rules_pins(), &pins.rules));
        assert!(std::ptr::eq(explanation.content_pins(), &pins.content));
        assert!(std::ptr::eq(explanation.build(), &pins.build));
        assert_eq!(explanation.source_bytes(), bytes);
        assert!(std::ptr::eq(
            registry.select(&pins, &selector, source).unwrap(),
            handler
        ));
    }
}

#[test]
fn cosmetic_package_bytes_and_identity_cannot_replace_selected_source_semantics() {
    let first_pins = pins();
    let mut changed_pins = first_pins.clone();
    changed_pins.content.content_digest = ContentDigest([91; 32]);
    changed_pins.content.package_digest = ContentDigest([92; 32]);
    let source = rule();
    let selector = label("compiled-selector");
    let handler = 1_u8;
    let entries = [CatalogEntry::new(&source, b"selected-mechanical-source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let first =
        DispatchRegistry::from_catalog(catalog(&first_pins, &entries), &registrations, 1).unwrap();
    let changed =
        DispatchRegistry::from_catalog(catalog(&changed_pins, &entries), &registrations, 1)
            .unwrap();

    let (_, before) = first
        .select_with_provenance(&first_pins, &selector, &source)
        .unwrap();
    let (_, after) = changed
        .select_with_provenance(&changed_pins, &selector, &source)
        .unwrap();
    for (pins, cosmetic_bytes) in [
        (&first_pins, b"original-portrait".as_slice()),
        (
            &changed_pins,
            b"generated-new-portrait-with-mechanics-claims".as_slice(),
        ),
    ] {
        let cosmetics = CatalogSnapshot::<_, _, ContentReference>::from_published(
            &pins.content.package,
            &pins.content,
            cosmetic_bytes,
            &[],
            CatalogLimits {
                max_complete_bytes: 64,
                max_entries: 0,
                max_item_bytes: 0,
                max_total_item_bytes: 0,
            },
        )
        .unwrap();
        assert_ne!(before.source_bytes(), cosmetics.complete_bytes());
        assert_ne!(after.source_bytes(), cosmetics.complete_bytes());
    }
    assert_eq!(before.rules_pins(), after.rules_pins());
    assert_eq!(before.source(), after.source());
    assert_eq!(before.source_bytes(), after.source_bytes());
    assert_ne!(before.content_pins(), after.content_pins());
    assert_eq!(before.build(), after.build());
    assert_eq!(
        first
            .select_with_provenance(&changed_pins, &selector, &source)
            .err(),
        Some(DispatchError::PinsMismatch)
    );
}

#[test]
fn indexed_unimplemented_source_and_unknown_selector_return_no_explanation() {
    let pins = pins();
    let source = rule();
    let mut unimplemented = source.clone();
    unimplemented.clause = label("indexed-but-unimplemented");
    let selector = label("compiled-selector");
    let handler = 1_u8;
    let entries = [
        CatalogEntry::new(&source, b"implemented-source"),
        CatalogEntry::new(&unimplemented, b"claims-of-support"),
    ];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry =
        DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 1).unwrap();

    assert_eq!(
        registry
            .select_with_provenance(&pins, &selector, &unimplemented)
            .err(),
        Some(DispatchError::UnsupportedSource)
    );
    assert_eq!(
        registry
            .select_with_provenance(&pins, &label("generated-selector"), &source)
            .err(),
        Some(DispatchError::UnknownHandler)
    );
    for field in 0..4 {
        let mut wrong_source = source.clone();
        match field {
            0 => wrong_source.catalog = label("other-catalog"),
            1 => wrong_source.source = label("other-source"),
            2 => wrong_source.entry = label("other-entry"),
            _ => wrong_source.clause = label("other-clause"),
        }
        assert_eq!(
            registry
                .select_with_provenance(&pins, &selector, &wrong_source)
                .err(),
            Some(DispatchError::UnsupportedSource)
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SentinelRejection {
    UnsupportedMechanic,
}

struct SentinelHandler {
    pins: CheckpointPins,
    source: RuleReference,
    candidate: Checkpoint,
    refuse: bool,
    calls: Cell<usize>,
}

impl RulesCommandHandler for SentinelHandler {
    type Rejection = SentinelRejection;

    fn pins(&self) -> &CheckpointPins {
        &self.pins
    }

    fn bound_source(&self) -> Option<&RuleReference> {
        Some(&self.source)
    }

    fn stage(
        &self,
        _: RulesCommandInput<'_>,
        _: &Checkpoint,
    ) -> Result<Checkpoint, Self::Rejection> {
        self.calls.set(self.calls.get() + 1);
        if self.refuse {
            Err(SentinelRejection::UnsupportedMechanic)
        } else {
            Ok(self.candidate.clone())
        }
    }
}

fn request(current: &Checkpoint) -> GameInput {
    GameInput::Game(CommandInput {
        basis: current.basis(),
        observed_revision: current.basis().revision,
        operation: OperationId::from_bytes(&[6; 16]).unwrap(),
        member: member(3),
        command: GameCommand::Speak {
            speaker: entity(4),
            text: "sentinel".into(),
            conversation: None,
        },
    })
}

fn sentinel(current: &Checkpoint) -> SentinelHandler {
    let mut next = current.basis();
    next.revision = next.revision.next_sequence().unwrap();
    let mut state = current.state().clone();
    state.decisions.push(AcceptedDecision {
        operation: OperationId::from_bytes(&[6; 16]).unwrap(),
        revision: next.revision,
        facts: vec![],
        draws: vec![],
        effects: vec![],
        source_policy: current.pins().rules.handler.clone(),
        semantic_output: None,
    });
    let candidate = Checkpoint::new(
        CHECKPOINT_SCHEMA,
        next,
        current.pins().clone(),
        state,
        ReferenceInventory {
            rules: &[rule()],
            content: &[content()],
            resources: &resource_constraints(),
            assets: &[],
        },
        limits(),
    )
    .unwrap();
    SentinelHandler {
        pins: current.pins().clone(),
        source: rule(),
        candidate,
        refuse: false,
        calls: Cell::new(0),
    }
}

#[test]
fn staged_explanation_binds_the_candidate_and_preserves_the_canonical_guard() {
    let current = checkpoint(state()).unwrap();
    let before = current.clone();
    let source = rule();
    let selector = label("compiled-selector");
    let handler = sentinel(&current);
    let entries = [CatalogEntry::new(&source, b"supplied-source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry =
        DispatchRegistry::from_catalog(catalog(current.pins(), &entries), &registrations, 1)
            .unwrap();
    let command = request(&current);
    let input = RulesCommandInput {
        command: &command,
        supplied_draws: &[],
    };

    let (candidate, explanation) = registry
        .stage_with_provenance(
            current.pins(),
            &selector,
            &source,
            input,
            &current,
            1024 * 1024,
        )
        .unwrap();
    assert_eq!(candidate, handler.candidate);
    assert_eq!(candidate.pins().rules, *explanation.rules_pins());
    assert_eq!(candidate.pins().content, *explanation.content_pins());
    assert_eq!(candidate.pins().build, *explanation.build());
    assert_eq!(candidate.state().decisions.len(), 1);
    assert_eq!(
        candidate.state().decisions[0].source_policy,
        explanation.rules_pins().handler
    );
    assert_eq!(
        registry
            .stage(
                current.pins(),
                &selector,
                &source,
                input,
                &current,
                1024 * 1024
            )
            .unwrap(),
        candidate
    );
    assert_eq!(handler.calls.get(), 2);
    assert_eq!(current, before);
}

#[test]
fn mechanical_source_and_candidate_refusals_never_return_a_success_explanation() {
    let current = checkpoint(state()).unwrap();
    let before = current.clone();
    let source = rule();
    let selector = label("compiled-selector");
    let command = request(&current);
    for case in 0..3 {
        let mut handler = sentinel(&current);
        match case {
            0 => handler.refuse = true,
            1 => handler.source.clause = label("different-bound-clause"),
            _ => handler.candidate = current.clone(),
        }
        let entries = [CatalogEntry::new(&source, b"supplied-source")];
        let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
        let registry =
            DispatchRegistry::from_catalog(catalog(current.pins(), &entries), &registrations, 1)
                .unwrap();
        let result = registry.stage_with_provenance(
            current.pins(),
            &selector,
            &source,
            RulesCommandInput {
                command: &command,
                supplied_draws: &[],
            },
            &current,
            1024 * 1024,
        );
        let expected = match case {
            0 => InvocationError::Handler(SentinelRejection::UnsupportedMechanic),
            1 => InvocationError::HandlerSourceMismatch,
            _ => InvocationError::CandidateBasisMismatch,
        };
        assert_eq!(result.err(), Some(expected));
        assert_eq!(handler.calls.get(), usize::from(case != 1));
        assert_eq!(current, before);
    }
}
