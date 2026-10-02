# Per-instance telemetry segment quota and retention

Task: `B-C-df-telemetry-D03` (`B-C-df-telemetry-D03-a1`)
Input revision: `575df9c3a619e8ee68eecd6bbf3d4c5ebdd00c35`
Owner: `df-telemetry`
Status: policy decision for I04; not a production implementation or deployment claim.

## Decision

Each native service process instance owns one protected local telemetry root, one
SQLite writer, and its durable spool. The root and spool stay on local storage;
instances never mount or write one another's live SQLite database or WAL. Review
federation uses bounded read-only queries or closed, validated immutable segment
copies. `df-auth` authorizes query, pin, and export separately from OTEL identity.
WASM uploads through the authorized API and never opens SQLite.

Per instance, cap the durable spool at **2 GiB**, the active SQLite database plus
WAL/SHM at **4 GiB**, and all telemetry-owned storage at **30 GiB**. The 30 GiB
cap includes active and rotated databases, spool frames, indexes/manifests,
temporary rotation space, and pinned evidence. Reserve **4 GiB** within that cap
for pinned evidence; ordinary unpinned writes may use at most the other 26 GiB.
Warn at 80% of the total or pin reserve. At the 4 GiB active limit, checkpoint
and close the active database, validate and seal it as immutable, then create a
new active database only if total-quota admission still succeeds. Keep committed
segments for at most **7 days** when capacity permits; quota pressure can evict
them earlier. Never expire an uncommitted spool frame by age: remove a frame only
after its SQLite commit and durable checkpoint. When a write would exceed quota,
reclaim the oldest unpinned closed segments first, recording each evicted
producer sequence range and reason; persist the gap manifest before removing
the segment. If that manifest cannot be committed, stop eviction and return
typed capacity refusal. If no eligible segment can satisfy the request, return
typed capacity refusal. Never evict pinned evidence automatically. Pin
authorization belongs to `df-auth`; `df-telemetry` accepts only committed
records in sealed segments, and charges the full physical size of every segment
containing the requested range against the reserve. Requests for uncommitted
records remain pending until a committed receipt; a pin into the active segment
first seals it. Reserve overflow returns capacity refusal. Unpinning is an
authenticated operator decision; exports retain their access and revocation
metadata. Age starts at segment close and uses trusted service UTC, never
producer event time. If that clock is missing or untrusted, suspend age-based
deletion; quota eviction can still use stable segment creation order and must
record a gap.

Queries return retained time bounds, per-producer accepted sequence bounds,
explicit missing/evicted ranges, segment identities, spool checkpoint, and
committed watermark. Keep gap metadata for the same seven-day query horizon;
outside it, report retention expiry and never claim producer-lifetime
completeness. Quota pressure wins over seven days. On spool or total-disk
exhaustion, reject durability explicitly, publish gap/degraded health, and emit
the permitted emergency diagnostic; do not acknowledge a record as durable or
silently sample enabled logs. Stop new high-volume admissions at hard-full while
already admitted gameplay reaches its safe checkpoint.

This selects finite proposed limits, not measured capacity or a retention/legal
approval. An evidence pin protects only against ordinary quota eviction; it
does not override D04 suppression, erasure, or legal-hold policy. If D04 requires
removing private payload, apply that decision across active and sealed segments,
indexes, manifests, and exports before serving them; retain only permitted
minimal nonpersonal provenance plus an explicit gap. A request to pin content
that cannot be retained under D04 returns `Redacted` or `Unavailable`. The exact
lifecycle authority and record classification remain D04-owned. The policy
needs a later operations qualification before deployment.

## Source-backed boundary and implementation gap

At the input revision, `df-observe::TelemetryLimits` defaults to a 32 MiB spool
and 64 MiB SQLite cap, and validation treats those defaults as hard ceilings
(`crates/df-observe/src/ingress.rs`). `df-telemetry::Store::open` constructs a
single `LocalIngress` worker over `Persistence`; persistence uses one local
`telemetry.sqlite3` WAL database plus numbered spool frames and advances its
checkpoint only after commit (`crates/df-telemetry/src/lib.rs`,
`persistence.rs`). `OwnedRoot` currently accepts only a synthetic fixture path
under `development/runtime/telemetry-g06`; it does not select a deployment path
(`ownership.rs`). The read-only `DiagnosticReader` pages one database and
reports “all durable spool frames retained; finite quota; no automatic
deletion” (`query.rs`). There is no production rotation, immutable segment
catalog, retention deletion, pin/export API, or production root composition in
this source. The CLI and native tests are synthetic fixtures. They establish
useful current limits and refusal behavior, not this decision's production
behavior. `Store::set_time(Duration)` supplies a controlled monotonic elapsed
clock for the worker; production segment age still needs a trusted UTC source
and deterministic clock tests. The accepted TELEMETRY-G06-001 foundation
receipt covers its finite native fixture and scoped ingress/spool/query
contracts, not this retention policy, D04 private-data lifecycle, production
federation, or deployment.

