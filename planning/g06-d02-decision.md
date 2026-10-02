# G06-D02: bounded telemetry record and redaction decision

Date: 2026-10-01  
Status: proposed source-backed design; independent review and production gates pending

## Decision

Use the OpenTelemetry LogRecord model as the semantic source, with a finite
application envelope for records admitted to the telemetry pipeline. Keep the
OTEL event and observed timestamps, severity, structured body/attributes,
resource identity, instrumentation scope, and optional trace/span relationship.
The application envelope adds only the already-planned safe correlation context:
session/run/action/job/utterance IDs when available, state revision, subsystem,
build/config/content revision, and provider operation classification. Startup
records may omit session and trace; they still carry process/build identity.
Stable producer/record identity, source sequence, and ingest position support
retry deduplication and gap reporting. Timestamp order alone is not causal order.

Redact by constructing a typed safe record before encoding or exporting. The
default path accepts a finite allowlist of diagnostic facts (identifiers,
revisions, counts, classifications, and bounded error codes). It never copies
arbitrary OTEL attributes, private player/NPC content, raw audio, credentials,
full prompts, or provider bodies. Unknown or sensitive fields fail closed as a
typed refusal; do not silently strip data and report a complete event. Scoped
diagnostic capture requires a separately authorized, expiring policy and is
outside this default schema. Trace context is correlation only: authorization
is supplied independently by `df-auth` and must be checked before query/export.

Use the currently documented service-operations candidate envelope for initial
capacity planning: 100 enabled log events/sec sustained, 1 KiB average, burst
1,000/sec for 10 sec; per-instance spool 2 GiB, active SQLite segment 4 GiB,
30 GiB total quota, and seven-day rotation target. These are proposed operating
numbers, not measured capacity or a guaranteed retention period. Quota pressure
wins over the time target, loss/gap ranges stay visible, and no claim of a full
corpus is allowed across missing producer/ingest ranges. The initial source does
not define a maximum serialized record or exact OTLP-to-SQLite columns/indexes;
those require an explicit later storage contract and measurement. The finite
illustrative contract below deliberately uses local conservative field budgets
to demonstrate reject-before-encode behavior; those values are not proposed
production limits.

## Ownership and failure behavior

`df-observe` owns shared instrumentation/context and bounded submit/export
ports; its pure policy returns facts and does not call an SDK or storage layer.
`df-telemetry` owns OTLP ingestion, spooling, SQLite mapping, deduplication,
retention, health, and authenticated read-only review. `df-api` authorizes and
passes client batches to the `TelemetryIngress` port owned by `df-observe`;
`df-server` wires it; `df-auth` independently authorizes operator query,
pin, and export. Keep telemetry separate from PostgreSQL gameplay data and the
workflow/devlog SQLite database.

Client submission is best effort and must not block rendering/input. Backend
export is bounded and telemetry failure must not stall already-admitted gameplay.
The authenticated client batch path has finite byte/batch/time limits and a
terminal durability receipt. A receipt distinguishes spooled from SQLite
committed; acknowledge durable acceptance only after that level is true, and
advance checkpoints only after SQLite commit. Retry uses stable record IDs.
Writer/sink/storage failures produce explicit rejected, delayed, or dropped
outcomes and visible lag/gap/retention watermarks; they never become fabricated
success. Disk exhaustion reports degradation/loss, preserves any reserved
priority failure evidence under the later selected policy, and does not block a
gameplay decision. Browser persistence/capture crash guarantees are not inferred
from native spooling.

## Alternatives

- Copy every OTEL attribute and blacklist known secret names: rejected because
  unrecognized private content can leak by default. Typed allowlisting rejects
  unknown fields before serialization.
- Store only a formatted message string: rejected because it loses typed
  attributes, resource/scope identity, and useful trace relationships.
- Sample enabled logs or silently drop records when full: rejected by the
  governing no-sampling requirement and because it hides failures. Make overload
  and gaps explicit; control verbosity only through explicit configuration.
