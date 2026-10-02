# Recovery journal watermark source decision

Task/attempt: `B-F24-D02-a1`  
Input revision: `308920e328ba6df85bea1e3d2abcbea5f1b796e6`  
Status: proposed source decision; no protected source or production recovery is implemented or qualified.

## Decision

Reuse the append-only protected recovery journal and its independently retained
authenticated head from [the canonical protected-journal policy](protected-journal-policy.md)
and [service operations](service-operations.md). The head is the sole protection
watermark for irreversible journal history. PostgreSQL backups, telemetry, local
caches, process memory, timestamps, and a restored journal's own latest row cannot
establish or replace it. Do not introduce a second journal, watermark issuer, or
recovery authority.

For each installation/recovery epoch and tenant scope, the protected source names
the last committed journal sequence and integrity-linked journal identity. Its
advance is conditional on the exact previously authenticated head and the next
sequence. A successful conditional update is the only operation that advances the
watermark. It must reject an absent/stale expected head, any sequence other than
the checked successor, and arithmetic exhaustion. Concurrent writers therefore
cannot publish a lower head, skip a sequence, or fork a valid successor. Source
initialization is a distinct, conditional create of the genesis head; it cannot
overwrite an existing head. The provider and its concrete consistency, retention,
key-custody, and conditional-write guarantees remain G05/X02 qualification choices.

Before affected private or commercial scopes reopen after restore, `df-persistence`
must authenticate the latest protected head and verify the local/restored journal's
integrity-linked contiguous history through exactly that sequence. A missing,
unauthenticated, regressed, or conflicting head; absent or incomplete range; local
history ending below the watermark; or local history extending beyond it as an
unconfirmed tail fails closed for those scopes. An unresolved append is quarantined
for reconciliation and cannot be silently used to raise the watermark. An absent
watermark is unknown history, never an empty journal. Replay privacy suppressions
before serving and overlay verified later irreversible entries as holds and
reconciliation records, as the canonical policy requires. This is a protected
recovery boundary, not gameplay-history RPO0: report the lost gameplay revision
range and preserve the declared PostgreSQL gameplay RPO.

The source receipt may expose only the authenticated head and the minimum journal
range/integrity facts needed by the persistence owner. It is not a consumer-facing
permit by itself. The native dispatcher still checks the journal-confirmed permit
and current owner fence immediately before egress; commerce retains unknown
liability; deletion/revocation acknowledgement waits for the confirmed boundary.
These are the existing consumer obligations, not new APIs in this decision.

## Alternatives and ownership

- A watermark stored only beside PostgreSQL rows or in the same backup can roll
  back with that backup, so it cannot prove that later irreversible work did not
  happen.
- A telemetry spool is operational observability with a separate SQLite lifecycle;
  treating it as authority would mix retention and trust boundaries.
- Advancing from the largest restored local sequence or repairing gaps by guessing
  can bless an unconfirmed append or hide lost entries. Refuse and reconcile.
- A second feature-specific journal or issuer would compete with the canonical
  protected-journal policy. Reuse that authority and its existing typed/source
  owners once implementation contracts are admitted.

`df-persistence` owns journal append, authenticated protected-head confirmation,
complete-range verification, restore holds/replay, conditional recovery-epoch
allocation, and receipt lookup behind its existing ports. G05 selects and qualifies
the concrete database, durable object backing, consistency, schema, key custody,
retention, migrations, backup and restore procedure; X02 qualifies the disaster
restore path and fault behavior. `df-assets` owns durable bytes and asset publication;
it does not store journal authority. The native egress/commerce/auth consumers above
apply their existing hooks. The coordinator owns sequential integration and original
whole-decision acceptance. No implementation, schema, provider, cost, concurrency,
or runtime guarantee is claimed here.

## Finite executable contract example

This std-only example checks the policy predicates over supplied facts. It does not
authenticate source data, perform storage/CAS, prove contiguous hashes, or recover a
service. `None` for the protected head is a refusal, not an empty-history default.
Equality of a supplied local contiguous end and authenticated watermark is required;
both a missing range and an ahead tail refuse serving.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Decision {
    Allow,
    Refuse,
}

fn restore(
    authenticated: bool,
    protected_head: Option<u64>,
    local_contiguous_end: Option<u64>,
) -> Decision {
    match (authenticated, protected_head, local_contiguous_end) {
        (true, Some(head), Some(end)) if head == end => Decision::Allow,
        _ => Decision::Refuse,
    }
}

fn advance(current: Option<u64>, expected: Option<u64>, proposed: u64) -> Decision {
    match (current, expected) {
        (None, None) if proposed == 1 => Decision::Allow,
        (Some(head), Some(expected_head))
            if head == expected_head && expected_head.checked_add(1) == Some(proposed) =>
        {
            Decision::Allow
        }
        _ => Decision::Refuse,
    }
}

fn main() {
    assert_eq!(restore(true, Some(7), Some(7)), Decision::Allow);
    assert_eq!(restore(false, Some(7), Some(7)), Decision::Refuse);
    assert_eq!(restore(true, None, Some(0)), Decision::Refuse);
    assert_eq!(restore(true, Some(7), None), Decision::Refuse);
    assert_eq!(restore(true, Some(7), Some(6)), Decision::Refuse);
    assert_eq!(restore(true, Some(7), Some(8)), Decision::Refuse);

    assert_eq!(advance(None, None, 1), Decision::Allow);
    assert_eq!(advance(Some(7), Some(7), 8), Decision::Allow);
    assert_eq!(advance(Some(7), Some(6), 8), Decision::Refuse);
    assert_eq!(advance(Some(7), Some(7), 9), Decision::Refuse);
    assert_eq!(advance(Some(u64::MAX), Some(u64::MAX), 0), Decision::Refuse);
}
```

The cases exercise a matching authenticated source/range, unauthenticated and
missing protected heads, missing/behind/ahead local history, conditional genesis,
one valid successor, stale expected state, a skipped sequence, and exhaustion.
They test only this finite decision contract; production source authenticity,
conditional-write linearizability, contiguous journal integrity, concurrency,
restore/replay, privacy, financial outcomes, provider behavior and paid readiness
remain unresolved qualification and integration gates.

## Source identity and verification boundary

Governing inputs were read at this attempt's input revision. Their SHA-256 values
are retained in the handoff. The literal above is extracted from this document and
checked with repository `rustfmt.toml`, then compiled with Rust 1.98.1, edition
2024, `-D warnings`, and executed under the wave's guarded command v3. Exact argv,
receipt paths, source/config/tool hashes, output and exit status are retained in
`handoff.json`. No Cargo, browser, provider, cryptographic, PostgreSQL, restore
drill, or device check was performed. Independent frontier review and root
integration remain pending.

Original acceptance and verification are unchanged; see the retained task record
`original-task.json` in this attempt's evidence directory.
