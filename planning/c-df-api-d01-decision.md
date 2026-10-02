# Exhaustive authorized wire projection

**Task:** B-C-df-api-D01, attempt B-C-df-api-D01-a1  
**Decision status:** policy proposal for the df-api G03 contract; no production API or schema is frozen  
**Source revision:** dispatch input `308920e328ba6df85bea1e3d2abcbea5f1b796e6`

## Decision

`df-api` must construct each public response from an explicitly enumerated, audience-authorized projection. A private canonical/domain/persisted object is never itself a wire DTO, never derives or receives a general-purpose public serializer, and is never handed to a generated encoder. The mapper first receives a detached read model and a trusted `AuthorizedAudience` established by `df-auth`/`df-session`; it then constructs the exact public message type. Request fields, client role hints, session IDs, trace metadata, a host capability, or possession of a canonical object do not grant or widen that audience. Trace context remains correlation only.

The projection boundary is **allowlist by construction**: every field of every outbound public message must appear in a source-reviewed mapping table and be assigned explicitly by a mapper. The mapper does not copy, flatten, reflect, stringify, or serialize the canonical object. New canonical fields are therefore absent from output until a reviewed mapping deliberately adds them. New public fields require a protocol-owner allocation and compatibility review; a DTO/schema change is not implicitly authorized by adding a domain field. Public, admin, and telemetry messages remain separate namespaces and service sets. Operator-only data never passes through a public message type, and telemetry-query ownership remains with `df-telemetry`.

This policy resolves the requested privacy outcome without pretending that the planned domain structures or concrete generated messages exist. `planning/rpc-api.md` and `planning/subsystem-interfaces.md` require player/display/host views, filtered before serialization; the API and protobuf allocation decisions remain G03 work. The policy example below tests a finite decision boundary, not real protobuf bytes, a production mapper, or integrated behavior.

## Projection rules

1. **Authorize before reading or projecting.** Authenticate and authorize the caller for the resource and requested operation. Derive the scope on the server. An untrusted request may identify an operation or resource under its method contract but cannot choose another member's audience. Re-check authorization when reading or transferring private records and assets, and throughout long-lived streams.
2. **Project a detached snapshot.** `df-api` maps a detached authorized read model into `df-protocol` public DTOs. Do not pass mutable session state, persistence documents, provider records, credentials, or raw engine structures to a wire encoder. A public projection contains only the fields required by its named view and audience.
3. **Enumerate both inclusion and exclusion.** For each public RPC/view, its G03 mapping table must list: source fact, audience predicate, public DTO field, presence rule, and redaction/omission behavior. It must also name private canonical field families that must not appear, including credentials/tokens, other members' character/resource/private-dialogue data, undiscovered knowledge and NPC secrets, hidden threat stages/forecasts, internal beat or director scores, provider payloads, tenant/ledger internals, trace payloads, raw diagnostics, and private sound/asset variants. A table entry authorizes only its named destination and audience, not reuse in another view.
4. **Use closed typed views.** Public response types are generated, finite message structures, not generic JSON/event maps or arbitrary debug objects. The `PlayerView`, `DisplayView`, and `HostPanelView` have separately enumerated fields. Host permission permits only the configured host controls/status; it is not an omniscient-data bypass. Admin diagnostics use restricted admin messages and independent authorization. The telemetry query service is separate again.
5. **Treat shared side channels as projections.** Captions, journal facts, notices, action previews, asset manifests/preload references, tempo profiles, cue identities, and shared audio can reveal hidden state even when the main view omits it. Apply the same audience and disclosure policy before producing each of them. Shared output must not carry a private variation, another observer's belief graph, internal score, or future branch forecast.
6. **Fail closed on incomplete or stale authority.** Missing authorization, audience mismatch, stale binding/lease, revoked membership, or an unrecognized required projection capability yields no private payload. Return the method's safe permission/opaque-resource RPC failure or documented typed capability/recovery result; never substitute a broader/default audience or partial omniscient view. On stream revocation, stop affected publication/transfer, close or resync, and clear obsolete client buffers. Bytes already delivered cannot be recalled.
7. **Keep errors and diagnostics safe.** Public typed domain rejections may include only an authorized safe code, localized key, permitted correction and, where allowed, a visible revision. RPC status/trailers carry auth/framework/resource failures. Neither surface includes raw user text, secrets, provider responses, ledger details, trace payloads, or internal exceptions. `df-api` reports classified facts through shared observation conventions; correlation IDs never become authorization.
8. **Keep domain and wire ownership separate.** `df-model` owns canonical game/persisted types, `df-auth` owns principals/audiences, `df-session` owns authorized detached reads, `df-protocol` alone owns protobuf declarations/generated services/field ledger, and `df-api` owns request handling, projection and registration of `PublicServiceSet`/`AdminServiceSet`. `df-server` composes dependencies. No competing projection authority, duplicate production DTO family, schema ledger, or serialization framework on domain types is introduced.

