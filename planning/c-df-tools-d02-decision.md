# df-tools owned preview and process decision

Task/attempt: `B-C-df-tools-D02/a2`
Input revision: `2022c90fa8b4f53b99eaec43d5e32f3e3b7205cc`
Status: bounded design decision; production preview implementation remains pending.

## Decision

`df-tools` is the sole owner of the managed preview's lifecycle record. One
attempt owns one explicitly scoped `DF_PREVIEW_STATE_ROOT`; managed mode also
requires `DF_PREVIEW_ATTEMPT_ID` and `DF_PREVIEW_WEB_SOURCE_ID`. The record binds
the serving process PID, bound port, canonical web root, private per-preview
data root, injected native `BUILD_ID`,
and the separately supplied web build/source label. These labels identify the
preview inputs; they do not claim a complete or approved native/WASM/asset
build manifest. Missing, malformed, shared, or already-owned registration
inputs are typed startup refusals. A PID, port, directory, or process name by
itself never grants release authority; cleanup never searches for and kills a
matching-looking process.

The serving scope owns its listener and a non-Clone private registration handle
for the same lifetime. The handle retains the exclusive record file and is the
release capability; its PID field is diagnostic, not authority. On normal
shutdown it releases only its own record after the listener stops and the
entire record still matches the held owner's saved record. The handle cannot be
reconstructed from observed labels or cloned; losing it leaves the record
ambiguous. The periodic cleanup worker never signals a PID or stops a live
preview. After a crash, the orphan record is ambiguous
and retained. The contract intentionally grants no PID-based process or record
action after the in-process owner is gone. It may remove an artifact only
inside an approved
`artifacts/build/` or `artifacts/tmp/` root, after its owner is terminal or
confirmed abandoned, no active use claim remains, it is superseded or
reproducible, it is not retained evidence or the latest verified build, and at
least 30 minutes have passed. Recheck those facts immediately before removal
under the coordinator's cleanup claim. Unknown or ambiguous ownership is kept.
This preserves ADR 0002's use-claim and retention rules and ADR 0004's
attempt-owned preview boundary. It does not authorize deletion of source, Git
state, databases, durable evidence, active runtime data, or a live preview.

The record's native build ID and web source ID are provenance labels only. They
do not establish a complete `BuildManifest`, paired native/WASM/asset identity,
atomic publication, or readiness of any build. The existing fixture serves a
supplied web root without modifying it. Full build publication and promotion
remain a separate implementation outcome.

## Current source and minimum implementation boundary

At the input revision, `crates/df-tools/src/main.rs` calls
`df_tools::fixture::serve()`. `src/fixture.rs::serve` takes the web root as its
first argument, accepts an optional nonzero port (default `43180`), binds only
`127.0.0.1`, serves the web root under `/pkg`, then runs until a server ends or
Ctrl-C arrives. Its existing comment says the caller owns the process and
output directory. `src/lib.rs` exposes the native `fixture` module and `BUILD_ID`.
There is no preview registry or structured build manifest. Compatibility means
keeping this direct fixture invocation and its current argument, loopback,
readiness, and shutdown behavior; unmanaged legacy invocations carry no cleanup
authority.

The smallest I02 implementation belongs in `crates/df-tools/src/preview.rs`,
privately wired from `crates/df-tools/src/lib.rs` and `fixture::serve` in
`crates/df-tools/src/fixture.rs`. When the three managed-mode environment values
are absent, current direct fixture invocations remain unregistered and retain
their behavior. If any managed-mode value is present but invalid or incomplete,
startup fails closed. The state root must be a canonical, attempt-owned
directory beneath `artifacts/tmp/`; no two previews share it. Registered mode
rejects the current `unregistered-cargo-build` sentinel. After the loopback
listener binds, create `preview-owner.txt` exclusively with a
`DF-PREVIEW-OWNER-V1` header and these finite fields: attempt ID, PID, bound
port, canonical web root, canonical data root, native build ID, and web source
ID. Attempt/build labels are 1–128 ASCII bytes from
`[A-Za-z0-9._-]`; PID is a decimal `u32`; port is a nonzero decimal `u16`; each
canonical absolute path is Rust-debug-quoted UTF-8 of at most 4,096 bytes; the
complete record is at most 16 KiB. A failed bind or record write never
announces readiness. A clean shutdown removes only the exact record whose
entire parsed identity, including owner, PID, port, preview roots, native build
ID, and web source ID, still matches this serving scope's private handle;
report release errors. An abrupt exit leaves the record for coordinator review;
no PID-based automatic retirement is authorized. The serving scope never
signals the PID, recursively deletes the data root, or deletes supplied web
assets. The periodic artifact cleanup policy remains ADR 0002; implementation
of that worker is outside I02. The existing
`df-transport-fixture` main is the
real consumer; no `dfctl`/`xtask` binary exists yet. Do not add public/admin RPC:
campaign publication or activation transport remains subject to X10/G03.
`df-tools` returns safe lifecycle facts and does not call telemetry SDKs.

