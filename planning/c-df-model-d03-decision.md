# C-df-model-D03: pending resolution state

Date: 2026-10-02
Status: bounded decision candidate; production contracts, independent review and integration pending

## Decision and ownership

`df-model` defines the persisted domain representation of a pending resolution. Preserve separate
choice, roll, reaction and ruling states, each with the stable resolution and current offer/window
identity and the exact continuation position. A reaction before a draw and a reaction after that
draw are different windows even when the same participant and option occur in both. A generic
“await input” flag or linear story phase cannot represent this contract.

The continuation binds session/run, the applicable full `df-types::SessionRevision`, pinned
rules/source/catalog/handler and content versions, causal event/window and relevant offer basis.
Retain accepted selections, ordered actual draws, resources already spent, remaining required
responses and source-linked scoped rulings. Restoring or replaying this state resumes the recorded
position; it cannot regenerate offers, reroll, spend again or reinterpret a ruling under new sources.
Changing a source/handler requires an explicit compatible migration or a visible unsupported gap.
Checkpoint D01 preserves this payload; command/effect D02 carries closed response kinds, without
duplicating the continuation model or confusing a mechanical trigger with a native effect intent.

`df-rules` exclusively defines legal participants/options, draw requirements, causal timing/order,
cost application and deadline/pause/expiry policy from pinned clauses. No generic fixed order or
timeout default is selected here. Bound exhaustion suspends with a gap/authorized ruling rather
than skipping a legal trigger. An unavailable adjudicator leaves the ruling pending; an authorized
`HostCommand::ResolveRuling` refers to the current permitted ruling offer and records/discloses the
scoped decision. Neither a client input nor AI prose can inject arbitrary outcomes.

`df-engine` owns the pending continuation in its candidate, according to canonical
[engine D01](c-df-engine-d01-decision.md); required pending input stops dependent consequence work.
`df-session` authenticates/rechecks principal, binding, owner fence and current context, resolves a
previously committed operation before treating a retry as new input, and commits the pending change,
draws/resources/facts atomically before publication. Client observed revision is a basis for relevant
offer revalidation, not a universal compare-and-swap lock. The server's accepted candidate uses its
current full revision for commit. A stale response cannot resolve a later window. Expiry callbacks
also require the current timer instance/run/generation and source policy; presentation and disconnect
do not consume or reorder windows. Codecs/projections belong to persistence/API consumers and expose
only authorized views. Pure model/rules/engine code performs no clocks, timers, DB/provider or SDK I/O.
The caller instruments classified pending/accepted/refused facts through shared OTEL, with safe IDs
and versions rather than private choices or future dice in default logs.

## Alternatives and next consumer

- One untyped input flag loses legal next-input kind, interruption order and restart position.
- Recomputing a continuation from current sources loses admitted offers and can reroll or spend twice.
- Treating a ruling or timeout as automatic completion invents source behavior and bypasses authority.

The next actual implementation belongs to `df-model`'s pending-state module, consumed by `df-rules`
prepare/resolve, `df-engine` staged continuation and `df-session` admission/commit. Freeze exact shared
fields/errors with those owners before adding production source. D01 checkpoint validation and D02
closed inputs are separately owned comparison decisions until approved; this file admits no public
production type, protobuf field, rules catalog or handler. Exact 2024 sources, legal offers and timing
remain G03/G07 gates. The engine D02 comparison candidate is not a prerequisite or authority here.

## Finite executable illustration

The private std-only fixture reuses the worktree's actual `df-types` library. Its deliberately synthetic
sequence is choice → reaction before draw → roll → reaction after draw → ruling → complete. The
integer options, die range and resource costs are fixture data, with no D&D interpretation. Each
accepted response returns an owned candidate advancing the revision, keeping resolution identity,
draws and spent resources; the harness supplies that candidate as its next state. This is not a DB
commit, production authorization, legal-offer revalidation or replay implementation. Exact fixture
basis equality models an already admitted server continuation; production relevant-basis validation
belongs to the session/rules owners described above.

