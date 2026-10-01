# Protected journal durability boundary

Status: proposed decision policy; provider qualification, production persistence
integration, cryptographic proof, and disaster-restore acceptance remain unperformed.

This policy defines the irreversible boundary for payment credit, possible external
sends, and privacy suppression when PostgreSQL is restored from an older backup.
PostgreSQL remains authoritative for gameplay snapshots and ordinary state at its
declared RPO. The protected journal is evidence used to hold/reconcile that state;
it is not a second gameplay writer, a replacement for PostgreSQL, or a telemetry
spool. An old PostgreSQL backup cannot establish that a later payment, dispatch,
delete, revocation, or retired namespace never happened.

## Decision

`df-persistence` owns the append and confirmation protocol behind the existing
persistence interfaces. `df-assets` owns the durable byte root and publication,
hash, access, and recovery policy for assets; it must not treat the journal as an
asset store. The coordinator owns sequential integration and whole-decision
acceptance. The example below is a pure policy model only: it does not implement
or claim any of those production hooks.

Each installation/recovery epoch has an append-only journal keyed by tenant,
monotonic sequence, and stable intent/event/deletion identity. Entries contain only
minimal permitted encrypted metadata: provider request/idempotency key or hash,
maximum liability and dispatch state, verified payment event/final result/credit
identity, suppression/revocation/rights state, and retired allocation/operation
namespaces. Do not copy private speech or other personal payloads into it. Entry
integrity links and the latest authenticated head watermark are retained
independently off-host from every PostgreSQL backup and from telemetry/spool data.

The external head advances by conditional compare-and-swap from the expected
previous sequence. A new append does not authorize an irreversible effect by itself.
The boundary is crossed only after both the journal entry and the matching latest
protected head are durably confirmed. Only then may the protected transition be
acknowledged, the paid dispatcher receive its permit, or new financial entitlement
become visible. Deletion and revocation acknowledgement also wait for that boundary.
The native dispatcher additionally checks a current journal-confirmed permit and
owner fence immediately before egress. These checks cannot retract a packet already
in flight.

On restore, authenticate the latest off-host head and prove the local journal is
complete through exactly that sequence before serving affected private or
commercial scopes. Missing, unauthenticated, regressed, or ahead/conflicting head
state fails closed for those scopes. Replay suppression first, then overlay
post-backup irreversible entries as holds/reconciliation records. Rebuild financial
facts only from known source observations. A durable append whose head publication
is unresolved is an unacknowledged tail: quarantine and reconcile it; do not dispatch,
credit, or acknowledge deletion from it. Allocate any recovery epoch only from a
verified head, using conditional compare-and-swap; missing/unverified head blocks
serving. A restored run reports its lost gameplay revision range honestly.

An ambiguous send retains worst-case liability. `UnknownLiability`, `Dispatching`,
and an extra journal entry for an operation that might not have been sent do not
permit automatic retry, wallet refund, or liability release merely because a lease
expired. Query/reconcile by the same stable provider key where supported. If the
provider cannot establish status or idempotency, keep the outcome unknown and use a
prepared fallback; a human reconciliation must not invent success or refund.
Release is allowed only after the dispatcher verifies `VerifiedUnsent` before any
socket send began, or after known settlement accounts for the liability. A provider
success may settle accounting under its attempt identity, but only the current actor
may commit a still-eligible gameplay result.

## State-transition example

This dependency-free Rust 2024 example makes the narrow policy executable. `HeadView`
stands for already supplied verification facts; it performs no cryptography, storage,
CAS, provider request, owner fencing, or recovery itself. Equality is intentionally
strict: a journal sequence above the protected watermark is an unresolved tail, not
permission to advance or infer the next head. Production CAS, authenticated key
custody, immutable retention/versioning, consistency guarantees, and failure recovery
remain provider-specific qualification gates.

