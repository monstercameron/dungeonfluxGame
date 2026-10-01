# Pinned Rust toolchain and initial targets

Date: 2026-10-01 00:27 UTC  
Task/attempt: `B-G01-D01/a2`  
Current source revision: `421f7834681bd84f6e3c22e3a30335f763639827`  
Original decision source revision: `916298ecbb0552c32981cbafab94eb94eb6bbadb`

## Decision

Use the repository's existing stable Rust `1.98.1` pin for native and
`wasm32-unknown-unknown` builds. Keep Rust edition `2024`, workspace resolver
`3`, and the checked-in `Cargo.lock`. The pinned minimal profile includes
`rustfmt` and Clippy. Native development is qualified on the observed host
`aarch64-apple-darwin`; the initial browser compilation target is
`wasm32-unknown-unknown`. The cached installation was enumerated before compiling.

At the original `916298e` input, the Cargo workspace contained five crates.
Current source `421f783` adds the native-only `df-telemetry` crate. Target
commands therefore select packages according to target support; a whole-workspace
WASM command does not apply to current source.

A small, dependency-free standard-library contract probe compiled with the pinned
compiler for both targets. Native execution returned `42` for input `35` and
reported saturating behavior at `u32::MAX`; the WASM module was inspected and
exports its contract functions. This proves bounded target compilation and
native execution only. It does not execute WASM in a browser or qualify the
application, RPC runtime, framework, UI, generated bindings, or production crate
feature sets.

No Rust application or UI framework is selected here. Roadmap G01/G04/G08 and
`planning/client-presentation.md` leave component implementation, device
support, UI frameworks, codecs, and presentation budgets open. The Rust-only
application rule and permission for generated `wasm-bindgen` JavaScript glue
remain governed by `AGENTS.md`; the build fixture expects `wasm-bindgen 0.2.129`,
but no glue was generated. Browser gRPC, executor and stream feasibility remain
G02 work under `planning/rpc-transport.md`.

## Cache and observed compiler provenance

Tool invocations use the repository-owned cache, with automatic installation
disabled and Cargo network access disabled:

```sh
export RUSTUP_HOME=/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/rustup
export CARGO_HOME=/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/cargo
export RUSTUP_AUTO_INSTALL=0
export RUSTUP_TOOLCHAIN=1.98.1
export CARGO_NET_OFFLINE=true
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
installation paths were queried under the explicit cache environment above; no
global toolchain path or installation was used. Captured command output and
binary hashes are retained in `development/evidence/toolchain-g01-d01/a2/worker/compiler-provenance.json`
(SHA-256 `a8a02e5bcaa1b9138aca29174cdb3973d7177d1158bc9a5d8578a39eb5467a0a`).

## Reproduction commands

The repository formatting gate uses the checked-in `rustfmt.toml` (style edition
2024):

```sh
cargo +1.98.1 fmt --all -- --check
```

For native affected crates, use the default Clippy lint set with warnings denied.
For WASM library crates, select library targets and do not combine mutually
exclusive features:

```sh
CARGO_BUILD_JOBS=1 cargo +1.98.1 clippy --locked --offline -p <crate> --all-targets --target aarch64-apple-darwin -- -D warnings
CARGO_BUILD_JOBS=1 cargo +1.98.1 clippy --locked --offline -p <wasm-library-crate> --lib --target wasm32-unknown-unknown -- -D warnings
```

The current six-crate workspace has no declared per-crate feature matrix. The
following commands document applicable gates; they were not run against the full
workspace:

```sh
CARGO_BUILD_JOBS=1 cargo +1.98.1 check --locked --offline --workspace --target aarch64-apple-darwin
CARGO_BUILD_JOBS=1 cargo +1.98.1 clippy --locked --offline --workspace --all-targets --target aarch64-apple-darwin -- -D warnings
CARGO_BUILD_JOBS=1 cargo +1.98.1 check --locked --offline --workspace --exclude df-telemetry --lib --target wasm32-unknown-unknown
CARGO_BUILD_JOBS=1 cargo +1.98.1 clippy --locked --offline --workspace --exclude df-telemetry --lib --target wasm32-unknown-unknown -- -D warnings
```

The current WASM library selection is `df-types`, `df-protocol`, `df-rpc-bridge`,
`df-observe`, and `df-tools`. `df-telemetry` is excluded: current source has a
`#[cfg(target_arch = "wasm32")] compile_error!` declaring that the synthetic
executable is native-only and has no browser storage. These are target/package
commands, not claims that current production crates passed them.

