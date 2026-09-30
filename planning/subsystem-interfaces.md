# Subsystem interface contracts

Date: 2026-09-29
Status: Planned public boundaries; concrete Rust/protobuf definitions pending gates

Read [Crate architecture](subsystem-architecture.md), [RPC API](rpc-api.md), and
[Coding style](coding-style.md). Signatures below describe Rust-facing shapes;
they are not compilable declarations or a choice of runtime/framework. Each owning
crate defines its listed request/result/error types; consumers cannot duplicate
them. Freeze concrete types and contract fixtures before parallel consumer work.

## Common contract rules

Use newtypes for session/member/client/run/operation/job/utterance/asset IDs and
revisions. Tokens are credentials, never IDs. `BuildIdentity` includes source,
native/WASM build, configuration and content revisions. `RulesetId` identifies
the pinned mechanics plus catalog/source revision; changing catalogs never changes
an existing session silently. Timestamps and durations have explicit units.

`OperationContext` holds correlation IDs, trace context, deadline/cancellation
scope and build/configuration/content provenance. It never authenticates an actor
or grants audience access. Pass `Principal`/`AuthorizedAudience` from `df-auth`
separately at privileged boundaries. The engine's `AudienceScope` is a pure
`df-model` value derived by the session/API after authorization; it imports no
auth or telemetry code. Client-supplied trace context is correlation only.
Native asynchronous ports use sendable futures
and owned/borrowed data with explicit lifetimes; browser operations use local
futures/browser callbacks. Do not force browser I/O to implement unsafe Send/Sync.
Port object safety, stream aliases and executor bindings are part of the transport
feasibility/type-definition task, not assumed from an async signature sketch.

Errors are local typed enums with safe public codes; preserve classified causes
for telemetry. Distinguish invalid input, domain rejection, conflict/staleness,
permission, not-found, cancellation, deadline, unavailable and capacity. Streams
define terminal status, ordering and cleanup, not just a `Vec` of messages.
All queues/records/files/streams have item and byte limits. The initial integration
task chooses and tests concrete limits from the device/load matrix; no production
queue may default to unlimited. Slow-client handling never blocks all sessions.

## Pure domain and content

| Crate | Planned entry points | Input/output and guarantees |
| --- | --- | --- |
| `df-types` | validated constructors; checked revision/unit arithmetic | no engine, serialization framework, network, database or secret-bearing global context |
| `df-model` | typed GameCommand/GameInput/Effect/GameFact; Snapshot::validate | state and durable DTOs cover membership linkage, creation, characters/resources, world/epistemic/NPC/narrative/experience/tempo/director state, scenes/encounters, presentation/asset demand, timers and jobs; versioned codecs live at persistence boundaries |
| `df-content` | ContentPack::validate; Catalog::get; SceneDefinition::lookup | immutable content/rules IDs, source citations, campaign beat/fact/threat/phase/delivery graph, NPC/social/world/encounter/tempo policies, map geometry, style/reference/asset dependency graph, localized text; reject cycles/missing references/unreachable alternatives |
| `df-rules` | Ruleset::legal_choices(state, actor); prepare(state, intent); resolve(state, resolution_input); validate_build(draft); reachable(state, movement_query) | pure typed rules outcomes, required choices/rolls/reactions, resource changes, modifier explanations and paths; no provider/network/clock access |
| `df-engine` | Engine::decide(input, logical_time, dice_input); apply(transition); read_model(audience_scope); checkpoint | validates mode/permission-relevant game context, composes rules and ten pure director boundaries, returns a candidate Transition with mutation, facts and typed effects; apply occurs only after durable commit |

[Campaign authoring](campaign-authoring.md) refines the missing producer boundary:
df-content validates immutable packages/templates; df-tools imports/authors/packages
bounded private drafts; existing persistence/session owners publish and activate
only authorized validated versions. [Long-horizon state](long-horizon-state.md)
adds provenance-bearing retrieval/summary/catch-up candidates; derived semantic
indexes never become another state authority. [Rules effects](rules-effect-model.md)
defines candidate closed source-grounded effect/trigger/pending shapes. Concrete
types, storage and any admin wire additions remain G03/G05/G07/G10/X10 decisions;
these refinements add no crate cycles or invented protobuf methods.
Native memory retrieval uses df-session-owned `MemoryCandidateStore::load` and a
df-persistence adapter, executed as an admitted staged job. `df-knowledge` only
reauthorizes/ranks bounded supplied candidates in a pure function; index/provider
I/O and generation publication stay in native effect execution. See the precise
source/access generation and replay rules in the long-horizon design.

