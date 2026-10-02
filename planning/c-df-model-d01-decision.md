# C-df-model-D01: immutable checkpoint envelope

Date: 2026-10-02
Status: bounded design decision; production types, codecs and recovery qualification pending

## Decision

`df-model` owns one versioned immutable envelope around a detached complete authoritative
checkpoint. Its minimum binding is schema version, `SessionId`, `RunId`, full
`SessionRevision`, exact ruleset/catalog/source/handler identity, immutable content/package
identity and source/configuration/build provenance. Reuse the existing `df-types` identities,
lexicographic recovery revision and bounded provenance labels. A revision label identifies a
supplied revision; it does not establish catalog correctness, source rights or compatibility.
Content/source digests bind the exact reviewed bytes, rather than an English edition name or
a mutable "latest" alias. The source manifest and handler revision distinguish rules data
from executable mechanics. This decision selects no 2024 book catalog or new public Rust type.

The session owner freezes the envelope and its state from one committed, consistent basis.
Later decisions create new checkpoints; they do not edit an earlier checkpoint's pins or
payload in place. Membership linkage, canonical facts and attributed knowledge, resources,
accepted decisions/draws, pending continuations and windows, logical time, and owned durable
timer/job/effect state must remain part of the complete checkpoint's governed state. The
envelope does not duplicate those separately owned models or embed live process handles.
An old checkpoint is historical evidence, not permission to overwrite newer state.

Normal resume validates the supported schema, exact session/run, full recovery epoch and
sequence, pinned rules/source/catalog/handler and content/package identities against the
trusted selected recovery basis and admitted supported configuration. Mismatches, unsupported
versions and stale bases return typed refusals before state becomes usable. Matching names,
matching sequence in a different epoch, or a lexicographically newer checkpoint supplied by
a caller do not substitute for exact expected-basis equality. The selected recovery basis may
be historical only in a separately authorized restore procedure. Schema migration or handler
compatibility needs an explicit reviewed mapping with consumers; it never rewrites a pin or
silently loads current catalogs. Build provenance remains recorded even when a future reviewed
compatibility mapping permits another runtime build. The finite example uses exact build
matching as its admitted policy, without freezing a production compatibility matrix.

Debug restore forks a new run and process generation, records the origin checkpoint/basis and
retains old history; it does not rewind live connections or restart old callbacks. Disaster
restore additionally requires the separately protected, strictly newer recovery epoch, retired
old operation namespaces and explicit lost-game-range accounting. Envelope construction cannot
issue an epoch, grant restore authority or prove zero game-data loss. Current owner fencing,
authorization, operation deduplication and durable commit remain session/repository checks.
Private payload suppression and rights restrictions remain effective after restore: a redacted
or unavailable checkpoint cannot be claimed intact or reconstructed from summaries/providers.
Immutable nonpersonal provenance does not override lawful personal-payload lifecycle.

## Consumers and comparison inputs

The next implementation is the existing `B-C-df-model-I01` validated checkpoint-constructor
task, initially in `crates/df-model/src/lib.rs` and its private checkpoint module after exact
state/pin contracts are reviewed with `df-content`, `df-rules`, `df-engine` and `df-session`.
No such source path is created here. `df-engine` supplies pure detached state; `df-session`
owns checkpoint admission/commit/restore; `df-persistence` owns versioned codecs and PostgreSQL
mapping; `df-api` owns authorized projection, never serialization of full state to clients.
Pure `df-model` imports no database, clock, network, provider or telemetry SDK. It returns
safe typed rejection facts for native consumers to instrument through `df-observe`, with
identities/revisions/classifications and no private payload by default. Hash possession grants
no disclosure or admission rights.

Separately assigned drafts are comparison inputs until independently approved/integrated:

- Model D02 owns closed input/effect variants and executor ownership. The checkpoint preserves
  admitted intents and their run/basis/job/operation/timer-instance bindings; it does not register
  executors or reactivate stale callbacks.
- Model D03 owns distinct pending choice/roll/reaction/ruling state. Preserve its stable
  resolution, exact continuation, source window, accepted responses/draws and current basis;
  restore must not regenerate offers, reroll or flatten timing windows.
- Persistence D01 treats the envelope as a consumer-owned versioned payload. Trusted
  tenant/session/run/full-revision repository columns must equal decoded envelope scope/basis;
  unknown schema/codec or disagreement refuses. Tenant authorization is native context, not
  authority conferred by checkpoint bytes. Physical layout and codecs remain its separate gate.
