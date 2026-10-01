# Typed recovery revision policy

Task/attempt: `B-C-df-types-D02/a1`  
Decision input: `aac3d48029f638e5dd15e3cb8a5db96cce1f2fb1`  
Canonical contract: `CONTRACT-G03-001`  
Approved recovery policy: `B-G03-D03/a2`, input `c9ac1203b7709fed04f89e5a84e9e577ec27fa34`  
Owner boundary: `df-types` owns the pure values and comparisons; native recovery/persistence owns epoch issuance and protected-head authority.

## Decision

Keep the existing canonical `df_types::RecoveryEpoch` and `df_types::SessionRevision` value contract. `RecoveryEpoch::new` accepts a caller-supplied nonzero `u64` and rejects zero. Construction does not issue, authenticate, verify, or authorize an epoch. `SessionRevision` contains that epoch and an in-epoch `u64` sequence. Sequence zero is valid. Its derived total ordering is lexicographic by the struct's declared fields: epoch first, sequence second. The epoch-eight/sequence-zero value therefore sorts above epoch-seven/sequence-`u64::MAX`.

Advance only within the current epoch by calling the canonical `next_sequence`. It uses checked addition and returns `RevisionError::SequenceOverflow` at `u64::MAX`; it never wraps and never issues an epoch. A maximum `RecoveryEpoch` value is representable as a supplied value, but this type does not provide an epoch-increment or recovery escape hatch.

This is a type-owner contract and preserves the already implemented primitive. It does not select a new wire representation. The approved G03 recovery policy requires explicit presence of the epoch message, epoch value, and sequence at the consuming protocol mapping; validates nonzero epoch; and accepts a present zero sequence. `df-protocol` owns wire schema/numbering, and the consuming API/client/persistence boundary owns explicit mapping and validation. No type-level ordering claim proves a generated-wire mapping.

The approved recovery policy also requires a disaster restore to authenticate the latest nonregressing protected head and conditionally persist a strictly greater epoch before reopening sessions. That issuance and verification authority belongs to the native persistence/recovery owner, not `df-types`. If a protected epoch head is `u64::MAX`, checked increment is exhausted and restore remains closed. Ordinary reconnects and restarts continue from the durable sequence without changing the disaster epoch.

## Bounded canonical-source example

This example imports the assigned worktree's existing `revision.rs`; it deliberately does not copy or reimplement the value types or their ordering. The assertions exercise sequence-zero validity, same-epoch sequence ordering, epoch-first ordering across an older maximum sequence, zero rejection, maximum values, checked sequence overflow, and epoch arithmetic exhaustion. The import is a standalone verification harness, not a second canonical API.

```rust
#[path = "/Users/earlcameron/Desktop/dungeonflux/artifacts/worktrees/types_revision_decision/crates/df-types/src/revision.rs"]
mod revision;

use revision::{RecoveryEpoch, RevisionError, SessionRevision};

fn main() {
    assert_eq!(RecoveryEpoch::new(0), Err(RevisionError::ZeroEpoch));

    let first_epoch = RecoveryEpoch::new(1).expect("one is a nonzero supplied epoch");
    let old_epoch = RecoveryEpoch::new(7).expect("seven is a nonzero supplied epoch");
    let new_epoch = RecoveryEpoch::new(8).expect("eight is a nonzero supplied epoch");
    let maximum_epoch = RecoveryEpoch::new(u64::MAX).expect("maximum supplied epoch is nonzero");

    let first_zero = SessionRevision::new(first_epoch, 0);
    let old_maximum = SessionRevision::new(old_epoch, u64::MAX);
    let new_zero = SessionRevision::new(new_epoch, 0);
    let new_one = new_zero.next_sequence().expect("sequence can advance");
    let maximum_sequence = SessionRevision::new(new_epoch, u64::MAX);
    let maximum_epoch_zero = SessionRevision::new(maximum_epoch, 0);

    assert_eq!(first_zero.sequence(), 0);
    assert!(new_one > new_zero);
    assert!(old_maximum > first_zero);
    assert!(new_zero > old_maximum);
    assert_eq!(new_one.epoch().get(), 8);
    assert_eq!(maximum_sequence.sequence(), u64::MAX);
    assert_eq!(
        maximum_sequence.next_sequence(),
        Err(RevisionError::SequenceOverflow)
    );
    assert_eq!(maximum_epoch_zero.epoch().get(), u64::MAX);
    assert_eq!(u64::MAX.checked_add(1), None);

    println!("PASS: canonical revision ordering, zero sequence, and checked boundaries");
}
```

## Alternatives and unresolved facts

A scalar sequence alone cannot establish that post-restore state sorts above an older snapshot whose counter is ahead, so it is not the chosen contract. Comparing sequence before epoch would place `(8, 0)` below `(7, u64::MAX)`, violating the recovery ordering. Wrapping sequence or epoch would make an exhausted value appear current; this contract returns an error for sequence exhaustion, while protected epoch exhaustion remains a native fail-closed recovery outcome. Allowing `df-types`, a clock, a process counter, or gameplay journal to issue epochs would duplicate protected durable authority and permit fork or regression.

Still unresolved and outside this type-owner decision: physical protected-head format and authenticity; epoch allocation, durable compare-and-swap/fencing and nonregression; journal completeness and restore fault qualification; namespace retirement and ambiguous-send reconciliation; actual generated protobuf field mappings/compatibility fixtures; and any production persistence integration. Their owners and gates remain the approved G03/G05/X02/API-wave boundaries. This decision does not mark I02 or G03 production complete, claim protected-head guarantees, or qualify restore, browser, WASM execution, privacy, finance, or integrated gameplay behavior.

## Verification boundary

The bounded example is extracted verbatim and compiled/executed against the canonical source path above with cached Rust 1.98.1 and repository rustfmt configuration. Exact extraction, formatter, compiler and run commands, outputs, tool/source hashes, and extracted example hash are retained under the assigned attempt's durable `worker/` evidence directory. Cargo, Clippy, browser/WASM execution, generated-Prost mapping execution, persistence, protected-head/CAS, restore, and independent frontier/integrated checks are unperformed here. The example verifies only the currently implemented pure value boundary.
