use df_model::checkpoint::{
    AcceptedDecision, ActualDraw, Basis, CheckpointPins, ContentDigest, ContentPins,
    ContentReference, EffectId, FactId, JobCompletion, JobId, JobOutcome, ResolutionId,
    RuleReference, RulesMode, RulesPins, WindowId,
};
use df_testkit::{
    DrawRequest, PreparedJobKey, ReplayContext, ReplayError, ReplayLimits, SemanticReplay,
    SuppliedDice,
};
use df_types::{
    BuildIdentity, OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision,
};

fn label(value: &str) -> RevisionLabel {
    RevisionLabel::new(Some(value)).unwrap()
}

fn operation(byte: u8) -> OperationId {
    OperationId::from_bytes(&[byte; 16]).unwrap()
}

fn revision(sequence: u64) -> SessionRevision {
    SessionRevision::new(RecoveryEpoch::new(4).unwrap(), sequence)
}

fn basis() -> Basis {
    Basis {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        revision: revision(8),
    }
}

fn pins() -> CheckpointPins {
    CheckpointPins {
        rules: RulesPins {
            mode: RulesMode::Standard2024,
            ruleset: label("rules-test"),
            catalog: label("catalog-test"),
            catalog_digest: ContentDigest([1; 32]),
            source_manifest: label("source-test"),
            source_manifest_digest: ContentDigest([2; 32]),
            handler: label("handler-test"),
            handler_digest: ContentDigest([3; 32]),
        },
        content: ContentPins {
            content: label("content-test"),
            content_digest: ContentDigest([4; 32]),
            package: label("pack-test"),
            package_digest: ContentDigest([5; 32]),
        },
        build: BuildIdentity::new(
            Some("source-test"),
            Some("native-test"),
            Some("wasm-test"),
            Some("config-test"),
            Some("content-test"),
        )
        .unwrap(),
    }
}

fn context<'a>(pins: &'a CheckpointPins, policy: &'a RevisionLabel) -> ReplayContext<'a> {
    ReplayContext {
        basis: basis(),
        pins,
        policy,
    }
}

fn limits() -> ReplayLimits {
    ReplayLimits {
        maximum_records: 32,
        maximum_semantic_bytes: 128,
    }
}

fn source() -> RuleReference {
    RuleReference {
        catalog: label("catalog-test"),
        source: label("synthetic-source"),
        entry: label("synthetic-entry"),
        clause: label("synthetic-clause"),
    }
}

fn draw(ordinal: u32, value: u32) -> ActualDraw {
    ActualDraw {
        operation: operation(3),
        ordinal,
        resolution: ResolutionId::from_bytes(&[4; 16]).unwrap(),
        window: WindowId::from_bytes(&[5; 16]).unwrap(),
        sides: 20,
        value,
        source: source(),
    }
}

fn request<'a>(context: ReplayContext<'a>, record: &'a ActualDraw) -> DrawRequest<'a> {
    DrawRequest {
        context,
        operation: record.operation,
        ordinal: record.ordinal,
        resolution: record.resolution,
        window: record.window,
        sides: record.sides,
        source: &record.source,
    }
}

fn decision() -> AcceptedDecision {
    AcceptedDecision {
        operation: operation(3),
        revision: revision(7),
        facts: vec![FactId::from_bytes(&[6; 16]).unwrap()],
        draws: vec![0, 1],
        effects: vec![EffectId::from_bytes(&[7; 16]).unwrap()],
        source_policy: label("policy-test"),
        semantic_output: Some("accepted\nUnicode: ü".to_owned()),
    }
}

fn job() -> JobCompletion {
    JobCompletion {
        basis: basis(),
        operation: operation(3),
        job: JobId::from_bytes(&[8; 16]).unwrap(),
        generation: 12,
        outcome: JobOutcome::Ai {
            semantic_output: "prepared\nUnicode: ü".to_owned(),
            policy: ContentReference {
                package: label("pack-test"),
                entry: label("policy-test"),
            },
            model: label("recorded-model"),
        },
    }
}

fn key(record: &JobCompletion) -> PreparedJobKey {
    PreparedJobKey {
        operation: record.operation,
        job: record.job,
        generation: record.generation,
    }
}

