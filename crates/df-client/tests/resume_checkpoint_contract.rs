use std::{cell::Cell, rc::Rc};

use df_client::{
    connection::RpcConnection,
    connection_views::{ConnectionViewAcceptance, ConnectionViews},
    revisions::ViewAcceptance,
};
use df_types::{ClientBindingId, RecoveryEpoch, SessionRevision};

struct CompleteView {
    label: &'static str,
    drops: Rc<Cell<usize>>,
}

impl Drop for CompleteView {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

fn binding(value: u8) -> ClientBindingId {
    ClientBindingId::from_bytes(&[value; 16]).expect("nonzero test binding")
}

fn revision(epoch: u64, sequence: u64) -> SessionRevision {
    SessionRevision::new(
        RecoveryEpoch::new(epoch).expect("nonzero test recovery epoch"),
        sequence,
    )
}

fn complete_view(label: &'static str, drops: &Rc<Cell<usize>>) -> CompleteView {
    CompleteView {
        label,
        drops: Rc::clone(drops),
    }
}

#[test]
fn resume_gap_uses_complete_snapshot_and_keeps_current_generation_and_revision() {
    let drops = Rc::new(Cell::new(0));
    let member_binding = binding(1);
    let old_connection = RpcConnection::new(());
    let old_generation = old_connection.generation();
    let mut views = ConnectionViews::new(member_binding, old_generation.clone());

    assert_eq!(
        views.accept(
            &old_generation,
            member_binding,
            revision(4, 7),
            complete_view("before disconnect", &drops),
        ),
        ConnectionViewAcceptance::View(ViewAcceptance::Applied)
    );

    old_connection.close();
    let new_connection = RpcConnection::new(());
    let new_generation = new_connection.generation();
    assert!(views.reconnect(new_generation.clone()));

    assert_eq!(
        views.accept(
            &old_generation,
            member_binding,
            revision(4, 99),
            complete_view("retired connection", &drops),
        ),
        ConnectionViewAcceptance::ObsoleteGeneration
    );
    assert_eq!(drops.get(), 1);
    assert_eq!(
        views.current().map(|(current, view)| (current, view.label)),
        Some((revision(4, 7), "before disconnect"))
    );

    // The protocol currently sends complete snapshots, so a revision gap is
    // applied as a complete replacement without requiring a delta base.
    assert_eq!(
        views.accept(
            &new_generation,
            member_binding,
            revision(4, 100),
            complete_view("complete gap snapshot", &drops),
        ),
        ConnectionViewAcceptance::View(ViewAcceptance::Applied)
    );
    assert_eq!(drops.get(), 2);
    assert_eq!(
        views.current().map(|(current, view)| (current, view.label)),
        Some((revision(4, 100), "complete gap snapshot"))
    );

    assert_eq!(
        views.accept(
            &new_generation,
            member_binding,
            revision(4, 99),
            complete_view("stale snapshot", &drops),
        ),
        ConnectionViewAcceptance::View(ViewAcceptance::Stale {
            current: revision(4, 100),
        })
    );
    assert_eq!(
        views.accept(
            &new_generation,
            member_binding,
            revision(4, 100),
            complete_view("conflicting duplicate", &drops),
        ),
        ConnectionViewAcceptance::View(ViewAcceptance::Duplicate {
            current: revision(4, 100),
        })
    );
    assert_eq!(drops.get(), 4);
    assert_eq!(
        views.current().map(|(current, view)| (current, view.label)),
        Some((revision(4, 100), "complete gap snapshot"))
    );

    assert_eq!(
        views.accept(
            &new_generation,
            member_binding,
            revision(5, 0),
            complete_view("new recovery epoch", &drops),
        ),
        ConnectionViewAcceptance::View(ViewAcceptance::Applied)
    );
    assert_eq!(
        views.accept(
            &new_generation,
            member_binding,
            revision(4, u64::MAX),
            complete_view("older recovery epoch", &drops),
        ),
        ConnectionViewAcceptance::View(ViewAcceptance::Stale {
            current: revision(5, 0),
        })
    );
    assert_eq!(drops.get(), 6);
    assert_eq!(
        views.current().map(|(current, view)| (current, view.label)),
        Some((revision(5, 0), "new recovery epoch"))
    );
}
