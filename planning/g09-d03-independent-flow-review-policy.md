# Independent review of delivered output

Date: 2026-10-01
Status: Policy decision for `df-workflow`; runner implementation pending

## Decision

The source of authority is [ADR 0001](../ADR/0001-sqlite-agent-workflow.md) for task/attempt review and coordinator-owned completion, [ADR 0004](../ADR/0004-development-reliability.md) for exact-build boundary verification and handoff, and [ADR 0005](../ADR/0005-frontier-output-evaluation.md) for independent operation and inspection of running output. [Implementation roadmap](implementation-roadmap.md) requires real browser evidence for user-facing slices; [subsystem interfaces](subsystem-interfaces.md) assigns rendering, audio, session authority, and workflow responsibilities to their owning boundaries. Resource and retained-evidence constraints follow [ADR 0002](../ADR/0002-resource-scheduling-and-cleanup.md), with observations recorded under [ADR 0003](../ADR/0003-agent-devlog.md). Together these preserve the existing task lifecycle and contracts; this policy introduces no additional role, table, or lifecycle state.

A task that changes user-visible behavior is accepted only after an independent
frontier evaluator operates the exact submitted candidate through its real user
flow and inspects the rendered output. This is a required output gate in addition
to code review and applicable automated checks. A code review, helper test,
fixture, DOM query, log, worker screenshot, or passing build can support the
review, but none alone establishes that a user can complete the flow or sees the
expected result.

Before inspecting worker evidence, the evaluator derives procedures and expected
observations from the task's original acceptance criteria and governing sources.
It starts the candidate from the recorded source/build/configuration/content and
asset identities, performs the relevant user inputs, and inspects meaningful
initial, intermediate, success, rejection/error, and recovery states implicated
by the contract. The procedure reaches a server-confirmed result where the flow
is server-authoritative. For a browser change, this means actual computer use in
the browser and visual inspection of the rendered result. DOM and native reports
can corroborate visible evidence and identify state; they do not replace visual
inspection. Use multiple roles, viewports, physical devices, or privacy checks
when the changed contract requires them. Do not claim a physical-device check
from desktop emulation.

The evaluator must be a different actor from the implementing worker and any
repair worker. At dispatch, verify and record the selected runner's actual
frontier model, computer-use and vision capabilities needed for this user-facing
review, and audio observation capability when audio is required. A model name or
configured preference does not prove a capability. If a required tool or
prerequisite is unavailable, the result is inconclusive and remains in review;
do not silently substitute a code-only reviewer or narrow the acceptance scope.
For a task with no user-visible output, use its actual affected executable or
integration boundary and do not invent a UI gate. Its dependent feature's final
acceptance still exercises the integrated user flow.

Evidence is bound to the exact task and attempt, input and submitted revision,
tested integrated revision when applicable, build/configuration/content/asset
identity, environment, evaluator identity, actual capabilities used, and retained
evidence references. Each original criterion records its procedure/input,
expected result, observed result, evidence, and one of `PASS`, `FAIL`, or
`UNVERIFIED`. Keep unsupported, pending, failed, and unperformed checks explicit.
A changed candidate or integration that affects reviewed behavior requires fresh
review of affected criteria. Reuse evidence only when its provenance establishes
that it applies to the exact unchanged criterion and candidate.

Approve only when every required criterion has direct, current evidence, relevant
failure cases pass, and no unresolved defect violates the contract. A demonstrated
defect is a rejection with reproduction steps and expected behavior. Missing or
ambiguous evidence, stale identity, unavailable capability, or an unperformed
check is inconclusive, never a pass. The coordinator alone integrates sequentially
and records completion after checking the integrated result under ADR 0001.
Review findings and meaningful blockers/outcomes belong in the append-only devlog
under ADR 0003. Preserve screenshots, interaction records, captures, output and
cited log records as durable evidence under ADR 0002; do not retain credentials
or private payloads in default diagnostics.

Audio acceptance requires actual playback and an audio-capable observation or
retained capture appropriate to the criterion. Visual indicators and logs cannot
prove audible or synchronized playback. If audio is not required by the task,
record it as out of scope rather than implying it was checked.

## Source-bound example

