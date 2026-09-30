# ADR 0003: Agent-written SQLite development log

Date: 2026-09-29
Status: Planning database/devlog provisioned; scoped runner implementation pending

## Context

Workers encounter unclear instructions, conflicting contracts, unexpected errors,
tool limitations, and difficult implementation choices. Final handoffs alone
lose useful context, especially when an attempt fails or is abandoned.

This devlog stays in the SQLite development workflow database. Runtime OTEL logs
and correlated spans live in a separate SQLite database agents query read-only;
PostgreSQL owns durable gameplay data. See
[Storage architecture](../planning/storage-architecture.md).

Keep this evidence so future process changes can target recurring problems and
measure whether task briefs, model routing, reviews, and tooling improve.

## Decision

Add an append-only `devlog` table to the workflow SQLite database. All spawned
workers, evaluators, and cleanup agents can record their own observations during
work, without waiting for the coordinator to transcribe a final report.

### What to record

Record meaningful confusion, conflicting requirements, unexpected errors,
defects, blockers, challenges, and discoveries. Include failures and unsuccessful
approaches, not only successful resolutions. Skip routine progress narration
and repeated copies of the same error.

Write an entry when an issue becomes clear, then append a linked outcome entry
after resolving or abandoning it. Before handoff, check that significant issues
and their known outcomes have been recorded. A clean attempt requires no invented
devlog entries.

### Minimal entry structure

| Field | Purpose |
| --- | --- |
| `id` | Caller-generated unique ID; retrying a write does not duplicate it |
| `created_at` | Timestamp assigned by the write command, in UTC |
| `agent_id`, `role` | Identity and role supplied by the agent's execution context |
| `task_id`, `attempt_id` | Links to work; nullable for observations outside an attempt |
| `kind` | `confusion`, `error`, `defect`, `blocker`, `challenge`, `discovery`, or `resolution` |
| `summary` | Short, searchable description |
| `details` | What happened, what was expected, and relevant context |
| `action` | What the agent tried, or why it could not proceed |
| `outcome` | Observed result; explicitly unknown when unresolved |
| `suggestion` | Optional concrete improvement to instructions, tooling, or routing |
| `related_entry_id` | Optional link to an earlier observation or resolution |
| `evidence_ref` | Optional durable log, test result, file/line, or commit reference |

Keep model, effort, and execution-brief revision on the attempt record and join
them during analysis. Preserve the brief used for each attempt so subsequent
edits do not erase the instructions that produced a problem.

Separate observations from hypotheses in the entry text. Agents must not invent
a root cause, claim a fix worked without verification, or present an estimate of
impact as a measured result. Keep entries concise and specific, usually 50–200
words; attach long output by reference.

### Write access and reliability

Provide one small devlog write command shared by agent runners. It validates the
entry, supplies trusted identity, and performs a short parameterized SQLite
insert. Bind task and attempt references to the agent's assigned context.

Workers may append entries; they cannot edit other entries or mutate queue state.
Corrections and resolutions are new rows linked to the original. Accept late
observations from known superseded attempts as historical evidence, while keeping
their submissions ineligible to complete the task.

Use the workflow's WAL database once provisioned and bounded retries for transient
write contention.
If the database remains unavailable, preserve the sanitized entry with the same
ID in durable attempt output and report the failure for later ingestion. Do not
silently lose observations or block development indefinitely on logging.

Index task/attempt references, timestamps, and kind. The devlog does not need a
separate logging service, search engine, or extra lifecycle state.

### Evidence and later process refinement

Keep credentials, personal data, and full private prompts out of entries. Evidence
references must point to retained output; the cleanup worker preserves these
files under ADR 0002. The coordinator's changelog remains a record of integrated
commits, while the devlog captures the problems encountered during development.

Runtime investigations may reference stable telemetry record/trace IDs and their
tested build. Pin or export cited evidence before normal telemetry retention
expires; querying a live log database does not make every result permanently
retained. Devlog authors do not change telemetry rows or its retention policy.

Periodically group entries by task type, model, brief revision, and problem kind.
Look for repeated ambiguity, environment failures, repair loops, and defects
missed by review. Count affected attempts, not raw entry totals, and compare with
all attempts so more verbose models do not appear less capable merely because
they log more observations.

Turn recurring findings into proposed brief changes, ADR updates, or improvement
tasks. Validate process changes against representative work before making them
the default. Agent observations are evidence for investigation, not automatically
verified conclusions or training-ready examples.

When investigating runtime defects, identify the tested commit/build, configuration,
relevant asset revision, and reproduction path in the details or retained evidence.
Distinguish a helper passing from the integrated behavior working, and a proposed
fix from a verified resolution. If the original symptom persists, append that
outcome and retain the unresolved gap rather than presenting the first patch as
completion. These distinctions repeatedly mattered in the archived devlog.

## Consequences

Subagents can capture issues while the details are fresh. Task and attempt links
make those observations useful for later analysis without adding another agent
role or changing who approves work.

The devlog table now exists in the authorized planning database described in
[Plan database](../planning/plan-database.md). Planning refinement observations use
it; the trusted scoped write command and Rust agent runner remain future work.
