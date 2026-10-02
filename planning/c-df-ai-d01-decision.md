# Validated semantic versus flavor output contract

Date: 2026-10-01  
Status: source-backed design decision; production types and integration pending

## Decision

`df-ai` returns a bounded, validated expression result with three disjoint
meanings. The result is descriptive output or a proposal; it is never a state
transition:

| Meaning | Contract | Authority after `df-ai` |
| --- | --- | --- |
| Grounded semantic expression | Ordered references to listener-authorized claim IDs and exact outcome/name/number/negation slots, with attribution, locale and a versioned basis. Canonical wording is rendered from reviewed localized templates or deterministic slots. | The owning world/rules/interaction subsystem and session owner retain truth and commit authority. |
| Intent proposal | A typed candidate limited to known offer IDs and the admitted input/basis. Questions, jokes, hypotheticals and plans remain their typed dispositions. | `df-engine`/`df-session` route and revalidate; `df-rules` determines legality and outcome. No proposal itself executes. |
| Flavor expression | Bounded local expression attached to an already permitted interaction, explicitly marked noncanonical and constrained by the approved style. | No subsystem may treat it as a fact, rule, location, quest, resource, successful check, promise, commitment, or durable lore. |

Use the listener-safe `SpeechActPlan` → per-audience `ExpressionContext` →
`SpeechEnvelope` policy in `planning/g11-d02-listener-safe-claim-policy.md` and
`planning/interaction-engine.md`. D01 does not define a second envelope or claim
authority. `df-ai::AiService::run(context, AiJob)` remains the service boundary
from `planning/subsystem-interfaces.md`; its `AiResult` is a validated text or
proposal, not a session mutation. `df-ai` owns qualification, cache/replay and
policy; `df-world` owns canonical causality, `df-rules` mechanical truth,
`df-knowledge` claim provenance/access, `df-interaction` speech acts and approved
deceit, and `df-engine`/`df-session` current-basis validation and commit.
`df-provider-api` owns provider contracts, `df-providers` native adapters, and
`df-api`/`df-client`/`df-persistence` their respective transport, rendering and
storage boundaries. No provider adapter or browser client is an authority.

For any audience, project only authorized claims before provider input,
retrieval, prompt construction, serialization, caption or audio. Bind output to
audience basis, claim/source/access revisions, locale, admitted job and execution
mode. Reject stale basis, unknown or unauthorized claim, missing/mismatched slot,
unsupported outcome, malformed schema, or resource-limit breach as a whole
response. Do not publish raw tokens or partial clauses while qualification is
pending. Commit an approved reveal before releasing its expression. If validation
fails, return a fixed source-safe expression matching the already approved
outcome, or an explicit `Uncertain`/`Rejected` result with matching caption/audio
fallback or visible silent gap. Never substitute success for refusal. A fallback
cannot invent a fact or change a committed outcome.

Structural validation, deterministic slot checks and string filters can reject
known invalid forms; they do not prove open prose has no factual implication.
A model critic is supplementary evidence and cannot grant truth, access, or
mechanical authority. Any flavor that asserts a claim outside the permitted
slots must be treated as uncertain/rejected or sent for an authorized review,
not promoted to canonical state. Attributed false belief or deceit remains an
interaction-approved speech act, preserving attribution without changing the
underlying canonical fact.

## Alternatives and rationale

1. **Treat every generated sentence as semantic truth.** Rejected: language
   generation cannot commit world causality, rules outcomes, knowledge grants or
   player commitments; prose may be false, ambiguous or adversarial.
2. **Let a second model certify the first.** Rejected: a critic cannot restore
   secret context already disclosed to a provider, guarantee semantic
   equivalence, authorize a reveal, or replace deterministic rules checks.
3. **Permit unrestricted prose, then parse it into canonical state.** Rejected:
   parsing free text does not make it an authorized typed transition and creates
   a competing state writer.
4. **Use templates only.** Safe for claims and outcomes but unnecessarily
   constrains voice and atmosphere. Keep deterministic templates/slots for
   semantics and permit qualified noncanonical flavor after listener projection.
5. **Stream tokens or clauses before whole-response qualification.** Deferred:
   a later clause, locale, basis or schema failure can require retracting exposed
   text/audio. Buffer the bounded response until the qualification contract and
   publication basis are stable.

## Finite std-only contract example

This standalone fixture demonstrates structural distinctions only. It has a
finite literal boundary for authorized projected claims, exact outcome slots,
proposal-only intents, noncanonical flavor, stale basis, unknown claim, slot
mismatch and obvious forbidden flavor content. Its short marker check is not an
injection detector or semantic proof. The byte and item bounds below exist only
for this example; production limits must be selected and measured under G10/G11.

