# Persistent, programmable browser presentation

Date: 2026-09-29
Status: Required client behavior; framework and measured device budgets remain gates

## Programming model and authority

The TV/shared-display and player applications are persistent Rust/WASM shells.
Normal game updates, phase changes, navigation, overlays, late assets and reconnect
must update the mounted application without a full-page reload. Initial loading
and an explicit deployment/recovery reload are separate operations, never a normal
response to a game update. Deployment activation is explicit and must not silently
replace a build in an active session or playtest.

Program the presentation through reusable Rust components, typed view models,
layout/theme settings and scene/animation primitives in the existing browser
crates. Components can be refined behind their public contracts independently of
game rules. Keep application and UI source Rust; generated WASM loading/browser
binding JavaScript is allowed. Framework selection stays open until G01/G04/G08.

Server views can select supported presentation variants and provide bounded typed
scene, camera, caption, transition and timeline instructions. Use an explicit
allowlist of implemented variants with schema versions and validation limits;
do not introduce arbitrary executable server code, scripts, expressions, CSS/HTML
injection, or an unrestricted UI interpreter. New component behavior requires a
Rust client build; existing variants and content can change through server views.
No new crate, independent presentation RPC service, or gameplay scripting runtime
is needed for this requirement.

The server supplies permitted data, legal offers, committed outcomes and audio.
Clients may interpolate a supplied movement path or animate an already resolved
roll, but never calculate legal movement, collision, dice results or turn timing.
Local tabs, expanded panels, focus, draft input, connection status and animation
progress are presentation state, with no authority over the game.

## Update and resource lifecycle

`df-web` owns one mounted shell and its connection/audio/resource lifetimes.
`df-client` accepts only ordered views from the active binding generation;
`df-player` and `df-display` convert those views to their role-specific composition.
`df-ui` owns reusable components/layout tokens, `df-render` scene presentation,
and `df-audio` playback/capture. See [Browser interfaces](subsystem-interfaces.md#browser-interfaces).

Start with complete server snapshots and update local presentation incrementally
using stable entity/component keys. A complete wire snapshot does not require
remounting the page or every component. Reconcile changed values and retain valid
focus, scroll, drafts, subscriptions and decoded resources. Clear state whose
member/run/permission ownership is obsolete; never preserve another member's
private data or submit an invalidated offer. Rejected drafts remain editable.
Delta encoding is optional later under the [RPC view contract](rpc-api.md#views-and-reliable-control).

Every mounted scope owns its listeners, subscriptions, pending asset/decode work,
animation handles and render resources. Update replaces or cancels only affected
work; unmount/dispose releases it exactly once. Asset completion is checked against
the active generation and resource key before publication. Keep caches and GPU,
decoded audio and pending-update memory bounded; repeated same-phase updates must
not accumulate layers, handlers or buffers.

Routes, panels and overlays update in place with touch, keyboard and mouse input,
responsive layouts and explicit focus restoration. Host controls remain permission
gated. Route selection cannot create permissions or initiate a game mutation by
itself. A failed optional renderer/asset uses the permitted flat/still fallback
while the shell, input and error surface stay usable.

## Motion, time and interruption

Animate on the browser frame clock with elapsed time rather than frame counts.
Server timeline anchors determine the current point in game-linked cues; the
monotonic local clock estimates presentation progress and never advances game
state. Synchronize captions/visual cues with observed audio playback where needed,
report drift/stalls, and expose blocked audio with a user-gesture unlock flow.

Each cue has a stable ID, presentation epoch, start/offset/duration and explicit
completion/interruption policy. Superseding updates retarget an ongoing visual
from its current rendered state when appropriate, or cancel it explicitly. Never
queue an unbounded transition backlog or delay authoritative input/results until
a cosmetic animation finishes. Cosmetic effects can be reduced before input or
essential information is degraded.

Coalesce superseded view snapshots before rendering while preserving reliable
control/cancel and audio semantics. A frame renders the newest accepted state;
bounded heavy decode/render preparation must not monopolize input processing.
Reduced-motion preferences remove nonessential motion while preserving outcomes,
captions and usable navigation. Avoid mandatory flashing effects.

On hidden-tab suspension, stop unnecessary rendering and bound buffered work.
On visibility return or reconnect, use the current permitted snapshot/timeline
and seek, skip or cancel cues according to their policy. Do not fast-forward an
unbounded backlog or replay completed one-shots. A new presentation epoch clears
obsolete cues/audio; the shell survives reconnect, with a visible connection state
and only still-valid local drafts retained.

## Acceptance and measurement

G04 records real supported browsers/devices, including whether direct smart-TV
browsing is supported. G08 sets measured input-to-render, frame-time tail, memory,
decode/upload and audio-drift budgets for representative devices. Choose refresh
targets and permitted degradation after measurement; no universal frame-rate or
phone/TV compatibility claim follows from a desktop viewport.

S01 must demonstrate both roles updating and navigating in place, retained input
focus/drafts across valid updates, and reconnect with no page reload or duplicated
listeners. S03 adds actual timed audio/caption observation, blocked audio unlock,
late asset fallback and interruption. S04/S07 exercise overlapping movement,
scene/overlay transitions, reduced motion and supported renderer fallbacks.

Use controlled-clock boundary checks for ordering, cue dedupe/cancellation,
visibility recovery and disposal, plus a frontier evaluator operating and visually
reviewing both real browser roles. Measure representative repeated updates and
simultaneous assets/audio/input; retain build/device identities, frame-time and
memory results with interaction evidence. Observe actual audio when relevant.
Client OTEL reports bounded update/render/decode costs, long frames, active cue
counts, fallback reasons, timeline drift and input-to-render correlation without
logging private view payloads. Unperformed device or playback checks stay pending.

After initial integration, optimize every affected crate through
[Optimization](optimization.md), preserving these contracts and the server's
authority. This document specifies planned behavior; no client runtime exists yet.
