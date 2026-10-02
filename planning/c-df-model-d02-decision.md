# C-df-model-D02: closed input and effect ownership

Date: 2026-10-02
Status: bounded source-backed decision; production types and runtime wiring remain gated
Input revision: `4c0e050c2d30e632bd3b32c51a92cfda52479e6e`
Task/attempt: `B-C-df-model-D02` / `B-C-df-model-D02-a1`

## Decision and governing source

`df-model` owns closed typed `GameInput` and native `Effect` intent families. `df-engine`
produces candidates; `df-session` owns admission, serialized state, durable effect intents,
callback validation and lifetime; `df-server` owns exactly one native registration for each
admitted effect kind. Registration is exhaustive, typed, and checked before readiness.
Unknown kinds cannot be interpreted as arbitrary strings, imported scripts, or success.
Missing, duplicate, incompatible-owner or incompatible-terminal registrations refuse readiness.
No unmatched effect may be silently dropped or routed to a default executor.

This follows `planning/subsystem-architecture.md` (Server crates and Integration and refinement),
`planning/subsystem-interfaces.md` (Common contract rules, Pure domain and content, Identity and
session ownership), and `planning/runtime-reliability.md` (Authority, Asynchronous work and timers).
`planning/rules-effect-model.md` (Effects, conditions and timing) distinguishes the source-grounded
mechanical `EffectDefinition`/`ActiveEffect` from these native execution intents. Mechanical effects
are resolved by `df-rules` and staged by `df-engine`, rather than native arbitrary game mutations.
The D03 pending-resolution decision owns choices, ordered actual draws/resources and trigger
windows; D01 owns checkpoint identity. Neither is redefined here.

The minimum closed input families are authenticated game commands, authorized host commands,
validated native job completions, timer expiry, and authorized presentation reports. Choice,
roll and reaction submissions remain typed game commands referencing the pending resolution;
`HostCommand::ResolveRuling` is the source-grounded authorized host continuation, not internal
injection. A client adapter cannot create native job completion or timer-expiry inputs. A
presentation report is evidence of delivery/playback/failure, never a rules outcome or state
mutation. `df-api` maps and authorizes client DTOs before owner admission; `df-session` constructs
internal inputs only after verifying their admitted job/timer and current binding.

The following seven kinds form the complete denominator of the bounded executable example.
Each has the same explicit composition registrar (`df-server`) and durable lifetime owner
(`df-session`); the executor column names the canonical downstream responsibility, not a second
state owner. These private example enums are comparison inputs for the engine D03 registration
decision. They are not admitted shared production types, a full rules catalog, or protobuf.

| Closed effect kind | Executor responsibility / native boundary | Terminal input family |
| --- | --- | --- |
| `RunAi` | `df-ai::AiService`, supplied through the session executor port | `AiFinished` job completion |
| `RunMedia` | `df-media::MediaService`, supplied through the session executor port | `MediaFinished` job completion |
| `LoadMemoryCandidates` | `df-session::MemoryCandidateStore::load`; `df-persistence` adapter | `MemoryCandidatesReady` job completion |
| `ArmTimer` | injected `df-session::Clock`, owned timer registration | `TimerExpired` |
| `CancelJob` | `df-session` owned-job executor cancellation port; downstream admitted service | `JobCancelled` job completion |
| `CancelTimer` | injected `df-session::Clock`, exact owned timer instance | `TimerCancelled` job completion |
| `PublishPresentation` | `df-api` current authorized audience projection/publication | `PresentationReported` |

Terminal names identify a closed result family: actual payloads must retain typed success,
failure, deadline, cancellation, stale and unavailable outcomes; requesting cancellation does
not imply it succeeded. Native publication and later browser playback are separate facts. A
missing client report cannot withhold or reverse committed mechanics; report expiry/failure is
explicit in the later delivery contract. `PublishPresentation` carries permitted plans, not
private state or executable client rules. Required fallback or rights/access refusals remain
owned by media/API/session consumers.

`planning/long-horizon-state.md` (Retrieval and contradiction semantics) fixes the memory
store and completion owners above. Derived index update/rebuild and consolidation are separately
gated jobs with the same native session/server ownership: the persistence adapter owns canonical
indexed reads/publication; AI/media/provider work uses separately admitted qualified service
ports. They cannot be smuggled into `LoadMemoryCandidates` or run during replay. New nested job
operations or effect kinds need their own reviewed typed payload, executor, terminal outcome,
source/access bindings, and the same exhaustive registration checks before admission. The seven
fixture kinds do not claim complete long-horizon, book/catalog or production feature coverage.
`planning/campaign-cinematics.md` (Recaps, Critical-event escalation, Continuity, Acceptance)
keeps recap/cue/continuity work under existing media and permitted presentation responsibilities;
cosmetic work never changes rules, pauses time or publishes exports without the explicit grant.

## Binding, authority and next consumers