`GameInput` is a closed enum: authenticated game commands, authorized host commands,
validated job completions, timer expiries and presentation reports. External clients
cannot submit arbitrary internal events. Rules may return `NeedsChoice`, `NeedsRoll`,
`NeedsReaction`, or source-grounded `NeedsRuling` with stable resolution identity;
the engine owns the pending resolution and its next legal inputs. Unsupported rules
return an explicit gap,
never a plausible AI guess or generic attack fallback.

The rules crate keeps modules for character creation/advancement, d20 tests,
combat/initiative/action economy, movement/space/visibility, damage/healing/death,
conditions, spellcasting/concentration, equipment, rests and exploration. They
share one approved state/outcome model and precedence rules, not a separate trait
or crate per spell. [Rules coverage](rules-coverage.md) controls completeness.
An authorized host resolves a `NeedsRuling` through `HostCommand::ResolveRuling`,
referencing the pending resolution/offer and its permitted typed inputs/source
provenance. It is not arbitrary event injection or operator-only forced dice.
Persist and disclose the scoped ruling. Missing authorized adjudication returns a
visible rules gap and leaves the resolution pending rather than inventing an answer.

`Transition` contains a validated state change, decision result, durable facts and
effect intents. It must be applicable exactly once to its expected revision.
Rejected actions leave state/resources/dice unchanged unless a contracted rule
explicitly consumes them. Ordered supplied dice outcomes are validated, recorded
and reproducible; production dice generation belongs to the session boundary and
must not expose future seeds to clients. Required draws/counters are committed
with the decision. Do not roll a fresh result when retrying a committed operation.

Story progress is content-driven through the separate director boundaries in
[Runtime directors](runtime-directors.md), [Narrative engine](narrative-engine.md)
and [Interaction engine](interaction-engine.md). `df-engine` owns creation/
exploration/conversation/encounter/resolution/end composition and staged changes;
`df-narrative` owns structural story policy, not the rules or a second state writer. A rules reaction may interrupt a story or encounter flow;
the old linear demo phases cannot block standard rules timing. No need for a generic
FSM crate unless actual reuse later warrants it. Narration/cinematics describe
committed outcomes and cannot substitute for those outcomes.

## Separate runtime subsystem boundaries

The ten additional pure crates have their complete public-operation/model/error/
ordering/bounds/version contracts in [Runtime directors](runtime-directors.md).
Their persistent state is defined once in `df-model` alongside game state; immutable
campaign authoring/policies live in `df-content`. `df-engine` composes proposals;
`df-session` commits with PostgreSQL fence/revision before publication/effect dispatch.
They cannot independently write DBs, start timers/providers, consume dice or mutate
another subsystem. Rules requests continue through `df-rules`, including reactions
and pending choices/rulings. `df-combat` chooses among legal tactics; initiative
and action economy remain rules authority. Shared presentation DTOs reach clients
only after exhaustive authorized projection, never through server-crate imports.

Extend `GameState` with versioned world, epistemic, interaction, narrative,
encounter/tactic policy, experience, tempo and presentation/asset-demand state.
`GameFact`/decision records bind chosen proposals, ordered causal IDs/draws,
semantic outputs and pinned source/policy/model versions for no-provider recovery.
`ContentPack::validate` includes beat/asset DAGs, source/missing-reference checks,
phase/alternative-delivery reachability, policy bounds and source-authorized social/
physical/encounter definitions. Unsupported replay versions are explicit gaps.

See [Tempo engine](tempo-engine.md) for timestamped profiles/cues and hidden-state
noninterference, and [Asset engine](asset-engine.md) for prediction/canonical packs/
job states. Coarse macro experience differs from moment tempo; neither restricts
legal actions or secretly changes world weather, geometry, game time or outcomes.

## Identity and session ownership

`df-auth` owns `Authenticator::authenticate(credentials) -> Principal`,
`authorize(principal, session, permission) -> AuthorizedAudience`, and a narrow
`CredentialStore` port for issue/lookup/revoke. Identity/membership creation has
idempotent intents, bounded invitation attempts and expiry/revocation semantics.
Player, shared-display access, host and operator are separate permissions.
Choosing a client role does not grant host or access another player's sheet.
Validate audience again when projecting/accessing private assets and records.
`BeginGuest` uses a bounded, scoped bootstrap-request deduplication key before a
principal exists. `Create` and `Join` use authenticated allocation-operation keys
before the target session/member exists. Allocate identity/session/membership and
persist its grant/result atomically. Expired request namespaces reject old retries;
receipt expiry cannot make an old allocation key fresh again.

