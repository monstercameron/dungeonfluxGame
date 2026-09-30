# Client/server RPC API shape

Date: 2026-09-29
Status: Planned v1 surface; field numbers, generated code and feasibility pending

This specifies the new Rust API, not a wire-compatible copy of the demo. Services
are logical groups in one backend, not independent microservices. Follow
[RPC transport](rpc-transport.md), [Subsystem interfaces](subsystem-interfaces.md)
and [Feature inventory](feature-inventory.md). Generation/library/framework versions
and concrete protobuf declarations are selected during the transport/type gate.

## Protocol and access

Use protobuf binary messages and generated Rust RPC stubs over the planned native
HTTP/2 gRPC-in-WebSocket bridge. Normal HTTP serves page/WASM bootstrap and health;
game commands/views/audio/assets use RPC. Asset transfers are lower priority and
bounded so they do not crowd action/audio delivery. Do not introduce hidden JSON
game endpoints or local rules logic as a shortcut around a missing browser driver.

Separate namespaces: `dungeonflux.public.v1`, `dungeonflux.admin.v1` and
`dungeonflux.telemetry.v1`. Public game services require scoped credentials in
tunneled RPC metadata, not bearer secrets in each request or URL. Browser socket
admission uses a tested origin policy and secure cookie or short-lived admission
ticket; native clients use the configured native admission mechanism. The exact
credential bootstrap/persistence method is a gate, not an assumed browser ability
to set arbitrary WebSocket headers. RPC authorization remains mandatory after
admission. Trace headers do not grant identity.

An admitted anonymous connection can use only configured bootstrap/join operations
under strict limits. The identity model supports revocable guest identities plus recoverable paid account
and tenant ownership through the planned CustomerService. Credential-provider
selection is open; account/entitlement/lifecycle contracts are mandatory in
[Commerce service](commerce-service.md). Creation
permission is deployment policy, not automatically granted to any guest. Invitations
grant only their configured player/display access, never host/operator permission.
Host is a capability attachable to a player or display binding; the selected role
does not grant it. Debug/operator services use separately restricted listeners or
routes and independent authorization, never the public anonymous bootstrap surface.

## Service surface

The following is protobuf-shaped pseudocode. Request/response contracts follow
below; no field numbers or generated source are claimed here.

