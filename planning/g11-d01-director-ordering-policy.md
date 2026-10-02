# G11-D01: Pure director ordering policy

Status: design contract; production interfaces and executable game behavior remain pending

## Decision

A decision pass is a finite, one-way composition of pure ports. The `df-session`
owner admits and serializes an operation per session: it authenticates the actor,
resolves an idempotent operation key, verifies the current owner fence, and
supplies an immutable basis
with session/run, current revision, logical time, source/content/policy revisions,
and causal input IDs. It passes that basis and typed input to `df-engine`.

`df-engine` is the pure composition owner. It invokes `df-intent`, the existing
`df-rules` authority, and the applicable G11 director ports in the order below.
It validates their bounded results, combines only compatible deltas, and selects
an isolated candidate `Transition` assembled from canonical `df-model` records
and `df-types` IDs/units. The engine owns its merge/selection policy; a
contradictory or non-commuting proposal
without a governing resolution is a typed conflict/pending result; one delta
never silently overwrites another. A whole-decision refusal exposes no candidate.
The engine does not authenticate, read a clock, access storage, call providers,
schedule work, or mutate shared state.

Shared IDs and units belong to `df-types`; shared persistence-bearing records
belong to `df-model`; immutable authored policies/content belong to `df-content`.
Subsystem-local request and error types remain with their owning pure crate. This
policy names those ownership boundaries but does not freeze new concrete records,
fields, enum variants, or function signatures: those remain G03/G10/G11 work.

After composition, `df-session` rechecks the admitted revision/fence and owns
authorization of the one durable commit through its `SessionRepository` port;
`df-persistence` implements that port. State transition, operation result,
ordered facts, and durable effect intents commit atomically. Only after the
persistence result confirms commit may session apply/publish the transition and
`df-server` supplied native executors dispatch committed effects. An ambiguous
commit stays unknown until operation lookup resolves it. No effect runs before
commit, and a post-commit delivery failure does not roll back gameplay state.

The pass orders *authority*, not an obligation to run every director. A stage may
be inapplicable and produce an explicit no-op. A domain decline is a typed result,
not an infrastructure error. Pending, invalid, stale, capacity-limited, and
unsupported outcomes stop dependent stages. They do not become success through
fallback parsing or generated narration.

| Causal order | Pure port / authority | Input and permitted result | Stop or defer rule |
| --- | --- | --- | --- |
| Admission | Native session (`df-session`) | Authorized actor; per-session serialized input; operation key, owner fence, immutable basis and typed input | A repeated key returns its recorded result without re-dispatch. Reject an unauthorized/stale binding, invalid fence, stale basis, or capacity before composition. |
| 1 | Intent (`df-intent`), invoked by `df-engine` | Input envelope and basis -> action/question/social/plan-only/meta/joke/clarify/rejected disposition | An action proposal is not authorization. Questions, hypotheticals, jokes, ambiguity, unsupported input, or stale semantic results cannot execute. |
| 2 | Rules (`df-rules`), invoked by `df-engine` | Typed action proposal -> source-valid preparation/resolution, explicit decline, or pending ruling | Rules own legality, costs, dice, modifiers, and mechanical outcome. Required ruling/choice pauses the uncommitted pass; no director/model invents a DC, roll, cost, or success. |
| 3a / 3b | World (`df-world`) and interaction (`df-interaction`) sibling ports, invoked by `df-engine` | Same immutable basis plus accepted rules result/permitted event -> bounded world and social proposals | Neither sibling calls or consumes the other's tentative result. Unsupported physical behavior returns a ruling/gap. NPC refusal, redirection, help, or no reaction is valid. |
| 4 | Knowledge (`df-knowledge`), invoked by `df-engine` | Actual permitted observations, provenance, and facts eligible in the candidate -> scoped knowledge/memory/belief/rumor delta | Truth changes only through an authorized world transition. No global-state broadcast or fabricated witness/contact path. Keep secret access audience-scoped. |
| 5 | Narrative (`df-narrative`), invoked by `df-engine` | Candidate opportunities and permitted interaction/knowledge inputs -> bounded proposal or no intervention | An opportunity is not an outcome. Check prerequisites, agency and budget. A request needing another earlier-authority choice is a later continuation, never a recursive call. |
| 6 | Encounter (`df-encounter`), invoked by `df-engine` | Candidate world and source/rules catalog -> bounded challenge/objective proposal or explicit gap | No enemies, rewards, or objective results enter the candidate before applicable rules/engine validation. |
| 7 | Combat (`df-combat`), invoked by `df-engine` | Combat observation, perception-limited knowledge, and rules-provided legal action set -> legal tactical/reinforcement proposal or no action | Rules keep initiative, reactions, resources and legality. A tactic is a proposal returned to rules/engine validation. |
| Selection | Pure engine composition (`df-engine`) | Applicable typed proposals -> compatible selected candidate `Transition` or typed conflict/refusal/pending | The engine owns merge and candidate selection. Reject contradictions that have no approved deterministic resolution. It returns owned data and never publishes it. |
| Commit | Session (`df-session`) through `SessionRepository` (`df-persistence` implementation) | Engine candidate plus operation result, ordered facts, effects, and current fence/revision -> durable decision receipt | Revalidate current owner fence/revision and atomically persist. Failure publishes nothing; ambiguous outcome remains unknown pending operation lookup. |
| Post-commit | Native composition (`df-server` executors supplied to session) | Confirmed committed effect intents -> owned job/result | Dispatch only after commit. Generation/job/run fences reject stale callbacks; provider failure cannot undo required committed outcomes. |

