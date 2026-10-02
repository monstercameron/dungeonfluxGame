# Geometry and legal-choice query authority

Task: `B-C-df-rules-D03`; attempt: `B-C-df-rules-D03-a1`.
Input revision: `d870db00dc349425e8f977bdfb20c6ffb24bfd3e`.
Status: bounded design submission; production types, source mechanics and integration remain gated.

## Original criteria and authority

The original objective is **Define geometry and legal-choice query ports**. Preserve both
original acceptance criteria:

- no client-side mechanics authority;
- The named outcome has actual source/build-bound evidence; unsupported, pending, failed and
  unperformed checks remain explicit.

The original verification calls for freezing the cited source decision and a bounded contract
example, comparing no client-side mechanics authority, and retaining the decision, alternatives
and unresolved facts. Its exact executable commands were TBD at G01 and prerequisite resolution;
that planning procedure did not claim Rust, browser or provider checks had run.

This decision follows [Subsystem interfaces: Pure domain and content](subsystem-interfaces.md),
[Subsystem architecture: Server and browser crates](subsystem-architecture.md),
[Rules support](rules-support.md), [Rules coverage: R06 and resolution](rules-coverage.md),
[Rules effects: source-linked testing](rules-effect-model.md),
[Interaction: intent and source-guided rulings](interaction-engine.md),
[Generated content: admission](generated-content.md), and
[Commercial validation: rights-complete launch](commercial-validation.md).
The brief retains their exact source hashes. AGENTS, coding style and ADRs 0001–0005 govern
isolated ownership, deterministic mechanics, evidence, devlog and independent evaluation.

## Decision and query ports

Keep the already planned `Ruleset::reachable(state, movement_query)` and
`Ruleset::legal_choices(state, actor)` in pure server `df-rules`. These are the query ports;
do not add a browser geometry authority, a geometry service, or a trait per mechanic. The rules
owner reads a consistent supplied authoritative `df-model` state and immutable `df-content`
geometry/catalog under the session's pinned `RulesetId`. Queries do not mutate state, draw dice,
spend resources, start providers, read clocks, or perform storage/network I/O.

| Port | Supplied server basis and question | Server result and limit |
| --- | --- | --- |
| `reachable` | Actor, current location, proposed destination/path, movement mode and the relevant state/content/source/timing basis | Source-valid reachable alternatives/path and exact source-defined cost, or typed refusal/gap; never client-declared collision, sight, cover, reach or cost as authority |
| `legal_choices` | Actor and current rules-relevant state, including resources, effects, pending window and geometry | Legal offers with retained rules/source and relevant-state basis, required typed inputs and safe explanations; unsupported choices are not usable standard offers |

The table defines ownership and semantics, not new Rust declarations or admitted protobuf fields.
Canonical `df-types`, `df-model`, `df-content`, rules D01/D02 and G03 owners freeze concrete IDs,
units, query/result/error types and bounded collection/path/byte limits before implementation.
No reviewed concrete shared types are dependencies in this attempt's brief. Neighboring drafts
are comparison material, not authority to copy or instantiate their types.

For R06, exact movement modes, distance/reach/range, terrain, space/occupancy, sight/light/cover,
hiding and opportunity timing come from pinned 2024 clauses and supported interactions.
This decision selects no grid metric, diagonal formula, distance unit representation, obstacle
shortcut, path budget, free movement or Dash-ending-turn rule. Source-specific exceptions and
effect precedence flow through the common rules owner; query results do not bypass reactions or
pending resolutions. Source-required discretion returns scoped `NeedsRuling` through the engine's
pending owner. Missing supported semantics returns an explicit source/unsupported gap, never
AI-generated geometry or a host-injected arbitrary mechanic.

`df-engine` consumes the query results for offers/proposals and revalidates a selected input at
prepare/resolve against the current relevant context. The client's observed revision is a basis,
not a global compare-and-swap lock: unrelated revision changes do not automatically invalidate
an offer. Changed relevant geometry, source/catalog, resources or timing must be rechecked.
`df-session` authenticates and binds the actor independently of correlation, looks up committed
operations first, serializes decision admission, and atomically commits before publication.
Querying and refusing a new selection leave state/resources/draws unchanged. Cancellation of a
query creates no committed result; cancellation/disconnect after commit cannot undo decisions.

`df-api`/projection selects the permitted information before serialization. Hidden occupancy,
creatures, reasons and paths cannot be shipped for the browser to hide; paired hidden states
must not leak through public offer diagnostics. `df-client` renders supplied alternatives,
geometry and previews and sends typed selections. `df-render` may interpolate a supplied path
but cannot establish movement cost, collision, visibility, legality or outcomes. Client gestures,
predicted path or claimed legality are proposals only. Pure query diagnostic facts pass to the
native caller's shared `df-observe` path; this design adds no telemetry SDK or ad hoc logger.

Standard 2024 support remains required. G07 must pin the exact mechanics/book/catalog/errata,
source locator/checksum and authorized use; SRD 5.2.1 is a candidate subset, not full catalog
qualification. Personal book/model access does not prove paid rights. New generated mechanics
require independently reviewed deterministic handlers and participant-disclosed distinct custom
ruleset admission; they never close missing standard R06 or R01–R17 coverage.

## Bounded executable authority example

