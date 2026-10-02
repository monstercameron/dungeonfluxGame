# C-df-engine-D02: pending continuations

Date: 2026-10-02  
Status: bounded source-backed decision; production contracts and integrated runtime pending

## Decision and owners

`df-engine` owns one staged pending resolution: stable resolution identity, current offer identity, expected revision, causal parent, admitted rules/content versions and the next permitted typed inputs. `df-rules` owns source-grounded timing, eligible actors, legal offers, actual draw validation, costs and the resumed mechanical result. Engine mode/submode and optional director availability cannot close a required rules window. This refines `c-df-engine-d01-decision.md`; optional typed decline/no-change and optional capacity defer retain required legal mechanics. Other required failures reject the candidate explicitly.

Rules preparation/resolution returns `NeedsChoice`, `NeedsRoll`, `NeedsReaction`, source-grounded `NeedsRuling`, complete, or an explicit unsupported gap. The engine stages a pending transition with no unvalidated downstream mechanics. That transition may preserve already validated costs/draws only when the pinned source requires their consumption before suspension; it never assumes all costs occur at one generic phase. Session durably records that suspension before apply/publication. A pending record includes the remaining continuation, ordered consumed draws/resources and legal offer provenance. Resolving one offer may produce another pending offer with the same causal resolution identity and a new offer identity. Neither retries nor reconnects mint a new identity or reroll. A resumed rules result still passes the D01 staged composition and final combined rules validation before acceptance.

Session authenticates and authorizes actor/host separately, serializes commands and rechecks owner fence/current revision before the atomic PostgreSQL decision commit. Engine checks resolution/offer/basis and typed input eligibility against current rules offers. Client data cannot create an internal timer, completion or host ruling. A ruling names the current pending offer and permitted typed interpretation/source provenance, is authorized and durably disclosed as a scoped ruling, and never silently becomes standard catalog support. Missing adjudication leaves pending plus a visible gap. Disconnect/request cancellation cannot cancel an already committed continuation. Timer/job replies carry admitted binding/generation and are revalidated by the session owner; stale replies do not consume dice/resources.

Session commits state, operation receipt, ordered decision/facts, actual draws and required effect intents atomically before engine apply/publication. Exact duplicate operations return their stored receipt at session lookup; a fresh operation that repeats a consumed offer is refused by engine. Ambiguous commit uses operation lookup/reload, never blind reapplication. Durable replay uses recorded decisions, offers, actual draws and pinned handler/content versions with complete reducers; it does not call providers or recreate dice. Unsupported versions and missing/redacted records are explicit replay gaps. A replayed record still matches its expected revision and applies once.

Rules choose legal nesting/order from source, while the admitted engine policy bounds trigger depth, offer count, output bytes and resumed work per pass. Exhaustion suspends with an explicit gap/authorized adjudication path and retained continuation; it cannot skip a required trigger or invent a default. Numeric production budgets require later evidence. Pure engine/rules return safe typed diagnostic facts for `df-session`/`df-observe`: resolution/offer/operation, basis/generation, versions, accepted/pending/stale/duplicate/gap classification and ordered-draw counts. Private choices, prompts and future dice are excluded from default telemetry and projected only through authorized audiences.

## Governing sources and next admission

The input revision and issued source hashes are retained in the immutable handoff/manifest. Applicable decisions are `subsystem-architecture.md` (engine/rules/model/session ownership), `subsystem-interfaces.md` (pure-domain pending results and exact-once Transition; identity/session commit), `runtime-directors.md` (staged candidates, required mechanics despite optional capacity, deterministic records), `runtime-reliability.md` (current binding/revision and committed ownership), `rules-effect-model.md` (source trigger windows, persisted continuation/draws/resources, bounded explosion), `rules-support.md` (required 2024 target and unpinned exact catalog), and `generated-content.md` (candidate validation and explicit custom ruleset). Coding/workflow/observability sources in the brief govern verification and evidence.

Next consumers are `df-engine`, `df-rules`, `df-model` and `df-session`; minimal later source admission is their continuation/outcome/state/decision modules and focused contract fixtures, frozen together with canonical shared ID/version types. No production source path/module, public shared type, protobuf field, catalog clause or deadline policy is admitted by this document. Required open gates: exact source/catalog/handler revisions, source-reviewed timing fixtures, canonical offer/input/provenance types, actor eligibility and timeout/pause/cancel rules, trigger budgets, serialized operation/fence behavior, durable recovery/migration/replay and audience projections. G03/shared-contract and actual source waves own those definitions.

Rejected alternatives: director-owned pending state duplicates authority; arbitrary event/host input bypasses source legality; a universal interruption stack imposes unsupported ordering; applying before persistence loses recovery; rolling again on retry changes outcomes; optional capacity blocking required rules violates D01. Explicit bounded pending state preserves the legal continuation without admitting those alternatives.

