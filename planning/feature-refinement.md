# Requested feature refinement and priority overlay

Date: 2026-09-30
Status: New user outline adopted as planned scope; implementation/evidence pending

The supplied 15-feature outline extends the existing plans. It is a product
hypothesis, not verified competitor absence, a market moat or proven customer
preference. Reuse [Competitive research](competitive-research.md) and its scoped
future playtests rather than accepting the pasted reference tokens as citations.
The core remains full-stack Rust, thin persistent TV/player clients, source-faithful
standard 2024 rules, PostgreSQL authority and correlated OTEL/SQLite diagnostics.

This feature-adoption pass added three capability families within the then-current40
crates (later service refinement adds independent df-commerce): F45 hosted remote
play is required core; F46 covers audience-safe campaign recaps/speculative trailers;
F47 covers source-compatible generated content and explicitly opted-in approved
custom definitions. Async/public community/marketplace remain conditional. Read
[Remote play](remote-play.md), [Campaign cinematics](campaign-cinematics.md) and
[Generated content](generated-content.md). There are no new RPC services/tables.
Concrete additional typed fields/ports remain G03 prerequisites.

## Fifteen numbered ideas

| Idea | Canonical plan | Concrete refinement / acceptance |
| --- | --- | --- |
| 1. Make the phones indispensable | F03/F07/F36; F03-PRIVATE-ACCEPT | Private contextual offers and disclose-to-party intent; no hidden payloads in public display. |
| 2. Turn the Tempo Engine into a signature feature | F43/F44/G12 | Typed accessible profiles; urgency cannot remove legal actions or invent a timer. |
| 3. Predictive media generation | F25–F28/F44/G08/G12 | Bounded heuristic forecasts; ready hits fast, misses have nonblocking permitted fallback, no instant guarantee. |
| 4. Make NPCs actual agents | F36/F38/MEMORY-LONGHORIZON-ACCEPT | Existing truth/belief/goals/secrets/relationship/obligation models, with NPC performed by validated expression. |
| 5. Rumors and social propagation | F36/F38/WORLD-TIME-ACCEPT | Witness/source/contact-path provenance, deterministic bounded rumor fanout/TTL and accepted logical time. |
| 6. Narrative convergence | F39/G10/G11 | Plausible bounded convergence must preserve choice, truth, NPC motivation and ignored/failed arcs. |
| 7. "Previously on DungeonFlux" | F46-MODEL/DELIVER/ACCEPT | Audience-permitted committed history; optional quoted video plus still/narration fallback and consented export. |
| 8. End-of-session trailer | F46-MODEL/DELIVER/ACCEPT | Explicit speculative label, known-thread-only imagery, no secrets/future fact/forced scene. |
| 9. Player spotlight director | F42; F42-SPOTLIGHT-ACCEPT | Opt-in observed interaction windows and consented credible hooks; no boredom inference or forced disclosure. |
| 10. Cinematic reactions to critical events | F15/F44; F15-CRITICAL-ACCEPT | Committed source-valid critical event eligibility; skippable cue/fallback, no outcome retcon or unauthorized game freeze. |
| 11. Generate bespoke legendary items | F47/F05/R12/R16 | Personalized origin/media on legal template; new mechanics only explicitly disclosed/approved custom RulesetId/handler. |
| 12. Dynamic boss design | F40/F41/F47/R15 | Approved source-compatible boss phases/objectives/hazards reflect committed history; no invented HP/effect/physics. |
| 13. Persistent world simulation | F35; WORLD-TIME-ACCEPT | Schedules/factions/resources advance under accepted campaign time; paused world does not run off wall time. |
| 14. Live environmental transformation | F35/F15/F25; F35-CONTINUITY-ACCEPT | Stable scene identity/version and validated world/geometry consequences with late-asset/stale-revisit safety. |
| 15. The "anything" interaction layer | F37/F38/R14/G11 | Bounded legal compound plans, step revalidation/partial outcomes; unsupported wine/physics uses NeedsRuling, never unrestricted simulation. |