#[test]
fn actual_records_and_committed_draw_counters_survive_semantic_replay() {
    let pins = pins();
    let policy = label("policy-test");
    let context = context(&pins, &policy);
    let draws = [draw(0, 19), draw(1, 2)];
    let decisions = [decision()];
    let jobs = [job()];
    let mut dice = SuppliedDice::new(context, operation(3), 0, &draws, limits()).unwrap();
    let replay = SemanticReplay::new(context, &decisions, &jobs, limits()).unwrap();

    let recorded = replay.decision(context, operation(3), revision(7)).unwrap();
    for (ordinal, actual) in recorded.draws.iter().zip(&draws) {
        assert_eq!(*ordinal, dice.next_ordinal());
        assert!(std::ptr::eq(
            dice.take(request(context, actual)).unwrap(),
            actual
        ));
    }
    assert_eq!(dice.next_ordinal(), 2);
    assert_eq!(dice.consumed(), 2);
    assert_eq!(dice.remaining(), 0);
    assert_eq!(dice.finish(), Ok(()));
    assert!(std::ptr::eq(recorded, &decisions[0]));
    assert_eq!(
        replay.decision(context, operation(3), revision(7)),
        Ok(recorded)
    );
    assert!(std::ptr::eq(
        replay.prepared_job(context, key(&jobs[0])).unwrap(),
        &jobs[0]
    ));
    assert_eq!(recorded.semantic_output, decisions[0].semantic_output);
    assert_eq!(dice.next_ordinal(), 2);
}

#[test]
fn missing_duplicate_and_unused_draws_leave_counters_exact() {
    let pins = pins();
    let policy = label("policy-test");
    let context = context(&pins, &policy);
    let draws = [draw(0, 19), draw(1, 2)];
    let mut dice = SuppliedDice::new(context, operation(3), 0, &draws, limits()).unwrap();
    assert_eq!(dice.finish(), Err(ReplayError::Unused { remaining: 2 }));
    assert_eq!(
        dice.take(request(context, &draws[1])),
        Err(ReplayError::InvalidOrder)
    );
    assert_eq!(dice.consumed(), 0);
    dice.take(request(context, &draws[0])).unwrap();
    assert_eq!(
        dice.take(request(context, &draws[0])),
        Err(ReplayError::Duplicate)
    );
    assert_eq!(dice.next_ordinal(), 1);
    assert_eq!(dice.finish(), Err(ReplayError::Unused { remaining: 1 }));
    dice.take(request(context, &draws[1])).unwrap();
    assert_eq!(
        dice.take(request(context, &draw(2, 12))),
        Err(ReplayError::Missing)
    );
    assert_eq!((dice.consumed(), dice.next_ordinal()), (2, 2));
}

#[test]
fn resumed_draw_ordinals_are_preserved_and_overflow_is_atomic() {
    let pins = pins();
    let policy = label("policy-test");
    let context = context(&pins, &policy);
    let draws = [draw(41, 20), draw(42, 1)];
    let mut dice = SuppliedDice::new(context, operation(3), 41, &draws, limits()).unwrap();
    assert_eq!(dice.take(request(context, &draws[0])), Ok(&draws[0]));
    assert_eq!(dice.take(request(context, &draws[1])), Ok(&draws[1]));
    assert_eq!((dice.consumed(), dice.next_ordinal()), (2, 43));
    let terminal = [draw(u32::MAX, 20)];
    let mut dice = SuppliedDice::new(context, operation(3), u32::MAX, &terminal, limits()).unwrap();
    assert_eq!(
        dice.take(request(context, &terminal[0])),
        Err(ReplayError::CounterOverflow)
    );
    assert_eq!(
        (dice.consumed(), dice.next_ordinal(), dice.remaining()),
        (0, u32::MAX, 1)
    );
}

#[test]
fn stale_draw_context_and_changed_source_never_consume() {
    let pins = pins();
    let policy = label("policy-test");
    let context = context(&pins, &policy);
    let draws = [draw(0, 10)];
    let mut dice = SuppliedDice::new(context, operation(3), 0, &draws, limits()).unwrap();
    let mut stale_pins = pins.clone();
    stale_pins.rules.handler_digest = ContentDigest([99; 32]);
    let mut stale = context;
    stale.pins = &stale_pins;
    assert_eq!(
        dice.take(request(stale, &draws[0])),
        Err(ReplayError::Stale)
    );
    for mutate in 0..5 {
        let mut changed = draws[0].clone();
        match mutate {
            0 => changed.operation = operation(9),
            1 => changed.resolution = ResolutionId::from_bytes(&[9; 16]).unwrap(),
            2 => changed.window = WindowId::from_bytes(&[9; 16]).unwrap(),
            3 => changed.source.clause = label("changed-clause"),
            _ => changed.source.source = label("changed-source"),
        }
        assert_eq!(
            dice.take(request(context, &changed)),
            Err(ReplayError::Stale)
        );
    }
    let mut changed = draws[0].clone();
    changed.sides = 6;
    assert_eq!(
        dice.take(request(context, &changed)),
        Err(ReplayError::InvalidDraw)
    );
    assert_eq!((dice.consumed(), dice.next_ordinal()), (0, 0));
    assert_eq!(dice.take(request(context, &draws[0])), Ok(&draws[0]));
}