```proto
service IdentityService {
  rpc BeginGuest(BeginGuestRequest) returns (IdentityGrant);
  rpc Renew(RenewIdentityRequest) returns (IdentityGrant);
  rpc Revoke(RevokeIdentityRequest) returns (RevocationReceipt);
}
service CustomerService {
  rpc Command(CustomerCommandRequest) returns (CustomerReceipt);
  rpc Inspect(CustomerInspectRequest) returns (CustomerView);
  rpc GetOperation(CustomerOperationRequest) returns (CustomerOperationLookup);
  rpc Export(CustomerExportRequest) returns (stream CustomerExportPart);
}
service SessionService {
  rpc Create(CreateSessionRequest) returns (SessionGrant);
  rpc Join(JoinSessionRequest) returns (SessionGrant);
  rpc Resume(ResumeSessionRequest) returns (SessionGrant);
  rpc BindClient(BindClientRequest) returns (ClientBinding);
  rpc Leave(LeaveSessionRequest) returns (DecisionReceipt);
  rpc SetPreferences(SetPreferencesRequest) returns (DecisionReceipt);
  rpc Watch(WatchViewRequest) returns (stream ViewMessage);
  rpc GetOperation(GetOperationRequest) returns (OperationLookup);
}
service ActionService {
  rpc Submit(SubmitActionRequest) returns (DecisionReceipt);
  rpc Preview(PreviewActionRequest) returns (ActionPreview);
  rpc ListOptions(ListOptionsRequest) returns (OptionPage);
  rpc SubmitText(SubmitTextRequest) returns (DecisionReceipt);
}
service VoiceService {
  rpc Talk(stream TalkInput) returns (stream TalkOutput);
}
service AudioService {
  rpc Listen(ListenRequest) returns (stream AudioStreamMessage);
}
service AssetService {
  rpc Manifest(AssetManifestRequest) returns (AssetManifestPage);
  rpc Get(GetAssetRequest) returns (stream AssetTransferMessage);
}
service JournalService {
  rpc ListEntries(JournalRequest) returns (JournalPage);
}
service ClientService {
  rpc WatchControl(WatchControlRequest) returns (stream ClientControl);
  rpc Report(ClientReport) returns (ReportReceipt);
  rpc UploadTelemetry(stream ClientTelemetryBatch) returns (IngestReceipt);
  rpc UploadDiagnostics(stream DiagnosticPart) returns (DiagnosticReceipt);
}
service HostService {
  rpc Command(HostCommandRequest) returns (DecisionReceipt);
}
// Restricted admin namespace; never registered on the public bootstrap surface.
service DebugService {
  rpc Command(DebugCommandRequest) returns (DecisionReceipt);
  rpc Inspect(InspectRequest) returns (DebugSnapshot);
  rpc WatchEvents(EventQuery) returns (stream DebugEvent);
  rpc SaveCheckpoint(SaveCheckpointRequest) returns (CheckpointInfo);
  rpc RestoreCheckpoint(RestoreCheckpointRequest) returns (DecisionReceipt);
  rpc SetFault(SetFaultRequest) returns (FaultReceipt);
  rpc RequestCapture(CaptureRequest) returns (CaptureTicket);
}
// Operator-only service owned by df-telemetry, not by the public game API.
service TelemetryService {
  rpc Query(TelemetryQuery) returns (TelemetryPage);
  rpc Timeline(TimelineQuery) returns (TimelinePage);
  rpc Health(TelemetryHealthRequest) returns (TelemetryHealth);
  rpc PinEvidence(PinEvidenceRequest) returns (EvidenceReference);
  rpc ExportEvidence(ExportEvidenceRequest) returns (stream EvidenceChunk);
}
```

Unary, server streaming, client streaming and bidirectional streaming are required.
The small first integration must demonstrate all four in real Rust/WASM, not just
generate every service. Grouping may change with evidence; semantics and ownership
cannot disappear silently during that change.

## Common envelopes and outcomes

| Shape | Required fields and semantics |
| --- | --- |
| MutationContext | operation_id, session_id (except Create/bootstrap), run_id where applicable, client_binding_id/input lease where applicable, observed_revision and optional strict precondition; actor comes from auth, not payload |
| SessionGrant | session_id, stable member_id/access role, display name, resolved preferences, supported capabilities, run/revision, refreshed scoped credential when required, binding instructions; only newly issued credentials are secret fields |
| ClientBinding | client_id/binding_id, selected view role, granted permissions, explicit input/audio/capture lease identities/expiry, connection generation, build/protocol capabilities, idempotency window |
| DecisionReceipt | operation_id, committed session/run/revision, typed accepted or rejected result, safe reason/detail, affected IDs, pending job/resolution IDs and resync instruction where needed |
| Rejection | stable code such as stale_offer, wrong_turn, invalid_selection, paused, resource_missing or capability_unavailable; safe display text/key, current revision, relevant permitted correction |
| OperationLookup | committed receipt, in_progress, not_recorded or expired/indeterminate; never invent an answer after retention loss |
| SnapshotStamp | session_id, run_id, durable session_revision, presentation_epoch, view_sequence, content/rules/build revision, server clock anchor |
| Page | bounded items, opaque next cursor, consistent/as-of revision and access scope; cursor cannot select another member's data |
| IngestReceipt | producer/batch IDs, accepted/rejected counts and safe reasons, durability stage (spooled/SQLite committed), watermark and gaps; no fake complete-corpus acknowledgement |

The server's transaction uses its current expected revision; a client's observed
revision is context, not an automatic global lock on every action. Each advertised
offer binds actor/run and the relevant state/choice conditions. Revalidate it against
current state; unrelated narration or another player's readiness need not invalidate
a still-legal action. Destructive host/debug changes require a strict revision
precondition. These policies are fixed by the offer/command, not a client option
that bypasses validation.