### Causal flow

The arrows below describe data/decision flow, not Rust crate imports:

```text
df-session admits actor/operation and binds immutable basis
  -> df-engine pure composition
       -> df-intent classification -> df-rules preparation/resolution
       -> df-world proposal ---------+
       -> df-interaction proposal ---+  same basis; neither calls the other
       -> df-knowledge -> df-narrative / df-encounter -> df-combat (when applicable)
  -> df-engine validates deltas and selects candidate Transition
  -> df-session rechecks fence/revision
  -> SessionRepository commit (implemented by df-persistence)
  -> confirmed commit -> apply/publish -> df-server native effect executors
```

### Rust crate ownership/import direction

These are ownership/import constraints, not a promise that every concrete crate
API or dependency edge is already implemented. `A -> B` means A may import B;
there is no reverse edge in this composition:

```text
df-model -> df-types
df-content -> df-types
df-rules, df-intent, df-world, df-interaction, df-knowledge,
df-narrative, df-encounter, df-combat -> df-types + df-model + df-content

df-engine -> df-types + df-model + df-content + df-rules + applicable pure directors
df-session imports df-types + df-model + df-engine; it declares SessionRepository
df-persistence imports df-types + df-model + the df-session repository contract
df-server imports df-session + df-persistence + native executor implementations
```

Shared IDs/units and persisted records are not redeclared by director crates.
Director crates do not import one another or `df-engine`; `df-engine` imports and
composes their pure ports. `df-session` does not import the persistence
implementation: it owns the repository contract, and the native composition root
wires the implementation. This keeps the application dependency graph acyclic
while leaving storage and native execution out of the pure engine.

World and interaction can be evaluated as siblings only because both consume the
same immutable basis. `df-engine` validates/combines compatible deltas; conflict
resolution is not delegated to the session. Knowledge consumes only observations
and provenance permitted for the selected candidate, never arbitrary hidden state.
Narrative/encounter proposals do not call back into earlier stages. Combat may be
evaluated from the current legal-action set in the same pass, but a required
ruling, new reaction, or newly created encounter waits for a later admitted pass.
Every continuation carries its basis/revision and causal IDs and is revalidated;
stale continuations are rejected instead of silently rebased.

Only a confirmed session commit makes selected candidate changes authoritative.
On a whole-decision rejection, no candidate delta is visible. A compound action
may deliberately return a bounded accepted prefix: committed steps and costs are
preserved, then the result explicitly reports `PartiallyCompleted`,
`NeedsRuling`, `NeedsClarification`, `Cancelled`, or another typed terminal /
continuation outcome. A later failure or request cancellation cannot roll back
already committed steps. Logical world time advances only through an accepted
rules/session decision under world pause/travel policy; wall time and presentation
time do not advance it. Due work is bounded and deterministically ordered. No
pure director owns a timer, task, transaction, provider call, or authoritative
mutable singleton. Pure code returns facts; diagnostics and private payloads follow
the existing observability/privacy boundaries.

## Alternatives considered