Every production intent and internal completion must reuse reviewed `df-types` session/run,
operation/job/timer-instance and full `SessionRevision` identities, plus exact rules/content/source
and relevant access/index/asset generations. This decision adds no replacement IDs or codecs.
`df-session` checks binding, generation, causal identity and current basis before accepting a
result; stale completion cannot release a newer job or timer. Timers use explicit logical-time
inputs; pure model/engine code reads no system clock, database, SDK, provider or socket.

The session commits validated state, ordered facts and required intents atomically before
publication or dispatch, as `planning/c-df-engine-d01-decision.md` requires. A rejected candidate
creates no dispatch. Request cancellation or disconnect cannot cancel committed run-owned work;
explicit owner cancellation/run termination/shutdown controls it. Recovery deduplicates intents
and completions, preserves ambiguous/Unknown dispatch rather than issuing blind paid retries,
and does not reroll or regenerate during replay. Bounds, deadlines and per-kind cancellation
payloads remain actual implementation prerequisites, never inferred from this fixture.

The next actual consumers are `df-model`'s closed domain declarations, `df-engine`'s exhaustive
emission and pending-input matches, `df-session`'s `EffectExecutor` and owner ingress, and
`df-server`'s registration/readiness wiring to AI/media/persistence/Clock/API adapters. Their
minimal future source areas are the corresponding crates' domain, dispatch and composition
modules; no module or crate is created by this decision. Codecs remain at `df-api`, `df-client`
and `df-persistence` boundaries. Engine D03 independently proves finite dispatch/terminal-handler
wiring using this seven-kind comparison inventory; it does not become the model enum owner.

Safe diagnostic facts identify kind, owner, missing/duplicate/incompatible registration,
commit/dispatch/completion and stale/terminal classification. Native callers emit them through
the shared `df-observe` path with captured correlation and source/build identity; pure model
code emits no SDK logs. Credentials, private payloads and authorization never come from trace IDs.

## Alternatives and unresolved facts

An open string registry or generic script effect loses exhaustive ownership and permits default
routing; reject it. Giving `df-engine` provider/DB callbacks or directors their own state writers
breaks the approved dependency graph; reject it. Registering mechanical conditions as native
I/O gives executors rules authority; reject it. One untyped success callback loses explicit
failure, cancellation and stale-result handling; reject it. Eager dispatch before commit violates
durable ownership; reject it.

Exact public payload fields, source-qualified mechanics, complete production inventory,
per-operation bounds, typed terminal outcome payloads, dispatch fencing and transactional recovery
remain the G03/source/runtime implementation gates. The current work resolves the bounded
closed-family ownership decision and exercises every kind in its finite denominator. It does
not waive those prerequisites or mark any provider, authentication, database or game qualified.

## Executable finite proof

The literal below exhaustively maps every example effect to one owner and terminal kind,
accepts a complete registry, and removes/duplicates/corrupts each of the seven entries in turn.
It also accepts the three external input families and refuses all six internal terminal
families at client ingress. This pure std-only program proves the named finite boundary; it
performs no production authorization, asynchronous work, publication or persistence.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Effect {
    RunAi,
    RunMedia,
    LoadMemoryCandidates,
    ArmTimer,
    CancelJob,
    CancelTimer,
    PublishPresentation,
}