The literal in `development/evidence/fanout-20261001/wave-02/B-C-df-api-D01/contract.rs` demonstrates the core rule with a complete finite pair of public projections and a canonical object containing adversarial private fields. It checks that each authorized audience produces only its explicit public field set, that a forged request target cannot widen a trusted scope, and that no listed private value is present in the emitted representation. Its tiny representation is not a protobuf encoding.

## Alternatives considered

- **Serialize canonical types and omit known secrets with annotations or deny-lists:** rejected. A new canonical field, nested object, serializer default, debug representation or provider extension can become public without a deliberate API mapping. The architecture explicitly keeps domain types free of serialization frameworks and says DTOs are explicitly mapped.
- **One universal view filtered by the browser:** rejected. Sending private data and asking a thin client to hide it violates the server-authoritative boundary and fails for shared displays, caches, logs and network inspection.
- **Use a generic JSON/event/debug envelope:** rejected. It bypasses closed public contracts, namespace separation, authorization-specific field review and protobuf allocation/compatibility controls.
- **Treat host as a universal private-data audience or rely on trace/session identifiers:** rejected. Host is a configured capability, separate from operator permission; correlation metadata never grants identity or audience access.
- **Freeze every feature DTO or protobuf number in this task:** rejected. G03 is the numbering/type gate, the shared-contract policy says later waves freeze only exact fields needed by consumers, and no generated production API is present. This decision sets a mapper invariant and ownership boundary only.

## Failure semantics and integration ownership

`df-auth` authenticates and grants the scoped `AuthorizedAudience`; `df-session` supplies the detached read model and fences binding/revocation; `df-api` checks method/resource scope and performs the exhaustive typed mapping; `df-protocol` owns generated messages, numbering, namespace compatibility and encoding; `df-server` registers the public/admin services with the proper dependencies/listeners. `df-assets` enforces access again on asset retrieval, while `df-media` and `df-presentation` supply only already-permitted plans/readiness. `df-telemetry` owns operator log-query service; `df-api` can use only the authorized narrow operator cost-inspection port described by subsystem architecture.

Authorization failure, stale or revoked binding, invalid request, capacity/deadline/cancellation, unavailable infrastructure and unexpected framework faults remain safe RPC status/trailer outcomes per `planning/rpc-api.md`; do not map them to invented successful responses or string-parsed domain codes. A well-formed domain rejection stays a typed domain outcome where the method defines one. Unknown or private resource existence may be hidden by the owning authorization policy. A missing required projection capability is an incompatibility/fallback decision as specified by that method, never a reason to serialize an internal representation. On uncertain mutation completion, preserve the operation ID/fingerprint and use that method's lookup contract; projection policy does not convert transport uncertainty to success or rejection.

The task's hook, “Define exhaustive authorized wire projection -> private canonical fields never serialize publicly,” is owned by `df-api`. Delivery here defines that rule and a finite contract fixture. It does not connect handlers, generated services, schemas, real session snapshots, the server composition root, or clients. Those connecting owners and concrete field lists belong in the G03/API implementation wave and feature acceptance.

## Bounded executable contract

This dependency-free Rust 2024 contract constructs two authorized wire representations from one canonical record and checks the allowed fields, private-field omission, and a request-target attack. The closed `WireView` and `WireField` matches are the complete serializer surface in this fixture. Actual API work must test every generated public DTO and audience, including late-field omission, and exercise real encode/decode at the integration boundary.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AuthorizedAudience {
    Player { member_id: u64 },
    Display,
}

#[derive(Debug)]
struct CanonicalRecord {
    member_id: u64,
    display_name: String,
    own_hit_points: u8,
    shared_scene: String,
    credential: String,
    other_member_secret: String,
    undiscovered_threat: String,
    provider_payload: String,
}

