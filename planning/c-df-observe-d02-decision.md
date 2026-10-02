# D02: Structured diagnostic field catalog

Date: 2026-10-02
Status: Design decision; browser buffer implementation and integrated capture remain pending
Input revision: `a3c31a00e7406898b0a82617fa7626dc0a4e4c94`
Task/attempt: `B-C-df-observe-D02` / `B-C-df-observe-D02-a1`

## Decision

The browser diagnostic buffer accepts only the existing OTEL record envelope and
this source-backed attribute catalog. It preserves OTEL's typed fields and
resource/scope/trace relationships; it does not add a DungeonFlux wire schema.
The browser boundary must build records from these approved fields before
serialization. It rejects an event with an unknown field or an invalid value
type and accounts for that refusal without retaining the rejected key or value.

| Record location | Accepted fields | Source and handling |
| --- | --- | --- |
| OTEL log envelope | Event timestamp, observed timestamp, severity number/text, event name, optional trace/span IDs | Standard typed OTEL fields accepted by `LogInput` and the native log decoder. Browser event/body text is excluded; event names come from static instrumentation code. |
| OTEL span envelope | Name, start/end timestamps, trace/span/parent IDs, status, events and links | Standard typed OTEL fields accepted by `SpanInput` and the native span decoder. Names/statuses are static or classified producer output, never copied error text. |
| Resource | `service.name`, `df.build`, `df.native`, `df.wasm`, `df.configuration`, `df.content` | Existing `NativeProducer` resource attributes. Revision values come from `BuildIdentity`; the synthetic service name is producer-owned. |
| Instrumentation scope | Name, version, schema URL; `fixture.synthetic` (boolean) | Existing scope envelope and fixture marker. Preserve scope schema metadata as typed OTEL fields. |
| Event/span attributes | `df.session` (string ID), `df.operation` (string ID), `fixture.build` (string), `rpc.status` (classified string), `rpc.bytes.measured` (boolean), `rpc.bytes` (integer only when measured) | Existing fields emitted by `df-observe`, accepted/indexed by `df-telemetry`, or emitted by the browser fixture. Session and operation values are correlation only and must originate from their owning typed IDs. Byte counts are observations, not payloads. |
| Capture envelope | `df.producer_id`, `df.record_id`, `df.source_sequence` | Reserved by `NativeProducer` before dispatch and required by `df-telemetry` for retry identity/order. Caller attributes may not set or override them. Ingest position, watermarks, durability, and gaps remain `IngestReceipt`/query metadata, not event attributes. |

An event body is absent in the browser catalog. Do not capture request/response
bodies, arbitrary messages, exception text, headers, cookies, credentials, raw
audio, player-authored/private content, prompts, secrets, actor/principal/role or
authorization claims. Correlation values and trace IDs never authenticate an
actor. Do not add `df.run`, job, utterance, action, state-revision, metric, or
other fields until the canonical producer type and the actual consuming source
exist and are reviewed; omission is explicit rather than an invented namespace.

## Why this is the consumer contract

At the frozen input revision, `df-observe::OperationContext` contains only
`trace_parent` and `build`. Its tracing helper produces the existing
`fixture.build`, `rpc.status`, `rpc.bytes.measured`, and `rpc.bytes` attributes.
The browser fixture reads incoming `traceparent` metadata, exercises the four
RPC modes, and records a `browser.fixture` span after the fixture completes; its
UI/report strings are fixture output, not a production diagnostic stream.
`df-observe::NativeProducer` accepts generic typed OTEL bodies and caller
attributes, so it does not itself enforce this privacy catalog. The browser
buffer must therefore apply the allowlist before serialization.

`df-telemetry::wire::Batch::decode` accepts standard OTLP logs/spans, requires
unique capture identity attributes, extracts `df.session` and `df.operation`,
reads `df.build` from the resource, and retains each typed single-record OTLP
request. The query layer filters by those indexed values and returns the retained
OTLP bytes. That behavior consumes the names above without requiring a new wire
shape. It is bounded by `TelemetryLimits`; those native record, batch, queue,
spool, and SQLite ceilings do not establish WASM buffer limits or browser
persistence guarantees.

