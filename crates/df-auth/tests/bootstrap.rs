use df_auth::bootstrap::{
    BootstrapAttemptOutcome, BootstrapError, BootstrapObserver, BootstrapPolicy,
    BootstrapStoreOutcome, GuestBootstrapStore, InvalidBootstrapLimit, MAX_BOOTSTRAP_ATTEMPTS,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Barrier, Mutex};

// Deliberately test-only caller-owned values, not production Principal or credentials.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Allocation {
    principal: u64,
    credential: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StoreFailure {
    TemporarilyUnavailable,
    CommitAcknowledgementLost,
    Capacity,
}

#[derive(Clone)]
enum Stored {
    Committed {
        fingerprint: u8,
        allocation: Allocation,
    },
    Retired,
}

#[derive(Default)]
struct AtomicOwner {
    results: BTreeMap<(u8, u8), Stored>,
    allocations: u64,
    calls: Vec<(u8, u8, u8)>,
    unavailable_calls: usize,
    lose_next_acknowledgement: bool,
    denied: bool,
}

#[derive(Clone, Default)]
struct Store(Arc<Mutex<AtomicOwner>>);

impl GuestBootstrapStore for Store {
    type Scope = u8;
    type Key = u8;
    type Fingerprint = u8;
    type Allocation = Allocation;
    type Error = StoreFailure;

    fn allocate_or_replay(
        &mut self,
        scope: &Self::Scope,
        key: &Self::Key,
        fingerprint: &Self::Fingerprint,
    ) -> BootstrapStoreOutcome<Self::Allocation, Self::Error> {
        // This fixture's one mutex serializes the actual compare, allocation and
        // result insertion. It proves policy composition, not PostgreSQL durability.
        let mut owner = self.0.lock().unwrap();
        owner.calls.push((*scope, *key, *fingerprint));
        if owner.denied {
            return BootstrapStoreOutcome::Denied;
        }
        if owner.unavailable_calls > 0 {
            owner.unavailable_calls -= 1;
            return BootstrapStoreOutcome::Retryable(StoreFailure::TemporarilyUnavailable);
        }
        if let Some(stored) = owner.results.get(&(*scope, *key)) {
            return match stored {
                Stored::Retired => BootstrapStoreOutcome::Expired,
                Stored::Committed {
                    fingerprint: existing,
                    allocation,
                } => {
                    if existing != fingerprint {
                        BootstrapStoreOutcome::Conflict
                    } else {
                        BootstrapStoreOutcome::Committed {
                            allocation: allocation.clone(),
                            replayed: true,
                        }
                    }
                }
            };
        }
        if owner.results.len() == 8 {
            return BootstrapStoreOutcome::Retryable(StoreFailure::Capacity);
        }
        owner.allocations += 1;
        let allocation = Allocation {
            principal: owner.allocations,
            credential: 1_000 + owner.allocations,
        };
        owner.results.insert(
            (*scope, *key),
            Stored::Committed {
                fingerprint: *fingerprint,
                allocation: allocation.clone(),
            },
        );
        if owner.lose_next_acknowledgement {
            owner.lose_next_acknowledgement = false;
            return BootstrapStoreOutcome::Indeterminate(StoreFailure::CommitAcknowledgementLost);
        }
        BootstrapStoreOutcome::Committed {
            allocation,
            replayed: false,
        }
    }
}

#[derive(Default)]
struct Observer {
    outcomes: Vec<(u8, BootstrapAttemptOutcome)>,
}
impl BootstrapObserver for Observer {
    fn attempt_finished(&mut self, attempt: u8, outcome: BootstrapAttemptOutcome) {
        self.outcomes.push((attempt, outcome));
    }
}

#[test]
fn repeated_matching_bootstrap_returns_one_principal_and_credential_allocation() {
    let policy = BootstrapPolicy::new(3).unwrap();
    let mut store = Store::default();
    let first = policy
        .begin_guest(&mut store, &1, &2, &3, &mut Observer::default())
        .unwrap();
    let second = policy
        .begin_guest(&mut store, &1, &2, &3, &mut Observer::default())
        .unwrap();
    assert!(!first.replayed);
    assert!(second.replayed);
    assert_eq!(first.allocation, second.allocation);
    assert_eq!(first.attempts, 1);
    assert_eq!(store.0.lock().unwrap().allocations, 1);
}

