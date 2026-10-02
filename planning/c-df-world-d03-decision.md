# Spatial affordance facts and provenance

Task: B-C-df-world-D03 / B-C-df-world-D03-a1. Input revision:
`d870db00dc349425e8f977bdfb20c6ffb24bfd3e`. Status: bounded design decision;
production contracts and independent review remain pending.

The original criteria remain: **physical suggestions do not become unsourced
rules**; **the named outcome has actual source/build-bound evidence; unsupported,
pending, failed and unperformed checks remain explicit**.

## Decision and authority

`df-world::affordances` returns bounded candidate facts from the supplied immutable
world/content/geometry basis. An affordance describes an authored or committed
physical condition and its provenance. It does not establish action legality,
damage, difficulty, resource cost, a roll, movement reachability or an outcome.
For example, an authored object fact that a curtain is flammable can be retained
as a candidate; an inferred claim that every wooden object ignites cannot become
that fact. Missing source support yields `NeedsRuling`, retained through the
engine's pending rules path, rather than a guessed mechanical result.

This follows [Runtime directors](runtime-directors.md), Authority and composition,
Pure public boundaries, and Time, recovery and bounded work; [Subsystem
interfaces](subsystem-interfaces.md), Common contract rules and Separate runtime
subsystem boundaries; [Subsystem architecture](subsystem-architecture.md), Pure
domain and content; and [Long-horizon state](long-horizon-state.md), World
granularity and pause. Content authoring belongs to `df-content`; persistence
shapes belong to `df-model`; revision/identity primitives belong to `df-types`.

The engine revalidates the session/run, current revision, entity/scene identity,
geometry, source/content/policy versions and causal input before composing the
candidate with `df-rules`. `df-rules` retains source legality and adjudication.
`df-session` alone commits the selected transition, operation result, ordered
facts and effects with the PostgreSQL owner fence/revision before apply or
publication. A rejected decision publishes no candidate changes. Pure world
queries perform no clock, dice, DB, provider, timer, socket or SDK I/O. They cannot
advance campaign time. Clients receive only the authorized projection of committed
layers, never raw private provenance or a candidate presented as a rule.

## Minimum fact requirements and refusals

Each eligible fact must bind the queried entity and exact committed scene/geometry
basis; identify the physical predicate and its immutable authored record or
committed source fact; retain the source/content/policy revisions and any explicit
supersession. Source references must resolve against supplied validated content or
committed facts, not merely contain a syntactically valid label. A resolved record
must actually support this entity/predicate; contradictory or absent support is a
visible ruling gap. Neither generated text, a material guess, image pixels,
telemetry, a summary nor a confidence score supplies that support.

Validate basis first, then input bounds, then source support, with deterministic
source order and stable deduplication in the eventual model. Refuse changed
revision/geometry as `StaleBasis`, unresolved content as `UnknownContent`, and
excess work/output as `Capacity`; preserve `NeedsRuling` for a well-formed but
unsourced physical claim. An empty eligible set is a successful no-change result.
The fixed one-item fixture below demonstrates the minimum admission boundary,
not a selected production capacity. G03/G10/G11 must freeze actual shared fact,
scene identity, source-reference and bounds contracts before implementation.

## Exact executable boundary example

The only Rust literal is a private, finite design witness. It imports existing
canonical revision/provenance primitives by path; it introduces no public types
or production API. Its authored fixture record identifies the entity and physical
predicate, and admission checks those fields before retaining the source. Label
construction alone is never such a resolver. The fixture checks candidate-only
retention, an unsourced flammability ruling gap and two stale bases; it cannot
qualify a rules catalog or prove standard-2024 mechanics.

