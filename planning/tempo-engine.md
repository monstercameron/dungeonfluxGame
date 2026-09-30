# Gameplay feel and tempo engine

Date: 2026-09-30
Status: First-class planned Rust subsystem; policies/device budgets uncalibrated

`df-experience` recommends macro pacing. `df-tempo` controls moment intensity.
`df-presentation` converts permitted semantic events/profiles into concrete plans;
`df-media` supplies approved assets/audio. Thin Rust/WASM clients interpolate,
render and play these plans in persistent mounted shells under
[Client presentation](client-presentation.md). They do not infer game danger or
run tempo/game policy independently.

## State and server output

`TempoState` persists normalized tension, energy, danger, mystery, urgency, triumph
and dread; rising/falling/stable momentum; rest/build/anticipation/reveal/conflict/
peak/release phase; target/curve anchors and policy version; recent impulses and
per-effect fatigue/refractory history. All values/units validate bounded ranges.
The supplied example increments/BPMs are tuning ideas, not universal targets.

`TempoEngine::advance` takes explicit presentation time and permitted inputs from
committed narrative/encounter/combat/world/interaction events, known player state
and `PacingRecommendation`. It returns a target `TempoFrame`, bounded curve/phase
change and `TempoCueIntent`. Continuous server-owned policy uses inertia, deadbands,
rate limits and hysteresis to avoid jitter; a named, approved shock may spike and
then decay within caps. Use scheduled boundary decisions and timestamped curves,
not 60 RPCs/DB commits/LLM calls per second. Clients evaluate cosmetic interpolation
against the server anchor and clamp capability/accessibility limits; they cannot
choose a new target, phase or rules expiry.

Frames/profiles bind session/run, presentation epoch/sequence, policy version,
start/timebase, duration/curve, audience scope and fallback. Current profile/active
cues are available in the full Watch snapshot; repeats do not replay impacts.
Visibility/reconnect resyncs current state, rejects old epochs, skips expired
one-shots and clears replaced resources. Audio Mix/Cancel messages preserve ordered
track/epoch/timeline semantics and actual playback reporting.

## Presentation profile and cues

| Channel | Typed parameters and constraints |
| --- | --- |
| Music | approved theme/stem IDs, per-layer gain, bounded crossfade, optional musical boundary, reveal duck/silence/stinger; cache campaign/NPC/faction motifs, no fresh song each tick |
| SFX | semantic library cue, density/priority/reverb/impact/ducking; ordinary effects use prepared/procedural-approved fallback without blocking a turn |
| Camera/screen | permitted framing, movement/zoom, contrast/vignette/particles and bounded optional impact treatment; no secret entity focus |
| Lighting/environment appearance | cosmetic shader/palette/fog treatment within allowed scene representation; real weather/visibility/collision changes require world/rules decisions |
| UI | supported emphasis/transition/layout variants preserving readability, stable controls/focus and all legal options; no fake countdown or shortened rule timer |
| Narration | approved cadence/pause/performance hints for future validated clauses; preserve wording/meaning/captions and accepted outcome |

Screen shake is an event impulse; tempo sets a ceiling. Fatigue budgets,
refractory windows, novelty rotation and rare peaks prevent continuous shaking,
flashing, maximal music or repeated stingers. All channels have count/amplitude/
duration budgets and priority conflict/interruption rules. Speech has protected
intelligibility; ducking/release changes are tied to actual narration timelines,
not guessed provider latency. Music layers must share compatible timebases/stem
metadata, with explicit fallback when they cannot be synchronized.

Cosmetic intensity never blocks accepted outcomes or necessary input, hides a
legal option, causes layout to jump under a finger, or turns decorative urgency
into a mechanical restriction. A real countdown comes only from an approved
server timer policy. Fictional NPC/environment choreography is a world/interactions
proposal when it changes actual state; a cosmetic animation cannot become fact.

## Audience, preferences and observations

Construct inputs/profiles from audience-permitted facts or a proven public cue.
A secret boss, unrevealed threat-clock stage, NPC belief or another player's hidden
HP cannot affect public music/lighting/tempo in a revealing way. Test paired states
that differ only in hidden data: unauthorized profile/cues/assets should match.
Player-specific known danger can have a separately routed permitted profile.
Projection filters before serialization/prefetch manifests, not just in a widget.

SetPreferences stores authorized reduced-motion/flashing, audio limits/captions
and readability preferences. Server plans honor these; clients clamp further to
safe supported capability settings without changing mechanics. Reduced-motion
means no screen shake/forced camera punches and an equivalent readable emphasis.
User-selected volume/mute remains respected; avoid coercive loudness. Actual
amplitude/contrast/flash/latency budgets need G12 device acceptance rather than
invented numerical safety claims.

Experience uses committed activity windows, voluntary participation/feedback and
explicit permitted reports. Inactivity is not proof of boredom, nor laughter proof
of low tension. No covert microphone monitoring, emotion inference or automatic
social penalty from local muting/audio cancellation. A capture/performance report
is an observation under current lease, not an NPC action. Any barge-in interaction
requires an authenticated explicit event and server validation under voice policy.

## Acceptance and optimization

F42–F44/G12 validate calm/build/anticipation/reveal/conflict/peak/release, bounded
shock/decay, fatigue/rotation, known versus hidden inputs, speech/music ducking,
stale/repeated cue rejection, reduced motion, stable legal controls and present-time
versus game-time separation. Test slow assets/provider failure/paused campaign,
late join, sleeping phone, reconnect mid-crossfade and obsolete-buffer cancellation.
Benchmark policy time, actor wakeups, snapshot bytes, frame timing/audio drift,
resource retention and actual video/prefetch spend. Frontier vision/computer-use
plus audio observation checks the running two-role result; policy tests alone do
not prove smoothness or pleasant gameplay. Tune from measured session feedback
without pretending heuristic values predict fun with certainty.
