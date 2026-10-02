# df-persistence D02: transaction fence and unique operation/intents

Date: 2026-10-02
Status: B-C-df-persistence-D02/a1 bounded DESIGN submission; independent review pending.
Input: `4c0e050c2d30e632bd3b32c51a92cfda52479e6e`.

The next `df-persistence` adapter must return a durable `DecisionReceipt` only after
PostgreSQL commits the decision. A bounded actor queue may acknowledge admission,
but that acknowledgment cannot be converted into accepted gameplay or effect work.
This decision freezes the minimum ordering and key requirements; it does not add a
repository implementation, public type, migration, database service or production stub.

## Sources and owners

[Subsystem interfaces](subsystem-interfaces.md), “Identity and session ownership”
and “PostgreSQL persistence and durable media”, owns `SessionRepository` and the
commit path. [Storage architecture](storage-architecture.md), “PostgreSQL planning
requirements” and “Runtime director state and replay”, requires coherent state,
ordered facts, operation results and intents. [RPC API](rpc-api.md), operation
identity/receipts and “Composite revision”, requires exact retry lookup before
stale-basis validation and forbids replay after uncertainty or namespace retirement.
[Runtime reliability](runtime-reliability.md) owns run/job/callback fencing.
[Service operations](service-operations.md), “Effect send boundary and uncertainty”,
“Tenant isolation and hostile input”, “Nonregressing irreversible recovery journal”
and “Composite revision after disaster restore”, governs lease expiry, native egress,
trusted tenant scope and restore. [Commerce service](commerce-service.md),
“Atomic hierarchical spend and concurrency”, retains the single ledger authority.

The source-backed authoring, long-horizon state and cinematic records use these same
fenced commits; summaries, imported content and telemetry never supply authority.
The architecture and backlog's C-df-persistence/B-C-df-persistence-D02 identify this
adapter owner. Coding style, observability, AGENTS and ADR 0001–0005 govern bounded
work, evidence and review. Frozen hashes are retained with the attempt brief.

`df-session` calls the existing consumer-owned `SessionRepository::commit_decision`
and applies/publishes only the confirmed result. `df-persistence` implements the
transaction and lookups. `df-model` owns checkpoint/state encoding; `df-engine`
owns pure staged transitions. `df-commerce` owns grant, reservation, exact accounting
and unknown liability through `CommerceRepository`; persistence must not create a
second wallet. `df-server` owns registered executors and the native egress dispatcher.
The minimum next source paths are `crates/df-session/src/` (port/caller),
`crates/df-persistence/src/` and its reviewed migrations (adapter/constraints), and
the existing `crates/df-types/src/revision.rs`. Those absent source directories and
concrete port signatures remain separately approved implementation work.

Wave D01 comparison aligns tenant-aware physical mappings and full-u64 numeric
epoch/sequence columns; D03 aligns transaction-local trusted tenant context and
pool reset. Model D01 owns recovery envelope compatibility. These draft decisions
are comparison inputs until approved; this document does not adopt their public APIs.

## Minimum PostgreSQL transaction

1. Authenticate and bind tenant/principal/session outside correlation metadata.
   Begin a bounded transaction with D03's trusted tenant scope. Lock the existing
   scoped session row with `SELECT ... FOR UPDATE`, in the adapter's common order.
   Operation lookup and all writes retain explicit scope predicates and RLS.
2. Look up `(tenant, session, principal, command_namespace, recovery_epoch,
   operation_id)`. Same key and canonical request fingerprint returns the stored
   receipt without a new revision, draw, allocation or intent. A changed fingerprint
   is `OperationConflict`. Retired keys return retained permitted receipt or
   expired/indeterminate; absent payload never makes an old key executable again.
   Lookup of a retained receipt is read-only and independently authorized even
   after owner/run/basis changes. It is not a permission to publish or dispatch.
3. For a new operation, check the current run, owner-fence token and strictly
   unexpired lease using the database clock. Check the server's expected composite
   `SessionRevision`; a client observed revision is not this server CAS. Destructive
   commands additionally check their strict client precondition. Reject stale basis,
   obsolete namespace/run or expired owner before writing. Revalidation belongs to
   the actor, not an adapter loop that changes the candidate's expected revision.
