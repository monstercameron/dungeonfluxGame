#![cfg(not(target_arch = "wasm32"))]

#[path = "../src/gameplay_browser/view_delivery.rs"]
mod view_delivery;

use df_client::{
    connection::RpcConnection, connection_views::ConnectionViewAcceptance,
    revisions::ViewAcceptance,
};
use df_protocol::common as rpc;
use df_types::{RecoveryEpoch, SessionRevision};
use prost::Message;
use view_delivery::{GameplayViews, ViewDeliveryError, ViewRole, ViewScope};

fn scope(session: u8, run: u8, binding: u8, role: ViewRole) -> ViewScope {
    ViewScope::from_wire(
        Some(&rpc::SessionId {
            value: Some(vec![session; 16]),
        }),
        Some(&rpc::RunId {
            value: Some(vec![run; 16]),
        }),
        Some(&rpc::ClientBindingId {
            value: Some(vec![binding; 16]),
        }),
        role,
    )
    .expect("nonzero fixture scope")
}

fn revision(epoch: u64, sequence: u64) -> SessionRevision {
    SessionRevision::new(RecoveryEpoch::new(epoch).unwrap(), sequence)
}

fn frame(epoch: u64, sequence: u64, narration: &str) -> rpc::ViewMessage {
    let message = rpc::ViewMessage {
        revision: Some(rpc::SessionRevision {
            epoch: Some(rpc::RecoveryEpoch { value: Some(epoch) }),
            sequence: Some(sequence),
        }),
        audience: Some(rpc::view_message::Audience::Player(
            rpc::PlayerGameplayView {
                narration: narration.to_owned(),
                scene: Some(rpc::GameplayScene {
                    scene_asset: "assets/ui/scenes/mara-harbor-v4.png".to_owned(),
                    ..Default::default()
                }),
                journey: Some(rpc::JourneyView {
                    phase: rpc::JourneyPhase::Opening as i32,
                    ..Default::default()
                }),
                ..Default::default()
            },
        )),
    };
    rpc::ViewMessage::decode(message.encode_to_vec().as_slice()).unwrap()
}

#[test]
fn replaced_live_generation_and_disposed_owner_cannot_deliver_queued_newer_frames() {
    let scope = scope(1, 2, 3, ViewRole::Player);
    let old = RpcConnection::new(());
    let old_generation = old.generation();
    let mut views = GameplayViews::new(scope, old_generation.clone());
    let initial = frame(2, 7, "retained current view");
    assert_eq!(
        views.accept(&old_generation, scope, initial.clone()),
        Ok(ConnectionViewAcceptance::View(ViewAcceptance::Applied))
    );

    let replacement = RpcConnection::new(());
    let current_generation = replacement.generation();
    assert!(views.reconnect(current_generation.clone()));
    assert!(old_generation.is_active());
    assert_eq!(
        views.accept(
            &old_generation,
            scope,
            frame(9, 99, "late retired callback")
        ),
        Ok(ConnectionViewAcceptance::ObsoleteGeneration)
    );
    assert_eq!(views.current(), Some(&initial));

    let current = frame(2, 8, "replacement view");
    assert_eq!(
        views.accept(&current_generation, scope, current.clone()),
        Ok(ConnectionViewAcceptance::View(ViewAcceptance::Applied))
    );
    drop(replacement);
    assert!(!current_generation.is_active());
    assert_eq!(
        views.accept(
            &current_generation,
            scope,
            frame(9, 999, "queued after disposal")
        ),
        Ok(ConnectionViewAcceptance::ObsoleteGeneration)
    );
    assert!(!views.reconnect(current_generation));
    assert_eq!(views.current(), Some(&current));
}

#[test]
fn reconnect_retains_wire_view_and_durable_recovery_watermark() {
    let scope = scope(1, 2, 3, ViewRole::Player);
    let old = RpcConnection::new(());
    let mut views = GameplayViews::new(scope, old.generation());
    assert_eq!(
        views.accept(&old.generation(), scope, frame(2, 99, "before recovery")),
        Ok(ConnectionViewAcceptance::View(ViewAcceptance::Applied))
    );
    drop(old);
    let owner = RpcConnection::new(());
    let generation = owner.generation();
    assert!(views.reconnect(generation.clone()));
    let recovered = frame(3, 0, "current recovered view");
    assert_eq!(
        views.accept(&generation, scope, recovered.clone()),
        Ok(ConnectionViewAcceptance::View(ViewAcceptance::Applied))
    );
    for stale in [
        frame(2, u64::MAX, "old epoch"),
        frame(1, u64::MAX, "older epoch"),
    ] {
        assert_eq!(
            views.accept(&generation, scope, stale),
            Ok(ConnectionViewAcceptance::View(ViewAcceptance::Stale {
                current: revision(3, 0),
            }))
        );
        assert_eq!(views.current(), Some(&recovered));
    }
    assert_eq!(
        views.accept(&generation, scope, frame(3, 0, "changed duplicate")),
        Ok(ConnectionViewAcceptance::View(ViewAcceptance::Duplicate {
            current: revision(3, 0),
        }))
    );
    // Receipt/busy redraws borrow this exact retained wire revision for next input.
    assert_eq!(views.current(), Some(&recovered));
}

