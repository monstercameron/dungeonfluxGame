# Listener-safe claim envelope

Status: design contract for G11-D02; production types and integration pending.

## Decision

Every player-visible or audible utterance is qualified for its intended listener
before any provider prompt, retrieval result, caption, audio, or asset cue is built.
The server first projects an `ExpressionContext` from claims that this listener is
authorized to receive. A trace ID, shared room membership, or the fact that a
claim exists in canonical world state grants no disclosure right. Two contexts
that differ only in an unrelated hidden fact must be identical for this listener.

Keep three meanings distinct:

* **Canonical fact and outcome:** approved world/rules truth and committed result.
  Only the authoritative owner changes these. Exact names, quantities, negation,
  and outcomes are rendered from reviewed localized templates or deterministic
  slots; a language model cannot change their meaning.
* **Attributed claim:** what a named speaker says, believes, remembers, or reports.
  It points to a permitted claim ID and attribution. A false belief or deliberate
  lie may be expressed only when the interaction policy has approved that speech
  act. It never changes the underlying fact or discloses its hidden counterpart.
* **Creative expression:** bounded, explicitly noncanonical flavor attached to
  an already permitted interaction. It may add voice, gesture, and atmosphere;
  it cannot establish durable lore, a location, quest, resource, rule, successful
  check, or player commitment. Natural-language flavor is not mechanically
  proven merely because structural checks pass.

The proposed envelope is an ordered, bounded list of grounded clauses and flavor
clauses, bound to listener/audience basis, claim/source/access revisions, locale,
and interaction outcome. Grounded clauses carry claim and outcome/name/number
slots with attribution. Flavor clauses carry a noncanonical marker, allowed style,
and local text. Reject the whole proposed response on an unknown or unauthorized
claim, stale basis, unsupported outcome, slot mismatch, malformed schema, excess
size, forbidden URL/tool/instruction content, or unqualified claim-bearing addition.
Do not emit raw generated tokens while qualification is pending. On refusal,
return a fixed source-safe prepared expression matching the approved outcome, or
an explicit uncertain/rejected result with a visible silent-gap fallback. Never
rewrite the refusal as success. Commit an approved reveal before releasing its
text or audio.

## Boundary and ownership

`df-world` owns canonical causality; `df-rules` owns mechanical authority;
`df-knowledge` owns observation, belief, memory, provenance, and recipient access;
`df-interaction` owns motivated speech acts and approved deceit; `df-intent` owns
input interpretation and plans; `df-narrative` may propose opportunities but
cannot force disclosure or success; encounter/combat proposals remain subject to
legal source and perception checks. `df-ai` may express a permitted envelope and
qualify it, but its critic is supplementary evidence, not authority. The engine
composes typed proposals and the session owner revalidates and commits. Projection
filters before provider input and serialization, never only in the client.

This document defines a policy boundary, not a production API or integrated
behavior. Domain code stays pure: it takes explicit basis/revisions and returns
bounded typed outcomes, without clocks, database/provider/socket I/O or telemetry
SDK calls. Native consumers own effects and telemetry. Wire codecs remain with
`df-api`/`df-client`/`df-persistence`.

## Alternatives considered

1. **Let the model answer from full server context, then ask a second model to
   check it.** Rejected: hidden context already crossed the disclosure boundary,
   and a critic cannot authorize claims or guarantee semantic equivalence.
2. **Allow unrestricted prose and parse it into canonical state.** Rejected:
   arbitrary text cannot safely create facts, rules, promises, or outcomes.
3. **Use only fixed dialogue templates.** Safe but unnecessarily constrains voice
   and atmosphere. Retain deterministic templates for claims and outcomes while
   allowing bounded, noncanonical flavor after listener projection and
   qualification.
4. **Stream generated clauses immediately.** Deferred: a later clause or locale
   check could require retracting already exposed content. Buffer the complete
   bounded response until stable-clause qualification is independently supported.

## Literal std-only contract example

The following standalone Rust example demonstrates the narrow structural
contract: only audience-projected claim IDs can be rendered; unknown claims and
obvious prohibited flavor are refused; the result never promotes flavor to fact.
It is illustrative and intentionally does not claim semantic validation of
arbitrary prose or production integration.

