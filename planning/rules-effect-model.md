# Source-grounded effects and interaction testing

Date: 2026-09-30
Status: Candidate internal representation; exact source/catalog/type gate pending

[Rules coverage](rules-coverage.md) remains the complete-support denominator.
The brief's “rules DSL” is a research option, not permission to run downloaded
code or approximate 2024 D&D with a universal invented effect language. `df-rules`
owns closed typed mechanics and source-specific handlers; `df-content` supplies
validated immutable parameters/provenance. Freeze concrete variants at G03/G07
before implementing consumers. No new crate or public generic script API is added.

## Effects, conditions and timing

`EffectDefinition` identifies source entry/clause, mechanical handler version,
legal parameters/targets, prerequisites, costs, duration/timing and explicit
stack/replace/removal policy. `ActiveEffect` records stable instance/origin,
source/target, start/expiry in rules time, concentration linkage where applicable,
and relevant choices. A condition is a source-grounded mechanic reference; matching
English labels alone do not establish identical behavior or stacking rules.

`TriggerWindow` identifies causal event, phase/order, eligible participants,
source-defined interrupt options and pending choice deadline/pause policy.
`PendingResolution` persists continuation, ordered actual dice, spent resources
and remaining choices/reactions. The rules owner orders preparation, legal offers,
choices, cost application and resolution according to pinned clauses and explicit
exceptions. Do not impose a generic initiative/effect ordering contrary to a source.
Prevent recursive trigger explosions with admitted depth/count bounds; a bound
hit suspends with an explicit resolution gap/host ruling, never skips a legal effect.

Modifiers include source/precedence/explanation, not a pile of untraceable numeric
bonuses. Social relationship/tempo scores cannot become D&D bonuses. Unsupported
spell/feature exceptions remain catalog gaps; `NeedsRuling` uses the existing scoped
host flow and records that local decision rather than claiming standard support.

`SpatialQuery` uses typed grid/distance/reach/light/cover/visibility/occupancy basis
and supported source-defined geometry; `reachable` and `legal_choices` share the
same authoritative state. Client path previews are permitted views of that result.
World/fire/weather/faction proposals pass through `df-engine` to rules requests;
there is no bidirectional Rust dependency or model-decided physics shortcut.

Progression, inventory/currency, attunement, rewards/rest and class/spell resource
updates are core R02/R10–R13/R16/S05/S06 work. `AwardId` and source/policy version
deduplicate awards; advancement creates persisted legal pending choices. Optional
trade/economy systems cannot delay those standard campaign mechanics to a platform
expansion. Migration preserves effect origins/pending windows/resources or rejects
unsupported state explicitly; recovery/replay never rerolls or spends twice.

## Source-linked testing architecture

`RuleFixture` records source locator/checksum, catalog/handler versions, initial
state, typed action/choices, explicit draw sequence and expected outcome/facts/
resources/pending windows. Goldens are reviewed against source clauses before
being accepted; an LLM-generated expected outcome is an unverified candidate.

Property/generative tests exercise constrained legal state and boundary choices
from those fixtures. Check rejection changes neither state nor draws; retry keeps
the same resolution/award; offer/submit agree on legality; inventory/resources obey
the selected mechanic; replay reaches the recorded state hash. Properties must
allow source exceptions (for example a feature granting an additional resource)
and cannot invent a universal invariant. Shrink failures to retained cases with
seed, source/handler versions and actual draws; place confirmed regressions in
the relevant family fixtures.

Prioritize interaction pairs: reaction versus triggering attack; concentration
versus damage/incapacitation; condition expiry versus turn boundary; temporary HP/
resistance ordering; cover/range/movement; replacement/stacking; rest recovery and
multiclass spell/feature boundaries. Derive exact expected behavior from G07 sources.
Coverage reports entry/clause/interaction denominator and unsupported cases, not
just line coverage or every spell name. Standard support stays open until R01–R17
required catalogs and integrated acceptance pass.

RULE-EFFECT-CONTRACT refines existing R09/G03/G07/F11 plans once required source
access exists. Actual Rust checks run on faithful fixtures; PostgreSQL and both-role
flows prove pending-resolution/restart/projection integration. Frontier evaluation
operates real tactics/conditions/reactions/advancement, with observed audio where
affected. No executable tests have run in this planning-only project.
