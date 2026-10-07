#![cfg(not(target_arch = "wasm32"))]

//! Bounded evidence for the current native wake and canonical timer boundaries.
//! This does not implement or qualify a session timer-completion owner.
#[path = "support/fixture_model.rs"]
#[allow(dead_code)]
mod fixture_model;

use df_model::checkpoint::*;
use df_model::commands::{CommandError, CommandLimits, validate_client_command};
use df_session::inbox::{AdmissionSequence, InboxHandle, Reducer, bounded_inbox};
use df_types::OperationId;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

const CONTRACT_LITERAL: &str = r##"{
  "task_id": "B-C-df-session-D03",
  "attempt_id": "B-C-df-session-D03-a2",
  "objective": "Define timer clock generation and recovery modes",
  "original_acceptance": [
    "paused logical time differs from process wall time",
    "The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit."
  ],
  "decision": {
    "status": "bounded current-source contract; production timer policy unresolved",
    "logical_time": "LogicalTime is canonical checkpoint data. Process monotonic deadlines may wake the native owner, but the wake does not advance logical time, prove a timer due, or admit gameplay. Only an authorized, source-valid canonical candidate acknowledged by the current durable owner can advance committed game time.",
    "clock_separation": "std::time::Instant/Duration represent process scheduling elapsed time. LogicalTime uses ticks and ticks_per_second. Do not convert process elapsed time or downtime into game ticks. A paused owner retains its canonical LogicalTime; external process time can pass independently.",
    "timer_instance": "OwnedTimer identity is (id, basis, generation, due, source, status). The timer ID names the instance; generation is a distinct callback fence. DurableIntent independently retains (id, basis, operation, slot, kind, job, timer, generation, status, definition). Neither counter reuse nor the timer ID alone replaces full current registration validation.",
    "recovery_epoch": "SessionRevision contains a RecoveryEpoch and an in-epoch sequence. A clock or ExecutionMode does not allocate an epoch, owner fence, process generation, client generation, or timer generation. Disaster restore/epoch retirement remains the protected recovery owner's responsibility.",
    "current_modes": "ExecutionMode is exactly Live, PreparedOnly, or Replay. It is preserved as checkpoint state and does not itself authorize a clock advance or determine pause/restart policy.",
    "owner_wake": "The current ActorLoop::run_with_owner_wake accepts an owner-supplied Instant deadline. It calls the reducer-owned wake outside the inbox lock; wake work creates no admission sequence. Stop cancels future wakes and drains already accepted inputs in FIFO order. This primitive does not admit logical-time gameplay changes or timer completions.",
    "client_boundary": "validate_client_command returns CommandError::InternalInput for GameInput::Job and GameInput::Timer. Client DTO admission cannot forge those native completion inputs.",
    "invalid_canonical_state": "Checkpoint::new rejects zero ticks_per_second with InvalidTime and zero OwnedTimer or DurableIntent generation with InvalidIntent. validate_resume rejects an expected stale SessionRevision basis with StaleBasis.",
    "no_fake_completion": "The wake probe never submits GameInput::Timer or GameInput::Job and never treats inbox admission as timer completion, decision, or commit."
  },
  "alternatives": [
    {
      "option": "Advance logical time from wall time, process uptime, reconnect, or downtime",
      "decision": "rejected",
      "reason": "This advances paused state without an authorized committed decision and makes replay depend on process timing."
    },
    {
      "option": "Treat a monotonic wake as proof that a canonical timer expired",
      "decision": "rejected",
      "reason": "A wake is scheduling only. The owner must revalidate current basis, run, registration, generation, due value, status, pause policy, and durable fence before any candidate can be considered. The current public session APIs do not provide that completion-admission path."
    },
    {
      "option": "Use one counter or timer name as both instance identity and generation",
      "decision": "rejected",
      "reason": "Replacement and stale callback validation require distinct timer instance identity plus generation and current basis."
    },
    {
      "option": "Let a clock or ExecutionMode issue recovery/process generations or pause authority",
      "decision": "rejected",
      "reason": "Those authorities belong to session/recovery owners; the current API does not select their policy."
    }
  ],
  "recovery_modes": {
    "current_execution_modes": ["Live", "PreparedOnly", "Replay"],
    "required_session_scenarios_not_current_modes": [
      "active run: reload committed logical time and pending records; never derive game time from process elapsed time",
      "paused run: retain logical time and pending records; a resume decision is required before policy can make due work eligible",
      "restart or owner takeover: reload compatible durable state, obtain current owner fence, and reject displaced process callbacks",
      "reset or stopped run: a new run/revision or explicit shutdown policy fences old work; shutdown does not invent completed expiry",
      "deterministic replay: consume committed logical inputs/outcomes under compatible pinned source without provider calls",
      "disaster restore: the protected recovery owner issues a verified newer RecoveryEpoch, retires old namespaces, and reports lost ranges before serving"
    ],
    "not_implemented_or_admitted_here": [
      "pause/cause policy owner",
      "registration allocator and current callback-generation admission",
      "general Session timer/job completion entry point",
      "restart, takeover, reset, or disaster-restore integration"
    ]
  },
  "actual_source_cases": [
    "Run the actual df-session ActorLoop::run_with_owner_wake with an expired native Instant deadline while its fixture owner marks the scenario paused; observe one owner wake and confirm the canonical checkpoint LogicalTime and not-yet-due timer remain equal. The callback submits no completion input.",
    "Use the existing df-session support fixture and actual df-model Checkpoint, OwnedTimer, DurableIntent, LogicalTime, Basis, and ExecutionMode types. Retain complete timer and durable-intent fields; show two distinct timer IDs can share a generation.",
    "Confirm Live, PreparedOnly, and Replay survive checkpoint construction without becoming clock authority.",
    "Refuse zero timer generation, zero time unit, and a stale resume basis through the actual canonical Checkpoint constructor/validator.",
    "Refuse actual TimerExpiry and JobCompletion values through validate_client_command as InternalInput.",
    "Preserve admitted inbox sequence/FIFO reduction and stop-drains-accepted-input behavior around the owner wake."
  ],
  "unresolved": [
    "No native selected pause/cause policy owner or general Session timer/job completion callable is established by current public APIs. This remains explicit and is not an I03 implementation claim.",
    "ExecutionMode is Live/PreparedOnly/Replay; it is not authority to allocate a recovery epoch, client generation, process generation, timer generation, or pause policy. Actor-specific policy is outside the two permitted paths.",
    "No current source admits/revalidates a timer callback against an active registration and durably commits timer expiry. The contract therefore does not claim a completed expiry path.",
    "No Rust fixture execution, formatting run, compilation, runtime execution, source commit/push, or independent frontier acceptance has occurred. Root owns gates and integration; finite checks await explicit root release.",
    "Compatibility rules, timer-instance allocation, callback delivery semantics, pause/resume transition authority, bounded catch-up order/limits, and recovery behavior require their named upstream owners and frozen contracts."
  ],
  "next_consumer": {
    "owner": "df-session actor and its df-persistence repository adapter after separately frozen prerequisite contracts",
    "minimum_source_paths": ["crates/df-session/src/actor.rs", "crates/df-session/src/clock.rs", "crates/df-persistence/src/"],
    "required_behavior": "A native scheduler only produces an owned wake. The serialization owner rechecks the current fenced session/run registration and canonical basis/due/source/status/pause policy, produces a source-valid candidate with a stable operation identity, and relies on durable acknowledgement before state or publication changes. Recovery explicitly invalidates stale registrations. These are future requirements, not delivered behavior."
  },
  "checks": {
    "source_contract_tests": "authored; execution UNPERFORMED pending explicit root finite-check release",
    "rustfmt": "UNPERFORMED; root owns pinned formatting gate",
    "rustc_rust_2024_d_warnings": "UNPERFORMED; root owns bounded compiler gate",
    "cargo_or_production_qualification": "UNPERFORMED and not claimed",
    "native_wasm_clippy_or_build": "UNPERFORMED",
    "database_restart_or_durable_commit": "UNPERFORMED",
    "browser_or_provider": "UNPERFORMED and not required for this native bounded example",
    "independent_frontier_original_boundary_review": "PENDING"
  }
}
"##;

