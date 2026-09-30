# Feature inventory from the archived game

Date: 2026-09-29
Status: Planning baseline; old evidence does not establish new implementation

## Evidence and interpretation

Reviewed archive: `dungeonflux.old`, revision
`a35d4259d2f8e6161ca403bcc1a5fe1202b84e9e`. Sources include service schemas,
production package inventory, key engine/runtime/provider/client implementations,
all todo titles, selected todo details, prior devlog extraction/review, and tester
findings. This is an architectural inventory, not an exhaustive source audit.

The todo file has 598 Markdown task rows: 350 checked and 248 unchecked, including
compound/nonnumeric IDs such as `PH-CRE-001`, `OPS-SPLAT-001` and `COMBAT-MOVE`. Some IDs repeat,
and unchecked wiring tasks overlap later repairs. Neither count proves running
feature coverage. The old engine explicitly implements a demo rules subset.

Paths in the source column are relative to the archive. Carry means plan the
capability; expand means the new requirements exceed the old implementation;
conditional means retain a boundary but resolve scope before implementation.
Feature IDs are stable planning references, now recorded in the authorized
[planning SQLite corpus](plan-database.md); they are not implemented features.

## Game and presentation capabilities

| ID | Capability / disposition | Primary old evidence | New owner and API |
| --- | --- | --- | --- |
| F01 | Sessions, room codes, invite/QR join, player name/readiness; carry without two-seat limit | `internal/api/session.go`, `internal/game/lobby.go`, `internal/wire/qr.go`; QA-025/026 | df-auth, df-session; SessionService |
| F02 | Stable membership, reload/sleep/reconnect, duplicate-tab input/audio ownership; carry | `web/shell/client_resync.go`, `internal/runtime/rooms.go`; EMK-001/003, QA-027 | df-client, df-session; Resume, Watch, BindClient |
| F03 | Shared display, personal player views, host controls; carry, host is a permission | `proto/dungeonflux/v1/common.proto`, `web/dm`, `web/phone`, `web/host` | df-display, df-player, df-ui; filtered views |
| F04 | Character choices, legal build, names, flavor, portraits/reference art, ready/lock; expand | `internal/game/phase/creation`, `internal/game/rules/build.go`; RULES-007/008 | df-rules, df-engine, df-ai, df-media; ActionService |
| F05 | Complete sheet, equipment, proficiencies, resources and progression; expand | `internal/domain/build_stats.go`, `web/phone/sheet.go`; PHONE-012/027 | df-rules, df-engine; CharacterView |
| F06 | Scenes, exploration, objectives, NPCs, story beats, encounter triggers, resolutions/endings; carry without hardcoded old story | `internal/game/phase`, `internal/content/oneshot.go`, `internal/content/world_bible.go` | df-content, df-world, df-narrative, df-presentation, df-engine; SceneView, SubmitAction |
| F07 | Legal actions with labels, choices, disabled reasons, targets/previews; carry with typed/versioned offers | `internal/game/legal.go`, `internal/domain/view.go`; ENG-020, INT-002 | df-rules, df-engine, df-api; ActionOffer |
| F08 | Typed dialogue, NPC context, validated interpretation and visible rejection/recovery; carry | `internal/api/act.go`, `internal/llmexec/interpret.go`, `internal/llmexec/spoken.go`; QA-042–048 | df-ai, df-intent, df-interaction, df-engine; SubmitText |
| F09 | Hold-to-talk, transcription, cancel, typed fallback, microphone ownership; carry | `internal/api/talk.go`, `internal/voice/in`, `web/phone/ptt_browser_wasm.go`; EMK-007, QA-049 | df-media, df-client, df-audio; VoiceService.Talk |
| F10 | Checks, saves, dice results, modifiers/explanations; expand beyond persuasion/weapon attacks | `internal/game/phase/check`, `internal/game/rules/rulings/rulings.go`, `web/phone/dice.go` | df-rules, df-engine; RollOffer, RollResultView |
| F11 | Initiative, actions/reactions, attacks/damage, conditions, enemy decisions/end outcomes; expand | `internal/game/combat`, `internal/game/rules/attack.go`; COMBAT-001–009 | df-rules, df-combat, df-encounter, df-engine; EncounterView |
| F12 | Authoritative geometry, reach/path, movement/dash, occupancy, range/visibility; expand beyond fixed grid shortcuts | `internal/game/combat/navigation.go`, `internal/game/combat/reach.go`, `internal/domain/combat_map.go` | df-rules, df-engine; TacticalView |
| F13 | Flat battle presentation, tokens, initiative/HP, camera cues, impact and animation dedupe; carry | `web/dm/battle_stage.go`, `web/phone/combat_map.go`, `internal/api/project_combat_map.go` | df-render, df-display, df-player; PresentationCue |
| F14 | Splat/3D scenes, LODs, colliders, billboards and cinematic camera effects; conditional fidelity extension with flat fallback | `web/splat`, `internal/content/battlefield_wooded.go`; SPLAT-001–026 | df-render, df-assets, df-tools; SceneManifest |
| F15 | Reference sheets, portraits, identity-consistent loops and cinematic finishers; carry capability, stage fidelity | `internal/media/reference.go`, `internal/media/billboard.go`, `internal/wire/killcam.go`; BB-002, KC-001–018 | df-media, df-assets, df-render; MediaJob, TimelineCue |
| F16 | Spoken narration/NPC lines, captions, matching fallback speech, scheduling/stop/drain; carry | `internal/voice/out`, `web/shell/audio`, `internal/api/listen.go`; EMK-009, QA-051 | df-media, df-presentation, df-audio; Listen, ReportPlayback |
| F17 | Music, ambience, SFX, loops, gain/duck/crossfade, display/player targeting; carry | `internal/media/audio_router.go`, `internal/game/audio_cues.go`; INT-006/007, AUD-002 | df-media, df-tempo, df-presentation, df-audio; AudioStreamMessage |
| F18 | Player/session locales, catalogs, narrative/speech locale and fallback; carry, language list open | `internal/i18n`, `internal/api/localize.go`; I18N-001–012, QA-028/029 | df-locale, df-content, df-ai, df-media; SetPreferences |
| F19 | Responsive layouts, keyboard/touch, pending/error/connection states, reduced motion/readability; carry | `web/dm/aspect.go`, `web/phone/screen.go`, `web/shell/splash_wasm.go`; UI-001/002 | df-ui, df-player, df-display |
| F20 | Journal/recap, history and public/private knowledge; carry baseline UI, expand durable history | `web/phone/journal.go`, `internal/content/plot_thread.go` | df-knowledge, df-engine, df-session; JournalService |
| F21 | Start/pause/resume, authoritative options, timers/new runs; carry, debug force/skip are not normal gameplay | `internal/api/host.go`, `internal/runtime/timers.go`; QA-004/006/010 | df-engine, df-session; HostService |

