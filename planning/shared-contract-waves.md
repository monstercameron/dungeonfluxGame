# Shared contract waves and compatibility policy

Task/attempt: `B-G03-D01/a2`
Decision input: `01c2ccf09545877d65ffe490975b4d9da267cd67`  
Canonical reviewed contract: `CONTRACT-G03-001`  
Owners: `df-types`, `df-model`, `df-protocol`

## Decision and initial S00 scope

The first shared wave freezes only contracts already implemented and exercised at the decision input: supplied identity values; composite recovery ordering; paired native/WASM build provenance; additive common protobuf DTOs; and a narrow version/capability/payload compatibility example. It does not freeze all future game models or authorize production services. Existing `CONTRACT-G03-001` is canonical; later tasks link to and refine it instead of defining competing identities, revisions, wire messages, ledgers, or consuming fixtures.

`df-types` owns the pure primitives. Its distinct `SessionId`, `MemberId`, `ClientBindingId`, `RunId`, and `OperationId` accept exactly 16 supplied bytes with at least one nonzero byte; they preserve byte order and do not issue identifiers or confer identity, membership, authentication, uniqueness, or permissions. `RecoveryEpoch` is nonzero; `SessionRevision` orders `(epoch, sequence)` lexicographically, permits sequence zero, and reports sequence overflow rather than inventing an epoch. Recovery epoch allocation and durable nonregression remain with the later persistence/recovery owner. `BuildIdentity` carries five required, nonsecret labels (source, native, WASM, configuration, content), each 1–128 ASCII bytes from the documented allowlist. Typed validation errors identify the component and reason without retaining the supplied label. These values are caller-supplied provenance, not approval, rights, catalog validation, or evidence that a build ran.

`df-protocol` owns the additive `dungeonflux.public.v1` common schema and depends on no project crate. Presence is explicit for scalar/bytes/string values and represented by message presence for composite values; generated DTO construction does not validate domain requirements. `df-tools` owns only the native/WASM-compatible consuming fixture and descriptor-ledger checks. Its explicit mapping validates generated DTOs and actual Prost encoding/decoding. The `dungeonflux.experimental.contract.v1` compatibility wrapper is test-only, has no service, and accepts protocol revision 1; it requires explicit capability kind and requiredness, rejects missing/zero/unsupported revision, unspecified enum values, unknown required capabilities, and missing/unknown-only payloads. Unknown optional capabilities and unknown wire fields beside a known payload remain additive. Unknown fields cannot safely express a future required meaning to an old reader: use a required capability or a protocol revision that the old reader rejects. Malformed wire data rejects during decode.

| Contract concern | Owner | Current consumer / evidence boundary | Change compatibility rule | Executable check boundary |
| --- | --- | --- | --- | --- |
| Typed supplied IDs, revisions, provenance | `df-types` | `df-protocol` DTO mapping consumed by `df-tools` | Preserve distinct types and validation semantics; coordinated consumer review for semantic changes; never imply issuance or authority | `df-types` tests and mapping/round-trip cases in `shared_contracts` |
| Public common wire DTOs | `df-protocol` | Native generated DTO encode/decode in `df-tools` | Add fresh optional fields/enums while preserving existing number, name, type, cardinality, presence, oneof, and meaning; required semantics need explicit admission | Actual Prost encode/decode and old-wire/additive cases in `shared_contracts` |
| Version/capability/payload example | `df-tools` fixture | `shared_contracts` executable tests | Keep revision 1 behavior explicit; old readers may skip unknown optional capabilities but reject unknown required meanings and unsupported revisions | `shared_contracts` executable tests |
| Allocations and retirement | `df-protocol` descriptor + ledger; reviewed by `df-tools` | `schema_ledger` derives records from the actual generated descriptor | Never reuse retired number/name; change descriptor and ledger together as a reviewed contract; reserved declarations are permanent | `schema_ledger` compares actual descriptor and exercises mutations |
| Future feature-domain contracts | `df-model` with the named feature owner | No current consumer in this first wave; later waves admit only their named consumer boundary | Freeze exact state/offer/view/resolution needs after prerequisites are reviewed; coordinate all affected consumers and compatibility fixtures | Pending until the later wave names concrete source and checks |
| Formatting and target gates | Each affected crate owner | Maintainers reviewing the wave | Any changed consumer or generated descriptor joins the same review boundary; do not infer runtime support from cross-compilation | Workspace format, affected native/WASM Clippy, affected library WASM build |

These names identify current owners and consumers only. RPC service design, wire codecs at production API/client/persistence boundaries, identity issuance, authorization, durable recovery authority, and all full G03 game types remain pending. Pure shared/domain crates do not acquire clocks, SDKs, databases, providers, sockets, or runtime I/O. Error facts cross to native consumers; private payloads and credentials do not enter default diagnostics.

## Compatibility and change admission