#[test]
fn malformed_supplied_draws_are_rejected_before_use() {
    let pins = pins();
    let policy = label("policy-test");
    let context = context(&pins, &policy);
    for (sides, value) in [(0, 1), (20, 0), (20, 21)] {
        let mut record = draw(0, value);
        record.sides = sides;
        assert_eq!(
            SuppliedDice::new(context, operation(3), 0, &[record], limits()).err(),
            Some(ReplayError::InvalidDraw)
        );
    }
    assert_eq!(
        SuppliedDice::new(
            context,
            operation(3),
            0,
            &[draw(0, 1), draw(0, 2)],
            limits()
        )
        .err(),
        Some(ReplayError::Duplicate)
    );
    assert_eq!(
        SuppliedDice::new(context, operation(3), 0, &[draw(1, 1)], limits()).err(),
        Some(ReplayError::InvalidOrder)
    );
    let mut record = draw(0, 1);
    record.source.catalog = label("other-catalog");
    assert_eq!(
        SuppliedDice::new(context, operation(3), 0, &[record], limits()).err(),
        Some(ReplayError::Stale)
    );
    let records = [draw(u32::MAX, 1), draw(0, 1)];
    assert_eq!(
        SuppliedDice::new(context, operation(3), u32::MAX, &records, limits()).err(),
        Some(ReplayError::CounterOverflow)
    );
}

#[test]
fn missing_semantic_records_and_missing_output_are_explicit() {
    let pins = pins();
    let policy = label("policy-test");
    let context = context(&pins, &policy);
    let replay = SemanticReplay::new(context, &[], &[], limits()).unwrap();
    assert_eq!(
        replay.decision(context, operation(3), revision(7)),
        Err(ReplayError::Missing)
    );
    assert_eq!(
        replay.prepared_job(context, key(&job())),
        Err(ReplayError::Missing)
    );
    let mut missing = decision();
    missing.semantic_output = None;
    let records = [missing];
    let replay = SemanticReplay::new(context, &records, &[], limits()).unwrap();
    assert_eq!(
        replay.decision(context, operation(3), revision(7)),
        Err(ReplayError::MissingSemantic)
    );
}

#[test]
fn stale_semantic_context_revision_and_job_generation_are_explicit() {
    let pins = pins();
    let policy = label("policy-test");
    let context = context(&pins, &policy);
    let decisions = [decision()];
    let jobs = [job()];
    let replay = SemanticReplay::new(context, &decisions, &jobs, limits()).unwrap();
    let mut stale = context;
    stale.basis.revision = revision(9);
    assert_eq!(
        replay.decision(stale, operation(3), revision(7)),
        Err(ReplayError::Stale)
    );
    assert_eq!(
        replay.prepared_job(stale, key(&jobs[0])),
        Err(ReplayError::Stale)
    );
    assert_eq!(
        replay.decision(context, operation(3), revision(6)),
        Err(ReplayError::Stale)
    );
    for field in 0..2 {
        let mut stale_key = key(&jobs[0]);
        if field == 0 {
            stale_key.generation += 1;
        } else {
            stale_key.operation = operation(9);
        }
        assert_eq!(
            replay.prepared_job(context, stale_key),
            Err(ReplayError::Stale)
        );
    }
    assert_eq!(
        replay.decision(context, operation(3), revision(7)),
        Ok(&decisions[0])
    );
    assert_eq!(replay.prepared_job(context, key(&jobs[0])), Ok(&jobs[0]));
}

#[test]
fn every_checkpoint_pin_and_session_run_policy_is_part_of_lookup_identity() {
    let original_pins = pins();
    let policy = label("policy-test");
    let context = context(&original_pins, &policy);
    let decisions = [decision()];
    let replay = SemanticReplay::new(context, &decisions, &[], limits()).unwrap();
    for field in 0..15 {
        let mut pins = original_pins.clone();
        let mut changed = context;
        let other_policy = label("other-policy");
        match field {
            0 => pins.rules.mode = RulesMode::DisclosedCustom,
            1 => pins.rules.ruleset = label("other"),
            2 => pins.rules.catalog = label("other"),
            3 => pins.rules.catalog_digest = ContentDigest([9; 32]),
            4 => pins.rules.source_manifest = label("other"),
            5 => pins.rules.source_manifest_digest = ContentDigest([9; 32]),
            6 => pins.rules.handler = label("other"),
            7 => pins.rules.handler_digest = ContentDigest([9; 32]),
            8 => pins.content.content = label("other"),
            9 => pins.content.content_digest = ContentDigest([9; 32]),
            10 => pins.content.package = label("other"),
            11 => pins.content.package_digest = ContentDigest([9; 32]),
            12 => changed.basis.session = SessionId::from_bytes(&[9; 16]).unwrap(),
            13 => changed.basis.run = RunId::from_bytes(&[9; 16]).unwrap(),
            _ => changed.policy = &other_policy,
        }
        changed.pins = &pins;
        assert_eq!(
            replay.decision(changed, operation(3), revision(7)),
            Err(ReplayError::Stale)
        );
    }
    let mut changed_pins = original_pins.clone();
    changed_pins.build = BuildIdentity::new(
        Some("different"),
        Some("native-test"),
        Some("wasm-test"),
        Some("config-test"),
        Some("content-test"),
    )
    .unwrap();
    let mut changed = context;
    changed.pins = &changed_pins;
    assert_eq!(
        replay.decision(changed, operation(3), revision(7)),
        Err(ReplayError::Stale)
    );
}