## Infrastructure and development capabilities

| ID | Capability / disposition | Primary old evidence | New owner and API |
| --- | --- | --- | --- |
| F22 | Structured RPC, snapshots, audio/mic/assets, auth/origin checks; carry Rust bridge | `proto/dungeonflux/v1/session.proto`, `internal/api/server.go`, `web/shell/client_transport_wasm.go` | df-protocol, df-rpc-bridge, df-api |
| F23 | Bounded owner, timer/job generations, registered effects, cancellation/shutdown; carry | `internal/runtime/room.go`, `internal/runtime/generation.go`, `internal/runtime/runner.go`; RT-009/010 | df-session, df-server |
| F24 | Durable sessions/members/characters/actions and restart recovery; replace gameplay SQLite with PostgreSQL | `internal/store/sqlite/schema.go`, `internal/runtime/recover.go`; BASE-018 | df-persistence; SessionRepository |
| F25 | Immutable manifests/bytes, preload, hashes, bounded browser cache/fallbacks; carry | `internal/api/assets.go`, `web/shell/assetcache.go`, `scripts/buildtime/manifest.go` | df-assets, df-client, df-tools; AssetService |
| F26 | AI/media adapters, deadline/concurrency/fallback/hedging policies; carry interfaces, reselect vendors | `internal/ports/ports.go`, `internal/modelchain/chain.go`, `internal/media/pool.go` | df-provider-api, df-providers, df-ai, df-media |
| F27 | Exact cache identities, complete recordings, deterministic replay/offline and faults; carry | `internal/modelchain/recording_key.go`, `internal/modelchain/stored_stream.go`, `internal/runtime/checkpoint.go`; QA-030–040 | df-ai, df-media, df-testkit, df-tools |
| F28 | Spend reservation, usage reconciliation and cost reports; carry with exact monetary units | `internal/budget/ledger.go`, `scripts/costcheck.ps1` | df-provider-api, df-persistence; BudgetStore |
| F29 | Debug state/views/choices/scopes/clients, checkpoints, faults and captures; carry scoped development tools | `proto/dungeonflux/v1/debug.proto`, `cmd/dfctl`, `internal/api/debug` | df-tools, df-api, df-session; DebugService |
| F30 | OTEL logs/traces, bounded telemetry queries, full-flow progress evidence; expand | `internal/logx`, `internal/api/debug/logs.go`; QA-044 | df-observe, df-telemetry; TelemetryService |
| F31 | Fake providers, controlled simulations/replay, contract/browser playtests; carry stronger output gates | `internal/fakes`, `internal/sim`, `internal/game/combat/combatsim`, `notes/emmaka/2026-09-27-playtests.md` | df-testkit, df-tools; ADR 0005 |
| F32 | Atomic builds, asset preparation, stable previews, owned processes and optimization; carry Rust tooling | `scripts/buildweb/main.go`, `scripts/devserver/supervisor.go`, `scripts/buildtime/run.go` | df-tools; optimization.md |
| F33 | Queue, frontier evaluation, devlog/escalation/cleanup, six-hour changelog; new workflow | `TODOS.md`, `AGENTS.md`, `docs/devlog.html`; new ADR 0001–0005 | df-workflow; five-table SQLite coordinator |
| F34 | Public project page/gallery, acknowledgements/rules attribution; carry separate from runtime | `docs/index.html`, `docs/gallery.html`, `web/shell/about.go` | docs/, df-ui |