The smallest I04 follow-on changes are `crates/df-observe/src/ingress.rs`
(currently hard-capped at 32 MiB spool and 64 MiB SQLite, so raise validated
production ceilings to admit the approved envelope),
`crates/df-telemetry/src/ownership.rs` (per-instance local root provisioning),
`crates/df-telemetry/src/persistence.rs` (segment rotation, quota accounting,
pin reserve, D04 lifecycle filtering, retention and gap manifest),
`crates/df-telemetry/src/query.rs` (bounded cross-segment reads and coverage
metadata), and
`crates/df-telemetry/src/lib.rs` (worker ownership/readiness/refusal wiring).
The eventual service composition must inject this store through the existing
`TelemetryIngress` boundary. The D04-owned lifecycle contract is a prerequisite
for retaining private diagnostic data; D01 topology and D02 spool receipt/gap
semantics are existing boundaries to preserve, not duplicate contracts. No
SQLite layout or hosted deployment path is claimed by this policy document.

## Bounded contract example

This exact standard-library-only example checks the selected boundaries and
refusals. It is not a second store or a substitute for I04 integration.

```rust
const GIB: u64 = 1024 * 1024 * 1024;
const ACTIVE_LIMIT: u64 = 4 * GIB;
const SPOOL_LIMIT: u64 = 2 * GIB;
const PIN_RESERVE: u64 = 4 * GIB;
const INSTANCE_LIMIT: u64 = 30 * GIB;
const UNPINNED_LIMIT: u64 = INSTANCE_LIMIT - PIN_RESERVE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Root {
    LocalInstance,
    SharedNetwork,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    SharedWritableDatabase,
    InstanceQuota,
    PinnedReserve,
    SpoolQuota,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WriteDecision {
    Append,
    Rotate,
    Refuse(Refusal),
}

fn decide_unpinned_write(
    root: Root,
    active_bytes: u64,
    instance_bytes: u64,
    unpinned_bytes: u64,
    incoming_bytes: u64,
) -> WriteDecision {
    if root == Root::SharedNetwork {
        return WriteDecision::Refuse(Refusal::SharedWritableDatabase);
    }
    if instance_bytes.saturating_add(incoming_bytes) > INSTANCE_LIMIT {
        return WriteDecision::Refuse(Refusal::InstanceQuota);
    }
    if unpinned_bytes.saturating_add(incoming_bytes) > UNPINNED_LIMIT {
        return WriteDecision::Refuse(Refusal::PinnedReserve);
    }
    if active_bytes.saturating_add(incoming_bytes) > ACTIVE_LIMIT {
        return WriteDecision::Rotate;
    }
    WriteDecision::Append
}

fn pin_fits(pinned_bytes: u64, incoming_bytes: u64) -> bool {
    pinned_bytes.saturating_add(incoming_bytes) <= PIN_RESERVE
}

fn spool_fits(spool_bytes: u64, incoming_bytes: u64) -> Result<(), Refusal> {
    if spool_bytes.saturating_add(incoming_bytes) > SPOOL_LIMIT {
        return Err(Refusal::SpoolQuota);
    }
    Ok(())
}

fn main() {
    assert_eq!(
        decide_unpinned_write(Root::LocalInstance, 1, 2, 2, 1),
        WriteDecision::Append
    );
    assert_eq!(
        decide_unpinned_write(Root::LocalInstance, ACTIVE_LIMIT, 2, 2, 1),
        WriteDecision::Rotate
    );
    assert_eq!(
        decide_unpinned_write(Root::SharedNetwork, 0, 0, 0, 1),
        WriteDecision::Refuse(Refusal::SharedWritableDatabase)
    );
    assert_eq!(
        decide_unpinned_write(Root::LocalInstance, 0, INSTANCE_LIMIT, 0, 1),
        WriteDecision::Refuse(Refusal::InstanceQuota)
    );
    assert_eq!(
        decide_unpinned_write(Root::LocalInstance, 0, 0, UNPINNED_LIMIT, 1),
        WriteDecision::Refuse(Refusal::PinnedReserve)
    );
    assert!(pin_fits(PIN_RESERVE - 1, 1));
    assert!(!pin_fits(PIN_RESERVE, 1));
    assert_eq!(spool_fits(SPOOL_LIMIT - 1, 1), Ok(()));
    assert_eq!(spool_fits(SPOOL_LIMIT, 1), Err(Refusal::SpoolQuota));
}
```

## Verification and open production checks

The bounded example's formatting, compilation, and finite execution are retained
under this attempt's evidence directory. They prove only that the literal
example is syntactically valid and rejects its listed cases. I04 still needs
tests against actual filesystems and SQLite: writer isolation, concurrent
readers, checkpoint/close/reopen rotation, crash during seal, pin/export and
revocation, eviction order, recovery after spool exhaustion, visible gaps and
readiness, and gameplay responsiveness. No production rotation, storage quota,
or seven-day retention check has run here.