The accepted browser review at
`development/evidence/browser-credit-stall/a2/integration-browser-reviewer/review.json`
was evaluated on 2026-10-01 at `12:12:42 UTC`. It identifies tested integrated
revision `9def9b845531e5cc589a28343b822b4664bd03fd`, the actual Codex IAB browser,
Chrome 154 on macOS, and computer use plus vision as capabilities actually used.
It records five original criteria with concrete interaction procedures, expected
and observed results, evidence references, and an `APPROVE` verdict. Among its
observations, the reviewer used the browser flow to exercise overlapping input,
visible running state, timeout failure, recovery, and cleanup, and retained both
visual/browser and native corroboration. This is a dated example that a source-
bound actual-flow review can produce direct evidence. Its approval applies only
to `BROWSER-CONNECTION-CREDIT-001-a2`; it does not approve this policy, prove
production readiness, establish unlisted physical-device/audio/gameplay checks,
or replace later review of another candidate.

The G09 backlog entry `B-G09-D03` remains an atomic design task owned by
`df-workflow`, with expected outcome `actual user flow` and its original
`computer_use`, `vision`, and `audio` capability fields all `false`. Those fields
are preserved as backlog inputs; they do not waive ADR 0005's conditional
capability requirement for a user-facing output review. For this user-facing
browser case, computer use and vision are required by the review contract. Audio
is not required by the named example or this task. This document defines policy
only; it does not claim a workflow runner or production gate has been implemented.

## Literal boundary example

This standard-library-only Rust example makes the admission rule explicit. A
review cannot pass when the flow or required capability is absent, the observed
candidate revision differs from the submitted revision, or the reviewer is the
worker. It is an illustrative contract example, not a production API.

```rust
use std::collections::BTreeSet;

#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    MissingFlow,
    MissingCapability,
    StaleRevision,
    DependentReviewer,
}

struct ReviewEvidence<'a> {
    flow_id: Option<&'a str>,
    required_capabilities: &'a [&'a str],
    used_capabilities: &'a BTreeSet<&'a str>,
    submitted_revision: &'a str,
    observed_revision: &'a str,
    worker_id: &'a str,
    reviewer_id: &'a str,
}

fn admit(evidence: &ReviewEvidence<'_>) -> Result<(), Refusal> {
    if evidence.flow_id.is_none_or(str::is_empty) {
        return Err(Refusal::MissingFlow);
    }
    if evidence
        .required_capabilities
        .iter()
        .any(|capability| !evidence.used_capabilities.contains(capability))
    {
        return Err(Refusal::MissingCapability);
    }
    if evidence.submitted_revision != evidence.observed_revision {
        return Err(Refusal::StaleRevision);
    }
    if evidence.worker_id == evidence.reviewer_id {
        return Err(Refusal::DependentReviewer);
    }
    Ok(())
}

fn main() {
    let capabilities = BTreeSet::from(["computer_use", "vision"]);
    let valid = ReviewEvidence {
        flow_id: Some("join-and-complete-turn"),
        required_capabilities: &["computer_use", "vision"],
        used_capabilities: &capabilities,
        submitted_revision: "rev-7",
        observed_revision: "rev-7",
        worker_id: "worker-a",
        reviewer_id: "reviewer-b",
    };
    assert_eq!(admit(&valid), Ok(()));

    let mut adversarial = ReviewEvidence {
        flow_id: None,
        ..valid
    };
    assert_eq!(admit(&adversarial), Err(Refusal::MissingFlow));

    adversarial.flow_id = Some("join-and-complete-turn");
    let no_vision = BTreeSet::from(["computer_use"]);
    adversarial.used_capabilities = &no_vision;
    assert_eq!(admit(&adversarial), Err(Refusal::MissingCapability));

    adversarial.used_capabilities = &capabilities;
    adversarial.observed_revision = "rev-6";
    assert_eq!(admit(&adversarial), Err(Refusal::StaleRevision));

    adversarial.observed_revision = "rev-7";
    adversarial.reviewer_id = "worker-a";
    assert_eq!(admit(&adversarial), Err(Refusal::DependentReviewer));
}
```

## Scope and pending checks

This decision preserves the original task criteria: define an independent output
review contract whose expected result is the actual user flow; bind its evidence
to exact source/build identity; and keep unsupported, pending, failed, and
unperformed checks explicit. The accepted review above is source evidence for
how to express that contract, not a substitute for satisfying this task's own
review and integration gates.

No compiler, Cargo, rustfmt, browser, or workspace check has been run for this
policy change. The Rust example is literal policy documentation and has not been
compiled. This planning task changes no application source, so native/WASM
workspace checks remain unperformed and are not claimed as passes. The runner
implementation, enforced admission behavior, and any future integrated policy
acceptance remain pending.
