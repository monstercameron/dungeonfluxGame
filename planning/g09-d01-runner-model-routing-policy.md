# Runner model routing and independent evaluation policy

Status: Design decision for G09; coordinator implementation pending

This policy defines how the future `df-workflow` coordinator selects workers and
independent evaluators. The coordinator itself must be frontier-class. Select it
only after current availability and frontier eligibility are verified.
These are routing rules, not claims that any model, provider, tool, or production
runner is currently available. The named outcome
is the routing and frontier-coordinator-evaluator contract; implementing the
runner, leases, evidence store, or game integration remains outstanding.

## Resolve capability before dispatch

At each dispatch, the coordinator obtains a fresh inventory from the configured
runner/provider integration. Treat model availability and tool capability as
separate facts. A model may be selectable while the required computer-use,
vision, or audio-observation capability is absent; such a runner cannot satisfy
a criterion that depends on that capability. Inventory data is scoped to the
provider, account/configuration, region if applicable, and observation time. An
old inventory or model-family label is not proof of present availability.

Keep three identities distinct in the attempt record and evidence:

- **Requested route:** the model class/name and runner capabilities required by
  the task policy.
- **Observed availability:** the provider's current inventory and advertised
  runner tools, with observation time and scope.
- **Serving identity:** the model/runner identity attested for the actual
  invocation and the capabilities actually enabled for that invocation.

Do not infer serving identity from the requested name or availability snapshot.
If the serving identity is unavailable or differs from the requested route,
record the actual value and explicit substitution reason. Refuse dispatch when a
mandatory model class or capability cannot be met. The coordinator may choose a
permitted substitute only when the task contract allows it; never describe a
substitute as the preferred model. Current product availability is an operational
lookup at dispatch time, not a durable design assumption.

For ordinary bounded, well-specified work, prefer an actually available Luna- or
Muse-class worker; Terra or Sol is appropriate when demonstrated complexity
justifies it. For UI work, prefer an actually available Sonnet 5.5-or-newer or
Opus-class route, using Opus sparingly. These are preferences from ADR 0001,
subject to task suitability and actual inventory. They do not override required
capabilities, prerequisites, or resource limits. Do not perform provider-specific
live calls to establish a policy example; inventory and serving identity belong
to the configured runner integration.

## Independent evaluator and evidence

Every submitted implementation/repair attempt receives an evaluator dispatched
by the frontier coordinator under ADR 0001 and ADR 0005. Before dispatch, require
both (a) current model availability and (b) separately verified frontier
eligibility from the configured trusted runner/provider integration. Eligibility
is its own evidence-backed fact, not inferred from inventory membership, model
name, strength, or capability flags. If frontier eligibility cannot be verified,
refuse the evaluator route and keep review pending.

Every evaluator invocation must have computer-use and vision capabilities
enabled at dispatch, including internal-library tasks. This is a baseline
frontier-evaluator requirement, not a claim that every task needs a user-facing
UI check. The evaluator operates and visually inspects the running output when
the acceptance contract has user-facing behavior; for internal outcomes it uses
the actual affected boundary or fixture without inventing a UI criterion. Audio
criteria also require a suitable audio-capable observation or retained capture.
A model label alone proves neither frontier eligibility nor enabled capabilities.

The evaluator must not be the implementer, a repair worker on the same attempt,
or an invocation whose identity cannot be distinguished from the implementer.
The coordinator checks the evaluator invocation identity, capability evidence,
and submitted revision/build identity before review. Evaluators derive checks
from the task criteria before relying on worker explanations. They exercise the
running boundary and inspect the output; code review, tests, logs, or worker
screenshots are supporting evidence only. For internal library outcomes, execute
the actual affected boundary or fixture specified by the contract; do not invent
a UI check. Map each criterion to current retained evidence and return pass, fail,
or unverified. Missing required tools or ambiguous identity leaves the attempt in
review as inconclusive, never approved.