const EFFECTS: [Effect; 7] = [
    Effect::RunAi,
    Effect::RunMedia,
    Effect::LoadMemoryCandidates,
    Effect::ArmTimer,
    Effect::CancelJob,
    Effect::CancelTimer,
    Effect::PublishPresentation,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExecutorOwner {
    Ai,
    Media,
    MemoryCandidateStore,
    Clock,
    OwnedJobExecutor,
    AuthorizedPublication,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TerminalKind {
    AiFinished,
    MediaFinished,
    MemoryCandidatesReady,
    TimerExpired,
    JobCancelled,
    TimerCancelled,
    PresentationReported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Registration {
    effect: Effect,
    owner: ExecutorOwner,
    terminal: TerminalKind,
}

// Exhaustive arms give every closed kind one canonical binding.
fn registration(effect: Effect) -> Registration {
    let (owner, terminal) = match effect {
        Effect::RunAi => (ExecutorOwner::Ai, TerminalKind::AiFinished),
        Effect::RunMedia => (ExecutorOwner::Media, TerminalKind::MediaFinished),
        Effect::LoadMemoryCandidates => (
            ExecutorOwner::MemoryCandidateStore,
            TerminalKind::MemoryCandidatesReady,
        ),
        Effect::ArmTimer => (ExecutorOwner::Clock, TerminalKind::TimerExpired),
        Effect::CancelJob => (ExecutorOwner::OwnedJobExecutor, TerminalKind::JobCancelled),
        Effect::CancelTimer => (ExecutorOwner::Clock, TerminalKind::TimerCancelled),
        Effect::PublishPresentation => (
            ExecutorOwner::AuthorizedPublication,
            TerminalKind::PresentationReported,
        ),
    };
    Registration {
        effect,
        owner,
        terminal,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Missing(Effect),
    Duplicate(Effect),
    WrongOwner(Effect),
    WrongTerminal(Effect),
    ClientInternalInput,
}

fn validate_registrations(registrations: &[Registration]) -> Result<(), Refusal> {
    for effect in EFFECTS {
        let mut matching = registrations.iter().filter(|entry| entry.effect == effect);
        let actual = matching.next().ok_or(Refusal::Missing(effect))?;
        if matching.next().is_some() {
            return Err(Refusal::Duplicate(effect));
        }
        let expected = registration(effect);
        if actual.owner != expected.owner {
            return Err(Refusal::WrongOwner(effect));
        }
        if actual.terminal != expected.terminal {
            return Err(Refusal::WrongTerminal(effect));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GameInput {
    GameCommand,
    HostCommand,
    JobCompletion(TerminalKind),
    TimerExpired,
    PresentationReported,
}

fn terminal_input(terminal: TerminalKind) -> GameInput {
    match terminal {
        TerminalKind::TimerExpired => GameInput::TimerExpired,
        TerminalKind::PresentationReported => GameInput::PresentationReported,
        TerminalKind::AiFinished
        | TerminalKind::MediaFinished
        | TerminalKind::MemoryCandidatesReady
        | TerminalKind::JobCancelled
        | TerminalKind::TimerCancelled => GameInput::JobCompletion(terminal),
    }
}

fn client_ingress(input: GameInput) -> Result<GameInput, Refusal> {
    match input {
        GameInput::GameCommand | GameInput::HostCommand | GameInput::PresentationReported => {
            Ok(input)
        }
        GameInput::JobCompletion(_) | GameInput::TimerExpired => Err(Refusal::ClientInternalInput),
    }
}

fn main() {
    let complete: Vec<_> = EFFECTS.into_iter().map(registration).collect();
    assert_eq!(validate_registrations(&complete), Ok(()));
    assert_eq!(
        client_ingress(GameInput::GameCommand),
        Ok(GameInput::GameCommand)
    );
    assert_eq!(
        client_ingress(GameInput::HostCommand),
        Ok(GameInput::HostCommand)
    );
    assert_eq!(
        client_ingress(GameInput::PresentationReported),
        Ok(GameInput::PresentationReported)
    );
    for effect in EFFECTS {
        let expected = registration(effect);
        let missing: Vec<_> = complete
            .iter()
            .copied()
            .filter(|r| r.effect != effect)
            .collect();
        assert_eq!(
            validate_registrations(&missing),
            Err(Refusal::Missing(effect))
        );
        let mut duplicate = complete.clone();
        duplicate.push(expected);
        assert_eq!(
            validate_registrations(&duplicate),
            Err(Refusal::Duplicate(effect))
        );
        let mut wrong_owner = complete.clone();
        for entry in &mut wrong_owner {
            if entry.effect == effect {
                entry.owner = if expected.owner == ExecutorOwner::Ai {
                    ExecutorOwner::Media
                } else {
                    ExecutorOwner::Ai
                };
            }
        }
        assert_eq!(
            validate_registrations(&wrong_owner),
            Err(Refusal::WrongOwner(effect))
        );
        let mut wrong_terminal = complete.clone();
        for entry in &mut wrong_terminal {
            if entry.effect == effect {
                entry.terminal = if expected.terminal == TerminalKind::AiFinished {
                    TerminalKind::MediaFinished
                } else {
                    TerminalKind::AiFinished
                };
            }
        }
        assert_eq!(
            validate_registrations(&wrong_terminal),
            Err(Refusal::WrongTerminal(effect))
        );
        let input = terminal_input(expected.terminal);
        match effect {
            Effect::PublishPresentation => assert_eq!(client_ingress(input), Ok(input)),
            Effect::RunAi
            | Effect::RunMedia
            | Effect::LoadMemoryCandidates
            | Effect::ArmTimer
            | Effect::CancelJob
            | Effect::CancelTimer => {
                assert_eq!(client_ingress(input), Err(Refusal::ClientInternalInput));
            }
        }
    }
    println!(
        "PASS closed=7 complete=1 missing=7 duplicate=7 wrong_owner=7 wrong_terminal=7 client_internal_refusals=6"
    );
}
```

## Verification scope

Retained evidence binds this exact Markdown fence byte-for-byte to the compiled source, issued
source hashes, pinned Rust 1.98.1 tools, Rust 2024 edition and root `rustfmt.toml`. The guard
preserves 40% admission, 256 MiB/60 seconds and one shared compiler slot. Formatting, warnings-denied
standalone compilation and execution results are recorded in the immutable handoff; unperformed
checks cannot be inferred from this document. No Cargo/Clippy workspace, WASM/browser, actual
service registration, session/recovery/DB, provider, rules-catalog, gameplay, audio or device
qualification is claimed. Independent review and coordinator integration remain separate gates.
