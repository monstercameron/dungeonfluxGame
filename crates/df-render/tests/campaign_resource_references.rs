#![cfg(not(target_arch = "wasm32"))]
use df_render::{
    DecodeBudget, RendererResource, ResourceCache, ResourceError, ResourceLimits, WorkOutcome,
};
use df_types::{ClientBindingId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision};
use std::{cell::Cell, rc::Rc};
struct Surface(u8);
impl RendererResource for Surface {
    type Key = u8;
    fn key(&self) -> &u8 {
        &self.0
    }
    fn decoded_bytes(&self) -> usize {
        4
    }
    fn is_current(&self) -> bool {
        true
    }
    fn retire(self) {}
}
fn owner() -> df_render::SceneOwner {
    (
        SessionId::from_bytes(&[1; 16]).unwrap(),
        RunId::from_bytes(&[2; 16]).unwrap(),
        ClientBindingId::from_bytes(&[3; 16]).unwrap(),
    )
}
fn rev(epoch: u64, seq: u64) -> SessionRevision {
    SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), seq)
}
fn cache() -> ResourceCache<Surface> {
    ResourceCache::new(
        owner(),
        ResourceLimits {
            max_references: 2,
            max_resident: 1,
            max_pending: 1,
            max_decoded_bytes: 4,
            max_work_bytes: 8,
        },
        RevisionLabel::new(Some("fixture-codec-v1")).unwrap(),
    )
    .unwrap()
}
fn budget() -> DecodeBudget {
    DecodeBudget {
        decoded_bytes: 4,
        work_bytes: 8,
    }
}

#[test]
fn illustration_admission_needs_no_scene_label_and_preserves_same_epoch_work() {
    let mut cache = cache();
    cache.apply_current(owner(), rev(1, 0), &[1]).unwrap();
    let aborts = Rc::new(Cell::new(0));
    let counter = aborts.clone();
    let token = cache
        .begin(&1, budget(), move || counter.set(counter.get() + 1))
        .unwrap();
    cache.apply_current(owner(), rev(1, 1), &[1]).unwrap();
    assert_eq!(aborts.get(), 0);
    assert_eq!(cache.work_bytes(), 8);
    cache.complete(&token, Surface(1)).unwrap();
    assert_eq!(cache.decoded_bytes(), 4);
    cache.apply_current(owner(), rev(1, 2), &[1]).unwrap();
    assert!(cache.get(&1).unwrap().is_some());
}
#[test]
fn same_key_epoch_and_a_b_a_do_not_release_cancelled_reservations_before_terminal() {
    for next_epoch in [false, true] {
        let mut cache = cache();
        cache.apply_current(owner(), rev(1, 0), &[1]).unwrap();
        let aborts = Rc::new(Cell::new(0));
        let counter = aborts.clone();
        let old = cache
            .begin(&1, budget(), move || counter.set(counter.get() + 1))
            .unwrap();
        if next_epoch {
            cache.apply_current(owner(), rev(2, 0), &[1]).unwrap();
        } else {
            cache.apply_current(owner(), rev(1, 1), &[2]).unwrap();
            cache.apply_current(owner(), rev(1, 2), &[1]).unwrap();
        }
        assert_eq!(aborts.get(), 1);
        assert_eq!(cache.work_bytes(), 8);
        assert!(matches!(
            cache.begin(&1, budget(), || {}),
            Err(ResourceError::AlreadyPending)
        ));
        assert_eq!(cache.finish_failed(&old), Ok(WorkOutcome::Cancelled));
        assert_eq!(cache.work_bytes(), 0);
        let new = cache.begin(&1, budget(), || {}).unwrap();
        assert_eq!(cache.finish_failed(&old), Err(ResourceError::StaleDecode));
        assert_eq!(cache.work_bytes(), 8);
        cache.complete(&new, Surface(1)).unwrap();
        assert_eq!(cache.decoded_bytes(), 4);
    }
}
#[test]
fn stale_duplicate_and_foreign_illustration_reference_admission_does_not_mutate_live_work() {
    let mut cache = cache();
    cache.apply_current(owner(), rev(1, 3), &[1]).unwrap();
    let token = cache.begin(&1, budget(), || {}).unwrap();
    assert_eq!(
        cache.apply_current(owner(), rev(1, 3), &[2]),
        Err(ResourceError::StaleRevision)
    );
    assert_eq!(
        cache.apply_current(owner(), rev(1, 4), &[2, 2]),
        Err(ResourceError::DuplicateReference)
    );
    let wrong = (
        owner().0,
        owner().1,
        ClientBindingId::from_bytes(&[9; 16]).unwrap(),
    );
    assert_eq!(
        cache.apply_current(wrong, rev(1, 4), &[2]),
        Err(ResourceError::WrongOwner)
    );
    assert_eq!(cache.work_bytes(), 8);
    cache.complete(&token, Surface(1)).unwrap();
    assert!(cache.get(&1).unwrap().is_some());
}
