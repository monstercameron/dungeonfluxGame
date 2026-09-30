# Post-integration optimization

Date: 2026-09-29
Status: Every subsystem crate requires an optimization pass after initial integration

## Sequence

First establish a working integrated game flow with correct behavior, agreed
interfaces, and the required observability. Keep implementations straightforward
during this stage. Once initial integration is verified, every subsystem crate
must receive a deliberate optimization pass, including shared libraries and both
browser clients. Track coverage so no crate is overlooked.

## Measure, optimize, verify

Record a reproducible baseline using representative integrated workloads and
target devices. Profile the owning crate and its contribution to the whole flow.
Choose concrete budgets based on observed player experience and operating cost.

Assess the dimensions relevant to each crate: CPU time, allocations and memory,
tail latency, throughput, queue contention, persistence or network I/O, and
serialization. Browser crates also assess WebAssembly/download size, startup,
frame time, and battery-sensitive work. AI/media orchestration assesses duplicate
work, cache behavior, provider usage, and cost as well as latency.

Optimize measured bottlenecks, starting with algorithms, data layout, unnecessary
work, and repeated allocations. Introduce concurrency or caching only when the
measurements justify their added complexity. Do not weaken rules correctness,
private-player view boundaries, error handling, or essential observability to
produce a better benchmark.

Preserve agreed public contracts. Changes requiring a new contract follow the
subsystem interface change process. Verify rules behavior and affected consumers,
then rerun the same benchmarks and integrated workload. Record baseline, result,
environment, tradeoffs, and any remaining bottlenecks. Revert optimizations that
do not deliver a useful measured improvement.

## Completion and regression control

The coordinator creates bounded optimization tasks for every crate after the
initial integration gate. Each task links to profiling evidence, a workload, and
measurable acceptance criteria. Evaluators check correctness and measured results.
A crate already meeting its budgets may complete its pass with evidence rather
than speculative rewrites; unresolved budget failures remain tracked work.

Retain benchmark definitions and compact results as durable project evidence;
disposable profiling output belongs in `artifacts/`. Add regression checks for
critical workloads with explicit tolerances and account for environmental noise.
Repeat targeted profiling when later changes affect a hot path or breach a
budget. Optimize individual crates and verify whole-game performance so local
improvements do not merely shift work to another subsystem.

## Runtime director and asset workloads

Include every added crate in the post-integration pass. Profile due-event queues,
active-world summaries, knowledge adjacency/retrieval/decay, bounded rumor fanout,
intent/compound steps, interaction decisions, convergence candidate search,
encounter/tactic selection, macro pacing windows, tempo inertia/fatigue and
presentation projections. Prefer typed indexed data and bounded work before
micro-optimizing arithmetic. Compare source-faithful outcomes/replay hashes and
hidden-state noninterference after changes.

Measure actor wakeups/decision p95/p99, per-session CPU/memory/query bytes, client
frame/input/audio drift, speech first-approved playback, cache hit/prefetch waste,
queue fairness and actual/unknown supplier spend at the supported campaign capacity.
Use cached SFX/stems/reference packs and low-cost fallbacks; optional video or 3D
cannot block the rules/input path. A tuned score is not a calibrated probability
or proof of fun; G11/G12 require real session/device/cost evidence.
