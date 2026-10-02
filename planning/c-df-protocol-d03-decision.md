# df-protocol D03: recovery revisions and retired stream cursors

Task: `B-C-df-protocol-D03`, attempt `B-C-df-protocol-D03-a1`.
Input: `d870db00dc349425e8f977bdfb20c6ffb24bfd3e`.
Status: bounded source decision and executable contract example; production admission pending.

## Original acceptance and scope

1. older strict writes expire rather than replay
2. The named outcome has actual source/build-bound evidence; unsupported, pending,
   failed and unperformed checks remain explicit.

Original verification is retained: freeze the cited source decision and a bounded
contract example; compare older strict writes expire rather than replay. Retain
decision, alternatives and unresolved facts. Exact executable commands were TBD
at G01 and scoped prerequisite resolution; that planned procedure was not a claim
that Rust/browser/provider checks ran. This attempt supplies the finite commands
below, with actual receipts in its handoff.

Only this decision file is admitted. No protobuf numbers, generated schema,
production module, new service, transport, epoch allocator or gameplay logic is
introduced. `df-types` already owns `RecoveryEpoch` and `SessionRevision`; this
example imports them rather than defining another revision type.

## Governing source decision

- `planning/rpc-api.md`, Common envelopes and outcomes: look up retained committed
  results before validating their original stale basis; observed revision is
  context, while destructive host/debug commands require strict preconditions.
- `planning/rpc-api.md`, Recovery revision ordering: compare `(RecoveryEpoch,
  in_epoch_sequence)` lexicographically; old expected revisions and operation/
  allocation namespaces become expired/indeterminate lookup-only after restore.
- `planning/subsystem-interfaces.md`, Identity and session ownership: the session
  owner authenticates, validates, commits and publishes; regular resets increase
  sequence without issuing an epoch, and disaster restore obtains a newer epoch.
- `planning/service-operations.md`, Composite revision after disaster restore:
  missing/unverified protected head blocks serving; publish a full permitted
  snapshot with the lost game-revision range, without claiming game RPO0.
- `planning/subsystem-architecture.md`, Shared and transport crates and Integration
  and refinement: protocol DTOs map explicitly to canonical domain/client types;
  G03 owns numbering, compatibility and coordinated consumers.
- `planning/coding-style.md`, typed errors, ownership, pure boundaries and checks;
  `planning/observability.md`: return safe classified facts to the native owner.
  ADR 0001–0005 govern isolated scope, resource admission, scoped devlog,
  source/build identity and independent affected-boundary evaluation.

A nonzero epoch constructor validates representation only. It cannot establish
protected-head verification or recovery authority. Native recovery must supply
that evidence before serving; this example assumes its supplied current revision
was obtained after those checks.

Strict new writes require exact equality with the current composite revision.
An older recovery epoch is expired even when its old sequence is numerically
larger than the restored sequence. A mismatch inside the active epoch is a
revision conflict. A retired operation namespace cannot be reopened by changing
expected revision, run, credentials or operation payload. Retained authorized
receipts remain lookup results; missing retired receipts remain expired/
indeterminate and cannot become a new execution. Active retained receipts also
precede the original stale precondition. Authentication, canonical fingerprint
comparison, binding/run checks, repository CAS and commit durability remain the
existing session owner's responsibilities and occur outside this pure example.

A recovery cursor from another epoch, including a future unrecognized epoch,
requires a full current permitted snapshot. A cursor beyond the current sequence
also requires resync. Same-epoch revision cursors can resume only if the existing
subscription owner independently confirms binding, audience, presentation epoch,
view sequence and retained contiguous history. The example's `SameEpochCandidate`
is eligibility for that check, never proof that a stream is resumable. A new
recovery snapshot clears obsolete audio/cues; missing game facts are disclosed,
not recreated. Run IDs and process generations continue to fence callbacks.

## Exact bounded Rust contract example

The following literal is extracted byte-for-byte to `contract-final.rs`. It links the
same worktree's canonical `df-types` source compiled as an rlib. All supplied
values are synthetic; assertions are finite fixture checks. CLI output is fixture
output, not an application logging path.

