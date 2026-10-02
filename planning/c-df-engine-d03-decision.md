# C-df-engine-D03: effect registration completeness

Date: 2026-10-02
Status: bounded registration decision; production contract and execution remain pending

## Decision and ownership

Before accepting a transition, `df-engine` must be able to establish that every kind it can emit belongs to the reviewed closed `df-model::Effect` inventory and has exactly one compatible executor and terminal `GameInput` route. At composition startup, `df-server` verifies the whole inventory, not merely the kinds present in one example transition. Missing, duplicate, unknown or incompatible registrations fail readiness; the engine cannot fabricate success, silently drop an effect or substitute a provider. A validated immutable registration capability supplied to the engine records the pinned inventory/configuration revision. Adding a kind requires an exhaustive shared-model mapping, executor registration, terminal route and coordinated consumer checks before readiness can succeed again.

The model D02 decision owns the draft closed inventory and its owner/terminal pairing. This engine decision consumes that inventory for registration and reachability; it does not create a second shared contract. The comparison inventory is `RunAi`, `RunMedia`, `LoadMemoryCandidates`, `ArmTimer`, `CancelJob`, `CancelTimer`, `PublishPresentation`, paired respectively with `AiFinished`, `MediaFinished`, `MemoryCandidatesReady`, `TimerExpired`, `JobCancelled`, `TimerCancelled`, `PresentationReported`. These are a finite planning illustration, not a frozen production enum, wire numbering or standard 2024 mechanics catalog. Actual source-specific mechanics stay within `df-rules`, not the native executor inventory.

`df-engine` remains pure: it emits intents and typed safe diagnostic facts, performs no native executor calls and starts no jobs. `df-session` owns its `EffectExecutor` port, durable session/run lifetime, admission, terminal input validation and authoritative serialization. `df-server` supplies all concrete adapters: AI through `df-ai`, media through `df-media`, memory through the session-owned `MemoryCandidateStore` implemented by `df-persistence`, clock and job/timer cancellation through session-owned ports, and authorized presentation publication through `df-api`. Adapter implementations cannot depend back through the engine. A terminal result returns to the session owner as closed validated input, never arbitrary public event injection.

Complete registration means both reachable native execution and a reachable terminal input consumer, including typed failure/cancellation/unavailability outcomes as appropriate to that kind. It does not mean every effect immediately finishes. Timer arming terminates through an expiry, explicit cancellation or terminal failure; presentation delivery and actual playback reports remain distinct observations. Terminal acceptance rechecks session/run, operation/job/timer instance, generation and admitted basis. A stale result cannot release newer work. Commit precedes dispatch; request cancellation cannot undo committed effects; unknown paid outcomes require reconciliation rather than blind retry. The finite example below exercises immediate success routing only and does not claim these durable runtime guarantees.

## Alternatives and unresolved facts

Counting registrations would permit duplicates to conceal missing kinds. Checking only emitted samples would miss dormant paths. Arbitrary string dispatch would permit unknown kinds and hide incompatible terminal routes. All are rejected in favor of a closed inventory, per-kind uniqueness and exhaustive compatible routing. A single generic executor or terminal no-op is insufficient production wiring; the functions below are explicit test fixtures, with the kind-specific exhaustive mapping checked on every finite route.

Exact production `Effect`/`GameInput` variants, payloads, IDs, compatibility revisions, executor object safety and terminal error unions remain G03 prerequisites. Required/optional capability classification, asynchronous bounds/deadlines, retries, outbox fencing, cancellation races and durable acceptance fixtures need their existing owning implementation tasks. Generated content cannot extend executability: an unsupported handler remains `UnsupportedMechanic`/`SourceGap` or explicitly selected custom approval, never a newly registered script. This task changes only this planning decision.

## Bounded executable registration consumer

This std-only program is an engine-registration consumer illustration of the model D02 draft inventory. It validates seven registrations, then actually calls each finite executor and terminal input consumer. Unknown numeric ingress is refused before dispatch; numbers are illustrative local fixture discriminants. Fixture calls during validation are safe because these functions are pure, immediate and test-only. Production readiness verifies typed bindings and controlled fixtures, never starts providers/timers merely to probe registration. Each refusal returns no ready registry, so partial registration cannot be used.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    RunAi,
    RunMedia,
    LoadMemoryCandidates,
    ArmTimer,
    CancelJob,
    CancelTimer,
    PublishPresentation,
}