Every authoritative mutation/allocation has a stable operation ID reused on retries.
Key deduplication by session/authenticated principal/operation namespace and compare
a canonical request
fingerprint including run, command and selections. Same key plus different payload
is invalid. Look up a committed result before rejecting its original stale basis.
Preserve results through the advertised retry window; if payload results expire
while writes remain valid, retain compact deduplication evidence or retire the
operation namespace. Never forget a key and execute an old retry as a new action.

Before a session exists, Create uses a principal/allocation-operation namespace;
Join deduplicates its membership allocation by principal/target-session/operation.
BeginGuest uses an expiring scoped bootstrap request key plus canonical fingerprint
before a principal exists. Each allocation and its grant/result commits atomically.
Retry may return a refreshed authorized credential, not another identity or seat.
Keep tombstones or retire the namespace after advertised result retention, rejecting
expired keys; lack of an old receipt must not authorize allocation again.

Renew/Revoke use a principal/credential-operation namespace and stable operation
ID; BindClient uses a principal/session/binding-operation namespace so lost responses
cannot repeat a takeover or issue conflicting leases. Fingerprints include the
requested credential/binding change. Resume restores access; credential rotation
uses the renewal operation contract. Observational Report/control acknowledgements
use report/control IDs and bounded deduplication; telemetry uses producer/record IDs.
These observations do not fabricate a durably committed game DecisionReceipt.

A receipt means the server made and durably committed its decision, not simply
enqueued input. Accepted dialogue/media work may still be pending; its later
resolution is explicitly identified in the view/job state. If a response is lost,
GetOperation/recovery can resolve it; a new operation ID must not replay an uncertain
mutation. Cancelling/deadlining an RPC does not undo a committed game decision.
Accepted effects have session/run-owned deadlines/cancellation independent of the
originating RPC. Caller cancellation stops a receipt wait or unaccepted admission;
owner-authorized cancellation/reset governs durable jobs.

## Session, action and text requests

| Request | Shape and validation |
| --- | --- |
| BeginGuest / Renew / Revoke | supported identity flow, display-name/preference hints, bootstrap request identity or stable credential-operation ID; no self-assigned privileges; issue/rotate/revoke only authenticated or policy-authorized credentials |
| Create | operation ID, authorized creator, pinned content/rules IDs, configured player limit, default locale and game options; validate catalog/assets/options, allocate session+host permission atomically |
| Join | operation ID, invitation reference or room code, permitted player/display role, player name and locale; atomically allocate/reuse membership and grant access; bounded capacity and join attempts |
| Resume | session/member reference under valid credentials; restore access without another join event, seat allocation or ready reset; expired access returns a typed recovery requirement |
| BindClient | operation ID, session, role, client instance, capabilities/codecs/build, requested input/audio binding and explicit takeover intent; grants only authorized leases |
| Leave | mutation context, explicit membership-leave intent; a temporary disconnect is not Leave; revoke bindings consistently |
| SetPreferences | mutation context, typed locale/accessibility/audio preference patch; presentation preference cannot change rules/game authority |
| Submit | mutation context, offer_id, typed selections by advertised input_id; validate actor/run/current legality, choice multiplicity, bounds, IDs and scope |
| Preview | same offer/selections without mutation; server-produced legal paths/costs/permitted modifier explanation, no dice draw, provider call, hidden-info leak or resource spending |
| ListOptions | session/offer/input ID, bounded search/page cursor; server-filtered character/spell/equipment/target options, not a downloaded client rules engine |
| SubmitText | mutation context, utterance operation ID, declared dialogue/freeform intent context and bounded UTF-8 text; server determines eligibility and interpretation; preserve rejected draft |

`ActionOffer` contains offer_id, run/basis/expiry context, action_kind, localized
label, enabled/rejection reason, and repeated typed InputSpecs. Selection values
are `choice_ids`, `entity_ids`, `destination`, bounded text or validated numeric
choice. The InputSpec explicitly lists allowed value type, limits, options/query
reference and requiredness. No string `arg` with hidden grammar or arbitrary JSON.

