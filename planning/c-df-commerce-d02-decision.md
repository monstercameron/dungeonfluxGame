# B-C-df-commerce-D02: commercial identity authority

Task/attempt: `B-C-df-commerce-D02-a1`
Status: proposed boundary decision; no commercial production types, issuer, database, or payment integration is implemented.

## Decision

`df-commerce` is the one canonical authority for commercial relationships and their versioned state: customer account, tenant ownership, payer assignment, subscription/price version, entitlement snapshot, allowance grant, reservations, and ledger. These names have distinct semantic identity roles. A campaign is owned by exactly one tenant and has exactly one current payer grant. A payer is a commercial relationship, not the authenticated principal, campaign host, tenant owner, member, or platform operator. Changing payer requires consent from both parties, current relationship revision, settlement or explicit resolution of outstanding reservations, and an idempotent operation result. Payment or payer status grants no game, secret, membership, or operator permission.

`df-auth` remains the authority for authentication and permissions. It derives trusted `TenantScope` from the authenticated principal; a request-supplied tenant/account/payer identifier cannot create that scope. `df-auth`'s `EntitlementReader` consumes a versioned grant projection from commerce, but a cached snapshot is presentation data only. `df-api` asks the native `df-server`-supplied `CommerceService` facade; the facade coordinates the pure `CommerceTransition` with consumer-owned ports. `df-persistence` owns durable transactions, outbox, uniqueness and idempotency; `df-providers` owns the `PaymentGateway` adapter and verified observations. `df-provider-api::BudgetStore` delegates to commerce and must not keep another wallet or allowance authority. `df-session` uses auth-authorized campaign capabilities and does not import commerce. Admission revalidates current grant and consent in the same durable transaction that accepts a reservation/effect.

Commercial identity roles must be distinct typed values, never interchangeable strings or aliases. Reuse the canonical opaque 16-byte identity validation contract in `df-types` (exact length, nonzero, supplied bytes preserved); that contract provides neither issuance, uniqueness, authentication, nor access. The actual existing `df-types` kinds are `SessionId`, `MemberId`, `ClientBindingId`, `RunId`, and `OperationId`; none semantically means customer account, tenant, payer, subscription, price version, entitlement revision, allowance grant, reservation, or ledger entry. Do not alias one of them or invent a second byte parser. Their future commercial role-specific wrappers and constructors belong to the `df-types` identity owner, with `df-commerce` owning the relationship rules that use them. No production commercial-ID type or issuer is claimed here.

Use the reviewed typed-units policy: currency values and usage quantities come from `df-types`; quote identity/version, customer consent, allowance and reservation accounting, payment state, and ledger transitions belong to `df-commerce`. Keep currency tag, usage kind, quote version, and amount distinct. Checked integer arithmetic, mismatch/overflow rejection, upward maximum-liability rounding, and no implicit currency conversion apply. A syntactically valid currency tag is not an approved currency, quote, gateway scale, or permission to charge. Gateway minor-unit conversion is a native adapter responsibility. An allowance grant is a ledger-backed entitlement event, not a mutable periodic counter: it is keyed by a verified settled invoice and its subscription, period, and pinned price version, and its unique identity makes duplicate renewal delivery harmless.

The durable state transition is the authority boundary. A successful decision is committed with ledger changes, idempotent result, grant projection changes, and outbox records atomically; external gateway observations enter through verified, persisted inbox data. A returned checkout URL or browser return does not prove payment. Unknown charge/refund or provider liability stays pending/unknown and retains worst-case liability; expiry alone never releases it. Stale, duplicate, out-of-order, unverified, or ambiguous payment evidence cannot add access or allowance. A failed transaction cannot report success. Retrying uses the same operation key and request fingerprint; a conflicting fingerprint is a typed conflict. Payer transfer, refund, cancellation, deletion, and reconciliation never rewrite historic ledger entries or silently regenerate deleted private payload.

## Alternatives and rationale

- Letting `df-auth`, `df-provider-api`, or the game own payer/allowance state would create competing commercial truth and allow cached or provider-local state to authorize spend. Rejected by the commerce ownership and budget delegation boundaries.
- Treating payer, host, tenant owner, and authenticated user as one `UserId` would conflate independent capabilities and make transfer or shared campaigns unsafe. Keep distinct typed roles and require explicit authorization at each boundary.
- Using ad hoc strings, IDs borrowed from gameplay, or a commerce-local byte parser would bypass canonical identity validation and permit accidental cross-role substitution. Reuse `df-types` validation and add role-specific types only through that canonical owner.
- Resetting a balance on a wall-clock period or crediting on webhook arrival would grant duplicate/unpaid allowance and lose auditability. Grant only once for verified settled invoice identity and record reversals as new ledger entries.
- Making cached entitlements or successful checkout redirects authoritative would make revocation and ambiguous payment unsafe. Revalidate the current version inside durable admission and wait for verified settlement.
- A gateway-owned price or floating-point total would import vendor scale/rounding into domain policy. Keep pinned quotes and exact integer liability in commerce, with explicit native adapter conversion.

## Bounded executable contract example

