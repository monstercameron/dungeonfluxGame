# D&D rules support

Date: 2026-09-29
Status: Standard 2024 fifth-edition D&D required; coverage/interface plan added, source pinning pending

## Requirement

DungeonFlux must support standard 2024 fifth-edition Dungeons & Dragons rules.
A custom ruleset or an AI approximation does not satisfy this requirement. Pin
the exact source revision before rules implementation begins; do not silently
mix editions or treat house rules as standard behavior.

The 2024 rules are the selected target. Wizards of the Coast provides the SRD 5.2
family for these rules; SRD 5.1 covers the earlier 2014 rules. The SRD is a
reference subset, not the entire core rulebook catalog. See the
[official SRD overview](https://www.dndbeyond.com/srd). Planning must track coverage
outside that subset rather than treating SRD completion as full rulebook support.

The optional [Generated content](generated-content.md) capability uses source-legal
approved templates in standard mode. New mechanics require explicit participant-
disclosed custom RulesetId/catalog and independently reviewed deterministic handler
admission. They cannot be counted as standard rules or close required coverage.

## Planned coverage and interfaces

[Rules coverage](rules-coverage.md) now defines R01–R17 mechanics/content families,
source manifests, staged subsets and the complete-support gate. The exact required
book/catalog/errata revisions remain prerequisites, not a worker-selected subset.
Record explicit gaps as work to complete; an early playable subset does not close
standard support.

For each implemented mechanic, record the source section and verify normal cases
and important interactions. Preserve the selected rules revision for existing
campaigns when rule data or implementation changes. [Subsystem interfaces](subsystem-interfaces.md)
defines deterministic legal-choice/prepare/resolve/build/path contracts and pending
resolution ownership. Typed source-grounded tabletop rulings remain explicit;
concrete type/codecs/catalog representation are frozen before dependent tasks.

The server resolves all game mechanics; thin browser clients receive presentation
views and never execute authoritative rules. AI may propose actions or narration;
authoritative rules validation and resolution remain server code responsibilities.

`df-rules` is the pure server mechanics crate; `df-content` owns pinned catalogs,
`df-engine` composes director proposals and owns pending resolutions;
`df-combat` selects NPC tactics while `df-rules` retains initiative/action economy.
Narrative/social/encounter proposals cannot override source legality. `df-session` owns durable
serialized decisions with explicit dice inputs. AI/media consume validated outcomes;
`df-api` produces permitted wire views under [RPC API](rpc-api.md). Follow
[Implementation roadmap](implementation-roadmap.md) for delivery and source gates.

## Paid rights and novice adjudication

Full required 2024 coverage remains mandatory; non-SRD RightsGrant evidence in
[Commercial validation](commercial-validation.md) is a paid-launch blocker, and
personal book access/model knowledge/private import cannot mint commercial rights.
[Interaction engine](interaction-engine.md) defines source-guided novice-host
RulingRequest/Response, supported automatic cases, explicit await/decline/alternative
and unavailable-adjudicator recovery. Autonomous-GM claims must disclose residual
human judgment; source-required mechanics cannot be replaced by invented defaults.
