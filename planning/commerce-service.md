# Customer, commerce and admission contracts

Status: proposed service design; no customer accounts, payment integration or paid deployment exists.

## Independent authority and boundaries

`df-commerce` is a native pure policy crate for customer organizations, subscription access and exact spend accounting. It imports only df-types; it does not own game state, authenticate credentials, contact payment/providers, or use clocks/DBs. State, supplied authoritative timestamps and verified gateway observations produce bounded `CommerceTransition` values. Ports owned here are implemented by df-persistence (`CommerceRepository`) and df-providers (`PaymentGateway`); df-server supplies a `CommerceService` facade to df-api and a `BudgetStore` adapter to df-media/df-ai. df-auth keeps identity, permissions and its consumer-owned `EntitlementReader`; its persistence adapter reads commerce's versioned grant projection. df-session receives df-auth-authorized campaign capabilities and does not import commerce. Game authority and commerce authority share PostgreSQL transactions at admission, without sharing mutable actors.

`CustomerAccount`, `TenantId`, `CampaignId`, `BillingCustomerId`, `SubscriptionId`, `PriceVersion`, `EntitlementRevision`, `SpendGrant`, `SpendReservation`, `LedgerEntry` and `GatewayEventId` have validated IDs and exact integer currency/subcurrency units. Tenant is the private ownership namespace; campaign host, payer, tenant owner, member and platform operator are separate capabilities. Every campaign has one tenant and one current payer grant; payer transfer requires both parties' acceptance, current revision, settled reservation policy and an idempotent result. Payment ownership never grants a player's secrets or operator access. Losing a host connection does not lose account ownership.

## Account and tenant lifecycle

`AccountCommand` covers establish verified identity, link current guest, recover credentials, revoke sessions, accept ownership transfer, request audience-safe export, and request deletion. Credential secrets and token hashes stay df-auth. Candidate managed identity versus native passkey/email adapter remains G04 selection; verified issuer/audience/nonce/state and reauthentication are mandatory. Guest linking requires proof of both identities and only transfers explicitly selected membership; never email-match a stranger or create a second campaign. Recovery uses single-use short-lived opaque challenge, bounded attempts, identical public nonexistence response and independently verified recovery channel. Challenge completion increments credential generation and revokes old refresh/binding grants. Existing committed decisions remain committed. Tenant owner transfer requires recipient consent and recovery capability; orphaned tenant becomes restricted, not public.

`TenantScope` is minted from authenticated permission, not a supplied tenant header. Repository ports accept it explicitly and reject omitted/mismatched tenant IDs on load/write/export/asset use. Unique operation keys include tenant/principal/command namespace and request fingerprint. Restricted/deleting tenants deny new game/provider admissions and enforce export rights; owners can retrieve permitted records without a new payment. Account deletion cannot silently delete others' shared campaigns: remove personal linkage, revoke private access and tombstone personal data according to the retention plan.

## Payment and entitlement state machine

`CommerceRepository::transact(scope, expected_revision, command_key, transition)` atomically writes ledger/state/idempotent result/grant changes/outbox. `PaymentGateway::create_checkout`, `inspect_subscription`, `inspect_payment`, `refund` and `verify_event(raw_body, signature, endpoint_version)` have exact request fingerprint/idempotency keys, bounded deadlines and typed KnownSuccess/KnownFailure/Unknown outcomes. Gateway calls execute after an outbox claim; unknown charge/refund is reconciled using the same key before any resend. Hosted checkout/customer portal is a candidate external redirect, allowing Rust-only application source and avoiding raw card handling. Return URL is allowlisted and its arrival proves no payment. Internal gateway status is mapped to versioned policy rather than vendor strings leaked into engine code.