## Fifteen priority rows

These retain the user's P0/P1/P2 priorities. Priority is importance, not removal
of source/authority/integration prerequisites. P0 cinema means eligibility,
canonical identity and prepared/skippable fallback contracts early; paid live
video quality follows measured cost/latency and actual device gates. The outline
also puts live cutscenes in Phase 3; that is the later fidelity/value test, not a
contradictory mandate to spend video before a playable campaign exists.

| Feature | Requested priority | Canonical plan |
| --- | --- | --- |
| Phone-as-character controller | P0 | F03/F07; F03-PRIVATE-ACCEPT |
| Tempo Engine | P0 | F43/F44/G12 |
| Predictive asset generation | P0 | F25–F28/F44/G08 |
| Live cinematic cutscenes | P0 | F15/F44; F15-CRITICAL-ACCEPT |
| NPC cognition + memory | P0 | F36/F38 |
| Narrative convergence engine | P0 | F39 |
| Persistent campaign state | P0 | F24/S05 |
| World simulation | P1 | F35 |
| Director-level pacing | P1 | F42 |
| Dynamic encounter generation | P1 | F40/F41 |
| Character continuity across media | P1 | F15/F25/F44; F35-CONTINUITY-ACCEPT |
| Private information channels | P1 | F03/F36; F03-PRIVATE-ACCEPT |
| Campaign recap cinematics | P1 | F46 |
| Player spotlight director | P1 | F42; F42-SPOTLIGHT-ACCEPT |
| AI-generated mechanics/content | P2 | F47 |

## Table stakes and phase delivery

Hosted remote is now explicitly selected. Earlier conditional-remote wording is
superseded; the same session actor and audience/lease model apply everywhere.
Required progression/rules/privacy/persistence cannot wait for optional spectacle.
Private creator packages/templates have their existing X10 producer contract;
a polished public graph editor/community remains separate conditional scope.

| Required table stake | Canonical plan |
| --- | --- |
| Persistent campaigns | F24/S05 |
| Full character progression | F05/R02/R10–R13/R16/S05/S06 |
| Real rules coverage | R01–R17/G07/S06 |
| Remote multiplayer | F45/G04/G08/S01/S03/S05 |
| Campaign/world creation | X10/G10 |
| Campaign templates | X10/df-content |
| Save/resume | F02/F24/F27/S05 |
| Character sheets | F05/S02 |
| Stable combat | F11/F12/F40/F41/S04 |
| Scene maps | F12/F13/F25/S04 |

| Product phase | Every requested item | Integration order |
| --- | --- | --- |
| Phase 1 — Make it a product | persistent campaigns; rules breadth; NPC persistence; save/resume; character progression; remote multiplayer | S01/S02/S04/S05/S06 + F45; foundation, source/catalog and reliable campaign acceptance first. |
| Phase 2 — Make it DungeonFlux | Narrative Engine; NPC Interaction Engine; Tempo Engine; Asset Orchestrator; phone interaction model; predictive generation | S03 + F03/F35–F39/F42–F44; policy contracts begin early, integrated accessible voice/stills precede polish. |
| Phase 3 — Make it magic | world simulation; rumor network; dynamic encounters; live cutscenes; private information; spotlight director; cinematic recaps; persistent visual evolution | S03/S04/S05/S07 + F46/F47; deepen already required privacy/continuity and add optional paid fidelity after budgets/latency. |

The phases overlay existing S00–S08 dependency order. No second competing roadmap
or duplicate implementation queue is created. Privacy/world/NPC/tempo schemas and
observability begin early; later phases deepen them with tested full campaign
coverage, participant capacity and optional generation value. Nothing here
approves an application scaffold, live provider call, deployment or customer test.

## Phones as contextual private world interfaces

