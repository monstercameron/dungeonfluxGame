# G10-D02: immutable pack and provenance validation policy

Status: design decision for G10. Production schemas, byte codecs, storage,
publication, replay, and integration remain pending their existing owners.

This policy resolves B-G10-D02's graph/provenance validator and immutable pack
version boundary. It follows the existing G10-D01 private authoring boundary and
G10-D03 replay/logical-time policy. It adds no crate, public RPC, transport
schema, database schema, or second state writer.

## Ownership and boundary

Responsibilities remain at the existing subsystem boundaries:

* `df-types` owns shared IDs and units, including the eventual pack-version,
  source, event, decision, and logical-time value types. This policy does not
  select a concrete public Rust representation or freeze the logical-time unit.
* `df-model` owns persistent versioned state, snapshot, decision, and effect
  records. It does not own immutable authoring schemas or content validation.
* `df-content` owns versioned immutable authoring records and pure pack
  validation: schema compatibility, source/provenance, references, beat/asset
  graphs, alternatives, and bounded diagnostics. It performs no I/O.
* `df-engine` composes pure director/rules decisions and returns a candidate
  transition. It does not commit or publish state and has no storage, clock,
  provider, or network I/O.
* `df-session` authorizes and serializes accepted inputs, asks `df-engine` for a
  candidate, then owns the durable commit of the selected state change, decision,
  facts, operation result, and effect intents through its `df-persistence`
  adapter under the existing revision/fence. Publication, activation, effects,
  and visible state follow a successful commit.
* `df-persistence` stores private draft bytes/references and complete immutable
  validated pack bytes with metadata. Existing session/persistence owners retain
  authorized publication and activation. `df-tools` owns local import/author/
  package operations; transport and persistence codecs remain at their consumer
  boundaries.

This policy describes domain invariants, not new production type declarations.
G03/G05/G07/G10 remain responsible for shared schema, codecs, source catalogs,
rights evidence, persistence layout, and concrete owner APIs. No consumer may
create a competing identity or commit path.

## Immutable version identity

G10-D01 separates exact private upload bytes from parsed or validated content.
The uploaded bytes are a private draft input. Import and validation produce a
complete candidate; only the complete accepted immutable pack bytes can become a
published pack version. A raw upload digest is therefore a draft/retry identity,
not automatically the identity of the validated pack.

The frozen version invariant is byte identity: a published pack version names
one exact complete validated byte sequence, and that sequence is immutable for
the lifetime of every reference to it. Re-publishing identical bytes may resolve
to the same version. Any byte change requires a distinct version; it cannot
overwrite or alias the older bytes. A storage-issued opaque version handle is
acceptable if the existing persistence owner can establish this invariant and
detect a handle/digest collision by comparing the complete bytes. A hash alone
does not grant access. Authorization remains independently scoped.

The identity algorithm and canonical pack encoder are deliberately unresolved:
the project has not yet frozen the complete v1 record schema, field framing,
catalog manifest, or persistence codec. The selected policy is exact validated
bytes plus an opaque immutable version binding, not a claim that a particular
byte encoder or hash has been agreed. If persistence later uses a digest, it
must cover the exact accepted complete bytes without lossy normalization, and a
collision must fail safely instead of aliasing distinct content. Draft upload
hashes, filenames, transport encodings, and display labels cannot substitute for
the validated pack version. No consumer may invent an alternate encoder or
reinterpret old bytes.

For comparison, equal complete bytes denote the same content version; changed
bytes denote a new version, while the old version remains resolvable. Equality
of an opaque version handle is valid only when its owner guarantees that byte
identity. The literal example below exercises this comparison directly on
bounded byte slices; it does not pretend to implement the pending encoder,
cryptographic digest, or storage handle.

At activation, `df-session` commits references to the exact immutable version,
compatible schema/rules/catalog/source revisions, and initial state. Draft
publication and run activation are separate authorized operations with operation
identity, expected basis, and access scope. A changed live pack requires an
explicit reviewed migration mapping preserved identities, facts, beats, sources,
and pending work under the normal fenced commit. Unsupported migration is
rejected or starts a separate authorized run; it never retcons a committed fact.

## Validation contract

`df-content` validates a bounded candidate against supplied pinned catalog/source
facts and returns validated content or a finite list of safe typed diagnostics.
It reads no files, database, wall clock, network, providers, or telemetry. The
native authoring/import boundary owns byte, archive, nesting, and allocation
limits before constructing a candidate. Validation limits cover records by kind,
graph nodes and edges, references, string bytes, and diagnostic count. Exact
production numbers remain selected by G05/G10 from deployment and load evidence;
crossing a limit rejects the entire candidate and cannot truncate into success.