Supported action families cover ready, character choices/finalization/name,
advancement, exploration/interact/dialogue, checks/roll confirmation, movement,
attacks, spells/features/items, reactions, end turn, equipment/inventory and rest.
The client never supplies a dice face, damage total, authoritative stat or rule
outcome. Rolls may be animated after the server resolves them; animations don't
make the decision. An unimplemented mechanic is visibly unavailable, not mislabeled
as a standard supported option. All offered actions come from the same engine/rules
state that validates submission.

## Views and reliable control

Watch selects an authorized role, not an arbitrary member ID. It begins with a
complete current Snapshot even in an idle lobby. Subsequent messages are complete
snapshots initially; add deltas only after measured need and a tested base/gap
contract. Coalesce superseded snapshots for slow readers. Never coalesce commands,
audio cancel, journal facts or completion events as though they were snapshots.

| View | Payload |
| --- | --- |
| Common | stamp, mode/submode, pause/connection state, server timer anchors, permitted roster, notices, permitted scene/narration, available capability/fallback state |
| PlayerView | own creation draft/sheet/resources/inventory, actionable offers and input eligibility, roll results, personal tactical reach/targets, permitted journal summaries, pending jobs and presentation timeline |
| DisplayView | public party, room invitation/QR reference, scene/layers, captions, visible encounter/tokens/initiative, permitted rolls/feedback, timeline/camera/clip cues and fallback/preload refs |
| HostPanelView | host-authorized commands and current authoritative options/status; no automatic operator logs, forced dice or other players' secrets |

Private information is filtered before serialization, including action previews,
asset manifests and journal entries. Shared display is not an omniscient human-DM
view. Unknown AC/DC/hidden entities are omitted unless the rules/content disclosure
policy permits them. Host permission alone is not a blanket private-data bypass.

Streams are authorized throughout their lifetime. Revocation, membership departure
and lease replacement fence affected publications/transfers and new input; close
or resync the stream and clear obsolete client buffers. Bound revocation propagation,
test it during active views/audio/assets/uploads, and never rely on a client notice
to enforce access. Bytes already delivered cannot be recalled. This access fencing
does not undo durably committed gameplay or implicitly cancel run-owned effects.

ViewStore accepts only the active binding/connection generation. Within an epoch,
sequence increases monotonically; a new epoch requires a full snapshot and cancels
obsolete audio/cues. Durable SessionRevision never moves backward across reset or
checkpoint restore. Same-revision media progress still has a new view sequence.
Ignore late frames from the replaced subscription. Resubscribe/resync preserves
member identity and current active timelines; it does not replay completed effects.

WatchControl carries sequenced typed binding revocation, resync requests and scoped
diagnostic capture requests. Report acknowledges each control ID and outcome.
Keep a bounded per-binding control history, with expiry and resync on a gap; never
let control backlog block the session owner. Control cannot request arbitrary
client-side code or mutate game state. Active animation/clip cues in a view carry
stable cue IDs, epochs, start/duration/offset and completion policy; repeat snapshots
must not replay a one-shot. Current cues remain recoverable from the server timeline.

Presentation fields select supported typed Rust component/layout/transition
variants and validated asset/timeline parameters with explicit version/size/count
bounds. They cannot carry executable code or unrestricted UI expressions. Unknown
optional variants use a permitted fallback and a bounded diagnostic; an unknown
required variant exposes incompatibility rather than inventing behavior. G03 freezes
the concrete enum/bounds/cue interruption and recovery policy before consumers.
The server `df-presentation` crate composes these DTOs; it does not add an RPC
service or a client-side rules interpreter. `df-tempo` supplies permitted anchored
profiles/curves and bounded cue impulses under [Tempo engine](tempo-engine.md).
Clients reconcile these snapshots into persistent keyed components without a
page reload; complete wire snapshots and incremental local rendering are compatible.
See [Client presentation](client-presentation.md) for focus/resource/animation,
visibility recovery, reduced-motion and measured acceptance contracts.

