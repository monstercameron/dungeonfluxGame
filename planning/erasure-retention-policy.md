# Erasure and retention overlay

Date: 2026-09-30

Task/attempt: `B-G05-D03/a1`

Input revision: `4ba37943691f3a01a49b8238983e82527bec91bf`

Status: decision and bounded policy example; production persistence and asset adapters are not implemented or qualified

## Decision

Treat personal-data suppression as a current, monotonic overlay on snapshots,
canonical replay, derived data, exports, and restored backups. A snapshot is not
authority to expose the personal payload it once contained. Before a restored
environment serves private data, authenticate the latest protected recovery
journal head, replay deletion/revocation tombstones and rights restrictions, and
invalidate affected indexes, summaries, caches, projections, and pending private
deliveries. If the protected head or a needed current decision cannot be
verified, hold the affected scope unavailable. Do not interpret missing overlay
state as permission.

At every private replay/retrieval dispatch and again before publishing its
result, resolve current trusted tenant/audience authorization, the current
suppression generation, applicable tombstones, source availability and version,
index generation/source version, and current reviewed source `RightsGrant`.
Reject stale work if any of these facts change while it runs. A stale snapshot,
index, prepared response, cache entry, or provider result cannot authorize its
own delivery. Provider calls are not part of deterministic replay or restore.
Replay and restore never ask a model to reconstruct missing personal text from
memory, summaries, provenance, or other retained material.

The result is explicit and scoped: `Redacted` when suppression removed a
personal payload, or `Unavailable` when a required source, current right, or
verified policy fact is absent, expired, incompatible, or stale. Retain only the
permitted nonpersonal action/source/catalog revision and minimal deletion
accounting needed to explain the gap and protect recovery. Do not claim an intact
replay where the required source is gone. A rights denial and a privacy
redaction can coexist; the privacy tombstone wins the example's result.

The Rust example below is a dependency-free decision-policy model. It exercises
these precedence and freshness rules; its types are illustrative and are not
production contracts, database logic, or proof that a caller enforces the rule.

## Ownership and lifecycle

`df-persistence` owns the durable `DeletionPlan`, suppression generations,
irreversible journal/recovery ordering, restore overlay, repository queries,
retention jobs, and the current facts supplied to callers. `df-assets` owns
durable media bytes, object references, publication/access checks, and its
source-owned cache invalidation. Existing projections, retrieval/index owners,
provider dispatch, export, and client cache owners must consume the same current
decision at their boundaries; this document does not assign them a second
policy implementation. The coordinator owns the later cross-system integration
hook and whole-decision acceptance.

A `DeletionPlan` records a trusted subject and tenant scope, basis, affected
source records and payloads, assets, derived indexes/summaries, caches, logs,
export manifests and backup generations, any reviewed hold, per-item outcomes,
completion stage, and a suppression tombstone. Exact schema and type names
remain subject to G03/G05 contract review. The plan is not complete merely
because a request or asynchronous job was accepted.

1. **Suppress first.** Commit prospective denial and advance the monotonic
   suppression generation before acknowledging completion or allowing another
   private read, replay, retrieval, prompt, export, distribution, or provider
   dispatch. Record the minimal irreversible suppression fact in the protected
   recovery journal and confirm its durable head before a completion
   acknowledgement. An ambiguous journal result means pending/unknown and
   affected private admissions stay closed.
2. **Invalidate and remove.** Bounded jobs remove or cryptographically erase
   permitted personal payloads and keys, invalidate derived personal indexes,
   summaries, caches, audience projections, and pending deliveries, and record
   fulfilled, retained-under-review, unsupported, or failed outcomes. Recheck
   generation/fence and current rights at dispatch and before any job result is
   published. Failed work remains visible and retryable by stable identity.
3. **Restore through the overlay.** Restore into quarantine. Verify the latest
   protected journal head and nonregressing watermark, apply all post-backup
   suppression/revocation/rights entries, rebuild only permitted derived data,
   then open authorized scopes. Missing keys, journal, completeness proof, or a
   current decision fails closed for affected scopes; reconcile and record
   explicit gaps rather than serving the backup as current.
4. **Close with limits stated.** Completion reports each scope and stage. It
   does not promise immediate physical removal from backups or retract bytes
   already downloaded or shared outside the service. Retained copies remain
   access-suppressed, are removed under the selected backup expiry policy, and
   must be suppressed again on every restore. Explain external-copy limits
   before export consent.

## Retention classes and distinct authorities

Use the currently proposed, versioned manifest as a planning baseline. These
values are not legal conclusions, measured backup behavior, or launch promises.

