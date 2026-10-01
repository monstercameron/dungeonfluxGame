# Reproducible command registry

Date: 2026-10-01  
Task/attempt: `B-G01-D04/a2`
Decision source revision: `c9a700b54ed90137517edb3be6c59e9674d0ecfc`
Scope: existing six-crate S00 transport fixture and bounded local checks; no application or runner implementation.

## Decision and limits

Use the repository's cached Rust `1.98.1` toolchain, the committed `Cargo.lock`, offline Cargo resolution, and one Cargo build job for reproducible local checks. Run `cargo +1.98.1 fmt --all -- --check` once for formatting; formatting is target-independent. Select native workspace gates for all six workspace crates. Select WASM library gates only for the five crates that have a WASM library surface (`df-types`, `df-protocol`, `df-observe`, `df-rpc-bridge`, and `df-tools`); `df-telemetry` is native-only and must not be passed to WASM commands. The fixture executable and preview server are native-only. The `df-tools` library is the fixture's generated WASM input.

For the recorded G01-D02 browser-pipeline invocation, retain the explicitly narrower two-crate WASM/native Clippy selections in that record. For the current integrated source gates, retain the exact narrower WASM Clippy selection that was actually run (`df-rpc-bridge`, `df-tools`, and `df-observe`). These are observed checks, not a retroactive claim that every command in the future registry ran. Future use of the complete five-library WASM matrix is a planned selection and requires its own execution record.

The checked-in `development/build-fixture.sh` is historical fixture context, not the selected bounded runner. It hardcodes `CARGO_BUILD_JOBS=2`, does not set `CARGO_NET_OFFLINE`, and its build mode stages and replaces `web` output. G01-D02 explicitly admits direct Cargo commands with `--locked --offline --jobs 1` instead of that wrapper for scoped checks. Do not edit or alter the wrapper.

Direct fixture builds must explicitly set the compile-time identity consumed by `df_tools::BUILD_ID`; never inherit an ambient `DF_FIXTURE_BUILD`. From the exact worktree being built, run this preflight and retain its output with both build records:

```sh
ROOT=$(git rev-parse --show-toplevel)
FIXTURE_SOURCE_COMMIT=$(git -C "$ROOT" rev-parse HEAD)
FIXTURE_SOURCE_STATUS=$(git -C "$ROOT" status --porcelain -- Cargo.toml Cargo.lock rust-toolchain.toml .cargo crates development/build-fixture.sh)
if test -n "$FIXTURE_SOURCE_STATUS"; then
  FIXTURE_BUILD_ID="$FIXTURE_SOURCE_COMMIT-dirty"
else
  FIXTURE_BUILD_ID="$FIXTURE_SOURCE_COMMIT"
fi
unset DF_FIXTURE_BUILD
export DF_FIXTURE_BUILD="$FIXTURE_BUILD_ID"
test "$DF_FIXTURE_BUILD" = "$FIXTURE_BUILD_ID"
printf '%s\n' "source_commit=$FIXTURE_SOURCE_COMMIT" "source_status_begin" "$FIXTURE_SOURCE_STATUS" "source_status_end" "fixture_build_id=$FIXTURE_BUILD_ID"
```

The status path set intentionally matches the checked-in wrapper, which includes tracked/untracked workspace source inputs, manifests, toolchain and build script. Record the exact HEAD, status output and resulting label in each build record. The assignment after `unset` prevents an inherited value from silently labeling another source; rerun the preflight if those source inputs change before a later build. This mirrors the wrapper's label rule and prevents the compile-time fallback `unregistered-cargo-build` for direct native and WASM fixture builds.

The direct bindgen command writes to the attempt-owned staging directory. Prepare that directory with `mkdir -p /Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G01-D04-a2/web-staging`; after bindgen succeeds, write `printf '%s\n' "$DF_FIXTURE_BUILD" > /Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G01-D04-a2/web-staging/source-revision.txt`. Verify the file contents with `test "$(cat /Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G01-D04-a2/web-staging/source-revision.txt)" = "$DF_FIXTURE_BUILD"`, hash it with `shasum -a 256 /Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G01-D04-a2/web-staging/source-revision.txt`, and include that hash beside every generated web artifact in the staged output identity. The source label identifies the exact commit and whether wrapper-relevant source inputs were dirty; it does not certify that a full application build or browser run occurred. `wasm-bindgen` rewrites the raw compiler module; retain and identify both binaries separately.

