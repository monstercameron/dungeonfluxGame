# Native telemetry foundation

This implements the native part of TELEMETRY-G06-001: a standalone synthetic
`df-telemetry` executable composes actual OpenTelemetry 0.31 SDK providers from
`df-observe`, bounded standard OTLP ingress, an owned durable spool, SQLite and
read-only diagnostic queries. The existing experimental `FixtureTelemetry` and
transport/browser consumers retain their existing behavior. They do not use this
store yet. Full G06 remains open for browser capture, authenticated sources and
operator access, gameplay integration, federation, privacy/retention and production
qualification. Diagnostic IDs and trace context grant no identity or authorization.

## Capture and receipt contract

`df-observe` owns `ProducerId`, `RecordId`, `SourceSequence`, `CaptureKey`, the
object-safe `TelemetryIngress`, bounded `NativeProducer`, typed inputs/errors and
receipts. Producer and record IDs are distinct supplied nonzero 16-byte values.
Sequence numbers range from 1 through i64::MAX with checked exhaustion. A producer
reserves the caller's record ID and source sequence before dispatch to its private
SDK providers. The caller owns source lifecycle uniqueness, including restart
sequence selection. Timestamps and message text never generate record identity.

The actual SDK log exporter is a concrete native adapter, because this version's
`LogExporter` is not object safe. SDK callbacks encode/preflight/enqueue only;
filesystem and SQLite work runs on the `Store`'s sole owned writer thread. Producer
methods return fail-fast typed admission errors or a `CapturedBatch` containing
exact standard OTLP bytes and a `PendingReceipt`. Retry those bytes and supplied
identities when a receipt is lost. Reserve does not itself emit anything. Local
queued capture has no durable guarantee. A receipt wait deadline cancels that wait;
it does not cancel accepted work. Polling pending receipts does not block emission.

A `Spooled` receipt follows SHA256-framed payload fsync and containing-directory
fsync. A `Committed` receipt additionally follows SQLite FULL synchronous commit
and fsynced checkpoint replacement. A receipt includes its spool position and the
current committed record watermark; a watermark for earlier records does not
upgrade a newer spooled record. Recovery commits at most one old frame between
new spool admissions. Explicit FIFO flush/shutdown drains the remaining backlog.
A blocked sink uses a 50ms SQLite busy timeout and one 250ms retry clock, allowing
spooling to continue. This is a finite spool acceptance guarantee, not instantaneous
SQLite availability. Identical producer/record/content retries deduplicate to the
original record position; byte-conflicting reuse and producer/sequence reuse for
another record return `Conflict`.

Resource and scope schema URLs remain distinct for logs and spans. The pinned
OTLP log grouping helper uses the resource URL for the scope; the native adapter
uses its correct per-record conversion. SDK resource attributes are sorted before
encoding, so rebuilding the same supplied log after process restart produces
stable bytes. Tests exercise the actual SDK with both schema URLs and process
restart retries. Enabled Trace/Debug/Info/Warn/Error/Fatal logs all capture when
their supplied trace context is unsampled. Actual span start/end, parent context,
links and typed attributes survive OTLP and SQLite. Log source/observed timestamps
preserve the full u64 nanosecond range using big-endian SQLite blobs; integer
AnyValue fields preserve i64 limits. Standard spans have no observed timestamp,
so that query field is null rather than invented. Complete canonical single-record
OTLP bytes preserve resource/scope, supported typed bodies and causal fields.

## Selected bounds and ownership

| Boundary | Selected ceiling |
| --- | ---: |
| Encoded record | 8,192 bytes |
| OTLP request | 65,536 bytes / 32 records |
| Pending admissions | 1,024 items / 1,048,576 accounted payload bytes |
| Retained spool | 33,554,432 bytes / 32,768 files |
| Retained unique identities | 32,768 |
| SQLite database plus sidecars | 67,108,864 bytes |
| Query page / conservative encoded response budget | 100 records / 1,048,576 bytes |
| Query filter label | 128 bytes |
| SQLite busy timeout | 50ms |
| Sink retry interval | 250ms |
| Controlled barrier / fixture process deadline | 30 seconds |
| Emergency fallback | 32 entries / 8,192 bytes |

The allocation-free wire pass checks supplied/declared lengths before prost decode,
including malformed varints/tags, 12 nested schema levels, 4,096 total fields,
128 fields per message, record size and count. Producer inputs additionally bound
attributes, nested AnyValue values, span links and timestamp conversion. Queue byte
accounting includes encoded requests, canonical records and fixed per-record
allowance. It is not a whole-process memory limit or browser 8MiB qualification.

Smaller boundary fixtures test a one-item queue, a 4KiB queue byte ceiling, 128-byte
spool and 256KiB SQLite ceiling. SQLite reserves half the quota for main database
pages and checks main/WAL/SHM bytes before commits with an additional bounded
reserve (half the quota, capped at 4MiB). The selected minimum SQLite quota is
256KiB, accommodating schema/sidecars while exercising explicit exhaustion. These
are conservative local admission limits rather than a hostile-filesystem sandbox.
Production 2GiB spool, 4GiB SQLite segments, 30GiB quota and seven-day retention are
unqualified. All spool files are retained; there is no automatic deletion/migration,
and exhausted stores report capacity rather than deleting evidence.

