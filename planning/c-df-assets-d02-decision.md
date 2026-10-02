# df-assets D02: authorize every asset open and range read

Date: 2026-10-01

Task/attempt: `B-C-df-assets-D02-a1`

Status: policy decision and bounded contract example; production asset/auth
adapters and integrated access evidence remain pending

## Decision

`df-assets` must obtain a fresh access decision for every `open` and every
subsequent byte-range read. The trusted caller supplies a `Principal` and
`AuthorizedAudience` obtained from `df-auth`; neither an `OperationContext`, a
trace/correlation identifier, an asset ID, a hash, a previously authorized
reference, nor possession of cached bytes grants access. `df-assets` asks the
existing metadata/access owner for the current tenant, audience, asset version,
scope/access generation, suppression state, and reviewed `RightsGrant` for the
requested purpose. It authorizes the requested byte interval only when the
audience and current grant both permit that asset and purpose. Rights include
distribution and derivative/use constraints as applicable; metadata presence
or a valid content hash is not a grant.

An open result is a short-lived, opaque stream binding, not a reusable bearer
capability. Before returning manifest information or opening bytes, the boundary
checks current audience and rights. Before emitting each bounded byte chunk, it
checks them again against current state and verifies that the stream's asset
version, audience/access generation, suppression generation, purpose, and
requested interval still match. A rights revocation, audience removal, tenant
change, suppression advance, asset-version mismatch, or stale scope fences the
stream before the next chunk. Already delivered bytes cannot be recalled; the
contract prevents prospective disclosure and does not claim retroactive erasure.
Adapters that cannot observe a current decision fail closed for private access.

The range is a half-open interval `[start, end)` in the immutable version's
byte sequence. Require `start < end`, check `end <= manifest.bytes` without
overflow, and cap each emitted chunk by the currently approved service limit.
The proposed initial service limit is 64 KiB per asset/audio chunk in
`service-operations.md`; it is a planning value, not a measured safe maximum.
An invalid, empty, overflowing, or out-of-bounds range is a typed range
rejection before byte I/O. Do not silently clamp or return partial success.
The stream ends only after exactly the authorized interval has been emitted;
early storage EOF is an explicit incomplete/unavailable failure.

Failures distinguish invalid range, stale authorization/basis, rights or
audience denial, unavailable metadata/rights authority, and byte-store failure
internally. At public boundaries, a caller without authority must not learn
whether a foreign asset exists: map denied and absent references to the same
nonexistence-safe response where required by the transport contract. Do not
include private metadata or bytes in diagnostics. A failure after some chunks
terminates the stream with an explicit error; it is never reported as a complete
asset. Retrying requires a fresh authorization decision and a new open.

## Ownership and integration

This is one access policy in the existing `df-assets` boundary, not a second
authorization subsystem. `df-auth` owns trusted `Principal` and
`AuthorizedAudience` issuance and membership/role authorization. The session/API
caller passes those values separately from correlation context. `df-assets`
owns byte I/O, immutable asset references, stream checks, and coordination of
the current decision. `AssetMetadataStore`/`df-persistence` owns durable
metadata, tenant predicates, access-scope revisions, source rights and current
suppression/revocation facts. The metadata/access query must return the current
facts from the authoritative path, not a stale client cache or an earlier open.
`df-api`/`df-session` revalidate the audience at projection/delivery boundaries
and fence affected streams after membership or lease changes; client caches
clear obsolete buffers on notices but those notices are not enforcement.

Use the existing reviewed `RightsGrant` and erasure/suppression authority. Do
not add an asset-local rights registry or let a package's declaration self-grant
distribution. The caller names the intended use (for example, in-session
display, audio playback, or export); a grant for one use does not imply another.
Prefetch, prepared assets, replay, exports, and cached hits all pass through the
same current-access decision immediately before disclosure. Export consent and
capture/contributor rights remain separate prerequisites for export.

## Alternatives and rationale

* **Authorize only at resolve/open and trust the stream afterward:** rejected;
  a long stream can outlive membership, grant, or suppression state.
* **Treat an authorized reference, signed URL, asset ID, or content hash as
  sufficient:** rejected; identity and possession do not prove current audience
  or use rights, and a reusable URL bypasses revocation checks.
* **Authorize each byte independently:** rejected as needlessly chatty and
  difficult to bound. A bounded chunk is the disclosure unit, with a current
  decision immediately before each chunk; an adapter may batch only if it can
  preserve this observable revocation boundary and never release later chunks
  after a changed decision.
* **Check once per chunk and permit the chunk already in flight to finish:**
  selected. Revocation cannot retract bytes already emitted or a chunk already
  delivered to the transport, but it fences the next chunk and reports a
  terminal refusal. The implementation must keep chunks bounded and avoid
  read-ahead into client-visible buffers.

## Literal std-only contract example

