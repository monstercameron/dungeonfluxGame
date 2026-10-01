# G10-D02: immutable pack and provenance validation contract

Status: design decision for G10; production types, codecs, hashing, persistence,
replay, and integration remain unimplemented.

This policy freezes the boundary expected by B-G10-D02. `df-model` owns shared
identity/version/value types, `df-content` owns the pure schemas and validator,
and `df-engine` owns activation, committed decisions, and replay. `df-persistence`
owns durable bytes, publication metadata, snapshots, decisions, and fencing.
Consumers must use these same contracts; this document does not authorize a
second pack format or a production stand-in.

## Immutable pack identity and encoding

The first contract version is `CampaignPackSchemaV1` with `schema_version = 1`.
An immutable pack consists of a manifest plus the complete canonical payload.
The manifest pins the stable `CampaignId`, schema version, exact `RulesetId`,
rules catalog revision and source-manifest digest, creator/access scope, locale
set, provenance/rights records, and all referenced campaign, director, world,
encounter, interaction, style, and asset records. Stable template IDs identify
definitions; runtime entity IDs identify each instantiated NPC, item, location,
or encounter. A `PackVersion` is the SHA-256 digest of the canonical payload;
the digest does not grant read or use access.

Canonical payload bytes use the `DFCP` four-byte magic, one byte of schema
version, then the manifest and records encoded in the schema's fixed field order.
Every variable-length byte/string/list field is prefixed by an unsigned 64-bit
big-endian byte/item length; integers are fixed-width big-endian; strings are
UTF-8 without normalization; optional values have a one-byte presence tag.
Records are sorted by their stable typed ID bytes, set-like ID lists are sorted,
and ordered lists retain semantic order. Duplicate IDs are invalid. Maps are
encoded as sorted key/value pairs. Floating point is forbidden in v1 canonical
records; use bounded integers or exact rational values with normalized positive
denominators. Hash the entire canonical byte sequence, including magic and schema
version, using SHA-256. Implementations must compare the recomputed digest before
publication or activation. Transport/storage codecs may differ only if their
decoded value produces these exact canonical bytes.

Pack versions are content addressed and never updated in place. Editing creates
a new draft and a new version. Publication atomically makes the complete bytes
and metadata visible; incomplete staging is never activatable. Activation pins
the exact pack version, ruleset/catalog/source revisions, schema, and initial
state in the run. A live change requires an explicit migration plan and fenced
commit mapping preserved IDs, facts, beats, sources, and pending jobs. If the
schema or migration is unsupported, reject activation/migration or create a
separate run; never reinterpret, partially apply, or retcon a version.

## Graph and provenance validation

`ContentPack::validate(candidate, pinned_catalog, limits)` is pure and returns
either a validated immutable candidate or bounded, source-located typed
diagnostics. It performs no filesystem, database, clock, network, provider, or
SDK work. Native tools own import and byte limits; `df-content` validates the
already bounded candidate and never fetches external references.

Validation applies these rules before publication:

* The schema, ruleset, catalog revision, and source-manifest digest must be
  supported and match the pinned catalog. Unsupported versions return
  `UnsupportedVersion`; absent/changed rules sources return `SourceGap` or
  `CatalogMismatch`, never a permissive fallback.
* IDs are unique within their declared type and namespace. Every typed
  reference resolves to exactly one record of the required type. Runtime
  instance IDs cannot substitute for template IDs.
* Beat prerequisites, disclosures, and asset dependencies form directed acyclic
  graphs. All referenced nodes exist. Each declared alternative has an explicit
  entry and terminal condition and is reachable from its owning beat; a
  condition is data interpreted by the approved director contract, never script
  or executable rules. A required campaign start reaches at least one valid
  terminal path. Cycles, dangling edges, and alternatives with no reachable
  resolution are rejected.
* Asset references resolve to a complete manifest entry in the same pinned pack
  or to an explicitly version-pinned dependency with compatible access and use
  rights. Fallback references are validated by the same rule. A digest alone is
  not authorization, and no URL is fetched during validation.
* Every canonical fact, rule-bearing template, disclosure, and imported claim
  has provenance linking it to a source-manifest entry and source locator or an
  authorized authored-origin record. Rights, permitted use, namespace, and
  attribution are explicit. Missing or incompatible provenance is rejected.
  Extracted/generated lore remains a candidate until a creator confirms each
  canonical claim and knowledge grant.
* Uniqueness/contradiction constraints, NPC knowledge and reveal conditions,
  localization keys, geometry/encounter references, and usable fallback assets
  are checked against the pinned domain contracts. Unsupported mechanics produce
  typed `UnsupportedRules`; imported prose is never executable behavior.
* Limits are explicit inputs: maximum bytes, records by type, graph nodes/edges,
  nesting depth, diagnostics, and string bytes. Exceeding any limit rejects the
  whole candidate with a typed limit diagnostic; validation never truncates
  into apparent success.

