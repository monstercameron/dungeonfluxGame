# Tenant repository scope

Task/attempt: `B-X01-D01/a1`
Input revision: `308920e328ba6df85bea1e3d2abcbea5f1b796e6`
Status: source-backed policy decision and finite illustrative Rust contract; repository implementation, schema and production qualification remain pending.

## Decision

Every read or write that can observe or change tenant-owned state must be executed with a current trusted `TenantScope` and the relevant `AuthorizedAudience` produced by `df-auth`. The adapter must constrain every base row and every joined/derived row by that tenant scope, then apply the authorized audience rule before it counts, orders, limits, aggregates, serializes, caches, or returns data. An opaque row/session/asset/operation identifier, trace context, request-supplied tenant identifier, prior snapshot, cached authorization, or knowledge of a content hash grants no access. A caller without current authorization gets a typed permission/unavailable outcome that does not distinguish another tenant's row from a missing row.

This makes the trusted principal and audience part of each repository operation's required input, alongside tenant scope. `df-auth` remains the sole principal, membership and audience authority. `df-session` owns the `SessionRepository` port and serializes session decisions; `df-persistence` implements consumer-owned ports and PostgreSQL queries. `df-assets` owns byte access and its metadata/access port; it reauthorizes manifests and byte opens. `df-commerce` owns tenant/customer/ledger policy and its ports carry trusted tenant scope. `df-api` authenticates and asks the owning authority for access, then projects only permitted results. No caller may implement a second membership registry or make an API handler the access authority. `OperationContext` and observability context correlate work but do not authenticate it.

At PostgreSQL, use the runtime role that is neither table owner nor superuser nor `BYPASSRLS`; force row-level security where applicable, and retain explicit parameterized tenant predicates in every query. Apply trusted scope to transaction-local database settings, reset it for every pooled transaction, and bind values rather than interpolating them. RLS is a second enforcement layer, not a replacement for the application principal/audience predicate. Administrative migration, backup and recovery roles remain separate and are never exposed to request-serving code. The repository query must bind authorization and data access to one transaction/decision boundary; do not authorize from a stale process cache and then query later under broader scope.

For owner-private rows, audience filtering also requires the row owner to be the authorized principal (or another explicit audience grant supplied by `df-auth`). A shared row is visible only where the current audience explicitly includes it. A tenant match alone is insufficient for another member's character sheet, capture, private dialogue, credentials, private asset, or other audience-restricted payload. Joins, search, pagination, counts, existence checks, exports, restore reads, temporary asset URLs, and cache hits obey the same rule. Apply scope before `LIMIT`/cursor advancement so hidden rows cannot leak through page sizes or omissions. Cache keys bind tenant, principal/audience, authorization/suppression generation and source revision; cache retrieval rechecks current access and clears obsolete private material on revocation.

Writes must establish tenant and row ownership from the trusted scope and persisted relationships, not copy a request's tenant/owner fields. Inserts, updates and deletes carry tenant predicates plus expected revision/owner fence as required by the owning contract. A foreign key or uniqueness conflict that reveals a cross-tenant row maps to a nonexistence-safe public result. Unknown outcomes after a storage failure remain unknown; do not retry a potentially committed write under a newly broadened scope or report success before durable commit.

These rules apply to authoritative and private data. The development workflow SQLite and runtime telemetry SQLite have separate purposes and are not tenant repositories for gameplay data. Telemetry is diagnostic, not a bypass for private content: redact credentials and private payloads before serialization, and never rely on trace identifiers for access.

## Source basis and alternatives