#[test]
fn lost_commit_acknowledgement_stops_then_explicit_same_key_retry_recovers_result() {
    let policy = BootstrapPolicy::new(16).unwrap();
    let mut store = Store::default();
    store.0.lock().unwrap().lose_next_acknowledgement = true;
    assert!(matches!(
        policy.begin_guest(&mut store, &1, &2, &3, &mut Observer::default()),
        Err(BootstrapError::Indeterminate(
            StoreFailure::CommitAcknowledgementLost
        ))
    ));
    assert_eq!(store.0.lock().unwrap().calls.len(), 1);
    let retry = policy
        .begin_guest(&mut store, &1, &2, &3, &mut Observer::default())
        .unwrap();
    assert!(retry.replayed);
    assert_eq!(retry.allocation.principal, 1);
    assert_eq!(retry.allocation.credential, 1_001);
    assert_eq!(store.0.lock().unwrap().allocations, 1);
}

#[test]
fn changed_fingerprint_conflicts_without_allocating_or_retrying() {
    let policy = BootstrapPolicy::new(16).unwrap();
    let mut store = Store::default();
    policy
        .begin_guest(&mut store, &1, &2, &3, &mut Observer::default())
        .unwrap();
    assert!(matches!(
        policy.begin_guest(&mut store, &1, &2, &4, &mut Observer::default()),
        Err(BootstrapError::Conflict)
    ));
    let owner = store.0.lock().unwrap();
    assert_eq!(owner.allocations, 1);
    assert_eq!(owner.calls.len(), 2);
}

#[test]
fn retired_key_never_reallocates_when_full_result_is_gone() {
    let policy = BootstrapPolicy::new(16).unwrap();
    let mut store = Store::default();
    policy
        .begin_guest(&mut store, &1, &2, &3, &mut Observer::default())
        .unwrap();
    store
        .0
        .lock()
        .unwrap()
        .results
        .insert((1, 2), Stored::Retired);
    assert!(matches!(
        policy.begin_guest(&mut store, &1, &2, &3, &mut Observer::default()),
        Err(BootstrapError::Expired)
    ));
    let owner = store.0.lock().unwrap();
    assert_eq!(owner.allocations, 1);
    assert_eq!(owner.calls.len(), 2);
}

#[test]
fn known_safe_failures_preserve_scope_key_and_fingerprint_through_exact_bound() {
    for maximum_attempts in 1..=MAX_BOOTSTRAP_ATTEMPTS {
        let policy = BootstrapPolicy::new(maximum_attempts).unwrap();
        let mut store = Store::default();
        store.0.lock().unwrap().unavailable_calls = usize::from(maximum_attempts);
        assert!(matches!(
            policy.begin_guest(&mut store, &9, &8, &7, &mut Observer::default()),
            Err(BootstrapError::Unavailable(
                StoreFailure::TemporarilyUnavailable
            ))
        ));
        let owner = store.0.lock().unwrap();
        assert_eq!(owner.calls, vec![(9, 8, 7); usize::from(maximum_attempts)]);
        assert_eq!(owner.allocations, 0);
    }
}

#[test]
fn temporary_refusal_can_commit_on_last_permitted_attempt() {
    let policy = BootstrapPolicy::new(3).unwrap();
    let mut store = Store::default();
    store.0.lock().unwrap().unavailable_calls = 2;
    let result = policy
        .begin_guest(&mut store, &1, &2, &3, &mut Observer::default())
        .unwrap();
    assert_eq!(result.attempts, 3);
    assert!(!result.replayed);
    assert_eq!(store.0.lock().unwrap().allocations, 1);
}

