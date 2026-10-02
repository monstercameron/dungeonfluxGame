# Private telemetry lifecycle, redaction, and evidence pins

Task: `B-C-df-telemetry-D04` (`B-C-df-telemetry-D04-a1`)  
Input revision: `4c0e050c2d30e632bd3b32c51a92cfda52479e6e`  
Owner: `df-telemetry`  
Status: source-backed lifecycle decision for the next native segment implementation; no production behavior or legal approval is claimed.

## Decision

The per-instance native telemetry owner applies an internal lifecycle overlay to
every query, pin, and export before it reads or serializes record payloads. The
overlay is derived from an authenticated deletion, suppression, retention, or
hold decision from the existing rights/service authority. It is an applied local
view of that decision, not a new authority or a new RPC. OTEL trace, producer,
session, build, and record identifiers remain correlation/filter inputs; none
grants access. `df-auth` continues to authorize query, pin, and export separately
and to derive the principal and audience from trusted identity.

Do not persist credentials, authentication material, raw audio, full prompts,
private player content, or private creative payload by default. A specifically
approved private diagnostic sample carries a trusted lifecycle class and scope
beside its existing telemetry identity. Apply purpose and audience checks before
reading payload bytes for a response, pin, or export. A missing or stale policy
epoch, unknown class, untrusted expiry clock, or unavailable authorization state
fails closed for private bytes and returns bounded `Unavailable` information.
The existing `Duration` clock remains monotonic elapsed time for ingest lag and
retry scheduling; it never establishes retention age or private-data permission.

Represent a deletion or suppression as a durable, versioned tombstone keyed to
the existing source identity and its trusted lifecycle scope. Persist and sync
the tombstone before acknowledging the policy update or allowing another read,
pin, or export. Apply it to pending spool frames, the active database, sealed
segments, scalar indexes, pin manifests, and locally served export copies. While
purge is incomplete, the overlay must still withhold the payload and report an
explicit redaction gap. Keep the existing producer/record/sequence identity,
ingestion position, committed watermark, spool checkpoint, and already-issued
receipt facts needed for dedupe and gap reporting. A replay using a tombstoned
identity cannot restore payload; report `Redacted` through the existing typed
domain boundary. Do not retain private content in a failure message or gap
manifest.

Sealed segments stay immutable for ordinary retention and pin operations. When
deletion must remove bytes from one, first make the overlay durable, then create
and validate a redacted replacement segment with explicit lineage, atomically
advance the local catalog, and retire the old segment and its WAL, spool, index,
and temporary copies. If that replacement cannot be committed, continue to deny
reads and pins for the affected segment and report `Unavailable`; never roll back
the tombstone to make the old copy readable. A segment replacement preserves
nonpersonal coverage facts and the existing segment/source identity relationship,
but removes payload-derived hashes when they can fingerprint the deleted value.
Preserve an opaque tombstone for each retired source identity so the same key
cannot be reintroduced. Durable backups and detached downloads follow the
existing deletion-plan/recovery policy: restore must replay deletion tombstones
before serving; a downloaded export cannot be recalled, so its manifest records
scope, expiry, and revocation status and the response says that physical recall
is unavailable.

Create a pin only for records whose durability receipt is `Committed`, whose
SQLite transaction and covered spool checkpoint are both durable, and whose
source segment is sealed and validated. Queue-only or spooled records remain
pending; pinning an active segment first completes the normal checkpoint and
seal. The pin records exact source segment lineage, selected producer/record/
sequence identities, position and committed watermark, build/source identity,
known and unconfirmed gaps, policy epoch, authorized audience, creation time,
and expiry. It refers to existing sealed data; it does not create a competing
ingest path or turn diagnostics into gameplay evidence. Charge the full physical
size of each referenced segment to D03's existing 4 GiB pin reserve, even when
the pin selects a subset of its records. Keep D03's 2 GiB spool, 4 GiB active
database, 30 GiB total instance quota, 4 GiB pin reserve, 26 GiB unpinned budget,
and seven-day query/retention horizon unchanged. Never evict a live pin for
ordinary quota pressure. A pin cannot extend a source lifecycle class: its expiry
is the earliest of the request, the source retention deadline, and seven days
after pin creation. A request that cannot be retained returns `Redacted` or
`Unavailable`; reserve exhaustion returns the existing typed capacity refusal.

