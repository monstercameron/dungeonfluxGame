use super::affordance::NativeTargetError;
use super::*;

fn member() -> MemberId {
    MemberId::from_bytes(&MEMBERS[0]).unwrap()
}
fn target_input(current: &Checkpoint, op: u8, entry: &str, targets: Vec<EntityId>) -> GameInput {
    let choices = if entry == "greatsword-attack" {
        vec![
            (
                model::label("savage-attacker").unwrap(),
                model::label("no").unwrap(),
            ),
            (model::label("graze").unwrap(), model::label("no").unwrap()),
        ]
    } else {
        vec![]
    };
    let mut request = super::tests::input(current, member(), op, entry, choices);
    let GameInput::Game(command) = &mut request else {
        panic!("typed input");
    };
    let GameCommand::ProposeAction {
        targets: selected, ..
    } = &mut command.command
    else {
        panic!("typed action");
    };
    *selected = targets;
    request
}
fn melee() -> Checkpoint {
    let current = super::tests::opening_story();
    let current = stage_with_supplier(
        &current,
        &target_input(&current, 10, "escort-courier", vec![]),
        &mut |_| panic!("escort cannot draw"),
    )
    .unwrap();
    stage_with_supplier(
        &current,
        &target_input(&current, 11, "defend-courier", vec![]),
        &mut |sides| Ok(sides),
    )
    .unwrap()
}

#[test]
fn affordance_target_current_native_offers_bind_actual_courier_identity_and_source() {
    let current = super::tests::opening_story();
    let offers = affordance::current(&current, member()).unwrap();
    assert_eq!(offers.len(), 2);
    for offer in &offers {
        assert_eq!(offer.set.basis, current.basis());
        assert_eq!(offer.set.pins, current.pins());
        assert_eq!(offer.set.matches.len(), 1);
        assert_eq!(offer.set.matches[0].source, rule().unwrap());
        assert_eq!(
            offer.set.matches[0].target.as_ref().unwrap().id,
            courier_reaction::courier().unwrap()
        );
    }
    assert_eq!(
        offered(&current, member()).unwrap(),
        authored_offered(&current, member()).unwrap()
    );
}

#[test]
fn affordance_target_explicit_courier_and_existing_button_keep_identical_registered_outcome() {
    let current = super::tests::opening_story();
    let before = current.clone();
    let implicit = target_input(&current, 12, "ask-courier", vec![]);
    let explicit = target_input(
        &current,
        12,
        "ask-courier",
        vec![courier_reaction::courier().unwrap()],
    );
    let first =
        stage_with_supplier(&current, &implicit, &mut |_| panic!("dialogue cannot draw")).unwrap();
    let second =
        stage_with_supplier(&current, &explicit, &mut |_| panic!("dialogue cannot draw")).unwrap();
    assert_eq!(first, second);
    assert_eq!(current, before);
    assert_eq!(
        first
            .state()
            .decisions
            .last()
            .unwrap()
            .source_policy
            .as_str(),
        THREAD_POLICY
    );
    assert!(first.state().draws.is_empty());
}

#[test]
fn affordance_target_wrong_unknown_duplicate_and_future_targets_never_stage_or_draw() {
    let current = super::tests::opening_story();
    let before = current.clone();
    let courier = courier_reaction::courier().unwrap();
    let other = player_entity(MemberId::from_bytes(&MEMBERS[1]).unwrap(), &current).unwrap();
    for targets in [
        vec![other],
        vec![entity(BANDIT).unwrap()],
        vec![entity([0x7f; 16]).unwrap()],
        vec![courier, courier],
        vec![courier, other],
    ] {
        assert_eq!(
            affordance::validate(
                &current,
                member(),
                &model::content("ask-courier").unwrap(),
                &targets
            ),
            Err(NativeTargetError::UnsupportedTarget)
        );
        let request = target_input(&current, 13, "ask-courier", targets);
        assert!(
            stage_with_supplier(&current, &request, &mut |_| panic!("refusal cannot draw"))
                .is_err()
        );
        assert_eq!(current, before);
    }
}

#[test]
fn affordance_target_unoffered_action_is_explicit_and_cannot_infer_general_physics() {
    let current = super::tests::opening_story();
    for entry in ["greatsword-attack", "short-rest", "open-unauthored-door"] {
        let action = model::content(entry).unwrap();
        assert_eq!(
            affordance::validate(&current, member(), &action, &[]),
            Err(NativeTargetError::UnsupportedAction)
        );
        let input = target_input(&current, 14, entry, vec![]);
        assert!(
            stage_with_supplier(&current, &input, &mut |_| panic!(
                "unoffered action cannot draw"
            ))
            .is_err()
        );
    }
}