```rust
use df_types::{RecoveryEpoch, RevisionError, SessionRevision};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WriteDisposition {
    ValidateNewWrite,
    RetainedReceiptLookup,
    ExpiredIndeterminate,
    RevisionConflict,
}

fn strict_write(
    current: SessionRevision,
    expected: SessionRevision,
    namespace_retired: bool,
    retained_receipt: bool,
) -> WriteDisposition {
    if retained_receipt {
        return WriteDisposition::RetainedReceiptLookup;
    }
    if namespace_retired || expected.epoch() < current.epoch() {
        return WriteDisposition::ExpiredIndeterminate;
    }
    if expected != current {
        return WriteDisposition::RevisionConflict;
    }
    WriteDisposition::ValidateNewWrite
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CursorDisposition {
    SameEpochCandidate,
    FullSnapshotRequired,
}

fn recovery_cursor(current: SessionRevision, cursor: SessionRevision) -> CursorDisposition {
    if cursor.epoch() != current.epoch() || cursor > current {
        return CursorDisposition::FullSnapshotRequired;
    }
    CursorDisposition::SameEpochCandidate
}

fn main() -> Result<(), RevisionError> {
    let prior_epoch = RecoveryEpoch::new(7)?;
    let restored_epoch = RecoveryEpoch::new(8)?;
    let current = SessionRevision::new(restored_epoch, 2);
    let old = SessionRevision::new(prior_epoch, u64::MAX);
    let stale = SessionRevision::new(restored_epoch, 1);
    let future = SessionRevision::new(RecoveryEpoch::new(9)?, 0);

    assert!(old < current);
    assert_eq!(current.epoch().get(), 8);
    assert_eq!(current.sequence(), 2);
    assert_eq!(
        current.next_sequence()?,
        SessionRevision::new(restored_epoch, 3)
    );
    assert_eq!(
        strict_write(current, current, false, false),
        WriteDisposition::ValidateNewWrite
    );
    assert_eq!(
        strict_write(current, stale, false, true),
        WriteDisposition::RetainedReceiptLookup
    );
    assert_eq!(
        recovery_cursor(current, stale),
        CursorDisposition::SameEpochCandidate
    );
    println!("valid: current strict basis, retained receipt lookup, same-epoch candidate");

    assert_eq!(
        strict_write(current, old, false, false),
        WriteDisposition::ExpiredIndeterminate
    );
    assert_eq!(
        strict_write(current, old, true, false),
        WriteDisposition::ExpiredIndeterminate
    );
    assert_eq!(
        strict_write(current, current, true, false),
        WriteDisposition::ExpiredIndeterminate
    );
    assert_eq!(
        strict_write(current, old, true, true),
        WriteDisposition::RetainedReceiptLookup
    );
    assert_eq!(
        strict_write(current, stale, false, false),
        WriteDisposition::RevisionConflict
    );
    assert_eq!(
        strict_write(current, future, false, false),
        WriteDisposition::RevisionConflict
    );
    for cursor in [old, future, SessionRevision::new(restored_epoch, 3)] {
        assert_eq!(
            recovery_cursor(current, cursor),
            CursorDisposition::FullSnapshotRequired
        );
    }
    assert_eq!(RecoveryEpoch::new(0), Err(RevisionError::ZeroEpoch));
    assert_eq!(
        SessionRevision::new(restored_epoch, u64::MAX).next_sequence(),
        Err(RevisionError::SequenceOverflow)
    );
    println!("refusal: retired writes never replay, mismatches refuse, recovery cursors resync");
    Ok(())
}
```

## Alternatives and next source consumer

A scalar sequence was rejected: `(7, MAX)` incorrectly outranks `(8, 2)` if
compared by sequence alone. Automatically retrying an expired write or issuing a
new operation ID was rejected because absence of retained receipt does not prove
that the original mutation was never committed. Treating all observed revisions
as strict locks was rejected because ordinary offers revalidate relevant state.
This example intentionally handles strict writes only.

Next source consumer: the G03 `df-protocol` owner freezes composite message fields,
compatibility fixtures and mapping without reimplementing `df-types`. Minimum
future source paths are `crates/df-protocol/proto/` (exact schema filenames and tags
remain G03-owned) and its generated build inputs. The connecting `df-api` mapper,
`df-session` admission/repository consumer and `df-client` subscription owner must
reuse this decision at their approved existing boundaries; their exact production
paths are not frozen by this planning-only submission. G05/X02 qualify epoch
issuance, retention/namespace retirement, recovery durability and lost-range
publication. No source consumer is implemented or admitted by this decision.

## Verification and honest limits

Use the pinned 1.98.1 tool directory from the brief and root `rustfmt.toml`.
Extract the single Rust fence without rewriting bytes; format a scratch copy and
require byte equality with the literal, then run rustfmt `--check`. Compile the
worktree's `crates/df-types/src/lib.rs` using `rustc --edition=2024 -Dwarnings
--crate-name df_types --crate-type rlib`, then compile the literal using
`rustc --edition=2024 -Dwarnings --extern df_types=<owned rlib>`. Execute its
finite valid and refusal assertions under frozen v6 serial admission. Retain
source, config, tool, argv, receipt, output and binary hashes; exact paths live in
the immutable handoff and streamed manifest. Compiler/proof execution waits for
explicit root release. Metadata uses eight shared 32 MiB slots; standalone Git
uses 64 MiB and shared compiler.lock, never nested.

Unperformed production qualification: protobuf numbering/generation/compatibility,
Cargo/Clippy/native workspace/WASM/browser RPC checks, database recovery drill,
protected-head epoch allocation/concurrent fencing, real authenticated receipt
lookup, audience/binding/stream history integration, lost-range notification,
OTEL pipeline, independent evaluator and integrated source checks. The finite
fixture cannot prove those boundaries, G03 admission or five-minute recovery
capacity. No provider, paid call, installation, gameplay or production readiness
claim follows from it.
