# Task admission and memory lease policy

Date: 2026-09-30  
Task/attempt: `B-G09-D02/a1`  
Input revision: `4ba37943691f3a01a49b8238983e82527bec91bf`  
Status: bounded policy decision and executable contract example; production runner remains pending

## Decision

`df-workflow` remains the single authority for task identity, edit-area ownership,
attempt phase, lease renewal, submission, evaluation, and sequential integration.
Reuse its existing task/attempt rows and lease lifecycle. Do not create a second
lease authority, lock service, table, or competing canonical implementation.
The current planning database was queried through SQLite `mode=ro` at the input
revision: it contains the five documented tables (`features`, `tasks`,
`dependencies`, `attempts`, `devlog`) and 29 triggers. That verifies the current
planning database shape only. It does not establish a trusted Rust runner or
prove that an external worker process has stopped.

Admission is a coordinator decision made before dispatch. Compute worker capacity
from *currently available* memory, not installed physical RAM:

```text
worker_budget = max(0,
    available_memory
    - system_headroom
    - frontier_review_reserve
    - integration_build_reserve)
memory_slots = floor(worker_budget / observed_worker_peak)
admitted = min(memory_slots, ready_nonconflicting_tasks,
               configured_model_or_provider_concurrency)
```

All quantities use bytes internally. `observed_worker_peak` includes the agent
and its owned children, measured as a process tree for the relevant work class;
the estimate must cover that class's native tools, browser or test processes and
temporary build activity where applicable. A missing or stale class measurement
is uncertainty: use the conservative one-worker startup allowance and observe
that run before increasing concurrency. If its peak cannot fit while preserving
reserves, admit zero. Never translate this rule into a fixed `128`-worker
promise.

Keep the reserves separate so concurrent review and integration remain possible.
The current 16 GiB host evidence uses 3.2 GiB system headroom (the ADR 0002
initial 20% setting), a 2 GiB frontier/review reserve, and a 2 GiB
integration/build reserve. The latter two are starting estimates, not device
guarantees. Existing evidence records a 0.3 GiB worker peak estimate for a
bounded native planning/contract worker, but this is not a representative browser,
vision, audio, local-model, or full-build measurement. Measure those separately
before admitting them into the corresponding pool. `memory_pressure`, sustained
swap, disk pressure, provider limits, ready work, edit-area conflicts, and review
backlog can all further reduce admission to zero; more free RAM alone cannot
override them. These settings and facts are retained in
`development/evidence/dependency-g01/dispatch-resource-evidence.json` and
`development/evidence/contracts-g03/dispatch-resource-evidence.json`.

The coordinator grants one task/attempt ownership claim for each overlapping edit
area through implementation, review, and integration. Source work uses that
attempt's isolated worktree. The coordinator integrates approved revisions one
at a time. Another worker must not edit or submit against an overlapping area
while the current owner still has a valid claim.

## Exact lease fence and expiry behavior

Each action that can affect the attempt (renewal, submission, evaluator verdict,
integration result, or owned-resource release) must match the current authoritative
attempt and phase snapshot on all of these values:

1. task ID and attempt ID;
2. current phase (`implementation`, `review`, or `integration` as applicable);
3. current lease owner bound to the trusted runner's principal and process
   identity;
4. current opaque lease token, compared without logging or exposing it;
5. current monotonically increasing ownership generation; and
6. an unexpired deadline according to the coordinator's injected clock
   (`now < expires_at`).

The coordinator performs the comparison and state change atomically against the
current attempt record. A successful phase/owner transfer rotates the token and
increments the generation. An old token, owner, generation, phase, attempt, or
expired deadline is a typed rejection, even if the old process later returns a
plausible result. These fences make stale work inadmissible; they do not kill a
process or prove its identity. The trusted runner must bind the principal/process
identity at dispatch and confirm its owned processes have exited or been stopped
before that process's edit/resource claim is released or the edit area is
reassigned.

Lease expiry starts recovery; it is not worker death and does not authorize
duplicate dispatch, cleanup, or reuse of edit areas. Preserve the candidate if a
valid submission is already in review and only the evaluator lease expired.
Otherwise, fence the old owner immediately; inspect tracked owned processes and
confirm their exit, or keep the task/edit area blocked pending a supervisor
decision. After termination is confirmed, recovery reclaims with a new token and
generation. Reject all later output from the superseded owner. Never infer old
worker death from SQLite state, a timer, missing heartbeat, or a model call
timeout. Artifact cleanup follows ADR 0002's stronger ownership/use/age checks.

## Current boundary and known gaps

ADR 0001 specifies five tables and attempt fields for phase, lease owner, and
expiry; it also requires fencing the old agent and confirming owned-process
shutdown before reclaim. ADR 0002 requires memory-aware dispatch and reserves
capacity for frontier review/integration; ADR 0004 requires a bounded, source-bound
handoff; ADR 0005 requires an independent frontier evaluator. Current design
sources assign these workflow hooks to `df-workflow` and the coordinator.

The current SQLite schema/count is not process identity, a process tree, current
memory, token secrecy, or a compare-and-swap implementation. No claim is made
here that Rust workflow code, trusted process supervision, model/tool availability,
or browser/vision/audio observation has been implemented or qualified. Exact
production types and persistence details remain the applicable G03/G09 and later
wave-specific contracts. Do not add schema fields or public Rust contracts in
this policy task; the documented fence is the required behavior for those
contracts to implement.

Alternatives rejected: a fixed worker count ignores workload peaks and reserves;
counting only the agent process misses owned browser/build children; expiry-as-
death permits duplicate edit ownership and unsafe artifact deletion; a second
lease/lock store conflicts with the canonical workflow owner. The exact future
trusted process-identity and OS termination adapter are unresolved G09 runner
design facts, not inferred from this example.

