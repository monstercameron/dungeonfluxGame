# Observability

Date: 2026-09-29
Status: OpenTelemetry/SQLite requirements and subsystem/API shapes planned; SDK/schema/bounds pending

## Goal

Make it possible to explain a player's experience end to end: what action they
sent, which rules resolved it, what asynchronous work followed, what each client
received, and where time or failures accumulated. Include observability in the
first playable slice and each subsystem plan, rather than adding it after bugs.

## Required capabilities

- Correlated structured logs and traces across browser clients, session handling,
  rules resolution, persistence, and AI/media jobs. Preserve causality when work
  crosses queues, retries, reconnects, and cancellation boundaries. Use the planned
  session/run/operation/job/utterance IDs and provenance in the subsystem contracts;
  freeze their concrete types before consumers. Do not trust client
  correlation IDs as identity or authorization.
- Metrics for action latency and failures, connected clients and reconnects,
  queue depth and age, rules rejection reasons, persistence latency, resource
  pressure, and provider latency, retries, usage, and cost where reported. Include
  browser errors, rendering performance, and delivery-to-display timing.
  Track server-to-playback timing, audio buffering, and playback failures; clients
  report presentation outcomes while the server records game decisions.
- Useful failure records: classified error, affected operation, crate/version,
  rules/content revision, relevant state revision, and safe diagnostic context.
  Retries, dropped messages, stale job results, and fallbacks must be visible.
- A session timeline joining actions, rule outcomes, jobs, and client delivery.
  Plan controlled diagnostic capture sufficient to reproduce rules defects,
  including dice inputs and state revisions, without recording all session data
  by default. This is a diagnostic requirement, not a mandate for event sourcing.
- Health and readiness checks plus actionable alerts for broken game flows,
  sustained errors, growing queues, and resource exhaustion. Define thresholds
  and performance budgets with the first playable slice; avoid arbitrary targets.

## Design constraints

Keep gameplay telemetry separate from the SQLite agent devlog. The devlog captures
development experience; runtime telemetry diagnoses the running game. Relevant
devlog entries may link to retained diagnostic evidence.

Use OpenTelemetry (OTEL) as the telemetry standard, with Rust instrumentation
behind a small shared convention or adapter. Store structured OTEL logs and their
correlated spans in a dedicated SQLite database agents can query. PostgreSQL owns
durable gameplay data; see [Storage architecture](storage-architecture.md).

`df-observe` owns shared instrumentation/export and the `TelemetryIngress` port;
`df-telemetry` implements ingestion/spooling/SQLite and authenticated operator review.
`df-api` accepts authorized client batches through that port; `df-server` wires it.
See [Subsystem interfaces](subsystem-interfaces.md) and [RPC API](rpc-api.md).
`OperationContext` carries correlation/deadline/provenance, never trusted actor
permissions. `df-auth` supplies authorization separately, including telemetry review.
Accepted batches and durable gameplay jobs are not cancelled by losing an RPC wait.

Use OTLP for native ingestion with a SQLite writer. Exact SDKs,
browser bindings, and adapters must be verified for the chosen native/WASM targets
before selection. Do not assume that a ready-made SQLite OTEL exporter exists.
Keep instrumentation independent of the SQLite storage layout so later deployment
changes do not require rewrites in every crate. Dashboards remain optional;
queryable evidence and a working full-flow trace are required in the first slice.

Instrumentation must cover native Rust and WebAssembly. Client telemetry is
best-effort and must not block input or rendering. Backend export is bounded;
telemetry outages must not stall gameplay. Expose dropped telemetry so collection
failures do not masquerade as a healthy game.
Browser uploads use finite byte/batch/time-bounded RPCs with terminal durability
receipts and stable IDs for retry under [RPC API](rpc-api.md). The browser's selected
persistence/capture boundary and gaps are explicit; native durable spooling cannot
be assumed to provide the same crash guarantees in a browser.

Capture every emitted application log at the enabled levels, across all backend
instances, jobs, adapters, and client presentation reporting. Do not sample logs
or silently filter away errors or gameplay decision records. Plan verbose-level
enablement explicitly. Trace sampling and metric aggregation have separate policies;
log capture must not depend on a trace being sampled. Include database, supervisor,
and ingestion-component diagnostics in the review corpus with their source
identity preserved; recover emergency fallback records when collection resumes.

Set buffering, retention, and overhead budgets. Keep high-cardinality
session/action/player IDs in logs and traces, not metric labels. Avoid logging
credentials, raw audio, private player content, or full AI prompts by default;
scope diagnostic capture and access explicitly. Preserve selected failure evidence
under a retention policy rather than putting it in disposable build artifacts.

## Correlation and searchable records

