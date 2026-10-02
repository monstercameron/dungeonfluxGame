# D04: asset retirement and replay suppression

Status: policy decision and finite contract proof; durable registry, asset adapter,
and restore integration remain unimplemented and unqualified

## Decision

Revocation and privacy suppression are current access decisions over immutable
asset versions. They do not rewrite a manifest, make an old cache authoritative,
or permit a stale completion to publish. The existing metadata/access authority
must retain a monotonic suppression generation and applicable tombstone. On each
private resolve, replay, open, and result publication, `df-assets` uses the
current authorized scope and suppression facts. A tombstoned or stale basis is
refused before private bytes are returned; cache presence and an old published
reference cannot override that refusal.

The suppression decision commits before completion is acknowledged or another
private read, replay, export, or publication is admitted. An ambiguous commit
keeps the affected scope closed with a pending/unknown outcome. A changed
generation fences queued and in-flight work. Before a completion becomes a
published reference, the asset owner rechecks the same current authority and the
captured generation, job, asset version, and source/access basis. A stale or
suppressed completion remains an explicit refusal; it cannot clear a tombstone,
advance a visible version, or trigger implicit regeneration. Reissue requires a
new current authorization and a new job identity under the existing admission
owner.

Replay and restore apply the latest protected suppression overlay before using
any snapshot, cache, prepared asset, recording, or retained byte object. Missing
or unverifiable current policy facts fail closed for the affected private scope.
The tombstone takes precedence over a stale published reference. Retained bytes
may remain for separately reviewed retention or recovery purposes, but their
presence does not grant access. Physical deletion and backup purge remain the
storage and lifecycle owners' separate work; this decision does not claim either
has run.

Asset retirement also follows the existing immutable-version policy: a new
version does not silently replace an old identity, and retirement of access does
not mutate bytes or historical references. Current rights revocation and subject
suppression are related access gates with distinct meanings. A revoked
`RightsGrant` blocks the uses covered by that grant; a privacy tombstone suppresses
the affected subject payload. Either can deny replay, and suppression remains
effective if another retained record or grant would otherwise allow use.

## Owners and next registry consumer

`df-persistence` is the next concrete registry consumer: the source design assigns
it the existing consumer-owned `AssetMetadataStore` implementation and the
durable deletion plan, suppression generation, tombstone, and recovery journal.
That implementation is not present in the current checkout. It must persist the
monotonic fact and supply the current access decision without creating a second
asset registry. `df-assets` owns the asset-facing resolve/open/publication checks
and source-owned cache invalidation. Existing authorized callers supply trusted
principal and audience context; IDs, hashes, trace fields, or cached keys do not
grant access. The future persistence and asset implementations must coordinate
their exact shared contract through the already assigned G03/G05 owners; this
decision invents no Rust type, database schema, or port method.

The first integrated boundary is `df-persistence`'s `AssetMetadataStore` result
consumed by `df-assets` for private resolve/open and completion publication. It
must demonstrate: commit suppression before acknowledgement; reject a read after
that commit even when a stale reference or matching cache entry exists; reject a
completion whose captured generation predates suppression; and reapply the
protected tombstone before serving a restored snapshot. `df-media` may request a
new candidate through the existing publication path, but cannot bypass current
rights or publish a stale completion. No deletion worker, provider, content,
live storage, or production adapter is introduced here.

## Alternatives and rationale

- **Treat an immutable manifest or cache entry as access authority:** rejected.
  Both can outlive current rights or suppression state.
- **Delete bytes first and suppress after cleanup:** rejected. A read, replay, or
  restore could race cleanup; the decision must block access before cleanup.
- **Let a late completion recreate a published reference:** rejected. It can
  publish private media after revocation even when the provider/job was valid
  when started.
- **Use a second asset-local tombstone registry:** rejected. Divergent registries
  can disagree after retries or restore; the existing metadata/lifecycle
  authority owns current durable facts.
- **Use the current protected overlay and a generation fence:** selected. It
  reuses the existing metadata/access and deletion/recovery ownership while
  refusing stale data at replay, open, and publication boundaries.

## Finite replay and stale-completion proof

