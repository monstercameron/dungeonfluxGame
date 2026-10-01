# G10-D01: Private authoring pack upload boundary

Status: Design contract; production types and enforcement pending G03/G05/G07/G10.

## Decision

The authoring intake accepts one bounded byte sequence for a private draft. The
payload is exactly the uploaded bytes: no client-supplied parsed objects, extracted
lore, archive members, remote URL, or inferred content are admitted at this
boundary. The illustrative contract below fixes a maximum of 8 MiB (8,388,608
bytes) per upload and rejects an empty upload. G05/G10 must validate this bound
against the selected deployment and authoring limits before production use.

The intake is transport-facing. An authenticated, authorized creator and private
draft scope must already be established by the owning admin/tool boundary; neither
the bytes nor correlation metadata grant identity, access, or publication rights.
The accepted bytes remain private to that draft and are not discoverable in public
manifests. The intake returns only an owned copy of the exact payload or a typed
size rejection. It does not parse a format, trust a filename or media type, extract
an archive, fetch references, validate source/rights, or publish/activate a pack.

`df-tools` owns explicit import/author/package operations. Its later import step
applies the selected allowlisted format and structural limits and creates a draft
plus safe diagnostics. `df-content` owns immutable schema and content validation,
including source compatibility, rights, references, DAG closure and alternative
reachability. `df-model` owns any shared versioned records only after G03/G10
freezes them. Existing persistence/session owners store private draft bytes and
references, and publish or activate only authorized validated versions. No new
crate, admin RPC, codec, or second implementation is introduced here.

For integrity and retries, downstream storage computes identity from the exact
accepted byte sequence and deduplicates by operation and payload; a filename is
not identity. A digest is not authorization and must not expose private draft
existence. Private payload bytes and lore are excluded from default logs; native
consumers may emit safe classified outcomes and bounded counts. Authorization is
checked independently of trace/correlation IDs, before disclosure or projection.

## Alternatives and unresolved gates

- Accepting parsed authoring fields at intake was rejected because it blurs the
  transport boundary with format parsing and risks admitting derived content
  before provenance and source checks.
- Accepting URLs or automatically fetched references was rejected; only explicit
  uploaded bytes are in scope. Import references are resolved by a separately
  admitted, allowlisted authoring operation, never by this boundary.
- Parsing/extracting archives while receiving was rejected. It expands the
  resource and traversal attack surface before format, depth, item, and rights
  limits are applied.
- The 8 MiB cap is a concrete contract-example bound, not a calibrated product or
  service target. G05/G10 must select and test production byte, item, depth,
  storage, and diagnostic limits. G03/G07/G10 must freeze schema, format,
  catalog/source and rights compatibility. X10/G03 must select the authorized
  creator workflow and publication interface. None is evidence of implementation.

## Literal std-only contract example

This standalone example models only the intake boundary. Its private wrapper
cannot be constructed from unbounded input, and the refusal cases prove the byte
cap and empty-input behavior. The accepted case proves byte-for-byte preservation;
no parser or external dependency is implied.

```rust
const MAX_UPLOAD_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
enum UploadError {
    Empty,
    TooLarge { actual: usize, maximum: usize },
}

#[derive(Debug, PartialEq, Eq)]
struct PrivateUpload(Vec<u8>);

impl PrivateUpload {
    fn accept(bytes: &[u8]) -> Result<Self, UploadError> {
        if bytes.is_empty() {
            return Err(UploadError::Empty);
        }
        if bytes.len() > MAX_UPLOAD_BYTES {
            return Err(UploadError::TooLarge {
                actual: bytes.len(),
                maximum: MAX_UPLOAD_BYTES,
            });
        }
        Ok(Self(bytes.to_vec()))
    }

    fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

fn main() {
    let uploaded = [0x00, 0xff, b'{', b'}'];
    let accepted = PrivateUpload::accept(&uploaded).expect("small bytes are accepted");
    assert_eq!(accepted.as_bytes(), uploaded);

    assert_eq!(PrivateUpload::accept(&[]), Err(UploadError::Empty));

    let oversized = vec![0; MAX_UPLOAD_BYTES + 1];
    assert_eq!(
        PrivateUpload::accept(&oversized),
        Err(UploadError::TooLarge {
            actual: MAX_UPLOAD_BYTES + 1,
            maximum: MAX_UPLOAD_BYTES,
        }),
    );
}
```

This example is a contract fixture, not a production API or an integrated
upload/storage path. It says nothing about HTTP framing, streaming allocation,
authentication implementation, encryption, durable publication, parsing,
source or rights validation, replay, or native/WASM application behavior.

## Source basis and acceptance

- `planning/campaign-authoring.md` (“Versioned authoring model”, “Pure validation
  and scoped publication”, “Delivery and acceptance”, “Personal payload and
  rights lifecycle”): private drafts, explicit allowlisted imports, bounded
  formats, df-tools authoring, df-content validation, and separate publication
  and activation.
- `planning/subsystem-interfaces.md` (“Pure domain and content”, “Separate runtime
  subsystem boundaries”, “Identity and session ownership”): single owners,
  immutable content validation, authorized audience boundaries and no duplicate
  persistence authority.
- `planning/implementation-roadmap.md` (“Prerequisite decisions”, “Gap-refinement
  integration”, “Runtime director delivery within existing slices”): G05/G07/G10
  gates and X10 authoring ownership; production authoring/replay remains pending.
- `planning/generated-content.md` (“Candidate, validation and admission”):
  generated content remains a candidate until approved, source-compatible,
  bounded admission.
- `planning/long-horizon-state.md` (“Personal payload and rights lifecycle”):
  private imports require explicit uploaded bytes; no arbitrary external import
  URL.
- `planning/campaign-cinematics.md` (“Recaps and trailers”): private/audience
  scope and rights remain enforced on downstream references and exports.
- `planning/runtime-directors.md` (“Authority and composition”, “Time, recovery
  and bounded work”): versioned authoring records, pure proposals, owner-committed
  state, and explicit unsupported-version/replay gaps.
- `planning/tempo-engine.md` (“Audience, preferences and observations”):
  audience filtering precedes serialization and hidden/private state cannot leak
  through presentation.
- `planning/coding-style.md` (“Names, modules, and public contracts”, “Functions,
  ownership, and errors”, “Persistence, RPC, and instrumentation”): typed local
  errors, private internals, bounded inputs, and no ad hoc logging.

Acceptance for this design item is the explicit uploaded-bytes-only contract,
alternatives and unresolved gates above, plus a literal example covering accepted
and refused inputs. The task brief requires extracting this literal example,
checking it with the repository-pinned formatter and compiling/executing the finite
assertions after separate compiler admission. Those checks have not run yet; no
production implementation, build, parser, or integrated behavior is claimed.