const INVENTORY: [Kind; 7] = [
    Kind::RunAi,
    Kind::RunMedia,
    Kind::LoadMemoryCandidates,
    Kind::ArmTimer,
    Kind::CancelJob,
    Kind::CancelTimer,
    Kind::PublishPresentation,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Terminal {
    AiFinished,
    MediaFinished,
    MemoryCandidatesReady,
    TimerExpired,
    JobCancelled,
    TimerCancelled,
    PresentationReported,
}

fn fixture_execute(kind: Kind) -> Terminal {
    match kind {
        Kind::RunAi => Terminal::AiFinished,
        Kind::RunMedia => Terminal::MediaFinished,
        Kind::LoadMemoryCandidates => Terminal::MemoryCandidatesReady,
        Kind::ArmTimer => Terminal::TimerExpired,
        Kind::CancelJob => Terminal::JobCancelled,
        Kind::CancelTimer => Terminal::TimerCancelled,
        Kind::PublishPresentation => Terminal::PresentationReported,
    }
}

fn fixture_consume(input: Terminal) -> Kind {
    match input {
        Terminal::AiFinished => Kind::RunAi,
        Terminal::MediaFinished => Kind::RunMedia,
        Terminal::MemoryCandidatesReady => Kind::LoadMemoryCandidates,
        Terminal::TimerExpired => Kind::ArmTimer,
        Terminal::JobCancelled => Kind::CancelJob,
        Terminal::TimerCancelled => Kind::CancelTimer,
        Terminal::PresentationReported => Kind::PublishPresentation,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Unknown(u8),
    Duplicate(Kind),
    MissingKind(Kind),
    MissingExecutor(Kind),
    MissingTerminalInput(Kind),
    IncompatibleExecutor(Kind),
    IncompatibleTerminalInput(Kind),
}

fn decode_kind(value: u8) -> Result<Kind, Refusal> {
    match value {
        0 => Ok(Kind::RunAi),
        1 => Ok(Kind::RunMedia),
        2 => Ok(Kind::LoadMemoryCandidates),
        3 => Ok(Kind::ArmTimer),
        4 => Ok(Kind::CancelJob),
        5 => Ok(Kind::CancelTimer),
        6 => Ok(Kind::PublishPresentation),
        other => Err(Refusal::Unknown(other)),
    }
}

#[derive(Clone, Copy)]
struct Registration {
    kind: u8,
    execute: Option<fn(Kind) -> Terminal>,
    consume: Option<fn(Terminal) -> Kind>,
}

struct ReadyRegistry<'a> {
    registrations: &'a [Registration],
}

impl<'a> ReadyRegistry<'a> {
    fn validate(registrations: &'a [Registration]) -> Result<Self, Refusal> {
        let mut seen = Vec::new();
        for registration in registrations {
            let kind = decode_kind(registration.kind)?;
            if seen.contains(&kind) {
                return Err(Refusal::Duplicate(kind));
            }
            seen.push(kind);
            let execute = registration.execute.ok_or(Refusal::MissingExecutor(kind))?;
            let consume = registration
                .consume
                .ok_or(Refusal::MissingTerminalInput(kind))?;
            let terminal = execute(kind);
            if terminal != fixture_execute(kind) {
                return Err(Refusal::IncompatibleExecutor(kind));
            }
            if consume(terminal) != kind {
                return Err(Refusal::IncompatibleTerminalInput(kind));
            }
        }
        for kind in INVENTORY {
            if !seen.contains(&kind) {
                return Err(Refusal::MissingKind(kind));
            }
        }
        Ok(Self { registrations })
    }

    fn dispatch(&self, raw_kind: u8) -> Result<(Terminal, Kind), Refusal> {
        let kind = decode_kind(raw_kind)?;
        let registration = self
            .registrations
            .iter()
            .find(|registration| registration.kind == raw_kind)
            .ok_or(Refusal::MissingKind(kind))?;
        let execute = registration.execute.ok_or(Refusal::MissingExecutor(kind))?;
        let consume = registration
            .consume
            .ok_or(Refusal::MissingTerminalInput(kind))?;
        let terminal = execute(kind);
        Ok((terminal, consume(terminal)))
    }
}

fn wrong_executor(_: Kind) -> Terminal {
    Terminal::MediaFinished
}

fn wrong_consumer(_: Terminal) -> Kind {
    Kind::RunMedia
}

fn main() {
    let registrations: Vec<_> = (0..7)
        .map(|kind| Registration {
            kind,
            execute: Some(fixture_execute),
            consume: Some(fixture_consume),
        })
        .collect();
    let ready = ReadyRegistry::validate(&registrations).ok().unwrap();
    for (raw, kind) in (0..7).zip(INVENTORY) {
        assert_eq!(ready.dispatch(raw), Ok((fixture_execute(kind), kind)));
    }
    assert_eq!(ready.dispatch(99), Err(Refusal::Unknown(99)));
    assert_eq!(
        ReadyRegistry::validate(&registrations[..6]).err(),
        Some(Refusal::MissingKind(Kind::PublishPresentation))
    );
    for (changed, expected) in [
        (
            Registration {
                kind: 0,
                execute: None,
                consume: Some(fixture_consume),
            },
            Refusal::MissingExecutor(Kind::RunAi),
        ),
        (
            Registration {
                kind: 0,
                execute: Some(fixture_execute),
                consume: None,
            },
            Refusal::MissingTerminalInput(Kind::RunAi),
        ),
        (
            Registration {
                kind: 0,
                execute: Some(wrong_executor),
                consume: Some(fixture_consume),
            },
            Refusal::IncompatibleExecutor(Kind::RunAi),
        ),
        (
            Registration {
                kind: 0,
                execute: Some(fixture_execute),
                consume: Some(wrong_consumer),
            },
            Refusal::IncompatibleTerminalInput(Kind::RunAi),
        ),
    ] {
        let mut invalid = registrations.clone();
        invalid[0] = changed;
        assert_eq!(ReadyRegistry::validate(&invalid).err(), Some(expected));
    }
    let mut duplicate = registrations.clone();
    duplicate.push(registrations[0]);
    assert_eq!(
        ReadyRegistry::validate(&duplicate).err(),
        Some(Refusal::Duplicate(Kind::RunAi))
    );
    let mut unknown = registrations.clone();
    unknown.push(Registration {
        kind: 99,
        execute: Some(fixture_execute),
        consume: Some(fixture_consume),
    });
    assert_eq!(
        ReadyRegistry::validate(&unknown).err(),
        Some(Refusal::Unknown(99))
    );
    println!(
        "PASS: seven executor-to-terminal routes; missing, duplicate, unknown and incompatible registration refused"
    );
}
```

## Sources, verification and next consumer

The decision follows `planning/subsystem-architecture.md` (server ownership, consumer ports, startup readiness), `planning/subsystem-interfaces.md` (closed internal inputs, pure effect intents, session and server wiring), `planning/generated-content.md` (candidate admission and no arbitrary executable mechanics), `planning/c-df-engine-d01-decision.md` (one staged pure transition and commit before effects), `planning/rules-effect-model.md` (source-backed closed mechanics), `planning/runtime-reliability.md` (owned work and stale completion rejection), and `planning/observability.md` (pure facts and classified native startup failures). The frozen backlog outcome, AGENTS, coding style and ADR 0001–0005 govern scope, bounded evidence and independent review. The adjacent model D02 decision supplies comparison inputs; final source identity is retained with the handoff rather than assumed integrated.

The verification procedure extracts the literal exactly, formats/checks it with pinned Rust 1.98.1/root rustfmt settings, compiles as Rust 2024 with warnings denied and executes under the unchanged v6 40% admission, 256 MiB/60-second guard. Extraction, pinned format-write/check, Rust 2024 compilation with warnings denied and finite execution passed. The first format-check admission returned `HOLD_NO_COMMAND_LAUNCHED`, so work stopped and its snapshot remains retained. One coordinator-approved continuation after the long dependency build ended and the shared lock became available completed only the unlaunched checks, with unchanged guard and assertions. Exact ordered inventory and all seven terminal mappings match model D02 commit `47daeb632f25c65db8af63087d7b29d02488632d` (document SHA256 `688cdb42cea69ae2477ec03f23070770b0025c1ac6b6dab0c63aa744623cc990`); its copied comparison source and hashes are retained. Receipts and the streamed manifest bind the submitted source, literal, tools/configuration and commands/results. This proves the finite illustration only. No Cargo, Clippy, WASM, browser/audio, provider, persistence or production executor check is claimed. Independent review and integrated revision validation remain coordinator gates.

The next actual source consumer is a coordinated `df-model` closed effect/input definition followed by `crates/df-engine/src/` effect-kind admission against a validated registration capability, `crates/df-session/src/` executor and terminal intake ports, and `crates/df-server/src/` exhaustive concrete registration/readiness. These paths are proposed future edit areas, not changes made by this task. Existing AI/media/persistence/API owners provide their adapters; the session owner proves committed intent dispatch and stale/failed/cancelled terminal handling through the real composition root. Production readiness cannot be inferred from this standalone example.
