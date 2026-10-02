# F46-D01: Committed recap fact selection

Status: bounded design decision; production contracts and integration pending.

This decision resolves the F46-D01 outcome, **audience permission pinned**. It
specifies how a recap obtains and selects established history for one authorized
audience. It does not freeze Rust production types, storage schemas, RPCs, a
provider flow, or cinematic behavior.

## Decision

A recap selects only exact facts and attributed claims returned from committed
campaign history for the requested bounded decision/event range, after the
requesting principal and intended audience have been authorized. The trusted
session/API boundary derives a pure `df-model::AudienceScope` from
`df-auth::AuthorizedAudience`; trace context, room membership, a caller-supplied
audience label, or existence in canonical state is not permission. The
`df-session`-owned `MemoryCandidateStore` native port performs an admitted
`RecapHistory` lookup through the `df-persistence` adapter. `df-knowledge`
reauthorizes supplied candidates against the audience and access/source basis;
it does not read SQL or treat an index or summary as authority. Selection returns
exact committed fact/claim references with observer scope and provenance. A
summary can help rank or phrase already selected references, but cannot introduce
or authorize a recap fact.

Selection refuses the entire candidate when authorization is absent, a fact is
pending/uncommitted/failed, the audience lacks access, the campaign/run/range or
source/access revision is stale or mismatched, the provenance is unsupported, or
the only support is a derived summary. Refusal returns an explicit typed gap or
the already prepared source-safe fallback; it never fabricates success or
silently broadens the audience. Empty permitted history is an empty/limited recap
outcome, not permission to fill gaps from hidden context.

Facts and attributed claims remain distinct. A belief, memory, report, rumor or
lie is eligible only if its claim and speaker/observer attribution are permitted
for this audience; it remains attributed and cannot replace canonical truth. For
a newly joined member, current entitlement to the bounded history is checked
independently of whether other members saw those events. Prior screenshots,
recordings and audio are separately checked for current audience and capture
rights. The recap selection does not imply consent to export or share.

This is a history-selection decision, not a guarantee that later narrative or
media output is safe. The complete selected basis must remain bound to campaign,
run, audience, event/decision range, policy/content/identity revisions and locale
through pure `df-presentation::plan_bookend`; after async work, the owning
`df-engine`/`df-session` path rechecks those bindings before committing effect
references or publishing. A stale result is rejected and uses the permitted
prepared fallback. Filtering occurs before provider input, prompt construction,
caption/audio generation and serialization. Default logs exclude private fact
payloads. Recaps do not alter canonical outcomes or block session entry.

### Ownership and integration

`df-auth` establishes the trusted principal and authorized audience. The session
and API derive the `df-model::AudienceScope`; `df-session` owns authorized bounded
history access and the `MemoryCandidateStore` port; `df-persistence` adapts that
port to exact committed records; `df-knowledge` applies pure observer access and
provenance rules to supplied candidates; `df-narrative` may rank story
significance but cannot grant access; `df-presentation` proposes a bounded
bookend from permitted facts; `df-media` runs only admitted optional jobs;
`df-assets` publishes complete immutable assets with access metadata. `df-engine`
composes candidate changes and `df-session` alone commits. `df-api` projects
authorized responses; `df-server` wires native effects. Domain code has no
database, clock, provider, socket or telemetry SDK I/O. Wire codecs belong to
their established API/client/persistence consumers. No cinematic crate, second
history writer or parallel permission authority is introduced.

The cross-system hooks are outstanding: bind authorized session/API audience to
the history lookup; project the same scope through knowledge and presentation;
recheck it at async completion and asset access; and verify recap/fallback in the
integrated browser. Owners named in the F46 plan are `df-knowledge`,
`df-narrative`, `df-presentation`, `df-media`, `df-assets` and `df-session`, with
the existing `df-auth`, `df-persistence`, `df-engine`, `df-api` and `df-server`
contracts at their stated boundaries.

### Alternatives

1. **Summarize all canonical history, then redact for each viewer.** Rejected:
   private details have already crossed into derived prose and may not be
   reliably removable. Authorize and filter exact source records first.
2. **Let a generated summary authorize facts.** Rejected: summaries/indexes are
   derived and can be stale, lossy or semantically broadened. They may rank only
   source-backed candidates; each selected claim retains an exact committed
   reference.
3. **Use only the latest shared-party recap or room membership.** Rejected:
   join time and membership do not establish permission to every covered fact;
   audience access is checked per fact and range.
4. **Show only fixed canned recap text.** Safe but needlessly discards useful
   source-backed history. Retain deterministic fact/claim slots and fallback;
   bounded presentation can arrange those authorized references without adding
   facts.
5. **Treat a recap request as consent to publish/export prior media.** Rejected:
   private recap access and capture/contributor rights are separate. Export needs
   the distinct explicit `ExportGrant` planned by F46.

## Literal std-only contract example

