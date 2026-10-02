# D01: immutable asset identity and publication contract

Status: design decision; hosted backing direction selected, with concrete storage adapter, digest, authorization and runtime integration pending their existing owners.

## Decision and authority

`df-assets` owns immutable durable media bytes and manifests through its existing `AssetStore` and `AssetMetadataStore` ports. Keep `AssetStore` authoritative for staging and durable byte I/O; keep `AssetMetadataStore` authoritative for PostgreSQL metadata and access references. The existing `df-persistence` adapter implements those consumer-owned ports. Publication is one `df-assets` operation across those authorities: a manifest must not become resolvable until the complete staged byte sequence is verified against its declared length and content hash and durable byte storage has confirmed it; metadata visibility follows successful byte completion. An incomplete or failed operation produces no published reference. A complete orphaned immutable blob is safe and can be recovered/cleaned up; a visible reference to missing, partial, or mismatched bytes is forbidden.

A published asset version binds one immutable manifest and one exact complete byte sequence. The manifest's content hash is computed over those exact bytes without lossy normalization. It carries that hash alongside MIME/codec, byte length, duration/timebase where relevant, variant relationships, source/generator revisions, readiness/fallback, and access scope. These values describe the same version: after completeness is established, publishing the same version identifier with different bytes or any different manifest field is a version conflict and must refuse without replacing the previous version. An exact repeat is idempotent. Changed bytes or metadata require a fresh version identifier; existing references continue to resolve the old version while it is retained. Equal bytes may be deduplicated only when the store preserves the same immutable binding. Hash equality alone is neither proof of byte completeness nor permission to read.

The operation retains typed outcomes: published or idempotently already-published on exact match; reject incomplete length, hash mismatch, immutable-version conflict, or unsupported manifest; report storage/metadata failure without a success receipt. Check complete length first. For an existing version, compare the entire manifest and complete byte sequence before checking the candidate digest or other manifest validity: an exact repeat is idempotent, and any complete non-identical reuse is a version conflict. For a new version, validate the manifest and compute its digest over the complete bytes before publication. This ordering does not accept a mismatched digest; it prevents the digest error from masking an attempted immutable-version replacement. If commit outcome is ambiguous, return/retain unknown and reconcile by the stable publication operation/version identity; never tell the caller publication succeeded based on a staged write or a lost acknowledgment. A retry of a confirmed identical operation can return the same immutable receipt. Cleanup only removes unreferenced staging/orphans under the byte-store owner's recovery policy.

## Alternatives and rationale

* **Metadata first, bytes later:** rejected because resolution could expose a reference to incomplete or absent media.
* **Treat the digest as the whole asset and omit full-byte verification:** rejected because a hash is metadata, can be incorrectly declared, and does not prove that all bytes arrived or remain available. Verify the staged complete bytes before publication and preserve the binding in durable storage.
* **Overwrite a mutable latest asset key:** rejected because old references could silently change content or metadata. A changed version is a new immutable identity.
* **Use an opaque immutable version binding plus an exact-byte digest check:** selected. This meets the version invariant while reusing the existing `df-assets` byte and metadata ports and avoids inventing a second authority.

## Ownership and integration hooks

`df-assets` owns manifest semantics, staging/publication orchestration, immutable version conflict behavior, resolve/open and the complete-byte refusal contract. `AssetStore` owns staging and durable byte I/O; `AssetMetadataStore` owns metadata persistence and access-scope enforcement; `df-persistence` implements both adapters and the durable atomic/ordering behavior. `df-auth` supplies trusted principal/audience separately; possession or knowledge of a content hash never authorizes access. `df-api` projects only authorized references; `df-media` may request publication of generated candidates through the same boundary, but has no alternate write path. `df-session` retains gameplay commit authority; this asset policy does not create a game-state writer, decide media-generation jobs, or authorize publication of a candidate.

The integration proof must cut/fail between staging, verification, durable byte completion, metadata publication and receipt; assert no partial/incorrect manifest resolves, retries do not overwrite, and old versions still resolve. Connect resolve/open authorization to current scope and verify served ranges against the pinned manifest version. These cross-owner hooks are proposals for later integration tasks, not completed behavior here.

## Unresolved production gates

The current plans do not freeze the manifest's concrete Rust/schema representation, canonical field encoding, cryptographic digest algorithm, opaque version identifier, or collision policy beyond safe refusal. `df-types` owns shared ID representations; `df-assets` owns domain manifest and publication outcomes; G03/G05 and `df-persistence` freeze storage layout and adapter transaction/recovery semantics. For hosted initial deployment, follow the [Service operations](service-operations.md#initial-and-scaled-deployment) refinement named as governing by the [Storage architecture](storage-architecture.md#hosted-service-operating-contract): one native Rust process, managed PostgreSQL for metadata, and durable object-backed media behind `AssetStore`. The earlier local durable file-root option remains suitable for development and integration through that adapter boundary; it does not replace the selected hosted backing. Concrete object provider, storage layout, consistency and durability evidence, transaction/recovery and restore qualification, and retention policy remain G05/storage/deployment gates. Complete durable bytes must precede metadata visibility in every adapter. MIME sniffing/decoder agreement and image/video resource limits remain the service-operations media-validation boundary. Concrete maximum sizes, durations, range arithmetic, staging expiry and cleanup bounds must be selected by the existing G05/storage and media owners from workload/deployment evidence, not inferred here. Access-policy/retention changes do not mutate the bytes or erase identity; revocation blocks future access through existing auth/access policy. No provider, generation, runtime integration, API freeze, benchmark, or production persistence behavior is claimed.

