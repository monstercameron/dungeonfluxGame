# Campaign continuity, memory and world time

Date: 2026-09-30
Status: Candidate refinements to F35/F36/F38/F39; implementation and bounds pending

Build on [Runtime directors](runtime-directors.md),
[Interaction engine](interaction-engine.md), and
[Storage architecture](storage-architecture.md). PostgreSQL snapshots and fenced
decisions remain authoritative; semantic indexes and summaries are derived aids.
Do not introduce another memory database, per-NPC LLM loop or crate dependency cycle.

## Retrieval and contradiction semantics

`MemoryEpisode` identifies committed event/decision, subject/participants, audience
scope, game time, source/content/run versions, salient tags and evidence/claim IDs.
`MemorySummary` records covered episode IDs/range, summarizer/model/policy revision,
source digest, scoped derived claims and uncertainty. `RetrievalRequest` binds
observer, purpose, permitted topics, relevant entities/game-time window and hard
item/byte/token budget. `RetrievedMemory` carries attributed snippets and exact
canonical/episode references, never an unbounded transcript.

Retrieval I/O is a native staged job, not a pure director query. `df-session` owns
`MemoryCandidateStore::load(context, AuthorizedMemoryQuery) -> MemoryCandidateBatch`
as a consumer port; `df-persistence` implements bounded indexed PostgreSQL reads.
`AuthorizedMemoryQuery` binds trusted observer/access generation, purpose, relevant
session/run/content/source digest, query policy and item/byte/token/deadline limits.
The session's existing native effect executor performs that read under the admitted
job/basis and returns a typed `MemoryCandidatesReady` input through the owner.
No transaction is held while a model runs. Cancellation, deadline, capacity,
permission and stale-index outcomes are explicit; pre-admission cancellation stops
work, and accepted job recovery follows existing run ownership/fencing.

The pure `df-knowledge::rank_candidates(request, current_knowledge_basis, batch)`
reauthorizes and ranks bounded supplied candidates by relevance/salience/recency
with deterministic tie breaks. It performs no DB/network/provider/system-clock
I/O. The batch contains source/index generation, coverage/lag and attributed
candidate references; the engine/session rechecks current basis, source digest and
observer revocation before committing or projecting selected memory. Neither a
past native access check nor an embedding grants current disclosure rights.

An optional semantic index is replaceable, versioned and rebuildable; access is
filtered before nearest-candidate selection and checked again on returned rows.
Missing/stale index falls back in the native store to bounded canonical queries or
returns `Incomplete` with coverage, never an unbounded scan. Shared vector scores,
caches and index diagnostics cannot reveal another subject's secret. G05/G08/G11
select an embedding/index implementation only if workload evidence warrants it.
If embeddings need providers, a separately admitted native index-build job uses a
qualified provider port frozen under G03/G08; none is silently added to the six
existing runtime ports. Store publishing validates source/access generation and
atomically switches derived index generation; a stale/revoked rebuild is discarded.
Canonical episode/summary acceptance emits the scoped index-update effect through
the same engine/session executor path. Replay loads recorded canonical data and
requires no embedding call; an absent optional index is explicit and rebuildable.

Canonical truth is resolved by committed world facts and explicit supersession,
not by the most confident summary. Beliefs retain contradictory attributed claims;
an NPC's false belief remains possible. A summary inconsistent with its sources is
rejected/quarantined and regenerated as a budgeted effect, without changing truth.
Compaction records coverage gaps and preserves obligations, identity-defining
events, source-linked rulings and active facts. Summary text cannot replace the
decision history needed for replay or turn absent provenance into a remembered event.

`ConsolidationCandidate` is returned through the normal durable job/basis path.
Revalidate source hashes, visibility and version before accepting it; stale
results cannot overwrite new episodes. Cancellation before admission discards it;
accepted run-owned jobs survive a disconnected requesting client. No generation
or index rebuild occurs during replay. Canonical records plus index generation
allow restart/resume; document what presentation evidence was not retained.

Personal character hooks require consented scope. Account deletion/export and
retention are X01/G05 decisions: remove/restrict personal indexes and optional
recordings under policy while retaining the minimum lawful scoped decision/audit
provenance. Neither a hash nor a hidden prompt makes private data anonymous.

## World granularity and pause

`SimulationTier` is Active, Scheduled or Dormant. Active entities receive bounded
event-driven decisions; scheduled important entities use indexed due events;
dormant entities retain summaries until a relevant event or accepted time advance.
`CatchUpCursor` identifies last processed logical event, content/policy version
and remaining work. Bound event count/CPU/output bytes per actor decision; commit
continuations through stable deduplicated IDs instead of an unbounded restart loop.

Travel/rest/environment/faction events advance `WorldTime` only through accepted
source-grounded transitions. Paused/stopped campaigns do not advance from wall
time. Resume applies already committed due events in deterministic order and
exposes remaining catch-up. Economy/resource flow uses authored finite stocks and
source-required inventory/rewards first; a market simulator is a conditional scope
decision, not a new core subsystem. World proposals become candidate rule inputs;
the engine composes them with source legality without a `df-world` ↔ `df-rules`
dependency or recursive callbacks.

## Acceptance

MEMORY-LONGHORIZON-ACCEPT extends existing F36/S05 evidence: a versioned multi-session
fixture retains a promise, secret, false rumor, contradiction and changed relationship
after summary/decay/restart; paired unauthorized queries have equal observable
results. Verify bounded retrieval, stale index and stale consolidation, source
hashes, missing episodes, cancellation, and zero paid replay calls. Report retrieval
precision/source-groundedness on frozen labeled cases and actual item/token/cost
bounds, not a claim that every long campaign stays coherent.

WORLD-TIME-ACCEPT extends F35: pause overnight, resume with a bounded due-event
backlog, travel and threat events, duplicate delivery and fencing. No event repeats,
hidden consequence leaks, free resource appears, or elapsed wall time damages a
paused character. Final frontier playtests check understandable continuity and
credible NPC behavior; unit/simulated time checks alone cannot establish player trust.

## Personal payload and rights lifecycle

[Service operations](service-operations.md) governs suppression generation,
DeletionPlan, personal payload redaction, immutable nonpersonal provenance,
index/summary/cache invalidation and deletion-tombstone replay before restored
backups serve. Source grant expiry never authorizes new prompt/export/distribution;
[Commercial validation](commercial-validation.md) owns reviewed RightsGrant evidence.
Private imports still accept only explicitly uploaded allowlisted bounded formats;
no arbitrary external import URL is newly allowed by the separate provider-egress
fetch policy. Redacted replay reports a scoped gap and never regenerates secrets.