`df-session` exposes:

```text
SessionDirectory.create(context, Principal, CreateSession) -> SessionCreated
SessionDirectory.join(context, Principal, JoinSession) -> SessionJoined
SessionDirectory.open(context, AuthorizedAudience, SessionId) -> SessionHandle
SessionHandle.submit(context, Principal, OperationEnvelope) -> DecisionReceipt
SessionHandle.operation(context, Principal, OperationId) -> OperationLookup
SessionHandle.snapshot(context, AuthorizedAudience) -> ReadSnapshot
SessionHandle.subscribe(context, AuthorizedAudience, cursor) -> ViewSubscription
SessionHandle.bind_client(context, AuthorizedAudience, ClientBindingRequest) -> ClientBinding
SessionHandle.report(context, AuthorizedAudience, BoundClientReport) -> ReportReceipt
SessionHandle.checkpoint(context, Principal, CheckpointCommand) -> CheckpointReceipt
SessionHandle.shutdown(context) -> ShutdownOutcome
```

Join validates invitation-scoped permission for the authenticated principal, then
routes membership allocation to the target session owner. It does not require an
existing member to open its own session first, and API handlers never create a
parallel seat registry. `commit_decision` includes membership/grant/allocation-result
writes atomically under the owner fence; a lost join receipt cannot allocate again.

One actor serializes each session's state, membership-linked gameplay commands,
timers and job results. Do not let API handlers read a mutable engine or keep a
parallel authoritative seat registry. The directory routes sessions; it does not
lock every session behind one actor. Detached ReadSnapshots are consistent and
carry session/run/revision plus presentation epoch/sequence.

Client bindings distinguish stable membership from transient connection/tab ID.
Grant explicit input and audio leases per capability group; returning a lease is
not ownership of the character state. Multiple read-only tabs may remain connected.
Takeover/revocation is explicit and observable; reconnect/resubscribe never Join
again. Renew/revalidate leases and enforce them at action/capture/report boundaries.

Authorization is not permanently captured at subscription time. Revocation,
membership departure and lease replacement fence new admissions and affected
views/audio/asset/control deliveries. Revalidate access as data is published and
at lease/token expiry; bound and test revocation propagation across owners/adapters.
Close or replace affected streams, discard unsent unauthorized data and clear
obsolete client buffers. A best-effort control notice is not the enforcement gate.
Already received data cannot be recalled; loss of connection/authorization alone
does not cancel an already committed game decision or its run-owned effects.

`SessionRepository` (defined here, implemented by `df-persistence`) exposes
create/load, acquire/renew/release owner fence, commit_decision, lookup_operation,
claim/complete effect intent, journal/checkpoint queries and retention operations.
`EffectExecutor` (defined here, supplied by `df-server`) exposes
start(job_context, EffectIntent) -> JobHandle and cancel(scope) -> CancellationOutcome.
Committed intents create a session/run-owned job context with their own deadline
and cancellation scope. Preserve originating trace links without inheriting RPC
cancellation. Cancelling/deadlining a receipt wait or losing its connection cannot
cancel durable accepted work. Before durable acceptance, caller cancellation may
stop admission/wait or discard an unaccepted capture according to its contract.
Explicit owner commands, run termination or shutdown govern accepted job lifetime.
`Clock` supplies deterministic logical time and owned timer registration; real and
virtual implementations are injected. Native task handles/timer callbacks carry
run generation, job/operation ID and timer instance ID.

The actor follows this commit path:

1. Authenticate/scope input, check binding and operation identity, and look up any
   previously committed result before validating a new action's current offers.
2. Validate the current owner fence, run and typed selections. Revalidate the
   offer's relevant context against current state; `observed_revision` is the
   client's basis, not a global compare-and-swap lock. Destructive host/debug
   commands enforce a strict revision precondition.
   Produce a pure transition without publishing it or starting external work.
3. Atomically commit state change, operation result, meaningful facts and required
   effect intents using the server's current compare-and-swap revision and owner
   fence in PostgreSQL. A competing commit requires reload/revalidation.
