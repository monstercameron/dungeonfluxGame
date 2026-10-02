# G05-D01: initial PostgreSQL and immutable media deployment

Date: 2026-10-01  
Status: decision proposal for independent review; no service provisioned or qualified

## Decision

For the initial deployment, select one native Rust application process, a managed PostgreSQL service, and a separately operated durable object-backed media store. PostgreSQL remains authoritative for game/session/player state, ownership fences, revisions, operation receipts, facts/checkpoints, effect intents, and asset manifests/access metadata. The media store owns immutable bytes. The first application deployment is one process; it is not a single-host database-and-bytes bundle. The database and media service are independent dependencies and neither is a workflow or telemetry store.

The deployment boundary follows `df-persistence` for PostgreSQL-backed repositories/migrations and `df-assets` for byte storage and metadata/access publication. `df-server` composes services; it does not introduce a second database owner. `df-session` stages pure transitions and commits state, receipt, facts and required intents atomically under the current owner fence/revision, then publishes. It never holds a PostgreSQL transaction over media/provider I/O or a client wait. `df-api` authenticates and scopes requests; trusted principal and audience, never an opaque asset ID or content hash, authorize media access. The asset service may issue/read a reference only after authorization is revalidated.

This selects the concrete initial backing described by service operations (managed PostgreSQL and durable object-backed media). Storage architecture and the subsystem interface also describe a local durable file root for initial bytes with object storage as a later adapter. That inconsistency must be reconciled by the storage and assets owners before implementation. This decision chooses object-backed media because the named topology must keep durable bytes independent of process lifecycle and disposable local storage. Do not silently treat a local cache, artifact directory, staging upload, or PostgreSQL large object as the selected durable root.

## PostgreSQL physical starting point

Start with one PostgreSQL database and one migration owner. Use normalized relational tables for identities/sessions/runs/membership, ownership leases/fences, allocation and operation receipts, asset metadata/access, effect intents, commerce reservations/ledger, and deletion/recovery references; use a versioned snapshot row per session/run revision for the large evolving world state. Keep ordered facts/journal/checkpoint and complete media/recording manifests as rows with stable IDs, tenant/session scope, version/provenance, and indexes for their actual query paths. This is a logical starting layout, not a frozen DDL: exact columns, constraints, partitioning, indexes, isolation/locking choices and snapshot/entity split require access-pattern measurements and reviewed G03/G05 contracts.

Enforce uniqueness at the transaction boundary for scoped operation/allocation keys and stable effect IDs; bind retries to a canonical request fingerprint and return the prior durable result. Keep a tombstone or retired namespace when the advertised receipt lookup window ends so an old retry cannot become a fresh write. Commit authoritative state revision, decision result, ordered facts and all mandatory effect intents together using compare-and-swap revision plus current unexpired owner fence. Use database time for fence expiry. A stale fence/revision rejects without publication. If the commit response is ambiguous, reload by operation key; do not repeat the transition blindly. If PostgreSQL is unavailable, reject new authoritative writes and paid dispatch, or return an explicit unknown outcome when the commit may have happened; never fabricate success.

Use a bounded pool, explicit transaction/query deadlines, tenant predicates and transaction-scoped settings reset on every pooled connection. Runtime credentials are separate from migration/backup roles and are neither owner nor superuser/BYPASSRLS. Tenant/audience checks remain in application code even when row-level security is enabled. No transaction spans object-store calls. Configure managed backups/PITR only after selecting and verifying the provider mode; acknowledged-decision RPO and restore objectives in service operations remain conditional candidate targets until a restore drill succeeds. Initial single-process operation has restart downtime and no application-node HA claim. The managed database's own HA/replication mode is a separate provider selection and must not be inferred from “managed.”

## Immutable media publication

`df-assets::AssetStore` owns byte I/O; its PostgreSQL `AssetMetadataStore` owns manifests and access metadata. Upload to a unique, non-public staging key; enforce configured request bounds while streaming. Before visibility, verify complete byte length and a cryptographic content digest against the declared digest, then finalize under a content-addressed immutable key (or an equivalent immutable version ID). Only after durable backing confirms the complete immutable object may the metadata transaction publish the manifest and audience scope. Resolve/open checks trusted scope and current authorization, then reads/ranges from the immutable object. Hashes identify bytes but grant no permission.

The required ordering is bytes durable → digest/length verified → immutable object finalized → authorized manifest committed → reference returned. A retry of the same content may reuse a verified immutable object, but metadata authorization is still checked. Failure before manifest commit leaves an inaccessible staging/orphan object, eligible for bounded owned cleanup; this is safe. A manifest must never point at absent, partial, mutable, or unverified bytes. If metadata commit is ambiguous, look up the stable publication identity before retrying. If object durability or PostgreSQL metadata is uncertain, return unavailable/unknown and do not publish a success reference. Deletion/rights policy can suppress access and schedule eligible byte deletion; “immutable” means bytes are never overwritten in place, not that personal data is retained against the rights lifecycle.

