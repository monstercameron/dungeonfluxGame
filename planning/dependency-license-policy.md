# Dependency license admission policy

Date: 2026-10-01  
Task/attempt: `B-G01-D03/a1`  
Input revision: `502f2940e32aa8b1fc71a182c4859189152cb3d2`

## Decision

Keep the project declaration `MIT` as-is. Admit a third-party dependency only after recording its exact package/version, source identity, declared SPDX expression or license-file evidence, selected license branch (where there is a choice), applicable text/notice/source obligations, and the evidence location/hash in the release inventory. Each supported target/build profile must point to that same pinned inventory. A lockfile or manifest is an inventory input, not proof of legal approval.

A route may be marked approved only for an explicitly selected and reviewed permissive license branch whose required notices, attribution, license text, source offer or corresponding source distribution, and other stated conditions have an owner and delivery location. This policy does not interpret a license for a particular distribution as legal advice. A copyleft or network-copyleft branch, custom/LicenseRef or exception not separately reviewed, ambiguous/non-SPDX declaration, missing declaration and missing license text, or unavailable provenance stays **HOLD** until a separate recorded rights decision approves that exact package, use, distribution and obligations. A package on hold blocks the distribution that contains it; it is not silently omitted from the inventory.

Parse valid SPDX expressions with a conforming parser and the pinned SPDX license/exception list. Never identify licenses by substring. `OR` means alternatives: select and record one branch, then satisfy that branch's terms; it does not automatically approve the expression. `AND` means cumulative requirements: every branch must be approved and every obligation completed. Parentheses and `WITH` exceptions retain their SPDX meaning and require the relevant exception text/evidence. Unknown syntax, parser errors, and SPDX identifiers outside the reviewed allowlist are HOLD. A human must review exact license texts and repository notices; metadata alone cannot establish that the declared terms match the shipped files.

The allowlist is a decision input controlled by the coordinator/release rights owner, not inferred from the word “permissive.” The starting route candidates are MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC and Zlib, each subject to exact-text and obligation review. Unlicense, BSL, Unicode-3.0, LLVM-exception combinations, and any other unfamiliar identifier are unresolved until individually added by a documented decision. SPDX validity is not policy approval.

This is dependency-source licensing only. Game rules/books/catalog rights and source citations, user imports, generated text/images/audio, provider terms, and commercial/service rights require separate asset/content/provider rights records. Dependency approval does not grant those rights or constitute full G01 completion, commercial clearance, or legal advice. The roadmap separately makes complete rights evidence a release gate.

## Current locked inventory observation

At input revision `502f294`, root `Cargo.toml` declares edition 2024, resolver 3, project `MIT`, six workspace members and a `[patch.crates-io]` redirect of `h2` to `crates/df-rpc-bridge/vendor/h2`. `Cargo.lock` pins `h2 0.4.19` as a path package without a registry source/checksum; the vendored normalized manifest declares `MIT`, repository `https://github.com/hyperium/h2`, and version `0.4.19`. The vendored `LICENSE` contains the 2017 h2 authors' MIT notice. This is a license-text observation, not source lineage proof. The original upstream revision, vendoring date, local patch diff, retained upstream notices, and build/distribution packaging obligations were not established here. Until those are recorded and the patch reviewed against upstream, `h2` provenance is HOLD. Preserve its LICENSE in distributed source/artifacts and include its attribution with the third-party notices when it is admitted.

The root checkout has no top-level `LICENSE` file. The Cargo package declarations inherit workspace `MIT`; this discrepancy requires a project-level follow-up to decide whether a root license file should be added. No file was added in this decision-only task.

The bounded locked/offline Cargo 1.98.1 metadata observations resolved 173 package records in the unfiltered graph (including six workspace packages and target/optional/dev dependency records), 153 for `aarch64-apple-darwin`, and 143 for `wasm32-unknown-unknown`. The platform-filtered commands used Cargo metadata defaults; they are a snapshot of the current lockfile and currently selected features, not an exhaustive feature-matrix build. All three report no package with an empty/missing `license` and no `license_file`; this does not mean all records are approved. Observed declarations include ordinary MIT/Apache alternatives, cumulative expressions such as `MIT AND BSD-3-Clause` and `(MIT OR Apache-2.0) AND Unicode-3.0`, two `MIT OR Apache-2.0 OR LGPL-2.1-or-later` expressions, five `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT`, `Unlicense OR MIT`, `Zlib`, BSL, and legacy slash forms such as `MIT/Apache-2.0` and `Apache-2.0 / MIT`. For the two LGPL alternatives, the permissive MIT or Apache branch can be selected only if the inventory records the exact selected branch and its notices; the LGPL branch remains unselected/HOLD. Slash forms are not silently normalized. The Unicode cumulative term, exception branch, BSL and Unlicense need individual review. Thus the current inventory has unresolved records and no release profile is certified by this metadata run.

Cargo metadata with `--filter-platform aarch64-apple-darwin` and `--filter-platform wasm32-unknown-unknown` records separate target-filtered current package graphs, but it does not enumerate license texts, inspect source-file headers, confirm actual vendored diffs, or prove shipped notices/source. It also does not cover unselected features or establish that every resolved package is compiled into a final artifact. Native and WASM closures must be refreshed for every supported feature profile when G01 freezes those profiles; every included dependency must then pass this policy. No artifact build or full dependency license audit is claimed.

## Bounded executable decision example

