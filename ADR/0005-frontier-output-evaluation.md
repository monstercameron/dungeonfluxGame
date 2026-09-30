# ADR 0005: Frontier evaluation of running outputs

Date: 2026-09-29
Status: Accepted strategy; coordinator implementation pending

## Context

Correct-looking code and passing unit checks can still produce broken game
behavior, inaccessible controls, bad layouts, missing assets, or silent audio.
The coordinator must obtain stronger evidence from an independent agent that
can operate the application and inspect what users actually receive.

## Decision

### Dispatch an independent frontier evaluator

The coordinator spawns a frontier-model evaluator for each submitted attempt.
Select a model/runner with computer-use tools and image understanding enabled;
a strong text model without those tools does not satisfy this requirement.
Check capabilities at dispatch rather than assuming them from a model name.
The evaluator is separate from the implementing worker and does not approve its
own implementation or repairs. This strengthens the existing evaluator role in
ADR 0001; it adds no orchestration role, table, or task lifecycle state.

Give it the task contract, governing planning sections, submitted revision, exact
build/preview identity, required checks, and retained evidence locations. Have it
derive its checks from the contract before consulting the worker's explanation.
Worker screenshots and claims are supporting evidence, not independent review.

Reserve frontier evaluation slots and memory for browsers, builds, and any
required multiple clients under ADR 0002. Reuse verified setup and focused checks
to keep reviews fast, but do not substitute an economical code-only reviewer for
the required output evaluator. Exact model/provider selection is a runtime
capability decision; unavailable capabilities leave review pending with a reason.

### Operate and inspect the delivered result

For user-facing changes, the evaluator must interact with a running candidate
through computer-use tools and visually inspect its rendered output. Use the
real browser/WASM build and affected display/player roles. Check the relevant
device sizes and complete the changed flow from input to server-confirmed result.
Inspect meaningful before/after states, text, controls, layout, assets, loading,
and error states. DOM queries alone do not prove the screen looks right, and a
single static screenshot does not prove the interaction works.

Choose relevant failure cases from the contract and change risk: rejected actions,
reconnect, cancellation, repeated input, stale responses, or missing assets.
Use the independent display/player clients when the feature crosses those roles;
inspect permitted views and privacy at the server boundary as well as the screen.

Visual inspection cannot establish that audio was audible or synchronized.
Audio acceptance requires playback plus an audio-capable observation or retained
capture appropriate to the criterion. If the runner cannot perform that check,
record the gap; do not infer successful playback from logs or a moving indicator.
Likewise, a desktop viewport does not prove a physical phone or TV was tested.

For internal crate tasks with no user-facing output, run the actual affected
boundary or executable fixture and inspect its results. Do not invent a UI gate
for a library task. The dependent feature's final acceptance still requires the
frontier evaluator to operate and visually review the integrated user flow.
Code review, tests, and read-only OTEL queries support these output checks.
Also verify the mandatory [Coding style](../planning/coding-style.md), including
format/lint results and semantic conventions on the authored changes. A clean
style check alone cannot approve an unverified running result.

### Approve from evidence, not a confidence score

Return a compact acceptance record using the existing attempt evidence/verdict:

- Evaluator identity, model, and capabilities actually used.
- Submitted/tested commit, build, configuration, asset revision, and environment.
- Each required criterion, procedure/input, expected result, observed result,
  and evidence reference; distinguish pass, fail, and unverified.
- Significant edge/failure checks and correlated telemetry IDs when relevant.
- Reproducible defects, verification gaps, and the resulting verdict.

Approve only when every required criterion has direct, current evidence, relevant
failure cases pass, and no unresolved defect violates the task contract. A
percentage confidence rating or worker assertion cannot replace a missing check.
Use targeted repeats when timing, reconnects, or nondeterminism leave uncertainty;
do not repeatedly rerun unrelated checks to create an impression of confidence.

A demonstrated defect yields rejection with reproduction steps and expected
behavior. Missing tools, unavailable prerequisites, stale builds, or ambiguous
observations yield an inconclusive verdict: keep the task in review and record
what is needed to finish verification. Never treat absence of evidence as a pass
or silently skip a required criterion. Append meaningful findings to the devlog.

Retain screenshots, interaction records, captures, test output, and cited log
records with task/attempt/build identity. Preserve evidence under ADR 0002/0003;
temporary screenshots in disposable output are not durable acceptance evidence.
Do not dump every frame or full log corpus into the evaluator prompt.

### Verify the integrated result

The coordinator integrates sequentially and checks the resulting build before
marking done. If integration changes reviewed code or behavior, obtain fresh
frontier evaluation of the affected criteria. Final feature acceptance exercises
the complete integrated flow and its significant failure cases, even when all
atomic tasks have individually passed. Reuse still-valid evidence for unchanged
criteria only when its build/configuration provenance establishes applicability.

Keep repair loops bounded as in ADR 0001. Fixing a contract defect requires an
independent evaluator rerun; creating a follow-up task does not permit approval
of unfinished original acceptance criteria.

## Consequences

Evaluation costs more than code-only review, but economical implementation
workers remain viable because completion is checked against the actual result.
Review scope follows the changed behavior and risk, with mandatory integrated
feature checks, so thousands of atomic tasks do not each repeat a full playtest.
This ADR defines future dispatch and approval behavior; it does not launch agents
or implement the coordinator during planning.
