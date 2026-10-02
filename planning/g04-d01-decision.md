# G04-D01: initial payer identity and recovery decision

Date: 2026-10-01  
Task/attempt: `B-G04-D01-a1`  
Input revision: `3676e59e6316d9e4d77eab1f9dd812776041c2c9eb70f5dc2e396a13e50bf6b0`  
Owner: `df-auth` for principal credentials, linking and recovery; `df-commerce` for payer/tenant authority; `df-session` for membership allocation; `df-persistence` for durable atomic receipts; `df-server` for composition.  
Status: selected identity relationship and recovery invariants; concrete issuer, credential format and running implementation remain pending.

## Decision

A payer is an authenticated account principal with separately granted authority over a commerce `TenantId` and campaign payer grant. A session `MemberId` is a different identity and a different authority. A payer may also join a campaign as a member, but payment or tenant ownership alone never creates a member, grants player/host/operator permission, or reveals private player data. A member may play without becoming payer or tenant owner. Campaign host, tenant owner, payer, member and platform operator remain separate capabilities; any one human may hold multiple capabilities only through explicit grants.

Create and retain the payer account independently of any one game session. Campaign creation records its tenant and current payer grant against the authenticated account in the same durable operation receipt as campaign/session creation. Joining creates membership only after authentication and invitation-scoped authorization, using the session owner's allocation path. Neither operation infers the other from a matching person, email address, client role, trace ID, or possession of an opaque ID. Retries use their original scoped operation key and return the original result; expired request namespaces reject old keys rather than allocating another account, campaign, or member.

Initial recovery is account recovery through `df-auth`: prove control of the independently configured, verified recovery channel with a short-lived, single-use challenge, bounded attempts, and the same public response whether an account exists or not. Successful recovery increments credential generation and revokes prior refresh and client-binding grants. It restores the same principal and its explicitly authorized tenant/payer records; it does not mint a new account, campaign, member, host grant, payment entitlement, or game decision. Account loss alone does not alter committed campaign/session state. Guest-to-account linking requires proof of both current guest credential and destination account; it transfers only explicitly selected memberships, never uses email matching, and never creates another campaign. Payer/tenant ownership transfer additionally requires current-owner and recipient acceptance, current revision, settled-reservation policy, and an idempotent result. An orphaned tenant becomes restricted pending recovery; it is not made public or reassigned by support guesswork.

`df-auth` remains the principal/credential/recovery authority and issues a typed principal after successful authentication. `df-commerce` remains authoritative for tenant, payer and entitlement records; it exposes no credential secrets. `df-session` remains authoritative for membership and session state; it does not create a parallel seat registry. `df-persistence` must commit account-link, allocation, membership, payer-grant and operation-result rows atomically at their respective authorized transaction boundary and retain retired-key fences. `df-server` composes these owners; clients display returned state and cannot confer authority.

## Rationale and alternatives

The roadmap requires payer and identity decisions before S01, and the commerce boundary already separates account, tenant, campaign, payer and member. Reusing `MemberId` as payer identity would let a game-seat lifecycle change billing ownership and would make payment accidentally confer play access. Making every payer a campaign member would also deny payer-only operators safe account recovery and campaign ownership after disconnect. A single human can deliberately have both roles, but the durable IDs, permissions, and revocation remain independent.

The selected recovery mechanism is proof of a verified recovery channel, not support-mediated lookup or email matching. That matches the existing commerce account lifecycle contract for guest linking, credential recovery and session revocation while leaving credential material within `df-auth`. A new account after lost credentials, a new campaign as a recovery shortcut, password reset based only on a typed email, and implicit payer transfer were rejected because none proves continuity of account authority and each can duplicate or misassign ownership. External identity-provider choice and whether the verified channel is email, passkey, or a managed issuer remain deployment/G04 decisions; this policy does not claim one is implemented or available.

## Finite contract example

