# Public and operator service-set registration

Status: proposed `df-api` boundary policy; production server and generated services remain unimplemented.

## Decision

Keep service registration as two closed sets in `df-api`: `PublicServiceSet` and `AdminServiceSet`. The public set contains the ten game/customer services in `planning/rpc-api.md`; the admin set contains `DebugService`. `TelemetryService` remains owned and registered by `df-telemetry`, with `df-server` composing it onto the same restricted operator listener through a separate owner-supplied registration hook. This is a registration boundary, not a claim that service names alone authorize calls.

The `df-server` composition root creates distinct public and operator listeners and passes only the matching set to each listener builder. Startup must fail closed if a privileged registration is offered to the public listener, if either set is mixed onto one listener, or if the operator listener lacks its independent operator authentication policy. Public admission and RPC authorization remain distinct: an admitted guest can invoke only explicitly allowed bootstrap methods, and every protected method reauthorizes its trusted principal and scope. Host capability remains separate from operator status. Trace context never authorizes.

`df-api` owns typed handler-to-session mapping, authorized audience projection, and the public/admin set declarations. `df-auth` owns principal/capability decisions; `df-telemetry` owns telemetry query handlers; `df-protocol` owns generated schemas and compatibility fixtures; `df-server` owns listener construction, registration wiring, readiness, and the operator auth configuration. The native HTTPS payment-webhook ingress is a separate `df-server` endpoint with its own signature verification and durable admission path; it does not enter either game RPC set.

The sets are logical service groupings inside one backend process, not microservices. Do not split listeners into per-service processes or expose raw database access. Any later grouping change requires preserving method authorization, ownership, compatibility, and failure semantics. No protobuf field numbers, generated code, production type names beyond the already planned set names, or actual listener implementation are frozen here.

## Registration and refusal contract

At composition time, registration rejects a service whose owner does not match the set, an admin-only service on the public surface, a mixed set/listener, and an operator surface lacking its independent authentication policy. Such configuration errors prevent readiness; they are not converted into a partially available public API. At request time, invalid/malformed input maps to safe transport faults; authenticated but unauthorized requests map to `PERMISSION_DENIED`; domain-level rejected actions remain typed outcomes in successful RPC responses. A missing object uses nonexistence-safe public codes where lookup could reveal another tenant's row. Capacity and unavailable dependencies return bounded classified faults. The server never silently drops an admin registration or reports ready with a required service missing.

The following std-only fixture models this composition decision and its meaningful refusal cases. It is an executable policy example, not the planned production API, protobuf registry, authentication implementation, or running server.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Surface {
    Public,
    Operator,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Service {
    Identity,
    Session,
    Debug,
    Telemetry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Listener {
    surface: Surface,
    operator_auth: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RegistrationError {
    WrongSurface,
    MissingOperatorAuth,
}

fn register(listener: Listener, service: Service) -> Result<(), RegistrationError> {
    let is_operator_service = matches!(service, Service::Debug | Service::Telemetry);
    match (listener.surface, is_operator_service) {
        (Surface::Public, true) | (Surface::Operator, false) => {
            Err(RegistrationError::WrongSurface)
        }
        (Surface::Operator, true) if !listener.operator_auth => {
            Err(RegistrationError::MissingOperatorAuth)
        }
        _ => Ok(()),
    }
}

fn main() {
    let public = Listener {
        surface: Surface::Public,
        operator_auth: false,
    };
    let operator = Listener {
        surface: Surface::Operator,
        operator_auth: true,
    };
    let operator_without_auth = Listener {
        surface: Surface::Operator,
        operator_auth: false,
    };

    assert_eq!(register(public, Service::Identity), Ok(()));
    assert_eq!(
        register(public, Service::Debug),
        Err(RegistrationError::WrongSurface)
    );
    assert_eq!(
        register(public, Service::Telemetry),
        Err(RegistrationError::WrongSurface)
    );
    assert_eq!(register(operator, Service::Debug), Ok(()));
    assert_eq!(register(operator, Service::Telemetry), Ok(()));
    assert_eq!(
        register(operator, Service::Session),
        Err(RegistrationError::WrongSurface)
    );
    assert_eq!(
        register(operator_without_auth, Service::Debug),
        Err(RegistrationError::MissingOperatorAuth),
    );
}
```

## Alternatives and tradeoffs

- One registry with route-level annotations was rejected for the initial boundary: an accidental public bootstrap registration would depend on every route annotation being correct. Separate sets make the privileged registration decision visible at composition.
- One process/listener with path prefixes was rejected as the default because separate listeners better express the required independent operator access policy and reduce accidental exposure. The actual transport may use a distinct restricted route only after equivalent isolation and startup checks are demonstrated; that is an unresolved deployment decision.
- Duplicating telemetry handlers inside `df-api` was rejected because it would move ownership away from `df-telemetry` and risk a reverse tooling/API dependency.
- A separate microservice per set was rejected because the architecture specifies one backend process initially; process separation is not required to isolate registration and would add deployment assumptions.

## Source basis and open gates

This decision follows [Subsystem architecture](subsystem-architecture.md) (`df-api` owns generated handlers and the named sets; `df-server` owns listeners/wiring) and [Subsystem interfaces](subsystem-interfaces.md) (public projection versus restricted admin services; telemetry query owned by `df-telemetry`; never expose raw DB). [RPC API](rpc-api.md) supplies the eleven public services, `DebugService`, and separately owned `TelemetryService`, requires independent operator authorization, and leaves G03 numbering and public bootstrap registration to its shared owner. [RPC transport](rpc-transport.md) leaves browser transport feasibility as a prerequisite; this task makes no browser/runtime claim. [Service operations](service-operations.md) requires separate operator authentication with MFA, time-limited scoped roles and reason/ticket, and redacted default support views. [Protobuf allocation policy](protobuf-allocation-policy.md) keeps service/method compatibility under the single schema owner. [Commerce service](commerce-service.md) and the RPC API keep customer lifecycle separate from game authority; signed native payment webhook ingress remains outside game RPC.

Production gates remain open: G03 must freeze service/method numbering, generation, compatibility rules, and concrete bootstrap registration; the transport owner must establish supported native and browser RPC paths; `df-auth` must supply the concrete operator principal/role and revocation contract; `df-server` must implement distinct listener wiring and fail-closed readiness; `df-telemetry` must supply its owned registration hook; deployment must select and qualify listener exposure, TLS, MFA, rate limits, and operator audit handling. No application service or server currently exists, so this policy does not prove production isolation. The original acceptance, “privileged method registration is isolated,” is a design contract with an executable finite fixture here; source/build-bound production evidence and independent review remain pending.

## Verification limits

The example is intended for extraction as-is, formatting with the repository `rustfmt.toml`, compilation using the pinned Rust 1.98.1 edition 2024 toolchain with `-D warnings`, and execution of its finite assertions through the wave's guarded command runner. Results, exact argument vectors, source hashes, and process receipts belong in this task's evidence root and handoff. This is not a Cargo, native application, WASM, browser, authentication, or production listener check. Those checks are unperformed because this attempt changes a policy document only and the relevant implementation prerequisites are not present. Independent boundary evaluation and root integration remain mandatory.