#[test]
fn duplicate_or_stale_semantic_history_is_rejected_at_construction() {
    let pins = pins();
    let policy = label("policy-test");
    let context = context(&pins, &policy);
    assert_eq!(
        SemanticReplay::new(context, &[decision(), decision()], &[], limits()).err(),
        Some(ReplayError::Duplicate)
    );
    assert_eq!(
        SemanticReplay::new(context, &[], &[job(), job()], limits()).err(),
        Some(ReplayError::Duplicate)
    );
    let mut duplicate = decision();
    duplicate.draws = vec![2, 2];
    assert_eq!(
        SemanticReplay::new(context, &[duplicate], &[], limits()).err(),
        Some(ReplayError::Duplicate)
    );
    let mut stale_decision = decision();
    stale_decision.source_policy = label("other-policy");
    assert_eq!(
        SemanticReplay::new(context, &[stale_decision], &[], limits()).err(),
        Some(ReplayError::Stale)
    );
    let mut future = decision();
    future.revision = revision(9);
    assert_eq!(
        SemanticReplay::new(context, &[future], &[], limits()).err(),
        Some(ReplayError::Stale)
    );
    let mut stale_job = job();
    stale_job.basis.revision = revision(7);
    assert_eq!(
        SemanticReplay::new(context, &[], &[stale_job], limits()).err(),
        Some(ReplayError::Stale)
    );
    let mut invalid = job();
    invalid.generation = 0;
    assert_eq!(
        SemanticReplay::new(context, &[], &[invalid], limits()).err(),
        Some(ReplayError::InvalidRecord)
    );
    let mut wrong_kind = job();
    wrong_kind.outcome = JobOutcome::JobCancelled {
        target: wrong_kind.job,
    };
    assert_eq!(
        SemanticReplay::new(context, &[], &[wrong_kind], limits()).err(),
        Some(ReplayError::InvalidRecord)
    );
    let mut wrong_package = job();
    if let JobOutcome::Ai { policy, .. } = &mut wrong_package.outcome {
        policy.package = label("other-pack");
    }
    assert_eq!(
        SemanticReplay::new(context, &[], &[wrong_package], limits()).err(),
        Some(ReplayError::Stale)
    );
}

#[test]
fn fixture_capacity_and_nested_semantic_text_bounds_are_explicit() {
    let pins = pins();
    let policy = label("policy-test");
    let context = context(&pins, &policy);
    let small = ReplayLimits {
        maximum_records: 1,
        maximum_semantic_bytes: 1,
    };
    assert_eq!(
        SuppliedDice::new(context, operation(3), 0, &[draw(0, 1), draw(1, 1)], small).err(),
        Some(ReplayError::Capacity)
    );
    assert_eq!(
        SemanticReplay::new(context, &[decision()], &[], small).err(),
        Some(ReplayError::Capacity)
    );
    let records = [decision()];
    let text_size = records[0].semantic_output.as_ref().unwrap().len();
    let exact = ReplayLimits {
        maximum_records: 5,
        maximum_semantic_bytes: text_size,
    };
    assert!(SemanticReplay::new(context, &records, &[], exact).is_ok());
    let combined_records = ReplayLimits {
        maximum_records: 32,
        ..exact
    };
    assert_eq!(
        SemanticReplay::new(context, &records, &[job()], combined_records).err(),
        Some(ReplayError::Capacity)
    );
    let text_short = ReplayLimits {
        maximum_records: 32,
        maximum_semantic_bytes: text_size - 1,
    };
    assert_eq!(
        SemanticReplay::new(context, &records, &[], text_short).err(),
        Some(ReplayError::Capacity)
    );
    for zero in [
        ReplayLimits {
            maximum_records: 0,
            maximum_semantic_bytes: 1,
        },
        ReplayLimits {
            maximum_records: 1,
            maximum_semantic_bytes: 0,
        },
    ] {
        assert_eq!(
            SuppliedDice::new(context, operation(3), 0, &[], zero).err(),
            Some(ReplayError::Capacity)
        );
        assert_eq!(
            SemanticReplay::new(context, &[], &[], zero).err(),
            Some(ReplayError::Capacity)
        );
    }
}