- Put telemetry in PostgreSQL or the workflow SQLite database: rejected by the
  established dedicated SQLite telemetry boundary and separation of gameplay,
  runtime diagnostics, and development workflow.
- Freeze concrete SDKs, columns, indexes, and a fixed record byte ceiling now:
  deferred because native/WASM SDK/export compatibility, exact storage mapping,
  and measured record/load bounds remain G06/G01 gates.

## Finite illustrative Rust contract

This is a std-only policy example, not a production API or runtime evidence.
It accepts only allowlisted, bounded diagnostic attributes before encoding;
it exercises safe acceptance, private-content refusal, unknown-field refusal,
oversize refusal, and invalid identifier refusal.

```rust
use std::collections::BTreeMap;

#[derive(Debug, PartialEq)]
enum Refusal {
    MissingRecordId,
    InvalidIdentifier,
    TooManyAttributes,
    InvalidKey,
    PrivateOrUnknownAttribute,
    ValueTooLong,
    InvalidSafeValue,
}

#[derive(Clone, Copy)]
enum EventBody {
    ActionRejected,
    QueueStalled,
}

struct Candidate<'a> {
    record_id: &'a str,
    subsystem: &'a str,
    body: EventBody,
    attributes: BTreeMap<&'a str, &'a str>,
}

struct SafeRecord {
    record_id: String,
    subsystem: String,
    body: String,
    attributes: BTreeMap<String, String>,
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

fn valid_decimal(value: &str) -> bool {
    !value.is_empty() && value.len() <= 32 && value.bytes().all(|byte| byte.is_ascii_digit())
}

fn prepare(candidate: Candidate<'_>) -> Result<SafeRecord, Refusal> {
    if candidate.record_id.is_empty() {
        return Err(Refusal::MissingRecordId);
    }
    if !valid_id(candidate.record_id) || !valid_id(candidate.subsystem) {
        return Err(Refusal::InvalidIdentifier);
    }
    if candidate.attributes.len() > 4 {
        return Err(Refusal::TooManyAttributes);
    }
    let mut safe = BTreeMap::new();
    for (key, value) in candidate.attributes {
        if key.is_empty()
            || key.len() > 24
            || !key.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
        {
            return Err(Refusal::InvalidKey);
        }
        if value.len() > 32 {
            return Err(Refusal::ValueTooLong);
        }
        match key {
            "state_revision" | "queue_depth" if valid_decimal(value) => {}
            "error_code" if matches!(value, "stale_revision" | "capacity" | "timeout") => {}
            "state_revision" | "queue_depth" | "error_code" => {
                return Err(Refusal::InvalidSafeValue);
            }
            _ => return Err(Refusal::PrivateOrUnknownAttribute),
        }
        safe.insert(key.to_owned(), value.to_owned());
    }

    Ok(SafeRecord {
        record_id: candidate.record_id.to_owned(),
        subsystem: candidate.subsystem.to_owned(),
        body: match candidate.body {
            EventBody::ActionRejected => "action rejected",
            EventBody::QueueStalled => "queue stalled",
        }
        .to_owned(),
        attributes: safe,
    })
}

fn main() {
    let accepted = prepare(Candidate {
        record_id: "evt-01",
        subsystem: "session",
        body: EventBody::ActionRejected,
        attributes: BTreeMap::from([("error_code", "stale_revision")]),
    })
    .expect("allowlisted bounded diagnostic record");
    assert_eq!(accepted.record_id, "evt-01");
    assert_eq!(accepted.subsystem, "session");
    assert_eq!(accepted.body, "action rejected");
    assert_eq!(accepted.attributes["error_code"], "stale_revision");

    let private = prepare(Candidate {
        record_id: "evt-02",
        subsystem: "session",
        body: EventBody::ActionRejected,
        attributes: BTreeMap::from([("player_message", "secret")]),
    });
    assert_eq!(private.err(), Some(Refusal::PrivateOrUnknownAttribute));

    let unknown = prepare(Candidate {
        record_id: "evt-03",
        subsystem: "session",
        body: EventBody::ActionRejected,
        attributes: BTreeMap::from([("unreviewed", "value")]),
    });
    assert_eq!(unknown.err(), Some(Refusal::PrivateOrUnknownAttribute));

    let too_many = prepare(Candidate {
        record_id: "evt-04",
        subsystem: "session",
        body: EventBody::ActionRejected,
        attributes: BTreeMap::from([("a", "1"), ("b", "2"), ("c", "3"), ("d", "4"), ("e", "5")]),
    });
    assert!(matches!(too_many, Err(Refusal::TooManyAttributes)));

    let invalid_id = prepare(Candidate {
        record_id: "bad id",
        subsystem: "session",
        body: EventBody::ActionRejected,
        attributes: BTreeMap::new(),
    });
    assert_eq!(invalid_id.err(), Some(Refusal::InvalidIdentifier));

    let private_value = prepare(Candidate {
        record_id: "evt-05",
        subsystem: "session",
        body: EventBody::QueueStalled,
        attributes: BTreeMap::from([("state_revision", "private player text")]),
    });
    assert_eq!(private_value.err(), Some(Refusal::InvalidSafeValue));
}
```

