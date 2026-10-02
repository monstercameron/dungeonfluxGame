# B-C-df-auth-D01: Principal, tenant, payer, and campaign authority

Task/attempt: `B-C-df-auth-D01-a1`  
Input revision: `4c0e050c2d30e632bd3b32c51a92cfda52479e6e`  
Status: bounded policy decision; production auth types, stores, identity issuers, and integrated authorization remain pending.

## Decision

A `Principal` is the stable authenticated subject used by `df-auth`. A guest is a principal with a guest/bootstrap lifecycle, not an anonymous request, credential, campaign role, or permission. Account linking or recovery must prove control of both identities and transfer only explicitly selected memberships; matching an email or payer record is not proof. A credential resolves to a principal only while its current credential generation is valid; D02 owns credential identity, generation, and binding fencing. Credential bytes and private payloads are excluded from default diagnostics. Before a principal exists, guest bootstrap deduplication is bounded and scoped; after authentication, create/join allocation uses its authenticated operation key and the session owner persists identity, membership, grant, and result together. Neither credentials nor trace/correlation data carry campaign roles.

Tenant ownership, payer assignment, and game membership are independent relations. `df-commerce` owns customer accounts, tenant ownership, payer assignment, and entitlement state. A payer can authorize only the scoped commercial action represented by its current grant. It is not thereby a tenant owner, campaign host, player, display, or operator. `df-auth` may derive a trusted `TenantScope` only after it verifies the principal's current explicit tenant permission; a request tenant/account/payer field and trace context cannot mint or widen that scope. Persistence consumes that trusted scope under its own tenant predicates and RLS boundary.

Campaign access comes from a current campaign-scoped membership or explicitly admitted grant for the authenticated principal. An invite is scoped to its target campaign and admission purpose, has an expiry and revocation state, and grants nothing until accepted by an authenticated principal. Acceptance allocates membership and its authorization grant/result atomically under the existing session owner and idempotent allocation operation. A guest may receive a campaign-scoped player grant through this path; guest status alone grants no campaign access. AFK, presence, and connection state are observations, not membership or consent; takeover requires the current explicit control permission and binding/lease checks. Client-selected role labels and possession of an invitation do not grant input or private-view permission.

The existing `df-auth` public boundary remains `Authenticator`, `CredentialStore`, `Principal`, and `AuthorizedAudience`; this decision adds no production type, storage schema, RPC field, or public method. Authorization maps current tenant permission to trusted `TenantScope` and current campaign permissions to a principal-bound, campaign/session-scoped `AuthorizedAudience` with the narrow capability set and access revision needed by its consumer. Host controls, player-private output, shared display, operator access, capture, and export are separate permissions. A display role grants only its shared audience. A member's permission does not disclose another member's private sheet or audio. Export requires a scoped grant naming its recipient and permitted fact/media rights, bound to its source/access revision; private viewing or capture consent alone does not imply export. This is a boundary requirement only: D04 owns the exact operator/export permission contract. Revocation fences future publication, while already received output cannot be recalled. The receiving session/API/assets boundary revalidates current access and generation before private projection or delivery; stale, revoked, expired, absent, and mismatched scope fails closed. Correlation IDs are never authorization inputs.

Financial authority does not imply private game access. A current payer or paid entitlement can satisfy the commercial prerequisite for a separately admitted paid operation, subject to the auth-owned `EntitlementReader` checking a versioned grant for trusted tenant/campaign scope and durable admission revalidating its current revision and consent. D03 freezes the reader's exact response contract. A cached snapshot cannot authorize a new operation. Payment cannot create campaign membership, host permission, `AuthorizedAudience`, or private output rights. A principal who is both payer and campaign member accesses only what the separate current campaign grant permits.

## Alternatives and rationale

- **Use one user/role for principal, tenant owner, payer, host, and player:** rejected because transfer, shared campaigns, payer changes, and private views would collapse independent capabilities.
- **Infer access from successful payment, tenant ownership, or an entitlement snapshot:** rejected because financial state is not campaign membership and snapshots can be stale or revoked.
- **Treat guest identity, invitation possession, room presence, or a client role selection as permission:** rejected because each is either lifecycle, unredeemed admission intent, observation, or untrusted input.
- **Authorize once at login or stream creation:** rejected because membership, credential/binding, and private-audience access can be revoked or replaced while a connection remains open. Revalidate at admission and disclosure boundaries.
- **Add a parallel auth role registry or duplicate identity parser here:** rejected because `df-auth` owns policy, `df-types` owns canonical shared identity primitives, commerce owns financial relationships, and persistence/session own durable allocation and enforcement.