This finite example models the boundary, not a production API or transport. It
checks the trusted audience and current purpose-specific rights on open and
before every chunk, rejects stale scope and revocation, and refuses malformed
or out-of-bounds ranges. The small values are fixtures, not deployment bounds.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Audience(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AssetVersion(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Range {
    start: usize,
    end: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CurrentAccess {
    audience: Audience,
    asset: AssetVersion,
    generation: u8,
    display_allowed: bool,
    revoked: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OpenStream {
    audience: Audience,
    asset: AssetVersion,
    generation: u8,
    range: Range,
    next: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Denied,
    Stale,
    InvalidRange,
}

fn authorize(
    stream: OpenStream,
    current: CurrentAccess,
    range: Range,
    total_bytes: usize,
) -> Result<(), Refusal> {
    if current.revoked || !current.display_allowed || current.audience != stream.audience {
        return Err(Refusal::Denied);
    }
    if current.asset != stream.asset || current.generation != stream.generation {
        return Err(Refusal::Stale);
    }
    if range.start >= range.end || range.end > total_bytes {
        return Err(Refusal::InvalidRange);
    }
    Ok(())
}

fn open(
    audience: Audience,
    asset: AssetVersion,
    generation: u8,
    range: Range,
    current: CurrentAccess,
    total_bytes: usize,
) -> Result<OpenStream, Refusal> {
    let stream = OpenStream {
        audience,
        asset,
        generation,
        range,
        next: range.start,
    };
    authorize(stream, current, range, total_bytes)?;
    Ok(stream)
}

fn next_chunk(
    stream: &mut OpenStream,
    current: CurrentAccess,
    bytes: &[u8],
) -> Result<Option<Vec<u8>>, Refusal> {
    authorize(*stream, current, stream.range, bytes.len())?;
    if stream.next == stream.range.end {
        return Ok(None);
    }
    let end = (stream.next + 2).min(stream.range.end);
    let chunk = bytes[stream.next..end].to_vec();
    stream.next = end;
    Ok(Some(chunk))
}

fn main() {
    let audience = Audience(1);
    let asset = AssetVersion(7);
    let range = Range { start: 1, end: 5 };
    let current = CurrentAccess {
        audience,
        asset,
        generation: 3,
        display_allowed: true,
        revoked: false,
    };
    let bytes = [10, 11, 12, 13, 14, 15];
    let mut stream =
        open(audience, asset, 3, range, current, bytes.len()).expect("fixture is authorized");
    assert_eq!(
        next_chunk(&mut stream, current, &bytes),
        Ok(Some(vec![11, 12]))
    );
    assert_eq!(
        next_chunk(&mut stream, current, &bytes),
        Ok(Some(vec![13, 14]))
    );
    assert_eq!(next_chunk(&mut stream, current, &bytes), Ok(None));

    let revoked = CurrentAccess {
        revoked: true,
        ..current
    };
    let mut after_revoke = open(audience, asset, 3, range, current, bytes.len())
        .expect("open was authorized before revocation");
    assert_eq!(
        next_chunk(&mut after_revoke, revoked, &bytes),
        Err(Refusal::Denied)
    );

    let changed_scope = CurrentAccess {
        generation: 4,
        ..current
    };
    let mut after_scope_change = open(audience, asset, 3, range, current, bytes.len())
        .expect("open was authorized before scope change");
    assert_eq!(
        next_chunk(&mut after_scope_change, changed_scope, &bytes),
        Err(Refusal::Stale)
    );

    assert_eq!(
        open(
            audience,
            asset,
            3,
            Range { start: 4, end: 4 },
            current,
            bytes.len(),
        ),
        Err(Refusal::InvalidRange)
    );
    assert_eq!(
        open(
            audience,
            asset,
            3,
            Range { start: 4, end: 7 },
            current,
            bytes.len(),
        ),
        Err(Refusal::InvalidRange)
    );
    assert_eq!(
        open(audience, asset, 3, range, current, bytes.len() - 2),
        Err(Refusal::InvalidRange)
    );
    assert_eq!(
        open(Audience(2), asset, 3, range, current, bytes.len()),
        Err(Refusal::Denied)
    );
}
```

The example demonstrates finite policy assertions only. It does not prove
current-state lookup, a trusted principal/audience, storage integrity, a real
transport's read-ahead behavior, bounded production memory, tenant isolation,
or revocation propagation across processes.

## Source basis and remaining gates

This decision follows [Subsystem interfaces](subsystem-interfaces.md) (trusted
`Principal`/`AuthorizedAudience`, current audience validation, and the split
`AssetStore`/`AssetMetadataStore` boundary), [Subsystem architecture](subsystem-architecture.md)
(`df-assets`, `df-auth`, `df-persistence`, `df-api`, and `df-session` owners),
[Asset engine](asset-engine.md) (complete immutable assets, audience-filtered
delivery, cache hits subject to current authorization), [Runtime reliability](runtime-reliability.md)
(stale result fencing), [Erasure and retention](erasure-retention-policy.md)
(current `RightsGrant`, suppression generation, and fail-closed stale data),
[G12-D03](g12-d03-heuristic-demand-policy.md) (current rights before prepared,
cache, replay, prompt, and delivery), [Campaign cinematics](campaign-cinematics.md)
(source/range/audience recheck and export rights), [Storage architecture](storage-architecture.md)
(durable complete-byte publication), and [Service operations](service-operations.md)
(tenant isolation, bounded media chunks, revocation and recovery).

Before implementation, G03 must freeze shared IDs/context/range/error types;
the applicable `df-auth` and `df-assets` owners must define how current audience,
tenant, rights, purpose, and suppression facts are queried and fenced; G05 must
provide the authoritative metadata/suppression storage and restore behavior; and
the API/session/persistence owners must establish cross-process revocation and
stream cancellation/read-ahead semantics. The 64 KiB service proposal, stream
buffer bounds, capability expiry, consistency/availability behavior during
metadata outage, exact `RightsGrant` use vocabulary, and transport mapping remain
production decisions to measure and review. No production API, public RPC,
database schema, browser behavior, or runtime rights enforcement is claimed by
this document. Native/WASM workspace checks and integrated two-audience
revocation tests remain unperformed.
