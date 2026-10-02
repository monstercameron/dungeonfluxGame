# Commerce transition and native facade boundary

Task/attempt: `B-C-df-commerce-D01/a1`  
Owner: `df-commerce`  
Status: design decision; production types, adapters, and customer behavior remain pending.

## Decision

`df-commerce` is the sole policy authority for customer, tenant, entitlement,
payment-observation, reservation, and ledger decisions. Its pure
`CommerceTransition` calculation receives validated current state, a typed
command or already verified observation, and any authoritative timestamps as
explicit values. It returns a bounded candidate transition and diagnostic facts;
it does not mutate a repository or perform work. The crate depends only on
`df-types`. It has no socket, network, clock, provider, database, filesystem,
telemetry SDK, or secret-bearing global context. Domain wire and storage codecs
remain consumer concerns.

`CommerceRepository` and `PaymentGateway` are ports defined at the commerce
boundary. `df-persistence` implements transactional repository writes and
`df-providers` implements gateway calls. `df-server` owns the native
`CommerceService` facade: it composes those adapters, claims durable outbox work,
performs I/O, maps observations into policy inputs, and returns typed outcomes to
`df-api`. It also composes the existing `BudgetStore` adapter for `df-media` and
`df-ai`; that port delegates to commerce's reservation authority rather than
creating another ledger. `df-auth` owns authorization and its consumer-side
`EntitlementReader`; `df-session` and browser crates do not import commerce.

The transition is a proposal until `CommerceRepository::transact` commits the
expected revision, idempotent result, ledger/state/grant changes, and outbox
atomically. Only a known commit is success. A conflict or stale revision is a
typed rejection; permission and validation failures remain distinct. If commit
status is ambiguous, the facade reports pending/unknown and looks up the same
operation key; it never fabricates success or retries a non-idempotent external
effect. A verified gateway observation may advance payment/entitlement policy,
but a webhook is not trusted until its native verifier has checked raw bytes,
signature, endpoint version, and durable inbox receipt. Unknown charge, refund,
or dispatch keeps its maximum liability reserved until reconciliation. Expiry
alone does not release it. Failure to establish the protected journal boundary
holds credit/admission/deletion acknowledgement as specified by the recovery
policy.

The finite Rust example below illustrates this separation using local types. Its
names and fields are explanatory, not a production signature or an additional
authority. It models only a narrow supplied payment observation; it does not
prove authentication, event verification, idempotent persistence, transaction
isolation, gateway delivery, entitlement scope, or runtime behavior.

## Alternatives and ownership

Putting gateway, clock, or database calls inside `df-commerce` was rejected:
that would make policy nondeterministic, couple the domain crate to runtime
choices, and blur the existing repository/provider owners. A second server-side
ledger or a `BudgetStore` implementation that independently accounts usage was
rejected because reservations, liabilities, and ledger entries need one
authority. Putting the facade in `df-api` was rejected because it would make an
RPC handler own composition and external I/O; `df-api` calls the native facade
provided by `df-server`. Putting identity checks in commerce was rejected
because `df-auth` is the identity and capability authority.

The crate child owns eventual primitive type implementation, the commerce rules
child owns source behavior, this feature child owns this named use-case adapter,
and the slice child owns the cross-system connection and end-to-end proof. This
decision defines the boundary only; no second implementation or consumer API is
authorized. Existing `df-types` values and reviewed contracts are reused once
their owners freeze them.

## Illustrative executable contract