Reserve memory and execution capacity for frontier evaluation, required browser
or multiple-client sessions, and integration/build work before dispatching more
implementation workers (ADR 0002). Throttle implementation when review capacity
is saturated. Do not downgrade the evaluator to an economical code-only worker
to relieve pressure. Keep worker and child-process peak memory bounded by
observed workload; start conservatively without measurements. The coordinator
owns tracked phase leases, recovery/fencing, edit-area exclusivity, resource
admission, and durable evidence references under the existing five-table
workflow; this policy adds no role, table, or task lifecycle state.

## Bounded economical work and escalation

Give one worker ownership of one scoped atomic task and its complete result.
Keep ordinary worker tasks bounded by explicit source revision, permitted paths,
prerequisites, success and refusal cases, integration-hook owners, and finite
checks. Avoid recursive delegation of the same item. Limit owned child work to
64 MiB peak memory and 60 seconds wall time unless a task-specific admission
explicitly raises either limit before dispatch. The coordinator accounts for
whole-process observed peak, not only the model client, and reserves evaluator
and build capacity first. On reaching a bound, stop/fence the owned work safely,
retain partial artifacts and outcomes, and report blocked/inconclusive status;
do not silently continue outside the admitted budget.

Count unsuccessful implementation/repair attempts on the same task across
workers and model substitutions. The initial attempt counts. After two
unsuccessful Luna-, Terra-, or Muse-class attempts, route the next logic,
interface, or computer-use implementation/debug attempt to Sol; route UI/UX or
gameplay-experience work to Sol or Opus according to the unresolved defect.
Changing worker, model, or brief wording does not reset the counter. An evaluator
rejection, failed required check, or worker's report that it cannot resolve the
assigned defect is unsuccessful. A known missing dependency, unavailable tool,
provider outage, or build-environment failure is a prerequisite blocker, not a
model attempt: fix that condition before dispatching a retry. Carry forward both
outcomes, defects, reproduction steps, tried approaches, evidence and devlog IDs.
Escalation does not change acceptance criteria and the escalated attempt still
requires independent frontier evaluation. After escalation failure, reassess the
contract, prerequisite, or explicit scope before any further dispatch; do not
start a blind retry loop.

## Literal decision example (standard library only)

