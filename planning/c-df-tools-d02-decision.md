# df-tools owned preview and process decision

Task/attempt: `B-C-df-tools-D02/a1`  
Input revision: `575df9c3a619e8ee68eecd6bbf3d4c5ebdd00c35`  
Status: bounded design decision; production preview implementation remains pending.

## Decision

`df-tools` is the sole owner of build-preview process identity and its owned
artifact claims. A managed preview has one attempt ID and one immutable preview
ID. Its record binds the actual process identity (PID plus OS process-start
identity), bound port, canonical web root, private per-preview data directory,
and build identity (the `df-tools` `BUILD_ID` plus the separately supplied web
build/source identity). A PID, port, directory, or process name by itself never
grants release authority. Unknown entries, stale process identity, or a request
from another attempt are typed refusals; cleanup never searches for and kills a
matching-looking process.

The serving scope owns its listener and registration for the same lifetime. On
normal shutdown it releases only its own record after the listener stops, after
rechecking the attempt, preview, and process identity. The periodic cleanup
worker never signals a PID or stops a live preview. After a crash, it may retire
a registration only after the OS process-start identity proves that exact
process is gone and ADR 0002's terminal/abandoned and use-claim rules pass. It
may remove an artifact only inside an approved
`artifacts/build/` or `artifacts/tmp/` root, after its owner is terminal or
confirmed abandoned, no active use claim remains, it is superseded or
reproducible, it is not retained evidence or the latest verified build, and at
least 30 minutes have passed. Recheck those facts immediately before removal
under the coordinator's cleanup claim. Unknown or ambiguous ownership is kept.
This preserves ADR 0002's use-claim and retention rules and ADR 0004's
attempt-owned preview boundary. It does not authorize deletion of source, Git
state, databases, durable evidence, active runtime data, or a live preview.

Build publication is atomic: the complete native/WASM/asset set and manifest
become visible together. A preview holds an active claim on that exact build
identity for its lifetime. A new build cannot replace files under a running
preview. A last-verified build remains available until a newer complete build
has passed readiness and is explicitly promoted. Attempt completion, failed
builds, cancellation, and uncertain shutdown are explicit results; none imply
successful publication or release of an unconfirmed process claim.

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
`crates/df-tools/src/fixture.rs`. A private native registration owner should
accept an explicit attempt-owned `DF_PREVIEW_STATE_ROOT` from its launching
tool; absence keeps existing invocations unregistered. After successful bind,
it atomically records the actual process-start
identity, port, canonical web and per-preview data roots, `BUILD_ID`, and the
separately supplied web-build/source identity. Keep registration opt-in so
existing direct fixture invocations work unchanged. A record must be exclusive
to its preview ID and removed on orderly shutdown only if its owner identity
still matches. A later reaper may retire a record or artifact only under the
cleanup conditions above; it must not infer process authority from the record
filename. The existing `df-transport-fixture` main is the real consumer; no
`dfctl`/`xtask` binary exists yet. Do not add public/admin RPC: campaign
publication or activation transport remains subject to X10/G03. `df-tools`
returns safe lifecycle facts and does not call telemetry SDKs.

## Bounded contract literal