struct PausedOwner {
    sender: InboxHandle<GameInput>,
    checkpoint: Checkpoint,
    paused_fixture: bool,
    wake_at: Option<Instant>,
    wake_count: usize,
    observed_deadline: Option<Instant>,
    inputs: Vec<(AdmissionSequence, GameInput)>,
}

impl Reducer<GameInput> for PausedOwner {
    fn reduce(&mut self, sequence: AdmissionSequence, input: GameInput) {
        self.inputs.push((sequence, input));
    }
}

fn timer_state(mode: ExecutionMode) -> GameState {
    let mut state = fixture_model::state();
    state.mode = mode;
    state.timers = vec![
        OwnedTimer {
            id: TimerId::from_bytes(&[81; 16]).unwrap(),
            basis: fixture_model::basis(),
            generation: 4,
            due: LogicalTime {
                ticks: 130,
                ticks_per_second: 10,
            },
            source: fixture_model::rule(),
            status: DurableStatus::Pending,
        },
        OwnedTimer {
            id: TimerId::from_bytes(&[82; 16]).unwrap(),
            basis: fixture_model::basis(),
            generation: 4,
            due: LogicalTime {
                ticks: 140,
                ticks_per_second: 10,
            },
            source: fixture_model::rule(),
            status: DurableStatus::Cancelled,
        },
    ];
    state.intents.push(DurableIntent {
        id: EffectId::from_bytes(&[83; 16]).unwrap(),
        basis: fixture_model::basis(),
        operation: OperationId::from_bytes(&[84; 16]).unwrap(),
        slot: 0,
        kind: EffectKind::ArmTimer,
        job: None,
        timer: Some(TimerId::from_bytes(&[81; 16]).unwrap()),
        generation: 4,
        status: DurableStatus::Pending,
        definition: fixture_model::content(),
    });
    state
}

