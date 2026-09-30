# Low-latency RPC transport

Date: 2026-09-29
Status: Rust bridge direction agreed; API shape planned, browser feasibility pending

## Direction

Use generated Rust RPC clients/services and Protocol Buffers for structured game
communication. Port the GoGRPCBridge transport idea into a dedicated Rust
subsystem: native HTTP/2 gRPC carried over a persistent binary WebSocket, secured
with WSS in deployment. Keep server handlers independent of the tunnel adapter.

The intended path is:

```text
Rust/WASM client -> binary WebSocket -> Rust byte-stream adapter
                -> HTTP/2 gRPC server -> application services -> session owner
```

Page and WebAssembly boot use normal browser HTTP delivery. Game commands, view
subscriptions, and server audio streaming use RPC. Any later bulk-asset delivery
policy must be explicit; do not inherit the old demo's transport exceptions.

## What the original bridge actually does

The old game pins GoGRPCBridge v1.1.2. Its `pkg/grpctunnel/conn.go` exposes binary
WebSocket payloads as a continuous byte stream, and `server.go` feeds that stream
into an HTTP/2 server or the native gRPC listener in `server_native.go`. Its WASM
dialer adapts the browser socket and bounds incoming buffering. This preserves
native gRPC behavior rather than translating calls into a new RPC envelope.
Reviewed tag tree: `d21b8151259d47dfa2496ed3e13ac3b11a968db7`.
See the [source at v1.1.2](https://github.com/monstercameron/GoGRPCBridge/tree/v1.1.2/pkg/grpctunnel)
and [browser adapter](https://github.com/monstercameron/GoGRPCBridge/blob/v1.1.2/pkg/wasm/dialer/websocket_conn.go).

Preserve unary, server-streaming, client-streaming, and bidirectional RPCs,
including metadata, statuses/trailers, deadlines, cancellation, and half-close.
Use established HTTP/2 and protobuf implementations; do not write a new HTTP/2
stack or invent an incompatible protocol while calling it a direct port.
The standard grpc-web browser client currently lacks client/bidirectional
streaming, so it is not a drop-in implementation of this requirement. See its
[streaming support](https://github.com/grpc/grpc-web#streaming-support).

## Rust feasibility gate

Candidate components are `tonic` for native gRPC, `prost` for protobuf generation,
an established Rust WebSocket server, and Rust browser WebSocket bindings. Library
versions and the exact browser RPC stack remain unselected.

Tonic supports custom connection streams on the server and custom connectors for
native clients. Its normal channel connector requires `Send` connection/future
types and uses a Tokio executor by default; that is not evidence that the same
channel works directly with single-threaded browser WebAssembly. Verify the
browser executor, HTTP/2 driver, I/O adapter, and generated-client compatibility
before dispatching dependent client work. See [server incoming streams](https://docs.rs/tonic/latest/tonic/transport/struct.Server.html)
and [channel connector requirements](https://docs.rs/tonic/latest/tonic/transport/struct.Endpoint.html).

The first transport implementation task, once detailed transport planning is complete,
must demonstrate all four RPC modes between a real Rust/WASM browser client and
the Rust server. Also verify trailers/errors, deadlines, half-close, cancellation,
simultaneous streams, and cleanup. Native-only success is insufficient. If a
browser limitation requires changing semantics or protocol, report the measured
gap and revise the design explicitly before dependent implementation.

Wire compatibility with the original Go bridge is an open acceptance decision;
do not assume the Rust port must preserve every Go API or legacy application
schema. Retain source license/attribution if code is translated or reused.

## Latency and bounded resources

Reuse an established connection and multiplex RPC streams. Keep action requests
small and binary; avoid per-action connection setup, JSON/base64 conversion, and
unnecessary buffering delays. Preserve byte ordering across partial reads/writes
and WebSocket message boundaries. Never silently discard tunnel bytes; overflow
needs a visible failure and controlled recovery.

Bound messages, concurrent RPCs, connection counts, and queue sizes in bytes as
well as item counts. Browser callbacks enqueue work without blocking rendering.
Respect HTTP/2 flow control and browser outbound buffering. Define cancellation,
drain, and close ownership so blocked I/O can be released during shutdown.

Audio uses paced chunks and bounded buffers. Measure action latency during audio
and asset traffic; multiplexing over one TCP connection still shares congestion
and ordered delivery. Keep bulk work from filling the action path. Tune frame
sizes, windows, scheduling, and compression from evidence; introduce extra
connections only if measurements justify them.

Measure cold connection time, warm RPC round trips, action-to-view latency,
first-audio time, jitter, p95/p99 queue delay, bytes, CPU, and memory. Separate
network delay, bridge overhead, session scheduling, database time, and provider
latency. Set budgets against the supported device/network matrix in transport
planning; do not promise low latency solely because the protocol is gRPC.

## Authority, recovery, and observability

RPC handlers submit actions to the session owner and return its confirmed result,
not just successful enqueue. Preserve operation identity and distinguish retryable
transport failure from an authoritative rejection. A disconnect after submission
can leave the result unknown; resync or query the result rather than blindly
replaying a mutating call. Reconnecting streams preserve identity and recover a
current permitted view without joining another player.

Plan browser-compatible authentication, origin checks, connection limits,
keepalive, liveness detection, bounded reconnect backoff, and foreground resync.
Check authentication at the RPC boundary as well as tunnel admission. Do not put
long-lived credentials in URLs or treat browser-supplied trace metadata as identity.

Instrument each RPC and the bridge with OpenTelemetry: method, status, duration,
connection lifecycle, queue delay, bytes, cancellation, retries, and reconnects.
Propagate trace context per RPC and through asynchronous jobs. Retain structured
logs and correlated spans in the SQLite review corpus under
[Observability](observability.md), with PostgreSQL game persistence unchanged.

Service methods, message shapes and compatibility semantics are now planned in
[RPC API](rpc-api.md); Rust boundaries are in
[Subsystem interfaces](subsystem-interfaces.md). Concrete protobuf definitions,
library/runtime selection and connection/device budgets remain gated in the
[roadmap](implementation-roadmap.md). Follow
[Runtime reliability](runtime-reliability.md) and
[Subsystem architecture](subsystem-architecture.md).

## Browser resource proof and feasibility result

G02 must qualify browser delivery buffers beyond bounded Rust channels. Standard
WebSocket has no receive backpressure; the bridge must limit peer emission through
HTTP/2 DATA window credit, not merely pause reading callbacks. Initial frame16KiB,
connection receive window1MiB, stream window256KiB, header16KiB and decoded tunnel
message256KiB fit the service envelope. Increment WINDOW_UPDATE only after the
corresponding bounded consumed bytes are released, including reserved control
credit, and assert total outstanding window/decompressed/application bytes per
connection<=8MiB. A compliant peer never emits beyond this credit; an untrusted
peer ignoring it is terminated, though browser allocation before callback remains
a platform risk that actual memory measurement must bound. Native gateway enforces
outgoing frame/message/credit limits before send. Client checks incoming message
size before parsing, closes on excess/flood, uses time-sliced decode and yields.
No arbitrary header continuations, tiny-frame CPU floods or decompression bombs.
Bulk assets have lower scheduling priority than action/audio/control; reserved
control frames remain bounded and do not starve DATA. WebSocket bufferedAmount is
send backlog only, not proof of bounded receive memory. [MDN WebSocket](https://developer.mozilla.org/en-US/docs/Web/API/WebSocket).

Real single-threaded Rust/WASM G02 fixture records all four RPC modes, status/trailers,
half-close/deadline/cancel under suspend/foreground, slow consumers, mixed bulk and
malicious messages across selected desktop/iOS/Android browsers; retain process and
WASM memory, native buffer observables/limitations, CPU and action/audio p95/p99.
Result is PASS/FAIL/INCONCLUSIVE with library/driver/browser/build versions. FAIL or
unobservable critical receive bound blocks every dependent transport/client task;
owner redesign must retain required native gRPC bridge semantics and Rust source.
No silent grpc-web/custom envelope/unsafe Send compatibility substitute. Any changed
transport mechanism requires renewed shared contract and user-scope decision.
None of these executable browser qualifications has been performed.
