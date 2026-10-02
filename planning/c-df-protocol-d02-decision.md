# Enum unknown values and field presence

Date: 2026-10-02
Task/attempt: `B-C-df-protocol-D02/a1`
Input revision: `d870db00dc349425e8f977bdfb20c6ffb24bfd3e`
Status: bounded planning decision; independent review and integration pending

## Decision and ownership

`df-protocol` owns generated wire DTOs and compatibility fixtures. It preserves
explicit scalar/enum presence and raw enum integers through decoding. `df-api`
and `df-client` own the explicit mapping and admission at their respective
boundaries; domain types remain with their canonical owners. This decision adds
no protobuf allocation, generated type, service, gameplay implementation or
production capability. It follows [RPC API, Debug, telemetry and compatibility](rpc-api.md),
[Subsystem interfaces, Projection, RPC and transport](subsystem-interfaces.md),
[Subsystem architecture, Shared and transport crates](subsystem-architecture.md),
and [Protobuf allocation policy](protobuf-allocation-policy.md).

For an enum with presence, distinguish absent (`None`), explicitly unspecified
(`Some(0)`), a supported nonzero value, and an unknown raw signed integer. Zero
never selects a behavior. Check presence before interpreting the integer and
use checked enum conversion; never replace missing/unknown values with zero or
use a defaulting generated accessor to decide authority. Preserve the raw unknown
integer in a typed outcome rather than pretending it is a known variant.

Required fields reject absence with a typed missing-field outcome. Present zero,
false, and empty values are validated according to that field's contract, not
rejected or accepted merely because they are defaults. Existing examples are
`SessionRevision.sequence`: explicit zero is valid, absence is invalid;
`RecoveryEpoch.value`: both absence and zero reject with distinct outcomes; and
`Capability.required`: absence rejects, explicit false means optional.

Unsupported required capabilities reject compatibility before dependent work.
An explicitly optional unknown capability can be skipped only where the owning
contract permits that behavior; retain the raw integer as a classified fact.
Missing capability kind and present unspecified kind both reject distinctly,
even for optional capabilities. Unknown command/action/control variants never
become a default command or permission. Unknown presentation variants require
the separately approved fallback policy. No generic fallback is authorized here.

Messages and known oneofs retain their generated presence. A required message or
known oneof absent after decoding rejects. An unknown future oneof alternative
may appear as no recognized alternative to an older decoder; absence alone
cannot identify its cause. Required capability/version negotiation must guard
that boundary. Repeated fields and proto3 scalars without explicit presence do
not distinguish absent from empty/default; do not claim they do. Where that
distinction matters, the schema owner must review explicit presence in a later
numbering/compatibility change. This decision does not change existing schemas.

Pure mapping returns typed facts and rejections. The native/browser consumer
owns bounded diagnostics through the shared observation path; no SDK, logger,
clock, socket, database or private payload enters this example. Authority and
audience filtering remain independent of compatibility and trace context.

## Existing source and next consumer

At the input revision, `crates/df-protocol/proto/contract_fixture.proto` already
defines optional enum `Capability.kind` and optional bool `required`, with zero
unspecified and `COMPOSITE_RECOVERY = 1`. `common.proto` has optional revision
sequence and epoch value. `crates/df-tools/tests/shared_contracts.rs` is the
existing test-only source consumer: `consume` reads both options, performs
checked enum conversion, refuses unknown required values and skips optional
unknown values; `read_revision` preserves explicit sequence zero. This source
comparison is not a claim that those generated/prost tests ran in this attempt.

The existing compatibility test already round-trips positive unknown values with
false/true required markers. It directly constructs absent kind, present zero
and absent requirement refusals; that alone does not assert those options after
protobuf decoding. It also tests a future unknown oneof as missing payload.
The minimum next source task belongs to the existing `df-protocol` fixture owner
and `df-tools` test owner: extend `crates/df-tools/tests/shared_contracts.rs` to
assert the decoded raw options for absent kind, present zero, unknown positive/
negative integers, and absent/false/true required markers, then reuse `consume`
for their outcomes. Reuse existing cases and
`crates/df-protocol/proto/contract_fixture.proto` unchanged. Do not copy this
example into production or create a second enum registry. Future `df-api` and
`df-client` mappings consume the generated DTOs after their own frozen waves;
those crates are not present at this input revision and this task does not add
them. Production oneof/version negotiation, diagnostic bounds and cross-version
native/WASM behavior remain separately scoped gates.

## Alternatives and unresolved facts

- Default missing and unknown enum values to unspecified: rejected because it
  destroys the original distinction and can select unintended behavior.
- Reject every unknown value: rejected as the general compatibility policy;
  the existing fixture explicitly permits unknown optional capabilities.
- Ignore every unknown value: rejected because required capabilities and commands
  cannot be safely admitted without understanding their semantics.
- Change all scalars to optional now: rejected as an unreviewed schema change;
  presence changes require the single schema owner's compatibility review.

The example below proves classification and admission of already-decoded values
only. It does not prove protobuf preservation, descriptor effective presence,
unknown-field forwarding, old/new oneof decoding, production DTO mappings,
authorization, transport modes or browser feasibility. Each needs its actual
source boundary and reviewed version/consumer contract.

## Exact bounded Rust example

All types are private fixture types. `CapabilityKind` mirrors only the two
existing fixture numbers; `Unknown(i32)` is an observation, never a production
capability. `SkipUnknown` retains the fact without authorizing its behavior.
Finite assertions are the executable valid/refusal oracle; printed labels are
CLI fixture output, not application logging.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CapabilityKind {
    Absent,
    Unspecified,
    CompositeRecovery,
    Unknown(i32),
}