## First-class runtime capabilities from supplied outlines

The supplied narrative, interaction, subsystem, tempo and asset outlines add these
requirements to the archive-derived F01–F34. Their example probabilities/modifiers/
thresholds are not approved mechanics or calibrated measurements. All listed
models/contracts are planned, not implemented.

| ID | Capability / disposition | Design source | New owner and boundary |
| --- | --- | --- | --- |
| F35 | Causal world time/travel/weather/resources, due events, NPC schedules/faction movement and threat stages | supplied major subsystems + interaction | df-world; WorldSimulator::advance/affordances, WorldDelta |
| F36 | Canonical truth versus observer knowledge/false belief, witnesses/provenance, memory decay and bounded rumor network | supplied interaction + major subsystems | df-knowledge; perceive/query/propagate/decay, KnowledgeDelta |
| F37 | Unified input interpretation, questions/jokes/meta/uncertainty, source-valid bounded compound plans and partial progress | supplied interaction + major subsystems | df-intent; classify/validate, IntentDisposition, ActionPlan |
| F38 | NPC personality/goals/emotion, autonomous reactions, multi-axis relationships, conversations/topics/secrets/promises and social/faction policy | supplied interaction | df-interaction; react/plan, InteractionProposal |
| F39 | Flexible arcs/phases/beat alternatives, threads/hooks, gravity/threat relevance, believable convergence and intervention budgets | supplied narrative | df-narrative; evaluate, NarrativeProposal |
| F40 | Appropriate combat/social/chase/hazard/puzzle/survival/negotiation/escape challenges, objectives and failure routes | supplied major subsystems | df-encounter; propose/validate, EncounterPlan |
| F41 | Legal NPC tactics, battlefield objectives, rule-grounded reinforcement/escalation and perception-limited decisions | supplied major subsystems | df-combat; choose, TacticalProposal; df-rules retains initiative/action economy |
| F42 | Macro pacing, activity windows and voluntary fair spotlight recommendations from permitted observations | supplied major subsystems + tempo | df-experience; observe/recommend, PacingRecommendation |
| F43 | Continuous intensity/phase/curve, inertia, bounded event impulses, fatigue/rotation and accessible audience-safe audiovisual/UI profiles | supplied tempo | df-tempo; advance, TempoFrame/TempoCueIntent |
| F44 | Structured semantic moments, concrete audience-safe presentation and ranked bounded predictive asset demand | supplied major subsystems + tempo + asset engine | df-presentation; compose/forecast, PresentationPlan/AssetDemand; df-media::AssetEngine executes generation |

