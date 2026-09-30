# Gap analysis and refinement

Date: 2026-09-30
Status: Planning refinements added; independent review and runtime evidence pending

The supplied ShellHacks assessment describes the archived one-shot. Its strength
labels are hypotheses/historical observations, not independently observed Rust
capabilities. The new game has no Cargo workspace or running server/clients.
[Competitive research](competitive-research.md) already corrects uniqueness claims:
same-screen/voice, deterministic-mechanics claims and cinematic tools exist in the
comparison set. Missing public documentation is unverified, not proof of absence.
The defensible hypothesis is a better actual living-room campaign, measured in
setup, fairness/privacy, continuity, voice/tempo and repeat play—not crate counts.

## Assessment disposition and priorities

“Covered” below means a concrete plan exists; it never means implemented or proved.
The full section-4 row mapping and source digest are retained in
[Gap brief mapping](../development/evidence/gap-brief-mapping.json).

| Assessment area | Disposition and canonical plans |
| --- | --- |
| Same-room party | Protect F01–F03/F19 and persistent two-role shell; COMP-PLAYTEST measures 3–6 and separately larger configured groups, no invented maximum. |
| AI narration/GM | F08/F39/F44 validate permitted claims before speech; narrative alternatives and continuity need actual evidence. |
| Deterministic authority | F07/F10/F11/F23/F24 and G03/G05; engine/session commit owns results, not LLM or animation. |
| Rules breadth | R01–R17 plus RULE-EFFECT-CONTRACT; G07 exact full required catalog/rights still open, SRD alone cannot close it. |
| Combat depth | F11/F12/F40/F41; source action economy/reactions/LOS/range, goals and legal enemy perception. |
| NPC knowledge safety | F36/F38 plus long-horizon refinement; truth/belief/provenance and paired hidden-state tests. |
| Persistent campaign/NPC memory | F24/F27/F36/F38/F39 plus MEMORY-LONGHORIZON-ACCEPT; bounded retrieval/summary truth and recovery expanded. |
| Narrative engine | F39/G10/G11; beats/threads/threats/agency/convergence already modeled, not implemented. |
| World simulation | F35 plus WORLD-TIME-ACCEPT; simulation tiers/catch-up/pause expanded, optional market economy remains X11. |
| Tempo/audiovisual direction | F42–F44/G12; accessibility/ducking/fatigue/permitted profiles; empirical fun/tuning still open. |
| Asset orchestration | F25–F28/F44; canonical dependency/cache/admission/fallback semantics already planned, actual waste/latency unknown. |
| Video/cutscenes | F15/S07/X09; optional quotes/spend, continuity and fallback first; ideal length/frequency need playtests. |
| Images | F15/F25 and provider qualification; style/reference/version evolution reuse immutable identity packs. |
| TTS/STT | F09/F16 plus SPEECH-ROOM-ACCEPT; streaming, voice identity, noisy room, barge-in and stable-clause acceptance. |
| Music/SFX | F17/F43; prepared motifs/stems, fatigue and ducking; live generation/rights conditional on qualification. |
| Creator/worldbuilding | Real producer/interface gap filled by X10 in existing df-content/df-tools; private packages/templates first, public tools X11. |
| Progression/inventory | Required R02/R12/R13/R16/F05/S05/S06; corrected from optional P2 framing to core campaign mechanics. |
| Remote/async | Latest feature outline promotes hosted remote to required F45 core; X11 retains conditional async/notification boundaries. Same actor authority/private leases apply. |
| Content/community | X10 private packs, X11 sharing/remix/moderation/marketplace decisions; no automatic launch community commitment. |
| Reliability/cost | F22–F32/X02/X05/X09; actual OTEL/Pg/recovery/providers/device/COGS evidence still required. |

P0 remains rules/source/effect, durable continuity and causal narrative/NPC state,
with observability and graceful degradation from the first slice. P1 uses the
existing tempo/assets/combat/identity plans. Private authoring/schema/template
production is needed before scalable campaign content; a polished graph editor
can follow stable schemas. Standard progression/inventory is core work even though
the brief labels it P2. Hosted remote is now required F45 core under the later feature outline. Async/community
remain expansion decisions; bounded opt-in custom content is now planned F47 with
distinct ruleset/source/handler admission, while broader homebrew remains conditional.