```rust
#[path = "../../../crates/df-types/src/provenance.rs"]
mod provenance;
#[path = "../../../crates/df-types/src/revision.rs"]
mod revision;

use provenance::{BuildIdentity, BuildRevision, RevisionLabel};
use revision::{RecoveryEpoch, SessionRevision};

#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    StaleBasis,
    NeedsRuling,
}

#[derive(Debug, PartialEq, Eq)]
struct PhysicalCandidate<'a> {
    entity: &'a str,
    source: &'a RevisionLabel,
    geometry: &'a RevisionLabel,
    revision: SessionRevision,
}

struct AuthoredFact<'a> {
    entity: &'a str,
    flammable: bool,
    source: &'a RevisionLabel,
}

fn retain_flammability<'a>(
    current: SessionRevision,
    current_geometry: &RevisionLabel,
    supplied: SessionRevision,
    supplied_geometry: &'a RevisionLabel,
    entity: &'a str,
    authored_fact: Option<&AuthoredFact<'a>>,
) -> Result<PhysicalCandidate<'a>, Refusal> {
    if current != supplied || current_geometry != supplied_geometry {
        return Err(Refusal::StaleBasis);
    }
    let fact = authored_fact.ok_or(Refusal::NeedsRuling)?;
    if fact.entity != entity || !fact.flammable {
        return Err(Refusal::NeedsRuling);
    }
    Ok(PhysicalCandidate {
        entity,
        source: fact.source,
        geometry: supplied_geometry,
        revision: supplied,
    })
}

fn main() {
    let epoch = RecoveryEpoch::new(7).unwrap();
    let current = SessionRevision::new(epoch, 3);
    assert_eq!(current.epoch().get(), 7);
    assert_eq!(current.sequence(), 3);
    let newer = current.next_sequence().unwrap();
    let geometry = RevisionLabel::new(Some("geometry:3")).unwrap();
    let old_geometry = RevisionLabel::new(Some("geometry:2")).unwrap();
    let authored = RevisionLabel::new(Some("fixture:curtain-flammable")).unwrap();
    let fact = AuthoredFact {
        entity: "curtain",
        flammable: true,
        source: &authored,
    };
    let build = BuildIdentity::new(
        Some("fixture-source"),
        Some("fixture-native"),
        Some("unperformed-wasm"),
        Some("fixture-config"),
        Some("fixture-content"),
    )
    .unwrap();
    for component in [
        BuildRevision::Source,
        BuildRevision::Native,
        BuildRevision::Wasm,
        BuildRevision::Configuration,
        BuildRevision::Content,
    ] {
        assert!(!build.revision(component).as_str().is_empty());
    }
    let valid = retain_flammability(current, &geometry, current, &geometry, "curtain", Some(&fact));
    assert_eq!(
        valid,
        Ok(PhysicalCandidate {
            entity: "curtain",
            source: &authored,
            geometry: &geometry,
            revision: current,
        })
    );
    assert_eq!(
        retain_flammability(current, &geometry, current, &geometry, "curtain", None),
        Err(Refusal::NeedsRuling)
    );
    assert_eq!(
        retain_flammability(current, &geometry, current, &geometry, "door", Some(&fact)),
        Err(Refusal::NeedsRuling)
    );
    assert_eq!(
        retain_flammability(newer, &geometry, current, &geometry, "curtain", Some(&fact)),
        Err(Refusal::StaleBasis)
    );
    assert_eq!(
        retain_flammability(current, &geometry, current, &old_geometry, "curtain", Some(&fact)),
        Err(Refusal::StaleBasis)
    );
    assert_eq!(current.sequence(), 3);
    println!("PASS: candidate-only authored fact; NeedsRuling; stale revision and geometry");
}
```

The valid result has no legality, damage, difficulty or transition field. The
immutable input remains unchanged after all calls. The absence of source support
does not become `false`, automatic immunity or ignition. A subsequent authorized
source-linked ruling can resolve the pending action only through the existing
engine/session path; this witness does not perform that ruling.

## Alternatives and next consumer

Reject a material/physics heuristic that grants mechanics: it invents support.
Reject a separate spatial rules engine: it duplicates `df-rules` authority.
Reject discarding every physical fact: source-grounded candidates are useful
inputs even though the rules result remains separately owned.

The next actual source consumer is **B-C-df-world-I03**, environmental state delta
validation (source facts and geometry checked), with **B-C-df-world-F03/F04** as
the unsourced-flammability and stale-source boundary checks. Minimum future source
areas are `crates/df-world/src/lib.rs` and its private affordance validation module,
the canonical `df-model` fact/basis records and `df-content` source resolver once
their G03 contracts are admitted. These crates/records are not present or admitted
at this input revision; this task does not create them. **B-C-df-world-W01** then
connects the candidate to engine stages and rules validation before commit;
**W02** connects committed scene identity to permitted presentation. Their owners
must implement and review those connections before any integrated feature claim.

## Verification and explicit gaps

Extract this exact Rust block into
`artifacts/tmp/B-C-df-world-D03-a1/example.rs` in this worktree so the relative
canonical imports resolve; compare the extracted bytes with the Markdown literal.
After the coordinator's finite-check release, use the pinned 1.98.1 `rustfmt`
write/check with the root configuration, pinned `rustc --edition=2024 -Dwarnings`
and run the resulting finite executable through the frozen v6 guard. Retain
actual argv, source/config/tool/binary hashes and outputs in the attempt evidence;
the handoff records failures and unperformed checks. No Cargo, native/WASM crate,
PostgreSQL, rules catalog, engine integration, authorized projection or player
experience is qualified by this witness. Independent frontier boundary review and
integrated consumer checks remain required.
