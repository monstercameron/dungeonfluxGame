# Recovery revision policy

Task/attempt: `B-G03-D03/a2`

Input source: `c9ac1203b7709fed04f89e5a84e9e577ec27fa34`

Owner boundary: `df-types`, `df-model`, `df-protocol`; this document specifies policy around the existing `df-types` primitive. It adds no issuer, journal authority, schema, or application implementation.

## Decision

Represent a session revision as the existing `df_types::SessionRevision`: a nonzero caller-supplied `RecoveryEpoch` followed by a `u64` in-epoch sequence. Compare the pair lexicographically (epoch first, sequence second). Sequence zero is valid, so `(8, 0) > (7, u64::MAX)`. Advance the sequence only through the existing checked `next_sequence`; overflow is a typed `RevisionError::SequenceOverflow`, never wraparound and never an implicit epoch change. `RecoveryEpoch::new` rejects zero. Issuing a strictly greater epoch is a separate persistence/recovery action; if the authenticated protected head is `u64::MAX`, checked epoch increment is exhausted and restore stays closed rather than wrapping or minting zero.

At the protocol boundary, require presence of the `SessionRevision.epoch` message, its optional `RecoveryEpoch.value`, and the optional `SessionRevision.sequence`; then validate the epoch value as nonzero. Missing epoch message, missing epoch value, explicit zero epoch, and missing sequence are distinct invalid encodings. A present sequence value of zero is valid. Do not infer presence from either optional scalar default. This follows the existing `common.proto` representation and leaves wire numbering/versioning owned by `df-protocol`.

A revision value carries ordering only. Its construction does not authenticate, grant access, establish uniqueness, or issue an epoch. Ordinary reconnects, process restarts, and owner restarts continue from durable current sequence and do not rewrite the disaster epoch. Only disaster restore may start a new epoch, after the native recovery authority authenticates the latest nonregressing protected head and conditionally persists a strictly greater epoch before reopening sessions. Missing, unverifiable, stale, or ambiguous head state fails closed for affected serving/admission. The protected journal records epoch issuance and irreversible liabilities/suppressions, not every game action; gameplay retains its declared five-minute RPO and restore reports the lost revision range.

Once a new epoch is durably issued, retire old expected-revision, operation, and allocation namespaces. Preserve exact retained receipt lookup; return expired/indeterminate when proof is unavailable. Never accept an old command as new work. Restore replays privacy suppressions before serving, overlays post-backup irreversible entries for reconciliation, and does not interpret an absent old-backup record as proof that an irreversible effect did not happen. Protected-head persistence, compare-and-swap/fencing, journal completeness, and restore fault qualification remain native `df-persistence`/G05/X02 work; this policy does not claim those guarantees are implemented or qualified.

## Alternatives and rationale

- A single scalar revision cannot order post-restore state above an older snapshot when its counter rolls back. The composite pair preserves the existing game RPO while making a new recovery epoch sort above every sequence in an older epoch.
- Wrapping either sequence or epoch would make old values appear current. Both exhaustion paths therefore stop with an explicit error/closed recovery state.
- Treating an absent optional field as zero aliases malformed or incomplete wire data with valid sequence zero. Preserve explicit message and value presence for the epoch; allow scalar sequence zero only after a valid epoch is present.
- Issuing epochs from `df-types`, a clock, a process-local counter, or the gameplay journal would duplicate durable authority and permit fork/regression after restore. Keep issuance at the protected persistence boundary using an authenticated head and conditional durable update.
- Journaling every gameplay revision would falsely promise game RPO0 and expand the protected journal into a second game-state writer. Retain the planned split: journal irreversible authority and epoch issuance; PostgreSQL gameplay snapshots retain the declared game RPO.

## Open facts and gates

The existing pure type proves ordering and checked in-epoch increment only. It cannot authenticate the protected head, perform CAS/fencing, persist issuance, prove that the latest journal head is complete, retire namespaces, reconcile unknown sends, or publish a permitted replacement snapshot. Those are unresolved G05/X02 implementation and fault-qualification gates. G03 still owns frozen native/WASM/protobuf compare encoding and compatibility fixtures; this document does not select a new wire encoding or claim generated-code, browser, restore, privacy, financial, or integrated end-to-end qualification. `u64::MAX` epoch exhaustion has no recovery bypass in this decision and requires an operator-visible fail-closed outcome plus an independently reviewed recovery procedure before any future change.

## Bounded executable evidence

