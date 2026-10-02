# G06-D03: Spool lag and readiness policy

Task: `B-G06-D03` (`G06:D:Define spool lag and readiness thresholds`)
Attempt: `B-G06-D03-a1`
Input revision: `308920e328ba6df85bea1e3d2abcbea5f1b796e6`
Owners: `df-observe`, `df-telemetry`; composition owner: `df-server`
Status: proposed policy decision; thresholds and recovery behavior remain to be measured and qualified.

## Decision

Apply the existing per-instance native SQLite/spool topology from
[G06-D01](g06-d01-decision.md). Preserve its one writer and distinct spool,
telemetry SQLite, workflow SQLite, PostgreSQL gameplay, and media failure
domains. This document sets the candidate policy for the existing
`TelemetryIngress`/receipt, `df-telemetry` ingestion-health, and `df-server`
readiness boundaries. It does not create a second pipeline or API.

Use the existing hosted operations proposal as the initial candidate envelope:

| Condition | Proposed policy | Meaning |
| --- | --- | --- |
| Oldest uncommitted spool record age exceeds 30 seconds | Warn and surface degraded ingestion health | The producer/collector is falling behind; record age and queue depth remain visible. |
| Oldest uncommitted record age exceeds 120 seconds | Degrade readiness for new campaign and provider admissions | Stop creating new admitted work whose operation requires these admission gates. Existing admitted gameplay continues toward a safe checkpoint. |
| Native spool reaches its proposed 2 GiB per-instance cap, or cannot durably accept a record | Report the source sequence/time gap and loss explicitly, preserve the priority failure diagnostic through the independent emergency stderr path, and stop new high-volume admissions | Do not acknowledge a record as durably spooled when it was not. Continue already-admitted gameplay toward its safe checkpoint. |
| SQLite commit fails or is delayed | Keep spool checkpoint unchanged; retry with stable record identities | A spool-accepted receipt is not a SQLite-committed receipt. |

These values are copied from `planning/service-operations.md`; they are proposed
starting limits, not measurements, SLO evidence, an SLA, or a supported maximum
outage. They apply to the native per-instance spool. Browser/WASM batches remain
best-effort bounded uploads and do not inherit native crash-durable guarantees.
No age threshold alone discards data. Capacity exhaustion can cause explicit
loss if input continues; report exact missing source ranges/watermarks and never
claim complete capture. Recovery replays stable IDs, deduplicates committed
records, and advances the durable spool checkpoint only after SQLite commit.

An admitted action/job is not cancelled because telemetry becomes unhealthy or
an RPC wait ends. “Safe checkpoint” means finish the currently admitted
authoritative game operation under normal ownership and durability rules, make
its terminal game outcome recoverable through the gameplay store, then refuse
subsequent admission while the gate remains degraded. Telemetry persistence must
not be a prerequisite for committing gameplay state. Do not start a new optional
provider dispatch after the gate closes. This preserves the separation between
PostgreSQL game authority and diagnostic SQLite. The exact admission unit and
which provider dispatches are mandatory for an already-admitted action must be
bound by the owning `df-session`/`df-server` contracts; this policy does not
invent an actor API or alter existing durable-job cancellation semantics.

## Alternatives and rationale

* **Block every game commit until telemetry reaches SQLite:** reject. It makes a
  diagnostic sink a gameplay availability dependency and contradicts the
  explicit requirement that telemetry outages do not stall gameplay.
* **Keep admitting indefinitely and drop silently after a bounded queue fills:**
  reject. It hides missing diagnostic evidence and can falsely imply full
  capture. The explicit gap/loss path and admission degradation are required.
* **Let already-admitted work continue to its safe checkpoint while closing new
  admission at a sustained lag/capacity boundary:** select. It matches the
  existing service-operations thresholds, preserves durable gameplay authority,
  and exposes diagnostic uncertainty.
