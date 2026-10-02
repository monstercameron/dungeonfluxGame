# Speculative trailer label and future-fact refusal

Status: design contract for F46-D02; production types and integration pending.

## Decision

Every trailer presentation must carry a fixed, plain-language label before its
first trailer frame or caption: **“Speculative trailer — possible future, not a
record or promise.”** The label is presentation metadata owned by the renderer,
not model-authored text; it remains visible on every trailer segment and in
accessible captions. A missing, stale, obscured, or unlocalized label refuses
trailer playback and selects the already prepared ordinary session-end fallback.
The fallback is not relabeled as a trailer.

The label does not make unsafe content safe. Trailer candidates may refer only to
permitted active threads and generic speculative imagery. They must not reveal an
unknown boss or future branch through images, music, subtitles, timing, filenames,
asset manifests, or prefetch; state a future outcome; fabricate chronology; or
bind a player to a scene. No candidate may turn into canonical knowledge or a
committed fact. Any actual foreshadowing disclosure is a separately authorized,
committed story action; it cannot be inferred from generation. This applies even
when the trailer is skipped, hidden from one role, or emitted only as metadata.

`df-knowledge` owns audience-scoped known-thread evidence and access. `df-narrative`
may propose a thread but cannot authorize disclosure or predict its outcome.
`df-presentation` owns the pure candidate plan, fixed label requirement, and
prepared fallback selection over supplied authorized facts/basis. `df-assets`
owns immutable bytes/manifests and audience access; `df-media` executes optional
bounded media generation; `df-session` owns admission, freshness recheck,
publication and deduplicated delivery. The session owner rechecks campaign/run,
audience, source/access revision, active-thread membership, expiry and label
locale before releasing a complete result. `df-engine`/`df-rules` preserve
committed outcomes and legal actions regardless of cinema. Native consumers own
provider, storage, telemetry and transport effects; no pure domain crate receives
those capabilities. Wire codecs stay with `df-api`, `df-client` and
`df-persistence` consumers when G03 admits them.

Optional video requires the separate quote/reservation and host-approved
allowance already required by campaign-cinematics.md. A provider failure,
expired result, stale basis, denied rights, missing locale/label, or failed
qualification returns the prepared still/narration/caption fallback or an
explicit no-trailer result. It never withholds a standard-source reward, adds a
scene, pauses legal play, or reports a generated trailer as successful. If the
safe fallback cannot be delivered, return a visible `Unavailable` result and
continue the session normally.

This policy names the user-facing outcome. It does not establish a production
API or prove that generated visuals/prose have no implicit future claim. The
structural fixture below accepts only enumerated, generic cues and a known active
thread; free-form model output requires later qualification and real-output
review.

## Alternatives considered

1. **No label, relying on the end-session context.** Rejected: trailers can be
   replayed, clipped, or encountered without that context. The label travels with
   every segment and accessible caption.
2. **A softer label such as “Coming soon” or “Next time.”** Rejected: it reads as
   a prediction or promise. “Possible future, not a record or promise” states
   uncertainty directly.
3. **Let the generator write its own disclaimer.** Rejected: it can omit, alter,
   mistranslate, or contradict the disclosure. The renderer supplies a fixed
   localized string outside generated content and refuses when no reviewed
   locale string exists.
4. **Allow a future reveal if the disclaimer is present.** Rejected: a label
   cannot grant knowledge rights or repair a spoiler. A real reveal must be a
   separately committed and authorized story event.
5. **Treat each proposal from `df-narrative` as known merely because it is
   plausible.** Rejected: a candidate proposal is not an audience permission or
   an active-thread fact. `df-knowledge` projection and session revalidation
   remain authoritative.

## Literal std-only contract example

This bounded example exercises the label and structural refusal boundary. A
thread identifier must be in the supplied authorized active-thread set; cues are
a closed enum of generic imagery. Outcome assertions, unknown entities, and
forced scenes have no accepted representation. The fixed label is attached by
the renderer, not supplied by the candidate.

