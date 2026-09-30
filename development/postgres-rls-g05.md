# Finite PostgreSQL tenant-policy qualification

Run from any source checkout with the approved PostgreSQL 18.6 binaries already
installed in the original repository:

```sh
python3 development/qualify-postgres-rls.py --fixture-id worker-a1-<short-fresh-suffix> \
  --protected-pids 18905,18907,25530,80873,<current-coordinator-recorder-pid>
```

Use `reviewer-a1-` and `integration-a1-` for independent candidate and integrated
checks. The suffix is lowercase ASCII letters/digits/hyphens, at most 32 characters;
the complete Unix socket path must also fit the native 104-byte bound. Eight-character
suffixes fit. Existing fixture IDs refuse before running PostgreSQL. Runtime roots
are fixed to the original repository's `development/runtime/rls-g05`, require local
ownership and mode0700, and reject symlinks in every ancestor. There is no reset,
adoption, deletion, download, build, network listener, or existing-cluster lifecycle
operation. Never use the worktree's `postgres.sh` for this check.

Supply the coordinator's current protected process configuration. The original
PostgreSQL and three previews are mandatory, along with the currently leased
coordinator recorder. The helper compares PID/parent/start/argv before and after;
it never adopts, signals or modifies these processes. A coordinator-owned expired
recorder may be replaced outside this fixture; retain that lifecycle receipt and
use its current PID. A missing supplied process fails before cluster creation.

Each run creates its own synthetic cluster, private `s/` socket, port55441, owner
record, immutable inputs/configuration, command output, source/binary hashes and
`result.json`. The helper owns the direct new PostgreSQL process/session, checks
actual SQL data_directory/configuration/version and the OS parent of a live backend,
and fast-stops/reaps only that retained child. `postmaster.pid` is never a lifecycle
authority. Interrupted and failed runs retain their evidence and data. All children
have30second deadlines, readiness10seconds, server statements2seconds and lock waits
500milliseconds; the helper reserves shutdown time within120seconds overall. SQL
inputs are at most64KiB, output at most1MiB per operation, query rows at most100,
max_connections4 and shared_buffers16MiB. Resource samples report the observed
helper/owned-child RSS; they are sampled measurements, not a guaranteed memory cap.
Durability settings remain enabled. `psql -X -w` receives only fixed argv and SQL,
an isolated HOME/config/password environment and explicit local socket/database/role.

The trusted developer fixture logs in directly as `fixture_runtime`, which has no
ownership of either RLS data table, superuser/BYPASSRLS/admin membership, role creation, database
creation or replication authority. The separate no-login owner owns both FORCE RLS
tables. Only table DML and schema USAGE are granted. Paired tenants use fixed nonzero
16-byte identifiers and identical local IDs1–32, with composite primary/foreign
keys. Invalid/empty/missing context admits zero rows. Explicit predicates accompany
normal reads, DML, tenant-composite joins, search and keyset pagination; additional
unpredicated/foreign-ID probes measure RLS itself.

The runtime script retains one real backend while alternating tenant scopes across
commit, rollback, savepoint rollback and a failed transaction. It deliberately sets
an inherited SESSION tenant and demonstrates that SET LOCAL restores that prior
tenant after commit. The trusted fixture clears the SESSION setting **before every
reused transaction**, then installs validated transaction-local context. This is a
native SQL experiment, not a Rust pool checkout/reset implementation. The concurrent
case uses two direct runtime backends blocked together on a controller-owned advisory
lock, verifies two waiting backend identities, releases the barrier and checks scoped
writes and reset after commit. There are no timing sleeps as concurrency evidence.

Expected SQLSTATE observations include42501 privilege/RLS denials,23505 unique
rejection,23503 composite FK rejection,22012 deliberate failed statement and25P02
aborted transaction. Owner-policy modification, owner/admin role assumption,
BYPASSRLS changes, TRUNCATE, REFERENCES and row_security=off reads are denied.
A deliberately unsafe global `probe UNIQUE` fixture column demonstrates a hidden
tenant existence channel: inserting an own-tenant row using a hidden tenant's probe
fails23505 while an absent probe succeeds then rolls back. PostgreSQL integrity
checks bypass RLS; composite keys constrain references but cannot establish safe
public error semantics. These raw errors are synthetic retained evidence only.

The REFERENCES probe uses a dedicated empty permanent `reference_probe` relation
owned by the runtime role. It checks that ownership and schemaUSAGE permit ALTER,
the referenced parent is permanent, REFERENCES is absent, and adding its FK fails
42501 without creating a constraint. This limited probe ownership grants no
ownership of either paired-tenant RLS table. A temporary-to-permanent FK probe was
rejected earlier with42P16, which is a probe-design limitation, not privilege evidence.

The local OS-user socket trust and a client able to choose a custom tenant GUC are
trusted developer mechanisms. Arbitrary SQL can choose either fixture tenant; this
is **not end-user authentication or authorization**. RLS does not issue TenantScope.
The qualifier does not implement production schemas/migrations, df-auth, Rust
pool/repository/adapters, safe public error mapping, audience filtering, cache/media
access, recovery/erasure, protected-journal restore, or workload qualification.
Full G05 and the df-persistence design remain pending. Python syntax and actual
native SQL are the affected checks; Rust/native/WASM source is unchanged, so no
Cargo/WASM gate is claimed. Independent frontier execution and merged MAIN checks
are required before this finite prerequisite can close.

Governing design: [Service operations, tenant isolation](../planning/service-operations.md#tenant-isolation-and-hostile-input),
[Storage architecture](../planning/storage-architecture.md#postgresql-planning-requirements),
[Subsystem interfaces](../planning/subsystem-interfaces.md#postgresql-persistence-and-durable-media)
and the frozen QUALIFY-G05-RLS-001/a1 brief. PostgreSQL's primary references explain
[RLS owner/bypass/constraint limitations](https://www.postgresql.org/docs/18/ddl-rowsecurity.html)
and [SET LOCAL transaction/savepoint/session behavior](https://www.postgresql.org/docs/18/sql-set.html).
