# Owned local PostgreSQL 18.6 runtime

This is a local development prerequisite for later persistence work. It does not
implement the game schema, tenant isolation, application credentials, migration
ownership, backup/restore qualification, or production deployment.

From the repository root, run `./development/postgres.sh setup` once, then use
`start`, `status`, and `stop`. Normal commands are capped at 60 seconds; readiness
is capped at 10 seconds. Setup is capped at 1200 seconds total, with native build
commands capped at 900 seconds. `setup` fetches the official PostgreSQL 18.6
source archive into `artifacts/cache/postgres-g05`, checks SHA256
`983ee554ec53dbeb9b70797bef9fcf4e67e117e7e48ca1463cc80b3ff8e8ff3f`, builds
with one make job, and installs into `artifacts/build/postgres-g05/install`.
The pinned URL is
`https://ftp.postgresql.org/pub/source/v18.6/postgresql-18.6.tar.gz`.
Optional ICU, readline, LLVM, LZ4, Zstandard, SSL and language integrations are
disabled for this development-only build. System zlib is enabled. Do not use this
as a production build.

The durable cluster, configuration, owner manifest, socket and server log live
under `development/runtime/postgres-g05`, outside `artifacts/`. The local
superuser uses the current OS account; local socket authentication is trust, and
host authentication is rejected. The server listens only on its private Unix
socket at `development/runtime/postgres-g05/socket`, uses port 55439, and allows
16 connections and caps PostgreSQL workers with `max_worker_processes=8`,
`max_parallel_workers=8`, `max_parallel_workers_per_gather=2`, and
`autovacuum_max_workers=3`. `fsync`, `full_page_writes`, and
`synchronous_commit` remain on. The data and socket directories are mode 0700.
The script rejects symlinks in every path component before reading or changing
the runtime. It authenticates `postmaster.pid` against the live process executable,
its `-D` argument, and a bounded SQL identity query over the private socket before
reporting readiness or invoking graceful stop. The data argument is checked as an
exact OS argument, and the live SQL backend's OS parent must be that postmaster
PID; a matching start timestamp alone is insufficient. Runtime logs, configuration,
PID/socket files, directory components and tool paths reject symlinks before use.
A stale, reused, or ambiguous PID file fails closed and is left for operator
inspection. Graceful fast shutdown signals the verified PID directly, avoiding a
second PID-file lookup, then waits up to 30 seconds for its PID file/socket removal.
Configure, native build, install, process probes,
and readiness each have explicit deadlines, including a 1200-second setup total
and 900-second native build cap. Never delete or reset this durable cluster as
artifact cleanup.

For transaction/restart verification after setup and start, use `psql` over the
socket, create a temporary table in a transaction and roll it back, then create
and commit a row, stop/start the owned server, and confirm the committed row
remains. Use `-X`, avoid printing connection strings or environment secrets,
and query `pg_isready` with the same socket and port. Full G05 persistence and
recovery requirements remain open.

A parameter-safe local readiness query is:

```sh
artifacts/build/postgres-g05/install/bin/pg_isready \
  -h "$(pwd)/development/runtime/postgres-g05/socket" -p 55439 -t 5
artifacts/build/postgres-g05/install/bin/psql -X -v ON_ERROR_STOP=1 \
  -h "$(pwd)/development/runtime/postgres-g05/socket" -p 55439 \
  -U "$(id -un)" -d postgres \
  -c "select version(), current_setting('listen_addresses'), current_setting('max_connections'), current_setting('max_worker_processes'), current_setting('max_parallel_workers'), current_setting('max_parallel_workers_per_gather'), current_setting('autovacuum_max_workers'), current_setting('fsync'), current_setting('full_page_writes'), current_setting('synchronous_commit')"
```

Use fixed SQL and `-X`; do not interpolate untrusted values into shell or SQL.
