# C-df-ai D03: Canonical speech claim, outcome, number, and negation slots

Status: Design decision. This document selects a deterministic contract direction; production Rust types, source catalogs, localized templates, and measured limits remain gated.

## Decision

A claim-bearing speech clause is publishable only when its claim identity and source/access basis resolve to an approved canonical fact, its outcome is explicitly selected from that fact's approved alternatives, and each emitted entity/name, quantity, and polarity/negation slot exactly corresponds to an approved value for that outcome. Render constrained meaning from reviewed locale templates and typed slot values. Model-generated wording may select only an already approved rendering variant; it cannot add, omit, paraphrase, or alter a constrained fact. A missing, ambiguous, unsupported, stale, or mismatching slot rejects the whole bounded response before any caption or audio is released. Keep creative flavor explicitly noncanonical and qualified under the existing listener-safe claim policy.

This is the selected deterministic approved basis for the named outcome. Canonical fact truth remains owned by the authoritative game/source transition, not `df-ai`. `df-ai` qualifies a candidate expression against immutable input facts and returns a typed qualified or refusal result. `df-session` supplies the authorized per-listener expression context and owns basis/version revalidation and publication after any required reveal is committed. `df-rules` / the owning source-backed subsystem determines mechanical outcome. No model critic, probability score, or provider qualification overrides this rule.

The slots are semantic bindings, not an arbitrary schema-count target: claim ID + outcome ID bind the fact and result; entity/name slots bind referenced canonical identities and their approved localized names; quantity slots bind exact approved value and unit; polarity slots bind affirmative/negative meaning. A template owns word order and locale grammar, so a natural-language negation token is not inferred by searching free text. Do not treat quantity formatting, translated forms, or morphology as new semantic values.

## Authority and integration

- `df-model` owns the single persisted canonical fact/decision representation and versioned IDs, as shared by the existing subsystem plan. D03 creates no competing fact or speech authority.
- The `df-ai` feature adapter owns `qualify_expression` behavior at the AI output edge: schema/allowlist checks and deterministic slot-to-approved-template comparison. It returns a typed refusal for unsupported, altered, or unverifiable claims. It does not make provider, source, or game-state truth.
- `df-session` owns authorized audience projection, access/source generation, accepted job basis, stale-result rejection, reveal commit ordering, and final delivery fencing.
- The authoritative source owner (`df-rules` for mechanics; `df-world` or other already assigned subsystem for its canonical facts) creates committed outcomes. `df-engine` composes proposals and `df-session` commits. `df-persistence` records the existing fact/decision and pinned revisions; it does not interpret generated speech.
- Captions and TTS consume the same fully qualified clause sequence. Until streaming can prove each clause stable against later retraction, buffer the whole bounded response. On refusal or stale basis, select prepared source-safe wording with matching captions/audio or expose an explicit safe gap; never publish raw tokens first.

Provider input uses only the listener-safe projected `ExpressionContext` already selected by `planning/g11-d02-listener-safe-claim-policy.md`. It excludes hidden true facts and secret identifiers. The qualifying result binds the exact source/content/locale/policy/access generations supplied by the request. Recheck at the session publication edge.

## Alternatives

- Let the model paraphrase approved facts and run a semantic critic afterward: rejected. A second probabilistic judgment cannot make altered numbers, negation, names, or claim scope deterministic or repair already released audio/captions.
- Permit unrestricted text when a claim ID is attached: rejected. Metadata does not constrain the words listeners receive.
- Reject all generated expression and use only fixed full sentences: rejected for this boundary because it removes the already-planned expressive flavor capability. Constrained facts still use reviewed deterministic templates; flavor stays noncanonical and qualified.
- Let `df-ai` create/update canonical facts: rejected because it duplicates game/source authority and violates committed-outcome ownership.

## Finite std-only contract example