Reauthorize every pin read and export against its recorded audience and the
current policy epoch. A later suppression, deletion, or expiry overrides the
pin immediately through the overlay, invalidates derived indexes and served
copies, and releases reserved bytes only after the tombstone is durable and
eligible data is retired. An export is a separately scoped copy with its own
expiry and revocation metadata; possession of an earlier pin or export does not
authorize future reads. Preserve D01's bounded query shape and scope/gap fields,
and return explicit `Redacted`/`Unavailable` rows or ranges without serializing
their private OTLP bytes. Never claim producer-lifetime completeness.

Use trusted service UTC for persisted retention and pin deadlines. If it is
missing, moves backward, or cannot be trusted after restart, suspend age-based
deletion and refuse new private pins/exports until lifecycle time is trustworthy;
continue enforcing current tombstones and hard byte quotas. Do not substitute
producer event time or the monotonic `Duration` elapsed clock for UTC. Preserve
D02's distinct `Spooled` and `Committed` receipts, commit-before-checkpoint
ordering, identity/digest conflict handling for live records, and uncertainty on
ambiguous outcomes. Preserve D03's local owned-root fence, quotas, gap-before-
eviction ordering, and read-only federation: no shared writable network SQLite
database and no second telemetry pipeline.

## Bounded contract example

This standard-library-only example exercises pin eligibility, expiry, authorization,
redaction, and quota refusal. `now_ms` and all persisted deadlines represent
trusted service UTC milliseconds. The monotonic elapsed clock example remains
separate so retention cannot move with `Store::set_time(Duration)`.

```rust
const GIB: u64 = 1024 * 1024 * 1024;
const PIN_RESERVE: u64 = 4 * GIB;
const MAX_PIN_AGE_MS: u64 = 7 * 24 * 60 * 60 * 1000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Unavailable,
    Redacted,
    RetentionExpired,
    PinCapacity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PinDecision {
    Pending,
    Allowed { expires_at_ms: u64 },
    Refuse(Refusal),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Source {
    committed: bool,
    sealed: bool,
    suppressed: bool,
    retention_expires_at_ms: u64,
    segment_bytes: u64,
}

fn decide_pin(
    source: Source,
    authorized: bool,
    trusted_clock: bool,
    now_ms: u64,
    requested_expiry_ms: u64,
    pinned_bytes: u64,
) -> PinDecision {
    if !authorized || !trusted_clock {
        return PinDecision::Refuse(Refusal::Unavailable);
    }
    if source.suppressed {
        return PinDecision::Refuse(Refusal::Redacted);
    }
    if !source.committed || !source.sealed {
        return PinDecision::Pending;
    }
    if now_ms >= source.retention_expires_at_ms {
        return PinDecision::Refuse(Refusal::RetentionExpired);
    }
    let maximum_pin_expiry = now_ms.saturating_add(MAX_PIN_AGE_MS);
    let expires_at_ms = requested_expiry_ms
        .min(source.retention_expires_at_ms)
        .min(maximum_pin_expiry);
    if expires_at_ms <= now_ms {
        return PinDecision::Refuse(Refusal::RetentionExpired);
    }
    if pinned_bytes.saturating_add(source.segment_bytes) > PIN_RESERVE {
        return PinDecision::Refuse(Refusal::PinCapacity);
    }
    PinDecision::Allowed { expires_at_ms }
}

fn monotonic_elapsed(previous_ms: u64, observed_ms: u64) -> u64 {
    previous_ms.max(observed_ms)
}

fn main() {
    let source = Source {
        committed: true,
        sealed: true,
        suppressed: false,
        retention_expires_at_ms: 20 * 24 * 60 * 60 * 1000,
        segment_bytes: 2 * GIB,
    };
    assert_eq!(
        decide_pin(source, true, true, 1_000, 10 * 24 * 60 * 60 * 1000, 0),
        PinDecision::Allowed {
            expires_at_ms: 1_000 + MAX_PIN_AGE_MS,
        }
    );
    assert_eq!(
        decide_pin(
            Source {
                committed: false,
                ..source
            },
            true,
            true,
            1_000,
            10_000,
            0,
        ),
        PinDecision::Pending
    );
    assert_eq!(
        decide_pin(
            Source {
                sealed: false,
                ..source
            },
            true,
            true,
            1_000,
            10_000,
            0,
        ),
        PinDecision::Pending
    );
    assert_eq!(
        decide_pin(source, false, true, 1_000, 10_000, 0),
        PinDecision::Refuse(Refusal::Unavailable)
    );
    assert_eq!(
        decide_pin(
            Source {
                suppressed: true,
                ..source
            },
            true,
            true,
            1_000,
            10_000,
            0,
        ),
        PinDecision::Refuse(Refusal::Redacted)
    );
    assert_eq!(
        decide_pin(source, true, false, 1_000, 10_000, 0),
        PinDecision::Refuse(Refusal::Unavailable)
    );
    assert_eq!(
        decide_pin(
            source,
            true,
            true,
            source.retention_expires_at_ms,
            30_000_000,
            0
        ),
        PinDecision::Refuse(Refusal::RetentionExpired)
    );
    assert_eq!(
        decide_pin(source, true, true, 1_000, 10_000, 3 * GIB),
        PinDecision::Refuse(Refusal::PinCapacity)
    );
    assert_eq!(monotonic_elapsed(31_000, 30_000), 31_000);
    assert_eq!(monotonic_elapsed(31_000, 32_000), 32_000);
}
```

