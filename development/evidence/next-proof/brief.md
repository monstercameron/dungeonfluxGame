# Next proof: bounded local Rust/WASM transport qualification

The current plan has independent design9.05/10 and reviewed commercial arithmetic.
**The income objective is unproven.** Current main is `8ab15233bccb126d3afba5f9cbaff87e00dd6168`;
140 families,307 plans,1,186 dependencies,3 historical attempts. G01/G02/G03/G07 and
S00-ACCEPT remain pending. There is no Cargo manifest or implemented Rust game.
The next local executable uncertainty is G02, alongside G07 source/rights feasibility.
This packet adds no task, architecture, permission or implementation.

## Approval boundary

[AGENTS.md:83](/Users/earlcameron/Documents/Codex/2026-09-29/cr/dungeonflux/AGENTS.md:83) says:

> Honor the requested work phase: planning and review do not authorize prototypes,
> dependency installation, or importing old game files.

That explicit phase restriction is why approval is required before this experiment.
The review goal and prior Git push authorization have not authorized prototype code
or dependency installation. The concrete next approval would authorize only the
local synthetic G01/G02 fixture, its reviewed Rust dependencies and required WASM
build target. No deployment, provider/model calls, customer contact or paid services.
A device/browser installation or system package beyond that selection needs its own
specific decision; a missing tool is an honest blocker, not permission to install it.

## Existing owners and prerequisite decision

Use existing `G02-RESOLVE`, owned by df-rpc-bridge/df-protocol; its actual queue edge
is `G02-RESOLVE -> G01-RESOLVE`. G03 follows G02 and shared boundary gates. The fixture
schema is isolated synthetic test data, never a frozen production RPC API. Shared
contract owner approves its tags; the twelve planned services/41 methods stay intact.
`S00-ACCEPT` later requires independent observed transport execution and its complete
slice prerequisites. A successful experiment alone cannot mark S00/game complete.

First resolve G01's exact toolchain/edition, native+`wasm32-unknown-unknown` features,
lockfile, compiler/protobuf generation method and compile/fmt/Clippy commands.
Then select exact licensed versions using official documentation and actual target
probes: native `tonic`/`prost` are existing candidates; established HTTP/2 and WebSocket
implementations plus browser Rust bindings/local executor must fit the generated
client. None is frozen now. Explicitly test single-threaded WASM execution and
connector/driver compatibility; a native Tonic channel's Send/Tokio requirements
cannot be bypassed with unsafe Send assertions. Record optional legacy Go-wire
compatibility as selected or deferred; do not translate/import archived game code.

## Allowed future edit areas

After approval, use one isolated Git worktree and one task owner. A proposed test-only
crate is confined to `experiments/g02/`: `README.md`, `Cargo.toml`, `Cargo.lock`,
`build.rs`, `proto/transport_probe.proto`, `src/lib.rs`, `src/bin/probe_server.rs`,
`src/wasm.rs` and `tests/transport.rs`. These are nominated paths, not existing files
or authorization to scaffold all subsystem crates. Reserve exact package/target names
through G01 before writing; any different path requires coordinator ownership update.

Keep generated WASM/browser loading glue, build output, browser profiles, disposable
certificates and staging under `artifacts/tmp/g02/`. Author application fixture source
in Rust; only generated JavaScript glue. Store final sanitized result JSON, versions,
traces/metrics and selected synthetic screenshots under
`development/evidence/g02/`; disposable bulk captures/builds remain in artifacts.
Production schemas, application handlers, Pg migrations, payment/media providers,
archive and public site are outside the fixture edit area. The coordinator separately
appends actual devlog/evidence and updates existing gate state after evaluation.

The local owned loopback server runs synthetic handlers only and boot files; browser
activity never joins a real game, exposes private data or invokes paid effects.
Loopback WS qualification must be labeled as such: it does not establish deployed
WSS/TLS, proxy, public Origin/auth or internet/mobile latency. Qualify those separately
under G04/deployment before claiming remote readiness.

## Execution matrix and observable acceptance

Use real generated Rust/WASM browser calls to native HTTP/2 gRPC through binary
WebSocket byte-stream adaptation, not a custom event envelope or grpc-web substitute.
Known numbered payloads and checksums make every lost/reordered byte detectable.
Concrete fixture counts below are proposed experiment cases, not supported group caps.

| Case | Procedure and required evidence |
| --- | --- |
| Unary | Echo a known binary request with synthetic trace metadata; validate response bytes, metadata, OK status/trailers. Trigger a typed INVALID_ARGUMENT and preserve its status/trailers. |
| Server streaming | Consume 128 numbered bounded responses; check order/checksums and final trailers. Pause reading, resume, cancel midstream and verify server/client resource release; other streams continue. |
| Client streaming | Send 64 numbered16KiB chunks, half-close the request direction, then receive the fixture's final count/checksum and trailers. A WebSocket close must not substitute for HTTP/2 END_STREAM. |
| Bidirectional | Interleave64numbered requests/responses on8streams; request half-close still permits the declared remaining response messages and trailers. Cancel one stream independently; no connection-wide silent cancellation. |
| Deadline/cleanup | A100ms-deadline call to a deliberately500ms synthetic handler ends with correct deadline status; manual cancellation preserves cancellation semantics. The selected fixture should release owned tasks/buffers within proposed2s; measure failure rather than claim it. |
| Byte-stream adapter | Split/merge HTTP/2 bytes across1/7/16KiB WebSocket boundaries, partial reads/writes and simultaneous streams. Exact order, metadata, status and EOF semantics survive; no dropped tunnel bytes. |
| Disconnect/rebind | Disconnect midcall: report transport-unknown honestly, never blindly replay a synthetic mutating operation. Reconnect with bounded backoff; explicitly test settled versus indeterminate receipts. This is fixture behavior, not production durable operation recovery. |
| Slow/hidden browser | Suspend/foreground, slow consumer and main-thread pressure; observe native browser receive allocations, WASM memory, process memory, CPU, queued bytes and WINDOW_UPDATE timing. Record unobservable critical bounds as INCONCLUSIVE. |
| Mixed traffic | Mix short action-like unary calls, paced synthetic audio chunks and lower-priority bulk streams on one connection; measure p50/p95/p99 action/queue/first-audio delay and starvation. Synthetic bytes do not qualify real codec/playback. |
| Hostile/oversize | Excess message/header/control frames, ignored flow credit, tiny-frame floods and pre/post-decompression limits terminate/reject visibly without unbounded allocation. Test64admitted streams plus65th bounded rejection/wait per selected policy, not an unbounded queue. |