4. Apply the committed transition, publish permitted views, dispatch committed
   effects and return the committed DecisionReceipt.
5. Consume results through the same owner, checking generation/job identity and
   committing authoritative changes before publication.

After an ambiguous commit timeout, reload/lookup the operation rather than blindly
applying or retrying it. A crash after commit/before publication recovers from the
persisted state/intents; a failure before commit publishes no success. A durable
decision may include pending AI/media jobs: receipt distinguishes acceptance from
their later completion. SessionRevision increases across reset/restore; new runs
have new RunIds. Callback fencing also uses a new process generation on recovery.

## PostgreSQL persistence and durable media

`df-persistence` implements the consumer-owned ports, with bounded pool/deadlines
and one migration owner. The planned data groups are:

| Group | Durable content / constraints |
| --- | --- |
| sessions/runs/membership | unique IDs, room invite access, pinned rules/content, current run/revision, owner fence/lease; identity survives new runs |
| state/characters/encounters | schema-versioned authoritative snapshot or entity documents, indexed session/revision; exact physical representation selected with initial access-pattern benchmarks |
| operation results | unique session/principal/operation key, request fingerprint, committed rejection/success and revision; advertised lookup retention plus tombstones or retired namespaces preventing forgotten retries from executing again |
| bootstrap/allocation results | principal-scoped Create/Join operation keys; scoped anonymous bootstrap request key for BeginGuest, bounded validity, atomic allocation plus grant/result; no duplicate identities/sessions/members on a lost receipt |
| facts/journal/checkpoints | ordered domain history and authorized audience; immutable checkpoint provenance; never runtime telemetry as recovery data |
| effect intents/jobs | transactionally committed pending work, stable identity, scope/generation/status; retry/recovery policy per effect |
| assets/recordings/usage | immutable references/provenance, complete recording manifests, exact cost reservations and reconciliation |
| commercial pricing and wallets (contract gate) | versioned supplier/customer prices and quotes, campaign payer/spend grants, exact wallet/reservations/consumption/refunds, provider invoice reconciliation and idempotent payment/webhook IDs; no card secrets in game state or diagnostics |

This is snapshot-based recovery with an audit/journal and durable effect intents;
it does not mandate replaying all events as the only source of truth. Each commit
checks owner fence and revision in the database; process-local locks alone are
insufficient for future multiple instances. No transaction spans provider calls or
client waits. Unavailable storage rejects or leaves outcome unknown explicitly.

`df-assets` exposes resolve(context, logical_ref), open(context, authorized_ref,
range), begin_publish(context, metadata), finish_publish(context, staged_bytes,
expected_hash) and get_manifest(context, scope, page). `AssetStore` owns byte I/O;
`AssetMetadataStore` owns PostgreSQL metadata/access. A durable asset is visible
only after complete bytes/hash validation and successful metadata publication.
Incomplete writes remain unreferenced staging objects with owned cleanup/recovery.
An orphaned immutable blob is safe; a published reference to missing bytes is not.

Manifests include content hash, MIME/codec, bytes, duration/timebase where relevant,
variant relationships, source/generator revisions, readiness/fallback and access
scope. QR/public UI assets may be public; secret scene/player assets require
permission. Knowing a hash alone grants nothing. Use local durable files for initial
deployment behind AssetStore; object-storage deployment remains an adapter choice.
Disposable caches are separate from durable byte roots, workflow and telemetry.

## AI/provider/media ports

`df-provider-api` defines these substitution boundaries:

| Port | Request -> result/stream |
| --- | --- |
| TextProvider | context/messages/output contract -> bounded text events or validated structured candidate |
| TranscriptionProvider | audio format, locale, bounded input -> transcript events/final result |
| SpeechProvider | validated text/voice/format -> ordered encoded or PCM audio events with timebase |
| ImageProvider | references/prompt/output specification -> progress and complete candidate images |
| VideoProvider | reference frames/output specification -> job handle, status and complete result |
| SoundProvider | sound/music/ambience intent/output specification -> complete sound asset and loop metadata |
| BudgetStore | reserve(job, estimate), reconcile(actual/unknown), release_unused, inspect -> exact money/usage records |
| RecordingStore | lookup(key), publish_complete(manifest), list -> complete typed response/audio references |