The earlier brief mapping remains historical research coverage. The later
[Feature refinement](feature-refinement.md) supersedes remote-deferred scope and
adds concrete recap/trailer/custom-content plans; it does not invalidate earlier
source/reliability/privacy requirements.

## Complete research backlog mapping

Each row keeps its original question/technique scope and references one canonical
implementation/acceptance family. Ranges denote existing family plans; they are not
new independent tasks or dispatch authorization. Named acceptance refinements
exercise existing implementations rather than create duplicate implementation owners.

| ID | Subsystem / brief research item | Disposition | Canonical plans / design |
| --- | --- | --- | --- |
| B01 | Narrative engine: Beat/fact graph representation vs scene graphs | Covered plan; continuity expanded | F39-DELIVER/F39-ACCEPT, G10-RESOLVE, G11-RESOLVE, MEMORY-LONGHORIZON-ACCEPT; [narrative-engine.md](narrative-engine.md) |
| B02 | Narrative engine: Threat clocks and world pressure models | Covered plan; continuity expanded | F39-DELIVER/F39-ACCEPT, G10-RESOLVE, G11-RESOLVE, MEMORY-LONGHORIZON-ACCEPT; [narrative-engine.md](narrative-engine.md) |
| B03 | Narrative engine: Narrative gravity without obvious railroading | Covered plan; continuity expanded | F39-DELIVER/F39-ACCEPT, G10-RESOLVE, G11-RESOLVE, MEMORY-LONGHORIZON-ACCEPT; [narrative-engine.md](narrative-engine.md) |
| B04 | Narrative engine: Convergence scoring: player intent × story relevance × plausibility | Covered plan; continuity expanded | F39-DELIVER/F39-ACCEPT, G10-RESOLVE, G11-RESOLVE, MEMORY-LONGHORIZON-ACCEPT; [narrative-engine.md](narrative-engine.md) |
| B05 | Narrative engine: Fail-forward patterns and substitute beats | Covered plan; continuity expanded | F39-DELIVER/F39-ACCEPT, G10-RESOLVE, G11-RESOLVE, MEMORY-LONGHORIZON-ACCEPT; [narrative-engine.md](narrative-engine.md) |
| B06 | Narrative engine: Long-horizon continuity tests and narrative evals | Covered plan; continuity expanded | F39-DELIVER/F39-ACCEPT, G10-RESOLVE, G11-RESOLVE, MEMORY-LONGHORIZON-ACCEPT; [narrative-engine.md](narrative-engine.md) |
| B07 | NPC & interaction engine: Truth vs belief vs rumor representation | Covered plan; memory expanded | F36-DELIVER/F36-ACCEPT, F37-DELIVER, F38-DELIVER, MEMORY-LONGHORIZON-ACCEPT, WORLD-TIME-ACCEPT; [interaction-engine.md](interaction-engine.md) |
| B08 | NPC & interaction engine: Memory salience, decay, consolidation and contradiction handling | Covered plan; memory expanded | F36-DELIVER/F36-ACCEPT, F37-DELIVER, F38-DELIVER, MEMORY-LONGHORIZON-ACCEPT, WORLD-TIME-ACCEPT; [interaction-engine.md](interaction-engine.md) |
| B09 | NPC & interaction engine: Relationship dimensions: trust/respect/fear/debt/etc. | Covered plan; memory expanded | F36-DELIVER/F36-ACCEPT, F37-DELIVER, F38-DELIVER, MEMORY-LONGHORIZON-ACCEPT, WORLD-TIME-ACCEPT; [interaction-engine.md](interaction-engine.md) |
| B10 | NPC & interaction engine: Secrets, promises, obligations and social consequences | Covered plan; memory expanded | F36-DELIVER/F36-ACCEPT, F37-DELIVER, F38-DELIVER, MEMORY-LONGHORIZON-ACCEPT, WORLD-TIME-ACCEPT; [interaction-engine.md](interaction-engine.md) |
| B11 | NPC & interaction engine: NPC schedules / off-screen actions | Covered plan; memory expanded | F36-DELIVER/F36-ACCEPT, F37-DELIVER, F38-DELIVER, MEMORY-LONGHORIZON-ACCEPT, WORLD-TIME-ACCEPT; [interaction-engine.md](interaction-engine.md) |
| B12 | NPC & interaction engine: Natural-language action → structured world interaction | Covered plan; memory expanded | F36-DELIVER/F36-ACCEPT, F37-DELIVER, F38-DELIVER, MEMORY-LONGHORIZON-ACCEPT, WORLD-TIME-ACCEPT; [interaction-engine.md](interaction-engine.md) |
| B13 | Rules & combat: SRD 2024 coverage map | Expanded; exact source unresolved | G07-RESOLVE, R01–R17-MODEL/DELIVER/ACCEPT, RULE-EFFECT-CONTRACT, F11/F12/F40/F41; [rules-effect-model.md](rules-effect-model.md) |
| B14 | Rules & combat: Generic effect/condition model | Expanded; exact source unresolved | G07-RESOLVE, R01–R17-SOURCE/IMPLEMENT/ACCEPT, RULE-EFFECT-CONTRACT, F11/F12/F40/F41; [rules-effect-model.md](rules-effect-model.md) |
| B15 | Rules & combat: Spell and ability execution architecture | Expanded; exact source unresolved | G07-RESOLVE, R01–R17-SOURCE/IMPLEMENT/ACCEPT, RULE-EFFECT-CONTRACT, F11/F12/F40/F41; [rules-effect-model.md](rules-effect-model.md) |
| B16 | Rules & combat: Action economy, reactions and interrupts | Expanded; exact source unresolved | G07-RESOLVE, R01–R17-SOURCE/IMPLEMENT/ACCEPT, RULE-EFFECT-CONTRACT, F11/F12/F40/F41; [rules-effect-model.md](rules-effect-model.md) |
| B17 | Rules & combat: LOS/range/grid abstractions | Expanded; exact source unresolved | G07-RESOLVE, R01–R17-SOURCE/IMPLEMENT/ACCEPT, RULE-EFFECT-CONTRACT, F11/F12/F40/F41; [rules-effect-model.md](rules-effect-model.md) |
| B18 | Rules & combat: Property-based and generated rules tests | Expanded; exact source unresolved | G07-RESOLVE, R01–R17-SOURCE/IMPLEMENT/ACCEPT, RULE-EFFECT-CONTRACT, F11/F12/F40/F41; [rules-effect-model.md](rules-effect-model.md) |
| B19 | Rules & combat: How much flexibility remains with deterministic adjudication | Expanded; exact source unresolved | G07-RESOLVE, R01–R17-SOURCE/IMPLEMENT/ACCEPT, RULE-EFFECT-CONTRACT, F11/F12/F40/F41; [rules-effect-model.md](rules-effect-model.md) |
| B20 | World simulation: Granularity: simulated vs summarized entities | Expanded; optional economy conditional | F35-DELIVER/F35-ACCEPT, WORLD-TIME-ACCEPT, G11-RESOLVE, X11-RESOLVE; [long-horizon-state.md](long-horizon-state.md) |
| B21 | World simulation: Faction goals and clocks | Expanded; optional economy conditional | F35-DELIVER/F35-ACCEPT, WORLD-TIME-ACCEPT, G11-RESOLVE, X11-RESOLVE; [long-horizon-state.md](long-horizon-state.md) |
| B22 | World simulation: Time/travel/environment state | Expanded; optional economy conditional | F35-DELIVER/F35-ACCEPT, WORLD-TIME-ACCEPT, G11-RESOLVE, X11-RESOLVE; [long-horizon-state.md](long-horizon-state.md) |
| B23 | World simulation: Off-screen event generation | Expanded; optional economy conditional | F35-DELIVER/F35-ACCEPT, WORLD-TIME-ACCEPT, G11-RESOLVE, X11-RESOLVE; [long-horizon-state.md](long-horizon-state.md) |
| B24 | World simulation: Economy/resource simulation: what actually adds fun? | Expanded; optional economy conditional | F35-DELIVER/F35-ACCEPT, WORLD-TIME-ACCEPT, G11-RESOLVE, X11-RESOLVE; [long-horizon-state.md](long-horizon-state.md) |
| B25 | World simulation: How simulation feeds narrative without exploding context/cost | Expanded; optional economy conditional | F35-DELIVER/F35-ACCEPT, WORLD-TIME-ACCEPT, G11-RESOLVE, X11-RESOLVE; [long-horizon-state.md](long-horizon-state.md) |
| B26 | Tempo engine: Continuous tension/energy/dread/urgency model | Covered plan; tuning unresolved | F43-DELIVER/F43-ACCEPT, F44-DELIVER, G12-RESOLVE, COMP-PLAYTEST; [tempo-engine.md](tempo-engine.md) |
| B27 | Tempo engine: Phase/state machine: calm → build → anticipation → peak → release | Covered plan; tuning unresolved | F43-DELIVER/F43-ACCEPT, F44-DELIVER, G12-RESOLVE, COMP-PLAYTEST; [tempo-engine.md](tempo-engine.md) |
| B28 | Tempo engine: Adaptive music stems and motif layering | Covered plan; tuning unresolved | F43-DELIVER/F43-ACCEPT, F44-DELIVER, G12-RESOLVE, COMP-PLAYTEST; [tempo-engine.md](tempo-engine.md) |
| B29 | Tempo engine: FX fatigue / novelty rotation | Covered plan; tuning unresolved | F43-DELIVER/F43-ACCEPT, F44-DELIVER, G12-RESOLVE, COMP-PLAYTEST; [tempo-engine.md](tempo-engine.md) |
| B30 | Tempo engine: Camera and screen-language rules | Covered plan; tuning unresolved | F43-DELIVER/F43-ACCEPT, F44-DELIVER, G12-RESOLVE, COMP-PLAYTEST; [tempo-engine.md](tempo-engine.md) |
| B31 | Tempo engine: Phone UI transformations under urgency | Covered plan; tuning unresolved | F43-DELIVER/F43-ACCEPT, F44-DELIVER, G12-RESOLVE, COMP-PLAYTEST; [tempo-engine.md](tempo-engine.md) |
| B32 | Tempo engine: Silence, ducking and reveal stingers | Covered plan; tuning unresolved | F43-DELIVER/F43-ACCEPT, F44-DELIVER, G12-RESOLVE, COMP-PLAYTEST; [tempo-engine.md](tempo-engine.md) |
| B33 | Asset orchestration: Speculative branch prediction | Covered plan; measured budgets unresolved | F25–F28, F44, G08-RESOLVE, G12-RESOLVE, PROVIDER-QUALIFY, PROVIDER-BENCHMARK; [asset-engine.md](asset-engine.md) |
| B34 | Asset orchestration: Priority = probability × importance × reuse ÷ cost | Covered plan; measured budgets unresolved | F25–F28, F44, G08-RESOLVE, G12-RESOLVE, PROVIDER-QUALIFY, PROVIDER-BENCHMARK; [asset-engine.md](asset-engine.md) |
| B35 | Asset orchestration: Canonical asset identity and versioning | Covered plan; measured budgets unresolved | F25–F28, F44, G08-RESOLVE, G12-RESOLVE, PROVIDER-QUALIFY, PROVIDER-BENCHMARK; [asset-engine.md](asset-engine.md) |
| B36 | Asset orchestration: Campaign visual bible / style hash | Covered plan; measured budgets unresolved | F25–F28, F44, G08-RESOLVE, G12-RESOLVE, PROVIDER-QUALIFY, PROVIDER-BENCHMARK; [asset-engine.md](asset-engine.md) |
| B37 | Asset orchestration: Failure and fallback hierarchy | Covered plan; measured budgets unresolved | F25–F28, F44, G08-RESOLVE, G12-RESOLVE, PROVIDER-QUALIFY, PROVIDER-BENCHMARK; [asset-engine.md](asset-engine.md) |
| B38 | Asset orchestration: Cross-vendor abstraction | Covered plan; measured budgets unresolved | F25–F28, F44, G08-RESOLVE, G12-RESOLVE, PROVIDER-QUALIFY, PROVIDER-BENCHMARK; [asset-engine.md](asset-engine.md) |
| B39 | Asset orchestration: Latency/cost budgets and cache policy | Covered plan; measured budgets unresolved | F25–F28, F44, G08-RESOLVE, G12-RESOLVE, PROVIDER-QUALIFY, PROVIDER-BENCHMARK; [asset-engine.md](asset-engine.md) |
| B40 | Speech: Streaming STT and partial-intent extraction | Covered plan; room acceptance expanded | F09/F16, SPEECH-ROOM-ACCEPT, G04-RESOLVE, G08-RESOLVE, G12-RESOLVE, PROVIDER-BENCHMARK; [gap-analysis.md](gap-analysis.md) |
| B41 | Speech: Voice activity detection in noisy living rooms | Covered plan; room acceptance expanded | F09/F16, SPEECH-ROOM-ACCEPT, G04-RESOLVE, G08-RESOLVE, G12-RESOLVE, PROVIDER-BENCHMARK; [gap-analysis.md](gap-analysis.md) |
| B42 | Speech: Barge-in / interruption semantics | Covered plan; room acceptance expanded | F09/F16, SPEECH-ROOM-ACCEPT, G04-RESOLVE, G08-RESOLVE, G12-RESOLVE, PROVIDER-BENCHMARK; [gap-analysis.md](gap-analysis.md) |
| B43 | Speech: Streaming clause-level TTS | Covered plan; room acceptance expanded | F09/F16, SPEECH-ROOM-ACCEPT, G04-RESOLVE, G08-RESOLVE, G12-RESOLVE, PROVIDER-BENCHMARK; [gap-analysis.md](gap-analysis.md) |
| B44 | Speech: Stable NPC voice identity | Covered plan; room acceptance expanded | F09/F16, SPEECH-ROOM-ACCEPT, G04-RESOLVE, G08-RESOLVE, G12-RESOLVE, PROVIDER-BENCHMARK; [gap-analysis.md](gap-analysis.md) |
| B45 | Speech: Emotional delivery controls without latency spikes | Covered plan; room acceptance expanded | F09/F16, SPEECH-ROOM-ACCEPT, G04-RESOLVE, G08-RESOLVE, G12-RESOLVE, PROVIDER-BENCHMARK; [gap-analysis.md](gap-analysis.md) |
| B46 | Images / scenes: Character consistency and references | Covered plan; measured continuity unresolved | F15/F25/F44, G08-RESOLVE, G12-RESOLVE, PROVIDER-BENCHMARK, COMP-OFFER; [asset-engine.md](asset-engine.md) |
| B47 | Images / scenes: Item evolution while retaining identity | Covered plan; measured continuity unresolved | F15/F25/F44, G08-RESOLVE, G12-RESOLVE, PROVIDER-BENCHMARK, COMP-OFFER; [asset-engine.md](asset-engine.md) |
| B48 | Images / scenes: Scene continuity across revisits | Covered plan; measured continuity unresolved | F15/F25/F44, G08-RESOLVE, G12-RESOLVE, PROVIDER-BENCHMARK, COMP-OFFER; [asset-engine.md](asset-engine.md) |
| B49 | Images / scenes: Background → combat arena → cutscene consistency | Covered plan; measured continuity unresolved | F15/F25/F44, G08-RESOLVE, G12-RESOLVE, PROVIDER-BENCHMARK, COMP-OFFER; [asset-engine.md](asset-engine.md) |
| B50 | Images / scenes: Placeholder → generated replacement UX | Covered plan; measured continuity unresolved | F15/F25/F44, G08-RESOLVE, G12-RESOLVE, PROVIDER-BENCHMARK, COMP-OFFER; [asset-engine.md](asset-engine.md) |
| B51 | Images / scenes: When static art beats expensive 3D/video | Covered plan; measured continuity unresolved | F15/F25/F44, G08-RESOLVE, G12-RESOLVE, PROVIDER-BENCHMARK, COMP-OFFER; [asset-engine.md](asset-engine.md) |
| B52 | Live cutscenes: Ideal duration and frequency | Covered optional fidelity; value unresolved | F15, S07-COMPOSE/S07-ACCEPT, G08-RESOLVE, PROVIDER-BENCHMARK, COMP-OFFER; [asset-engine.md](asset-engine.md) |
| B53 | Live cutscenes: Shot-plan schema | Covered optional fidelity; value unresolved | F15, S07-COMPOSE/S07-ACCEPT, G08-RESOLVE, PROVIDER-BENCHMARK, COMP-OFFER; [asset-engine.md](asset-engine.md) |
| B54 | Live cutscenes: Pre-generation from likely branches | Covered optional fidelity; value unresolved | F15, S07-COMPOSE/S07-ACCEPT, G08-RESOLVE, PROVIDER-BENCHMARK, COMP-OFFER; [asset-engine.md](asset-engine.md) |
| B55 | Live cutscenes: First/last-frame and reference-image consistency | Covered optional fidelity; value unresolved | F15, S07-COMPOSE/S07-ACCEPT, G08-RESOLVE, PROVIDER-BENCHMARK, COMP-OFFER; [asset-engine.md](asset-engine.md) |
| B56 | Live cutscenes: Fallback from video → animated still → realtime 3D → narration | Covered optional fidelity; value unresolved | F15, S07-COMPOSE/S07-ACCEPT, G08-RESOLVE, PROVIDER-BENCHMARK, COMP-OFFER; [asset-engine.md](asset-engine.md) |
| B57 | Live cutscenes: Avoiding disruption to social table flow | Covered optional fidelity; value unresolved | F15, S07-COMPOSE/S07-ACCEPT, G08-RESOLVE, PROVIDER-BENCHMARK, COMP-OFFER; [asset-engine.md](asset-engine.md) |
| B58 | Persistence & memory: Event sourcing vs snapshotting | Expanded; migrations/replay evidence unresolved | F24/F27/F36, G05-RESOLVE, G10-RESOLVE, MEMORY-LONGHORIZON-ACCEPT; [long-horizon-state.md](long-horizon-state.md) |
| B59 | Persistence & memory: Campaign resume semantics | Expanded; migrations/replay evidence unresolved | F24/F27/F36, G05-RESOLVE, G10-RESOLVE, MEMORY-LONGHORIZON-ACCEPT; [long-horizon-state.md](long-horizon-state.md) |
| B60 | Persistence & memory: Memory compaction and retrieval | Expanded; migrations/replay evidence unresolved | F24/F27/F36, G05-RESOLVE, G10-RESOLVE, MEMORY-LONGHORIZON-ACCEPT; [long-horizon-state.md](long-horizon-state.md) |
| B61 | Persistence & memory: Versioning generated worlds | Expanded; migrations/replay evidence unresolved | F24/F27/F36, G05-RESOLVE, G10-RESOLVE, MEMORY-LONGHORIZON-ACCEPT; [long-horizon-state.md](long-horizon-state.md) |
| B62 | Persistence & memory: Deterministic replay boundaries when AI output is nondeterministic | Expanded; migrations/replay evidence unresolved | F24/F27/F36, G05-RESOLVE, G10-RESOLVE, MEMORY-LONGHORIZON-ACCEPT; [long-horizon-state.md](long-horizon-state.md) |
| B63 | Persistence & memory: Migration strategy for evolving schemas | Expanded; migrations/replay evidence unresolved | F24/F27/F36, G05-RESOLVE, G10-RESOLVE, MEMORY-LONGHORIZON-ACCEPT; [long-horizon-state.md](long-horizon-state.md) |
| B64 | Creator / worldbuilding tools: Campaign schema UX | Expanded private tools; community conditional | X10-RESOLVE/X10-AUTHOR/X10-ACCEPT, G10-RESOLVE, X11-RESOLVE/X11-ACCEPT; [campaign-authoring.md](campaign-authoring.md) |
| B65 | Creator / worldbuilding tools: Graph editor for beats/threads/factions | Expanded private tools; community conditional | X10-RESOLVE/X10-AUTHOR/X10-ACCEPT, G10-RESOLVE, X11-RESOLVE/X11-ACCEPT; [campaign-authoring.md](campaign-authoring.md) |
| B66 | Creator / worldbuilding tools: Lore ingestion and structured extraction | Expanded private tools; community conditional | X10-RESOLVE/X10-AUTHOR/X10-ACCEPT, G10-RESOLVE, X11-RESOLVE/X11-ACCEPT; [campaign-authoring.md](campaign-authoring.md) |
| B67 | Creator / worldbuilding tools: Reusable NPC/location/item templates | Expanded private tools; community conditional | X10-RESOLVE/X10-AUTHOR/X10-ACCEPT, G10-RESOLVE, X11-RESOLVE/X11-ACCEPT; [campaign-authoring.md](campaign-authoring.md) |
| B68 | Creator / worldbuilding tools: Publishing/remixing/versioning | Expanded private tools; community conditional | X10-RESOLVE/X10-AUTHOR/X10-ACCEPT, G10-RESOLVE, X11-RESOLVE/X11-ACCEPT; [campaign-authoring.md](campaign-authoring.md) |
| B69 | Creator / worldbuilding tools: Safety/moderation/licensing for community content | Expanded private tools; community conditional | X10-RESOLVE/X10-AUTHOR/X10-ACCEPT, G10-RESOLVE, X11-RESOLVE/X11-ACCEPT; [campaign-authoring.md](campaign-authoring.md) |
| B70 | Production systems: Telemetry for perceived latency | Covered plan; core degradation expanded | F02/F24/F26/F28/F30/F31, X02/X05/X09, SPEECH-ROOM-ACCEPT, COMP-PLAYTEST, PROVIDER-BENCHMARK; [expansion-boundaries.md](expansion-boundaries.md) |
| B71 | Production systems: Vendor health/fallback routing | Covered plan; core degradation expanded | F02/F24/F26/F28/F30/F31, X02/X05/X09, SPEECH-ROOM-ACCEPT, COMP-PLAYTEST, PROVIDER-BENCHMARK; [expansion-boundaries.md](expansion-boundaries.md) |
| B72 | Production systems: Token/media cost accounting per session | Covered plan; core degradation expanded | F02/F24/F26/F28/F30/F31, X02/X05/X09, SPEECH-ROOM-ACCEPT, COMP-PLAYTEST, PROVIDER-BENCHMARK; [expansion-boundaries.md](expansion-boundaries.md) |
| B73 | Production systems: Replayable bug reports | Covered plan; core degradation expanded | F02/F24/F26/F28/F30/F31, X02/X05/X09, SPEECH-ROOM-ACCEPT, COMP-PLAYTEST, PROVIDER-BENCHMARK; [expansion-boundaries.md](expansion-boundaries.md) |
| B74 | Production systems: Automated end-to-end eval campaigns | Covered plan; core degradation expanded | F02/F24/F26/F28/F30/F31, X02/X05/X09, SPEECH-ROOM-ACCEPT, COMP-PLAYTEST, PROVIDER-BENCHMARK; [expansion-boundaries.md](expansion-boundaries.md) |
| B75 | Production systems: Load testing 4–6 player sessions | Covered plan; core degradation expanded | F02/F24/F26/F28/F30/F31, X02/X05/X09, SPEECH-ROOM-ACCEPT, COMP-PLAYTEST, PROVIDER-BENCHMARK; [expansion-boundaries.md](expansion-boundaries.md) |
| B76 | Production systems: Offline/degraded mode expectations | Covered plan; core degradation expanded | F02/F24/F26/F28/F30/F31, X02/X05/X09, SPEECH-ROOM-ACCEPT, COMP-PLAYTEST, PROVIDER-BENCHMARK; [expansion-boundaries.md](expansion-boundaries.md) |

