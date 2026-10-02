# Durable spool receipt and gap semantics

Task: `B-C-df-telemetry-D02` (`B-C-df-telemetry-D02-a1`)  
Input: `a3c31a00e7406898b0a82617fa7626dc0a4e4c94`  
Owner: `df-telemetry`  
Status: source-backed compatibility decision; no production rotation, pin, or crash-recovery claim.

## Decision

The receipt acknowledges a completed durability stage, never bounded RAM queue admission. The existing stages are `PendingReceipt` (no acknowledgement yet), `Durability::Spooled` (the frame and its directory entry were synced), and `Durability::Committed` (the SQLite transaction committed and the covered spool checkpoint was synced and published). Keep the existing `TelemetryIngress::submit`, `IngestReceipt`, `CaptureKey`, and `Durability` boundary. A pending wait deadline ends only the wait; it does not cancel work already owned by ingestion.

`Spooled` is the first durable receipt and permits recovery to replay the same spool frame. `Committed` requires both the SQLite commit and a durable checkpoint at or beyond that receipt's spool position. SQLite's committed watermark and spool checkpoint remain distinct monotonic positions: a transaction can commit while checkpoint publication is still pending. If commit or checkpoint outcome is uncertain, return no newer success, retain/recover the frame, and replay the same producer/record/sequence identity and bytes. Identical replay deduplicates; a changed payload, record identity, or sequence conflicts. Do not reuse a consumed sequence after capture, admission, or receipt failure.

Queue rejection, malformed/oversized input, capacity refusal, sink failure before a spool receipt, lost receipt, and capture failure are gaps or unconfirmed work, never successful acknowledgement. Keep the existing visible aggregate gap signal and sequence-derived missing count; neither proves producer-lifetime completeness. A source sequence is allocated before capture, so an unaccepted attempt can leave a hole. Do not infer that all earlier or later producer records exist from the first/highest accepted sequence. `Duration` supplied to `Store::set_time` is monotonic elapsed time and must remain the basis for lag/retry clocks; producer timestamps and wall-clock jumps cannot move it backward.

For the next segment/retention consumer, preserve these boundaries: do not discard a spool frame before the committed transaction and durable checkpoint cover it; do not describe queue-only work as durable; expose committed watermark, spool checkpoint, and known/unconfirmed gaps separately; and make uncertain replay idempotent under the stable source identity. A rotation or pin operation cannot promote a pending/spooled record into committed evidence. Existing D03 candidate policy allows pinning only committed records in sealed segments, but D03 remains pending review and D04 lifecycle/suppression policy remains unfrozen. This decision freezes no segment quota, pin schema/API, public error, wire field, retention rule, or deployment behavior.

## Source basis and limits

At the input revision, `df-observe::TelemetryIngress::submit` returns a `PendingReceipt`; its docs explicitly say enqueued work is unconfirmed, and `IngestReceipt` has distinct spooled and committed values (`crates/df-observe/src/ingress.rs`). `NativeProducer::reserve` consumes the source sequence before dispatch and documents possible gaps on failed capture (`crates/df-observe/src/producer.rs`).

`Persistence::spool` bounds spool capacity, writes a checksummed frame, calls `sync_all` on the frame and spool directory, then records the frame in its in-memory backlog (`crates/df-telemetry/src/persistence.rs`). `commit_front` writes stable producer/record/sequence identities under a SQLite transaction and commits it before returning. `checkpoint` writes and syncs `checkpoint-next`, renames it to `checkpoint`, syncs the root directory, and only then advances the in-memory checkpoint/removes that backlog position. The worker grants `Committed` only when the checkpoint covers the admitted position; otherwise it returns `Spooled` or an error (`crates/df-telemetry/src/lib.rs`). Reopen validates checkpoint coverage against SQLite content, replays positions above the checkpoint, and uses unique identities plus content digests to distinguish duplicates from conflicts (`persistence.rs`).

