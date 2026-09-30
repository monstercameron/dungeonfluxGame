# Durable project plan database

Date: 2026-09-29
Status: Planning SQLite provisioned; game/runtime and Rust coordinator implementation pending

## Location and ownership

The user authorized the full project plan corpus and an independent adversarial
refinement loop. `development/workflow.sqlite3` is the durable local planning and
development database. It uses the five tables in ADR 0001: `features`, `tasks`,
`dependencies`, `attempts`, and append-only `devlog`. SQLite is not gameplay
storage: PostgreSQL remains required. Runtime OTEL uses its own SQLite database.

`development/schema.sql`, `development/plan-manifest.json`, and
`development/queries.sql` preserve reproducible structure, seeded plans and
read-only inspection. The manifest includes stable plan IDs, exact governing
document hashes, source sections, the model ledger and the planned RPC methods.
Keep source manifests in version control when Git is initialized; live databases,
WAL/SHM and retained operational evidence remain durable local data outside
`artifacts/`. Periodic build cleanup never deletes them.

The coordinator alone changes queue state. A local SQLite file is not an access
control service: runners need the scoped trusted commands and identity binding
required by ADR 0001/0003. SQL constraints catch routine invalid states; they
cannot establish that a claimed screenshot, model capability or approval is true.
The Rust coordinator and those runner commands are still future work.
The current planning-phase `development/devlog.py` tool accepts an observation
JSON and coordinator-supplied execution-context JSON, validates linked task/attempt
identity and appends parameterized immutable rows with idempotent retries. It
cannot change queue state. The future runner still provides trusted context/access.

## Complete plans and bounded dispatch

The database covers all 41 crates, F01–F47 capabilities, R01–R17 standard rules
families, G01–G12 prerequisites, S00–S08 delivery slices and crosscutting security,
operations, model routing, evidence, performance and accessibility plans.
Each plan identifies its models, owner, source, boundary and meaningful failure
checks. Crate boundary plans and all-crate optimization passes remain distinct
from feature delivery and final integrated acceptance.

This is the entire planning corpus, not thousands of invented per-spell tasks.
Exact catalog entries and required book access are resolved at G07; catalog
completion cannot be counted before its denominator/source revision exists.
Model/vendor/tool availability, transport/WASM feasibility, target devices,
PostgreSQL layout, OTEL limits and concrete budgets are owned resolution gates.
They remain unfinished, rather than being declared solved by mentioning a brand.
The ten runtime additions include world/knowledge/intent/interaction and the
narrative/encounter/combat/experience/tempo/presentation directors. Their persisted
types, pure proposal boundaries, private knowledge, pause-aware clocks, deterministic
replay and predictive asset budgets follow [Runtime directors](runtime-directors.md).

Implementation/acceptance/optimization records are bounded-expansion blueprints.
Before dispatch, the coordinator freezes the next actual atomic scope, applicable
contracts, precise edit paths, commands, limits, hook owners and tool capability
evidence in the preserved brief. Only then may it set `dispatch_ready` true.
`dependency_ready_plans` therefore differs from `dispatch_ready_tasks`: satisfying
dependencies alone does not authorize a large family blueprint as a runnable task.
Expand only the next useful two to three atomic tasks per available worker.

Feature and slice blueprints reuse one canonical bounded implementation child.
Before expansion, look up the same behavior, owner, contract revision and scope;
reuse its task ID and evidence instead of creating another implementation owner.
The slice `COMPOSE` task owns wiring/registration/mounting only. For example,
an atomic F01/S01 join-allocation child is created once, then both acceptance
records depend on that child plus the slice's connecting work. Record that map in
the briefs/dependencies before dispatch. Refuse an overlapping active assignment;
the existing edit-area and single-coordinator rules enforce execution ownership.

G03 freezes definitions in slice order, not every future model before S00. The
full model ledger is a coverage plan; later slices refine their concrete types
before consumers. S08 grows alongside the game from S00. Optional fidelity and
source-defined optional modules need an explicit selected/unselected scope
decision. An unselected task is deliberately rescoped/cancelled with dependents
redirected; cancellation never automatically satisfies a prerequisite.