```rust
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ClaimId(&'static str);

#[derive(Clone, Debug, Eq, PartialEq)]
enum Clause {
    Grounded(ClaimId),
    Flavor(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Refusal {
    UnknownOrUnauthorizedClaim,
    EmptyFlavor,
    FlavorTooLong,
    ForbiddenFlavorContent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Rendered {
    text: String,
    flavor_is_canonical: bool,
}

fn qualify(
    clauses: &[Clause],
    projected_claims: &BTreeMap<ClaimId, &'static str>,
) -> Result<Rendered, Refusal> {
    let mut text = String::new();
    let mut has_flavor = false;

    for clause in clauses {
        let rendered = match clause {
            Clause::Grounded(id) => *projected_claims
                .get(id)
                .ok_or(Refusal::UnknownOrUnauthorizedClaim)?,
            Clause::Flavor(value) => {
                if value.trim().is_empty() {
                    return Err(Refusal::EmptyFlavor);
                }
                if value.len() > 160 {
                    return Err(Refusal::FlavorTooLong);
                }
                let lower = value.to_ascii_lowercase();
                if ["http://", "https://", "<tool>", "ignore previous"]
                    .iter()
                    .any(|marker| lower.contains(marker))
                {
                    return Err(Refusal::ForbiddenFlavorContent);
                }
                has_flavor = true;
                value.as_str()
            }
        };
        if !text.is_empty() {
            text.push(' ');
        }
        text.push_str(rendered);
    }

    Ok(Rendered {
        text,
        flavor_is_canonical: !has_flavor,
    })
}

fn main() {
    let mut public = BTreeMap::new();
    public.insert(ClaimId("outcome-7"), "The gate is open.");
    let hidden = ClaimId("secret-9");

    let accepted = qualify(
        &[
            Clause::Grounded(ClaimId("outcome-7")),
            Clause::Flavor("Mara lowers her voice.".to_owned()),
        ],
        &public,
    )
    .expect("projected fact and noncanonical flavor are allowed");
    assert_eq!(accepted.text, "The gate is open. Mara lowers her voice.");
    assert!(!accepted.flavor_is_canonical);

    assert_eq!(
        qualify(&[Clause::Grounded(hidden)], &public),
        Err(Refusal::UnknownOrUnauthorizedClaim),
    );
    assert_eq!(
        qualify(
            &[Clause::Flavor("See https://private.example".to_owned())],
            &public,
        ),
        Err(Refusal::ForbiddenFlavorContent),
    );
    assert_eq!(
        qualify(&[Clause::Flavor("  ".to_owned())], &public),
        Err(Refusal::EmptyFlavor),
    );
}
```

The trusted `projected_claims` map stands for an upstream, authorized per-listener
projection; constructing that map is not modeled here. Production qualification
also needs source/access version binding, audience identity, stable slot and
attribution checks, locale-aware number/name/negation checks, schema limits,
provider-input filtering, stale-result rejection, and matching caption/audio
fallbacks. The small string checks above are examples of explicit refusal cases,
not a comprehensive injection detector or proof that flavor has no factual
implication.

## Acceptance and unresolved production gaps

Retain the original acceptance unchanged:

1. Define **facts versus creative expression**.
2. The named outcome has actual source/build-bound evidence; unsupported, pending,
   failed, and unperformed checks remain explicit.

For production acceptance, freeze concrete types and source/catalog/locale
revisions under G03/G07/G10/G11. Exercise paired hidden-secret states and verify
identical unauthorized expression contexts, prompts, retrieval, captions, and
cues; authorized disclosure after commit; attributed lies and false beliefs that
do not mutate truth; contradictory rumors; entity/name/number/negation changes;
encoded instructions and extraction attempts; stale async basis; unsupported
claims; bounded output; and refusal/fallback. Freeze multilingual adversarial
fixtures and report denominator, false accepts/rejects, delay, and residual error.
Zero observed errors is not a guarantee of zero future errors. Independent frontier
review must exercise the real output boundary; a code review, test helper, or
model critic alone does not establish listener trust or running behavior.

Known pending items: G03 production contract/types, G07 source and locale corpus,
G09 evaluator/tool availability, G10 replay/basis schemas, G11 cross-owner policy
resolution and bounds, and later production integration. The standalone literal
example passed pinned rustfmt and rustc checks and its finite assertions executed
successfully under the admitted bounded runner. No application source, provider
flow, browser, audio, native/WASM workspace check, or independent output
evaluation was run for this design change. Passing the standalone contract does
not establish integrated or listener-facing behavior.

## Source basis

Frozen inputs are from attempt `B-G11-D02-a1`, source revision
`9def9b845531e5cc589a28343b822b4664bd03fd`, with hashes captured in its
`brief.json`. Primary decisions: `planning/interaction-engine.md` (persistent
social/epistemic model; grounded creative speech and listener-safe generation),
`planning/runtime-directors.md` (pure subsystem boundaries and G10/G11/G12 gates),
`planning/subsystem-interfaces.md` (typed pure-domain and session ownership),
`planning/long-horizon-state.md` (bounded memory, privacy and replay gaps),
`planning/generated-content.md` (candidate-only output, privacy and refusal), and
`planning/implementation-roadmap.md` (dispatch, dependencies, slice owners and
independent acceptance). Workflow decisions follow `AGENTS.md`, `planning/coding-style.md`,
and ADR 0001–0005. The brief’s input hashes are the source identity; current files
were checked against those hashes before editing.
