# Pure diagnostic handoff to native instrumentation

Date: 2026-10-02

Status: B-C-df-observe-D03/a1 design decision; production adapter remains a later G06 task

## Decision

Pure domain owners return their ordinary typed result together with any bounded,
immutable diagnostic facts needed to explain that result. Facts describe domain
meaning only: a stable classification and safe typed values such as IDs already
present in the input/result, revisions, counts, and booleans. A rejection returns
its classification in the same way as an accepted result. Facts are owned values;
they are neither emitted through a side channel nor changed after return.

The owning native caller captures `OperationContext` when it dispatches the work
and retains it until the result is handled. At that boundary it combines the
returned facts with the actual outcome, caller-owned event/observed times, current
build/resource/scope identity, and SDK-created trace/span/link identity. The native
instrumentation owner assigns producer/record/sequence identity, converts typed
fields into OTEL records, and submits bounded encoded batches through
`TelemetryIngress`; `df-telemetry` remains the sole spool/SQLite writer. It records
admission, durability, lag, gaps, and rejection at the component that owns each
transition. A timeout while waiting for a receipt does not cancel admitted work.

Correlation is provenance only. The caller supplies authenticated identity and
authorized audience from `df-auth` through their own boundary; trace IDs, client
metadata, diagnostic facts, and producer IDs confer no identity or permission.
Pure rules/engine/director code has no OTEL/SDK, clock, logger, filesystem, or
telemetry dependency. It does not construct spans, serialize OTLP, allocate
telemetry identities, or decide who may see a record. A browser pure result follows
the same value boundary; any browser reporter/buffer is a separate native/WASM
adapter decision and does not inherit native durability guarantees.

This freezes the handoff and ownership only. D02 owns the structured field catalog;
this decision defines no competing keys, severity mapping, OTEL field schema,
production byte/item/time limits, browser queue, exporter choice, or SQLite layout.
The values below are a small executable contract illustration, not production
public types. The actual workspace's `NativeProducer`/`TelemetryIngress` path is
the existing synthetic native capture foundation, not proof of a registered game
producer or a live browser-to-SQLite pipeline.

