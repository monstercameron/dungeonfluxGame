# Public and operator protobuf namespace reservation

Task: B-C-df-protocol-D01; attempt: B-C-df-protocol-D01-a1.
Input revision: `d870db00dc349425e8f977bdfb20c6ffb24bfd3e`.
Status: bounded source decision; generated game-service schemas and runtime registration remain G03 work.

The original acceptance remains: private operator fields never enter the public
service set, and the named outcome has source/build-bound evidence with unsupported,
pending, failed and unperformed checks stated explicitly. This decision reserves
the boundary now; the executable example below checks its valid and refusal cases
without admitting production protobuf declarations or claiming running API privacy.

## Source decision and ownership

Follow [RPC API: Protocol and access, Service surface, Debug, telemetry and
compatibility](rpc-api.md), [Subsystem architecture: Shared and transport crates,
Server crates, Browser crates and Tooling crates](subsystem-architecture.md),
[Subsystem interfaces: Common contract rules, Projection, RPC and transport and
Observability and development tooling](subsystem-interfaces.md), and
[RPC transport](rpc-transport.md). The frozen brief supplies exact source hashes.
[Coding style](coding-style.md), root `rustfmt.toml`, and ADRs 0001–0005 govern
source scope, resource admission, independent review, evidence and scoped devlog.

| Reserved package | Service boundary | Owner and access |
| --- | --- | --- |
| `dungeonflux.public.v1` | IdentityService, CustomerService, SessionService, ActionService, VoiceService, AudioService, AssetService, JournalService, ClientService, HostService | df-protocol owns schemas; df-api maps authorized public requests/views; df-server registers PublicServiceSet |
| `dungeonflux.admin.v1` | DebugService | df-protocol owns schema; df-api owns restricted debug handling; df-server supplies separately restricted listener/routes |
| `dungeonflux.telemetry.v1` | TelemetryService | df-protocol owns schema; df-telemetry owns operator query/timeline/health/pin/export; df-auth separately authorizes operators |

The public set is an explicit allowlist of these ten existing services, including
CustomerService. Sharing a crate, generated build or backend does not authorize a
service on the public listener. DebugService and TelemetryService never enter that
set, even for a host-capable binding. Anonymous public admission separately allows
only configured, bounded bootstrap/join or customer challenge operations; namespace
membership does not make every public method anonymously accessible.

Every public method's request and response descriptor closure must remain public.
Check nested messages, repeated/map value messages and every oneof alternative,
including inactive alternatives; an operator reference inside a public wrapper is
still refused. A scalar/private field cannot be made safe by moving its declaration
to the public package: only the owning G03/API wave's reviewed audience-safe field
inventory may enter a public message. Unknown or unreviewed descriptors fail closed.
No generic bytes/JSON/Any payload, raw state, prompt, provider/admin secret, SQL
query, fixture or script field may tunnel operator material into public schemas.

HostPanelView/HostCommand retain only approved host controls and authorized status.
ForceDice, JumpPhase, ApplyFixture, faults, checkpoint restore and scoped director/job
inspection remain operator functions. ClientService.UploadTelemetry and
UploadDiagnostics stay public upload operations under their authorization/bounds;
they do not expose TelemetryService query/export or a private log corpus.
Public IngestReceipt conveys safe counts/reasons, durability stage, watermark/gaps,
not operator records. Authorization and audience filtering occur before serialization,
including previews, manifests, journal, audio and presentation references.
Correlation IDs never grant access; diagnostics use the existing df-observe path.

Admin schemas may reuse reviewed public receipts when needed; this creates no
public-to-operator dependency. Browser generation/exposure must omit operator
clients and descriptor material, while native operator tooling uses the separately
authorized set. Generated-code module/features and exact descriptor build mechanism
are not selected here. Package reservation alone does not prove runtime routing,
field projection or browser bundle exclusion.

## Alternatives and unresolved facts

A combined public/operator package with handler-only checks was rejected: it makes
accidental public registration and private message reuse harder to detect.
Hiding operator data in clients or granting all hosts operator access contradicts
the source privacy boundary. Separate new RPC services/crates and a custom envelope
are unnecessary; reuse the existing service owners and transport.

G03 still owns field/tag numbering, generated types, module/features, dependency
descriptor extraction, approved field inventories and compatibility fixtures.
Removed protobuf tags/names remain reserved; breaking changes need a new namespace
and announced compatibility window. This decision assigns no tags and does not
freeze D02's unknown/presence policy or D03's recovery policy. Actual df-api/df-server
registration, df-auth authorization and audience projections require their scoped
source work and integrated verification. No production helper or public Rust type
is introduced by the private executable fixture.

## Minimum next-consumer contract

The existing source boundary at the input revision is
`crates/df-protocol/proto/common.proto` (`dungeonflux.public.v1` identity/revision
messages), `contract_fixture.proto` and `transport_fixture.proto` (explicitly
experimental packages), `build.rs` and `src/lib.rs` (the actual
`FILE_DESCRIPTOR_SET`), plus `crates/df-tools/tests/schema_ledger.rs`
(descriptor/field-ledger comparison) and `shared_contracts.rs` (mapping fixtures).
No public game, admin or telemetry service is declared by these current schemas.
The experimental TransportFixture remains outside the production public allowlist;
its generated four-mode experiment does not reserve another production service.
Reuse this descriptor/generation and ledger boundary, rather than a second schema
scanner, new namespace metadata type or replacement shared identity model.