* **Treat 30s/120s/2GiB as proven operating limits:** reject. No current load,
  outage, disk-pressure, crash/restart, or recovery benchmark establishes them.
  Keep them visibly proposed until qualification establishes service-specific
  bounds.

## Source-backed authority and integration

`planning/observability.md` requires bounded async batches, durable spooling,
stable-ID retries, acknowledgement only after durable acceptance, checkpoint
advance only after SQLite commit, visible queue age/depth/gaps/drops, explicit
disk-full/crash behavior, and responsive gameplay under telemetry failure.
`planning/service-operations.md` supplies the proposed 30s warning, 120s
admission-readiness degradation, 2GiB spool, and explicit hard-full behavior.
`planning/subsystem-interfaces.md` assigns `TelemetryIngress` and shared
instrumentation to `df-observe`, ingestion/spool/SQLite/health to `df-telemetry`,
and wiring/readiness/shutdown to `df-server`. `planning/otel-instance-topology.md`
selects the per-native-instance writer/spool and leaves browser durable storage
unassumed. `planning/storage-architecture.md` keeps SQLite telemetry distinct
from PostgreSQL gameplay and the development workflow database.

The exact failure semantics are:

1. `df-observe` producer/export paths remain bounded and do not block input or
   rendering; classify the capture boundary and preserve stable record IDs.
2. `df-telemetry` returns distinct spooled and SQLite-committed receipts. It
   does not acknowledge durable spool acceptance before the bounded local write
   succeeds, and it does not advance the checkpoint before SQLite commit.
3. `df-telemetry` reports lag, queue depth, last commit, retry/reject/drop
   counts, gaps and watermarks independently of the failing exporter. Hard-full
   reporting uses the agreed independent emergency path.
4. `df-server` surfaces telemetry degradation separately from core readiness
   requirements and closes new campaign/provider admissions according to the
   proposed threshold. An already-admitted game operation reaches its safe
   authoritative checkpoint; failure to commit gameplay remains a gameplay
   storage failure and is handled by the existing gameplay contract.
5. `df-auth` remains authority for operator query/export access. Correlation or
   producer IDs do not grant access. Read-only review returns gaps and retention
   watermarks with results.

## Finite contract example