The following complete private Rust literal exercises selection against a server-owned query
result and rejects stale relevant geometry or an unoffered choice. Two source-linked query
answers are supplied as synthetic fixtures; this code does not calculate D&D movement, choose
source clauses, admit public types, authorize actors, or prove catalog completeness. The client
can select a displayed offer but cannot supply a server query result or legality flag. A repeated
current query produces the same answer without changing state. The native example's stdout is
finite contract evidence, not application telemetry.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RelevantBasis {
    geometry_revision: u64,
    rules_source_revision: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ChoiceKey(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ServerQuery {
    basis: RelevantBasis,
    reachable_choice: ChoiceKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ClientSelection {
    observed_basis: RelevantBasis,
    choice: ChoiceKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SelectionRefusal {
    ChangedRelevantBasis,
    ChoiceNotOffered,
}

fn validate_selection(
    current: &ServerQuery,
    selection: ClientSelection,
) -> Result<ChoiceKey, SelectionRefusal> {
    if selection.observed_basis != current.basis {
        return Err(SelectionRefusal::ChangedRelevantBasis);
    }
    if selection.choice != current.reachable_choice {
        return Err(SelectionRefusal::ChoiceNotOffered);
    }
    Ok(selection.choice)
}

fn main() {
    let current = ServerQuery {
        basis: RelevantBasis {
            geometry_revision: 7,
            rules_source_revision: 4,
        },
        reachable_choice: ChoiceKey(11),
    };
    let initial = current;
    let valid = ClientSelection {
        observed_basis: current.basis,
        choice: ChoiceKey(11),
    };
    assert_eq!(validate_selection(&current, valid), Ok(ChoiceKey(11)));
    assert_eq!(validate_selection(&current, valid), Ok(ChoiceKey(11)));
    println!("valid: server-offered choice selected; query state unchanged");

    let unoffered = ClientSelection {
        choice: ChoiceKey(99),
        ..valid
    };
    assert_eq!(
        validate_selection(&current, unoffered),
        Err(SelectionRefusal::ChoiceNotOffered)
    );
    println!("refusal: client choice cannot establish reachability or legality");

    let changed_geometry = ServerQuery {
        basis: RelevantBasis {
            geometry_revision: 8,
            ..current.basis
        },
        ..current
    };
    assert_eq!(
        validate_selection(&changed_geometry, valid),
        Err(SelectionRefusal::ChangedRelevantBasis)
    );
    let changed_source = ServerQuery {
        basis: RelevantBasis {
            rules_source_revision: 5,
            ..current.basis
        },
        ..current
    };
    assert_eq!(
        validate_selection(&changed_source, valid),
        Err(SelectionRefusal::ChangedRelevantBasis)
    );
    assert_eq!(current, initial);
    println!("refusal: changed relevant geometry or source requires server revalidation");
}
```

The example's basis contains only synthetic geometry/source revisions to keep this demonstration
minimal. Production relevant-basis validation must also include actor, run, effects/resources,
content and pending timing as applicable; the example is not a reusable public offer contract.
No unit, numeric revision or choice key in it expresses a standard mechanic.

## Alternatives, next consumer and remaining gates

Rejected alternatives: browser-computed legal paths or offers (mechanical authority and hidden
state leakage); geometry-specific parallel authority (diverges from rules exceptions); arbitrary
client outcome/effect submission (bypasses prepare/resolve); executable imported scripts or AI
rulings (unapproved mechanics); invalidating every observed revision (incorrectly rejects
unrelated changes). Keep server-owned pure queries and relevant-context revalidation instead.

The concrete next consumer is `B-C-df-rules-I03`, **Implement legal option and submit agreement**,
whose original expected outcome is **same rules request validates both paths**. I03 must use
the canonical query and prepare/resolve source dispatch, not copy this fixture as its own rules
implementation. I04's rule-state precondition validation consumes the same current basis.
Following G01/G03/G07 admission, minimum intended source areas are the planned canonical rules crate's
`crates/df-rules/src/lib.rs` boundary, private movement/space/visibility implementation, and a
rules contract fixture; precise module names and permitted files are frozen in that later brief.
Engine/session offer handling and API/client projection are separately owned integration hooks,
not files authorized for this task. No production source tree or wire schema is created here.

Checks for this submission are extraction of this exact Rust literal with byte comparison,
pinned Rust 1.98.1 rustfmt write/check using root configuration, pinned rustc `--edition=2024
-Dwarnings`, and finite executable valid/refusal output under the frozen v6 guard. Commands,
source/tool/config hashes, receipts and results belong to the retained attempt handoff. Compiler
and proof execution were held until explicit root release; the wait was not a failed attempt. Independent
frontier review and coordinator integration remain separate acceptance gates.

Unperformed production checks remain explicit: Cargo/Clippy/native and WASM crate builds,
source-clause movement/visibility/cover/cost fixtures, bounded path search, complete relevant-basis
validation, standard catalog/rights qualification, actor authorization, pending reactions/rulings,
offer/submit integration, PostgreSQL/replay, paired hidden-state projection, browser render/input
flow and actual OTEL ingestion. This authority example cannot establish those results. Required
support and the original acceptance stay open until their applicable evidence is reviewed.

Worker observation on 2026-10-02: exact-literal extraction/byte comparison, pinned root-config
rustfmt write/check, Rust 2024 rustc with warnings denied, and the finite valid/refusal execution
passed after group02 release. The literal needed no formatting or compiler repair. Retained
receipts and outputs are under `development/evidence/fanout-20261001/wave-03/B-C-df-rules-D03/a1`;
the final handoff binds source, build, tools, configuration and exact argv. This is standalone
authority-boundary evidence; independent review and integrated production acceptance remain open.