This registry does not satisfy the separate dependency-license admission policy. In particular, the vendored `h2` provenance/patch record remains unresolved. It does not close G02 device, supported-browser, suspension, adversarial resource, CPU or memory qualification; choose a UI/framework; certify production/release hosting or artifact publication; or prove production readiness.

## Pinned cached environment

Use the repository's existing cache; do not install, download, update, or contact a provider. These absolute paths are host-specific:

```sh
export RUSTUP_HOME=/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/rustup
export CARGO_HOME=/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/cargo
export CARGO_TARGET_DIR=/Users/earlcameron/Desktop/dungeonflux/artifacts/build/command-registry-g01-d04-a2
export TMPDIR=/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G01-D04-a2/temp
export RUSTUP_AUTO_INSTALL=0
export RUSTUP_TOOLCHAIN=1.98.1
export CARGO_NET_OFFLINE=true
export CARGO_BUILD_JOBS=1
export PATH="$CARGO_HOME/bin:/usr/bin:/bin:/usr/sbin:/sbin"
```

At the decision source, `rust-toolchain.toml` pins stable `1.98.1`, minimal profile, with `rustfmt` and Clippy, and declares `wasm32-unknown-unknown`; `rustfmt.toml` selects edition 2024. Cargo is locked to the tracked `Cargo.lock`. The cached executable/version/hash observations are retained by the toolchain decision in `planning/toolchain-targets.md` and `development/evidence/toolchain-g01-d01/a2/worker/compiler-provenance.json`. The browser generator observed by G01-D02 is `/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/cargo/bin/wasm-bindgen`, version `0.2.129`, SHA-256 `83323ac851f9cd3d6c9143bf2f913f19731b72b6355ae363c146cf3e69b8842d`; the lockfile resolves the Rust wasm-bindgen dependency to `0.2.129`.

The exact environment used for the historical G01-D02 run additionally used `CARGO_TARGET_DIR=/Users/earlcameron/Desktop/dungeonflux/artifacts/build/browser-pipeline-g01-d02` and `TMPDIR=/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G01-D02-a2/temp`; preserve those values when interpreting that run's evidence. Do not treat this example registry environment as evidence that any registry command executed.

## Command selection

Commands below are argv as executed from the repository/worktree root after setting the environment above. `--jobs 1` is explicit on Cargo operations that can build. Evidence for a run must bind at least the source commit, clean/dirty source state, exact argv, tool identity, environment/cache identity, exit status, elapsed time, stdout/stderr, and owned output hashes where the command produces artifacts. A failed, unavailable, unsupported, or unperformed command remains explicit; do not infer success from a related command.

| ID | Scope / target applicability | Exact argv | Owned output / evidence |
| --- | --- | --- | --- |
| `fmt` | All Rust source; target-independent and selectable for either Native or WASM request | `cargo +1.98.1 fmt --all -- --check` | stdout/stderr, exit, elapsed, source/config hashes |
| `native-clippy` | All workspace crates, native host `aarch64-apple-darwin` | `cargo +1.98.1 clippy --locked --offline --workspace --all-targets --target aarch64-apple-darwin --jobs 1 -- -D warnings` | Cargo log and command record |
| `native-test` | All six crates, native host only | `cargo +1.98.1 test --locked --offline --workspace --target aarch64-apple-darwin --jobs 1` | Test log, test counts, command record |
| `native-fixture-build` | Runnable fixture binary in `df-tools`, native host only; run with the preflighted `DF_FIXTURE_BUILD` environment | `cargo +1.98.1 build --locked --offline -p df-tools --bin df-transport-fixture --target aarch64-apple-darwin --jobs 1` | Binary hash, source/build identity and recorded `DF_FIXTURE_BUILD` value |
| `wasm-clippy` | WASM libraries only: `df-types`, `df-protocol`, `df-observe`, `df-rpc-bridge`, `df-tools`; exclude native-only `df-telemetry` | `cargo +1.98.1 clippy --locked --offline -p df-types -p df-protocol -p df-observe -p df-rpc-bridge -p df-tools --lib --target wasm32-unknown-unknown --jobs 1 -- -D warnings` | Cargo log and command record |
| `wasm-fixture-build` | `df-tools` library only, `wasm32-unknown-unknown` | `cargo +1.98.1 build --locked --offline -p df-tools --lib --target wasm32-unknown-unknown --jobs 1` | Raw `df_tools.wasm` hash, bytes, source/build identity and common `DF_FIXTURE_BUILD` |
| `wasm-bindgen` | Bindgen web output for exactly the raw fixture module above | `/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/cargo/bin/wasm-bindgen --target web --out-dir /Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G01-D04-a2/web-staging /Users/earlcameron/Desktop/dungeonflux/artifacts/build/command-registry-g01-d04-a2/wasm32-unknown-unknown/debug/df_tools.wasm` | Generator executable/version/hash; argv/exit; hashes for JS, declarations, bound `df_tools_bg.wasm`, and the staged `source-revision.txt`; explicitly record raw-vs-bound module identity and common `DF_FIXTURE_BUILD` |

