# df-tools atomic artifact publication and provenance decision

Task/attempt: `B-C-df-tools-D03/a2`

Input revision: `4c0e050c2d30e632bd3b32c51a92cfda52479e6e`
Repair reference: rejected a1 candidate `f255f1fb31561bb0bfa625ea478e72499bc60f3d`, findings D03-R1/R2.
Status: bounded design decision; filesystem publication and production qualification remain pending.

## Decision

Treat one native/WASM/configuration/assets set as the publication unit. A candidate
set is eligible only when its manifest contains a complete shared
`df_types::BuildIdentity` (`source`, `native`, `wasm`, `configuration`, and
`content`), a content digest for each native and WASM output, a digest for the
resolved build configuration, and one digest for every asset ID in the resolved
build input. Asset IDs are explicit, unique, bounded nonsecret ASCII tokens using
the current `RevisionLabel` grammar; the manifest records the sorted ID/digest
pairs. The build input's asset-ID set and manifest's asset-ID set must be equal.
No component is inferred from a filename, directory name, source label, or another
component's label. The digest is over the exact bytes published for that component.
The build owner freezes an expected set before validation: all five labels, the
expected native/WASM/configuration digests, and the complete asset ID/digest map.
Observed candidate digests must equal that set for every component and asset;
unchanged labels and IDs do not excuse a byte mismatch. Expected digests are not
copied from the candidate under validation. In production, freezing trustworthy
expected bytes/digests and computing observed hashes are I01 prerequisites.

`BuildIdentity` is the actual shared source contract at this revision. It requires
five independently supplied labels and validates each as 1–128 ASCII bytes from
`[A-Za-z0-9._:/-]`; it explicitly does not assert testing, approval, content
rights, or catalog validity. There is no `AssetId` type in `df-types` yet, despite
its planned crate boundary. Until that shared type is implemented, the manifest's
serialized asset ID uses the same already-supported bounded label grammar. The
next shared-types change must give it a dedicated typed constructor before
consumers rely on a Rust `AssetId` API. This decision does not change
`df-types` or add a second identity authority.

The manifest is immutable once accepted and binds the five revision labels,
native and WASM byte digests, configuration byte digest, and complete sorted
asset ID/digest list. It also records the builder/toolchain identity and
nonsecret normalized build arguments needed to explain how those exact bytes
were produced. Credentials and secret argument values are rejected or omitted;
the resulting manifest must make omission explicit rather than imply a
reproducible invocation. A source label by itself, including D02's
`native_build_id` or `web_source_id`, is not a manifest and cannot qualify or
promote a build.

`df-tools` owns staging, validation of the complete candidate, and the local
accepted-build pointer. It writes each build into a new immutable attempt/build
directory beneath `artifacts/build/`, verifies the component digests and exact
asset set against the build inputs, writes the complete manifest, and only then
atomically replaces one pointer file that names that manifest. Write and sync the
staged files and manifest before pointer replacement; use a temporary pointer in
the same filesystem, atomic rename, and parent-directory sync. The pointer is the
commit point: before successful rename, readers see the prior accepted manifest;
after successful rename, it names a complete immutable set. Never publish
per-component pointers or mutate the current accepted directory in place.
A missing label, missing native/WASM/configuration component, missing or extra
asset, duplicate/invalid asset ID, digest mismatch, incomplete evidence, or a
known staging/write/file-sync/manifest-sync/temporary-pointer-sync failure before
rename is a typed pre-commit refusal that preserves the prior selected pointer.
A known rename failure with confirmed no replacement has the same guarantee.

A successful rename followed by failed parent-directory sync is instead
`CommittedDurabilityUnconfirmed`: the new complete set is currently selected,
but crash/power-loss durability is unconfirmed. It cannot promise that the old
pointer remains selected. A lost/ambiguous replacement outcome is
`ReplacementOutcomeUnknown`, whether replacement actually happened or not;
neither success nor prior-pointer preservation is inferred from the attempted
write. Reconciliation under the publication owner's exclusion rereads and
validates the selected pointer/immutable manifest identity, then attempts the
required sync. It reports the observed selected set separately from confirmed
or unconfirmed durability; rereading alone does not prove durability. Missing,
corrupt, unexpected, or inaccessible pointer/manifest remains unresolved rather
than triggering a blind publish retry. Preserve both old and new immutable bytes
and their claims until reconciliation and retention permit retirement. A
compensating rollback would be a separate publication and is not an atomic undo
of the successful rename. Cancellation cannot undo that committed selection.