The query boundary reports the committed watermark, spool checkpoint, per-source observed sequence bounds, missing count between observations, and a permanently true `unconfirmed_capture_gaps` indicator; the CLI explicitly prints `producer_lifetime_complete=false` (`crates/df-telemetry/src/query.rs`, `crates/df-telemetry/src/main.rs`). Lag time is injected as `Duration` and advanced with `fetch_max` (`crates/df-telemetry/src/lib.rs`). Existing native fixtures cover a sink lock with a spooled receipt and same-byte duplicate replay, spool/SQLite capacity refusal, spool corruption/truncation, and checkpoint/content mismatch on reopen (`crates/df-telemetry/tests/native.rs`). Those are controlled fixtures, not power-loss tests.

The current `OwnedRoot` permits only synthetic fixture roots beneath `development/runtime/telemetry-g06`; the current implementation has one SQLite database, numbered spool frames, and no rotation, immutable segment catalog, retention deletion, or pin/export API (`crates/df-telemetry/src/ownership.rs`, `persistence.rs`, `query.rs`). Therefore file/database sync calls support the code-level ordering stated here, but do not establish hardware power-loss guarantees, production root provisioning, an exact persisted gap-range ledger, seven-day retention, or an actual pin's durability. `planning/service-operations.md` contains proposed production bounds and runbook targets; the sibling D03 document is a candidate awaiting review, and D04's data lifecycle remains open. They are inputs for later implementation, not approved behavior from this task.

## Bounded contract literal

This standard-library-only literal is retained and run with the exact formatter/compiler in the handoff. It exercises the selected receipt, refusal, retry, uncertain-checkpoint, and monotonic-clock rules as a pure decision example. It does not open SQLite, sync files, inject a crash, or prove physical durability.

```rust
enum DurableStage {
    QueueOnly,
    Spooled,
    Committed,
    Uncertain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Receipt {
    Pending,
    Spooled,
    Committed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Retry {
    ReplaySameIdentityAndBytes,
    RefuseConflict,
}

fn receipt(stage: DurableStage, checkpoint_covers_position: bool) -> Receipt {
    match stage {
        DurableStage::QueueOnly | DurableStage::Uncertain => Receipt::Pending,
        DurableStage::Spooled => Receipt::Spooled,
        DurableStage::Committed if checkpoint_covers_position => Receipt::Committed,
        DurableStage::Committed => Receipt::Spooled,
    }
}

fn retry(same_identity: bool, same_bytes: bool) -> Retry {
    if same_identity && same_bytes {
        Retry::ReplaySameIdentityAndBytes
    } else {
        Retry::RefuseConflict
    }
}

fn monotonic_elapsed(previous_ms: u64, observed_ms: u64) -> u64 {
    previous_ms.max(observed_ms)
}

fn main() {
    assert_eq!(receipt(DurableStage::QueueOnly, false), Receipt::Pending);
    assert_eq!(receipt(DurableStage::Uncertain, false), Receipt::Pending);
    assert_eq!(receipt(DurableStage::Spooled, false), Receipt::Spooled);
    assert_eq!(receipt(DurableStage::Committed, true), Receipt::Committed);
    assert_eq!(receipt(DurableStage::Committed, false), Receipt::Spooled);
    assert_eq!(retry(true, true), Retry::ReplaySameIdentityAndBytes);
    assert_eq!(retry(true, false), Retry::RefuseConflict);
    assert_eq!(retry(false, true), Retry::RefuseConflict);
    assert_eq!(monotonic_elapsed(31_000, 30_000), 31_000);
    assert_eq!(monotonic_elapsed(31_000, 32_000), 32_000);
}
```

## Required follow-through

The future implementation owner must connect a production `Store` only through the existing ingress contract, preserve commit-before-checkpoint ordering, and add filesystem/SQLite boundary evidence for real rotation, pinning, replay after each failure boundary, and gap visibility. The D02 literal check establishes only the examples above. Independent review, segment operation, D04 lifecycle decisions, power-loss behavior, and production qualification remain unperformed.
