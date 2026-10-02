# G11-D01: Pure director ordering policy

Status: design contract; production interfaces and executable game behavior remain pending

## Decision

A decision pass is a finite, one-way composition of pure ports. `df-session` owns
its lifecycle and the only authoritative commit. It binds a trusted actor and
session/run, checks the operation key and owner fence, and supplies an immutable
basis containing the current revision, logical time, rules/content/policy
revisions, and causal input IDs. The pure director ports do not authenticate,
read clocks, access storage, call providers, schedule jobs, or mutate shared state.

The pass orders *authority*, not an obligation to run every director. A stage may
be inapplicable and produce an explicit no-op. A domain decline is a typed result,
not an infrastructure error. Pending, invalid, stale, capacity-limited, and
unsupported outcomes stop dependent stages. They do not become success through
fallback parsing or generated narration.

| Order | Port / owner | Input and permitted result | Stop or defer rule |
| --- | --- | --- | --- |
| 0 | Session admission (`df-session`) | Authorized actor, operation/idempotency key, fenced current revision, immutable basis | Reject unauthorized/stale binding, stale revision, or capacity before director work. A duplicate operation key returns its recorded result; it never dispatches the decision twice. Authorization and fencing are not director policy. |
| 1 | Intent (`df-intent`) | Validated input envelope and basis -> action/question/social/plan-only/meta/joke/clarify/rejected disposition; an action proposal is never authorization | Questions, hypotheticals, jokes, ambiguity, unsupported input, or stale semantic results cannot execute. Clarification or rejection is terminal for this pass. |
| 2 | Rules authority (`df-rules`, existing owner) | Typed action proposal -> source-valid preparation/resolution, explicit decline, or pending ruling | Rules own legality, costs, dice, modifiers, and mechanical outcome. A required ruling/choice pauses the uncommitted continuation; no director or model invents a DC, roll, cost, or success. |
| 3 | Consequence proposals (`df-world` and `df-interaction`) | The same immutable admitted basis plus the accepted rules result/permitted event -> bounded world and social proposals | These ports are siblings: neither calls the other or consumes its tentative output. Contradictory proposals require explicit session conflict handling or a new pass; never recursively iterate to convergence. Unsupported physical behavior returns a ruling/gap. NPC refusal, redirection, help, or no reaction is a valid result. |
| 4 | Knowledge (`df-knowledge`) | Accepted candidate facts, actual permitted observations/witnesses, and provenance -> scoped knowledge/memory/belief/rumor delta | Truth changes only through an authorized world transition. Knowledge is not broadcast from global state; no witness/contact path means no fabricated awareness. Secret access is audience-scoped before any projection. |
| 5 | Narrative (`df-narrative`) | Candidate opportunities from the accepted candidate plus permitted interaction/knowledge inputs -> bounded proposal or no intervention | Opportunity is not an outcome. Validate prerequisites, agency, and budget; a refusal is preserved. A proposal that needs a further reaction, ruling, or world choice becomes an explicit continuation for a later pass. |
| 6 | Encounter (`df-encounter`) | Current candidate world and source/rules catalog -> legal bounded challenge/objective proposal or explicit gap | No enemies, rewards, or objective results enter state before rules/session acceptance. Invalid or unsupported compositions stop that proposal. |
| 7 | Combat tactics (`df-combat`) | Current combat observation, perception-limited knowledge, and rules-provided legal action set -> one legal tactical/reinforcement proposal or no action | Rules keep initiative, reactions, resources, and action legality. Hidden information cannot influence what an actor appears to know. The proposal returns to the rules/session authority for acceptance. |
| 8 | Session decision/commit (`df-session`) | Selected validated candidate, operation result, ordered facts, and effect intents -> one fenced revision transition | Commit the selected decision, resulting state, result/idempotency record, ordered facts, and durable effect intents atomically. If validation or commit fails, publish no candidate state. An ambiguous commit remains unknown until operation lookup resolves it. |

The dependency graph is acyclic:

```text
admission -> intent -> rules result
                         |       |
                         +-> world proposal
                         +-> interaction proposal
                                  |
             accepted candidate + provenance -> knowledge delta
                                  |
                    narrative / encounter proposals
                                  |
                  combat proposal (when applicable)
                                  |
                    session validation + commit
```

World and interaction are concurrent-capable siblings only because they receive
the same immutable inputs. Their merge is a bounded, explicit validation step;
there is no implicit world <-> interaction feedback edge. Knowledge consumes only
observed or selected candidate facts, never arbitrary hidden state. Narrative and
encounter proposals do not call back into earlier stages. Combat may be evaluated
from the current legal-action set in the same decision, but any requested ruling,
new reaction, or newly created encounter waits for another admitted pass. Every
continuation carries its basis/revision and stable causal IDs and is revalidated;
stale continuations are rejected rather than silently rebased.