The following standard-library-only model illustrates the admission rule. It is not an SPDX parser or production admission tool. `Approved` carries explicit booleans for reviewed source provenance and completed obligations; cumulative terms hold if either record is incomplete, OR selection returns the selected branch index, and any held route blocks distribution.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Evidence {
    provenance_recorded: bool,
    obligations_recorded: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    Approved(Evidence),
    Hold(&'static str),
}

impl Route {
    fn admitted(self) -> bool {
        matches!(
            self,
            Self::Approved(Evidence {
                provenance_recorded: true,
                obligations_recorded: true
            })
        )
    }
}

fn and(parts: &[Route]) -> Route {
    if parts.iter().all(|part| part.admitted()) {
        Route::Approved(Evidence {
            provenance_recorded: true,
            obligations_recorded: true,
        })
    } else {
        Route::Hold("at least one obligation or provenance record is unresolved")
    }
}

fn choose_or(selected: usize, branches: &[Route]) -> (usize, Route) {
    (
        selected,
        branches
            .get(selected)
            .copied()
            .unwrap_or(Route::Hold("invalid selection")),
    )
}

fn distribution_allowed(routes: &[Route]) -> bool {
    routes.iter().all(|route| route.admitted())
}

fn main() {
    let mit = Route::Approved(Evidence {
        provenance_recorded: true,
        obligations_recorded: true,
    });
    let missing_provenance = Route::Approved(Evidence {
        provenance_recorded: false,
        obligations_recorded: true,
    });
    let missing_obligation = Route::Approved(Evidence {
        provenance_recorded: true,
        obligations_recorded: false,
    });
    let copyleft = Route::Hold("copyleft route needs separate recorded decision");
    let custom = Route::Hold("custom or unknown terms");
    let missing = Route::Hold("missing license evidence");

    assert_eq!(choose_or(1, &[copyleft, mit]), (1, mit));
    assert_eq!(and(&[mit, mit]), mit);
    assert_eq!(
        and(&[mit, custom]),
        Route::Hold("at least one obligation or provenance record is unresolved")
    );
    assert!(!and(&[missing_provenance]).admitted());
    assert!(!and(&[missing_obligation]).admitted());
    assert_eq!(choose_or(0, &[custom, copyleft]), (0, custom));
    assert_eq!(choose_or(2, &[mit]), (2, Route::Hold("invalid selection")));
    assert_eq!(missing, Route::Hold("missing license evidence"));
    assert!(!distribution_allowed(&[mit, missing]));
    assert!(distribution_allowed(&[mit]));
}
```

This exact example was formatted, compiled, and run from a temporary source file with the cached Rust 1.98.1 `rustfmt` and `rustc`; all assertions passed. Its explicit `Route` values model evidence already reviewed by a human; it does not prove that strings are valid SPDX, validate provenance, infer obligations, or certify any real dependency.

## Source references and evidence

- Rust toolchain and targets: [planning/toolchain-targets.md](toolchain-targets.md); pin is stable Rust 1.98.1, edition 2024, Cargo.lock committed.
- Integration/release gates and prohibition on dependency installation by planning: [planning/implementation-roadmap.md](implementation-roadmap.md), especially “Service release stages and expensive prerequisites.”
- Source/catalog/commercial rights remain separate: [planning/rules-support.md](rules-support.md), “Paid rights and novice adjudication.”
- Current source inputs: `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `crates/df-rpc-bridge/vendor/h2/Cargo.toml`, and `crates/df-rpc-bridge/vendor/h2/LICENSE`, all at input revision above. SHA-256: Cargo.toml `a8f2d7317d401a25eb5ef4315dd0b42184f8f947d4cf82b055fd64c4656a5f05`; Cargo.lock `c29d0c7cd8d60e7d63aa16803713de3bd8145ed21e614b50121c0d0f55fdd947`; vendored h2 manifest `94e50ea2e4efc0f1a5c206ca0a79ba7cee99ff73416761db4cafb478aafc3818`; h2 LICENSE `b21623012e6c453d944b0342c515b631cfcbf30704c2621b291526b69c10724d`.
- Offline metadata JSON and resource output: `/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G01-D03-a1/metadata.json`, `metadata-native.json`, `metadata-wasm.json`, and matching `*-time.txt` files. Cargo exited 0 for each; the unfiltered command took 0.29 seconds with maximum resident set 75,677,696 bytes. Target-filtered graphs have 153 native and 143 WASM package records. No downloads or builds were run. Metadata output is retained in the assigned scratch root, outside Git.
- Model source/binary: `/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/B-G01-D03-a1/decision.rs` and `decision`; the source is also reproduced above. `rustfmt +1.98.1` and `rustc +1.98.1 --edition=2024` completed successfully; executable exit 0. Temporary Rust source was not added to the repository.
- SPDX license-expression concepts: [SPDX License List](https://spdx.org/licenses/) and [SPDX 2.3 License Expressions](https://spdx.github.io/spdx-spec/v2.3/SPDX-license-expressions/) (accessed 2026-10-01). SPDX defines `OR` as a choice and `AND` as simultaneous requirements; the cited older specification supports the operators' stated distinction, while this policy does not rely on it for a particular license's legal effect.
- Cargo package license metadata and nonstandard `license-file`: [Cargo manifest reference](https://doc.rust-lang.org/cargo/reference/manifest.html) and [Cargo metadata reference](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html) (accessed 2026-10-01). Declared metadata is package-provided; this policy therefore requires text/provenance review as well.

## Original acceptance criteria

- approved routes
- The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.

## Original verification criteria

- Freeze the cited source decision and a bounded contract example; compare approved routes. Retain decision, alternatives and unresolved facts.
- Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.

## Remaining work and check boundary

The policy route is selected, and the bounded source/model evidence is retained. Unresolved package terms and the `h2` source lineage/patch review keep distribution admission open. A complete SPDX-aware package/text/notice audit, exact native/WASM closures, and coordinator approval of resulting routes remain outstanding. This source decision is not an integrated acceptance or full G01 result.