The approved D01 candidate at `105aec9813b26c92dfa3a0ca5302816444e54e56`
provides context that correlation and build provenance are separate from actor
authority, and that game IDs are optional correlation fields. It is not integrated
at this D02 input revision. This catalog freezes only fields with current
producer/consumer evidence; its future field families do not silently become
implemented fields.

## Alternatives and rationale

- **Serialize arbitrary OTEL bodies and attributes, then redact known secrets:**
  rejected because generic bodies and attributes can contain private content, and
  an incomplete denylist cannot establish exclusion.
- **Define a new browser-only JSON/protobuf diagnostic envelope:** rejected
  because OTEL logs/spans and the current telemetry consumer already provide the
  record shape and preserve typed data; a parallel wire schema is unnecessary.
- **Treat correlation IDs or `traceparent` as identity:** rejected because D01
  and the current source define them as diagnostic provenance only.

## Bounded contract example

This exact field-name/type predicate is a design fixture for the buffer boundary,
not a replacement producer API or proof that a running browser applies it. Value
provenance still comes from the named typed/static producer sources above.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ValueKind {
    String,
    Boolean,
    Integer,
}

fn browser_attribute_allowed(name: &str, kind: ValueKind) -> bool {
    matches!(
        (name, kind),
        ("df.session", ValueKind::String)
            | ("df.operation", ValueKind::String)
            | ("fixture.build", ValueKind::String)
            | ("rpc.status", ValueKind::String)
            | ("rpc.bytes.measured", ValueKind::Boolean)
            | ("rpc.bytes", ValueKind::Integer)
            | ("fixture.synthetic", ValueKind::Boolean)
    )
}

fn main() {
    assert!(browser_attribute_allowed("rpc.status", ValueKind::String));
    assert!(browser_attribute_allowed(
        "rpc.bytes.measured",
        ValueKind::Boolean
    ));
    assert!(browser_attribute_allowed("df.session", ValueKind::String));
    assert!(browser_attribute_allowed("rpc.bytes", ValueKind::Integer));
    assert!(!browser_attribute_allowed("rpc.bytes", ValueKind::String));
    assert!(!browser_attribute_allowed(
        "authorization",
        ValueKind::String
    ));
    assert!(!browser_attribute_allowed(
        "player.message",
        ValueKind::String
    ));
    assert!(!browser_attribute_allowed("body", ValueKind::String));
}
```

The valid cases preserve existing typed fields. The refusal cases reject a type
mismatch and representative credential/private-payload fields. The predicate
alone does not sanitize values or verify source identity; the browser adapter
must select values only from the typed/static sources in the catalog and refuse
all unlisted fields before encoding.

## Unresolved facts and next consumer

No browser diagnostic buffer, browser upload receipt path, or production gameplay
telemetry consumer exists in the input source. The next consumer must apply this
catalog at browser capture, define its separately bounded WASM buffer/overflow
behavior under its own implementation brief, and connect standard OTLP records
to the existing native `TelemetryIngress` contract. It must preserve visible
refusal/loss and retry identities without treating a pending receipt as durable
acceptance. Native `TelemetryLimits` cannot be reused as browser-memory evidence.

The generic native producer currently allows caller-supplied bodies and
attributes; this decision does not retrofit or claim privacy enforcement for
that API. Production consumers that need additional identifiers or fields must
first provide source-backed canonical producer values and a consuming index/use.

## Source and verification evidence

- Governing design: `planning/observability.md`,
  `planning/subsystem-architecture.md`, `planning/subsystem-interfaces.md`, and
  `planning/coding-style.md`; workflow and handoff: ADRs 0001–0005 and `AGENTS.md`.
- Source revision: `a3c31a00e7406898b0a82617fa7626dc0a4e4c94`.
- Inspected producer/ingress/types: `crates/df-observe/src/lib.rs`,
  `producer.rs`, and `ingress.rs`; browser/fixture consumer:
  `crates/df-tools/src/browser.rs` and `fixture.rs`; native consumer:
  `crates/df-telemetry/src/wire.rs`, `query.rs`, and `main.rs`.
- The finite valid/refusal predicate was formatted, compiled with warnings denied,
  and executed under the new v5 guard. This checks the documented field-name/type
  boundary only. No Cargo, WASM/browser, upload, production telemetry, or
  integrated gameplay check was run or implied.
