# OTEL producer and collector topology

Task: `B-G06-D01` (`G06:D:Select OTEL producer collector topology`)
Attempt: `B-G06-D01-a1`
Input revision: `4ba37943691f3a01a49b8238983e82527bec91bf`
Owners: `df-observe`, `df-telemetry`
Status: topology decision recorded; production composition and qualification remain open.

## Decision

Use one native telemetry writer, bounded durable spool, and separate SQLite
database per native process instance. A process instance owns its telemetry root
and its writer lock. Keep telemetry separate from PostgreSQL gameplay state,
durable media, and the SQLite development workflow database. Do not mount one
writable SQLite database or spool over a shared network filesystem.

`df-observe` owns shared instrumentation, OTLP encoding, native SDK/export
capture, and the consumer-owned `TelemetryIngress` port. Native producer callbacks
perform bounded encoding and queue admission; an independent ingestion worker owns
filesystem and SQLite work. `df-telemetry` owns native OTLP validation, the local
spool, SQLite commit/checkpoint, bounded read-only queries, ingestion health, and
later segment export. An ingestion receipt must distinguish durable spool
acceptance from SQLite commit; retry retains stable producer/record identities.

WASM clients do not open SQLite. When integrated, they submit finite best-effort
diagnostic batches through the planned RPC/API boundary. In that composition,
`df-api` authenticates and authorizes the upload separately from supplied trace
context, calls the `TelemetryIngress` port in `df-observe`, and `df-server` injects
its `df-telemetry` implementation.
Authorization for operator query, pin, or export is independently supplied by
`df-auth`; a trace ID, producer ID, build label, or session label grants no access.
The browser receives an explicit terminal durability result where supported;
native crash-safe spool guarantees do not imply browser persistence guarantees.
If the native writer is unavailable or capacity is exhausted, telemetry loss or
degradation stays visible and gameplay remains responsive.

For later federation, close and validate bounded indexed SQLite segments, record
immutable segment identity and watermarks, then expose them to `df-telemetry` as
read-only sources. Federation reads copies or immutable segments; it never joins
or writes through a live shared SQLite file. Missing producer ranges and segment
retention are reported as gaps, not represented as complete failover.

## Why per-instance SQLite

| Option | Decision | Reason |
| --- | --- | --- |
| One writable SQLite file shared by instances, including a network-mounted file | Reject | It couples independent processes to one filesystem lock/WAL failure domain and makes writer ownership, outage recovery, and segment loss hard to bound. |
| One writable SQLite file per native instance with a local bounded spool | Select | It keeps the only writer and its recovery state with the producer instance; the existing crate boundary already separates capture from the storage implementation. |
| Native SQLite writer per instance plus immutable federated segments | Select for later review federation | It supports bounded cross-instance review without making federation another live writer. Segment replication/restore remains a separate qualified operation. |
| Browser/WASM SQLite writer or assumed durable browser spool | Reject | Current source explicitly marks `df-telemetry` native-only; browser storage durability has not been selected or verified. Use bounded authorized upload instead. |
| One process-wide/shared in-memory exporter as the production corpus | Reject | The bounded in-memory exporter in `df-observe` is explicitly a fixture and cannot provide restart recovery or durable review evidence. |

This decision selects ownership and failure boundaries. It does not provision
paths, create a hosted collector, choose a browser SDK, promise lossless delivery,
or claim deployment readiness.

## Bounded routing contract example

This standard-library-only example models path ownership and access modes for the
decision. It is not the production store, filesystem authorization, or evidence
that the application is integrated. A writer path is derived from one instance
identity; a different identity cannot claim it. A local owner may query its own
store read-only. Cross-instance federation accepts immutable segments for reads
only; a mutable segment and any federated write are rejected.

