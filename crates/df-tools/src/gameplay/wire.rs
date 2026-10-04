use df_model::checkpoint::{Basis, Checkpoint, CommandInput, GameCommand, GameInput};
use df_protocol::common as rpc;
use df_session::submission::RepositoryError;
use df_types::{OperationId, SessionRevision};

use super::model;

pub(super) fn revision(value: SessionRevision) -> rpc::SessionRevision {
    rpc::SessionRevision {
        epoch: Some(rpc::RecoveryEpoch {
            value: Some(value.epoch().get()),
        }),
        sequence: Some(value.sequence()),
    }
}
pub(super) fn receipt(
    basis: Basis,
    operation: OperationId,
    outcome: rpc::decision_receipt::Outcome,
    replayed: bool,
) -> rpc::DecisionReceipt {
    rpc::DecisionReceipt {
        operation_id: Some(rpc::OperationId {
            value: Some(operation.as_bytes().to_vec()),
        }),
        revision: Some(revision(basis.revision)),
        session_id: Some(rpc::SessionId {
            value: Some(basis.session.as_bytes().to_vec()),
        }),
        run_id: Some(rpc::RunId {
            value: Some(basis.run.as_bytes().to_vec()),
        }),
        replayed,
        outcome: Some(outcome),
    }
}
pub(super) fn rejected(
    basis: Basis,
    operation: OperationId,
    code: rpc::RejectionCode,
) -> rpc::DecisionReceipt {
    receipt(
        basis,
        operation,
        rpc::decision_receipt::Outcome::Rejected(rpc::Rejection { code: code as i32 }),
        false,
    )
}
pub(super) fn committed(receipt: rpc::DecisionReceipt) -> rpc::SubmitActionResponse {
    rpc::SubmitActionResponse {
        outcome: Some(rpc::submit_action_response::Outcome::CommittedDecision(
            Box::new(receipt),
        )),
    }
}
pub(super) fn observation(code: rpc::RejectionCode) -> rpc::SubmitActionResponse {
    rpc::SubmitActionResponse {
        outcome: Some(rpc::submit_action_response::Outcome::OperationObservation(
            rpc::OperationObservation { code: code as i32 },
        )),
    }
}
pub(super) fn action_kind(entry: &str) -> Option<rpc::GameplayActionKind> {
    Some(match entry {
        "create-character" => rpc::GameplayActionKind::CreateCharacter,
        "begin-story" => rpc::GameplayActionKind::BeginStory,
        "ask-courier" => rpc::GameplayActionKind::AskCourier,
        "escort-courier" => rpc::GameplayActionKind::EscortCourier,
        "defend-courier" => rpc::GameplayActionKind::DefendCourier,
        "greatsword-attack" => rpc::GameplayActionKind::GreatswordAttack,
        "second-wind" => rpc::GameplayActionKind::SecondWind,
        "end-turn" => rpc::GameplayActionKind::EndTurn,
        _ => return None,
    })
}
pub(super) fn action_entry(kind: rpc::GameplayActionKind) -> Option<&'static str> {
    Some(match kind {
        rpc::GameplayActionKind::CreateCharacter => "create-character",
        rpc::GameplayActionKind::BeginStory => "begin-story",
        rpc::GameplayActionKind::AskCourier => "ask-courier",
        rpc::GameplayActionKind::EscortCourier => "escort-courier",
        rpc::GameplayActionKind::DefendCourier => "defend-courier",
        rpc::GameplayActionKind::GreatswordAttack => "greatsword-attack",
        rpc::GameplayActionKind::SecondWind => "second-wind",
        rpc::GameplayActionKind::EndTurn => "end-turn",
        _ => return None,
    })
}
pub(super) fn unrelated_payload(
    request: &rpc::SubmitActionRequest,
    kind: rpc::GameplayActionKind,
) -> bool {
    request.destination != rpc::HarborDestination::Unspecified as i32
        || (kind != rpc::GameplayActionKind::CreateCharacter && request.character.is_some())
        || (kind != rpc::GameplayActionKind::GreatswordAttack
            && (request.savage_attacker || request.graze))
}
pub(super) fn journey_input(
    request: &rpc::SubmitActionRequest,
    member: df_types::MemberId,
    current: &Checkpoint,
) -> Result<GameInput, tonic::Status> {
    let invalid = || tonic::Status::invalid_argument("required bounded action fields invalid");
    let session = df_api::session_id(request.session_id.as_ref()).map_err(|_| invalid())?;
    let run = df_api::run_id(request.run_id.as_ref()).map_err(|_| invalid())?;
    let observed =
        df_api::session_revision(request.observed_revision.as_ref()).map_err(|_| invalid())?;
    let operation = df_api::operation_id(request.operation_id.as_ref()).map_err(|_| invalid())?;
    let kind = rpc::GameplayActionKind::try_from(request.action_kind)
        .map_err(|_| tonic::Status::invalid_argument("unknown action kind"))?;
    let entry =
        action_entry(kind).ok_or_else(|| tonic::Status::invalid_argument("unsupported action"))?;
    if request.offer_id.len() > 96 {
        return Err(tonic::Status::invalid_argument("offer exceeds bound"));
    }
    let mut choices = Vec::new();
    if let Some(character) = &request.character {
        if character.name.len() > 48 || character.choices.len() > 16 {
            return Err(tonic::Status::invalid_argument(
                "character choices exceed bound",
            ));
        }
        let encoded_name = if character.name.is_empty() {
            "empty-name".to_owned()
        } else {
            super::journey::hex(character.name.as_bytes())
        };
        choices.push((
            model::label("name").map_err(|_| invalid())?,
            model::label(&encoded_name).map_err(|_| invalid())?,
        ));
        for selection in &character.choices {
            if selection.group_id.len() > 64 || selection.option_id.len() > 64 {
                return Err(tonic::Status::invalid_argument(
                    "character option exceeds bound",
                ));
            }
            choices.push((
                model::label(&selection.group_id).map_err(|_| invalid())?,
                model::label(&selection.option_id).map_err(|_| invalid())?,
            ));
        }
    }
    if kind == rpc::GameplayActionKind::GreatswordAttack {
        for (key, value) in [
            ("savage-attacker", request.savage_attacker),
            ("graze", request.graze),
        ] {
            choices.push((
                model::label(key).map_err(|_| invalid())?,
                model::label(if value { "yes" } else { "no" }).map_err(|_| invalid())?,
            ));
        }
    }
    Ok(GameInput::Game(CommandInput {
        basis: Basis {
            session,
            run,
            revision: observed,
        },
        operation,
        member,
        observed_revision: observed,
        command: GameCommand::ProposeAction {
            actor: super::journey::player_entity(member, current)
                .map_err(|_| tonic::Status::permission_denied("character binding unavailable"))?,
            action: model::content(entry).map_err(|_| invalid())?,
            targets: Vec::new(),
            choices,
        },
    }))
}
fn attack_group(id: &str, label: &str, description: &str) -> rpc::JourneyGroup {
    rpc::JourneyGroup {
        group_id: id.to_owned(),
        label: label.to_owned(),
        options: vec![
            rpc::JourneyOption {
                option_id: "no".to_owned(),
                label: "Do not use".to_owned(),
                description: String::new(),
            },
            rpc::JourneyOption {
                option_id: "yes".to_owned(),
                label: "Use".to_owned(),
                description: description.to_owned(),
            },
        ],
    }
}
pub(super) fn creation_offer() -> rpc::CharacterCreationOffer {
    use df_rules::local_journey as rules;
    let group = |id: &str, label: &str, values: &[(&str, &str, &str)]| rpc::JourneyGroup {
        group_id: id.to_owned(),
        label: label.to_owned(),
        options: values
            .iter()
            .map(|(id, label, description)| rpc::JourneyOption {
                option_id: (*id).to_owned(),
                label: (*label).to_owned(),
                description: (*description).to_owned(),
            })
            .collect(),
    };
    let choice_group = |id: &str, label: &str, values: &[&str]| {
        group(
            id,
            label,
            &values.iter().map(|id| (*id, *id, "")).collect::<Vec<_>>(),
        )
    };
    rpc::CharacterCreationOffer {description:"Supported adventure builds: Level 1 Dwarf Fighter / Soldier. Choose one option in each group. Other classes and species remain in development; every submitted build is validated on the server.".to_owned(),
        groups:vec![
            group("species","Ancestry",&[("dwarf","Dwarf","Dwarven Toughness, Darkvision, Dwarven Resilience and Stonecunning retained.")]),
            group("class","Class",&[("fighter-1","Fighter · level 1","Weapon training, Fighting Style, Second Wind, three masteries.")]),
            group("background","Background",&[("soldier","Soldier","Athletics, Intimidation, gaming set and Savage Attacker.")]),
            group("array","Ability training",&[("stalwart","Stalwart","STR17 DEX13 CON15 INT10 WIS12 CHA8 · Soldier +2STR/+1CON"),
                ("vanguard","Vanguard","STR17 DEX14 CON14 INT8 WIS10 CHA12 · Soldier +2STR/+1CON")]),
            choice_group("alignment","Alignment",rules::ALIGNMENTS),
            choice_group("language-1","First additional language",rules::STANDARD_LANGUAGES),
            choice_group("language-2","Second additional language",rules::STANDARD_LANGUAGES),
            choice_group("skill-1","First Fighter skill",rules::FIGHTER_SKILLS),
            choice_group("skill-2","Second Fighter skill",rules::FIGHTER_SKILLS),
            choice_group("gaming-set","Gaming set proficiency",rules::GAMING_SETS),
            group("equipment","Starting equipment",&[("fighter-a-soldier-a","Fighter A + Soldier A","Chain Mail, Greatsword, Flail, 8 Javelins, Dungeoneer's Pack; Spear, Shortbow, 20 arrows, gaming set, Healer's Kit, Quiver, Traveler's Clothes; 18 GP.")]),
            group("style-masteries","Fighting Style and masteries",&[("defense-greatsword-flail-javelin","Defense · Greatsword / Flail / Javelin","Defense +1 armored AC. Graze / Sap / Slow masteries. This encounter offers Greatsword only.")]),
            group("allied-ties","Initiative tie agreement",&[("join-order","Party agrees: earlier join acts first","Party ties use join order; the GM chooses allies before the Bandit when tied.")]),
        ],source_revision:rules::SOURCE_REVISION.to_owned()}
}
fn party_member(
    state: &df_model::checkpoint::GameState,
    who: df_model::checkpoint::EntityId,
    ready: bool,
) -> Result<rpc::PartyMemberView, RepositoryError> {
    use super::journey;
    let active = state
        .encounters
        .first()
        .and_then(|encounter| encounter.active_turn)
        == Some(who);
    Ok(rpc::PartyMemberView {
        name: if ready {
            journey::name(state, who)?
        } else {
            "Creating a hero…".to_owned()
        },
        character_ready: ready,
        hit_points: if ready {
            journey::value(state, who, "hit-points")?
        } else {
            0
        },
        maximum_hit_points: if who.as_bytes() == &[0x65; 16] {
            11
        } else if ready {
            13
        } else {
            0
        },
        active_turn: active,
        unconscious: ready && journey::value(state, who, "unconscious")? != 0,
        participant_id: who.as_bytes().to_vec(),
    })
}
pub(super) fn journey_view(
    current: &Checkpoint,
    role: df_persistence::local_demo_scope::LocalDemoRole,
    principal: df_types::MemberId,
) -> Result<rpc::ViewMessage, RepositoryError> {
    use super::journey;
    use df_persistence::local_demo_scope::LocalDemoRole;
    let state = current.state();
    let phase = journey::phase(current)?;
    let mut party = Vec::new();
    for member in journey::participants(current) {
        let who = member.character.ok_or(RepositoryError::InvalidCandidate)?;
        party.push(party_member(
            state,
            who,
            state
                .characters
                .iter()
                .any(|character| character.entity == who),
        )?);
    }
    let mut own = None;
    let mut creation = None;
    let mut offers = Vec::new();
    let mut private_clue = String::new();
    if role == LocalDemoRole::Player {
        let who = journey::player_entity(principal, current)?;
        if state
            .characters
            .iter()
            .any(|character| character.entity == who)
        {
            own = Some(journey::character_sheet(state, who)?);
        } else {
            creation = Some(creation_offer());
        }
        for (kind, label) in journey::offered(current, principal)? {
            offers.push(rpc::GameplayActionOffer {
                offer_id: journey::offer_id(current, kind),
                action_kind: kind as i32,
                label: label.to_owned(),
                destinations: Vec::new(),
                input_groups: if kind == rpc::GameplayActionKind::GreatswordAttack {
                    vec![
                        attack_group(
                            "savage-attacker",
                            "Savage Attacker",
                            "Roll weapon damage twice and take the higher total",
                        ),
                        attack_group(
                            "graze",
                            "Graze",
                            "On a miss, apply your Strength modifier as damage",
                        ),
                    ]
                } else {
                    Vec::new()
                },
            });
        }
        if state.facts.iter().any(|fact| matches!(&fact.value,
            df_model::checkpoint::FactValue::ContentEvent {definition,..} if definition.entry.as_str()=="private-courier-note")
            && matches!(&fact.audience,df_model::checkpoint::AudienceScope::Members(members) if members.contains(&principal))) {
            private_clue="The courier quietly tells you: the sealed packet is addressed to Vell at the Harbor Inn. Keep its destination between you.".to_owned();
        }
    }
    let mut combat = None;
    if let Some(encounter) = state.encounters.first() {
        let mut participants = party.clone();
        participants.push(party_member(state, journey::entity([0x65; 16])?, true)?);
        let outcomes = state
            .decisions
            .iter()
            .filter(|decision| decision.source_policy.as_str() == "local-journey-rpc-1")
            .map(journey::accepted)
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flat_map(|accepted| accepted.combat)
            .collect::<Vec<_>>();
        combat = Some(rpc::CombatView {
            participants,
            active_actor: encounter
                .active_turn
                .map(|who| journey::name(state, who))
                .transpose()?
                .unwrap_or_default(),
            round: u32::try_from(state.logical_time.ticks / 6 + 1)
                .map_err(|_| RepositoryError::Capacity)?,
            outcomes: outcomes
                .into_iter()
                .rev()
                .take(6)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect(),
            finished: encounter.active_turn.is_none(),
            source_revision: df_rules::local_journey::SOURCE_REVISION.to_owned(),
            active_participant_id: encounter
                .active_turn
                .map_or_else(Vec::new, |who| who.as_bytes().to_vec()),
        });
    }
    let (title, narration, speaker, dialogue) = match phase {
        rpc::JourneyPhase::Room => (
            "Gather at the Lantern Wharf",
            "A rain-soaked harbor waits beyond the lanterns. Share the room with your companions, then bring your heroes into the story.",
            "",
            "",
        ),
        rpc::JourneyPhase::Opening => (
            "The Broken Seal",
            "You reach the Lantern Wharf as a frightened courier slips from the shadows. Behind him, a boot scrapes across the boards.",
            "Courier",
            "Please. I can't deliver this alone. The seal was broken before the ship reached port.",
        ),
        rpc::JourneyPhase::Dialogue => {
            if state.narrative.active_beats[0].entry.as_str() == "courier-answer-seal" {
                (
                    "A Whisper in the Rain",
                    "The courier presses the packet against his coat. A dockside enforcer approaches, one hand on a scimitar.",
                    "Courier",
                    "The crest belongs to a ship that never appears on the ledger. I think someone aboard followed me.",
                )
            } else {
                (
                    "An Oath at the Wharf",
                    "The courier steadies himself behind you. The approaching enforcer draws a scimitar and demands the packet.",
                    "Courier",
                    "If you help me reach the inn, I'll tell Vell what I saw aboard that ship. But he won't let us leave.",
                )
            }
        }
        rpc::JourneyPhase::Combat => (
            "Steel Beneath the Lanterns",
            "The Bandit is within 5 feet of both heroes. All weapons are wielded openly: no surprise, cover or movement. This encounter uses nonlethal melee attacks.",
            "Bandit",
            "Hand over the packet. Last warning.",
        ),
        rpc::JourneyPhase::Complete => {
            if journey::value(state, journey::entity([0x65; 16])?, "unconscious")? != 0 {
                (
                    "The Courier's Road",
                    "The Bandit falls unconscious with 1 HP. The courier lowers his shaking hands. The packet—and its secret—remain with your party.",
                    "Courier",
                    "You stood your ground. Come on. There's a light waiting for us at the inn.",
                )
            } else {
                (
                    "A Costly Encounter",
                    "Both heroes are unconscious with 1 HP after nonlethal attacks. Their Short Rests have begun; the campaign clock has not advanced that rest. The Bandit takes the packet and leaves.",
                    "",
                    "",
                )
            }
        }
        _ => return Err(RepositoryError::InvalidCandidate),
    };
    let journey = rpc::JourneyView {
        phase: if creation.is_some() {
            rpc::JourneyPhase::CharacterCreation as i32
        } else {
            phase as i32
        },
        room_code: journey::ROOM_CODE.to_owned(),
        join_path: format!("/gameplay/player?room={}", journey::ROOM_CODE),
        party,
        creation,
        own_character: own,
        combat,
        speaker: speaker.to_owned(),
        dialogue: dialogue.to_owned(),
    };
    let scene = rpc::GameplayScene {
        destination: rpc::HarborDestination::Unspecified as i32,
        title: title.to_owned(),
        scene_asset: "assets/ui/scenes/mara-harbor-v4.png".to_owned(),
    };
    let audience = match role {
        LocalDemoRole::Display => rpc::view_message::Audience::Display(rpc::DisplayGameplayView {
            narration: narration.to_owned(),
            scene_asset: scene.scene_asset.clone(),
            scene: Some(scene),
            journey: Some(journey),
        }),
        LocalDemoRole::Player => rpc::view_message::Audience::Player(rpc::PlayerGameplayView {
            offer_id: String::new(),
            narration: narration.to_owned(),
            check: None,
            private_clue,
            action_available: !offers.is_empty(),
            offers,
            scene: Some(scene),
            journey: Some(journey),
        }),
    };
    Ok(rpc::ViewMessage {
        revision: Some(revision(current.basis().revision)),
        audience: Some(audience),
    })
}
