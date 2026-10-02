# D01: Operation context propagation contract

Date: 2026-10-02  
Status: Design decision; implementation and integrated runtime qualification pending  
Input: `5759dfc3a619e8ee68eecd6bbf3d4c5ebdd00c35`  
Task/attempt: `B-C-df-observe-D01` / `B-C-df-observe-D01-a1`

## Decision

`df_observe::OperationContext` is diagnostic correlation and nonsecret build
provenance. It never carries or proves a principal, permission, role, credential,
or authorized audience. `df-auth` remains the authority for those values; callers
pass its trusted principal/audience separately and reauthorize at the protected
boundary. A trace ID, operation ID, session ID, or possession of a context cannot
grant access. This is the D01 acceptance rule: trace provenance never authenticates
an actor.

The propagation contract has two existing scopes. `OperationContext` is the
immutable correlation/provenance snapshot captured when an operation is admitted.
The native task context remains the owner of a dispatched task's job identity,
generation, deadline, cancellation scope, and captured operation context. Copy the
admitted snapshot into owned asynchronous work; do not look up whichever session
is current when a callback finishes. A child operation keeps the originating trace
parent or an explicit span link. Durable accepted jobs get their own lifetime and
deadline: expiry/cancellation of an RPC wait cannot cancel committed work. Before
acceptance, request cancellation may stop admission or discard unaccepted work.
Generation and job/operation identity are checked when results return.

The field contract is:

| Field family | Meaning and scope |
| --- | --- |
| Trace context | Optional W3C/OTEL trace parent for causal correlation. Parse/validate at the boundary; malformed or absent metadata yields an unparented operation. Client-supplied values remain untrusted correlation. Preserve span parentage and links; sampling/incompleteness is not inferred from an ID. |
| Game correlation | Optional typed session, run, operation, job, utterance/action, and state-revision IDs, only where that operation has them. They are high-cardinality log/span attributes, never metric labels or authority. Use the owning crate's canonical ID types; add no parallel ID types in `df-observe`. |
| Build provenance | Source, native, WASM, configuration, and content revisions from canonical `df_types::BuildIdentity` when available. Startup may lack game IDs but still records process/build identity. A supplied label describes a revision; it does not prove a build was tested or approved. |
| Development provenance | Task/attempt IDs only for a test or preview with that context. Keep these out of unrelated production operations. |
| Deadline/cancellation | Operation deadline and cancellation scope follow the operation only while work remains request-owned. Native dispatch snapshots correlation into its canonical task context and assigns the task's own deadline/cancellation owner when work becomes independently owned. These controls do not confer authorization. |

Keep the record envelope distinct from operation context. OTEL resource identity
describes the emitting service/process/build; instrumentation scope describes the
instrumentation library. Producer ID, record ID, source sequence, and ingest
position belong to capture/ingest records and batches, not to the ambient operation
context. Event/observed timestamps, severity, typed body/attributes, span IDs and
links remain typed OTEL record fields. Retry preserves producer/record/sequence
identity. Missing optional IDs remain absent; do not synthesize values from clocks,
current session state, or user payload.

Default records contain safe identifiers, revisions, counts, and classified
outcomes. Exclude credentials, raw audio, private player content, full prompts,
and secrets before serialization. Any scoped diagnostic capture must use its
separate explicit access policy. This contract applies equally to browser and
native producers; browser capture is best-effort and bounded, while native
durability is owned by the existing ingress/spool boundary. Overflow, refusal,
gaps, and unconfirmed acceptance remain visible; a pending wait is not a durable
receipt.

## Existing source boundary and consumers

At the input revision, `crates/df-observe/src/lib.rs` defines the canonical
`OperationContext` with `trace_parent: String` and `build: String`. `parent` accepts
the implemented version-00 traceparent shape and otherwise creates a root context;
`begin` attaches `fixture.build`; `OperationSpan` ends once and Drop records
`abandoned`. This is the current source shape, not evidence that every field family
above is implemented. In particular, `OperationContext` does not yet contain the
planned game IDs, typed `BuildIdentity`, or deadline/cancellation controls.