The following conditions are required for publication:

1. The pack schema, ruleset, catalog revision, and source manifest match an
   explicitly supported pinned set. Unknown pack schemas return an explicit
   unsupported-version outcome at validation. Replay/activation boundaries also
   reject unsupported reducer versions. Missing or incompatible source/catalog
   facts return `SourceGap` or `CatalogMismatch`; they never get a permissive
   default.
2. Stable IDs are unique in their declared kind and namespace. Every typed
   reference resolves to exactly one record of the required kind. Runtime
   instances do not stand in for authoring templates.
3. Beat prerequisite and disclosure graphs are closed and acyclic. Each
   alternative has an owning beat, explicit reachable entry, and terminal
   condition. The entry must be reachable from its owner through declared
   progression edges, and a terminal satisfying that alternative must be
   reachable from its entry. A required campaign start must reach at least one
   terminal path. Missing, mistyped, cyclic, unreachable, or unterminated paths
   reject the candidate. Conditions are typed data evaluated by the approved
   director; they are never scripts or executable rules.
4. Asset dependency graphs are closed and acyclic. Every asset and fallback
   reference resolves to a complete manifest entry in the pack or to an explicit
   version-pinned dependency with compatible access and use rights. Validation
   performs no external fetch. A hash or reference alone is not authorization.
5. Every canonical fact, rule-bearing template, disclosure, and imported claim
   has provenance linking to a pinned source-manifest entry and locator, or an
   authorized authored-origin record. Rights, permitted use, attribution, and
   access are explicit. Missing or incompatible provenance rejects the
   candidate. Extracted/generated lore remains a candidate until an authorized
   creator confirms canonical claims, secrets, and knowledge grants.
6. Uniqueness and contradiction rules, NPC knowledge and reveal conditions,
   localization, geometry, encounters, and usable fallback assets satisfy their
   pinned domain contracts. Unsupported mechanics return `UnsupportedRules`;
   imported prose never becomes executable behavior.

Diagnostics contain safe codes, typed IDs, and source locations, not private lore
or credentials. Failed validation yields no validated candidate, publication
handle, or activatable reference. Private drafts are not publicly discoverable;
partial imports cannot mutate an active run. Publication makes complete bytes
and metadata visible atomically through the existing persistence/session path.
Retries deduplicate by operation and payload, not filename. Stale authorization,
draft basis, or activation basis rejects without changing the current pack.

## Decision alternatives and unresolved facts

The chosen contract uses the exact complete validated bytes as the immutable
content identity, with an opaque version handle supplied by the existing durable
owner. This preserves D01's requirement that storage identity derive from the
accepted byte sequence, while avoiding a fabricated canonical encoder.

Alternatives considered:

* **Freeze a new canonical encoder and hash now:** this would give convenient
  cross-storage digest comparison, but the full record schema, field framing,
  source catalog, and persistence codec are still G03/G05/G07/G10 owner decisions.
  The prior DFCP/SHA-256 wording did not define those fields sufficiently for two
  implementations to emit the same bytes, so this is deferred until the owners
  can freeze fixtures with the actual schemas.
* **Use the uploaded draft bytes as the published version:** rejected because
  D01 makes upload intake opaque and private. Parsing, rights checks, validation,
  and packaging may change or reject content; a draft identity cannot claim that
  the result is an immutable validated pack.
* **Use a mutable latest-version counter or filename:** rejected because it does
  not identify content, permits accidental overwrite/aliasing, and cannot safely
  bind replay or lost-receipt lookup to the accepted payload.
* **Use the exact complete validated byte sequence as identity:** selected as the
  semantic contract now. Existing persistence may use an opaque handle or
  collision-checked digest internally, but only if equality and immutability
  correspond to those exact bytes. The concrete encoder/digest implementation
  remains assigned to the existing format/storage owners.

Still unresolved and assigned, not silently decided here: `df-types` owns
concrete shared IDs and the explicit logical-time unit; G03/G07 and `df-content`
owners freeze full records, allowed format, pinned source/catalog and rights
compatibility; `df-persistence`/G05 select byte storage, digest/handle mapping,
collision behavior, and atomic metadata publication; `df-session` owns authorized
publication/activation and durable commit; `df-engine` owns pure transition and
reducer behavior; G10 owners provide schema/migration/replay fixtures and select
finite production limits. No API, public wire schema, transaction schema, or
codec is invented by this policy.

