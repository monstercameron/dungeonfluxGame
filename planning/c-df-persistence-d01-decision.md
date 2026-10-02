# C-df-persistence D01: consumer-owned PostgreSQL mappings

Task/attempt: `B-C-df-persistence-D01-a1`
Input revision: `4c0e050c2d30e632bd3b32c51a92cfda52479e6e`
Status: proposed bounded design; independent review and integration pending.

## Decision and ownership

`df-persistence` implements PostgreSQL mappings for ports owned by their consumers.
`df-model` owns checkpoint/state meaning and compatibility; `df-session` owns
`SessionRepository`, `EffectRepository`, `MemoryCandidateStore` and accepted-work
lifetime; `df-auth` owns `CredentialStore`, trusted scope and `EntitlementReader`;
`df-assets` owns asset metadata/access; `df-provider-api` owns recording/budget
ports. `df-commerce` owns its `CommerceRepository`, commercial relationships,
grants, prices and single exact ledger. `BudgetStore` delegates to this authority.
Adapters depend on those contracts; consumers never import SQL or persistence.
No repository-owned replacement domain types, wallet, rules, or permission issuer
is introduced. Correlation IDs are not authorization.

Use explicit relational keys and bounded versioned `BYTEA` documents for the
initial snapshot recovery spine. These are mapping requirements for the next
adapter, not applied tables or migrations. Indexed extracted fields support the
known accesses; entity decomposition and additional indexes require G05/G10
workload evidence. A blob-only store with unbounded scans and fully decomposing
all game state now are both rejected: the former cannot support scoped access,
the latter freezes unmeasured domain detail. SQLite gameplay and external graph
databases are rejected; workflow/devlog SQLite and telemetry SQLite remain separate.

| Consumer contract / proposed row family | Required physical basis and access |
| --- | --- |
| SessionRepository: sessions, runs, membership | Tenant-aware keys/FKs; session/current run, full revision, pinned rules/content, owner fence/lease; load by trusted tenant/session. Membership identity survives new runs. |
| df-model checkpoint: snapshots, checkpoints | Tenant/session/run, schema/codec version, full revision and immutable provenance; bounded bytes retain rules source/catalog/handler/content pins and origin. Read exact checkpoint or latest compatible session snapshot. |
| SessionRepository: operation/allocation results, retired namespaces | Tenant/principal/command namespace/operation and canonical fingerprint; session/run/recovery namespace where applicable. Bootstrap uses its scoped pre-principal key; Create/Join allocate with the result. Result expiry preserves a nonreplayable tombstone/retired namespace. |
| SessionRepository: decisions, facts, director state | Tenant/session/run, full revision plus within-decision order, source/handler/policy versions, accepted semantics and draws; bounded authorized history pages. Canonical truth remains snapshots/committed decisions. |
| EffectRepository: intents, jobs, dispatch attempts | Tenant/session/recovery namespace/operation/slot uniqueness, stable effect/job/attempt, run/process generation, owner fence, status and reservation linkage. Dispatching/Unknown cannot become unsent through lease expiry. |
| MemoryCandidateStore: episodes, summaries, derived indexes | Tenant/session/run, observer access/source/index generation, source digest, coverage and attributed references. Filter access before candidate selection; bounded canonical fallback or Incomplete; atomic generation publication fences source/access. No index replaces canonical history. |
| Content/asset metadata: packages, publications, manifests | Immutable package/draft/version/hash/rights and dependency references; tenant/access generation and approved export/bookend/identity/award provenance. Complete validated durable bytes precede visible metadata. |
| CredentialStore / EntitlementReader | Scoped credential generations/revocations and versioned commerce grant projection. Secrets stay in auth storage; projection is derived from the one ledger and cannot authorize by stale cache. |
| CommerceRepository / BudgetStore / RecordingStore | Tenant/payer/campaign/grant/quote/price/reservation/attempt, exact tagged units, append-only ledger entries, stable payment/webhook/provider IDs and complete recording manifests. Unknown liability remains full; no second wallet. |
| Lifecycle suppression and protected journal references | Tenant/subject/rights/suppression/backup generations, redaction/tombstones, journal-confirmed permit/head identity. Independently protected journal bytes/head are outside the old PostgreSQL backup; missing latest verified range fails closed. |

Every tenant-owned row and unique/FK key includes `tenant_id`; opaque IDs alone
are insufficient. D03 owns trusted transaction scope, explicit predicates, FORCE
RLS and pooled transaction reset qualification. D02 owns atomic fence/CAS,
bootstrap/allocation/result/intent writes and ambiguity lookup. Its receipt proves
known durable commit only; queue acceptance is distinct. No transaction spans
provider calls, journal object I/O, or client waits. Multi-store irreversible
publication follows the existing journal-confirmation choreography; this mapping
does not invent an atomic PostgreSQL/object-store transaction.