```rust
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Instance(&'static str);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Access {
    Write,
    Query,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Grant {
    Writer,
    ReadOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecisionError {
    WritablePathConflict,
    ForeignInstanceRoot,
    MutableFederation,
}

impl fmt::Display for DecisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

fn instance_root(instance: Instance) -> String {
    format!("/telemetry/instances/{}", instance.0)
}

fn authorize_local(
    identity: Instance,
    requested_root: &str,
    access: Access,
) -> Result<Grant, DecisionError> {
    if requested_root != instance_root(identity) {
        return Err(match access {
            Access::Write => DecisionError::WritablePathConflict,
            Access::Query => DecisionError::ForeignInstanceRoot,
        });
    }
    match access {
        Access::Write => Ok(Grant::Writer),
        Access::Query => Ok(Grant::ReadOnly),
    }
}

fn query_federated_segment(immutable: bool) -> Result<Grant, DecisionError> {
    if immutable {
        Ok(Grant::ReadOnly)
    } else {
        Err(DecisionError::MutableFederation)
    }
}

fn main() -> Result<(), DecisionError> {
    let alpha = Instance("alpha");
    let beta = Instance("beta");
    let alpha_root = instance_root(alpha);
    let beta_root = instance_root(beta);

    assert_eq!(
        authorize_local(alpha, &alpha_root, Access::Write)?,
        Grant::Writer
    );
    assert_eq!(
        authorize_local(beta, &beta_root, Access::Write)?,
        Grant::Writer
    );
    assert_eq!(
        authorize_local(beta, &alpha_root, Access::Write),
        Err(DecisionError::WritablePathConflict)
    );
    assert_eq!(
        authorize_local(alpha, &alpha_root, Access::Query)?,
        Grant::ReadOnly
    );
    assert_eq!(
        authorize_local(beta, &alpha_root, Access::Query),
        Err(DecisionError::ForeignInstanceRoot)
    );
    assert_eq!(query_federated_segment(true)?, Grant::ReadOnly);
    assert_eq!(
        query_federated_segment(false),
        Err(DecisionError::MutableFederation)
    );

    println!("per-instance writer, owner query, and immutable federation decisions passed");
    Ok(())
}
```

The example proves only these pure routing assertions when compiled and run. It
does not test a real SQLite file, spool durability, process concurrency, hostile
filesystem paths, principal authorization, a network filesystem, or deployment.

## Current source boundary and explicit gaps

At the input revision, `Cargo.toml` lists six workspace crates:
`df-types`, `df-protocol`, `df-rpc-bridge`, `df-observe`, `df-tools`, and
`df-telemetry`. The pinned toolchain decision records that the original G01
source had five crates and current source added `df-telemetry`; native uses
Rust `1.98.1` and the workspace's Rust 2024 edition. The WASM package selection
excludes `df-telemetry`, whose executable source has a WASM `compile_error!`.

The present `df-observe` source has OTEL operation spans, a native producer,
bounded OTLP capture, and a fixture-only bounded in-memory exporter. Its native
producer calls the `TelemetryIngress` port and leaves disk work to ingestion.
The present `df-telemetry` source implements that native port, validates bounded
OTLP, creates a SQLite database and spool beneath an explicitly restricted local
synthetic fixture root, locks an open root, advances commit/checkpoint state, and
provides a SQLite read-only diagnostic connection. The fixture limits currently
cap records at 8 KiB, batches at 64 KiB/32 records, pending work at 1,024 items
and 1 MiB, spool at 32 MiB, and SQLite at 64 MiB. The synthetic root is
hard-coded under `development/runtime/telemetry-g06/ownedfixture-*`; this is
fixture containment, not deployment path configuration.

The current root marker identifies a synthetic owned root, and an exclusive
process lock prevents two active fixture writers opening that root. It does not
bind a production instance identity to a root, configure instance-specific paths,
or implement federation. The standalone example records the missing identity to
path policy; it must not be mistaken for that enforcement already existing.

The six-crate workspace has no current `df-auth`, `df-api`, or `df-server`
crate. Consequently there is no integrated trusted-principal upload admission,
operator authorization, server composition, hosted runtime, or browser telemetry
upload here. Current tests/fixture code are not a production workload or browser
qualification. Do not claim authenticated review, private-data redaction,
retention/deletion, native/WASM SDK compatibility, live UI behavior, or production
recovery/load targets from this decision example or source inspection.

Do not record credentials, raw audio, private dialogue, player speech, full
prompts, or unrevealed/private game state in default telemetry. Filter and redact
before serialization at an authorized boundary; OTEL correlation remains
non-authoritative. Exact schema-level redaction belongs to the separate
telemetry-schema decision and must be verified in its implementation. Planned
service-operations envelopes (100 enabled events/sec sustained at 1 KiB average,
1,000/sec burst for 10 seconds, 2 GiB spool, 4 GiB active SQLite segment,
30 GiB per-instance quota, and 7-day target retention subject to quota) are
qualification proposals, not achieved bounds or promises. The smaller fixture
limits above are not substitutes for that workload qualification.

## Original acceptance and verification criteria

The two original acceptance criteria remain:

1. SQLite per instance.
2. The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.

The two original verification instructions remain:

1. Freeze the cited source decision and a bounded contract example; compare SQLite per instance. Retain decision, alternatives and unresolved facts.
2. Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.

This attempt additionally executed the bounded pure Rust example with the pinned
cached compiler. It did not run Cargo builds, project tests, browser checks,
provider checks, storage qualification, or production integration. The exact
example/compiler/binary hashes, commands, outputs, and exits are retained under
the attempt's worker evidence directory. Independent frontier evaluation and
MAIN integration acceptance remain required before this task can be considered
done.
