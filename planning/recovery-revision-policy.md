# Recovery revision policy

Task/attempt: `B-G03-D03/a1`

Input source: `4ba37943691f3a01a49b8238983e82527bec91bf`

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

A small standalone Rust harness imports the exact current `crates/df-types/src/revision.rs` file by path, then exercises its public `RecoveryEpoch` and `SessionRevision` API with rustc 1.98.1. It checks zero rejection, valid sequence zero, lexicographic `(8, 0) > (7, u64::MAX)`, and checked sequence overflow. `Option` assertions model absent epoch message / absent epoch value / present zero epoch, and absent sequence / present zero sequence; these assertions are policy-model evidence only, not execution of prost-generated mappings. `u64::MAX.checked_add(1) == None` records the exhaustion boundary; the harness is not an epoch issuer. The existing `df-tools/tests/shared_contracts.rs` `read_revision`/`write_revision` mapping and `missing_and_zero_recovery_components_and_old_scalar_ambiguity_reject` fixture were source-compared for message/value/sequence presence, nonzero epoch validation, and legacy-scalar rejection; that generated-mapping fixture was not run. This direct-module harness is not a Cargo build or a build of the complete canonical `df-types` library. Its source, compiler identity, command, output, exit status and hashes are retained in the assigned attempt output. No Cargo, Clippy, WASM, protobuf generation, generated-mapping test, browser, persistence, or restore fixture was run.

## Original acceptance criteria

- `lexicographic monotonicity`
- `The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.`

## Original verification

- `Freeze the cited source decision and a bounded contract example; compare lexicographic monotonicity. Retain decision, alternatives and unresolved facts.`
- `Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.`
