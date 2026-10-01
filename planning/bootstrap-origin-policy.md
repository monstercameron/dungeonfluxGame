# Bootstrap cookie and origin policy

Date: 2026-09-30
Task/attempt: `B-G04-D03/a1`
Input revision: `4ba37943691f3a01a49b8238983e82527bec91bf`
Owner: `df-auth` (browser admission); `df-rpc-bridge` (tunnel transport); deployment/proxy owner (external-origin configuration)
Status: Decision and bounded contract example; production auth, deployment, and parser integration remain pending.

## Decision

For browser bootstrap, serve the page and WASM from the deployment's configured public HTTPS origin and admit its WebSocket tunnel only over WSS at that same origin. Compare the browser's single serialized `Origin` against the configured canonical external origin, including scheme, host, and effective port. A production implementation must use the selected, reviewed URI/origin parser and compare normalized origin components; it must not use substring, suffix, wildcard, reflected `Access-Control-Allow-Origin`, or an untrusted `Host`/`Forwarded` value as an allow rule. Reject an absent Origin, `Origin: null`, duplicate Origin fields, malformed origins, and any scheme/host/port mismatch. Reject an insecure external transport for the browser tunnel. Never derive a permissive origin from the request itself.

The browser's initial credential bootstrap uses a host-only cookie scoped to the configured public host, with `Secure`, `HttpOnly`, and an explicitly selected restrictive `SameSite` policy. Do not set `Domain` to widen the cookie to sibling hosts. Do not put a long-lived credential in a URL, query string, fragment, trace field, or client-selected role. The exact cookie path, lifetime, rotation overlap, SameSite setting, and whether the bootstrap cookie is exchanged for a narrower RPC credential are deployment/G04/G03 decisions that must be frozen with the concrete credential and route contracts. The cookie admits a browser handshake; it does not identify a principal by itself or authorize arbitrary RPCs.

A short-lived, single-use admission ticket is a possible browser fallback only if the selected browser transport cannot use the cookie contract and G02/G04 explicitly select and test its delivery and redaction. It is not the selected default. Its binding, replay window, and non-URL delivery mechanism remain unselected; it must not become a long-lived URL bearer secret. Native clients use the separately configured native admission mechanism. Native admission does not receive a browser-origin exemption that creates a public bypass, and absence of a browser Origin is not proof of native identity.

## Authorization and CSRF boundaries

`Origin` is a browser cross-site request/CSRF signal at admission, not a credential, principal, permission, or identity proof. A non-browser caller can forge it. After tunnel admission, authenticate the credential and authorize each RPC against the current principal, target session, operation, and permission. Do not capture authorization permanently at stream creation: revalidate at the documented publication, delivery, lease, and expiry boundaries, and fence/revoke active streams and bindings as specified by the identity/session contracts. Trace context and correlation IDs never grant access.