## Director and asset additions within existing services

G03/G10/G12 freeze additive versioned DTOs rather than one RPC service per director.
Action/SubmitText/Talk retain normal admission and operation identity while carrying
bounded `IntentDisposition`/clarification and `ActionPlanProgress` (stable plan/step
IDs, committed partial outcomes, pending choice/reaction/ruling and cancellation).
Questions, plans-only, jokes and tentative transcripts cannot execute silently.
The server validates current state/offer basis at each admitted step.

Player/Display views add audience-permitted discovered threads/knowledge,
conversation/topics/commitments, encounter objectives, presentation `TempoProfile`
(phase, allowed intensity channels, anchor/curve, version, clamps/fallback), active
stable `TimelineCue` IDs and permitted asset readiness/canonical identity refs.
They never contain internal beat scores, undiscovered threat stages, NPC secrets,
future branch forecasts or another observer's belief graph. Journal entries use
committed authorized facts/claims and disclosure provenance. Private variations
must not leak through shared sound, tempo, cue IDs, manifests or preload refs.

SetPreferences extends existing typed motion/flash/audio/caption/readability
preferences; HostCommand exposes only configured authorized pacing/presentation/
world-time policy controls after G10/G12 signature freeze. No user-authored code,
unrestricted director-state editor or free dice override is added. Debug Inspect
can expose scoped director/candidate/job provenance under operator policy.
Barge-in uses an authenticated Talk/report/control shape frozen at G03/G04/G08,
validated by session against the active conversation/cue; a local playback stop
is observational and cannot infer an NPC penalty. These contracts preserve the
11 service boundaries while concrete added field numbering remains gated.

## Voice, audio and asset streams

TalkInput is one Start, ordered Chunk messages, then End or Cancel. Start includes
mutation context, utterance_id, capture lease, locale hint and declared supported
codec/sample format. The server admits or rejects capture before significant upload;
bound bytes, duration and in-flight chunks. Chunks carry sequence and offset;
TalkOutput returns Accepted/ChunkAck, permitted partial/final transcript, typed
rejection/error and terminal outcome. Half-close finishes input but preserves final
output. Cancellation releases unaccepted capture/transcription scope; once an
interpreted action/job is durably accepted, its lifetime belongs to the session/run
and is independent of Talk disconnect/cancellation. Transcribed intent enters
normal validated action/text handling; it does not bypass turn or dialogue guards.
Negotiated upload formats and actual iPhone/Android behavior require browser tests.

Listen is authorized for the binding's current audio lease/audience. Messages carry
stream epoch, track/utterance/cue ID, channel, sequence and explicit timebase:

- Start: codec, sample/channel format, start anchor/offset, audience and timeline.
- Chunk: contiguous sequence/sample or media offset, bytes, duration.
- Mix: typed gain/duck/crossfade/loop change with a timeline anchor.
- End: explicit normal drain and final sequence/duration.
- Cancel: named track/utterance or old epoch; stop queued buffers and reject late data.

No per-frame audience wildcard lets the client retrieve another player's audio.
Clients report actual start/end/blocked/error, not server delivery as playback.
Lost/slow streams recover using a new epoch and current timeline; never drop arbitrary
codec bytes and continue corrupted playback. Plan bounded jitter/decode buffers and
clock-anchor correction; exact codecs/budgets are a device feasibility gate.

Asset Manifest is paginated by authorized session/content scope and returns immutable
refs/hash/MIME/size/variants/readiness. Get requests an authorized AssetId/hash plus
bounded resume offset/range. Transfer starts with metadata, then contiguous chunks,
then explicit completion/status. The receiver verifies size/hash (whole asset on
completion or specified chunk hashes for a range) before making it a usable cache
entry. A miss is not implicit paid regeneration. Runtime portraits/QR/clips and
prepared assets use the same manifest/access system. Renderer failures switch to
the supplied matching flat/still asset; never invent another battlefield.

## Reports, history and host controls