## Bounded contract literal

This standalone literal models exclusive registration, a held capability,
whole-record recheck, and artifact retention. Its `record` stands for the
exclusively created file's bounded, successfully parsed contents;
`simulate_external_record_change` stands for a missing or replaced file and is
used only by these finite cases. I02 must perform exclusive file creation and
a bounded 16 KiB parse/recheck while the in-process owner is held and after
listener shutdown. This is evidence for the pure authorization boundary only; it
does not claim production process signaling, a running preview, or an
integrated `df-tools` implementation.

```rust
mod preview_contract {
    use std::{cell::Cell, rc::Rc};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Attempt(&'static str);

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct ProcessIdentity(u32);

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct Preview {
        id: &'static str,
        owner: Attempt,
        process: ProcessIdentity,
        port: u16,
        web_root: &'static str,
        data_root: &'static str,
        native_build_id: &'static str,
        web_source_id: &'static str,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum RegisterError {
        Occupied,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ReleaseDecision {
        RemoveOwnRecord,
        RefuseListenerActive,
        RefuseClosed,
        RefuseMissingRecord,
        RefuseReplacedRecord,
    }

    // Models an exclusive record file. Only successful exclusive registration
    // creates a handle; observed labels alone do not create one.
    struct Registry {
        record: Rc<Cell<Option<Preview>>>,
    }

    impl Registry {
        fn new() -> Self {
            Self {
                record: Rc::new(Cell::new(None)),
            }
        }

        fn register(&self, preview: Preview) -> Result<OwnerHandle, RegisterError> {
            if self.record.get().is_some() {
                return Err(RegisterError::Occupied);
            }
            self.record.set(Some(preview));
            Ok(OwnerHandle {
                expected: preview,
                record: Rc::clone(&self.record),
                listener_stopped: false,
                released: false,
            })
        }

        // Models a changed/missing on-disk record; only the finite cases call it.
        fn simulate_external_record_change(&self, observed: Option<Preview>) {
            self.record.set(observed);
        }
    }

    // Private fields and no Clone/Copy implementation: only the registering
    // serving scope can hold this capability. Drop does not retire the record.
    struct OwnerHandle {
        expected: Preview,
        record: Rc<Cell<Option<Preview>>>,
        listener_stopped: bool,
        released: bool,
    }

    impl OwnerHandle {
        fn listener_stopped(&mut self) {
            self.listener_stopped = true;
        }

        fn release(&mut self) -> ReleaseDecision {
            if self.released {
                return ReleaseDecision::RefuseClosed;
            }
            if !self.listener_stopped {
                return ReleaseDecision::RefuseListenerActive;
            }
            match self.record.get() {
                None => ReleaseDecision::RefuseMissingRecord,
                Some(observed) if observed != self.expected => {
                    ReleaseDecision::RefuseReplacedRecord
                }
                Some(_) => {
                    self.record.set(None);
                    self.released = true;
                    ReleaseDecision::RemoveOwnRecord
                }
            }
        }
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct ArtifactFacts {
        approved_root: bool,
        owner_terminal_or_abandoned: bool,
        active_use_claim: bool,
        superseded_or_reproducible: bool,
        retained_evidence: bool,
        latest_verified_build: bool,
        terminal_age_seconds: u64,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ArtifactDecision {
        Remove,
        Retain,
    }

    fn artifact_cleanup(facts: ArtifactFacts) -> ArtifactDecision {
        if facts.approved_root
            && facts.owner_terminal_or_abandoned
            && !facts.active_use_claim
            && facts.superseded_or_reproducible
            && !facts.retained_evidence
            && !facts.latest_verified_build
            && facts.terminal_age_seconds >= 1_800
        {
            ArtifactDecision::Remove
        } else {
            ArtifactDecision::Retain
        }
    }

    pub(super) fn run_cases() {
        let preview = Preview {
            id: "preview-a2",
            owner: Attempt("a2"),
            process: ProcessIdentity(41),
            port: 43_123,
            web_root: "/artifacts/build/a2/web",
            data_root: "/artifacts/tmp/a2/preview",
            native_build_id: "native-a2",
            web_source_id: "web-a2",
        };
        let registry = Registry::new();
        let mut owner = registry.register(preview).unwrap();
        assert_eq!(owner.release(), ReleaseDecision::RefuseListenerActive);
        assert!(matches!(
            registry.register(preview),
            Err(RegisterError::Occupied)
        ));
        owner.listener_stopped();
        assert_eq!(owner.release(), ReleaseDecision::RemoveOwnRecord);
        assert_eq!(owner.release(), ReleaseDecision::RefuseClosed);
        assert_eq!(registry.record.get(), None);

        let replacements = [
            ("missing", None),
            (
                "id",
                Some(Preview {
                    id: "replacement",
                    ..preview
                }),
            ),
            (
                "owner",
                Some(Preview {
                    owner: Attempt("other"),
                    ..preview
                }),
            ),
            (
                "pid",
                Some(Preview {
                    process: ProcessIdentity(42),
                    ..preview
                }),
            ),
            (
                "port",
                Some(Preview {
                    port: 43_124,
                    ..preview
                }),
            ),
            (
                "web root",
                Some(Preview {
                    web_root: "/other/web",
                    ..preview
                }),
            ),
            (
                "data root",
                Some(Preview {
                    data_root: "/other/data",
                    ..preview
                }),
            ),
            (
                "native build",
                Some(Preview {
                    native_build_id: "native-other",
                    ..preview
                }),
            ),
            (
                "web source",
                Some(Preview {
                    web_source_id: "web-other",
                    ..preview
                }),
            ),
        ];
        for (field, replacement) in replacements {
            let registry = Registry::new();
            let mut owner = registry.register(preview).unwrap();
            owner.listener_stopped();
            registry.simulate_external_record_change(replacement);
            let expected = if replacement.is_some() {
                ReleaseDecision::RefuseReplacedRecord
            } else {
                ReleaseDecision::RefuseMissingRecord
            };
            assert_eq!(owner.release(), expected, "{field}");
            assert_eq!(registry.record.get(), replacement, "{field}");
        }

        let registry = Registry::new();
        let dropped_owner = registry.register(preview).unwrap();
        drop(dropped_owner); // Abrupt exit or lost handle retains ambiguous record.
        assert_eq!(registry.record.get(), Some(preview));
        assert!(matches!(
            registry.register(preview),
            Err(RegisterError::Occupied)
        ));
        let legacy_unregistered = Registry::new();
        assert_eq!(legacy_unregistered.record.get(), None);
        // Neither the dropped nor legacy scope has a handle on which to call release.

        let eligible = ArtifactFacts {
            approved_root: true,
            owner_terminal_or_abandoned: true,
            active_use_claim: false,
            superseded_or_reproducible: true,
            retained_evidence: false,
            latest_verified_build: false,
            terminal_age_seconds: 1_800,
        };
        for mask in 0_u32..64 {
            for age in [0, 1_799, 1_800, 1_801, u64::MAX] {
                let facts = ArtifactFacts {
                    approved_root: mask & 1 != 0,
                    owner_terminal_or_abandoned: mask & 2 != 0,
                    active_use_claim: mask & 4 != 0,
                    superseded_or_reproducible: mask & 8 != 0,
                    retained_evidence: mask & 16 != 0,
                    latest_verified_build: mask & 32 != 0,
                    terminal_age_seconds: age,
                };
                let expected = if mask == 11 && age >= 1_800 {
                    ArtifactDecision::Remove
                } else {
                    ArtifactDecision::Retain
                };
                assert_eq!(artifact_cleanup(facts), expected);
            }
        }
        assert_eq!(artifact_cleanup(eligible), ArtifactDecision::Remove);
        println!("PASS: held owner, full record, lifetime, and 320 retention cases");
    }
}

fn main() {
    preview_contract::run_cases();
}
```