The next consumer is the canonical df-protocol G03 schema/generation owner, with
the df-tools schema-ledger test owner as its existing descriptor consumer. Before
admitting public generated source, it must enumerate the ten public services above,
derive the full request/response field dependency closure from the actual candidate
descriptors and compare each field to the reviewed public inventory. Reject a
privileged service, privileged reachable type/field, unresolved reference or generic
operator tunnel. Preserve exact schemas/descriptors, generator/tool/config hashes
and passing/refused fixtures. The minimum next bounded source
step extends `crates/df-tools/tests/schema_ledger.rs` to consume the existing
`df_protocol::FILE_DESCRIPTOR_SET` and check reserved package membership and
public reachability. Later admitted game schemas update the canonical
`crates/df-protocol/proto/`, `build.rs`, `src/lib.rs` and field ledger together
under their own G03 brief; this task edits only this decision file. df-api's PublicServiceSet and
AdminServiceSet, df-server listener wiring and df-telemetry's restricted service are
separate owner connections, not completed here.

The literal below is a private descriptor-closure decision fixture. Each namespace
edge represents a reviewed field/type reference, including scalar classification
and nested/oneof/map/repeated references. The consumer must derive these edges from
real reviewed descriptors; this hand-written finite graph does not inspect protobuf,
test a serializer or prove that a public scalar was correctly classified.
It uses no production DTO substitutes. A public HostService/ClientService shape is
accepted; direct admin/telemetry registration and an indirect operator field are
refused. A malformed reference is refused rather than skipped.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Namespace {
    Public,
    Admin,
    Telemetry,
}

struct Descriptor {
    namespace: Namespace,
    fields: &'static [usize],
}

#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    OperatorService,
    OperatorField,
    UnresolvedReference,
}

fn public_closure(
    service: Namespace,
    roots: &[usize],
    descriptors: &[Descriptor],
) -> Result<(), Refusal> {
    if service != Namespace::Public {
        return Err(Refusal::OperatorService);
    }
    let mut pending = roots.to_vec();
    let mut visited = vec![false; descriptors.len()];
    while let Some(reference) = pending.pop() {
        let descriptor = descriptors
            .get(reference)
            .ok_or(Refusal::UnresolvedReference)?;
        if descriptor.namespace != Namespace::Public {
            return Err(Refusal::OperatorField);
        }
        let seen = visited
            .get_mut(reference)
            .ok_or(Refusal::UnresolvedReference)?;
        if !*seen {
            *seen = true;
            pending.extend_from_slice(descriptor.fields);
        }
    }
    Ok(())
}

fn main() {
    // Finite reviewed fixture: public receipt, wrapper, admin field, telemetry field.
    let descriptors = [
        Descriptor {
            namespace: Namespace::Public,
            fields: &[],
        },
        Descriptor {
            namespace: Namespace::Public,
            fields: &[0],
        },
        Descriptor {
            namespace: Namespace::Admin,
            fields: &[],
        },
        Descriptor {
            namespace: Namespace::Telemetry,
            fields: &[],
        },
        Descriptor {
            namespace: Namespace::Public,
            fields: &[1, 2],
        },
        Descriptor {
            namespace: Namespace::Public,
            fields: &[3],
        },
    ];
    assert_eq!(
        public_closure(Namespace::Public, &[1], &descriptors),
        Ok(())
    );
    for operator in [Namespace::Admin, Namespace::Telemetry] {
        assert_eq!(
            public_closure(operator, &[0], &descriptors),
            Err(Refusal::OperatorService)
        );
    }
    for root in [2, 3, 4, 5] {
        assert_eq!(
            public_closure(Namespace::Public, &[root], &descriptors),
            Err(Refusal::OperatorField)
        );
    }
    assert_eq!(
        public_closure(Namespace::Public, &[6], &descriptors),
        Err(Refusal::UnresolvedReference)
    );
    println!(
        "valid public closure accepted; operator services/fields and unresolved reference refused"
    );
}
```

## Bounded verification and limitations

Extract this exact single Rust literal, compare it byte-for-byte with the retained
source, format/check it using pinned Rust 1.98.1 and root `rustfmt.toml`, compile
with `rustc --edition 2024 -D warnings`, then execute the finite assertions under
the frozen v6 guard after explicit root release. Retain actual argv, outcomes,
source/tool/config/binary hashes and any failed versions in attempt evidence.
No Cargo, dependency installation, production qualification or unperformed pass
is implied. Resource HOLD/cap/deadline stops without retry or larger bounds.

The immutable attempt handoff records actual performed checks and unperformed
checks against the committed document. Runtime service reachability, real protobuf
descriptor/field closure and compatibility, public serialization, operator
authorization, WASM generation/bundle exclusion, browser/device/transport and OTEL
pipeline checks remain unperformed by this bounded example. Independent review and
integrated source verification remain coordinator gates.