```rust
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ClaimId(&'static str);

#[derive(Clone, Debug, Eq, PartialEq)]
struct Basis(u64);

#[derive(Clone, Debug, Eq, PartialEq)]
struct Grounded {
    claim: ClaimId,
    outcome_slot: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Candidate {
    Grounded(Grounded),
    IntentProposal(&'static str),
    Flavor(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProjectedClaim {
    basis: Basis,
    outcome_slot: &'static str,
    canonical_text: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Refusal {
    StaleBasis,
    UnknownOrUnauthorizedClaim,
    SlotMismatch,
    UnknownOffer,
    EmptyFlavor,
    FlavorTooLong,
    ForbiddenFlavorContent,
    TooManyClauses,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Qualified {
    Grounded(&'static str),
    IntentProposal(&'static str),
    Flavor { text: String, canonical: bool },
}

fn qualify(
    candidate: &Candidate,
    current: &Basis,
    projected: &BTreeMap<ClaimId, ProjectedClaim>,
    known_offers: &[&'static str],
) -> Result<Qualified, Refusal> {
    match candidate {
        Candidate::Grounded(grounded) => {
            let claim = projected
                .get(&grounded.claim)
                .ok_or(Refusal::UnknownOrUnauthorizedClaim)?;
            if &claim.basis != current {
                return Err(Refusal::StaleBasis);
            }
            if grounded.outcome_slot != claim.outcome_slot {
                return Err(Refusal::SlotMismatch);
            }
            Ok(Qualified::Grounded(claim.canonical_text))
        }
        Candidate::IntentProposal(offer) => {
            if !known_offers.contains(offer) {
                return Err(Refusal::UnknownOffer);
            }
            Ok(Qualified::IntentProposal(offer))
        }
        Candidate::Flavor(text) => {
            if text.trim().is_empty() {
                return Err(Refusal::EmptyFlavor);
            }
            if text.len() > 160 {
                return Err(Refusal::FlavorTooLong);
            }
            let lower = text.to_ascii_lowercase();
            if ["http://", "https://", "<tool>", "ignore previous"]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                return Err(Refusal::ForbiddenFlavorContent);
            }
            Ok(Qualified::Flavor {
                text: text.clone(),
                canonical: false,
            })
        }
    }
}

fn qualify_envelope(
    candidates: &[Candidate],
    current: &Basis,
    projected: &BTreeMap<ClaimId, ProjectedClaim>,
    known_offers: &[&'static str],
) -> Result<Vec<Qualified>, Refusal> {
    if candidates.len() > 4 {
        return Err(Refusal::TooManyClauses);
    }
    candidates
        .iter()
        .map(|candidate| qualify(candidate, current, projected, known_offers))
        .collect()
}

fn main() {
    let current = Basis(7);
    let mut projected = BTreeMap::new();
    projected.insert(
        ClaimId("outcome-7"),
        ProjectedClaim {
            basis: current.clone(),
            outcome_slot: "gate-open",
            canonical_text: "The gate is open.",
        },
    );
    let known_offers = ["offer-inspect"];

    let accepted = qualify_envelope(
        &[
            Candidate::Grounded(Grounded {
                claim: ClaimId("outcome-7"),
                outcome_slot: "gate-open",
            }),
            Candidate::IntentProposal("offer-inspect"),
            Candidate::Flavor("Mara lowers her voice.".to_owned()),
        ],
        &current,
        &projected,
        &known_offers,
    )
    .expect("each output kind remains distinct");
    assert_eq!(
        accepted,
        vec![
            Qualified::Grounded("The gate is open."),
            Qualified::IntentProposal("offer-inspect"),
            Qualified::Flavor {
                text: "Mara lowers her voice.".to_owned(),
                canonical: false,
            },
        ]
    );

    assert_eq!(
        qualify(
            &Candidate::Grounded(Grounded {
                claim: ClaimId("hidden-secret"),
                outcome_slot: "secret",
            }),
            &current,
            &projected,
            &known_offers,
        ),
        Err(Refusal::UnknownOrUnauthorizedClaim),
    );
    assert_eq!(
        qualify(
            &Candidate::Grounded(Grounded {
                claim: ClaimId("outcome-7"),
                outcome_slot: "gate-closed",
            }),
            &current,
            &projected,
            &known_offers,
        ),
        Err(Refusal::SlotMismatch),
    );
    let stale = Basis(6);
    assert_eq!(
        qualify(
            &Candidate::Grounded(Grounded {
                claim: ClaimId("outcome-7"),
                outcome_slot: "gate-open",
            }),
            &stale,
            &projected,
            &known_offers,
        ),
        Err(Refusal::StaleBasis),
    );
    assert_eq!(
        qualify(
            &Candidate::IntentProposal("invented-offer"),
            &current,
            &projected,
            &known_offers,
        ),
        Err(Refusal::UnknownOffer),
    );
    assert_eq!(
        qualify(
            &Candidate::Flavor("See https://private.example".to_owned()),
            &current,
            &projected,
            &known_offers,
        ),
        Err(Refusal::ForbiddenFlavorContent),
    );
    assert_eq!(
        qualify(
            &Candidate::Flavor("  ".to_owned()),
            &current,
            &projected,
            &known_offers,
        ),
        Err(Refusal::EmptyFlavor),
    );
    let oversized = "x".repeat(161);
    assert_eq!(
        qualify(
            &Candidate::Flavor(oversized),
            &current,
            &projected,
            &known_offers,
        ),
        Err(Refusal::FlavorTooLong),
    );
    let five = vec![Candidate::Flavor("mood".to_owned()); 5];
    assert_eq!(
        qualify_envelope(&five, &current, &projected, &known_offers),
        Err(Refusal::TooManyClauses),
    );
}
```