## Finite literal contract example

This std-only example exercises the publication decision with a deliberately small, non-cryptographic checksum to keep it executable without dependencies. It demonstrates completeness, hash/manifest agreement, immutable version conflict, idempotent identical retry, and preservation of an old version. The checksum and tiny in-memory store are fixtures, not a proposed production digest, API, storage implementation, byte bound, or authorization mechanism. A production adapter must use its selected digest over exact complete bytes and collision-safe version binding.

```rust
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
struct Manifest {
    version: u8,
    hash: u32,
    byte_len: u64,
    mime: &'static str,
    codec: &'static str,
    access_scope: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Published {
    manifest: Manifest,
    bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Incomplete,
    HashMismatch,
    InvalidManifest,
    VersionConflict,
}

#[derive(Default)]
struct Store {
    versions: BTreeMap<u8, Published>,
}

// Fixture checksum only; production selects and pins a cryptographic digest.
fn fixture_hash(bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .fold(0_u32, |sum, byte| sum.wrapping_add(u32::from(*byte)))
}

impl Store {
    fn publish(&mut self, manifest: Manifest, staged: &[u8]) -> Result<bool, Refusal> {
        if u64::try_from(staged.len()).ok() != Some(manifest.byte_len) {
            return Err(Refusal::Incomplete);
        }
        if let Some(previous) = self.versions.get(&manifest.version) {
            return if previous.manifest == manifest && previous.bytes.as_slice() == staged {
                Ok(false)
            } else {
                Err(Refusal::VersionConflict)
            };
        }
        if manifest.mime.is_empty() || manifest.codec.is_empty() {
            return Err(Refusal::InvalidManifest);
        }
        if fixture_hash(staged) != manifest.hash {
            return Err(Refusal::HashMismatch);
        }
        self.versions.insert(
            manifest.version,
            Published {
                manifest,
                bytes: staged.to_vec(),
            },
        );
        Ok(true)
    }
}

fn main() {
    let complete = b"asset-v1";
    let manifest = Manifest {
        version: 1,
        hash: fixture_hash(complete),
        byte_len: complete.len() as u64,
        mime: "image/png",
        codec: "png",
        access_scope: 7,
    };
    let mut store = Store::default();
    assert_eq!(store.publish(manifest.clone(), complete), Ok(true));
    assert_eq!(store.publish(manifest.clone(), complete), Ok(false));
    assert_eq!(
        store.publish(manifest.clone(), &complete[..3]),
        Err(Refusal::Incomplete)
    );

    let mut wrong_hash = manifest.clone();
    wrong_hash.version = 3;
    wrong_hash.hash = wrong_hash.hash.wrapping_add(1);
    assert_eq!(
        store.publish(wrong_hash, complete),
        Err(Refusal::HashMismatch)
    );

    let mut changed_manifest = manifest.clone();
    changed_manifest.access_scope = 8;
    assert_eq!(
        store.publish(changed_manifest, complete),
        Err(Refusal::VersionConflict)
    );
    assert_eq!(
        store.publish(manifest.clone(), b"asset-v2"),
        Err(Refusal::VersionConflict)
    );
    // The fixture checksum also collides for these complete, different bytes.
    assert_eq!(fixture_hash(complete), fixture_hash(b"asset-w0"));
    assert_eq!(
        store.publish(manifest.clone(), b"asset-w0"),
        Err(Refusal::VersionConflict)
    );
    let mut unsupported = manifest.clone();
    unsupported.version = 4;
    unsupported.mime = "";
    assert_eq!(
        store.publish(unsupported, complete),
        Err(Refusal::InvalidManifest)
    );
    assert_eq!(store.versions.get(&1).unwrap().bytes, complete);

    let changed = b"asset-v2";
    let next = Manifest {
        version: 2,
        hash: fixture_hash(changed),
        byte_len: changed.len() as u64,
        mime: "image/png",
        codec: "png",
        access_scope: 7,
    };
    assert_eq!(store.publish(next, changed), Ok(true));
    assert_eq!(store.versions.get(&1).unwrap().bytes, complete);
    assert_eq!(store.versions.get(&2).unwrap().bytes, changed);
}
```

## Source basis and compatibility

This decision uses the existing [subsystem crate map](subsystem-architecture.md), especially `df-assets`, `df-persistence`, `df-auth`, `df-api` and `df-media`; the [durable-media port contract](subsystem-interfaces.md#postgresql-persistence-and-durable-media); [storage recovery and immutable version policy](storage-architecture.md#runtime-director-state-and-replay); [asset lifecycle and complete publication requirements](asset-engine.md#models-public-operations-and-lifecycle); [runtime media boundaries](runtime-reliability.md#audio-media-and-provider-boundaries); [media hostile-input controls](service-operations.md#tenant-isolation-and-hostile-input) and [media decode limits](service-operations.md#additional-security-enforcement-details); and [cinematic location/item identity continuity](campaign-cinematics.md#location-and-item-continuity). G10-D02's exact-byte immutable-version rule is compatible background, but this task does not implement a pack encoder, pack publication flow or activation policy. Existing planned port signatures remain candidates; no competing type or authority is added.
```