## Speech, latency and fair participation acceptance

SPEECH-ROOM-ACCEPT extends F09/F16/G04/G12 with explicit push-to-talk/capture lease
and noisy living-room fixtures: cross-talk/background TV, names/numbers/negation,
accents, denied microphone, sleeping phone, slow network and interrupted narration.
Partial STT can assist bounded prediction; only final validated input can propose
an action, and uncertain joke/question/negation requires clarification. No always-on
ambient capture or emotion inference is introduced. Cache keys for speech/intent
bind permitted speaker/audience/run/context/locale/model/codec/policy, preventing
another speaker's or stale utterance's cached action from executing.

Barge-in is an explicit authenticated capture/input event: stop/duck only permitted
output under the current audio/capture lease, preserve committed outcomes and
captions, and report actual old-buffer cancellation. Rejected/ambiguous input never
becomes a free action; quiet/AFK players receive voluntary opportunities, not social
penalties or automatically spent turns. Streaming TTS uses proven immutable safe
clauses, otherwise full validated buffering. Voice/emotion parameters remain
bounded admitted provider features with an intelligible prepared fallback.

Measure final input to first understood/validated intent, committed result, first
approved audible output and caption timing separately; report p50/p95/p99, sample
count, cold/warm load, noise fixture, failure rate, clarification rate, interruption
errors and actual cost. No vendor inference number is an end-to-end promise. Record
source/build/device/provider revisions and permitted evidence; default logs contain
no raw private speech. Actual audio and both-role browser review are mandatory.