Only the session owner decides which validated proposals comprise the selected
candidate. On a whole-decision rejection, no candidate delta is visible. A
compound action may deliberately accept a bounded prefix: each accepted step's
outcome and cost is recorded, then the operation returns `PartiallyCompleted`,
`NeedsRuling`, `NeedsClarification`, `Cancelled`, or another explicit terminal /
continuation result. Already committed steps are not rolled back because a later
step fails or the request is cancelled. Request cancellation cannot undo a commit
or admitted durable effect. Generation/job/run bindings fence late callbacks;
post-commit effects execute through the owning server/session ports, outside pure
policy functions.

Logical world time advances only through an accepted rules/session decision under
the world pause/travel policy. Wall time, process wakeups, and presentation time do
not independently advance game time. Due work is bounded and deterministically
ordered; remaining catch-up is explicit. No director owns its own timer, database
transaction, task, or authoritative singleton. Diagnostic facts are returned to
native consumers; pure domain code does not emit SDK telemetry. Trace IDs do not
authorize a principal, and private payloads stay out of default diagnostics.

## Alternatives considered

- **Direct director calls / mutual feedback:** rejected because narrative-to-NPC-to-world-to-narrative recursion has no finite bound and creates crate cycles. Cross-stage intent is returned as typed data and, when it needs earlier authority again, as a later continuation.
- **One monolithic director owning rules and state:** rejected because it would duplicate rules authority and combine policy, persistence, and effect execution. The existing rules and session owners retain these responsibilities.
- **Commit each director independently or publish a partially merged working copy:** rejected because a later rejection could expose inconsistent facts, costs, and effects. The session commits one selected candidate atomically; intentional partial progress is represented as an accepted bounded prefix with an explicit result.
- **Treat every director as mandatory on every input:** rejected because questions, no-op turns, and non-combat actions have different applicable ports. Applicability is explicit and does not weaken the order of any port that does run.

## Executable contract example