This pointer means “accepted local build set under the frozen build checks.” It
does not mean browser/server readiness, deployment approval, campaign publication
or activation, a successful preview, or physical-device qualification. D02's
private preview owner remains the sole owner of preview process and registration
lifecycle. Preview consumes one immutable accepted set and holds its use claim
for that set for its lifetime; it cannot assemble components from different
manifests. It may retain and continue serving its prior accepted set when a new
candidate is incomplete or refused. Promotion/readiness authority for a stable
last-verified preview remains unfrozen by D02 and is not granted here. Publication
does not authorize cleanup: ADR 0002 ownership, active-use, retention, age, and
cleanup-claim conditions still govern artifact removal.

## Source boundary and next implementation

At the input revision, `crates/df-types/src/provenance.rs` provides the complete
five-field `BuildIdentity` and `RevisionLabel` validation. `crates/df-tools/src/lib.rs`
injects only `BUILD_ID`, falling back to `unregistered-cargo-build`; fixture
telemetry uses that value as a correlation/build label. `fixture.rs` serves a
caller-supplied web root. The accepted D02 decision stores a native build label
and a web source label in an owner-held preview record and explicitly disclaims
complete paired native/WASM/asset identity. The fixture's current labels therefore
cannot be promoted or translated into this manifest without the actual component
bytes, configuration, and asset ID list.

The next implementation belongs to `df-tools` and should add a private build
publication module in `crates/df-tools/src/publication.rs`, privately wired by
`crates/df-tools/src/lib.rs`; exact existing CLI/fixture entry point wiring is
frozen with I01. Its first input is
an explicit build request containing the five shared identity labels, build
configuration bytes, resolved asset IDs, toolchain identity, and nonsecret build
arguments. It stages native, WASM, config, and asset bytes under one attempt-owned
directory; computes digests from those bytes; validates exact set closure; writes
the immutable manifest; then commits the single pointer. A reader returns one
manifest and its immutable directory as a unit. Keep preview registration private
to the D02 owner and require it to claim that returned set, not reconstruct it
from labels. No public/admin RPC or content-publication operation is added; those
transport and authority decisions remain gated by X10/G03.

## Alternatives

- Independent per-component pointers and in-place updates were rejected because
  readers could combine incomplete or mismatched sets.
- Accepting source labels, asset IDs, or candidate-supplied digests as the expected
  oracle was rejected because it cannot detect same-label byte substitutions.
- Promising old-pointer selection after successful rename, or silently rolling
  back on directory-sync failure, was rejected because replacement has committed;
  visibility and durability need distinct outcomes and explicit reconciliation.

## Bounded contract literal

This standalone literal imports the actual `df-types` provenance source by path.
It models complete identity construction, a separately frozen expected digest
set, exact asset-set closure, the rename commit point, and typed uncertainty and
reconciliation on either side of replacement. It does not perform
filesystem writes, calculate cryptographic digests, run a Cargo build, or prove
crash durability. The finite checks establish the contract decision only.

