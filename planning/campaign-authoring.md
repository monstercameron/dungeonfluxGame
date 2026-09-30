# Campaign authoring and content lifecycle

Date: 2026-09-30
Status: Candidate contracts; X10/G07/G10 decisions and implementation pending

The supplied gap brief exposes a missing producer for the existing `ContentPack`.
Keep the existing game crate graph plus independent commerce policy: `df-content` owns immutable schemas/validation,
`df-tools` owns local import/author/validate/package commands, `df-persistence`
stores published metadata/authorized references, and `df-engine`/`df-session`
activate a validated version. A visual editor is a later interface decision, not
a prerequisite for a hand-authored tested launch campaign. See
[Runtime directors](runtime-directors.md) and [Rules coverage](rules-coverage.md).

## Versioned authoring model

`CampaignPackage` contains a stable campaign ID, immutable pack version/hash,
schema version, compatible RulesetId/catalog manifest, creator/access scope,
source/provenance/use-rights entries, locales and a bounded dependency manifest.
It references arcs/beats/alternative disclosures, canonical facts, knowledge
grants, NPC/location/item/encounter templates, factions/schedules/threats, maps,
style/voice/asset packs, fallback cues and the initial world state. Templates have
stable IDs and explicit instantiation parameters; runtime entities get separate
IDs, so two guards do not share mutable memories or inventory.

`DraftRevision` has expected parent/hash and owner, diagnostics and unresolved
references. `ImportSpec` names accepted format/schema, source identity/checksum,
rights assertion, permitted namespace, byte/item/depth limits and merge strategy.
`ImportReport` records accepted/rejected entries, safe diagnostics, conflicts and
provenance. Lore extraction by an LLM produces `ExtractionCandidate` only: an
authorized creator confirms canonical claims, secrets and NPC knowledge before
validation/publication. Imported prose cannot become executable rules, arbitrary
URLs/file access, or automatically authorized model context.

## Pure validation and scoped publication

Candidate shapes: `ContentPack::validate(package, pinned_catalog, limits)` returns
`ValidatedPack` or bounded source-located diagnostics; `df-tools::import` produces
a draft/report; `package(validated_pack)` produces immutable bytes/hash.
`PublishContent` and `ActivateContent` are separate authorized coordinator/host
operations with operation ID, fingerprint, expected draft/current pack version
and permitted access. Concrete admin RPC versus local CLI transport is frozen by
X10/G03 before implementation; this plan adds no unnumbered public RPC methods.

Validation checks schema/rules compatibility, unique stable IDs, reference and
asset DAG closure, beat prerequisites/alternative reachability, contradictory
unique truths, NPC knowledge/reveal conditions, valid geometry/encounters, localized
labels and usable fallback assets. Missing source/catalog rights or mechanically
unsupported content are explicit `SourceGap`/`UnsupportedRules`, not silent imports.
Reject traversal/archive bombs, excessive nesting, oversized counts, reference
cycles and external fetching; use a bounded allowlist import format first.

Private drafts are never discoverable in public manifests. Every exported/remixed
dependency must have compatible rights and access; a hash does not grant access.
Publication stores complete validated bytes and metadata atomically under the
existing asset/content publication contract. Cancel before publication abandons
owned staging; lost receipts use operation lookup, and completed versions remain
immutable. Partial imports cannot mutate a running campaign. Retry deduplicates
by operation and payload, not filename.

Activation for a new run selects an immutable pack. A live version change requires
an explicit migration plan mapping preserved entity/fact/beat/source IDs and
pending jobs, with a fenced commit/checkpoint and dry-run diagnostics. Unsupported
migrations are rejected or start a separate run; they never retcon old facts.
Recovery loads the committed pack/hash and does not reimport source or call an LLM.

## Delivery and acceptance

X10-RESOLVE freezes minimum schema/import format, rights checks, size bounds,
publication/admin owner and creator workflow. X10-AUTHOR then implements the
bounded `df-tools` path using G07/G10 contracts. X10-ACCEPT validates authoring
a small complete campaign from templates, alternatives, secrets and fallback
media through real S03/S05 activation/resume. Graph editing, public publication,
discovery, marketplace/remixing UI and module-import formats beyond the allowlist
remain [Expansion decisions](expansion-boundaries.md).

Checks include incompatible catalogs, missing secret/reference, unreachable beat,
duplicate instance IDs, extraction hallucination, rights mismatch, malformed import,
cancel/lost receipt, unpublished draft access, concurrent draft edit and stale live
activation. Record import/validation/publication latency and bounded diagnostic
counts through shared OTEL without dumping private lore. Frontier review operates
the actual authoring output and two-role campaign; a JSON schema alone is insufficient.

## Personal payload and rights lifecycle

[Service operations](service-operations.md) governs suppression generation,
DeletionPlan, personal payload redaction, immutable nonpersonal provenance,
index/summary/cache invalidation and deletion-tombstone replay before restored
backups serve. Source grant expiry never authorizes new prompt/export/distribution;
[Commercial validation](commercial-validation.md) owns reviewed RightsGrant evidence.
Private imports still accept only explicitly uploaded allowlisted bounded formats;
no arbitrary external import URL is newly allowed by the separate provider-egress
fetch policy. Redacted replay reports a scoped gap and never regenerates secrets.
