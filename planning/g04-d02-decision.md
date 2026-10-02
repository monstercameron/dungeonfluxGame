# G04-D02: Device and browser role matrix

Task/attempt: `B-G04-D02-a1`  
Input revision: `308920e328ba6df85bea1e3d2abcbea5f1b796e6`  
Owner: `df-auth` owns guest/allocation admission and capacity decisions; `df-client`/`df-web` own client role selection and device adaptation; `df-server` composes session allocation.  
Status: design decision and finite contract example; implementation, actual browser/device validation, and measured service capacity remain open.

## Decision

Keep two client roles, independent of device: **player** (one player's authorized view and input) and **shared display** (the group view). Target the following browser/device combinations for the first supported matrix:

| Client role | Target device and browser families | Role behavior |
| --- | --- | --- |
| Player | Phone or tablet using Safari on iOS/iPadOS, or Chrome on Android | One authenticated player audience per admitted membership; responsive touch-first controls. |
| Player | Laptop/desktop using Chrome, Edge, Firefox, or Safari | Same player audience and commands; keyboard, pointer, and touch where available. |
| Shared display | Laptop/desktop browser (Chrome, Edge, Firefox, or Safari), shown on its screen or connected to a TV | Group-safe projection only; a connected TV is a display surface, not another identity or participant. |

The matrix identifies intended browser families, not a claim that a particular version or device has passed compatibility checks. Direct browsing on smart-TV operating systems is outside this initial target; a TV may show the shared-display role through an attached laptop/desktop. A laptop may run either role, but one client instance selects one role at a time. The **host** is an independently authorized capability on a player or shared-display client, not a third client role. Role selection, device class, browser claims, and trace metadata grant no permission.

Do not impose a fixed participant/seat count. `df-auth` admits each guest/member allocation under current invitation, identity, and session policy, with idempotent allocation keys. The session owner remains the sole authority for membership; there is no parallel seat registry. Admission is bounded by measured per-session and service resource budgets (including connection/stream, projection, relay, and recipient-scaled delivery costs) and configured operational capacity. The number of player memberships is therefore an observed consequence of currently available measured capacity, not a guessed product constant. When the applicable budget is exhausted or unavailable, refuse new admission with a typed capacity/unavailable outcome; do not evict or invalidate already admitted members to make room. Do not describe this as unlimited capacity. Set no numeric budget until representative load and target-device measurements justify it.

Reuse the existing `df-types` opaque IDs and `df-auth` credential/bootstrap authority. Browser bootstrap follows [Bootstrap cookie and origin policy](bootstrap-origin-policy.md): configured-origin HTTPS/WSS admission is separate from principal authentication and per-operation authorization. The player/display permission and host/operator permissions remain distinct. Stable membership and client binding remain distinct from transient tabs/connections; reconnect rebinds the existing member rather than allocating another one. Authorization is rechecked at publication/delivery and revocation fences active streams. No new crate, RPC, identity type, seat registry, or credential authority is introduced.

The matrix selects role and device/browser targets only. It does not select browser version floors, transport implementation, credential storage/rotation, codec/capture formats, audio routing, memory/frame/latency targets, or deployment topology. Those remain the named G01/G02/G03/G04/G08 gates, and no browser/device compatibility is claimed before they are resolved and tested.

## Rationale and alternatives

The existing client architecture explicitly defines player and shared-display roles as device-independent, with phones/laptops/tablets for player clients and TV/laptop surfaces for the shared display. This preserves that division while selecting concrete target browser families and leaving the host as a permission overlay. The browser implementation remains Rust/WASM; browser API glue is not a second application implementation.

An app-only/native-wrapper matrix was rejected for the initial target because it would add a distribution/runtime path without support in the agreed browser architecture. A mandatory smart-TV browser target was rejected because TV browsers are a separate compatibility surface with no evidence or requirement that the client run directly there; attached display output meets the existing shared-display target. A single generic “modern browser” target was rejected because it does not name the phone/tablet and shared-display surfaces this decision is meant to cover. A fixed seat count was rejected because no representative capacity evidence supports one; it would cap sessions without reference to actual load or device mix. An unbounded admission promise was also rejected: measured resource exhaustion must refuse new work safely.

## Failure and refusal behavior

Reject unauthenticated or duplicate/stale allocation attempts according to existing `df-auth` identity, expiry, and deduplication rules. Reject a requested audience that the authenticated principal does not hold; choosing Player or Shared Display cannot manufacture that grant or the host/operator capability. Reject unsupported client/device/browser combinations explicitly at the client compatibility boundary once the version policy is selected; do not silently switch role, weaken private projection, or claim a successful connection. If measured admission capacity is exhausted, refuse the new allocation before committing membership. If storage/commit outcome is uncertain, look up the same idempotency key before retrying; never allocate twice or report fabricated success. On active revocation, close/fence affected streams and clear obsolete private client state. Loss of authorization or connection does not cancel an already committed decision or its owned effects.

## Finite std-only contract example

This example is a literal, standalone decision fixture. Its resource values are illustrative inputs to show the budget rule; they are not measured limits or chosen production bounds. It exercises the target role/device matrix, separate host capability, no numeric participant cap, budget exhaustion, wrong-role authority, and unsupported direct smart-TV browsing. It does not implement browser detection, authentication, deduplication, persistence, stream revocation, load measurement, or device/browser behavior.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    Player,
    SharedDisplay,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Device {
    PhoneIos,
    TabletIos,
    PhoneAndroid,
    TabletAndroid,
    LaptopDesktop,
    DirectSmartTv,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Browser {
    Safari,
    Chrome,
    Edge,
    Firefox,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Permission {
    Player,
    SharedDisplay,
    Host,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Refusal {
    UnsupportedTarget,
    MissingPermission,
    CapacityExhausted,
}

fn target_supported(role: Role, device: Device, browser: Browser) -> bool {
    match (role, device, browser) {
        (Role::Player, Device::PhoneIos | Device::TabletIos, Browser::Safari) => true,
        (Role::Player, Device::PhoneAndroid | Device::TabletAndroid, Browser::Chrome) => true,
        (Role::Player, Device::LaptopDesktop, _) => true,
        (Role::SharedDisplay, Device::LaptopDesktop, _) => true,
        _ => false,
    }
}

fn required_permission(role: Role) -> Permission {
    match role {
        Role::Player => Permission::Player,
        Role::SharedDisplay => Permission::SharedDisplay,
    }
}

fn admit(
    role: Role,
    device: Device,
    browser: Browser,
    grants: &[Permission],
    requested_host: bool,
    remaining_measured_budget: u32,
    measured_cost: u32,
) -> Result<(), Refusal> {
    if !target_supported(role, device, browser) {
        return Err(Refusal::UnsupportedTarget);
    }
    if !grants.contains(&required_permission(role))
        || (requested_host && !grants.contains(&Permission::Host))
    {
        return Err(Refusal::MissingPermission);
    }
    if measured_cost > remaining_measured_budget {
        return Err(Refusal::CapacityExhausted);
    }
    Ok(())
}

fn main() {
    let player_and_host = [Permission::Player, Permission::Host];
    let display = [Permission::SharedDisplay];

    assert_eq!(
        admit(
            Role::Player,
            Device::PhoneIos,
            Browser::Safari,
            &player_and_host,
            true,
            8,
            3,
        ),
        Ok(())
    );
    assert_eq!(
        admit(
            Role::Player,
            Device::TabletIos,
            Browser::Safari,
            &[Permission::Player],
            false,
            8,
            3,
        ),
        Ok(())
    );
    assert_eq!(
        admit(
            Role::Player,
            Device::TabletAndroid,
            Browser::Chrome,
            &[Permission::Player],
            false,
            8,
            3,
        ),
        Ok(())
    );
    assert_eq!(
        admit(
            Role::SharedDisplay,
            Device::LaptopDesktop,
            Browser::Firefox,
            &display,
            false,
            8,
            3,
        ),
        Ok(())
    );
    assert_eq!(
        admit(
            Role::Player,
            Device::LaptopDesktop,
            Browser::Edge,
            &[Permission::Player],
            true,
            8,
            1,
        ),
        Err(Refusal::MissingPermission)
    );
    assert_eq!(
        admit(
            Role::Player,
            Device::DirectSmartTv,
            Browser::Chrome,
            &[Permission::Player],
            false,
            8,
            1,
        ),
        Err(Refusal::UnsupportedTarget)
    );
    assert_eq!(
        admit(
            Role::Player,
            Device::PhoneAndroid,
            Browser::Chrome,
            &[Permission::Player],
            false,
            2,
            3,
        ),
        Err(Refusal::CapacityExhausted)
    );

    // Allocations continue while measured budget remains; no seat-count branch exists.
    let remaining = 20;
    let per_player_cost = 2;
    let mut admitted_players = 0;
    let mut spent = 0;
    while spent + per_player_cost <= remaining {
        assert_eq!(
            admit(
                Role::Player,
                Device::LaptopDesktop,
                Browser::Chrome,
                &[Permission::Player],
                false,
                remaining - spent,
                per_player_cost,
            ),
            Ok(())
        );
        admitted_players += 1;
        spent += per_player_cost;
    }
    assert_eq!(admitted_players, 10);
    assert_eq!(spent, remaining);
    println!("device role matrix example: 8 assertions plus budget loop passed");
}
```

## Sources and unresolved production gates

The source-backed roles and device surfaces are in [Browser client architecture](client-architecture.md#agreed-direction). Permission separation, idempotent scoped guest/allocation admission, single session membership authority, bindings, revocation, and capacity failure semantics are in [Subsystem interface contracts](subsystem-interfaces.md#identity-and-session-ownership) and [Remote play](remote-play.md#model-permissions-and-audiovisual-topology). The authentication/bootstrap owner and origin boundary are in [Bootstrap cookie and origin policy](bootstrap-origin-policy.md). The crate ownership boundary is in [Subsystem crate architecture](subsystem-architecture.md). The roadmap names G04's device/auth scope and requires bounds from representative devices/load ([Implementation roadmap](implementation-roadmap.md#prerequisite-decisions)). Required phone and remote-device evidence and the distinction between measured campaign capacity and seat caps are in [Remote play](remote-play.md#gates-observability-and-acceptance) and [Expansion boundaries](expansion-boundaries.md#connectivity-presence-and-degraded-play).

This source revision contains the existing six-crate Rust workspace ([workspace manifest](../Cargo.toml)) and shared Rust/WASM contract foundation ([shared contract waves](shared-contract-waves.md)). Production `df-auth` and the selected device/role admission implementation are absent. The change is a design document with a standalone Rust contract example: native/WASM workspace checks were unperformed for this design review, while the example's standalone checks are source/build-bound as recorded below. The selected browser families, role matrix, and resource-admission rule are policy decisions, not verified compatibility claims. Required unresolved gates include: exact browser/version floor; real iOS/iPadOS and Android device smoke coverage (including sleep/network/reconnect); desktop browser smoke coverage; G02 browser transport feasibility; G03 shared typed contracts; credential persistence and rotation; actual capacity measurement across device/load mixes; per-session/service resource budgets; phone/tablet input and layout acceptance; codecs/capture constraints and G08 audio/memory/frame budgets; external deployment/TLS/proxy details. Smart-TV direct browsing remains out of target pending a separately reviewed scope decision. No native/WASM workspace build, browser, phone, tablet, provider, deployment, audio, or vision qualification was performed for this design decision.

## Alternatives and unresolved facts

- **Native-only client or wrapper:** rejected for this target because it adds another distribution and runtime contract rather than serving the agreed browser clients.
- **Direct smart-TV browser as a supported client:** rejected for this initial matrix due to absent requirement and compatibility evidence; attached display output remains supported as a display surface.
- **One vague “modern browser” target:** rejected because it fails to name the requested phone/tablet and display matrix.
- **Fixed seat cap:** rejected because no measured evidence supports a participant count; capacity is enforced from measured resource budgets instead.
- **Unlimited-capacity claim:** rejected because service, stream, projection, relay, and recipient delivery consume finite resources.

Exact version support and the real measured budgets remain unresolved; do not treat example values as production limits. This decision adds no production APIs and does not claim game runtime behavior.

## Original acceptance and verification status

Original acceptance criteria (verbatim):

1. `no arbitrary seat cap`
2. `The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.`

Original planned verification (verbatim):

1. `Freeze the cited source decision and a bounded contract example; compare no arbitrary seat cap. Retain decision, alternatives and unresolved facts.`
2. `Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.`

The literal source is retained at `development/evidence/fanout-20261001/wave-02/B-G04-D02/contract/g04-d02-contract.rs` (SHA-256 `fe63c7deeb6c4613d533eafce270e00cc76c0154b719dba2708f34ec2a21cc2b`). Pinned Rust 1.98.1 (`rustc --edition=2024 -D warnings ...`) compiled it successfully; execution printed `device role matrix example: 8 assertions plus budget loop passed`. Exact guarded commands, stdout/stderr, resource samples, exit statuses, and binary identity are retained in that `contract/` directory. The initial strict compile failed on an unconstructed `TabletIos` case; its immutable receipt is retained, and the correction added an explicit tablet assertion before the passing rerun. The compiled binary SHA-256 is `95265058ec4899480556a52c9020d408f5e4cb8d033a805140afddbdf73f4492`. The decision source hash, governing-input hash manifest, and commit are recorded in `handoff.json`.

These finite assertions establish only the literal decision contract. Device/browser compatibility and integrated capacity remain unperformed. Independent frontier evaluation and coordinator integration are required; this document does not declare the task complete.