## Competitor and product questions

The brief's twelve research questions map to existing COMP-VERIFY/COMP-PLAYTEST/
COMP-OFFER and PROVIDER-BENCHMARK, without another vendor survey. Authority, memory,
NPC epistemics, story structure/ignored-story behavior, simultaneous/AFK input,
live/prepared media and vendor degradation are documented vendor claims or
unverified internals until operated against fixed versions. Perceived latency,
creator quality, real table enjoyment and two-hour four-person accepted-output
COGS require measured runs; the pricing sheet is a scenario, not that benchmark.
The core test harness includes a two-hour/four-person normalization, then larger
configured campaign capacities separately; it does not impose a four-player cap.

Use one short research record per actual investigation:

| Field | Required entry |
| --- | --- |
| Source / version / date / evidence | Exact primary link/revision and retained observed evidence, or unverified claim |
| Subsystem / owner / current plan | One canonical family/task, relevant source and reviewer |
| Strength / representation | What was actually observed; model/authority boundary, not marketing inference |
| Candidate borrowing | Minimal compatible Rust/server/rights-safe adaptation |
| Tradeoffs / failure modes | Privacy, cancellation, cost, version, recovery and bounded-load risks |
| Prototype / checks | Specific fixture/metrics, build/inputs/spend limits and actual review capabilities |
| Decision / follow-up | Selected/rejected/inconclusive, reasoning, remaining owned gate and task ID |

The causal diagram's rules/world arrow means engine composition of typed candidate
values. It cannot introduce a Rust dependency cycle, a second world writer or an
LLM authority. The corresponding content/retrieval/effect/conditional models are
in [Campaign authoring](campaign-authoring.md),
[Long-horizon state](long-horizon-state.md), [Rules effects](rules-effect-model.md)
and [Expansion boundaries](expansion-boundaries.md).

## Remaining evidence and claim limits

Exact book access/catalog rights, concrete Rust/WASM transport/framework, device/
codec bounds, PostgreSQL layouts/migrations, telemetry pressure/retention, provider
rights/quotes/latency/cost, campaign capacity, narrative/tempo calibration, and
customer renewal remain open owned gates. This pass fills engineering model and
coverage gaps without inventing answers to those experiments. Prior planning
critic PASS applies to its retained source fingerprint; it does not automatically
approve these revisions or any running game. Current approval requires a new
independent review tied to this source/manifest revision.