The following Rust example directly imports the existing `df-types` revision module from the current assigned worktree. Extract this literal fence, check formatting, compile, and run it with the exact commands below from the worktree root. The assertions cover valid sequence zero, epoch-first tuple ordering at the maximum old sequence, next-sequence advancement and overflow rejection, zero-epoch rejection, and checked arithmetic at epoch exhaustion. Its `Option` values illustrate required wire-presence distinctions only; they do not implement or call a Prost mapper. The example is not an epoch issuer, recovery journal, restore implementation, or production integration proof.

```rust
#[path = "/Users/earlcameron/Desktop/dungeonflux/artifacts/worktrees/recovery_policy_implementation/crates/df-types/src/revision.rs"]
mod revision;

use revision::{RecoveryEpoch, RevisionError, SessionRevision};

fn main() {
    assert_eq!(RecoveryEpoch::new(0), Err(RevisionError::ZeroEpoch));

    let old_epoch = RecoveryEpoch::new(7).expect("nonzero old epoch");
    let new_epoch = RecoveryEpoch::new(8).expect("nonzero new epoch");
    let old_max = SessionRevision::new(old_epoch, u64::MAX);
    let new_zero = SessionRevision::new(new_epoch, 0);
    let new_one = new_zero.next_sequence().expect("sequence can advance");

    assert_eq!(new_zero.sequence(), 0);
    assert_eq!(new_one.sequence(), 1);
    assert_eq!(new_one.epoch().get(), 8);
    assert!(new_one > new_zero);
    assert!(new_zero > old_max);
    assert_eq!(
        old_max.next_sequence(),
        Err(RevisionError::SequenceOverflow)
    );

    // This is a checked-arithmetic boundary, not a recovery-epoch issuer.
    assert_eq!(u64::MAX.checked_add(1), None);

    // Illustrative presence values only; this is not a generated-Prost mapping.
    let missing_epoch_message: Option<Option<u64>> = None;
    let missing_epoch_value: Option<Option<u64>> = Some(None);
    let zero_epoch_value: Option<Option<u64>> = Some(Some(0));
    let missing_sequence: Option<u64> = None;
    let present_zero_sequence: Option<u64> = Some(0);

    assert_eq!(missing_epoch_message, None);
    assert_eq!(missing_epoch_value, Some(None));
    assert_eq!(zero_epoch_value, Some(Some(0)));
    assert_eq!(missing_sequence, None);
    assert_eq!(present_zero_sequence, Some(0));

    println!(
        "PASS: revision ordering, checked sequence, epoch-zero rejection, arithmetic exhaustion, and illustrative field presence"
    );
}
```

Run these commands from `/Users/earlcameron/Desktop/dungeonflux/artifacts/worktrees/recovery_policy_implementation`; the output root is the durable a2 worker directory supplied in the brief and the binary stays in this attempt's scratch directory:

```sh
awk 'BEGIN { in_code = 0; found = 0 } /^```rust$/ && !found { in_code = 1; found = 1; next } in_code && /^```$/ { exit } in_code { print } END { if (!found) exit 1 }' planning/recovery-revision-policy.md > /Users/earlcameron/Desktop/dungeonflux/development/evidence/parallel-decisions/B-G03-D03/a2/worker/recovery-policy-example.rs
/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/rustup/toolchains/1.98.1-aarch64-apple-darwin/bin/rustfmt --check --edition 2024 --config-path rustfmt.toml /Users/earlcameron/Desktop/dungeonflux/development/evidence/parallel-decisions/B-G03-D03/a2/worker/recovery-policy-example.rs
/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/rustup/toolchains/1.98.1-aarch64-apple-darwin/bin/rustc --edition=2024 -C opt-level=0 /Users/earlcameron/Desktop/dungeonflux/development/evidence/parallel-decisions/B-G03-D03/a2/worker/recovery-policy-example.rs -o /Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G03-D03-a2/recovery-policy-example
/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G03-D03-a2/recovery-policy-example
```

Retain the freshly extracted source under the durable a2 worker directory. Compare its optional-presence model with `common.proto` and `df-tools/tests/shared_contracts.rs`: the source mapping requires epoch message, epoch value, and sequence presence, validates nonzero epoch, and accepts present sequence zero. This attempt executes the literal document example and compares those protocol sources; it does not execute generated Prost mappings. Cargo, Clippy, WASM, browser, persistence, protected-head/CAS, and disaster-restore checks are outside this bounded repair and remain unperformed. The canonical `df-types` implementation itself still only proves caller-supplied nonzero epoch values, lexicographic ordering, and checked in-epoch advancement.

## Original acceptance criteria

- `lexicographic monotonicity`
- `The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.`

## Original verification

- `Freeze the cited source decision and a bounded contract example; compare lexicographic monotonicity. Retain decision, alternatives and unresolved facts.`
- `Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.`