Commercial planning in [Pricing and costs](pricing-and-costs.md) extends budget
records with versioned price quotes, host-approved campaign allowance/wallet,
actual/unknown supplier reconciliation and idempotent customer purchase/refund
records. Before commercial implementation, X09/G08 must freeze the exact billing
commands, payment-provider port/admission signature, authenticated payer-versus-host
permissions, webhook replay verification, ledger invariants and API/composition
owner. Reuse existing crates and PostgreSQL ownership; do not hide a payment HTTP
endpoint or invent a new RPC service/unsafe provider dispatch in ordinary game
handlers before that boundary is approved. Initial campaign-player capacity is
measured and configured; shared video is generated once per campaign while
individual/relay delivery costs still grow with actual recipients.

Provider events contain provider request ID, usage, classified failure and completion
identity. Native adapter streams clean up on cancellation/deadline. `df-providers`
maps actual fields/status/EOF semantics; local contract fixtures catch mismatched
reference-frame fields and trailing data before any paid live test.

`df-ai::AiService::run(context, AiJob)` accepts bounded permitted NPC/story context,
locale, known action offer IDs, output schema/policy and recording mode. It returns
validated dialogue/narration/flavor or an `IntentProposal`; no result mutates the
session directly. The engine validates every proposed action against current state.
Sensitive/constrained text is held until validation before captions/TTS. Incremental
publication requires the separately proven stable-clause contract in
[Asset engine](asset-engine.md): permitted-fact/secret/source validation, stable
sequence/hash and no subsequent retraction; otherwise buffer the complete response.
It cannot expose raw tokens or act on tentative STT partials.

`df-media::AssetEngine` owns predictive media orchestration with
admit/status/cancel/complete/reconcile operations in [Asset engine](asset-engine.md).
It receives committed `AssetDemand` from the presentation plan, enforces fair
priority/budget/mode/deadline/identity dependencies and uses existing provider and
asset ports. It neither owns canonical gameplay nor blocks a turn on optional
video. Queued/generating/ready/failed/stale/superseded status and canonical-pack
references are distinct; a cache hit requires complete verified publication.

`df-media::MediaService::run(context, MediaJob)` covers transcription, validated
speech, portraits/reference sheets, scene compositing, loops/clips/finishers and
sound preparation. Results reference durable assets or identified live audio;
job progress/status is explicit. Portrait failure cannot leave a legal character
unfinished indefinitely; use an identified prepared fallback and preserve the build.

Both services use bounded provider concurrency, deadlines and budget admission.
Every job receives an explicit `ExecutionMode` pinned to its admitted intent:
`live`, `prepared_only` or `replay`. `df-model` defines the mode; host policy permits
which changes may be selected, the session owner commits them, and AI/media services
enforce it across text, STT, TTS, image, video and sound branches. `prepared_only`
uses validated prepared assets/responses without network generation; `replay` uses
complete contract-matching recordings. A missing required entry returns typed
unavailability and never implicitly starts a paid provider. A live-mode automatic
fallback is separately identified, not a change of execution mode. A mode change
applies to new admitted jobs; replacing active work requires explicit cancel/reissue
with new identity, never silently changing an existing branch. Fixtures/faults are
operator-only and cannot be selected by normal host/play input.
Fallback/hedging is configured per operation, not every call; reserve cost for each
actually started branch and cancel losers. Record usage even when actual billed
cost exceeds an estimate; do not omit it to pretend a hard cap held. Unknown paid
outcomes keep a conservative reservation/reconciliation record. Outbox delivery
is at least once; use provider idempotency when supported, and do not automatically
repeat unknown non-idempotent paid calls. Job results are deduplicated by the owner.

Cache/recording keys include contract/schema, role, source/context/content revision,
locale, provider/model and all relevant output/voice parameters. Publish only on
successful complete stream finalization. Replay mode forbids live providers on a
miss; live mode records whether fallback/cache was actually used. A fallback line
publishes its actual matching captions and speech, not old captions with new audio.

## Projection, RPC and transport

`df-api` authenticates/maps requests to SessionHandle, then maps a detached
authorized read model into public wire views. Projection is a typed exhaustive
mapping with tests for omitted/late fields, modes and privacy. Host capability adds
approved controls/status; operator internals are confined to admin services.
Input request fields cannot select another member's private view.

