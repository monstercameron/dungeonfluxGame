# DungeonFlux detailed backlog review

Status: Independently approved and applied to the authoritative SQLite queue on 2026-09-30. Integrated checks passed.

The backlog contains **3,145 tasks across all 140 systems**:2,838 new explicit outcomes,306 preserved coarse evidence aggregates and one preserved blocked DNS operation. One new protocol mapping record is a non-runnable reference to the canonical df-api codec; the other2,837 new outcomes are atomic planning blueprints. Every new record is pending and nondispatchable. No game source was implemented.

Coverage includes all 41 crates,F01–F47,R01–R17,G01–G12,S00–S08,X01–X12 and the two project-wide acceptance families. The curated outcomes specify concrete inputs/expectations instead of multiplying generic verbs by entities. Source catalogs still await G07; no spell or monster denominator was invented.

## Beginning-to-end arcs

Each system has its applicable arc:contract/data decisions, canonical implementation, consumer wiring, named failure/privacy/recovery fixtures, observable acceptance, and measurement after an accepted integrated baseline. Gates have decisions and component qualification before integrated evidence; slices connect existing producers and consumers. Every fixture depends on its implementation or qualification, and every optimization waits for acceptance. Already-within-budget outcomes may retain measured no-change evidence.

| Phase | New outcomes |
| --- | ---: |
| Contract/data design | 491 |
| Decision/component qualification | 48 |
| Canonical implementation or explicit reference | 502 |
| Use-case/consumer wiring | 274 |
| Negative/failure fixtures | 738 |
| Observable acceptance | 507 |
| Measured optimization | 278 |

## Concrete examples

| System | Beginning | Integration / expected outcome | Failure / expected outcome | End |
| --- | --- | --- | --- | --- |
| C-df-rpc-bridge | Specify continuous byte-stream adaptation → WebSocket boundaries do not define RPC messages | Connect synthetic generated client to native fixture → all four gRPC modes reach real handlers | Split HTTP2 frame across several WebSocket payloads → checksum and stream ordering remain exact | Profile warm calls under mixed stream load → identify bridge overhead separately from provider time |
| C-df-knowledge | Define fact belief memory rumor provenance → summaries never replace canonical truth | Connect native MemoryCandidateStore result to pure ranking → no database or provider I/O enters knowledge | Retrieve stale summary after fact correction → canonical correction wins | Profile ranking and reauthorization at accepted candidate caps → preserve privacy and canonical basis |
| C-df-commerce | Define pure CommerceTransition and native facade distinction → policy has no socket clock provider or database I/O | Connect persistence gateway and controlled egress consumers → pure policy delegates all external I/O | Replay distinct out-of-order invoice events → semantic idempotence preserves legitimate updates | Profile commercial locking and reconciliation at accepted load → exact accounting and tenant authority retained |
| F09 | Specify capture consent and mic-owner lease → one active capture per principal | Connect capture lease to VoiceService → native admits current member grant | Deny microphone permission → no endless pending spinner | Measure STT cache and live latency separately → namespace and miss cost explicit |
| F11 | Define encounter turn view → initiative source-pinned | Bind encounter state to role views → same committed ordering | Duplicate attack command → one spend | Measure turn-loop latency → accepted rules unchanged |
| F45 | Specify remote mixed-room topology → one authoritative actor | Bind remote and local clients → same campaign revision | High-latency remote member → honest pending state | Measure remote tail latency and fanout → qualified capacity |
| F46 | Specify committed recap fact selection → audience permission pinned | Bind bookend skip to ongoing game → no outcome change | Recap includes private fact → reject | Measure prepared bookend latency → no instant claim |
| R10 | Map spell preparation and known options → class source | Integrate spell handlers into action offers → R11 registry reused | Cast unavailable spell → deny | Benchmark spell preparation → complete legal offers |
| X12 | Define customer lifecycle and payer scope → auth identity separate | Integrate payment webhook and reconciliation → canonical commerce persistence | Out-of-order webhook observation → no stale premium grant | Measure accepted commerce operational overhead → loaded support included |

## Ownership, dependencies and completion

