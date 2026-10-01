# Shared contract waves and compatibility policy

Task/attempt: `B-G03-D01/a1`  
Decision input: `01c2ccf09545877d65ffe490975b4d9da267cd67`  
Canonical reviewed contract: `CONTRACT-G03-001`  
Owners: `df-types`, `df-model`, `df-protocol`

## Decision and initial S00 scope

The first shared wave freezes only contracts already implemented and exercised at the decision input: supplied identity values; composite recovery ordering; paired native/WASM build provenance; additive common protobuf DTOs; and a narrow version/capability/payload compatibility example. It does not freeze all future game models or authorize production services. Existing `CONTRACT-G03-001` is canonical; later tasks link to and refine it instead of defining competing identities, revisions, wire messages, ledgers, or consuming fixtures.

`df-types` owns the pure primitives. Its distinct `SessionId`, `MemberId`, `ClientBindingId`, `RunId`, and `OperationId` accept exactly 16 supplied bytes with at least one nonzero byte; they preserve byte order and do not issue identifiers or confer identity, membership, authentication, uniqueness, or permissions. `RecoveryEpoch` is nonzero; `SessionRevision` orders `(epoch, sequence)` lexicographically, permits sequence zero, and reports sequence overflow rather than inventing an epoch. Recovery epoch allocation and durable nonregression remain with the later persistence/recovery owner. `BuildIdentity` carries five required, nonsecret labels (source, native, WASM, configuration, content), each 1–128 ASCII bytes from the documented allowlist. Typed validation errors identify the component and reason without retaining the supplied label. These values are caller-supplied provenance, not approval, rights, catalog validation, or evidence that a build ran.

`df-protocol` owns the additive `dungeonflux.public.v1` common schema and depends on no project crate. Presence is explicit for scalar/bytes/string values and represented by message presence for composite values; generated DTO construction does not validate domain requirements. `df-tools` owns only the native/WASM-compatible consuming fixture and descriptor-ledger checks. Its explicit mapping validates generated DTOs and actual Prost encoding/decoding. The `dungeonflux.experimental.contract.v1` compatibility wrapper is test-only, has no service, and accepts protocol revision 1; it requires explicit capability kind and requiredness, rejects missing/zero/unsupported revision, unspecified enum values, unknown required capabilities, and missing/unknown-only payloads. Unknown optional capabilities and unknown wire fields beside a known payload remain additive. Unknown fields cannot safely express a future required meaning to an old reader: use a required capability or a protocol revision that the old reader rejects. Malformed wire data rejects during decode.

| Contract concern | Owner | Current consumer / evidence boundary | Error or compatibility behavior |
| --- | --- | --- | --- |
| Typed supplied IDs, revisions, provenance | `df-types` | `df-protocol` DTO mapping consumed by `df-tools` | Distinct typed errors; absent fields, invalid lengths/zero IDs, zero recovery epoch, overflow, and invalid labels reject |
| Public common wire DTOs | `df-protocol` | Native generated DTO encode/decode in `df-tools` | Additive optional presence; decoding alone is not validation; consumer performs explicit domain validation |
| Version/capability/payload example | `df-tools` fixture | `shared_contracts` executable tests | Revision 1 only; unknown optional additions accepted, unknown required meaning rejected, no implicit payload |
| Allocations and retirement | `df-protocol` descriptor + ledger; reviewed by `df-tools` | `schema_ledger` derives records from the actual generated descriptor | Duplicate, reserved, removed, renumbered, reused, or changed allocations fail; retired names and numbers remain reserved |

These names identify current owners and consumers only. RPC service design, wire codecs at production API/client/persistence boundaries, identity issuance, authorization, durable recovery authority, and all full G03 game types remain pending. Pure shared/domain crates do not acquire clocks, SDKs, databases, providers, sockets, or runtime I/O. Error facts cross to native consumers; private payloads and credentials do not enter default diagnostics.

## Compatibility and change admission

