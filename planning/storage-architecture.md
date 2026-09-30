# Storage architecture

Date: 2026-09-29
Status: Database roles and persistence contract/data groups planned; physical schemas/deployment pending

## Database roles

| Store | Technology | Responsibility |
| --- | --- | --- |
| Game backend | PostgreSQL | Authoritative durable game/session/player state and required gameplay records |
| Development workflow | SQLite | Features, tasks, dependencies, attempts, and agent devlog under ADR 0001/0003 |
| Runtime telemetry | Separate SQLite database | OpenTelemetry logs and correlated spans for agent review |

PostgreSQL is the required backend database, including local integration tests.
Do not build a SQLite gameplay adapter as a substitute. The workflow and telemetry
databases remain separate so high-volume ingestion cannot contend with task
claims or devlog writes. Telemetry is diagnostic evidence, not authoritative game
state or a replacement for PostgreSQL recovery.

## PostgreSQL planning requirements

`df-persistence` implements consumer-owned `SessionRepository`, `CredentialStore`,
asset metadata, budget and recording ports in [Subsystem interfaces](subsystem-interfaces.md).
That plan defines data groups, fenced compare-and-swap decision commits, atomic
bootstrap/session/member allocation, operation deduplication, journal/checkpoints
and committed effect intents. A receipt means durable decision, not queue admission;
its loss/cancellation does not cancel accepted session/run-owned work. A client's
observed revision is context; the server's current revision/fence protects the write.
Destructive commands additionally enforce strict client preconditions.

The physical snapshot/entity schema, migrations, constraints/indexes, pool settings,
backup topology and finite result-retention/tombstone policy are frozen and checked
at the roadmap's storage gate. Retired keys/namespaces must prevent expired retries
from becoming new writes. Database transactions never span AI/media calls or client waits.

Use a bounded connection pool, explicit query/transaction deadlines, and indexes
chosen from actual access patterns. Account for aggregate connection limits when
adding backend instances. Verify query plans, realistic data volumes, concurrent
sessions, restart recovery, and migration compatibility during integration and
the later optimization pass. Selecting PostgreSQL does not replace workload
measurement or session-ownership planning.

Plan backups and tested restore, deployment migration ownership, and behavior
during database unavailability. Record database latency and failures through
OpenTelemetry without logging credentials or unrestricted query parameters.
`df-assets` owns durable media publication/access through AssetStore; initial
deployment uses a separately configured durable file root, with object storage
available through the adapter boundary later. PostgreSQL holds metadata/references;
complete hashed bytes must exist before metadata publication. Roots/deployment
policy remain a storage gate, not a provisioned service.

## Runtime director state and replay

PostgreSQL persists versioned world/time/entity/schedule/threat state; canonical
facts and observer-specific knowledge/beliefs/witnesses/memories/rumor provenance;
NPC relationships/conversations/secrets/obligations; arcs/phases/beats/threads/hooks
and intervention budgets; encounter objectives/tactics; experience windows; tempo
anchors/fatigue and audience-safe presentation timelines. Asset demands/jobs,
identity/style/canonical packs and dependency versions share existing effect/asset/
budget ownership. No external graph DB is required: bounded relational adjacency/
indexed documents support knowledge, beat and asset graphs under G05/G10 benchmarks.

A single fenced decision commit stores coherent staged state, ordered decision/
fact records, exact accepted semantic outputs/draws, pinned source/policy/model
versions and required effect intents. Snapshots remain the initial recovery spine.
Replay reconstruction is supported only for complete compatible reducers/migrations
verified against state hashes; it makes zero LLM/paid calls and returns explicit
missing-version/evidence gaps. Retain completed one-shot and rumor/encounter dedupe
identities; recovery cannot respawn effects or turn unknown paid work into a refund.
Versioned byte assets remain immutable when a newer identity supersedes them.
See [Runtime directors](runtime-directors.md) and [Asset engine](asset-engine.md).

## Authoring, retrieval and conditional expansion

[Campaign authoring](campaign-authoring.md) adds immutable package/draft/version/
rights provenance and fenced activation using existing publication/session ports.
[Long-horizon state](long-horizon-state.md) adds attributed episodes, source-bound
summaries and replaceable authorized retrieval indexes; they never supersede
canonical facts or required replay records. Physical storage/index choices remain
G05/G10/G11 workload gates. Core snapshots/catch-up use pause-aware logical time.
[Expansion boundaries](expansion-boundaries.md) keeps notification/community
records conditional on explicit scope/privacy/rights decisions rather than
creating unselected tables or a second async authority.

## Remote, bookend and generated-content records

F45/F46/F47 candidate policy/presence/audio-topology, observer bookend/fact-selection/
export grants, identity revisions and approved content/award provenance remain
versioned PostgreSQL state/metadata through existing session/asset/budget ports.
Native scoped retrieval supplies authorized committed history; pure directors do
not query SQL. Export access/revocation version and rights are explicit, without a
promise to erase downloaded media. Custom definitions pin distinct approved
RulesetId/handler/catalog versions and replay records; provider failures never
withhold source-required rewards. Physical schema decisions remain G05/G10; no
new database, schema migration or extra workflow table is created by this plan.

## SQLite telemetry and workflow

The telemetry pipeline follows [Observability](observability.md). Agents receive
read-only query access to the retained runtime corpus; the ingestion component
owns telemetry writes. Workflow writes retain their existing scoped permissions.

SQLite WAL supports concurrent readers with one writer, so batch telemetry
inserts and bound agent queries. Keep live WAL databases on a local filesystem;
remote agents use the review/query interface or consistent exported snapshots,
not a shared network-mounted database. See [SQLite WAL](https://www.sqlite.org/wal.html).

Measure ingestion rate, query latency, disk growth, and checkpoint progress under
the planned game load and concurrent agent reviews. PostgreSQL's backend capacity
does not make a SQLite telemetry writer unbounded. Plan retention/rotation and
capacity thresholds before production scale, preserving query access across the
retained corpus and any pinned defect evidence.

Workflow records, retained telemetry, and ingestion spools are durable operational
data. Keep them out of disposable build/cache roots and periodic artifact cleanup.
The development planning database now exists at `development/workflow.sqlite3`,
with its reproducible schema/manifest under
[Plan database](plan-database.md). PostgreSQL gameplay, SQLite runtime telemetry,
spools and their deployment/driver choices remain unprovisioned; planning-data
creation does not claim those services or the coordinator runner exist.

## Hosted service operating contract

[Service operations](service-operations.md) is the governing refinement for initial
single-instance and later multi-instance routing, fenced claimed dispatch/unknown
spend, tenant-scoped repositories, durable media availability, per-instance SQLite
segments/spool, finite proposed bounds, controlled-launch SLO/RPO/RTO and deletion/
backup restore. These are declared candidate targets, not measured capacity or HA.
Existing promises of canonical replay/immutable history are scoped by personal-data
redaction and rights retention: return explicit Redacted/Unavailable and never
reconstruct removed private payload. [Commerce service](commerce-service.md) owns
subscription/entitlement and hierarchical spend authority; process-local counters
and cached grants cannot override it. Independent current-output acceptance is
required before any paid-release or recovery claim.

Irreversible financial/dispatch/erasure acknowledgements require the independently
protected append-only recovery journal and head watermark in service-operations.md.
It is not restored from the same old PostgreSQL backup and is not gameplay authority.
Without its verified latest range, private/commercial admissions fail closed;
PITR absence never proves unsent work or a fresh operation namespace. Future G05/X02
qualification includes the pre-send/pre-deletion-backup restore fixture and cost.
