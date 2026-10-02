# df-session actor ownership, lease fence, and revision decision

Task/attempt: `B-C-df-session-D01-a1`  
Input revision: `308920e328ba6df85bea1e3d2abcbea5f1b796e6`  
Owner: `df-session`; permitted path: this decision document only.

## Decision

`df-session` has one serialization actor per live session. The directory resolves a
session to its current owner; it is a router and does not serialize all sessions
behind a global lock. The actor is the sole publisher and committer of gameplay
decisions, membership-linked gameplay commands, timer outcomes, and asynchronous
job results. HTTP/RPC handlers, timers, workers, and directors submit messages to
that actor; they never mutate engine state or publish a candidate transition.

`df-persistence` implements the consumer-owned `SessionRepository` port in
`planning/subsystem-interfaces.md`. Its durable owner record is authoritative and
contains an opaque owner identity, monotonically increasing fencing token, and
lease expiry according to database time. Acquire/takeover is one conditional
durable operation: it succeeds only if no live lease prevents it, or a strictly
newer fence atomically supersedes the prior owner. Renewal and release condition
on both owner identity and fence; a stale owner cannot extend, release, or commit
after takeover. The exact schema, lease duration, clock-skew policy and query
implementation remain the G05 storage gate; this decision invents no measured
lease duration. A process-local mutex can serialize one actor's mailbox but is
never durable authority.

For a decision, the actor checks its current lease/fence and loaded revision,
validates the command and produces a pure candidate transition. In one database
transaction, `commit_decision` compares the supplied owner fence and expected
current revision with durable values, then writes the next session state,
operation result, ordered facts and required effect intents atomically. The
revision advances exactly once for a committed decision; state, receipt and
intents cannot split across transactions. A losing revision comparison means
reload and revalidate from current state, not retrying the old transition. The
client's `observed_revision` is context for offer validation; it is not the
server's write lock. Destructive host/debug commands retain their stricter
precondition. The decision uses the existing `SessionRevision` ordering from
`planning/typed-revision-policy.md`: verified recovery epoch first, then
in-epoch sequence, checked without wrapping. Only the protected native recovery
owner may establish a new epoch after verified disaster restore; ordinary actor
takeover does not advance it.

No success view, stream event, effect dispatch, or success receipt is emitted
before commit acknowledgment. After commit the same owner applies the committed
transition, publishes authorized detached snapshots and dispatches the recorded
intents. If the commit response is ambiguous, it looks up the stable operation
identity and reloads durable state before deciding what to report. It never
re-executes an uncertain operation merely because the caller timed out. A known
pre-commit storage failure returns a safe unavailable outcome; an ambiguous
commit returns an explicit unknown/pending outcome until lookup resolves it. A
stale fence, expired lease, or revision conflict rejects the write and suppresses
publication. If storage/quorum is unavailable, the actor cannot create a local
fallback authority. Process restart or takeover reloads durable state and starts
with a fresh actor generation; queued callbacks/jobs must match current session,
run, operation/job and timer-instance identities before they can propose work.
Accepted effect intents survive RPC cancellation and remain session/run owned.

The authority chain is therefore `SessionDirectory` routing -> current fenced
`df-session` actor -> `df-persistence` conditional durable commit -> authorized
projection and `df-server` effect execution. `df-engine` proposes transitions;
`df-rules` owns mechanics; pure directors propose staged changes. None commits
or publishes independently. API owns authentication and mapping, not a parallel
seat registry. The client renders a detached `ReadSnapshot` and its cursor; it
never chooses the authoritative revision or owner.

## Alternatives and rationale

A process-local lock alone fails across restart, split routing, or multiple server
instances. A directory-only lease without a fence leaves a paused former owner
able to resume and commit after takeover. A fence without expected-revision CAS
allows two writes from one still-current owner to overwrite each other. Global
session serialization would make unrelated sessions contend and does not improve
per-session ordering. Actor-per-session plus durable fencing and revision CAS
matches the existing repository boundary and keeps unrelated sessions independent.

## Finite contract example