```rust
#[derive(Debug, PartialEq, Eq)]
enum RestoreDecision {
    Ready,
    FailClosed,
}

#[derive(Debug, PartialEq, Eq)]
enum BoundaryDecision {
    MayProceed,
    HoldAndReconcile,
}

#[derive(Debug, PartialEq, Eq)]
enum SendKnowledge {
    VerifiedUnsent,
    Unknown,
    MayHaveBeenSent,
}

#[derive(Debug, PartialEq, Eq)]
enum LiabilityDecision {
    Release,
    HoldAndReconcile,
}

struct HeadView {
    authenticated: bool,
    journal_head: Option<u64>,
    protected_head: Option<u64>,
}

struct BoundaryEvidence {
    entry_durable: bool,
    entry_sequence: Option<u64>,
    head_durable: bool,
    protected_head_sequence: Option<u64>,
}

fn restore_decision(head: &HeadView) -> RestoreDecision {
    if !head.authenticated {
        return RestoreDecision::FailClosed;
    }

    match (head.journal_head, head.protected_head) {
        (Some(journal), Some(protected)) if journal == protected => RestoreDecision::Ready,
        _ => RestoreDecision::FailClosed,
    }
}

fn boundary_decision(evidence: &BoundaryEvidence) -> BoundaryDecision {
    if !evidence.entry_durable || !evidence.head_durable {
        return BoundaryDecision::HoldAndReconcile;
    }

    match (evidence.entry_sequence, evidence.protected_head_sequence) {
        (Some(entry), Some(protected)) if entry == protected => BoundaryDecision::MayProceed,
        _ => BoundaryDecision::HoldAndReconcile,
    }
}

fn liability_decision(send: SendKnowledge) -> LiabilityDecision {
    match send {
        SendKnowledge::VerifiedUnsent => LiabilityDecision::Release,
        SendKnowledge::Unknown | SendKnowledge::MayHaveBeenSent => {
            LiabilityDecision::HoldAndReconcile
        }
    }
}

fn main() {
    let verified = HeadView {
        authenticated: true,
        journal_head: Some(41),
        protected_head: Some(41),
    };
    assert_eq!(restore_decision(&verified), RestoreDecision::Ready);
    assert_eq!(
        boundary_decision(&BoundaryEvidence {
            entry_durable: true,
            entry_sequence: Some(41),
            head_durable: true,
            protected_head_sequence: Some(41),
        }),
        BoundaryDecision::MayProceed
    );

    let rollback = HeadView {
        authenticated: true,
        journal_head: Some(40),
        protected_head: Some(41),
    };
    assert_eq!(restore_decision(&rollback), RestoreDecision::FailClosed);

    let missing_head = HeadView {
        authenticated: true,
        journal_head: Some(41),
        protected_head: None,
    };
    assert_eq!(restore_decision(&missing_head), RestoreDecision::FailClosed);

    assert_eq!(
        boundary_decision(&BoundaryEvidence {
            entry_durable: true,
            entry_sequence: Some(42),
            head_durable: false,
            protected_head_sequence: Some(41),
        }),
        BoundaryDecision::HoldAndReconcile
    );
    assert_eq!(
        boundary_decision(&BoundaryEvidence {
            entry_durable: true,
            entry_sequence: Some(42),
            head_durable: true,
            protected_head_sequence: Some(41),
        }),
        BoundaryDecision::HoldAndReconcile
    );
    assert_eq!(
        liability_decision(SendKnowledge::Unknown),
        LiabilityDecision::HoldAndReconcile
    );
    assert_eq!(
        liability_decision(SendKnowledge::MayHaveBeenSent),
        LiabilityDecision::HoldAndReconcile
    );
    assert_eq!(
        liability_decision(SendKnowledge::VerifiedUnsent),
        LiabilityDecision::Release
    );
}
```

The example covers a verified matching head and published boundary, a regressed
journal head, a missing protected head, a durable entry without its matching durable
head, an unknown or possibly-sent request, and verified-unsent release. A matching pair is only a policy input;
it is not evidence that a real provider supplied authenticated bytes, a CAS was
linearizable, or a live restore is safe.

## Owners and consumer hooks

- `df-persistence` implements the existing repositories and owns PostgreSQL
  transaction/fence, append, protected-head confirmation, replay, hold, and receipt
  lookup integration. Its exact schema, access patterns, fencing, idempotency
  retention, migrations, backups, and recovery remain G05 owner decisions.
- `df-assets` owns durable bytes, validated hash/access publication and media
  recovery; the journal may record a minimal suppression/reference fact but not
  replace `AssetStore` or `AssetMetadataStore`.
- The native egress dispatcher consumes a journal-confirmed permit and current
  fence. Commerce/payment policy exposes only verified journal-backed credit and
  keeps `UnknownLiability` reserved; `df-auth`/private projections apply suppression
  before serialization. These are consumer obligations, not types introduced here.
- The coordinator owns sequential merge and original whole-decision acceptance.
  Production integration is not asserted by this planning artifact.

## Alternatives and unresolved qualification

A PostgreSQL-only audit row or backup was rejected because restoring an older backup
can erase its own later rows. A telemetry spool is also unsuitable: telemetry has a
separate SQLite lifecycle and is not authoritative recovery evidence. Rewriting the
old backup or replaying game events as the only source of truth is outside this
boundary; gameplay retains the declared database RPO and reports lost revisions.

The durable backing provider, off-host watermark service, authenticated key custody,
immutable/version-retention controls, consistency model, compare-and-swap semantics,
key rotation/recovery, and correlated failure guarantees are unselected and
unqualified. Their cost and request overhead are unmeasured. No journal has been
provisioned, no cryptographic verification is implemented, and no payment, provider,
asset, deletion, or PostgreSQL restore integration was run. Until the backing can
prove the protected RPO0 guarantee for acknowledged irreversible transitions,
paid release remains blocked; uncertainty fails closed and triggers incident
reconstruction rather than an advertised financial-loss tolerance.

## Verification and original acceptance

The bounded example was extracted from this document and compiled/run with cached
Rust `1.98.1` (`rustc --edition=2024`), then formatted and checked with the
repository `rustfmt.toml`. Exact commands, versions, source/config/compiler/binary
hashes, output, exit statuses, and limitations are retained in the attempt handoff.
No Cargo build, dependency download, browser/device check, provider qualification,
cryptographic check, PostgreSQL integration, or restore drill is claimed.

Original acceptance criteria (unchanged):

1. `nonregressing head`
2. `The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.`

Original verification procedures (unchanged):

1. `Freeze the cited source decision and a bounded contract example; compare nonregressing head. Retain decision, alternatives and unresolved facts.`
2. `Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.`

The example's execution is bounded decision-policy evidence only. It does not satisfy
independent frontier review or resulting MAIN acceptance; those remain pending.
