# D02: Listener-safe expression context

Status: design decision; concrete production types and integrated behavior remain pending.

## Decision

`df-ai::ExpressionContext` is the listener-specific, provider-facing projection for one approved `SpeechActPlan`. Build it only after `df-knowledge`/`df-interaction` have selected the listener-authorized claims and the session has supplied the current audience/access basis. It contains the bounded permitted claims and their exact canonical slots, approved attributed speech-act metadata, locale, allowed flavor style, and only audience-cleared retrieval/style references. It does not contain NPC private reasoning, private beliefs or memories, hidden claim IDs or text, unrestricted transcripts, or server-only rationale. Those data are not fields of this context and are not serialized into prompts, retrieval queries, captions, audio, or asset cues.

The context is constructed separately for each listener, then schema/size validated before any provider request. Access filtering precedes serialization and retrieval; a trace/correlation ID or room membership never grants access. Paired requests with the same listener basis and permitted claims but differing only in unrelated hidden NPC knowledge must produce byte-identical contexts and provider inputs. If the authorized projection is missing, stale, revoked, or ambiguous, do not broaden it: return typed unavailable/stale/refused and use the prepared expression for the already-approved outcome or an explicit uncertain/rejected result with a visible silent gap.

The provider may choose expression over the permitted envelope. `df-ai::qualify_expression` (the existing G11-D02 policy owner) must validate the complete bounded response against source/access revisions, authorized claim IDs and attribution, outcome/name/number/negation slots, locale, style and content restrictions before any text or audio is released. Canonical outcomes and quantities render from reviewed templates or deterministic slots. Creative flavor remains explicitly noncanonical; structural checks do not prove arbitrary prose is semantically harmless. Buffer the complete response until qualification succeeds. Do not publish raw tokens or rely on a second model's assurance.

This is the D02 `df-ai` use-case contract for `ExpressionContext`, not a second claim policy, public API freeze, authority, or implementation. It reuses [G11-D02 listener-safe claim envelope](g11-d02-listener-safe-claim-policy.md) for projection and qualification, and [G11-D03 retrieval ownership](g11-d03-native-memory-retrieval-policy.md) for authorized bounded retrieval. It introduces no provider, database, job, or session-state owner.

## Ownership and integration hooks

| Owner | Responsibility at this boundary |
| --- | --- |
| `df-knowledge` | Supplies listener-authorized claims/beliefs and provenance from already bounded retrieval; private/unpermitted material is excluded before it reaches this boundary. |
| `df-interaction` | Supplies the approved speech act, attribution, permitted lie/false-belief claim, and audience basis. An approved lie is attributed speech, never a canonical fact. |
| `df-session` / engine | Owns current run, listener/access generation, commit/reveal ordering, basis revalidation, and final audience projection. Commit any approved reveal before releasing its expression. |
| `df-ai` | Owns the listener-safe context adapter, existing `AiService::run` call, bounded generation and `qualify_expression`; returns typed validated expression/proposal or refusal, never mutates session state. |
| `df-media` | Consumes only committed, audience-cleared presentation demand and validated speech; preserves matching captions/audio or explicit silent-gap fallback. |
| `df-api` / `df-client` | Wire and rendering owners; receive only audience-projected results and do not hide private state client-side. |

Integration hook: at the existing `df-ai::AiService::run(context, AiJob)` boundary, pass the projected `ExpressionContext` as the only narrative context input. Revalidate its basis after async completion and before qualification/publication. Claim/retrieval authority is reused; no duplicate production API or independent disclosure decision is introduced here.

## Alternatives and rationale

- **Pass the full NPC state and ask a critic to redact it:** rejected because disclosure already occurred at provider input, and a critic cannot restore secrecy or grant claim rights.
- **Filter only after generation or in the client:** rejected because hidden text/IDs can leak through provider prompts, retrieval, logs, captions, cues, or wire payloads before client rendering.
- **Use only fixed templates:** safe but unnecessarily removes voice and atmosphere. Keep canonical meaning in deterministic slots and permit bounded flavor subject to qualification.
- **Allow unrestricted prose to define facts, promises, locations, or outcomes:** rejected because text cannot become canonical world/rules state. Such additions are uncertain/rejected or sent for review.
- **Stream tokens as they arrive:** deferred until stable-clause and locale qualification demonstrate that no later output can require retraction. Default is whole-response buffering.

## Literal std-only boundary example

This finite illustration exercises the context boundary and refusal cases. The projection input type has no private-reasoning field, so that data cannot be copied into the constructed context through this function. `authorized_claims` represents the existing trusted per-listener projection, not a new authorization mechanism. The checks are structural examples, not a general natural-language secrecy detector or production API.

