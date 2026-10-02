# Tenant RLS and pooled transaction context

Task: B-C-df-persistence-D03 / B-C-df-persistence-D03-a1. Input revision:
`4c0e050c2d30e632bd3b32c51a92cfda52479e6e`. Status: bounded DESIGN submission;
production SQL policies, role privileges and pool behavior remain unqualified.

## Governing decision

[Service operations](service-operations.md), “Tenant isolation and hostile input”,
requires trusted `TenantScope` plus audience, a runtime role that is neither owner
nor superuser/BYPASSRLS, FORCE RLS where applicable, explicit tenant predicates and
transaction-local settings cleared on pooled reuse. [Commerce service](commerce-service.md),
“Account and tenant lifecycle”, mints scope from authenticated permission.
[Storage architecture](storage-architecture.md), “PostgreSQL planning requirements”,
and [Subsystem interfaces](subsystem-interfaces.md), “PostgreSQL persistence and
durable media”, retain bounded native adapters, separate migration ownership and
consumer-owned repository contracts. [Coding style](coding-style.md) governs the
private illustrative Rust contract. This decision creates no production public
type, migration, service, authentication provider or database command.

Select one native adapter transaction scope: verified current principal and tenant
permission -> explicit consumer `TenantScope` -> begin -> transaction-local context
-> scoped statements -> commit/rollback -> clean pool return. Auth D01 owns minting
and current permission; tenant scope grants neither another member's audience nor
payer/operator authority. Persistence D01 owns tenant-bearing keys/envelopes and
tenant-aware foreign keys/uniqueness; D02 owns fences, idempotency and atomic writes.
Those sibling drafts are comparison inputs pending independent approval.

