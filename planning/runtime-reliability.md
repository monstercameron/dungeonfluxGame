# Runtime reliability requirements

Date: 2026-09-29
Status: Archive-derived constraints mapped to planned contracts; concrete implementation pending

These constraints strengthen the agreed Rust, server-authoritative, thin-client
design. [Subsystem interfaces](subsystem-interfaces.md) and [RPC API](rpc-api.md)
now specify ownership, durable decisions, offers, streams and recovery shapes.
[Subsystem architecture](subsystem-architecture.md) maps owners; the
[roadmap](implementation-roadmap.md) gates concrete types/frameworks/schemas/bounds.

## Authority and state ownership

Give each session's mutable gameplay state one serialization owner. Player input,
timer callbacks, job results, and debug mutations enter through that owner.
Inspection reads a detached consistent view rather than retaining mutable engine
references across reset. Rules resolution uses explicit dice and time inputs so
the same inputs are reproducible without network, filesystem, or provider calls.

Queue admission and game acceptance are different outcomes. A successful action
response must reflect the server's decision, with an actionable rejection reason.
Client controls display confirmed server state; transient sending/loading state
must not pretend that an outcome is confirmed. Preserve rejected input drafts.
Plan operation identities and retry semantics so reconnects, duplicate taps, or
lost responses cannot apply the same action twice.

Server-provided legal actions must be accepted under the same governing state,
or rejected with a reason when that state has changed. Carry authoritative
character/rules fields through every projection to the appropriate view; never
replace missing fields with independent client rules or fabricated statistics.

## Director composition and causal safeguards

All new runtime directors/simulators return bounded staged proposals through
`df-engine`; only the session owner commits. `df-rules` remains mechanics authority,
NPCs receive actual permitted perceptions and knowledge, and world/threat game time
is distinct from presentation curves and disconnected chat wall time. Content,
knowledge, beat and asset references are validated/versioned before admission.

Compound plans have stable per-step progress/cost/revalidation and explicit partial
outcomes, never magical rollback of committed steps. Tentative ASR, jokes/questions
and ambiguous semantic output cannot silently perform actions. Narrative/pacing
requests can be refused by autonomous NPC/world/rules constraints. Clients receive
permitted anchored tempo/media plans and cannot infer/decide game outcomes; paired
hidden-state cases must produce no unauthorized audiovisual/preload differences.
Recover compatible snapshots/decision records without rerolling, asking an LLM or
repeating unknown paid calls. See [Runtime directors](runtime-directors.md).

## Asynchronous work and timers

Every asynchronous job has an owner, lifetime, deadline, and explicit completion,
failure, or cancellation outcome. Bound queues and define their overflow behavior.
Slow consumers must not freeze the session owner or silently lose required work.
Committed effect jobs inherit session/run ownership with independent deadlines,
not the originating RPC's cancellation scope. A cancelled/lost receipt stops the
caller's wait without cancelling durable accepted work; explicit owner cancellation
or run termination controls it. Unaccepted capture/admission clears its request scope.

Cancellation alone cannot remove results already queued. Validate session/run
generation and operation identity when consuming results. Timer replacement also
needs a distinct instance identity; a reused name or restarted counter is
insufficient. An old completion cannot release a newer operation's busy state.

Give prepared work the lifetime it actually needs: phase work may end with its
phase, while intentionally prepared future media may belong to the session/run.
A same-state interaction must not accidentally cancel the response it starts.
Define pause, reset, shutdown, and reconnect behavior for each owned resource.

Specify transitions and prerequisites explicitly, including timeout and provider
failure paths. Every advertised route needs a valid terminal outcome or an
explainable waiting condition. Do not use debug Skip to conceal a stuck normal
flow. Recovery must preserve the invariants required by the next state; the old
demo's fixed timers, forced default characters, and scripted phases are not new
game rules.

## Thin-client recovery and presentation

Send a current permitted view when a client first subscribes and when it resyncs,
including in an idle session. Use explicit freshness/version rules to prevent
older updates from replacing current views. Presentation revisions must remain
unambiguous across reset or any future rewind feature.

Separate reconnect from joining a player. A sleeping tab or changed network can
leave a connection apparently open but stalled: define liveness, bounded reconnect
backoff, and resync on return to foreground. Reconnect must preserve authenticated
identity and must not create another player or replay one-shot effects blindly.
Specify duplicate-tab ownership for audio/input rather than silently stealing it.

Server timers carry enough timing information for an accurate visual countdown
after a late join or reconnect. Local countdown display does not decide expiry.
Check same-phase state updates, late asset loads, mounting/unmounting, and cleanup
in the actual browser renderer; avoid unnecessary remounts on every snapshot.
Validate browser capabilities and fallbacks on the device matrix chosen later.
Derive public join links from the configured public origin when behind a proxy.

## Audio, media, and provider boundaries

Audio delivery, browser receipt, scheduling, and playback completion are separate
facts. Give each line/track an identity and stable playback timeline; do not base
each chunk's offset on its own arrival time. Pace or buffer audio deliberately,
preserve terminal chunks, and define bounded buffering and lag recovery.

Normal completion drains the intended queued tail. Cancellation explicitly stops
already-buffered client playback and rejects late frames for that line. Account
for browser user-gesture requirements and verify audible playback rather than
assuming server emission proves it. Keep captions tied to the actual spoken
content, including fallback recordings. Speech fallback remains server-produced
audio, consistent with the thin-client design.

