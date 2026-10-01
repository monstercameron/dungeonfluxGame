# Experimental native HTTP/2 transport fixture decision

Date: 2026-10-01
Task/attempt: `B-G02-D01-a1`
Decision input: `e6d69216f02862677742198e14fcde4e0cec026d`

## Selected boundary

Select the existing S00 fixture path: a generated Rust `tonic`/`prost` client sends
ordinary gRPC over an established `h2` HTTP/2 session. Its ordered bytes cross
one binary browser WebSocket to the native byte pump, then enter the native
`tonic` server. WebSocket message boundaries do not define RPC calls. There is
**no custom RPC envelope**, grpc-web translation, or application JSON framing.
This is a local experimental decision, not production transport qualification.

The browser uses `web_sys::WebSocket` as bounded `AsyncRead`/`AsyncWrite` and a
single-threaded local `h2` driver. `BrowserChannel` implements the service shape
consumed by the generated `tonic` client without treating a native `Channel` as
browser-safe or adding unsafe `Send`/`Sync`. The native `axum` WebSocket adapter
feeds a bounded byte pipe accepted by the `tonic` HTTP/2 server. The bridge does
not know protobuf service methods or game semantics. `df-protocol` owns the
experimental four-method schema; `df-tools` owns the runnable fixture.

The frozen source pins Rust 1.98.1, `tonic` 0.14.6, `h2` 0.4.19, `web-time` 1.1.0,
and `wasm-bindgen` 0.2.129 through `rust-toolchain.toml` and `Cargo.lock`.
The narrowly vendored `h2` copy changes two wasm32 `Instant` imports to
`web_time::Instant` so cancellation reset retention can use a browser clock;
native timing and HTTP/2 framing remain the established implementation. Its
published source identity, MIT attribution and patch are recorded in
`crates/df-rpc-bridge/vendor/h2/PATCH.md`.

The experiment selects 16 KiB HTTP/2 frames, a 256 KiB decoded WebSocket message,
1 MiB connection receive credit, 256 KiB stream credit, eight concurrent streams,
and 64 KiB encoded/decoded service protobuf. The browser callback queue is capped
at 1 MiB and 256 messages. Owned driver, upload, callback and pipe lifetimes have
explicit cancellation or drop paths. These selected values and local observations
do not establish a total 8 MiB browser connection bound: browser allocation before
the callback, engine heap, allocator overhead and network buffers remain outside
the counted payload reservation.

## Current-source comparison

At the decision input revision, `crates/df-protocol/proto/transport_fixture.proto`
declares `Unary`, `ServerStream`, `ClientStream` and `Bidi` in the experimental
namespace. `df-protocol` generates their Rust types. In
`crates/df-rpc-bridge/src/browser.rs`, `h2::client::Builder` handshakes on the
browser WebSocket byte stream, `spawn_local` owns the driver, and `BrowserChannel`
forwards HTTP/2 request/body/trailer frames to the generated client. In
`crates/df-rpc-bridge/src/native.rs`, the accepted WebSocket supplies bytes through
the native pipe to `tonic`. `crates/df-tools/src/fixture.rs` mounts the generated
service, native preview and diagnostics; its Rust/WASM browser consumer exercises
the four RPC modes. `crates/df-rpc-bridge/src/resources.rs` observes frames,
credit and owned buffers while leaving HTTP/2 and HPACK semantics to `h2`.

The current telemetry owner records actual application payload bytes for service
spans and observed wire bytes for bridge spans. If a bridge measurement fails or
a span is abandoned, `df-observe` sets `rpc.bytes.measured=false` and omits the
byte attribute. This is diagnostic evidence, not transport authority or durable
G06 telemetry.

The source anchors for this decision are input commit
`e6d69216f02862677742198e14fcde4e0cec026d`, `Cargo.lock` SHA256
`c29d0c7cd8d60e7d63aa16803713de3bd8145ed21e614b50121c0d0f55fdd947`,
schema SHA256 `047445cb5208bb1c2e16c2afadc43996f5652015053cf2ffe049d9bfc726470d`,
browser bridge SHA256 `927f2959ab4eb6a3787923eee71a56f6ba06e6fec3145aacedc2499bcb6211e9`,
native bridge SHA256 `87976accf11c7b483c418a0b0b539e253dd2dd4b1e53c9943971d97e53d7c128`,
and telemetry owner SHA256 `7da9ff47ac7f9f8d104d82b232c04cab234fc37d82ef97da15e7c541cf228301`.
The attempt handoff records the exact submitted commit and rebuilt artifacts.

## Bounded contract example

This small example shows only the byte-stream choice. Two binary WebSocket
messages may split the HTTP/2 client preface anywhere; the native byte adapter
concatenates their bytes for the established HTTP/2 parser. The WebSocket messages
carry no call identifier, method, status, or added length field. The actual
bridge's flow control, parsing, RPC behavior and memory safety require the real
source and browser checks; this example does not implement them.

```rust
const HTTP2_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

fn native_byte_stream(binary_messages: &[&[u8]]) -> Vec<u8> {
    binary_messages
        .iter()
        .flat_map(|part| part.iter().copied())
        .collect()
}

fn main() {
    let split = 11;
    let messages = [&HTTP2_PREFACE[..split], &HTTP2_PREFACE[split..]];
    assert_eq!(native_byte_stream(&messages), HTTP2_PREFACE);
}
```

## Alternatives and unresolved facts

- **grpc-web or a custom RPC envelope:** rejected because these would change the
  selected native HTTP/2 gRPC byte path and the required four-mode semantics.
- **Native `tonic::Channel` in browser WASM:** rejected as an unsupported runtime
  assumption; its native connector and executor requirements do not prove a
  single-threaded browser implementation.
- **New HTTP/2 parser or RPC dispatcher in the bridge:** rejected because `h2`
  and generated `tonic`/`prost` already own those semantics.
- **Promote the synthetic fixture to production:** rejected. G03 owns production
  contracts, numbering and authorization; the fixture is not a game service.

The current source and bounded example support the selected protocol decision.
The attempt's exact committed build and desktop browser reports supply separate
running evidence. Full G02 remains inconclusive until device/network coverage,
suspend and foreground behavior, browser engine allocations before callback,
CPU and process memory, action-to-view and actual audio timing, and the required
resource/control-rate acceptance are measured. The desktop fixture does not
authorize production clients or establish Go wire/API compatibility. Shared CUA
engine memory cannot be attributed to this attempt and must be reported as a
resource-observation gap.

## Original acceptance and verification

The original criterion “no custom envelope” is met as a source decision by the
unchanged HTTP/2 byte path above; the exact submitted build and live browser
checks are retained separately in this attempt's worker evidence. The second
criterion requires that every source/build claim name its actual identity and
that failed, unsupported, pending and unperformed checks remain explicit. The
original verification calls for the cited source decision, a bounded example,
alternatives and unresolved facts; this document supplies those without changing
the criterion. Its formerly TBD executable commands are resolved by the frozen
attempt brief and retained with their actual results, not inferred from this
document. Independent frontier review and resulting mainline acceptance remain
coordinator decisions.