This standalone standard-library Rust model exercises the decision's authority boundary. It demonstrates payer identity surviving independently of membership and rejects payment-as-membership, membership-as-payment, stale payer transfer, and transfer without both parties' acceptance. Local newtypes and integer values here illustrate the rule; they are not production `df-types`, `df-auth`, or `df-commerce` APIs and do not prove persistence, authentication, or runtime behavior.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Principal(u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Tenant(u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Member(u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Campaign(u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Revision(u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PayerGrant {
    campaign: Campaign,
    tenant: Tenant,
    principal: Principal,
    revision: Revision,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Membership {
    campaign: Campaign,
    member: Member,
    principal: Principal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    NotPayer,
    NotMember,
    StaleRevision,
    MissingOwnerAcceptance,
    MissingRecipientAcceptance,
}

fn may_spend(grant: PayerGrant, principal: Principal) -> Result<(), Refusal> {
    if grant.principal == principal {
        Ok(())
    } else {
        Err(Refusal::NotPayer)
    }
}

fn may_play(membership: Membership, principal: Principal) -> Result<(), Refusal> {
    if membership.principal == principal {
        Ok(())
    } else {
        Err(Refusal::NotMember)
    }
}

fn transfer_payer(
    grant: PayerGrant,
    caller: Principal,
    recipient: Principal,
    expected_revision: Revision,
    owner_accepted: bool,
    recipient_accepted: bool,
) -> Result<PayerGrant, Refusal> {
    if grant.revision != expected_revision {
        return Err(Refusal::StaleRevision);
    }
    if grant.principal != caller || !owner_accepted {
        return Err(Refusal::MissingOwnerAcceptance);
    }
    if !recipient_accepted {
        return Err(Refusal::MissingRecipientAcceptance);
    }
    Ok(PayerGrant {
        principal: recipient,
        revision: Revision(grant.revision.0 + 1),
        ..grant
    })
}

fn main() {
    let payer = Principal(1);
    let player = Principal(2);
    let recipient = Principal(3);
    let campaign = Campaign(9);
    let grant = PayerGrant {
        campaign,
        tenant: Tenant(4),
        principal: payer,
        revision: Revision(7),
    };
    let member = Membership {
        campaign,
        member: Member(5),
        principal: player,
    };

    assert_eq!(may_spend(grant, payer), Ok(()));
    assert_eq!(may_play(member, player), Ok(()));
    assert_eq!(may_play(member, payer), Err(Refusal::NotMember));
    assert_eq!(may_spend(grant, player), Err(Refusal::NotPayer));
    assert_eq!(
        transfer_payer(grant, payer, recipient, Revision(6), true, true),
        Err(Refusal::StaleRevision)
    );
    assert_eq!(
        transfer_payer(grant, payer, recipient, Revision(7), true, false),
        Err(Refusal::MissingRecipientAcceptance)
    );
    assert_eq!(
        transfer_payer(grant, payer, recipient, Revision(7), false, true),
        Err(Refusal::MissingOwnerAcceptance)
    );
    let transferred = transfer_payer(grant, payer, recipient, Revision(7), true, true).unwrap();
    assert_eq!(transferred.principal, recipient);
    assert_eq!(transferred.revision, Revision(8));
    assert_eq!(may_play(member, player), Ok(()));
    assert_eq!(may_spend(transferred, payer), Err(Refusal::NotPayer));
}
```

## Failure semantics and unresolved production gates

Bad credentials, unverified recovery proof, expired/used challenge, excessive attempts, stale owner revision, missing transfer consent, invitation denial, and mismatched operation fingerprints are typed domain refusals. Transport/auth-service/storage failures are safe operational errors, not denials that imply a new identity exists. If durable commit outcome is uncertain, return pending/unknown with lookup guidance; do not retry under a fresh key, show success, create a duplicate membership/campaign, or release payer authority. A committed allocation remains committed if the client disconnects. Revocation fences new admissions and private deliveries; data already received cannot be recalled. Recovery revokes old credentials/bindings but does not reverse already committed game decisions or effects.

Still open before production: verified identity issuer and channel (managed issuer vs native passkey/email adapter); challenge and credential wire/storage types, cryptographic verifier and rotation overlap; anonymous guest bootstrap and its operation namespace; admission/origin and cookie details in G04-D03; exact principal/member/payer typed production contracts and compatibility in G03; PostgreSQL schema, uniqueness, atomic multi-owner transaction and retired-key retention in G05; host/operator capability matrix; browser, phone, tablet and display support matrix; credential persistence/security behavior on each selected device; actual tenant restriction, dispute, deletion, legal hold and export rules. Device/browser and provider/authentication services were not exercised. Proposed service budgets remain proposals, not measured identity capacity. Production recovery journal and restore replay for revocation/ownership records must pass service-operations gates before promising disaster recovery. S01 integration and independent output review remain required.

## Evidence status

The governing source files and hashes are retained in this attempt's `input-hashes.json`. The literal example above is to be extracted and checked with repository Rust 1.98.1, edition 2024, `rustfmt.toml`, `rustfmt --check`, `rustc -D warnings`, and finite execution. A passing fixture supports only these pure authority decisions. No Cargo, Clippy, WASM, browser/device, issuer, credential-store, PostgreSQL, recovery/restore, payment, or S01 end-to-end check is claimed.

## Original acceptance and verification

Acceptance criteria, preserved verbatim:

- `payer distinct membership`
- `The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.`

Original verification, preserved verbatim:

- `Freeze the cited source decision and a bounded contract example; compare payer distinct membership. Retain decision, alternatives and unresolved facts.`
- `Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.`
