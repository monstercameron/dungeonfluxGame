# Predictive asset engine

Date: 2026-09-30
Status: User asset outline adopted; provider selection and measured bounds pending

`df-media` is the separate Asset Engine crate; extend its existing media boundary
with `AssetEngine` orchestration rather than add a competing generation owner.
`df-presentation` requests audience-safe semantic media; `df-ai` realizes approved
text and semantics; existing provider ports/adapters and `df-assets` supply generation
and durable bytes. [Runtime directors](runtime-directors.md) defines authority.
Supplier names/features/rates in the pasted outline are research leads, not frozen
contracts; current provider research and G08 decide routing/actual capabilities.

## Control plane and latency classes

A `NarrativeMoment` is a structured committed event/fact reference with location,
permitted characters, reveal scope, semantic mood/focus/importance and continuity
basis. It drives text/performance/music/visual requests without each generator
independently reinventing the story. A candidate moment remains tentative until
engine validation/commit; generated media cannot canonize dice, world truth or
player commitments. Keep speculative futures server-only, including asset indexes.

| Class | Initial delivery policy |
| --- | --- |
| Live input | streaming STT/VAD with explicit capture lease/format; tentative partial classification, final validated intent before actions |
| Live validated expression | narration/NPC text and streaming TTS; stable approved clauses only after the incremental-publication safety gate, otherwise complete validated text |
| Immediate reused sound | prepared SFX and campaign/NPC/faction music themes/stems, compatible mixer/timebase metadata; ordinary hits do not wait for paid calls |
| Asynchronous identity assets | portraits, items, location stills/backgrounds and reference packs; canonical reuse, prepared placeholder followed by matching ready asset |
| Prefetched/milestone cinema | bounded short shot plans/loops; strict campaign video permission/reservation; no required arbitrary live multi-minute generation |
| Interactive world/tactical rendering | server geometry plus cached/procedural-approved assets; optional 3D only after working flat/still/audio experience |

STT partials can prime bounded cheap/cancellable candidate work; they cannot
execute a predicted action or irreversibly spend an unapproved speculative budget.
The final transcript is validated by `df-intent` and current state. Streaming text
to TTS/captions requires a stable clause identified by sequence/hash, fully checked
against permitted facts, source/outcome/secret policy and a no-later-retraction
contract; unsafe partials remain buffered. Record emitted clauses and actual speech
for replay; model tokens are not direct public output. If the proof fails, use full
validated responses while optimizing latency through other measured paths.

Barge-in uses the authenticated capture/binding lease and a supported explicit
utterance-interruption event routed through the session. The server validates
current conversation/cue identity and issues timed fade/cancel before updating NPC
interaction under approved policy. Client-local mute or stopping audio is never
proof of an NPC interruption or an automatic relationship penalty. Lost/cancelled
unaccepted capture cleans up; accepted run-owned work follows normal job fencing.

## Models, public operations and lifecycle

`df-model` owns `NarrativeMoment`, `AssetDemand`, `AssetRequestKey`, `AssetJobState`,
`AssetDependency`, `CanonicalPack`, `VisualBible`, `EntityIdentityRevision`,
`ShotPlan`, `PerformanceHint`, `DemandPriority` and `PrefetchPolicy` persisted
shapes. `df-media` owns runtime admission/queue/request/errors; `df-assets` owns
complete immutable byte/metadata access. `VisualBible` and canonical NPC/location/
item packs pin art palette/style/appearance, voice and reference hashes/versions.
Future updates create a new identity revision, preserving old referenced assets.
Art is an appearance reference; it never changes item statistics or actual geometry.

Planned Rust-facing operations are:

```text
AssetEngine.admit(JobContext, AssetDemand, BudgetGrant) -> AssetAdmission
AssetEngine.status(JobContext, AssetJobId) -> AssetStatus
AssetEngine.cancel(JobContext, AssetJobId, CancelReason) -> CancelOutcome
AssetEngine.complete(JobContext, GeneratedCandidate) -> ValidatedAssetResult
AssetEngine.reconcile(JobContext, UsageEvidence) -> ReconciliationOutcome
```

Context includes run/generation, source/policy/identity revision, audience and
admitted execution mode/deadline. Admission returns existing identical job/ready
asset, queued reservation, deferred capacity, denied budget or typed unsupported
capability. Runtime queue transitions produce results through the session owner;
only committed demand creates accepted run-owned work. No direct authoritative DB
write from an asset queue callback. Native effect delivery is at least once, so
claim/result/paid-call identities and unknown-outcome reconciliation are required.

