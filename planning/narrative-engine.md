# Structural narrative engine

Date: 2026-09-30
Status: Outline adopted; authored campaign and policy calibration pending

`df-narrative` owns story relevance and believable opportunity selection.
[Runtime directors](runtime-directors.md) defines composition, authority, types,
bounds and recovery. State lives in the same authoritative `df-model::GameState`,
not a second narrative database; separation is by typed state/policy boundary.

## Authoring and state

A `CampaignStructure` defines arcs, flexible phases, `StoryBeat` requirements,
canonical fact references, threat stages and alternative delivery strategies.
A beat identifies required information/conditions, preconditions, compatible
character hooks, importance/flexibility, consequences and substitute deliveries.
Its dependency means prerequisite understanding, not completion of a fixed quest
scene. The same location fact can become known by testimony, notes, observation
or a legally resolved magical investigation. Knowledge grants record the actual
recipient; one player's private discovery is not automatically party knowledge.

`NarrativeState` persists current arcs/phases, active/dormant/completed/failed beats,
open world/mystery/character threads, player-declared interests and evidenced
inferred interests, recent choices/interventions, threat references, narrative
gravity, world momentum and intervention/coincidence budgets. `CharacterHook`
contains consented backstory/goals/relationships/values/fears/secrets and audience
scope; personal hooks never disclose a player's secret to the group by default.
Content/world own truth and threat progression; narrative refers to those records
and proposes consequences without becoming another clock writer.

`ContentPack::validate` rejects duplicate/missing IDs, cyclic beat prerequisites,
invalid phase predicates, unreachable authored alternatives, absent fact/NPC/source
references, contradictory unique truths and invalid cue/asset references. Validate
substitute-delivery equivalence and at least one feasible path under stated
conditions; failure/refusal can select an alternative rather than fabricate access.
Loops in repeatable world events need explicit iteration/TTL bounds instead of
being disguised as acyclic beat dependencies. Validate/migrate against pinned
schema/content/policy versions; editing a pack cannot silently retcon a live run.

## Director pass

`NarrativeDirector::evaluate` observes committed/rules-resolved events and permitted
candidate opportunities. It updates threads/interests, evaluates beat/phase
conditions, searches bounded convergence candidates, then returns a proposal or
`NoIntervention`. Convergence may join a player goal, open thread, story beat and
existing world event only when location, chronology, NPC knowledge/motivation and
canonical truth permit it. `df-interaction` can reject an implausible NPC delivery.
The engine then chooses another opportunity; it never puppets an ignorant NPC.

Score components include story/player/character relevance, plausibility, novelty
and urgency, with repetition, coercion and coincidence penalties. Persist chosen
candidate/components, causal basis, relevant alternatives and policy revision so
an operator can explain the result. Example decimal scores in the supplied
outline are heuristics; calibration against actual sessions is G12 work.

Gravity increases the salience of unresolved threats through credible clues,
character connections and consequences. It cannot teleport players, secretly
rewrite their intent, guarantee required beats, erase a valid choice or punish
out-of-world disconnects. Intervention budget replenishes during approved free
play and caps strong coincidences/events. Players may ignore, oppose or abandon
an arc; the world can progress to an authored alternate/failure state. Pause,
threat stages and game-time advance follow `df-world`, not chat wall time.

Fail-forward means a failure may open a source-valid costly alternative or change
circumstances. It never grants forbidden information, converts a failed die into
success, repeats a roll without permission or consumes an unapproved resource.
A failed persuasion can leave a plausible trail only if the NPC's valid action
and world circumstances create one. Arc transitions use explicit predicates (for
example a threshold of disclosed prerequisites), not a hard-coded linear demo.

## Realization and acceptance

A selected `NarrativeMoment` identifies committed event/fact references, permitted
participants/location, semantic mood/focus and presentation importance. It goes to
`df-presentation`, tempo and bounded AI/media effects. AI expresses the approved
moment; invented claims remain candidate text and cannot create canonical facts,
completed beats, dice results or hidden knowledge. Validate text before captions
or speech; apply the separately gated stable-clause contract only when proven.

Acceptance includes alternative deliveries, private versus party knowledge,
blocked NPC disclosure, ignored-arc consequences, pause/travel threat behavior,
excessive coincidence rejection, personal-hook privacy, fail-forward rules
integrity, source edits/replay, stale completions and bounded candidate/LLM work.
Frontier playtests check agency and comprehensibility; measured scores cannot
prove a story is fun. F39/G10/G11/G12 own remaining tasks and evidence.
