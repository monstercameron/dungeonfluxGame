# G12-D02: Tempo inertia and accessibility profiles

Status: Design contract; implementation and device/session calibration pending

## Outcome and authority

This decision defines how moment-to-moment intensity changes and how a participant's
presentation preferences constrain delivery. It preserves the backlog acceptance
criterion verbatim: **legal timing unchanged**. Inertia and accessibility affect
only cosmetic presentation. They cannot change rules timers, turn/action availability,
accepted outcomes, or the expiry of a legal offer. A timeout or countdown exists only
when an independently authorized rules/session policy supplies it.

`df-experience` may recommend macro pacing from permitted committed activity windows,
voluntary feedback and explicit preferences. It must not infer boredom or emotion from
silence, laughter, microphone activity or muting. `df-tempo` consumes a bounded set of
permitted inputs and an explicit presentation-time anchor, then emits a versioned
`TempoFrame` and bounded cue intents. `df-presentation` filters and composes those
intents for each authorized audience before serialization or asset forecasting.
`df-media` executes only admitted, budgeted asset demands. Clients interpolate from
the server anchor and may clamp to locally supported capabilities; they do not choose
new targets, alter phases, expose hidden state or make policy decisions.

This is a pure policy boundary: no clock, database, socket, provider or SDK access.
Inputs bind session/run, presentation epoch and sequence, policy/profile revision,
timebase and audience scope. Session ownership commits accepted changes and fences
stale callbacks. Game time and presentation time remain distinct: a paused campaign
does not advance from wall time, and cosmetic interpolation never advances game time.

## Inertia and cue policy

Represent intensity dimensions using validated normalized policy values with a
documented unit/range in the eventual shared contract. They are design units, not
probabilities, D&D mechanics, calibrated quality scores or device safety limits.
The versioned tempo policy supplies per-dimension slew limits, deadbands, hysteresis,
and named impulse/decay rules. An ordinary recommendation approaches its target
gradually; values inside the deadband retain the current value. A specifically
approved event impulse may move faster, but remains capped and decays under the same
versioned policy. Invalid values, an unknown policy, backward presentation time,
duplicate/stale sequence, or an unauthorized/hidden input are rejected or omitted
with a typed reason; they must not silently reset state or force a cue.

Cue intents carry stable IDs, semantic type, audience, priority, timebase, expiry,
fallback and bounded channel parameters. Per-effect fatigue/refractory state and
per-channel count/amplitude/duration budgets suppress or rotate repeated effects.
Priority resolves conflicts without displacing protected speech intelligibility,
required captions, legal controls or necessary input. Speech ducking and release bind
to actual narration timeline observations, not guessed synthesis latency. Compatible
stem/timebase metadata is required for a synchronized mix; otherwise choose a safe
prepared fallback or explicit silence. Late, stale, duplicate, expired or wrong-epoch
cues are dropped; reconnect resynchronizes the current frame and never replays an
expired one-shot.

## Accessibility profiles and agency

Store authorized preferences as versioned inputs to server presentation planning.
The profile can express reduced motion, flash avoidance, audio ceiling/mute, captions,
readability/contrast and supported capability clamps. Its values describe user intent
or declared device capability; they are not proof that a device is safe or supported.
Unknown or contradictory profile data fails closed for optional effects and retains
the ordinary readable controls. A client may apply a stricter local clamp, but cannot
relax a server or user constraint.

Reduced motion removes shake, forced camera punches and unnecessary movement while
providing equivalent readable emphasis (for example, a stable outline or text label).
Flash avoidance suppresses flashing effects. Audio limits and mute are respected
without coercive loudness; captions remain available when requested. The profile
never removes a legal action, hides a permitted option, requires an audio cue to
understand an outcome, moves a control under an active pointer, or creates a fake
countdown. A locally muted or cancelled cue cannot cause a social penalty or alter
game state. Barge-in is an authenticated explicit input, then validated by the
existing voice/action policy; capture or performance reports are observations, not
NPC actions.

Presentation inputs are audience-filtered before composition, serialization and
prefetch manifest construction. Paired sessions differing only in an unauthorized
secret (including hidden boss state, threat stage, NPC belief or another player's
HP) must produce equivalent unauthorized frames, cue intents and asset demands.
Per-player known danger may affect only that player's authorized route. A private
phone cue with no safe private playback falls back to permitted phone text; it must
never spill to public display captions or audio. Disclosure remains an explicit
player action under the existing audience/reveal contract.

## Asset demand and calibration boundary