`development/build-fixture.sh` was not run. It unconditionally exports
`CARGO_BUILD_JOBS=2`; its `build` branch therefore cannot meet the one-job bound
as written and has no built-in Cargo offline flag. The separate `cargo` passthrough
can take explicit `--offline --jobs 1` arguments if used later; it was not used
here. Its native `df-tools` binary and WASM `df-tools` library steps are source
facts; no build-branch wrapper execution, build output replacement, or generated
glue run is claimed. The script expects `wasm-bindgen 0.2.129` for its glue
stage.

## Bounded probe and results

Probe root: `/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G01-D01-a2/probe`  
Build root: `/Users/earlcameron/Desktop/dungeonflux/artifacts/build/toolchain-g01-d01-a2`  
Source revision: `421f7834681bd84f6e3c22e3a30335f763639827`

The probe exports `contract_value(u32) -> u32` and
`contract_saturation(u32) -> u32`, each implemented with saturating addition by
seven; the native executable calls them with `35` and `u32::MAX`. The probe has
no third-party dependencies. Its isolated manifest declares edition 2024 and an
empty workspace so Cargo does not inherit the parent project workspace.

```sh
cd /Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G01-D01-a2/probe
export RUSTUP_HOME=/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/rustup
export CARGO_HOME=/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/cargo
export CARGO_TARGET_DIR=/Users/earlcameron/Desktop/dungeonflux/artifacts/build/toolchain-g01-d01-a2
export TMPDIR=/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G01-D01-a2/temp
export CARGO_NET_OFFLINE=true
export RUSTUP_AUTO_INSTALL=0
export RUSTUP_TOOLCHAIN=1.98.1
export PATH="$CARGO_HOME/bin:/usr/bin:/bin:/usr/sbin:/sbin"
export CARGO_BUILD_JOBS=1
cargo +1.98.1 generate-lockfile --offline
cargo +1.98.1 fmt --all -- --check --config-path /Users/earlcameron/Desktop/dungeonflux/artifacts/worktrees/toolchain-g01-d01-a2/rustfmt.toml
cargo +1.98.1 clippy --locked --offline --all-targets --target aarch64-apple-darwin -- -D warnings
cargo +1.98.1 clippy --locked --offline --all-targets --target wasm32-unknown-unknown -- -D warnings
cargo +1.98.1 build --locked --offline --target aarch64-apple-darwin
cargo +1.98.1 build --locked --offline --lib --target wasm32-unknown-unknown
/Users/earlcameron/Desktop/dungeonflux/artifacts/build/toolchain-g01-d01-a2/aarch64-apple-darwin/debug/toolchain_contract_probe_a2
```

All seven commands exited `0`; each child had a 60-second timeout and Cargo ran
with one build job and offline mode. The retained JSON records exact argv,
stdout, stderr, and elapsed time. Cargo emitted normal `Checking`, `Compiling`,
and `Finished` progress on stderr for Clippy/build, with no warning or error
diagnostics. Native execution printed `native contract: 35 -> 42; saturation ->
4294967295`. The runner recorded `RUSAGE_CHILDREN.ru_maxrss` of 98,484,224 bytes
(maximum child RSS, not an aggregate). `file` identified
the WASM output as an MVP module; the retained section reader found no imports
and exports `memory` (kind 2), `contract_saturation` and `contract_value`
(kind 0). WASM was compiled and inspected only; there was no browser/runtime
execution.

Exact command output and argv are retained in
`artifacts/tmp/B-G01-D01-a2/probe/verification.json` (SHA-256
`9d633348b183a4fe7f12883feae228902cda1418e0c4657d7f8f3ebaf6abe8df`). Inspector
source `inspect_wasm.py` has SHA-256
`f40a85cdcbc924b0e760ff78ae4158a815af4e898c257d3d35587b4371ef5bbe`; its output
`wasm-inspection.json` has SHA-256
`ff33e4e192d5c2238b6a92db22d3fad584f8b0817ede40c3e37e2a6338a5052d`.

Probe input hashes:

| File | SHA-256 |
| --- | --- |
| `Cargo.toml` | `3c8fa56efe390f165b7d9a6b443ef7aaab8e10aa1ec90791d3c53b7d548176ed` |
| `Cargo.lock` | `e4b99f0d24aba5fdf71816497b6105ad871da8bf0427494a27d0c4631b28a402` |
| `src/lib.rs` | `7218dfdfabf559dc8c8363fdada860a4699a18373937b68a9d933e72cb650e50` |
| `src/main.rs` | `a6d7f375aee695af717d5dd3d2b7849f35e06a95924b2738b0b911f9c433cb0f` |

