# Effect, condition and trigger composition decision

Task: B-C-df-rules-D02; attempt: B-C-df-rules-D02-a1. Input revision:
`d870db00dc349425e8f977bdfb20c6ffb24bfd3e`.
Status: bounded design decision and checked standalone structural example; production
types, source handlers, catalog coverage and integrated behavior remain unverified.

## Governing authority and decision

[Rules support](rules-support.md) and [Rules coverage](rules-coverage.md) require
the complete selected standard 2024 fifth-edition scope, with pinned primary source,
book/catalog and errata evidence. This document does not select a smaller catalog,
copy unprovided source text, qualify a mechanic or establish commercial rights.
[Rules effect model](rules-effect-model.md), “Effects, conditions and timing” and
“Source-linked testing architecture”, governs this interface. The exact source
entry/clause, handler revision and exceptional precedence must be reviewed before
that mechanic is usable. Matching condition labels is insufficient.

Use one closed typed handler composition inside pure `df-rules`. Immutable
parameters and provenance come from `df-content`; imported content never executes
scripts. The engine owns the pending continuation; the session commits its state,
actual ordered dice and resources together before publication. This follows
[Subsystem interfaces](subsystem-interfaces.md), “Pure domain and content” and
“Identity and session ownership”, and [Subsystem architecture](subsystem-architecture.md).
No public Rust declarations or serialization schema are admitted by this decision.
Existing `RulesetId`, identity/revision and typed result contracts must be reused
after their owning gates approve the exact representations.

The minimum contract to freeze at G03/G07 is:

| Existing candidate concept | Required composition information |
| --- | --- |
| EffectDefinition | source entry/clause and digest; handler version; legal parameters, targets, prerequisites and costs; timing/duration; explicit source-specific stacking, replacement and removal |
| ActiveEffect | stable instance/origin and source/target; rules-time start/expiry; applicable concentration linkage; retained choices |
| TriggerWindow | stable causal event and window identity; source phase/order and exceptional precedence; eligible participants and source-defined interrupt options; admitted choice/deadline/pause policy |
| PendingResolution | stable continuation and current window; ordered actual dice already consumed; committed resources; unresolved choices/reactions and explicit bounded suspension reason |

Preparation derives legal offers from the pinned state and clauses. Submission
revalidates window identity, current basis, eligible participant and permitted
typed response before proposing costs or resolution. The source owner specifies
whether an interrupt precedes or follows the triggering outcome; this decision
does not impose initiative sorting, label ordering or universal modifier arithmetic.
Expiry, stacking/replacement/removal and concentration use the same pinned handler
and source-specific precedence. A stale, unknown or unsupported handler/window
cannot silently fall back to a generic action.

“Closed” means a finite set of reviewed responses and outcomes, with every pending
window either retained for its named next input, explicitly suspended, or closed
by a source-valid resolution/cancellation. It does not mean refusing all reactions
or closing an unresolved window on a timeout. “Bounded” means admitted item/byte,
trigger depth/count and continuation-lifetime limits with visible outcomes. A bound
hit preserves the unresolved causal window and prior committed work, suspending
with an explicit gap/scoped ruling requirement; it never skips a legal effect.
Production values remain an admission prerequisite. Choice deadlines and logical
pause are campaign policy, not a fabricated standard combat rule.

Source-discretionary `NeedsRuling` uses the existing scoped authorized host flow
in [Interaction engine](interaction-engine.md), “Source-guided rulings for an
inexperienced host”. Absent adjudication retains a visible pending gap and permitted
alternatives. No timeout rolls dice, spends resources or invents a ruling. Recovery
retains the same rules/source/handler basis, dice and resource record; unsupported
migration explicitly rejects. Client disconnect cannot erase committed work.
Pure rules return safe structured diagnostic facts to their native consumer under
[Observability](observability.md); this interface adds no SDK, wall clock, database,
provider, socket or competing logging path.

[Generated content](generated-content.md) requires approved source-compatible
templates in standard mode. Novel mechanics require independently reviewed closed
handlers, a disclosed distinct custom RulesetId/catalog and applicable gates;
host approval alone cannot admit executable code. [Commercial validation](commercial-validation.md)
keeps full required coverage and reviewed rights as separate launch prerequisites.

## Exact bounded Rust example

