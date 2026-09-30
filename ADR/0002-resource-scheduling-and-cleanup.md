# ADR 0002: Resource-aware agent scheduling and artifact cleanup

Date: 2026-09-29
Status: Accepted strategy; implementation pending

## Context

The coordinator should keep as many useful agents working as the machine can
support. Worker processes, builds, tests, and browser sessions consume memory
and disk. Unbounded dispatch can reduce throughput through memory pressure,
while stale build output wastes disk and can hide verification failures.

## Decision

### Dispatch according to available resources

Spawn as many agents as memory can afford, subject to ready tasks, independent
edit areas, provider and runtime limits, and review capacity. A large backlog
does not justify starting workers that cannot make useful progress.

Measure worker memory together with its child processes. Use an observed peak
for the relevant workload, including builds, tests, and browsers, rather than
counting only the agent process. Remote model inference does not consume local
RAM in the same way as a locally hosted model.

Before dispatching additional workers, estimate:

```text
additional slots = floor(
  (available memory - system headroom - reserved build/review memory)
  / estimated peak memory per additional worker
)
```

Clamp the result to zero and to the number of ready, non-conflicting tasks.
Start conservatively when no measurements exist. Make the reserve configurable;
20% of physical RAM is an initial system-headroom setting to tune from evidence.
Reserve capacity for evaluation and integration so workers cannot starve the
process that completes their tasks.
Include frontier evaluator slots, actual computer-use/vision capability, and
browser/multi-client memory in this reserve under
[ADR 0005](0005-frontier-output-evaluation.md). Throttle implementation dispatch
when review is saturated; do not downgrade the required output evaluation.

Increase concurrency gradually while accepted completions improve. Stop new
dispatch when memory pressure, sustained swapping, disk exhaustion, or a growing
review queue shows that the current workload is already saturated. Let existing
work finish where possible; do not kill unrelated processes to free resources.
CPU and disk contention also constrain useful concurrency.

Resource checks belong in the coordinator's ordinary code. They do not require
a frontier-model call on each dispatch.

### Clean stale artifacts every 30 minutes

During an active development run, the coordinator dispatches one lightweight
cleanup worker every 30 minutes. Use a Luna-class worker, with a deterministic
inventory and deletion script doing the routine filesystem work. Cleanup is an
auxiliary worker responsibility, not a fourth orchestration role.

Only one cleanup attempt may run at a time. Coalesce missed intervals into one
run after recovery. The cleanup worker uses the same tracked attempt and lease
mechanism as other workers and records paths removed, bytes reclaimed, and any
uncertain candidates it retained.

Workers also remove their own superseded temporary build output after successful
handoff. The periodic worker catches leftovers from completed or abandoned work.

### Define stale output by ownership and use

All project-controlled caches, generated output, and temporary staging files that
do not need to be saved in Git belong under the gitignored `artifacts/` folder.
Use `artifacts/cache/` for caches, `artifacts/build/` for builds, and
`artifacts/tmp/` for scratch and staging output. Identify per-attempt output by
its attempt ID and keep a small manifest containing its owner, source commit,
and purpose. Initially, only build and temporary-output roots are eligible for
periodic cleanup; shared caches remain subject to a separate retention policy.

Output is eligible for deletion only when all of the following hold:

- It is inside an approved generated-output root.
- Its owning attempt is terminal or confirmed abandoned; lease expiry alone
  does not prove that an old process has stopped using its files.
- No worker, evaluator, integrator, or running preview holds an active use claim.
- It is superseded or reproducible and is not retained verification evidence.
- At least 30 minutes have passed since the attempt became terminal or was
  confirmed abandoned.

Age alone does not establish staleness. Recheck ownership and use immediately
before deleting, under the coordinator's cleanup claim. New users of an artifact
must respect that claim. Retain unknown or ambiguous files for investigation.

Never delete source files, Git data, SQLite databases or their WAL/SHM files,
user deliverables, durable evidence referenced by attempts, active runtime data,
or the latest verified build. Do not traverse symlinks or include `.old` archives.
Shared dependency and compiler caches need a separate retention policy; the
initial cleanup worker does not clear them.

## Consequences

Concurrency grows with measured capacity, while evaluation and integration keep
enough resources to finish work. Periodic cleanup remains inexpensive and
recoverable through the existing coordinator and attempt records.

This ADR specifies future coordinator behavior. It does not create a desktop
automation or start a cleanup process before the coordinator is implemented.