This std-only model exercises exactly the chosen guard: only the live owner/fence
can commit against the current revision, each accepted commit advances revision
once, stale owners and expired leases refuse, and a same-owner stale revision
must reload. It is illustrative contract evidence, not a production API or proof
of PostgreSQL transaction behavior.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Owner(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Revision {
    epoch: u8,
    sequence: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Lease {
    owner: Owner,
    fence: u8,
    expires_at: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Expired,
    StaleFence,
    RevisionConflict,
    SequenceExhausted,
}

fn commit(
    lease: Lease,
    current: Revision,
    now: u8,
    supplied_owner: Owner,
    supplied_fence: u8,
    expected: Revision,
) -> Result<Revision, Refusal> {
    if now >= lease.expires_at {
        return Err(Refusal::Expired);
    }
    if supplied_owner != lease.owner || supplied_fence != lease.fence {
        return Err(Refusal::StaleFence);
    }
    if expected != current {
        return Err(Refusal::RevisionConflict);
    }
    let sequence = current
        .sequence
        .checked_add(1)
        .ok_or(Refusal::SequenceExhausted)?;
    Ok(Revision {
        epoch: current.epoch,
        sequence,
    })
}

fn main() {
    let old = Owner(1);
    let new = Owner(2);
    let lease = Lease {
        owner: new,
        fence: 2,
        expires_at: 10,
    };
    let revision = Revision {
        epoch: 3,
        sequence: 7,
    };

    assert_eq!(
        commit(lease, revision, 9, new, 2, revision),
        Ok(Revision {
            epoch: 3,
            sequence: 8,
        })
    );
    assert_eq!(
        commit(lease, revision, 9, old, 1, revision),
        Err(Refusal::StaleFence)
    );
    assert_eq!(
        commit(lease, revision, 10, new, 2, revision),
        Err(Refusal::Expired)
    );
    assert_eq!(
        commit(
            lease,
            revision,
            9,
            new,
            2,
            Revision {
                epoch: 3,
                sequence: 6,
            },
        ),
        Err(Refusal::RevisionConflict)
    );
    assert_eq!(
        commit(
            lease,
            Revision {
                epoch: 3,
                sequence: u8::MAX,
            },
            9,
            new,
            2,
            Revision {
                epoch: 3,
                sequence: u8::MAX,
            },
        ),
        Err(Refusal::SequenceExhausted)
    );
}
```

## Ownership, integration and open gates

`df-session` owns the actor protocol, admission/revalidation ordering, commit
receipt interpretation, stale callback suppression, and authorized publication.
`df-persistence` owns PostgreSQL owner lease/fence and conditional atomic
`commit_decision`, operation lookup, schema/migration and transaction behavior.
`df-server` owns native `EffectExecutor` workers and credentials; committed work
is dispatched only after durable acceptance. `df-api` authenticates/maps requests
and projects audience-safe snapshots; `df-protocol` owns wire schema/mapping;
`df-types` owns canonical revision values and comparison. Recovery epoch issuance,
protected head verification and lost-range reporting remain with the native
recovery/storage owner under G03/G05 and `service-operations.md`.

Production gates still open: durable lease schema/DB-time semantics, fencing
counter nonregression and owner failover, transaction isolation and ambiguous
commit resolution, operation-key tombstone retention, actor mailbox and shutdown
limits, multi-instance routing behavior, protected restore epoch integration,
telemetry checkpoints, generated wire compatibility, and restart/concurrency
fault tests. No production API, schema, lease duration, throughput bound, or
availability guarantee is frozen here. The isolated example proves only finite
pure guard behavior. Extraction, formatter, compiler and execution receipts are
retained under `development/evidence/fanout-20261001/wave-02/B-C-df-session-D01/worker/`
(`extract-01.json`, `rustfmt-check-01.json`, `rustc-01.json`, `run-01.json`), alongside
`input-hashes.json`; the extracted literal and this document are SHA-256 recorded
in `handoff.json`. Native/WASM workspace checks, PostgreSQL integration,
restart/failover, browser/device behavior, and independent frontier/integrated
review remain unperformed and mandatory at their owning gates.
