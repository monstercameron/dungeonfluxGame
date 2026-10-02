# G12-D01: Consented observation and spotlight scope

Date: 2026-10-01
Status: Policy decision; production contracts, limits and calibration pending

## Decision and preserved acceptance

The named outcome remains **“Define opt-in observation and spotlight scope”**;
its original expected result is **“no ambient emotion.”** This policy defines
the permissible observation inputs and voluntary spotlight behavior for the
existing `df-experience`, `df-tempo`, `df-presentation` and `df-media` owners.
It adds no subsystem or second implementation owner. A pure director may consume
only bounded, explicitly supplied, authorized inputs and return a typed proposal;
it does not observe people or acquire consent itself.

## Alternatives considered

* **Infer mood or boredom from speech, silence, inactivity, presence, or action
  volume.** Rejected. The tempo policy says inactivity is not proof of boredom
  and disallows covert microphone monitoring, emotion inference, and penalties
  based on local muting or cancellation. G12's named outcome also requires no
  ambient emotion. A model-produced affect label would still be an inference
  from those signals, not a permitted observation. (Sources: [tempo engine —
  Audience, preferences and observations](tempo-engine.md#audience-preferences-and-observations);
  [implementation roadmap — G12](implementation-roadmap.md#prerequisite-decisions).)
* **Treat joining, enabling audio, or silence as implicit consent.** Rejected.
  The spotlight refinement specifies an explicit `SpotlightPreference` and
  voluntary opportunities; subsystem contracts require authorization at
  privileged boundaries and revalidation when projecting private data. These
  actions do not express a purpose-specific preference or permission to observe.
  (Sources: [feature refinement — Rumors, spotlight and grounded encounters](feature-refinement.md#rumors-spotlight-and-grounded-encounters);
  [subsystem interfaces — Common contract rules](subsystem-interfaces.md#common-contract-rules)
  and [Identity and session ownership](subsystem-interfaces.md#identity-and-session-ownership).)
* **Permit only direct feedback and discard every activity count.** Rejected as
  the complete policy. The runtime-director contract permits bounded committed-
  activity windows alongside voluntary feedback. Keep that narrow aggregate for
  pacing opportunity context while forbidding it from representing mood,
  willingness, or attention. (Sources: [runtime directors — Pure public
  boundaries](runtime-directors.md#pure-public-boundaries);
  [feature refinement — Rumors, spotlight and grounded encounters](feature-refinement.md#rumors-spotlight-and-grounded-encounters).)
* **Equalize speaking turns or automatically spotlight quiet participants.**
  Rejected. The feature acceptance says fairness measures opportunities and
  voluntary feedback, not equal forced speaking shares or an unproven boredom
  classifier. Offer a credible choice with decline and ignore paths instead.
  (Source: [feature refinement — Rumors, spotlight and grounded encounters](feature-refinement.md#rumors-spotlight-and-grounded-encounters).)
* **Use spotlight consent to disclose a personal hook to the whole room.**
  Rejected. The refinement requires separate disclosure scope for player/backstory
  secrets, and projection contracts authorize audience scope before serialization
  and asset delivery. A participation preference does not grant that access.
  (Sources: [feature refinement — Rumors, spotlight and grounded encounters](feature-refinement.md#rumors-spotlight-and-grounded-encounters);
  [interaction engine — Grounded creative speech and listener-safe generation](interaction-engine.md#grounded-creative-speech-and-listener-safe-generation).)

“No ambient emotion” is a hard refusal boundary. DungeonFlux must not infer,
estimate, classify, rank, or act on a player's emotion, boredom, attention,
engagement, fatigue, enthusiasm, discomfort, or social willingness from ambient
audio, microphone state, speech prosody, camera/video, biometrics, typing or
pointer cadence, silence, inactivity, presence, response latency, action volume,
or other behavioral proxies. A model/provider output or a score assembled from
those signals is still an invalid inference. Consent to a permitted observation
does not authorize emotion inference. No inferred affect may alter pacing,
NPC behavior, spotlight selection, difficulty, rewards, access, or another
participant's experience.

## Permitted observations and consent

Only the following bounded inputs may inform macro pacing or opportunity
recommendation:

* Committed, in-game interaction counts and elapsed **game** time, aggregated
  over a versioned `ActivityWindow` policy. Do not use wall-clock delay,
  inactivity, attendance, speaking duration, or message volume as a proxy for
  mood or willingness. An interaction count is a workload fact, not a person
  rating.
* Explicit, voluntary feedback supplied for a named purpose, such as “more
  chances for my character to act” or a direct pacing preference. Preserve its
  stated meaning; do not relabel it as an emotional state.
* A current, explicit `SpotlightPreference` for voluntary opportunities, scoped
  to the participant, campaign/run, purpose, and policy version. Preference is
  separate from permission to disclose a backstory or other personal fact.

Consent is an affirmative, authenticated choice for a specific purpose and
scope. It is off by default; joining, speaking, accepting a microphone prompt,
or enabling audio/video does not opt a person into observation or spotlighting.
The UI explains the signal categories and resulting use before opt-in. Record
the consent version and scope needed to enforce it, not a hidden profile. The
participant can withdraw or narrow consent at any time. Withdrawal fences new
observation reads, recommendations, projections and speculative asset demands
under the old consent generation; already committed game outcomes remain
committed. Expired, stale, missing, ambiguous or unauthorized consent fails
closed. A co-player's consent never covers another participant.

Collect the minimum window aggregate and explicit feedback needed for the
stated purpose. Apply bounded retention and access scope selected in G05/G12.
Do not place private feedback or personal backstory in a public cue, telemetry
field, provider prompt, or another participant's projection without a separate
authorized basis. Required gameplay, rules, rewards, and ordinary play remain
available after opt-out or withdrawal.

## Spotlight behavior and fairness

With active `SpotlightPreference`, `df-experience` may propose a bounded,
credible `SpotlightOpportunity` from an authorized character hook or committed
world fact. The proposal binds its source/basis, eligible audience, stable ID,
expiry and clear accept/decline/ignore path. It is only a recommendation. The
existing engine, interaction and rules owners validate any consequence; a
presentation cue cannot disclose a secret, create a check bonus, consume a turn,
force speech, change a rule, or commit an unaccepted action.

Ignoring or declining is a complete valid outcome. No repeated nagging,
punishment, inferred disinterest, lost reward, NPC relationship penalty, or
automatic turn spend follows. Repeated opportunities are bounded and
deduplicated. Revoking preference invalidates pending opportunities and their
uncommitted cues. Any personal hook has its own explicit disclosure scope, and
the audience projection is authorized before serialization, prefetch, or media
generation.

Assess fairness by auditing the distribution and expiry of offered opportunities,
consent/withdrawal handling, valid refusal paths, and optional direct feedback.
Do not optimize for equal forced speaking time, action volume, or acceptance
rate. A low response rate is not evidence of boredom. Keep opportunity and
feedback measures distinct; do not rank real people or reward spam. Compare
paired runs differing only in unauthorized hidden facts: public profiles, cues,
asset demands and observable outputs must remain equal.

## Boundaries and unresolved production gates

This policy preserves the rest of G12 rather than claiming it complete. Concrete
versioned request/result/error types and persisted shapes remain G03 work owned
by shared contract owners. G04 owns identity, device and capture capability
admission. G05 selects storage, retention and deletion behavior. G06 owns
telemetry scope and redaction. G08 selects provider/media, audio, device and
spend limits. G10/G11 retain authored-policy, provenance, knowledge, engine
validation and replay responsibilities. G12 still needs measured session/device
calibration; accessibility, audio and UI budgets; tempo inertia, fatigue and
stem timing; fair asset prefetch, expiry, reservations and waste limits; stable
clause/barge-in safety; and integrated latency/quality/cost evidence.

The illustrative Rust contract below demonstrates only this consent/refusal
boundary. It is not a production API, persistence model, wire schema, complete
privacy system, or substitute for the named owners' reviewed contracts. It uses
only the Rust standard library and deliberately refuses inferred emotion even
when general observation consent is active. The `current_generation` input
represents the trusted current consent generation supplied by the existing
authorization boundary; it is not caller-controlled. This small pure example
does not load consent or implement revocation propagation.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Purpose {
    Pacing,
    Spotlight,
}

#[derive(Clone, Copy, Debug)]
struct Consent {
    active: bool,
    purpose: Purpose,
    generation: u64,
}

#[derive(Clone, Copy, Debug)]
enum Input {
    CommittedInteraction,
    ExplicitPacingFeedback,
    InferredEmotion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Observation {
    CommittedInteraction,
    ExplicitPacingFeedback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    StaleConsent,
    ConsentRequired,
    WrongPurpose,
    InvalidInference,
}

fn admit(consent: Consent, current_generation: u64, input: Input) -> Result<Observation, Refusal> {
    if consent.generation != current_generation {
        return Err(Refusal::StaleConsent);
    }
    if !consent.active {
        return Err(Refusal::ConsentRequired);
    }
    if consent.purpose != Purpose::Pacing {
        return Err(Refusal::WrongPurpose);
    }

    match input {
        Input::CommittedInteraction => Ok(Observation::CommittedInteraction),
        Input::ExplicitPacingFeedback => Ok(Observation::ExplicitPacingFeedback),
        Input::InferredEmotion => Err(Refusal::InvalidInference),
    }
}

fn main() {
    let consent = Consent {
        active: true,
        purpose: Purpose::Pacing,
        generation: 4,
    };
    assert_eq!(
        admit(consent, 4, Input::CommittedInteraction),
        Ok(Observation::CommittedInteraction)
    );

    let withdrawn = Consent {
        active: false,
        generation: 5,
        ..consent
    };
    assert_eq!(
        admit(withdrawn, 5, Input::ExplicitPacingFeedback),
        Err(Refusal::ConsentRequired)
    );

    assert_eq!(
        admit(consent, 5, Input::CommittedInteraction),
        Err(Refusal::StaleConsent)
    );

    let spotlight_only = Consent {
        active: true,
        purpose: Purpose::Spotlight,
        generation: 6,
    };
    assert_eq!(
        admit(spotlight_only, 6, Input::ExplicitPacingFeedback),
        Err(Refusal::WrongPurpose)
    );

    assert_eq!(
        admit(consent, 4, Input::InferredEmotion),
        Err(Refusal::InvalidInference)
    );
}
```

## Preserved acceptance and evidence gaps

The original catalog criterion remains unchanged: no ambient emotion. Acceptance
must also retain the governing G12 and F42/F42-SPOTLIGHT-ACCEPT criteria: opt-in
permitted observations; voluntary credible opportunities; no boredom/emotion
inference; valid decline/ignore; consent revocation; hidden-state
noninterference; accessible stable controls; bounded repeated cues; and measured
device/session/cost behavior. The code example is a contract illustration only;
the exact literal in this document revision was extracted, checked with the
pinned Rust formatter, compiled with warnings denied, and executed against its
finite acceptance/refusal assertions. Those standalone checks establish only
the example's behavior at this boundary. They do not establish independent
review, current-consent lookup or revocation propagation in a running service,
integrated spotlight delivery, or production behavior. No device calibration,
browser flow, audio playback, visual output, or frontier evaluation is claimed
by this document. Cargo/native/WASM workspace checks are unperformed because
this task changes no application source.

Sources: [implementation roadmap](implementation-roadmap.md) (G12, delivery
slices, runtime director delivery, release gaps); [subsystem interfaces](subsystem-interfaces.md)
(common contracts, pure boundaries, session authorization, media, projection,
browser and observability boundaries); [runtime directors](runtime-directors.md)
(authority, pure operations, bounds and gates); [tempo engine](tempo-engine.md)
(audience observations, preferences, accessibility and acceptance);
[interaction engine](interaction-engine.md) (knowledge, consent, listener-safe
generation and acceptance); [feature refinement](feature-refinement.md)
(ParticipationWindow, SpotlightPreference and F42-SPOTLIGHT-ACCEPT);
[long-horizon state](long-horizon-state.md) (personal payload rights lifecycle).
Governing source hashes for this attempt are retained in the task brief.