```rust
use std::collections::BTreeSet;

const LABEL: &str = "Speculative trailer — possible future, not a record or promise.";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ThreadId(&'static str);

#[derive(Clone, Debug, Eq, PartialEq)]
enum GenericCue {
    DistantStorm,
    UnlitRoad,
    EmptyDoorway,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CandidateItem {
    KnownThread(ThreadId, GenericCue),
    AssertOutcome(&'static str),
    RevealUnknown(&'static str),
    ForceScene(&'static str),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Refusal {
    UnknownOrUnauthorizedThread,
    FutureOutcome,
    UnknownEntity,
    ForcedScene,
    EmptyCandidate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Trailer {
    label: &'static str,
    cues: Vec<GenericCue>,
}

fn qualify(
    items: &[CandidateItem],
    active_threads: &BTreeSet<ThreadId>,
) -> Result<Trailer, Refusal> {
    if items.is_empty() {
        return Err(Refusal::EmptyCandidate);
    }
    let mut cues = Vec::new();
    for item in items {
        match item {
            CandidateItem::KnownThread(thread, cue) => {
                if !active_threads.contains(thread) {
                    return Err(Refusal::UnknownOrUnauthorizedThread);
                }
                cues.push(cue.clone());
            }
            CandidateItem::AssertOutcome(_) => return Err(Refusal::FutureOutcome),
            CandidateItem::RevealUnknown(_) => return Err(Refusal::UnknownEntity),
            CandidateItem::ForceScene(_) => return Err(Refusal::ForcedScene),
        }
    }
    Ok(Trailer { label: LABEL, cues })
}

fn main() {
    let thread = ThreadId("thread-4");
    let active = BTreeSet::from([thread.clone()]);
    let accepted = qualify(
        &[
            CandidateItem::KnownThread(thread, GenericCue::UnlitRoad),
            CandidateItem::KnownThread(ThreadId("thread-4"), GenericCue::DistantStorm),
        ],
        &active,
    )
    .expect("known active thread and generic cue are allowed");
    assert_eq!(accepted.label, LABEL);
    assert_eq!(
        accepted.cues,
        vec![GenericCue::UnlitRoad, GenericCue::DistantStorm],
    );

    assert_eq!(
        qualify(
            &[CandidateItem::KnownThread(
                ThreadId("unseen-9"),
                GenericCue::EmptyDoorway,
            )],
            &active,
        ),
        Err(Refusal::UnknownOrUnauthorizedThread),
    );
    assert_eq!(
        qualify(&[CandidateItem::AssertOutcome("the queen falls")], &active),
        Err(Refusal::FutureOutcome),
    );
    assert_eq!(
        qualify(&[CandidateItem::RevealUnknown("unknown-boss")], &active),
        Err(Refusal::UnknownEntity),
    );
    assert_eq!(
        qualify(&[CandidateItem::ForceScene("you enter the tower")], &active),
        Err(Refusal::ForcedScene),
    );
    assert_eq!(qualify(&[], &active), Err(Refusal::EmptyCandidate));
}
```

The fixture proves only these structural cases. It does not detect a future claim
implied by arbitrary artwork, music, pacing, filenames, free text, or localization;
nor does it model identity, audience rights, source revisions, expiry, concurrent
staleness, provider behavior, bytes, cost, or playback. A real-output evaluation
must inspect all modalities and metadata. The label must be localized from a
reviewed fixed catalog; fallback remains prepared and permitted under the same
audience rights.

## Acceptance and unresolved production gates

Original acceptance remains unchanged: **no future facts**; the named outcome
must have actual source/build-bound evidence, with unsupported, pending, failed
and unperformed checks explicit. This document makes the policy decision and
literal structural contract reviewable; it does not claim production behavior.

Before implementation, G03/G07/G10/G11 must freeze compatible source/access,
locale, audience, active-thread, job/freshness and presentation contracts. The
owning crates must define a reviewed label catalog, exact safe fallback per
locale, maximum candidate/asset bounds from measured budgets, consent/rights and
revocation handling, expiry/staleness semantics, and optional-video admission.
Do not treat any illustrative duration or bound in another proposal as measured.
The cross-system implementation and end-to-end proof belong to a later F46 slice;
this policy does not duplicate history lookup, media execution, asset storage, or
session authority.

Required production evidence remains pending: paired private/public/member
states with noninterference across frames, audio, captions, manifests and prefetch;
unknown-boss/branch and outcome inference attacks; chronology and forced-scene
refusal; label visibility/accessibility/localization; stale basis and revoked
rights; safe fallback and provider failure; skip/expiry and legal-action parity.
Retain denominator and false accept/reject results on a frozen adversarial corpus.
Independent frontier review must inspect actual user-facing output. No application
source, provider/browser/audio, native/WASM workspace check, or independent
output evaluation was run for this design-only submission. Phone testing is
unperformed; no browser or device support is claimed.

## Source basis

The frozen attempt brief is `development/evidence/fanout-20261001/wave-02/B-F46-D02/brief.json`;
all its governing-source hashes matched before editing. The primary product rule
is `planning/campaign-cinematics.md` (“Recaps and trailers”, “Acceptance”), which
requires known-thread-only generic speculation, no future outcome or forced
montage, prepared fallback, session recheck, and actual zero-invented-future-fact
review. `planning/feature-refinement.md` (“Fifteen numbered ideas”, “Table stakes and phase delivery”, “Planning completion”) selects an explicit speculative label while
keeping paid fidelity optional and unresolved gates real. Ownership comes from
`planning/subsystem-architecture.md`, `planning/subsystem-interfaces.md`, and
`planning/shared-contract-waves.md`. `planning/g11-d02-listener-safe-claim-policy.md`
is reused for listener-scoped claim/access authority and candidate-vs-canonical
separation; `planning/g10-d03-replay-logical-time-policy.md` is reused for
source-bound freshness/replay semantics. These sibling policies are concurrent
proposals, so this document depends on their cited existing ownership decisions,
not on their acceptance status. Workflow, style and evidence requirements follow
`AGENTS.md`, `planning/coding-style.md`, and ADR 0001–0005. Frozen source hashes
and exact fixture checks are retained in the task handoff evidence.