The graph has **47084 source-backed edges**, and all 3,145 staged task IDs form a DAG. Shared primitives and source handlers are owned once. Features own their named use-case adapters; slices own composition and end-to-end proof. Explicit case owners include native persistence/media, browser role mounts, public docs, and consumer codecs. Persistent domain types stay in df-model and authoring policy in df-content. Pure code has no provider/DB/SDK/clock/socket I/O.

Original coarse task IDs and their criteria remain as coordinator-owned evidence aggregates. The actual dispatch view excludes aggregates and references even if their dispatch flag is accidentally true. A frozen atomic/operational scope is required. Children and aggregates use the existing independent reviewer/integrated-attempt triggers; unfinished dependencies prevent aggregate completion. Child counts never auto-complete a feature.

G03 freezes the relevant next wave. S00 foundation and G03 component closure do not wait for later G07/G10/G12 game arcs. G06 complete-flow traces and G12 audiovisual evidence follow the corresponding integrated slice. S08 operations grow from S00. Optional fidelity/custom/community decisions require explicit reviewed selection/rescope; cancelled tasks never satisfy prerequisite edges, and the required flat/core game is preserved.

## Verification and remaining gates

Staging uses a SQLite backup of the preserved baseline, not a raw live-file copy. The worker verified all 307 original execution records,140 family states,3 attempt rows and94 devlog rows unchanged; live coordinator/recorder observations remain separately preserved by the root. Integrity and foreign keys pass, with zero dispatch-ready tasks. Existing 31 negative SQL probes plus isolated positive/negative dispatch-role controls pass. Repeated backlog generation is byte-identical. The standard source-refinement pipeline applies the durable catalogue last so an older refiner cannot erase detailed coverage.

The canonical source and staged candidate identities are:

- Governing fingerprint: `3676e59e6316d9e4d77eab1f9dd812776041c2c9eb70f5dc2e396a13e50bf6b0`
- Candidate SHA256: `ae2dfe764bb9b6fa65d31c8838e7ac0d729a1608e7a990af33b3d819f06da844`
- Reviewed selection-schema SHA256: `240475ca0d073ce40561245738878415146327a52af50fe4d91f0c61f08f0456`

Executable Rust/WASM/browser/provider/rules/book/right/device/performance and commercial proof remain unperformed. Exact future filenames, command lines and limits are frozen by prerequisite resolution and scoped dispatch. Required full 2024 support, all 47 features, thin Rust clients, PostgreSQL authority, separate OTEL SQLite, hosted remote play, and honest paid media admission remain intact. Planning count is not implementation or income.

## Per-system inventory

Counts below are new explicit outcomes; preserved coarse aggregate records are additional.