- Persistence D02 preserves operation results/intents and recovery namespaces in one fenced
  atomic decision. Unknown commit resolves by same-operation lookup, never reapplication of
  a checkpoint or a new draw.

## Alternatives and unresolved facts

Mutable current-catalog pointers would change old mechanics/content after deployment; reject
them in favor of exact immutable pins. A sequence-only revision would admit old recovery
namespaces; reuse full `SessionRevision`. Overwriting a live run from a saved payload would
revive stale jobs and forgotten operations; require the explicit fork/restore procedure.
Using telemetry, memory summaries or generated media as checkpoint truth would lose committed
state and provenance; preserve canonical snapshot/decision records instead.

The exact schema number, complete `GameState`, `RulesetId` and catalog/source/handler manifest,
content/package contract, payload integrity codec, migration compatibility, item/byte bounds,
redaction outcomes and durable restore ports remain reviewed production prerequisites. The
fixture's schema number and digest bytes are synthetic comparison values, not approved source
or production contracts. Full 2024 support and rights remain the rules/catalog gates.

## Bounded executable contract

The private std-only illustration imports the actual worktree `df-types` library. It validates
exact pins and basis without changing the owned synthetic state marker. It is not a repository,
codec, production checkpoint, rules fixture or restore implementation. Assertions/printing are
finite harness output. No clock, provider, database, random draw or external authorization runs.

```rust
use df_types::{BuildIdentity, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision};

#[derive(Clone, Debug, Eq, PartialEq)]
struct RulesPins {
    ruleset: RevisionLabel,
    catalog: RevisionLabel,
    source_digest: [u8; 32],
    handler: RevisionLabel,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ContentPins {
    package: RevisionLabel,
    digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Basis {
    session: SessionId,
    run: RunId,
    revision: SessionRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Checkpoint {
    schema: u16,
    basis: Basis,
    rules: RulesPins,
    content: ContentPins,
    build: BuildIdentity,
    state_marker: [u8; 4],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    UnsupportedSchema,
    WrongSession,
    WrongRun,
    StaleBasis,
    RulesMismatch,
    ContentMismatch,
    BuildMismatch,
}

fn validate_resume<'a>(
    checkpoint: &'a Checkpoint,
    expected: Basis,
    rules: &RulesPins,
    content: &ContentPins,
    build: &BuildIdentity,
) -> Result<&'a Checkpoint, Refusal> {
    if checkpoint.schema != 1 {
        return Err(Refusal::UnsupportedSchema);
    }
    if checkpoint.basis.session != expected.session {
        return Err(Refusal::WrongSession);
    }
    if checkpoint.basis.run != expected.run {
        return Err(Refusal::WrongRun);
    }
    if checkpoint.basis.revision != expected.revision {
        return Err(Refusal::StaleBasis);
    }
    if &checkpoint.rules != rules {
        return Err(Refusal::RulesMismatch);
    }
    if &checkpoint.content != content {
        return Err(Refusal::ContentMismatch);
    }
    if &checkpoint.build != build {
        return Err(Refusal::BuildMismatch);
    }
    Ok(checkpoint)
}

fn label(value: &str) -> RevisionLabel {
    RevisionLabel::new(Some(value)).unwrap()
}

fn main() {
    let basis = Basis {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        revision: SessionRevision::new(RecoveryEpoch::new(3).unwrap(), 7),
    };
    let rules = RulesPins {
        ruleset: label("fixture-rules-v1"),
        catalog: label("fixture-catalog-v1"),
        source_digest: [3; 32],
        handler: label("fixture-handler-v1"),
    };
    let content = ContentPins {
        package: label("fixture-package-v1"),
        digest: [4; 32],
    };
    let build = BuildIdentity::new(
        Some("fixture-source-v1"),
        Some("fixture-native-v1"),
        Some("fixture-wasm-v1"),
        Some("fixture-config-v1"),
        Some("fixture-content-v1"),
    )
    .unwrap();
    let checkpoint = Checkpoint {
        schema: 1,
        basis,
        rules: rules.clone(),
        content: content.clone(),
        build: build.clone(),
        state_marker: [5; 4],
    };
    let before = checkpoint.clone();
    let resume = |candidate_basis, candidate_rules: &RulesPins, candidate_content: &ContentPins| {
        validate_resume(
            &checkpoint,
            candidate_basis,
            candidate_rules,
            candidate_content,
            &build,
        )
    };
    assert_eq!(resume(basis, &rules, &content), Ok(&checkpoint));
    assert_eq!(checkpoint.state_marker, [5; 4]);

    let mut wrong_session = basis;
    wrong_session.session = SessionId::from_bytes(&[6; 16]).unwrap();
    assert_eq!(
        resume(wrong_session, &rules, &content),
        Err(Refusal::WrongSession)
    );
    let mut wrong_run = basis;
    wrong_run.run = RunId::from_bytes(&[7; 16]).unwrap();
    assert_eq!(resume(wrong_run, &rules, &content), Err(Refusal::WrongRun));
    for (epoch, sequence) in [(3, 6), (3, 8), (4, 7), (2, 7)] {
        let mut stale = basis;
        stale.revision = SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), sequence);
        assert_eq!(resume(stale, &rules, &content), Err(Refusal::StaleBasis));
    }
    let mut wrong_rules = rules.clone();
    wrong_rules.ruleset = label("fixture-rules-v2");
    assert_eq!(
        resume(basis, &wrong_rules, &content),
        Err(Refusal::RulesMismatch)
    );
    wrong_rules = rules.clone();
    wrong_rules.catalog = label("fixture-catalog-v2");
    assert_eq!(
        resume(basis, &wrong_rules, &content),
        Err(Refusal::RulesMismatch)
    );
    wrong_rules = rules.clone();
    wrong_rules.source_digest = [8; 32];
    assert_eq!(
        resume(basis, &wrong_rules, &content),
        Err(Refusal::RulesMismatch)
    );
    wrong_rules = rules.clone();
    wrong_rules.handler = label("fixture-handler-v2");
    assert_eq!(
        resume(basis, &wrong_rules, &content),
        Err(Refusal::RulesMismatch)
    );
    let mut wrong_content = content.clone();
    wrong_content.package = label("fixture-package-v2");
    assert_eq!(
        resume(basis, &rules, &wrong_content),
        Err(Refusal::ContentMismatch)
    );
    wrong_content = content.clone();
    wrong_content.digest = [9; 32];
    assert_eq!(
        resume(basis, &rules, &wrong_content),
        Err(Refusal::ContentMismatch)
    );
    let mut unsupported = checkpoint.clone();
    unsupported.schema = 2;
    assert_eq!(
        validate_resume(&unsupported, basis, &rules, &content, &build),
        Err(Refusal::UnsupportedSchema)
    );
    let wrong_build = BuildIdentity::new(
        Some("fixture-source-v2"),
        Some("fixture-native-v1"),
        Some("fixture-wasm-v1"),
        Some("fixture-config-v1"),
        Some("fixture-content-v1"),
    )
    .unwrap();
    assert_eq!(
        validate_resume(&checkpoint, basis, &rules, &content, &wrong_build),
        Err(Refusal::BuildMismatch)
    );
    assert_eq!(checkpoint, before);
    println!("PASS: exact checkpoint pins; 14 mismatch/stale refusals; original unchanged");
}
```