| Class | Current proposal | Replay and erasure treatment |
| --- | --- | --- |
| Credentials | Revoke immediately | Reject new authentication/use; do not wait for payload cleanup. |
| Challenge tokens | 15 minutes | Expire; associated replay never grants a new challenge. |
| Anonymous bootstrap dedupe | 24 hours, then a retired-key tombstone | Old keys remain non-replayable and cannot allocate fresh work. |
| Operation receipts | 90 days, then a nonreplayable namespace tombstone | Retired operation keys reject; expiry does not make an old mutation new. |
| Raw capture | Persistence disabled by default; approved diagnostic sample at most 24 hours | A sample needs a scoped approval; delete/expire it and its derived private copies under the applicable suppression. |
| Private creative speech and episodic data | Campaign lifetime plus a 30-day deletion window | Treat as personal payload; suppress at request/decision before cleanup; do not regenerate it. |
| Canonical replay | Pseudonymous minimal facts and catalog revisions | Preserve permitted nonpersonal provenance; redact or access-restrict personal identifiers and payload. Missing personal text returns `Redacted`/`Unavailable`. |
| Billing and tax | Period selected after jurisdiction/legal review | Keep only required records under that reviewed policy; do not use the retention duty as gameplay or private-dialogue access authority. |
| Rights-cleared attribution | Kept separately where the grant permits | Current grant governs use and distribution; attribution is not a license to retain unrelated personal payload. |
| Backups | Candidate expiry window of at most 30 days | Physical purge is not instantaneous; protected tombstones are applied before service on every restore. |

Privacy erasure, source-use rights, and legal holds are separate decisions.
Subject suppression blocks the service's use and disclosure of affected
personal payload even when another rule requires a limited retained record.
An expired/revoked `RightsGrant` blocks new source reuse, derivation,
distribution, replay, and export as covered by that grant; it does not itself
erase a player's account or rewrite a committed game decision. A reviewed,
scoped legal hold can preserve only the identified material needed for that
hold; it does not silently restore general retrieval, prompt, replay, or
distribution access. Release of a hold is a separate reviewed action. The
applicable jurisdiction, legal basis, statutory periods, and hold authority
remain legal-launch decisions; this document is not legal advice.

Personal payload and provenance must not be conflated. Remove or restrict
private text, identifiers, voice, imported personal media, and any derived
summary/index/cache capable of revealing them. Retain only minimal, permitted
nonpersonal facts needed to represent the shared committed outcome, source and
catalog revision, and an auditable scoped gap. Do not rewrite another
participant's shared campaign history to pretend a committed event never
happened; remove the departed participant's private linkage/payload and apply
the audience policy to the remaining shared record. Immutable provenance is not
an excuse to preserve an immutable personal original.

## Replay admission model