## Bounded executable contract example

This fixture demonstrates independent tenant, billing, and campaign checks. Small integers stand in for already validated identities. It is not a production authenticator, invitation verifier, entitlement reader, store, or authorization token. The principal, payer, tenant, campaign, and membership values are intentionally distinct.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Principal(u8);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Tenant(u8);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Payer(u8);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Campaign(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CampaignRole {
    Host,
    Player,
    SharedDisplay,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    NoTenantPermission,
    NoCampaignMembership,
    WrongPrincipal,
    WrongCampaign,
    Revoked,
    Stale,
    SharedOnly,
    WrongPayer,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TenantPermission {
    principal: Principal,
    tenant: Tenant,
    active: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CampaignMembership {
    principal: Principal,
    campaign: Campaign,
    role: CampaignRole,
    generation: u8,
    active: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PayerGrant {
    payer: Payer,
    tenant: Tenant,
    campaign: Campaign,
    active: bool,
}

fn trusted_tenant_scope(
    principal: Principal,
    requested_tenant: Tenant,
    permission: Option<TenantPermission>,
) -> Result<Tenant, Refusal> {
    let Some(permission) = permission else {
        return Err(Refusal::NoTenantPermission);
    };
    if !permission.active || permission.principal != principal {
        return Err(Refusal::NoTenantPermission);
    }
    if permission.tenant != requested_tenant {
        return Err(Refusal::NoTenantPermission);
    }
    Ok(permission.tenant)
}

fn may_pay(
    payer: Payer,
    tenant: Tenant,
    campaign: Campaign,
    grant: PayerGrant,
) -> Result<(), Refusal> {
    if !grant.active || grant.payer != payer || grant.tenant != tenant || grant.campaign != campaign
    {
        return Err(Refusal::WrongPayer);
    }
    Ok(())
}

fn private_campaign_role(
    principal: Principal,
    campaign: Campaign,
    membership: Option<CampaignMembership>,
    current_generation: u8,
) -> Result<CampaignRole, Refusal> {
    let Some(membership) = membership else {
        return Err(Refusal::NoCampaignMembership);
    };
    if membership.principal != principal {
        return Err(Refusal::WrongPrincipal);
    }
    if membership.campaign != campaign {
        return Err(Refusal::WrongCampaign);
    }
    if !membership.active {
        return Err(Refusal::Revoked);
    }
    if membership.generation != current_generation {
        return Err(Refusal::Stale);
    }
    if membership.role == CampaignRole::SharedDisplay {
        return Err(Refusal::SharedOnly);
    }
    Ok(membership.role)
}

fn main() {
    let payer_only = Principal(1);
    let guest_player = Principal(2);
    let tenant = Tenant(4);
    let campaign = Campaign(7);
    let payer = Payer(9);
    let payment = PayerGrant {
        payer,
        tenant,
        campaign,
        active: true,
    };
    let owner_scope = TenantPermission {
        principal: payer_only,
        tenant,
        active: true,
    };
    let guest_membership = CampaignMembership {
        principal: guest_player,
        campaign,
        role: CampaignRole::Player,
        generation: 3,
        active: true,
    };

    assert_eq!(may_pay(payer, tenant, campaign, payment), Ok(()));
    assert_eq!(
        trusted_tenant_scope(payer_only, tenant, Some(owner_scope)),
        Ok(tenant)
    );
    assert_eq!(
        trusted_tenant_scope(payer_only, tenant, None),
        Err(Refusal::NoTenantPermission)
    );
    assert_eq!(
        private_campaign_role(payer_only, campaign, None, 3),
        Err(Refusal::NoCampaignMembership)
    );
    let host_membership = CampaignMembership {
        principal: payer_only,
        campaign,
        role: CampaignRole::Host,
        generation: 3,
        active: true,
    };
    assert_eq!(
        private_campaign_role(payer_only, campaign, Some(host_membership), 3),
        Ok(CampaignRole::Host)
    );
    assert_eq!(
        private_campaign_role(guest_player, campaign, Some(guest_membership), 3),
        Ok(CampaignRole::Player)
    );
    assert_eq!(
        private_campaign_role(guest_player, campaign, Some(guest_membership), 2),
        Err(Refusal::Stale)
    );
    let display = CampaignMembership {
        role: CampaignRole::SharedDisplay,
        ..guest_membership
    };
    assert_eq!(
        private_campaign_role(guest_player, campaign, Some(display), 3),
        Err(Refusal::SharedOnly)
    );
    assert_eq!(
        trusted_tenant_scope(payer_only, Tenant(5), Some(owner_scope)),
        Err(Refusal::NoTenantPermission)
    );
}
```

## Owners and unresolved facts

`df-auth` owns principal authentication, tenant permission derivation, campaign admission/role policy, and `AuthorizedAudience` issuance. D02 owns credential identity/generation and client binding fencing. D03 owns the auth-side `EntitlementReader` consumer boundary; `df-commerce` remains the sole payer/entitlement authority. D04 owns operator and export permission details. `df-session` owns serialized create/join allocation and durable membership grants; `df-persistence` owns their durable adapters and tenant enforcement; `df-api`/assets revalidate and filter before disclosure. These are integration hooks; this decision does not implement any of them.

Concrete principal/account/guest recovery types, invitation entropy and use limits, tenant permission set, role-to-capability mapping, audience revision representation, exact ExportGrant/operator policy, entitlement projection schema, revocation propagation SLO, persistence schema/RLS qualification, wire DTOs, and production authentication method remain open to their named shared-contract decisions. The example establishes only that paid authority and private campaign authorization are separate checks; it proves no live auth, database, payment, remote-play, or provider behavior.

## Governing input hashes

These are SHA-256 hashes of the frozen files cited in the attempt brief. Sections are listed there.

```text
cf549f4f046f76713c881228d82c6a7111d8ccc84db208cb9d9db67bb192d9f6  planning/subsystem-architecture.md
f26e1dca42e878f8a816c9f9aa37463cb8f061224598214ceee62632a161907a  planning/subsystem-interfaces.md
788a1ca834314d0245b711d454deb45c0dd4269a8990714621e2139871783ee3  planning/expansion-boundaries.md
ddd81d0d427c3bd09c8ced551f60e65f360f0469b1b186033bf8b16874f9aacc  planning/remote-play.md
bf56a48d8055639cb3634ba6fa32d72b19fb64ea3fc8df0cb649e5f7ad768b5e  planning/commerce-service.md
039b0a03b4085b43ad32c4063e2cb8fc789fc55fff7552aec6e951ec3b4704c3  development/backlog-catalog.json
55578ed92c477dfe3c306db38c6ad20390bfe8208ca998e6f3bf6f6bebbec182  AGENTS.md
2d8e327e4172643544bd80b591f38226c25b83940a10f1d9e18ada9044faeabb  planning/coding-style.md
acfe32a8d5e846aa4d3b2981529533b9f53cdcc11088424128e60c20b722adc8  ADR/0001-sqlite-agent-workflow.md
8f03d02d478086fd1f50d7aa10e8e7e766e3b15ae02f7f96efa43757c98a4dae  ADR/0004-development-reliability.md
25ab35a57ee8516a272b1ff3d04bba4def91319255d89158c9be55283ca6c35c  ADR/0005-frontier-output-evaluation.md
a6476cfef449e089639109cc6d3cf5f0800b792b57a32696258d5ac0b9511966  ADR/0002-resource-scheduling-and-cleanup.md
ee293673b01391c3a39577bd116a60abb3edfa662a7a8d1315556d6fe537d912  ADR/0003-agent-devlog.md
30860fcd071abe3664487a796e424748f40622fafbaf15d56dd18dd0328cbbf2  planning/observability.md
a388e152c973efcd937b2725e3a4878699a22f5ea928ab55701d0ec06ade0432  planning/rpc-api.md
780360bcff243a96f5fc9a39a88b47efc1402e563623c134229f56579eb714df  planning/service-operations.md
06d4e0fd31ec8aac686b755cb23a8befab3ecd019973aaeea5960f42f4882a16  planning/c-df-commerce-d01-decision.md
e1fdaa03a809002d959798ee79ce9c4259a8fb7eeb123aa84d34e61a35c477e2  planning/c-df-commerce-d02-decision.md
290105c88b1fba4c2572e058b5c6e889958e1797b8748b91f36550d0d3e779ce  planning/c-df-commerce-d03-decision.md
7a142cbd3f5e9c09c6dc4a1850e053415530eb5450d27983dc26c75230b8c5ba  rustfmt.toml
9500030ccefd0bab631fb7f1763f79f4103eca3a344c36cf15be869e330683bb  rust-toolchain.toml
```