These commands are the selected future registry. The native workspace `check` and `build` commands are omitted as separate gates because the native Clippy all-targets and fixture build compile the affected targets; add a scoped `check` only when a consumer needs a check-only boundary. The WASM library build is intentionally the existing fixture's `df-tools` library build; it does not claim all five libraries were built in the accepted integrated review. No cross-target WASM test execution is selected: tests are exercised by the native workspace test gate, while WASM gets library lint/build gates. Do not run the browser generator on an arbitrary or stale WASM path.

The exact historical S00 wrapper argv (from the project root) were:

```sh
DUNGEONFLUX_ARTIFACT_ROOT=/Users/earlcameron/Desktop/dungeonflux/artifacts ./development/build-fixture.sh cargo fmt --all -- --check
DUNGEONFLUX_ARTIFACT_ROOT=/Users/earlcameron/Desktop/dungeonflux/artifacts ./development/build-fixture.sh cargo clippy --locked --workspace --all-targets -- -D warnings
DUNGEONFLUX_ARTIFACT_ROOT=/Users/earlcameron/Desktop/dungeonflux/artifacts ./development/build-fixture.sh cargo test --locked --workspace
DUNGEONFLUX_ARTIFACT_ROOT=/Users/earlcameron/Desktop/dungeonflux/artifacts ./development/build-fixture.sh cargo clippy --locked -p df-rpc-bridge --target wasm32-unknown-unknown -- -D warnings
DUNGEONFLUX_ARTIFACT_ROOT=/Users/earlcameron/Desktop/dungeonflux/artifacts ./development/build-fixture.sh cargo clippy --locked -p df-tools --lib --target wasm32-unknown-unknown -- -D warnings
DUNGEONFLUX_ARTIFACT_ROOT=/Users/earlcameron/Desktop/dungeonflux/artifacts ./development/build-fixture.sh cargo build --locked -p df-rpc-bridge --target wasm32-unknown-unknown
DUNGEONFLUX_ARTIFACT_ROOT=/Users/earlcameron/Desktop/dungeonflux/artifacts ./development/build-fixture.sh build
```

S00's build-mode script source specifies the following child argv, then checks the generator version before generation: `cargo build --locked -p df-tools --bin df-transport-fixture`, `cargo build --locked -p df-tools --lib --target wasm32-unknown-unknown`, `wasm-bindgen --version`, and `wasm-bindgen --target web --out-dir "$STAGING" "$CARGO_TARGET_DIR/wasm32-unknown-unknown/debug/df_tools.wasm"`. The final two child arguments depend on the wrapper's `STAGING` and build-root environment and are not a frozen absolute invocation. G01-D02's actual generator argv is retained separately in its `pipeline-evidence.json`: `/Users/earlcameron/Desktop/dungeonflux/artifacts/cache/cargo/bin/wasm-bindgen --target web --out-dir /Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G01-D02-a2/web-staging /Users/earlcameron/Desktop/dungeonflux/artifacts/build/browser-pipeline-g01-d02/wasm32-unknown-unknown/debug/df_tools.wasm`. These prior commands identify their own source/build records. They are not the bounded command selection in this document: wrapper passthrough omits offline and keeps the wrapper's two-job environment; wrapper build also stages and publishes generated output.

## Bounded executable contract example

This standard-library-only example makes the selected target applicability and evidence binding explicit. The `Any` target lets formatting resolve for either target; native-only and WASM-only commands still refuse the opposite target. It checks the exact selected native-test argv and refuses evidence without source, exact argv, successful exit, or required output identity. `DEMO_SOURCE_NOT_EVIDENCE` and `DEMO_HASH_NOT_EVIDENCE` are deliberate sentinels used only to demonstrate field-presence checks; neither is a real source or artifact identity. The example is a registry contract illustration, not a Cargo runner, a production coordinator, or proof that a real full build/browser run occurred.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Any,
    Native,
    Wasm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Command {
    id: &'static str,
    target: Target,
    argv: &'static [&'static str],
    requires_output_hash: bool,
}

