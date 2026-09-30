# Mandatory coding style

Date: 2026-09-29
Status: Required for all agents; workspace/CI enforcement pending implementation

## Scope and enforcement

All coordinators, workers, repair agents, and evaluators MUST follow this style
for authored project code. Task briefs reference this document; evaluation checks
compliance alongside behavior. Generated protobuf/WASM output follows its generator
and must not be hand-edited to satisfy authored-code conventions. Rust application
and UI source remains mandatory under [Client architecture](client-architecture.md).

Use the [official Rust style](https://doc.rust-lang.org/style-guide/) and the root
`rustfmt.toml`. Formatting is mechanical: four spaces, a 100-column formatting
target, Unix line endings, and Rust 2024 style edition. Let rustfmt decide wrapping
and imports; do not manually enforce a different layout. The
[style edition](https://doc.rust-lang.org/edition-guide/rust-2024/rustfmt-style-edition.html)
is a formatter setting, separate from language edition/toolchain selection.

When the workspace is created, pin one supported stable toolchain with rustfmt
and Clippy for every agent and CI. Use its checked-in language editions, targets,
feature sets, and lockfile; do not upgrade tooling or dependencies during unrelated
work. Keep the application workspace's `Cargo.lock` tracked. Build/cache output
belongs under `artifacts/` as already required.

Required implementation gates:

- `cargo fmt --all -- --check` using the repository configuration.
- Clippy with warnings denied for the affected authored crates and supported
  target/feature combinations, using the standard default lint set.
- Relevant compile, contract, and behavior checks from the task brief, including
  browser/WASM checks for client changes.

An illustrative crate check is `cargo clippy --locked -p <crate> --all-targets -- -D warnings`.
The workspace plan must provide concrete native/WASM commands: a native workspace
check alone is not a client check, and mutually exclusive features must not be
combined blindly. Do not enable every pedantic/restriction lint by default.
These gates become executable when source/workspace tooling exists; they have
not passed merely because this document exists. See
[Clippy usage](https://doc.rust-lang.org/clippy/usage.html).

Do not disable format/lint gates to make a submission pass. A necessary exception
must be narrow, state the invariant or platform limitation beside the code, and
appear in the handoff for evaluator review. No blanket crate-wide suppression or
formatting skips without a concrete reason. Unrelated existing violations are
reported to the coordinator; avoid opportunistic repository-wide rewrites.

## Names, modules, and public contracts

- Use `snake_case` for modules, functions, and variables, `UpperCamelCase` for
  types/traits/variants, and `SCREAMING_SNAKE_CASE` for constants. Spell names by
  responsibility; avoid vague `manager`, `helper`, or `utils` dumping grounds.
- Keep modules cohesive and private by default. Use the narrowest visibility
  required, with a small deliberate public surface per subsystem crate. Consumers
  depend on approved contracts, never another crate's implementation details.
- Use structs for related values, enums for meaningful alternatives and states,
  and distinct ID/unit types where accidental interchange would cause defects.
  Use `Duration` for elapsed time, explicit units at boundaries, and integer or
  exact representations where game rules require exact arithmetic.
- Separate domain, transport, persistence, and presentation representations at
  their actual boundaries. Map them explicitly; do not scatter serialization,
  SQL, or browser dependencies through rules code. Interface/schema definitions
  follow [Subsystem interfaces](subsystem-interfaces.md) and [RPC API](rpc-api.md),
  with concrete types frozen before consumers; workers cannot invent alternatives.
- Start with ordinary functions and concrete types. Introduce traits, generics,
  macros, or trait objects only for a real reuse/substitution need. Avoid generic
  frameworks, redundant wrappers, and speculative extension points.

Follow [Subsystem architecture](subsystem-architecture.md). Public contract
changes include governing design, affected consumers, and checks in one coordinated
change. Generated RPC types are not a license to expose private server state.

## Functions, ownership, and errors

- Give each function a clear responsibility. Prefer guard clauses and explicit
  control flow when they reduce nesting; use iterator chains when readable.
  Do not impose arbitrary line-count limits or compress code into clever one-liners.
- Borrow when ownership is unnecessary; move when ownership transfers. Use clones
  deliberately and shared ownership only when lifetimes require it. Do not add
  `Arc`, mutexes, or `.clone()` merely to silence an ownership error.
- Return typed `Result` errors for recoverable failures and meaningful `Option`
  values for absence. Use `?` to propagate with useful context; distinguish domain
  rejection, transport failure, cancellation, and internal failure at boundaries.
  Callers must not parse human-readable strings to decide behavior.
- No `unwrap`, `expect`, indexing, or panic on unvalidated user, network, storage,
  or provider input. Validate and return the contracted outcome. Test assertions
  may panic; a production assertion must represent a documented internal invariant.
- Never silently discard errors or turn failures into fabricated success/default
  state. Explicitly handle best-effort work, record its outcome, and preserve the
  difference between accepted work and completed work.
- Prefer safe Rust. Any necessary unsafe/FFI belongs in a minimal isolated boundary
  with a `SAFETY` explanation of its invariants and focused checks. Do not use
  unsafe `Send`/`Sync` implementations to bypass browser or runtime incompatibility.

## Async code and deterministic gameplay

Every spawned task, timer, stream, and callback has a named owner, lifetime,
cancellation/close behavior, and observable result. Use bounded channels/buffers
with explicit overflow behavior. Do not detach work and lose its outcome.
Keep blocking I/O and long CPU work off async/browser event-loop execution paths.

Do not hold synchronous locks or database transactions across `.await`. An async
lock spanning an await requires a documented ownership need and cancellation
analysis; prefer taking a snapshot or transferring work to its owner. Capture
operation/correlation context when dispatching, and reject stale completions under
[Runtime reliability](runtime-reliability.md).

Keep authoritative rules deterministic with explicit time/dice inputs. Avoid
hidden wall-clock reads, randomness, global mutable state, or external I/O inside
rules resolution. Clients render server decisions and provide presentation/input;
client countdowns, visual predictions, and animations do not mutate game authority.

## Persistence, RPC, and instrumentation

Use parameterized SQL and the approved storage boundary; keep migrations reviewed
and versioned. Explicitly map storage and RPC failures to public outcomes without
leaking internal details. Preserve idempotency and transaction semantics from the
contract rather than adding ad hoc retry loops.

Use the shared OTEL instrumentation path, structured fields, stable event meaning,
and propagated operation context. Record a failure at the boundary that owns its
handling rather than repeatedly logging the same propagated error at every layer.
Retries and distinct state transitions remain observable. Routine application
logging must not use scattered `println!`/`eprintln!` calls; CLI output and the
approved emergency diagnostic path are separate concerns.

Never include credentials or private payloads in diagnostics by default. Respect
native/WASM runtime requirements and the planned RPC interfaces. Read
[Observability](observability.md), [Storage architecture](storage-architecture.md),
and [RPC transport](rpc-transport.md) for governing behavior.

## Documentation, tests, and changes

Document public contracts with purpose, invariants, units, errors, and relevant
ordering/cancellation guarantees. Add examples when they clarify actual use.
Comments explain reasons or non-obvious invariants; do not narrate each statement
or add banners. TODOs for intentionally deferred work reference a tracked task;
required current behavior cannot be left behind a TODO or production stub.

Use descriptive behavioral test names and explicit arrange/act/assert steps.
Prefer small fixtures and controlled time/synchronization over real sleeps.
Test contract outcomes and relevant failure cases, not private implementation
shape. Do not add tests for pure formatting or otherwise reversible cosmetic edits.
Keep Rust tests near the module for unit behavior and at crate boundaries for
integration behavior; use the existing layout once established.

Keep changes focused and consistent with adjacent compliant code. Add dependencies
only for the assigned need and check native/WASM suitability; do not invent a
parallel helper, logging convention, or style variant for each agent. The
evaluator verifies these rules on the submitted and affected integrated source,
alongside the running-output checks in ADR 0005. Record any violation or unperformed
gate explicitly; style checks never replace behavioral acceptance.
