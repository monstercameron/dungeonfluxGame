# G02 transport resource and pressure qualification

`QUALIFY-G02-001`, attempt `QUALIFY-G02-001-a1`, extends the frozen experimental
S00 service without changing protobuf numbering or HTTP/2/gRPC semantics. This is
one required step toward the full project. **Full G02 remains INCONCLUSIVE** until
physical iOS/Android, supported desktop/network, suspend/foreground, CPU/process
memory, browser engine allocation and actual audio/action-to-view evidence exists.
The desktop loopback subset never authorizes dependent production clients.

## Reproduce the isolated preview

Default `./development/build-fixture.sh build` / `serve` still uses
`artifacts/build/start-s00`, `artifacts/tmp/start-s00` and loopback port43180. An
isolated attempt can select independent owned roots and a different loopback port:

```sh
export DUNGEONFLUX_ARTIFACT_ROOT=/Users/earlcameron/Desktop/dungeonflux/artifacts
export DUNGEONFLUX_BUILD_ROOT="$DUNGEONFLUX_ARTIFACT_ROOT/build/qualify-g02"
export DUNGEONFLUX_TMP_ROOT="$DUNGEONFLUX_ARTIFACT_ROOT/tmp/QUALIFY-G02-001-a1"
export DUNGEONFLUX_PREVIEW_PORT=43181
./development/build-fixture.sh build
./development/build-fixture.sh serve
```

Open `http://127.0.0.1:43181`. Run **Run transport checks**, then **Run desktop
pressure qualification**. Both share an exclusive run owner; a second click while
one is active does not start competing work. Source/build identity appears in the
page and reports. Freeze a clean candidate, rebuild and record hashes before review.
The server accepts only its own configured loopback Origin, at most four admitted
connections and eight concurrent streams per connection. No paid/provider call occurs.

Full Rust-produced reports are retained by the preview at
`/fixture-report/semantics` and `/fixture-report/qualification`. Two bounded64KiB
slots retain only synthetic diagnostics; writes require the preview Origin and a
bounded POST body. `/fixture-resources` shows current/peak connection snapshots,
retaining at most20 owners (active plus closed), pruning only fully released old
pipes. `/fixture-health` exposes native active/completed/cancelled/deadline counters;
`/fixture-telemetry` exposes the existing bounded, nondurable shared OTEL export.

## What the experiment measures and bounds

`ConnectionSnapshot` records actual byte/item current, peak and transfer totals for
browser callback queues, native pipes and adapter-owned WebSocket receipt references.
Native pipe reads/writes update counters in the same short synchronous poll critical
section, preventing a receiver from subtracting before a sender records its write.
Both pipe endpoints own the counters; only their final drop clears freed unread
bytes. Receipt guards release actual message ownership on success/error/cancellation.
Browser queues clear when callbacks/driver are dropped. `closed` means adapter drop;
`pipe_owners` independently proves native pipe release. Browser `bufferedAmount` and
native in-flight send size are sampled; the last observed engine backlog is preserved
on close because the engine can continue draining. Send `total` is observed wire bytes,
not an estimate of allocation or delivery acknowledgement.

A fixed-size passive observer handles arbitrarily split HTTP/2 frame headers,
connection DATA and WINDOW_UPDATE. DATA frame payload length consumes real credit;
connection WINDOW_UPDATE in the opposite direction grants it, starting from RFC
initial65535 bytes. `available`, `peak_grant`, DATA/update totals and `peak_held` are
wire credit observations, not heap measurements. Stream semantics, HPACK, reset
retention and protocol validation remain entirely in established h2. Oversized
frames and more than2048 HTTP/2 frames per100ms terminate explicitly. The browser
and native adapters additionally reject the 101st peer-originating control frame
in a rolling second per physical connection. The incoming count includes
PRIORITY, RST_STREAM, SETTINGS (also ACK), PUSH_PROMISE, PING, GOAWAY,
WINDOW_UPDATE (connection and stream), and unknown extension types. DATA,
HEADERS and CONTINUATION remain under the existing DATA credit, header/protocol
and coarse frame protections. The outgoing direction retains the coarse frame
guard and separate control counts. The bounded incoming diagnostic retains at
most100 admitted frame types, stream IDs and relative times plus the rejected
frame, so a valid WINDOW_UPDATE pressure conflict remains visible rather than
being silently exempted.
The browser
adapter hands h2 at most16KiB per read, yields through an owned cancellable timer
at64KiB, and caps callback backlog at1MiB/256items. Native work yields each message
and16KiB chunk, including empty/control messages. A finite run owns all wait timers,
frame callbacks, streams and driver cancellation; total qualification deadline30s.

The native WebSocket read/write buffers are selected16KiB, maximum write buffer
256KiB, decoded message/frame256KiB; the pump admits one message at a time. h2
connection receive credit1MiB, stream credit256KiB, send buffer256KiB/stream,
decoded header list16KiB and codec frame16KiB are unchanged. The pinned h2 codec
caps unfinished continuation chains at five nonterminal frames at these settings.
Decoded/encoded protobuf remains64KiB. No compression is enabled.
The browser shares at most eight private consumed-credit slots per physical
connection. After the caller next polls a delivered DATA frame, it returns that
frame's credit through h2 at256KiB per stream,512KiB aggregate, or when that
stream's h2 receive poll is Pending; terminal/error/drop paths return consumed
remainders and remove cloned handles. Unconsumed current frames are never
credited early. This coalesces ordinary updates without changing either
advertised window or the100 incoming-control/second admission rule.