const COMMANDS: &[Command] = &[
    Command {
        id: "fmt",
        target: Target::Any,
        argv: &["cargo", "+1.98.1", "fmt", "--all", "--", "--check"],
        requires_output_hash: false,
    },
    Command {
        id: "native-test",
        target: Target::Native,
        argv: &[
            "cargo",
            "+1.98.1",
            "test",
            "--locked",
            "--offline",
            "--workspace",
            "--target",
            "aarch64-apple-darwin",
            "--jobs",
            "1",
        ],
        requires_output_hash: false,
    },
    Command {
        id: "wasm-fixture-build",
        target: Target::Wasm,
        argv: &[
            "cargo",
            "+1.98.1",
            "build",
            "--locked",
            "--offline",
            "-p",
            "df-tools",
            "--lib",
            "--target",
            "wasm32-unknown-unknown",
            "--jobs",
            "1",
        ],
        requires_output_hash: true,
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Evidence {
    source_commit: Option<&'static str>,
    argv_recorded: bool,
    exit_code: Option<i32>,
    output_sha256: Option<&'static str>,
}

fn select(id: &str, target: Target) -> Option<&'static Command> {
    COMMANDS.iter().find(|command| {
        command.id == id && (command.target == Target::Any || command.target == target)
    })
}

fn accept(command: &Command, evidence: Evidence) -> bool {
    evidence.source_commit.is_some()
        && evidence.argv_recorded
        && evidence.exit_code == Some(0)
        && (!command.requires_output_hash || evidence.output_sha256.is_some())
}

fn main() {
    let fmt_native = select("fmt", Target::Native).expect("fmt applies to native");
    let fmt_wasm = select("fmt", Target::Wasm).expect("fmt applies to WASM");
    assert_eq!(fmt_native.argv, fmt_wasm.argv);
    assert_eq!(
        fmt_native.argv,
        &["cargo", "+1.98.1", "fmt", "--all", "--", "--check"]
    );

    let native_test = select("native-test", Target::Native).expect("registered native test");
    assert_eq!(
        native_test.argv,
        &[
            "cargo",
            "+1.98.1",
            "test",
            "--locked",
            "--offline",
            "--workspace",
            "--target",
            "aarch64-apple-darwin",
            "--jobs",
            "1",
        ]
    );
    assert!(select("native-test", Target::Wasm).is_none());
    assert!(select("wasm-fixture-build", Target::Native).is_none());
    assert!(select("unknown", Target::Native).is_none());

    let wasm = select("wasm-fixture-build", Target::Wasm).expect("registered WASM build");
    let complete = Evidence {
        source_commit: Some("DEMO_SOURCE_NOT_EVIDENCE"),
        argv_recorded: true,
        exit_code: Some(0),
        output_sha256: Some("DEMO_HASH_NOT_EVIDENCE"),
    };
    assert!(accept(wasm, complete));
    assert!(!accept(
        wasm,
        Evidence {
            source_commit: None,
            ..complete
        }
    ));
    assert!(!accept(
        wasm,
        Evidence {
            argv_recorded: false,
            ..complete
        }
    ));
    assert!(!accept(
        wasm,
        Evidence {
            output_sha256: None,
            ..complete
        }
    ));
    assert!(!accept(
        wasm,
        Evidence {
            exit_code: Some(1),
            ..complete
        }
    ));
}
```

For the bounded validation, extract the fenced Rust source from this document to attempt-owned scratch, run cached `rustfmt +1.98.1 --check` (or format the extracted copy with that executable), compile using cached `rustc +1.98.1 --edition=2024`, and run the produced executable. Record the actual commands, executable paths/versions, source hash, exit codes and elapsed times. This example validation does not run Cargo or any application checks.

## Evidence at the decision source and prior results

Current accepted integrated source is `003578c71c0816407d5e5f4481d44b66a6801e62`, independently built/reviewed in `development/evidence/control-rate-repair/a2/integration-reviewer/`. The reviewer retained native binary/WASM/generated-web hashes, the exact source worktree and commit, plus the staged `source-revision.txt` hash; the file itself contains `003578c71c0816407d5e5f4481d44b66a6801e62`. This externally recorded build/source identity does not prove that the compile-time `DF_FIXTURE_BUILD` variable was injected into the native executable. Do not backfill the new preflight as a historical command or treat its staged label as a code-level binary hash. `independent-gates.json` binds its outputs to source hashes in `independent-source-identity.json`. At that source, the reviewer observed successful `fmt`, native workspace Clippy, native workspace tests, the selected three-package WASM Clippy, a `df-tools` WASM library build, and native `df-tools` fixture build. Each has exit code zero and its own log; those are accepted prior observations, not executions performed for this documentation decision. The integrated build identity separately records the native fixture binary, raw WASM, and generated web files/hashes. The integration reviewer observed a real-browser fixture run for control-rate repair at this source; that evidence belongs to that work and does not add a new browser qualification claim here.

The broader selected five-package WASM Clippy command above and the selected explicit native target variants have **not** been executed as this registry. No G01-D04 application command set was executed. The bounded Rust contract example uses sentinel data only and does not validate source hashes for an application build or browser output. The registry's exact output paths are reserved, not evidence of generated files.

G01-D02/a2 ran a distinct six-command Cargo set and wasm-bindgen on candidate `8e274abc8ef4b50d8eb7658e9c87d3d83050d644` (document input `01c2ccf09545877d65ffe490975b4d9da267cd67`). Its exact argv, environment, exits, elapsed times, raw module hash, generator executable/version/hash, generated outputs, and separate raw/bound WASM hashes are recorded in `development/evidence/browser-pipeline-g01-d02/a2/worker/verification.json`, `pipeline-evidence.json`, and `wasm-module-inventory.json`. That run passed its scoped G01-D02 gates. It was not run at this document's source revision and is not a new browser test.

The S00 wrapper's actual fixture checks and browser observation are retained in `development/evidence/start-s00/worker/handoff.json` and the sibling logs. They were performed against S00's recorded build, not rerun here. Keep exact source and output identities attached when comparing them; never combine these distinct attempts into one passing run.

## Alternatives and open facts

The wrapper is convenient for the original fixture lifecycle but does not supply this bounded offline one-job policy and performs publication side effects in build mode. Direct Cargo argv with explicit lock/offline/jobs bounds is selected for checks and build steps; bindgen receives one fixed raw module and an attempt-owned staging path. Floating stable, global cache/toolchain resolution, network-enabled Cargo, the two-job wrapper mode, whole-workspace WASM commands including telemetry, and hand-authored glue are not selected.

Open: complete license/source provenance admission (including vendored `h2`), a complete independently reviewed native/WASM profile and feature closure, G02 device/browser/resource/CPU qualification, browser RPC runtime feasibility beyond compilation, UI/framework, production hosting/MIME/cache/release publication and rollback, and performance budgets. None is solved by command reproducibility or this bounded contract example.

## Original criterion mapping

The a1 rejection is preserved at `development/evidence/command-registry-g01-d04/a1/reviewer/review.json`; its three reproduced defects were the target-independent `fmt` selector, a mismatched native-test example argv, and the missing direct-build source-label/staged-label lifecycle. This a2 repair changes the same one document and preserves a1 artifacts and verdict.

The two acceptance strings and two verification strings below are copied verbatim from the frozen brief and remain immutable.

| Original criterion | Result / retained evidence |
| --- | --- |
| “exact commands recorded at resolution” | Decision recorded above; selected future commands are distinguished from historical executions and their exact-source evidence. |
| “The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.” | Accepted integrated003 evidence is cited with actual source/output identities. Earlier fixture fallback behavior and external source labels are not promoted to proof of `DF_FIXTURE_BUILD` injection. Broader selected checks, G01-D04 application builds, complete G02 and production work remain explicitly unperformed/open. |
| “Freeze the cited source decision and a bounded contract example; compare exact commands recorded at resolution. Retain decision, alternatives and unresolved facts.” | The example selects `fmt` for both targets, checks the table's exact native-test argv and refuses unsupported target/evidence cases. Its sentinel values are explicitly illustrative, not real artifact evidence. |
| “Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.” | Exact executable argv is selected here. Only the standalone example and refusal checks run for a2; no application Cargo, browser, or provider check is claimed. Prior source-bound evidence is reported separately. |

## Governing references

This decision follows `planning/coding-style.md` (documentation/source boundaries), `planning/implementation-roadmap.md` (G01, S00/S08 and ownership), `planning/subsystem-interfaces.md` (shared transport target applicability), `planning/toolchain-targets.md` (Rust pin and target map), `planning/browser-build-pipeline.md` (raw/bound WASM distinction and G01-D02 one-job/offline exception), `planning/dependency-license-policy.md` (unresolved rights are separate), `development/build-fixture.sh`, `development/README.md`, root Cargo/toolchain/format configuration and the source-hashed backlog catalog. ADRs 0001–0005 govern one-owner submission, resource bounds, append-only devlog, precise handoff and independent frontier evaluation. The complete source set and immutable original criteria are pinned by `development/evidence/command-registry-g01-d04/a2/brief.json`; this repair also preserves the a1 rejection and its failure lineage.