#[test]
fn affordance_target_actual_bandit_selection_preserves_registered_attack_and_draws() {
    let current = melee();
    assert!(
        offered(&current, member())
            .unwrap()
            .iter()
            .any(|(kind, _)| *kind == rpc::GameplayActionKind::GreatswordAttack)
    );
    let implicit = target_input(&current, 15, "greatsword-attack", vec![]);
    let explicit = target_input(
        &current,
        15,
        "greatsword-attack",
        vec![entity(BANDIT).unwrap()],
    );
    let first = stage_with_supplier(&current, &implicit, &mut |sides| Ok(sides)).unwrap();
    let second = stage_with_supplier(&current, &explicit, &mut |sides| Ok(sides)).unwrap();
    assert_eq!(first, second);
    assert!(!first.state().draws.is_empty());
    assert_eq!(
        current.state().decisions.len() + 1,
        first.state().decisions.len()
    );
    assert_eq!(
        affordance::validate(
            &current,
            member(),
            &model::content("greatsword-attack").unwrap(),
            &[courier_reaction::courier().unwrap()]
        ),
        Err(NativeTargetError::UnsupportedTarget)
    );
}

#[test]
fn affordance_target_self_binding_does_not_accept_another_party_member() {
    let current = melee();
    let actor = player_entity(member(), &current).unwrap();
    let other = player_entity(MemberId::from_bytes(&MEMBERS[1]).unwrap(), &current).unwrap();
    assert_eq!(
        affordance::validate(
            &current,
            member(),
            &model::content("second-wind").unwrap(),
            &[actor]
        ),
        Ok(())
    );
    assert_eq!(
        affordance::validate(
            &current,
            member(),
            &model::content("second-wind").unwrap(),
            &[other]
        ),
        Err(NativeTargetError::UnsupportedTarget)
    );
    let next = stage_with_supplier(
        &current,
        &target_input(&current, 16, "second-wind", vec![actor]),
        &mut |sides| Ok(sides),
    )
    .unwrap();
    assert_eq!(value(next.state(), actor, "bonus-used").unwrap(), 1);
    assert_eq!(value(next.state(), actor, "second-wind").unwrap(), 1);
}

#[test]
fn affordance_target_stale_owner_and_foreign_content_refuse_before_real_handler() {
    let current = super::tests::opening_story();
    let before = current.clone();
    for mutation in 0..3 {
        let mut input = target_input(
            &current,
            17,
            "ask-courier",
            vec![courier_reaction::courier().unwrap()],
        );
        let GameInput::Game(command) = &mut input else {
            panic!("typed");
        };
        match mutation {
            0 => command.basis.revision = command.basis.revision.next_sequence().unwrap(),
            1 => command.member = MemberId::from_bytes(&[0x7e; 16]).unwrap(),
            _ => {
                let GameCommand::ProposeAction { action, .. } = &mut command.command else {
                    panic!("action");
                };
                action.package = model::label("foreign-package").unwrap();
            }
        }
        assert!(
            stage_with_supplier(&current, &input, &mut |_| panic!(
                "invalid input cannot draw"
            ))
            .is_err()
        );
        assert_eq!(current, before);
    }
}

#[test]
fn affordance_target_named_contract_has_actual_native_source_and_fixture_identity() {
    use sha2::{Digest, Sha256};
    let current = super::tests::opening_story();
    let before = current.clone();
    let offers = affordance::current(&current, member()).unwrap();
    assert_eq!(offers.len(), 2);
    let source = &offers[0].set.matches[0].source;
    assert_eq!(*source, rule().unwrap());
    assert_eq!(
        affordance::validate(
            &current,
            member(),
            &offers[0].set.action,
            &[player_entity(member(), &current).unwrap()]
        ),
        Err(NativeTargetError::UnsupportedTarget)
    );
    assert_eq!(current, before);
    let fixture =
        include_bytes!("../../../../df-intent/tests/support/affordance_target_contract.json");
    println!(
        "D03 actual native registered affordance/unsupported-target witness: source={}/{}/{} catalog={} fixture_sha256={:x}; generic ambiguity/source/capacity exercised by World+Intent tests",
        source.source.as_str(),
        source.entry.as_str(),
        source.clause.as_str(),
        current.pins().rules.catalog.as_str(),
        Sha256::digest(fixture)
    );
}
