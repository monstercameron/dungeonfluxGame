# D04: Browser buffer and bounded export lifetime

Date: 2026-10-02
Status: Source-backed decision; browser buffer/upload implementation remains pending
Input revision: `4c0e050c2d30e632bd3b32c51a92cfda52479e6e`
Task/attempt: `B-C-df-observe-D04` / `B-C-df-observe-D04-a1`

## Decision

The browser buffer is owned by one live client binding generation. It accepts only
already allowlisted, typed OTEL single-record bytes from the D02 catalog. It is
best-effort memory owned by that page/binding; it does not claim browser crash
persistence. The buffer retains records until a terminal upload receipt says they
were durably accepted. A send, half-close, pending receipt, or timeout alone never
releases them. Retries reuse the same record identities and bytes.

Use a 128-record / 256-KiB retained-memory ceiling, including the one in-flight
lease, with a single upload lease of at most 32 records / 64 KiB. These values
bound the browser adapter independently of native memory and fit the existing
`TelemetryLimits` batch ceiling. A record that cannot fit, or a new record arriving
at capacity, is refused without evicting older records; increment a safe overflow
count and expose it as a gap. Do not retain refused field names or values. The
buffer owns encoded record bytes, not generic event bodies or unfiltered
attributes.

At most one upload is active per binding generation. Its owner is the binding's
buffer/export state, with a finite 10-second attempt deadline. On deadline, close,
or transport failure, preserve the batch in that state and report the classified
outcome; do not infer that the server rejected it or cancel server-owned work. A
terminal durability receipt releases only records it identifies as accepted. An
explicit partial/rejected receipt remains visible and leaves unaccepted records
owned for a policy-correct retry or operator-visible stop. A callback may change
state only when both its captured generation and lease still match the current
owner. Rebinding invalidates the old lease; page destruction may lose the
best-effort in-memory remainder, which is reported as a gap when the lifecycle
allows reporting.

This is an internal ownership seam, not a new public Rust API, wire envelope,
OTEL SDK, or uploader implementation. The future `df-api` adapter authorizes the
client independently and connects the approved OTLP records to
`TelemetryIngress`; `df-telemetry` remains the sole native spool/SQLite owner.
The exact generated `UploadTelemetry` request/receipt types and browser transport
feasibility are not present in this source revision, so their mapping remains a
consumer implementation gate. Operation/trace context is provenance only.

## Source fit

- D01 keeps correlation separate from authorization and requires owned work to
  retain captured context across request-span completion. The page binding owns
  only its best-effort browser buffer; server-accepted work has its own lifetime.
- D02 is the browser capture allowlist: preserve standard typed OTEL records,
  reserved producer/record/sequence identity, and reject unlisted/private data
  before encoding. Its native limits do not establish browser memory bounds.
- D03 assigns serialization and native ingress to the caller-side adapter. Pure
  results remain facts; the browser reporter does not install a second logger,
  queue owner, or durable store.
- `crates/df-observe/src/ingress.rs` defines 32 records/64 KiB native batches,
  stable capture identities, and a `PendingReceipt` whose timeout cancels only
  the wait. `crates/df-observe/src/producer.rs` is native-only. Neither file
  supplies a browser queue or browser upload receipt type.
- `planning/rpc-api.md` specifies bounded client-streaming uploads, terminal
  durability receipts, and retry by stable record IDs. It gives no numeric
  browser buffer or RPC deadline. The 10-second attempt deadline and browser
  retention bounds above are this decision's local buffer policy, not a claim
  about server processing time or durability.

## Bounded contract example

The literal below models only the page-owned retained buffer and generation/lease
checks. `bytes` stands for one already D02-filtered, bounded OTEL record. It does
not define or simulate an RPC receipt or an implemented browser uploader.

