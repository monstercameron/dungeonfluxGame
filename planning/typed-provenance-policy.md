# Typed build provenance policy

Task/attempt: `B-C-df-types-D04/a1`  
Input revision: `eef8bf5826757f98650cddfe4d8df92d79971319`  
Owner: `df-types` owns the canonical pure provenance primitives; build, content,
rules, native, and WASM owners supply and verify their own revisions.

Governing design: [Subsystem architecture](subsystem-architecture.md) and
[Subsystem interfaces](subsystem-interfaces.md), with [shared contract
waves](shared-contract-waves.md), [typed identity policy](typed-identity-policy.md),
[typed revision policy](typed-revision-policy.md), and [typed units
policy](typed-units-policy.md). Rules and catalog constraints remain governed by
[rules support](rules-support.md) and [rules coverage](rules-coverage.md). Workflow,
style, and exact supplied-source hashes are frozen in the attempt brief; applicable
sources include [AGENTS.md](../AGENTS.md), [coding style](coding-style.md), and
ADRs [0001](../ADR/0001-sqlite-agent-workflow.md),
[0003](../ADR/0003-agent-devlog.md), [0004](../ADR/0004-development-reliability.md),
and [0005](../ADR/0005-frontier-output-evaluation.md).

## Decision

Use the existing canonical `df_types::BuildIdentity`, `BuildRevision`,
`BuildIdentityError`, `RevisionLabel`, and `RevisionLabelError` in
`crates/df-types/src/provenance.rs`. Do not add another public implementation or
change the crate API in this decision. `BuildIdentity` has five required,
separately addressable components: `Source`, `Native`, `Wasm`, `Configuration`,
and `Content`. Each component stores its own `RevisionLabel`, and
`revision(BuildRevision)` selects the requested component. Equal strings in two
components remain two distinct component values with distinct selectors and error
attribution; equality of labels does not establish equality of builds or artifacts.

Each label is caller-supplied, non-secret provenance text. `RevisionLabel::new`
requires `Some` and accepts 1 through 128 UTF-8 bytes, limited to ASCII letters,
digits, `.`, `_`, `-`, `:`, and `/`. The exact failure cases are `Missing`,
`Empty`, `TooLong { actual }`, and `InvalidCharacter { byte_index }`. The byte
length is checked before character validation. `BuildIdentity::new` validates in
source, native, WASM, configuration, then content order and reports the failing
`BuildRevision` together with its `RevisionLabelError`. Errors retain no input
text. A non-ASCII character is rejected at its first UTF-8 byte index when within
the length limit; overlong input reports its byte length first.

These values preserve supplied labels only. They do not prove that a source exists,
that an artifact was built from it, that native and WASM outputs correspond, that
configuration or content is complete, or that a build was tested, approved, or
reproducible. They carry no credentials, authorization, issuance, or rights.
The type boundary keeps native, WASM, configuration, and content identities
independently supplied even when their text happens to match.

A rules/catalog revision remains a content-owner-reviewed input to the content
revision boundary. This decision does not define a new `RulesetId`, grant label
syntax authority over rule meaning, or infer edition/source validity from a label.
The exact required standard-2024 catalog and source manifest remain unresolved at
the existing rules gate. Preserve the full-catalog requirement, source pinning,
permitted-use and attribution evidence, and explicit content-rights review. A
syntax-valid label cannot substitute for a trusted build approval or a verified
native/WASM build relationship.

## Rationale and alternatives

Keeping one typed field and selector per supplied revision prevents a consumer from
silently using one string as all provenance dimensions. Reusing the current
primitive avoids a duplicate implementation whose validation could drift. Using a
single composite label, optional component fields, or inferring one component from
another would erase which owner supplied each fact. This decision rejects those
alternatives while leaving future consumer-specific bindings to their owning
contract waves.

A richer parser could validate repository commits, artifact digests, configuration
formats, content manifests, or signatures, but this crate has neither the source
nor authority to establish those facts. It would conflate syntax with provenance
truth and approval. A new `RulesetId` or rule-label grammar would similarly freeze
rule semantics before source and catalog owners have completed their gate, so it is
not introduced here.

## Bounded executable example

This standalone example imports the assigned worktree's canonical source file
literally. It tests all five selectors independently, including equal label text;
missing, empty, 128-byte, 129-byte, forbidden ASCII, and multibyte invalid input;
component-specific error attribution; and errors that do not retain the supplied
text. It does not copy or reimplement the production types.

