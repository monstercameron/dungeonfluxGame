# Runtime directors and simulation

Date: 2026-09-30
Status: User outline adopted as planned contracts; implementation/calibration pending

## Authority and composition

The four supplied narrative, interaction, major-subsystem and tempo outlines are
the design inputs for this revision. They supersede the earlier monolithic story
policy in `df-engine`; their sample numbers are illustrative policy scores, not
D&D mechanics, calibrated probabilities or promised quality/cost targets.

There are 40 planned crates. Ten additions have narrow concrete authorities:

| Crate / feature | Authority | Persistent or contract model in df-model |
| --- | --- | --- |
| df-world / F35 | What changes through world causality and game time | WorldState, WorldEntity, WorldTime, Schedule, TravelLeg, EnvironmentalState, ThreatClock, WorldDelta |
| df-knowledge / F36 | Who observed/knows/believes/remembers a claim | CanonicalFact, KnowledgeGrant, Belief, EvidenceSource, WitnessRecord, Memory, RumorTransmission, KnowledgeDelta |
| df-intent / F37 | What input means and whether it proposes an action | RawInputRef, IntentDisposition, IntentProposal, ActionPlan, PlanStep, IntentBasis |
| df-interaction / F38 | Motivated NPC/object/faction reactions | NpcState, Relationship, ConversationState, TopicGraph, SecretPolicy, Obligation, InteractionProposal |
| df-narrative / F39 | Why events matter and which plausible opportunity to propose | NarrativeState, Arc, PhaseCondition, StoryBeat, Thread, CharacterHook, NarrativeBudget, NarrativeProposal |
| df-encounter / F40 | Which valid challenge and objectives suit circumstances | EncounterRequest, EncounterPlan, EncounterObjective, EscapeCondition, ChallengeBudget |
| df-combat / F41 | Legal NPC tactics and authored battlefield escalation | CombatPolicyState, TacticalObservation, TacticalProposal, ReinforcementTrigger |
| df-experience / F42 | Macro pacing and fair voluntary participation opportunities | ExperienceState, ActivityWindow, SpotlightOpportunity, PacingRecommendation |
| df-tempo / F43 | Moment-to-moment audiovisual intensity | TempoState, TempoFrame, TempoPolicy, EffectFatigue, TempoCueIntent |
| df-presentation / F44 | How permitted events/profiles become audiovisual/UI plans | PresentationPlan, AudiencePresentationBasis, TimelineCue, AssetDemand, ForecastCandidate |

Shared persistence-bearing records are defined once in `df-model` to avoid cyclic
crate imports and competing serializers. Policy-specific request/error types live
in their consuming subsystem. `df-content` defines versioned immutable authoring
records: arcs/beats/delivery alternatives, threats, NPC traits/schedules/topics,
entity affordances, encounter templates/tactics, knowledge dissemination and
experience/tempo/presentation policies. Shared IDs/units remain in `df-types`.
No rulebook catalog, provider API or graph database is duplicated by these crates.

A causal diagram is not a Rust dependency graph. Proposals pass through `df-engine`
as typed values; directors never call each other recursively. The engine orders a
bounded decision pass: validate input/basis; rules preparation/resolution; world
and interaction consequences; knowledge updates; narrative/experience/encounter
recommendations; tempo; permitted presentation. Intermediate values belong to a
candidate working copy. A proposal that requires further rules choices/reactions,
a ruling or async interpretation becomes an explicit pending state. It cannot
skip prerequisites. Rejecting a whole decision publishes no candidate changes.

`df-session` serializes all accepted inputs and commits the selected state change,
operation result, ordered facts, decisions and effect intents atomically with the
PostgreSQL owner fence and current revision. Apply/publication/external execution
follow that commit. No director owns a task, timer, DB transaction or authoritative
mutable singleton outside this path. `df-server` supplies executors to `df-session`;
AI/media calls are durable effects, not I/O inside pure policy functions.

## Pure public boundaries

Each operation below borrows an immutable state/content basis and returns owned,
bounded candidate output. Explicit logical time, supplied dice where required,
policy revision and causal input IDs make decisions reproducible. Functions return
local typed errors (`InvalidInput`, `UnknownContent`, `StaleBasis`, `Capacity`,
`NeedsRuling`, or `UnsupportedPolicy` as relevant); domain decline/no change is a
successful typed outcome, not an error. No function reaches a system clock or LLM.