PublicServiceSet registers the game services in [RPC API](rpc-api.md).
AdminServiceSet separately registers restricted debug services; `df-telemetry`
owns the operator log-query service. Never expose raw DB access or a public generic
JSON event injection endpoint. RPC metadata carries auth and trace context; domain
rejections are typed outcomes, framework/auth faults use gRPC status/trailers.
For client telemetry upload, `df-api` calls the consumer-owned `TelemetryIngress`
port in `df-observe`; `df-telemetry` implements it and `df-server` injects it. This
avoids exposing SQLite or adding a reverse tooling/API dependency. Upload admission
is authorized separately and cancellation does not discard a durably accepted batch.

`df-protocol` owns generated requests/views and compatibility fixtures.
`df-rpc-bridge` exposes accept/connect -> TunnelStream plus configure_limits,
observe_liveness and close/drain. It preserves an ordered byte stream across
WebSocket boundaries, native gRPC modes/status/half-close/cancel and bounded flow
control. It knows no session or protobuf service schema. Native and browser adapter
implementations may differ while preserving this contract. Prove the browser
HTTP/2/client/executor path before dependent work under [RPC transport](rpc-transport.md).

## Feature-specific candidates within existing boundaries

[Remote play](remote-play.md) adds RemotePlayPolicy/ParticipantPresence/AudioTopology
with existing auth/session/API/client/audio owners and candidate generated view
fields. [Campaign cinematics](campaign-cinematics.md) adds BookendSpec/FactSelection/
BookendPlan/ExportGrant, CriticalCueEligibility and immutable scene/item identity
versions. [Generated content](generated-content.md) adds ContentCandidate/Validation/
Admission, standard-compatible templates and explicit distinct custom RulesetId.
[Feature refinement](feature-refinement.md) adds ContextualPrivateOffer/KnowledgeCue
and ParticipationWindow/SpotlightPreference. Shared persisted shapes remain in
df-model; pure directors receive bounded authorized supplied data, and native
session executors perform scoped history/provider I/O. Concrete codecs/ports and
any field numbering are G03 prerequisites; no new RPC service or generic event/
script API exists. Generating assets never changes committed rules or due rewards.

## Browser interfaces

| Crate | Planned entry points | Required behavior |
| --- | --- | --- |
| `df-client` | ClientSession::join/resume/bind/submit/subscribe; ViewStore::accept; AssetCache::fetch/release | one active connection generation, initial snapshot, freshness and cursor rules, operation lookup before uncertain replay, bounded/auth-scoped cache and blob-URL cleanup |
| `df-ui` | theme/components; JoinPanel, HostPanel, AboutPanel | reusable Rust components and typed layout variants, keyed updates, shared design tokens, focus/touch/keyboard/readability, localized presentation/reduced motion; host intent -> typed command; no engine/role authority |
| `df-render` | SceneRenderer::mount/update/capabilities/dispose | incrementally render supplied geometry/path/timeline, frame-clock interpolation, bounded interruptible cues with stable IDs, usable flat mode, pre-first-frame fallback, measured capability report; no collision/rules decisions |
| `df-audio` | AudioPlayer::consume/cancel/unlock/dispose; CaptureSession::start/chunk/finish/cancel | preserve timeline/sequence, bounded decode/schedule buffers, stop old epoch buffers, report actual playback; microphone user gesture/format/cancel; no local NPC generation |
| `df-player` | PlayerScreen::mount/update/unmount | persistent join/lobby/creation/sheet/exploration/dialogue/checks/combat/journal/end and waiting/error states; in-place navigation/overlays, keyed updates preserving valid focus/drafts; submits advertised typed selections |
| `df-display` | DisplayScreen::mount/update/unmount | persistent group scene/party/dice/captions/initiative/cinematics/join information; updates same-phase, transitions and late assets in place without duplicate layers or full-page reload |
| `df-web` | boot/configure_route/mount/shutdown | owns persistent client/UI shell and scoped resource disposal, in-place routes, visibility/reconnect lifecycle, current build and error surface; no raw gameplay logic; generated JS bootstrap only |
| `df-locale` | settle(requested, supported, default); Catalog::format(key, args) | explicit fallback/catalog revision and missing-key diagnostics; no rule translation that changes mechanics |

The client reports renderer readiness/failure, audio playback and capture state;
reports are observations, never proof of game outcomes or authority over time.
Server time and playback offsets drive visuals; late subscribers do not replay
finished one-shots. Generation changes/reconnect clear obsolete buffers and recover
the server's active presentation timeline. Browser audio failures remain visible
and use server-selected fallback content under the same access/routing policy.