The `projected` map is trusted input representing upstream listener authorization;
this fixture does not implement projection, source/access revision resolution,
provider-input construction, session commit, persistence, localization, captions
or audio. `Qualified::IntentProposal` deliberately expresses no legal/success
outcome and changes no state. Production qualification also needs audience and
claim/source/access revisions, attribution, allowlisted names/numbers/negation,
locale policy, exact schemas and measured bounds, hidden-state noninterference,
stale async job fencing, disclosure-before-release, safe fallback matching and
independent adversarial evaluation. Natural-language flavor remains epistemically
uncertain even when these structural checks pass.

## Acceptance and unresolved production gates

The original acceptance is unchanged: (1) define facts versus creative
expression; (2) provide actual source/build-bound evidence for this named outcome,
with unsupported, pending, failed and unperformed checks explicit. The literal
fixture documents the narrow contract only; its compile/run is not production
acceptance or proof of integrated game behavior.

Production types and source/catalog/locale revisions remain G03/G07/G10/G11
gates. Freeze concrete byte/item/time limits from representative device/load
measurements; choose versioned claim and audience bindings; verify paired hidden
secret states produce identical unauthorized contexts/prompts/retrieval/captions/
cues; test authorized reveal commit-before-publication, attributed lies, false
beliefs, contradictory rumors, names/numbers/negation, encoded instructions,
stale results, unsupported claims, refusal and prepared fallback. Freeze
multilingual adversarial fixtures and report denominator, false accepts/rejects,
delay and residual error. Independent frontier review must exercise actual
output boundaries. No application source or production API is implemented here;
no browser, audio, provider, native/WASM workspace or independent output check was
run. Those checks remain pending; no live provider or device claims are made.

## Source basis and input identity

Decision grounded in the frozen attempt inputs:

- `planning/subsystem-architecture.md` — Design; crate ownership and dependency
  map, especially `df-ai`, `df-world`, `df-rules`, `df-knowledge`,
  `df-interaction`, `df-engine`, `df-session`, `df-provider-api`, `df-providers`,
  `df-api` and client crates.
- `planning/subsystem-interfaces.md` — common contract rules; pure domain and
  content; `df-ai::AiService::run`; audience projection, session authority and
  typed provider/result boundaries.
- `planning/generated-content.md` — candidate validation/admission and rejection;
  model output cannot become live executable rules or canonical inventory.
- `planning/interaction-engine.md` — persistent truth/belief distinction and
  “Grounded creative speech and listener-safe generation”.
- `planning/g11-d02-listener-safe-claim-policy.md` — existing reviewed
  listener-safe envelope reused as the canonical policy.
- `planning/g11-d03-native-memory-retrieval-policy.md` — native retrieval and
  private context boundary; does not grant generation or truth authority.
- `planning/runtime-reliability.md`, `planning/storage-architecture.md`,
  `planning/observability.md`, and `planning/shared-contract-waves.md` — job,
  replay, recovery, privacy and shared gate constraints.
- `development/backlog-catalog.json` — source objective and acceptance.
- `AGENTS.md`, `planning/coding-style.md`, ADR 0001–0005,
  `rust-toolchain.toml` and `rustfmt.toml` — workflow and fixture rules.

The issued worker-context JSON freezes SHA-256 identity for all governing inputs;
current hashes were checked before editing. The concurrent policies are not
accepted dependencies. D01 introduces no provider, UI, browser, workspace,
protocol, storage or shared type changes.