Built artifact hashes:

| Artifact | SHA-256 |
| --- | --- |
| Native executable `aarch64-apple-darwin/debug/toolchain_contract_probe_a2` | `5ee3dab393f77f1a04e0d911ff6ef5d2c1bcda6fc1d1955d75f2c47da00006f5` |
| WASM module `wasm32-unknown-unknown/debug/toolchain_contract_probe_a2.wasm` | `6fbbb3fe39ff1c4aeb70d1b8a5d2df8ba279f6d2e18c11073be64c8016ca826c` |

Historical repository inputs at `916298e` (five workspace members) identify the
original decision basis:

| File | SHA-256 at `916298e` |
| --- | --- |
| `rust-toolchain.toml` | `9500030ccefd0bab631fb7f1763f79f4103eca3a344c36cf15be869e330683bb` |
| `Cargo.toml` | `fd965c9b0d70724e3f46fef34992549bb68ea45eec56165f41479bd47a29719c` |
| `Cargo.lock` | `9e16a6f4aa6e9808a1b7f75fbf5f3cc7dd0029fc0cfb82617fa7ec353dabaeb3` |
| `.cargo/config.toml` | `651f8afd5bda8ec35bf5aa3c58e0cbe5eaf4d4da9f9acadf236312f1a6260ed9` |
| `rustfmt.toml` | `7a142cbd3f5e9c09c6dc4a1850e053415530eb5450d27983dc26c75230b8c5ba` |
| `development/build-fixture.sh` | `ee2e24cebf0efe154670b10d58b6881e26d9a0df1643e16db9fb855dc940732f` |

Current source inputs at `421f783` match the same toolchain, formatter, Cargo
configuration, and wrapper except the workspace manifest and lockfile updates:

| File | SHA-256 at `421f783` |
| --- | --- |
| `rust-toolchain.toml` | `9500030ccefd0bab631fb7f1763f79f4103eca3a344c36cf15be869e330683bb` |
| `Cargo.toml` | `a8f2d7317d401a25eb5ef4315dd0b42184f8f947d4cf82b055fd64c4656a5f05` |
| `Cargo.lock` | `c29d0c7cd8d60e7d63aa16803713de3bd8145ed21e614b50121c0d0f55fdd947` |
| `.cargo/config.toml` | `651f8afd5bda8ec35bf5aa3c58e0cbe5eaf4d4da9f9acadf236312f1a6260ed9` |
| `rustfmt.toml` | `7a142cbd3f5e9c09c6dc4a1850e053415530eb5450d27983dc26c75230b8c5ba` |
| `development/build-fixture.sh` | `ee2e24cebf0efe154670b10d58b6881e26d9a0df1643e16db9fb855dc940732f` |

`crates/df-telemetry/Cargo.toml` at `421f783` is native-only and
`src/main.rs` includes the WASM `compile_error!` guard (source SHA-256
`9507ed5ab89c51cff884cbd0ac77cb2370a061105dbf615aa814786009612154`; its
manifest SHA-256 is `4b7ece3fcdfe0824fb0a3a88c6451768f8d0754600cb18bf16725b06c6f55972`). The
full workspace WASM command therefore does not apply to current source.
Independent review observed the guard with direct `rustc`; no current full-
workspace WASM build was run.

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
native outputs `42` and `u32::MAX`, and inspected WASM exports above. This is
bounded target proof, not an application or browser qualification.

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
now concrete. Only the bounded probe commands are reported as run; no browser,
provider, or full-workspace command is claimed.

The previous attempt's retained verification has normal Cargo progress on stderr
for its four Clippy/build commands (no warnings/errors); exact JSON remains
preserved at `artifacts/tmp/B-G01-D01-a1/probe/verification.json` (SHA-256
`614d83457a3319ac490cf79dfd5752430473f3add26cda608ec080d427626f11`). Devlog
observation `toolchain-g01-d01-a1-cargo-workspace-probe-20261001` records the
scratch Cargo workspace resolution and its bounded correction. This attempt's
probe started with an empty `[workspace]` and preserves its actual Cargo progress
output in its run record. This repair is recorded as
`toolchain-g01-d01-a2-wrapper-and-target-scope-correction-20261001`.