Follow [Client presentation](client-presentation.md): typed supported presentation
variants are configuration, not remote executable code or a second rules engine.
Complete wire snapshots reconcile mounted components incrementally. Each scope
owns bounded listeners/cues/decode/render resources and releases them on disposal;
ordinary updates and reconnect do not remount the entire page. Cosmetic animation
must not block authoritative results/input, and obsolete/private state is cleared.

## Observability and development tooling

`df-observe` owns structured instrumentation/context and bounded submit/export
ports plus `TelemetryIngress::ingest(context, trusted_source, batch) -> IngestReceipt`,
including safe startup failure diagnostics and browser-to-server reporting.
`df-telemetry` owns ingest(OTLP), query(filter, cursor), timeline, pin/export and
ingestion_health. Ingestion receipts distinguish spooled from SQLite-committed
records; advance checkpoints after commits, deduplicate stable record IDs, and
expose gaps/retention/watermarks. Query access is read-only and paginated. Telemetry
retention operations are internal operator policy, not agent SQL mutation.
`df-auth` authorizes operator query/pin/export access separately from OTEL context.

`df-testkit` supplies virtual time/dice, faithful provider/storage fixtures,
state/contract/replay comparison and adversarial ordering. Integrated PostgreSQL
checks use PostgreSQL, not a fake SQLite gameplay backend. Browser visual/audio
acceptance uses real builds under ADR 0005 and is not replaced by simulation.

`df-tools` provides typed dfctl commands, build/source/asset manifests, previews,
asset preparation and process ownership. Debug restore forks a new run/generation
with source/schema/rules checks and explicitly handles abandoned work/audio; it
never rewinds live connections or overwrites history silently. Preview fixtures
are clearly marked and do not implicitly invoke paid providers.

`df-workflow` exposes claim/renew/submit/evaluate/integrate and scoped append_devlog
commands over the existing five-table SQLite design. The coordinator alone changes
queue state; runtime telemetry query is separate. It enforces the frontier
computer-use/vision evaluator, two-attempt escalation, evidence retention,
resource-aware concurrency, thirty-minute cleanup and six-hour commit changelog.
Model runners are configured adapters; agent scheduling does not change gameplay
provider choices. Tooling itself needs output/contract acceptance and measured
optimization once integrated.

`df-server` validates configuration/content/registrations and owns runtime handles,
native listeners, scoped admin exposure, graceful admission stop, bounded drain,
storage flush and telemetry shutdown. Readiness requires connected PostgreSQL,
valid rules/content, all required executors and usable fallback assets; telemetry
degradation is surfaced independently; admitted gameplay proceeds to a safe
checkpoint, while new admissions/readiness follow the bounded telemetry policy
in service-operations.md. Exact hosting,
SDK/framework choices, budgets and signatures are frozen in the first implementation
tasks identified in [Implementation roadmap](implementation-roadmap.md).

## Commerce and lifecycle ports

[Commerce service](commerce-service.md) governs pure df-commerce policy and native
facade, account/tenant/payer identities, recoverable ownership, subscription/grace/
downgrade/cancellation and exact hierarchical ledger. Commercial models are owned
once there; df-provider-api BudgetStore delegates to that authority rather than
maintaining another wallet. Its inspect/reserve/dispatch/settle/reconcile operations
carry trusted TenantScope, grant/quote versions and reservation identity. df-auth's
EntitlementReader returns a versioned capability; current version is revalidated
inside atomic durable admission and controlled egress, never authorized by stale cache.

Consumer-owned df-session EffectRepository claim_dispatch records owner/attempt/
fence/reservation and Dispatching before native egress; df-server controls credentials
and provider send. df-persistence implements it and commerce ports. Loss of ownership
cannot classify Dispatching as unsent; settlement is attempt-bound and gameplay result
commit is current-run fenced. [Service operations](service-operations.md) specifies
irreducible external uncertainty, finite limits and recovery tests.

DataLifecycleRequest/DeletionPlan bind trusted subject/tenant/rights, suppression
generation, affected sources/indexes/assets/logs/exports/backups and fulfilled/retained/
unsupported outcomes. Minimal source/action accounting survives with permitted
redaction/tombstone/access restriction; deleted private text cannot be rebuilt by
summary/replay/provider. Typed Redacted/Unavailable replaces intact replay claims.
Grounded SpeechActPlan/ExpressionContext/SpeechEnvelope and RulingRequest/Response are
in [Interaction engine](interaction-engine.md); rich flavor remains qualified while
canonical slots and permitted claims have deterministic source checks.