## Alternatives and unresolved facts

- Killing processes discovered by PID, port, command name, or broad project
  search was rejected because those observations do not establish ownership and
  can match unrelated work or a reused PID.
- Letting the periodic cleanup worker signal preview processes was rejected;
  preview lifetime belongs to the owner registry, while cleanup handles only
  revalidated stale output.
- Treating attempt termination, cancellation, age, or a successful build
  command as sufficient deletion/publication authority was rejected by ADR 0002
  and the build identity/readiness requirements in ADR 0004.
- Port reservation handoff, complete production build manifest fields,
  readiness probe, last-verified promotion authority, and the
  coordinator-to-registry recovery call are not frozen by current source or
  upstream gates. They are outside this lifecycle boundary; do not infer them
  from the fixture's PID or provenance labels.
- No executable application or production preview exists at this revision.
  Browser/device behavior, native/WASM build publication, process signaling,
  coordinator integration, and cleanup execution remain unperformed.

## Provenance and handoff

The a2 input commit is `2022c90fa8b4f53b99eaec43d5e32f3e3b7205cc`.
The exact submitted source, tool, literal, executable, guard-receipt and output
hashes, plus the original criterion and unresolved production gaps, are in
`handoff-manifest.json` under the frozen a2 attempt evidence root. The
extracted literal passed the pinned `rustfmt --check`, pinned
`rustc --edition=2024 -Dwarnings`, and finite execution under the a2 v5 guard.
Compiler-negative cases confirmed that a sibling module cannot construct the
private handle and that the handle cannot be cloned. The finite run covers valid
owned release, refused active/closed/missing/replaced release, dropped-owner
ambiguity, unregistered legacy scope, and all 320 original artifact-retention
truth-table cases. No Cargo, production, browser, preview-process, native/WASM
build, or integrated checks were run or inferred from the literal. The rejected
a1 review and its independent failure corpus remain preserved. This attempt
changes only this decision file; I02 still owns application implementation.
