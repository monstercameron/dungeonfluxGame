# AFK, disconnect, and resume admission decision

Task/attempt: `B-F45-D02/a1`

Decision input revision: `308920e328ba6df85bea1e3d2abcbea5f1b796e6`

Owner boundary: `df-auth` authenticates the principal and authorizes session access;
`df-session` owns membership-linked state, input serialization, durable policy and
connection-generation fences; `df-api` maps the decision to safe RPC outcomes;
`df-client` reconnects and accepts only fresh authorized views; `df-audio` enforces
the distinct capture/playback leases. The feature policy belongs at the existing
`df-session`/`df-auth` composition boundary. This document creates no new authority,
crate, wire method, identity type, store, or implementation API.

Governing sources: [Hosted remote and mixed-room play](remote-play.md) (Model,
permissions and audiovisual topology; Native ownership, failure and recovery; Gates,
observability and acceptance), [Runtime reliability](runtime-reliability.md)
(Authority and state ownership; Director composition and causal safeguards; Async
work and timers; Thin-client recovery and presentation; Persistence and diagnosis),
[Subsystem interfaces](subsystem-interfaces.md) (Common contract rules; Identity
and session ownership; PostgreSQL persistence and durable media; Projection, RPC and
transport; Browser interfaces), [RPC API](rpc-api.md) (Protocol and access; Common
envelopes and outcomes; SessionService methods), [Client architecture](client-architecture.md)
(Agreed direction; Agreed division of responsibilities; Decisions still open),
[Storage architecture](storage-architecture.md) (PostgreSQL planning requirements;
Remote, bookend and generated-content records), [Typed identity policy](typed-identity-policy.md)
(Decision), and [Feature refinement](feature-refinement.md) (F45 table-stakes and
delivery references). Frozen source hashes are retained in this attempt's evidence
directory; all supplied hashes matched before editing.

## Decision

Presence is observation, not authority. `ParticipantPresence` may report connected,
sleeping, disconnected, or voluntary AFK, but none of those observations changes
membership, spends a turn, pauses logical world time, expires a pending choice, or
authorizes another actor. AFK is a deliberate command that must be accepted by the
session owner under the campaign's versioned `RemotePlayPolicy`. Mere inactivity,
browser backgrounding, a heartbeat gap, or transport loss cannot be interpreted as
that command. A host pause or takeover likewise requires an explicitly authorized
command and the policy that permits it. Do not infer a universal AFK duration,
response deadline, grace period, or takeover rule from this design.

On transport loss, retain the authenticated membership and committed session state.
Fence input and capture for the departed connection generation and close its
affected streams/leases. Do not manufacture a leave, AFK, turn, NPC takeover,
reroll, duplicate reward, cancellation of committed work, or automatic game-time
advance. Pending offers remain governed by their source-defined legality and an
explicitly admitted policy deadline, if one exists. Until those rules are frozen,
disconnect alone neither accepts nor expires an offer. Existing committed jobs stay
owned by their session/run and complete or fail under their own contract.

Resume is explicit admission, not Join. `df-auth` must authenticate the returning
principal and authorize the existing session/member. `df-session` serializes the
request and verifies the current membership, policy version, any required recovery
state, and current lease/binding ownership before admitting the connection. It
atomically issues or replaces only the requested transient connection binding and
its explicitly granted capability leases. A duplicate retry with the same stable
operation identity and canonical request returns its recorded result; a changed
request using that identity is refused. Resume never allocates a second member or
seat. Reconnect alone cannot steal an active input/audio lease; replacement is a
separate explicit takeover/revocation decision authorized by the selected policy.

After admission, the server supplies a current authorized snapshot and current
pending choices/timeline. The client must reject a snapshot or stream update from an
older connection generation, session revision, or presentation epoch/sequence under
the applicable freshness contract. Revalidate submitted offers and operation
identity against current server state; an old draft is uncommitted and needs a
current offer. Look up an uncertain prior operation before considering replay. If
the old generation, lease, policy, or operation namespace is stale/retired, refuse
with the existing safe typed stale/recovery/operation outcome. Do not silently
convert the request to Join or treat missing retained receipt data as proof that an
operation never committed.

The admission transaction and durable decision remain under the existing fenced
`df-session` owner and PostgreSQL repository. Authorization is checked independently
of connection IDs, trace IDs, view cursors, and client claims. Private projection,
asset access, input, capture, and audio are reauthorized at their boundaries;
closing a connection invalidates its affected lease and unsent deliveries. Already
received bytes cannot be recalled. Reconnection does not replay obsolete one-shot
audio or old buffers. `df-client` reports actual receipt/playback facts; it does not
infer that playback happened from server delivery.

## Alternatives and rationale

- Automatically mark AFK after a fixed inactivity interval: rejected because no
  source-defined interval exists, wall-clock/browser signals are imperfect, and
  inactivity is not affirmative consent to change action or takeover policy.
- Auto-pass, expire a reaction, advance time, or hand the character to an NPC on
  disconnect: rejected because these alter source-governed game outcomes and can
  duplicate or consume decisions during network uncertainty.
- Treat reconnect as a new Join: rejected because it can create duplicate members,
  bypass invitation/access checks, and repeat allocations after a lost receipt.
- Reuse the old connection's input/audio lease by stable membership alone: rejected
  because stale tabs and concurrent devices could both control or capture. The
  binding generation and per-capability lease must be checked at each relevant
  boundary; takeover needs an explicit authorized decision.
- Let a client decide freshness or replay locally: rejected because observed
  revision is context and server owner state/fencing is authoritative. Current
  authorized state and operation lookup decide admission and retry.

