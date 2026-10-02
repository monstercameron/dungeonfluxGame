# B-C-df-auth-D03: current entitlement reader boundary

Task/attempt: `B-C-df-auth-D03/a1`  
Owner: `df-auth`  
Status: bounded design decision; production types, persistence adapter, and admission transaction remain unimplemented.

## Decision

`df-auth` owns the consumer-side `EntitlementReader` port and its persistence projection. The port reads the current, committed, versioned campaign grant projection owned by the single `df-commerce` authority. Its lookup is scoped by a trusted `TenantScope` derived from the authenticated `Principal` and an explicit campaign identity. A caller-supplied tenant, payer, account, cached snapshot, checkout result, provider string, or trace identifier cannot establish that scope or create a grant.

The port reports a current commercial capability and its commerce revision as facts for a later admission decision. It does not own payer, subscription, payment, allowance, or entitlement state. `df-commerce` remains their sole authority; the native `df-server` `CommerceService` is the existing facade and `df-persistence` owns durable transaction implementation. The grant revision must be revalidated with the current commerce authority in the same durable transaction that accepts a paid reservation/effect. A cached projection is presentation data and cannot authorize admission.

Commercial capability and game permission are independent. `df-auth` must separately establish campaign membership and the requested private `AuthorizedAudience`/role from current auth state. A valid paid grant cannot create a member, host, player, operator, invitation acceptance, or private audience. A paid private operation requires both the independently authorized audience and its applicable current commercial capability. Financial facts do not grant access to another user's private game material.

The conceptual operation is `read_current_grant(trusted_scope, campaign)`: it returns a current commerce snapshot, no grant, or a typed read failure. Its production Rust signature is intentionally left for the shared type and persistence owners to freeze. The minimum behavior is to scope one campaign grant to the trusted tenant, return the grant revision and bounded capability facts, and leave all commercial state unchanged. Durable admission must distinguish a current revision from a stale one before accepting a paid reservation/effect. Rejections fail closed and preserve the distinction between absent/revoked/expired or unresolved access and an unavailable or inconsistent source. They do not return success-shaped defaults.

## Ownership and alternatives

The ownership follows the existing subsystem boundary: commerce commits and versions commercial truth; auth consumes that projection and owns identity, membership, role, and audience permission; persistence performs the transaction; server composes the native facade. A second auth wallet or entitlement ledger was rejected because it would duplicate commercial truth and could disagree after payment uncertainty, refund, cancellation, or revocation. Letting commerce authenticate a principal or authorize private game content was rejected because commercial relationships do not establish game identity or membership. Treating a cached grant, successful redirect, or trace context as current authority was rejected because it cannot safely represent revocation or unsettled payment.

The Rust example is a finite contract fixture, not a public production type, database adapter, or complete authentication implementation. Its small integers stand for already validated identities. It demonstrates the current-version gate and the independent private-audience gate; it does not prove that production reads are fresh, atomic, durable, or correctly authorized.