This finite Rust example models the commercial decision boundary only. Its primitive integers stand in for already validated typed IDs and revisions; it does not define production IDs, storage, transaction semantics, gateway verification, or an API. It exercises one accepted grant and refusal of wrong tenant, stale grant version, and replayed allowance identity.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tenant(u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Payer(u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Grant(u8);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Revision(u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Refusal {
    WrongTenant,
    StaleGrant,
    DuplicateGrant,
}

#[derive(Debug, Clone, Copy)]
struct Authority {
    tenant: Tenant,
    payer: Payer,
    current: Grant,
    revision: Revision,
}

fn apply_verified_settlement(
    authority: Authority,
    trusted_tenant: Tenant,
    observed_payer: Payer,
    observed_grant: Grant,
    observed_revision: Revision,
    already_recorded: bool,
) -> Result<Grant, Refusal> {
    if trusted_tenant != authority.tenant || observed_payer != authority.payer {
        return Err(Refusal::WrongTenant);
    }
    if observed_revision != authority.revision || observed_grant != authority.current {
        return Err(Refusal::StaleGrant);
    }
    if already_recorded {
        return Err(Refusal::DuplicateGrant);
    }
    Ok(observed_grant)
}

fn main() {
    let authority = Authority {
        tenant: Tenant(4),
        payer: Payer(9),
        current: Grant(12),
        revision: Revision(3),
    };
    assert_eq!(
        apply_verified_settlement(
            authority,
            Tenant(4),
            Payer(9),
            Grant(12),
            Revision(3),
            false
        ),
        Ok(Grant(12))
    );
    assert_eq!(
        apply_verified_settlement(
            authority,
            Tenant(5),
            Payer(9),
            Grant(12),
            Revision(3),
            false
        ),
        Err(Refusal::WrongTenant)
    );
    assert_eq!(
        apply_verified_settlement(
            authority,
            Tenant(4),
            Payer(9),
            Grant(12),
            Revision(2),
            false
        ),
        Err(Refusal::StaleGrant)
    );
    assert_eq!(
        apply_verified_settlement(authority, Tenant(4), Payer(9), Grant(12), Revision(3), true),
        Err(Refusal::DuplicateGrant)
    );
}
```

## Owners, evidence, and unresolved gates

Canonical policy owner: `df-commerce`. Primitive ID and exact unit owners: `df-types`. Authentication/scope/permission owner: `df-auth`. Durable atomicity, uniqueness, inbox/outbox, and recovery implementation: `df-persistence`. Gateway verification and conversion: native `df-providers`/`df-server` adapters. Budget consumer: `df-provider-api`, delegating to commerce. RPC/client codecs and views remain owned by `df-api`, `df-client`, and their API wave; game/session authority remains with `df-session` and `df-engine`. These are integration hooks, not authorization to implement those contracts here.

Unresolved production gates include concrete commercial newtypes and trusted ID allocation; selected authentication/recovery channel; supported currency registry and gateway scale mapping; quote authority/effective intervals and customer disclosure; exact offer, grace, refund, deletion and retention policy; database schema/isolation and protected recovery-journal integration; verified webhook endpoint/API version and reconciliation operations; public/admin RPC mappings and compatibility; operational funded exposure/capacity limits; and production native/WASM, browser, provider, payment, concurrency, restore, and customer-flow evidence. The commerce source's numerical launch limits and $8 allowance are hypotheses/configuration proposals, not measured demand or provider qualification. No browser/device support is tested by this decision. No production runtime behavior is proven by the example.

## Governing input hashes

Hashes are SHA-256 of the frozen source files read for this decision. Source sections are listed in the issued worker context; the policy and its example are source-bound at the commit recorded by the handoff.

```text
0160ad8e8ec38f768e2348209b9989e30e8f403d9b1a4ebf694f0801f7206932  planning/implementation-roadmap.md
bf56a48d8055639cb3634ba6fa32d72b19fb64ea3fc8df0cb649e5f7ad768b5e  planning/commerce-service.md
039b0a03b4085b43ad32c4063e2cb8fc789fc55fff7552aec6e951ec3b4704c3  development/backlog-catalog.json
55578ed92c477dfe3c306db38c6ad20390bfe8208ca998e6f3bf6f6bebbec182  AGENTS.md
2d8e327e4172643544bd80b591f38226c25b83940a10f1d9e18ada9044faeabb  planning/coding-style.md
acfe32a8d5e846aa4d3b2981529533b9f53cdcc11088424128e60c20b722adc8  ADR/0001-sqlite-agent-workflow.md
8f03d02d478086fd1f50d7aa10e8e7e766e3b15ae02f7f96efa43757c98a4dae  ADR/0004-development-reliability.md
25ab35a57ee8516a272b1ff3d04bba4def91319255d89158c9be55283ca6c35c  ADR/0005-frontier-output-evaluation.md
cf549f4f046f76713c881228d82c6a7111d8ccc84db208cb9d9db67bb192d9f6  planning/subsystem-architecture.md
f26e1dca42e878f8a816c9f9aa37463cb8f061224598214ceee62632a161907a  planning/subsystem-interfaces.md
176e5a8fb04e1d9cb2402225c9eb1bb4ffd110abf353f226a302e10e780acf05  planning/runtime-reliability.md
69c6ef1ec2ce02b0ecc33c361e83ef9a027f669df1abe49199605552466a1da6  planning/storage-architecture.md
30860fcd071abe3664487a796e424748f40622fafbaf15d56dd18dd0328cbbf2  planning/observability.md
3498bda5d59aca8e4064cca74ce711e1a52a1b0eb71a3c7c240018b107b777c2  planning/shared-contract-waves.md
cfa768418cd2ee4ad046994f409257770fb71b0d43ba106a13c79e020d350604  planning/typed-units-policy.md
7f16ae9557c244710e5b59c260c371bfe3d25a84aab62c6990cfe502b17edb67  planning/protected-journal-policy.md
7a142cbd3f5e9c09c6dc4a1850e053415530eb5450d27983dc26c75230b8c5ba  rustfmt.toml
9500030ccefd0bab631fb7f1763f79f4103eca3a344c36cf15be869e330683bb  rust-toolchain.toml
```
