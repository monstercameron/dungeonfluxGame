# Protobuf field allocation policy

- Date: 2026-10-01
- Task/attempt: `B-G03-D02/a1`
- Input revision: `4ba37943691f3a01a49b8238983e82527bec91bf`
- Status: frozen planning decision; production ownership metadata and additional lint coverage remain open

## Decision

`df-protocol` is the sole owner of protobuf source files, generated RPC/schema
contracts, and the canonical descriptor-derived allocation ledger at
`crates/df-protocol/proto/field-ledger.txt`. It owns numbering/version changes
and coordinates the affected `df-types` and `df-model` contracts before a schema
change. `df-types` owns the small domain IDs, revisions, units, and provenance
values; `df-model` owns game/domain and persisted document types. Those crates
map to transport DTOs at their boundaries; neither duplicates protobuf
allocation state or edits generated output. RPC and persistence consumers map
wire DTOs explicitly. The domain types do not acquire a serialization
framework. This follows the planned crate boundaries in
[Subsystem architecture](subsystem-architecture.md), the shared-contract rules
in [Subsystem interfaces](subsystem-interfaces.md), and the single-owner rule in
[RPC API](rpc-api.md).

`common.proto` remains the shared `dungeonflux.public.v1` schema for the current
common identities, recovery epoch/revision, and build provenance. It is not an
unversioned bucket for future gameplay messages. The existing
`field-ledger.txt` remains the canonical implementation record for the actual
compiled schema: file/package/syntax, messages, field number/name/type/
cardinality/presence/target/oneof, enum/value allocations, oneofs, services,
methods, and protobuf reservations. The build emits the descriptor set and
`df-tools/tests/schema_ledger.rs` compares that descriptor against this ledger.
This policy document defines responsibility and review rules; it does not add a
second registry, generator, production API, or schema.

Allocate only a legal, unused number in the owning message. Protobuf field
numbers are 1 through 536,870,911; 19,000 through 19,999 are reserved by
protobuf and unavailable. Existing and retired numbers and names are permanently
occupied. An allocation or meaning/type/presence change requires prior review by
the `df-protocol` owner plus the affected `df-types`/`df-model` owner(s), an
explicit compatibility decision, and a reviewed update to the source schema and
descriptor-derived ledger in the same change. Approval does not authorize reuse
or a silent meaning change. New optional fields are additive where compatible;
use explicit presence when missing differs from zero/false. Breaking changes use
a new namespace and an announced compatibility window.

Retirement is append-only. Remove a retired field from active use only in a
reviewed compatible change, then preserve both its exact name and number in the
message's protobuf `reserved` declarations and in the canonical ledger's
reserved records forever. Never reassign either, including across later schema
versions. Enum values follow the same permanent name/number reservation rule;
zero remains the unspecified enum value. Message reserved-range ends are
exclusive; enum reserved-range ends are inclusive, matching descriptor
semantics. A service or method retirement has no protobuf `reserved` syntax:
keep the retired fully qualified service/method name in the ledger's durable
history/review record, never recreate that name with changed meaning, and use a
new versioned service name for a replacement. Removing a service/method is a
compatibility change requiring the announced window; the current normalized
ledger only reflects the present descriptor and does not itself preserve
historical service/method names.

Every proposed allocation records the owning crate/team, fully qualified
file/package/message or enum/service path, name, number, type/cardinality,
effective presence, oneof, compatibility rationale, affected consumers, prior
owner approval reference, and source/build identity. The current ledger format
has no owner or approval-reference fields, so those facts must be supplied by
the reviewed change and its provenance until a coordinated canonical format
extension is approved. Do not claim the existing ledger already enforces those
metadata fields.

## Alternatives considered

- Let each consumer crate number its own messages: rejected because DTO/schema
  ownership would fragment and permit duplicate or incompatible wire contracts.
- Infer allocations from `.proto` files or regenerate the ledger automatically:
  rejected as approval. Generation can report descriptors, but cannot authorize
  a changed allocation; the explicit reviewed ledger remains the compatibility
  baseline.
- Reuse a removed tag/name after a compatibility window: rejected because old
  serialized data can still carry it and reinterpretation is unsafe.
- Put all evolving gameplay and admin contracts into `common.proto`: rejected;
  common identifiers/provenance stay narrow, and versioned feature namespaces
  keep ownership and compatibility decisions explicit.

## Current implementation comparison and precise gaps

At the input revision, `crates/df-protocol/build.rs` compiles exactly
`transport_fixture.proto`, `common.proto`, and `contract_fixture.proto`, emits
`FILE_DESCRIPTOR_SET`, and turns transport generation off for these fixtures.
The current actual ledger has those three files, 12 messages, 2 enums, 6 enum
values, 29 fields, 16 oneofs, 1 service, and 4 methods. The service and its four
streaming modes are explicitly experimental fixtures; they are not production
service registration. The common schema currently contains five ID messages,
`RecoveryEpoch`, `SessionRevision`, and `BuildIdentity`; the descriptor-derived
ledger confirms their current field numbers and presence.