`ContextualPrivateOffer` binds known fact/attributed belief, actor/run/source basis,
permitted targets/action-offer IDs, audience/reveal rights, expiry and presentation
variant. `KnowledgeCue` may present a recognition/vision/warning or private whisper
only when supported by disclosed knowledge, character source features or explicit
approved story context. Different subjective knowledge is permitted; the server
cannot invent incompatible canonical truths or leak others' hidden state.

F03/F07/F36 share server-derived cues/options with ordinary ListOptions/Submit and
private Watch/Listen projections. A RevealToParty selection is an explicit scoped
player action with eligible facts/recipients, revalidated and committed before
public projection; knowing a secret is not automatic consent to announce it.
Quietly declining creates no forced promise, check or resource spend. Private
voice routing uses [Remote play](remote-play.md); missing safe private playback
falls back to permitted phone text, never TV captions/audio. Reconnect/rebind clears
obsolete/private cues while retaining legal drafts; private caches/manifests remain
scoped. There is no new generic private-event injection API or client rules engine.

F03-PRIVATE-ACCEPT checks paired states, private clues/false beliefs, scoped disclosure,
public TV/music/prefetch noninterference, whisper routing, urgent reaction options,
expiry/revocation and stable focus/drafts. Tempo can emphasize but not simplify away
legal actions, add a fake timer or shorten a rule window. A still/quiet phone must
retain accessible input and all relevant legal choices.

## Rumors, spotlight and grounded encounters

Existing F36/F38 propagate attributed claims through actual witnesses/contact paths,
with hop/fanout/TTL/dedupe bounds and source-aware forgetting. Distortion changes a
belief, not canonical truth; no witness means no invented reputation consequence.
F35 advances those events only through accepted game time, not a paused overnight.
Rumor/loyalty/secret/promise tests extend existing memory/world acceptance rather
than create another social engine.

`ParticipationWindow` is a bounded aggregate of permitted accepted interaction
counts/time and explicit preferences. It is not emotion/boredom measurement, covert
microphone analysis, real-person ranking or an incentive to spam actions.
`SpotlightPreference` explicitly opts into voluntary opportunities; the director
uses credible consented hooks and NPC/world knowledge to propose a
`SpotlightOpportunity` with source/basis, eligible audience, expiry and rejection
path. Player/backstory secrets require their own disclosure scope. Ignoring a cue
is valid; no compelled speech, invented check bonus, punishment or automatic turn
spend follows. The candidate passes narrative/interaction/rules validation through
the same engine owner; actual delivery/rejection is committed and deduplicated.

F42-SPOTLIGHT-ACCEPT checks dominant/quiet/AFK/declining participants, spam/counter
bounds, stale opportunities, consent revocation, secret hooks and credible refusal.
Fairness tests measure opportunities and voluntary feedback, not equal forced
speaking shares or an unproven boredom classifier. Dynamic encounter/boss objectives
and terrain reflect committed choices through approved F40/F41 rules/templates;
source-unsupported novel effects follow F47's explicit custom gate.

## Planning completion and remaining evidence

The source-digested [Feature brief mapping](../development/evidence/feature-brief-mapping.json)
retains all 15 numbered ideas, 15 priority rows, 10 table stakes and three phase
lists. F45/F46/F47 have bounded MODEL/DELIVER/ACCEPT blueprints; four targeted
acceptance plans extend existing implementation families. All remain pending until
scoped source/contracts/limits are frozen and actual independent output checks pass.

G01–G12 remain real gates: source/catalog access/rights, browser transport, targets,
private capture/audio/network topology, PostgreSQL/recovery, telemetry load,
provider qualification/cost, and story/tempo calibration. Customer/media claims
need actual fixed-build playtests. Forecast scores are heuristics; cache misses
cannot promise instant assets. No generated mechanic counts toward missing
standard rule support. Paid video is optional, explicitly quoted and bounded;
nonblocking permitted stills/voice/captions always preserve legal play.