## Bounded executable contract example

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Tenant(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Campaign(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TenantScope {
    tenant: Tenant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GrantState {
    Active,
    Restricted,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GrantProjection {
    tenant: Tenant,
    campaign: Campaign,
    revision: u64,
    paid_through: u64,
    state: GrantState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct VersionedCapability {
    campaign: Campaign,
    commerce_revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReadOutcome {
    Current(GrantProjection),
    Missing,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    NoGrant,
    SourceUnavailable,
    WrongTenant,
    WrongCampaign,
    RestrictedGrant,
    UnknownGrant,
    ExpiredGrant,
    StaleRevision,
    PrivateAudienceRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PrivateAudience {
    campaign: Campaign,
    authorized: bool,
}

fn current_capability(
    scope: TenantScope,
    campaign: Campaign,
    outcome: ReadOutcome,
    now: u64,
) -> Result<VersionedCapability, Refusal> {
    let projection = match outcome {
        ReadOutcome::Current(projection) => projection,
        ReadOutcome::Missing => return Err(Refusal::NoGrant),
        ReadOutcome::Unavailable => return Err(Refusal::SourceUnavailable),
    };
    if projection.tenant != scope.tenant {
        return Err(Refusal::WrongTenant);
    }
    if projection.campaign != campaign {
        return Err(Refusal::WrongCampaign);
    }
    match projection.state {
        GrantState::Active => {}
        GrantState::Restricted => return Err(Refusal::RestrictedGrant),
        GrantState::Unknown => return Err(Refusal::UnknownGrant),
    }
    if now >= projection.paid_through {
        return Err(Refusal::ExpiredGrant);
    }
    Ok(VersionedCapability {
        campaign,
        commerce_revision: projection.revision,
    })
}

fn admit_private_campaign(
    capability: VersionedCapability,
    current_commerce_revision: u64,
    audience: PrivateAudience,
) -> Result<(), Refusal> {
    if capability.commerce_revision != current_commerce_revision {
        return Err(Refusal::StaleRevision);
    }
    if audience.campaign != capability.campaign || !audience.authorized {
        return Err(Refusal::PrivateAudienceRequired);
    }
    Ok(())
}

fn main() {
    let scope = TenantScope { tenant: Tenant(4) };
    let campaign = Campaign(8);
    let projection = GrantProjection {
        tenant: Tenant(4),
        campaign,
        revision: 12,
        paid_through: 100,
        state: GrantState::Active,
    };
    let capability = current_capability(scope, campaign, ReadOutcome::Current(projection), 99);
    assert_eq!(
        capability,
        Ok(VersionedCapability {
            campaign,
            commerce_revision: 12,
        })
    );
    let capability = match capability {
        Ok(capability) => capability,
        Err(refusal) => panic!("unexpected refusal: {refusal:?}"),
    };
    assert_eq!(
        admit_private_campaign(
            capability,
            12,
            PrivateAudience {
                campaign,
                authorized: true,
            },
        ),
        Ok(())
    );
    assert_eq!(
        admit_private_campaign(
            capability,
            13,
            PrivateAudience {
                campaign,
                authorized: true,
            },
        ),
        Err(Refusal::StaleRevision)
    );
    assert_eq!(
        admit_private_campaign(
            capability,
            12,
            PrivateAudience {
                campaign,
                authorized: false,
            },
        ),
        Err(Refusal::PrivateAudienceRequired)
    );
    assert_eq!(
        current_capability(
            TenantScope { tenant: Tenant(5) },
            campaign,
            ReadOutcome::Current(projection),
            99,
        ),
        Err(Refusal::WrongTenant)
    );
    assert_eq!(
        current_capability(scope, Campaign(9), ReadOutcome::Current(projection), 99,),
        Err(Refusal::WrongCampaign)
    );
    assert_eq!(
        current_capability(scope, campaign, ReadOutcome::Missing, 99),
        Err(Refusal::NoGrant)
    );
    assert_eq!(
        current_capability(scope, campaign, ReadOutcome::Unavailable, 99),
        Err(Refusal::SourceUnavailable)
    );
    assert_eq!(
        current_capability(
            scope,
            campaign,
            ReadOutcome::Current(GrantProjection {
                state: GrantState::Restricted,
                ..projection
            }),
            99,
        ),
        Err(Refusal::RestrictedGrant)
    );
    assert_eq!(
        current_capability(
            scope,
            campaign,
            ReadOutcome::Current(GrantProjection {
                state: GrantState::Unknown,
                ..projection
            }),
            99,
        ),
        Err(Refusal::UnknownGrant)
    );
    assert_eq!(
        current_capability(scope, campaign, ReadOutcome::Current(projection), 100,),
        Err(Refusal::ExpiredGrant)
    );
}
```

## Evidence limits and follow-up

The fixture proves only that this finite mapping accepts a current active unexpired scoped projection, carries its revision forward, refuses the listed invalid inputs, and does not substitute payment for private audience authorization. It does not establish production `TenantScope` provenance, commerce projection freshness, payment verification, rights, membership storage, transaction isolation, reservation atomicity, revocation races, provider behavior, RPC exposure, or running game behavior. Those checks remain with the concrete shared-type, persistence, commerce, server, and integration owners. No live service or provider call was used.
