# Operation allocation and job identity retention

Task/attempt: `B-C-df-session-D02/a1`  
Decision input: `d870db00dc349425e8f977bdfb20c6ffb24bfd3e`  
Owner: `df-session`; this decision adds no production public type or schema.

Governing sources: [Subsystem architecture](subsystem-architecture.md) (Design,
Server crates, Integration and refinement), [Subsystem interfaces](subsystem-interfaces.md)
(Common contract rules, Identity and session ownership, PostgreSQL persistence and
durable media, AI/provider/media ports), [Runtime reliability](runtime-reliability.md)
(input acceptance, cancellation and recovery), [RPC API](rpc-api.md) (mutation,
allocation and lookup semantics), [Service operations](service-operations.md)
(Effect send boundary and uncertainty, Retention, deletion and provenance,
Nonregressing irreversible recovery journal, Composite revision after disaster restore),
and [Observability](observability.md) govern this boundary. Campaign authoring,
remote play, expansion boundaries and generated-content decisions retain the same
operation lookup, committed allocation and run-owned job rules. Workflow/style and
source fingerprints remain those frozen in the attempt brief, including
[AGENTS](../AGENTS.md), [coding style](coding-style.md) and ADRs
[0001](../ADR/0001-sqlite-agent-workflow.md),
[0002](../ADR/0002-resource-scheduling-and-cleanup.md),
[0003](../ADR/0003-agent-devlog.md),
[0004](../ADR/0004-development-reliability.md) and
[0005](../ADR/0005-frontier-output-evaluation.md).

## Decision

Allocate a stable operation identity before submitting an authoritative mutation
or allocation, retain it with the canonical request fingerprint, and reuse that
identity when resolving retries. The existing `df-types::OperationId` is the
canonical byte primitive; it does not issue unique identities or authorize access.
Issuance/collision policy and concrete namespace representations remain consumer
contract gates. Deduplication uses the authenticated principal, target session
where one exists, and the method's operation namespace. The fingerprint covers
run, command and selections; an identical key with a different fingerprint refuses
as OperationConflict. Trace IDs and transient connection IDs cannot replace the key.

Create uses principal/allocation-operation scope before a session exists; Join uses
principal/target-session/allocation-operation scope before membership exists.
BeginGuest uses its bounded bootstrap request scope before a principal exists.
Renew/Revoke and BindClient keep their credential/binding operation namespaces.
Their existing response shapes remain grants/revocation/binding results. The
allocation, authority/grant and result commit atomically under the existing owner
fence; a retry cannot allocate another identity, member, binding takeover or lease.
A refreshed authorized credential on lookup is not another allocation.

The current actor authenticates/scopes input and looks up the retained operation
before testing new offers against current state. A matching committed result can
therefore resolve an old request whose original basis is now stale. It returns only
the currently authorized projection. A new decision still requires current run,
binding/lease, owner fence and server revision checks. State, receipt, facts and
effect intents commit together through `SessionRepository::commit_decision`.
No success, view or effect dispatch precedes acknowledged durable acceptance.

A lost response, ambiguous commit, disconnect or RPC timeout/cancellation keeps the
original identity and fingerprint and invokes `lookup_operation`/GetOperation plus
durable reload. It does not reroll, regenerate, allocate a new operation to replay
the same uncertain intent, or dispatch effects from an unacknowledged candidate.
Committed, in-progress, not-recorded and expired/indeterminate are distinct outcomes.
In-progress waits/looks up; not-recorded alone does not permit automatic replay.
Unavailable storage leaves uncertainty explicit and prevents local fallback authority.

Retain the result through the advertised retry window. If private payload/results
are expired, redacted or unavailable, preserve permitted compact deduplication
evidence or permanently retire that namespace. Receipt loss never makes an old key
fresh. The service-operations candidate of 90-day receipt retention followed by
nonreplayable namespace tombstones is inherited as a candidate, not qualified here.
Disaster restore retires old unknown operation/allocation namespaces under the
protected recovery epoch; exact retained receipts remain lookup-only. Missing old
game data cannot prove an uncertain old allocation never happened.

The committed effect intent owns its stable job identity and originating operation
link. Retry/lookup returns that same pending identity instead of starting another
job. The actor consumes completions only after matching session, run, current actor
generation, originating operation and job identity; timer replacement additionally
requires its distinct timer instance. An obsolete result cannot publish or release
a newer job's busy state. A recovered worker may have a new generation while
retaining the durable job identity; admission rebinds that generation explicitly.
Receipt wait cancellation/disconnect does not cancel accepted work. Explicit
owner cancellation, run termination and shutdown govern job lifetime and terminal
outcomes. Completed/canceled job deduplication evidence remains nonreplayable while
its namespace admits retries; no concrete JobId representation is frozen here.