4. Within that same transaction, persist the staged coherent versioned state,
   ordered facts/decision/draws, immutable
   operation fingerprint/result, and all required intent identities/payloads.
   The final conditional session update advances the revision only when trusted
   scope, current run/fence, expected epoch/sequence and `lease_until >
   clock_timestamp()` still match in PostgreSQL. A transaction-start timestamp or
   cached actor clock cannot satisfy this expiry check. The common session lock
   excludes takeover through commit; CAS is the transaction's ownership check,
   not a claim that WAL commit latency cannot cross a wall-clock lease boundary.
   Require exactly one updated session row; zero is a refusal and rolls back.
   Sequence overflow refuses; this adapter cannot issue a new recovery epoch.
   Any later insert/storage/constraint failure rolls back the entire set.
5. Commit with the reviewed durable configuration and require confirmed success.
   Only then expose the receipt to the actor, which applies and publishes permitted
   views and schedules committed intents. Transaction acquisition, execution and
   receipt wait have finite bounds; no provider calls or client waits occur inside
   the transaction. Lost receipt/caller cancellation cannot undo committed work.

Required constraints use non-null key columns and tenant-aware foreign references:
unique operation key above; unique ordered fact position `(tenant, session,
committed_epoch, committed_sequence, ordinal)`; unique intent slot `(tenant,
session, principal, command_namespace, recovery_epoch, operation_id, slot)` plus
unique stable effect/job identity. Slots are deterministic within the staged
decision, not a new ID on every retry. Run/generation, execution mode, source
versions and reservation linkage are immutable intent payload/binding data.
Duplicate slots or conflicting stable identities roll back; never use a blind
upsert that replaces another result or intent. Physical DDL and bounds are a later
migration gate. Allocation before session existence uses the already defined
principal/allocation-operation or scoped bootstrap namespace and atomically stores
allocation, grant and result; it cannot use a nonexistent session lock as protection.
Its unique key and transaction are independently required at the auth adapter gate.