Validate constrained AI output before publishing it to subtitles, audio, or state;
checking only the final text is too late if raw chunks were already exposed.
Bound any validation buffer. Ordinary dialogue must not automatically become a
rules check or action. The model proposes content/actions; the server decides.

Define complete cache/recording identities for the relevant content, language,
role, format/schema, provider/model, and audio parameters. Exact cache keys and
rehearsal keys may have different semantics; document each rather than copying
the old key format. Publish reusable recordings only after complete successful
streams; cancellation or partial failure must not replace a valid entry. An
explicit prepared-only/replay mode must not silently invoke live providers on a miss.
The session owner commits host-authorized configured execution-mode changes for
new text/STT/TTS/image/video/sound jobs. Active jobs retain their admitted mode;
automatic live fallback is a separate reported outcome. Fault/fixture controls
require operator permission, not normal host access.

Register and verify the asset bytes a view references, including runtime-generated
assets. Promote required reusable media from disposable cache into durable asset
storage under the manifest/publication/access contract in the subsystem plan.
Concrete roots/deployment/retention are storage-gate decisions. A fresh checkout
must not depend on an old developer's cache or repeat paid generation implicitly.
If expensive media cannot meet the interaction deadline, plan prepared content or
a server-selected fallback; log which path was actually used.

## Persistence and diagnosis

Use PostgreSQL for durable backend data and a separate SQLite OTEL corpus for
runtime diagnosis, as defined in [Storage architecture](storage-architecture.md).
The development workflow's SQLite devlog is separate from both.

Define durable-state acknowledgement, restart recovery, migrations, and identity
allocation before persistence integration. Test restart against an existing data
directory, not only a fresh temporary database. Lossy telemetry queues are not an
acceptable policy for authoritative persistence: backpressure and write failures
need explicit handling consistent with the promised recovery guarantees.

Capture startup and child-process failures as well as gameplay traces. Record
rejections, stale-result discards, missing registrations/assets, fallback use,
and client presentation failures. A game can stall with no error-level log, so
check user-flow invariants and expected progress in addition to infrastructure
health. Debug controls go through normal ownership/validation; their help and
parse errors must never print credential defaults.

## Source evidence

Archive paths below are relative to `dungeonflux.old`; devlog IDs identify entries
in `docs/devlog.html`. These are evidence for the invariants, not APIs to copy.

| Requirement | Devlog / todos | Code inspected |
| --- | --- | --- |
| Authoritative responses and controls | `e-20260927-audit-two-defect-patterns`, QA-006/007/010/047 | `internal/api/ack.go`, `internal/runtime/room.go` |
| Reject abandoned jobs and timers | `e-20260927-abandoned-results`, `e-20260927-replaced-timer-callback`, QA-003/020 | `internal/runtime/generation.go`, `internal/runtime/timers.go`, `internal/runtime/timers_stale_test.go` |
| Explicit ownership and full-flow checks | `e-20260926-debug-service-race`, `e-20260926-eng015-dispatcher-gap`, E2E-004 | `internal/runtime/room.go`, `internal/wire/execs.go`, `internal/wire/check_e2e_test.go` |
| Initial view and reconnect | `e-20260926-watchhub-idle-lobby`, `e-20260927-silent-watch-stream`, `e-20260927-creation-countdown-reconnect` | `internal/api/watch.go`, `web/shell/client_resync.go` |
| Actual playable audio | `e-20260926-silent-narration-three-causes`, `e-20260926-audio-hub-and-cancel-fixes`, `e-20260926-audio-choppy-cutoff` | `internal/wire/execs.go`; tester findings in `notes/emmaka/2026-09-27-playtests.md` |
| Validate before publication | `e-20260927-streamed-dialogue-leak`, QA-042/043 | `internal/llmexec/spoken.go` |
| Complete, contract-specific recordings | `e-20260927-complete-cancelable-recordings`, `e-20260927-rehearsal-recording-isolation`, QA-030/031/033 | `internal/modelchain/stored_stream.go`, `internal/modelchain/recording_key.go` |
| Durable assets and restart policy | `e-20260926-lfs-buildtime-media`, `e-20260926-two-crash-loops`, STORE-001/002/003 | `internal/store/sqlite/writer.go` |

The former game used a demo-specific subset and house rulings. Do not inherit
its fixed odds, shortcuts, timing caps, AI/provider choices, or client-side speech
proposal as requirements for this game. Standard D&D 2024 support remains governed
by [Rules support](rules-support.md).

## Hosted service operating contract

[Service operations](service-operations.md) is the governing refinement for initial
single-instance and later multi-instance routing, fenced claimed dispatch/unknown
spend, tenant-scoped repositories, durable media availability, per-instance SQLite
segments/spool, finite proposed bounds, controlled-launch SLO/RPO/RTO and deletion/
backup restore. These are declared candidate targets, not measured capacity or HA.
Existing promises of canonical replay/immutable history are scoped by personal-data
redaction and rights retention: return explicit Redacted/Unavailable and never
reconstruct removed private payload. [Commerce service](commerce-service.md) owns
subscription/entitlement and hierarchical spend authority; process-local counters
and cached grants cannot override it. Independent current-output acceptance is
required before any paid-release or recovery claim.