```rust
#[path = "/Users/earlcameron/Desktop/dungeonflux/artifacts/worktrees/wave04_build_publication_repair-a2/crates/df-types/src/provenance.rs"]
mod provenance;

use provenance::{BuildIdentity, BuildIdentityError, BuildRevision, RevisionLabel};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Digest([u8; 32]);

#[derive(Clone, Debug, PartialEq, Eq)]
struct AssetEntry {
    id: String,
    digest: Digest,
}

struct ExpectedSet {
    identity: [&'static str; 5],
    native: Digest,
    wasm: Digest,
    configuration: Digest,
    assets: Vec<AssetEntry>,
}

#[derive(Clone)]
struct Candidate {
    identity: [&'static str; 5],
    native: Option<Digest>,
    wasm: Option<Digest>,
    configuration: Option<Digest>,
    assets: Vec<AssetEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Manifest {
    identity: BuildIdentity,
    native: Digest,
    wasm: Digest,
    configuration: Digest,
    assets: Vec<AssetEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Component {
    Native,
    Wasm,
    Configuration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PreCommitFailure {
    Staging,
    Write,
    FileSync,
    ManifestSync,
    PointerWrite,
    PointerSync,
    RenameNotReplaced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Refusal {
    IncompleteIdentity,
    IdentityMismatch,
    MissingComponent,
    InvalidAssetId,
    DuplicateAssetId,
    AssetSetMismatch,
    DigestMismatch(Component),
    AssetDigestMismatch(String),
    BeforeCommit(PreCommitFailure),
}

#[derive(Clone, Copy)]
enum Boundary {
    Ready,
    BeforeRename(PreCommitFailure),
    ParentSyncFailed,
    LostBeforeRename,
    LostAfterRename,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Publication {
    Durable,
    CommittedDurabilityUnconfirmed,
    ReplacementOutcomeUnknown,
}

fn identity(labels: [&str; 5]) -> Result<BuildIdentity, Refusal> {
    BuildIdentity::new(
        Some(labels[0]),
        Some(labels[1]),
        Some(labels[2]),
        Some(labels[3]),
        Some(labels[4]),
    )
    .map_err(|_: BuildIdentityError| Refusal::IncompleteIdentity)
}

fn asset_map(assets: &[AssetEntry]) -> Result<BTreeMap<String, Digest>, Refusal> {
    let mut entries = BTreeMap::new();
    for asset in assets {
        RevisionLabel::new(Some(&asset.id)).map_err(|_| Refusal::InvalidAssetId)?;
        if entries.insert(asset.id.clone(), asset.digest).is_some() {
            return Err(Refusal::DuplicateAssetId);
        }
    }
    Ok(entries)
}

fn publish(
    current: &mut Option<Manifest>,
    expected: &ExpectedSet,
    candidate: Candidate,
    boundary: Boundary,
) -> Result<Publication, Refusal> {
    let identity = identity(candidate.identity)?;
    if identity != self::identity(expected.identity)? {
        return Err(Refusal::IdentityMismatch);
    }
    let native = candidate.native.ok_or(Refusal::MissingComponent)?;
    let wasm = candidate.wasm.ok_or(Refusal::MissingComponent)?;
    let configuration = candidate.configuration.ok_or(Refusal::MissingComponent)?;
    for (component, observed, expected_digest) in [
        (Component::Native, native, expected.native),
        (Component::Wasm, wasm, expected.wasm),
        (
            Component::Configuration,
            configuration,
            expected.configuration,
        ),
    ] {
        if observed != expected_digest {
            return Err(Refusal::DigestMismatch(component));
        }
    }
    let expected_assets = asset_map(&expected.assets)?;
    let observed_assets = asset_map(&candidate.assets)?;
    if !expected_assets.keys().eq(observed_assets.keys()) {
        return Err(Refusal::AssetSetMismatch);
    }
    for (id, digest) in &observed_assets {
        if expected_assets.get(id) != Some(digest) {
            return Err(Refusal::AssetDigestMismatch(id.clone()));
        }
    }
    match boundary {
        Boundary::BeforeRename(failure) => return Err(Refusal::BeforeCommit(failure)),
        Boundary::LostBeforeRename => return Ok(Publication::ReplacementOutcomeUnknown),
        _ => {}
    }
    let assets = observed_assets
        .into_iter()
        .map(|(id, digest)| AssetEntry { id, digest })
        .collect();
    // Finite stand-in for successful atomic rename, not filesystem evidence.
    *current = Some(Manifest {
        identity,
        native,
        wasm,
        configuration,
        assets,
    });
    Ok(match boundary {
        Boundary::ParentSyncFailed => Publication::CommittedDurabilityUnconfirmed,
        Boundary::LostAfterRename => Publication::ReplacementOutcomeUnknown,
        _ => Publication::Durable,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Durability {
    Confirmed,
    Unconfirmed,
}

#[derive(Debug, PartialEq, Eq)]
struct Reconciled {
    selected: Option<Manifest>,
    durability: Durability,
}

// Models a successful validated reread under owner exclusion and a separate
// sync result. The reread alone never confirms durability or republishes.
fn reconcile(current: &Option<Manifest>, sync_succeeded: bool) -> Reconciled {
    Reconciled {
        selected: current.clone(),
        durability: if sync_succeeded {
            Durability::Confirmed
        } else {
            Durability::Unconfirmed
        },
    }
}

fn expected_set() -> ExpectedSet {
    ExpectedSet {
        identity: ["src-1", "native-1", "wasm-1", "config-1", "content-1"],
        native: Digest([1; 32]),
        wasm: Digest([2; 32]),
        configuration: Digest([3; 32]),
        assets: vec![
            AssetEntry {
                id: "map:crypt-1".to_owned(),
                digest: Digest([4; 32]),
            },
            AssetEntry {
                id: "audio:theme-1".to_owned(),
                digest: Digest([5; 32]),
            },
        ],
    }
}

fn candidate() -> Candidate {
    Candidate {
        identity: ["src-1", "native-1", "wasm-1", "config-1", "content-1"],
        native: Some(Digest([1; 32])),
        wasm: Some(Digest([2; 32])),
        configuration: Some(Digest([3; 32])),
        assets: vec![
            AssetEntry {
                id: "audio:theme-1".to_owned(),
                digest: Digest([5; 32]),
            },
            AssetEntry {
                id: "map:crypt-1".to_owned(),
                digest: Digest([4; 32]),
            },
        ],
    }
}

fn main() {
    let expected = expected_set();
    let mut accepted = None;
    assert_eq!(
        publish(&mut accepted, &expected, candidate(), Boundary::Ready),
        Ok(Publication::Durable)
    );
    let previous = accepted.clone();
    let mut cases = Vec::new();
    let mut incomplete = candidate();
    incomplete.identity[4] = "";
    cases.push((incomplete, Refusal::IncompleteIdentity));
    let mut missing_wasm = candidate();
    missing_wasm.wasm = None;
    cases.push((missing_wasm, Refusal::MissingComponent));
    let mut omitted_asset = candidate();
    omitted_asset.assets.pop();
    cases.push((omitted_asset, Refusal::AssetSetMismatch));
    let mut duplicate_asset = candidate();
    duplicate_asset
        .assets
        .push(duplicate_asset.assets[0].clone());
    cases.push((duplicate_asset, Refusal::DuplicateAssetId));
    let mut invalid_asset = candidate();
    invalid_asset.assets[0].id = "private token".to_owned();
    cases.push((invalid_asset, Refusal::InvalidAssetId));
    let mut changed_identity = candidate();
    changed_identity.identity[0] = "src-other";
    cases.push((changed_identity, Refusal::IdentityMismatch));
    for (component, refusal) in [
        (
            Component::Native,
            Refusal::DigestMismatch(Component::Native),
        ),
        (Component::Wasm, Refusal::DigestMismatch(Component::Wasm)),
        (
            Component::Configuration,
            Refusal::DigestMismatch(Component::Configuration),
        ),
    ] {
        let mut changed = candidate();
        match component {
            Component::Native => changed.native = Some(Digest([9; 32])),
            Component::Wasm => changed.wasm = Some(Digest([9; 32])),
            Component::Configuration => changed.configuration = Some(Digest([9; 32])),
        }
        assert_eq!(changed.identity, expected.identity);
        cases.push((changed, refusal));
    }
    for index in 0..2 {
        let mut changed = candidate();
        changed.assets[index].digest = Digest([9; 32]);
        assert_eq!(changed.identity, expected.identity);
        cases.push((
            changed.clone(),
            Refusal::AssetDigestMismatch(changed.assets[index].id.clone()),
        ));
    }
    for (invalid, refusal) in cases {
        assert_eq!(
            publish(&mut accepted, &expected, invalid, Boundary::Ready),
            Err(refusal)
        );
        assert_eq!(accepted, previous);
    }
    for failure in [
        PreCommitFailure::Staging,
        PreCommitFailure::Write,
        PreCommitFailure::FileSync,
        PreCommitFailure::ManifestSync,
        PreCommitFailure::PointerWrite,
        PreCommitFailure::PointerSync,
        PreCommitFailure::RenameNotReplaced,
    ] {
        assert_eq!(
            publish(
                &mut accepted,
                &expected,
                candidate(),
                Boundary::BeforeRename(failure)
            ),
            Err(Refusal::BeforeCommit(failure))
        );
        assert_eq!(accepted, previous);
    }
    let manifest = accepted.as_ref().expect("finite valid fixture");
    for (revision, label) in [
        (BuildRevision::Source, "src-1"),
        (BuildRevision::Native, "native-1"),
        (BuildRevision::Wasm, "wasm-1"),
        (BuildRevision::Configuration, "config-1"),
        (BuildRevision::Content, "content-1"),
    ] {
        assert_eq!(manifest.identity.revision(revision).as_str(), label);
    }
    assert_ne!(manifest.native, manifest.wasm);
    assert_eq!(manifest.assets[0].id, "audio:theme-1");

    // A distinct valid replacement makes old-pointer guarantees falsifiable.
    let mut replacement_expected = expected_set();
    replacement_expected.identity[0] = "src-2";
    let mut replacement = candidate();
    replacement.identity[0] = "src-2";
    for (boundary, outcome, replaced) in [
        (
            Boundary::ParentSyncFailed,
            Publication::CommittedDurabilityUnconfirmed,
            true,
        ),
        (
            Boundary::LostBeforeRename,
            Publication::ReplacementOutcomeUnknown,
            false,
        ),
        (
            Boundary::LostAfterRename,
            Publication::ReplacementOutcomeUnknown,
            true,
        ),
        (Boundary::Ready, Publication::Durable, true),
    ] {
        accepted = previous.clone();
        assert_eq!(
            publish(
                &mut accepted,
                &replacement_expected,
                replacement.clone(),
                boundary
            ),
            Ok(outcome)
        );
        assert_eq!(accepted != previous, replaced);
        let reread = reconcile(&accepted, false);
        assert_eq!(reread.selected, accepted);
        assert_eq!(reread.durability, Durability::Unconfirmed);
        let synced = reconcile(&accepted, true);
        assert_eq!(synced.selected, accepted);
        assert_eq!(synced.durability, Durability::Confirmed);
        assert_eq!(
            previous
                .as_ref()
                .unwrap()
                .identity
                .revision(BuildRevision::Source)
                .as_str(),
            "src-1"
        );
    }
    println!(
        "PASS: five labels, frozen digests, exact asset closure; 11 validation and 7 pre-commit refusals preserve prior; 4 replacement outcomes and 8 reconciliation cases"
    );
}
```