The actual `schema_ledger.rs` normalizes the generated descriptor and checks
current allocations, field and enum duplicate names/numbers, active allocations
against declared reserved names/ranges, ledger equality, malformed descriptor
bytes, and mutations for field collision, removal, rename, type/presence change,
and changed number. Its hypothetical field-retirement test removes
`SessionRevision.sequence`, reserves number 2/name `sequence`, rejects a reused
number/name, and detects forgotten field reservations. The ordinary common
schema has no retired fields; this fixture is hypothetical rather than proof of
a historical migration.

Coverage gaps remain explicit:

- **Effective presence:** the ledger serializes `proto3_optional` as
  `presence=true/false`, and its fixture catches a `proto3_optional` mutation.
  It does not compute a general effective-presence model from syntax, label,
  message/oneof membership, and optional presence. In particular, message
  presence is represented by `target`/label and oneof layout, not a single
  effective-presence verdict; repeated fields and scalar defaults also need
  distinct semantics. The current comparison proves descriptor-record equality,
  not semantic validation of every presence combination.
- **Enum retirement:** active enum names/numbers and declared enum reservations
  are normalized, and active values that conflict with those reservations
  reject. The explicit retirement/reuse/forgotten-reservation mutation test
  covers a field only, not enum retirement. The current actual schemas have no
  retired enum values, so it does not prove a real enum migration.
- **Service/method retirement:** the current descriptor ledger records service
  and method names, signatures, and streaming flags, so an unreviewed change
  differs from the baseline. It has no durable retired-service/method registry,
  protobuf `reserved` declaration, or service-retirement/reintroduction test.
  Regenerating the ledger after deletion could bless that deletion unless the
  compatibility review/history is independently retained.
- **Number legality:** duplicate allocation and conflict with declared
  descriptor reservations are checked, but the code does not enforce the
  global protobuf field-number domain or 19,000–19,999 exclusion for every
  active field. No test mutates a field to zero, a value above 536,870,911, or
  the protobuf-reserved 19,000–19,999 block. Existing schemas do not declare
  those reserved ranges, so the current reserved-allocation branch does not
  cover this global rule. The current tests therefore do not establish full
  number-range validation.
- **Ownership/approval metadata:** normalized records have schema identity but
  no crate owner, prior approval reference, or compatibility rationale. A
  baseline equality check cannot prove that the correct owner reviewed an
  update.

These are precise current gaps, not claims that current descriptor comparison
has no value. Their production lint/test implementation is a separate scoped
follow-up; this planning task does not change canonical code or schemas.

## Bounded policy example

This dependency-free Rust example demonstrates only the decision rule: accept an
unused legal candidate only with prior owner approval; reject the protobuf
reserved interval, an out-of-domain number, and any number already present in
the active-or-retired allocation set. The caller-supplied approval boolean is a
fixture, not an approval system. The example is not a generator, descriptor
validator, implementation of `field-ledger.txt`, or proof of production
integration.

```rust
const MAX_FIELD_NUMBER: u32 = 536_870_911;
const RESERVED_START: u32 = 19_000;
const RESERVED_END: u32 = 19_999;

#[derive(Debug, PartialEq, Eq)]
enum Decision {
    Admit,
    OwnerApprovalRequired,
    RejectReservedNumber,
    RejectOutOfRange,
    RejectReusedNumber,
}

fn decide(number: u32, occupied: &[u32], owner_approved: bool) -> Decision {
    if number == 0 || number > MAX_FIELD_NUMBER {
        return Decision::RejectOutOfRange;
    }
    if (RESERVED_START..=RESERVED_END).contains(&number) {
        return Decision::RejectReservedNumber;
    }
    if occupied.contains(&number) {
        return Decision::RejectReusedNumber;
    }
    if !owner_approved {
        return Decision::OwnerApprovalRequired;
    }
    Decision::Admit
}

fn main() {
    let occupied = [1, 2, 3];
    let cases = [
        (
            "unused legal number with prior approval",
            6,
            true,
            Decision::Admit,
        ),
        (
            "unused legal number without prior approval",
            7,
            false,
            Decision::OwnerApprovalRequired,
        ),
        (
            "protobuf-reserved number",
            19_000,
            true,
            Decision::RejectReservedNumber,
        ),
        (
            "number above protobuf range",
            536_870_912,
            true,
            Decision::RejectOutOfRange,
        ),
        (
            "active-or-retired number reuse",
            2,
            true,
            Decision::RejectReusedNumber,
        ),
    ];

    for (label, number, approved, expected) in cases {
        let actual = decide(number, &occupied, approved);
        assert_eq!(actual, expected, "{label}");
        println!("{label}: {actual:?}");
    }
}
```

## Verification evidence and limits

