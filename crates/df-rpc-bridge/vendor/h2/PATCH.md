# Experimental browser clock patch

Upstream: `hyperium/h2` version 0.4.19, crates.io source distribution. This is the
Upstream commit: `d57d1b852fec9dda6d42d3454502006d52104da8`. Published crate archive SHA256:
`ef8e5e5a340588f4452631496976cf8636d4a7ecf600239fdc27615d2530bc16`.

This is the established HTTP/2 implementation, retained under its MIT license. Source/README/
LICENSE and normalized package manifest are copied from the published package;
examples, upstream tests, benches and lockfile are omitted from this minimal copy.

Only two authored source changes replace the `Instant` imports in
`src/proto/streams/{recv,stream}.rs` with `web_time::Instant` on wasm32. A wasm32
`web-time = 1.1.0` dependency is added. Native still uses std::time::Instant.

Observed reason: the unpatched driver completed all four browser RPC modes but
panicked at `NextResetExpire::set_queued` on cancellation because std Instant has
no browser clock. Disabling reset retention would change the handling of legal
in-flight frames after cancellation; this patch preserves retention and HTTP/2
semantics. web-time uses browser Performance.now. Suspend-time behavior still
needs physical-device qualification, as documented upstream by web-time.

No HTTP/2 framing, HPACK, flow-control or cancellation logic is changed. The
workspace patch applies to both targets so there is one driver version. This
copy is excluded from authored-code formatting gates; the two import adaptations
are reviewed with the bridge. Future upstream support can replace it only with
renewed browser/reset qualification.

References: https://docs.rs/h2/0.4.19/ ; https://docs.rs/web-time/1.1.0/
