use df_session::inbox::{ActorInput, AdmissionSequence, InboxHandle, Reducer, bounded_inbox};
use std::sync::{Arc, Barrier};
use std::time::{Duration, Instant};

struct Input(u8);
impl ActorInput for Input {
    fn retained_heap_bytes(&self) -> Option<usize> {
        Some(0)
    }
}
struct Owner {
    sender: InboxHandle<Input>,
    deadline: Option<Instant>,
    inputs: Vec<(AdmissionSequence, u8)>,
    wakes: usize,
}
impl Reducer<Input> for Owner {
    fn reduce(&mut self, sequence: AdmissionSequence, input: Input) {
        self.inputs.push((sequence, input.0));
    }
}

#[test]
fn idle_owner_wake_runs_once_outside_lock_and_preserves_admission_order() {
    let (sender, actor) = bounded_inbox();
    assert!(sender.try_submit(Input(1)).is_ok());
    assert!(sender.try_submit(Input(2)).is_ok());
    let mut owner = Owner {
        sender,
        deadline: Some(Instant::now()),
        inputs: vec![],
        wakes: 0,
    };
    let outcome = actor
        .run_with_owner_wake(
            &mut owner,
            |owner| {
                // Both deadline and wake callbacks can acquire the producer lock.
                assert!(owner.sender.usage().is_ok());
                owner.deadline
            },
            |owner| {
                owner.wakes += 1;
                owner.deadline = None;
                assert!(owner.sender.try_submit(Input(3)).is_ok());
                owner.sender.stop().unwrap();
            },
        )
        .unwrap();
    assert_eq!(owner.wakes, 1);
    assert_eq!(
        owner.inputs,
        vec![
            (AdmissionSequence(1), 1),
            (AdmissionSequence(2), 2),
            (AdmissionSequence(3), 3)
        ]
    );
    assert_eq!(outcome.reduced_inputs, 3);
}

#[test]
fn early_input_notification_does_not_run_future_owner_work() {
    let (sender, actor) = bounded_inbox();
    let entered = Arc::new(Barrier::new(2));
    let continue_owner = Arc::new(Barrier::new(2));
    let (first, second) = (entered.clone(), continue_owner.clone());
    let producer = sender.clone();
    let thread = std::thread::spawn(move || {
        let mut owner = Owner {
            sender,
            deadline: Some(Instant::now() + Duration::from_secs(3600)),
            inputs: vec![],
            wakes: 0,
        };
        let signaled = std::sync::atomic::AtomicBool::new(false);
        actor
            .run_with_owner_wake(
                &mut owner,
                |owner| {
                    if !signaled.swap(true, std::sync::atomic::Ordering::SeqCst) {
                        first.wait();
                        second.wait();
                    }
                    owner.deadline
                },
                |_| panic!("future deadline cannot run early"),
            )
            .unwrap();
        owner
    });
    // Submit and stop are both ordinary producer notifications before the due time.
    entered.wait();
    assert!(producer.try_submit(Input(1)).is_ok());
    producer.stop().unwrap();
    continue_owner.wait();
    let owner = thread.join().unwrap();
    assert_eq!(owner.inputs, vec![(AdmissionSequence(1), 1)]);
    assert_eq!(owner.wakes, 0);
}

#[test]
fn stop_between_deadline_callback_and_wait_cancels_due_work_and_drains_inputs() {
    let (sender, actor) = bounded_inbox();
    assert!(sender.try_submit(Input(1)).is_ok());
    let entered = Arc::new(Barrier::new(2));
    let released = Arc::new(Barrier::new(2));
    let (owner_entered, owner_released) = (entered.clone(), released.clone());
    let producer = sender.clone();
    let thread = std::thread::spawn(move || {
        let mut owner = Owner {
            sender,
            deadline: Some(Instant::now()),
            inputs: vec![],
            wakes: 0,
        };
        let outcome = actor
            .run_with_owner_wake(
                &mut owner,
                |owner| {
                    if owner.inputs.is_empty() {
                        owner_entered.wait();
                        owner_released.wait();
                    }
                    owner.deadline
                },
                |_| panic!("closed ingress cancels due owner work"),
            )
            .unwrap();
        (owner, outcome)
    });
    entered.wait();
    producer.stop().unwrap();
    released.wait();
    let (owner, outcome) = thread.join().unwrap();
    assert_eq!(owner.inputs, vec![(AdmissionSequence(1), 1)]);
    assert_eq!(outcome.reduced_inputs, 1);
    assert!(!producer.usage().unwrap().accepting);
}