```rust
use std::collections::VecDeque;

const MAX_ITEMS: usize = 128;
const MAX_BYTES: usize = 256 * 1024;
const MAX_BATCH_ITEMS: usize = 32;
const MAX_BATCH_BYTES: usize = 64 * 1024;
const UPLOAD_DEADLINE_MS: u64 = 10_000;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Record(Vec<u8>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct UploadLease {
    generation: u64,
    records: usize,
    deadline_ms: u64,
}

#[derive(Debug, PartialEq, Eq)]
enum BufferError {
    Empty,
    Oversized,
    Capacity,
    StaleLease,
    Deadline,
    NotExpired,
}

struct BrowserBuffer {
    generation: u64,
    records: VecDeque<Record>,
    bytes: usize,
    refused: usize,
    lease: Option<UploadLease>,
}

impl BrowserBuffer {
    fn new(generation: u64) -> Self {
        Self {
            generation,
            records: VecDeque::new(),
            bytes: 0,
            refused: 0,
            lease: None,
        }
    }

    fn retain(&mut self, record: Record) -> Result<(), BufferError> {
        let length = record.0.len();
        if length == 0 {
            self.refused += 1;
            return Err(BufferError::Empty);
        }
        if length > MAX_BATCH_BYTES {
            self.refused += 1;
            return Err(BufferError::Oversized);
        }
        if self.records.len() >= MAX_ITEMS
            || self.bytes.checked_add(length).is_none_or(|total| total > MAX_BYTES)
        {
            self.refused += 1;
            return Err(BufferError::Capacity);
        }
        self.bytes += length;
        self.records.push_back(record);
        Ok(())
    }

    fn begin_upload(&mut self, now_ms: u64) -> Result<UploadLease, BufferError> {
        if self.lease.is_some() {
            return Err(BufferError::Capacity);
        }
        let mut bytes = 0;
        let mut records = 0;
        for record in &self.records {
            if records == MAX_BATCH_ITEMS
                || bytes + record.0.len() > MAX_BATCH_BYTES
            {
                break;
            }
            bytes += record.0.len();
            records += 1;
        }
        if records == 0 {
            return Err(BufferError::Empty);
        }
        let lease = UploadLease {
            generation: self.generation,
            records,
            deadline_ms: now_ms.saturating_add(UPLOAD_DEADLINE_MS),
        };
        self.lease = Some(lease);
        Ok(lease)
    }

    // A timed-out attempt releases only its lease; retained records remain retryable.
    fn expire_upload(&mut self, now_ms: u64) -> Result<(), BufferError> {
        let lease = self.lease.ok_or(BufferError::Empty)?;
        if now_ms < lease.deadline_ms {
            return Err(BufferError::NotExpired);
        }
        self.lease = None;
        Ok(())
    }

    // Invalidate callbacks and report the best-effort records lost with this binding.
    fn invalidate(&mut self, generation: u64) -> usize {
        let lost = self.records.len();
        self.records.clear();
        self.bytes = 0;
        self.lease = None;
        self.generation = generation;
        lost
    }

    // Only a matching, in-deadline durable acknowledgement releases owned bytes.
    fn acknowledge_durable(&mut self, lease: UploadLease, now_ms: u64) -> Result<(), BufferError> {
        if lease.generation != self.generation || self.lease != Some(lease) {
            return Err(BufferError::StaleLease);
        }
        if now_ms > lease.deadline_ms {
            return Err(BufferError::Deadline);
        }
        for _ in 0..lease.records {
            if let Some(record) = self.records.pop_front() {
                self.bytes -= record.0.len();
            }
        }
        self.lease = None;
        Ok(())
    }
}

fn main() {
    let mut buffer = BrowserBuffer::new(7);
    buffer.retain(Record(vec![1, 2, 3])).unwrap();
    let lease = buffer.begin_upload(1_000).unwrap();
    assert_eq!(lease.deadline_ms, 11_000);
    assert_eq!(buffer.bytes, 3);
    assert_eq!(buffer.expire_upload(11_000), Ok(()));
    let retry = buffer.begin_upload(11_000).unwrap();
    assert_eq!(retry.records, lease.records);
    assert_eq!(
        buffer.acknowledge_durable(lease, 11_001),
        Err(BufferError::StaleLease)
    );
    assert_eq!(buffer.bytes, 3);
    assert_eq!(buffer.acknowledge_durable(retry, 12_000), Ok(()));
    assert_eq!(buffer.bytes, 0);

    let mut refused = BrowserBuffer::new(8);
    assert_eq!(refused.retain(Record(Vec::new())), Err(BufferError::Empty));
    refused.retain(Record(vec![4])).unwrap();
    let stale = refused.begin_upload(1_000).unwrap();
    assert_eq!(refused.invalidate(9), 1);
    refused.retain(Record(vec![5])).unwrap();
    assert_eq!(
        refused.acknowledge_durable(stale, 2_000),
        Err(BufferError::StaleLease)
    );
    assert_eq!(refused.bytes, 1);
    assert_eq!(refused.refused, 1);
}
```

The valid path proves this contract fixture retains bytes until a matching
acknowledgement; refusal cases cover empty input and stale-generation completion.
They do not prove receipt identity matching, privacy filtering, overflow reporting,
RPC behavior, timer scheduling, page-close reporting, or a running browser path.

## Alternatives

- Evicting the oldest record on overflow would silently lose accepted work; refuse
  the newest item and expose the gap instead.
- Clearing a batch when send starts or when the caller's wait times out would lose
  ownership before terminal durability is known; retain the lease through the
  receipt outcome.
- A second browser JSON schema, SDK exporter, or durable store would duplicate
  existing OTEL/ingress ownership without source support.

## Evidence and remaining checks

Source revision: `4c0e050c2d30e632bd3b32c51a92cfda52479e6e`. The literal and
valid/refusal executions are prepared, but no source-active formatting or compile
check passed. An initial guard invocation omitted required arguments and failed
before receipt creation; a direct rustfmt check outside the guard found one
formatting difference. The subsequent guarded rustfmt admission wrote
`HOLD_NO_COMMAND_LAUNCHED` at 36% free proxy, below the 40% floor. Per the v6
policy, no formatting, compile, or refusal check was retried after HOLD. The
standalone fixture is therefore unverified. Cargo, WASM, real-browser, RPC,
authorization, upload, durability, telemetry-pipeline, and frontier
computer-use/vision evaluation remain unperformed and pending.
