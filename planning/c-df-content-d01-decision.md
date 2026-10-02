# B-C-df-content-D01: ContentPack publication identity

Status: design decision; production schemas, source registry, rights evidence,
encoding, durable publication, and activation remain pending their named owners.

## Decision

An immutable published ContentPack version binds the exact complete validated
pack bytes to the complete, pinned dependency and provenance manifest carried by
those bytes. The manifest names the pack schema revision, RulesetId and catalog
revision, each source entry's publisher/source identity and revision, locator,
content checksum, permitted namespace and namespace revision, and the reviewed
RightsGrant identity/revision and applicable use. The binding is one indivisible
version: a change to the bytes or to any pinned revision creates a distinct
version; it never mutates or aliases a previously published version. Identical
complete bytes may resolve to the same opaque durable version handle only when
the existing persistence owner guarantees exact byte identity and checks any
handle/digest collision against the complete bytes. The digest, if chosen later,
does not authorize access.

The complete manifest is validation input and part of the immutable published
representation. It is not enough to retain only today's rights answer: each
source/use claim must name the reviewed grant and revision that supported
publication, while later revocation/expiry policy controls new admissions and
delivery under the governing rights policy. A source namespace, catalog revision,
or rights revision mismatch, missing grant, unsupported schema/rules revision,
or absent use permission refuses validation/publication with a typed safe
diagnostic. No default grant, current-latest substitution, namespace inference,
or partial pack publication is permitted. Validation is pure; it evaluates only
the supplied pinned facts and performs no rights lookup or I/O.

`df-content` owns immutable domain records, source/rights/provenance checks,
and pure validation. `df-tools` owns the allowlisted import/author/package
workflow. `df-types` owns shared typed IDs/revisions after G03 freezes them.
Existing `df-persistence` owns exact complete bytes and immutable metadata;
existing authorized publication and activation remain with the session/persistence
owners. `df-session` authorizes publication/activation and commits the selected
reference under the existing fence. `df-model` owns persistent game state,
not authoring schemas. `df-engine` consumes a pinned validated version but does
not publish or commit it. Wire and persistence codecs remain with their existing
consumers. This adds no crate, authority, namespace, storage writer, or competing
policy implementation; it refines the G10-D02 immutable-pack and provenance
contract and the campaign-authoring flow.

## Refusal and change behavior

The validator returns a typed rejection for unknown pack/schema/rules/catalog
revisions, source or namespace revision mismatch, missing/expired/revoked or
use-incompatible RightsGrant facts, missing provenance/locator, duplicate or
ambiguous source identity, unresolved references, unsupported mechanics, and
bounded-input/diagnostic-limit overflow. It produces no `ValidatedPack`, version
handle, publication, activation reference, or successful truncated diagnostics
on refusal. Diagnostics expose safe codes and source IDs/locations, never private
lore, credentials, or grant evidence contents.

An already published version remains immutable and addressable by its existing
references. A newly revoked/expired grant blocks affected new publication and
delivery according to the reviewed rights policy; cached bytes do not bypass
that check. Replacing a grant, source namespace, source revision, catalog, schema,
or ruleset requires a newly validated version. A live campaign cannot silently
retcon: session-owned migration must explicitly map preserved source/content
identities and pending work, pass compatibility checks, and commit under the
normal fence, or refuse/start a separately authorized run. Lost receipts are
resolved by the existing operation identity and payload binding, not by a
filename or “latest” pointer.

## Alternatives and rationale

* A mutable `latest` pack or revision counter alone was rejected: it can change
  what an existing reference means and omits the source/rights basis.
* Filename, upload digest, or private draft bytes as publication identity was
  rejected: the import/validation pipeline can transform or refuse them, and a
  draft digest neither establishes validated content nor grants access.
* A newly invented canonical serialization/hash was deferred: G03/G05/G07/G10
  have not frozen the full schema, field framing, source registry, and durable
  codec. Claiming a reproducible encoding now would be false precision.
* Treating a source ID or rights grant as an unversioned current lookup was
  rejected: later source/namespace/grant changes would rewrite the basis of an
  old pack. Pinning the reviewed identities/revisions preserves that basis.
* A separate publication or rights ledger inside `df-content` was rejected:
  rights evidence remains owned by the reviewed rights/commercial authority;
  the content validator consumes supplied pinned facts and cannot mint grants.

## Literal std-only contract example

This finite illustration checks that publication identity includes exact pack
bytes and the source namespace/source/rights revisions, and refuses missing or
mismatched authorization facts before yielding an identity. The local structs
are explanatory fixture types, not production APIs, hashes, encoders, or durable
handles.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Pins {
    namespace_id: u8,
    namespace_revision: u16,
    source_id: u8,
    source_revision: u16,
    grant_id: u8,
    rights_revision: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    MissingRights,
    NamespaceMismatch,
    SourceMismatch,
    RightsMismatch,
    UseNotGranted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RightsEvidence {
    pins: Pins,
    use_allowed: bool,
}

#[derive(Debug, Eq, PartialEq)]
struct Version<'a> {
    complete_bytes: &'a [u8],
    pins: Pins,
}