See [Runtime directors](runtime-directors.md), [Narrative engine](narrative-engine.md),
[Interaction engine](interaction-engine.md), [Tempo engine](tempo-engine.md) and
[Asset engine](asset-engine.md). Extend F15/F16/F17/F25–F28 with canonical packs/
style/reference dependency versions, stable-clause validation, explicit barge-in,
asset lifecycle/priority/fairness, prefetch waste/spend and nonblocking fallbacks.
F24/F27 include compatible no-provider state reconstruction from snapshots and
committed decisions; F30/F31 include safe policy provenance, privacy/replay and
running-output evidence. These extensions reuse existing persistence/media/tools,
not extra provider/graph databases or one crate per asset/vendor.

## New required capability families

The latest [Feature refinement](feature-refinement.md) preserved the then-current 40 game/tooling crates; service refinement adds independent df-commerce
and adds three canonical families. Concrete typed schema additions remain G03 work.

| ID | Capability / disposition | Design source | New owner and boundary |
| --- | --- | --- | --- |
| F45 | Hosted remote and mixed-room campaigns, simultaneous input/presence/AFK, authorized private/public capture/audio routing and network recovery; required core | latest table-stakes/Phase 1 request; remote-play.md | df-auth/session/api/client/audio; existing Session/Client/Voice/Audio RPC and single fenced actor |
| F46 | Observer-safe committed-history recaps and clearly speculative spoiler-safe trailers, optional budgeted video, still/voice fallback and consented rights-scoped export | latest numbered ideas 7/8; campaign-cinematics.md | df-knowledge/narrative/presentation/media/assets/engine/session; BookendSpec/Plan, FactSelection, ExportGrant |
| F47 | Personalized source-compatible items/enemies/spells and explicit disclosed versioned opt-in custom definitions using approved deterministic handlers | latest ideas 11/12 and P2 generation; generated-content.md | df-content/rules/ai/engine/session/persistence with existing media; ContentCandidate/Validation/Admission, ItemOrigin |

F03/F07/F36 add ContextualPrivateOffer/KnowledgeCue and explicit disclose-to-party
intent; F42 adds opt-in ParticipationWindow/SpotlightPreference opportunities.
F15/F44 add committed CriticalCueEligibility and skippable cosmetic escalation;
F35/F15/F25 add SceneIdentityRevision and source-grounded item/world appearance
continuity. Rumor/NPC/convergence/compound interaction remains in existing
F35–F41; these are deeper acceptance/contracts, not duplicate engines. Standard
progression and source-required rewards remain due regardless of generation failure.

## Rules coverage absent from the archive

Standard 2024 support additionally needs advancement; selected class/subclass,
species/background/feat content; multiclassing where included; spell preparation,
casting and resources; reactions/timing; concentration; saves/condition interactions;
healing/death/rests; equipment/inventory; exploration/hazards; monster features and
encounter rewards. These require source-driven implementation, not legacy parity.
See [Rules coverage](rules-coverage.md).

## Exclusions and open decisions

Do not inherit two seats, mandatory random classes, fixed Charisma/HP bands,
forced Persuasion proficiency, Dash ending the turn, thirty-second combat caps,
fixed enemy/story/languages, Windows/PowerShell assumptions, or authored JavaScript
renderers. The old rules/build/rulings code and tester findings expose those limits.
Browser-generated glue remains allowed; browser speech synthesis is not the
server-produced audio fallback selected for this game.

Local-model trials, an MCP wrapper, vendor brands, the contest schedule, backup
demo videos and a public scene editor are conditional ideas. Renderer fidelity,
device/browser matrix, exact rulebook revisions/catalog, hosting/auth provider,
media deployment, locales and latency/capacity numbers remain explicit decisions
in [Implementation roadmap](implementation-roadmap.md).

Previously reviewed devlog entries `e-20260927-battle-review-failures`,
`e-20260927-abandoned-results`, `e-20260927-streamed-dialogue-leak`, and
`e-20260926-browser-bring-up-five-bugs` justify rendered-output checks, fenced
callbacks, validation before publication, and browser bring-up. Supporting
invariants/evidence remain in [Runtime reliability](runtime-reliability.md) and ADR 0004.