A compatible additive change uses a fresh field or enum number, preserves existing field number/name/type/cardinality/presence/oneof meaning, and keeps old readers safe when they ignore unknown optional data. Requiredness, authorization, or behavior changes need explicit capability/revision admission and coordinated consumers; protobuf unknown-field tolerance alone is insufficient. Never reuse a retired field or enum number/name. A retirement change updates the protobuf `reserved` declaration and durable ledger together, preserving the old number and name permanently. The ledger is reviewed source, not a generated file to refresh in order to bless a changed descriptor. The initial ledger has no retired allocations. Message reserved ranges exclude their end; enum reserved ranges include their end.

Admit each later shared contract in a bounded wave when its prerequisite decisions and reviewed canonical owners are named, then freeze only the exact fields and behaviors needed by the next consumer. Before admission, record the source/configuration revision, owning crate, exact consumers, presence and error behavior, compatibility/retirement effect, source-backed example, and affected native/WASM checks. Consumer changes, generated descriptors, fixtures, ledgers, and dependency checks must be coordinated at the same boundary. A later contract that changes the public meaning requires review of all consumers and compatibility fixtures; no silent field reinterpretation, broad future schema freeze, string-parsed errors, test-only duplicate of the current schema, or parallel implementation is permitted.

For each admitted wave, source-bound evidence identifies the commit, configuration, exact commands and exit results. Run the real affected consuming boundary and descriptor mutation checks; run formatting plus affected native and WASM lint/build gates where targets apply. A successful WASM cross-compile proves compilation only, not browser execution, audio, device behavior, or provider/production authority. A current fixture does not prove a production service or full game flow.

## Alternatives and unresolved facts

A single broad schema freeze for all planned features would freeze meanings before their owners and consumers exist, so it is deferred. Ad hoc per-feature IDs, duplicate test schemas, or stringly typed errors would make boundaries disagree and bypass the reviewed canonical contract, so consumers must reuse `CONTRACT-G03-001`. Relying on unknown protobuf fields to signal required future behavior is unsafe for old readers, so explicit required capabilities or revision rejection are required. Reusing retired allocations is rejected because old serialized data may still carry their former meanings.

Later waves must resolve concrete models and errors for their own use cases; public/admin RPC services and production mappings; authorization and trusted-principal/audience filtering; persistence-owned epoch allocation and recovery-head nonregression; and each feature's exact required state, offer, view, resolution, or stream types. Compatibility consumers and deployment/version skew policy beyond this fixture remain open. Full G03, S00/S01 completion, gameplay, service/provider authority, physical device/network behavior, allocation/heap bounds, audible audio, and retained-tab reload remain pending. The current fixture has no retired field, does not exercise a production consumer, and does not close the historical four-crate G01 audit.

## Required task acceptance and verification

Acceptance criterion 1 (preserved): “initial S00 scope not all future features”

Acceptance criterion 2 (preserved): “The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.”

Original verification 1 (preserved): “Freeze the cited source decision and a bounded contract example; compare initial S00 scope not all future features. Retain decision, alternatives and unresolved facts.”

Original verification 2 (preserved): “Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.”

At this source revision the bounded example is the canonical identity/recovery/build DTO mapping and revision/capability/payload fixture in `crates/df-tools/tests/shared_contracts.rs`; its descriptor source is `FILE_DESCRIPTOR_SET`, checked against `crates/df-protocol/proto/field-ledger.txt` by `crates/df-tools/tests/schema_ledger.rs`. The source-bound commands for this attempt are the focused native tests (including both named descriptor/consumer tests and `df-types` tests), workspace formatting, affected-package native and WASM Clippy, and affected-library WASM build. Capture the exact source/configuration identity, command, exit status, output, and artifact identity for each run. The build-fixture wrapper sets `CARGO_BUILD_JOBS=2`; it cannot be used for bounded builds without overriding its hardcoded setting, so invoke the cached Cargo toolchain directly with `--jobs 1`, offline mode, automatic rustup installation disabled, and the attempt-owned cache/build/scratch paths. No dependency downloads or paid calls are in scope.

Native consumer/descriptor execution and WASM compilation are separate results. No full G03/game, production, browser, audio, physical-device, provider, full-workspace WASM, or full G01 audit claim follows from these checks.