Writable roots are limited to coordinator-provisioned
`/Users/earlcameron/Desktop/dungeonflux/development/runtime/telemetry-g06/ownedfixture-*`.
An explicit marker, fixed filenames, no symlink or multiply-linked regular file
adoption, nofollow opens and an exclusive OS file lock protect ownership. Foreign
stores and incompatible schema versions are refused; corrupted, truncated or
noncontiguous spool/checkpoint state is refused without repair. Workflow SQLite,
PostgreSQL, evidence and unrelated runtime stores are separate. Runtime data never
belongs beneath artifacts or in the source worktree. This local path binding is
intentional; there is no deployment configuration or remote listener.

Keep the store alive until explicit shutdown succeeds. A shutdown deadline retains
its worker handle for retry. Drop signals closure; it cannot safely cancel a blocked
OS fsync. The owning fixture process enforces the external 30-second bound and
terminates only its own children. No losslessness claim covers SDK/pending capture
before a durable receipt, an interrupted wait, or an unknown producer lifetime.

## Diagnostics and health

`DiagnosticReader` opens SQLite read-only with `query_only`, nofollow paths and a
fixed schema version. Its fixed typed filters cover producer, session, operation,
minimum severity, build, trace and inclusive source-time bounds. Pages use immutable
ingestion positions as cursors and short read transactions while ingestion runs.
The CLI emits complete OTLP as hex, timestamps, record identities, committed
watermark, durable spool checkpoint and per-source accepted sequence watermarks.
Mutation attempts are exercised and denied. There is no arbitrary SQL endpoint,
remote source access, permission simulation or operator authorization.

Per-source sequence gaps count missing numbers only between first and highest
observed accepted sequence; prefixes and lifetime completeness remain unknown.
`unconfirmed_gaps` starts at one as a producer-lifetime incompleteness indicator,
then counts rejected admission/receipt/fallback incidents. It is not an exact count
of lost records. For example, the preserved failed burst reported 2,403 known
fail-fast rejections and 2,404 gap indicators: those same rejections plus the one
baseline, not 2,404 additional losses. Admitted, spooled, committed, retries,
duplicates, rejected, capacity, pending/backlog, checkpoint, last commit and lag
remain independent counters. Injected monotonic backlog age gives warning after
30 seconds and degraded after 120 seconds; this has no gameplay readiness authority.

Sink/capacity failures write a bounded independent local `emergency` file with
source, safe error class, checkpoint and backlog. Exhaustion reports its outcome
and falls back to bounded stderr; it never recursively invokes OTEL. Later query
results link/include this source. A caller sees pre-durable refusal through the
same owning producer/ingress path and can retain its already captured identity.

## Executable and verification

Use the repository wrapper with explicit jobs1, shared MAIN artifact/cache roots and
separate temporary root. After a frozen build, retain binary/source/configuration
hashes alongside each corpus. Example commands, with a fresh owned runtime root:

```sh
sh development/build-fixture.sh cargo build --locked --offline --release -p df-telemetry --jobs 1
<binary> emit <ownedfixture-root> 10 1
<binary> load <ownedfixture-root> 100 10
<binary> load <ownedfixture-root> 1000 10
<binary> replay <ownedfixture-root>
<binary> query <ownedfixture-root> 100 0 severity=9 session=synthetic-session
```

The CLI prints safe synthetic evidence, not private application logs. `emit` can
expose controlled preaccept/spooled/committed barriers for the external crash
harness. `load` independently schedules offered arrival times and polls receipts;
it does not reinterpret sequential receipt waiting as a 1,000/s offered burst.
Its output separates offer duration, admission/SDK latency, durable receipt latency,
spooled/committed receipt counts, post-offer drain, schedule lateness and backlog.

The optimized pre-freeze fair-recovery candidate offered 10,000 representative
1KiB bodies over 10.000331s with an actual approximately two-second SQLite writer
lock. It admitted/spooled/committed all 10,000 with no rejected offers, 8,997 spooled
and 1,003 committed receipts, p95 emit 21us, p95 durable receipt 41.069ms and
post-offer drain 3.712389s. Peak queue was 96 items/358,272 accounted bytes and
backlog 328 frames. The healthy burst had p95 emit 29us, receipt 50.013ms and
32.154ms post-offer drain. Concurrent read-only query p95 was at most 7.47ms in
these runs. Exact final frozen measurement evidence supersedes these observations;
they are finite synthetic outcomes, not production performance guarantees.

Earlier debug sequential receipt-wait measurements took 14.38s healthy and 15.95s
with lock; they did not establish a 1,000/s arrival envelope. Earlier correctly
paced optimized attempts rejected 1,337 and then 2,403 offers because sink retries
and whole-backlog draining blocked spool fairness. Preserve these outcomes with
artifact/configuration identities. Fair one-frame catch-up addresses that measured
problem. Broader optimization and retention are separate work.

Required handoff gates include workspace format, native workspace Clippy/tests,
affected `df-observe` and existing `df-tools` WASM compile/Clippy, actual process
crash/replay/rebuilt-byte retries, concurrent queries and synthetic load. Native
SQLite/SDK exporter dependencies must be absent from the affected WASM closure.
Exact locked archive checksums, manifest/features/source and available license
notices accompany native and WASM graphs; OTLP defaults are explicitly disabled
because its actual tagged Cargo default is full. Independent frontier evaluation
of all six original criteria and a separate merged MAIN boundary belong to the
coordinator. This document is not an approval or full G06 completion record.