#[test]
fn scoped_keys_do_not_merge_different_bootstrap_namespaces() {
    let policy = BootstrapPolicy::new(2).unwrap();
    let mut store = Store::default();
    let first = policy
        .begin_guest(&mut store, &1, &2, &3, &mut Observer::default())
        .unwrap();
    let second = policy
        .begin_guest(&mut store, &4, &2, &3, &mut Observer::default())
        .unwrap();
    assert_ne!(first.allocation, second.allocation);
    assert_eq!(store.0.lock().unwrap().allocations, 2);
}

#[test]
fn concurrent_same_key_callers_receive_one_atomic_allocation() {
    let store = Store::default();
    let barrier = Arc::new(Barrier::new(8));
    let mut workers = Vec::new();
    for _ in 0..8 {
        let mut caller_store = store.clone();
        let start = Arc::clone(&barrier);
        workers.push(std::thread::spawn(move || {
            start.wait();
            BootstrapPolicy::new(3)
                .unwrap()
                .begin_guest(&mut caller_store, &1, &2, &3, &mut Observer::default())
                .unwrap()
        }));
    }
    let receipts: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(
        receipts.iter().filter(|receipt| !receipt.replayed).count(),
        1
    );
    assert!(
        receipts
            .iter()
            .all(|receipt| receipt.allocation == receipts[0].allocation)
    );
    assert_eq!(store.0.lock().unwrap().allocations, 1);
}

#[test]
fn denial_and_capacity_never_manufacture_a_successful_receipt() {
    let policy = BootstrapPolicy::new(3).unwrap();
    let mut store = Store::default();
    store.0.lock().unwrap().denied = true;
    assert!(matches!(
        policy.begin_guest(&mut store, &1, &2, &3, &mut Observer::default()),
        Err(BootstrapError::Denied)
    ));
    assert_eq!(store.0.lock().unwrap().calls.len(), 1);
    store.0.lock().unwrap().denied = false;
    for key in 0..8 {
        policy
            .begin_guest(&mut store, &1, &key, &3, &mut Observer::default())
            .unwrap();
    }
    assert!(matches!(
        policy.begin_guest(&mut store, &1, &9, &3, &mut Observer::default()),
        Err(BootstrapError::Unavailable(StoreFailure::Capacity))
    ));
    assert_eq!(store.0.lock().unwrap().allocations, 8);
    // Retention pressure never evicts an old key and makes its retry look fresh.
    let replay = policy
        .begin_guest(&mut store, &1, &0, &3, &mut Observer::default())
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(store.0.lock().unwrap().allocations, 8);
}

#[test]
fn policy_rejects_zero_or_excessive_attempts_before_store_invocation() {
    assert_eq!(BootstrapPolicy::new(0), Err(InvalidBootstrapLimit));
    for maximum in (MAX_BOOTSTRAP_ATTEMPTS + 1)..=u8::MAX {
        assert_eq!(BootstrapPolicy::new(maximum), Err(InvalidBootstrapLimit));
    }
}

#[test]
fn caller_observer_receives_only_actual_attempt_counts_and_safe_classifications() {
    let policy = BootstrapPolicy::new(3).unwrap();
    let mut store = Store::default();
    let mut observer = Observer::default();
    store.0.lock().unwrap().unavailable_calls = 2;
    let receipt = policy
        .begin_guest(&mut store, &1, &2, &3, &mut observer)
        .unwrap();
    assert_eq!(receipt.attempts, 3);
    assert_eq!(
        observer.outcomes,
        vec![
            (1, BootstrapAttemptOutcome::Unavailable),
            (2, BootstrapAttemptOutcome::Unavailable),
            (3, BootstrapAttemptOutcome::Committed),
        ]
    );
    observer.outcomes.clear();
    policy
        .begin_guest(&mut store, &1, &2, &3, &mut observer)
        .unwrap();
    assert_eq!(
        observer.outcomes,
        vec![(1, BootstrapAttemptOutcome::Replayed)]
    );
    observer.outcomes.clear();
    assert!(matches!(
        policy.begin_guest(&mut store, &1, &2, &4, &mut observer),
        Err(BootstrapError::Conflict)
    ));
    assert_eq!(
        observer.outcomes,
        vec![(1, BootstrapAttemptOutcome::Conflict)]
    );
}