fn checkpoint(state: GameState) -> Checkpoint {
    fixture_model::checkpoint(state).unwrap()
}

fn assert_client_input_refused(current: &Checkpoint, input: &GameInput) {
    let rules = [fixture_model::rule()];
    let contents = [fixture_model::content()];
    let resources = fixture_model::resource_constraints();
    let inventory = ReferenceInventory {
        rules: &rules,
        content: &contents,
        resources: &resources,
        assets: &[],
    };
    let limits = CommandLimits {
        maximum_records: 16,
        maximum_text_bytes: 1024,
        maximum_retained_bytes: 65536,
    };
    assert_eq!(
        validate_client_command(input, current, inventory, limits),
        Err(CommandError::InternalInput)
    );
}

#[test]
fn owner_monotonic_wake_does_not_advance_canonical_time_or_fake_timer_admission() {
    let (sender, actor) = bounded_inbox::<GameInput>();
    let current = checkpoint(timer_state(ExecutionMode::Live));
    let before = current.state().logical_time;
    let due_timer = current.state().timers[0].clone();
    let first = fixture_model::action();
    let second = GameInput::Host(HostInput {
        basis: fixture_model::basis(),
        operation: OperationId::from_bytes(&[85; 16]).unwrap(),
        host: fixture_model::member(3),
        command: HostCommand::RequestCheckpoint,
    });
    assert!(sender.try_submit(first.clone()).is_ok());
    assert!(sender.try_submit(second.clone()).is_ok());

    // This owner flag is a test condition only; current canonical Session state has
    // no selected pause policy field or timer callback admission port.
    let started_at = Instant::now();
    let deadline = started_at + Duration::from_millis(5);
    let mut owner = PausedOwner {
        sender: sender.clone(),
        checkpoint: current.clone(),
        paused_fixture: true,
        wake_at: Some(deadline),
        wake_count: 0,
        observed_deadline: None,
        inputs: Vec::new(),
    };
    let outcome = actor
        .run_with_owner_wake(
            &mut owner,
            |owner| {
                assert!(owner.sender.usage().is_ok());
                owner.wake_at
            },
            |owner| {
                owner.wake_count += 1;
                owner.observed_deadline = Some(Instant::now());
                owner.wake_at = None;
                // A scheduler wake is not submitted as GameInput::Timer or Job.
                owner.sender.stop().unwrap();
            },
        )
        .unwrap();

    assert_eq!(owner.wake_count, 1);
    assert!(owner.observed_deadline.unwrap() >= deadline);
    assert!(
        owner.observed_deadline.unwrap().duration_since(started_at) >= Duration::from_millis(5)
    );
    assert!(owner.paused_fixture);
    assert_eq!(
        before,
        LogicalTime {
            ticks: 120,
            ticks_per_second: 10
        }
    );
    assert_eq!(owner.checkpoint.state().logical_time, before);
    assert_eq!(owner.checkpoint.state().timers[0], due_timer);
    assert!(due_timer.due.ticks > before.ticks);
    assert_eq!(
        owner.inputs,
        vec![
            (AdmissionSequence(1), first),
            (AdmissionSequence(2), second)
        ]
    );
    assert_eq!(outcome.reduced_inputs, 2);
    assert!(!sender.usage().unwrap().accepting);
}