Reuse canonical nonzero 16-byte `df-types` identities without byte reordering:
propose `BYTEA` plus exact-length/nonzero constraints, not a UUID parser that adds
version assumptions. Commercial IDs remain distinct future canonical wrappers;
do not alias SessionId to TenantId. Store RecoveryEpoch and sequence as
`NUMERIC(20,0)` constrained to `1..=u64::MAX` and `0..=u64::MAX`, respectively,
and compare `(epoch, sequence)` lexicographically. `BIGINT` narrowing or a scalar
revision is rejected. A parsed epoch does not issue/authenticate a recovery epoch.
Store Money micros and Usage quantities as `NUMERIC(39,0)` constrained to
`0..=u128::MAX`, preserving explicit currency/usage kind. No float, implicit FX,
signed adjustment masquerading as Money, or narrowing gateway conversion.
Reversals use commerce-owned typed append entries. Schema tags, enum discriminants,
integer ranges and byte lengths validate before constructing consumer values;
unknown versions, corrupt mappings and scope mismatches are typed failures.

Before serialization/publication, the consumer validates the complete envelope;
on load, the adapter checks decoded envelope tenant/session/run/revision against
row keys and invokes consumer compatibility validation. Physical key extraction
never silently overwrites payload scope or pins. Missing/redacted records return
the consumer's explicit gap rather than empty success or regenerated private data.
Immutable nonpersonal provenance can survive redaction; it cannot reconstruct a
deleted payload. Restores retain lost-game-range disclosure, retire old namespaces,
and require a strictly newer verified protected epoch before serving restored data.

## Finite executable mapping contract

The private checkpoint and liability records below stand in for two consumer
contracts that are not implemented yet. They are deliberately narrow, not a full
checkpoint codec, repository fake, production interface, authentication system or
ledger implementation. Checkpoint marker bytes demonstrate exact opaque-byte
preservation; the real codec must retain the complete consumer-owned envelope and
pins. Tenant(u8) stands in for already authorized scope, without parsing or minting
a production TenantId. Actual SessionId, RunId, SessionRevision, Currency and Money
come from the frozen worktree's `df-types` library. Decimal strings illustrate
exact SQL numeric values; no SQL driver or PostgreSQL has been run.