## Alternatives and rationale

A fresh key after timeout can duplicate a committed decision, allocation or paid
effect. Deduplication by payload alone collapses two deliberate identical actions.
A process-local cache fails on restart/takeover; forgetting a key after receipt
expiry allows old retry replay. Reusing OperationId as JobId erases distinct roles
and allows one operation's several effect intents to collide. The existing durable
session/repository/executor ownership resolves these cases without a second registry,
state authority or logging path.

## Bounded executable example

This private, std-only decision model imports unchanged canonical identities from
the assigned worktree. Numeric namespace, fingerprint, job-slot and generation
values are finite fixture labels, not production encodings, canonical hashing,
issuance or a public JobId. The one retained record is a bounded observation of
already committed storage; it neither commits nor allocates. Authentication and
tenant/audience scoping are supplied by the real auth/repository boundary; the
boolean models its access decision without treating MemberId as a credential.

```rust
#[path = "/Users/earlcameron/Desktop/dungeonflux/artifacts/worktrees/wave05_session_d02/crates/df-types/src/identity.rs"]
pub mod identity;

use identity::{MemberId, OperationId, RunId, SessionId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Key {
    session: SessionId,
    principal_fixture: MemberId,
    namespace: u8,
    operation: OperationId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Receipt {
    run: RunId,
    job_slot: u8,
}

#[derive(Clone, Copy)]
struct Record {
    key: Key,
    fingerprint: u8,
    receipt: Option<Receipt>,
}

#[derive(Debug, Eq, PartialEq)]
enum Resolution {
    Retained(Receipt),
    Conflict,
    ExpiredOrIndeterminate,
    Unresolved,
    Denied,
}

fn lookup(authorized: bool, key: Key, fingerprint: u8, record: Option<Record>) -> Resolution {
    if !authorized {
        return Resolution::Denied;
    }
    let Some(record) = record.filter(|record| record.key == key) else {
        return Resolution::Unresolved;
    };
    if record.fingerprint != fingerprint {
        return Resolution::Conflict;
    }
    match record.receipt {
        Some(receipt) => Resolution::Retained(receipt),
        None => Resolution::ExpiredOrIndeterminate,
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Next {
    Lookup(Key),
    Resolved(Receipt),
    Refuse,
}

fn after_ambiguous_response(key: Key) -> Next {
    Next::Lookup(key)
}

fn after_lookup(resolution: Resolution) -> Next {
    match resolution {
        Resolution::Retained(receipt) => Next::Resolved(receipt),
        Resolution::Conflict
        | Resolution::ExpiredOrIndeterminate
        | Resolution::Unresolved
        | Resolution::Denied => Next::Refuse,
    }
}

fn completion_is_current(
    expected_key: Key,
    expected_receipt: Receipt,
    expected_generation: u8,
    supplied_key: Key,
    supplied_receipt: Receipt,
    supplied_generation: u8,
) -> bool {
    expected_key == supplied_key
        && expected_receipt == supplied_receipt
        && expected_generation == supplied_generation
}

fn main() {
    let key = Key {
        session: SessionId::from_bytes(&[1; 16]).expect("validated fixture"),
        principal_fixture: MemberId::from_bytes(&[2; 16]).expect("validated fixture"),
        namespace: 3,
        operation: OperationId::from_bytes(&[4; 16]).expect("validated fixture"),
    };
    let receipt = Receipt {
        run: RunId::from_bytes(&[5; 16]).expect("validated fixture"),
        job_slot: 6,
    };
    let record = Record {
        key,
        fingerprint: 7,
        receipt: Some(receipt),
    };

    assert_eq!(after_ambiguous_response(key), Next::Lookup(key));
    for _ in 0..2 {
        assert_eq!(
            after_lookup(lookup(true, key, 7, Some(record))),
            Next::Resolved(receipt)
        );
    }
    assert_eq!(lookup(true, key, 8, Some(record)), Resolution::Conflict);
    assert_eq!(lookup(false, key, 7, Some(record)), Resolution::Denied);
    assert_eq!(after_lookup(lookup(true, key, 7, None)), Next::Refuse);
    assert_eq!(
        lookup(
            true,
            key,
            7,
            Some(Record {
                receipt: None,
                ..record
            })
        ),
        Resolution::ExpiredOrIndeterminate
    );
    assert_eq!(
        after_lookup(lookup(
            true,
            key,
            7,
            Some(Record {
                receipt: None,
                ..record
            })
        )),
        Next::Refuse
    );
    let retired_key = Key {
        namespace: 2,
        ..key
    };
    assert_eq!(
        after_lookup(lookup(true, retired_key, 7, Some(record))),
        Next::Refuse
    );
    let other_principal = Key {
        principal_fixture: MemberId::from_bytes(&[9; 16]).expect("validated fixture"),
        ..key
    };
    assert_eq!(
        lookup(true, other_principal, 7, Some(record)),
        Resolution::Unresolved
    );
    assert!(completion_is_current(key, receipt, 10, key, receipt, 10));
    assert!(!completion_is_current(key, receipt, 10, key, receipt, 9));
    assert!(!completion_is_current(
        key,
        receipt,
        10,
        key,
        Receipt {
            job_slot: 8,
            ..receipt
        },
        10,
    ));
    assert!(!completion_is_current(
        key,
        receipt,
        10,
        key,
        Receipt {
            run: RunId::from_bytes(&[8; 16]).expect("validated fixture"),
            ..receipt
        },
        10,
    ));
    assert!(!completion_is_current(
        key,
        receipt,
        10,
        Key {
            operation: OperationId::from_bytes(&[8; 16]).expect("validated fixture"),
            ..key
        },
        receipt,
        10,
    ));
    println!(
        "PASS: ambiguous lookup retains one receipt/job; conflict, expiry, absence, scope and stale completion refuse"
    );
}
```