This standard-library-only model makes the proposed thresholds explicit and
exercises normal operation, lag refusal, hard-full refusal, and a safe admitted
checkpoint. The constants encode proposals from the existing plan; the example
is not a measurement, production API, filesystem/SQLite test, or runtime proof.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TelemetryHealth {
    Healthy,
    Warn,
    Degraded,
    HardFull,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Admission {
    AcceptNew,
    RefuseNew,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Checkpoint {
    ContinueAdmitted,
    FinishCurrentThenStop,
    NoAdmittedWork,
}

const WARN_AFTER_SECONDS: u64 = 30;
const DEGRADE_AFTER_SECONDS: u64 = 120;
const PROPOSED_SPOOL_CAP_BYTES: u64 = 2 * 1024 * 1024 * 1024;

fn health(oldest_uncommitted_seconds: u64, spool_bytes: u64) -> TelemetryHealth {
    if spool_bytes >= PROPOSED_SPOOL_CAP_BYTES {
        TelemetryHealth::HardFull
    } else if oldest_uncommitted_seconds > DEGRADE_AFTER_SECONDS {
        TelemetryHealth::Degraded
    } else if oldest_uncommitted_seconds > WARN_AFTER_SECONDS {
        TelemetryHealth::Warn
    } else {
        TelemetryHealth::Healthy
    }
}

fn admission(health: TelemetryHealth) -> Admission {
    match health {
        TelemetryHealth::Healthy | TelemetryHealth::Warn => Admission::AcceptNew,
        TelemetryHealth::Degraded | TelemetryHealth::HardFull => Admission::RefuseNew,
    }
}

fn checkpoint(health: TelemetryHealth, already_admitted: bool) -> Checkpoint {
    if !already_admitted {
        return Checkpoint::NoAdmittedWork;
    }
    if matches!(
        health,
        TelemetryHealth::Degraded | TelemetryHealth::HardFull
    ) {
        Checkpoint::FinishCurrentThenStop
    } else {
        Checkpoint::ContinueAdmitted
    }
}

fn main() {
    assert_eq!(health(30, 0), TelemetryHealth::Healthy);
    assert_eq!(health(31, 0), TelemetryHealth::Warn);
    assert_eq!(admission(health(120, 0)), Admission::AcceptNew);
    assert_eq!(health(121, 0), TelemetryHealth::Degraded);
    assert_eq!(admission(health(121, 0)), Admission::RefuseNew);
    assert_eq!(
        checkpoint(health(121, 0), true),
        Checkpoint::FinishCurrentThenStop
    );
    assert_eq!(
        health(0, PROPOSED_SPOOL_CAP_BYTES),
        TelemetryHealth::HardFull
    );
    assert_eq!(admission(TelemetryHealth::HardFull), Admission::RefuseNew);
    assert_eq!(
        checkpoint(TelemetryHealth::HardFull, true),
        Checkpoint::FinishCurrentThenStop
    );
    assert_eq!(
        checkpoint(TelemetryHealth::HardFull, false),
        Checkpoint::NoAdmittedWork
    );
}
```

## Unresolved production gates

* Measure event rate/size distribution, burst absorption, fsync latency, spool
  write/SQLite commit throughput, query contention, restart replay time, and
  disk-full behavior on the selected native filesystem and deployment class.
* Select and test maximum batch/record sizes, memory queues, spool segment
  format, retention/quota and reserved space for priority failure diagnostics.
  The current 2GiB cap is not enough to infer a supported outage duration.
* Define precise age origin (event time versus local enqueue/oldest durable
  uncommitted time), clock-jump handling, hysteresis/recovery-open behavior, and
  whether warning/lag/capacity thresholds are process-wide or per producer.
* Demonstrate crash boundaries before spool acceptance, after acceptance/before
  SQLite commit, and after commit/before checkpoint update; prove deduplication,
  visible source gaps and no false committed receipt.
* Verify the selected Rust OTEL SDK/exporter and native/WASM capture paths,
  upload receipt behavior, authentication, and end-to-end OTLP-to-SQLite field
  mapping. Phone and other physical-device testing is deferred; browser support
  on untested devices remains unverified.
* Bind the safe-checkpoint admission unit to the real session/job lifecycle and
  show that PostgreSQL/gameplay failures remain distinct from telemetry
  failures. No application source exists in this task's scope, and this example
  does not prove integrated or running behavior.
* Establish load-qualified controlled-launch SLO/RPO/RTO and recovery evidence
  before describing these proposals as operational promises.

## Evidence and input identity

Governing inputs are the exact paths and hashes in
`development/evidence/fanout-20261001/wave-02/B-G06-D03/brief.json` and
`original-task.json`; the issued source revision is recorded above. The worker
must retain fresh hash output, exact formatter/compiler/fixture receipts,
literal-source hash, commit, and devlog entry in the task evidence root and
`handoff.json`. The shared wave guard is
`development/evidence/fanout-20261001/wave-02/guarded-design-command-v3.py`.

## Verification boundary

The Rust example is a finite policy contract only. Native/WASM workspace checks,
SQLite/spool recovery, real admission wiring, load/recovery measurement, and
browser/device observation are unperformed in this design task. Independent
frontier review and root integration remain required.

For attempt `B-G06-D03-a1`, the final extracted literal compiled with pinned
Rust 1.98.1 (`rustc --edition 2024 -D warnings`) and its finite assertions
exited successfully; guarded receipts are retained as
`rustc-correction-01.json` and `fixture-correction-01.json` in the task
evidence root. The final `rustfmt --check` was held before launch when the fresh
free-memory proxy was 38%; its receipt is `rustfmt-correction-03.json`. An
earlier formatter diff was corrected in this source, but the final formatter
result remains unverified. These checks establish only the standalone contract,
not integrated or runtime behavior.