```rust
use df_types::{Currency, Money, RecoveryEpoch, RunId, SessionId, SessionRevision};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tenant(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Scope {
    tenant: Tenant,
    session: SessionId,
    run: RunId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Checkpoint {
    scope: Scope,
    revision: SessionRevision,
    marker: [u8; 4],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckpointRow {
    tenant: Tenant,
    session: Vec<u8>,
    run: Vec<u8>,
    owner: u16,
    schema: i32,
    epoch: String,
    sequence: String,
    marker: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Refusal {
    ScopeMismatch,
    WrongOwner,
    UnsupportedSchema,
    InvalidIdentity,
    InvalidNumeric,
    InvalidEpoch,
    InvalidPayload,
    InvalidCurrency,
}

fn exact_unsigned(text: &str, max_digits: usize) -> Result<u128, Refusal> {
    if text.is_empty()
        || text.len() > max_digits
        || !text.bytes().all(|byte| byte.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return Err(Refusal::InvalidNumeric);
    }
    text.parse().map_err(|_| Refusal::InvalidNumeric)
}

fn to_row(trusted: Scope, checkpoint: Checkpoint) -> Result<CheckpointRow, Refusal> {
    if checkpoint.scope != trusted {
        return Err(Refusal::ScopeMismatch);
    }
    Ok(CheckpointRow {
        tenant: trusted.tenant,
        session: trusted.session.as_bytes().to_vec(),
        run: trusted.run.as_bytes().to_vec(),
        owner: 1,
        schema: 1,
        epoch: checkpoint.revision.epoch().get().to_string(),
        sequence: checkpoint.revision.sequence().to_string(),
        marker: checkpoint.marker.to_vec(),
    })
}

fn from_row(trusted: Scope, row: &CheckpointRow) -> Result<Checkpoint, Refusal> {
    if row.tenant != trusted.tenant {
        return Err(Refusal::ScopeMismatch);
    }
    if row.owner != 1 {
        return Err(Refusal::WrongOwner);
    }
    if row.schema != 1 {
        return Err(Refusal::UnsupportedSchema);
    }
    let session = SessionId::from_bytes(&row.session).map_err(|_| Refusal::InvalidIdentity)?;
    let run = RunId::from_bytes(&row.run).map_err(|_| Refusal::InvalidIdentity)?;
    if session != trusted.session || run != trusted.run {
        return Err(Refusal::ScopeMismatch);
    }
    let epoch =
        u64::try_from(exact_unsigned(&row.epoch, 20)?).map_err(|_| Refusal::InvalidNumeric)?;
    let sequence =
        u64::try_from(exact_unsigned(&row.sequence, 20)?).map_err(|_| Refusal::InvalidNumeric)?;
    let epoch = RecoveryEpoch::new(epoch).map_err(|_| Refusal::InvalidEpoch)?;
    let marker = row
        .marker
        .as_slice()
        .try_into()
        .map_err(|_| Refusal::InvalidPayload)?;
    Ok(Checkpoint {
        scope: trusted,
        revision: SessionRevision::new(epoch, sequence),
        marker,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LiabilityRow {
    tenant: Tenant,
    owner: u16,
    schema: i32,
    currency: String,
    micros: String,
}

fn liability_to_row(trusted: Tenant, amount: Money) -> LiabilityRow {
    LiabilityRow {
        tenant: trusted,
        owner: 2,
        schema: 1,
        currency: amount.currency().as_str().to_owned(),
        micros: amount.micros().to_string(),
    }
}

fn liability_from_row(trusted: Tenant, row: &LiabilityRow) -> Result<Money, Refusal> {
    if row.tenant != trusted {
        return Err(Refusal::ScopeMismatch);
    }
    if row.owner != 2 {
        return Err(Refusal::WrongOwner);
    }
    if row.schema != 1 {
        return Err(Refusal::UnsupportedSchema);
    }
    let currency = Currency::parse(&row.currency).map_err(|_| Refusal::InvalidCurrency)?;
    Ok(Money::new(currency, exact_unsigned(&row.micros, 39)?))
}

fn main() {
    let scope = Scope {
        tenant: Tenant(7),
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
    };
    for epoch in [1, i64::MAX as u64 + 1, u64::MAX] {
        for sequence in [0, i64::MAX as u64 + 1, u64::MAX] {
            let checkpoint = Checkpoint {
                scope,
                revision: SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), sequence),
                marker: [0, 255, 3, 4],
            };
            let row = to_row(scope, checkpoint).unwrap();
            assert_eq!(from_row(scope, &row), Ok(checkpoint));
            let foreign = Scope {
                tenant: Tenant(8),
                ..scope
            };
            assert_eq!(to_row(foreign, checkpoint), Err(Refusal::ScopeMismatch));
            assert_eq!(from_row(foreign, &row), Err(Refusal::ScopeMismatch));
        }
    }
    let checkpoint = Checkpoint {
        scope,
        revision: SessionRevision::new(RecoveryEpoch::new(2).unwrap(), 0),
        marker: [1, 2, 3, 4],
    };
    let row = to_row(scope, checkpoint).unwrap();
    assert_eq!(
        from_row(
            Scope {
                session: SessionId::from_bytes(&[3; 16]).unwrap(),
                ..scope
            },
            &row
        ),
        Err(Refusal::ScopeMismatch)
    );
    assert_eq!(
        from_row(
            Scope {
                run: RunId::from_bytes(&[3; 16]).unwrap(),
                ..scope
            },
            &row
        ),
        Err(Refusal::ScopeMismatch)
    );
    let mut invalid = row.clone();
    invalid.owner = 2;
    assert_eq!(from_row(scope, &invalid), Err(Refusal::WrongOwner));
    invalid = row.clone();
    invalid.schema = 2;
    assert_eq!(from_row(scope, &invalid), Err(Refusal::UnsupportedSchema));
    for bytes in [vec![0; 16], vec![1; 15], vec![1; 17]] {
        invalid = row.clone();
        invalid.session = bytes;
        assert_eq!(from_row(scope, &invalid), Err(Refusal::InvalidIdentity));
    }
    invalid = row.clone();
    invalid.marker.push(5);
    assert_eq!(from_row(scope, &invalid), Err(Refusal::InvalidPayload));
    for numeric in [
        "",
        "-1",
        "+1",
        "01",
        "1.0",
        "1e2",
        " 1",
        "18446744073709551616",
    ] {
        invalid = row.clone();
        invalid.epoch = numeric.to_owned();
        assert_eq!(from_row(scope, &invalid), Err(Refusal::InvalidNumeric));
        invalid = row.clone();
        invalid.sequence = numeric.to_owned();
        assert_eq!(from_row(scope, &invalid), Err(Refusal::InvalidNumeric));
    }
    invalid = row.clone();
    invalid.epoch = "0".to_owned();
    assert_eq!(from_row(scope, &invalid), Err(Refusal::InvalidEpoch));
    let previous = SessionRevision::new(RecoveryEpoch::new(1).unwrap(), u64::MAX);
    assert!(checkpoint.revision > previous);
    for currency in ["USD", "EUR"] {
        for micros in [0, 1, u128::MAX] {
            let amount = Money::new(Currency::parse(currency).unwrap(), micros);
            let row = liability_to_row(scope.tenant, amount);
            assert_eq!(liability_from_row(scope.tenant, &row), Ok(amount));
            assert_eq!(
                liability_from_row(Tenant(8), &row),
                Err(Refusal::ScopeMismatch)
            );
        }
    }
    let money = Money::new(Currency::parse("USD").unwrap(), 1);
    let money_row = liability_to_row(scope.tenant, money);
    let mut invalid_money = money_row.clone();
    invalid_money.owner = 1;
    assert_eq!(
        liability_from_row(scope.tenant, &invalid_money),
        Err(Refusal::WrongOwner)
    );
    invalid_money = money_row.clone();
    invalid_money.schema = 0;
    assert_eq!(
        liability_from_row(scope.tenant, &invalid_money),
        Err(Refusal::UnsupportedSchema)
    );
    invalid_money = money_row.clone();
    invalid_money.currency = "usd".to_owned();
    assert_eq!(
        liability_from_row(scope.tenant, &invalid_money),
        Err(Refusal::InvalidCurrency)
    );
    for numeric in ["-1", "1.1", "00", "340282366920938463463374607431768211456"] {
        invalid_money = money_row.clone();
        invalid_money.micros = numeric.to_owned();
        assert_eq!(
            liability_from_row(scope.tenant, &invalid_money),
            Err(Refusal::InvalidNumeric)
        );
    }
    assert_eq!(from_row(scope, &row), Ok(checkpoint));
    assert_eq!(liability_from_row(scope.tenant, &money_row), Ok(money));
    println!("PASS: scoped checkpoint and exact commercial amount round trips; mapping refusals");
}
```

