# Experimental transport protobuf policy

Date: 2026-09-30
Task/attempt: `B-G02-D02-a1`
Decision input revision: `4ba37943691f3a01a49b8238983e82527bec91bf`
Submitted revision: recorded in the attempt handoff after commit

## Frozen decision

`dungeonflux.experimental.transport.v1.TransportFixture` and its `Sample` are an
S00 transport experiment only. Its four methods exercise unary,
server-streaming, client-streaming, and bidirectional streaming; they are not
production game RPCs, a production namespace/version, or a source of production
field-number allocation. Keep this fixture distinct from G03 production
contracts. `df-protocol` remains the sole protobuf numbering and generated-code
owner, `df-tools` consumes the fixture, and `df-rpc-bridge` transports an ordered
byte stream without knowing protobuf services or game meaning.

The current fixture declaration is the complete bounded outcome for this task:

```proto
package dungeonflux.experimental.transport.v1;
service TransportFixture {
  rpc Unary(Sample) returns (Sample);
  rpc ServerStream(Sample) returns (stream Sample);
  rpc ClientStream(stream Sample) returns (Sample);
  rpc Bidi(stream Sample) returns (stream Sample);
}
```

The excerpt names the four shapes, not a new schema proposal. Preserve the
canonical schema, descriptor, and field ledger. Do not add speculative messages,
services, version negotiation, production mappings, or duplicate definitions.

## Current-source comparison

At the frozen input revision, the current source already expresses this boundary:

- `crates/df-protocol/proto/transport_fixture.proto` declares only
  `dungeonflux.experimental.transport.v1.TransportFixture` and labels it a
  frozen S00 experiment. `Sample` carries synthetic sequence/payload/behavior and
  fixture diagnostics only.
- `crates/df-protocol/src/lib.rs` includes generated code under
  `transport_fixture`; `crates/df-protocol/build.rs` is the generation owner and
  compiles the transport, common, and compatibility-fixture schemas. The shared
  `proto/field-ledger.txt` retains descriptor-derived names, numbers, presence,
  service and method shapes. `df-tools/tests/schema_ledger.rs` checks the ledger
  against the generated descriptor.
- `crates/df-tools` is the fixture consumer: its native service and browser
  fixture use the generated `TransportFixture` client/server. The current test
  and fixture paths demonstrate the four modes and synthetic failures, but do
  not create production game methods.
- `crates/df-rpc-bridge/src/lib.rs` exposes native accept/browser connect and
  bounded byte-stream resources. The adapters carry HTTP/2 bytes; the bridge
  source does not import or dispatch a protobuf service. This is the intended
  generic transport boundary.
- `crates/df-protocol/proto/common.proto` separately declares public identity and
  revision/build messages. It is not the production RPC service contract. The
  planned `planning/rpc-api.md` production service list is pseudocode: it
  explicitly says field numbers and generated source are not selected. G03 and
  later source/type decisions remain open.

This is a source comparison, not proof that the browser/runtime feasibility gate
or an integrated production API has passed. Existing source already supplies
this fixture; this submission records its scope and a bounded policy example
without modifying canonical source.

## Alternatives and unresolved facts

- **Promote the fixture to production or reuse its field numbers.** Rejected:
  that would conflate a synthetic S00 fixture with G03 production contracts and
  bypass the sole `df-protocol` numbering owner.
- **Add a second experimental service or a compatibility/negotiation envelope.**
  Rejected: the existing four-method fixture suffices to exercise the required
  shapes, while no new contract is needed to state its scope.
- **Put method/schema knowledge into `df-rpc-bridge`.** Rejected: the bridge's
  generic ordered byte-stream boundary is already separate from generated
  service ownership and game semantics.
- **Claim transport qualification from the generated schema or example.**
  Rejected: this policy/example cannot prove an actual browser HTTP/2 executor,
  WebSocket memory/flow-credit bounds, real cross-runtime all-mode behavior,
  physical browser/device support, or production integration.