fn classify_kind(raw: Option<i32>) -> CapabilityKind {
    match raw {
        None => CapabilityKind::Absent,
        Some(0) => CapabilityKind::Unspecified,
        Some(1) => CapabilityKind::CompositeRecovery,
        Some(value) => CapabilityKind::Unknown(value),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum CapabilityOutcome {
    Supported,
    SkipUnknown(i32),
}

#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    MissingKind,
    MissingRequirement,
    UnspecifiedKind,
    UnknownRequired(i32),
    MissingSequence,
}

fn consume_capability(
    raw: Option<i32>,
    required: Option<bool>,
) -> Result<CapabilityOutcome, Refusal> {
    let kind = raw.ok_or(Refusal::MissingKind)?;
    let required = required.ok_or(Refusal::MissingRequirement)?;
    match classify_kind(Some(kind)) {
        CapabilityKind::Unspecified => Err(Refusal::UnspecifiedKind),
        CapabilityKind::CompositeRecovery => Ok(CapabilityOutcome::Supported),
        CapabilityKind::Unknown(value) if required => Err(Refusal::UnknownRequired(value)),
        CapabilityKind::Unknown(value) => Ok(CapabilityOutcome::SkipUnknown(value)),
        CapabilityKind::Absent => Err(Refusal::MissingKind),
    }
}

fn consume_sequence(raw: Option<u64>) -> Result<u64, Refusal> {
    raw.ok_or(Refusal::MissingSequence)
}

fn main() {
    for (raw, expected) in [
        (None, CapabilityKind::Absent),
        (Some(0), CapabilityKind::Unspecified),
        (Some(1), CapabilityKind::CompositeRecovery),
        (Some(99), CapabilityKind::Unknown(99)),
        (Some(-1), CapabilityKind::Unknown(-1)),
    ] {
        let actual = classify_kind(raw);
        assert_eq!(actual, expected);
        println!("enum {raw:?}: {actual:?}");
    }

    for (raw, required, expected) in [
        (None, Some(false), Err(Refusal::MissingKind)),
        (Some(0), Some(false), Err(Refusal::UnspecifiedKind)),
        (Some(1), None, Err(Refusal::MissingRequirement)),
        (Some(1), Some(false), Ok(CapabilityOutcome::Supported)),
        (Some(1), Some(true), Ok(CapabilityOutcome::Supported)),
        (Some(99), None, Err(Refusal::MissingRequirement)),
        (
            Some(99),
            Some(false),
            Ok(CapabilityOutcome::SkipUnknown(99)),
        ),
        (Some(99), Some(true), Err(Refusal::UnknownRequired(99))),
        (
            Some(-1),
            Some(false),
            Ok(CapabilityOutcome::SkipUnknown(-1)),
        ),
        (Some(-1), Some(true), Err(Refusal::UnknownRequired(-1))),
    ] {
        let actual = consume_capability(raw, required);
        assert_eq!(actual, expected);
        println!("capability {raw:?}, required {required:?}: {actual:?}");
    }

    for (raw, expected) in [(None, Err(Refusal::MissingSequence)), (Some(0), Ok(0))] {
        let actual = consume_sequence(raw);
        assert_eq!(actual, expected);
        println!("sequence {raw:?}: {actual:?}");
    }
}
```

## Verification and original criteria

Original acceptance, unchanged:

1. “absent zero and unknown remain distinguishable”
2. “The named outcome has actual source/build-bound evidence; unsupported,
   pending, failed and unperformed checks remain explicit.”

Original verification, unchanged:

1. “Freeze the cited source decision and a bounded contract example; compare
   absent zero and unknown remain distinguishable. Retain decision, alternatives
   and unresolved facts.”
2. “Exact executable commands: TBD at G01 and scoped prerequisite resolution;
   this planned procedure is not a claim that Rust/browser/provider checks ran.”

After the root's explicit finite-check release, the exact Markdown literal was
extracted, formatted by the pinned Rust 1.98.1 `rustfmt` with the root
`rustfmt.toml`, copied back verbatim, and checked with that formatter. Compilation
with `rustc --edition=2024 -Dwarnings -C opt-level=0 -C debuginfo=0` and execution
passed all 17 assertions. No compiler or assertion repair was needed. The frozen
v6 guard admitted each command at at least 40% free-memory proxy, with unchanged
256 MiB/60-second bounds and the single shared compiler lock. Each owned command
exited zero and left no owned process group. Compilation's sampled guard/group
peak was 148,963,328 bytes; this is an observed finite fixture run, not a capacity
qualification.

`exact-example.rs` retains the executed Rust source. `verified-identity.json`
binds its byte-for-byte equality to this document and records source/tool/
configuration/binary hashes. Receipts `12-rustfmt-write.json`,
`14-rustfmt-check.json`, `15-rustc.json` and `16-execute.json` retain actual argv,
stdout/stderr, resource outcomes and observed exits. `handoff.json` identifies the
clean sole-path submitted commit; `manifest.json` seals retained evidence using
64 KiB hash chunks. All generated output stays in the attempt scratch/build
roots. The original criteria are evidenced at this bounded literal boundary;
independent review and production acceptance are still pending.

No Cargo, Clippy, production protobuf/generator qualification, native/WASM crate
build, browser, provider, gameplay integration, device or audio check is performed
by this bounded example. Independent frontier review and sequential integration
checks remain pending. The durable attempt evidence root is
`development/evidence/fanout-20261001/wave-03/B-C-df-protocol-D02/a1/`.