The fixture budgets and allowlist are instructional and must be replaced only by a reviewed
production contract. Before encoding, production must also bound total OTLP
batch bytes and reject before large allocations; the illustrative per-field
checks alone do not prove that property.

## Unresolved production gates

1. Verify the selected Rust OTEL SDK/export path on supported native and WASM
   targets; the current plans explicitly say this is unverified.
2. Freeze OTLP-to-SQLite column/type mapping, indexes, stable identity and
   source-sequence ownership, query pagination, and schema/version migration.
3. Select exact maximum record, batch, queue, spool, flush/shutdown, and capture
   limits from the G06 workload and measure CPU, memory, disk, writer contention,
   query latency, and outage recovery. Current service envelope values are
   proposals, not observations.
4. Define retention/deletion interaction with private-data rights, backup
   tombstones, evidence pinning, priority diagnostics, and per-instance segment
   recovery; prove gap reporting across restart and instance loss.
5. Freeze trusted source/audience validation, browser upload receipts and retry
   behavior, and authenticated read-only operator filters at their owning API,
   auth, and server boundaries.
6. Inject duplicate delivery, malformed/oversize input, sink outage, writer
   contention, restart, disk full, and browser crash cases through the real
   pipeline. Demonstrate queryable end-to-end correlation and gameplay
   responsiveness on the source/build under review.

No production telemetry code or production end-to-end behavior is claimed here.
The only verification for this submission is the separately retained finite
contract fixture execution and formatting/compiler checks, if their guarded
receipts succeed.

## Governing sources

- `planning/observability.md` (2026-09-29): OTEL semantic fields, default
  redaction, no log sampling, bounded backend export, durable spool/receipt,
  deduplication, health/gap reporting, and implementation gates.
- `planning/subsystem-interfaces.md`, “Observability and development tooling”:
  `df-observe` ports, `df-telemetry` ingest/query/retention, receipt levels, and
  `df-auth` operator authorization.
- `planning/subsystem-architecture.md`: crate ownership and dependency direction.
- `planning/storage-architecture.md`: runtime telemetry SQLite separation from
  PostgreSQL gameplay and workflow SQLite.
- `planning/service-operations.md`, “Initial and scaled deployment”: proposed
  event/spool/segment/quota/retention and lag envelopes, expressly unmeasured.
- `planning/implementation-roadmap.md`, “Prerequisite decisions” and
  “S01 First durable two-role slice”: G06 owner and the actual pipeline/query
  acceptance boundary.