Open facts remain with G02/G03 and the owning later tasks: browser executor and
client compatibility, real browser all-mode/status/trailer/deadline/cancel/
half-close behavior, browser receive-memory and flow-credit qualification,
production message/service numbering, and production auth/session integration.
The planned browser memory limits in `planning/rpc-transport.md` are requirements,
not measured evidence. This task performed no Cargo build, browser, provider,
physical-device, or integrated-service check.

## Bounded pure-Rust policy example

This dependency-free example models only the frozen naming/scope decision: an
explicit S00 experimental call can exercise any of the four fixture modes; the
same experimental call cannot be admitted as a production game RPC. The enum
input is an already-classified policy case for illustration. It is not a
request parser, production admission hook, authorization check, schema validator,
transport implementation, or proof that the running service enforces this rule.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RpcMode {
    Unary,
    ServerStreaming,
    ClientStreaming,
    BidirectionalStreaming,
}

#[derive(Clone, Copy, Debug)]
enum Boundary {
    S00ExperimentalFixture,
    ProductionGameRpc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PolicyDecision {
    ExerciseFixture(RpcMode),
    RejectExperimentalProductionUse(RpcMode),
}

fn decide(boundary: Boundary, mode: RpcMode) -> PolicyDecision {
    match boundary {
        Boundary::S00ExperimentalFixture => PolicyDecision::ExerciseFixture(mode),
        Boundary::ProductionGameRpc => PolicyDecision::RejectExperimentalProductionUse(mode),
    }
}

fn main() {
    let modes = [
        RpcMode::Unary,
        RpcMode::ServerStreaming,
        RpcMode::ClientStreaming,
        RpcMode::BidirectionalStreaming,
    ];

    for mode in modes {
        let fixture = decide(Boundary::S00ExperimentalFixture, mode);
        assert_eq!(fixture, PolicyDecision::ExerciseFixture(mode));
        println!("fixture {mode:?}: {fixture:?}");

        let production = decide(Boundary::ProductionGameRpc, mode);
        assert_eq!(
            production,
            PolicyDecision::RejectExperimentalProductionUse(mode)
        );
        println!("production {mode:?}: {production:?}");
    }
}
```

The retained run compiles this exact fenced source with the cached Rust 1.98.1
compiler in edition 2024, formats it with the pinned formatter, and executes the
native binary. Evidence records source/config/compiler/example/binary identities,
commands, exit codes, stdout and stderr. Its passing assertions establish only
that this small policy function maps the eight supplied classification/mode
pairs as written.

## Acceptance and verification record

Original acceptance criterion 1: “distinct from G03 production”. Evidence:
current schema/consumer/bridge comparison above; the S00-only policy and the
example's production rejection. No production API, numbering, or integration is
claimed.

Original acceptance criterion 2: “The named outcome has actual source/build-bound
evidence; unsupported, pending, failed and unperformed checks remain explicit.”
Evidence: the task input and current canonical file hashes, submitted document
and example hashes, exact compiler/formatter executable identity, compile/run
commands and results are retained in the attempt worker output. The example is a
standalone policy proof. Browser, physical device, provider, Cargo, production
service and integrated behavior checks are explicitly unperformed.

Original verification 1: “Freeze the cited source decision and a bounded contract
example; compare distinct from G03 production. Retain decision, alternatives and
unresolved facts.” The frozen decision, alternatives, current-source comparison,
open facts, and bounded example are above; source identities and run evidence
are retained in the handoff output.

Original verification 2: “Exact executable commands: TBD at G01 and scoped
prerequisite resolution; this planned procedure is not a claim that Rust/browser/
provider checks ran.” Only the dependency-free Rust policy example was compiled
and executed under the scoped cached compiler. No Cargo, browser, provider,
physical-device, or production qualification command was run.

Integration hook owner: coordinator, for sequential merge and original whole
decision acceptance. No production integration is asserted. Independent frontier
review of this exact boundary/example and resulting mainline acceptance remain
pending.
