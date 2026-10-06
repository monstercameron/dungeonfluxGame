//! Executable private reference model for the utterance-to-media handoff.
//!
//! These types are deliberately test-local. Current production media accepts opaque speech
//! bytes; this example checks the proposed owner rule around its real bounded scheduler without
//! claiming that qualification, caption projection, audio decoding, or playback is implemented.

use df_media::schedule::{ScheduleLimits, ScheduleSnapshot};
use df_media::speech::{
    SpeechError, SpeechIdentity, SpeechLimits, SpeechReceipt, SpeechScheduler, SpeechStopReason,
};
use df_model::checkpoint::{Basis, JobId};
use df_types::{OperationId, RecoveryEpoch, RunId, SessionId, SessionRevision};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Branch {
    Live,
    Prepared,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct UtteranceScope {
    basis: Basis,
    audience: u8,
    utterance: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Qualification {
    Approved,
    Pending,
    Rejected,
    Unsupported,
    Unperformed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ApprovedText {
    scope: UtteranceScope,
    branch: Branch,
    value: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RouteError {
    NotApproved,
    WrongScope,
    WrongAttempt,
    TextMismatch,
    Media,
}

#[derive(Debug)]
struct AdmittedAttempt {
    identity: SpeechIdentity,
    approved: ApprovedText,
}

#[derive(Debug)]
struct SelectedAttempt {
    receipt: SpeechReceipt,
    admitted: AdmittedAttempt,
}

struct RouteOwner {
    scheduler: SpeechScheduler,
    current_basis: Basis,
    queued: Vec<AdmittedAttempt>,
    current: Vec<(UtteranceScope, SpeechIdentity)>,
}

#[derive(Debug, Eq, PartialEq)]
struct VisibleGap {
    scope: UtteranceScope,
    reason: GapReason,
    audio: Option<Vec<u8>>,
}

#[derive(Debug, Eq, PartialEq)]
enum GapReason {
    NoUsableFallback,
}

#[derive(Debug, Eq, PartialEq)]
struct DeliveredPair {
    scope: UtteranceScope,
    branch: Branch,
    caption_text: String,
    speech_request_text: String,
    opaque_audio_bytes: Vec<u8>,
}

fn basis(revision: u64) -> Basis {
    Basis {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        revision: SessionRevision::new(RecoveryEpoch::new(1).unwrap(), revision),
    }
}

fn identity(job: u8, operation: u8, generation: u64, basis: Basis) -> SpeechIdentity {
    SpeechIdentity {
        basis,
        job: JobId::from_bytes(&[job; 16]).unwrap(),
        operation: OperationId::from_bytes(&[operation; 16]).unwrap(),
        generation,
    }
}

fn owner(current: Basis) -> RouteOwner {
    let scheduler = SpeechScheduler::new(
        current,
        ScheduleLimits {
            queue_items: 4,
            queue_bytes: 128,
            speech_items: 4,
            speech_bytes: 128,
            execution_slots: 2,
            speech_slots: 2,
        },
        SpeechLimits {
            maximum_chunks: 4,
            maximum_bytes: 16,
        },
    )
    .unwrap();
    RouteOwner {
        scheduler,
        current_basis: current,
        queued: Vec::new(),
        current: Vec::new(),
    }
}

fn advance_basis(owner: &mut RouteOwner, current: Basis) -> Result<(), SpeechError> {
    owner.scheduler.replace_basis(current)?;
    owner.current_basis = current;
    Ok(())
}

fn select_text(
    scope: UtteranceScope,
    branch: Branch,
    qualification: Qualification,
    value: &str,
) -> Result<ApprovedText, RouteError> {
    if qualification != Qualification::Approved {
        return Err(RouteError::NotApproved);
    }
    Ok(ApprovedText {
        scope,
        branch,
        value: value.to_owned(),
    })
}

fn admit_attempt(
    owner: &mut RouteOwner,
    approved: &ApprovedText,
    scope: UtteranceScope,
    request_text: &str,
    identity: SpeechIdentity,
) -> Result<(), RouteError> {
    if scope != approved.scope
        || identity.basis != scope.basis
        || scope.basis != owner.current_basis
    {
        return Err(RouteError::WrongScope);
    }
    if request_text != approved.value {
        return Err(RouteError::TextMismatch);
    }
    owner
        .scheduler
        .admit(
            identity,
            request_text.as_bytes().to_vec().into_boxed_slice(),
        )
        .map_err(|_| RouteError::Media)?;
    owner.queued.push(AdmittedAttempt {
        identity,
        approved: approved.clone(),
    });
    Ok(())
}

fn begin_attempt(owner: &mut RouteOwner) -> Result<SelectedAttempt, RouteError> {
    let receipt = owner
        .scheduler
        .begin()
        .map_err(|_| RouteError::Media)?
        .ok_or(RouteError::Media)?;
    let Some(position) = owner
        .queued
        .iter()
        .position(|admitted| admitted.identity == receipt.identity())
    else {
        owner
            .scheduler
            .stop(&receipt, SpeechStopReason::Cancelled)
            .map_err(|_| RouteError::Media)?;
        return Err(RouteError::WrongAttempt);
    };
    let admitted = owner.queued.remove(position);
    owner
        .current
        .retain(|(scope, _)| *scope != admitted.approved.scope);
    owner
        .current
        .push((admitted.approved.scope, admitted.identity));
    Ok(SelectedAttempt { receipt, admitted })
}

fn start_attempt(
    owner: &mut RouteOwner,
    approved: &ApprovedText,
    scope: UtteranceScope,
    request_text: &str,
    identity: SpeechIdentity,
) -> Result<SelectedAttempt, RouteError> {
    admit_attempt(owner, approved, scope, request_text, identity)?;
    begin_attempt(owner)
}

fn complete_attempt(
    owner: &mut RouteOwner,
    selected: &SelectedAttempt,
    receipt: &SpeechReceipt,
    approved: &ApprovedText,
    scope: UtteranceScope,
    request_text: &str,
    encoded_fixture_bytes: &[u8],
) -> Result<DeliveredPair, RouteError> {
    if scope != approved.scope
        || scope != selected.admitted.approved.scope
        || scope.basis != owner.current_basis
        || approved.branch != selected.admitted.approved.branch
    {
        return Err(RouteError::WrongScope);
    }
    if receipt.identity() != selected.admitted.identity
        || selected.receipt.identity() != selected.admitted.identity
        || !owner.current.contains(&(scope, selected.admitted.identity))
    {
        return Err(RouteError::WrongAttempt);
    }
    if request_text != selected.admitted.approved.value
        || approved.value != selected.admitted.approved.value
    {
        return Err(RouteError::TextMismatch);
    }
    owner
        .scheduler
        .queue_chunk(
            receipt,
            receipt.identity(),
            0,
            encoded_fixture_bytes.to_vec().into_boxed_slice(),
        )
        .map_err(|_| RouteError::Media)?;
    owner
        .scheduler
        .end(receipt, receipt.identity(), 1)
        .map_err(|_| RouteError::Media)?;
    owner
        .scheduler
        .eof(receipt, receipt.identity())
        .map_err(|_| RouteError::Media)?;
    let chunk = owner
        .scheduler
        .next(receipt)
        .map_err(|_| RouteError::Media)?
        .ok_or(RouteError::Media)?;
    let opaque_audio_bytes = chunk.bytes.to_vec();
    owner
        .scheduler
        .acknowledge(receipt, 0)
        .map_err(|_| RouteError::Media)?;
    let request = owner
        .scheduler
        .finish(receipt)
        .map_err(|_| RouteError::Media)?;
    if request.payload() != request_text.as_bytes() {
        return Err(RouteError::TextMismatch);
    }
    owner
        .current
        .retain(|(bound_scope, identity)| *bound_scope != scope || *identity != receipt.identity());
    Ok(DeliveredPair {
        scope,
        branch: approved.branch,
        caption_text: approved.value.clone(),
        speech_request_text: request_text.to_owned(),
        opaque_audio_bytes,
    })
}

fn visible_gap(
    owner: &mut RouteOwner,
    selected: &SelectedAttempt,
) -> Result<VisibleGap, RouteError> {
    let scope = selected.admitted.approved.scope;
    if scope.basis != owner.current_basis
        || !owner.current.contains(&(scope, selected.admitted.identity))
    {
        return Err(RouteError::WrongAttempt);
    }
    owner
        .scheduler
        .stop(&selected.receipt, SpeechStopReason::Cancelled)
        .map_err(|_| RouteError::Media)?;
    owner.current.retain(|(bound_scope, identity)| {
        *bound_scope != scope || *identity != selected.admitted.identity
    });
    Ok(VisibleGap {
        scope,
        reason: GapReason::NoUsableFallback,
        audio: None,
    })
}

fn snapshot(owner: &RouteOwner) -> ScheduleSnapshot {
    owner.scheduler.snapshot().schedule
}

#[test]
fn selected_live_branch_uses_one_approved_text_for_caption_and_speech_request() {
    let current = basis(7);
    let scope = UtteranceScope {
        basis: current,
        audience: 3,
        utterance: 9,
    };
    let approved = select_text(
        scope,
        Branch::Live,
        Qualification::Approved,
        "The north gate is open.",
    )
    .unwrap();
    let mut scheduler = owner(current);
    let selected = start_attempt(
        &mut scheduler,
        &approved,
        scope,
        "The north gate is open.",
        identity(4, 5, 11, current),
    )
    .unwrap();

    let pair = complete_attempt(
        &mut scheduler,
        &selected,
        &selected.receipt,
        &approved,
        scope,
        "The north gate is open.",
        &[0x41, 0x42],
    )
    .unwrap();

    assert_eq!(pair.caption_text, approved.value);
    assert_eq!(pair.speech_request_text, pair.caption_text);
    assert_eq!(pair.scope, scope);
    assert_eq!(pair.branch, Branch::Live);
    assert_eq!(pair.opaque_audio_bytes, vec![0x41, 0x42]);
    assert_eq!(snapshot(&scheduler).completed_dispatches, 1);
}

#[test]
fn prepared_fallback_uses_its_own_text_and_late_live_data_is_rejected() {
    let current = basis(7);
    let scope = UtteranceScope {
        basis: current,
        audience: 3,
        utterance: 9,
    };
    let live_text = select_text(
        scope,
        Branch::Live,
        Qualification::Approved,
        "A live candidate that will be interrupted.",
    )
    .unwrap();
    let fallback_text = select_text(
        scope,
        Branch::Prepared,
        Qualification::Approved,
        "The prepared gate announcement is ready.",
    )
    .unwrap();
    let mut scheduler = owner(current);
    let live_identity = identity(4, 5, 11, current);
    let live_selected = start_attempt(
        &mut scheduler,
        &live_text,
        scope,
        &live_text.value,
        live_identity,
    )
    .unwrap();
    scheduler
        .scheduler
        .stop(&live_selected.receipt, SpeechStopReason::Cancelled)
        .unwrap();
    let late = scheduler.scheduler.queue_chunk(
        &live_selected.receipt,
        live_identity,
        0,
        Box::from([0x55]),
    );
    assert_eq!(late.unwrap_err().reason, SpeechError::Stale);

    let fallback_identity = identity(6, 7, 12, current);
    let fallback_selected = start_attempt(
        &mut scheduler,
        &fallback_text,
        scope,
        &fallback_text.value,
        fallback_identity,
    )
    .unwrap();
    let pair = complete_attempt(
        &mut scheduler,
        &fallback_selected,
        &fallback_selected.receipt,
        &fallback_text,
        scope,
        &fallback_text.value,
        &[0x61],
    )
    .unwrap();

    assert_ne!(live_identity.job, fallback_identity.job);
    assert_eq!(pair.scope, scope);
    assert_eq!(pair.branch, Branch::Prepared);
    assert_eq!(
        pair.caption_text,
        "The prepared gate announcement is ready."
    );
    assert_eq!(pair.speech_request_text, pair.caption_text);
    assert!(!pair.caption_text.contains("interrupted"));
    assert_eq!(snapshot(&scheduler).cancelled_dispatches, 1);
    assert_eq!(snapshot(&scheduler).completed_dispatches, 1);
}

#[test]
fn nonapproved_text_scope_mismatch_and_text_mismatch_do_not_admit_media_work() {
    let current = basis(7);
    let scope = UtteranceScope {
        basis: current,
        audience: 3,
        utterance: 9,
    };
    let mut scheduler = owner(current);
    let before = snapshot(&scheduler);
    for qualification in [
        Qualification::Pending,
        Qualification::Rejected,
        Qualification::Unsupported,
        Qualification::Unperformed,
    ] {
        assert_eq!(
            select_text(scope, Branch::Live, qualification, "unapproved words"),
            Err(RouteError::NotApproved)
        );
    }
    let approved = select_text(
        scope,
        Branch::Live,
        Qualification::Approved,
        "Approved words.",
    )
    .unwrap();
    let identity = identity(4, 5, 11, current);
    assert!(matches!(
        start_attempt(
            &mut scheduler,
            &approved,
            UtteranceScope {
                audience: 8,
                ..scope
            },
            &approved.value,
            identity,
        ),
        Err(RouteError::WrongScope)
    ));
    assert!(matches!(
        start_attempt(
            &mut scheduler,
            &approved,
            scope,
            "different TTS words",
            identity,
        ),
        Err(RouteError::TextMismatch)
    ));
    assert_eq!(snapshot(&scheduler), before);
}

#[test]
fn admitted_receipt_cannot_be_rebound_to_another_scope_branch_or_text() {
    let current = basis(7);
    let scope = UtteranceScope {
        basis: current,
        audience: 3,
        utterance: 9,
    };
    let original = select_text(
        scope,
        Branch::Live,
        Qualification::Approved,
        "Approved words.",
    )
    .unwrap();
    let alternatives = [
        (
            UtteranceScope {
                audience: 8,
                ..scope
            },
            Branch::Live,
            "Approved words.",
            RouteError::WrongScope,
        ),
        (
            UtteranceScope {
                utterance: 10,
                ..scope
            },
            Branch::Live,
            "Approved words.",
            RouteError::WrongScope,
        ),
        (
            scope,
            Branch::Prepared,
            "Approved words.",
            RouteError::WrongScope,
        ),
        (
            scope,
            Branch::Live,
            "Other words.",
            RouteError::TextMismatch,
        ),
    ];
    for (rebound_scope, branch, text, expected) in alternatives {
        let mut owner = owner(current);
        let selected = start_attempt(
            &mut owner,
            &original,
            scope,
            &original.value,
            identity(4, 5, 11, current),
        )
        .unwrap();
        let rebound = select_text(rebound_scope, branch, Qualification::Approved, text).unwrap();
        let before = owner.scheduler.snapshot();
        assert_eq!(
            complete_attempt(
                &mut owner,
                &selected,
                &selected.receipt,
                &rebound,
                rebound_scope,
                &rebound.value,
                &[0x41],
            ),
            Err(expected)
        );
        assert_eq!(owner.scheduler.snapshot(), before);
        let pair = complete_attempt(
            &mut owner,
            &selected,
            &selected.receipt,
            &original,
            scope,
            &original.value,
            &[0x41],
        )
        .unwrap();
        assert_eq!(pair.scope, scope);
        assert_eq!(pair.branch, Branch::Live);
    }
}

#[test]
fn actual_scheduler_selection_uses_the_older_admitted_context() {
    let current = basis(7);
    let older_scope = UtteranceScope {
        basis: current,
        audience: 3,
        utterance: 9,
    };
    let newer_scope = UtteranceScope {
        audience: 8,
        utterance: 10,
        ..older_scope
    };
    let older = select_text(
        older_scope,
        Branch::Live,
        Qualification::Approved,
        "Earlier approved words.",
    )
    .unwrap();
    let newer = select_text(
        newer_scope,
        Branch::Prepared,
        Qualification::Approved,
        "Later prepared words.",
    )
    .unwrap();
    let older_identity = identity(4, 5, 11, current);
    let newer_identity = identity(6, 7, 12, current);
    let mut owner = owner(current);
    admit_attempt(
        &mut owner,
        &older,
        older_scope,
        &older.value,
        older_identity,
    )
    .unwrap();
    let first = start_attempt(
        &mut owner,
        &newer,
        newer_scope,
        &newer.value,
        newer_identity,
    )
    .unwrap();
    assert_eq!(first.receipt.identity(), older_identity);
    assert_eq!(first.admitted.approved, older);
    let second = begin_attempt(&mut owner).unwrap();
    assert_eq!(second.receipt.identity(), newer_identity);
    assert_eq!(second.admitted.approved, newer);
    let first_pair = complete_attempt(
        &mut owner,
        &first,
        &first.receipt,
        &older,
        older_scope,
        &older.value,
        &[0x41],
    )
    .unwrap();
    let second_pair = complete_attempt(
        &mut owner,
        &second,
        &second.receipt,
        &newer,
        newer_scope,
        &newer.value,
        &[0x42],
    )
    .unwrap();
    assert_eq!(first_pair.caption_text, older.value);
    assert_eq!(second_pair.caption_text, newer.value);
}

#[test]
fn wrong_receipt_identity_and_superseded_selection_cannot_publish() {
    let current = basis(7);
    let scope = UtteranceScope {
        basis: current,
        audience: 3,
        utterance: 9,
    };
    let approved = select_text(
        scope,
        Branch::Live,
        Qualification::Approved,
        "Current words.",
    )
    .unwrap();
    let selected_identity = identity(4, 5, 11, current);
    for foreign_identity in [
        identity(6, 5, 11, current),
        identity(4, 7, 11, current),
        identity(4, 5, 12, current),
        identity(4, 5, 11, basis(8)),
    ] {
        let mut owner = owner(current);
        let selected = start_attempt(
            &mut owner,
            &approved,
            scope,
            &approved.value,
            selected_identity,
        )
        .unwrap();
        let mut foreign_owner = self::owner(foreign_identity.basis);
        let foreign_scope = UtteranceScope {
            basis: foreign_identity.basis,
            ..scope
        };
        let foreign_text = select_text(
            foreign_scope,
            Branch::Live,
            Qualification::Approved,
            &approved.value,
        )
        .unwrap();
        let foreign = start_attempt(
            &mut foreign_owner,
            &foreign_text,
            foreign_scope,
            &foreign_text.value,
            foreign_identity,
        )
        .unwrap();
        let before = owner.scheduler.snapshot();
        assert_eq!(
            complete_attempt(
                &mut owner,
                &selected,
                &foreign.receipt,
                &approved,
                scope,
                &approved.value,
                &[0x41],
            ),
            Err(RouteError::WrongAttempt)
        );
        assert_eq!(owner.scheduler.snapshot(), before);
    }

    let mut owner = owner(current);
    let live = start_attempt(
        &mut owner,
        &approved,
        scope,
        &approved.value,
        selected_identity,
    )
    .unwrap();
    let prepared = select_text(
        scope,
        Branch::Prepared,
        Qualification::Approved,
        "Prepared words.",
    )
    .unwrap();
    let fallback = start_attempt(
        &mut owner,
        &prepared,
        scope,
        &prepared.value,
        identity(6, 7, 12, current),
    )
    .unwrap();
    let before = owner.scheduler.snapshot();
    assert_eq!(
        complete_attempt(
            &mut owner,
            &live,
            &live.receipt,
            &approved,
            scope,
            &approved.value,
            &[0x41],
        ),
        Err(RouteError::WrongAttempt)
    );
    assert_eq!(owner.scheduler.snapshot(), before);
    let pair = complete_attempt(
        &mut owner,
        &fallback,
        &fallback.receipt,
        &prepared,
        scope,
        &prepared.value,
        &[0x42],
    )
    .unwrap();
    assert_eq!(pair.branch, Branch::Prepared);
    owner
        .scheduler
        .stop(&live.receipt, SpeechStopReason::Cancelled)
        .unwrap();
}

#[test]
fn stale_basis_and_silent_failure_leave_no_fabricated_audio() {
    let current = basis(7);
    let scope = UtteranceScope {
        basis: current,
        audience: 3,
        utterance: 9,
    };
    let approved = select_text(
        scope,
        Branch::Live,
        Qualification::Approved,
        "Approved words.",
    )
    .unwrap();
    let mut stale_owner = owner(current);
    let stale = start_attempt(
        &mut stale_owner,
        &approved,
        scope,
        &approved.value,
        identity(4, 5, 11, current),
    )
    .unwrap();
    advance_basis(&mut stale_owner, basis(8)).unwrap();
    let before = stale_owner.scheduler.snapshot();
    assert_eq!(
        complete_attempt(
            &mut stale_owner,
            &stale,
            &stale.receipt,
            &approved,
            scope,
            &approved.value,
            &[0x41],
        ),
        Err(RouteError::WrongScope)
    );
    assert_eq!(stale_owner.scheduler.snapshot(), before);

    let mut owner = owner(current);
    let selected = start_attempt(
        &mut owner,
        &approved,
        scope,
        &approved.value,
        identity(4, 5, 11, current),
    )
    .unwrap();
    let gap = visible_gap(&mut owner, &selected).unwrap();
    assert_eq!(gap.scope, scope);
    assert_eq!(gap.reason, GapReason::NoUsableFallback);
    assert_eq!(gap.audio, None);
    assert_eq!(snapshot(&owner).cancelled_dispatches, 1);
    assert_eq!(snapshot(&owner).completed_dispatches, 0);
    assert_eq!(
        visible_gap(&mut owner, &selected),
        Err(RouteError::WrongAttempt)
    );
}