- **Direct director calls / mutual feedback:** rejected because narrative-to-NPC-to-world-to-narrative recursion has no finite bound and creates crate cycles. Cross-stage intent is returned as typed data and, when it needs earlier authority again, as a later continuation.
- **One monolithic director owning rules and state:** rejected because it would duplicate rules authority and combine policy, persistence, and effect execution. The existing rules and session owners retain these responsibilities.
- **Commit each director independently or publish a partially merged working copy:** rejected because a later rejection could expose inconsistent facts, costs, and effects. `df-engine` selects one isolated candidate; session authorizes and commits it atomically through persistence. Intentional partial progress is represented as an accepted bounded prefix with an explicit result.
- **Treat every director as mandatory on every input:** rejected because questions, no-op turns, and non-combat actions have different applicable ports. Applicability is explicit and does not weaken the order of any port that does run.

## Executable contract example

This literal, standalone Rust 2024 example uses only the standard library. Its
names are local specification labels, not production API. It distinguishes
session admission, director calls, engine candidate selection, a separate session
fence recheck and durable persistence result, then post-commit native effects. It also asserts that refusal,
pending work, candidate conflict, stale fence, and uncertain commit stop before
any forbidden downstream action.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stage {
    SessionAdmission,
    Intent,
    Rules,
    World,
    Interaction,
    Knowledge,
    Narrative,
    Encounter,
    Combat,
    EngineSelectCandidate,
    SessionFenceRecheck,
    PersistenceCommit,
    NativeEffectDispatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PortResult {
    Accept,
    Decline,
    Pending,
    Conflict,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Decision {
    Completed(Vec<Stage>),
    Refused { at: Stage, visited: Vec<Stage> },
    Awaiting { at: Stage, visited: Vec<Stage> },
    Conflicted { visited: Vec<Stage> },
    StaleFence { visited: Vec<Stage> },
    CommitUnknown { visited: Vec<Stage> },
    EffectsPending { visited: Vec<Stage> },
}

const ORDER: [Stage; 13] = [
    Stage::SessionAdmission,
    Stage::Intent,
    Stage::Rules,
    Stage::World,
    Stage::Interaction,
    Stage::Knowledge,
    Stage::Narrative,
    Stage::Encounter,
    Stage::Combat,
    Stage::EngineSelectCandidate,
    Stage::SessionFenceRecheck,
    Stage::PersistenceCommit,
    Stage::NativeEffectDispatch,
];

fn decide(fence_is_current: bool, results: [PortResult; 13]) -> Decision {
    if !fence_is_current {
        return Decision::StaleFence {
            visited: Vec::new(),
        };
    }

    let mut visited = Vec::new();
    for (stage, result) in ORDER.into_iter().zip(results) {
        visited.push(stage);
        match result {
            PortResult::Accept => {}
            PortResult::Decline if stage == Stage::SessionFenceRecheck => {
                return Decision::StaleFence { visited };
            }
            PortResult::Decline => return Decision::Refused { at: stage, visited },
            PortResult::Pending => return Decision::Awaiting { at: stage, visited },
            PortResult::Conflict if stage == Stage::EngineSelectCandidate => {
                return Decision::Conflicted { visited };
            }
            PortResult::Conflict => return Decision::Refused { at: stage, visited },
            PortResult::Failed if stage == Stage::PersistenceCommit => {
                return Decision::CommitUnknown { visited };
            }
            PortResult::Failed if stage == Stage::NativeEffectDispatch => {
                return Decision::EffectsPending { visited };
            }
            PortResult::Failed => return Decision::Refused { at: stage, visited },
        }
    }
    Decision::Completed(visited)
}

fn main() {
    let all_accept = [PortResult::Accept; 13];
    assert_eq!(
        decide(true, all_accept),
        Decision::Completed(ORDER.to_vec())
    );

    let mut refuses_at_rules = all_accept;
    refuses_at_rules[2] = PortResult::Decline;
    assert_eq!(
        decide(true, refuses_at_rules),
        Decision::Refused {
            at: Stage::Rules,
            visited: ORDER[..3].to_vec(),
        },
    );

    let mut waits_at_interaction = all_accept;
    waits_at_interaction[4] = PortResult::Pending;
    assert_eq!(
        decide(true, waits_at_interaction),
        Decision::Awaiting {
            at: Stage::Interaction,
            visited: ORDER[..5].to_vec(),
        },
    );

    let mut sibling_conflict = all_accept;
    sibling_conflict[9] = PortResult::Conflict;
    assert_eq!(
        decide(true, sibling_conflict),
        Decision::Conflicted {
            visited: ORDER[..10].to_vec(),
        },
    );

    let mut stale_fence = all_accept;
    stale_fence[10] = PortResult::Decline;
    assert_eq!(
        decide(true, stale_fence),
        Decision::StaleFence {
            visited: ORDER[..11].to_vec(),
        },
    );

    let mut uncertain_commit = all_accept;
    uncertain_commit[11] = PortResult::Failed;
    assert_eq!(
        decide(true, uncertain_commit),
        Decision::CommitUnknown {
            visited: ORDER[..12].to_vec(),
        },
    );

    assert_eq!(
        decide(false, all_accept),
        Decision::StaleFence {
            visited: Vec::new()
        },
    );

    let mut executor_failure = all_accept;
    executor_failure[12] = PortResult::Failed;
    assert_eq!(
        decide(true, executor_failure),
        Decision::EffectsPending {
            visited: ORDER.to_vec(),
        },
    );
}
```

The example specifies control-flow ownership; it does not emulate actual candidate
merging or database transactions. Optional director applicability, owned domain
records, canonical IDs/units, and exact port signatures await G03/G10/G11. The
example makes the invariant observable: only engine selection precedes the
session commit gate, and native effect execution follows it. The session remains
the only durable commit authority; a candidate conflict cannot reach commit, an
uncertain commit cannot dispatch effects, and post-commit executor failure cannot
undo the committed decision.

## Unresolved production gates and acceptance evidence

This policy freezes the ordering decision and port ownership only. It does not
supply production signatures, stable shared IDs/enums, serialization, schemas,
content graph validation, source catalog coverage, calibrated bounds, transaction
implementation, executors, or a running integration. G01/G03/G05/G06/G07/G09/G10/
G11/G12 and the named prerequisite children remain applicable at their planned
boundaries; this task does not mark them complete. In particular, source-grounded
2024 social/physical rulings and concrete actor/event/plan, memory/rumor, simulation,
latency, and candidate-count limits need their owning tasks and evidence. Exact
shared-record shapes/fields, port signatures, merge-compatible fields, and
repository contract methods are not frozen by this design excerpt.

Acceptance for this design boundary is review that the table and example preserve
the frozen ordering, no feedback cycle, typed refusal/pending behavior, candidate
isolation, engine-owned candidate selection, and sole session-authorized durable
commit ownership. The exact a2 extracted code passed pinned repository
`rustfmt --check`, `rustc --edition 2024 -D warnings`, and execution of its finite
ownership/refusal assertions. Guard receipts and captured stdout/stderr are
retained under `development/evidence/fanout-20261001/B-G11-D01/a2/`. Earlier a1
receipts remain separate and are not a2 evidence. Native/WASM workspace checks,
production contract checks, and integrated runtime/frontier output checks are
unperformed and cannot be inferred from this example.

## Governing sources

The frozen input brief is `development/evidence/fanout-20261001/B-G11-D01/a2/brief.json`
(task `B-G11-D01`, attempt `B-G11-D01-a2`; input revision
`a0575c15cdf5171c1a90d4aa36a385317747660b`). Governing files were checked against
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
- `AGENTS.md` (`55578ed92c477dfe3c306db38c6ad20390bfe8208ca998e6f3bf6f6bebbec182`) and ADR 0001–0005 (`acfe32a8d5e846aa4d3b2981529533b9f53cdcc11088424128e60c20b722adc8`,
  `a6476cfef449e089639109cc6d3cf5f0800b792b57a32696258d5ac0b9511966`,
  `ee293673b01391c3a39577bd116a60abb3edfa662a7a8d1315556d6fe537d912`,
  `8f03d02d478086fd1f50d7aa10e8e7e766e3b15ae02f7f96efa43757c98a4dae`,
  `25ab35a57ee8516a272b1ff3d04bba4def91319255d89158c9be55283ca6c35c`): scoped ownership, isolated worktree, evidence, devlog,
  resource, verification, and independent-review procedure.

The design does not claim that matching source hashes, a document review, or the
contract example proves a production implementation. The roadmap's dependent
crate owners retain their named integration hooks; no duplicate implementation
or task is introduced here.