```rust
#[path = "../../../crates/df-types/src/provenance.rs"]
mod provenance;

use provenance::{
    BuildIdentity, BuildIdentityError, BuildRevision, RevisionLabel, RevisionLabelError,
};

fn main() {
    let identity = BuildIdentity::new(
        Some("same"),
        Some("same"),
        Some("same"),
        Some("same"),
        Some("same"),
    )
    .expect("all five supplied labels are valid");
    for revision in [
        BuildRevision::Source,
        BuildRevision::Native,
        BuildRevision::Wasm,
        BuildRevision::Configuration,
        BuildRevision::Content,
    ] {
        assert_eq!(identity.revision(revision).as_str(), "same");
    }

    let distinct = BuildIdentity::new(
        Some("source"),
        Some("native"),
        Some("wasm"),
        Some("config"),
        Some("content"),
    )
    .expect("independent component labels are valid");
    assert_eq!(distinct.revision(BuildRevision::Source).as_str(), "source");
    assert_eq!(distinct.revision(BuildRevision::Native).as_str(), "native");
    assert_eq!(distinct.revision(BuildRevision::Wasm).as_str(), "wasm");
    assert_eq!(
        distinct.revision(BuildRevision::Configuration).as_str(),
        "config"
    );
    assert_eq!(
        distinct.revision(BuildRevision::Content).as_str(),
        "content"
    );

    assert_eq!(RevisionLabel::new(None), Err(RevisionLabelError::Missing));
    assert_eq!(RevisionLabel::new(Some("")), Err(RevisionLabelError::Empty));
    let maximum = "a".repeat(128);
    assert_eq!(
        RevisionLabel::new(Some(&maximum)).unwrap().as_str(),
        maximum
    );
    assert_eq!(
        RevisionLabel::new(Some(&"a".repeat(129))),
        Err(RevisionLabelError::TooLong { actual: 129 })
    );
    assert_eq!(
        RevisionLabel::new(Some("bad label")),
        Err(RevisionLabelError::InvalidCharacter { byte_index: 3 })
    );
    assert_eq!(
        RevisionLabel::new(Some("aé")),
        Err(RevisionLabelError::InvalidCharacter { byte_index: 1 })
    );

    let revisions = [
        BuildRevision::Source,
        BuildRevision::Native,
        BuildRevision::Wasm,
        BuildRevision::Configuration,
        BuildRevision::Content,
    ];
    let long = "x".repeat(129);
    let failures = [None, Some(""), Some("bad label"), Some("aé"), Some(&long)];
    for revision in revisions {
        for value in failures {
            let valid = Some("valid");
            let inputs = match revision {
                BuildRevision::Source => (value, valid, valid, valid, valid),
                BuildRevision::Native => (valid, value, valid, valid, valid),
                BuildRevision::Wasm => (valid, valid, value, valid, valid),
                BuildRevision::Configuration => (valid, valid, valid, value, valid),
                BuildRevision::Content => (valid, valid, valid, valid, value),
            };
            let error = BuildIdentity::new(inputs.0, inputs.1, inputs.2, inputs.3, inputs.4)
                .expect_err("each invalid supplied component is rejected");
            assert_eq!(error.revision, revision);
            match (value, error.cause) {
                (None, RevisionLabelError::Missing) => {}
                (Some(""), RevisionLabelError::Empty) => {}
                (Some("bad label"), RevisionLabelError::InvalidCharacter { byte_index: 3 }) => {}
                (Some("aé"), RevisionLabelError::InvalidCharacter { byte_index: 1 }) => {}
                (Some(value), RevisionLabelError::TooLong { actual: 129 }) if value == long => {}
                _ => panic!("unexpected input-free typed error"),
            }
            assert!(!format!("{error:?}").contains("bad label"));
            assert!(!format!("{error:?}").contains("aé"));
        }
    }

    let error = BuildIdentity::new(Some(&long), Some("n"), Some("w"), Some("c"), Some("d"))
        .expect_err("source component over length is rejected first");
    assert_eq!(
        error,
        BuildIdentityError {
            revision: BuildRevision::Source,
            cause: RevisionLabelError::TooLong { actual: 129 },
        }
    );
    assert!(!format!("{error:?}").contains(&long));

    println!("PASS: canonical five-component provenance labels and typed validation boundary");
}
```

## Unresolved boundary and verification

The actual content owner must supply a pinned rules/catalog revision and evidence for
the full required standard-2024 source and catalog scope, edition consistency,
permitted use, attribution, and rights. The actual build owner must bind labels to
observed source, native, WASM, configuration, and content artifacts and separately
establish any trusted approval. Consumer wire/storage codecs and cross-system
integration remain with their named owners. This decision makes no production
build, browser/WASM execution, rules-catalog, rights, approval, or integrated
behavior claim.

The exact isolated source import, formatter, compiler, run commands, tool and input
hashes, and observed outputs are retained in the attempt's worker handoff. The
bounded example checks the existing pure Rust boundary only. Cargo builds,
Clippy, WASM target execution, full catalog/source/rights gates, and production
native/WASM integration are unperformed. The whole decision requires independent
frontier review, followed by acceptance against the exact resulting source before
the task can be marked done.
