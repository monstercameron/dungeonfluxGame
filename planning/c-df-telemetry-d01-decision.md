# C-df-telemetry D01: OTLP SQLite mapping and read scope

Status: source-compatible decision for the current native fixture; production operator, retention, segment, pin, and export behavior remains unqualified.

## Decision

Keep runtime telemetry in the per-instance `df-telemetry` SQLite corpus and its local spool. PostgreSQL remains authoritative for gameplay; the workflow SQLite database remains authoritative for task/development records. Runtime logs, traces, watermarks, pinned evidence, and exports are diagnostic evidence only: none can create, amend, or prove a game fact.

The existing record mapping is the compatibility contract for the next bounded segment/pin work:

| SQLite field | Meaning |
| --- | --- |
| `position` | Monotonic committed ingest position, primary key and read cursor. |
| `producer`, `record`, `sequence` | Stable 16-byte producer lifecycle namespace, stable 16-byte record identity, positive source sequence; unique on `(producer, record)` and `(producer, sequence)`. |
| `signal` | `1` logs, `2` spans. |
| `source_time`, `observed_time` | Unsigned OTLP event/start and observed timestamps as 8-byte big-endian values; span observed time is absent. |
| `severity`, `trace`, `span` | Indexed diagnostic severity and trace/span correlation bytes. Correlation is provenance only. |
| `session`, `operation`, `build` | Optional text projections from `df.session`, `df.operation`, and resource `df.build`; these aid filtering and do not establish identity or access. |
| `digest`, `otlp` | Signal-qualified SHA-256 and the complete encoded single-record OTLP export request. Keep the protobuf as the typed source of truth, including resource, instrumentation scope, schema URLs, attributes, events/links, and causal fields; indexed columns are query projections. |

Retry identity is `(producer, record)` and ordering/gap diagnosis uses `(producer, sequence)`. Reuse of either key with changed content/sequence is a conflict/corruption, not a new event. The producer ID is an explicit source-lifecycle namespace and must not be reset on restart while old identities could recur. `position` and the committed watermark order the local corpus; the source sequence and per-source watermark expose source gaps. Neither timestamp order nor a filtered page establishes causality or corpus completeness.

Preserve the existing indexes on trace, session, operation, event time, and build. The read surface stays a typed parameterized filter (`producer`, `session`, `operation`, minimum severity, `build`, `trace`, inclusive event-time bounds), ordered by `position` after an opaque numeric position cursor. A query takes a fresh short snapshot; pagination does not retain a transaction across caller think time. Keep current request bounds: 1–100 rows, 1 MiB response, 128-byte text filters, severity 0–24, and `from <= through`. The page reports committed watermark, spool checkpoint, per-source accepted/count/highest/first/missing sequence facts, emergency diagnostic bytes, and explicit unknown capture completeness/retention. A page with no matches does not prove no events exist outside its watermark, retained segments, or source coverage.

The diagnostic reader opens SQLite with `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NOFOLLOW` and `query_only=ON`; it exposes no arbitrary SQL. Keep each instance's writable SQLite database and spool on its protected local volume. Review federation, if added, reads bounded indexed copies/segments. Never share a writable SQLite file over a network filesystem.

Query filters and OTEL context never authorize access. `df-api`/the review boundary must derive the trusted principal and permitted audience from `df-auth`, authorize before reading or serializing rows, and apply privacy/redaction policy before response, pin, or export. A client-supplied actor, session, trace, build, or producer value is a filter value only. Default diagnostics omit credentials, raw audio, private player content, and full prompts. Pin/export must name the selected records and build/source identity, honor deletion/suppression and retention policy, and return explicit scope/gap metadata; a pin does not turn telemetry into durable game truth or exempt it from lawful suppression.

Typed-value example: preserve an OTLP log attribute `rpc.bytes = IntValue(4096)` inside the complete single-record payload; do not coerce a string value `"4096"` to an integer. The current writer/query carries encoded OTLP bytes, and the native fixture already asserts typed integer values survive the wire representation.

For source-compatible bounded capture, the existing finite limits are:

```rust
TelemetryLimits {
    record_bytes: 8_192,
    batch_bytes: 65_536,
    batch_records: 32,
    queue_items: 1_024,
    queue_bytes: 1_048_576,
    spool_bytes: 33_554_432,
    sqlite_bytes: 67_108_864,
}
```

These are the current `TelemetryLimits::default()` values, not a production capacity guarantee. Admission rejects zero or over-ceiling fields, and SQLite must be at least 262,144 bytes. The existing segment/pin work may enforce smaller per-operation bounds but must preserve this meaning and never advertise completeness beyond observed durable ranges.

## Alternatives and unresolved facts

Keep OTLP protobuf bytes as the complete payload instead of flattening typed attributes into ad hoc columns; retain only the source-backed scalar projections and indexes above. Keep `position` as the page cursor instead of timestamps, since timestamps can collide or arrive out of order. Keep a per-instance local writable database instead of a shared network SQLite writer. These choices match current code and the approved storage boundary.

Current source does not establish authenticated operator review, cross-instance federation, immutable segment rotation, pin/export authorization, suppression propagation, a production retention schedule, or crash-safe client capture. The fixture retains accepted spool frames within finite quotas and explicitly reports unknown producer-lifetime completeness; it performs no automatic deletion. The service policy's proposed per-instance segment/spool sizes and seven-day target remain proposals until implementation and restore/retention qualification. D01 freezes only the mapping/query compatibility needed to continue that work; it does not qualify D02 spool receipts or the pending D03 segment candidate.

## Source boundary and evidence

Source inspected at input revision `a3c31a00e7406898b0a82617fa7626dc0a4e4c94`: `crates/df-telemetry/src/{persistence,query,wire}.rs`, `crates/df-observe/src/{ingress,producer}.rs`, and `crates/df-telemetry/src/main.rs`. Governing design: `planning/observability.md` (mapping, ingestion reliability, review access), `planning/storage-architecture.md`, `planning/erasure-retention-policy.md`, `planning/service-operations.md`, and `planning/subsystem-interfaces.md` (trusted principal and audience separation). The development SQLite queue/devlog is intentionally outside the telemetry database.

The Rust literal above is the bounded contract example for the next consumer. Source/build-bound evidence must record the exact input revision and hashes, rustfmt config/tool, direct rustc command/output, and finite valid/refusal execution. No Cargo, native/WASM, live-ingestion, authenticated-review, rotation, pin/export, retention, or recovery check is implied by this design decision; those checks remain unperformed until their prerequisites and implementation exist.