fn publish<'a>(
    complete_bytes: &'a [u8],
    expected: Pins,
    supplied: Option<RightsEvidence>,
) -> Result<Version<'a>, Refusal> {
    let evidence = supplied.ok_or(Refusal::MissingRights)?;
    let actual = evidence.pins;
    if actual.namespace_revision != expected.namespace_revision {
        return Err(Refusal::NamespaceMismatch);
    }
    if actual.namespace_id != expected.namespace_id {
        return Err(Refusal::NamespaceMismatch);
    }
    if actual.source_revision != expected.source_revision {
        return Err(Refusal::SourceMismatch);
    }
    if actual.source_id != expected.source_id {
        return Err(Refusal::SourceMismatch);
    }
    if actual.rights_revision != expected.rights_revision {
        return Err(Refusal::RightsMismatch);
    }
    if actual.grant_id != expected.grant_id {
        return Err(Refusal::RightsMismatch);
    }
    if !evidence.use_allowed {
        return Err(Refusal::UseNotGranted);
    }
    Ok(Version {
        complete_bytes,
        pins: actual,
    })
}

fn main() {
    let expected = Pins {
        namespace_id: 2,
        namespace_revision: 4,
        source_id: 5,
        source_revision: 7,
        grant_id: 8,
        rights_revision: 3,
    };
    let evidence = RightsEvidence {
        pins: expected,
        use_allowed: true,
    };
    let bytes_a = [0x43, 0x50, 0x01];
    let bytes_b = [0x43, 0x50, 0x02];
    let accepted = publish(&bytes_a, expected, Some(evidence)).unwrap();
    assert_eq!(accepted.complete_bytes, &bytes_a);
    assert_eq!(accepted.pins, expected);

    let changed_bytes = publish(&bytes_b, expected, Some(evidence)).unwrap();
    assert_ne!(accepted, changed_bytes);
    assert_eq!(
        publish(&bytes_a, expected, None),
        Err(Refusal::MissingRights)
    );

    let stale_namespace = Pins {
        namespace_revision: 3,
        ..expected
    };
    assert_eq!(
        publish(
            &bytes_a,
            expected,
            Some(RightsEvidence {
                pins: stale_namespace,
                ..evidence
            }),
        ),
        Err(Refusal::NamespaceMismatch)
    );
    let stale_source = Pins {
        source_revision: 6,
        ..expected
    };
    assert_eq!(
        publish(
            &bytes_a,
            expected,
            Some(RightsEvidence {
                pins: stale_source,
                ..evidence
            }),
        ),
        Err(Refusal::SourceMismatch)
    );
    let stale_rights = Pins {
        rights_revision: 2,
        ..expected
    };
    assert_eq!(
        publish(
            &bytes_a,
            expected,
            Some(RightsEvidence {
                pins: stale_rights,
                ..evidence
            }),
        ),
        Err(Refusal::RightsMismatch)
    );
    assert_eq!(
        publish(
            &bytes_a,
            expected,
            Some(RightsEvidence {
                use_allowed: false,
                ..evidence
            }),
        ),
        Err(Refusal::UseNotGranted),
    );
}
```

This fixture does not perform cryptographic hashing, canonical serialization,
authorization, revocation lookup, durable collision checking, or live migration.
Production owners must replace its illustrative pins with the reviewed typed
records and complete-manifest validation without changing the refusal guarantees.

## Sources, unresolved gates, and evidence

This decision follows `planning/g10-d02-pack-provenance-validation-policy.md`
(exact complete validated bytes, supplied pinned source facts, explicit rights,
immutable version, and existing owner boundaries), `planning/g10-d01-private-authoring-pack-policy.md` (opaque private draft and explicit import boundary),
`planning/campaign-authoring.md` (“Versioned authoring model”, “Pure validation
and scoped publication”, “Personal payload and rights lifecycle”),
`planning/commercial-validation.md` (“Rights-complete launch gate”),
`planning/subsystem-architecture.md` (“Shared and transport crates”, “Server
crates”), `planning/subsystem-interfaces.md` (“Common contract rules”, “Pure
domain and content”, “Identity and session ownership”, “PostgreSQL persistence
and durable media”), and `planning/storage-architecture.md` (“Content authoring
and personal payload”). Shared compatibility changes follow
`planning/shared-contract-waves.md`; diagnostics and local error style follow
`planning/coding-style.md`.

Still open: G03 freezes concrete shared types and namespace/revision semantics;
G05/G10 choose complete manifest/schema and bounded production limits;
G07 plus the reviewed commercial/rightsholder authority selects supported source
families, grant evidence and revocation rules; `df-content` implements pure
validation; existing tools/import owners select the allowlisted format;
`df-persistence` selects storage representation, digest/opaque handle mapping,
collision response, atomic metadata binding, and retention; `df-session` freezes
authorized publish/activation interfaces and fenced migration; consuming codec
owners freeze wire/storage encodings. Commercial use remains blocked for any
source family without reviewed grant evidence. No production or runtime behavior
is claimed by this design or by the finite fixture.

The source evidence root retains hashes of every frozen governing input, the
literal extracted verbatim from this document, guarded rustfmt/rustc/execution
receipts, commit identity, and handoff. Native/WASM workspace checks, full game
behavior, browser/device testing, rights-holder clearance, and durable publication
integration are unperformed. Independent frontier review and root integration
remain separate gates.
