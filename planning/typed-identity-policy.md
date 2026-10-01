# Typed identity policy

Task/attempt: `B-C-df-types-D01/a1`

Decision input: `aac3d48029f638e5dd15e3cb8a5db96cce1f2fb1`

Owner: `df-types` for canonical primitive identity types; the consuming owner selects
any later semantic identity types with its frozen contract.

Governing source sections: [Subsystem architecture](subsystem-architecture.md)
(Design; Shared and transport crates; Server crates; Browser crates; Tooling crates;
Integration and refinement; Commercial service composition), [Subsystem
interfaces](subsystem-interfaces.md) (Common contract rules; Pure domain and content;
Identity and session ownership; PostgreSQL persistence and durable media; AI/provider/
media ports; Projection, RPC and transport; Feature-specific candidates within
existing boundaries; Browser interfaces; Observability and development tooling;
Commerce and lifecycle ports), [Shared contract waves](shared-contract-waves.md),
[First shared contract wave](../development/shared-contracts.md), and [Recovery
revision policy](recovery-revision-policy.md). Workflow, style and supplied-source
fingerprints are frozen in the attempt brief. Its workflow requirements are
[AGENTS.md](../AGENTS.md), [coding style](coding-style.md), and ADRs [0001](../ADR/0001-sqlite-agent-workflow.md), [0003](../ADR/0003-agent-devlog.md), [0004](../ADR/0004-development-reliability.md), and [0005](../ADR/0005-frontier-output-evaluation.md); the source outcome is [backlog item B-C-df-types-D01](../development/backlog-catalog.json). The interface's [commerce and lifecycle ports](subsystem-interfaces.md#commerce-and-lifecycle-ports), [commerce authority](commerce-service.md), [service operations](service-operations.md), and [pricing and costs](pricing-and-costs.md) keep credentials, entitlements, authority and usage records at their own boundaries. The contract examples below import the unchanged canonical `identity.rs` from its input revision.

## Decision

The five existing `df-types` identities remain distinct opaque values:
`SessionId`, `MemberId`, `ClientBindingId`, `RunId`, and `OperationId`. Each accepts
exactly 16 caller-supplied bytes, preserved byte-for-byte without reordering, and
rejects the all-zero value. The current source defines no conversion among these kinds; callers must
carry the correct kind through APIs and persistence mappings. Their values are not
credentials, proof of identity, membership, permission, uniqueness, or issuance.
`df-types` owns this byte primitive. Consumers explicitly map their wire and storage
representations and do not replace it with duplicate string parsers or local ID
newtypes.

An identifier's input representation and its semantic role are separate decisions.
The byte array is the current primitive representation. When a consumer wave freezes
a distinct role, it gives that role its own owning type; it does not alias two
roles merely because both currently contain 16 bytes. In particular, the session,
member, and stable client-binding roles are distinct from a transient connection or
tab identifier. A `ClientBindingId` identifies the binding contract, not a
connection/tab instance. Do not predeclare every possible future identity in
`df-types`; the consuming boundary names only the roles it needs. The future type
owner must reuse the canonical byte validation and preserve the no-cross-kind rule.

For later I01 text ingress, select one bounded canonical spelling: exactly 32
lowercase ASCII hexadecimal characters (`0`–`9`, `a`–`f`), representing 16 bytes
in their supplied order. It has no prefix, separator, whitespace, or normalization.
Input length is measured in UTF-8 bytes and must be exactly 32 before digit
validation. A wrong byte length is `InvalidLength { actual }`; a 32-byte value with
any non-grammar byte is `Malformed`; the decoded all-zero value is `Zero`. This is
an ingress syntax policy only. The implementation will map the accepted bytes to
the eventual role's canonical typed ID and will not treat the text as a credential
or issuance mechanism.

The current `identity.rs` exposes only byte constructors. It has neither this text
boundary nor an implemented I01 constructor. The executable parser below is a
bounded decision model that exercises the grammar against the current
`ClientBindingId`; it is not a public API and does not claim I01 is implemented.

## Rationale and alternatives

Distinct newtypes prevent accidental substitution at compile time, while sharing
byte validation avoids multiple representations of the same primitive contract.
Using raw `[u8; 16]` at every consumer would erase the cross-kind check. Aliasing
client binding, connection/tab, member, or session IDs would collapse different
lifecycle and authority roles. A UUID dependency, generator, textual public
constructor, or stringly-typed ID framework is not required by the current source
or this task, so none is introduced.

For text ingress, accepting variable-length hex, a UUID prefix, separators,
whitespace, or Unicode lookalikes creates multiple spellings for one byte value.
Case-folding is also normalization. The selected lowercase fixed-width spelling is
unambiguous and has a constant 32-byte bound. The alternative of accepting both
hex cases is deliberately rejected for this canonical input boundary; any future
human-facing display format remains a separate consumer decision.

## Bounded executable decision example

This literal example imports the existing implementation by path and exercises
its real byte constructor. Its text parser is only a model of the selected future
grammar. The compiler-failure case is a separate literal snippet below.

