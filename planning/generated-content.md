# Source-safe generated content and explicit custom rules

Date: 2026-09-30
Status: F47 planned; source/handler approval gate and implementation pending

The new outline selects generation of bespoke items/enemies/spells as a future
capability. Preserve required standard 2024 fifth-edition support. In standard
mode, generated content is a source-compatible approved template/definition with
personalized name, origin, lore and media. The example Stormward damage redirection
is not automatically an approved standard item or a new standard bonus.
See [Rules coverage](rules-coverage.md), [Rules effects](rules-effect-model.md),
[Campaign authoring](campaign-authoring.md) and [Asset engine](asset-engine.md).

## Candidate, validation and admission

`ContentCandidate` binds generator/model/policy and catalog/source revisions,
permitted event/context, proposed stable definition ID, legal template/handler,
typed bounded parameters, origin/provenance, audience and budget/execution mode.
LLM output is a candidate, never a live executable rule or canonical inventory entry.
`ContentValidation` returns StandardCompatible, NeedsCustomApproval,
UnsupportedMechanic, SourceGap or Rejected with safe source-linked diagnostics.
`ContentAdmission` preserves authorized approval, approved definition hash/version,
RulesetId/content pack and an explicit award/spawn operation identity.

`df-content` validates schema/catalog/rights/template limits. Pure `df-rules`
validates target/timing/resource/effect/condition interactions from approved
source-linked handlers. `df-encounter`/`df-combat` propose source-valid composition
and legal tactics; narrative ties origin to permitted committed history.
`df-ai` proposes bounded candidates as admitted native effects; `df-engine`/
`df-session` revalidate current basis before publishing a definition/award/spawn.
`df-persistence` stores immutable admission/provenance and exactly-once identities.
`df-media` generates only authorized cosmetic assets through budget/asset ports.

Dynamic bosses combine approved creatures, legal special actions, authored
objective/escape/reinforcement/hazard templates and committed player consequences.
Ritual anchors and collapse have explicit target, geometry, source-valid effect
and partial-state transitions. No LLM invents HP, free summons, phase immunity,
damage, physics or action economy. Missing supported mechanics becomes a catalog
gap or scoped NeedsRuling, not a plausible cinematic success. Encounter/boss
generation remains in existing F40/F41; F47 does not create another encounter owner.

## Optional custom definitions are a distinct ruleset

F47-MODEL freezes a reviewed deterministic handler/template allowlist, parameter/
power/complexity/trigger bounds, source/rights provenance, compatibility fixtures,
approval authority and migration policy. New mechanics require explicit host/
campaign opt-in with participant disclosure, distinct immutable RulesetId/custom
catalog version and persisted approved definition. Normal standard campaigns do
not silently adopt these mechanics. This is selected opt-in capability planning;
broader arbitrary rules engines/scripts and mixed editions remain unselected.

Creators/models cannot upload Rust/WASM/JS scripts or arbitrary DSL execution.
A custom idea outside reviewed deterministic handlers stays NeedsCustomApproval/
UnsupportedMechanic until an independently reviewed source task implements/tests
the bounded handler. Host approval alone does not make unimplemented code safe,
source-compatible or standard. No implementation task is dispatch-ready until
F47-MODEL and applicable G03/G07/G10/G11 gates freeze the exact boundary.

An approved custom item/spell/enemy definition carries all relevant action/reaction,
trigger/stacking/duration/resource/timing/target/geometry semantics and advertised
limitations. Replay stores actual chosen definition/hash, handler revision/draws
and accepted outcomes; it makes no generation calls. Changing/removing content
requires explicit source/state/effect-compatible migration or rejection/new run.
Progression/upgrade rules remain pinned; image edits do not alter mechanics.

## Failure, privacy and acceptance

Reject unsupported/source-mismatched/oversized/recursive candidates before
publication, gameplay resource spend or client offers. Provider admission may
already have incurred billed generation/waste; it is still recorded/reconciled. Duplicate admissions/awards return
the same authorized result; payload mismatch rejects. Pre-admission cancel discards
the candidate, accepted run-owned work survives client disconnect and stale results
cannot overwrite a newer catalog/run. Generated private lore, secret boss plans,
reference packs and validation diagnostics follow observer access before any
provider request, manifest or view. Record decision/validation IDs and safe OTEL
status/usage, not raw private content. Unknown paid generation outcomes remain
reserved/reconciled; fallback uses an approved standard item/encounter or declines
an optional new generated offer explicitly without fabricating a free resource.
Provider failure cannot revoke a committed award or withhold a source-required
progression/quest reward; preserve due mechanics and use approved/prepared cosmetic
fallbacks while generation is unavailable.

F47-DELIVER implements bounded approved standard-compatible templates first and
only frozen explicitly selected custom handlers. F47-ACCEPT checks incompatible
source/edition, unsupported redirection/trigger, excessive budgets, approval and
participant disclosure, standard-mode rejection, legal boss phases/hazards,
duplicate/stale award, migration/replay and private cosmetics. Source-linked
goldens/property/interaction tests and real creation/equip/cast/combat/progression
flows are required. Standard R01–R17 completion remains independent of custom
capability; novel generated content cannot close missing standard mechanics.