## Bounded deterministic Rust contract example

This dependency-free example is a policy proof only. It demonstrates byte-based
bounded admission and rejects a stale submission when any phase/token/generation/
owner/attempt/deadline fence is wrong. It is not connected to SQLite, a coordinator,
a process supervisor, or the application.

```rust
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Implementation,
    Review,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RejectReason {
    WrongAttempt,
    WrongPhase,
    WrongOwner,
    WrongProcess,
    WrongToken,
    WrongGeneration,
    Expired,
}

#[derive(Clone, Copy, Debug)]
struct LeaseFence {
    attempt_id: u64,
    phase: Phase,
    owner_id: u64,
    runner_process_id: u64,
    token: u64,
    generation: u64,
    expires_at: Duration,
}

#[derive(Clone, Copy, Debug)]
struct Submission {
    attempt_id: u64,
    phase: Phase,
    owner_id: u64,
    runner_process_id: u64,
    token: u64,
    generation: u64,
}

fn check_submission(
    current: LeaseFence,
    submission: Submission,
    now: Duration,
) -> Result<(), RejectReason> {
    if submission.attempt_id != current.attempt_id {
        return Err(RejectReason::WrongAttempt);
    }
    if submission.phase != current.phase {
        return Err(RejectReason::WrongPhase);
    }
    if submission.owner_id != current.owner_id {
        return Err(RejectReason::WrongOwner);
    }
    if submission.runner_process_id != current.runner_process_id {
        return Err(RejectReason::WrongProcess);
    }
    if submission.token != current.token {
        return Err(RejectReason::WrongToken);
    }
    if submission.generation != current.generation {
        return Err(RejectReason::WrongGeneration);
    }
    if now >= current.expires_at {
        return Err(RejectReason::Expired);
    }
    Ok(())
}

fn memory_slots(
    available_bytes: u64,
    system_headroom_bytes: u64,
    frontier_review_bytes: u64,
    integration_build_bytes: u64,
    worker_peak_bytes: u64,
    ready_nonconflicting: usize,
    provider_limit: usize,
) -> usize {
    if worker_peak_bytes == 0 {
        return 0;
    }
    let reserved_bytes = system_headroom_bytes
        .saturating_add(frontier_review_bytes)
        .saturating_add(integration_build_bytes);
    let worker_budget = available_bytes.saturating_sub(reserved_bytes);
    let by_memory = (worker_budget / worker_peak_bytes) as usize;
    by_memory.min(ready_nonconflicting).min(provider_limit)
}

fn main() {
    let available = 12 * 1024 * 1024 * 1024_u64;
    let gib = 1024 * 1024 * 1024_u64;
    assert_eq!(
        memory_slots(available, 3 * gib, 2 * gib, 2 * gib, gib, 8, 5),
        5
    );
    assert_eq!(memory_slots(6 * gib, 4 * gib, 2 * gib, 1, gib, 8, 5), 0);

    let lease = LeaseFence {
        attempt_id: 41,
        phase: Phase::Implementation,
        owner_id: 7,
        runner_process_id: 700,
        token: 9001,
        generation: 3,
        expires_at: Duration::from_secs(100),
    };
    let current_submission = Submission {
        attempt_id: 41,
        phase: Phase::Implementation,
        owner_id: 7,
        runner_process_id: 700,
        token: 9001,
        generation: 3,
    };
    assert_eq!(
        check_submission(lease, current_submission, Duration::from_secs(99)),
        Ok(())
    );
    assert_eq!(
        check_submission(lease, current_submission, Duration::from_secs(100)),
        Err(RejectReason::Expired)
    );
    let stale_token = Submission {
        token: 9000,
        ..current_submission
    };
    assert_eq!(
        check_submission(lease, stale_token, Duration::from_secs(99)),
        Err(RejectReason::WrongToken)
    );
    let stale_generation = Submission {
        generation: 2,
        ..current_submission
    };
    assert_eq!(
        check_submission(lease, stale_generation, Duration::from_secs(99)),
        Err(RejectReason::WrongGeneration)
    );
    let wrong_phase = Submission {
        phase: Phase::Review,
        ..current_submission
    };
    assert_eq!(
        check_submission(lease, wrong_phase, Duration::from_secs(99)),
        Err(RejectReason::WrongPhase)
    );
    let wrong_owner = Submission {
        owner_id: 8,
        ..current_submission
    };
    assert_eq!(
        check_submission(lease, wrong_owner, Duration::from_secs(99)),
        Err(RejectReason::WrongOwner)
    );
    let wrong_process = Submission {
        runner_process_id: 701,
        ..current_submission
    };
    assert_eq!(
        check_submission(lease, wrong_process, Duration::from_secs(99)),
        Err(RejectReason::WrongProcess)
    );
    let wrong_attempt = Submission {
        attempt_id: 42,
        ..current_submission
    };
    assert_eq!(
        check_submission(lease, wrong_attempt, Duration::from_secs(99)),
        Err(RejectReason::WrongAttempt)
    );
    println!("bounded admission and seven stale-submission fences: passed");
}
```

## Acceptance and verification record

The original acceptance criteria are preserved verbatim:

1. bounded agent admission
2. The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.

The original verification statements are preserved verbatim:

1. Freeze the cited source decision and a bounded contract example; compare bounded agent admission. Retain decision, alternatives and unresolved facts.
2. Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.

This attempt adds the specifically authorized bounded decision-policy example
command in its retained worker evidence. Its compiler run does not replace the
original planned application, browser, provider, actual runner, independent
frontier, or integrated acceptance gates. Those remain pending/unperformed as
listed in `development/evidence/parallel-decisions/B-G09-D02/a1/worker/`.