States: PendingInitial -> Active only after verified paid invoice/entitlement observation; initial failure/expiry -> NoAccess. Active with cancel-at-period-end remains Active until paid-through boundary. Renewal failure -> Grace for **72 hours**, visibly reported, with no new optional paid video and no increase in included allowance; then ReadOnly until confirmed settlement. Immediate cancellation, confirmed refund ending entitlement, chargeback or abuse -> Restricted according to disclosed policy. Cancellation cannot revoke a committed outcome or spend owed rewards. Scheduled downgrade activates at next period boundary; preserve campaigns, permitted downloads and existing assets, stop new over-limit admissions and offer priced alternatives. No hidden deletion or mid-reaction lockout: actor completes already admitted source-valid resolution, then yields read-only safe checkpoint; cosmetic/provider futures settle their admitted liabilities. Trial access is a separately bounded zero-spend grant, never assumed from gateway status.

Webhook HTTPS handler verifies signature over raw bounded body and endpoint/API version, then persists `GatewayInbox` before returning success. Deduplicate exact event ID; separate events concerning one object are not ignored blindly. No delivery ordering or timestamp sorting assumption: process with object-level revision/refresh and reconcile authoritative latest invoice/subscription state. Unknown/stale observation cannot grant new access. Daily reconciliation plus startup backlog reconciliation repairs missed deliveries; retry exhaustion alerts support. Stripe documents unordered and duplicate delivery, so the proposed adapter explicitly tests both. [Stripe webhooks](https://docs.stripe.com/webhooks), [subscription states](https://docs.stripe.com/api/subscriptions/object).

`EntitlementSnapshot` includes tenant/campaign/payer, paid-through/grace, allowed modes, included usage units, rights policy and monotonically increasing grant revision. API and actor admission check current grant under the same durable transaction as new paid reservation/accepted effect. Cached snapshot is useful for rendering only; cache TTL cannot authorize revoked access. Revalidate private delivery through auth separately. Mutation failures return safe retry/lookup guidance, never false checkout or game success.

## Atomic hierarchical spend and concurrency

`SpendAdmission` contains tenant/payer/campaign/job/effect IDs, source/config/provider/price/quote versions, maximum units, `max_supplier_liability`, currency, mode, expiry and a consented spend grant. In one serializable/explicit-lock PostgreSQL transaction, lock canonical counter rows in stable order: platform period -> supplier/account -> tenant/payer -> campaign -> job. Check each remaining spend/concurrency/storage allowance, decrement only if all fit, insert unique reservation and accepted intent linkage, and return a revisioned permit. Insufficient shared budget is capacity/budget rejection with prepared fallback; individually valid reservations cannot collectively overspend the platform. No provider I/O while holding locks. Fixed-point calculations round liability up and settlement deliberately; no float comparison or currency conversion without pinned rate.

Reservations: Reserved -> Dispatching -> SettledKnown/UnknownLiability, or UnsentCanceled -> Released. Unknown keeps the full worst-case liability reserved, holds supplier concurrency until reconciled or an operator-approved maximum duration/closed-world policy; expiry alone never releases unknown spend. Actual known cost plus billed failed/regenerated waste settles once; unused known remainder releases. Underestimated invoice creates explicit platform loss/overrun incident and blocks affected supplier admissions; it cannot bill customer beyond consent. Top-ups produce cash plus refundable/deferred-credit liability, not earned game income. Consumption creates earned service revenue and wallet reduction; credit/refund/dispute adjustments append reversing entries, never edit historic charges. No negative balance enables paid work; earned nonvideo entitlement is separate from prepaid video wallet.

Default initial limits: platform supplier liability **$100/day and $1,000/month**, no customer grant above their explicit quoted cap, optional video **disabled** until funded approval; four in-flight image/video jobs globally and one video job per campaign, 32 cheap text/STT/TTS jobs globally, two per campaign. These are proposed controlled-launch configuration, not vendor or campaign-seat limits. Existing cache/prepared fallbacks and fairness queue provide safe service when full. Increase only with measured capacity and funded exposure approval. Multiple runtime instances must use the same counter/dispatch repository; process-local semaphores are extra protection only.

## Acceptance and failure matrix

Required fixtures: lost checkout receipt; spoofed/replayed/out-of-order/duplicate webhook; initial/renewal/3DS failure; refund unknown; payer differs from host; guest linking/recovery hostile email; downgrade during reaction/active media; one-cent rounding; concurrent campaigns individually fit but collective platform cap fails; reservation expiry before known/unknown response; restart after payment before grant publication; reconciliation after deletion/restore. Assert exact ledger conservation, no duplicate charge/usage/grant, unchanged game decision, audience-safe export and no unconsented spend. Computer-use review observes Rust host controls/account recovery/checkout redirect and honest disabled/read-only/fallback states on real output when implemented. No payment, demand or entitlement tests have run yet.

## Allowance renewal, repricing and fair admission

`AllowanceGrantId(subscription, paid_invoice, period, price_version)` is unique;
renewal credits exactly once only for a verified settled invoice, never by wall-clock
counter reset. Upgrade is an explicit fresh price/quote consent with known gateway
invoice before expanded grant; no speculative immediate proration credit. Downgrade
and unused-credit/refund terms are disclosed versioned policy. Initial nonvideo offer
reserves a worst-case supplier allowance **$8/campaign/paid period** for included
usage under pinned unit quotes; insufficient remaining allowance uses prepared
fallback or optional newly consented top-up, rather than unlimited unprofitable
billable context. This is a supplier-cost ceiling hypothesis, not a promise four
sessions always fit. Provider prices/workload qualification must establish useful
included units before sale. Reserved quoted maxima plus unknown charges count
against period ceiling; retries/hedges consume it as well.

Admission queues use weighted deficit fairness per campaign with FIFO within class:
source decision/input and already prepared audio never wait for provider permits;
interactive STT/TTS/text has a reserved **16 of32** cheap-job capacity, speculative
prefetch uses at most8, image/video separate4slot pool. Provider account token/request
rate bucket and concurrency unit are independent counters from currency liability.
Circuit open on supplier timeout/price mismatch/unknown overage stops that supplier,
returns prepared/silent typed fallback and retains liabilities. Override requires
explicit platform operator policy revision, reason, bounded currency/expiry and
funded exposure; cannot override customer consent/rights or fabricate settlement.

Financial/entitlement publication, webhook durable acceptance and deletion-complete
acknowledgement require the nonregressing protected recovery-journal boundary in
service-operations.md, so restoring old Pg cannot erase a payment, possible send or
suppression. Current versions alone in an old backup are insufficient. Journal
failure returns pending/unknown and holds affected admissions; it never issues
unprotected customer credit or provider dispatch.

## Mounted Rust customer experience

Existing df-client owns the generated CustomerService adapter and customer operation
lookup/receipt freshness independently of game session receipts. df-ui owns reusable
account/link/recovery/tenant/price/checkout/export/deletion panels; df-web mounts them
in the persistent Rust shell without reloading or destroying ongoing game state.
df-server owns native facade/adapter composition. These are existing subsystem owners,
not new client crates or browser imports of commerce policy. Server views supply
current grant/terms/allowance/payment Pending/Unknown/read-only/recovery state;
client counters/buttons cannot authorize or fabricate credit. An approved external
hosted-checkout URL is allowlisted, scoped to the initiating operation and opened
with a safe return path; return triggers operation lookup, never inferred payment.
No card secrets or raw gateway credential enter browser app/storage/logs. Operation
retry keeps exact key; obsolete account/view epochs dispose private buffers and do
not reset another player's character. Independent computer-use/vision observes
honest loss/recovery, transfer, cancel/downgrade, disabled spend, paymentUnknown and
scoped export/delete in these mounted panels, alongside uninterrupted game clients.

Customer adapter/panel delivery is blocked by G02 real Rust/WASM bridge feasibility
and existing df-client/df-ui/df-web boundary freezes as well as X12 source/auth/
storage contracts; native policy design cannot bypass those client prerequisites.