The embedded example was extracted without modification to the attempt scratch
root, formatted with the repository `rustfmt.toml`, compiled directly with the
cached Rust `1.98.1` compiler (`--edition 2024`), and executed on the observed
`aarch64-apple-darwin` host. Exact source, formatter configuration, compiler and
formatter binaries, command lines, exit codes, stdout/stderr, elapsed time, and
binary hashes are retained in the attempt output. This is a bounded pure policy
example only; it is not a Cargo build or a production descriptor/schema test.

No Cargo build, workspace formatter/lint, WASM compile/browser run, provider,
audio, device, or application integration check was run for this task. No
production schema, application code, shared types, manifest, or Cargo file was
changed. Independent frontier execution/review and sequential integration of the
submitted revision remain pending; the example cannot establish the production
generator or integrated behavior.

Original acceptance criterion 1, unchanged: “protobuf ownership explicit”. This
document records the `df-protocol` wire-schema/ledger owner and the adjacent
`df-types`/`df-model` contract responsibilities and review boundary.

Original acceptance criterion 2, unchanged: “The named outcome has actual
source/build-bound evidence; unsupported, pending, failed and unperformed checks
remain explicit.” The evidence identifies the input and submitted commit,
document/extracted-source/config/compiler/binary hashes and recorded execution;
production acceptance and independent review are still pending.

Original verification 1, unchanged: “Freeze the cited source decision and a
bounded contract example; compare protobuf ownership explicit. Retain decision,
alternatives and unresolved facts.” Decision, alternatives, present canonical
ledger comparison, example, gaps, and pending review are stated above.

Original verification 2, unchanged: “Exact executable commands: TBD at G01 and
scoped prerequisite resolution; this planned procedure is not a claim that Rust/
browser/provider checks ran.” This attempt supplies and runs only its scoped
example commands below; it does not claim the original broader checks ran.

### Reproduction commands and retained identity

The attempt scratch root is
`/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G03-D02-a1/`. The
retained `verification.json` records the actual argv and complete process
results. The commands used the repository-owned rustup cache, disabled
installation and Cargo network access, and wrote all generated files under the
attempt scratch root. No Cargo command was invoked.

Exact commands (the recorded argv in `verification.json` is authoritative):

```sh
env RUSTUP_HOME=/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/rustup \
  CARGO_HOME=/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/cargo \
  TMPDIR=/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G03-D02-a1 \
  RUSTUP_AUTO_INSTALL=0 RUSTUP_TOOLCHAIN=1.98.1 CARGO_NET_OFFLINE=true \
  PATH=/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin \
  /Users/earlcameron/Desktop/dungeonflux/artifacts/cache/rustup/toolchains/1.98.1-aarch64-apple-darwin/bin/rustfmt \
  --edition 2024 --check \
  --config-path /Users/earlcameron/Desktop/dungeonflux/artifacts/worktrees/ledger_policy_implementation/rustfmt.toml \
  /Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G03-D02-a1/allocation_policy_example.rs

env RUSTUP_HOME=/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/rustup \
  CARGO_HOME=/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/cargo \
  TMPDIR=/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G03-D02-a1 \
  RUSTUP_AUTO_INSTALL=0 RUSTUP_TOOLCHAIN=1.98.1 CARGO_NET_OFFLINE=true \
  PATH=/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin \
  /Users/earlcameron/Desktop/dungeonflux/artifacts/cache/rustup/toolchains/1.98.1-aarch64-apple-darwin/bin/rustc \
  --edition=2024 -C opt-level=0 -C debuginfo=0 \
  /Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G03-D02-a1/allocation_policy_example.rs \
  -o /Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G03-D02-a1/allocation_policy_example

/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G03-D02-a1/allocation_policy_example
```

The recorded compiler was `rustc 1.98.1 (48a229cea 2026-09-01)`, host
`aarch64-apple-darwin`, LLVM `22.1.8`; executable SHA-256
`766eda9d8f53afd6fc7f27b3cd2e444dd22afacb5afa710a5625fc8e45b8c941`.
The repository formatter config SHA-256 is
`7a142cbd3f5e9c09c6dc4a1850e053415530eb5450d27983dc26c75230b8c5ba`.
The passing run recorded these SHA-256 identities: extracted example
`94a3cc85016810b88b7a9bbf614e98a8738c6730646fe6beec2dd13d063bd8e2`,
`rustfmt` executable
`98e8da71078a8b5710f1818c98da25d92de162a122bd9a6db222ca929e545468`, and
binary
`87f6513732ad2eee0e1a2319858e34eb08aecca831a8269757fec821b9f5f68d`.
`verification.json` records the document SHA-256, exact argv, all exits,
stdout/stderr, elapsed time, and child-resource observation. The native run
exited `0` within the 30-second and 0.12-GiB bounds and printed:

```text
unused legal number with prior approval: Admit
unused legal number without prior approval: OwnerApprovalRequired
protobuf-reserved number: RejectReservedNumber
number above protobuf range: RejectOutOfRange
active-or-retired number reuse: RejectReusedNumber
```
