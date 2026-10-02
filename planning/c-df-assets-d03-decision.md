# Durable backing and disposable asset cache decision

## Decision

A cache is a disposable read optimization. It is never the authority for whether a published asset exists or is available. `df-assets` owns the publication/access boundary and composes the existing `AssetStore` byte port with `AssetMetadataStore` metadata/access port. It must not introduce another durable registry or infer publication from a path, filename, hash-shaped value, cache index, or successful local read.

Publication has one ordered visibility boundary: write the complete immutable bytes to the configured durable byte root; verify their complete content identity against the candidate manifest; then commit the manifest/access metadata through the existing metadata owner. Only a successful metadata publication after byte verification creates a published reference. Until then, the candidate is staging/unpublished. Partial writes and failed or uncertain metadata commits return typed failure/unknown outcomes, never a ready or published reference. An immutable orphan blob is safe and may be reclaimed only by the storage owner's separately defined recovery/retention process; a published reference whose durable bytes are absent or fail integrity is unavailable and must fail closed.

For resolve/open, require a metadata-published immutable version and the caller's authorized scope. The durable `AssetStore` must confirm and provide the requested complete version. A cache may supply the response bytes only after the storage authority has confirmed that same published version is durably present and the cached bytes match its manifest identity; otherwise use the verified durable bytes or return an explicit refusal. A cache-only copy cannot repair a missing backing object, establish publication, change the version, or silently fall back to another version. Cache miss falls through to durable backing. Cache corruption/staleness evicts or ignores that entry and falls through to backing. A missing/corrupt durable object yields unavailable/integrity failure even if matching-looking local bytes exist. Cache eviction changes no manifest or publication state.

Cache keys include the complete immutable asset version identity and any representation/range dimensions that affect bytes. Access authorization is checked independently on every open; neither a cache key nor knowledge of a digest grants access. Range reads must bind to the authorized immutable version and report incomplete/truncated delivery as failure; only complete verified bytes can populate a reusable cache entry. The exact production hash/encoding, range completion protocol, cache capacity/TTL, filesystem durability/fsync policy, cleanup and recovery rules remain assigned to the existing G03/shared-type, `df-assets`, persistence, and deployment/storage owners. No numerical bound is selected here without measurements.

This keeps disposable cache roots separate from durable roots, workflow data, and telemetry. Local durable files may implement the configured initial backing adapter, but “local” alone is not a durability claim: the storage/deployment owner must define the owned root, restart/backup/restore behavior, permissions, health checks, and operational evidence. A developer checkout, browser cache, temporary file, staging path, or surviving cache across restart does not prove publication availability.

## Alternatives and limits

- **Treat any readable local/cache file as available:** rejected. It can be stale, partial, unauthorized, unreferenced, or left behind after metadata or backing loss; it also makes one developer's machine an accidental source of truth.
- **Let metadata alone prove byte availability:** rejected. The planned metadata store records references and access; it does not contain the media bytes. A published row whose object is absent is a broken reference and must be reported as unavailable.
- **Allow cache to rescue missing durable backing:** rejected. That converts an evictable, potentially node-local copy into hidden authority and masks a failed publication/recovery condition. If product requirements later need explicitly degraded cache-only service, that requires a separate reviewed contract with bounded lifetime, identity/integrity evidence, authorization, and observable degraded status; this task does not authorize it.
- **Check durable backing, then prefer a matching cache:** selected. It preserves cache usefulness for avoiding repeated byte transfer while keeping publication availability anchored to the existing durable byte and metadata owners. The adapter's durable-presence/integrity result must be trustworthy; a bare local `exists()` check is not that result.

## Owner and integration boundary

`df-assets` owns immutable byte publication, resolve/open behavior, durable-versus-disposable separation, and typed unavailable/integrity/version outcomes. `AssetStore` owns durable byte I/O and complete-byte verification; `AssetMetadataStore` owns PostgreSQL manifest/access metadata and publication visibility. The existing `df-persistence` owner implements the metadata persistence port. `df-session` or the existing authorized caller supplies trusted scope; access is not inferred from an ID or hash. Browser `AssetCache` remains a bounded client optimization under `df-client`; its local files never establish server publication or authorize access. Native deployment owns the configured durable root and its recovery/backup operations. Object storage may replace the byte adapter without changing the contract.

Integration must connect publish to staging completion, durable verification, metadata commit, and then resolve/open; connect asset references to authorized projections and byte delivery; and exercise metadata-present/backing-missing, cache hit, cache miss, stale cache, unauthorized scope, version mismatch, truncation, restart, and durable-root recovery against the selected adapter. This policy is not an implementation, transaction schema, hash specification, availability SLA, or proof of an integrated running service.

## Literal std-only contract example