## Verification and gaps

The literal is extracted byte-for-byte from this decision and compared with the
standalone input. The pinned `rustfmt` with the repository `rustfmt.toml`, pinned
`rustc --edition=2024 -Dwarnings`, and the finite valid/refusal executable run
are the bounded checks for this decision. Their exact commands, tool hashes,
literal/source/config hashes, outputs, and guard receipt belong in the immutable
attempt handoff manifest. These checks do not qualify production bytes, hash
algorithms, filesystem atomicity, power-loss durability, readiness, preview
consumption, browser output, or integrated native/WASM builds. Cargo, Clippy,
browser, provider, production-build, and integrated publication checks remain
unperformed.

The exact literal covers 11 validation refusals (including all three component
digests and both asset digests), seven known pre-commit failures, four replacement
outcomes, and eight reread/sync outcomes. The retained separate harness supplies
its own frozen oracle, exercises 27 invalid/incomplete cases and five same-label
digest substitutions against a distinct prior selection, then repeats all seven
pre-commit failures and four replacement/eight reconciliation boundaries. It
checks both typed outcomes and the actual model pointer after every operation.
These are finite sequencing counterexamples; no filesystem publication or
physical durability is inferred from them. Independent frontier review and
coordinator integration remain required.

Open implementation/integration facts: the typed shared `AssetId` does not yet
exist; exact content/configuration serialization and digest algorithm need to be
frozen by the implementation prerequisite; output-specific native/WASM build
commands and toolchain are not yet part of this source; atomic replacement and
directory-sync behavior need platform-specific verification; D02 readiness and
stable-preview promotion authority remain unresolved; no production build or
publication code exists. The scoped D03 devlog entry must report this missing
shared type and the fact that evidence is contract-only. No workflow queue or
authoritative task state is changed by this decision.
