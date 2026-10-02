# C-df-engine-D01: staged director composition

Date: 2026-10-01  
Status: source-backed composition decision; concrete production types and integrated behavior remain pending

## Decision

`df-engine` owns the pure use-case composition and candidate `Transition`; `df-rules` is the sole mechanics authority; each director owns only its own bounded policy proposal; `df-model` owns shared proposal/state/fact/effect shapes; `df-session` owns serialization, durable commit and publication. A causal order is a contract for consuming values and ordering decisions. It does not imply that one director imports or calls another. Directors exchange typed `df-model` values through engine orchestration and remain separately importable according to the crate map.

For an input, the engine first validates its authorized current basis and resolves any required rules choices/reactions/rulings. Rules preparation can supply the legal, provisional mechanical outcome directors need as context. The engine then invokes applicable pure directors in the causal stages in `planning/runtime-directors.md`: world and interaction consequences; knowledge; narrative/experience/encounter recommendations; tempo; and authorized presentation. Within a stage, only actual causal dependencies constrain order; otherwise use stable causal IDs and pinned policy revisions, never incidental hash-map or crate-import order. Directors do not recursively call each other. Their outputs and any tentative mechanical outcome stay in an owned candidate working copy.

Before returning a transition, `df-engine` sends the complete combined candidate through `df-rules` validation. A proposal that asks for a new choice, reaction, roll, ruling, or unsupported interpretation returns a typed pending/gap outcome with stable resolution identity and the legal next input; it is not partly treated as completed. An explicitly accepted pending transition may record that suspension, but applies no unvalidated downstream mechanics. Any failure, stale basis, refusal, unsupported rule, or capacity limit discards the entire uncommitted candidate. There is one candidate transition for the input, applicable at its expected revision exactly once. `df-engine::decide` returns it without publishing or performing I/O. `df-session` rechecks the current owner fence/revision, atomically commits state, operation result, ordered decision/fact records and required effect intents, then applies/publishes and dispatches committed effects. An ambiguous commit is resolved by operation lookup/reload, never by blind re-application.

This reading satisfies both the subsystem contract (rules preparation/resolution precedes consequence proposals) and the named outcome (pure proposals are gathered before final rules validation and one accepted transition). “Rules validation” here means validation of the combined candidate before acceptance; proposal generation cannot itself grant mechanical legality. A `NeedsChoice`/`NeedsReaction`/`NeedsRuling` from the initial rules step stops dependent proposal work until the required resolution is supplied. Logical time, dice and content/policy versions are explicit inputs. Replay records chosen proposals, actual ordered draws, causal IDs and pinned versions; replay does not rerun providers or silently choose new outcomes.

## Ownership and integration

The owning production boundary is `df-engine`; it composes `df-rules` plus `df-world`, `df-knowledge`, `df-intent`, `df-interaction`, `df-narrative`, `df-encounter`, `df-combat`, `df-experience`, `df-tempo`, and `df-presentation`, exchanging common values through `df-model`. `df-content` supplies validated immutable content and versioned policies. These ten pure director crates do not become engine submodules or each other's callers. `df-session` alone owns authoritative state mutation, fencing, durable decisions/effects and commit-before-publication. `df-server` wires native executors; `df-persistence` implements the repository boundary. Pure engine/rules/director code has no database, clock, provider, socket, telemetry SDK, or task/timer ownership.

This is a policy decision, not permission to implement missing G03 contracts. Exact `Transition`, rules outcome, proposal, pending-resolution, replay and effect-intent types must reuse reviewed canonical contracts and be frozen with their consumers. The example below is a finite boundary model, not a production API or runtime/game evidence.

## Alternatives considered

- Having directors call each other would encode causal policy in Rust imports, create cycles and make ordering accidental. Rejected: engine-mediated typed proposals preserve the stated independent crate boundaries.
- Letting directors mutate shared state or letting `df-engine` commit would create a second authority and break the session owner fence. Rejected: engine owns only a candidate; session commits.
- Accepting each director result immediately would expose partial consequences if a later director or rules check refuses. Rejected: keep the entire candidate private until combined rules validation succeeds, then return one transition.
- Running every stage as an unconstrained parallel batch would lose causal inputs and deterministic order. A fixed serial chain for every director would overstate dependencies. Rejected: stage order is fixed where specified; within a stage, enforce only declared causal edges and deterministically order independent IDs.
- Treating every rules request as a generic failure would lose legitimate pending play; treating it as success would bypass required choices. Rejected: preserve typed pending identity and legal next input, with no dependent partial effects.

## Bounded executable contract illustration