| Crate | Public operation shape | Output/critical guarantee |
| --- | --- | --- |
| df-world | advance(WorldBasis, TimeAdvance, WorldPolicy); affordances(EntityQuery) | WorldDelta and eligible affordances; bounded due-event catch-up, game-time pause/travel policy, no invented geometry/physics |
| df-knowledge | perceive(Observation, KnowledgeBasis); query(Subject, ObserverScope); propagate(PropagationBatch); decay(GameTime) | scoped evidence/beliefs and KnowledgeDelta; truth immutable except authorized world transition, no secret context in an NPC query |
| df-intent | classify(InputEnvelope, IntentBasis, optional validated SemanticCandidate); validate(ActionPlan, current basis) | Action, Question, Social, PlanOnly, Meta, Joke, Clarify or Rejected disposition; language proposal cannot authorize an action |
| df-interaction | react(PerceivedEvent, NpcBasis, InteractionPolicy); plan(InteractionRequest, WorldBasis) | bounded reaction/social/object proposal with motivation/evidence and rules request; narrative request may be declined |
| df-narrative | evaluate(NarrativeBasis, CandidateOpportunities, NarrativePolicy) | bounded scored proposal or NoIntervention; prerequisites and coincidence/agency budget checked, no forced beat success |
| df-encounter | propose(EncounterRequest, CurrentWorldBasis); validate(EncounterPlan, RulesCatalog) | source-legal template/composition/objectives or explicit gap; no uncommitted enemies/rewards |
| df-combat | choose(TacticalObservation, LegalActionSet, CombatPolicy) | selected legal action/reinforcement request or NoAction; rules own initiative/reactions/resources, perception limits enemy knowledge |
| df-experience | observe(CommittedActivity, ExperienceState); recommend(ActivityWindow, ExperiencePolicy) | bounded pacing/spotlight request; inactivity is observation, not measured boredom or involuntary participation |
| df-tempo | advance(TempoState, PermittedTempoInputs, PresentationTime, TempoPolicy) | TempoFrame plus bounded impulse intent/delta; continuous inertia/fatigue, no new mechanical deadline or changed action legality |
| df-presentation | compose(PermittedMoment, TempoFrame, AudiencePresentationBasis); forecast(PermittedForecastBasis, DemandBudget) | typed PresentationPlan/AssetDemand; no remote code, hidden-info cue leak or direct provider dispatch |

Every basis binds session/run, relevant state/content/policy versions and causal
IDs. An async semantic/media result returns the admitted basis and job ID: the
owner revalidates before accepting it. Typed plans use stable step/cue IDs and
count/byte/time limits. Idempotent commit/result handling prevents repeated
convergence, rumors, reinforcement or impact on retry. Bounded iterative work can
produce an explicit continuation request; it never loops until the story feels
complete. A full queue defers/rejects optional work visibly without losing required
rules outcomes. Cancellation before admission discards a proposal; committed
continuations/effects follow session/run scope and generation fences.

## Time, recovery and bounded work

World time advances through accepted travel/rest/time-advance decisions and
scheduled logical events under approved pause policy. Real elapsed wall time does
not silently advance a threat while a campaign is stopped. Presentation time is a
separate monotonic anchor; cosmetic interpolation cannot advance WorldTime or
rules timers. Due events carry unique IDs and deterministic ordering, with bounded
catch-up and an explicit remaining backlog after recovery.

State recovery uses versioned snapshots plus committed decision/fact records and
effect intents. A replayable decision record preserves chosen proposals, actual
ordered draws, source/policy/model versions and accepted semantic outputs. Replay
applies those records without LLM or paid provider calls; full event reconstruction
is enabled only for versions with complete deterministic reducers/migrations and
matching state-hash tests. Unsupported old versions return an explicit replay gap,
never regenerate narrative or pretend telemetry is a recovery log. Optional exact
presentation replay also needs retained cue/asset/timebase records; unavailable
assets yield a stated limitation. Reload/recovery never rerolls, respawns completed
encounters or resends completed one-shots.

Work scales with active relevance, not all NPCs every frame: due-event queues,
bounded neighborhood/knowledge queries, capped convergence candidates, rumor
hop/fanout/TTL/deduplication and memory retention. Scheduled important NPCs can use
coarse updates; distant entities retain durable summary state. Select/test concrete
bounds under G10/G11/G12, including campaign/session capacity and actor latency.

`df-presentation::forecast` ranks likely reusable assets using labeled heuristic
scores. `df-media` executes only admitted AssetDemand effects through existing
provider/budget ports. Prefetch has lower priority, concurrency/byte/expiry limits,
reserved campaign allowance, cache keys and cancellation scope; never a paid call
per NPC/tick. Retire obsolete speculative demand without deleting referenced assets.
Unknown supplier spend remains reserved/reconciled; prefetch cannot consume a
customer's unapproved video budget. Prepared-only/replay misses never invoke live
providers. See [Pricing and costs](pricing-and-costs.md).

## Added feature projections

[Feature refinement](feature-refinement.md) extends existing authorities with
contextual private phone cues/disclosure, consented activity-based spotlight,
rumor/NPC depth, source-legal dynamic boss objectives and world/media evolution.
[Campaign cinematics](campaign-cinematics.md) adds observer-safe recaps/trailers and
committed critical-event eligibility. [Generated content](generated-content.md)
keeps generated mechanics candidate-only until source/handler/opt-in admission.
[Remote play](remote-play.md) uses the same actor/directors for remote and local
participants. These additions do not introduce another director, DB writer,
network loop or crate dependency. Standard rules/progression/rewards remain due
regardless of provider/media failure.

## Acceptance and unresolved gates

G10 freezes authoring/model/replay schemas; G11 freezes bounded causal/social/NPC
policy and legal interaction source coverage; G12 freezes experiential/tempo
profiles, audience privacy/accessibility, device budgets and cost calibration.
See [Implementation roadmap](implementation-roadmap.md). These are resolution
and implementation tasks, not completed proofs.

Use controlled clocks/dice/semantic fixtures for beat alternatives, offscreen
consequences, rumor provenance, NPC ignorance/refusal, compound partial outcomes,
legal tactics, repetition/fatigue and replay/hash comparison. Test stale async
results, pause/restart/fenced commits, invalid authored graphs, hidden-state
noninterference and provider-call bounds. Frontier output review then plays both
Rust/WASM roles through quiet/build/reveal/conflict/release, observes audio,
reconnects mid-cue and confirms agency/accessibility/latency. An LLM critic alone
cannot certify story quality, fun, probability calibration or profitable capacity.