Cookie-authenticated state-changing HTTP/customer routes also require an explicit CSRF/state defense (including origin validation against the same configured origin and the route's selected anti-CSRF mechanism). Redirect return URLs and CORS policy are not authorization. Do not reflect an arbitrary Origin in CORS headers or let permissive CORS substitute for server authorization. Keep public anonymous bootstrap operations narrow, rate-limited, and deduplicated; a successful origin check alone grants no guest identity, seat, host capability, or operator access.

When TLS terminates at a reverse proxy, the application may use forwarded scheme/host information only from an explicitly trusted proxy path whose ingress overwrites those fields. The deployment supplies one canonical public HTTPS origin out of band. Untrusted clients cannot select it by injecting forwarding headers. The deployment must verify that external WSS maps to the intended tunnel route and that redirects/cookies remain on the configured host.

## Owner matrix and integration boundary

| Concern | Owner | Contract/output for this decision | Still required before production |
| --- | --- | --- | --- |
| Credential/bootstrap admission, cookie issuance/lookup/revoke, principal and permission checks | `df-auth` | Enforce exact configured browser origin policy before browser-cookie bootstrap; authenticate and authorize RPCs independently; current authorization/revocation remains effective | Concrete credential store, bootstrap operation/idempotency/expiry, cookie attributes and rotation, typed errors and stream-revocation integration under G03/G04 |
| WebSocket and RPC tunnel | `df-rpc-bridge` | Preserve browser/native transport separation; reject browser-origin failures before tunnel admission; transport admission is not RPC authorization | Concrete selected parser/header multiplicity access, browser driver and HTTP/WSS integration under G02/G03 |
| HTTP, TLS termination, public route, trusted proxy and origin configuration | Deployment/composition owner (`df-server` when selected) | Configure one external HTTPS origin, expose matching WSS tunnel, and provide trusted proxy provenance without reflecting request values | Actual hosting topology, TLS/proxy configuration and deployment verification are unselected |
| Browser and native clients | Client owner | Browser uses supported cookie handshake; native uses its configured native admission; neither role selection nor trace metadata grants privileges | Supported browser/device matrix and credential persistence/recovery contract remain G04/G08 decisions |
| CSRF protections on cookie-authenticated HTTP mutations and redirects | `df-auth` with HTTP route/composition owner | Treat Origin as CSRF input and enforce route authorization/state checks | Concrete HTTP mutation inventory, anti-CSRF token/state mechanism, redirect and external identity flows remain unselected |

The boundary is intentionally limited to the named bootstrap-origin outcome. It adds no public type, RPC, endpoint, schema, crate, provider, or cookie implementation. Cookie admission is only the browser credential bootstrap selection; the `df-auth` owner must still define how the cookie is minted, rotated, revoked, mapped to credentials, and checked with typed current authorization.

## Bounded native decision example

This dependency-free example freezes the rejection contract for review. Its explicit assumptions are that the expected origin is already a deployment-validated canonical HTTPS origin and that the sample input represents the browser's serialized Origin field values. It compares the complete string and checks policy revision/secure transport; it does **not** parse or normalize URLs, inspect HTTP header bytes, validate TLS/proxy provenance, issue/read cookies, authenticate a principal, authorize an RPC, implement CSRF, or demonstrate production enforcement. The production adapter must replace the example's string assumptions with the selected URI parser and actual HTTP/WebSocket integration. The example is not a production URL parser or an authentication implementation.

```rust
#[derive(Debug, PartialEq)]
enum Reject {
    Disabled,
    StalePolicy,
    InsecureTransport,
    MissingOrigin,
    DuplicateOrigin,
    NullOrigin,
    OriginMismatch,
}

struct Policy {
    revision: u64,
    enabled: bool,
    canonical_https_origin: &'static str,
}

fn decide(
    policy: &Policy,
    request_policy_revision: u64,
    externally_secure_wss: bool,
    origins: &[&str],
) -> Result<(), Reject> {
    if !policy.enabled {
        return Err(Reject::Disabled);
    }
    if request_policy_revision != policy.revision {
        return Err(Reject::StalePolicy);
    }
    if !externally_secure_wss {
        return Err(Reject::InsecureTransport);
    }
    match origins {
        [] => Err(Reject::MissingOrigin),
        ["null"] => Err(Reject::NullOrigin),
        [_first, _second, ..] => Err(Reject::DuplicateOrigin),
        [origin] if *origin == policy.canonical_https_origin => Ok(()),
        [_] => Err(Reject::OriginMismatch),
    }
}

fn main() {
    let policy = Policy {
        revision: 7,
        enabled: true,
        canonical_https_origin: "https://play.example.test",
    };
    let check = |revision, secure, origins| decide(&policy, revision, secure, origins);

    assert_eq!(check(7, true, &["https://play.example.test"]), Ok(()));
    assert_eq!(
        check(7, true, &["http://play.example.test"]),
        Err(Reject::OriginMismatch)
    );
    assert_eq!(
        check(7, true, &["https://evil.example.test"]),
        Err(Reject::OriginMismatch)
    );
    assert_eq!(
        check(7, true, &["https://play.example.test:8443"]),
        Err(Reject::OriginMismatch)
    );
    assert_eq!(check(7, true, &["null"]), Err(Reject::NullOrigin));
    assert_eq!(check(7, true, &[]), Err(Reject::MissingOrigin));
    assert_eq!(
        check(
            7,
            true,
            &["https://play.example.test", "https://play.example.test"]
        ),
        Err(Reject::DuplicateOrigin)
    );
    assert_eq!(
        check(6, true, &["https://play.example.test"]),
        Err(Reject::StalePolicy)
    );
    assert_eq!(
        check(7, false, &["https://play.example.test"]),
        Err(Reject::InsecureTransport)
    );
    println!("origin decision example: 9 assertions passed (1 admit, 8 reject)");
}
```

The table summarizes the contract example's bounded cases; it is not a substitute for parser, framework, proxy, cookie, CSRF, or authorization fixtures.

| Input | Expected decision |
| --- | --- |
| One exact configured HTTPS origin, current policy, external WSS | Admit to browser-bootstrap boundary only |
| Wrong `http` scheme, different host, or different explicit port | Reject origin mismatch |
| `Origin: null` | Reject opaque origin |
| Missing Origin | Reject browser bootstrap |
| More than one Origin value | Reject ambiguous/duplicate header |
| Old policy revision | Reject stale policy |
| Non-WSS/insecure external transport | Reject browser tunnel |

## Current source and unresolved facts

At input revision `4ba37943691f3a01a49b8238983e82527bec91bf`, the planned `df-auth` crate does not exist in `crates/`; there is no production cookie bootstrap, public-origin configuration, or auth enforcement implementation to inspect or claim. `planning/subsystem-interfaces.md` assigns authentication, authorization and credential issue/lookup/revoke to `df-auth`, and requires current authorization to fence stream publication/delivery. `planning/rpc-api.md` specifies browser origin admission with a secure cookie or short-lived ticket, separate native admission, narrow anonymous bootstrap, and mandatory per-RPC authorization, while leaving credential bootstrap/persistence and browser ability to set WebSocket headers gated. `planning/rpc-transport.md` requires browser-compatible auth and origin checks and WSS deployment but keeps concrete library/runtime selection under G02. `planning/service-operations.md` calls for an origin allowlist and Secure HttpOnly same-site cookie or admission ticket, plus CSRF/state protection for customer mutations and redirects.

The only current code match is the S00 synthetic transport fixture in `crates/df-tools/src/fixture.rs`: its WebSocket and synthetic report handlers compare one `Origin` header value with a configured loopback origin and reject a mismatch. The fixture configures `http://127.0.0.1:<port>` for local synthetic use. That narrow fixture check is not the production origin policy: it does not establish HTTPS/WSS deployment, external-origin normalization, duplicate-header behavior, cookies, CSRF, identity, current RPC authorization, trusted-proxy behavior, or production endpoint integration. No `df-auth` canonical implementation was available to reuse; this artifact does not introduce a competing one.

Deployment/parser/provider facts still open: public hostname and effective-port policy; TLS termination and trusted-proxy topology; selected production URI/origin parser and duplicate-header semantics; final cookie path, lifetime, SameSite and rotation choices; concrete `df-auth` credential/cookie storage and bootstrap exchange; whether the bounded ticket alternative is needed and how it is delivered without URL secrets; native-client admission mechanism; selected HTTP mutation/redirect CSRF mechanism; supported browser/device behavior; and G02/G03/G04 integration contracts. No browser, provider, deployment, or real application check was run by this bounded contract proof.

## Verification and acceptance status

The source/build-bound evidence for the example is retained at `development/evidence/parallel-decisions/B-G04-D03/a1/worker/`. Exact commands, compiler and binary identities, source hash, output, and exit status are in `example-verification.json`. The proof establishes only that this standalone Rust policy-decision example compiles and exercises its listed cases using the cached pinned compiler. It does not satisfy independent frontier review or integrated MAIN acceptance.

Original acceptance criteria (preserved verbatim):

1. `same-origin HTTPS WSS`
2. `The named outcome has actual source/build-bound evidence; unsupported, pending, failed and unperformed checks remain explicit.`

Original verification procedures (preserved verbatim):

1. `Freeze the cited source decision and a bounded contract example; compare same-origin HTTPS WSS. Retain decision, alternatives and unresolved facts.`
2. `Exact executable commands: TBD at G01 and scoped prerequisite resolution; this planned procedure is not a claim that Rust/browser/provider checks ran.`

Verification status: the bounded example was compiled and run; all other production/deployment/browser/provider/auth checks are **unperformed** pending their named gates. Independent frontier review of this exact boundary and the coordinator's integrated source acceptance remain required before the task can be marked done.