The model has no submit/dispatch operation after ambiguity or unresolved lookup.
It proves only this finite branch boundary and identity comparisons, not SQL
atomicity, uniqueness, retention enforcement, authentication, cancellation,
concurrency or external exactly-once delivery.

## Next consumer and unresolved gates

The next source consumer is the existing backlog item `B-C-df-session-I02`,
Implement durable operation submission (atomic commit precedes confirmed receipt),
through the `df-session` actor established by `B-C-df-session-I01`. At this input,
`crates/df-session/` does not yet exist. Its minimum proposed entry path is
`crates/df-session/src/lib.rs`; its implementation brief must freeze any private
module layout before editing. The only existing Rust source consumed here is
`crates/df-types/src/identity.rs`; no additional identity primitive file is needed.
The consumer owns `SessionRepository::lookup_operation/commit_decision` and
`EffectExecutor` ports; `df-persistence` implements atomic allocation/receipt/intents,
lookup and retention; `df-server` supplies bounded native job execution.
`df-api` maps authorized lookup/receipt outcomes and `df-client` keeps the original
operation while resolving ambiguity. Those adapters and generated wire consumers
are separately frozen implementation work, not delivered by this document.

Open gates: reviewed operation issuer/collision handling, method/tenant/principal/
recovery namespace types, canonical fingerprint/codec versions, concrete job types
and tombstone policy, PostgreSQL transactions and ambiguous lookup consistency,
recovery-journal namespace retirement, executor completion fencing and job
reconciliation, finite store/mailbox/deadline limits, and restart/restore/concurrent
fault tests. Runtime observability uses shared df-observe checkpoints for admission,
commit/ambiguity/lookup, dispatch, stale completion and authorized publication with
source/build context; private payloads and credentials stay out of default logs.
The model's CLI PASS line is test output, not a competing application logger.

## Original criteria and evidence status

Acceptance, preserved verbatim:

- `ambiguous response is lookup not replay`
- `The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.`

Original verification, preserved verbatim:

- `Freeze the cited source decision and a bounded contract example; compare ambiguous response is lookup not replay. Retain decision, alternatives and unresolved facts.`
- `Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.`

After the root's group03 release, this exact Rust literal passed extraction and
byte comparison, pinned Rust 1.98.1 rustfmt write/check with root rustfmt.toml,
Rust2024 compilation with warnings denied against the unchanged canonical identity
source, and execution of its finite valid/refusal assertions. Compiler/proof commands
used the unchanged issued v6 guard at actual 40% admission; no HOLD, cap, compiler
failure or assertion repair occurred. Mechanical formatter changes alone were
synchronized back into this literal; the pre-format source is retained.

Receipts are retained under
`development/evidence/fanout-20261001/wave-03/B-C-df-session-D02/a1/`: `extract-01.json`,
`sync-literal-01.json`, `rustfmt-write-01.json`, `rustfmt-check-01.json`,
`rustc-01.json` and `run-01.json`. The final handoff binds the decision, exact
literal, canonical source, tool/config and tested binary hashes. The earlier
source-only handoff remains unchanged. Cargo, Clippy, native/WASM workspace,
PostgreSQL, actor runtime, browser/audio/provider, independent frontier review
and integrated revision checks remain unperformed; this bounded design evidence
cannot qualify those boundaries.