Logical lifecycle is Missing -> Queued -> Generating -> Ready, with canonical-pack
selection an explicit committed reference change. Failed/Cancelled/Stale/Superseded
are explicit outcomes. Ready means complete verified bytes and successful metadata
publication; partial files are not cache hits. Superseded ready bytes may remain
valid for old references. Invalidating a style/reference dependency marks affected
future demand stale without deleting historical/referenced assets or restarting a
paid job automatically. Dependency DAG validation rejects cycles/missing roots;
keys include schema, source/moment/identity/style/voice, provider/model/format,
reference hashes, audience and relevant parameter revisions.

## Prediction, fairness and spend

Predictive candidates rank likelihood-of-use heuristic, importance, urgency,
reuse and estimated cost. Sample decimal scores are not calibrated probabilities.
Queues distinguish interaction-critical, soon-likely and optional demand with
fairness/aging and per-campaign/provider concurrency, bytes, time and spend bounds.
Speech/input capacity remains available while optional cinema runs. Forecast
horizon/top-K/branch count/expiry are explicit policy; do not generate every future.
Prepare reusable references/themes once, select cache hits and record hit rate,
lead time, used-versus-abandoned assets and speculative waste by version.

Optional prefetched image/video requires explicit campaign allowance and a budget
reservation through `BudgetStore`; provider dispatch checks the admitted mode and
wallet. Never refund a spent/unknown call merely because the asset expired or was
unused. Reconcile actual usage, cancellation fees and unknown supplier outcomes;
reserve conservatively rather than auto-repeat an ambiguous paid call. Retire
unstarted demand when obsolete; accepted job cancellation does not guarantee a
supplier refund. No predictable cost means no silent spend. Prepared-only/replay
misses are typed unavailable and cannot go live.

## Requested bookends and identity evolution

[Campaign cinematics](campaign-cinematics.md) defines F46 recap/trailer jobs,
committed critical-event eligibility and scene/item identity revisions. Ready
canonical assets and prepared stills/narration/captions are the ordinary path;
optional live video requires a separate quote/cap. Audience/source/access versions
filter fact selection and reference inputs before provider dispatch, manifests or
export. Speculative trailers never establish future facts or expose secret branches.
[Generated content](generated-content.md) admits item/enemy/spell mechanics before
cosmetic generation; failed paid imagery cannot revoke due source rewards. All
these jobs use existing demand/budget/publication/cancellation/replay semantics.

## Nonblocking presentation and acceptance

Presentation chooses the best currently permitted available representation:
ready authorized video; matching still with bounded pan/zoom; prepared illustration/
flat scene; validated narration/captions. Optional usable 3D may improve this path,
but is never prerequisite for a cheaper fallback. Failed/late assets keep game
progress/input available and expose status. A late clip has expiry/resume/replacement
policy and cannot replay a finished milestone, expose an unrealized branch or
change an authoritative result. Semantic generation context and manifests are
filtered before delivery; never send secret future assets for clients to hide.

F15/F16/F17/F25–F28/F44 and G08/G10/G12 require canonical identity continuity,
DAG invalidation, complete publication, stale/duplicate/fenced jobs, fair queue,
cache/prepared/replay misses, actual spend/waste, clause safety, barge-in policy,
late assets and no-block fallback. Benchmark speech first-understood-intent/
first-approved-audio latency, deadline misses, prefetch hit/waste rate, bytes and
cost across actual campaign capacity/device targets. Frontier browser/vision/audio
review checks the running result. No provider feature, quality or profitable
capacity claim is accepted from the pasted vendor examples alone.

## Service spend and income refinement

[Commerce service](commerce-service.md) supplies the single exact hierarchical
platform/supplier/payer/campaign/job spend and concurrency authority, including
current entitlements, reservations and unresolved liabilities. Every hedged branch
requires its own admitted reservation under the same global cap; cache/prepared
modes reserve no new supplier spend. [Commercial validation](commercial-validation.md)
and the reproducible service-economics model separate net earned service revenue,
credit cash/liability, support, rights, acquisition/churn and fixed operating costs.
The earlier small-server/$2 support contribution illustration is not company profit
or proven capacity. [Service operations](service-operations.md) supplies finite
proposed workload and recovery targets. Paid launch requires the observed evidence,
not just source-backed planning parity.