This finite model checks the selected rule using full byte equality as a fixture identity. `published_version` represents a reference already made visible by the ordered publication boundary; `backing` represents a positive result from the durable storage authority, not `Path::exists()`. The example does not define production IDs, hashing, authorization, filesystem semantics, or cache capacity.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Version(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    NotPublished,
    VersionMismatch,
    BackingMissing,
    BackingIntegrityFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Source {
    CacheHit,
    DurableBacking,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Opened {
    source: Source,
    byte_count: usize,
}

fn open_asset(
    published_version: Option<Version>,
    requested_version: Version,
    backing: Option<(Version, &[u8])>,
    cache: Option<(Version, &[u8])>,
    expected_bytes: &[u8],
) -> Result<Opened, Refusal> {
    let Some(published) = published_version else {
        return Err(Refusal::NotPublished);
    };
    if requested_version != published {
        return Err(Refusal::VersionMismatch);
    }
    let Some((backing_version, backing_bytes)) = backing else {
        return Err(Refusal::BackingMissing);
    };
    if backing_version != published || backing_bytes != expected_bytes {
        return Err(Refusal::BackingIntegrityFailure);
    }
    if let Some((cache_version, cache_bytes)) = cache {
        if cache_version == published && cache_bytes == expected_bytes {
            return Ok(Opened {
                source: Source::CacheHit,
                byte_count: cache_bytes.len(),
            });
        }
    }
    Ok(Opened {
        source: Source::DurableBacking,
        byte_count: backing_bytes.len(),
    })
}

fn main() {
    let version = Version(7);
    let bytes = [0x44, 0x46, 0x01];

    let hit = open_asset(
        Some(version),
        version,
        Some((version, &bytes)),
        Some((version, &bytes)),
        &bytes,
    )
    .expect("published matching version with verified backing opens");
    assert_eq!(hit.source, Source::CacheHit);
    assert_eq!(hit.byte_count, bytes.len());

    let miss = open_asset(
        Some(version),
        version,
        Some((version, &bytes)),
        None,
        &bytes,
    )
    .expect("cache miss reads durable backing");
    assert_eq!(miss.source, Source::DurableBacking);

    assert_eq!(
        open_asset(
            Some(version),
            version,
            None,
            Some((version, &bytes)),
            &bytes
        ),
        Err(Refusal::BackingMissing),
    );
    assert_eq!(
        open_asset(
            Some(version),
            Version(8),
            Some((version, &bytes)),
            None,
            &bytes
        ),
        Err(Refusal::VersionMismatch),
    );
    assert_eq!(
        open_asset(
            None,
            version,
            Some((version, &bytes)),
            Some((version, &bytes)),
            &bytes
        ),
        Err(Refusal::NotPublished),
    );
    let changed = [0x44, 0x46, 0x02];
    assert_eq!(
        open_asset(
            Some(version),
            version,
            Some((version, &changed)),
            Some((version, &bytes)),
            &bytes,
        ),
        Err(Refusal::BackingIntegrityFailure),
    );
    let stale_cache = [0x44, 0x46, 0x00];
    let stale = open_asset(
        Some(version),
        version,
        Some((version, &bytes)),
        Some((Version(6), &stale_cache)),
        &bytes,
    )
    .expect("stale cache is ignored in favor of backing");
    assert_eq!(stale.source, Source::DurableBacking);
}
```

The executable fixture demonstrates cache hit, miss, absent publication, requested-version mismatch, backing absence despite a cache hit, backing integrity refusal, and stale-cache fallback. It proves only these local assertions. It does not prove that a production storage adapter's confirmation is durable or complete, nor publication atomicity, PostgreSQL behavior, authorization, stream/range completion, backup/restore, native/WASM integration, or real service availability.

## Unresolved production gates

Before implementation can claim the contract in production, the existing owners must freeze/reuse shared immutable asset IDs/manifests and hash/encoding fixtures; define typed staging, publish, lookup, open, range and failure/unknown outcomes; select and test the local durable-root lifecycle or object adapter; bind byte integrity checks to metadata publication; define transaction/uncertain-commit reconciliation and orphan recovery; enforce tenant/audience authorization before metadata and byte access; define range truncation/completion behavior and cache invalidation/version key dimensions; set cache and request limits from representative measurement; and exercise restart, restore, corruption, absent bytes, stale cache, and concurrent publication against the integrated native and browser boundaries. The browser cache/device behavior has not been tested, and this policy makes no phone compatibility claim. Independent review and root integration remain separate gates.

## Source basis

This decision applies [Subsystem architecture](subsystem-architecture.md) (the `df-assets` owner and `AssetStore`/`AssetMetadataStore` ports), [Subsystem interfaces](subsystem-interfaces.md) (PostgreSQL metadata versus durable byte I/O, byte verification before metadata publication, authorized resolve/open and separated disposable cache), [Storage architecture](storage-architecture.md) (separately configured durable root and object adapter), [Runtime reliability](runtime-reliability.md) (promote reusable media and do not depend on old developer caches), [Asset engine](asset-engine.md) (complete verified bytes plus metadata publication define ready; partial files are not hits), and [Service operations](service-operations.md) (tenant/audience authorization, hostile input, retention/deletion). Campaign-cinematics and the subsystem interfaces also govern audience-safe asset access and complete byte delivery. The immutable manifest and publication outcome are reused from the existing contracts; this policy adds no competing authority.
