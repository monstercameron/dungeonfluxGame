# ADR 0004: Development lessons from the archived game

Date: 2026-09-29
Status: Accepted development guidance; tooling implementation pending

## Context

The archived game repeatedly passed package checks while the running game failed
at integration boundaries. Its devlog also records shared-file collisions, stale
builds, invisible workers, and timing-dependent tests. Carry those lessons into
the SQLite workflow without importing its Go stack, fixed lanes, demo schedule,
coverage quota, or one-off story rules.

The archive is reference material, not the specification for the Rust game.
Current user decisions and approved new planning govern development. Details
explicitly deferred in planning remain deferred.

## Decision

### Briefs must stand alone

Preserve each attempt's brief. Give a worker these concrete instructions:

1. Objective and one sentence explaining why the change matters.
2. Governing design sections, mandatory coding style, input revision, and
   prerequisite contracts.
3. Owning crate, exact permitted edit areas, and relevant existing examples.
4. Observable acceptance criteria, including significant rejection/failure cases.
5. Integration hooks: registration, caller, projection, mounting, or asset entry
   needed to make the result reachable, with an owner for each hook.
6. Focused verification commands or procedures, their required environment, and
   the expected evidence. Never require a gate whose prerequisites are unfinished.
7. Known pitfalls relevant to this task, and the handoff requirements below.

Keep briefs short and specific; do not paste the whole backlog into every prompt.
All code-writing and evaluation agents follow
[Coding style](../planning/coding-style.md); verification instructions include its
applicable formatter, lint, and native/WASM target gates once tooling exists.
An implementation task can intentionally deliver a library component, but its
feature cannot be declared complete until that component is integrated and its
user behavior is verified. Assign connecting work before dispatch, rather than
discovering that no worker owns it after handoff.

### Protect integration and shared files

Once Git exists, source implementation uses isolated worktrees and sequential
integration in ADR 0001. Earlier planning/documentation uses explicit file ownership
and content hashes; it does not require initializing Git. Task edit areas
must include shared composition, schema, manifest, and layout files they touch;
different feature names do not make edits to the same file independent. Shared
contracts, generated types, and workspace configuration have one active owner.

Land shared foundations before dependent tasks. Contract changes update affected
projections, consumers, fixtures, and dependency checks together. A worker needing
a missing contract reports the exact requirement; it must not bypass the boundary
with reflection, a competing public type, or a silent production stand-in.

Before dispatch or resume, recheck prerequisites and the current source. Preserve
useful context on recovery, but do not assume earlier reads or checks still apply.
Do not touch another attempt's files or Git index. Stage only the task's paths.

### Verify behavior at the boundary that can fail

Use focused crate checks for fast feedback, contract checks for cross-crate
assumptions, and a working integration slice through the real composition root
early. Browser work needs browser-target checks and a real browser smoke test;
native-only checks cannot establish client startup, rendering, or audio playback.

Use fakes and recorded fixtures for ordinary provider checks. Fakes must preserve
the meaningful contract: accepted and rejected actions, completion/cancellation,
valid payloads, and matching text/audio. Test adapters' actual outgoing request
fields and error handling with local fixtures. Live provider checks are separate,
explicitly scoped work with deadlines and spend limits.

Test time and concurrency with controllable clocks, explicit synchronization,
and adversarial event ordering. Wall-clock sleeps and longer timeouts must not
hide a scheduling defect. Update a stale assertion only after verifying the
approved behavior changed; do not weaken checks merely to make a gate pass.

No blanket coverage percentage or subjective critic score proves completion.
Check observable behavior and the failure cases implicated by the change. Keep
review loops bounded as in ADR 0001, and record concrete unresolved defects.
The coordinator dispatches frontier evaluators with computer use and vision under
[ADR 0005](0005-frontier-output-evaluation.md). They independently operate and
inspect the running result, attach evidence to each required criterion, and leave
review pending when a required check cannot be performed.

### Test the build that will actually run

Identify the evaluated commit and integrated commit, plus configuration and asset
revision where relevant. Build from the actual candidate source with its declared
generated inputs; do not silently substitute stale HEAD, an old binary, or an
archive that omits the change. A changed candidate needs the affected checks again.

Use one isolated preview per attempt with its own port, data directory, and
process ownership. Keep a stable last-verified preview available for human testing
once preview tooling exists. Publish native/browser build sets atomically, include
compressed variants in invalidation, and confirm readiness before announcing a
new build. Freeze the tested build during a playtest or performance measurement.

Track every worker and its child processes. An exit code alone does not prove a
commit or handoff exists. Clean up only owned processes and superseded output;
bound searches to relevant project paths. Respect retained evidence and active
previews under ADR 0002.

### Compact, evidence-based handoff

Each handoff identifies task/attempt, input and submitted commit, changed paths,
behavior delivered, integration hooks completed or outstanding, and verification
commands with results and build identity. Include retained evidence references,
known gaps, proposed distinct follow-up tasks, and relevant devlog entry IDs.

Distinguish unit-tested, integrated, browser-observed, and physically tested
behavior. A CLI reaching the end does not establish that a phone rendered or a
speaker played audio. Never claim an unperformed check passed. The evaluator
confirms the submitted result; the coordinator confirms the integrated result
before marking done.

## Evidence from the archive

Reviewed local snapshot: `dungeonflux.old`, Git revision
`a35d4259d2f8e6161ca403bcc1a5fe1202b84e9e`.
Devlog references below are stable entry IDs in its `docs/devlog.html`; todo IDs
refer to its `TODOS.md`. Historical entries describe their own test conditions,
not a guarantee that all related defects were fixed.

| Evidence | Lesson carried forward |
| --- | --- |
| `e-20260926-shared-index`, `e-20260926-shared-tree-hazards`, `e-20260926-nineteen-lanes-interlocking-edits` | Isolate workers and include shared files in conflict areas |
| `e-20260926-dm-concepts-layout-rebuild` | Establish shared layout/components before parallel screen work |
| `e-20260926-eng015-dispatcher-gap`, `e-20260926-rt009-rt010-noop-runner`, RT-009/010 | Exercise the real composition root and executor registration |
| `e-20260926-integration-todos-after-lanes`, `e-20260926-integration-hook-rule`, INT-007 | Name integration hooks and owners in the brief |
| `e-20260926-browser-bring-up-five-bugs`, `e-20260926-integration-gaps-green-lanes`, E2E-003/004/007 | Verify browser startup and complete game flows early |
| `e-20260927-battle-review-failures`, QA-011 | Test the complete mounted component, not only helpers |
| `e-20260926-llm014-flaky-synctest`, `e-20260927-room-sequence-test-barrier`, QA-003 | Use controlled clocks and synchronization |
| `e-20260926-tracked-worker-launches`, `e-20260926-network-cert` | Track launches and recover with refreshed source/context |
| `e-20260926-git-archive-vs-moving-tree`, `e-20260927-live-reload-fingerprint`, KC-005 | Tie evidence to current candidate/build identity |
| `e-20260926-atomic-web-builds`, `e-20260927-reload-ready-recovery`, QA-009/012 | Publish complete build sets and wait for readiness |
| `e-20260926-concept-art`, `e-20260926-no-code` | Art supplies visual direction; research does not authorize implementation |

Runtime-specific lessons and supporting code references are carried into
[Runtime reliability](../planning/runtime-reliability.md). Existing SQLite task
and devlog structure stays unchanged; these rules improve briefs and checks.