The **payload capacity reservation** sums: native two1MiB pipes (browser one1MiB
callback queue), one1MiB connection credit, eight stream reservations of
256KiB send +64KiB encode +64KiB decode +16KiB decoded headers, two256KiB
WebSocket message/send reserves, and ten16KiB frame/partial-header/scratch reserves.
This is7,110,656 native /6,062,080 browser bytes, below the selected8MiB envelope.
It deliberately reserves concurrent capacities rather than adding credit to actual
current bytes as if both were independent live data. It excludes allocator/control
metadata, process/shared instrumentation, engine heap, network kernel buffering and
browser allocations before callback entry. Therefore it is an implemented bounded
payload plan, **not proof that total per-connection memory fits8MiB**. Current/peak
counters and whole allocated WASM linear-buffer length remain distinct observations.
An oversized engine ArrayBuffer is observed only after allocation; rejection before
Rust copying does not bound that engine allocation.

Shared OTEL counts now use actual application payload bytes at service scopes and
wire bytes at bridge scopes. Measurement failures and abandoned spans mark
`rpc.bytes.measured=false` and omit the byte attribute instead of fabricating zero.
Fixture telemetry still lacks durable G06 export and full browser SDK ingestion.

## Real traffic and adversarial scenarios

Pressure runs use existing synthetic Sample fields: `pressure:slow`,
`pressure:paced`, `pressure:bulk` payloads and a bounded sequence request count.
These are fixture behavior, not new production service APIs. The same connection
simultaneously carries64×48KiB slow-consumer replies (20ms consume pauses),
80×8KiB paced replies (8ms server pacing),128×48KiB bulk replies and60 small unary
RPCs. Thirty warm unary samples precede pressure. Exact sequence counts and generated
OK/custom terminal trailers prove no silent loss. Reports retain every RPC latency
and animation-frame interval, p95/p99, cold connection time, before/peak/after
allocated WASM buffer length, browser user agent, actual resource snapshots, and
real server cancellation/owner counters. A WAIT stream is cancelled and polled back
to baseline. A post-close snapshot verifies browser queue/callback owner cleanup.

Finite malicious loopback server routes test browser receipt of256KiB+1 binary
messages,16KiB+1 HTTP/2 frames, compressed small HPACK blocks with decoded lists
above16KiB,128 unfinished continuation frames, and2049 tiny HTTP/2 control frames.
An additional finite route sends exactly101 total SETTINGS controls including
the initial SETTINGS and requires the 101st to be rejected by the adapter.
Each probe requires an actual generated RPC error within2500ms. Header-list excess
is a stream rejection; a healthy connection need not fail. Other probes require
observed adapter rejection or h2 driver failure. Endpoint send/wait work is finite
and admission-bounded. These deliberate test servers do not weaken the normal tunnel.
Native synchronized tests exercise oversized WebSocket/frame boundaries, header-list
rejection response plus pinned PROTOCOL_ERROR stream reset, h2 continuation cap,
control flood termination, accounting and owner release with finite deadlines.
Controlled-time bridge tests cover exactly100 admitted controls, the 101st
rejection, exact one-second expiry, split headers, mixed streams/types,
independent connections, outgoing isolation, and the unchanged 2048/100ms
guard using DATA-only frames. If the unchanged simultaneous pressure run fails,
its report retains bounded incoming frame type/stream/relative-time observations,
the rejected frame, connection cleanup and tests not reached. Such a failure
blocks this repair pending a contract decision; it is never labeled PASS.

Local RPC p95/p99 and animation intervals are measurements, not approved production
budgets. Synthetic paced bytes do not measure first audio, acoustic jitter, playback
buffer behavior, action-to-view latency or remote WSS. Whole-process RSS/CPU samples,
when retained, are process observations and cannot be divided into fabricated
per-connection allocations. Browser engine pre-callback memory remains unresolved.

## Required checks and evidence

Use the selected artifact-root variables above for all worktree commands:

```sh
./development/build-fixture.sh cargo fmt --all -- --check
./development/build-fixture.sh cargo clippy --locked --workspace --all-targets -- -D warnings
./development/build-fixture.sh cargo test --locked --workspace
./development/build-fixture.sh cargo clippy --locked -p df-rpc-bridge --target wasm32-unknown-unknown -- -D warnings
./development/build-fixture.sh cargo clippy --locked -p df-tools --lib --target wasm32-unknown-unknown -- -D warnings
./development/build-fixture.sh build
```

Retained task evidence belongs in `development/evidence/qualify-g02/worker` on the
main repository: exact candidate/hash mapping, host/browser identity, command
results, full reports, native snapshots/health, process samples and actual CUA
screenshots. The startup verified preview and build stay protected; qualification
has its own preview PID and output root. Preserve PASS/FAIL/INCONCLUSIVE separately,
and request the physical-device/network inputs needed to close full G02.
