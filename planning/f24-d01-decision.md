# F24-D01: PostgreSQL receipt and snapshot authority

Task/attempt: `B-F24-D01-a1`\
Input revision: `308920e328ba6df85bea1e3d2abcbea5f1b796e6`\
Owner: `df-persistence`\
Status: design decision; production implementation and independent review pending

## Decision

PostgreSQL is the authoritative durable store for gameplay state, decisions, and
operation receipts. A snapshot is a versioned projection of one committed session
revision, not an independently writable authority. In one database transaction,
`df-persistence` must compare the current session revision and live owner fence,
then persist the coherent new snapshot (or state documents), operation result,
ordered accepted semantic/proposal/draw/source records, meaningful domain facts,
and required effect intents. The transaction is the commit boundary. A receipt
describes that committed decision and revision; enqueueing, actor admission, or
beginning provider work is not durable acceptance.

The decision may be a committed domain rejection or a committed accepted
transition. Only the latter may include committed effects. A typed invalid-input
or permission failure rejected before a transaction is not represented as a
committed decision receipt. For retry safety, store the operation identity,
principal/session scope, request fingerprint, and exact committed result/revision
under a unique key. A retry with the same identity and fingerprint returns that
result; a different fingerprint conflicts. A lookup that cannot establish whether
the transaction committed returns indeterminate/unavailable and must not execute
the operation again. Receipt retention is finite, but expiration does not make a
key reusable: retain a tombstone or retired namespace and answer expired or
indeterminate when the exact result is no longer available.

The operation receipt confirms state/decision commit only. Required effect intents
are committed atomically with it, while execution and completion happen later under
their own job identities, deadlines and cancellation scopes. Losing or cancelling
the RPC wait does not cancel accepted session/run-owned work. A receipt does not
claim that AI/media/provider work completed, a client received a view, or an
external effect happened exactly once.

Use one versioned authoritative snapshot/revision as the recovery spine, with
ordered committed facts and exact accepted source/proposal/draw records sufficient
for supported deterministic replay. Pin schema, source, rules/content, and other
producer revisions needed to interpret each record. Replay is allowed only when
the reducer and migration chain are complete and compatible; it performs no model
or paid-provider call. Unsupported/missing versions return an explicit gap. The
physical representation (single document versus normalized entities), indexes,
constraint layout, migration machinery, pool sizes, and concrete byte/retention
bounds stay with the G05 `df-persistence` decision after access-pattern and
workload evidence. No arbitrary limits are inferred here.

This boundary is not the authority for irreversible liabilities. The separately
protected journal/head policy governs payment/provider uncertainty, privacy
suppression, retired namespaces, and recovery epoch issuance because restoring an
older PostgreSQL backup can erase later PostgreSQL rows. PostgreSQL remains the
gameplay authority at its declared recovery point objective; journal absence or
an old backup never proves an irreversible action did not occur. A required
protected-journal confirmation must precede acknowledgement/egress for its own
protected transitions. Missing, stale, unauthenticated, regressed, ahead, or
ambiguous protected head data fails closed for affected private/commercial scopes
and triggers reconciliation. This document does not reimplement that journal.

## Alternatives and rationale

- Queue admission as receipt was refused because a queued item can be lost before
  state and operation result commit. It would also make retry after a lost response
  capable of duplicating an allocation or decision.
- Separate transactions for snapshot, operation result, facts, or effect intents
  were refused because a crash could expose a partial decision, omit replay/source
  evidence, or lose an accepted effect. Commit those related records atomically.
- An append-only event log as the sole gameplay authority was refused: the plan
  chooses versioned snapshots plus supported audit/replay records, while full event
  sourcing would broaden the state authority and replay/migration obligations.
- SQLite gameplay storage, telemetry storage, process-local memory, or provider
  queues were refused as substitutes. SQLite remains limited to development
  workflow and runtime telemetry; telemetry is diagnostic, not game recovery data.
- A PostgreSQL-only audit row, receipt, or backup was refused as proof of later
  irreversible actions after restoring an older backup. The protected recovery
  authority remains separate and can hold/reconcile against PostgreSQL state.

## Failure and recovery semantics

- Before commit: publish no successful decision or receipt. Storage unavailability
  rejects admission or leaves the caller without a receipt.
- Compare-and-swap/fence conflict: reload current durable state and revalidate;
  never overwrite using process-local state or the client's observed revision.
- Commit outcome ambiguous: lookup by the original operation identity and
  fingerprint. Do not apply locally, re-enqueue, or retry as a fresh operation
  until the exact result is established. If unavailable, report indeterminate.
- Commit confirmed, response/publication lost: return the stored receipt on lookup;
  recover state and pending intents from PostgreSQL, then publish/execute through
  their owning components.
- Expired receipt payload: return expired/indeterminate with the key still
  non-replayable. Never infer “not committed” from absence in an old backup.