This standard-library-only Rust literal exercises one bounded ordering. It first
publishes an authorized private version, saves a replay reference, and starts a
completion at generation 7. It then commits suppression at generation 8. The
stale completion, replay reference, and cache bytes are all refused against the
current overlay. The in-memory registry and scalar IDs are fixtures, not
production API, durable storage, cryptographic identity, or authorization.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Version(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Completion {
    version: Version,
    generation: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReplayReference {
    version: Version,
    generation: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Suppressed,
    StaleCompletion,
    StaleReference,
    NotPublished,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CurrentPolicy {
    generation: u8,
    tombstoned: bool,
}

#[derive(Default)]
struct Registry {
    current: Option<Version>,
}

fn publish_completion(
    registry: &mut Registry,
    policy: CurrentPolicy,
    completion: Completion,
) -> Result<(), Refusal> {
    if policy.tombstoned {
        return Err(Refusal::Suppressed);
    }
    if completion.generation != policy.generation {
        return Err(Refusal::StaleCompletion);
    }
    registry.current = Some(completion.version);
    Ok(())
}

fn resolve_replay(
    registry: &Registry,
    policy: CurrentPolicy,
    saved_reference: ReplayReference,
    cache_contains_bytes: bool,
) -> Result<Version, Refusal> {
    if policy.tombstoned {
        return Err(Refusal::Suppressed);
    }
    if saved_reference.generation != policy.generation {
        return Err(Refusal::StaleReference);
    }
    if !cache_contains_bytes || registry.current != Some(saved_reference.version) {
        return Err(Refusal::NotPublished);
    }
    Ok(saved_reference.version)
}

fn main() {
    let version = Version(4);
    let before_suppression = CurrentPolicy {
        generation: 7,
        tombstoned: false,
    };
    let mut registry = Registry::default();
    let original = Completion {
        version,
        generation: before_suppression.generation,
    };
    assert_eq!(
        publish_completion(&mut registry, before_suppression, original),
        Ok(())
    );
    let saved_reference = ReplayReference {
        version: registry.current.expect("fixture published a version"),
        generation: before_suppression.generation,
    };
    let in_flight = Completion {
        version: Version(5),
        generation: before_suppression.generation,
    };

    // The durable suppression decision advances before later asset work is admitted.
    let after_suppression = CurrentPolicy {
        generation: 8,
        tombstoned: true,
    };
    assert_eq!(
        publish_completion(&mut registry, after_suppression, in_flight),
        Err(Refusal::Suppressed)
    );
    assert_eq!(registry.current, Some(version));
    assert_eq!(
        resolve_replay(&registry, after_suppression, saved_reference, true),
        Err(Refusal::Suppressed)
    );

    // A changed scope generation also fences the old job and replay reference.
    let changed_generation = CurrentPolicy {
        generation: 8,
        tombstoned: false,
    };
    assert_eq!(
        publish_completion(&mut registry, changed_generation, in_flight),
        Err(Refusal::StaleCompletion)
    );
    assert_eq!(registry.current, Some(version));
    assert_eq!(
        resolve_replay(&registry, changed_generation, saved_reference, true),
        Err(Refusal::StaleReference)
    );
}
```

The fixture proves its explicit finite cases: an old completion is refused after
the suppression generation advances; tombstoned replay is refused even with a
published reference and matching cache bytes; and a completion remains stale if
the generation changes while the current scope remains active. A saved reference
from that earlier generation is also refused. It does not
prove durable ordering, trusted-principal authorization, concurrent requests,
cross-process propagation, range/read-ahead cancellation, cryptographic byte
integrity, backup restoration, physical erasure, or running service behavior.
Those integrated checks remain unperformed because the named registry and asset
crates are absent in this source revision.

## Remaining implementation gates

The G03/G05 owners must freeze or reuse shared identity, authorization, version,
and current-policy contracts. `df-persistence` must establish durable monotonic
suppression and nonregressing restore behavior; `df-assets` must consume those
facts at resolve/open and before publication, including stream completion and
source-owned cache invalidation. The coordinator must then integrate and review
the adapter boundary with replay, stale completion, restore, and multi-audience
cases against the selected storage implementation. Exact schema, transaction
fencing, consistency behavior during journal/storage outage, bounded cleanup,
backup purge, native/WASM checks, and running-output review remain pending. This
policy proves no production implementation or deletion outcome.

## Source basis

This decision follows [D01](c-df-assets-d01-decision.md) (immutable publication
and access scope), [D02](c-df-assets-d02-decision.md) (current rights and source
reuse), [D03](c-df-assets-d03-decision.md) (durable backing versus disposable
cache), [Erasure and retention](erasure-retention-policy.md) (suppression-first,
monotonic tombstones and restore overlay), [Runtime reliability](runtime-reliability.md)
(generation/job fencing), [Asset engine](asset-engine.md) (stale and superseded
results), [G12-D03](g12-d03-heuristic-demand-policy.md) (current rights before
replay and delivery), [Campaign cinematics](campaign-cinematics.md) (source,
audience, and export rechecks), and [Subsystem interfaces](subsystem-interfaces.md)
(`AssetStore` and `AssetMetadataStore` ownership). The design assigns durable
deletion records and the metadata adapter to `df-persistence`, and asset-facing
checks to `df-assets`; implementation and service behavior are not present here.