Required initial source bounds:16KiB HTTP/2 frames,1MiB connection receive window,
256KiB stream window,16KiB compressed header block,256KiB decoded tunnel message,
64concurrent RPCs and8MiB application receive/send budget per connection. Reserve
control queue128frames/256KiB, limit100controlframes/sec. Command/snapshot/chunk
fixtures respect64KiB/1MiB/64KiB envelopes; snapshots use bounded framing, not a
single oversized WebSocket message. Release receive credit only after corresponding
consumed bytes are freed; count outstanding window/decompressed/application buffers.
These are source-defined proposed limits, not measurements and not a whole-browser
8MiB memory promise. bufferedAmount measures outbound backlog, not bounded receive.

Select a bounded desktop fixture first, then the G04-approved desktop/iOS/Android
matrix. Actual physical-mobile evidence is required for mobile-qualified claims;
desktop viewport emulation does not supply it. No mobile device access means an
explicit remaining INCONCLUSIVE gate. One owned server/browser and one build at a
time initially; five person-days is the roadmap's uncertain planning budget, not
permission for unlimited retries. No paid calls; stop on safety/bound failures.

## Commands and evidence, without invented completion

There is no runnable Cargo workspace yet. G01 must pin actual package/target/features
before any compile command is presented as executable. Known source style gates are
fmt/Clippy plus native and WASM compilation; the eventual forms include
`cargo fmt --all -- --check`, `cargo clippy` with the selected target/features, and
locked native/WASM checks/tests. Do not claim these were run, name nonexistent test
binaries, install dependencies during this audit or invent a successful build.

After authorizing and writing the fixture, its README records exact start/build/test
commands and cleanup ownership. Evidence binds Git revision, lock/toolchain/library
versions, browser/OS/device, native/WASM build, selected limits and source digests;
includes results for every row, request IDs, statuses/trailers, metrics distributions,
memory baseline/peak/outstanding credit, cleanup observations and trace/log gap ranges.
Use local read-only OTEL SQLite evidence when its selected G06 adapter is qualified;
unimplemented telemetry integration remains a named gap, not a fabricated database.

Independent frontier computer-use/vision evaluation must operate the real owned
browser fixture, verify live stream/half-close/error/deadline/reconnect and bounded
failure displays, and inspect measured evidence. Code review and screenshots alone
cannot demonstrate protocol or receive-memory correctness. Audible/media tests are
required later for actual game sound, not claimed from this byte fixture.

Result: PASS only for every required selected scope with demonstrated compatible
native/WASM stack and bounded browser receive behavior; FAIL on lost bytes/mode/status,
unsafe workaround/unbounded resource/cleanup failure; INCONCLUSIVE where critical
measurement/device/tool capability is missing. FAIL or unresolved critical receive
bounds block dependent transport/client work. Stop and return the evidence to the
owner before changing protocol or writing broader clients; no silent grpc-web,
custom-envelope, unsafe Send or native-only substitute. Scope changes require a
shared-contract/user decision, not an evaluator's arbitrary approval.

## Goal completion and income proof

| Gate | Actual state / remaining proof |
| --- | --- |
| DesignReady | Independent9.05design and consistent source-backed corpus; historical scope retained. |
| G01/G02/S00 | Pending; exact tools and real native/WASM transport fixture unperformed. |
| G07/full rules rights | Pending; source catalog/errata and reviewed lawful representation for full R01–R17 required. This local transport test cannot clear rights. |
| Executable/full game | No application; all47features/full2024, remote/living-room/private phone, rich directors and optimization remain required. Initial fixture is an evidence slice, not a smaller product. |
| PaidRelease | Pending working source-valid full scope, rights/security/recovery/device/commerce/provider qualification and measured offer/cohort evidence. |
| IncomeValidated | Unproven; actual recognized revenue/COGS/support/acquisition/churn/cash against the user-selected target needed. The target is unanswered; do not replace it with the earlier provisional$50k gross scenario. |

The next income-linked evidence remains an authorized functioning-game cohort with
matched offer/usage, loaded support+generation, price acceptance and retained paid
play. Local G02 success only removes one engineering uncertainty. G07 rights feasibility
and transport feasibility precede broad irreversible implementation; neither proves
buyer demand, server scale or income. No customer contact/counsel purchase is authorized.

Sources: current governing fingerprint
`11b95cdca8377b33867e9991df2d0029d27697d0ee677c99a5fb132796b1c21c`;
manifest `5a8c0c6a952c04b8e700bc6d5235f166a785d1702ca6cf59334ad3b2e0b858d9`.
This packet is a read-only next-phase decision; independent completion/packet review PASS. No prototype/dependency authorization has been given.