Diagnostics expose safe codes, typed record IDs, and source locations, not private
lore or credentials. A failed validation yields no `ValidatedPack`, publication
handle, or activatable reference. Drafts remain private and partial imports cannot
modify an active run. Publish and activate are distinct authorized operations
with operation ID, payload fingerprint, expected draft/current version, and
access scope; retries deduplicate by operation and payload, not filename.

## Decisions, time, and replay provenance

Authoritative time is `WorldTime`, an exact integer count of milliseconds from a
run-defined origin. It advances only through a committed, source-valid transition
or approved due-event processing. Wall-clock time is not an input to rules or
replay. A paused/stopped run cannot advance from elapsed wall time. Presentation
time has a separate monotonic epoch and cannot advance world time or rules timers.
Due work is ordered by `(world_time_ms, event_id)`; catch-up is bounded and its
remaining cursor is persisted explicitly.

Each committed decision records, in order: run and session revision; exact pack,
schema, rules/catalog/source, reducer, and policy versions; decision/event ID;
authorized input basis and source fact references; ordered proposals and accepted
semantic outputs; ordered random/dice draws including stream identity; resulting
facts/effect intents; and the resulting canonical state hash. No provider response
is regenerated to reconstruct an accepted decision. If generated semantic output
affected a commit, the accepted output and its provenance are part of the durable
decision record; private prompt material follows the separate rights/retention
policy.

Replay applies recorded decisions in original order with no model/provider calls
and no hidden clock or random source. A reducer is replay-compatible only for an
explicit `(schema_version, reducer_version, ruleset/catalog revision)` tuple and
must produce the recorded state hash on fixtures. Snapshot state is authoritative
with its committed decision history; it need not be rebuilt from the beginning.
Full reconstruction is available only where every reducer/migration in the range
is supported and hashes match. Otherwise return a typed `ReplayGap` naming the
first unsupported tuple, missing provenance, or hash mismatch; do not continue
with substituted semantics or report successful replay.

Schema additions that preserve old meaning get a new schema version and an
explicit migration. Any field removal, byte interpretation change, graph semantic
change, provenance ordering change, or hash framing change is incompatible and
requires a new major schema/reducer contract. Old immutable bytes remain readable
only through an explicitly supported decoder. Unknown versions are rejected at
decode, validation, activation, and replay boundaries.

## Required boundary behavior and ownership

`df-model` defines distinct typed IDs, `PackVersion`, `SchemaVersion`,
`RulesetId`, `SourceManifestDigest`, `WorldTime`, `DecisionId`, `EventId`, and
typed rejection/gap codes. `df-content` owns v1 authoring records, canonical
encoding, digest verification, limits, graph checks, provenance checks, and
diagnostics. `df-engine` activates an exact validated version and commits ordered
decision provenance under the existing session owner/fence; its pure reducer
accepts explicit time/draw inputs. `df-persistence` atomically stores complete
immutable bytes and metadata, fenced snapshots/decisions, and operation results.
`df-tools` may produce a draft/import report but cannot publish or activate without
the separately authorized operation. RPC and serialization codecs remain owned
by `df-api`/`df-protocol`/persistence consumers under G03; this contract adds no
RPC method.

Required rejection cases include unsupported schema/reducer/catalog versions,
digest mismatch, duplicate IDs, dangling or mistyped references, beat/asset
cycles, unreachable alternatives, missing source/rights provenance, malformed
canonical fields, limit overflow, stale activation/migration basis, unauthorized
private draft access, and replay hash mismatch. Rejections are typed domain
outcomes; storage/auth/capacity failures remain safe boundary failures with
uncertain commit preserved where applicable.

## Literal std-only contract example

This small example demonstrates the graph/provenance boundary and refusal
behavior. It is illustrative contract code, not a production API or a substitute
for canonical encoding, cryptographic digest verification, source catalogs, or
durable transaction/fencing. `rustc --edition 2024 -D warnings` and execution are
required after coordinator admission; neither has been run for this design change.