Back up PostgreSQL and media separately and preserve their manifest/hash linkage. Restore must validate referenced object availability and hashes before reopening access; missing media yields explicit unavailable and never a placeholder success. Reconcile immutable orphans without deleting referenced objects. Financial/dispatch/erasure nonregression still depends on the independently protected journal/head policy, not an older PostgreSQL backup. No availability, RPO, RTO, throughput, durability class, geographic redundancy, cost, retention period, or legal deletion period is asserted by this selection.

## Alternatives and rationale

- PostgreSQL as both authority and media byte store: rejected for the initial deployment. It couples large byte transfer/retention to transactional database capacity and does not follow the existing `AssetStore`/`AssetMetadataStore` split.
- Local durable filesystem on the application node: consistent with the interface/storage-architecture wording, but rejected for this selected deployment because node lifecycle and byte durability become coupled; the source conflict is an explicit pre-implementation integration gate.
- Disposable local/object cache as authoritative bytes: rejected because cache loss cannot preserve a published manifest.
- One local SQLite gameplay store: rejected by the architecture and storage decision; SQLite remains workflow and separate telemetry only.
- Multi-process/application-node cluster at launch: deferred. It adds routing and deployment complexity before one process plus existing PostgreSQL fencing is qualified. One process is not a claim of high availability; scale-out follows the same PostgreSQL owner directory and durable media boundary only after fencing, restore, load and migration qualification.

## Owners and integration hooks

`df-persistence` owns PostgreSQL schema/migrations, repository transactions, constraints/indexes, bounded pool, operation/allocation retention, restore and fence implementation. `df-assets` owns immutable byte-root adapter, streaming/digest verification, publication state, manifest/access metadata and orphan reconciliation. `df-session` owns commit-before-publication use of the repository; `df-auth` owns principal/audience authorization; `df-server` composes the services; `df-api` owns request boundary. `df-commerce` remains the independent spend/entitlement authority sharing PostgreSQL transactions at admission; it does not move into session state. `df-telemetry` and workflow SQLite remain separate.

Before production implementation, resolve the local-file versus object-backed initial-root conflict; select actual managed PostgreSQL and media provider/mode, credential/key custody, encryption/versioning/retention/region/consistency behavior, and migration/backup roles; freeze finite schemas and dedupe/tombstone retention; qualify access patterns, query plans, pool/concurrency limits, tenant isolation, ambiguous commit/publication, object loss, migrations, backup restore, deletion replay, and media hash/access behavior. No provider or secrets are selected here. No dependency contracts were supplied in this task, so this proposal adds none and does not claim sibling proposals accepted. Browser/phone testing is inapplicable; no runtime, native, WASM, provider, benchmark, integration, restore, or independent-review check has been performed.

## Finite illustrative contract example

This std-only fixture exercises the selected safety ordering at a narrow boundary. `digest_matches` is a supplied result from the real cryptographic verifier; the fixture does not implement a hash algorithm, database, authorization system, or object store. The example is not a production API and proves no runtime durability.

```rust
#[derive(Debug, Eq, PartialEq)]
enum Publish {
    Visible,
    Refused,
}

fn publish_manifest(
    complete: bool,
    digest_matches: bool,
    durable_object: bool,
    authorized: bool,
) -> Publish {
    if complete && digest_matches && durable_object && authorized {
        Publish::Visible
    } else {
        Publish::Refused
    }
}

fn main() {
    assert_eq!(
        publish_manifest(true, true, true, true),
        Publish::Visible,
    );
    assert_eq!(
        publish_manifest(false, true, true, true),
        Publish::Refused,
    );
    assert_eq!(
        publish_manifest(true, false, true, true),
        Publish::Refused,
    );
    assert_eq!(
        publish_manifest(true, true, false, true),
        Publish::Refused,
    );
    assert_eq!(
        publish_manifest(true, true, true, false),
        Publish::Refused,
    );
}
```

## Evidence and limits

The frozen governing inputs and their SHA-256 values are recorded in the task handoff. The example must be extracted byte-for-byte and checked with Rust 1.98.1, edition 2024, repository `rustfmt.toml`, `rustfmt --check`, `rustc -D warnings`, and execution through the wave v3 guard. A guard exit 75 is a retained no-launch hold, not a pass. Native/WASM workspace checks remain unperformed because this change is a planning document and no application source is changed. Independent frontier review and root integration are separate required gates.