```rust
#[path = "/Users/earlcameron/Desktop/dungeonflux/artifacts/worktrees/types_identity_decision/crates/df-types/src/identity.rs"]
mod identity;

use identity::{ClientBindingId, IdentityError, MemberId, OperationId, RunId, SessionId};

#[derive(Debug, PartialEq, Eq)]
enum TextIdentityError {
    InvalidLength { actual: usize },
    Malformed,
    Zero,
}

fn lowercase_hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn parse_binding_hex_model(text: &str) -> Result<ClientBindingId, TextIdentityError> {
    let encoded = text.as_bytes();
    if encoded.len() != 32 {
        return Err(TextIdentityError::InvalidLength {
            actual: encoded.len(),
        });
    }

    let mut bytes = [0_u8; 16];
    for (index, pair) in encoded.chunks_exact(2).enumerate() {
        let high = lowercase_hex_nibble(pair[0]).ok_or(TextIdentityError::Malformed)?;
        let low = lowercase_hex_nibble(pair[1]).ok_or(TextIdentityError::Malformed)?;
        bytes[index] = (high << 4) | low;
    }

    ClientBindingId::from_bytes(&bytes).map_err(|error| match error {
        IdentityError::InvalidLength { actual } => TextIdentityError::InvalidLength { actual },
        IdentityError::Zero => TextIdentityError::Zero,
    })
}

fn main() {
    let supplied = [1_u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
    let session = SessionId::from_bytes(&supplied).expect("16 nonzero bytes");
    assert_eq!(session.as_bytes(), &supplied);
    assert_eq!(
        SessionId::from_bytes(&[1_u8; 15]),
        Err(IdentityError::InvalidLength { actual: 15 })
    );
    assert_eq!(SessionId::from_bytes(&[0_u8; 16]), Err(IdentityError::Zero));

    let valid = "0102030405060708090a0b0c0d0e0f10";
    let binding = parse_binding_hex_model(valid).expect("canonical 32-byte hex");
    assert_eq!(binding.as_bytes(), &supplied);
    assert_eq!(
        parse_binding_hex_model("0123456789abcdef0123456789abcde"),
        Err(TextIdentityError::InvalidLength { actual: 31 })
    );
    assert_eq!(
        parse_binding_hex_model("0123456789abcdef0123456789abcdef0"),
        Err(TextIdentityError::InvalidLength { actual: 33 })
    );
    assert_eq!(
        parse_binding_hex_model("0123456789abcdef0123456789abcdeg"),
        Err(TextIdentityError::Malformed)
    );
    assert_eq!(
        parse_binding_hex_model("0123456789abcdef0123456789abcdef "),
        Err(TextIdentityError::InvalidLength { actual: 33 })
    );
    assert_eq!(
        parse_binding_hex_model("0123456789ABCDEF0123456789ABCDEF"),
        Err(TextIdentityError::Malformed)
    );
    assert_eq!(
        parse_binding_hex_model("00000000000000000000000000000000"),
        Err(TextIdentityError::Zero)
    );

    let non_ascii_same_byte_length = format!("{}é", "0".repeat(30));
    assert_eq!(non_ascii_same_byte_length.len(), 32);
    assert_eq!(
        parse_binding_hex_model(&non_ascii_same_byte_length),
        Err(TextIdentityError::Malformed)
    );

    // Distinct semantic kinds are constructible but cannot be passed as SessionId.
    let member = MemberId::from_bytes(&supplied).expect("16 nonzero bytes");
    let run = RunId::from_bytes(&supplied).expect("16 nonzero bytes");
    let operation = OperationId::from_bytes(&supplied).expect("16 nonzero bytes");
    assert_eq!(member.as_bytes(), &supplied);
    assert_eq!(run.as_bytes(), &supplied);
    assert_eq!(operation.as_bytes(), &supplied);
    println!(
        "PASS: actual byte identity, cross-kind compile-time boundary, and model-only bounded text grammar"
    );
}
```

This must fail to compile because the actual supplied types are distinct:

```compile_fail
#[path = "/Users/earlcameron/Desktop/dungeonflux/artifacts/worktrees/types_identity_decision/crates/df-types/src/identity.rs"]
mod identity;

use identity::{MemberId, SessionId};

fn session_only(_: SessionId) {}

fn main() {
    let member = MemberId::from_bytes(&[1_u8; 16]).unwrap();
    session_only(member);
}
```

## Alternatives and unresolved facts

The accepted type boundary does not decide where identifiers come from, how
uniqueness is enforced, how credentials are represented or rotated, how principals
are authenticated, how session membership is authorized, or how a client binding
is allocated/revoked. Those remain with the separately owned auth/session and
durable persistence contracts. It does not define connection/tab ID representation,
other planned IDs, wire encoding, database columns, UUID interoperability, or
display formatting. No generated ID, random source, timestamp, credential, or
authorization capability is implied by a valid 16-byte value.

The lowercase grammar and typed text errors are selected for later I01. Their
production constructor, codec location, compatibility/versioning behavior, caller
mapping and actual consumer tests are still pending that task's frozen boundary.
The current crate has only `InvalidLength { actual }` and `Zero` for byte input;
the bounded parser's `Malformed` error is a model outcome, not a promised public
variant. No schema, API, Cargo dependency, parser source file, issuance path or
production behavior was added here.

## Evidence status

The executable example is intended to be extracted literally, formatted using the
repository `rustfmt.toml`, compiled directly with cached Rust 1.98.1, and run from
the assigned source tree. Its evidence is limited to this decision boundary. It
does not run Cargo, Clippy, WASM, browser, provider, authentication, persistence,
allocation, uniqueness, or end-to-end checks. Those checks remain unperformed and
later gates retain their owners.

## Original acceptance criteria and verification

Acceptance criteria, preserved verbatim:

- `cross-kind conversions are forbidden`
- `The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.`

Original verification, preserved verbatim:

- `Freeze the cited source decision and a bounded contract example; compare cross-kind conversions are forbidden. Retain decision, alternatives and unresolved facts.`
- `Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.`