```rust
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Id(u16);

#[derive(Clone, Debug)]
struct Beat {
    id: Id,
    prerequisites: Vec<Id>,
    provenance_source: Option<Id>,
}

#[derive(Clone, Debug)]
struct Pack {
    schema: u16,
    sources: BTreeSet<Id>,
    beats: Vec<Beat>,
    assets: BTreeMap<Id, Vec<Id>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Refusal {
    UnsupportedSchema,
    DuplicateBeat(Id),
    MissingSource(Id),
    MissingBeat { beat: Id, target: Id },
    BeatCycle(Id),
    MissingAsset(Id),
    AssetCycle(Id),
}

fn visit(
    id: Id,
    edges: &BTreeMap<Id, Vec<Id>>,
    temporary: &mut BTreeSet<Id>,
    permanent: &mut BTreeSet<Id>,
) -> Result<(), Id> {
    if permanent.contains(&id) {
        return Ok(());
    }
    if !temporary.insert(id) {
        return Err(id);
    }
    if let Some(next) = edges.get(&id) {
        for child in next {
            visit(*child, edges, temporary, permanent)?;
        }
    }
    temporary.remove(&id);
    permanent.insert(id);
    Ok(())
}

fn acyclic(edges: &BTreeMap<Id, Vec<Id>>) -> Result<(), Id> {
    let mut temporary = BTreeSet::new();
    let mut permanent = BTreeSet::new();
    for id in edges.keys() {
        visit(*id, edges, &mut temporary, &mut permanent)?;
    }
    Ok(())
}

fn validate(pack: &Pack) -> Result<(), Refusal> {
    if pack.schema != 1 {
        return Err(Refusal::UnsupportedSchema);
    }
    let mut beats = BTreeMap::new();
    for beat in &pack.beats {
        if beats.insert(beat.id, beat).is_some() {
            return Err(Refusal::DuplicateBeat(beat.id));
        }
        if let Some(source) = beat.provenance_source {
            if !pack.sources.contains(&source) {
                return Err(Refusal::MissingSource(source));
            }
        } else {
            return Err(Refusal::MissingSource(beat.id));
        }
    }
    let mut beat_edges = BTreeMap::new();
    for beat in &pack.beats {
        for target in &beat.prerequisites {
            if !beats.contains_key(target) {
                return Err(Refusal::MissingBeat {
                    beat: beat.id,
                    target: *target,
                });
            }
        }
        beat_edges.insert(beat.id, beat.prerequisites.clone());
    }
    acyclic(&beat_edges).map_err(Refusal::BeatCycle)?;
    for (asset, dependencies) in &pack.assets {
        for dependency in dependencies {
            if !pack.assets.contains_key(dependency) {
                return Err(Refusal::MissingAsset(*dependency));
            }
        }
        let _ = asset;
    }
    acyclic(&pack.assets).map_err(Refusal::AssetCycle)?;
    Ok(())
}

fn main() {
    let valid = Pack {
        schema: 1,
        sources: BTreeSet::from([Id(90)]),
        beats: vec![Beat {
            id: Id(1),
            prerequisites: vec![],
            provenance_source: Some(Id(90)),
        }],
        assets: BTreeMap::from([(Id(20), vec![])]),
    };
    assert_eq!(validate(&valid), Ok(()));

    let mut unsupported = valid.clone();
    unsupported.schema = 2;
    assert_eq!(validate(&unsupported), Err(Refusal::UnsupportedSchema));

    let mut missing_source = valid.clone();
    missing_source.beats[0].provenance_source = Some(Id(91));
    assert_eq!(
        validate(&missing_source),
        Err(Refusal::MissingSource(Id(91)))
    );

    let mut cycle = valid.clone();
    cycle.beats.push(Beat {
        id: Id(2),
        prerequisites: vec![Id(1)],
        provenance_source: Some(Id(90)),
    });
    cycle.beats[0].prerequisites = vec![Id(2)];
    assert_eq!(validate(&cycle), Err(Refusal::BeatCycle(Id(1))));

    let mut dangling_asset = valid.clone();
    dangling_asset.assets.insert(Id(20), vec![Id(21)]);
    assert_eq!(
        validate(&dangling_asset),
        Err(Refusal::MissingAsset(Id(21)))
    );

    let mut asset_cycle = valid;
    asset_cycle.assets.insert(Id(20), vec![Id(21)]);
    asset_cycle.assets.insert(Id(21), vec![Id(20)]);
    assert_eq!(validate(&asset_cycle), Err(Refusal::AssetCycle(Id(20))));
}
```

## Source basis and open production gates

This decision applies and resolves the G10 portion of:
[Implementation roadmap](implementation-roadmap.md) (G10 prerequisite, existing
slices, runtime-director delivery, first-slice failure envelope, and release
gates); [Subsystem interfaces](subsystem-interfaces.md) (common contract, pure
domain/content, subsystem boundaries, storage and effects, feature candidates);
[Campaign authoring](campaign-authoring.md) (versioned model, validation,
publication/activation, and rights); [Runtime directors](runtime-directors.md)
(versioned state, explicit time, committed provenance, bounded state, and
acceptance); [Long-horizon state](long-horizon-state.md) (canonical facts,
pause/catch-up, and replay); [Campaign cinematics](campaign-cinematics.md)
(committed fact/media identity and audience scope); and [Generated content](generated-content.md)
(candidate-only mechanics and explicit admission). The implementation must also
follow [Coding style](coding-style.md), [AGENTS.md](../AGENTS.md), and ADR
[0001](../ADR/0001-sqlite-agent-workflow.md) through
[0005](../ADR/0005-frontier-output-evaluation.md).

This document freezes domain meaning and canonical pack identity. Before
implementation, the owners still need to land the concrete shared Rust types,
consumer codecs, approved source/catalog manifest and rights evidence, bounded
import numbers, migration fixtures, deterministic reducer fixtures, PostgreSQL
schema/access paths, operation/activation RPC or local command shape, and native
composition hooks. The example above does not prove any of those gates, nor does
this policy claim a built workspace, integrated behavior, publication, or replay
has been verified.