```rust
use df_types::{MemberId, RecoveryEpoch, RunId, SessionId, SessionRevision};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionId(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Window {
    Choice,
    BeforeDraw,
    Draw,
    AfterDraw,
    Ruling,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Binding {
    session: SessionId,
    run: RunId,
    revision: SessionRevision,
    source_pin: u8,
    resolution: ResolutionId,
    window: Window,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Pending {
    Choice,
    Reaction,
    Roll,
    Ruling,
    Complete,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct State {
    binding: Binding,
    pending: Pending,
    participant: MemberId,
    selected: Option<u8>,
    actual_draw: Option<u8>,
    resources_spent: u8,
    scoped_ruling: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Answer {
    Choice(u8),
    Reaction(u8),
    Roll(u8),
    Ruling(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Input {
    binding: Binding,
    participant: MemberId,
    authorized_host: bool,
    answer: Answer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    StaleBinding,
    IneligibleParticipant,
    WrongInputKind,
    InvalidSelection,
    UnauthorizedRuling,
    AlreadyComplete,
    InvalidContinuation,
    RevisionOverflow,
}

fn resolve(state: &State, input: Input) -> Result<State, Refusal> {
    if input.binding != state.binding {
        return Err(Refusal::StaleBinding);
    }
    if state.pending == Pending::Complete {
        return Err(Refusal::AlreadyComplete);
    }
    if input.participant != state.participant {
        return Err(Refusal::IneligibleParticipant);
    }
    if !matches!(
        (state.pending, state.binding.window),
        (Pending::Choice, Window::Choice)
            | (Pending::Reaction, Window::BeforeDraw | Window::AfterDraw)
            | (Pending::Roll, Window::Draw)
            | (Pending::Ruling, Window::Ruling)
    ) {
        return Err(Refusal::InvalidContinuation);
    }
    let mut next = *state;
    match (state.pending, input.answer) {
        (Pending::Choice, Answer::Choice(1 | 2)) => {
            if let Answer::Choice(selection) = input.answer {
                next.selected = Some(selection);
            }
            next.pending = Pending::Reaction;
            next.binding.window = Window::BeforeDraw;
        }
        (Pending::Reaction, Answer::Reaction(3)) => {
            next.resources_spent = state
                .resources_spent
                .checked_add(1)
                .ok_or(Refusal::InvalidContinuation)?;
            if state.binding.window == Window::BeforeDraw {
                next.pending = Pending::Roll;
                next.binding.window = Window::Draw;
            } else {
                next.pending = Pending::Ruling;
                next.binding.window = Window::Ruling;
            }
        }
        (Pending::Roll, Answer::Roll(draw @ 1..=20)) => {
            next.actual_draw = Some(draw);
            next.pending = Pending::Reaction;
            next.binding.window = Window::AfterDraw;
        }
        (Pending::Ruling, Answer::Ruling(7)) => {
            if !input.authorized_host {
                return Err(Refusal::UnauthorizedRuling);
            }
            next.scoped_ruling = Some(7);
            next.pending = Pending::Complete;
            next.binding.window = Window::Complete;
        }
        (Pending::Choice, Answer::Choice(_))
        | (Pending::Reaction, Answer::Reaction(_))
        | (Pending::Roll, Answer::Roll(_))
        | (Pending::Ruling, Answer::Ruling(_)) => return Err(Refusal::InvalidSelection),
        _ => return Err(Refusal::WrongInputKind),
    }
    next.binding.revision = state
        .binding
        .revision
        .next_sequence()
        .map_err(|_| Refusal::RevisionOverflow)?;
    Ok(next)
}

fn input(state: &State, answer: Answer) -> Input {
    Input {
        binding: state.binding,
        participant: state.participant,
        authorized_host: false,
        answer,
    }
}

fn main() {
    let epoch = RecoveryEpoch::new(2).unwrap();
    let initial = State {
        binding: Binding {
            session: SessionId::from_bytes(&[1; 16]).unwrap(),
            run: RunId::from_bytes(&[2; 16]).unwrap(),
            revision: SessionRevision::new(epoch, 4),
            source_pin: 1,
            resolution: ResolutionId(9),
            window: Window::Choice,
        },
        pending: Pending::Choice,
        participant: MemberId::from_bytes(&[3; 16]).unwrap(),
        selected: None,
        actual_draw: None,
        resources_spent: 0,
        scoped_ruling: None,
    };
    let before = resolve(&initial, input(&initial, Answer::Choice(1))).unwrap();
    assert_eq!(before.pending, Pending::Reaction);
    assert_eq!(before.binding.window, Window::BeforeDraw);
    assert_eq!(before.actual_draw, None);
    let before_answer = input(&before, Answer::Reaction(3));
    let roll = resolve(&before, before_answer).unwrap();
    assert_eq!(roll.pending, Pending::Roll);
    assert_eq!(roll.resources_spent, 1);
    let after = resolve(&roll, input(&roll, Answer::Roll(17))).unwrap();
    assert_eq!(after.pending, Pending::Reaction);
    assert_eq!(after.binding.window, Window::AfterDraw);
    assert_eq!(after.actual_draw, Some(17));
    assert_eq!(after.resources_spent, 1);
    assert_ne!(before.binding.window, after.binding.window);
    let restored = after;
    assert_eq!(
        resolve(&restored, before_answer),
        Err(Refusal::StaleBinding)
    );
    let ruling = resolve(&restored, input(&restored, Answer::Reaction(3))).unwrap();
    assert_eq!(ruling.pending, Pending::Ruling);
    assert_eq!(ruling.resources_spent, 2);
    let mut host_input = input(&ruling, Answer::Ruling(7));
    assert_eq!(
        resolve(&ruling, host_input),
        Err(Refusal::UnauthorizedRuling)
    );
    assert_eq!(ruling.scoped_ruling, None);
    host_input.authorized_host = true;
    let complete = resolve(&ruling, host_input).unwrap();
    assert_eq!(complete.pending, Pending::Complete);
    assert_eq!(complete.binding.resolution, initial.binding.resolution);
    assert_eq!(complete.binding.revision, SessionRevision::new(epoch, 9));
    assert_eq!(complete.selected, Some(1));
    assert_eq!(complete.actual_draw, Some(17));
    assert_eq!(complete.resources_spent, 2);
    assert_eq!(complete.scoped_ruling, Some(7));
    assert_eq!(resolve(&complete, host_input), Err(Refusal::StaleBinding));
    assert_eq!(
        resolve(&complete, input(&complete, Answer::Ruling(7))),
        Err(Refusal::AlreadyComplete)
    );

    for (state, answer, refusal) in [
        (initial, Answer::Choice(8), Refusal::InvalidSelection),
        (before, Answer::Roll(17), Refusal::WrongInputKind),
        (before, Answer::Reaction(8), Refusal::InvalidSelection),
        (roll, Answer::Roll(0), Refusal::InvalidSelection),
        (roll, Answer::Roll(21), Refusal::InvalidSelection),
        (roll, Answer::Reaction(3), Refusal::WrongInputKind),
        (after, Answer::Choice(1), Refusal::WrongInputKind),
        (ruling, Answer::Ruling(8), Refusal::InvalidSelection),
    ] {
        let original = state;
        assert_eq!(resolve(&state, input(&state, answer)), Err(refusal));
        assert_eq!(state, original);
    }
    let current = input(&after, Answer::Reaction(3));
    for binding in [
        Binding {
            session: SessionId::from_bytes(&[4; 16]).unwrap(),
            ..current.binding
        },
        Binding {
            run: RunId::from_bytes(&[5; 16]).unwrap(),
            ..current.binding
        },
        Binding {
            revision: SessionRevision::new(RecoveryEpoch::new(1).unwrap(), 7),
            ..current.binding
        },
        Binding {
            revision: SessionRevision::new(epoch, 6),
            ..current.binding
        },
        Binding {
            source_pin: 2,
            ..current.binding
        },
        Binding {
            resolution: ResolutionId(10),
            ..current.binding
        },
        Binding {
            window: Window::BeforeDraw,
            ..current.binding
        },
    ] {
        assert_eq!(
            resolve(&after, Input { binding, ..current }),
            Err(Refusal::StaleBinding)
        );
    }
    assert_eq!(
        resolve(
            &after,
            Input {
                participant: MemberId::from_bytes(&[6; 16]).unwrap(),
                ..current
            }
        ),
        Err(Refusal::IneligibleParticipant)
    );
    let mut exhausted = initial;
    exhausted.binding.revision = SessionRevision::new(epoch, u64::MAX);
    assert_eq!(
        resolve(&exhausted, input(&exhausted, Answer::Choice(1))),
        Err(Refusal::RevisionOverflow)
    );
    assert_eq!(exhausted.selected, None);
    let mut malformed = after;
    malformed.binding.window = Window::Draw;
    assert_eq!(
        resolve(&malformed, input(&malformed, Answer::Reaction(3))),
        Err(Refusal::InvalidContinuation)
    );
}
```