## Logical time, provenance, and replay

This policy follows G10-D03. Logical/world time is an integer value with an
explicit unit owned by `df-types`; this document does not pick that unit. It
advances only in an authorized, source-valid candidate transition that
`df-session` commits durably. Paused or stopped campaigns do not derive game time
from wall time, uptime, reconnect, browser visibility, or presentation frames.
Presentation has a separate cosmetic monotonic anchor and cannot advance world
time, consume rules timers, or delay legal input. Resume makes committed due work
eligible for bounded catch-up ordered by logical due time and stable event
identity; remaining work stays explicit and resumable.

Persistent versioned snapshots, decisions, facts, and effect intents belong to
`df-model`. A committed decision retains, in order, its run/revision, exact pack
version and required schema/rules/catalog/source/policy/reducer versions,
decision/event identity, authorized input basis and source references, proposals
and accepted semantic outputs, actual ordered draws with stream identity,
resulting facts/effect intents, accepted logical time, and resulting state hash.
The `df-engine` reducer receives explicit recorded time and draw inputs and
returns a candidate; it does not generate missing history or persist its result.
`df-session` commits only the selected candidate and its provenance under the
current revision/fence; `df-persistence` implements its storage adapter.

Recorded-decision recovery reuses accepted semantic outputs, choices, draws, and
facts without calling providers or rerolling. Deterministic reconstruction is
claimed only when every required version/reducer/input is supported and the
computed state hash matches the retained hash. Migration is explicit, reviewed,
versioned, compatible with source/state/effect meaning, hash-checked, and committed
by `df-session` under its normal fence. Unsupported version, missing provenance,
missing reducer/input, rights-redacted payload, or hash mismatch yields an
explicit replay/migration gap at the affected checkpoint; partial reconstruction
is never published as success. These conditions and the distinction between
recorded-decision recovery and full reducer reconstruction match D03.

## Literal std-only contract example

This local example covers graph closure, provenance, alternative reachability,
asset-cycle refusal, and immutable-version comparison. Its types are confined to
the example and are not production APIs. It compares complete bytes directly;
it does not claim an encoder, hash algorithm, persistence handle, authorization
path, or production-sized resource bound has been frozen.