The following is a literal, standalone Rust 2024 specification example using only
the standard library. Its names are local to the example and do not claim to be
production API. It demonstrates the fixed order, stopping on a typed refusal or
pending result, and rejecting a stale basis before any director is visited.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stage {
    Intent,
    Rules,
    World,
    Interaction,
    Knowledge,
    Narrative,
    Encounter,
    Combat,
    SessionCommit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PortResult {
    Accept,
    Decline,
    Pending,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Decision {
    Committed(Vec<Stage>),
    Refused { at: Stage, visited: Vec<Stage> },
    Awaiting { at: Stage, visited: Vec<Stage> },
    StaleBasis,
}

const ORDER: [Stage; 9] = [
    Stage::Intent,
    Stage::Rules,
    Stage::World,
    Stage::Interaction,
    Stage::Knowledge,
    Stage::Narrative,
    Stage::Encounter,
    Stage::Combat,
    Stage::SessionCommit,
];

fn decide(basis_is_current: bool, results: [PortResult; 9]) -> Decision {
    if !basis_is_current {
        return Decision::StaleBasis;
    }

    let mut visited = Vec::new();
    for (stage, result) in ORDER.into_iter().zip(results) {
        visited.push(stage);
        match result {
            PortResult::Accept => {}
            PortResult::Decline => return Decision::Refused { at: stage, visited },
            PortResult::Pending => return Decision::Awaiting { at: stage, visited },
        }
    }
    Decision::Committed(visited)
}

fn main() {
    let all_accept = [PortResult::Accept; 9];
    assert_eq!(
        decide(true, all_accept),
        Decision::Committed(ORDER.to_vec())
    );

    let mut refuses_at_rules = all_accept;
    refuses_at_rules[1] = PortResult::Decline;
    assert_eq!(
        decide(true, refuses_at_rules),
        Decision::Refused {
            at: Stage::Rules,
            visited: vec![Stage::Intent, Stage::Rules],
        },
    );

    let mut waits_at_interaction = all_accept;
    waits_at_interaction[3] = PortResult::Pending;
    assert_eq!(
        decide(true, waits_at_interaction),
        Decision::Awaiting {
            at: Stage::Interaction,
            visited: vec![
                Stage::Intent,
                Stage::Rules,
                Stage::World,
                Stage::Interaction
            ],
        },
    );

    assert_eq!(decide(false, all_accept), Decision::StaleBasis);
}
```

The example models control flow only. It deliberately omits domain types,
source validation, concurrency, candidate merging, transaction implementation,
serialization, telemetry, and effects. In production, refusal/pending classification
is typed per port, optional stages have explicit applicability/no-op results, and
the session owner—not a director—performs the actual atomic commit. The example
is not evidence of a compiled application or an integrated running feature.

## Unresolved production gates and acceptance evidence

This policy freezes the ordering decision and port ownership only. It does not
supply production signatures, stable shared IDs/enums, serialization, schemas,
content graph validation, source catalog coverage, calibrated bounds, transaction
implementation, executors, or a running integration. G01/G03/G05/G06/G07/G09/G10/
G11/G12 and the named prerequisite children remain applicable at their planned
boundaries; this task does not mark them complete. In particular, source-grounded
2024 social/physical rulings and concrete actor/event/plan, memory/rumor, simulation,
latency, and candidate-count limits need their owning tasks and evidence.

Acceptance for this design boundary is review that the table and example preserve
the frozen ordering, no feedback cycle, typed refusal/pending behavior, candidate
isolation, and sole session commit ownership. The exact extracted code passed
pinned repository `rustfmt --check`, `rustc --edition 2024 -D warnings`, and
execution of its finite boundary assertions. Guard receipts and captured stdout/
stderr are retained under
`development/evidence/fanout-20261001/B-G11-D01/`: `rustfmt-check-03.json`,
`rustc-contract-02.json`, and `run-contract-01.json`. Formatter and compiler
resource admission was recorded by the guard. Earlier formatting and path typos
are retained as separate failed receipts; they were corrected before the passing
checks. Native/WASM workspace checks, production contract checks, and integrated
runtime/frontier output checks are unperformed and cannot be inferred from this
example.

## Governing sources

The frozen input brief is `development/evidence/fanout-20261001/B-G11-D01/brief.json`
(task `B-G11-D01`, attempt `B-G11-D01-a1`; input revision
`9def9b845531e5cc589a28343b822b4664bd03fd`). Governing files were checked against
the brief's SHA-256 values before editing:

- `planning/implementation-roadmap.md` (`0160ad8e8ec38f768e2348209b9989e30e8f403d9b1a4ebf694f0801f7206932`): dispatch/ownership,
  G11 prerequisites, S03/S04/S05 director delivery, failure envelope and release gates.
- `planning/subsystem-interfaces.md` (`f26e1dca42e878f8a816c9f9aa37463cb8f061224598214ceee62632a161907a`): pure ports, authority,
  bounded owned results, session composition and persistence/effect separation.
- `planning/long-horizon-state.md` (`977b5a346190a55c119242fdb7009a645e6d76aa2946d0684c46571d846e96ff`): provenance, scoped
  memory/retrieval, bounded rumor propagation and logical-time constraints.
- `planning/generated-content.md` (`0b252a24a8911108660f3276d8359d9df825a3398c7c7a4033c62e2279007ac0`): candidate validation,
  opt-in ruleset separation, privacy and refusal behavior.
- `planning/interaction-engine.md` (`1bad7c28609ac5e5a51a3ea0b3c1fae17ebeb6380dbdb8fce582085a8ede354f`): source authority,
  NPC agency, intent/plan outcomes, private expression and ruling semantics.
- `planning/runtime-directors.md` (`ed8944f316d912498107ee550431e560795ba8114cb63a181b6aadeca70b48ca`): director ownership,
  one-way composition, candidate working copy, atomic session commit and bounds.
- `planning/tempo-engine.md` (`e227dda990685f5771b577529652c44d59704a23db8ad206e3e59be41f4c81d6`): world/presentation time
  separation and downstream presentation ownership.
- `planning/coding-style.md` (`2d8e327e4172643544bd80b591f38226c25b83940a10f1d9e18ada9044faeabb`), `rustfmt.toml`
  (`7a142cbd3f5e9c09c6dc4a1850e053415530eb5450d27983dc26c75230b8c5ba`), and `rust-toolchain.toml`
  (`9500030ccefd0bab631fb7f1763f79f4103eca3a344c36cf15be869e330683bb`): example style and deferred verification environment.
- `development/backlog-catalog.json` (`039b0a03b4085b43ad32c4063e2cb8fc789fc55fff7552aec6e951ec3b4704c3`): G11 / B-G11-D01
  original objective and criteria, unchanged.
- `AGENTS.md` (`55578ed92c477dfe3c306db38c6ad20390bfe8208ca998e6f3bf6f6bebec182`) and ADR 0001–0005 (`acfe32a8d5e846aa4d3b2981529533b9f53cdcc11088424128e60c20b722adc8`,
  `a6476cfef449e089639109cc6d3cf5f0800b792b57a32696258d5ac0b951196`,
  `ee293673b01391c3a39577bd116a60abb3edfa662a7a8d1315556d6fe537d912`,
  `8f03d02d478086fd1f50d7aa10e8e7e766e3b15ae02f7f96efa43757c98a4dae`,
  `25ab35a57ee8516a272b1ff3d04bba4def91319255d89158c9be55283ca6c35c`): scoped ownership, isolated worktree, evidence, devlog,
  resource, verification, and independent-review procedure.

The design does not claim that matching source hashes, a document review, or the
contract example proves a production implementation. The roadmap's dependent
crate owners retain their named integration hooks; no duplicate implementation
or task is introduced here.