This finite example demonstrates selection from a trusted history result. It
accepts an authorized committed fact and attributed belief, and refuses an
unauthorized private fact, a pending fact, and a stale source revision. Its
structs are illustrative; they are not a competing production API or evidence of
runtime/game behavior. The example assumes that the session/auth boundary supplied
the authorized viewer and that records came from the committed-history port.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Viewer(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FactId(u8);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceRevision(u16);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClaimKind {
    CanonicalFact,
    AttributedBelief { speaker: u8 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CommittedCandidate {
    id: FactId,
    kind: ClaimKind,
    committed: bool,
    permitted_viewers: &'static [u8],
    source_revision: SourceRevision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    NotCommitted,
    AudienceDenied,
    StaleSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SelectedFact {
    id: FactId,
    kind: ClaimKind,
    source_revision: SourceRevision,
}

fn select_for_recap(
    viewer: Viewer,
    current_source: SourceRevision,
    candidates: &[CommittedCandidate],
) -> Result<Vec<SelectedFact>, Refusal> {
    let mut selected = Vec::new();
    for candidate in candidates {
        if !candidate.committed {
            return Err(Refusal::NotCommitted);
        }
        if !candidate.permitted_viewers.contains(&viewer.0) {
            return Err(Refusal::AudienceDenied);
        }
        if candidate.source_revision != current_source {
            return Err(Refusal::StaleSource);
        }
        selected.push(SelectedFact {
            id: candidate.id,
            kind: candidate.kind,
            source_revision: candidate.source_revision,
        });
    }
    Ok(selected)
}

fn main() {
    let alice = Viewer(1);
    let bob = Viewer(2);
    let current = SourceRevision(7);
    let public_fact = CommittedCandidate {
        id: FactId(10),
        kind: ClaimKind::CanonicalFact,
        committed: true,
        permitted_viewers: &[1, 2],
        source_revision: current,
    };
    let attributed_belief = CommittedCandidate {
        id: FactId(11),
        kind: ClaimKind::AttributedBelief { speaker: 4 },
        committed: true,
        permitted_viewers: &[1],
        source_revision: current,
    };
    let private_fact = CommittedCandidate {
        id: FactId(12),
        kind: ClaimKind::CanonicalFact,
        committed: true,
        permitted_viewers: &[1],
        source_revision: current,
    };
    let pending_fact = CommittedCandidate {
        committed: false,
        ..public_fact
    };
    let stale_fact = CommittedCandidate {
        source_revision: SourceRevision(6),
        ..public_fact
    };

    let selected = select_for_recap(alice, current, &[public_fact, attributed_belief])
        .expect("authorized committed candidates are selectable");
    assert_eq!(selected.len(), 2);
    assert_eq!(selected[0].id, FactId(10));
    assert_eq!(selected[1].kind, ClaimKind::AttributedBelief { speaker: 4 });
    assert_eq!(
        select_for_recap(bob, current, &[private_fact]),
        Err(Refusal::AudienceDenied)
    );
    assert_eq!(
        select_for_recap(alice, current, &[pending_fact]),
        Err(Refusal::NotCommitted)
    );
    assert_eq!(
        select_for_recap(alice, current, &[stale_fact]),
        Err(Refusal::StaleSource)
    );
}
```

The example's all-or-refuse behavior prevents a partial result from concealing a
failed authorization check. It does not model database lookup, trusted principal
construction, range/run binding, audience-policy revisions, access revocation,
summary rejection, retention, async generation fencing, export rights, or
publication. Production selection must bind those inputs and return typed
unsupported/pending/failed outcomes; no estimated numerical limits are proposed
here.

## Failure semantics and remaining gates

Permission denial, stale source/access basis, unsupported provenance, an
uncommitted/pending record, or a summary-only candidate yields a typed refusal
with no generated output. The caller may use a prepared fallback that is itself
authorized for that viewer, or return a visible limited/unavailable result.
Storage/auth/capacity/deadline failures remain distinct operational failures; an
ambiguous durable commit preserves uncertainty and is never reported as success.
Cancellation cannot undo a committed decision. Generation/job and audience/source
binding checks reject stale completions. No failure broadens access or causes a
provider retry during prepared-only/replay operation.

Still unresolved before production dispatch: G03 must freeze compatible typed
identity, audience, claim, revision, result and error contracts; G05/G07 must
resolve persistence and source/catalog provenance; G10/G11 must pin the required
replay, access-generation, revocation and listener-safe basis; owners must choose
and measure decision/event, result-byte, retrieval, expiry and retention bounds;
F46-MODEL/DELIVER/ACCEPT must connect the session lookup, knowledge projection,
presentation plan, admitted optional media, immutable asset publication and
authorized API view. Independent frontier evaluation must exercise the integrated
browser flow, relevant member/private audiences, stale/cancel/fallback cases and
actual audio when applicable. Phone testing remains deferred. No provider,
browser, source implementation, native/WASM workspace check, production storage
check, or independent output review was performed for this policy decision.

## Governing source basis

Frozen task inputs for `B-F46-D01-a1` are retained in its issued `brief.json` and
`worker-context.json`; their governing hashes are recorded in the handoff.
The direct decision is [Campaign bookends, critical cues and media continuity](campaign-cinematics.md),
“Recaps and trailers”: exact committed event/claim references, observer scope,
authorized bounded history lookup, exact committed facts/attributed beliefs, and
no summary-only authorization. [Feature refinement](feature-refinement.md)
selects audience-permitted committed history and lists F46 as pending actual
acceptance. [Subsystem interfaces](subsystem-interfaces.md) separates trusted
audience from correlation and assigns the native memory port to `df-session`,
with supplied-candidate reauthorization in `df-knowledge`. [Subsystem
architecture](subsystem-architecture.md) assigns auth, session, persistence,
knowledge, presentation, media, assets and API to their existing owners.

The previously reviewed [G11 listener-safe claim policy](g11-d02-listener-safe-claim-policy.md)
is used for its established distinctions among canonical fact, attributed claim,
and creative expression. It is a policy reference, not a declared task dependency
or production implementation. [G10 replay policy](g10-d03-replay-logical-time-policy.md)
supplies the no-regeneration-on-replay and explicit unsupported-gap posture. The
architecture/storage/observability/runtime plans and `AGENTS.md`,
`planning/coding-style.md`, ADR 0001–0005 govern purity, durable authority,
privacy, bounded evidence, handoff, and independent evaluation.