- Restore from older PostgreSQL data: validate schema/source compatibility, replay
  suppressions and reconcile protected journal records before serving affected
  scopes; issue any new recovery epoch only through the separate verified
  protected-head CAS flow. Report the potentially lost gameplay revision range.
- Protected-head or effect-send uncertainty: hold the liability or private action
  for reconciliation; no automatic refund, resend, namespace reset, or fabricated
  success.

## Ownership and integration hooks

`df-session` owns serialized input handling and produces a staged candidate; its
consumer-owned `SessionRepository` port defines the commit/lookup contract.
`df-persistence` implements that port and owns PostgreSQL transactions, owner-fence
and revision comparison, schema/migration/access-pattern decisions, atomic storage,
and exact receipt lookup. `df-model` owns versioned snapshot/fact/provenance types;
engine and rules/director crates produce pure outcomes and do not write SQL.
`df-server` supplies effect executors, while `df-assets` owns immutable asset bytes
and metadata publication. `df-auth` supplies principal/audience authorization;
trace context never grants access. The protected-journal implementation and
disaster-restore qualification are separate `df-persistence` G05/X02 hooks.

## Open production gates

G05 must freeze and implement physical schema, uniqueness/FK/check constraints,
revision/fence transaction, migration ownership and compatibility, receipt result
retention/tombstones, finite database limits, query plans, backup/PITR topology and
restore operations. G03/G10 must provide concrete versioned snapshot and ordered
record codecs/reducers with compatibility fixtures. G05/X02 must qualify protected
journal head authenticity, durability, CAS/fencing, restore completeness, and
correlated failure behavior. PostgreSQL integration and restart/fence-conflict,
duplicate/lost-receipt, ambiguous-commit, migration, realistic-load, and restore
fault tests remain unperformed. No database or protected journal is provisioned;
no production API or end-to-end behavior is claimed. Independent frontier review
and coordinator integration remain required.

## Finite executable policy contract

This standard-library-only example models only the receipt boundary from already
known commit evidence. It does not implement a database, transaction, fence, or
recovery. It proves the distinction between admission and committed receipt and
that an ambiguous commit is never silently retried as new work.

```rust
#[derive(Debug, PartialEq, Eq)]
enum StoreOutcome {
    AdmissionOnly,
    Committed {
        revision: u64,
        fingerprint_matches: bool,
    },
    Conflict,
    CommitUnknown,
    Unavailable,
}

#[derive(Debug, PartialEq, Eq)]
enum ReceiptOutcome {
    Durable { revision: u64 },
    Conflict,
    Indeterminate,
}

fn receipt(outcome: StoreOutcome) -> ReceiptOutcome {
    match outcome {
        StoreOutcome::Committed {
            revision,
            fingerprint_matches: true,
        } => ReceiptOutcome::Durable { revision },
        StoreOutcome::Committed {
            fingerprint_matches: false,
            ..
        }
        | StoreOutcome::Conflict => ReceiptOutcome::Conflict,
        StoreOutcome::AdmissionOnly | StoreOutcome::CommitUnknown | StoreOutcome::Unavailable => {
            ReceiptOutcome::Indeterminate
        }
    }
}

fn main() {
    assert_eq!(
        receipt(StoreOutcome::Committed {
            revision: 9,
            fingerprint_matches: true,
        }),
        ReceiptOutcome::Durable { revision: 9 }
    );
    assert_eq!(
        receipt(StoreOutcome::Committed {
            revision: 9,
            fingerprint_matches: false,
        }),
        ReceiptOutcome::Conflict
    );
    assert_eq!(receipt(StoreOutcome::Conflict), ReceiptOutcome::Conflict);
    assert_eq!(
        receipt(StoreOutcome::AdmissionOnly),
        ReceiptOutcome::Indeterminate
    );
    assert_eq!(
        receipt(StoreOutcome::CommitUnknown),
        ReceiptOutcome::Indeterminate
    );
    assert_eq!(
        receipt(StoreOutcome::Unavailable),
        ReceiptOutcome::Indeterminate
    );
}
```

The executable assertions establish policy mapping only. They do not prove a
PostgreSQL transaction is atomic, that commit evidence is authentic, or that the
runtime returns these outcomes. The exact literal is extracted and checked with
the pinned compiler and repository formatter; evidence is recorded in the handoff.

## Source basis

The decision follows `planning/storage-architecture.md` (PostgreSQL gameplay
authority; receipt is durable decision, not queue admission; snapshot recovery;
no transaction across providers), `planning/subsystem-interfaces.md` (consumer
ports, atomic commit, operation lookup and ambiguous timeout behavior),
`planning/subsystem-architecture.md` (`df-session` commits through persistence;
`df-persistence` implements consumer ports), `planning/runtime-reliability.md`
(authority, cancellation and recovery), and `planning/service-operations.md`
(fences, RPO, protected irreversible recovery and fail-closed uncertainty). The
issued hashes for all governing inputs are frozen in the task context and retained
in the worker evidence manifest; the primary source hashes above are listed in
`handoff.json`.