#[derive(Debug, PartialEq, Eq)]
enum WireField<'a> {
    PlayerName(&'a str),
    OwnHitPoints(u8),
    SharedScene(&'a str),
}

#[derive(Debug, PartialEq, Eq)]
enum WireView<'a> {
    Player(Vec<WireField<'a>>),
    Display(Vec<WireField<'a>>),
}

impl WireView<'_> {
    fn encode(&self) -> String {
        match self {
            Self::Player(fields) => format!("player:{fields:?}"),
            Self::Display(fields) => format!("display:{fields:?}"),
        }
    }
}

fn project<'a>(
    record: &'a CanonicalRecord,
    audience: AuthorizedAudience,
    requested_member_id: u64,
) -> Option<WireView<'a>> {
    match audience {
        AuthorizedAudience::Player { member_id }
            if member_id == record.member_id && requested_member_id == member_id =>
        {
            Some(WireView::Player(vec![
                WireField::PlayerName(&record.display_name),
                WireField::OwnHitPoints(record.own_hit_points),
            ]))
        }
        AuthorizedAudience::Display => Some(WireView::Display(vec![WireField::SharedScene(
            &record.shared_scene,
        )])),
        AuthorizedAudience::Player { .. } => None,
    }
}

fn main() {
    let record = CanonicalRecord {
        member_id: 7,
        display_name: "Ari".to_owned(),
        own_hit_points: 9,
        shared_scene: "Market square".to_owned(),
        credential: "secret-credential".to_owned(),
        other_member_secret: "private-dialogue".to_owned(),
        undiscovered_threat: "hidden-wraith".to_owned(),
        provider_payload: "unfiltered-provider-output".to_owned(),
    };

    let player = project(&record, AuthorizedAudience::Player { member_id: 7 }, 7)
        .expect("authorized member receives their explicit player projection");
    assert_eq!(
        player,
        WireView::Player(vec![
            WireField::PlayerName("Ari"),
            WireField::OwnHitPoints(9),
        ])
    );

    let display = project(&record, AuthorizedAudience::Display, 999)
        .expect("display audience receives its independent shared projection");
    assert_eq!(
        display,
        WireView::Display(vec![WireField::SharedScene("Market square")])
    );

    assert_eq!(
        project(&record, AuthorizedAudience::Player { member_id: 7 }, 8),
        None,
        "request target cannot widen the trusted member scope"
    );

    for encoded in [player.encode(), display.encode()] {
        for private_value in [
            &record.credential,
            &record.other_member_secret,
            &record.undiscovered_threat,
            &record.provider_payload,
        ] {
            assert!(
                !encoded.contains(private_value),
                "private canonical value appeared in public output"
            );
        }
    }
}
```

The fixture's strings and record values are finite test data, not numerical resource limits or claims about measured bounds. It relies only on `std`; it does not create a production Rust type, protobuf message, encoder, authentication system, or runtime behavior.

## Unresolved production gates

- G03 must name every generated request/response field, presence rule, source owner, audience predicate, and allocation; update the actual descriptor-derived `df-protocol` ledger and compatibility fixtures in the same reviewed boundary.
- The actual exhaustive mapper and compile-time/test strategy for catching a new DTO field require production types. The illustrative closed match cannot prove exhaustiveness over future generated fields.
- Auth/session integration must prove trusted audience provenance, resource binding, detached snapshot consistency, revocation fencing, and safe opaque denial against real handlers and streams.
- Every direct and indirect leak channel (including logs, captions, asset preloads, tempo/cue data, caches, exports, and private audio) needs consumer-specific boundary tests. Integration must cover stale/replaced binding, revocation during active streams, and old client buffers.
- Browser gRPC/WASM feasibility and native transport modes remain transport gates. This fixture says nothing about generated service registration, real bytes, cross-version behavior, device rendering, network properties, or audible output.
- There is no application workspace/build or running API boundary in this task's evidence. Native/WASM, Clippy, browser, provider, end-to-end and frontier running-output checks are unperformed; an independent evaluator and sequential root integration remain mandatory.

## Source basis

This decision follows [Subsystem architecture](subsystem-architecture.md) (crate owners, DTO mapping, thin clients), [Subsystem interfaces](subsystem-interfaces.md) (trusted audience provenance and typed exhaustive projection), [RPC API](rpc-api.md) (separate namespaces/service sets, filtered views, errors, stream fencing and G03 freeze), [RPC transport](rpc-transport.md) (transport feasibility and authority), [Protobuf allocation policy](protobuf-allocation-policy.md) (sole schema owner and explicit mappings), [shared-contract waves](shared-contract-waves.md) (G03/compatibility ownership), [remote play](remote-play.md) (filter before serialization/assets), [commerce service](commerce-service.md) (separate customer/tenant authority), [expansion boundaries](expansion-boundaries.md) (conditional features), [runtime reliability](runtime-reliability.md), [storage architecture](storage-architecture.md), [observability](observability.md), and [coding style](coding-style.md). Governing input hashes and command evidence are retained beside the worker handoff.