```rust
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Id(u8);

#[derive(Clone, Debug)]
struct Beat {
    id: Id,
    next: Vec<Id>,
    source: Option<Id>,
    terminal: bool,
}

#[derive(Clone, Copy, Debug)]
struct Alternative {
    owner: Id,
    entry: Id,
    terminal: Id,
}

#[derive(Clone, Debug)]
struct Asset {
    dependencies: Vec<Id>,
    source: Option<Id>,
}

#[derive(Clone, Debug)]
struct Pack {
    schema: u8,
    start: Id,
    sources: BTreeSet<Id>,
    beats: Vec<Beat>,
    alternatives: Vec<Alternative>,
    assets: BTreeMap<Id, Asset>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    UnsupportedSchema,
    DuplicateBeat(Id),
    MissingSource(Id),
    MissingBeat { from: Id, to: Id },
    BeatCycle(Id),
    MissingAlternativeNode(Id),
    UnreachableAlternative { owner: Id, entry: Id },
    UnreachableAlternativeTerminal { entry: Id, terminal: Id },
    NoCampaignTerminal,
    MissingAsset(Id),
    AssetCycle(Id),
}

fn reachable(from: Id, to: Id, edges: &BTreeMap<Id, Vec<Id>>) -> bool {
    let mut pending = vec![from];
    let mut seen = BTreeSet::new();
    while let Some(current) = pending.pop() {
        if current == to {
            return true;
        }
        if !seen.insert(current) {
            continue;
        }
        if let Some(next) = edges.get(&current) {
            pending.extend(next.iter().copied());
        }
    }
    false
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
        if let Some(source) = beat.source {
            if !pack.sources.contains(&source) {
                return Err(Refusal::MissingSource(source));
            }
        } else {
            return Err(Refusal::MissingSource(beat.id));
        }
    }
    if !beats.contains_key(&pack.start) {
        return Err(Refusal::MissingBeat {
            from: pack.start,
            to: pack.start,
        });
    }

    let mut beat_edges = BTreeMap::new();
    for beat in &pack.beats {
        for next in &beat.next {
            if !beats.contains_key(next) {
                return Err(Refusal::MissingBeat {
                    from: beat.id,
                    to: *next,
                });
            }
        }
        beat_edges.insert(beat.id, beat.next.clone());
    }
    acyclic(&beat_edges).map_err(Refusal::BeatCycle)?;
    if !beats
        .values()
        .any(|beat| beat.terminal && reachable(pack.start, beat.id, &beat_edges))
    {
        return Err(Refusal::NoCampaignTerminal);
    }
    for alternative in &pack.alternatives {
        let Some(owner) = beats.get(&alternative.owner) else {
            return Err(Refusal::MissingAlternativeNode(alternative.owner));
        };
        let Some(entry) = beats.get(&alternative.entry) else {
            return Err(Refusal::MissingAlternativeNode(alternative.entry));
        };
        let Some(terminal) = beats.get(&alternative.terminal) else {
            return Err(Refusal::MissingAlternativeNode(alternative.terminal));
        };
        if !reachable(owner.id, entry.id, &beat_edges) {
            return Err(Refusal::UnreachableAlternative {
                owner: owner.id,
                entry: entry.id,
            });
        }
        if !terminal.terminal || !reachable(entry.id, terminal.id, &beat_edges) {
            return Err(Refusal::UnreachableAlternativeTerminal {
                entry: entry.id,
                terminal: terminal.id,
            });
        }
    }

    for asset in pack.assets.values() {
        if let Some(source) = asset.source {
            if !pack.sources.contains(&source) {
                return Err(Refusal::MissingSource(source));
            }
        } else {
            return Err(Refusal::MissingSource(pack.start));
        }
    }
    let mut asset_edges = BTreeMap::new();
    for (id, asset) in &pack.assets {
        for dependency in &asset.dependencies {
            if !pack.assets.contains_key(dependency) {
                return Err(Refusal::MissingAsset(*dependency));
            }
        }
        asset_edges.insert(*id, asset.dependencies.clone());
    }
    acyclic(&asset_edges).map_err(Refusal::AssetCycle)?;
    Ok(())
}

fn same_immutable_version(existing: &[u8], candidate: &[u8]) -> bool {
    existing == candidate
}

fn main() {
    let beats = vec![
        Beat {
            id: Id(1),
            next: vec![Id(2), Id(3)],
            source: Some(Id(90)),
            terminal: false,
        },
        Beat {
            id: Id(2),
            next: vec![],
            source: Some(Id(90)),
            terminal: true,
        },
        Beat {
            id: Id(3),
            next: vec![],
            source: Some(Id(90)),
            terminal: true,
        },
    ];
    let valid = Pack {
        schema: 1,
        start: Id(1),
        sources: BTreeSet::from([Id(90)]),
        beats: beats.clone(),
        alternatives: vec![
            Alternative {
                owner: Id(1),
                entry: Id(2),
                terminal: Id(2),
            },
            Alternative {
                owner: Id(1),
                entry: Id(3),
                terminal: Id(3),
            },
        ],
        assets: BTreeMap::from([
            (
                Id(10),
                Asset {
                    dependencies: vec![Id(11)],
                    source: Some(Id(90)),
                },
            ),
            (
                Id(11),
                Asset {
                    dependencies: vec![],
                    source: Some(Id(90)),
                },
            ),
        ]),
    };
    assert_eq!(validate(&valid), Ok(()));

    let published_bytes = [0x44, 0x46, 0x01, 0x10];
    let identical_bytes = [0x44, 0x46, 0x01, 0x10];
    let changed_bytes = [0x44, 0x46, 0x01, 0x11];
    assert!(same_immutable_version(&published_bytes, &identical_bytes));
    assert!(!same_immutable_version(&published_bytes, &changed_bytes));

    let mut unsupported = valid.clone();
    unsupported.schema = 2;
    assert_eq!(validate(&unsupported), Err(Refusal::UnsupportedSchema));

    let mut missing_source = valid.clone();
    missing_source.beats[0].source = Some(Id(91));
    assert_eq!(
        validate(&missing_source),
        Err(Refusal::MissingSource(Id(91)))
    );

    let mut dangling_beat = valid.clone();
    dangling_beat.beats[0].next.push(Id(99));
    assert_eq!(
        validate(&dangling_beat),
        Err(Refusal::MissingBeat {
            from: Id(1),
            to: Id(99),
        }),
    );

    let mut cyclic_beats = valid.clone();
    cyclic_beats.beats[1].next.push(Id(1));
    assert_eq!(validate(&cyclic_beats), Err(Refusal::BeatCycle(Id(1))));

    let mut unreachable_alternative = valid.clone();
    unreachable_alternative.alternatives[0].entry = Id(3);
    unreachable_alternative.beats[0].next = vec![Id(2)];
    assert_eq!(
        validate(&unreachable_alternative),
        Err(Refusal::UnreachableAlternative {
            owner: Id(1),
            entry: Id(3),
        }),
    );

    let mut unreachable_alternative_terminal = valid.clone();
    unreachable_alternative_terminal.alternatives[0].terminal = Id(3);
    assert_eq!(
        validate(&unreachable_alternative_terminal),
        Err(Refusal::UnreachableAlternativeTerminal {
            entry: Id(2),
            terminal: Id(3),
        }),
    );

    let mut missing_alternative_owner = valid.clone();
    missing_alternative_owner.alternatives[0].owner = Id(99);
    assert_eq!(
        validate(&missing_alternative_owner),
        Err(Refusal::MissingAlternativeNode(Id(99))),
    );

    let mut duplicate_beat = valid.clone();
    duplicate_beat.beats.push(duplicate_beat.beats[0].clone());
    assert_eq!(
        validate(&duplicate_beat),
        Err(Refusal::DuplicateBeat(Id(1)))
    );

    let mut no_terminal = valid.clone();
    for beat in &mut no_terminal.beats {
        beat.terminal = false;
    }
    assert_eq!(validate(&no_terminal), Err(Refusal::NoCampaignTerminal));

    let mut missing_asset = valid.clone();
    missing_asset
        .assets
        .get_mut(&Id(10))
        .expect("fixture contains root asset")
        .dependencies
        .push(Id(12));
    assert_eq!(validate(&missing_asset), Err(Refusal::MissingAsset(Id(12))));

    let mut cyclic_assets = valid;
    cyclic_assets
        .assets
        .get_mut(&Id(11))
        .expect("fixture contains child asset")
        .dependencies
        .push(Id(10));
    assert_eq!(validate(&cyclic_assets), Err(Refusal::AssetCycle(Id(10))));
}
```