The existing concrete consumers are `crates/df-tools/src/fixture.rs` (reads
incoming `traceparent` metadata and calls `FixtureTelemetry::begin`),
`crates/df-tools/src/browser.rs` (records the transport fixture result), and
`crates/df-rpc-bridge/src/browser.rs` (creates connection/driver spans with an
empty parent when none is available). `df-observe::NativeProducer` uses
`BuildIdentity` to attach source/native/WASM/configuration/content revisions to
the OTEL resource. Its `CapturedBatch` and the existing `TelemetryIngress`,
`PendingReceipt`, and `IngestReceipt` in `df-observe/src/producer.rs` and
`df-observe/src/ingress.rs` own record submission and durability reporting;
`df-telemetry` owns local spooling/SQLite and its independent emergency path.
These source paths prove fixture/native capture contracts only. They do not prove
production `df-api`/`df-session` dispatch, browser buffering, durable export, or a
running gameplay trace; those crates are not present at this input revision.
The present native emergency path in `crates/df-telemetry/src/lib.rs` avoids OTEL
recursion, bounds fallback to 32 records/8 KiB, and writes safe source/error/
checkpoint/backlog/unconfirmed facts to its local file, then stderr if file I/O
fails. `crates/df-telemetry/src/query.rs` exposes retained emergency bytes with review
pages. These are existing native fallback facts, not browser-buffer limits or a
cross-platform guarantee.

## Bounded contract example

This literal is the current source type shape. A valid case carries only trace
correlation and a build label. The refusal case attempts to put actor authority on
the same type and must fail to compile; authorization is supplied separately by
the owning auth boundary.

```rust
#[derive(Clone)]
struct OperationContext {
    trace_parent: String,
    build: String,
}

fn valid() -> OperationContext {
    OperationContext {
        trace_parent: "00-11111111111111111111111111111111-2222222222222222-01".to_owned(),
        build: "fixture-build".to_owned(),
    }
}

fn main() {
    let context = valid();
    assert_eq!(context.trace_parent.len(), 55);
    assert_eq!(context.build, "fixture-build");
}
```

Compile-refusal case (expected compiler error, not a successful runtime check):

```rust
let _context = OperationContext {
    trace_parent: String::new(),
    build: "fixture-build".to_owned(),
    actor: "player-1",
};
```

The valid literal is checked against the exact two-field definition read from
`crates/df-observe/src/lib.rs`; the refusal case verifies that this public
correlation type has no actor field. Neither case authenticates a trace, checks
authorization, exercises asynchronous propagation, or qualifies a running
telemetry pipeline.

## Minimum follow-on work and unresolved facts

The minimum implementation prerequisite is the existing `df-observe` task-context
capture at native dispatch (catalog task `B-C-df-observe-I02`): use the canonical
native task context to snapshot the admitted operation context and retain trace
parent/link plus job/generation ownership after the request span ends. Its
consumer-side hook is `df-session` dispatch through the `EffectExecutor` boundary;
`df-server` supplies the executor. No `df-session`, `df-server`, `df-api`, or
production capture/buffer implementation exists at this revision, so the hook
cannot yet be connected or run here.

For the remaining browser diagnostic buffer (`B-C-df-observe-I03`), reuse this
field partition and the existing finite `TelemetryLimits`/ingress receipt meaning;
the browser buffer owner must separately freeze its WASM-safe item/byte limits,
overflow accounting, capture boundary, and upload receipt behavior before code.
This decision does not choose new limits or claim browser persistence. The concrete
OTEL schema mapping, supported browser capture boundary, precise newtyped IDs for
job/utterance, and runtime target feasibility remain implementation gates.

## Source and verification evidence

- Governing decisions: ADRs 0001–0005; `planning/coding-style.md`,
  `planning/observability.md`, `planning/runtime-reliability.md`,
  `planning/subsystem-architecture.md`, and `planning/subsystem-interfaces.md`.
- Source inspected at `5759dfc3a619e8ee68eecd6bbf3d4c5ebdd00c35`, including the
  concrete consumers and ingress/producer paths listed above.
- The exact literal and valid/refusal harness are retained under
  `artifacts/tmp/B-C-df-observe-D01-a1/`. Initial v4 receipt
  `development/evidence/fanout-20261001/wave-03/B-C-df-observe-D01/a1/checks/source-shape-v4.json`
  records FAIL because `rustfmt` was absent from
  the guard child's PATH; it stopped before formatting or compilation. Root then
  supplied readmission
  `development/evidence/fanout-20261001/wave-03/B-C-df-observe-D01/a1/root-tool-readmission-01.json`
  for direct use of the
  pinned cached binaries. Fresh v4 receipt
  `development/evidence/fanout-20261001/wave-03/B-C-df-observe-D01/a1/checks/source-shape-v4-readmission-01.json`
  records PASS: root-configured
  rustfmt check, Rust 2024 `rustc -D warnings` valid compilation and execution,
  and expected compiler rejection of an `actor` field. The refusal compiler output
  names unknown field `actor`. These are bounded source-shape checks, not Cargo,
  native/WASM, browser, provider, integration, or runtime-pipeline checks.
- All compile/execution checks use the immutable wave-02 v4 guard. Any HOLD,
  failure, or unperformed check remains explicit; no retry or resource-budget
  change is implied by this document.
