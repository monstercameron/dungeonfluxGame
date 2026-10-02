# B-C-df-content-D02: Campaign graph and alternative disclosures

Status: design decision with a finite contract example; production schemas and
validation remain pending the named owners.

## Decision

`df-content` owns the immutable authored campaign graph and pure structural
validation. A graph contains stable beat IDs, explicit progression edges, one
start beat, terminal exits, referenced canonical facts, and alternative delivery
records. An edge means the authored progression can continue to that beat; it
does not mark a beat complete, reveal a fact, or force a scene. Runtime directors
evaluate the authored conditions against committed world and knowledge state.

Validation resolves every beat and fact reference, rejects duplicate IDs and
cycles, and checks that the start reaches at least one terminal exit. Each
alternative names an owner beat, an entry beat, and a terminal exit. The owner
must be reachable from the campaign start, the entry must be reachable from the
owner, and its named terminal exit must be reachable from the entry. A referenced
disclosure fact must exist in the pack. The validator returns a typed refusal and
no accepted graph on any failure.

An alternative's disclosure is either `AtOwner` or `AfterFact(fact_id)`. This
describes when the route itself may be presented. It does not expose the facts,
private scene contents, or a player's discovery to another audience. At delivery,
the existing session/knowledge path must reauthorize the actual recipient against
the committed knowledge grant. A referenced fact is a condition identifier, not
proof that a particular viewer knows it. The graph has no scripts or executable
conditions.

IDs and record shapes in the example are local fixture types, not frozen shared
`df-types` contracts. Production IDs, graph and diagnostic bounds, phase
predicates, provenance records, serialization, and disclosure audience inputs
remain for G03/G05/G07/G10 and the existing consumers. Validation is pure and
performs no storage, provider, network, clock, authorization, or telemetry I/O.

## Alternatives and rationale

* A linear ordered beat list was rejected because it cannot represent authored
  branches, substitutions, or multiple feasible exits.
* Treating a beat reference as proof of reachability was rejected: a well-formed
  ID can still point into a disconnected component. References and path
  feasibility are separate checks.
* Implicit or prose-inferred alternatives were rejected. Every route needs a
  stable owner, entry, terminal, and explicit disclosure timing so validation
  and later delivery can reason about the same authored choice.
* Revealing all alternatives or treating `AfterFact` as authorization was
  rejected. Content validation proves only structural closure; disclosure to an
  audience remains subject to current committed knowledge and access checks.
* Executable predicates in content were deferred. Conditions are typed
  references evaluated by the approved runtime owner; imported prose cannot
  mint rules or disclosure authority.

## Literal std-only contract example

This finite example checks a valid branching graph, a dangling beat reference, an
unreachable alternative exit, and refusal of an alternative whose disclosure
fact is absent. It does not implement production import, provenance/rights
qualification, runtime condition evaluation, audience authorization, or an
integrated campaign.

