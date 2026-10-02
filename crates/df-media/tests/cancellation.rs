use df_media::cancellation::{CompletionRefusal, LifecycleError, MediaLifecycle, SceneBinding};
use df_types::{OperationId, RecoveryEpoch, RunId, SessionId, SessionRevision};

fn operation(value: u8) -> OperationId {
    OperationId::from_bytes(&[value; 16]).unwrap()
}

fn binding(epoch: u64, sequence: u64, scene: u8, cue: u8) -> SceneBinding<u8, u8> {
    SceneBinding {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        scene,
        cue,
        revision: SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), sequence),
    }
}

#[test]
fn admitted_output_updates_the_owned_scene_once_and_preserves_operation() {
    let scene = binding(1, 4, 1, 3);
    let mut owner = MediaLifecycle::new(scene.clone(), 2).unwrap();
    let token = owner.admit(1_u8, operation(3), scene).unwrap();
    let mut presented = None;
    owner
        .complete(
            token,
            String::from("validated image"),
            |operation_id, image| {
                presented = Some((operation_id, image));
            },
        )
        .unwrap();
    assert_eq!(
        presented,
        Some((operation(3), String::from("validated image")))
    );
    assert_eq!(owner.active_jobs(), 0);
}

#[test]
fn explicit_cancel_returns_late_output_without_calling_scene_publisher() {
    let scene = binding(1, 4, 1, 3);
    let mut owner = MediaLifecycle::new(scene.clone(), 1).unwrap();
    let token = owner.admit(1_u8, operation(3), scene).unwrap();
    assert!(owner.cancel(&token));
    assert!(!owner.cancel(&token));
    let original = vec![7_u8, 8, 9];
    let address = original.as_ptr();
    let error = owner
        .complete(token, original, |_, _| panic!("cancelled result published"))
        .unwrap_err();
    assert_eq!(error.reason, CompletionRefusal::NotActive);
    assert_eq!(error.output, vec![7, 8, 9]);
    assert_eq!(error.output.as_ptr(), address);
}

#[test]
fn delayed_completion_and_delayed_cancel_cannot_release_reused_job_identity() {
    let scene = binding(1, 4, 1, 3);
    let mut owner = MediaLifecycle::new(scene.clone(), 1).unwrap();
    let old = owner.admit(1_u8, operation(3), scene.clone()).unwrap();
    assert!(owner.cancel(&old));
    let current = owner.admit(1_u8, operation(3), scene).unwrap();
    assert!(!owner.cancel(&old));
    let error = owner
        .complete(old, "obsolete", |_, _| panic!("replaced result published"))
        .unwrap_err();
    assert_eq!(error.reason, CompletionRefusal::ReplacedJob);
    assert_eq!(owner.active_jobs(), 1);
    let mut presented = None;
    owner
        .complete(current, "current", |_, image| presented = Some(image))
        .unwrap();
    assert_eq!(presented, Some("current"));
}

#[test]
fn scene_or_cue_replacement_rejects_queued_completion() {
    let old_scene = binding(1, 4, 1, 3);
    let mut owner = MediaLifecycle::new(old_scene.clone(), 2).unwrap();
    let token = owner.admit(1_u8, operation(3), old_scene.clone()).unwrap();
    let current = binding(1, 5, 2, 4);
    assert_eq!(owner.replace_scene(current.clone()), Ok(1));
    let error = owner
        .complete(token, "old scene", |_, _| panic!("old scene updated"))
        .unwrap_err();
    assert_eq!(error.reason, CompletionRefusal::ObsoleteScene);
    assert_eq!(owner.current(), &current);
    assert_eq!(
        owner.admit(2_u8, operation(4), old_scene).unwrap_err(),
        LifecycleError::ObsoleteScene
    );
}

#[test]
fn recovered_epoch_rejects_old_output_and_revision_regression() {
    let old_scene = binding(1, u64::MAX, 1, 3);
    let mut owner = MediaLifecycle::new(old_scene.clone(), 1).unwrap();
    let token = owner.admit(1_u8, operation(3), old_scene.clone()).unwrap();
    let recovered = binding(2, 0, 1, 3);
    assert_eq!(owner.replace_scene(recovered.clone()), Ok(1));
    assert_eq!(
        owner.replace_scene(old_scene),
        Err(LifecycleError::RevisionNotAdvanced)
    );
    assert_eq!(
        owner.replace_scene(recovered),
        Err(LifecycleError::RevisionNotAdvanced)
    );
    assert_eq!(
        owner
            .complete(token, (), |_, _| panic!("old epoch updated"))
            .unwrap_err()
            .reason,
        CompletionRefusal::ObsoleteScene
    );
}