Client Report is a typed oneof playback, renderer readiness/failure, connection/
capability state, capture state or control acknowledgement. It includes binding,
build, relevant epoch/cue/operation IDs and observed timing/error. Ignore stale or
unauthorized reports; a report cannot create a combat outcome or decide expiry.

UploadTelemetry is client streaming of bounded structured batches with stable
producer/record IDs. Server validates/enriches trusted identity, preserves typed
OTEL attributes and reports rejection, durability stage and ingest position. Flush
acknowledgement does not block player input. Native ingestion remains OTLP under
the shared observability contract; agents query the separate SQLite corpus.
Each upload RPC has byte/batch/time limits and half-closes for its terminal receipt;
it is not an endless stream waiting forever for its only acknowledgement. Repeated
bounded uploads reuse the connection. On a lost receipt, retry the same record IDs;
accepted durable batches survive cancellation, and the receipt's spooled/committed
stage and per-batch outcomes make partial acceptance explicit. Browser buffering
remains best-effort with visible gaps under its selected persistence limits.
UploadDiagnostics is authorized only for an expiring capture ticket, purpose,
target/binding, approved MIME/size/hash and chunk sequence. It returns a retained
evidence reference, not a public asset URL. Full screenshots/private payloads are
not ordinary always-on logs.

Journal ListEntries uses bounded cursor/as-of revision and authorized public/own
knowledge scope. Entries include stable fact/entry ID, run/scene/time/author labels,
safe narrative and rules outcome references. It never returns hidden NPC context
or another player's private dialogue because a client requests a different filter.

HostCommand is a typed oneof start, pause, resume, end/start-new-run, set session
locale, set optional timer policy, adjust a named allowed timer or set presentation
preference. Commands return the committed current value; avoid blind Toggle.
It also supports `resolve_ruling(pending_resolution_id, permitted_typed_choice,
source_basis)` for explicitly authorized tabletop adjudication. The engine validates
the source question, scope and current pending resolution; persist/disclose the
ruling rather than allowing arbitrary rule changes or debug forced outcomes.
Include the current authorized offer identity, validate its permitted selections,
and surface a rules gap when no authorized adjudication flow is available.
Include `set_execution_mode(live | prepared_only | replay)` only for a host with
that configured permission. Return the committed mode and enforce it for every
new text/STT/TTS/image/video/sound job. Prepared-only and replay misses return typed
unavailability without live generation. Live automatic fallback is a separate
reported outcome. Active intents retain their admitted mode unless explicitly
cancelled/reissued with new identity; a switch never rewrites an existing job.
Time limits are optional session policy, not standard D&D rules. Destructive commands
use strict preconditions and explicit intent. ForceDice, JumpPhase, ApplyFixture,
fault configuration and RestoreCheckpoint are development/operator functions with
separate permissions; don't expose the demo's Skip as required normal progression.

## Debug, telemetry and compatibility

Debug Command is an operator-only typed oneof `ForceDice`, `JumpPhase` and
`ApplyFixture`. Include mutation context, strict preconditions, configured fixture
IDs and source/build/rules provenance; route it through the session owner and
record the resulting decision. Forced draws are constrained by the pending roll,
and jumps/fixtures validate target-state prerequisites. It is not raw event/JSON
injection and is disabled unless the deployment explicitly enables it.

Debug Inspect requests typed permitted categories (state summary, legal choices,
timers/jobs/scopes, assets, clients, costs) and returns build/run/revision provenance.
WatchEvents streams bounded domain diagnostic records/cursors; it is not the OTEL
database. Checkpoints validate schema/rules/content and restore to a new run/fence,
explicitly abandoning old jobs/audio. SetFault uses a configured fixture/fault ID
with expiry, never arbitrary code. CaptureTicket identifies a scoped computer-use/
visual/audio investigation. Test fixture/forced-dice commands go through the normal
session owner, including validation and logging, when explicitly enabled.

Telemetry Query/Timeline are read-only paginated filters by build, session/run,
operation/job, time, severity/service and trace. Return stable record IDs, ingestion
watermarks/gaps and retention status. Pin/Export preserve cited evidence under
operator retention policy; neither lets an agent edit/delete telemetry rows.

