# Browser build pipeline

Date: 2026-10-01 02:00 UTC
Task/attempt: `B-G01-D02/a2`
Input revision: `01c2ccf09545877d65ffe490975b4d9da267cd67`

## Decision

Build browser clients from Rust application source for `wasm32-unknown-unknown`, using the repository-pinned stable Rust `1.98.1`, Rust edition `2024`, workspace resolver `3`, and tracked `Cargo.lock`. Keep native builds on the same compiler pin. Generate browser loading and API glue from the compiled Rust/WASM module using the cached, lock-consistent `wasm-bindgen 0.2.129` CLI with `--target web`. Generated glue is an output of the build; browser application and UI logic remain Rust. Do not hand-author JavaScript or TypeScript application code, introduce a new framework or crate as part of this decision, or commit generated output.

The browser boundary is Rust source → Cargo's WASM artifact → version-matched `wasm-bindgen` generated loader/types/bindings. Keep raw compiler WASM and bindgen-rewritten WASM separately identified: they are different artifacts. The generated web loader expects its generated `df_tools_bg.wasm` beside it by default. Stage the complete output set together so loader, declarations, and bound module share one build identity. The actual preview publication strategy and atomic paired replacement remain unresolved.

This selects a build pipeline, not a UI framework. Existing source uses `wasm-bindgen` from `df-tools` entrypoints and `df-rpc-bridge` browser APIs. Cargo lock resolution pins the Rust crates; the cached CLI version must match that graph. The repository wrapper `development/build-fixture.sh` is useful source context but is not the bounded invocation: it sets `CARGO_BUILD_JOBS=2`, omits Cargo offline mode, and moves a staged directory into its target tree. Invoke Cargo directly with explicit `--jobs 1 --locked --offline` for reproducible, bounded local checks.

## Bounded contract example

For the existing `df-tools` Rust library, compile the `wasm32-unknown-unknown` `cdylib`/`rlib`, then run the matching generator against exactly `artifacts/build/browser-pipeline-g01-d02/wasm32-unknown-unknown/debug/df_tools.wasm`. With `--target web`, the generated interface exposes Rust's `#[wasm_bindgen(start)]` entry as `start(): void` and an async default initializer; the declarations describe the module ABI, and the generated loader provides the WebAssembly instantiation and browser shims. This is evidence of the source/build/generator boundary and generated glue, not proof that the page runs.

## Pinned tool and package policy

- Rust toolchain: `1.98.1`, minimal profile with `rustfmt` and Clippy; target `wasm32-unknown-unknown` is declared in `rust-toolchain.toml`.
- Language and workspace: edition `2024`, minimum Rust `1.98`, resolver `3`.
- Dependency reproducibility: retain `Cargo.lock`; use `--locked --offline`. `wasm-bindgen` Rust dependency resolves to `0.2.129` in the lockfile, and the observed cached generator is `wasm-bindgen 0.2.129`.
- Initial native qualification target for this host: `aarch64-apple-darwin`. Native-only `df-telemetry` is excluded from WASM checks by selecting only the affected WASM-capable crates.
- Build roots are attempt-owned: `artifacts/build/browser-pipeline-g01-d02` and scratch/output staging `artifacts/tmp/B-G01-D02-a2`. Cache roots are the existing repository cache under `artifacts/cache`; automatic tool installation and network access are disabled.
- Do not invoke the wrapper's build mode for this bounded run. Its fixed two-job setting and lack of offline mode do not meet this attempt's explicit resource and network bounds.

## Exact checks and generated artifacts

The following commands were executed from the worktree root with `RUSTUP_HOME=/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/rustup`, `CARGO_HOME=/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/cargo`, `CARGO_TARGET_DIR=/Users/earlcameron/Desktop/dungeonflux/artifacts/build/browser-pipeline-g01-d02`, `TMPDIR=/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G01-D02-a2/temp`, `RUSTUP_AUTO_INSTALL=0`, `RUSTUP_TOOLCHAIN=1.98.1`, `CARGO_NET_OFFLINE=true`, `CARGO_BUILD_JOBS=1`, and PATH preferring the cached Cargo bin directory:

```sh
cargo +1.98.1 fmt --all -- --check
cargo +1.98.1 clippy --locked --offline --jobs 1 -p df-tools --all-targets --target aarch64-apple-darwin -- -D warnings
cargo +1.98.1 clippy --locked --offline --jobs 1 -p df-rpc-bridge --all-targets --target aarch64-apple-darwin -- -D warnings
cargo +1.98.1 clippy --locked --offline --jobs 1 -p df-tools --all-targets --target wasm32-unknown-unknown -- -D warnings
cargo +1.98.1 clippy --locked --offline --jobs 1 -p df-rpc-bridge --all-targets --target wasm32-unknown-unknown -- -D warnings
cargo +1.98.1 build --locked --offline --jobs 1 -p df-tools --lib --target wasm32-unknown-unknown
/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/cargo/bin/wasm-bindgen --target web --out-dir /Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G01-D02-a2/web-staging /Users/earlcameron/Desktop/dungeonflux/artifacts/build/browser-pipeline-g01-d02/wasm32-unknown-unknown/debug/df_tools.wasm
```

All six Cargo checks/build commands and generation exited successfully. The generated staging contains `df_tools.js`, `df_tools.d.ts`, `df_tools_bg.wasm`, and `df_tools_bg.wasm.d.ts`. A bounded section reader inspected both actual WASM binaries: the raw compiler module has 70 imports and 2,715 exports, while bindgen’s rewritten module has 68 imports from `./df_tools_bg.js` and 18 exports (including `start`, memory, the externref table, and the bindgen start shim). The complete names, kinds, and indices are retained in `wasm-module-inventory.json`; its scoped standard-library section reader is also retained and hashed. The declaration file independently inventories the public generated interface. The generated loader uses `WebAssembly.instantiateStreaming` when possible and a `WebAssembly.instantiate` fallback. Generated glue contains browser API shims used by the existing Rust source. No browser was launched and no audio was observed.

Exact argv, environment, stdout/stderr, elapsed times and exit statuses are retained in `development/evidence/browser-pipeline-g01-d02/a2/worker/verification.json`. Tool, source/config/package, raw WASM, generated output, module import/export inventory, and distinct raw-versus-bound WASM hashes and sizes are retained in `development/evidence/browser-pipeline-g01-d02/a2/worker/pipeline-evidence.json` and `wasm-module-inventory.json`. Generated build files are not repository source and are not committed.

## Alternatives and unresolved decisions

A floating stable channel does not give a reproducible compiler identity; nightly is unnecessary for the current code and adds churn. The checked-in stable pin is therefore used. A manually authored JS/TS loader or application, a different framework, another Rust/WASM generator, or a new dependency would exceed this task's policy or source approval. The existing generated `wasm-bindgen` path is available and works with the cached artifacts, so no alternative generator is selected.

The following remain open and are not claims of this decision: Rust UI/component framework and rendering architecture; browser/server application integration; browser RPC driver, HTTP/2 and executor feasibility; actual browser startup/interaction; device and supported-browser matrix; touch, keyboard, accessibility and display constraints; browser memory/CPU and performance budgets; release bundling, MIME/hosting configuration, cache policy, paired artifact publication/rollback; audio playback; production readiness. Existing subsystem and roadmap owners must resolve these at their named gates. Compilation and generated interface inspection do not establish any of them.

## Acceptance and verification mapping

Original acceptance criterion 1: “Rust source generated glue”
Evidence: existing Rust browser entry/source, successful fresh WASM library build, and successful `wasm-bindgen 0.2.129 --target web` generation from that exact raw WASM input. The loader/types and bound WASM are retained in owned staging, with hashes in the attempt evidence.

Original acceptance criterion 2: “The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.”
Evidence: pinned source/config/package hashes, exact command results, raw module hash, generator binary/version hash, generated artifact hashes and bounded interface inventory are recorded. Browser execution, audio, device compatibility, release/hosting and production checks are explicitly unperformed.

Original verification 1 (preserved): “Freeze the cited source decision and a bounded contract example; compare Rust source generated glue. Retain decision, alternatives and unresolved facts.”
Result: satisfied by the decision, bounded `df-tools` contract example, alternatives, open items, and the recorded build-bound generated interface.

Original verification 2 (preserved): “Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.”
Result: G01 supplied exact bounded commands; all commands listed above were run and their outcomes retained. The statement's original caveat remains: no browser/provider execution is claimed. Provider checks are outside this pipeline decision.

## Provenance

Governing design: `planning/implementation-roadmap.md` (G01 toolchain/targets and G02 transport gates), `planning/subsystem-interfaces.md` (browser boundary), `planning/client-architecture.md` (Rust application/UI and generated-glue policy), `planning/rpc-transport.md` (browser feasibility remains a G02 acceptance gate), and `planning/toolchain-targets.md` (pinned toolchain/target and package selection). `AGENTS.md`, `planning/coding-style.md`, ADR 0001–0005, the fixture, manifests, lockfile, toolchain and formatting config were consulted as required. Their exact input hashes are in the retained evidence record. Submitted source changes only this policy document; no source, dependency, lockfile, generated output, app, script or framework was added.