## Next implementation and evidence limits

The next consumer is `df-session` snapshot load/commit with `df-model`'s reviewed
checkpoint envelope. Minimum later source areas are the consumer port/envelope,
`df-persistence` private snapshot codec/rows and reviewed PostgreSQL migration,
plus `df-server` wiring and PostgreSQL boundary fixtures. Those paths are proposals,
not edits authorized here. Commerce/entitlement adapters subsequently reuse their
own reviewed ports, tagged Money and current grant under atomic admission.

D01 mapping, D02 transaction fences, D03 tenant isolation and model D01 envelope
are separately owned comparison decisions until independently approved. The
finite fixture cannot prove envelope pin completeness, real PostgreSQL numeric
driver behavior/constraints/RLS, durability, atomic transactions, concurrent
admission, bounded queries, query plans or migration/restore compatibility.
Real PostgreSQL mapping, role/pool, crash/timeout, lost receipt, redaction/replay,
protected journal and realistic workload qualification remain unperformed G05/X02
work. No database mutation, migration, provider call or production repository fake
is delivered. Cargo/Clippy, WASM, browser/audio and deployed OTEL are unperformed;
this internal standalone literal has no client or runtime logging path.

The attempt evidence retains exact Markdown extraction/byte comparison, pinned
rustfmt write/check with root config, Rust 2024 rustc with warnings denied against
actual df-types, and finite execution under the frozen v6 admission/cap guard.
Scoped handoff records actual outcomes, tool/source/config hashes and argv; this
paragraph defines the checks and does not itself assert that they passed.
Production adapters must emit classified storage/schema/scope/timeout/ambiguity
outcomes and latency through shared OTEL, without private bytes, credentials or
unrestricted SQL parameters; diagnostics never replace authoritative records.

## Governing basis

The brief's frozen sections of [Subsystem architecture](subsystem-architecture.md)
and [Subsystem interfaces](subsystem-interfaces.md) govern contract ownership and
the existing row groups. [Storage architecture](storage-architecture.md),
[Runtime reliability](runtime-reliability.md), [RPC API](rpc-api.md) and
[Service operations](service-operations.md) govern PostgreSQL authority,
composite revisions, fences, retries, isolation, suppression and recovery.
[Campaign authoring](campaign-authoring.md), [Long-horizon state](long-horizon-state.md)
and [Campaign cinematics](campaign-cinematics.md) govern immutable provenance,
derived retrieval and observer/rights-scoped metadata. [Commerce service](commerce-service.md)
and the integrated commerce D01–D03 decisions preserve the one ledger/consumer
authority; [Commercial validation](commercial-validation.md) grants no clearance
or income claim. AGENTS, [Coding style](coding-style.md),
[Observability](observability.md), ADR 0001–0005, root formatter/toolchain and the
frozen backlog C-df-persistence/D01 basis govern bounded evidence and review.
