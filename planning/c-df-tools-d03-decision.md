# df-tools atomic artifact publication and provenance decision

Task/attempt: `B-C-df-tools-D03/a1`

Input revision: `4c0e050c2d30e632bd3b32c51a92cfda52479e6e`
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
commit point: before rename, readers see the prior accepted manifest; after
rename, it names a complete immutable set. Never publish per-component pointers
or mutate the current accepted directory in place. A missing label, missing
native/WASM/configuration component, missing or extra asset, duplicate/invalid
asset ID, digest mismatch, incomplete evidence, or any staging/write/sync/rename
failure leaves the previous pointer intact and reports a typed refusal/failure.
Ambiguous pointer-replacement outcomes must be resolved by rereading the pointer
and manifest identity; they must not be reported as success based on the attempted
write alone.

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
publication module wired from the existing crate entry points. Its first input is
an explicit build request containing the five shared identity labels, build
configuration bytes, resolved asset IDs, toolchain identity, and nonsecret build
arguments. It stages native, WASM, config, and asset bytes under one attempt-owned
directory; computes digests from those bytes; validates exact set closure; writes
the immutable manifest; then commits the single pointer. A reader returns one
manifest and its immutable directory as a unit. Keep preview registration private
to the D02 owner and require it to claim that returned set, not reconstruct it
from labels. No public/admin RPC or content-publication operation is added; those
transport and authority decisions remain gated by X10/G03.

## Bounded contract literal

This standalone literal imports the actual `df-types` provenance source by path.
It models complete identity construction, distinct component digests, exact
asset-set closure, and the final single pointer assignment. It does not perform
filesystem writes, calculate cryptographic digests, run a Cargo build, or prove
crash durability. The finite checks establish the contract decision only.

```rust
#[path = "/Users/earlcameron/Desktop/dungeonflux/artifacts/worktrees/tools_artifact_provenance/crates/df-types/src/provenance.rs"]
mod provenance;

use provenance::{BuildIdentity, BuildIdentityError, RevisionLabel};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Digest([u8; 32]);

#[derive(Clone, Debug, PartialEq, Eq)]
struct AssetEntry {
    id: String,
    digest: Digest,
}

#[derive(Clone, Debug)]
struct Candidate {
    identity: [&'static str; 5],
    native: Option<Digest>,
    wasm: Option<Digest>,
    configuration: Option<Digest>,
    expected_asset_ids: Vec<String>,
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
enum Refusal {
    IncompleteIdentity,
    MissingComponent,
    InvalidAssetId,
    DuplicateAssetId,
    AssetSetMismatch,
}

fn publish(current: &mut Option<Manifest>, candidate: Candidate) -> Result<(), Refusal> {
    let identity = BuildIdentity::new(
        Some(candidate.identity[0]),
        Some(candidate.identity[1]),
        Some(candidate.identity[2]),
        Some(candidate.identity[3]),
        Some(candidate.identity[4]),
    )
    .map_err(|_: BuildIdentityError| Refusal::IncompleteIdentity)?;
    let native = candidate.native.ok_or(Refusal::MissingComponent)?;
    let wasm = candidate.wasm.ok_or(Refusal::MissingComponent)?;
    let configuration = candidate.configuration.ok_or(Refusal::MissingComponent)?;

    let mut expected = BTreeSet::new();
    for id in candidate.expected_asset_ids {
        RevisionLabel::new(Some(&id)).map_err(|_| Refusal::InvalidAssetId)?;
        if !expected.insert(id) {
            return Err(Refusal::DuplicateAssetId);
        }
    }
    let mut observed = BTreeSet::new();
    for asset in &candidate.assets {
        RevisionLabel::new(Some(&asset.id)).map_err(|_| Refusal::InvalidAssetId)?;
        if !observed.insert(asset.id.clone()) {
            return Err(Refusal::DuplicateAssetId);
        }
    }
    if expected != observed {
        return Err(Refusal::AssetSetMismatch);
    }

    let mut assets = candidate.assets;
    assets.sort_by(|left, right| left.id.cmp(&right.id));
    let manifest = Manifest {
        identity,
        native,
        wasm,
        configuration,
        assets,
    };
    // The implementation's analogous operation is one atomic pointer rename.
    *current = Some(manifest);
    Ok(())
}

fn candidate() -> Candidate {
    Candidate {
        identity: ["src-1", "native-1", "wasm-1", "config-1", "content-1"],
        native: Some(Digest([1; 32])),
        wasm: Some(Digest([2; 32])),
        configuration: Some(Digest([3; 32])),
        expected_asset_ids: vec!["map:crypt-1".to_owned(), "audio:theme-1".to_owned()],
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
    let mut accepted = None;
    assert_eq!(publish(&mut accepted, candidate()), Ok(()));
    let previous = accepted.clone();

    let mut incomplete = candidate();
    incomplete.identity[4] = "";
    assert_eq!(
        publish(&mut accepted, incomplete),
        Err(Refusal::IncompleteIdentity)
    );
    assert_eq!(accepted, previous);

    let mut missing_wasm = candidate();
    missing_wasm.wasm = None;
    assert_eq!(
        publish(&mut accepted, missing_wasm),
        Err(Refusal::MissingComponent)
    );
    assert_eq!(accepted, previous);

    let mut omitted_asset = candidate();
    omitted_asset.assets.pop();
    assert_eq!(
        publish(&mut accepted, omitted_asset),
        Err(Refusal::AssetSetMismatch)
    );
    assert_eq!(accepted, previous);

    let mut duplicate_asset = candidate();
    duplicate_asset
        .assets
        .push(duplicate_asset.assets[0].clone());
    assert_eq!(
        publish(&mut accepted, duplicate_asset),
        Err(Refusal::DuplicateAssetId)
    );
    assert_eq!(accepted, previous);

    let mut invalid_asset = candidate();
    invalid_asset.assets[0].id = "private token".to_owned();
    assert_eq!(
        publish(&mut accepted, invalid_asset),
        Err(Refusal::InvalidAssetId)
    );
    assert_eq!(accepted, previous);

    let manifest = accepted.expect("the valid complete candidate remains accepted");
    assert_eq!(
        manifest
            .identity
            .revision(provenance::BuildRevision::Source)
            .as_str(),
        "src-1"
    );
    assert_ne!(manifest.native, manifest.wasm);
    assert_eq!(manifest.assets[0].id, "audio:theme-1");
    println!(
        "PASS: complete identity, distinct artifacts, exact asset closure, 5 refusals retain prior pointer"
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

Open implementation/integration facts: the typed shared `AssetId` does not yet
exist; exact content/configuration serialization and digest algorithm need to be
frozen by the implementation prerequisite; output-specific native/WASM build
commands and toolchain are not yet part of this source; atomic replacement and
directory-sync behavior need platform-specific verification; D02 readiness and
stable-preview promotion authority remain unresolved; no production build or
publication code exists. The scoped D03 devlog entry must report this missing
shared type and the fact that evidence is contract-only. No workflow queue or
authoritative task state is changed by this decision.