This is a private, dependency-free structural fixture, not a standard D&D mechanic,
handler registry, public type proposal or source-linked golden. Synthetic scalar
identities stand in for the canonical owning types only inside this executable.
The fixture's count/depth limit of two is an explicit test bound, not a production
or tabletop rule. `Close` represents an already validated source outcome supplied
by the future handler; the fixture cannot establish that source validation itself.
It proves admission, retention on refusal and a terminal close at this boundary.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pending {
    window: u8,
    basis: u8,
    remaining: u8,
    depth: u8,
    consumed_draws: u8,
    spent_resources: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Response {
    Close,
    Interrupt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    StaleWindow,
    BoundHit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    Closed,
    Pending(Pending),
    Suspended { pending: Pending, reason: Refusal },
}

fn compose(pending: Pending, window: u8, basis: u8, response: Response) -> Outcome {
    if window != pending.window || basis != pending.basis {
        return Outcome::Suspended {
            pending,
            reason: Refusal::StaleWindow,
        };
    }
    if pending.remaining == 0 || pending.remaining > 2 || pending.depth > 2 {
        return Outcome::Suspended {
            pending,
            reason: Refusal::BoundHit,
        };
    }
    match response {
        Response::Close => Outcome::Closed,
        Response::Interrupt if pending.remaining == 2 && pending.depth < 2 => {
            Outcome::Pending(Pending {
                remaining: pending.remaining - 1,
                depth: pending.depth + 1,
                ..pending
            })
        }
        Response::Interrupt => Outcome::Suspended {
            pending,
            reason: Refusal::BoundHit,
        },
    }
}

fn main() {
    let initial = Pending {
        window: 7,
        basis: 3,
        remaining: 2,
        depth: 0,
        consumed_draws: 1,
        spent_resources: 1,
    };
    let next = Pending {
        remaining: 1,
        depth: 1,
        ..initial
    };
    assert_eq!(
        compose(initial, 7, 3, Response::Interrupt),
        Outcome::Pending(next)
    );
    assert_eq!(compose(next, 7, 3, Response::Close), Outcome::Closed);
    assert_eq!(
        compose(initial, 8, 3, Response::Close),
        Outcome::Suspended {
            pending: initial,
            reason: Refusal::StaleWindow,
        }
    );
    assert_eq!(
        compose(initial, 7, 4, Response::Close),
        Outcome::Suspended {
            pending: initial,
            reason: Refusal::StaleWindow,
        }
    );
    assert_eq!(
        compose(next, 7, 3, Response::Interrupt),
        Outcome::Suspended {
            pending: next,
            reason: Refusal::BoundHit,
        }
    );
    let at_depth_bound = Pending {
        depth: 2,
        ..initial
    };
    assert_eq!(
        compose(at_depth_bound, 7, 3, Response::Interrupt),
        Outcome::Suspended {
            pending: at_depth_bound,
            reason: Refusal::BoundHit,
        }
    );
    for invalid in [
        Pending {
            remaining: 0,
            ..initial
        },
        Pending {
            remaining: 3,
            ..initial
        },
        Pending {
            depth: 3,
            ..initial
        },
    ] {
        assert_eq!(
            compose(invalid, 7, 3, Response::Close),
            Outcome::Suspended {
                pending: invalid,
                reason: Refusal::BoundHit,
            }
        );
    }
    println!(
        "PASS: finite interrupt/close; stale window/basis; count/depth suspension retains pending"
    );
}
```

The assertions preserve the exact pending value on stale/bound refusal, including
already consumed draws/resources. Every return is a closed enum outcome, no recursive
call or unbounded collection exists, and admitted interrupt progress is finite.
The terminal close is structural; actual effect application, specific precedence,
resource costs, participants, byte/lifetime admission and scoped ruling recovery
must be implemented and tested against pinned source fixtures by their owners.

## Alternatives and next consumers

Reject a downloaded DSL, creator scripts or a public generic effect framework:
they exceed this task and bypass closed reviewed source handlers. Reject label-based
condition merges and blanket effect ordering: source exceptions would be lost.
Reject silently truncating trigger work at a bound or auto-closing on timeout:
unresolved legal work would disappear. Reject a duplicate engine pending store:
the engine/session ownership is already specified.

The next mechanical consumer is existing `B-C-df-rules-I01`, “Implement source-pinned
handler dispatch registry”, followed by `B-C-df-rules-W01`, “Connect R01-R17 canonical
handlers to resolver”, in `development/backlog-catalog.json`. It must attach the
reviewed source identity and explicit unsupported result to this composition.
The connecting consumer is existing `B-C-df-engine-D02`, “Define pending choice
reaction and ruling continuations”, then `B-C-df-engine-I03`, “Implement
pending-resolution resumption”. Those owners preserve this boundary in the engine's
single pending continuation; its existing [D02 decision](c-df-engine-d02-decision.md)
is a comparison boundary until coordinator approval. `df-session` supplies atomic
commit/recovery.
Their minimum eventual source paths are the approved private effect/trigger module
inside `crates/df-rules/src/` and pending-resumption module inside
`crates/df-engine/src/`, plus owning contract fixtures. Those crate paths are intended
implementation locations, not an assertion that modules already exist or permission
for this attempt to create them. Public shared types/codecs remain separate G03 and
consumer-owner work. Other drafts are comparison-only until approved.

## Original criteria and verification boundary

Original acceptance, preserved verbatim:

- pending windows remain closed and bounded
- The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.

Original verification, preserved verbatim:

- Freeze the cited source decision and a bounded contract example; compare pending windows remain closed and bounded. Retain decision, alternatives and unresolved facts.
- Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.

The bounded procedure extracts this sole Rust fence byte-for-byte, runs pinned
1.98.1 rustfmt write/check with root `rustfmt.toml`, compiles with pinned rustc
`--edition=2024 -Dwarnings`, then executes the finite assertions through the supplied
v6 guard. Evidence retains source/document/tool/config digests, exact argv, stdout,
stderr and guard receipts. Compiler/proof execution requires the coordinator's
explicit finite-check release. After the group02 release on 2026-10-02, the exact
literal was extracted and compared byte-for-byte, formatted and checked with pinned
rustfmt/root configuration, compiled as Rust 2024 with warnings denied, and executed
successfully. The assertions observed valid interrupt/close, stale window/basis,
count/depth exhaustion and invalid zero/oversized pending bounds, retaining the exact
pending value on refusal. No compiler or assertion repair was needed. The immutable
attempt handoff records actual source/tool/config/argv, binary identity and receipts.
These results establish this structural boundary; they do not qualify source mechanics.

Unresolved gates: exact required source/catalog/rights manifests; reviewed handler
variants and exceptional source precedence; G03 public types; actual operational
item/byte/depth/count/lifetime admission; source-linked condition/expiry/stacking/
concentration interaction fixtures; engine/session persistence/replay and permitted
projection. No Cargo, Clippy, WASM, browser, PostgreSQL, provider, audio or complete
standard-2024 qualification is claimed. Independent frontier review of the exact
internal fixture and integrated verification remain required under ADR 0005.