## Source boundary, handoff, and unresolved facts

At the input revision, the current code is a native synthetic fixture rooted
under `development/runtime/telemetry-g06`. `OwnedRoot` admits only that local
owned root; `Store` has one WAL SQLite database and numbered spool frames;
`DiagnosticReader` opens one database read-only, and its retention text says all
durable fixture spool frames are retained with no automatic deletion. The source
has no segment rotation/catalog, private lifecycle scope, deletion overlay,
retention worker, pin/export API, or production service root. Current identities,
signal-qualified content digest, typed `Spooled`/`Committed` receipts, checkpoint,
watermarks, and controlled monotonic `Duration` clock are the D01/D02 boundaries
to preserve, not evidence that lifecycle operations already work. Existing
fixture source also cannot authenticate `df-auth` principals or reveal a trusted
per-record lifecycle scope; the native composition owner must provide those
facts internally before D04 private-data behavior can be integrated.

The minimum I04 source boundary is `crates/df-observe/src/ingress.rs` for the
existing hard-capped native admission limits; `crates/df-telemetry/src/ownership.rs`
for per-instance local ownership; `persistence.rs` for rotation, catalog,
overlay/tombstones, quota, deletion, and pin metadata; `query.rs` for bounded
cross-segment reads and redaction/gap output; and `lib.rs` for the existing
worker, receipt, clock, and readiness boundary. Use private native types and the
existing authorized service composition. Do not add shared RPC/protobuf fields,
another public telemetry service, another spool, a new database authority, or a
parallel pipeline. Preserve all six source Cargo bounds as currently guarded by
the coordinator; no Cargo edit or Cargo build is part of this decision.

The example's exact Markdown Rust literal is formatted with the root
`rustfmt.toml`, compiled with the pinned Rust 2024 compiler and `-D warnings`,
then run to exercise every listed valid and refusal case. Those checks establish
only the pure decision example. No Cargo, native/WASM crate, SQLite, filesystem,
authorization, deletion, rotation, pin/export, backup-restore, retention, clock
recovery, power-loss, or running service check is claimed. I04 still needs
source/build-bound tests for tombstone-before-serve ordering; pending and
committed receipts through redaction; same-identity replay after purge; active
and sealed segment replacement; recovery with a stale overlay; concurrent
readers and rotation; pin expiry/revocation/export; quota pressure; explicit gap
coverage; and service responsiveness. Exact legal authority, source classification
metadata, trusted UTC provisioning, cryptographic erasure requirements, backup
expiry, and detached-export recall remain external policy or implementation
qualification facts, not assumptions this decision approves.