Preserve the OTEL log model: event and observed timestamps, severity, structured
body/attributes, resource identity, instrumentation scope, and trace/span IDs
where applicable. Preserve typed attributes rather than flattening everything
into a message string. See the [OTEL log data model](https://opentelemetry.io/docs/specs/otel/logs/data-model/).

Add the relevant game context: session/run, action/job/utterance, state revision,
subsystem, build/configuration/content revision, and provider operation. Include
development task/attempt IDs when a test or preview has that context. Context must
survive asynchronous dispatch and be captured at dispatch, not reconstructed from
whatever session is current when a callback completes. Startup logs can lack a
session or trace; their process/build identity must still be searchable.

Assign stable producer/record identities for retry deduplication, plus source
sequence and ingest position for ordering and gap diagnosis. Do not assume client
clocks agree or that timestamps alone establish causality. Preserve span parent
relationships and links; report when a referenced trace is sampled or incomplete.

## Runtime director and predictive asset evidence

Pure policy functions return bounded diagnostic decision facts to the caller;
`df-session` instruments commit/execution through `df-observe`. Correlate input/
plan/step, perception/evidence, world advance, rules resolution, narrative candidate,
encounter/tactic, pacing recommendation, tempo/profile/cue and asset demand/job IDs
with relevant run/revision/content/policy/identity versions. Record selection/decline/
capacity/stale reasons and source provenance; a rejected candidate is not a game fact.
Default logs contain safe identifiers/counts/classifications, not private NPC
beliefs/secrets, player backstories, full transcripts or unrevealed forecasts.
Scoped operator evidence may capture necessary details under existing policy.

Measure due-event backlog/age, candidate/rumor/memory bounds, reaction propagation,
clarification/partial plans, repeated interventions and voluntary spotlight
opportunities. Measure tempo decision time/wakeups/bytes, cue fatigue/expiry,
frame/audio outcomes, speech first-understood-intent/first-approved-audio latency,
asset queue wait/aging/admission/complete/fallback, prefetch hit/lead-time/waste and
actual/unknown/reserved cost. No per-player/NPC metric labels and no claim that a
silence/inactivity metric measures fun or emotion. Separate immutable committed
decision/replay records in PostgreSQL from this diagnostic SQLite corpus.

## Ingestion reliability

Use bounded asynchronous batching with durable local spooling and retry through
temporary sink outages. Acknowledge ingestion only after the record is durably
accepted, and advance spool checkpoints only after successful SQLite commits.
Retries reuse record identities so replay does not duplicate the review corpus.
Define shutdown flush and restart recovery explicitly.

Set size/time limits for batches, memory buffers, disk spools, and records. Track
queue age/depth, last successful commit, ingest lag, source gaps, retries, rejects,
and any dropped/truncated records. Expose ingestion health independently of the
failing exporter, with a local fallback diagnostic path. Collector failures must
not disappear into the same failed logging pipeline they are reporting.

Reliability acceptance must state the capture boundary, supported outage window,
and behavior on disk exhaustion or crash before spooling. Gameplay remains
responsive under telemetry failure; if retention capacity is exhausted, report
loss/degradation explicitly and retain priority failure diagnostics under the
agreed policy. Never claim a complete corpus when a producer or ingestion segment
is missing. Persistent queues have capacity and failure limits; see
[OTEL resilience](https://opentelemetry.io/docs/collector/resiliency/).

## Agent review access

Provide a small read-only query surface with filters for time window, session,
action/job, service/subsystem, severity, build, and trace/span. Support full trace
and session timelines, pagination, structured output, and retained evidence export.
Return ingestion watermarks and gap/retention information with results so an agent
can distinguish "no matching event" from "logs have not arrived or are missing."

Agents can inspect the whole retained log corpus through bounded pages; avoid
dumping every row into a prompt or keeping long transactions open. Index the
common filters and measure queries while ingestion runs. Agents cannot modify or
delete telemetry. Findings link stable records/traces and build identity into
the separate workflow devlog. Retention deletion belongs to an explicit telemetry
policy, never stale-build cleanup. Query/Timeline/Health/PinEvidence/ExportEvidence
shapes and client upload receipts are planned in [RPC API](rpc-api.md). Concrete
OTEL-to-SQLite field mapping, indexes and retention bounds remain a telemetry gate.

## Acceptance during implementation

The initial OTEL gate emits known logs and spans through the actual Rust pipeline,
queries SQLite, and verifies fields, correlation, ordering, and record counts.
Inject sink outages, writer contention, collector restart, duplicate delivery,
malformed/oversized records, and storage exhaustion. Verify spool recovery,
deduplication, read-only agent access, visible missing-data indicators, and gameplay
responsiveness under the declared failure envelope. Benchmark ingestion alongside
agent queries at the target load before increasing backend scale.

Observe game progress as well as infrastructure health: received input, server
decision, job completion, current view delivery, and actual client presentation
are separate checkpoints. Include startup/child-process failures, rejected actions,
stale callbacks, missing executors/assets, fallback selection, and audio playback
failures. The old playtests stalled without error-level logs; a quiet log is not
evidence of a working game. Correlate diagnostics with the running build,
configuration, and rules/content/asset revisions.

Each subsystem task identifies the events, metrics, and failures needed to explain
its behavior. Verify both its successful path and relevant failure path. For each
integrated game flow, demonstrate correlation from player input through the
backend outcome to the appropriate client update. Verify telemetry failure does
not break the game. Concrete schemas/limits and executable checks are frozen at
the telemetry gate in [Implementation roadmap](implementation-roadmap.md), with
the actual pipeline included in the first durable two-role slice.

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