Every tenant-owned table uses a non-null tenant key and enabled RLS with an explicit
tenant equality predicate in both `USING` and `WITH CHECK`. Require FORCE RLS for
these tables. The separately provisioned runtime role has NOSUPERUSER, NOBYPASSRLS,
NOCREATEROLE, no table/schema ownership, no privileged role membership or permission
to switch to such a role, and only required scoped DML privileges. Do not grant
TRUNCATE, schema creation, policy changes, or unrestricted SECURITY DEFINER routines.
The migration/backup role and its credentials never enter runtime pool configuration.
Audit views/functions and every applicable policy: an additional permissive policy
must not widen the tenant predicate. RLS does not mediate TRUNCATE or referential
integrity checks; map constraint errors to nonexistence-safe public failures.
[PostgreSQL 18 row security](https://www.postgresql.org/docs/18/ddl-rowsecurity.html).

Require a database-verified transaction scope binding before any tenant statement.
The database resolver must validate a binding issued through the approved auth path
and recover its authorized tenant; runtime SQL cannot mint, edit or rebind that
authority. The RLS predicate consumes this verified result, never an unsigned tenant
setting. Missing, empty, malformed, expired, revoked or unverifiable binding fails
closed. Explicit repository predicates additionally bind the same trusted tenant;
request headers, foreign IDs and trace correlation cannot select it. Transaction-local
settings may carry the binding, using parameterized `set_config(..., ..., true)`,
but setting its bytes is not verification. No session-level SET is permitted by the
adapter. Transaction-local configuration expires at commit or rollback.
[PostgreSQL configuration functions](https://www.postgresql.org/docs/18/functions-admin.html)
and [SET LOCAL](https://www.postgresql.org/docs/18/sql-set.html).

On checkout verify the connection is idle and has no surviving tenant context. Never
change tenant during an active transaction. On error, cancellation or deadline,
roll back and await driver confirmation before returning the connection. Drop it
if rollback/reset or its transaction state is uncertain. Savepoint rollback can
revert a local setting: no tenant statement follows without verifying the binding.
Do not keep transactions across provider calls or client waits. Actual pool/query
limits belong to G05; this finite example makes no load/deadline claim.

## Security boundary, alternatives and unresolved facts

RLS is defense in depth, not application authentication. A shared SQL role can set
an ordinary custom setting to another tenant; NOBYPASSRLS alone does not authenticate
that value. A writable unsigned GUC configuration is refused by this decision and
the fixture. The required database-verifiable scope binding is an implementation
prerequisite: G04/G05 must select and qualify a capability verification or role-bound
identity mechanism, including issuer ownership, unforgeability, binding lifetime and
revocation, before production admission. Merely naming a SECURITY DEFINER setter
does not establish those properties. No cryptographic implementation or privileged
setter is invented here. A compromised authorized scope issuer is beyond this
fixture; no security qualification is claimed.

Reject application-only filtering because one missed query predicate leaks data.
Reject session SET because pooled reuse can preserve a previous tenant. Reject a
runtime owner/BYPASSRLS role because policy enforcement is then ineffective. The
choice between database-verified capabilities and role-bound identities remains open;
credentials/pool sizing/verification ownership need an approved bounded contract.
Physical schema/ID encoding, selected driver, exact SQL policies, role memberships,
view/function grants, reset hooks and restore qualification remain G04/G05 work.

The whole original criterion, “runtime role cannot bypass scopes”, is preserved:
this design refuses bypass-capable role facts, writable unsigned authority, and
missing/unverified/mismatched transaction scopes. Actual PostgreSQL scope verification,
role/policy enforcement and arbitrary-SQL resistance are unperformed prerequisites,
not implied by the finite fixture or narrowed to role attribute checks.

## Executable finite contract

Private local names illustrate the future consumer boundary; shared `TenantScope`
has not been implemented and no substitute is installed. The role facts are supplied
qualification observations, not catalog queries. Database verification is a supplied
finite observation, not simulated cryptography. The fixture executes trusted scope
minting, admission, matching read/write, missing/unverified/cross-tenant refusal, rejected tenant
switch, commit/rollback clearing, uncertain-reset discard and unsafe-role refusal.
It does not execute SQL, pool sockets, cryptography, provider work or audience policy.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TenantId(u64);

#[derive(Clone, Copy)]
struct CurrentPermission {
    tenant: TenantId,
    current: bool,
}

#[derive(Clone, Copy)]
struct TenantScope(TenantId);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    UnverifiedPermission,
    UnverifiedBinding,
    MissingScope,
    CrossTenant,
    UnsafeRole,
    ActiveTransaction,
    NoTransaction,
    DiscardedConnection,
}

fn mint_scope(permission: CurrentPermission, requested: TenantId) -> Result<TenantScope, Refusal> {
    if !permission.current {
        return Err(Refusal::UnverifiedPermission);
    }
    if permission.tenant != requested {
        return Err(Refusal::CrossTenant);
    }
    Ok(TenantScope(permission.tenant))
}

#[derive(Clone, Copy)]
struct RuntimeRole {
    owner: bool,
    superuser: bool,
    bypass_rls: bool,
    privileged_role_switch: bool,
    unrestricted_ddl_or_function: bool,
    unsigned_context_authorizes: bool,
}

impl RuntimeRole {
    fn qualify(self) -> Result<(), Refusal> {
        if self.owner
            || self.superuser
            || self.bypass_rls
            || self.privileged_role_switch
            || self.unrestricted_ddl_or_function
        {
            return Err(Refusal::UnsafeRole);
        }
        if self.unsigned_context_authorizes {
            return Err(Refusal::UnverifiedBinding);
        }
        Ok(())
    }
}

struct PooledConnection {
    role: RuntimeRole,
    transaction_scope: Option<TenantScope>,
    discarded: bool,
}

#[derive(Clone, Copy)]
enum DatabaseVerification {
    Missing,
    Unverified,
    Verified(TenantId),
}

impl PooledConnection {
    fn begin(
        &mut self,
        scope: Option<TenantScope>,
        verification: DatabaseVerification,
    ) -> Result<(), Refusal> {
        if self.discarded {
            return Err(Refusal::DiscardedConnection);
        }
        self.role.qualify()?;
        if self.transaction_scope.is_some() {
            return Err(Refusal::ActiveTransaction);
        }
        let scope = scope.ok_or(Refusal::MissingScope)?;
        match verification {
            DatabaseVerification::Verified(tenant) if tenant == scope.0 => {}
            DatabaseVerification::Verified(_) => return Err(Refusal::CrossTenant),
            DatabaseVerification::Missing | DatabaseVerification::Unverified => {
                return Err(Refusal::UnverifiedBinding);
            }
        }
        self.transaction_scope = Some(scope);
        Ok(())
    }

    fn scoped_statement(&self, old_row: TenantId, new_row: TenantId) -> Result<(), Refusal> {
        if self.discarded {
            return Err(Refusal::DiscardedConnection);
        }
        self.role.qualify()?;
        let scope = self.transaction_scope.ok_or(Refusal::NoTransaction)?;
        if old_row != scope.0 || new_row != scope.0 {
            return Err(Refusal::CrossTenant);
        }
        Ok(())
    }

    fn finish(&mut self, reset_confirmed: bool) {
        self.transaction_scope = None;
        if !reset_confirmed {
            self.discarded = true;
        }
    }
}

fn main() {
    let tenant_a = TenantId(1);
    let tenant_b = TenantId(2);
    let permission_a = CurrentPermission {
        tenant: tenant_a,
        current: true,
    };
    let scope_a = mint_scope(permission_a, tenant_a);
    assert!(scope_a.is_ok());
    assert!(matches!(
        mint_scope(permission_a, tenant_b),
        Err(Refusal::CrossTenant)
    ));
    assert!(matches!(
        mint_scope(
            CurrentPermission {
                current: false,
                ..permission_a
            },
            tenant_a
        ),
        Err(Refusal::UnverifiedPermission)
    ));
    let role = RuntimeRole {
        owner: false,
        superuser: false,
        bypass_rls: false,
        privileged_role_switch: false,
        unrestricted_ddl_or_function: false,
        unsigned_context_authorizes: false,
    };
    let mut connection = PooledConnection {
        role,
        transaction_scope: None,
        discarded: false,
    };
    let verified_a = DatabaseVerification::Verified(tenant_a);
    let verified_b = DatabaseVerification::Verified(tenant_b);
    assert_eq!(
        connection.begin(None, verified_a),
        Err(Refusal::MissingScope)
    );
    assert_eq!(
        connection.begin(scope_a.ok(), DatabaseVerification::Missing),
        Err(Refusal::UnverifiedBinding)
    );
    assert_eq!(
        connection.begin(scope_a.ok(), DatabaseVerification::Unverified),
        Err(Refusal::UnverifiedBinding)
    );
    assert_eq!(
        connection.begin(scope_a.ok(), verified_b),
        Err(Refusal::CrossTenant)
    );
    assert_eq!(
        connection.scoped_statement(tenant_a, tenant_a),
        Err(Refusal::NoTransaction)
    );
    assert_eq!(connection.begin(scope_a.ok(), verified_a), Ok(()));
    assert_eq!(connection.scoped_statement(tenant_a, tenant_a), Ok(()));
    assert_eq!(
        connection.scoped_statement(tenant_b, tenant_b),
        Err(Refusal::CrossTenant)
    );
    assert_eq!(
        connection.scoped_statement(tenant_a, tenant_b),
        Err(Refusal::CrossTenant)
    );
    let scope_b = mint_scope(
        CurrentPermission {
            tenant: tenant_b,
            current: true,
        },
        tenant_b,
    )
    .ok();
    assert_eq!(
        connection.begin(scope_b, verified_b),
        Err(Refusal::ActiveTransaction)
    );
    assert_eq!(connection.scoped_statement(tenant_a, tenant_a), Ok(()));
    connection.finish(true); // Confirmed commit clears the local context.
    assert_eq!(
        connection.scoped_statement(tenant_a, tenant_a),
        Err(Refusal::NoTransaction)
    );
    assert_eq!(connection.begin(scope_b, verified_b), Ok(()));
    assert_eq!(connection.scoped_statement(tenant_b, tenant_b), Ok(()));
    assert_eq!(
        connection.scoped_statement(tenant_a, tenant_a),
        Err(Refusal::CrossTenant)
    );
    connection.finish(true); // Confirmed rollback also clears the context.
    assert_eq!(
        connection.begin(None, verified_b),
        Err(Refusal::MissingScope)
    );
    assert_eq!(
        connection.scoped_statement(tenant_b, tenant_b),
        Err(Refusal::NoTransaction)
    );
    assert_eq!(connection.begin(scope_a.ok(), verified_a), Ok(()));
    connection.finish(false);
    assert_eq!(
        connection.begin(scope_b, verified_b),
        Err(Refusal::DiscardedConnection)
    );
    assert_eq!(
        connection.scoped_statement(tenant_a, tenant_a),
        Err(Refusal::DiscardedConnection)
    );
    for unsafe_role in [
        RuntimeRole {
            owner: true,
            ..role
        },
        RuntimeRole {
            superuser: true,
            ..role
        },
        RuntimeRole {
            bypass_rls: true,
            ..role
        },
        RuntimeRole {
            privileged_role_switch: true,
            ..role
        },
        RuntimeRole {
            unrestricted_ddl_or_function: true,
            ..role
        },
    ] {
        let mut unsafe_connection = PooledConnection {
            role: unsafe_role,
            transaction_scope: None,
            discarded: false,
        };
        assert_eq!(
            unsafe_connection.begin(scope_a.ok(), verified_a),
            Err(Refusal::UnsafeRole)
        );
        assert!(unsafe_connection.transaction_scope.is_none());
    }
    let mut unsigned_connection = PooledConnection {
        role: RuntimeRole {
            unsigned_context_authorizes: true,
            ..role
        },
        transaction_scope: None,
        discarded: false,
    };
    assert_eq!(
        unsigned_connection.begin(scope_a.ok(), verified_a),
        Err(Refusal::UnverifiedBinding)
    );
    assert!(unsigned_connection.transaction_scope.is_none());
    println!(
        "PASS tenant scope: trusted/missing/unverified/unsigned/cross-tenant/switch/commit/rollback/discard/roles"
    );
}
```

## Verification and next consumer

Extract this sole Rust fence byte-for-byte; pinned Rust 1.98.1 rustfmt write/check
uses root `rustfmt.toml`, then `rustc --edition=2024 -Dwarnings` and execution use
the unchanged v6 40% admission, 256 MiB/60-second guard and shared compiler lock.
Retained receipts identify the exact submitted document, literal, tools, config and
binary. No Cargo/Clippy/WASM gate is claimed for this standalone design literal.

Next actual `df-persistence` implementation minimally needs its tenant transaction
adapter and pool reset/discard path, scoped consumer repository adapters, and the
single migration owner's RLS/grant definitions; auth supplies its approved trusted
scope. Candidate paths are `crates/df-persistence/src/tenant.rs`, `pool.rs` and the
owned repository/migration paths once frozen. These files do not yet exist and
this document does not authorize creating them. Qualify under the real runtime
role: missing/empty/malformed context, all DML and tenant-changing writes, joins and
foreign-key/unique errors, unsafe role/policy grants, transaction reuse after commit,
rollback, cancellation/savepoint/error and uncertain reset, and tenant-scoped cache/
pagination/restore behavior. Authentication revocation/audience and database role
catalog inspection are independent requirements. Real SQL RLS/role/pool qualification,
database load/recovery, production integration and independent frontier review
remain unperformed.
