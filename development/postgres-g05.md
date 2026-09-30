# Owned local PostgreSQL 18.6 runtime

This is a local development prerequisite for later persistence work. It does not
implement the game schema, tenant isolation, application credentials, migration
ownership, backup/restore qualification, or production deployment.

From the repository root, run `./development/postgres.sh setup` once, then use
`start`, `status`, and `stop`. `setup` fetches the official PostgreSQL 18.6
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
16 connections. `fsync`, `full_page_writes`, and `synchronous_commit` remain on.
The data and socket directories are mode 0700. The script refuses symlinked
paths, an unowned or malformed cluster, stale PID files, and replacing existing
data. It only stops a server that `pg_ctl` verifies against this owned data
folder. Never delete or reset this durable cluster as artifact cleanup.

For transaction/restart verification after setup and start, use `psql` over the
socket, create a temporary table in a transaction and roll it back, then create
and commit a row, stop/start the owned server, and confirm the committed row
remains. Use `-X`, avoid printing connection strings or environment secrets,
and query `pg_isready` with the same socket and port. Full G05 persistence and
recovery requirements remain open.

A parameter-safe local readiness query is:

```sh
artifacts/build/postgres-g05/install/bin/pg_isready \
  -h development/runtime/postgres-g05/socket -p 55439 -t 5
artifacts/build/postgres-g05/install/bin/psql -X -v ON_ERROR_STOP=1 \
  -h development/runtime/postgres-g05/socket -p 55439 \
  -U "$(id -un)" -d postgres \
  -c "select version(), current_setting('listen_addresses'), current_setting('max_connections'), current_setting('fsync'), current_setting('full_page_writes'), current_setting('synchronous_commit')"
```

Use fixed SQL and `-X`; do not interpolate untrusted values into shell or SQL.