This illustrative contract has no production runner API. `available_models`
answers what the current inventory reports. `FrontierVerification` is distinct, identity/model-bound evidence from the
trusted eligibility-verification boundary; it cannot be derived from the
availability list. The coordinator accepts this result only from the configured
trusted verifier, which must be independent of both worker and evaluator.
`serving_identity_attested` records whether the actual invocation identity was
attested. Capability, frontier, identity, and
evaluator-independence refusals are explicit decision results.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ModelClass {
    Luna,
    Muse,
    Terra,
    Sol,
    Sonnet,
    Opus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FrontierVerification {
    subject_identity: u64,
    subject_model: ModelClass,
    verifier_identity: u64,
    verified_frontier: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Capabilities {
    computer_use: bool,
    vision: bool,
    audio_observation: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Invocation {
    identity: u64,
    serving_identity_attested: bool,
    model: ModelClass,
    capabilities: Capabilities,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RouteDecision {
    Dispatch(Invocation),
    RefuseUnavailable,
    RefuseNotFrontier,
    RefuseUnattestedIdentity,
    RefuseCapabilities,
    RefuseNotIndependent,
}

fn choose_evaluator(
    available_models: &[ModelClass],
    frontier_verification: FrontierVerification,
    candidate: Invocation,
    implementer_identity: u64,
    requires_audio_observation: bool,
) -> RouteDecision {
    if candidate.identity == implementer_identity {
        return RouteDecision::RefuseNotIndependent;
    }
    if !available_models.contains(&candidate.model) {
        return RouteDecision::RefuseUnavailable;
    }
    if frontier_verification.subject_identity != candidate.identity
        || frontier_verification.subject_model != candidate.model
        || frontier_verification.verifier_identity == candidate.identity
        || frontier_verification.verifier_identity == implementer_identity
        || !frontier_verification.verified_frontier
    {
        return RouteDecision::RefuseNotFrontier;
    }
    if !candidate.serving_identity_attested {
        return RouteDecision::RefuseUnattestedIdentity;
    }
    if !candidate.capabilities.computer_use
        || !candidate.capabilities.vision
        || (requires_audio_observation && !candidate.capabilities.audio_observation)
    {
        return RouteDecision::RefuseCapabilities;
    }
    RouteDecision::Dispatch(candidate)
}

fn main() {
    let available = [
        ModelClass::Luna,
        ModelClass::Muse,
        ModelClass::Terra,
        ModelClass::Sol,
        ModelClass::Sonnet,
    ];
    let good = Invocation {
        identity: 20,
        serving_identity_attested: true,
        model: ModelClass::Sol,
        capabilities: Capabilities {
            computer_use: true,
            vision: true,
            audio_observation: false,
        },
    };
    let verified_frontier = FrontierVerification {
        subject_identity: 20,
        subject_model: ModelClass::Sol,
        verifier_identity: 30,
        verified_frontier: true,
    };
    assert_eq!(
        choose_evaluator(&available, verified_frontier, good, 10, false),
        RouteDecision::Dispatch(good),
    );
    assert_eq!(
        choose_evaluator(&available, verified_frontier, good, 10, true),
        RouteDecision::RefuseCapabilities,
    );
    assert_eq!(
        choose_evaluator(
            &available,
            verified_frontier,
            Invocation {
                capabilities: Capabilities {
                    computer_use: false,
                    ..good.capabilities
                },
                ..good
            },
            10,
            false,
        ),
        RouteDecision::RefuseCapabilities,
    );
    assert_eq!(
        choose_evaluator(
            &available,
            verified_frontier,
            Invocation {
                capabilities: Capabilities {
                    vision: false,
                    ..good.capabilities
                },
                ..good
            },
            10,
            false,
        ),
        RouteDecision::RefuseCapabilities,
    );
    assert_eq!(
        choose_evaluator(
            &available,
            FrontierVerification {
                verified_frontier: false,
                ..verified_frontier
            },
            good,
            10,
            false,
        ),
        RouteDecision::RefuseNotFrontier,
    );
    assert_eq!(
        choose_evaluator(
            &available,
            FrontierVerification {
                subject_identity: 21,
                subject_model: ModelClass::Luna,
                verifier_identity: 30,
                verified_frontier: false,
            },
            Invocation {
                identity: 21,
                model: ModelClass::Luna,
                ..good
            },
            10,
            false,
        ),
        RouteDecision::RefuseNotFrontier,
    );
    assert_eq!(
        choose_evaluator(
            &available,
            FrontierVerification {
                verifier_identity: 10,
                ..verified_frontier
            },
            good,
            10,
            false,
        ),
        RouteDecision::RefuseNotFrontier,
    );
    assert_eq!(
        choose_evaluator(
            &available,
            FrontierVerification {
                verifier_identity: 20,
                ..verified_frontier
            },
            good,
            10,
            false,
        ),
        RouteDecision::RefuseNotFrontier,
    );
    assert_eq!(
        choose_evaluator(
            &available,
            FrontierVerification {
                subject_model: ModelClass::Luna,
                ..verified_frontier
            },
            good,
            10,
            false,
        ),
        RouteDecision::RefuseNotFrontier,
    );
    assert_eq!(
        choose_evaluator(
            &available,
            verified_frontier,
            Invocation {
                serving_identity_attested: false,
                ..good
            },
            10,
            false,
        ),
        RouteDecision::RefuseUnattestedIdentity,
    );
    assert_eq!(
        choose_evaluator(
            &available,
            verified_frontier,
            Invocation {
                identity: 10,
                ..good
            },
            10,
            false,
        ),
        RouteDecision::RefuseNotIndependent,
    );
    assert_eq!(
        choose_evaluator(
            &available,
            FrontierVerification {
                subject_model: ModelClass::Opus,
                ..verified_frontier
            },
            Invocation {
                model: ModelClass::Opus,
                ..good
            },
            10,
            false,
        ),
        RouteDecision::RefuseUnavailable,
    );
}
```

## Alternatives and unresolved production gates

A model-name-only route was rejected because a name does not establish current
availability, verified frontier eligibility, invocation identity, or enabled
tools. Availability and frontier proof remain separate inputs in the example; an
available Luna candidate with an independently verified negative eligibility
result is refused. A text-only independent review was rejected because it cannot
establish visible behavior or audibility.
Adding a new review role/table or resetting escalation per worker was rejected
because ADR 0001 already assigns review to the evaluator role and stores attempt
state, and the existing escalation rule counts the same task across retries.

The concrete Rust example is a decision contract only; it is not wired to an
agent runtime and does not validate an actual provider response. G09 production
work still needs the coordinator/`df-workflow` owner to implement and demonstrate:
current provider/model/tool inventory and serving-identity attestation; an
independent trusted frontier-verification result bound to each actual invocation;
scoped capability negotiation; tracked leases, safe recovery/fencing and edit-area
ownership; measured process/child memory and finite time admission; evaluator
reservation and independent dispatch; durable evidence references and retention;
workflow-backed cross-worker escalation; and integrated tests against real
runner boundaries. Native/WASM application checks and browser/audio observations
are unperformed because this document adds no application source or running
feature. This document does not establish that any named model is presently
available.

## Governing sources and provenance

This is attempt `B-G09-D01-a2`, repairing the rejected A1 submission. Its
input revision is `2a1adc3c8e65f393046c911cf572ca5f7d6d573b`; original task
acceptance is unchanged. A1 source, review, receipts, and handoff remain preserved in the prior
attempt evidence directory. Governing sources, frozen SHA-256:

- `planning/implementation-roadmap.md` — `0160ad8e8ec38f768e2348209b9989e30e8f403d9b1a4ebf694f0801f7206932`
- `planning/subsystem-interfaces.md` — `f26e1dca42e878f8a816c9f9aa37463cb8f061224598214ceee62632a161907a`
- `development/backlog-catalog.json` — `039b0a03b4085b43ad32c4063e2cb8fc789fc55fff7552aec6e951ec3b4704c3`
- `AGENTS.md` — `55578ed92c477dfe3c306db38c6ad20390bfe8208ca998e6f3bf6f6bebbec182`
- `planning/coding-style.md` — `2d8e327e4172643544bd80b591f38226c25b83940a10f1d9e18ada9044faeabb`
- `ADR/0001-sqlite-agent-workflow.md` — `acfe32a8d5e846aa4d3b2981529533b9f53cdcc11088424128e60c20b722adc8`
- `ADR/0002-resource-scheduling-and-cleanup.md` — `a6476cfef449e089639109cc6d3cf5f0800b792b57a32696258d5ac0b9511966`
- `ADR/0003-agent-devlog.md` — `ee293673b01391c3a39577bd116a60abb3edfa662a7a8d1315556d6fe537d912`
- `ADR/0004-development-reliability.md` — `8f03d02d478086fd1f50d7aa10e8e7e766e3b15ae02f7f96efa43757c98a4dae`
- `ADR/0005-frontier-output-evaluation.md` — `25ab35a57ee8516a272b1ff3d04bba4def91319255d89158c9be55283ca6c35c`
- `rustfmt.toml` — `7a142cbd3f5e9c09c6dc4a1850e053415530eb5450d27983dc26c75230b8c5ba`
- `rust-toolchain.toml` — `9500030ccefd0bab631fb7f1763f79f4103eca3a344c36cf15be869e330683bb`

No provider-specific product claims are made. Availability and serving identity
remain runtime facts to collect when the runner is implemented.
