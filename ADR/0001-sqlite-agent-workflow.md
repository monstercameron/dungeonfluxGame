# ADR 0001: SQLite-backed development workflow

Date: 2026-09-29
Status: Planning database provisioned; Rust coordinator implementation pending

## Context

Development plans produce atomic tasks that workers implement and independent
evaluators review. A Markdown todo list becomes difficult to manage as the
backlog grows to thousands of features and many more tasks.

The workflow must support parallel work, retries, dependencies, and reliable
completion while remaining small enough to understand and operate.

## Decision

Use SQLite as the authoritative work queue, with five tables, three roles, and
one coordinator loop. Repository ADRs hold development strategies; the database
holds executable work and its current state. Resource scheduling and artifact
cleanup follow [ADR 0002](0002-resource-scheduling-and-cleanup.md). Agent-written
development observations follow [ADR 0003](0003-agent-devlog.md).
Briefs, integration ownership, verification, and handoffs follow
[ADR 0004](0004-development-reliability.md), which captures lessons from the old
game's devlog without changing the five-table workflow.
Frontier output evaluation follows
[ADR 0005](0005-frontier-output-evaluation.md).

This SQLite database is development infrastructure. PostgreSQL stores durable
gameplay data, and OTEL logs/spans use a separate SQLite telemetry database under
[Storage architecture](../planning/storage-architecture.md).
The user subsequently authorized the actual planning database. Its durable
location, source-backed manifest, schema and blueprint-versus-dispatch distinction
are recorded in [Plan database](../planning/plan-database.md). Creating those
records does not implement the agent runner or the game.

### Features and tasks

A feature describes a user-visible outcome or bounded development capability
and its acceptance criteria. Tasks describe independently reviewable changes
that deliver that outcome. Atomic
means one bounded result, not necessarily one file.

The user authorized a detailed full-project backlog with several thousand
meaningful outcomes. Keep source-backed contracts, integration, failure, acceptance
and optimization arcs planned across every system; prepare approximately two to
three dispatch-ready atomic tasks per worker.
Reassess plans as implementation reveals new information.

Dispatch each user work message or scoped atomic work-item message to one
implementing subagent that owns its complete result. Do not divide that item
among parallel implementation agents, duplicate its assignment or recursively
delegate the same item. Independent scoped messages can run on separate workers
when dependencies, edit areas and resource limits permit. If a user item is too
large, its one owner plans and completes it coherently; further implementation
items require explicit scope rather than silently splitting the user's message.
Independent frontier evaluation is still a separate completion gate, not a split
implementation assignment. This policy uses existing task/attempt ownership and
adds no table or orchestration layer.

### Five tables

| Table | Responsibility |
| --- | --- |
| `features` | Title, goal, acceptance criteria, priority, status |
| `tasks` | Feature ID, objective/kind, edit areas, acceptance criteria, verification, status, blocking reason, optional originating task ID |
| `dependencies` | Task ID and prerequisite task ID |
| `attempts` | Task ID, worker/model/effort, execution-brief revision, phase/lease owner/expiry, submitted and integrated revisions, verification evidence, evaluator identity/model/capabilities/verdict, defects |
| `devlog` | Agent-written observations, errors, challenges, actions, and outcomes linked to tasks and attempts |

Task identity survives retries. Each retry creates a new attempt. Store review
decisions on attempts rather than introducing a separate reviews table. Keep
large logs in files and reference them from the database.

The attempt's phase and lease identify its current worker, evaluator or integrator;
retain their identities and evidence on the same attempt. Review is not an untracked
process after the worker exits. Cleanup is a bounded operational task under a
development feature, using these same rows and scoped devlog access. Operational
tasks submit an inventory/result rather than inventing a source commit. Before Git
exists, documentation/planning submissions identify file hashes and input provenance;
once Git exists, code submissions and integration must identify actual commits.

Use WAL mode, short transactions, and indexes for task selection and dependency
lookups. Do not hold database transactions open during agent work or verification.

### Three roles

- **Coordinator:** expands features, selects ready tasks, claims work, dispatches
  workers and evaluators, integrates approved commits, and updates SQLite.
- **Worker:** owns one task and submits its result, explanation, and verification
  evidence; source implementation uses an isolated worktree and commit.
- **Evaluator:** a coordinator-spawned independent frontier model with computer
  use and vision; verifies running outputs against the task contract as well as
  code and checks. Returns evidence-based approval, precise defects, or an
  inconclusive verdict identifying missing verification.

Workers and evaluators return structured results and may append their own devlog
entries through a scoped write command. The coordinator alone records completion
and changes authoritative queue state. Devlog entries do not change task status.

### Model routing

Use a frontier coordinator for planning and judgment. Prefer current Luna-class
or Muse Spark-class workers for ordinary bounded implementation; use Terra or Sol
sparingly for harder logic/interfaces. Prefer Claude Sonnet 5.5 or newer for UI
work, with Opus used sparingly where its capabilities are needed. These are user
routing preferences, not claims that a provider/model is available. Check configured
runner, model and tool availability before dispatch; record any required routing
substitution explicitly. Every submission still receives independent frontier
evaluation, and the two-attempt escalation below remains mandatory.

### Task lifecycle

`pending -> running -> review -> done`

Additional states are `blocked` and `cancelled`.

1. Select a pending task whose prerequisites are done and whose edit areas are
   available. Reject dependency cycles when creating dependencies.
2. Transactionally claim the task with a new attempt ID and an expiring lease.
3. Dispatch the worker. Renew the lease while the attempt remains active.
4. Validate that a submission matches the current attempt, then enter review.
   Keep inconclusive evaluations in review until the missing verification is
   resolved; they cannot approve completion.
