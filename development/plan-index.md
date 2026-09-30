# Project plan index

Current source-backed **service-repair candidate**, independent re-review pending.
No application runtime, payment/customers or rights clearance exists.

Source `98b4badc06df2254a0032fd9ecfae373c82359e92602a45c01a10f42c126fd4c`; manifest `e9370eccceaa34350c1c679c81da174dae8026fa2d5c5f3571beb734d670b6de`.

## Current counts

- Families: **140**
- Actual SQLite plans, including blocked DNS: **307**
- Dependencies: **1186**
- Crates: **41**; game feature families: **47**; rules: **17**
- Gates: **12**; delivery slices: **9**
- RPC services/methods: **12/41**; governing sources: **44**
- Dispatch-ready: **0**; original attempts preserved: **3**; devlogs: **45**

## Current service repair

ASR-01..12 map concrete customer/payment/entitlement, hierarchical spend, dispatch,
tenant/privacy/restore, browser/speech/ruling and commercial contracts to canonical
plans in [mapping](evidence/service-defect-mapping.json).
[Commerce](../planning/commerce-service.md), [operations](../planning/service-operations.md)
and [commercial evidence](../planning/commercial-validation.md) are governing refinements.
Exactly C-df-commerce/X12 and ten pending plans were added; no schema or game scope changed.
Current offline [model](evidence/service-economics.json) and
[exact checks](evidence/service-economics-checks.json) do not prove profit/demand/capacity.

## Historical adoption reviews

Gap review source110f2a.../135 families284plans and feature review
source231a6f.../138families297plans remain immutable historical scopes, available in
[evidence/gap-critic/review.json](evidence/gap-critic/review.json) and
[evidence/feature-critic/feature-review.json](evidence/feature-critic/feature-review.json).
They do not approve subsequent commerce/service changes. All76 research rows and
15ideas/15priorities/10stakes/3phases remain mapped; F45remote core, F46bookends and
F47disclosedcustom option preserve their full required designs.

## Family index