This stand-alone example accepts only a verified, current, settled initial
payment observation. It explicitly refuses unverified, stale, non-settled, and
wrong-state inputs. Time is supplied as a value; the function reads no clock,
network, provider, or database and produces a candidate rather than committing
it.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Access {
    PendingInitial,
    Active,
    NoAccess,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct State {
    access: Access,
    revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Observation {
    verified: bool,
    settled: bool,
    expected_revision: u64,
    paid_through: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Unverified,
    Unsettled,
    StaleRevision,
    WrongState,
    InvalidPaidThrough,
    RevisionOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CommerceTransition {
    before_revision: u64,
    next: State,
    paid_through: u64,
}

fn transition(state: State, observation: Observation) -> Result<CommerceTransition, Refusal> {
    if !observation.verified {
        return Err(Refusal::Unverified);
    }
    if !observation.settled {
        return Err(Refusal::Unsettled);
    }
    if observation.expected_revision != state.revision {
        return Err(Refusal::StaleRevision);
    }
    if state.access != Access::PendingInitial {
        return Err(Refusal::WrongState);
    }
    if observation.paid_through == 0 {
        return Err(Refusal::InvalidPaidThrough);
    }
    let revision = state
        .revision
        .checked_add(1)
        .ok_or(Refusal::RevisionOverflow)?;
    Ok(CommerceTransition {
        before_revision: state.revision,
        next: State {
            access: Access::Active,
            revision,
        },
        paid_through: observation.paid_through,
    })
}

fn main() {
    let pending = State {
        access: Access::PendingInitial,
        revision: 7,
    };
    let settled = Observation {
        verified: true,
        settled: true,
        expected_revision: 7,
        paid_through: 90,
    };
    let accepted = transition(pending, settled);
    assert_eq!(
        accepted,
        Ok(CommerceTransition {
            before_revision: 7,
            next: State {
                access: Access::Active,
                revision: 8,
            },
            paid_through: 90,
        })
    );
    assert_eq!(pending.access, Access::PendingInitial);
    assert_eq!(
        transition(
            pending,
            Observation {
                verified: false,
                ..settled
            }
        ),
        Err(Refusal::Unverified)
    );
    assert_eq!(
        transition(
            pending,
            Observation {
                settled: false,
                ..settled
            }
        ),
        Err(Refusal::Unsettled)
    );
    assert_eq!(
        transition(
            pending,
            Observation {
                expected_revision: 6,
                ..settled
            }
        ),
        Err(Refusal::StaleRevision)
    );
    assert_eq!(
        transition(
            State {
                access: Access::NoAccess,
                ..pending
            },
            settled,
        ),
        Err(Refusal::WrongState)
    );
    assert_eq!(
        transition(
            pending,
            Observation {
                paid_through: 0,
                ..settled
            }
        ),
        Err(Refusal::InvalidPaidThrough)
    );
    assert_eq!(
        transition(
            State {
                revision: u64::MAX,
                ..pending
            },
            Observation {
                expected_revision: u64::MAX,
                ..settled
            },
        ),
        Err(Refusal::RevisionOverflow)
    );
}
```

## Open production gates and checks

G01 must pin workspace/toolchain and native/WASM checks; G03 must freeze shared
IDs, observation, transition, error, and grant contracts. G04/G05/G06/G08 and
X12 still govern identity, durable schema/recovery, telemetry, provider/price
qualification, and paid-service readiness. Exact gateway identity verification,
webhook durability/order/replay, repository transaction and idempotency
retention, journal-provider qualification, account recovery/privacy, quote and
currency rules, customer UI, and live end-to-end behavior remain unimplemented
and unverified. The illustrative `u64` timestamp/revision choices are bounded
example values, not measured production limits or committed field types.

For this design attempt the source-backed policy and literal fixture are the
only checks in scope. Exact formatter/compiler commands, outputs, source/config
and binary hashes are retained in `handoff.json`; native/WASM crate builds,
Clippy, provider and PostgreSQL qualification, browser/phone testing, and
independent frontier evaluation remain unperformed. The transition itself
contains no socket, clock read, provider call, or database I/O.

## Governing sources

- [Implementation roadmap](implementation-roadmap.md): ownership, gates, and
  bounded delivery order.
- [Commerce service](commerce-service.md): independent authority, lifecycle,
  payment observations, exact spend, and failure semantics.
- [Subsystem architecture](subsystem-architecture.md) and
  [subsystem interfaces](subsystem-interfaces.md): crate edges and consumer
  ports.
- [Shared contract waves](shared-contract-waves.md),
  [typed units policy](typed-units-policy.md), and
  [protected journal policy](protected-journal-policy.md): later contract
  ownership, exact values, and fail-closed recovery.
- [Coding style](coding-style.md) and [ADR 0001](../ADR/0001-sqlite-agent-workflow.md),
  [ADR 0002](../ADR/0002-resource-scheduling-and-cleanup.md),
  [ADR 0003](../ADR/0003-agent-devlog.md),
  [ADR 0004](../ADR/0004-development-reliability.md),
  [ADR 0005](../ADR/0005-frontier-output-evaluation.md): source discipline,
  execution bounds, evidence, and independent review.