This standalone literal captures the decision's authorization and artifact
retention cases. It is evidence for this pure decision table only; it does not
claim production process signaling, a running preview, or an integrated
`df-tools` implementation.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Attempt(&'static str);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProcessIdentity {
    pid: u32,
    started_at: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Preview {
    id: &'static str,
    owner: Attempt,
    process: ProcessIdentity,
    port: u16,
    data_dir: &'static str,
    build_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReleaseRequest {
    id: &'static str,
    caller: Attempt,
    observed_process: ProcessIdentity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReleaseDecision {
    RemoveOwnRecord,
    RefuseUnknownPreview,
    RefuseWrongOwner,
    RefuseStaleProcessIdentity,
}

fn authorize_release(registered: Option<&Preview>, request: ReleaseRequest) -> ReleaseDecision {
    let Some(preview) = registered else {
        return ReleaseDecision::RefuseUnknownPreview;
    };
    if preview.id != request.id {
        return ReleaseDecision::RefuseUnknownPreview;
    }
    if preview.owner != request.caller {
        return ReleaseDecision::RefuseWrongOwner;
    }
    if preview.process != request.observed_process {
        return ReleaseDecision::RefuseStaleProcessIdentity;
    }
    ReleaseDecision::RemoveOwnRecord
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

fn main() {
    let process = ProcessIdentity {
        pid: 41,
        started_at: 900,
    };
    let preview = Preview {
        id: "preview-a1",
        owner: Attempt("a1"),
        process,
        port: 43123,
        data_dir: "artifacts/tmp/a1/preview-a1",
        build_hash: [7; 32],
    };
    assert_eq!(preview.port, 43_123);
    assert_eq!(preview.data_dir, "artifacts/tmp/a1/preview-a1");
    assert_eq!(preview.build_hash, [7; 32]);
    let request = ReleaseRequest {
        id: preview.id,
        caller: preview.owner,
        observed_process: process,
    };
    assert_eq!(
        authorize_release(Some(&preview), request),
        ReleaseDecision::RemoveOwnRecord
    );
    assert_eq!(
        authorize_release(None, request),
        ReleaseDecision::RefuseUnknownPreview
    );
    assert_eq!(
        authorize_release(
            Some(&preview),
            ReleaseRequest {
                caller: Attempt("a2"),
                ..request
            }
        ),
        ReleaseDecision::RefuseWrongOwner
    );
    assert_eq!(
        authorize_release(
            Some(&preview),
            ReleaseRequest {
                observed_process: ProcessIdentity {
                    pid: 41,
                    started_at: 899,
                },
                ..request
            }
        ),
        ReleaseDecision::RefuseStaleProcessIdentity
    );
    let eligible = ArtifactFacts {
        approved_root: true,
        owner_terminal_or_abandoned: true,
        active_use_claim: false,
        superseded_or_reproducible: true,
        retained_evidence: false,
        latest_verified_build: false,
        terminal_age_seconds: 1_800,
    };
    assert_eq!(artifact_cleanup(eligible), ArtifactDecision::Remove);
    assert_eq!(
        artifact_cleanup(ArtifactFacts {
            active_use_claim: true,
            ..eligible
        }),
        ArtifactDecision::Retain
    );
    assert_eq!(
        artifact_cleanup(ArtifactFacts {
            terminal_age_seconds: 1_799,
            ..eligible
        }),
        ArtifactDecision::Retain
    );
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
- Exact portable process-start identity verification, port reservation
  handoff, data-directory
  layout, complete build manifest fields, readiness probe, last-verified
  promotion authority, and the coordinator-to-registry recovery call are not
  frozen by the current source or upstream gates. Resolve them in the bounded
  I02 implementation brief; do not weaken the ownership checks above.
- No executable application or production preview exists at this revision.
  Browser/device behavior, native/WASM build publication, process signaling,
  coordinator integration, and cleanup execution remain unperformed.

## Provenance and handoff

The input commit is `575df9c3a619e8ee68eecd6bbf3d4c5ebdd00c35`; source and
tool hashes, extracted literal, executable, exact commands, guard receipts,
attempt outputs, and criteria-to-evidence status are recorded in
`handoff-manifest.json` under the frozen attempt evidence root. The literal
passed pinned `rustfmt --check`, pinned `rustc --edition=2024 -Dwarnings`, and
its finite assertions under the immutable v4 guard. Each fresh admission read
44% against the 39% floor; sampled guard-plus-command peaks were 46,219,264,
114,065,408, and 12,288,000 bytes. All three commands exited 0 with no remaining
owned process group.

The first rustfmt guard attempt failed before command launch because the guard's
child PATH omitted `rustup`. Root recorded the changed-prerequisite admission;
the missing-command exception and empty stdout/stderr files remain preserved.
The retry used the existing pinned absolute binaries and fresh receipts, with
the same limits. This attempt changes only this decision file. No Cargo,
production, browser, preview-process, native/WASM build, or integrated checks
were run or inferred from the literal. No queue/devlog database was mutated;
the devlog entry ID is unavailable and must be supplied by the coordinator if
recorded.