The namespace boundary follows [Protocol and access](rpc-api.md#protocol-and-access) and [Debug, telemetry and compatibility](rpc-api.md#debug-telemetry-and-compatibility) in `planning/rpc-api.md`: public, admin, and telemetry APIs remain separate namespaces, and a breaking wire change uses a new namespace with an announced compatibility window. The revision/capability rejection in the current test-only fixture complements this governing rule; it does not replace namespace separation or the announced window. Concrete future services, compatibility-window duration, and rollout sequence are unselected and remain for the owning API wave.

A compatible additive change uses a fresh field or enum number, preserves existing field number/name/type/cardinality/presence/oneof meaning, and keeps old readers safe when they ignore unknown optional data. Requiredness, authorization, or behavior changes need explicit capability/revision admission and coordinated consumers; protobuf unknown-field tolerance alone is insufficient. A breaking wire change also requires a new namespace and an announced compatibility window under `planning/rpc-api.md`; choosing the revision or duration is a later API-wave decision. Never reuse a retired field or enum number/name. A retirement change updates the protobuf `reserved` declaration and durable ledger together, preserving the old number and name permanently. The ledger is reviewed source, not a generated file to refresh in order to bless a changed descriptor. The initial ledger has no retired allocations. Message reserved ranges exclude their end; enum reserved ranges include their end.

Admit each later shared contract in a bounded wave when its prerequisite decisions and reviewed canonical owners are named, then freeze only the exact fields and behaviors needed by the next consumer. Before admission, record the source/configuration revision, owning crate, exact consumers, presence and error behavior, compatibility/retirement effect, source-backed example, and affected native/WASM checks. Consumer changes, generated descriptors, fixtures, ledgers, and dependency checks must be coordinated at the same boundary. A later contract that changes the public meaning requires review of all consumers and compatibility fixtures; no silent field reinterpretation, broad future schema freeze, string-parsed errors, test-only duplicate of the current schema, or parallel implementation is permitted.

For each admitted wave, source-bound evidence identifies the commit, configuration, exact commands and exit results. Run the real affected consuming boundary and descriptor mutation checks; run formatting plus affected native and WASM lint/build gates where targets apply. A successful WASM cross-compile proves compilation only, not browser execution, audio, device behavior, or provider/production authority. A current fixture does not prove a production service or full game flow.

## Alternatives and unresolved facts

A single broad schema freeze for all planned features would freeze meanings before their owners and consumers exist, so it is deferred. Ad hoc per-feature IDs, duplicate test schemas, or stringly typed errors would make boundaries disagree and bypass the reviewed canonical contract, so consumers must reuse `CONTRACT-G03-001`. Relying on unknown protobuf fields to signal required future behavior is unsafe for old readers, so explicit required capabilities or revision rejection are required. Reusing the existing namespace for a breaking wire change is also rejected; the governing RPC policy requires a new namespace and announced compatibility window, while its duration and rollout remain open. Reusing retired allocations is rejected because old serialized data may still carry their former meanings.

Later waves must resolve concrete models and errors for their own use cases; public/admin RPC services and production mappings; authorization and trusted-principal/audience filtering; persistence-owned epoch allocation and recovery-head nonregression; and each feature's exact required state, offer, view, resolution, or stream types. Compatibility consumers and deployment/version skew policy beyond this fixture remain open. Full G03, S00/S01 completion, gameplay, service/provider authority, physical device/network behavior, allocation/heap bounds, audible audio, and retained-tab reload remain pending. The current fixture has no retired field, does not exercise a production consumer, and does not close the historical four-crate G01 audit.

## Required task acceptance and verification

Acceptance criterion 1 (preserved): “initial S00 scope not all future features”

Acceptance criterion 2 (preserved): “The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.”

Original verification 1 (preserved): “Freeze the cited source decision and a bounded contract example; compare initial S00 scope not all future features. Retain decision, alternatives and unresolved facts.”

Original verification 2 (preserved): “Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.”

At this source revision the bounded example is the canonical identity/recovery/build DTO mapping and revision/capability/payload fixture in `crates/df-tools/tests/shared_contracts.rs`; its descriptor source is `FILE_DESCRIPTOR_SET`, checked against `crates/df-protocol/proto/field-ledger.txt` by `crates/df-tools/tests/schema_ledger.rs`. The source-bound commands for this attempt are the focused native tests (including both named descriptor/consumer tests and `df-types` tests), workspace formatting, affected-package native and WASM Clippy, and affected-library WASM build. Capture the exact source/configuration identity, command, exit status, output, and artifact identity for each run. The build-fixture wrapper sets `CARGO_BUILD_JOBS=2`; it cannot be used for bounded builds without overriding its hardcoded setting, so invoke the cached Cargo toolchain directly with `--jobs 1`, offline mode, automatic rustup installation disabled, and the attempt-owned cache/build/scratch paths. No dependency downloads or paid calls are in scope.

Native consumer/descriptor execution and WASM compilation are separate results. No full G03/game, production, browser, audio, physical-device, provider, full-workspace WASM, or full G01 audit claim follows from these checks.

## B-C-df-types-I01 applicability and execution boundary

The reviewed I01 prerequisite correction retains `B-C-df-types-D01` and requires
both `CONTRACT-G03-001` and `B-G03-D01`. Its original acceptance and verification
clauses in the backlog remain verbatim, including empty, whitespace, and oversized
rejection. This is an applicability decision, not I01 dispatch or completion.
The five currently implemented, distinct byte kinds are `SessionId`, `MemberId`,
`ClientBindingId`, `RunId`, and `OperationId` in `crates/df-types/src/identity.rs`.
Their existing `from_bytes` accepts exactly 16 supplied bytes, rejects zero,
preserves order, and has no issuance or authority semantics. I01's selected text
ingress is exactly 32 lowercase ASCII hex characters representing those 16 bytes
in order. Measure UTF-8 byte length first; reject wrong lengths as
`InvalidLength { actual }`, any non-grammar byte at length 32 as `Malformed`,
and decoded zero as `Zero`. No prefix, separator, whitespace, case folding, or
Unicode normalization is admitted. The D01 parser is a decision model, not a
production constructor; I01 must freeze the public method/error shape and all
five kind mappings with its consumer before implementation. Distinct kinds must
remain noninterchangeable.

The original family also names client, job, utterance, and asset roles. A stable
`ClientBindingId` does not identify a transient connection or tab. The future
client connection/tab role belongs to the `B-C-df-client-D01` binding/freshness
contract and `B-C-df-session-D02` allocation/identity contract. Job identity
belongs to `B-C-df-session-D02`, `B-C-df-provider-api-D02`, and their later
implementation/consumer work. Utterance identity belongs to
`B-C-df-media-D02` and its speech/consumer work. Asset identity belongs to
`B-C-df-assets-D01` and its publication/consumer work. Each owner must freeze
its distinct semantic role and reuse canonical byte validation before introducing
a type; none is predeclared or aliased here. Independent review of the I01 frozen
brief must confirm that this mapping preserves the original task's full required
scope. If that cannot be established, I01 stays pending with the contract gap
explicit; the dependency correction alone cannot make it dispatchable.

At this source revision the only actual consuming identity mapping is the
test-only `crates/df-tools/tests/shared_contracts.rs`: it maps all five typed IDs
to the five `dungeonflux.public.v1` DTOs in
`crates/df-protocol/proto/common.proto`, and checks byte round trips, missing,
empty, short, long, and zero inputs. `crates/df-tools/tests/schema_ledger.rs`
checks the actual generated descriptor against the durable field ledger and
mutation cases. `df-api`, `df-client`, and `df-persistence` have no production
identity codec here. If I01 changes only the text constructor, the current wire
bytes and descriptor must remain identical; if a future boundary needs a wire
change, its owning protocol/API wave must admit and test that change separately.
I01 must test all five text mappings, wrong length including empty and oversized,
whitespace, uppercase, non-ASCII and malformed 32-byte inputs, zero, byte order,
and cross-kind compile-time separation. The actual shared-contract round trip and
descriptor ledger checks are the current compatibility probes, not production
consumer evidence.

For the later I01 implementation/review, pin the source revision, `Cargo.lock`,
`rust-toolchain.toml` (Rust 1.98.1), `rustfmt.toml`, configuration and owned build
root. Use the existing cached `RUSTUP_HOME=artifacts/cache/rustup` and
`CARGO_HOME=artifacts/cache/cargo`, set `RUSTUP_AUTO_INSTALL=0`,
`CARGO_NET_OFFLINE=true`, `CARGO_BUILD_JOBS=1`, and an attempt-owned
`CARGO_TARGET_DIR` under `artifacts/build/`. From the repository root, retain
commands, exit codes, output and artifact hashes for:

```text
cargo +1.98.1 fmt --all -- --check
cargo +1.98.1 test --locked --offline -p df-types --jobs 1
cargo +1.98.1 test --locked --offline -p df-tools --test shared_contracts --test schema_ledger --jobs 1
cargo +1.98.1 clippy --locked --offline -p df-types -p df-protocol -p df-tools --all-targets --jobs 1 -- -D warnings
cargo +1.98.1 clippy --locked --offline -p df-types -p df-protocol -p df-tools --all-targets --target wasm32-unknown-unknown --jobs 1 -- -D warnings
cargo +1.98.1 build --locked --offline -p df-types -p df-protocol -p df-tools --lib --target wasm32-unknown-unknown --jobs 1
```

The `df-tools` tests execute the native mapping and descriptor checks; the WASM
commands compile the affected dependency closure. These commands are pinned
future checks, not results of this planning correction. Production caller mapping,
future kind contracts, full G01/G02/G03 gates, browser/network/physical-device
observations, and S00/S01 completion retain their separate owners and evidence.