The chosen session path uses one predetermined locked row plus conditional CAS and
unique inserts at Read Committed; every competing session commit must use that row.
PostgreSQL rechecks an updated row's predicate after waiting, while row locks block
competing writers until transaction end. This supports the proposed single-session
path; it does not qualify multirow business invariants.
[Transaction isolation](https://www.postgresql.org/docs/18/transaction-iso.html) and
[Explicit locking](https://www.postgresql.org/docs/18/explicit-locking.html), checked
2026-10-02. Non-null composite unique constraints are the database backstop, not
process locks. [CREATE TABLE](https://www.postgresql.org/docs/18/sql-createtable.html).
The commerce transaction retains its separately mandated serializable/explicit-lock
hierarchical admission and stable lock order; unrelated counters cannot be protected
by the session row alone. Cross-port atomic linkage and lock order need the commerce
owner's approved implementation. A candidate requiring paid work cannot be accepted
unless current grant validation, hierarchical reservation and accepted intent linkage
all participate in the same transaction with the agreed common lock order; otherwise
admission refuses. No second admission result is synthesized here.

Durability qualification must verify `fsync`, WAL and `synchronous_commit` policy;
local commit confirmation is not a replicated-HA or disaster-RPO-zero promise.
Asynchronous commit can acknowledge before WAL reaches durable storage, so it is
incompatible with claiming this acceptance boundary without an explicitly revised
promise. [WAL settings](https://www.postgresql.org/docs/18/runtime-config-wal.html).
Protected journal confirmation remains additionally required before irreversible
paid egress under service operations; a game receipt is not an egress permit.

## Failure and uncertainty

| Observed boundary | Required result |
| --- | --- |
| Enqueue accepted | Queued/in-progress only; no receipt, state publication or dispatch |
| Fence/run/CAS/duplicate refusal or known rollback | Typed refusal/failure; no staged writes become visible |
| Confirmed commit | Durable receipt, possibly with explicitly pending jobs |
| Commit response lost | Unknown; do not apply, publish success or dispatch from the uncertain candidate |
| Lookup confirms committed operation | Return same authorized receipt; current actor recovers state/intents before publication |
| Lookup absent after ambiguous commit | Unresolved, not permission to replay with a new ID; resolve transaction/retention/recovery status |
| Crash after commit before actor apply | Recover persisted revision/state/result/intents; no recomputation or second decision |
| Disaster restore | Verified greater epoch, old namespaces lookup-only/retired, explicit lost game range |

Committed domain rejections are immutable operation results too, while pre-admission
storage/auth/capacity failures are not fabricated committed decisions. Serialization
or deadlock failure with confirmed rollback may be revalidated under the same key
within an approved finite retry policy; an ambiguous commit never takes that path.
Dispatch claim remains a separate durable commerce/effect boundary: it checks the
current owner, reservation, grant and dispatcher ownership, records Dispatching and
stable provider key, then native egress sends. Unknown liability persists even after
lease expiry; unique intent rows cannot promise external exactly-once delivery.
Typed stale/error/unknown checkpoints flow through shared OTEL with operation,
run, revision and source/build context, without credentials or private payloads.

Alternatives rejected: queue success as receipt loses writes on failure; a process
mutex cannot fence another instance; separate state/result/intent commits allow
partial acceptance; blind `ON CONFLICT` replacement hides payload conflict;
event-sourcing redesign is unnecessary for the existing snapshot recovery spine.

## Executable finite ordering example

The literal imports the actual worktree `SessionRevision`; the check driver copies
that source byte-for-byte beside the extracted file. All other labels, fingerprints,
lease clock and values are private synthetic fixture data. `ModeledCommitted` is a
finite ledger transition, **not a durable PostgreSQL acceptance receipt**. No SQL,
database, concurrent processes, executor, provider, filesystem durability or game
rules are exercised. Fault cuts model a rollback and both possibilities of a lost
commit response. The assertions exercise enqueue, commit-before-publication, two
same-basis writers before actor apply, duplicate/conflicting operations and intent
slots, expired/stale fences, overflow, run/epoch retirement, tenant/principal key
isolation, committed rejection and recovery after uncertain commit.

```rust
#[path = "revision.rs"]
mod revision;

use revision::{RecoveryEpoch, SessionRevision};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OperationKey {
    tenant: u8,
    session: u8,
    principal: u8,
    namespace: u8,
    epoch: RecoveryEpoch,
    operation: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Decision {
    AcceptedPending,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Candidate {
    key: OperationKey,
    fingerprint: u64,
    expected: SessionRevision,
    fence: u8,
    run: u8,
    value: u8,
    slots: [u8; 2],
    decision: Decision,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Receipt {
    key: OperationKey,
    fingerprint: u64,
    revision: SessionRevision,
    decision: Decision,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    Scope,
    Conflict,
    Retired,
    Fence,
    Run,
    Stale,
    DuplicateIntent,
    Overflow,
    Storage,
}

#[derive(Debug, PartialEq, Eq)]
enum Reply {
    Queued,
    ModeledCommitted(Receipt),
    Unknown,
}

#[derive(Clone, Copy)]
enum Fault {
    None,
    BeforeCommit,
    LostReply { persisted: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Ledger {
    revision: SessionRevision,
    value: u8,
    results: Vec<Receipt>,
    intents: Vec<(OperationKey, u8)>,
}

struct Owner {
    tenant: u8,
    session: u8,
    fence: u8,
    lease_until: u64,
    run: u8,
    ledger: Ledger,
    published: Vec<SessionRevision>,
    dispatched: Vec<(OperationKey, u8)>,
}

impl Owner {
    fn enqueue(&self, _candidate: Candidate) -> Reply {
        Reply::Queued
    }

    fn lookup(&self, candidate: Candidate) -> Result<Option<Receipt>, Refusal> {
        if candidate.key.tenant != self.tenant || candidate.key.session != self.session {
            return Err(Refusal::Scope);
        }
        if let Some(receipt) = self.ledger.results.iter().find(|r| r.key == candidate.key) {
            if receipt.fingerprint != candidate.fingerprint {
                return Err(Refusal::Conflict);
            }
            return Ok(Some(*receipt));
        }
        if candidate.key.epoch != self.ledger.revision.epoch() {
            return Err(Refusal::Retired);
        }
        Ok(None)
    }

    fn commit(&mut self, candidate: Candidate, now: u64, fault: Fault) -> Result<Reply, Refusal> {
        if let Some(receipt) = self.lookup(candidate)? {
            return Ok(Reply::ModeledCommitted(receipt));
        }
        if candidate.fence != self.fence || now >= self.lease_until {
            return Err(Refusal::Fence);
        }
        if candidate.run != self.run {
            return Err(Refusal::Run);
        }
        if candidate.expected != self.ledger.revision {
            return Err(Refusal::Stale);
        }
        if candidate.slots[0] == candidate.slots[1] {
            return Err(Refusal::DuplicateIntent);
        }
        let revision = self
            .ledger
            .revision
            .next_sequence()
            .map_err(|_| Refusal::Overflow)?;
        let receipt = Receipt {
            key: candidate.key,
            fingerprint: candidate.fingerprint,
            revision,
            decision: candidate.decision,
        };
        let mut staged = self.ledger.clone();
        staged.revision = revision;
        staged.results.push(receipt);
        if candidate.decision == Decision::AcceptedPending {
            staged.value = candidate.value;
            staged
                .intents
                .extend(candidate.slots.map(|slot| (candidate.key, slot)));
        }
        match fault {
            Fault::BeforeCommit => Err(Refusal::Storage),
            Fault::LostReply { persisted } => {
                if persisted {
                    self.ledger = staged;
                }
                Ok(Reply::Unknown)
            }
            Fault::None => {
                self.ledger = staged;
                Ok(Reply::ModeledCommitted(receipt))
            }
        }
    }

    // This synthetic consumer records publication only after ledger confirmation.
    // Real publication additionally requires current actor, run and audience checks.
    fn consume(&mut self, reply: &Reply) {
        if let Reply::ModeledCommitted(receipt) = reply {
            if self.ledger.results.contains(receipt)
                && receipt.revision == self.ledger.revision
                && !self.published.contains(&receipt.revision)
            {
                self.published.push(receipt.revision);
                self.dispatched.extend(
                    self.ledger
                        .intents
                        .iter()
                        .copied()
                        .filter(|(key, _)| *key == receipt.key),
                );
            }
        }
    }
}

fn fixture() -> (Owner, Candidate) {
    let epoch = RecoveryEpoch::new(7).expect("finite nonzero fixture epoch");
    assert_eq!(epoch.get(), 7);
    let revision = SessionRevision::new(epoch, 4);
    assert_eq!(revision.sequence(), 4);
    let owner = Owner {
        tenant: 1,
        session: 2,
        fence: 3,
        lease_until: 20,
        run: 4,
        ledger: Ledger {
            revision,
            value: 0,
            results: vec![],
            intents: vec![],
        },
        published: vec![],
        dispatched: vec![],
    };
    let candidate = Candidate {
        key: OperationKey {
            tenant: 1,
            session: 2,
            principal: 5,
            namespace: 1,
            epoch,
            operation: 9,
        },
        fingerprint: 42,
        expected: revision,
        fence: 3,
        run: 4,
        value: 1,
        slots: [0, 1],
        decision: Decision::AcceptedPending,
    };
    (owner, candidate)
}

fn main() {
    let (mut owner, candidate) = fixture();
    let before = owner.ledger.clone();
    let queued = owner.enqueue(candidate);
    assert_eq!(queued, Reply::Queued);
    owner.consume(&queued);
    assert_eq!(owner.ledger, before);
    assert!(owner.published.is_empty() && owner.dispatched.is_empty());
    assert_eq!(
        owner.commit(candidate, 10, Fault::BeforeCommit),
        Err(Refusal::Storage)
    );
    assert_eq!(owner.ledger, before);
    let committed = owner
        .commit(candidate, 10, Fault::None)
        .expect("fixture commit");
    assert!(owner.published.is_empty() && owner.dispatched.is_empty());
    let after = owner.ledger.clone();
    let competitor = Candidate {
        key: OperationKey {
            operation: 10,
            ..candidate.key
        },
        ..candidate
    };
    assert_eq!(
        owner.commit(competitor, 10, Fault::None),
        Err(Refusal::Stale)
    );
    assert_eq!(owner.ledger, after);
    owner.consume(&committed);
    assert_eq!(owner.published, vec![after.revision]);
    assert_eq!(owner.dispatched.len(), 2);
    owner.consume(&committed);
    assert_eq!(owner.published.len(), 1);
    assert_eq!(owner.dispatched.len(), 2);
    owner.fence = 8;
    owner.run = 8;
    assert_eq!(owner.commit(candidate, 30, Fault::None), Ok(committed));
    assert_eq!(owner.ledger, after);
    let changed = Candidate {
        fingerprint: 43,
        ..candidate
    };
    assert_eq!(
        owner.commit(changed, 10, Fault::None),
        Err(Refusal::Conflict)
    );

    for refused in [
        Candidate {
            fence: 9,
            ..candidate
        },
        Candidate {
            run: 9,
            ..candidate
        },
        Candidate {
            expected: candidate.expected.next_sequence().expect("fixture next"),
            ..candidate
        },
        Candidate {
            slots: [1, 1],
            ..candidate
        },
        Candidate {
            key: OperationKey {
                tenant: 9,
                ..candidate.key
            },
            ..candidate
        },
    ] {
        let (mut owner, _) = fixture();
        let original = owner.ledger.clone();
        assert!(owner.commit(refused, 10, Fault::None).is_err());
        assert_eq!(owner.ledger, original);
        assert!(owner.published.is_empty() && owner.dispatched.is_empty());
    }
    let (mut owner, _) = fixture();
    assert_eq!(
        owner.commit(candidate, 20, Fault::None),
        Err(Refusal::Fence)
    );
    owner.ledger.revision = SessionRevision::new(candidate.key.epoch, u64::MAX);
    let overflow = Candidate {
        expected: owner.ledger.revision,
        ..candidate
    };
    assert_eq!(
        owner.commit(overflow, 10, Fault::None),
        Err(Refusal::Overflow)
    );

    for persisted in [false, true] {
        let (mut owner, _) = fixture();
        let unknown = owner
            .commit(candidate, 10, Fault::LostReply { persisted })
            .expect("fault cut");
        assert_eq!(unknown, Reply::Unknown);
        owner.consume(&unknown);
        assert!(owner.published.is_empty() && owner.dispatched.is_empty());
        let found = owner.lookup(candidate).expect("authorized lookup");
        assert_eq!(found.is_some(), persisted);
        if let Some(receipt) = found {
            let same = owner.ledger.clone();
            assert_eq!(
                owner.commit(candidate, 10, Fault::None),
                Ok(Reply::ModeledCommitted(receipt))
            );
            assert_eq!(owner.ledger, same);
            owner.consume(&Reply::ModeledCommitted(receipt));
            assert_eq!(owner.published.len(), 1);
        }
        // Absent lookup is deliberately unresolved; this branch does not retry.
    }

    let (mut owner, _) = fixture();
    owner
        .commit(candidate, 10, Fault::None)
        .expect("fixture original");
    let other_principal = Candidate {
        key: OperationKey {
            principal: 6,
            ..candidate.key
        },
        expected: owner.ledger.revision,
        decision: Decision::Rejected,
        ..candidate
    };
    let rejected = owner
        .commit(other_principal, 10, Fault::None)
        .expect("recorded rejection");
    assert!(matches!(
        rejected,
        Reply::ModeledCommitted(Receipt {
            decision: Decision::Rejected,
            ..
        })
    ));
    assert_eq!(owner.ledger.results.len(), 2);
    assert_eq!(owner.ledger.intents.len(), 2);
    assert_eq!(owner.ledger.value, 1);
    let (mut other_tenant, _) = fixture();
    other_tenant.tenant = 2;
    let scoped = Candidate {
        key: OperationKey {
            tenant: 2,
            ..candidate.key
        },
        ..candidate
    };
    assert!(other_tenant.commit(scoped, 10, Fault::None).is_ok());
    assert_eq!(owner.lookup(scoped), Err(Refusal::Scope));

    let newer = RecoveryEpoch::new(8).expect("finite verified-head stand-in");
    owner.ledger.revision = SessionRevision::new(newer, 0);
    assert!(owner.ledger.revision > candidate.expected);
    assert!(owner.lookup(candidate).expect("retained lookup").is_some());
    assert_eq!(
        owner.commit(competitor, 10, Fault::None),
        Err(Refusal::Retired)
    );
    assert!(RecoveryEpoch::new(0).is_err());
    println!(
        "PASS finite enqueue/ordering/CAS/duplicate/fence/rollback/unknown/retirement cases; no database durability claim"
    );
}
```

## Reviewable result and remaining qualification

The finite fixture is the executable bounded contract example required by the
original DESIGN criteria, not implementation acceptance. Retained receipts bind
the literal, shared revision source, root format configuration, pinned tool binaries,
argv, stdout/stderr, source revision and resulting sole-path commit. Exact extraction,
rustfmt write/check, Rust 2024 warnings-denied compilation and finite execution are
reported in the sealed handoff; this prose does not substitute for their results.

Still unperformed: Cargo/Clippy/WASM gates, actual PostgreSQL migrations and SQL
constraint/race/rollback tests, lease DB-clock and concurrent-instance fencing,
driver/pool reset, commit-loss/restart and crash durability, recovery journal/epoch
issuance, bootstrap allocation, real commerce admission/claim/Unknown reconciliation,
session apply/projection/executor wiring and independently evaluated running game.
G05 must exercise two writers, each transaction fault cut, exact retry after commit
before reply, duplicate slot collision and retained/retired keys against existing
PostgreSQL data. Production durability requires those observations and its reviewed
configuration; the local finite ledger supplies none of them.