## Finite executable boundary illustration

This private std-only fixture scripts four synthetic offers, with one allowed value per offer, one illustrative draw, a four-step limit and one fixture source version. Its rules adapter owns that sequence; it is not a D&D mechanic, production API or measured limit. Engine owns offer identity/revision and staged state. A local session ledger models commit-before-apply and replay; it proves finite ordering assertions only, not PostgreSQL durability, production authorization, actual trigger nesting or integrated gameplay. No optional director is required to resolve these offers.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionId(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OfferId(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Answer {
    Choice(u8),
    Roll(u8),
    Reaction(bool),
    Ruling(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Pending {
    resolution: ResolutionId,
    offer: OfferId,
    allowed: Answer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct State {
    revision: u8,
    pending: Option<Pending>,
    steps: u8,
    draw: Option<u8>,
    last_operation: Option<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Input {
    operation: u8,
    revision: u8,
    resolution: ResolutionId,
    offer: OfferId,
    source_version: u8,
    answer: Answer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Duplicate,
    StaleBasis,
    NoPending,
    StaleResolution,
    StaleOffer,
    UnsupportedSource,
    IllegalInput,
    BoundGap,
    RevisionOverflow,
    CommitFailed,
    NotCommitted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Transition {
    expected_revision: u8,
    next: State,
}

fn rules_resolve(pending: Pending, answer: Answer) -> Result<Option<Pending>, Refusal> {
    if answer != pending.allowed {
        return Err(Refusal::IllegalInput);
    }
    let next = match answer {
        Answer::Choice(_) => Some((2, Answer::Roll(11))),
        Answer::Roll(_) => Some((3, Answer::Reaction(false))),
        Answer::Reaction(_) => Some((4, Answer::Ruling(2))),
        Answer::Ruling(_) => None,
    };
    Ok(next.map(|(offer, allowed)| Pending {
        resolution: pending.resolution,
        offer: OfferId(offer),
        allowed,
    }))
}

fn decide(state: State, input: Input) -> Result<Transition, Refusal> {
    if state.last_operation == Some(input.operation) {
        return Err(Refusal::Duplicate);
    }
    if state.revision != input.revision {
        return Err(Refusal::StaleBasis);
    }
    let pending = state.pending.ok_or(Refusal::NoPending)?;
    if pending.resolution != input.resolution {
        return Err(Refusal::StaleResolution);
    }
    if pending.offer != input.offer {
        return Err(Refusal::StaleOffer);
    }
    if input.source_version != 1 {
        return Err(Refusal::UnsupportedSource);
    }
    if state.steps >= 4 {
        return Err(Refusal::BoundGap);
    }
    let next_pending = rules_resolve(pending, input.answer)?;
    let revision = state
        .revision
        .checked_add(1)
        .ok_or(Refusal::RevisionOverflow)?;
    Ok(Transition {
        expected_revision: state.revision,
        next: State {
            revision,
            pending: next_pending,
            steps: state.steps + 1,
            draw: match input.answer {
                Answer::Roll(draw) => Some(draw),
                _ => state.draw,
            },
            last_operation: Some(input.operation),
        },
    })
}

struct SessionFixture {
    state: State,
    records: Vec<Transition>,
}

impl SessionFixture {
    fn commit(&mut self, transition: Transition, storage_available: bool) -> Result<(), Refusal> {
        if !storage_available {
            return Err(Refusal::CommitFailed);
        }
        if self.state.revision != transition.expected_revision {
            return Err(Refusal::StaleBasis);
        }
        if self.records.last() == Some(&transition) {
            return Err(Refusal::Duplicate);
        }
        if self.records.len() >= 4 {
            return Err(Refusal::BoundGap);
        }
        self.records.push(transition);
        Ok(())
    }

    fn apply(&mut self, transition: Transition) -> Result<(), Refusal> {
        if self.state.revision != transition.expected_revision {
            return Err(Refusal::StaleBasis);
        }
        if self.records.last() != Some(&transition) {
            return Err(Refusal::NotCommitted);
        }
        self.state = transition.next;
        Ok(())
    }
}

fn main() {
    let initial = State {
        revision: 0,
        pending: Some(Pending {
            resolution: ResolutionId(7),
            offer: OfferId(1),
            allowed: Answer::Choice(1),
        }),
        steps: 0,
        draw: None,
        last_operation: None,
    };
    let first_input = Input {
        operation: 1,
        revision: 0,
        resolution: ResolutionId(7),
        offer: OfferId(1),
        source_version: 1,
        answer: Answer::Choice(1),
    };
    let mut session = SessionFixture {
        state: initial,
        records: Vec::new(),
    };
    for (bad, expected) in [
        (
            Input {
                revision: 9,
                ..first_input
            },
            Refusal::StaleBasis,
        ),
        (
            Input {
                resolution: ResolutionId(8),
                ..first_input
            },
            Refusal::StaleResolution,
        ),
        (
            Input {
                offer: OfferId(9),
                ..first_input
            },
            Refusal::StaleOffer,
        ),
        (
            Input {
                source_version: 2,
                ..first_input
            },
            Refusal::UnsupportedSource,
        ),
        (
            Input {
                answer: Answer::Choice(9),
                ..first_input
            },
            Refusal::IllegalInput,
        ),
        (
            Input {
                answer: Answer::Roll(11),
                ..first_input
            },
            Refusal::IllegalInput,
        ),
    ] {
        assert_eq!(decide(session.state, bad), Err(expected));
        assert_eq!(session.state, initial);
    }
    assert_eq!(
        decide(
            State {
                steps: 4,
                ..initial
            },
            first_input
        ),
        Err(Refusal::BoundGap)
    );
    assert_eq!(
        decide(
            State {
                revision: u8::MAX,
                ..initial
            },
            Input {
                revision: u8::MAX,
                ..first_input
            }
        ),
        Err(Refusal::RevisionOverflow)
    );
    let first = decide(initial, first_input).unwrap();
    assert_eq!(session.apply(first), Err(Refusal::NotCommitted));
    assert_eq!(session.commit(first, false), Err(Refusal::CommitFailed));
    assert_eq!(session.state, initial);
    assert!(session.records.is_empty());
    let mut previous_input = first_input;
    for operation in 1..=4 {
        let pending = session.state.pending.unwrap();
        let input = Input {
            operation,
            revision: session.state.revision,
            resolution: pending.resolution,
            offer: pending.offer,
            source_version: 1,
            answer: pending.allowed,
        };
        if operation > 1 {
            assert_eq!(
                decide(session.state, previous_input),
                Err(Refusal::Duplicate)
            );
            assert_eq!(
                decide(
                    session.state,
                    Input {
                        operation,
                        revision: session.state.revision,
                        ..previous_input
                    }
                ),
                Err(Refusal::StaleOffer)
            );
        }
        let transition = decide(session.state, input).unwrap();
        let before = session.state;
        session.commit(transition, true).unwrap();
        assert_eq!(session.commit(transition, true), Err(Refusal::Duplicate));
        assert_eq!(session.state, before);
        session.apply(transition).unwrap();
        assert_eq!(session.apply(transition), Err(Refusal::StaleBasis));
        previous_input = input;
    }
    assert_eq!(session.state.pending, None);
    assert_eq!(session.state.draw, Some(11));
    assert_eq!(session.state.steps, 4);
    assert_eq!(
        decide(
            session.state,
            Input {
                operation: 5,
                revision: 4,
                ..first_input
            }
        ),
        Err(Refusal::NoPending)
    );
    let mut replay = SessionFixture {
        state: initial,
        records: Vec::new(),
    };
    for record in &session.records {
        replay.commit(*record, true).unwrap();
        replay.apply(*record).unwrap();
    }
    assert_eq!(replay.state, session.state);
    println!(
        "PASS: 4 typed offers; stale/duplicate/illegal/source/bound/overflow refusals; commit-before-apply; deterministic replay"
    );
}
```

## Verification limits

The exact Markdown Rust literal was extracted and compared byte-for-byte, formatted with pinned Rust 1.98.1/root configuration, checked, compiled as edition 2024 with warnings denied, and executed successfully. Actual tool versions, argv, source/config hashes and finite assertion output are retained in the immutable v2 handoff/manifest. The initial v6 admission HOLD at39% launched no command and remains preserved with the earlier unverified candidate. After coordinator readmission based on an observed43%, both fresh guard calls admitted at40% within the unchanged256 MiB/60-second bound. No compiler or behavioral-assertion repair was needed. One evidence helper incorrectly assumed rustfmt changes whitespace only; a retained formatter-reproduction check addresses its added trailing commas without changing behavioral assertions. Evidence proves this decision and its finite synthetic boundary only; independent frontier review and integrated acceptance remain coordinator gates.

No Cargo, Clippy, WASM, browser, audio, provider, production session/PostgreSQL durability, authorization, timer/job cancellation or source-grounded 2024 mechanics check is claimed. The fixture keeps only the last operation for its duplicate refusal; production session operation lookup owns the complete admitted idempotency namespace. Production budgets, source catalog, handler migrations, rights and concrete shared types remain explicit prerequisites.