#[test]
fn foreign_owner_token_cannot_match_a_recreated_owner() {
    let scene = binding(1, 4, 1, 3);
    let mut old_owner = MediaLifecycle::new(scene.clone(), 1).unwrap();
    let old = old_owner.admit(1_u8, operation(3), scene.clone()).unwrap();
    drop(old_owner);
    let mut owner = MediaLifecycle::new(scene.clone(), 1).unwrap();
    let current = owner.admit(1_u8, operation(3), scene).unwrap();
    assert!(!owner.cancel(&old));
    assert_eq!(
        owner
            .complete(old, "foreign", |_, _| panic!("foreign result updated"))
            .unwrap_err()
            .reason,
        CompletionRefusal::ForeignOwner
    );
    assert_eq!(owner.active_jobs(), 1);
    owner.complete(current, (), |_, _| {}).unwrap();
}

#[test]
fn bounded_admission_refuses_duplicate_and_overflow_without_losing_active_work() {
    let scene = binding(1, 4, 1, 3);
    assert_eq!(
        MediaLifecycle::<u8, u8, u8>::new(scene.clone(), 0).unwrap_err(),
        LifecycleError::ZeroCapacity
    );
    let mut owner = MediaLifecycle::new(scene.clone(), 1).unwrap();
    let token = owner.admit(1_u8, operation(3), scene.clone()).unwrap();
    assert_eq!(
        owner.admit(1_u8, operation(4), scene.clone()).unwrap_err(),
        LifecycleError::AlreadyActive
    );
    assert_eq!(
        owner.admit(2_u8, operation(4), scene).unwrap_err(),
        LifecycleError::Capacity
    );
    assert_eq!(owner.active_jobs(), 1);
    owner.complete(token, (), |_, _| {}).unwrap();
}

#[test]
fn a_completion_already_queued_before_cancel_cannot_update_the_scene() {
    use std::sync::mpsc;
    use std::thread;

    let scene = binding(1, 4, 1, 3);
    let mut owner = MediaLifecycle::new(scene.clone(), 1).unwrap();
    let token = owner.admit(1_u8, operation(3), scene).unwrap();
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::scope(|scope| {
        let producer = scope.spawn(move || {
            sender
                .send((token, String::from("queued provider result")))
                .unwrap();
        });
        producer.join().unwrap();
        let (queued_token, output) = receiver.recv().unwrap();
        assert!(owner.cancel(&queued_token));
        let mut current_image = String::from("valid current image");
        let error = owner
            .complete(queued_token, output, |_, image| current_image = image)
            .unwrap_err();
        assert_eq!(error.reason, CompletionRefusal::NotActive);
        assert_eq!(error.output, "queued provider result");
        assert_eq!(current_image, "valid current image");
    });
}

#[test]
fn admitting_another_job_in_the_same_state_does_not_cancel_existing_work() {
    let scene = binding(1, 4, 1, 3);
    let mut owner = MediaLifecycle::new(scene.clone(), 2).unwrap();
    let first = owner.admit(1_u8, operation(3), scene.clone()).unwrap();
    let second = owner.admit(2_u8, operation(4), scene).unwrap();
    let mut published = Vec::new();
    owner
        .complete(second, "second", |operation_id, image| {
            published.push((operation_id, image))
        })
        .unwrap();
    assert_eq!(owner.active_jobs(), 1);
    owner
        .complete(first, "first", |operation_id, image| {
            published.push((operation_id, image))
        })
        .unwrap();
    assert_eq!(
        published,
        vec![(operation(4), "second"), (operation(3), "first")]
    );
    assert_eq!(owner.active_jobs(), 0);
}

#[test]
fn another_session_run_or_cue_cannot_admit_into_this_owner() {
    let scene = binding(1, 4, 1, 3);
    let mut owner = MediaLifecycle::new(scene.clone(), 1).unwrap();
    let mut other_session = scene.clone();
    other_session.session = SessionId::from_bytes(&[9; 16]).unwrap();
    let mut other_run = scene.clone();
    other_run.run = RunId::from_bytes(&[9; 16]).unwrap();
    let mut other_cue = scene.clone();
    other_cue.cue = 9;
    for other in [other_session, other_run, other_cue] {
        assert_eq!(
            owner.admit(1_u8, operation(3), other).unwrap_err(),
            LifecycleError::ObsoleteScene
        );
    }
    assert_eq!(owner.active_jobs(), 0);
    let accepted = owner.admit(1_u8, operation(3), scene).unwrap();
    owner.complete(accepted, (), |_, _| {}).unwrap();
}
