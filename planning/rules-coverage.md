# Standard 2024 rules coverage

Date: 2026-09-29
Status: Required coverage families planned; source pinning and implementation pending

## Scope and source gate

The target is standard 2024 fifth-edition D&D, not the archived demo's rules.
The coverage families below are the planning baseline for a complete rules engine
and content catalog. No mechanics or catalogs have been implemented in this project.
An early playable subset must be labeled as a subset; it cannot close the standard
support feature. Read [Rules support](rules-support.md),
[Feature inventory](feature-inventory.md), and [Subsystem interfaces](subsystem-interfaces.md).

Use primary sources. The official [SRD page](https://www.dndbeyond.com/srd)
currently identifies SRD 5.2.1, including a downloadable fixed document. It is a
useful initial mechanics/catalog source but omits core-book content. The official
[Player's Handbook overview](https://wpn.wizards.com/en/products/2024-players-handbook)
and [Basic Rules index](https://www.dndbeyond.com/sources/dnd/br-2024) establish
the core categories; neither a changing web index nor the old implementation is
a frozen source revision. The revised
[Monster Manual](https://wpn.wizards.com/en/products/2024-monster-manual) was published in 2025; book
publication year must not be confused with the selected 2024 rules family.

Before rules implementation, the coordinator/source owner must record the exact
mechanics baseline, required book/catalog manifest and errata policy. Pin every
source with publisher/title/version/language, acquisition date, immutable checksum
or retained revision, section/page locator and permitted content use/attribution.
The planning candidate is SRD 5.2.1 plus the selected 2024 Player's Handbook core
catalog and compatible Dungeon Master's Guide/Monster Manual material required
by these families. Book access and usable catalog content outside the SRD are
explicit prerequisites; do not claim access, import unprovided book text or silently
reduce the target to the SRD. Expansion books/legacy options require an explicit
additional catalog decision and never mix editions by accident.

`RulesetId` pins mechanics implementation, catalog and source/errata revisions.
Sessions keep that ID; changes require an explicit validated migration or a new
session. Localized labels retain stable mechanic IDs. Content provenance is not
lost when an asset, spell, monster or character build is saved.

## Capability matrix

All rows currently have status **planned**. A family is complete only against its
pinned catalog, source clauses, supported interactions and integrated checks.
“Selected” means the catalog chosen at the source gate, not a worker's convenient
subset. Core player options needed for standard support stay on the completion
backlog even when the first slice exposes fewer of them.

| ID | Family / required shape | Owner and API/read model | Important acceptance cases |
| --- | --- | --- | --- |
| R01 | Ability generation/assignment, creation sequence, legal choices, proficiencies/languages and derived statistics | df-content catalogs; df-rules::validate_build; creation offers/sheet | prerequisites, missing/duplicate choices, correct recalculation, persisted draft/finalization; no forced demo class/stat bands |
| R02 | Selected classes through levels 1–20, subclass features, advancement, resource recovery, multiclass prerequisites/progression | df-rules creation/advancement; df-engine pending choices | level boundaries, feature replacement/stacking, mixed progression and resource totals; no fabricated high-level feature |
| R03 | Selected species, backgrounds, origin/general/fighting-style/epic-boon feats and their prerequisites/options | df-content immutable IDs; server ListOptions/Submit | legal alternatives, repeatability restrictions, ability/feature interactions, source-specific exceptions |
| R04 | D20 tests: checks, saves, attacks, proficiency/expertise, advantage/disadvantage, modifiers and specific exceptions | df-rules::prepare/resolve; RollResultView | opposing modifiers, conditional features, exact arithmetic/rounding, natural-roll rules by test type, passive checks; no universal critical-success shortcut |
| R05 | Initiative/turn order, actions/bonus actions/reactions, ready/delay triggers where supported by source, interruptions and timing | df-rules combat; df-engine pending resolution/reaction; EncounterView | one legal resource spend, nested trigger ordering, turn/round boundaries, interrupt before/after outcome, skipped/incapacitated actors |
| R06 | Movement/speed modes, distance/reach/range, terrain, space/occupancy, sight/light/cover, hiding and opportunity triggers | df-rules::reachable/resolve; TacticalView/Preview | path costs, large creatures, occupied/hidden spaces, threatened movement, permitted information; no client path authority or Dash-ending-turn house rule |
| R07 | Weapon/unarmed attacks, grappling/shoving, weapon mastery and selected class/feat attack interactions | df-rules combat/equipment modules; attack offers | legal target, mastery eligibility, attack-versus-save distinction, bounded additional attacks, ammunition/equipment/resource spending |
| R08 | Damage types, resistance/vulnerability/immunity, temporary HP, healing, unconsciousness/death and death saves | df-rules health; CharacterView/EncounterView | ordering/rounding, excess damage, temporary-HP replacement, stabilization/revival and zero-HP interactions |
| R09 | All selected rules conditions, durations, stacking/replacement and removal | df-rules condition system; permitted condition views | condition immunity, source ownership, saves/end triggers, incapacitation restrictions, effect expiry on correct boundary |
| R10 | Spellcasting/preparation/known options, slots/rituals, components, ranges/areas/targets, saves/attacks and duration | df-content spell catalog; df-rules casting; spell offers | components/costs, legal targeting, resource use, concentration, dispel/counterspell interactions, upcasting, simultaneous effects; no AI-resolved spell |
| R11 | Every selected spell and class/subclass/feat feature, including summons/transformation and exceptional rules | data plus explicit df-rules effect modules | one source-linked behavior fixture per entry plus relevant interaction fixtures; unsupported entries never appear as usable standard choices |
| R12 | Equipment/inventory, carrying/access, armor/shields/weapons, currencies, tools/consumables and selected magic items/attunement | df-content items; df-rules equipment; inventory offers | transfer/drop/use, occupied hands/equipment restrictions, charges, attunement limits, durable quantities and exact money; no silent item duplication |
| R13 | Short/long rests, recovery, exhaustion and other sustained resource/time effects | df-rules rests; df-world time proposals; df-engine logical-time transitions | interrupted/invalid rests, recovery ordering, prevention features, persisted resources after restart |
| R14 | Exploration/social mechanics, searching/stealth/perception, travel, environmental hazards, traps, falling and other selected hazards | df-rules exploration; df-encounter/df-interaction source-valid proposals; df-engine composition | source-grounded checks and outcomes, party participation, revealed knowledge, hazard timing; freeform narration cannot fabricate legal modifiers |
| R15 | Selected monster stat blocks/features, senses, movement, spells, recharge and special encounter actions | df-content monsters; df-rules shared resolution; df-combat perception-limited legal tactics | same attack/save/condition rules as players, recharge/timing, multi-part actions, legal enemy choices; hidden facts filtered from clients |
| R16 | Encounter rewards, XP/milestone policy, advancement/retraining where source permits, treasure and ongoing campaign resources | df-narrative progression proposals; df-engine exactly-once awards; df-rules build changes; journal/sheet | explicit configured progression policy, exactly-once award, pending legal level choices, cross-session/run recovery |
| R17 | DM procedures and optional modules explicitly selected from the pinned core-book scope | df-content/df-engine; authorized host offers | identify optional versus standard behavior, typed rulings, preserved provenance; unresolved module scope remains visible, not silently counted complete |

R17 includes a source-gate checklist for downtime/crafting, additional encounter/
hazard procedures, optional tactical conventions and any requested campaign systems.
These are not permission to add every expansion-book system. The coordinator records
whether each is required, optional configured content or an explicit open decision;
normal host controls cannot inject arbitrary rule changes. Time caps are optional
session policy and cannot be presented as a standard combat rule.

The candidate [Effect and trigger model](rules-effect-model.md) refines R05–R13
source-linked execution, pending windows and meaningful golden/property tests.
Standard advancement/inventory/rewards remain required campaign work, not optional
platform scope. RULE-EFFECT-CONTRACT refines existing family implementations.

## Resolution and tabletop ambiguity

Keep mechanics pure and deterministic with explicit dice/logical time. The engine
owns pending choices, rolls and reactions with stable identity; the session owner
commits the result, resources and dice counters together. Narration and cinematic
effects follow committed outcomes. Monster AI or language models may propose only
legal inputs and cannot invent AC, DC, damage, conditions or character features.

Implement general precedence and entry-specific exceptions from the pinned source.
When tabletop judgment is required, use a typed `NeedsRuling` with the source
question, permitted inputs and an authorized host resolution. Persist the decision
and disclose it as a ruling, not a universal standard rule. If no authorized ruling
flow exists yet, report a rules gap; don't approximate behind a plausible response.
This flow grants only scoped adjudication, not operator force-dice/debug permissions.

## Tracking and completion gates

The source/catalog manifest records stable entry IDs, family, source locator,
implementation/fixture references, unsupported interactions and coverage status.
The authorized [planning database](plan-database.md) records these families and
their source/model/delivery/acceptance plans before the Rust runner exists.
Generate detailed atomic tasks for the next integrated families/catalog
batch, not thousands of speculative per-spell tasks at once. One cohesive feature
may cover many data entries through a shared tested mechanic.

Require source-linked normal/boundary/failure fixtures, interaction checks for
timing/resource/condition conflicts, deterministic replay, and server-side legal
offer versus submit consistency. Each integrated family checks the real permitted
views, PostgreSQL persistence and OTEL decision evidence. Frontier evaluation
operates the resulting creation/combat/spell/rest/progression flow under ADR 0005.

Completion requires the pinned required catalog denominator, all required mechanics
and interaction gates, visible explicit gaps and verified integrated behavior.
Counts alone are insufficient: “all class names” or “all spell IDs” does not prove
their behavior. Until that gate passes, document the supported subset and leave
full standard support open. Post-integration optimization preserves source fidelity.