Use typed domain rejections in successful RPC responses; malformed/auth/resource/
framework failures map to INVALID_ARGUMENT, UNAUTHENTICATED, PERMISSION_DENIED,
NOT_FOUND, RESOURCE_EXHAUSTED, UNAVAILABLE, DEADLINE_EXCEEDED, CANCELLED or safe
INTERNAL as appropriate. Never parse status strings to select UI behavior.
Deadlines/half-close/cancel/error trailers have contract tests in all modes. See
[gRPC deadlines](https://grpc.io/docs/guides/deadlines/),
[cancellation](https://grpc.io/docs/guides/cancellation/) and
[flow control](https://grpc.io/docs/guides/flow-control/).

Reserve removed protobuf field numbers/names; never reuse tags or silently change
field meaning. New optional fields are additive, enum zero is unspecified, and
unknown/required capabilities have explicit handling. Use explicit field presence
where missing differs from zero/false. Breaking changes use a new namespace and
an announced compatibility window. Cross-version browser/server tests cover
required capabilities and unknown enum/oneof cases. Schema numbering and generated
code are a single-owner task before consumers. See
[protobuf best practices](https://protobuf.dev/best-practices/dos-donts/).

## API acceptance

Exercise both client roles plus host permissions with a real Rust/WASM build:
join/readiness, denied private access, legal/rejected/stale action, duplicate and
lost-response recovery, initial/same-phase views, sleep/reconnect/rebind, voice
half-close/cancel, actual audio plus stop, asset hash/range failure/fallback,
credential/binding retry and revocation during active streams, scoped host ruling,
PostgreSQL restart/fenced owners, and OTEL correlation through the complete flow.
Run concurrent audio/asset traffic while measuring p95/p99 action-to-view latency.
Faults cannot require host Skip to escape an otherwise normal game flow.
Frontier computer-use/vision evaluation under ADR 0005 is the feature acceptance
gate; missing actual device/audio checks are recorded as gaps, not inferred passes.

## Customer and payment boundary

The twelve services expose 41 planned methods. CustomerService is closed typed
variants with per-command authorization: verified account establishment/link/recovery,
tenant transfer acceptance, checkout/portal initiation, subscription change/cancel,
and lifecycle request. Anonymous admission allows only narrowly validated challenge
start/completion with identical safe responses and rate limits; it never exposes
Inspect/Export or generic tenant commands. Every mutation has account/tenant/principal
operation namespace, canonical fingerprint and lookup semantics independent of game
SessionId. Inspect returns payer/tenant permitted price/allowance/paid-through/credit
state without other members' private facts. Export streams only an explicit current
ExportGrant and DataLifecycle scope under bounded chunk/byte/item budgets; a payer
cannot automatically export player secrets. CustomerReceipt distinguishes durable
request acceptance, external Pending/Unknown, effective entitlement revision and
current safe next action; a checkout return URL is never paid confirmation.

These are candidate schemas with no protobuf numbers assigned. G03 owns compatible
numbering/version freeze and public bootstrap registration. Native bounded HTTPS
payment webhook is the vendor ingress exception, isolated from game endpoints;
verify raw-body signature, durably admit then acknowledge. Provider/admin secrets
never enter browser payloads. See commerce/service-operations state machines.

## Recovery revision ordering

SessionRevision is the composite (RecoveryEpoch, in_epoch_sequence), compared
lexicographically on server and Rust clients. G03 freezes typed/protobuf encoding
and compatibility fixtures; it cannot be treated as an ambiguous old scalar.
Regular run reset increments sequence in the same epoch; disaster recovery from
older Pg state uses a verified strictly increasing protected epoch before serving
a new full snapshot, with declared lost game-revision range. Old strict expected
revisions and operation/allocation namespaces are expired/indeterminate lookup-only,
never fresh writes. Game RPO remains up to the declared five-minute range; monotonic
version ordering does not claim missing game facts were recovered.