5. On rejection, preserve the evaluator's defects and return the task to pending
   for another attempt. After two unsuccessful Luna-, Terra-, or Muse-class
   implementation/repair attempts, escalate under the policy below before
   dispatching a third attempt. Reassess scope and dependencies at that boundary.
6. On approval, integrate the reviewed commit sequentially and run relevant
   integration checks. Keep the task in review until this succeeds, then mark done.
   Record the resulting revision before checks so recovery cannot integrate the
   same commit twice. Non-source operational tasks instead verify their declared
   result/inventory; they do not manufacture an empty commit.

An expired phase lease triggers recovery, not automatic duplicate dispatch. Fence
the old agent and confirm its owned processes/edit-area use have stopped before
reclaiming implementation; retain a valid submitted candidate when only an evaluator
needs replacement. Reject submissions and verdicts from superseded ownership.
A blocked task records a concrete reason and resumes when it is resolved.
Cancelled work is intentionally abandoned and does not satisfy dependencies.

If integration changes the reviewed code, obtain another review before completion.
Record the integrated commit so completion is tied to the actual repository state.

### Concurrency and verification

Tasks declare a small set of edit areas, such as `combat`, `phone-ui`, or `storage`.
Allow one active task per overlapping area, including tasks awaiting integration.
The single coordinator tracks active areas; a separate lock service is unnecessary.
Source implementation uses isolated worktrees once Git exists; the coordinator
integrates one commit at a time. Earlier planning obeys the file-ownership and
provenance rules above.

Every task needs a concrete result, bounded edit scope, observable acceptance
criteria, and a focused verification command or procedure. Run focused checks per
attempt and broader checks at integration batches and milestones. Run any checks
needed for dependent tasks before declaring their prerequisites done.
Every implementation/repair brief references the mandatory
[Coding style](../planning/coding-style.md). Evaluation checks its formatting,
lint/target gates, and semantic conventions as well as the delivered behavior.

Briefs also name each required integration hook and its owner, the governing
design/input revision, relevant known pitfalls, and meaningful failure cases.
Include shared wiring/schema files in edit areas. An integration hook outside the
worker's scope gets an explicit connecting task or coordinator responsibility.
Gate prerequisites must be available when the task is dispatched.

Each feature includes a final acceptance task that depends on its implementation
tasks and verifies the complete user behavior. Completing this task completes the
feature.

Verify an integrated game slice early, before executing many isolated components.
Detailed full-project planning is now authorized; it does not waive early running
integration or scoped dispatch. Browser-facing feature acceptance includes real browser
behavior; helpers, coverage figures, and a server-only walkthrough are supporting
evidence. Record the exact integrated build checked and any remaining gaps.
The frontier evaluator operates the affected browser flow and visually inspects
the output; every required criterion needs current evidence under ADR 0005.

### Repairs and discoveries

#### Escalate struggling implementation workers

After two unsuccessful attempts on a task by Luna-, Terra-, or Muse-class
workers, the coordinator must spawn a stronger worker for the next attempt:

| Work being repaired | Escalated worker |
| --- | --- |
| Logic, subsystem interfaces/contracts, or computer-use implementation/debugging | Sol-class subagent |
| UI, UX, or gameplay experience | Sol- or Opus-class subagent |

Count the initial implementation and its repair as two attempts, not two repairs
after the initial implementation. Count across workers and these model families
on the same task; changing workers, models, or brief wording must not reset the
counter. Use existing attempt records rather than a new table or retry service.
Record concrete unsuccessful outcomes: evaluator rejection, failed required
checks, or a worker reporting that it cannot resolve the assigned defect.

Missing dependencies, unavailable tools, provider outages, and build-environment
failures require fixing the prerequisite or environment; do not spend another
model attempt replaying a known blocked setup. Record their causes separately
from unsuccessful implementation attempts. Escalation never waives the task's
acceptance criteria or authorizes new public contracts.

For UI/UX/gameplay, choose between Sol and Opus based on the unresolved problem
and required capabilities. Route server rules/logic and interface correctness to
Sol, and visual/interaction/gameplay-experience work to Sol or Opus. Check actual
model availability and computer-use/vision tooling where needed; do not silently
substitute another economical worker if the escalation target is unavailable.

Pass the escalated worker the original contract, current source/build, both
attempt outcomes, evaluator reproduction steps, tried approaches, retained output
and telemetry evidence, and relevant devlog IDs. Preserve useful work, use a new
attempt/lease, and record the escalation reason plus selected model on the attempt.
Keep the task identity and unresolved acceptance criteria intact.

The escalated worker still receives independent frontier evaluation under
ADR 0005 and cannot approve its own fix. If escalation also fails, the coordinator
reassesses the requirement, contract, prerequisites, or explicit scope before further
dispatch; do not begin another blind retry loop.

#### Preserve completion requirements

A defect within the original acceptance criteria stays on the original task.
Creating a follow-up cannot turn unfinished work into completed work.

A distinct discovery becomes a linked task after checking the existing backlog
for duplicates. An explicitly rescoped replacement cancels the replaced task and
redirects its dependents. Keep one implementing owner per work message; a discovered
subtask does not authorize splitting an active user item across multiple workers.

## Consequences

The workflow provides searchable state, bounded parallelism, independent review,
and an audit trail without a general-purpose workflow engine. A compact status
view should show ready work, active workers, review backlog, blockers, and
completed features.

Conservative edit areas may reduce parallelism, and sequential integration can
become a bottleneck. Start with these rules and refine area boundaries or checks
only when observed workload justifies the added complexity.