## Bounded executable contract example

This finite, std-only example is a decision model for the policy above, not a frozen
production API or proof of runtime behavior. It executes accepted explicit AFK and
resume requests, rejects unauthenticated/nonmember resumes, prevents implicit AFK
or takeover, rejects stale generations and policy versions, and rejects duplicate
active input ownership. Its integers are fixture identities/revisions, not proposed
production bounds or measured timeouts.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Presence {
    Connected,
    Sleeping,
    Disconnected,
    VoluntaryAfk,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    Unauthenticated,
    NotMember,
    StaleGeneration,
    StalePolicy,
    InputLeaseHeld,
    AfkRequiresCommand,
    TakeoverRequiresCommand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Participant {
    member: u8,
    principal: u8,
    generation: u8,
    policy_version: u8,
    presence: Presence,
    input_lease: Option<u8>,
}

fn explicit_afk(participant: &mut Participant, command_authorized: bool) -> Result<(), Refusal> {
    if !command_authorized {
        return Err(Refusal::AfkRequiresCommand);
    }
    participant.presence = Presence::VoluntaryAfk;
    Ok(())
}

fn attempt_takeover(explicitly_authorized: bool) -> Result<(), Refusal> {
    if explicitly_authorized {
        Ok(())
    } else {
        Err(Refusal::TakeoverRequiresCommand)
    }
}

fn resume(
    participant: &mut Participant,
    authenticated_principal: Option<u8>,
    existing_member: Option<u8>,
    expected_generation: u8,
    current_policy_version: u8,
    requested_lease: u8,
) -> Result<u8, Refusal> {
    let principal = authenticated_principal.ok_or(Refusal::Unauthenticated)?;
    if principal != participant.principal || existing_member != Some(participant.member) {
        return Err(Refusal::NotMember);
    }
    if expected_generation != participant.generation {
        return Err(Refusal::StaleGeneration);
    }
    if current_policy_version != participant.policy_version {
        return Err(Refusal::StalePolicy);
    }
    if participant.input_lease.is_some() {
        return Err(Refusal::InputLeaseHeld);
    }

    participant.generation += 1;
    participant.input_lease = Some(requested_lease);
    participant.presence = Presence::Connected;
    Ok(participant.generation)
}

fn main() {
    let mut participant = Participant {
        member: 7,
        principal: 9,
        generation: 3,
        policy_version: 4,
        presence: Presence::Disconnected,
        input_lease: None,
    };

    let mut sleeping = participant;
    sleeping.presence = Presence::Sleeping;
    assert_eq!(sleeping.presence, Presence::Sleeping);

    // An observation does not become an AFK command or a takeover.
    assert_eq!(participant.presence, Presence::Disconnected);
    assert_eq!(
        explicit_afk(&mut participant, false),
        Err(Refusal::AfkRequiresCommand)
    );
    assert_eq!(participant.presence, Presence::Disconnected);
    assert_eq!(
        attempt_takeover(false),
        Err(Refusal::TakeoverRequiresCommand)
    );
    explicit_afk(&mut participant, true).expect("explicit authorized AFK");
    assert_eq!(participant.presence, Presence::VoluntaryAfk);

    assert_eq!(
        resume(&mut participant, None, Some(7), 3, 4, 20),
        Err(Refusal::Unauthenticated)
    );
    assert_eq!(
        resume(&mut participant, Some(9), Some(8), 3, 4, 20),
        Err(Refusal::NotMember)
    );
    assert_eq!(
        resume(&mut participant, Some(9), Some(7), 2, 4, 20),
        Err(Refusal::StaleGeneration)
    );
    assert_eq!(
        resume(&mut participant, Some(9), Some(7), 3, 5, 20),
        Err(Refusal::StalePolicy)
    );

    assert_eq!(resume(&mut participant, Some(9), Some(7), 3, 4, 20), Ok(4));
    assert_eq!(participant.member, 7, "resume preserves membership");
    assert_eq!(participant.presence, Presence::Connected);
    assert_eq!(participant.input_lease, Some(20));
    assert_eq!(
        resume(&mut participant, Some(9), Some(7), 4, 4, 21),
        Err(Refusal::InputLeaseHeld)
    );

    println!("PASS: explicit AFK, explicit admission, and stale/unauthorized refusal model");
}
```

## Unresolved production gates

This decision does not choose AFK/host-pause semantics for each campaign, any
response-expiry rule, numerical reconnect/backoff/heartbeat or admission limits,
lease durations/renewal, credential/bootstrap recovery, multi-tab takeover UX,
retention periods, PostgreSQL schema, protobuf numbers, browser transport, or device
matrix. Those require the owning G03/G04/G05/G08 contracts, measured load/device
evidence, and coordinated source changes. Concurrent sibling proposals are not
accepted dependencies. Supported phones are untested; desktop or multi-tab success
cannot establish phone sleep/network behavior. No application source/workspace
implementation exists in this task revision, so no runtime, browser, phone, audio,
provider, native, or WASM product behavior is claimed.

## Original acceptance criteria and verification

Acceptance criteria, preserved verbatim:

- `explicit admission`
- `The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.`

Original verification, preserved verbatim:

- `Freeze the cited source decision and a bounded contract example; compare explicit admission. Retain decision, alternatives and unresolved facts.`
- `Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.`

This source example is bounded policy evidence only. Native/WASM workspace checks,
Clippy, RPC/auth/session integration, persistence, browser/device/audio, and actual
remote play acceptance remain unperformed.