This std-only example exercises the named boundary: an owned pure proposal is gathered, the full candidate is mechanically validated, and only a single candidate `Transition` is returned. Stale basis, a required rules choice, illegal combined effect, and incomplete proposal all refuse without yielding a transition. It does not model async work, persistence, production authorization, or actual DungeonFlux rules.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct State {
    revision: u64,
    score: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Proposal {
    causal_id: u8,
    score_delta: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Transition {
    expected_revision: u64,
    next: State,
    causal_id: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    StaleBasis,
    NeedsChoice(u8),
    Unsupported,
    IllegalCombinedCandidate,
    IncompleteProposal,
    RevisionOverflow,
}

fn decide(
    state: State,
    expected_revision: u64,
    choice_resolved: bool,
    proposal: Option<Proposal>,
) -> Result<Transition, Refusal> {
    if state.revision != expected_revision {
        return Err(Refusal::StaleBasis);
    }
    if !choice_resolved {
        return Err(Refusal::NeedsChoice(7));
    }
    let proposal = proposal.ok_or(Refusal::IncompleteProposal)?;
    if proposal.causal_id == 0 {
        return Err(Refusal::Unsupported);
    }
    let next_score = state
        .score
        .checked_add(proposal.score_delta)
        .ok_or(Refusal::IllegalCombinedCandidate)?;
    if next_score > 10 {
        return Err(Refusal::IllegalCombinedCandidate);
    }
    let next_revision = state
        .revision
        .checked_add(1)
        .ok_or(Refusal::RevisionOverflow)?;
    Ok(Transition {
        expected_revision,
        next: State {
            revision: next_revision,
            score: next_score,
        },
        causal_id: proposal.causal_id,
    })
}

fn main() {
    let state = State {
        revision: 4,
        score: 6,
    };
    let proposal = Some(Proposal {
        causal_id: 9,
        score_delta: 2,
    });
    let accepted = decide(state, 4, true, proposal).unwrap();
    assert_eq!(accepted.expected_revision, 4);
    assert_eq!(
        accepted.next,
        State {
            revision: 5,
            score: 8
        }
    );
    assert_eq!(accepted.causal_id, 9);

    assert_eq!(decide(state, 3, true, proposal), Err(Refusal::StaleBasis));
    assert_eq!(
        decide(state, 4, false, proposal),
        Err(Refusal::NeedsChoice(7))
    );
    assert_eq!(
        decide(
            state,
            4,
            true,
            Some(Proposal {
                causal_id: 0,
                score_delta: 1
            })
        ),
        Err(Refusal::Unsupported),
    );
    assert_eq!(
        decide(
            state,
            4,
            true,
            Some(Proposal {
                causal_id: 9,
                score_delta: 5
            })
        ),
        Err(Refusal::IllegalCombinedCandidate),
    );
    assert_eq!(
        decide(state, 4, true, None),
        Err(Refusal::IncompleteProposal)
    );
    assert_eq!(
        decide(
            State {
                revision: u64::MAX,
                score: 1
            },
            u64::MAX,
            true,
            Some(Proposal {
                causal_id: 9,
                score_delta: 1
            })
        ),
        Err(Refusal::RevisionOverflow),
    );
}
```

## Evidence and unresolved production gates

Applicable source at this attempt's input revision:

- `planning/subsystem-architecture.md` (df-engine/rules/session owners and dependency boundaries; `df-engine` row and pure-code authority).
- `planning/subsystem-interfaces.md` (common contract rules; pure-domain table; pending rules outcomes; `Transition` validity; director/session ownership; actor commit path).
- `planning/runtime-directors.md` (authority/composition, pure operation contracts, bounds, deterministic ordering, pending outcomes and replay).
- `planning/runtime-reliability.md` (director composition/candidate safeguards and session commit path).
- `planning/storage-architecture.md` (one fenced decision commit and replay limits).
- `planning/generated-content.md` (candidate-only policy and current-basis revalidation before publication).
- `planning/shared-contract-waves.md` (later production contract wave and no inferred full G03 readiness).
- `planning/coding-style.md`, `rustfmt.toml`, and `rust-toolchain.toml` (bounded typed contracts and required example checks).

The issued hashes of all governing/workflow files, frozen brief, original task and this document are retained in this task's handoff manifest. The literal is checked independently with pinned Rust 1.98.1, edition 2024, repository rustfmt settings and `rustc -D warnings`; receipts name the exact extracted literal, commands, results and hashes. This standalone check proves only that the illustration compiles and its finite assertions pass. No Cargo/workspace, Clippy, WASM, browser, session/persistence integration, running game, device, audio or provider check was performed for this documentation-only task.

Still open for the owning later contract/implementation waves: exact shared proposal and `Transition` fields/errors; the full directed partial order and tie-break encoding for all applicable director outputs; rules prepare-versus-final validation shape for choices/reactions/rulings; numeric output/byte/work budgets selected from evidence; production deterministic time/dice inputs; authorization and content/policy freshness checks; async admission and cancellation fencing; operation idempotency, durable commit/recovery/migration/replay; all real director registrations/callers; native and WASM workspace checks; independent frontier review and integrated behavior. No arbitrary illustrative bound in the example is a measured gameplay limit.
