use super::*;

#[test]
fn duplicate_name_offer_is_rejected_by_the_registered_creation_stage() {
    let mut current = initial().expect("empty room");
    current = stage_join(
        &current,
        &super::tests::input(
            &current,
            bootstrap_member().expect("bootstrap"),
            1,
            "join-room",
            vec![],
        ),
    )
    .expect("real joined member");
    let member = MemberId::from_bytes(&MEMBERS[0]).expect("joined member");
    let mut choices = super::tests::build("Brynn");
    choices.push((
        model::label("name").expect("name offer"),
        model::label(&hex(b"Neris")).expect("second encoded name"),
    ));
    let request = super::tests::input(&current, member, 2, "create-character", choices);
    let before = current.clone();

    assert!(
        stage_with_supplier(&current, &request, &mut |_| {
            panic!("character creation cannot draw")
        })
        .is_err()
    );
    assert_eq!(current, before);
    assert!(current.state().characters.is_empty());
}

#[test]
fn creation_rejects_foreign_choice_member_actor_and_basis_at_stage_boundary() {
    let mut current = initial().expect("empty room");
    current = stage_join(
        &current,
        &super::tests::input(
            &current,
            bootstrap_member().expect("bootstrap"),
            1,
            "join-room",
            vec![],
        ),
    )
    .expect("real joined member");
    let member = MemberId::from_bytes(&MEMBERS[0]).expect("joined member");
    let valid_choices = super::tests::build("Brynn");
    let before = current.clone();

    let mut foreign_choice = valid_choices.clone();
    let class = foreign_choice
        .iter_mut()
        .find(|(offer, _)| offer.as_str() == "class")
        .expect("class choice");
    class.1 = model::label("fighter-foreign").expect("foreign class");
    let wrong_choice = super::tests::input(&current, member, 2, "create-character", foreign_choice);
    assert!(
        stage_with_supplier(&current, &wrong_choice, &mut |_| {
            panic!("character creation cannot draw")
        })
        .is_err()
    );
    assert_eq!(current, before);

    let mut foreign_member = super::tests::input(
        &current,
        member,
        3,
        "create-character",
        valid_choices.clone(),
    );
    if let GameInput::Game(command) = &mut foreign_member {
        command.member = MemberId::from_bytes(&MEMBERS[1]).expect("foreign member");
    }
    assert!(
        stage_with_supplier(&current, &foreign_member, &mut |_| {
            panic!("character creation cannot draw")
        })
        .is_err()
    );
    assert_eq!(current, before);

    let mut wrong_actor = super::tests::input(
        &current,
        member,
        4,
        "create-character",
        valid_choices.clone(),
    );
    if let GameInput::Game(command) = &mut wrong_actor
        && let GameCommand::ProposeAction { actor, .. } = &mut command.command
    {
        *actor = room_entity().expect("room actor");
    }
    assert!(
        stage_with_supplier(&current, &wrong_actor, &mut |_| {
            panic!("character creation cannot draw")
        })
        .is_err()
    );
    assert_eq!(current, before);

    let mut stale_basis =
        super::tests::input(&current, member, 5, "create-character", valid_choices);
    if let GameInput::Game(command) = &mut stale_basis {
        command.basis.revision = command
            .basis
            .revision
            .next_sequence()
            .expect("next revision");
    }
    assert!(
        stage_with_supplier(&current, &stale_basis, &mut |_| {
            panic!("character creation cannot draw")
        })
        .is_err()
    );
    assert_eq!(current, before);
}

#[test]
fn accepted_creation_keeps_canonical_choices_and_source_backed_inventory() {
    let mut current = initial().expect("empty room");
    current = stage_join(
        &current,
        &super::tests::input(
            &current,
            bootstrap_member().expect("bootstrap"),
            1,
            "join-room",
            vec![],
        ),
    )
    .expect("real joined member");
    let member = MemberId::from_bytes(&MEMBERS[0]).expect("joined member");
    let request = super::tests::input(
        &current,
        member,
        2,
        "create-character",
        super::tests::build("Brynn"),
    );

    let created = stage_with_supplier(&current, &request, &mut |_| {
        panic!("character creation cannot draw")
    })
    .expect("source-qualified legal build");
    let entity = player_entity(member, &created).expect("member character");
    let character = created
        .state()
        .characters
        .iter()
        .find(|character| character.entity == entity)
        .expect("accepted character");
    assert_eq!(character.owner, member);
    assert_eq!(
        character.choices.len(),
        df_rules::local_journey::BUILD_GROUPS.len() + 1
    );
    assert_eq!(
        character
            .choices
            .iter()
            .filter(|choice| choice.offer.as_str() == "name")
            .count(),
        1
    );
    assert!(
        character.choices.iter().any(|choice| {
            choice.offer.as_str() == "name" && choice.selected.as_str() == "Brynn"
        })
    );
    assert_eq!(value(created.state(), entity, "hit-points"), Ok(13));
    assert_eq!(value(created.state(), entity, "armor-class"), Ok(17));
    assert_eq!(value(created.state(), entity, "strength"), Ok(17));
    assert_eq!(value(created.state(), entity, "constitution"), Ok(15));
    assert_eq!(value(created.state(), entity, "second-wind"), Ok(2));
    assert!(
        character
            .choices
            .iter()
            .all(|choice| choice.participant == member && choice.source == rule().expect("source"))
    );

    let inventory = created
        .state()
        .inventory
        .iter()
        .filter(|item| item.owner == entity)
        .collect::<Vec<_>>();
    assert_eq!(inventory.len(), 13);
    assert!(
        inventory
            .iter()
            .all(|item| item.source == rule().expect("source"))
    );
    assert!(inventory.iter().all(|item| {
        item.attunement_owner.is_none()
            && item.origin.package.as_str() == "harbor-investigation-demo-profile6-npc-1"
            && created.state().entities.iter().any(|world| {
                world.id == item.item
                    && world.definition == item.origin
                    && world.identity_revision.as_str() == "source-starting-equipment-1"
            })
    }));
    let quantity_for = |origin: &str| {
        inventory
            .iter()
            .find(|item| item.origin.entry.as_str() == origin)
            .map(|item| item.quantity)
    };
    assert_eq!(quantity_for("javelin"), Some(8));
    assert_eq!(quantity_for("spear"), Some(1));
    assert_eq!(quantity_for("shortbow"), Some(1));
    assert_eq!(quantity_for("arrow"), Some(20));
    assert_eq!(quantity_for("chain-mail"), Some(1));
    assert_eq!(quantity_for("greatsword"), Some(1));
    assert_eq!(quantity_for("flail"), Some(1));
    assert_eq!(quantity_for("dungeoneer-pack"), Some(1));
    assert_eq!(quantity_for("gaming-set"), Some(1));
    assert_eq!(quantity_for("healers-kit"), Some(1));
    assert_eq!(quantity_for("quiver"), Some(1));
    assert_eq!(quantity_for("travelers-clothes"), Some(1));
    assert_eq!(quantity_for("gold-piece"), Some(18));
}