```rust
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Id(u8);

#[derive(Clone, Copy, Debug)]
struct Beat {
    id: Id,
    next: &'static [Id],
    terminal: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Disclosure {
    AtOwner,
    AfterFact(Id),
}

#[derive(Clone, Copy, Debug)]
struct Alternative {
    owner: Id,
    entry: Id,
    exit: Id,
    disclosure: Disclosure,
}

#[derive(Clone, Copy, Debug)]
struct Pack<'a> {
    schema: u8,
    start: Id,
    beats: &'a [Beat],
    facts: &'a [Id],
    alternatives: &'a [Alternative],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    UnsupportedSchema,
    DuplicateBeat(Id),
    MissingReference { from: Id, to: Id },
    MissingDisclosureFact(Id),
    BeatCycle(Id),
    UnreachableAlternativeOwner(Id),
    UnreachableAlternativeEntry { owner: Id, entry: Id },
    AlternativeExitNotTerminal(Id),
    UnreachableAlternativeExit { entry: Id, exit: Id },
    NoReachableExit,
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

fn validate(pack: &Pack<'_>) -> Result<(), Refusal> {
    if pack.schema != 1 {
        return Err(Refusal::UnsupportedSchema);
    }

    let mut beats = BTreeMap::new();
    for beat in pack.beats {
        if beats.insert(beat.id, beat).is_some() {
            return Err(Refusal::DuplicateBeat(beat.id));
        }
    }
    if !beats.contains_key(&pack.start) {
        return Err(Refusal::MissingReference {
            from: pack.start,
            to: pack.start,
        });
    }

    let mut edges = BTreeMap::new();
    for beat in pack.beats {
        for target in beat.next {
            if !beats.contains_key(target) {
                return Err(Refusal::MissingReference {
                    from: beat.id,
                    to: *target,
                });
            }
        }
        edges.insert(beat.id, beat.next.to_vec());
    }
    acyclic(&edges).map_err(Refusal::BeatCycle)?;

    let has_reachable_exit = pack
        .beats
        .iter()
        .any(|beat| beat.terminal && reachable(pack.start, beat.id, &edges));
    if !has_reachable_exit {
        return Err(Refusal::NoReachableExit);
    }

    let facts: BTreeSet<Id> = pack.facts.iter().copied().collect();
    for alternative in pack.alternatives {
        if let Disclosure::AfterFact(fact) = alternative.disclosure {
            if !facts.contains(&fact) {
                return Err(Refusal::MissingDisclosureFact(fact));
            }
        }
        if !beats.contains_key(&alternative.owner) {
            return Err(Refusal::MissingReference {
                from: alternative.owner,
                to: alternative.owner,
            });
        }
        if !beats.contains_key(&alternative.entry) {
            return Err(Refusal::MissingReference {
                from: alternative.owner,
                to: alternative.entry,
            });
        }
        let Some(exit) = beats.get(&alternative.exit) else {
            return Err(Refusal::MissingReference {
                from: alternative.entry,
                to: alternative.exit,
            });
        };
        if !reachable(pack.start, alternative.owner, &edges) {
            return Err(Refusal::UnreachableAlternativeOwner(alternative.owner));
        }
        if !reachable(alternative.owner, alternative.entry, &edges) {
            return Err(Refusal::UnreachableAlternativeEntry {
                owner: alternative.owner,
                entry: alternative.entry,
            });
        }
        if !exit.terminal {
            return Err(Refusal::AlternativeExitNotTerminal(alternative.exit));
        }
        if !reachable(alternative.entry, alternative.exit, &edges) {
            return Err(Refusal::UnreachableAlternativeExit {
                entry: alternative.entry,
                exit: alternative.exit,
            });
        }
    }
    Ok(())
}

fn main() {
    const BEATS: &[Beat] = &[
        Beat {
            id: Id(1),
            next: &[Id(2), Id(3)],
            terminal: false,
        },
        Beat {
            id: Id(2),
            next: &[Id(4)],
            terminal: false,
        },
        Beat {
            id: Id(3),
            next: &[Id(5)],
            terminal: false,
        },
        Beat {
            id: Id(4),
            next: &[],
            terminal: true,
        },
        Beat {
            id: Id(5),
            next: &[],
            terminal: true,
        },
    ];
    const ALTERNATIVES: &[Alternative] = &[
        Alternative {
            owner: Id(1),
            entry: Id(2),
            exit: Id(4),
            disclosure: Disclosure::AtOwner,
        },
        Alternative {
            owner: Id(1),
            entry: Id(3),
            exit: Id(5),
            disclosure: Disclosure::AfterFact(Id(7)),
        },
    ];
    let valid = Pack {
        schema: 1,
        start: Id(1),
        beats: BEATS,
        facts: &[Id(7)],
        alternatives: ALTERNATIVES,
    };
    assert_eq!(validate(&valid), Ok(()));

    let missing_reference_beats: &[Beat] = &[
        Beat {
            id: Id(1),
            next: &[Id(99)],
            terminal: false,
        },
        Beat {
            id: Id(4),
            next: &[],
            terminal: true,
        },
    ];
    let missing_reference = Pack {
        beats: missing_reference_beats,
        alternatives: &[],
        ..valid
    };
    assert_eq!(
        validate(&missing_reference),
        Err(Refusal::MissingReference {
            from: Id(1),
            to: Id(99),
        }),
    );

    let unreachable_exit_alternative = [Alternative {
        owner: Id(1),
        entry: Id(2),
        exit: Id(5),
        disclosure: Disclosure::AtOwner,
    }];
    let unreachable_exit = Pack {
        alternatives: &unreachable_exit_alternative,
        ..valid
    };
    assert_eq!(
        validate(&unreachable_exit),
        Err(Refusal::UnreachableAlternativeExit {
            entry: Id(2),
            exit: Id(5),
        }),
    );

    let missing_fact_alternative = [Alternative {
        owner: Id(1),
        entry: Id(3),
        exit: Id(5),
        disclosure: Disclosure::AfterFact(Id(8)),
    }];
    let missing_fact = Pack {
        alternatives: &missing_fact_alternative,
        ..valid
    };
    assert_eq!(
        validate(&missing_fact),
        Err(Refusal::MissingDisclosureFact(Id(8)))
    );
}
```

The graph example is not an implementation or a claim that a full campaign has
been imported, published, or run. The fixture uses finite static data and does
not establish production graph bounds, runtime feasibility under predicates,
provenance or rights validity, private-knowledge delivery, or integrated
behavior.

## Sources, next owner, and unresolved gates

This decision follows [Campaign authoring](campaign-authoring.md) (“Versioned
authoring model”, “Pure validation and scoped publication”), [Narrative engine](narrative-engine.md)
(“Authoring and state”, “Realization and acceptance”), [Subsystem architecture](subsystem-architecture.md)
(“Server crates”), [Subsystem interfaces](subsystem-interfaces.md) (“Pure domain
and content”), [Runtime directors](runtime-directors.md), [D01 ContentPack
publication identity](c-df-content-d01-decision.md), and [Coding style](coding-style.md).
`df-content` owns the structural validator; `df-narrative` evaluates runtime
conditions; `df-knowledge` and the session projection path authorize disclosure
to each audience. `df-tools` remains the import/author/package owner, while
existing session/persistence owners control publication and activation.

The next consumer should implement this graph boundary in `df-content` after
G03 freezes shared IDs and G05/G10 freeze schema and input bounds. It should
retain the distinction between structural reachability and runtime feasibility,
and must not treat disclosure timing as audience permission. Exact production
phase/condition semantics, source and rights manifests, bounded diagnostic
counts, codec, authoring format, and integrated campaign activation remain
unresolved. This decision creates no production catalog, campaign importer,
generated rules, RPC, persistence writer, or runtime behavior.

The bounded evidence for this decision is retained with attempt
`B-C-df-content-D02-a1`: source hash, extracted literal, formatter/compiler and
execution receipts, commit, and handoff manifest. Workspace Cargo, Clippy,
WASM, integration, import, rights-holder and frontier campaign checks remain
unperformed; this finite contract example does not satisfy those gates.
