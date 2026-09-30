# First shared contract wave

CONTRACT-G03-001 freezes the first S00/S01 common identity, paired build provenance,
and composite recovery revision contracts. Governing sources are
`planning/subsystem-interfaces.md` (Common contract rules, Pure domain and content,
Persistence and assets), `planning/subsystem-architecture.md` (Shared and transport
crates, Tooling and integration), `planning/rpc-api.md` (Protocol and access and
Recovery revision ordering), and `planning/service-operations.md` (Composite
revision after disaster restore). Mandatory style follows `planning/coding-style.md`.

`df-types` has no dependencies. Its five distinct identity types accept exactly
16 canonical bytes with at least one nonzero byte, preserve supplied byte order,
and neither generate values nor establish uniqueness, authentication, membership,
permissions, or credentials. No cross-ID conversions exist. They are identifiers,
not public issuance APIs.

`RecoveryEpoch` accepts a nonzero u64. `SessionRevision` contains that epoch and a
u64 in-epoch sequence; zero sequence is a valid initial state. Ordering is
lexicographic: (8, 0) follows (7, u64::MAX). Checked next-sequence refuses overflow
without issuing a new epoch. Authority to allocate a protected epoch and verify
nonregressing recovery heads remains in the future persistence/recovery wave.
Old scalar revisions cannot supply an epoch and are refused at the consuming
boundary. No clock, network, SQL, runtime, engine, or serde dependency enters types.
Local elapsed durations continue to use Rust Duration; this wave needs no additional
physical quantities. Protocol revision and sequence are dimensionless, explicitly
named wire values.

`BuildIdentity` describes a paired native/WASM fixture with five required labels:
source, native, WASM, configuration, content. Each label is 1..128 ASCII bytes;
only letters, digits, period, underscore, hyphen, colon and slash are accepted.
Missing, empty, oversized and invalid-character errors are distinct and identify
the affected build component. Errors retain no supplied label text. Callers must
supply nonsecret provenance. Validation cannot determine whether an allowed label
contains a credential, so these labels must never be populated from secrets.
Construction proves neither tested-build approval nor content rights/catalog validity.
Later runtime-specific build shapes require an explicit consumer contract refinement.

The additive `dungeonflux.public.v1` common schema uses optional scalar/bytes/string
presence, and message presence for composite values. Missing is distinct from zero
or an empty label; generated DTO construction is not validation. `df-protocol`
imports no project crates. `df-tools/tests/shared_contracts.rs` explicitly maps and
validates the actual generated DTOs, and runs actual prost encoding/decoding. The
mapping is a native/WASM-compatible consuming fixture, not a production API or
browser behavior change.

Compatibility wrappers in `dungeonflux.experimental.contract.v1` are test-only
experiments and declare no services. Their consuming boundary supports exactly
protocol revision 1, rejects missing/zero/unsupported revisions, and requires
explicit capability kind and requiredness. Enum zero is unspecified and rejected.
Unknown optional capabilities are skipped; unknown required capabilities reject
with a typed incompatibility. A missing/unknown-only oneof rejects, never creates
an implicit payload. Protobuf does not preserve unknown-field semantics: an unknown
field alongside a known payload is optional additive data, and cannot erase or
replace the known payload at this old consumer. A future *required* meaning must
use explicit required capability/revision incompatibility, never rely on old
consumers recognizing an unknown oneof tag. Malformed wire data rejects in decoding.

`crates/df-protocol/proto/field-ledger.txt` records all actual common and experimental
allocations, including the unchanged transport fixture: file/package, message,
field name/number/type/cardinality/presence/oneof, enums and values, oneofs, services
and methods. The actual protoc descriptor is emitted with generation and exposed
as FILE_DESCRIPTOR_SET. `df-tools/tests/schema_ledger.rs` compares that descriptor
to the durable ledger, rejects duplicate/reserved allocations and schema changes,
and exercises actual descriptor mutations for removed/renumbered/reused fields,
type/presence/name changes and future retirement reuse. No test-only schema clone
is used as the current baseline. There are no retired fields in this initial wave.
When a later coordinated change retires a field/enum value, retain its number/name
in protobuf `reserved` declarations and ledger reserved records permanently.
Changing the ledger is a reviewed contract change, not an automatic regeneration
that can bless incompatible edits. Message ranges exclude their end; enum reserved
ranges include their end, matching descriptor semantics.

The historical G01 audit remains frozen to four crates/source inputs. This wave
adds the fifth workspace crate and therefore must make its verifier refuse stale
Cargo inputs. Current native/WASM dependency inspection is retained with this wave;
it does not repin, update or extend the historical legal/license audit.

The existing experimental four-mode transport service, bridge, browser UI and
preview sources are unchanged. Native execution of the consuming contracts and
WASM cross-compilation are distinct evidence; cross-compilation does not prove
WASM execution or browser/audio/device qualification. Native/WASM fixture artifacts
are built into a separate owned root without replacing verified preview builds.

This first contract wave leaves full G03 models, effects, audiences, authorization,
services and production consuming paths pending. Full G02 physical device/network,
whole process/pre-callback allocation/heap/8MiB bounds, recovery and audible audio
gates remain pending, including the retained-tab reload defect. No result here
claims production gameplay or completion of S00/S01/the project.