This decision reuses the existing policy in [Service operations](service-operations.md#tenant-isolation-and-hostile-input): every repository/asset/history/commerce operation needs trusted tenant scope plus audience; opaque IDs alone are insufficient; runtime PostgreSQL role separation, FORCE RLS where applicable, explicit predicates, transaction-local settings and paired cross-tenant fixtures are already required. [Subsystem interfaces](subsystem-interfaces.md#identity-and-session-ownership) separately makes `df-auth` the owner of `Principal` and `AuthorizedAudience`, and names consumer-owned session and asset repository ports. Its PostgreSQL section assigns adapters and metadata access to `df-persistence` and documents scope-bearing asset operations. [Subsystem architecture](subsystem-architecture.md) assigns principal/audience decisions to `df-auth`, durable adapter work to `df-persistence`, bytes to `df-assets`, and commerce to `df-commerce`; the engine receives only a pure audience scope after authorization. [Storage architecture](storage-architecture.md) requires PostgreSQL for durable game state and explicitly leaves schema, indexes, pool settings and migrations to the storage gate. [Erasure and retention](erasure-retention-policy.md) requires current tenant/audience authorization and suppression checks before private reads and again before publication, including after restore. [Asset engine](asset-engine.md) requires private assets filtered before manifests/provider dispatch and access revalidation. These sources form one policy; this document specifies its repository-wide query consequence and does not add another authorization authority.

Alternative approaches considered:

- **Filter only in API handlers or after loading rows:** rejected because joins, counts, pagination, internal callers and serialization/cache paths can expose cross-tenant or private-row facts before the handler filter.
- **Rely only on RLS:** rejected because privileged/owner roles can bypass policies, row ownership/audience can be finer than tenant, and application authorization must bind the principal to access. RLS remains defense in depth.
- **Trust opaque identifiers or a request-selected tenant:** rejected because identifiers are not capabilities and callers can submit a foreign ID or tenant value.
- **Create a generic repository/auth crate or duplicate access policy in each adapter:** rejected because existing ownership puts principal/audience decisions in `df-auth` and consumer-owned repository ports in their domain/session owners; duplicate policies drift.
- **Use a tenant-only predicate for every row:** insufficient for same-tenant private data. Tenant scope is mandatory, with audience filtering wherever row visibility is narrower than tenant membership.

## Bounded contract example

The standalone, standard-library-only Rust example below exercises the policy at the repository boundary. Its structs are explanatory fixtures, not production APIs or SQL. `AuthorizedScope` stands for a fresh result from the existing `df-auth` authority; a production adapter must obtain/revalidate that authority at the boundary. The example's fixture generation counter models invalidation/revocation freshness. It checks that a foreign tenant, same-tenant private peer, mismatched caller, and stale scope are refused while a tenant-visible row and own private row are returned.

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct Tenant(u8);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct Principal(u8);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct RowId(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Audience {
    TenantMembers,
    Owner(Principal),
}

#[derive(Clone, Copy, Debug)]
struct Row {
    id: RowId,
    tenant: Tenant,
    audience: Audience,
}

#[derive(Debug, Eq, PartialEq)]
enum Refusal {
    CallerMismatch,
    StaleAuthorization,
}

// Fixture stand-in for a current df-auth AuthorizedAudience + TenantScope.
#[derive(Debug)]
struct AuthorizedScope {
    tenant: Tenant,
    principal: Principal,
    generation: u64,
}

struct Repository {
    rows: Vec<Row>,
    current_generation: u64,
}

impl Repository {
    fn query(&self, caller: Principal, scope: &AuthorizedScope) -> Result<Vec<RowId>, Refusal> {
        if caller != scope.principal {
            return Err(Refusal::CallerMismatch);
        }
        if scope.generation != self.current_generation {
            return Err(Refusal::StaleAuthorization);
        }

        Ok(self
            .rows
            .iter()
            .filter(|row| row.tenant == scope.tenant)
            .filter(|row| match row.audience {
                Audience::TenantMembers => true,
                Audience::Owner(owner) => owner == scope.principal,
            })
            .map(|row| row.id)
            .collect())
    }
}

fn main() {
    let alice = Principal(1);
    let bob = Principal(2);
    let acme = Tenant(10);
    let other_tenant = Tenant(20);
    let repo = Repository {
        rows: vec![
            Row {
                id: RowId(1),
                tenant: acme,
                audience: Audience::TenantMembers,
            },
            Row {
                id: RowId(2),
                tenant: acme,
                audience: Audience::Owner(alice),
            },
            Row {
                id: RowId(3),
                tenant: acme,
                audience: Audience::Owner(bob),
            },
            Row {
                id: RowId(4),
                tenant: other_tenant,
                audience: Audience::TenantMembers,
            },
            Row {
                id: RowId(5),
                tenant: other_tenant,
                audience: Audience::Owner(alice),
            },
        ],
        current_generation: 7,
    };
    let alice_scope = AuthorizedScope {
        tenant: acme,
        principal: alice,
        generation: 7,
    };

    // Tenant-visible plus Alice's own private row; excludes Bob and foreign-tenant rows.
    assert_eq!(
        repo.query(alice, &alice_scope).unwrap(),
        vec![RowId(1), RowId(2)]
    );
    assert_eq!(repo.query(bob, &alice_scope), Err(Refusal::CallerMismatch),);

    let stale_scope = AuthorizedScope {
        generation: 6,
        ..alice_scope
    };
    assert_eq!(
        repo.query(alice, &stale_scope),
        Err(Refusal::StaleAuthorization),
    );

    // The same principal in another tenant requires a separately authorized scope.
    let other_scope = AuthorizedScope {
        tenant: other_tenant,
        principal: alice,
        generation: 7,
    };
    assert_eq!(
        repo.query(alice, &other_scope).unwrap(),
        vec![RowId(4), RowId(5)]
    );

    // A scope for that tenant never exposes its rows to an Acme query.
    assert!(!repo.query(alice, &alice_scope).unwrap().contains(&RowId(4)));
}
```

The contract intentionally returns row IDs only after both tenant and audience predicates. A production implementation additionally needs current authorization and suppression checks at each actual read/publication boundary, transaction-bound scope, error noninterference, cache invalidation and bounded query behavior; this in-memory fixture cannot establish those properties.

## Integration owners and failure semantics

- `df-auth` owns authenticate/authorize, current membership/grant/audience decisions, revocation and the trusted tenant scope; callers cannot construct authority from user fields.
- Each consumer owns its port (`df-session` session repository, `df-assets` access/metadata, `df-commerce` commerce repository, and other private-record owners). `df-persistence` implements PostgreSQL adapters and migrations. The server wires these ports; `df-api` projects authorized DTOs.
- The coordinator owns integration of this policy across repository, history, commerce, assets, search, restore, caches, exports and query fixtures. It also owns the later G03/G05 concrete types/schema, DB roles/RLS policies, SQL query audit and S00/S01 cross-system acceptance.

Missing, expired, revoked, stale-generation, wrong-tenant or wrong-audience authority fails closed before data is returned or mutation committed. Storage unavailability produces a typed unavailable/unknown outcome, not an empty-success fallback. Scope validation failure does not reveal whether the requested row exists. Queries filter before pagination/aggregation; if one component of a joined result lacks current access, omit or fail the result according to its owning typed contract without leaking that component's existence. Revocation after a read but before publication requires revalidation and suppression of queued/cached private data, as specified by session, asset and erasure owners.

## Pending production gates and unperformed work

This decision does not freeze a Rust API, SQL/schema, tenant/membership model, RLS policy text, query-builder, pool/session-setting mechanism, index plan, cache implementation or migration. Those require reviewed G03 concrete types and G05 storage work. The canonical `df-auth` principal/audience model and relevant repository ports must be materialized before integration; they are named owners in current planning but are not compiled runtime code in this checkout. Security-definer functions, cross-tenant unique constraints and operational migration/backup access need explicit SQL review. Exact error mapping and whether a mixed joined result omits or rejects a row are owning-contract details, but must remain nonexistence-safe. Current pooled-connection reset behavior, transaction isolation, RLS coverage and every query path have not been measured or tested against PostgreSQL.

Paired adversarial production fixtures must cover tenant A/B, two principals in one tenant, foreign opaque IDs, joins, aggregates/count/existence, search, keyset pagination, cache reuse after role change/revocation, temporary URLs, history/export, restore of a pre-deletion backup, and updates/deletes. Tests must inspect the runtime role and forced policies and prove that the same predicates execute for every access path. Run native/WASM workspace checks, PostgreSQL integration/restart/restore tests, browser/device behavior and deployed role review only after those production surfaces exist. This attempt validates only the finite Rust policy fixture; it makes no game, payment, provider, browser, phone, production security or deployment claim.

## Preserved acceptance and verification

Acceptance: “every query principal-filtered”; “The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.”

Original verification: freeze the cited source decision and bounded contract example, compare every query principal-filtered, and retain decision, alternatives and unresolved facts. The exact source revision, literal hash, formatter/compiler/fixture commands and receipts are recorded in this attempt's handoff. Native/WASM workspace checks remain unperformed because this is a policy document with no application source change.