Forecasting is a heuristic ordering signal only. It is not a calibrated probability
or permission to spend. `df-presentation` proposes bounded typed demands from
permitted forecasts; `df-media` enforces admitted mode, concurrency, bytes, expiry,
cancellation scope, cache identity and reserved campaign allowance before dispatch.
Optional prefetch cannot consume unapproved video spend or crowd out required speech,
fallbacks or current-turn assets. Expired or superseded speculative work is retired
without deleting referenced assets. Unknown supplier spend remains reserved until
reconciled. Prepared-only and replay misses make no live provider call.

No numeric device, latency, frame, audio drift, flash, contrast, memory, cost or
prefetch-waste ceiling is established here. G04/G08/G12 must select representative
devices and obtain device/audio, session-load and cost measurements before those
limits are frozen. Record source/build/configuration/content/profile revisions,
device matrix, sample/session denominators, distributions and failure cases; do not
claim a heuristic target is measured or a desktop viewport proves phone support.

## Rust contract example (illustrative, not a production API)

The following standard-library-only example demonstrates a proposed transition
boundary. Its `0..=100` units, step cap and refractory length exist only to make the
finite example concrete; they are not measured limits or selected product defaults.
The example keeps a legal-action flag outside tempo state to make the noninterference
invariant explicit.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Profile {
    reduced_motion: bool,
    avoid_flash: bool,
    audio_muted: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct State {
    intensity: u8,
    presentation_ms: u64,
    sequence: u64,
    last_shake_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Frame {
    intensity: u8,
    shake: bool,
    flash: bool,
    audio: bool,
    legal_action_available: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    OutOfRange,
    StaleSequence,
    TimeWentBackwards,
}

fn advance(
    state: State,
    requested: u8,
    impulse: bool,
    profile: Profile,
    now_ms: u64,
    sequence: u64,
    legal_action_available: bool,
) -> Result<(State, Frame), Refusal> {
    if requested > 100 {
        return Err(Refusal::OutOfRange);
    }
    if sequence <= state.sequence {
        return Err(Refusal::StaleSequence);
    }
    if now_ms < state.presentation_ms {
        return Err(Refusal::TimeWentBackwards);
    }

    let step = if impulse { 20 } else { 5 };
    let intensity = if requested > state.intensity {
        state.intensity.saturating_add(step).min(requested)
    } else {
        state.intensity.saturating_sub(step).max(requested)
    };
    let refractory_elapsed = state
        .last_shake_ms
        .map(|last| now_ms.saturating_sub(last) >= 1_000)
        .unwrap_or(true);
    let shake = impulse && !profile.reduced_motion && refractory_elapsed;
    let next = State {
        intensity,
        presentation_ms: now_ms,
        sequence,
        last_shake_ms: if shake { Some(now_ms) } else { state.last_shake_ms },
    };
    let frame = Frame {
        intensity,
        shake,
        flash: impulse && !profile.avoid_flash,
        audio: !profile.audio_muted,
        legal_action_available,
    };
    Ok((next, frame))
}

fn main() {
    let profile = Profile {
        reduced_motion: true,
        avoid_flash: true,
        audio_muted: true,
    };
    let initial = State {
        intensity: 10,
        presentation_ms: 100,
        sequence: 1,
        last_shake_ms: None,
    };
    let (built, frame) = advance(initial, 80, false, profile, 200, 2, true).unwrap();
    assert_eq!(built.intensity, 15);
    assert!(frame.legal_action_available);
    assert!(!frame.shake && !frame.flash && !frame.audio);

    let (peak, _) = advance(built, 80, true, profile, 300, 3, true).unwrap();
    assert_eq!(peak.intensity, 35);
    assert_eq!(
        advance(peak, 101, false, profile, 400, 4, true),
        Err(Refusal::OutOfRange)
    );
    assert_eq!(
        advance(peak, 40, false, profile, 400, 3, true),
        Err(Refusal::StaleSequence)
    );
    assert_eq!(
        advance(peak, 40, false, profile, 299, 4, true),
        Err(Refusal::TimeWentBackwards)
    );
}
```

This example does not represent approved shared types, persistence, authorization,
audience filtering, asset admission, audio synchronization, accessibility
conformance, or integrated behavior. Production transitions must use frozen G03/G08
contracts and the session owner. Required refinements include typed channel budgets,
policy/profile versions, epoch checks, explicit expiry/fallback, and source-safe
audience projection; this task does not create duplicate production APIs.

## Acceptance and pending gates

Preserve the named hook: `Define tempo inertia and accessibility profiles` across
`df-experience`, `df-tempo`, `df-presentation` and `df-media`, with **legal timing
unchanged**. At contract and later integrated boundaries, exercise:

- Ordinary rise/fall, deadband, named impulse/decay, fatigue and refractory behavior
  with controlled presentation time; reject invalid, stale, expired and wrong-epoch
  inputs without modifying committed state.
- Reduced motion, flash avoidance, mute/audio ceiling and captions while all legal
  offers and stable controls remain available; interrupt speech/music and reconnect
  mid-crossfade without stale-buffer playback or repeated one-shots.
- Paused game time versus advancing presentation time; slow/missing providers and
  assets leave committed outcomes, timers, input and safe fallbacks intact.
- Paired hidden-state cases yield equal unauthorized profiles, cues and prefetch
  demands; private phone fallback never leaks into public display/audio.
- Fair demand priority, expiry, cancellation, reservations, waste and unknown-spend
  recovery under measured representative session/device workloads.

The current dispatch has no reviewed prerequisite contracts (`canonical_task_refs`
is empty), and G03/G08/G10/G11/G12 production types and calibration remain gates.
The exact policy outcome is specified here, but this document does not claim an
integrated implementation or completed acceptance. Original pending production
gates remain: source/build-bound implementation and contract review; representative
device/session/cost measurement; actual phone sleep/reconnect/private-routing tests;
and appropriate audible synchronized playback observation. Native/WASM workspace
checks and frontier evaluation are unperformed for this document-only attempt.

## Source basis and provenance

Applicable design: [Implementation roadmap](implementation-roadmap.md) (G12 and
S03/runtime director delivery), [Subsystem interfaces](subsystem-interfaces.md)
(tempo/presentation/media boundaries), [Runtime directors](runtime-directors.md)
(pure staged composition, time and effect ownership), [Gameplay feel and tempo
engine](tempo-engine.md), [Interaction engine](interaction-engine.md) (consent,
voice and listener-safe output), [Long-horizon state](long-horizon-state.md)
(paused world time). Process and evidence follow [AGENTS.md](../AGENTS.md),
[Coding style](coding-style.md) and ADR 0001–0005. The original acceptance text is
in backlog item `B-G12-D02` (`G12:D:Define tempo inertia and accessibility profiles`,
expected: `legal timing unchanged`).

Frozen input revision: `9def9b845531e5cc589a28343b822b4664bd03fd`. Governing source
SHA-256 values recorded before editing:

| Source | SHA-256 |
| --- | --- |
| `planning/implementation-roadmap.md` | `0160ad8e8ec38f768e2348209b9989e30e8f403d9b1a4ebf694f0801f7206932` |
| `planning/subsystem-interfaces.md` | `f26e1dca42e878f8a816c9f9aa37463cb8f061224598214ceee62632a161907a` |
| `planning/interaction-engine.md` | `1bad7c28609ac5e5a51a3ea0b3c1fae17ebeb6380dbdb8fce582085a8ede354f` |
| `development/backlog-catalog.json` | `039b0a03b4085b43ad32c4063e2cb8fc789fc55fff7552aec6e951ec3b4704c3` |
| `AGENTS.md` | `55578ed92c477dfe3c306db38c6ad20390bfe8208ca998e6f3bf6f6bebbec182` |
| `planning/coding-style.md` | `2d8e327e4172643544bd80b591f38226c25b83940a10f1d9e18ada9044faeabb` |
| `ADR/0001-sqlite-agent-workflow.md` | `acfe32a8d5e846aa4d3a2981529533b9f53cdcc11088424128e60c20b722adc8` |
| `ADR/0002-resource-scheduling-and-cleanup.md` | `a6476cfef449e089639109cc6d3cf5f0800b792b57a32696258d5ac0b9511966` |
| `ADR/0003-agent-devlog.md` | `ee293673b01391c3a39577bd116a60abb3edfa662a7a8d1315556d6fe537d912` |
| `ADR/0004-development-reliability.md` | `8f03d02d478086fd1f50d7aa10e8e7e766e3b15ae02f7f96efa43757c98a4dae` |
| `ADR/0005-frontier-output-evaluation.md` | `25ab35a57ee8516a272b1ff3d04bba4def91319255d89158c9be55283ca6c35c` |
| `rustfmt.toml` | `7a142cbd3f5e9c09c6dc4a1850e053415530eb5450d27983dc26c75230b8c5ba` |
| `rust-toolchain.toml` | `9500030ccefd0bab631fb7f1763f79f4103eca3a344c36cf15be869e330683bb` |
| `planning/runtime-directors.md` | `ed8944f316d912498107ee550431e560795ba8114cb63a181b6aadeca70b48ca` |
| `planning/tempo-engine.md` | `e227dda990685f5771b577529652c44d59704a23db8ad206e3e59be41f4c81d6` |
| `planning/long-horizon-state.md` | `977b5a346190a55c119242fdb7009a645e6d76aa2946d0684c46571d846e96ff` |