#[test]
fn stop_notification_cancels_future_wake_and_drains_admitted_input() {
    let (sender, actor) = bounded_inbox::<GameInput>();
    let current = checkpoint(timer_state(ExecutionMode::Replay));
    let before = current.state().logical_time;
    let entered = Arc::new(Barrier::new(2));
    let resume = Arc::new(Barrier::new(2));
    let (owner_entered, owner_resume) = (entered.clone(), resume.clone());
    let already_waited = Arc::new(AtomicBool::new(false));
    let check_once = already_waited.clone();
    let producer = sender.clone();
    let input = fixture_model::action();
    let mut owner = PausedOwner {
        sender,
        checkpoint: current,
        paused_fixture: true,
        wake_at: Some(Instant::now() + Duration::from_secs(3600)),
        wake_count: 0,
        observed_deadline: None,
        inputs: Vec::new(),
    };
    let thread = std::thread::spawn(move || {
        let outcome = actor
            .run_with_owner_wake(
                &mut owner,
                |owner| {
                    if !check_once.swap(true, Ordering::SeqCst) {
                        owner_entered.wait();
                        owner_resume.wait();
                    }
                    owner.wake_at
                },
                |_| panic!("stop notification cancels a future owner wake"),
            )
            .unwrap();
        (owner, outcome)
    });

    entered.wait();
    assert!(producer.try_submit(input.clone()).is_ok());
    producer.stop().unwrap();
    resume.wait();
    let (owner, outcome) = thread.join().unwrap();

    assert_eq!(owner.wake_count, 0);
    assert!(owner.observed_deadline.is_none());
    assert!(owner.paused_fixture);
    assert_eq!(owner.checkpoint.state().logical_time, before);
    assert_eq!(owner.inputs, vec![(AdmissionSequence(1), input)]);
    assert_eq!(outcome.reduced_inputs, 1);
    assert!(!producer.usage().unwrap().accepting);
}

#[test]
fn checkpoint_retains_timer_identity_generation_due_source_status_and_execution_modes() {
    for mode in [
        ExecutionMode::Live,
        ExecutionMode::PreparedOnly,
        ExecutionMode::Replay,
    ] {
        let canonical = checkpoint(timer_state(mode));
        assert_eq!(canonical.state().mode, mode);
        assert_eq!(canonical.state().timers.len(), 2);
        let first = &canonical.state().timers[0];
        let second = &canonical.state().timers[1];
        assert_ne!(first.id, second.id);
        assert_eq!(first.generation, second.generation);
        assert_eq!(first.basis, canonical.basis());
        assert_eq!(first.due.ticks, 130);
        assert_eq!(first.source, fixture_model::rule());
        assert_eq!(first.status, DurableStatus::Pending);
        assert_eq!(second.due.ticks, 140);
        assert_eq!(second.status, DurableStatus::Cancelled);

        let intent = &canonical.state().intents[0];
        assert_eq!(intent.basis, canonical.basis());
        assert_eq!(
            intent.operation,
            OperationId::from_bytes(&[84; 16]).unwrap()
        );
        assert_eq!(intent.job, None);
        assert_eq!(intent.timer, Some(first.id));
        assert_eq!(intent.generation, first.generation);
        assert_eq!(intent.status, DurableStatus::Pending);
    }
}

#[test]
fn checkpoint_rejects_invalid_timer_generation_time_unit_and_stale_basis() {
    let mut invalid_generation = timer_state(ExecutionMode::Replay);
    invalid_generation.timers[0].generation = 0;
    assert!(matches!(
        fixture_model::checkpoint(invalid_generation),
        Err(CheckpointError::InvalidIntent)
    ));

    let mut invalid_intent_generation = timer_state(ExecutionMode::Replay);
    invalid_intent_generation.intents[0].generation = 0;
    assert!(matches!(
        fixture_model::checkpoint(invalid_intent_generation),
        Err(CheckpointError::InvalidIntent)
    ));

    let mut invalid_time = timer_state(ExecutionMode::Replay);
    invalid_time.logical_time.ticks_per_second = 0;
    assert!(matches!(
        fixture_model::checkpoint(invalid_time),
        Err(CheckpointError::InvalidTime)
    ));

    let current = checkpoint(timer_state(ExecutionMode::Replay));
    let mut stale_basis = current.basis();
    stale_basis.revision = fixture_model::revision(2, 9);
    assert!(matches!(
        current.validate_resume(stale_basis, &fixture_model::pins()),
        Err(CheckpointError::StaleBasis)
    ));
}

#[test]
fn client_command_validator_refuses_timer_and_job_completion_inputs() {
    let current = checkpoint(timer_state(ExecutionMode::Live));
    let timer = GameInput::Timer(TimerExpiry {
        basis: current.basis(),
        timer: current.state().timers[0].id,
        generation: current.state().timers[0].generation,
        observed_time: current.state().logical_time,
    });
    let job = GameInput::Job(JobCompletion {
        basis: current.basis(),
        operation: OperationId::from_bytes(&[86; 16]).unwrap(),
        job: JobId::from_bytes(&[87; 16]).unwrap(),
        generation: 4,
        outcome: JobOutcome::Failed(NativeFailure::Unavailable),
    });

    assert_client_input_refused(&current, &timer);
    assert_client_input_refused(&current, &job);
    assert_eq!(current.state().logical_time.ticks, 120);
    assert_eq!(current.state().timers[0].status, DurableStatus::Pending);
}

#[test]
fn saved_contract_is_byte_equal_to_the_rust_literal() {
    assert_eq!(include_str!("timer_clock_contract.json"), CONTRACT_LITERAL);
}
