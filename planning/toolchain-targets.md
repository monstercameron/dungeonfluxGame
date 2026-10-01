# Pinned Rust toolchain and initial targets

Date: 2026-09-30 20:15 America/New_York  
Source revision: `916298ecbb0552c32981cbafab94eb94eb6bbadb`  
Task/attempt: `B-G01-D01/a1`

## Decision

Use the repository's existing stable Rust `1.98.1` pin for native and
`wasm32-unknown-unknown` builds. Keep Rust edition `2024`, workspace resolver
`3`, and the checked-in `Cargo.lock`. The pinned minimal profile includes
`rustfmt` and Clippy. Native development is qualified on the observed host
`aarch64-apple-darwin`; the initial browser compilation target is
`wasm32-unknown-unknown`. This chooses the compiler and target triples already
declared by `rust-toolchain.toml` and `Cargo.toml`, and the cached installation
was independently enumerated before compiling.

The decision is supported by an isolated, dependency-free standard-library
contract probe compiled with the pinned compiler for both targets. Native code
was executed and returned `42` for input `35`; the WebAssembly file was read as
a real MVP module and its export section contains `memory` and `contract_value`.
That demonstrates target compilation and a common Rust contract symbol. It does
not execute WebAssembly in a browser or qualify the application, RPC runtime,
framework, UI, generated bindings, or production crate feature set.

No Rust application or UI framework is selected here. Roadmap G01/G04/G08 and
`planning/client-presentation.md` leave component implementation, device
support, UI frameworks, codecs, and presentation budgets open. The Rust-only
application rule and permission for generated `wasm-bindgen` JavaScript glue
remain governed by `AGENTS.md`; the checked-in build fixture currently pins
`wasm-bindgen 0.2.129`, but the probe did not generate glue. Browser gRPC,
executor and stream feasibility remain G02 work under
`planning/rpc-transport.md`.

## Cache and observed compiler provenance

All tool invocations below used the repository-owned cache, with automatic
toolchain installation disabled:

```sh
export RUSTUP_HOME=/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/rustup
export CARGO_HOME=/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/cargo
export RUSTUP_AUTO_INSTALL=0
export RUSTUP_TOOLCHAIN=1.98.1
export PATH="$CARGO_HOME/bin:/usr/bin:/bin:/usr/sbin:/sbin"
```

`rustup toolchain list` reported only `1.98.1-aarch64-apple-darwin` as
installed and active. `rustup target list --installed --toolchain 1.98.1`
reported `aarch64-apple-darwin` and `wasm32-unknown-unknown`.
`rustup component list --installed --toolchain 1.98.1` reported
`cargo-aarch64-apple-darwin`, `clippy-aarch64-apple-darwin`,
`rust-std-aarch64-apple-darwin`, `rust-std-wasm32-unknown-unknown`,
`rustc-aarch64-apple-darwin`, and `rustfmt-aarch64-apple-darwin`.

Observed versions and SHA-256 hashes of the actual cached executable files:

| Executable | Version | SHA-256 |
| --- | --- | --- |
| `rustc` | `1.98.1 (48a229cea 2026-09-01)`, host `aarch64-apple-darwin`, LLVM `22.1.8` | `766eda9d8f53afd6fc7f27b3cd2e444dd22afacb5afa710a5625fc8e45b8c941` |
| `cargo` | `1.98.1 (797e8a9bc 2026-08-05)` | `6e17e865f3a20dd55a1d212f849f58b77124179f0de7c52973096d84ba34118d` |
| `rustfmt` | `1.9.0-stable (48a229ceae 2026-09-01)` | `98e8da71078a8b5710f1818c98da25d92de162a122bd9a6db222ca929e545468` |
| `clippy-driver` | `0.1.98 (48a229ceae 2026-09-01)` | `3e43a13a5b341ce6ceaf90addfd94bd7958c0e1988cf44d357fe07c6383ce3e9` |

Host observed with `uname -a`: Darwin `25.6.0`, ARM64. Compiler identity and
the installation paths were queried under the explicit cache environment above;
no global toolchain path or installation was used.

## Reproduction commands

The repository formatting gate uses `rustfmt.toml` (style edition 2024):

```sh
cargo +1.98.1 fmt --all -- --check
```

For an affected package, run Clippy separately for each supported target and
feature combination; keep warnings denied and do not combine mutually exclusive
features:

```sh
CARGO_BUILD_JOBS=1 cargo +1.98.1 clippy --locked --offline -p <crate> --all-targets --target aarch64-apple-darwin -- -D warnings
CARGO_BUILD_JOBS=1 cargo +1.98.1 clippy --locked --offline -p <crate> --all-targets --target wasm32-unknown-unknown -- -D warnings
```

The current source has no declared per-crate feature matrix. These commands are
templates for the affected crate and its approved features, not a claim that all
five current workspace crates passed. Native and WebAssembly compile checks use
the same target selection and offline, locked inputs:

```sh
CARGO_BUILD_JOBS=1 cargo +1.98.1 check --locked --offline --workspace --target aarch64-apple-darwin
CARGO_BUILD_JOBS=1 cargo +1.98.1 check --locked --offline --workspace --target wasm32-unknown-unknown
```

For the existing fixture wrapper, supply its owned roots and override its
two-job default before using the compile path:

```sh
DUNGEONFLUX_ARTIFACT_ROOT=/Users/earlcameron/Desktop/dungeonflux/artifacts \
DUNGEONFLUX_BUILD_ROOT=/Users/earlcameron/Desktop/dungeonflux/artifacts/build/toolchain-g01-d01 \
DUNGEONFLUX_TMP_ROOT=/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G01-D01-a1 \
CARGO_BUILD_JOBS=1 development/build-fixture.sh build
```

The wrapper currently builds `df-tools` native and WASM artifacts and generates
web glue with its pinned `wasm-bindgen 0.2.129`. It was not run for this task:
the bounded standard-library probe gives target evidence without building all
crate dependencies or regenerating fixture output. If run later, its replacement
and staging operations must remain confined to the supplied owned artifact roots.

## Bounded probe and results

Probe root: `/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G01-D01-a1/probe`  
Build root: `/Users/earlcameron/Desktop/dungeonflux/artifacts/build/toolchain-g01-d01`  
Source revision: `916298ecbb0552c32981cbafab94eb94eb6bbadb`

The probe exports `contract_value(u32) -> u32`, implemented with saturating
addition by seven; the native executable calls it with `35`. The probe has no
third-party dependencies. Its isolated manifest declares edition 2024 and an
empty workspace because it lives under the repository's Cargo workspace path.

```sh
cd /Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G01-D01-a1/probe
export CARGO_TARGET_DIR=/Users/earlcameron/Desktop/dungeonflux/artifacts/build/toolchain-g01-d01
export CARGO_BUILD_JOBS=1
cargo generate-lockfile --offline
cargo +1.98.1 fmt --all -- --check
cargo +1.98.1 clippy --offline --locked --all-targets --target aarch64-apple-darwin -- -D warnings
cargo +1.98.1 clippy --offline --locked --all-targets --target wasm32-unknown-unknown -- -D warnings
cargo +1.98.1 build --offline --locked --target aarch64-apple-darwin
cargo +1.98.1 build --offline --locked --lib --target wasm32-unknown-unknown
/Users/earlcameron/Desktop/dungeonflux/artifacts/build/toolchain-g01-d01/aarch64-apple-darwin/debug/toolchain_contract_probe
```

All commands exited `0`; the retained JSON run record reports per-command elapsed
time (each under 0.24 seconds), empty stderr, and runner child-process peak RSS
of 41,484,288 bytes. Clippy completed for each target, native and WASM builds
completed, and native execution printed `42`. The captured output and exact argv
are retained in `artifacts/tmp/B-G01-D01-a1/probe/verification.json` (SHA-256
`614d83457a3319ac490cf79dfd5752430473f3add26cda608ec080d427626f11`). `file` identified the WASM output
as `WebAssembly (wasm) binary module version 0x1 (MVP)`. A bounded section
reader found exports `memory` (kind 2) and `contract_value` (kind 0). The retained
inspector source is `artifacts/tmp/B-G01-D01-a1/probe/inspect_wasm.py`
(SHA-256 `6b691b498a224bcf991e11040e3599f001aa8ca917c1c82dcb2165e611db02f2`);
its output is `wasm-inspection.json` (SHA-256
`3405cd9e8845b18853a5f34dd535c0a797944e3bb4e5afbb9b403daff338ad90`). WASM was
compiled and inspected only; there was no browser/runtime execution.

Probe input hashes:

| File | SHA-256 |
| --- | --- |
| `Cargo.toml` | `693660365cdf528a2ca0dae2fb6c6b1881aa0294dc7159b8f447ff4870132822` |
| `Cargo.lock` | `8719c3c5285f7e3e493d795aab17278597a50d0cec97f5524b2b8c8479b17903` |
| `src/lib.rs` | `2cb966e3d77c52b775c9993c6509f7120da63f0ae4b51531e11a8dba33ef2e06` |
| `src/main.rs` | `0f3bf2c65e9e76b406e8305e77934fd51bbd3ff1462ad5ed5b2e8e3943c29cb1` |

Built artifact hashes:

| Artifact | SHA-256 |
| --- | --- |
| Native executable `aarch64-apple-darwin/debug/toolchain_contract_probe` | `f3cba9c834b8aa8cf732fe2f6cbba4efd6dfc8d8a04a9ec0c8b23342332caa5f` |
| WASM module `wasm32-unknown-unknown/debug/toolchain_contract_probe.wasm` | `62428491dc459448b2109b1ca961fbb0ceca86159cc680ffe9d039e710864e94` |

The repository's checked-in inputs observed at this source revision are:

| File | SHA-256 |
| --- | --- |
| `rust-toolchain.toml` | `9500030ccefd0bab631fb7f1763f79f4103eca3a344c36cf15be869e330683bb` |
| `Cargo.toml` | `fd965c9b0d70724e3f46fef34992549bb68ea45eec56165f41479bd47a29719c` |
| `Cargo.lock` | `9e16a6f4aa6e9808a1b7f75fbf5f3cc7dd0029fc0cfb82617fa7ec353dabaeb3` |
| `.cargo/config.toml` | `651f8afd5bda8ec35bf5aa3c58e0cbe5eaf4d4da9f9acadf236312f1a6260ed9` |
| `rustfmt.toml` | `7a142cbd3f5e9c09c6dc4a1850e053415530eb5450d27983dc26c75230b8c5ba` |
| `development/build-fixture.sh` | `ee2e24cebf0efe154670b10d58b6881e26d9a0df1643e16db9fb855dc940732f` |

The probe's first Cargo invocation found the parent workspace and exited `101`
before compilation. Adding an empty `[workspace]` to the isolated probe manifest
made Cargo treat it as an independent package; all later probe commands passed.
No repository manifest, lockfile, shared target directory, or cache was changed.

## Alternatives and open evidence

Floating stable would change compiler bits without a source revision and would
not reproduce the cached compiler hashes above. Nightly would add avoidable
compiler and feature churn; the code needs no nightly-only capability. The
checked-in stable exact pin is therefore retained.

This evidence does not qualify Linux or Windows builds, other Apple targets,
physical devices, browser execution, browser gRPC, audio, the application or all
workspace crates/features. No framework, commercial license/policy, full
dependency approval, browser/device matrix, or 41-crate completion is selected.
Those requirements remain with their roadmap owners and gates. No dependency
download, global toolchain installation, paid call, or source code change was
made.

## Original acceptance and verification

Acceptance criterion 1: “native and WASM targets”. Evidence: the cached installed
native and WASM targets, successful Clippy/build commands for both probe targets,
native output `42`, and inspected WASM MVP exports above. This is bounded target
proof, not an application or browser qualification.

Acceptance criterion 2: “The named outcome has actual source/build-bound
evidence; unsupported, pending, failed and unperformed checks remain explicit.”
Evidence identity is the source revision, declared/isolated config and input
hashes, compiler executable hashes, exact commands, native and WASM artifact
hashes, and recorded outputs above. Unperformed and unsupported checks are
listed explicitly.

Original verification 1: “Freeze the cited source decision and a bounded contract
example; compare native and WASM targets. Retain decision, alternatives and
unresolved facts.” Decision, probe, both targets, alternatives, and unresolved
facts are recorded above.

Original verification 2: “Exact executable commands: TBD at G01 and scoped
prerequisite resolution; this planned procedure is not a claim that Rust/browser/
provider checks ran.” The target/style command templates and probe commands are
now concrete. Only the probe commands are reported as run; no browser, provider,
or full-workspace command is claimed.

Devlog observation: `toolchain-g01-d01-a1-cargo-workspace-probe-20261001`
records the scratch Cargo workspace resolution and its bounded correction.
