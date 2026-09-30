# Platform scope and conditional expansion

Date: 2026-09-30
Status: Hosted remote selected as required core F45; remaining X11 expansion decisions pending

The core remains living-room shared display plus individual thin Rust clients,
source-faithful 2024 rules, durable campaigns and directed voice/presentation.
The latest feature outline selects hosted remote/mixed-room play as required
core under [Remote play](remote-play.md), superseding the earlier gap brief's
remote-deferred option. Async/community/platform expansion retains explicit scope
and cost/privacy/adoption decisions. An owned decision is planning coverage; it does
not mean those products or features exist. Reuse the game crate graph plus independent commerce policy unless a
selected boundary later needs a reviewed independent authority.

## Connectivity, presence and degraded play

Core reconnect already uses stable membership, binding generations and operation
lookup under [RPC API](rpc-api.md). `PresenceObservation` may describe connected,
sleeping, disconnected or voluntarily AFK at a bounded recent timestamp. It does
not revoke membership, imply consent to an NPC action, or advance a paused game.
Server-hosted remote connections still use the same session actor, private views,
input leases and legal turn/reaction policy; distributed players do not need a
second rules authority. F45/G04/G08 must freeze
invite abuse/admission controls, latency/capture targets, private audio versus shared
speaker routing, AFK/host pause/takeover consent and actual remote device evidence.

Offline core UI shows a stale/read-only permitted snapshot and explicit pending
receipt state. A locally saved typed draft is uncommitted: reconnect looks up an
uncertain admitted operation, refreshes current server offers and requests deliberate
resubmission/confirmation when stale. Never queue an assumed successful roll,
advance a turn locally, auto-spend resources or promise all voice actions replay.
Cached assets remain scoped/expiring; revoked private state is cleared on validated
revocation/rebind. Provider outages use prepared/current approved fallbacks and
visible pending/rejection; unavailable PostgreSQL prevents authoritative commits.

## Async and notifications if selected

`AsyncTurnPolicy` would bind eligible actor, source-legal pending resolution,
voluntary reminders/AFK choices and explicit logical-time pause/expiry. Commands
still commit through the same actor; reaction simultaneity has an explicit pending
window and authorized resolution policy. Offline/async never creates a parallel
state database. Real-world expiry is a selected disclosed session policy, not a
standard D&D rule; absent consent, world time remains paused while input is missing.

`NotificationIntent` would reference only an audience-safe committed event,
recipient consent/channel, dedupe key, expiry and minimal redacted message.
Existing outbox/persistence ports own durable at-least-once delivery, bounded retry,
revocation/unsubscribe and uncertain provider outcomes. Default lock-screen text
contains no secret lore, transcript or hidden turn result. Notification delivery
never grants auth or changes a turn; privacy/anti-spam/retention/channel costs must
be decided before creating a provider or publicly promising reminders.

## Sharing, community and broader content if selected

[Campaign authoring](campaign-authoring.md) covers private validated packs/templates.
Public discovery/remix would add `PublicationRecord` with immutable pack/source/
rights versions, creator attribution/access/license, moderation status and bounded
index metadata. Review/quarantine/takedown appeals and dependency rights propagate
before new distribution; existing campaigns must handle withdrawn content under
an explicit retention/access policy. Forks get a new pack version/ID and retain
provenance. Marketplace payment/payout/refund/fraud policies require a separate
commercial decision, not reuse of game-wallet code without review.

External module import requires an authorized format/rights converter with bounded
untrusted-input validation. The selected F47 opt-in custom-content boundary in [Generated content](generated-content.md)
uses reviewed typed deterministic handlers and a distinct RulesetId/catalog with
explicit participant disclosure. Broader unrestricted rulesets/homebrew remain
conditional; do not let mutable creator scripts alter standard outcomes
or silently mix editions. New mobile-first outside-room flows require their own
device/attention/voice usability and spend proof; existing browser targets do not
establish that product choice. Public economy simulations, publisher incentives
and unlimited content quotas are also unselected scope/cost decisions.

## Decision and acceptance

X11-RESOLVE records the selected required F45 hosted remote decision, the selected
F47 bounded opt-in custom-content boundary, and selected/deferred/rejected scope
for async, notifications,
public publishing/discovery/remix/moderation, marketplace, extra import formats,
broader rules/homebrew beyond F47 and outside-room mobile UX; each includes owner, rationale,
rights/privacy/cost prerequisites and the bounded prototype/evidence required.
X11-ACCEPT checks selected plans have explicit actor/auth/storage/provider/client
hooks and unresolved gates, and excluded work cannot be counted as launch support.
Do not create implementation tasks for an unselected feature. If selected later,
expand one canonical bounded task and link it to these existing decisions.

Core checks already belong to F02/F09/F24/F25/S01/S03/S05: sleep, reconnect, mute,
AFK, uncertain operation, source-expired offers, network partition, provider failure
and PostgreSQL outage without fake results. Required F45 acceptance adds simultaneous remote/mixed-room input and private
audio/device/network evidence. Conditional acceptance adds async reaction/expiry,
notification privacy/duplicates/unsubscribe,
malicious content import, revoked rights and moderation recovery. Computer-use,
vision and audio evidence remains required for affected running flows.