The current facts passed to this pure function are assumed to have been read
from trusted, authenticated current state. The real caller must obtain them
under the trusted tenant/audience boundary and repeat admission immediately
before output. A production implementation must bind these checks to its
durable fences and exact source/index contracts; the example does not do so.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Decision {
    Permit,
    Redacted,
    Unavailable(Reason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Reason {
    RightsExpired,
    SourceMissing,
    StaleSnapshot,
    StaleIndex,
}

#[derive(Clone, Copy)]
struct Snapshot {
    suppression_generation: u64,
    source_generation: u64,
    index_source_generation: u64,
}

#[derive(Clone, Copy)]
struct Current {
    suppression_generation: u64,
    source_generation: u64,
    tombstoned: bool,
    source_available: bool,
    rights_current: bool,
}

fn admit(snapshot: Snapshot, current: Current) -> Decision {
    if current.tombstoned {
        return Decision::Redacted;
    }
    if !current.rights_current {
        return Decision::Unavailable(Reason::RightsExpired);
    }
    if !current.source_available {
        return Decision::Unavailable(Reason::SourceMissing);
    }
    if snapshot.suppression_generation != current.suppression_generation {
        return Decision::Unavailable(Reason::StaleSnapshot);
    }
    if snapshot.index_source_generation != current.source_generation {
        return Decision::Unavailable(Reason::StaleIndex);
    }
    if snapshot.source_generation != current.source_generation {
        return Decision::Unavailable(Reason::StaleSnapshot);
    }
    Decision::Permit
}

fn main() {
    let old = Snapshot {
        suppression_generation: 7,
        source_generation: 12,
        index_source_generation: 12,
    };
    let restored_after_deletion = Current {
        suppression_generation: 8,
        source_generation: 12,
        tombstoned: true,
        source_available: true,
        rights_current: false,
    };
    let restored_with_expired_right = Current {
        tombstoned: false,
        ..restored_after_deletion
    };
    let stale_index = Snapshot {
        index_source_generation: 11,
        ..old
    };
    let stale_source = Snapshot {
        source_generation: 11,
        ..old
    };
    let missing_source = Current {
        source_available: false,
        rights_current: true,
        ..restored_with_expired_right
    };
    let current = Current {
        suppression_generation: 7,
        tombstoned: false,
        rights_current: true,
        ..restored_with_expired_right
    };

    let cases = [
        (
            "restored pre-tombstone snapshot",
            admit(old, restored_after_deletion),
        ),
        (
            "current rights expired",
            admit(old, restored_with_expired_right),
        ),
        ("stale source index", admit(stale_index, current)),
        ("stale source generation", admit(stale_source, current)),
        ("source unavailable", admit(old, missing_source)),
        ("current source and rights", admit(old, current)),
    ];
    for (label, decision) in cases {
        println!("{label}: {decision:?}");
    }
    assert_eq!(cases[0].1, Decision::Redacted);
    assert_eq!(cases[1].1, Decision::Unavailable(Reason::RightsExpired));
    assert_eq!(cases[2].1, Decision::Unavailable(Reason::StaleIndex));
    assert_eq!(cases[3].1, Decision::Unavailable(Reason::StaleSnapshot));
    assert_eq!(cases[4].1, Decision::Unavailable(Reason::SourceMissing));
    assert_eq!(cases[5].1, Decision::Permit);
}
```

This model demonstrates only local decision precedence and generation
comparisons. Its `Current` values are supplied facts; there is no tenant auth,
database transaction, concurrent fence, deletion worker, asset/cache invalidator,
journal, backup restore, provider, or real replay caller in this executable.
Its `Permit` case is not proof that private delivery is safe or integrated.

## Alternatives and unresolved decisions

* **Keep immutable personal payload and filter only at read time:** rejected.
  Backup restore, stale indexes/caches, alternate retrieval paths, or provider
  buffers could bypass the filter. Suppression must be current, generation-bound,
  and applied before service after restore.
* **Rewrite shared canonical game history to remove every event associated with
  the subject:** rejected. It can falsify other participants' committed history.
  Remove private linkage/payload and retain only reviewed, permitted minimal
  provenance plus an explicit gap.
* **Rely on retention TTL or model reconstruction:** rejected. TTL does not
  prevent reads before expiry or resurrection from a backup; model memory is not
  a source of authority and cannot restore deleted text.

Still open for the owning decisions: PostgreSQL physical groups, constraints,
access patterns, migrations, index ownership, and fences; exact shared typed
contracts, the scope/key of the monotonic suppression generation, and the
rights-check source; protected journal object root, key
custody, independently retained head/watermark and qualified RPO; legal
jurisdictions, statutory billing/tax periods and hold review; exact permitted
minimal subject/suppression identifiers; backup expiry and proof of physical
purge; and adapters that invalidate every real producer, derived store,
projection, provider buffer, export and client cache. The current checkout
contains no `df-persistence` or `df-assets` crate and no canonical
`DeletionPlan`, `RightsGrant`, or lifecycle runtime type. No production adapter,
schema, storage choice, deletion completion, or application behavior is claimed.

## Evidence and handoff

Source decision: the submitted planning document is built from input commit
`4ba37943691f3a01a49b8238983e82527bec91bf`. The required source files and their
SHA-256 hashes are recorded in the attempt's retained `source-hashes.txt`.
The standalone example was extracted from this document and compiled/executed
with cached `rustc 1.98.1` in the attempt scratch directory. Exact executable,
configuration, example, binary hashes, commands, output, and exit codes are in
`verification.json` under that attempt's worker evidence directory. This is a
bounded pure Rust decision-policy proof only; no Cargo build, browser, provider,
schema, restore, production adapter, or integrated application check ran.
The scoped source-discovery observation is devlog entry
`dlog-B-G05-D03-a1-missing-persistence-assets-contracts`; the append succeeded
against the shared workflow database and did not change queue state.

Original acceptance criteria (preserved verbatim):

1. `replay restrictions`
2. `The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.`

Original verification procedures (preserved verbatim):

1. `Freeze the cited source decision and a bounded contract example; compare replay restrictions. Retain decision, alternatives and unresolved facts.`
2. `Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.`

## References

* [Implementation roadmap](implementation-roadmap.md), especially G05 ownership,
  delivery order, first-slice failure envelope, and independent integration
  acceptance.
* [Subsystem interfaces](subsystem-interfaces.md), PostgreSQL/durable media,
  persistence port ownership, lifecycle ports, and typed `Redacted`/
  `Unavailable` replay outcomes.
* [Long-horizon state](long-horizon-state.md), retrieval/contradiction and
  personal payload/rights lifecycle.
* [Service operations](service-operations.md), retention classes, deletion
  plans, restore ordering, tenant/audience authority, and protected
  nonregressing recovery journal.
* [Commerce service](commerce-service.md), separate account/entitlement and
  reviewed rights authority.
* [Commercial validation](commercial-validation.md), reviewed `RightsGrant`
  evidence and unresolved legal/source clearance. This is referenced as the
  governing source for rights even though it is outside this attempt's frozen
  source-hash list.
* [Coding style](coding-style.md), [ADR 0001](../ADR/0001-sqlite-agent-workflow.md),
  [ADR 0003](../ADR/0003-agent-devlog.md),
  [ADR 0004](../ADR/0004-development-reliability.md), and
  [ADR 0005](../ADR/0005-frontier-output-evaluation.md).
