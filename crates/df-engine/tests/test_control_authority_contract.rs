#[path = "support/fixture_model.rs"]
#[allow(dead_code)]
mod fixture_model;

use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_engine::command_entry::{
    CommandEntryContext, CommandEntryLimits, CommandRejection, RulesCommandHandler,
    RulesCommandInput, decide_registered_command,
};
use df_model::checkpoint::*;
use df_model::commands::{CommandError, CommandLimits};
use df_rules::{DispatchError, DispatchRegistry, HandlerRegistration, InvocationError};
use df_testkit::{
    ClockDomain, ClockError, DrawRequest, PreparedJobKey, ReplayContext, ReplayError, ReplayLimits,
    SemanticReplay, SuppliedDice, VirtualClocks,
};
use df_types::{OperationId, RevisionLabel, RunId};
use fixture_model::*;
use std::cell::Cell;
use std::convert::Infallible;
use std::time::Duration;

struct StagingHandler {
    pins: CheckpointPins,
    candidate: Checkpoint,
    expected_draws: Vec<ActualDraw>,
    expected_draw_pointer: *const ActualDraw,
    observed_draw_pointer: Cell<*const ActualDraw>,
    calls: Cell<usize>,
}

impl RulesCommandHandler for StagingHandler {
    type Rejection = Infallible;

    fn pins(&self) -> &CheckpointPins {
        &self.pins
    }

    fn stage(
        &self,
        input: RulesCommandInput<'_>,
        _: &Checkpoint,
    ) -> Result<Checkpoint, Self::Rejection> {
        self.calls.set(self.calls.get() + 1);
        self.observed_draw_pointer
            .set(input.supplied_draws.as_ptr());
        assert_eq!(input.supplied_draws, self.expected_draws);
        assert_eq!(input.supplied_draws.as_ptr(), self.expected_draw_pointer);
        Ok(self.candidate.clone())
    }
}

fn staged_candidate(current: &Checkpoint, supplied_draws: &[ActualDraw]) -> Checkpoint {
    let mut next_basis = current.basis();
    next_basis.revision = next_basis.revision.next_sequence().unwrap();
    let operation = operation();
    let mut next_state = current.state().clone();
    next_state.draws.extend_from_slice(supplied_draws);

    let facts: Vec<_> = supplied_draws
        .iter()
        .enumerate()
        .map(|(index, draw)| {
            let id = FactId::from_bytes(&[90 + u8::try_from(index).unwrap(); 16]).unwrap();
            next_state.facts.push(GameFact {
                id,
                revision: next_basis.revision,
                operation,
                ordinal: u32::try_from(index).unwrap(),
                cause: None,
                audience: AudienceScope::Shared,
                value: FactValue::DrawAccepted {
                    operation: draw.operation,
                    ordinal: draw.ordinal,
                },
            });
            id
        })
        .collect();
    next_state.decisions.push(AcceptedDecision {
        operation,
        revision: next_basis.revision,
        facts,
        draws: supplied_draws.iter().map(|draw| draw.ordinal).collect(),
        effects: vec![],
        source_policy: label("fixture-policy"),
        semantic_output: Some("fixture accepted outcome".to_owned()),
    });

    let rules = [rule()];
    let content = [content()];
    let resources = resource_constraints();
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        next_basis,
        current.pins().clone(),
        next_state,
        ReferenceInventory {
            rules: &rules,
            content: &content,
            resources: &resources,
            assets: &[],
        },
        limits(),
    )
    .unwrap()
}

fn handler(current: &Checkpoint, supplied_draws: &[ActualDraw]) -> StagingHandler {
    StagingHandler {
        pins: current.pins().clone(),
        candidate: staged_candidate(current, supplied_draws),
        expected_draws: supplied_draws.to_vec(),
        expected_draw_pointer: supplied_draws.as_ptr(),
        observed_draw_pointer: Cell::new(std::ptr::null()),
        calls: Cell::new(0),
    }
}

fn bounds() -> CommandEntryLimits {
    CommandEntryLimits {
        command: CommandLimits {
            maximum_records: 8,
            maximum_text_bytes: 32,
            maximum_retained_bytes: 8192,
        },
        maximum_staged_bytes: 1024 * 1024,
    }
}

