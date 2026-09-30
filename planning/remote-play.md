# Hosted remote and mixed-room play

Date: 2026-09-30
Status: Required planned core F45; concrete limits and implementation pending

The latest user feature outline explicitly selects remote multiplayer as a table
stake. This supersedes earlier X11 remote-deferred wording. Same-room TV/phone
play stays central; remote and mixed-room players use the same authoritative
session, generated RPC/WSS, private views and legal decisions. Async play, public
community and marketplace remain conditional. No peer rules authority, separate
world writer or new transport is introduced. See [RPC API](rpc-api.md),
[Client presentation](client-presentation.md) and [Runtime directors](runtime-directors.md).

## Model, permissions and audiovisual topology

`RemotePlayPolicy` binds campaign/session policy version, admitted participant/
connection/capture limits, invite admission, explicit AFK/pause/takeover rules and
supported network/device matrix. `ParticipantPresence` is a bounded observed
connected/sleeping/disconnected/voluntary-AFK state, distinct from membership and
character life/action eligibility. It cannot spend a turn, authorize someone else's
input or advance logical time. G04 freezes trusted invitation/credential storage,
rate/admission bounds, principal/binding leases and revocation within active streams.

`AudioTopology` routes approved tracks to a public room speaker or individual
listener bindings. A remote player hears permitted public narration plus their
own private audio; a TV cannot obtain private whispers through a shared stream,
manifest, caption, prefetch cue or tempo change. Mixed-room output prevents
duplicate local public tracks through explicit output leases. Separate private
headphone routing requires a user-approved/unlocked device and visible routing
state; failure uses authorized text/captions, never the public speaker. No browser
proximity detection or automatic headphone assumption is required.

Existing BindClient/WatchControl/Talk/Listen/Report operations carry these typed
view/policy additions after G03 numbering/version fixtures. They are candidate
fields, not invented frozen protobuf numbers or extra services. Server projection
filters before serialization and asset demand. Clients report actual capture and
playback observations; they never decide membership, turn order or private access.

## Native ownership, failure and recovery

`df-auth` validates guest/invite/admission; `df-session` serializes simultaneous
remote and local input; `df-engine`/`df-rules` retain pending reactions, choices and
source legality; `df-api` maps permitted views; `df-client` owns network recovery;
`df-audio`/`df-media` enforce capture/audio leases and supported codecs. Pure director
policy has no network/database/provider I/O. PostgreSQL records membership, policy,
binding/operation decisions and pending outcomes under the existing fenced commit.

Retry looks up an uncertain operation before reissue. Slow/disconnected clients
receive bounded stream termination/resync without blocking another session.
After reconnect, initial authorized snapshots contain current pending choices and
active timeline; obsolete epochs, finished one-shots and old audio buffers do not
replay. Drafts remain explicitly uncommitted and need current offer revalidation.
Revocation closes sensitive streams and invalidates private cache access. Database
failure prevents authoritative commit; provider failure uses admitted prepared
fallbacks. Disconnect never rerolls, duplicates rewards, cancels accepted jobs or
hands input to an NPC without selected consent/policy.

AFK and host pause are deliberate approved commands, not inactivity penalties.
Any response expiry or takeover must identify permitted actor, source pending
window and disclosed policy; no invented D&D deadline. Paused/stopped world time
does not advance from internet wall time. Cancel before admission releases capture;
accepted decisions/jobs keep run ownership independently of the request connection.

## Gates, observability and acceptance

F45-MODEL extends G03/G04/G08 with remote/mixed-room topology, invite abuse bounds,
concurrency and selected device/network/capture/audio targets. Concrete supported
campaign capacity is measured, not a four/six-player cap or an unlimited promise.
F45-DELIVER reuses F01/F02/F03/F09/F16/F22/F24 implementations; no duplicate remote
engine or separate input protocol. S01 includes remote join/recovery; S03 adds
actual private/public voice routing; S05 proves remote campaign resume.

F45-ACCEPT uses real independently networked browser roles and fixed builds:
simultaneous legal input, reaction choice, same-room-plus-remote output, invitation
denial, slow consumer, packet delay/loss, reconnect mid-cue, denied mic, mute,
sleep/AFK and active revocation. Check source-valid results, exact operation
dedupe, private paired-state noninterference, old-buffer cancellation and actual
audible routing. Record p50/p95/p99 action-to-view and final-input-to-approved-audio,
timeouts/reconnects, bytes/CPU/memory and participant-scaled relay/usage cost.
Shared OTEL records causal stages without raw private speech or credentials.
Desktop viewport or local multi-tab success alone cannot prove remote readiness.