#[test]
fn cross_scope_and_invalid_wire_frames_cannot_seed_or_replace_private_view() {
    let permitted = scope(1, 2, 3, ViewRole::Player);
    let owner = RpcConnection::new(());
    let generation = owner.generation();
    let mut views = GameplayViews::new(permitted, generation.clone());
    for other in [
        scope(4, 2, 3, ViewRole::Player),
        scope(1, 4, 3, ViewRole::Player),
        scope(1, 2, 4, ViewRole::Player),
        scope(1, 2, 3, ViewRole::Display),
    ] {
        assert_eq!(
            views.accept(&generation, other, frame(9, 99, "other scope")),
            Err(ViewDeliveryError::WrongScope)
        );
        assert_eq!(views.current(), None);
    }
    let current = frame(3, 8, "permitted private view");
    assert_eq!(
        views.accept(&generation, permitted, current.clone()),
        Ok(ConnectionViewAcceptance::View(ViewAcceptance::Applied))
    );
    assert_eq!(
        views.accept(&generation, permitted, frame(3, 7, "old same-epoch frame")),
        Ok(ConnectionViewAcceptance::View(ViewAcceptance::Stale {
            current: revision(3, 8),
        }))
    );
    assert_eq!(
        views.accept(
            &generation,
            scope(1, 2, 4, ViewRole::Player),
            frame(9, 99, "wrong binding")
        ),
        Err(ViewDeliveryError::WrongScope)
    );
    assert_eq!(views.current(), Some(&current));
    let mut absent_revision = frame(9, 99, "missing revision");
    absent_revision.revision = None;
    let mut absent_sequence = frame(9, 99, "missing sequence");
    absent_sequence.revision.as_mut().unwrap().sequence = None;
    let mut absent_epoch = frame(9, 99, "missing epoch");
    absent_epoch.revision.as_mut().unwrap().epoch = None;
    for invalid in [
        absent_revision,
        absent_sequence,
        absent_epoch,
        frame(0, 99, "zero epoch"),
    ] {
        assert_eq!(
            views.accept(&generation, permitted, invalid),
            Err(ViewDeliveryError::InvalidRevision)
        );
        assert_eq!(views.current(), Some(&current));
    }
    let mut wrong_audience = frame(9, 99, "wrong audience");
    wrong_audience.audience = Some(rpc::view_message::Audience::Display(Default::default()));
    assert_eq!(
        views.accept(&generation, permitted, wrong_audience),
        Err(ViewDeliveryError::WrongAudience)
    );
    assert_eq!(views.current(), Some(&current));
    let mut missing_journey = frame(9, 99, "incomplete projection");
    if let Some(rpc::view_message::Audience::Player(player)) = missing_journey.audience.as_mut() {
        player.journey = None;
    }
    assert_eq!(
        views.accept(&generation, permitted, missing_journey),
        Err(ViewDeliveryError::InvalidProjection)
    );
    assert_eq!(views.current(), Some(&current));
}

#[test]
fn display_projection_and_scope_use_the_same_generated_contract() {
    let scope = scope(1, 2, 3, ViewRole::Display);
    let owner = RpcConnection::new(());
    let generation = owner.generation();
    let mut views = GameplayViews::new(scope, generation.clone());
    let mut display = frame(1, 0, "public display");
    display.audience = Some(rpc::view_message::Audience::Display(
        rpc::DisplayGameplayView {
            journey: Some(rpc::JourneyView::default()),
            narration: "public display".to_owned(),
            ..Default::default()
        },
    ));
    assert_eq!(
        views.accept(&generation, scope, display.clone()),
        Ok(ConnectionViewAcceptance::View(ViewAcceptance::Applied))
    );
    assert_eq!(views.current(), Some(&display));
    assert_eq!(
        ViewScope::from_wire(None, None, None, ViewRole::Display),
        Err(ViewDeliveryError::InvalidScope)
    );
}

#[test]
fn replacing_authorized_scope_starts_with_no_previous_private_view() {
    let old_scope = scope(1, 2, 3, ViewRole::Player);
    let old = RpcConnection::new(());
    let old_generation = old.generation();
    let mut views = GameplayViews::new(old_scope, old_generation.clone());
    assert_eq!(
        views.accept(&old_generation, old_scope, frame(9, 99, "old private view")),
        Ok(ConnectionViewAcceptance::View(ViewAcceptance::Applied))
    );
    drop(old);
    let new_scope = scope(4, 5, 6, ViewRole::Player);
    let new = RpcConnection::new(());
    views = GameplayViews::new(new_scope, new.generation());
    assert_eq!(views.scope(), new_scope);
    assert_eq!(views.current(), None);
    assert_eq!(
        views.accept(
            &old_generation,
            old_scope,
            frame(9, 100, "old queued private view")
        ),
        Ok(ConnectionViewAcceptance::ObsoleteGeneration)
    );
    assert_eq!(views.current(), None);
    assert_eq!(
        views.accept(
            &new.generation(),
            new_scope,
            frame(1, 0, "new permitted scope")
        ),
        Ok(ConnectionViewAcceptance::View(ViewAcceptance::Applied))
    );
}