| System | Outcome | D | R | I | W | F | A | O | New total |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| C-df-ai | df-ai public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-api | df-api public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-assets | df-assets public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-audio | df-audio public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-auth | df-auth public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-client | df-client public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-combat | df-combat public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-commerce | df-commerce public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-content | df-content public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-display | df-display public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-encounter | df-encounter public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-engine | df-engine public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-experience | df-experience public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-intent | df-intent public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-interaction | df-interaction public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-knowledge | df-knowledge public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-locale | df-locale public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-media | df-media public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-model | df-model public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-narrative | df-narrative public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-observe | df-observe public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-persistence | df-persistence public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-player | df-player public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-presentation | df-presentation public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-protocol | df-protocol public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-provider-api | df-provider-api public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-providers | df-providers public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-render | df-render public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-rpc-bridge | df-rpc-bridge public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-rules | df-rules public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-server | df-server public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-session | df-session public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-telemetry | df-telemetry public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-tempo | df-tempo public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-testkit | df-testkit public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-tools | df-tools public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-types | df-types public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-ui | df-ui public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-web | df-web public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-workflow | df-workflow public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| C-df-world | df-world public contract and data model | 4 | 0 | 4 | 2 | 5 | 3 | 2 | 20 |
| F01 | Sessions, room codes, invite/QR join, player name/readiness; carry without two-seat limit | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F02 | Stable membership, reload/sleep/reconnect, duplicate-tab input/audio ownership; carry | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F03 | Shared display, personal player views, host controls; carry, host is a permission | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F04 | Character choices, legal build, names, flavor, portraits/reference art, ready/lock; expand | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F05 | Complete sheet, equipment, proficiencies, resources and progression; expand | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F06 | Scenes, exploration, objectives, NPCs, story beats, encounter triggers, resolutions/endings; carry without hardcoded old story | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F07 | Legal actions with labels, choices, disabled reasons, targets/previews; carry with typed/versioned offers | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F08 | Typed dialogue, NPC context, validated interpretation and visible rejection/recovery; carry | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F09 | Hold-to-talk, transcription, cancel, typed fallback, microphone ownership; carry | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F10 | Checks, saves, dice results, modifiers/explanations; expand beyond persuasion/weapon attacks | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F11 | Initiative, actions/reactions, attacks/damage, conditions, enemy decisions/end outcomes; expand | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F12 | Authoritative geometry, reach/path, movement/dash, occupancy, range/visibility; expand beyond fixed grid shortcuts | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F13 | Flat battle presentation, tokens, initiative/HP, camera cues, impact and animation dedupe; carry | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F14 | Splat/3D scenes, LODs, colliders, billboards and cinematic camera effects; conditional fidelity extension with flat fallback | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F15 | Reference sheets, portraits, identity-consistent loops and cinematic finishers; carry capability, stage fidelity | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F16 | Spoken narration/NPC lines, captions, matching fallback speech, scheduling/stop/drain; carry | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F17 | Music, ambience, SFX, loops, gain/duck/crossfade, display/player targeting; carry | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F18 | Player/session locales, catalogs, narrative/speech locale and fallback; carry, language list open | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F19 | Responsive layouts, keyboard/touch, pending/error/connection states, reduced motion/readability; carry | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F20 | Journal/recap, history and public/private knowledge; carry baseline UI, expand durable history | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F21 | Start/pause/resume, authoritative options, timers/new runs; carry, debug force/skip are not normal gameplay | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F22 | Structured RPC, snapshots, audio/mic/assets, auth/origin checks; carry Rust bridge | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F23 | Bounded owner, timer/job generations, registered effects, cancellation/shutdown; carry | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F24 | Durable sessions/members/characters/actions and restart recovery; replace gameplay SQLite with PostgreSQL | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F25 | Immutable manifests/bytes, preload, hashes, bounded browser cache/fallbacks; carry | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F26 | AI/media adapters, deadline/concurrency/fallback/hedging policies; carry interfaces, reselect vendors | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F27 | Exact cache identities, complete recordings, deterministic replay/offline and faults; carry | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F28 | Spend reservation, usage reconciliation and cost reports; carry with exact monetary units | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F29 | Debug state/views/choices/scopes/clients, checkpoints, faults and captures; carry scoped development tools | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F30 | OTEL logs/traces, bounded telemetry queries, full-flow progress evidence; expand | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F31 | Fake providers, controlled simulations/replay, contract/browser playtests; carry stronger output gates | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F32 | Atomic builds, asset preparation, stable previews, owned processes and optimization; carry Rust tooling | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F33 | Queue, frontier evaluation, devlog/escalation/cleanup, six-hour changelog; new workflow | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F34 | Public project page/gallery, acknowledgements/rules attribution; carry separate from runtime | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F35 | Causal world time/travel/weather/resources, due events, NPC schedules/faction movement and threat stages | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F36 | Canonical truth versus observer knowledge/false belief, witnesses/provenance, memory decay and bounded rumor network | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F37 | Unified input interpretation, questions/jokes/meta/uncertainty, source-valid bounded compound plans and partial progress | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F38 | NPC personality/goals/emotion, autonomous reactions, multi-axis relationships, conversations/topics/secrets/promises and social/faction policy | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F39 | Flexible arcs/phases/beat alternatives, threads/hooks, gravity/threat relevance, believable convergence and intervention budgets | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F40 | Appropriate combat/social/chase/hazard/puzzle/survival/negotiation/escape challenges, objectives and failure routes | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F41 | Legal NPC tactics, battlefield objectives, rule-grounded reinforcement/escalation and perception-limited decisions | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F42 | Macro pacing, activity windows and voluntary fair spotlight recommendations from permitted observations | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F43 | Continuous intensity/phase/curve, inertia, bounded event impulses, fatigue/rotation and accessible audience-safe audiovisual/UI profiles | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F44 | Structured semantic moments, concrete audience-safe presentation and ranked bounded predictive asset demand | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F45 | Hosted remote and mixed-room campaigns | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F46 | Audience-safe campaign recaps and speculative trailers | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| F47 | Source-safe generated content and approved custom opt-in | 3 | 0 | 4 | 2 | 5 | 4 | 2 | 20 |
| G01 | Toolchain/targets | 4 | 4 | 0 | 0 | 4 | 2 | 2 | 16 |
| G02 | Transport/runtime | 4 | 4 | 0 | 0 | 4 | 2 | 2 | 16 |
| G03 | Shared contracts | 4 | 4 | 0 | 0 | 4 | 2 | 2 | 16 |
| G04 | Identity/devices | 4 | 4 | 0 | 0 | 4 | 2 | 2 | 16 |
| G05 | Durable state/assets | 4 | 4 | 0 | 0 | 4 | 2 | 2 | 16 |
| G06 | Telemetry | 4 | 4 | 0 | 0 | 4 | 2 | 2 | 16 |
| G07 | Rules/content | 4 | 4 | 0 | 0 | 4 | 2 | 2 | 16 |
| G08 | Providers/presentation budgets | 4 | 4 | 0 | 0 | 4 | 2 | 2 | 16 |
| G09 | Agent runner/evidence | 4 | 4 | 0 | 0 | 4 | 2 | 2 | 16 |
| G10 | Campaign authoring and replay | 4 | 4 | 0 | 0 | 4 | 2 | 2 | 16 |
| G11 | Causal interaction and bounded directors | 4 | 4 | 0 | 0 | 4 | 2 | 2 | 16 |
| G12 | Experience, tempo and predictive assets | 4 | 4 | 0 | 0 | 4 | 2 | 2 | 16 |
| P00 | Complete source-backed project plan corpus | 2 | 0 | 0 | 2 | 3 | 2 | 1 | 10 |
| P01 | Final project acceptance | 2 | 0 | 0 | 2 | 3 | 2 | 1 | 10 |
| R01 | Ability generation/assignment, creation sequence, legal choices, proficiencies/languages and derived statistics | 4 | 0 | 6 | 2 | 8 | 6 | 2 | 28 |
| R02 | Selected classes through levels 1–20, subclass features, advancement, resource recovery, multiclass prerequisites/progression | 4 | 0 | 6 | 2 | 8 | 6 | 2 | 28 |
| R03 | Selected species, backgrounds, origin/general/fighting-style/epic-boon feats and their prerequisites/options | 4 | 0 | 6 | 2 | 8 | 6 | 2 | 28 |
| R04 | D20 tests: checks, saves, attacks, proficiency/expertise, advantage/disadvantage, modifiers and specific exceptions | 4 | 0 | 6 | 2 | 8 | 6 | 2 | 28 |
| R05 | Initiative/turn order, actions/bonus actions/reactions, ready/delay triggers where supported by source, interruptions and timing | 4 | 0 | 6 | 2 | 8 | 6 | 2 | 28 |
| R06 | Movement/speed modes, distance/reach/range, terrain, space/occupancy, sight/light/cover, hiding and opportunity triggers | 4 | 0 | 6 | 2 | 8 | 6 | 2 | 28 |
| R07 | Weapon/unarmed attacks, grappling/shoving, weapon mastery and selected class/feat attack interactions | 4 | 0 | 6 | 2 | 8 | 6 | 2 | 28 |
| R08 | Damage types, resistance/vulnerability/immunity, temporary HP, healing, unconsciousness/death and death saves | 4 | 0 | 6 | 2 | 8 | 6 | 2 | 28 |
| R09 | All selected rules conditions, durations, stacking/replacement and removal | 4 | 0 | 6 | 2 | 8 | 6 | 2 | 28 |
| R10 | Spellcasting/preparation/known options, slots/rituals, components, ranges/areas/targets, saves/attacks and duration | 4 | 0 | 6 | 2 | 8 | 6 | 2 | 28 |
| R11 | Every selected spell and class/subclass/feat feature, including summons/transformation and exceptional rules | 4 | 0 | 6 | 2 | 8 | 6 | 2 | 28 |
| R12 | Equipment/inventory, carrying/access, armor/shields/weapons, currencies, tools/consumables and selected magic items/attunement | 4 | 0 | 6 | 2 | 8 | 6 | 2 | 28 |
| R13 | Short/long rests, recovery, exhaustion and other sustained resource/time effects | 4 | 0 | 6 | 2 | 8 | 6 | 2 | 28 |
| R14 | Exploration/social mechanics, searching/stealth/perception, travel, environmental hazards, traps, falling and other selected hazards | 4 | 0 | 6 | 2 | 8 | 6 | 2 | 28 |
| R15 | Selected monster stat blocks/features, senses, movement, spells, recharge and special encounter actions | 4 | 0 | 6 | 2 | 8 | 6 | 2 | 28 |
| R16 | Encounter rewards, XP/milestone policy, advancement/retraining where source permits, treasure and ongoing campaign resources | 4 | 0 | 6 | 2 | 8 | 6 | 2 | 28 |
| R17 | DM procedures and optional modules explicitly selected from the pinned core-book scope | 4 | 0 | 6 | 2 | 8 | 6 | 2 | 28 |
| S00 | Contract and execution foundation | 2 | 0 | 0 | 4 | 4 | 2 | 2 | 14 |
| S01 | First durable two-role slice | 2 | 0 | 0 | 4 | 4 | 2 | 2 | 14 |
| S02 | Creation and first source-faithful rules | 2 | 0 | 0 | 4 | 4 | 2 | 2 | 14 |
| S03 | Story, dialogue, voice and audio | 2 | 0 | 0 | 4 | 4 | 2 | 2 | 14 |
| S04 | Combat, spells and tactical rendering | 2 | 0 | 0 | 4 | 4 | 2 | 2 | 14 |
| S05 | Campaign continuity and controls | 2 | 0 | 0 | 4 | 4 | 2 | 2 | 14 |
| S06 | Complete required rules/catalog | 2 | 0 | 0 | 4 | 4 | 2 | 2 | 14 |
| S07 | Conditional fidelity | 2 | 0 | 0 | 4 | 4 | 2 | 2 | 14 |
| S08 | Delivery and developer operations | 2 | 0 | 0 | 4 | 4 | 2 | 2 | 14 |
| X01 | Security, access and data lifecycle | 4 | 0 | 4 | 2 | 6 | 4 | 2 | 22 |
| X02 | Deployment, operations and capacity | 4 | 0 | 4 | 2 | 6 | 4 | 2 | 22 |
| X03 | Model routing and escalation | 4 | 0 | 4 | 2 | 6 | 4 | 2 | 22 |
| X04 | Source provenance, standard coverage and catalog refinement | 4 | 0 | 4 | 2 | 6 | 4 | 2 | 22 |
| X05 | Performance, resource and degradation budgets | 4 | 0 | 4 | 2 | 6 | 4 | 2 | 22 |
| X06 | Agent evidence, process refinement and changelog | 4 | 0 | 4 | 2 | 6 | 4 | 2 | 22 |
| X07 | Accessibility, device behavior and user recovery | 4 | 0 | 4 | 2 | 6 | 4 | 2 | 22 |
| X08 | Minute-cadence code quality review and deduplicated intake | 4 | 0 | 4 | 2 | 6 | 4 | 2 | 22 |
| X09 | Campaign pricing and profitability | 4 | 0 | 4 | 2 | 6 | 4 | 2 | 22 |
| X10 | Private campaign authoring and content lifecycle | 4 | 0 | 4 | 2 | 6 | 4 | 2 | 22 |
| X11 | Conditional expansion decisions and honest degraded play | 4 | 0 | 4 | 2 | 6 | 4 | 2 | 22 |
| X12 | Recoverable customer commerce and entitlement lifecycle | 4 | 0 | 4 | 2 | 6 | 4 | 2 | 22 |

## Coordinator integration

The live queue contains 3,145 tasks, 140 families and 47,084 dependency edges. All 307 original task execution states, three attempts and 197 pre-apply devlog entries were preserved; later review/recorder observations remain appended. Integrity, foreign keys, the complete dependency DAG and 31 negative transition checks pass. No task is dispatch-ready. The frozen independent review remains tied to the candidate SHA256 above; this status update records the subsequent live apply.