```rust
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ClaimId(&'static str);

#[derive(Clone, Debug, Eq, PartialEq)]
struct PublicClaim {
    text: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExpressionContext {
    claims: BTreeMap<ClaimId, PublicClaim>,
    locale: &'static str,
    allowed_style: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Clause {
    Grounded(ClaimId),
    Flavor(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Refusal {
    StaleBasis,
    UnknownOrUnauthorizedClaim,
    EmptyFlavor,
    FlavorTooLong,
    ForbiddenFlavorContent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Qualified {
    text: String,
    flavor_is_canonical: bool,
}

fn project(
    basis_current: bool,
    authorized_claims: BTreeMap<ClaimId, PublicClaim>,
    locale: &'static str,
    allowed_style: &'static str,
) -> Result<ExpressionContext, Refusal> {
    if !basis_current {
        return Err(Refusal::StaleBasis);
    }
    Ok(ExpressionContext {
        claims: authorized_claims,
        locale,
        allowed_style,
    })
}

fn qualify(clauses: &[Clause], context: &ExpressionContext) -> Result<Qualified, Refusal> {
    let mut text = String::new();
    let mut has_flavor = false;
    for clause in clauses {
        let rendered = match clause {
            Clause::Grounded(id) => context
                .claims
                .get(id)
                .ok_or(Refusal::UnknownOrUnauthorizedClaim)?
                .text,
            Clause::Flavor(value) => {
                if value.trim().is_empty() {
                    return Err(Refusal::EmptyFlavor);
                }
                if value.len() > 80 {
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
    Ok(Qualified {
        text,
        flavor_is_canonical: !has_flavor,
    })
}

fn public_claims() -> BTreeMap<ClaimId, PublicClaim> {
    let mut claims = BTreeMap::new();
    claims.insert(
        ClaimId("outcome-7"),
        PublicClaim {
            text: "The gate is open.",
        },
    );
    claims
}

fn main() {
    // Hidden NPC reasoning varies between these situations but is not an input
    // to project(), so the provider-facing context is identical in both.
    let hidden_a = "Mara secretly knows the keeper is a spy.";
    let hidden_b = "Mara secretly suspects the keeper is innocent.";
    assert_ne!(hidden_a, hidden_b);
    let first = project(true, public_claims(), "en", "quiet").unwrap();
    let second = project(true, public_claims(), "en", "quiet").unwrap();
    assert_eq!(first, second);

    let accepted = qualify(
        &[
            Clause::Grounded(ClaimId("outcome-7")),
            Clause::Flavor("Mara lowers her voice.".to_owned()),
        ],
        &first,
    )
    .unwrap();
    assert_eq!(accepted.text, "The gate is open. Mara lowers her voice.");
    assert!(!accepted.flavor_is_canonical);

    assert_eq!(
        project(false, public_claims(), "en", "quiet"),
        Err(Refusal::StaleBasis),
    );
    assert_eq!(
        qualify(&[Clause::Grounded(ClaimId("secret-9"))], &first),
        Err(Refusal::UnknownOrUnauthorizedClaim),
    );
    assert_eq!(
        qualify(&[Clause::Flavor("  ".to_owned())], &first),
        Err(Refusal::EmptyFlavor),
    );
    assert_eq!(
        qualify(&[Clause::Flavor("See https://private.example".to_owned())], &first),
        Err(Refusal::ForbiddenFlavorContent),
    );
}
```

The finite checks show typed stale-basis, unauthorized-claim, empty-flavor, and forbidden-content refusals, and accepted grounded text plus noncanonical flavor. Production must also bind audience/access/source versions, validate attribution and exact outcome/name/number/negation slots with the approved locale, bound encoded size and provider/retrieval work, redact diagnostics, reject stale async completion, and exercise paired hidden-secret contexts/prompts/captions/cues. These checks do not prove semantic noninterference for arbitrary language.

## Acceptance, evidence, and unresolved gates

The original acceptance remains: private NPC reasoning is absent from the public prompt; the named outcome has source/build-bound evidence and unsupported, pending, failed, and unperformed checks are explicit. This submission establishes a source-backed design boundary and finite illustrative executable contract only. It does not prove a production prompt or running game behavior.

Concrete type and schema freeze remains under G03/G10/G11; approved source and locale corpora and adversarial fixtures remain under G07/G11; integrated session/AI/provider/media wiring, native/WASM workspace checks, and actual provider-input inspection are unperformed. Independent frontier review of the exact boundary and paired hidden-secret cases is still required. Browser/device, audio, provider, and user-facing runtime checks were not run. No browser support or device compatibility is claimed.

A production acceptance run must capture exact build/config/source identity and verify byte-identical listener contexts and provider inputs under paired irrelevant hidden-secret states, absence of private values across prompt/retrieval/log/caption/audio/cue boundaries, authorized reveal after commit, permitted attributed lies without truth mutation, contradictory claims, entity/number/negation mismatches, encoded extraction/instructions, stale access/run basis, bounded output, and prepared/rejected/silent-gap fallback. Record fixture denominator, false accepts/rejects, latency and residual error; zero observed errors are not a guarantee.

The literal is intended for pinned Rust 1.98.1 edition 2024, repository `rustfmt.toml`, `rustfmt --check`, `rustc -D warnings`, and finite assertion execution. Exact guarded receipts, source/literal hashes, unperformed checks, and integration handoff are recorded separately in this attempt's evidence. A successful standalone run is not integrated behavior or independent approval.

## Source basis

- `planning/interaction-engine.md`, “Grounded creative speech and listener-safe generation”: listener-specific projection, hidden facts excluded from prompt/retrieval/style/cues, paired-state equivalence, grounded slots, qualification, buffering and fallback.
- `planning/g11-d02-listener-safe-claim-policy.md`: existing claim envelope, owner split, refusal semantics, and required production acceptance; reused rather than duplicated.
- `planning/g11-d03-native-memory-retrieval-policy.md`: existing `df-session` native retrieval job and `df-knowledge` pure consumer ownership; only already-authorized bounded results can contribute.
- `planning/subsystem-interfaces.md`, `df-ai::AiService::run` and grounded speech references: bounded permitted context, locale, known offers/schema/mode; validated result with no session mutation; publication waits for validation.
- `planning/subsystem-architecture.md`: `df-ai` public service owner and existing dependencies.
- `planning/generated-content.md`: generated output is candidate-only, private lore follows audience access, failure is explicit.
- `AGENTS.md`, `planning/coding-style.md`, `planning/shared-contract-waves.md`, runtime/storage/observability plans, and ADR 0001–0005: scoped source-backed design, typed boundaries, versioning, privacy, independent review and honest verification.

Governing source hashes and this literal's hash are retained in this task's evidence directory; current inputs matched the issued hashes before drafting.