## Governing source and evidence scope

Input revision: `4c0e050c2d30e632bd3b32c51a92cfda52479e6e`. The frozen brief is
`development/evidence/fanout-20261001/wave-03/B-C-df-model-D01/a1/brief.json`.
Its original criterion is "rules content and run identities are pinned" plus honest
source/build-bound evidence. Relevant source is `subsystem-architecture.md` (shared/server
owners), `subsystem-interfaces.md` (common contracts, pure domain, identity/session,
persistence, projection/tooling), `long-horizon-state.md` (canonical authority, source/access
and rights), `campaign-cinematics.md` (run/source/identity continuity), `rules-effect-model.md`
(pending/resources/effect migration), `rules-support.md` (exact source and catalog gate),
`runtime-reliability.md` (stale work and recovery), and `c-df-engine-d01-decision.md` (single
candidate/session commit). AGENTS, coding style, ADR 0001–0005, observability and pinned
tool/configuration govern execution and handoff.

Exact Markdown literal extraction/byte comparison, pinned rustfmt write/check with root
configuration, Rust 2024 `-Dwarnings` compilation and finite execution are the bounded checks.
The durable attempt handoff records their actual commands, outputs, source/tool/configuration
hashes and results under unchanged v6 admission limits. No Cargo/Clippy, WASM execution,
production schema/codec round trip, PostgreSQL durability, protected-epoch issuance,
authorization/redaction, live rules/game/provider, browser or independent approval is claimed.
Those remain distinct reviewed implementation/integration gates; this document cannot close
the required complete standard-rules coverage or prove runtime restore safety.