## Gap assessment refinement

The supplied historical assessment is reconciled in [Gap analysis](gap-analysis.md)
with 20 assessment areas and all 76 research backlog rows. X10 adds the private
campaign-authoring/import/template producer; X11 owns conditional expansion scope.
Existing F35/F36/F09/R09 gain targeted world-time, continuity, noisy-room and
source-grounded effect-contract refinement plans rather than duplicate feature
implementations. That earlier refinement preserved the 40-crate/44-feature/17-rule/12-gate/9-slice
baseline. The later [Feature refinement](feature-refinement.md) adds F45 hosted
remote core, F46 recap/trailer bookends and F47 approved generated content: now
47 capability families within the same 40 crates, 17 rules families, 12 gates and
9 slices. Three MODEL/DELIVER/ACCEPT sets and four targeted acceptance plans reuse
canonical implementations and the existing refinement/provision tool.
`development/refine-gap-plans.py --output <candidate.json>` deterministically
updates source references and these briefs from the existing self-contained manifest;
it writes only the explicit output file, not SQLite state. The derived Mermaid
engine overview is informational and excluded from governing-source fingerprints.
Provision the reviewed candidate through the existing coordinator tool, preserving
execution state/history and separately merged operational tasks. Historic critic
approvals remain tied to their original source/schema fingerprints; current
refinement needs independent review rather than inheriting a prior PASS.

## Schema and evidence invariants

Stable IDs and unique dependency edges support idempotent parameterized seed
updates. Seeds update plan records without resetting existing execution status,
attempts or devlog; schema migrations preserve history. Foreign keys, JSON/check
constraints and a recursive insertion trigger reject dangling references and
dependency cycles. Dependency updates use validated delete/insert.

An attempt preserves its brief/source, phase, lease owner/token/generation,
worker/model, independent evaluator identity/capabilities and criterion evidence.
One live attempt per task and task/attempt generation checks reject stale
submissions. Lease expiry starts fenced recovery; it does not prove owned
processes have stopped. The coordinator confirms process/edit-area safety before
reassigning work, and may replace only a failed evaluator while retaining its
valid submitted candidate. Review and integration use tracked phase leases.

Completion requires finished prerequisites and independent approval tied to the
tested and integrated revision/result plus review/integration evidence. Planning
uses document fingerprints because Git does not yet exist; source implementation
uses actual commits once Git exists. Operational cleanup submits inventories,
not fabricated commits. These gates do not verify evidence content by themselves:
the independent frontier evaluator still executes the affected boundary, with
computer use/vision and audio observation where the contract requires them.
An expired lease cannot submit or newly integrate work. After integration has
already completed, delayed bookkeeping can close the task from its retained
evidence if the same nonempty attempt token/generation and candidate remain
current; terminal evidence does not need an indefinitely renewed lease.

## Adversarial refinement and queries

The independent critic uses explicit pass/fail/inconclusive criteria: complete
coverage, coherent data/API/owner/error/stream/privacy/recovery contracts, genuine
source/catalog gates, delivery order, measured performance/resource plans and
honest executable acceptance. It probes schema failure paths as well as reviewing
the manifest. Findings and linked resolutions are append-only devlog rows with
retained review evidence. Limit the initial loop to three passes; remaining
blocking findings require reassessment, not a confidence score or false approval.

Open `sqlite3 -readonly development/workflow.sqlite3` from the project directory
and use `development/queries.sql`. Inspect counts, gate owners, a plan's full JSON
brief, direct prerequisites, review attempts and linked findings in bounded pages.
The final plan index reports actually performed checks and remaining gates.

## Service-plan refinement

The current plan adds independent native df-commerce/C-df-commerce and X12 commercial
policy, four CustomerService RPCs and bounded specific repair acceptance. It preserves
all47 game/17rules/12gates/9slices and existing execution state. Current source/manifest
identity and counts are in development/plan-index.md; earlier gap/feature PASS evidence
retains its historical identity and does not approve new service behavior.