| ID | Kind | Title | Plans |
| --- | --- | --- | --- |
| C-df-ai | crate | df-ai public contract and data model | 2 |
| C-df-api | crate | df-api public contract and data model | 2 |
| C-df-assets | crate | df-assets public contract and data model | 2 |
| C-df-audio | crate | df-audio public contract and data model | 2 |
| C-df-auth | crate | df-auth public contract and data model | 2 |
| C-df-client | crate | df-client public contract and data model | 2 |
| C-df-combat | crate | df-combat public contract and data model | 2 |
| C-df-commerce | crate | df-commerce public contract and data model | 2 |
| C-df-content | crate | df-content public contract and data model | 2 |
| C-df-display | crate | df-display public contract and data model | 2 |
| C-df-encounter | crate | df-encounter public contract and data model | 2 |
| C-df-engine | crate | df-engine public contract and data model | 2 |
| C-df-experience | crate | df-experience public contract and data model | 2 |
| C-df-intent | crate | df-intent public contract and data model | 2 |
| C-df-interaction | crate | df-interaction public contract and data model | 2 |
| C-df-knowledge | crate | df-knowledge public contract and data model | 2 |
| C-df-locale | crate | df-locale public contract and data model | 2 |
| C-df-media | crate | df-media public contract and data model | 2 |
| C-df-model | crate | df-model public contract and data model | 2 |
| C-df-narrative | crate | df-narrative public contract and data model | 2 |
| C-df-observe | crate | df-observe public contract and data model | 2 |
| C-df-persistence | crate | df-persistence public contract and data model | 2 |
| C-df-player | crate | df-player public contract and data model | 2 |
| C-df-presentation | crate | df-presentation public contract and data model | 2 |
| C-df-protocol | crate | df-protocol public contract and data model | 2 |
| C-df-provider-api | crate | df-provider-api public contract and data model | 2 |
| C-df-providers | crate | df-providers public contract and data model | 2 |
| C-df-render | crate | df-render public contract and data model | 2 |
| C-df-rpc-bridge | crate | df-rpc-bridge public contract and data model | 2 |
| C-df-rules | crate | df-rules public contract and data model | 2 |
| C-df-server | crate | df-server public contract and data model | 2 |
| C-df-session | crate | df-session public contract and data model | 2 |
| C-df-telemetry | crate | df-telemetry public contract and data model | 2 |
| C-df-tempo | crate | df-tempo public contract and data model | 2 |
| C-df-testkit | crate | df-testkit public contract and data model | 2 |
| C-df-tools | crate | df-tools public contract and data model | 2 |
| C-df-types | crate | df-types public contract and data model | 2 |
| C-df-ui | crate | df-ui public contract and data model | 2 |
| C-df-web | crate | df-web public contract and data model | 2 |
| C-df-workflow | crate | df-workflow public contract and data model | 2 |
| C-df-world | crate | df-world public contract and data model | 2 |
| F01 | capability | Sessions, room codes, invite/QR join, player name/readiness; carry without two-seat limit | 2 |
| F02 | capability | Stable membership, reload/sleep/reconnect, duplicate-tab input/audio ownership; carry | 2 |
| F03 | capability | Shared display, personal player views, host controls; carry, host is a permission | 3 |
| F04 | capability | Character choices, legal build, names, flavor, portraits/reference art, ready/lock; expand | 2 |
| F05 | capability | Complete sheet, equipment, proficiencies, resources and progression; expand | 2 |
| F06 | capability | Scenes, exploration, objectives, NPCs, story beats, encounter triggers, resolutions/endings; carry without hardcoded old story | 2 |
| F07 | capability | Legal actions with labels, choices, disabled reasons, targets/previews; carry with typed/versioned offers | 2 |
| F08 | capability | Typed dialogue, NPC context, validated interpretation and visible rejection/recovery; carry | 2 |
| F09 | capability | Hold-to-talk, transcription, cancel, typed fallback, microphone ownership; carry | 3 |
| F10 | capability | Checks, saves, dice results, modifiers/explanations; expand beyond persuasion/weapon attacks | 2 |
| F11 | capability | Initiative, actions/reactions, attacks/damage, conditions, enemy decisions/end outcomes; expand | 2 |
| F12 | capability | Authoritative geometry, reach/path, movement/dash, occupancy, range/visibility; expand beyond fixed grid shortcuts | 2 |
| F13 | capability | Flat battle presentation, tokens, initiative/HP, camera cues, impact and animation dedupe; carry | 2 |
| F14 | capability | Splat/3D scenes, LODs, colliders, billboards and cinematic camera effects; conditional fidelity extension with flat fallback | 2 |
| F15 | capability | Reference sheets, portraits, identity-consistent loops and cinematic finishers; carry capability, stage fidelity | 3 |
| F16 | capability | Spoken narration/NPC lines, captions, matching fallback speech, scheduling/stop/drain; carry | 2 |
| F17 | capability | Music, ambience, SFX, loops, gain/duck/crossfade, display/player targeting; carry | 2 |
| F18 | capability | Player/session locales, catalogs, narrative/speech locale and fallback; carry, language list open | 2 |
| F19 | capability | Responsive layouts, keyboard/touch, pending/error/connection states, reduced motion/readability; carry | 2 |
| F20 | capability | Journal/recap, history and public/private knowledge; carry baseline UI, expand durable history | 2 |
| F21 | capability | Start/pause/resume, authoritative options, timers/new runs; carry, debug force/skip are not normal gameplay | 2 |
| F22 | capability | Structured RPC, snapshots, audio/mic/assets, auth/origin checks; carry Rust bridge | 2 |
| F23 | capability | Bounded owner, timer/job generations, registered effects, cancellation/shutdown; carry | 2 |
| F24 | capability | Durable sessions/members/characters/actions and restart recovery; replace gameplay SQLite with PostgreSQL | 3 |
| F25 | capability | Immutable manifests/bytes, preload, hashes, bounded browser cache/fallbacks; carry | 2 |
| F26 | capability | AI/media adapters, deadline/concurrency/fallback/hedging policies; carry interfaces, reselect vendors | 2 |
| F27 | capability | Exact cache identities, complete recordings, deterministic replay/offline and faults; carry | 2 |
| F28 | capability | Spend reservation, usage reconciliation and cost reports; carry with exact monetary units | 2 |
| F29 | capability | Debug state/views/choices/scopes/clients, checkpoints, faults and captures; carry scoped development tools | 2 |
| F30 | capability | OTEL logs/traces, bounded telemetry queries, full-flow progress evidence; expand | 2 |
| F31 | capability | Fake providers, controlled simulations/replay, contract/browser playtests; carry stronger output gates | 2 |
| F32 | capability | Atomic builds, asset preparation, stable previews, owned processes and optimization; carry Rust tooling | 2 |
| F33 | capability | Queue, frontier evaluation, devlog/escalation/cleanup, six-hour changelog; new workflow | 2 |
| F34 | capability | Public project page/gallery, acknowledgements/rules attribution; carry separate from runtime | 3 |
| F35 | capability | Causal world time/travel/weather/resources, due events, NPC schedules/faction movement and threat stages | 4 |
| F36 | capability | Canonical truth versus observer knowledge/false belief, witnesses/provenance, memory decay and bounded rumor network | 3 |
| F37 | capability | Unified input interpretation, questions/jokes/meta/uncertainty, source-valid bounded compound plans and partial progress | 2 |
| F38 | capability | NPC personality/goals/emotion, autonomous reactions, multi-axis relationships, conversations/topics/secrets/promises and social/faction policy | 3 |
| F39 | capability | Flexible arcs/phases/beat alternatives, threads/hooks, gravity/threat relevance, believable convergence and intervention budgets | 2 |
| F40 | capability | Appropriate combat/social/chase/hazard/puzzle/survival/negotiation/escape challenges, objectives and failure routes | 2 |
| F41 | capability | Legal NPC tactics, battlefield objectives, rule-grounded reinforcement/escalation and perception-limited decisions | 2 |
| F42 | capability | Macro pacing, activity windows and voluntary fair spotlight recommendations from permitted observations | 3 |
| F43 | capability | Continuous intensity/phase/curve, inertia, bounded event impulses, fatigue/rotation and accessible audience-safe audiovisual/UI profiles | 2 |
| F44 | capability | Structured semantic moments, concrete audience-safe presentation and ranked bounded predictive asset demand | 2 |
| F45 | capability | Hosted remote and mixed-room campaigns | 3 |
| F46 | capability | Audience-safe campaign recaps and speculative trailers | 3 |
| F47 | capability | Source-safe generated content and approved custom opt-in | 3 |
| G01 | gate | Toolchain/targets | 1 |
| G02 | gate | Transport/runtime | 1 |
| G03 | gate | Shared contracts | 1 |
| G04 | gate | Identity/devices | 1 |
| G05 | gate | Durable state/assets | 1 |
| G06 | gate | Telemetry | 1 |
| G07 | gate | Rules/content | 1 |
| G08 | gate | Providers/presentation budgets | 1 |
| G09 | gate | Agent runner/evidence | 1 |
| G10 | gate | Campaign authoring and replay | 1 |
| G11 | gate | Causal interaction and bounded directors | 1 |
| G12 | gate | Experience, tempo and predictive assets | 1 |
| P00 | planning | Complete source-backed project plan corpus | 1 |
| P01 | planning | Final project acceptance | 1 |
| R01 | rules | Ability generation/assignment, creation sequence, legal choices, proficiencies/languages and derived statistics | 3 |
| R02 | rules | Selected classes through levels 1–20, subclass features, advancement, resource recovery, multiclass prerequisites/progression | 3 |
| R03 | rules | Selected species, backgrounds, origin/general/fighting-style/epic-boon feats and their prerequisites/options | 3 |
| R04 | rules | D20 tests: checks, saves, attacks, proficiency/expertise, advantage/disadvantage, modifiers and specific exceptions | 3 |
| R05 | rules | Initiative/turn order, actions/bonus actions/reactions, ready/delay triggers where supported by source, interruptions and timing | 3 |
| R06 | rules | Movement/speed modes, distance/reach/range, terrain, space/occupancy, sight/light/cover, hiding and opportunity triggers | 3 |
| R07 | rules | Weapon/unarmed attacks, grappling/shoving, weapon mastery and selected class/feat attack interactions | 3 |
| R08 | rules | Damage types, resistance/vulnerability/immunity, temporary HP, healing, unconsciousness/death and death saves | 3 |
| R09 | rules | All selected rules conditions, durations, stacking/replacement and removal | 4 |
| R10 | rules | Spellcasting/preparation/known options, slots/rituals, components, ranges/areas/targets, saves/attacks and duration | 3 |
| R11 | rules | Every selected spell and class/subclass/feat feature, including summons/transformation and exceptional rules | 3 |
| R12 | rules | Equipment/inventory, carrying/access, armor/shields/weapons, currencies, tools/consumables and selected magic items/attunement | 3 |
| R13 | rules | Short/long rests, recovery, exhaustion and other sustained resource/time effects | 3 |
| R14 | rules | Exploration/social mechanics, searching/stealth/perception, travel, environmental hazards, traps, falling and other selected hazards | 4 |
| R15 | rules | Selected monster stat blocks/features, senses, movement, spells, recharge and special encounter actions | 3 |
| R16 | rules | Encounter rewards, XP/milestone policy, advancement/retraining where source permits, treasure and ongoing campaign resources | 3 |
| R17 | rules | DM procedures and optional modules explicitly selected from the pinned core-book scope | 3 |
| S00 | slice | Contract and execution foundation | 2 |
| S01 | slice | First durable two-role slice | 2 |
| S02 | slice | Creation and first source-faithful rules | 2 |
| S03 | slice | Story, dialogue, voice and audio | 2 |
| S04 | slice | Combat, spells and tactical rendering | 2 |
| S05 | slice | Campaign continuity and controls | 2 |
| S06 | slice | Complete required rules/catalog | 2 |
| S07 | slice | Conditional fidelity | 2 |
| S08 | slice | Delivery and developer operations | 2 |
| X01 | crosscutting | Security, access and data lifecycle | 3 |
| X02 | crosscutting | Deployment, operations and capacity | 2 |
| X03 | crosscutting | Model routing and escalation | 2 |
| X04 | crosscutting | Source provenance, standard coverage and catalog refinement | 2 |
| X05 | crosscutting | Performance, resource and degradation budgets | 2 |
| X06 | crosscutting | Agent evidence, process refinement and changelog | 2 |
| X07 | crosscutting | Accessibility, device behavior and user recovery | 2 |
| X08 | crosscutting | Minute-cadence code quality review and deduplicated intake | 2 |
| X09 | crosscutting | Campaign pricing and profitability | 8 |
| X10 | crosscutting | Private campaign authoring and content lifecycle | 3 |
| X11 | crosscutting | Conditional expansion decisions and honest degraded play | 2 |
| X12 | crosscutting | Recoverable customer commerce and entitlement lifecycle | 3 |