This example shows equality against an approved outcome and exact slots, including explicit altered-quantity, negation-flip, unknown-claim, and stale-basis refusals. It is illustrative, not a production API, localization implementation, or proof that a real provider, application, or listener-facing output passed. Strings represent already resolved typed slots solely to keep the example std-only and finite.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
struct ClaimId(&'static str);

#[derive(Clone, Debug, Eq, PartialEq)]
struct OutcomeId(&'static str);

#[derive(Clone, Debug, Eq, PartialEq)]
struct Slots {
    subject: &'static str,
    quantity: u8,
    unit: &'static str,
    affirmed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Clause {
    claim: ClaimId,
    outcome: OutcomeId,
    slots: Slots,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Approved {
    claim: ClaimId,
    outcome: OutcomeId,
    slots: Slots,
    template: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Refusal {
    StaleBasis,
    UnsupportedClaimOrOutcome,
    AlteredSlots,
}

fn qualify(
    current_basis: bool,
    clause: &Clause,
    approved: &[Approved],
) -> Result<&'static str, Refusal> {
    if !current_basis {
        return Err(Refusal::StaleBasis);
    }
    let fact = approved
        .iter()
        .find(|fact| fact.claim == clause.claim && fact.outcome == clause.outcome)
        .ok_or(Refusal::UnsupportedClaimOrOutcome)?;
    if fact.slots != clause.slots {
        return Err(Refusal::AlteredSlots);
    }
    Ok(fact.template)
}

fn main() {
    let approved = [Approved {
        claim: ClaimId("gate-7"),
        outcome: OutcomeId("opened"),
        slots: Slots {
            subject: "north gate",
            quantity: 1,
            unit: "gate",
            affirmed: true,
        },
        template: "The north gate is open.",
    }];
    let accepted = Clause {
        claim: ClaimId("gate-7"),
        outcome: OutcomeId("opened"),
        slots: approved[0].slots.clone(),
    };
    assert_eq!(
        qualify(true, &accepted, &approved),
        Ok("The north gate is open.")
    );

    let mut changed_number = accepted.clone();
    changed_number.slots.quantity = 2;
    assert_eq!(
        qualify(true, &changed_number, &approved),
        Err(Refusal::AlteredSlots)
    );

    let mut flipped_negation = accepted.clone();
    flipped_negation.slots.affirmed = false;
    assert_eq!(
        qualify(true, &flipped_negation, &approved),
        Err(Refusal::AlteredSlots)
    );

    let unknown_claim = Clause {
        claim: ClaimId("secret-9"),
        ..accepted.clone()
    };
    assert_eq!(
        qualify(true, &unknown_claim, &approved),
        Err(Refusal::UnsupportedClaimOrOutcome)
    );
    assert_eq!(
        qualify(false, &accepted, &approved),
        Err(Refusal::StaleBasis)
    );
}
```

The example's `current_basis` flag stands in for trusted session/source/access checks; the tiny approved slice stands in for versioned committed facts and reviewed templates. Production must use typed IDs and quantities/units, locale-aware deterministic templates (including grammatical negation), per-listener authorization, source and policy digests, bounded schemas/text, instruction/tool/URL filters for flavor, stale job rejection, safe diagnostics, and matching caption/audio fallback. It has no provider, network, persistence, session state, or real speech-output path.

## Acceptance and unresolved gates

The original acceptance is preserved: canonical facts have a deterministic approved basis; the named outcome has actual source/build-bound evidence, while unsupported, pending, failed, and unperformed checks remain explicit. This document decides the contract direction and supplies a finite executable boundary example. Its standalone check evidence does not establish integrated behavior.

G03 must freeze concrete shared IDs, outcome/slot/error types, codecs, and version migration; G07 must freeze applicable source/catalog and locale/template corpus plus supported grammatical forms; G10 must bind replay/decision records to the exact approved slots, templates, and source/policy revisions; G11 must resolve adversarial policy and calibrated bounds. The subsystem owners must review this adapter against the integrated canonical claim policy before production implementation. Browser, native/WASM, provider, speech, and listener-facing checks remain unperformed, and phone testing is deferred. No arbitrary field counts, byte caps, or quality thresholds are claimed as measured here.

Required production evidence includes paired hidden-secret states yielding identical unauthorized expression contexts/provider inputs; accepted committed result; changed number/unit, name/entity, outcome, and negation refusals; unsupported and stale claims; contradictory belief versus canonical truth; encoded instruction/extraction attempts; locale fixtures; captions/audio parity; and no raw-token publication. Report exact fixture denominator, false accepts/rejects, timing and residual gaps. Independent frontier review must exercise the integrated boundary and observed output; the example alone cannot satisfy that gate.

## Source basis

- `planning/interaction-engine.md`, “Grounded creative speech and listener-safe generation”: approved `SpeechActPlan`, `ExpressionContext`, `SpeechEnvelope`, `GroundedClause` claim/outcome/entity/name/number slots, deterministic constrained rendering, and full-response buffering.
- `planning/g11-d02-listener-safe-claim-policy.md`: canonical source/access basis and listener-safe projection; D03 extends its clause qualification contract and does not replace it.
- `planning/subsystem-interfaces.md`: `df-model` shared truth records, pure AI/provider proposal boundary, session commit/publication authority, typed failures, audience access and versioned builds.
- `planning/subsystem-architecture.md` and `planning/runtime-directors.md`: crate authority separation, pure proposals and session-owned commit.
- `planning/generated-content.md`: model output remains a candidate; pinned provenance, stale rejection, privacy and explicit failure.
- `planning/implementation-roadmap.md`: G03/G07/G10/G11 unresolved gates and independent integration acceptance.
- `planning/coding-style.md`, `rustfmt.toml`, `rust-toolchain.toml`, `AGENTS.md`, ADR 0001–0005: contract style, pinned toolchain, scoped source and evidence workflow.
- Frozen task `B-C-df-ai-D03-a1`: “Define claim outcome number and negation slots”; original acceptance and named `df-ai` feature adapter.