## Evidence and limits

The frozen input is `4c0e050c2d30e632bd3b32c51a92cfda52479e6e`. Governing sources are
subsystem architecture/interfaces, rules effect model/support, runtime reliability, long-horizon
state, campaign cinematics, canonical engine D01, observability, AGENTS, coding style and ADR 0001–0005.
Their issued hashes and actual `df-types` source are retained with the frozen brief. The attempt
evidence identifies exact Markdown extraction, byte comparison, pinned Rust 1.98.1 tools/config,
Rust 2024 warning-denied compilation and finite execution under the unchanged v6 admission guard.
These checks establish only finite state/window preservation and typed refusal, with explicit
supplied draws and no input mutation. The restored value is an in-memory copy, not a codec/DB restart.
The fixture omits timer expiry, trigger nesting, full multi-draw transcripts, resource mechanics,
operation deduplication, final combined-rules validation, authorization and durable commit/recovery.
They remain requirements for later owner implementations, not claimed passing runtime behavior.

No Cargo, Clippy, WASM/browser, PostgreSQL, provider, audio, actual D&D or integrated game checks
were performed for this decision. Independent frontier review and coordinator resulting-source
verification remain separate acceptance gates. Exact source clauses, production contracts and
measured bounds must be frozen before their consumers are dispatched.