The example proves only its finite local boundary. It is not a production API,
canonical encoder, SHA implementation, database transaction, import/upload path,
or evidence of source rights, deployment limits, migration, or full replay.

## Source basis, compatibility, and remaining gates

This policy applies [Implementation roadmap](implementation-roadmap.md) (G10
ownership and release/dependency gates), [Subsystem interfaces](subsystem-interfaces.md)
(pure `df-types`/`df-model`/`df-content`/`df-engine` responsibilities and
`df-session`/persistence commit authority), [Campaign authoring](campaign-authoring.md)
(immutable package, alternatives, publication and activation), [Runtime directors](runtime-directors.md)
(shared IDs in `df-types`, content validation, pure proposals, session commit),
[Long-horizon state](long-horizon-state.md) (canonical facts and bounded time),
[Tempo engine](tempo-engine.md) (separate presentation and world-time anchors),
[Campaign cinematics](campaign-cinematics.md) (committed source/audience identity),
and [Generated content](generated-content.md) (candidate-only mechanics and
explicit opt-in).

This decision aligns with the current G10-D01 private-authoring proposal:
private upload bytes remain an opaque draft input; validated bytes are a distinct
downstream immutable content identity, with storage identity bound to the exact
accepted sequence. It also aligns with the current G10-D03 replay/logical-time
proposal: `df-types` owns the still-unselected logical-time unit, `df-model` stores
versioned records, `df-engine` returns pure candidate transitions, and
`df-session`/storage own fenced commit and replay migration. Those neighboring
proposals remain independently reviewed/integrated; this document does not claim
or make changes to them.

Before production implementation, the existing owners still need to freeze
shared Rust value types in their G03 wave, full schema and encoding fixtures,
source/catalog and rights manifests, concrete bounded import/graph limits,
storage/digest/opaque-handle and collision semantics, activation authorization
and operation shape, durable publication transaction, reducer/migration fixtures,
and native/WASM integration contracts. This design document does not claim these
gates or running application behavior are complete. It follows
[Coding style](coding-style.md), [AGENTS.md](../AGENTS.md), and ADR
[0001](../ADR/0001-sqlite-agent-workflow.md) through
[0005](../ADR/0005-frontier-output-evaluation.md); checks are recorded in the
attempt handoff and do not replace independent evaluation.