## Bounded contract illustration

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
enum FactValue {
    Code(&'static str),
    Count(u32),
    Flag(bool),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PureFacts {
    classification: &'static str,
    values: Vec<FactValue>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PureResult {
    accepted: bool,
    facts: PureFacts,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NativeContext {
    trace_parent: &'static str,
    build: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct NativeRecord {
    result: PureResult,
    context: NativeContext,
    event_time_ms: u64,
    observed_time_ms: u64,
}

fn native_handoff(result: PureResult, context: NativeContext) -> NativeRecord {
    NativeRecord {
        result,
        context,
        event_time_ms: 1_000,
        observed_time_ms: 1_004,
    }
}

fn main() {
    let rejected = PureResult {
        accepted: false,
        facts: PureFacts {
            classification: "example.refusal",
            values: vec![
                FactValue::Code("stale_revision"),
                FactValue::Count(1),
                FactValue::Flag(false),
            ],
        },
    };
    let original = rejected.clone();
    let record = native_handoff(
        rejected,
        NativeContext {
            trace_parent: "00-11111111111111111111111111111111-2222222222222222-01",
            build: "synthetic-build",
        },
    );

    assert_eq!(record.result, original);
    assert_eq!(record.event_time_ms, 1_000);
    assert_eq!(record.observed_time_ms, 1_004);
    assert!(!record.result.accepted);
    assert_eq!(record.result.facts.values.len(), 3);

    let private_payload = "full player transcript";
    let credential = "secret token";
    assert!(!format!("{record:?}").contains(private_payload));
    assert!(!format!("{record:?}").contains(credential));
}
```

The native function shown only demonstrates the ownership join: it receives an
immutable pure result and independently owned correlation/time inputs. Production
code must use the frozen D02 field/value catalog, caller clock, approved `df-types`
identities and `BuildIdentity`, and the existing native adapter/ingress contract;
it must not copy these demonstration strings, time values, or wrapper types into
the public API.

## Alternatives and refusals

- **Pure crates emit OTEL directly:** refused. This would add SDK/transport concerns
  to deterministic domain crates and make clocks, exporter state, or I/O affect
  rule evaluation. It conflicts with `planning/subsystem-architecture.md`'s
  explicit pure-facts boundary and `planning/observability.md`'s session-owned
  instrumentation.
- **A global logger or ambient current span:** refused. It loses explicit dispatch
  context across async work and makes ownership, cancellation, and tests implicit.
- **Reconstruct context when a callback completes:** refused. It can attach the
  wrong session/run/operation after replacement or reconnect; context must be
  captured at dispatch as required by `planning/observability.md` and
  `planning/runtime-reliability.md`.
- **Treat trace metadata as actor identity or audience:** refused. The interface
  contract explicitly separates correlation from `Principal` and
  `AuthorizedAudience`; authorization stays with `df-auth`.
- **Have pure facts carry wall-clock timestamps, OTEL IDs, or telemetry record
  identities:** refused. The native producer/clock owns those values; pure output
  contains only facts derived from explicit inputs and its computed result.
- **Add a second browser/native queue, logger, or durable store here:** refused.
  `df-observe` is the shared instrumentation/ingress boundary and
  `df-telemetry` owns ingestion/spooling/SQLite. Browser buffering and browser
  durability require a separately reviewed target-specific implementation.
- **Claim current `NativeProducer` proves the production handoff:** refused. Current
  code calls itself synthetic, provides an in-memory fixture exporter, and has no
  registered `df-session`/`df-engine` game producer or production browser upload
  path in this workspace.

## Source-backed fit and remaining gates

- `planning/observability.md`: pure diagnostic facts return to the caller;
  `df-session` captures dispatch context; native ingestion is bounded and
  separately durable; trace sampling cannot filter logs; browser reporting is
  best-effort and its capture/durability boundary stays explicit.
- `planning/subsystem-architecture.md`: pure rules/engine/directors return facts
  without an SDK; `df-observe` owns the shared convention/export hook;
  `df-telemetry` owns spool/SQLite; `df-server` wires components.
- `planning/subsystem-interfaces.md`: `OperationContext` holds correlation,
  deadline/cancellation, and provenance but never authorization; the engine
  returns candidate transitions; telemetry ingress is injected by the server.
- `planning/runtime-reliability.md`: async work carries captured operation context;
  stale callbacks are rejected by owner generation/operation checks; emitted
  telemetry is diagnostic and cannot replace durable gameplay state.
- Current code evidence: `crates/df-observe/src/producer.rs` installs native SDK
  providers, encodes OTLP, reserves source identity and calls the ingress port;
  `crates/df-observe/src/ingress.rs` defines stable IDs, bounded limits, pending
  receipts, and `TelemetryIngress::submit`; `crates/df-telemetry/src/lib.rs`
  implements the native queue/spool/SQLite owner. The currently compiled
  `OperationContext` in `crates/df-observe/src/lib.rs` is only `trace_parent` and
  `build`, so it does not yet realize the full planned context contract.
- `crates/df-tools/src/browser.rs` and `crates/df-rpc-bridge/src/browser.rs` use
  `df-observe` in synthetic transport fixtures. Their presence does not establish
  browser capture buffering or a production browser upload contract.
- No `df-rules`, `df-model`, `df-engine`, `df-session`, `df-api`, or `df-server`
  implementation crate is present in this workspace. Therefore the exact
  production `PureFacts` owner, concrete D02 field mapping, context type completion,
  and caller registration remain G03/G06 implementation decisions.

Next implementation must bind this handoff to the existing typed result owned by
the exact pure crate and the actual `df-session`/native dispatch caller, then prove
the resulting record traverses the real `df-observe` -> `df-telemetry` path. Until
that work exists, this is a source-backed design decision; no running game/browser
capture, production SDK configuration, SQLite durability, or end-to-end trace is
claimed.