fn catalog<'a>(
    pins: &'a CheckpointPins,
    entries: &'a [CatalogEntry<'a, RuleReference>],
) -> CatalogSnapshot<'a, RevisionLabel, CheckpointPins, RuleReference> {
    CatalogSnapshot::from_published(
        &pins.rules.catalog,
        pins,
        b"source-pinned fixture catalog",
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

fn registered(
    input: &GameInput,
    current: &Checkpoint,
    registry: &DispatchRegistry<'_, StagingHandler>,
    selector: &RevisionLabel,
    source: &RuleReference,
    supplied_draws: &[ActualDraw],
) -> Result<Checkpoint, CommandRejection<Infallible>> {
    let pins = current.pins().clone();
    let rules = [rule()];
    let content = [content()];
    let resources = resource_constraints();
    decide_registered_command(
        RulesCommandInput {
            command: input,
            supplied_draws,
        },
        current,
        CommandEntryContext {
            current_basis: current.basis(),
            admitted_pins: &pins,
            inventory: ReferenceInventory {
                rules: &rules,
                content: &content,
                resources: &resources,
                assets: &[],
            },
            limits: bounds(),
        },
        registry,
        selector,
        source,
    )
}

fn actual_draw() -> ActualDraw {
    ActualDraw {
        operation: operation(),
        ordinal: 0,
        resolution: ResolutionId::from_bytes(&[40; 16]).unwrap(),
        window: WindowId::from_bytes(&[41; 16]).unwrap(),
        sides: 20,
        value: 17,
        source: rule(),
    }
}

fn replay_limits() -> ReplayLimits {
    // One decision, one job, one decision fact and one draw ordinal are retained here.
    ReplayLimits {
        maximum_records: 4,
        maximum_semantic_bytes: "fixture accepted outcome".len() + "recorded provider output".len(),
    }
}

#[test]
fn fixture_controls_feed_registered_staging_without_granting_authority() {
    let mut clocks = VirtualClocks::new(Duration::from_secs(120), Duration::from_secs(7));
    assert_eq!(
        clocks.advance_presentation_by(Duration::from_millis(250)),
        Ok(Duration::from_millis(7250))
    );
    assert_eq!(clocks.snapshot().logical, Duration::from_secs(120));
    assert_eq!(
        clocks.advance_logical_by(Duration::from_secs(3)),
        Ok(Duration::from_secs(123))
    );

    let current = checkpoint(state()).unwrap();
    let original = current.clone();
    let input = action();
    let saved_input = input.clone();
    let draws = [actual_draw()];
    let policy = label("fixture-policy");
    let replay_context = ReplayContext {
        basis: current.basis(),
        pins: current.pins(),
        policy: &policy,
    };
    let mut dice =
        SuppliedDice::new(replay_context, operation(), 0, &draws, replay_limits()).unwrap();
    let replayed = dice
        .take(DrawRequest {
            context: replay_context,
            operation: draws[0].operation,
            ordinal: draws[0].ordinal,
            resolution: draws[0].resolution,
            window: draws[0].window,
            sides: draws[0].sides,
            source: &draws[0].source,
        })
        .unwrap();
    assert!(std::ptr::eq(replayed, &draws[0]));
    assert_eq!(dice.consumed(), 1);
    assert_eq!(dice.finish(), Ok(()));

    let handler = handler(&current, &draws);
    let pins = current.pins().clone();
    let source = rule();
    let selector = label("compiled-source-selector");
    let entries = [CatalogEntry::new(&source, b"reviewed fixture source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry =
        DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 1).unwrap();

    let staged = registered(&input, &current, &registry, &selector, &source, &draws).unwrap();
    assert_eq!(staged, handler.candidate);
    assert_eq!(handler.calls.get(), 1);
    assert_eq!(
        handler.observed_draw_pointer.get(),
        draws.as_ptr(),
        "the exact supplied draw slice reaches the registered handler"
    );
    assert_eq!(staged.state().draws.as_slice(), draws.as_slice());
    assert_eq!(current, original, "the returned candidate is not applied");
    assert_eq!(input, saved_input);
    assert_eq!(clocks.snapshot().logical, Duration::from_secs(123));
    assert_eq!(clocks.snapshot().presentation, Duration::from_millis(7250));
}

#[test]
fn invalid_identity_basis_pins_and_source_refuse_before_handler_or_draw_use() {
    let current = checkpoint(state()).unwrap();
    let original = current.clone();
    let draws = [actual_draw()];
    let source = rule();
    let selector = label("compiled-source-selector");
    let policy = label("fixture-policy");
    let context = ReplayContext {
        basis: current.basis(),
        pins: current.pins(),
        policy: &policy,
    };

    for refusal_case in 0..3 {
        let mut input = action();
        if let GameInput::Game(command) = &mut input {
            match refusal_case {
                0 => command.member = member(99),
                1 => {
                    if let GameCommand::ProposeAction { actor, .. } = &mut command.command {
                        *actor = entity(99);
                    }
                }
                2 => command.basis.revision = revision(2, 9),
                _ => {}
            }
        }
        let saved_input = input.clone();
        let handler = handler(&current, &draws);
        let pins = current.pins().clone();
        let entries = [CatalogEntry::new(&source, b"reviewed fixture source")];
        let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
        let registry =
            DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 1).unwrap();
        let result = registered(&input, &current, &registry, &selector, &source, &draws);
        let expected = match refusal_case {
            0 => Err(CommandRejection::Structural(CommandError::UnknownMember)),
            1 => Err(CommandRejection::Structural(CommandError::InvalidReference)),
            2 => Err(CommandRejection::Structural(CommandError::StaleRevision)),
            _ => unreachable!(),
        };
        assert_eq!(result, expected);
        assert_eq!(handler.calls.get(), 0);
        assert_eq!(current, original);
        assert_eq!(input, saved_input);

        // A fresh supplied-dice fixture remains untouched when admission rejects.
        let dice = SuppliedDice::new(context, operation(), 0, &draws, replay_limits()).unwrap();
        assert_eq!(dice.consumed(), 0);
        assert_eq!(dice.remaining(), 1);
        assert_eq!(dice.next_ordinal(), 0);
    }

    let input = action();
    let handler = handler(&current, &draws);
    let pins = current.pins().clone();
    let entries = [CatalogEntry::new(&source, b"reviewed fixture source")];
    let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
    let registry =
        DispatchRegistry::from_catalog(catalog(&pins, &entries), &registrations, 1).unwrap();

    let mut changed_pins = pins.clone();
    changed_pins.rules.handler_digest = ContentDigest([99; 32]);
    let changed_registry =
        DispatchRegistry::from_catalog(catalog(&changed_pins, &entries), &registrations, 1)
            .unwrap();
    let changed_rules = [source.clone()];
    let changed_content = [content()];
    let changed_resources = resource_constraints();
    let changed_context = CommandEntryContext {
        current_basis: current.basis(),
        admitted_pins: &changed_pins,
        inventory: ReferenceInventory {
            rules: &changed_rules,
            content: &changed_content,
            resources: &changed_resources,
            assets: &[],
        },
        limits: bounds(),
    };
    let pin_result = decide_registered_command(
        RulesCommandInput {
            command: &input,
            supplied_draws: &draws,
        },
        &current,
        changed_context,
        &changed_registry,
        &selector,
        &source,
    );
    assert_eq!(
        pin_result,
        Err(CommandRejection::Checkpoint(CheckpointError::RulesMismatch))
    );
    assert_eq!(handler.calls.get(), 0);

    let wrong_source = RuleReference {
        source: label("unregistered-source"),
        ..source.clone()
    };
    let source_result = registered(
        &input,
        &current,
        &registry,
        &selector,
        &wrong_source,
        &draws,
    );
    assert_eq!(
        source_result,
        Err(CommandRejection::Invocation(InvocationError::Dispatch(
            DispatchError::UnsupportedSource
        )))
    );
    assert_eq!(handler.calls.get(), 0);
}

#[test]
fn clock_regression_overflow_and_semantic_replay_are_bounded_read_only_controls() {
    let current_time = Duration::from_secs(5);
    let mut clocks = VirtualClocks::new(current_time, current_time);
    let before = clocks.snapshot();
    assert_eq!(
        clocks.advance_logical_to(current_time - Duration::from_nanos(1)),
        Err(ClockError::TimeRegression {
            domain: ClockDomain::Logical,
            current: current_time,
            requested: current_time - Duration::from_nanos(1),
        })
    );
    assert_eq!(clocks.snapshot(), before);
    let near_max = Duration::MAX - Duration::from_nanos(1);
    let mut overflow = VirtualClocks::new(near_max, near_max);
    let before_overflow = overflow.snapshot();
    assert_eq!(
        overflow.advance_presentation_by(Duration::from_nanos(2)),
        Err(ClockError::Overflow {
            domain: ClockDomain::Presentation,
            current: near_max,
            delta: Duration::from_nanos(2),
        })
    );
    assert_eq!(overflow.snapshot(), before_overflow);

    let current = checkpoint(state()).unwrap();
    let candidate = staged_candidate(&current, &[actual_draw()]);
    let decision = candidate.state().decisions[0].clone();
    let job = JobCompletion {
        basis: candidate.basis(),
        operation: operation(),
        job: JobId::from_bytes(&[42; 16]).unwrap(),
        generation: 7,
        outcome: JobOutcome::Ai {
            semantic_output: "recorded provider output".to_owned(),
            policy: content(),
            model: label("recorded-model"),
        },
    };
    let decisions = [decision];
    let jobs = [job];
    let policy = label("fixture-policy");
    let context = ReplayContext {
        basis: candidate.basis(),
        pins: candidate.pins(),
        policy: &policy,
    };
    let replay = SemanticReplay::new(context, &decisions, &jobs, replay_limits()).unwrap();
    let decision_ref = replay
        .decision(context, operation(), candidate.basis().revision)
        .unwrap();
    assert!(std::ptr::eq(decision_ref, &decisions[0]));
    let key = PreparedJobKey {
        operation: jobs[0].operation,
        job: jobs[0].job,
        generation: jobs[0].generation,
    };
    assert!(std::ptr::eq(
        replay.prepared_job(context, key).unwrap(),
        &jobs[0]
    ));

    let stale_generation = PreparedJobKey {
        generation: key.generation + 1,
        ..key
    };
    assert_eq!(
        replay.prepared_job(context, stale_generation),
        Err(ReplayError::Stale)
    );
    let stale_operation = PreparedJobKey {
        operation: OperationId::from_bytes(&[43; 16]).unwrap(),
        ..key
    };
    assert_eq!(
        replay.prepared_job(context, stale_operation),
        Err(ReplayError::Stale)
    );
    let wrong_policy = label("other-policy");
    let changed_context = ReplayContext {
        policy: &wrong_policy,
        ..context
    };
    assert_eq!(
        replay.decision(changed_context, operation(), candidate.basis().revision),
        Err(ReplayError::Stale)
    );
    assert_eq!(
        replay.prepared_job(changed_context, key),
        Err(ReplayError::Stale)
    );
    let mut changed_basis = context.basis;
    changed_basis.run = RunId::from_bytes(&[8; 16]).unwrap();
    let wrong_basis_context = ReplayContext {
        basis: changed_basis,
        ..context
    };
    assert_eq!(
        replay.decision(wrong_basis_context, operation(), candidate.basis().revision),
        Err(ReplayError::Stale)
    );
    let mut changed_pins = candidate.pins().clone();
    changed_pins.rules.handler_digest = ContentDigest([88; 32]);
    let wrong_pins_context = ReplayContext {
        pins: &changed_pins,
        ..context
    };
    assert_eq!(
        replay.prepared_job(wrong_pins_context, key),
        Err(ReplayError::Stale)
    );
    assert_eq!(
        replay.decision(context, operation(), current.basis().revision),
        Err(ReplayError::Stale)
    );
    assert_eq!(current, checkpoint(state()).unwrap());
}
