//! Canonical room, authored story and source-qualified combat composition.
use df_model::checkpoint::*;
use df_persistence::local_demo_scope::PLAYER;
use df_protocol::common as rpc;
use df_rules::local_journey as rules;
use df_session::submission::RepositoryError;
use df_types::{MemberId, OperationId};
use prost::Message;
use std::time::Duration;

use super::{model, wire};

pub(super) const ROOM_CODE: &str = "LANTERN";
pub(super) const ROOM_ENTITY: [u8; 16] = [0x45; 16];
const MEMBERS: [[u8; 16]; 2] = [[0x61; 16], [0x62; 16]];
const ENTITIES: [[u8; 16]; 2] = [[0x63; 16], [0x64; 16]];
const BANDIT: [u8; 16] = [0x65; 16];
const ENCOUNTER: [u8; 16] = [0x66; 16];
pub(super) const THREAD_POLICY: &str = "local-journey-rpc-2-threads-1";
const PACKET_THREAD: &str = "sealed-packet-delivery-thread";
const THREAT_THREAD: &str = "dockside-threat-thread";
// This fixed authored slice admits one causal event to one thread. Packet delivery is outside
// this journey; combat victory resolves only the dockside threat and never delivers the packet.
const THREAD_EVENTS: &[(&str, &str)] = &[
    ("begin-story", PACKET_THREAD),
    ("private-courier-note", PACKET_THREAD),
    ("escort-courier", PACKET_THREAD),
    ("defend-courier", THREAT_THREAD),
    ("greatsword-attack", THREAT_THREAD),
    ("second-wind", THREAT_THREAD),
    ("end-turn", THREAT_THREAD),
];
pub(super) const CONTENT_ENTRIES: &[&str] = &[
    "room",
    "join-room",
    "joined-player",
    "dwarf-fighter-soldier",
    "create-character",
    "begin-story",
    "opening",
    "dialogue",
    "ask-courier",
    "escort-courier",
    "courier-answer-seal",
    "courier-answer-escort",
    "private-courier-note",
    "defend-courier",
    "combat",
    "complete",
    "bandit",
    "greatsword-attack",
    "second-wind",
    "end-turn",
    "normal-nonlethal-melee",
    "fighter-a-soldier-a",
    "chain-mail",
    "greatsword",
    "flail",
    "javelin",
    "dungeoneer-pack",
    "spear",
    "shortbow",
    "arrow",
    "gaming-set",
    "healers-kit",
    "quiver",
    "travelers-clothes",
    "gold-piece",
    PACKET_THREAD,
    THREAT_THREAD,
];
fn bad<T>(_: T) -> RepositoryError {
    RepositoryError::InvalidCandidate
}
pub(super) fn rule() -> Result<RuleReference, RepositoryError> {
    Ok(RuleReference {
        catalog: model::label("srd521-journey-subset-1")?,
        source: model::label("srd521-creation-combat-selected")?,
        entry: model::label("dwarf-fighter-soldier-bandit")?,
        clause: model::label("normal-adjacent-nonlethal")?,
    })
}
pub(super) fn entity(bytes: [u8; 16]) -> Result<EntityId, RepositoryError> {
    EntityId::from_bytes(&bytes).map_err(bad)
}
pub(super) fn room_entity() -> Result<EntityId, RepositoryError> {
    entity(ROOM_ENTITY)
}
pub(super) fn bootstrap_member() -> Result<MemberId, RepositoryError> {
    MemberId::from_bytes(&PLAYER).map_err(bad)
}
pub(super) fn player_entity(
    member: MemberId,
    current: &Checkpoint,
) -> Result<EntityId, RepositoryError> {
    current
        .state()
        .members
        .iter()
        .find(|link| link.member == member && member.as_bytes() != &PLAYER)
        .and_then(|link| link.character)
        .ok_or(RepositoryError::Unauthorized)
}
pub(super) fn participants(current: &Checkpoint) -> Vec<&MembershipLink> {
    current
        .state()
        .members
        .iter()
        .filter(|link| link.member.as_bytes() != &PLAYER)
        .collect()
}
pub(super) fn initial() -> Result<Checkpoint, RepositoryError> {
    let mut state = model::initial()?.state().clone();
    state.members = vec![MembershipLink {
        member: bootstrap_member()?,
        character: Some(room_entity()?),
    }];
    state.entities = vec![WorldEntity {
        id: room_entity()?,
        definition: model::content("room")?,
        location: None,
        position: None,
        identity_revision: model::label("local-room-1")?,
    }];
    state.characters.clear();
    state.narrative.definition = model::content("room")?;
    state.narrative.active_beats = vec![model::content("room")?];
    state.narrative.remaining_budget = 8;
    model::checkpoint(model::basis()?, state)
}
pub(super) fn phase(current: &Checkpoint) -> Result<rpc::JourneyPhase, RepositoryError> {
    match current.state().narrative.active_beats.as_slice() {
        [beat] => match beat.entry.as_str() {
            "room" => Ok(rpc::JourneyPhase::Room),
            "opening" => Ok(rpc::JourneyPhase::Opening),
            "dialogue" | "courier-answer-seal" | "courier-answer-escort" => {
                Ok(rpc::JourneyPhase::Dialogue)
            }
            "combat" => Ok(rpc::JourneyPhase::Combat),
            "complete" => Ok(rpc::JourneyPhase::Complete),
            _ => Err(RepositoryError::InvalidCandidate),
        },
        _ => Err(RepositoryError::InvalidCandidate),
    }
}
pub(super) fn resources() -> Result<Vec<ResourceConstraint>, RepositoryError> {
    let mut result = Vec::new();
    for owner in ENTITIES.into_iter().chain([BANDIT]) {
        for (name, minimum, maximum) in [
            ("hit-points", 1, if owner == BANDIT { 11 } else { 13 }),
            (
                "armor-class",
                if owner == BANDIT { 12 } else { 17 },
                if owner == BANDIT { 12 } else { 17 },
            ),
            ("strength", 1, 20),
            ("dexterity", 1, 20),
            ("constitution", 1, 20),
            ("intelligence", 1, 20),
            ("wisdom", 1, 20),
            ("charisma", 1, 20),
            ("second-wind", 0, 2),
            ("unconscious", 0, 1),
            ("action-used", 0, 1),
            ("bonus-used", 0, 1),
        ] {
            result.push(ResourceConstraint {
                owner: entity(owner)?,
                resource: model::label(name)?,
                minimum,
                maximum,
                source: rule()?,
            });
        }
    }
    Ok(result)
}
fn add_resource(
    state: &mut GameState,
    who: EntityId,
    name: &str,
    value: i64,
) -> Result<(), RepositoryError> {
    let label = model::label(name)?;
    let constraint = resources()?
        .into_iter()
        .find(|value| value.owner == who && value.resource == label)
        .ok_or(RepositoryError::InvalidCandidate)?;
    state.resources.push(ResourceState {
        owner: who,
        resource: label,
        value,
        minimum: constraint.minimum,
        maximum: constraint.maximum,
        source: constraint.source,
    });
    Ok(())
}
pub(super) fn value(state: &GameState, who: EntityId, name: &str) -> Result<u32, RepositoryError> {
    let label = model::label(name)?;
    state
        .resources
        .iter()
        .find(|resource| resource.owner == who && resource.resource == label)
        .ok_or(RepositoryError::InvalidCandidate)
        .and_then(|resource| u32::try_from(resource.value).map_err(bad))
}
fn set(
    state: &mut GameState,
    who: EntityId,
    name: &str,
    value: u32,
) -> Result<(), RepositoryError> {
    let label = model::label(name)?;
    let resource = state
        .resources
        .iter_mut()
        .find(|resource| resource.owner == who && resource.resource == label)
        .ok_or(RepositoryError::InvalidCandidate)?;
    resource.value = i64::from(value);
    Ok(())
}
pub(super) fn name(state: &GameState, who: EntityId) -> Result<String, RepositoryError> {
    if who == entity(BANDIT)? {
        return Ok("Dockside Bandit".to_owned());
    }
    let character = state
        .characters
        .iter()
        .find(|character| character.entity == who)
        .ok_or(RepositoryError::InvalidCandidate)?;
    character
        .choices
        .iter()
        .find(|choice| choice.offer.as_str() == "name")
        .map(|choice| choice.selected.as_str().to_owned())
        .ok_or(RepositoryError::InvalidCandidate)
}
pub(super) fn character_sheet(
    state: &GameState,
    who: EntityId,
) -> Result<rpc::CharacterSheet, RepositoryError> {
    let mut facts = Vec::new();
    for (label, value) in [
        ("Species", "Dwarf"),
        ("Class", "Fighter · level 1"),
        ("Background", "Soldier"),
        ("Armor", "Chain Mail · Defense"),
        ("Weapon", "Greatsword · Graze"),
        ("Origin feat", "Savage Attacker"),
        ("Masteries", "Greatsword, Flail, Javelin"),
        ("Starting equipment", "Fighter A + Soldier A · 18 GP"),
    ] {
        facts.push(rpc::SheetFact {
            label: label.to_owned(),
            value: value.to_owned(),
        });
    }
    let character = state
        .characters
        .iter()
        .find(|character| character.entity == who)
        .ok_or(RepositoryError::InvalidCandidate)?;
    for (label, key) in [
        ("First language", "language-1"),
        ("Second language", "language-2"),
        ("First class skill", "skill-1"),
        ("Second class skill", "skill-2"),
        ("Alignment", "alignment"),
        ("Gaming set", "gaming-set"),
    ] {
        let selected = character
            .choices
            .iter()
            .find(|choice| choice.offer.as_str() == key)
            .ok_or(RepositoryError::InvalidCandidate)?;
        facts.push(rpc::SheetFact {
            label: label.to_owned(),
            value: selected.selected.as_str().replace('-', " "),
        });
    }
    facts.push(rpc::SheetFact {
        label: "Base language".to_owned(),
        value: "Common".to_owned(),
    });
    for (label, key) in [
        ("STR", "strength"),
        ("DEX", "dexterity"),
        ("CON", "constitution"),
        ("INT", "intelligence"),
        ("WIS", "wisdom"),
        ("CHA", "charisma"),
        ("Second Wind uses", "second-wind"),
    ] {
        facts.push(rpc::SheetFact {
            label: label.to_owned(),
            value: value(state, who, key)?.to_string(),
        });
    }
    Ok(rpc::CharacterSheet {
        name: name(state, who)?,
        facts,
        hit_points: value(state, who, "hit-points")?,
        maximum_hit_points: 13,
        armor_class: value(state, who, "armor-class")?,
        source_revision: rules::SOURCE_REVISION.to_owned(),
    })
}
fn beat(state: &mut GameState, next: &str) -> Result<(), RepositoryError> {
    state
        .narrative
        .completed_beats
        .append(&mut state.narrative.active_beats);
    state.narrative.active_beats = vec![model::content(next)?];
    Ok(())
}
pub(super) fn offer_id(current: &Checkpoint, kind: rpc::GameplayActionKind) -> String {
    if matches!(
        kind,
        rpc::GameplayActionKind::GreatswordAttack
            | rpc::GameplayActionKind::SecondWind
            | rpc::GameplayActionKind::EndTurn
    ) && let Some(encounter) = current.state().encounters.first()
        && let Some(active) = encounter.active_turn
    {
        return format!(
            "journey-{}-{}-{}",
            kind as i32,
            current.state().logical_time.ticks,
            hex(active.as_bytes())
        );
    }
    format!("journey-{}-1", kind as i32)
}
pub(super) fn offered(
    current: &Checkpoint,
    member: MemberId,
) -> Result<Vec<(rpc::GameplayActionKind, &'static str)>, RepositoryError> {
    let who = player_entity(member, current)?;
    let ready = current
        .state()
        .characters
        .iter()
        .any(|character| character.entity == who);
    if !ready {
        return Ok(vec![(
            rpc::GameplayActionKind::CreateCharacter,
            "Bring your hero to life",
        )]);
    }
    let result = match phase(current)? {
        rpc::JourneyPhase::Room if current.state().characters.len() == 2 => vec![(
            rpc::GameplayActionKind::BeginStory,
            "Enter the Lantern Wharf",
        )],
        rpc::JourneyPhase::Room => vec![],
        rpc::JourneyPhase::Opening => vec![
            (
                rpc::GameplayActionKind::AskCourier,
                "Ask about the broken seal",
            ),
            (
                rpc::GameplayActionKind::EscortCourier,
                "Offer to protect the courier",
            ),
        ],
        rpc::JourneyPhase::Dialogue => vec![(
            rpc::GameplayActionKind::DefendCourier,
            "Stand between the courier and the bandit",
        )],
        rpc::JourneyPhase::Combat => {
            let encounter = current
                .state()
                .encounters
                .first()
                .ok_or(RepositoryError::InvalidCandidate)?;
            if encounter.active_turn != Some(who)
                || value(current.state(), who, "unconscious")? != 0
            {
                vec![]
            } else {
                let mut offered = Vec::new();
                if value(current.state(), who, "action-used")? == 0 {
                    offered.push((
                        rpc::GameplayActionKind::GreatswordAttack,
                        "Greatsword strike · nonlethal",
                    ));
                }
                if value(current.state(), who, "bonus-used")? == 0
                    && value(current.state(), who, "second-wind")? > 0
                {
                    offered.push((
                        rpc::GameplayActionKind::SecondWind,
                        "Second Wind · bonus action",
                    ));
                }
                offered.push((rpc::GameplayActionKind::EndTurn, "End your turn"));
                offered
            }
        }
        rpc::JourneyPhase::Complete => vec![],
        _ => return Err(RepositoryError::InvalidCandidate),
    };
    Ok(result)
}
pub(super) fn accepted(
    decision: &AcceptedDecision,
) -> Result<rpc::AcceptedAction, RepositoryError> {
    if decision.source_policy.as_str() != THREAD_POLICY {
        return Err(RepositoryError::InvalidReceipt);
    }
    let text = decision
        .semantic_output
        .as_deref()
        .ok_or(RepositoryError::InvalidReceipt)?;
    let bytes = unhex(text)?;
    rpc::AcceptedAction::decode(bytes.as_slice()).map_err(bad)
}
pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
pub(super) fn unhex(text: &str) -> Result<Vec<u8>, RepositoryError> {
    if text.len() > 4096 || !text.len().is_multiple_of(2) {
        return Err(RepositoryError::InvalidReceipt);
    }
    text.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let digit = |byte| match byte {
                b'0'..=b'9' => Some(byte - b'0'),
                b'a'..=b'f' => Some(byte - b'a' + 10),
                _ => None,
            };
            Ok(
                (digit(pair[0]).ok_or(RepositoryError::InvalidReceipt)? << 4)
                    | digit(pair[1]).ok_or(RepositoryError::InvalidReceipt)?,
            )
        })
        .collect()
}
pub(super) fn decode_join(
    decision: &AcceptedDecision,
) -> Result<([u8; 16], [u8; 16]), RepositoryError> {
    if decision.source_policy.as_str() != "local-room-join-1" {
        return Err(RepositoryError::InvalidReceipt);
    }
    let text = decision
        .semantic_output
        .as_deref()
        .ok_or(RepositoryError::InvalidReceipt)?;
    let values = text.split(':').collect::<Vec<_>>();
    let ["join", member, entity] = values.as_slice() else {
        return Err(RepositoryError::InvalidReceipt);
    };
    Ok((
        unhex(member)?.try_into().map_err(bad)?,
        unhex(entity)?.try_into().map_err(bad)?,
    ))
}
pub(super) fn join_selection(
    current: &Checkpoint,
) -> Result<([u8; 16], [u8; 16]), RepositoryError> {
    let index = participants(current).len();
    Ok((
        *MEMBERS.get(index).ok_or(RepositoryError::Capacity)?,
        *ENTITIES.get(index).ok_or(RepositoryError::Capacity)?,
    ))
}
pub(super) fn stage_join(
    current: &Checkpoint,
    input: &GameInput,
) -> Result<Checkpoint, RepositoryError> {
    let GameInput::Game(command) = input else {
        return Err(RepositoryError::InvalidCandidate);
    };
    let GameCommand::ProposeAction {
        actor,
        action,
        targets,
        choices,
    } = &command.command
    else {
        return Err(RepositoryError::InvalidCandidate);
    };
    let (member_id, entity_id) = join_selection(current)?;
    if *actor != room_entity()?
        || command.member != bootstrap_member()?
        || *action != model::content("join-room")?
        || !targets.is_empty()
        || !choices.is_empty()
        || phase(current)? != rpc::JourneyPhase::Room
    {
        return Err(RepositoryError::InvalidCandidate);
    }
    let mut basis = current.basis();
    basis.revision = basis.revision.next_sequence().map_err(bad)?;
    let mut state = current.state().clone();
    state.members.push(MembershipLink {
        member: MemberId::from_bytes(&member_id).map_err(bad)?,
        character: Some(entity(entity_id)?),
    });
    state.entities.push(WorldEntity {
        id: entity(entity_id)?,
        definition: model::content("joined-player")?,
        location: Some(room_entity()?),
        position: Some(if entity_id == ENTITIES[0] {
            Position { x: 0, y: 5, z: 0 }
        } else {
            Position { x: 5, y: 0, z: 0 }
        }),
        identity_revision: model::label("joined-player-1")?,
    });
    finish(
        current,
        basis,
        state,
        command.operation,
        Vec::new(),
        DecisionContent {
            entry: "join-room",
            semantic: Some(format!("join:{}:{}", hex(&member_id), hex(&entity_id))),
            policy: "local-room-join-1",
            audience: AudienceScope::Shared,
        },
    )
}
struct DecisionContent<'a> {
    entry: &'a str,
    semantic: Option<String>,
    policy: &'a str,
    audience: AudienceScope,
}
fn finish(
    current: &Checkpoint,
    basis: Basis,
    mut state: GameState,
    operation: OperationId,
    draws: Vec<ActualDraw>,
    content: DecisionContent<'_>,
) -> Result<Checkpoint, RepositoryError> {
    let DecisionContent {
        entry,
        semantic,
        policy,
        audience,
    } = content;
    let mut facts = Vec::new();
    let make_id = |ordinal: u32| {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(
            [
                operation.as_bytes().as_slice(),
                ordinal.to_le_bytes().as_slice(),
            ]
            .concat(),
        );
        FactId::from_bytes(&digest[..16]).map_err(bad)
    };
    for draw in &draws {
        let fact_id = make_id(draw.ordinal)?;
        state.facts.push(GameFact {
            id: fact_id,
            revision: basis.revision,
            operation,
            ordinal: draw.ordinal,
            cause: current.state().facts.last().map(|fact| fact.id),
            audience: AudienceScope::Shared,
            value: FactValue::DrawAccepted {
                operation,
                ordinal: draw.ordinal,
            },
        });
        facts.push(fact_id);
    }
    for resource in &state.resources {
        if let Some(before) = current
            .state()
            .resources
            .iter()
            .find(|value| value.owner == resource.owner && value.resource == resource.resource)
            && before.value != resource.value
        {
            let ordinal = facts.len() as u32;
            let id = make_id(ordinal)?;
            state.facts.push(GameFact {
                id,
                revision: basis.revision,
                operation,
                ordinal,
                cause: facts.last().copied(),
                audience: AudienceScope::Shared,
                value: FactValue::ResourceChanged {
                    entity: resource.owner,
                    resource: resource.resource.clone(),
                    before: before.value,
                    after: resource.value,
                    source: resource.source.clone(),
                },
            });
            facts.push(id);
        }
    }
    if state.logical_time != current.state().logical_time {
        let ordinal = facts.len() as u32;
        let id = make_id(ordinal)?;
        state.facts.push(GameFact {
            id,
            revision: basis.revision,
            operation,
            ordinal,
            cause: facts.last().copied(),
            audience: AudienceScope::Shared,
            value: FactValue::TimeAdvanced {
                before: current.state().logical_time,
                after: state.logical_time,
            },
        });
        facts.push(id);
    }
    let fact_id = make_id(facts.len() as u32)?;
    state.facts.push(GameFact {
        id: fact_id,
        revision: basis.revision,
        operation,
        ordinal: facts.len() as u32,
        cause: facts
            .last()
            .copied()
            .or_else(|| current.state().facts.last().map(|fact| fact.id)),
        audience,
        value: FactValue::ContentEvent {
            definition: model::content(entry)?,
            subjects: Vec::new(),
        },
    });
    facts.push(fact_id);
    state.decisions.push(AcceptedDecision {
        operation,
        revision: basis.revision,
        facts,
        draws: draws.iter().map(|draw| draw.ordinal).collect(),
        effects: Vec::new(),
        source_policy: model::label(policy)?,
        semantic_output: semantic,
    });
    state.draws.extend(draws);
    let candidate = model::checkpoint(basis, state)?;
    stage_authored_threads(candidate, fact_id)
}

fn stage_authored_threads(
    candidate: Checkpoint,
    source_id: FactId,
) -> Result<Checkpoint, RepositoryError> {
    use df_engine::director_staging::{
        DirectorLimits, DirectorStaging, ThreadDirectorLimits, compose_director_thread_progress,
    };
    use df_narrative::{
        ProgressLimits, ThreadCheckpointRequest, ThreadConsequenceSelection, ThreadDisposition,
    };
    let source = candidate
        .state()
        .facts
        .iter()
        .find(|fact| fact.id == source_id)
        .ok_or(RepositoryError::InvalidCandidate)?;
    let FactValue::ContentEvent { definition, .. } = &source.value else {
        return Err(RepositoryError::InvalidCandidate);
    };
    let mut admitted_thread = None;
    for &(event, thread) in THREAD_EVENTS {
        if *definition == model::content(event)? {
            admitted_thread = Some(model::content(thread)?);
            break;
        }
    }
    let Some(thread) = admitted_thread else {
        return Ok(candidate);
    };
    if !candidate.state().decisions.iter().any(|decision| {
        decision.operation == source.operation
            && decision.revision == source.revision
            && decision.source_policy.as_str() == THREAD_POLICY
            && decision.facts.contains(&source_id)
    }) {
        return Err(RepositoryError::InvalidCandidate);
    }
    let bandit = entity(BANDIT)?;
    let unconscious = model::label("unconscious")?;
    let source_rule = rule()?;
    let defeated = thread == model::content(THREAT_THREAD)? && candidate.state().facts.iter().any(|fact| {
        fact.operation == source.operation && fact.revision == source.revision && fact.ordinal < source.ordinal
            && matches!(&fact.value, FactValue::ResourceChanged { entity, resource, before: 0, after: 1, source }
                if *entity == bandit && *resource == unconscious && *source == source_rule)
    });
    let selections = [ThreadConsequenceSelection {
        thread: &thread,
        event_definition: definition,
        source: source_id,
        disposition: if defeated {
            ThreadDisposition::Resolve
        } else {
            ThreadDisposition::Continue
        },
    }];
    let policy = model::label(THREAD_POLICY)?;
    let pins = model::pins()?;
    let staged = compose_director_thread_progress(
        &candidate,
        &pins,
        df_world::DueSelectionRequest {
            expected_basis: candidate.basis(),
            target_time: candidate.state().logical_time,
            paused: false,
            deadline_remaining: Duration::from_secs(1),
            policy: &candidate.state().narrative.definition,
        },
        None,
        ThreadCheckpointRequest {
            expected_basis: candidate.basis(),
            admitted_pins: &pins,
            policy: &policy,
            expected_policy: &policy,
            inventory: ReferenceInventory {
                rules: &[model::rule()?, source_rule],
                content: &model::contents()?,
                resources: &resources()?,
                assets: &[],
            },
            checkpoint_limits: model::limits(),
            selections: &selections,
        },
        ThreadDirectorLimits {
            directors: DirectorLimits {
                maximum_checkpoint_bytes: model::limits().maximum_retained_bytes,
                maximum_pass_bytes: 8 * 1024 * 1024,
                maximum_relationships: 512,
                world: df_world::DueSelectionLimits {
                    queue_events: 512,
                    selected_events: 8,
                    output_bytes: 128 * 1024,
                },
            },
            narrative: ProgressLimits {
                records: 512,
                consequences: 1,
                work: 1024 * 1024,
            },
        },
    )
    .map_err(bad)?;
    match staged {
        DirectorStaging::Staged(staged) => Ok((*staged).into_candidate()),
        // The local slice has no scheduled/pending continuation producer. Refusal leaves the
        // current checkpoint and pending prerequisites untouched; nothing reaches session commit.
        DirectorStaging::RulesPending(_)
        | DirectorStaging::WorldPending(_)
        | DirectorStaging::MissingEnvironmentalProducer => Err(RepositoryError::InvalidCandidate),
    }
}

fn command(input: &GameInput) -> Result<&CommandInput, RepositoryError> {
    match input {
        GameInput::Game(command) => Ok(command),
        _ => Err(RepositoryError::InvalidCandidate),
    }
}
fn choice<'a>(
    choices: &'a [(df_types::RevisionLabel, df_types::RevisionLabel)],
    key: &str,
) -> Result<&'a str, RepositoryError> {
    choices
        .iter()
        .find(|(label, _)| label.as_str() == key)
        .map(|(_, value)| value.as_str())
        .ok_or(RepositoryError::InvalidCandidate)
}
type DiceSource<'a> = &'a mut dyn FnMut(u32) -> Result<u32, RepositoryError>;
fn sample_face(sides: u32) -> Result<u32, RepositoryError> {
    if !matches!(sides, 6 | 10 | 20) {
        return Err(RepositoryError::InvalidCandidate);
    }
    for _ in 0..16 {
        let mut bytes = [0_u8; 1];
        use std::io::Read;
        std::fs::File::open("/dev/urandom")
            .and_then(|mut source| source.read_exact(&mut bytes))
            .map_err(|_| RepositoryError::Unavailable)?;
        let [byte] = bytes;
        let bucket = 256 - (256 % sides);
        if u32::from(byte) < bucket {
            return Ok(u32::from(byte) % sides + 1);
        }
    }
    Err(RepositoryError::Unavailable)
}
fn draw(
    operation: OperationId,
    sides: u32,
    draws: &mut Vec<ActualDraw>,
    supplier: DiceSource<'_>,
) -> Result<u32, RepositoryError> {
    if draws.len() >= 32 {
        return Err(RepositoryError::Capacity);
    }
    let value = supplier(sides)?;
    if !(1..=sides).contains(&value) {
        return Err(RepositoryError::InvalidCandidate);
    }
    draws.push(ActualDraw {
        operation,
        ordinal: draws.len() as u32,
        resolution: ResolutionId::from_bytes(operation.as_bytes()).map_err(bad)?,
        window: WindowId::from_bytes(operation.as_bytes()).map_err(bad)?,
        sides,
        value,
        source: rule()?,
    });
    Ok(value)
}
#[derive(Clone, Copy)]
struct AttackOptions {
    savage: bool,
    graze: bool,
}
fn attack(
    state: &mut GameState,
    operation: OperationId,
    who: EntityId,
    target: EntityId,
    options: AttackOptions,
    draws: &mut Vec<ActualDraw>,
    supplier: DiceSource<'_>,
) -> Result<rpc::CombatOutcome, RepositoryError> {
    let AttackOptions { savage, graze } = options;
    if value(state, who, "unconscious")? != 0 || value(state, target, "unconscious")? != 0 {
        return Err(RepositoryError::InvalidCandidate);
    }
    let monster = who == entity(BANDIT)?;
    let ability = if monster {
        1
    } else {
        rules::ability_modifier(value(state, who, "strength")? as u8)
    };
    let modifier = ability + 2;
    let die = draw(operation, 20, draws, supplier)?;
    let ac = value(state, target, "armor-class")?;
    let hit = die != 1 && (die == 20 || die as i32 + modifier >= ac as i32);
    let mut dice = Vec::new();
    let count = (if monster { 1 } else { 2 })
        * (if die == 20 { 2 } else { 1 })
        * (if savage { 2 } else { 1 });
    if hit {
        for _ in 0..count {
            dice.push(draw(operation, 6, draws, supplier)?);
        }
    }
    let outcome = rules::resolve_melee(rules::NormalMeleeAttack {
        die,
        modifier,
        armor_class: ac,
        damage_dice: &dice,
        weapon_dice: if monster { 1 } else { 2 },
        damage_modifier: ability,
        savage_attacker_take_higher: savage,
        graze,
        target_hit_points: value(state, target, "hit-points")?,
    })
    .map_err(bad)?;
    set(state, target, "hit-points", outcome.target_hit_points)?;
    if outcome.knocked_out {
        set(state, target, "unconscious", 1)?;
    }
    Ok(rpc::CombatOutcome {
        actor_name: name(state, who)?,
        target_name: name(state, target)?,
        attack_die: die,
        attack_modifier: modifier,
        attack_total: outcome.total,
        target_armor_class: ac,
        hit: outcome.hit,
        critical: outcome.critical,
        damage_dice: outcome.damage_dice,
        damage: outcome.damage,
        target_hit_points: outcome.target_hit_points,
        knocked_out: outcome.knocked_out,
        grazed: outcome.grazed,
        source_revision: rules::SOURCE_REVISION.to_owned(),
        actor_id: who.as_bytes().to_vec(),
        target_id: target.as_bytes().to_vec(),
    })
}
fn finish_combat(state: &mut GameState) -> Result<bool, RepositoryError> {
    let enemy = entity(BANDIT)?;
    let all_players_down = state
        .characters
        .iter()
        .map(|character| value(state, character.entity, "unconscious"))
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .all(|value| *value != 0);
    if value(state, enemy, "unconscious")? != 0 || all_players_down {
        state
            .encounters
            .first_mut()
            .ok_or(RepositoryError::InvalidCandidate)?
            .active_turn = None;
        beat(state, "complete")?;
        return Ok(true);
    }
    Ok(false)
}
fn advance(state: &mut GameState) -> Result<(), RepositoryError> {
    let encounter = state
        .encounters
        .first()
        .ok_or(RepositoryError::InvalidCandidate)?;
    let who = encounter
        .active_turn
        .ok_or(RepositoryError::InvalidCandidate)?;
    let order = encounter.turn_order.clone();
    let index = order
        .iter()
        .position(|value| *value == who)
        .ok_or(RepositoryError::InvalidCandidate)?;
    for offset in 1..=order.len() {
        let next = (index + offset) % order.len();
        if value(state, order[next], "unconscious")? == 0 {
            if next <= index {
                state.logical_time.ticks = state
                    .logical_time
                    .ticks
                    .checked_add(6)
                    .ok_or(RepositoryError::Capacity)?;
            }
            state
                .encounters
                .first_mut()
                .ok_or(RepositoryError::InvalidCandidate)?
                .active_turn = Some(order[next]);
            set(state, order[next], "action-used", 0)?;
            set(state, order[next], "bonus-used", 0)?;
            return Ok(());
        }
    }
    Err(RepositoryError::InvalidCandidate)
}
fn enemy_turn(
    state: &mut GameState,
    operation: OperationId,
    draws: &mut Vec<ActualDraw>,
    outcomes: &mut Vec<rpc::CombatOutcome>,
    supplier: DiceSource<'_>,
) -> Result<(), RepositoryError> {
    if finish_combat(state)? {
        return Ok(());
    }
    let monster = entity(BANDIT)?;
    if state
        .encounters
        .first()
        .ok_or(RepositoryError::InvalidCandidate)?
        .active_turn
        != Some(monster)
    {
        return Ok(());
    }
    let target = state
        .characters
        .iter()
        .find(|character| value(state, character.entity, "unconscious") == Ok(0))
        .map(|character| character.entity)
        .ok_or(RepositoryError::InvalidCandidate)?;
    outcomes.push(attack(
        state,
        operation,
        monster,
        target,
        AttackOptions {
            savage: false,
            graze: false,
        },
        draws,
        supplier,
    )?);
    set(state, monster, "action-used", 1)?;
    if !finish_combat(state)? {
        advance(state)?;
    }
    Ok(())
}
fn start_combat(
    state: &mut GameState,
    operation: OperationId,
    draws: &mut Vec<ActualDraw>,
    outcomes: &mut Vec<rpc::CombatOutcome>,
    supplier: DiceSource<'_>,
) -> Result<(), RepositoryError> {
    let monster = entity(BANDIT)?;
    state.entities.push(WorldEntity {
        id: monster,
        definition: model::content("bandit")?,
        location: Some(room_entity()?),
        position: Some(Position { x: 0, y: 0, z: 0 }),
        identity_revision: model::label("srd521-bandit-1")?,
    });
    for (key, value) in [
        ("hit-points", 11),
        ("armor-class", 12),
        ("strength", 11),
        ("dexterity", 12),
        ("constitution", 12),
        ("intelligence", 10),
        ("wisdom", 10),
        ("charisma", 10),
        ("second-wind", 0),
        ("unconscious", 0),
        ("action-used", 0),
        ("bonus-used", 0),
    ] {
        add_resource(state, monster, key, value)?;
    }
    let participants = state
        .members
        .iter()
        .filter(|link| link.member.as_bytes() != &PLAYER)
        .filter_map(|link| link.character)
        .chain([monster])
        .collect::<Vec<_>>();
    let mut initiative = Vec::new();
    for (priority, who) in participants.iter().enumerate() {
        let die = draw(operation, 20, draws, supplier)?;
        let dex = rules::ability_modifier(value(state, *who, "dexterity")? as u8);
        initiative.push((*who, die as i32 + dex, priority));
    }
    initiative.sort_by_key(|(_, count, priority)| (std::cmp::Reverse(*count), *priority));
    let order = initiative
        .into_iter()
        .map(|(who, _, _)| who)
        .collect::<Vec<_>>();
    let active = order
        .first()
        .copied()
        .ok_or(RepositoryError::InvalidCandidate)?;
    state.encounters.push(EncounterState {
        id: RecordId::from_bytes(&ENCOUNTER).map_err(bad)?,
        definition: model::content("combat")?,
        participants,
        turn_order: order,
        active_turn: Some(active),
        objectives: vec![model::content("defend-courier")?],
        combat_policy: model::content("normal-nonlethal-melee")?,
    });
    beat(state, "combat")?;
    enemy_turn(state, operation, draws, outcomes, supplier)
}
fn accept_character(
    state: &mut GameState,
    who: EntityId,
    member: MemberId,
    choices: &[(df_types::RevisionLabel, df_types::RevisionLabel)],
) -> Result<(), RepositoryError> {
    let encoded = choice(choices, "name")?;
    let encoded_name = if encoded == "empty-name" {
        Vec::new()
    } else {
        unhex(encoded)?
    };
    let name = std::str::from_utf8(&encoded_name).map_err(bad)?;
    let options = choices
        .iter()
        .filter(|(group, _)| group.as_str() != "name")
        .map(|(group, option)| (group.as_str(), option.as_str()))
        .collect::<Vec<_>>();
    let build = rules::validate_character(name, &options).map_err(bad)?;
    if state
        .characters
        .iter()
        .any(|character| character.entity == who)
    {
        return Err(RepositoryError::InvalidCandidate);
    }
    let source = rule()?;
    let name_label = model::label(name)?;
    let selected = choices
        .iter()
        .map(|(offer, selected)| AcceptedChoice {
            participant: member,
            offer: offer.clone(),
            selected: if offer.as_str() == "name" {
                name_label.clone()
            } else {
                selected.clone()
            },
            source: source.clone(),
        })
        .collect::<Vec<_>>();
    state.characters.push(CharacterState {
        entity: who,
        build: model::content("dwarf-fighter-soldier")?,
        owner: member,
        choices: selected.clone(),
    });
    state.continuity.creation.push(CharacterDraft {
        entity: who,
        member,
        ancestry: Some(model::content("dwarf-fighter-soldier")?),
        background: Some(model::content("dwarf-fighter-soldier")?),
        classes: vec![model::content("dwarf-fighter-soldier")?],
        choices: selected,
        phase: CreationPhase::Accepted,
    });
    for (key, value) in [
        ("hit-points", build.hit_points),
        ("armor-class", build.armor_class),
        ("strength", u32::from(build.abilities[0])),
        ("dexterity", u32::from(build.abilities[1])),
        ("constitution", u32::from(build.abilities[2])),
        ("intelligence", u32::from(build.abilities[3])),
        ("wisdom", u32::from(build.abilities[4])),
        ("charisma", u32::from(build.abilities[5])),
        ("second-wind", build.second_wind_uses),
        ("unconscious", 0),
        ("action-used", 0),
        ("bonus-used", 0),
    ] {
        add_resource(state, who, key, i64::from(value))?;
    }
    // The source-selected bundles are inventory, not cosmetic rewards or generated loot.
    for (index, (quantity, entry)) in [
        (1, "chain-mail"),
        (1, "greatsword"),
        (1, "flail"),
        (8, "javelin"),
        (1, "dungeoneer-pack"),
        (1, "spear"),
        (1, "shortbow"),
        (20, "arrow"),
        (1, "gaming-set"),
        (1, "healers-kit"),
        (1, "quiver"),
        (1, "travelers-clothes"),
        (18, "gold-piece"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut id = *who.as_bytes();
        id[0] ^= 0x80;
        id[15] ^= (index as u8) + 1;
        state.entities.push(WorldEntity {
            id: entity(id)?,
            definition: model::content(entry)?,
            location: Some(who),
            position: None,
            identity_revision: model::label("source-starting-equipment-1")?,
        });
        state.inventory.push(InventoryItem {
            item: entity(id)?,
            owner: who,
            quantity,
            source: rule()?,
            origin: model::content(entry)?,
            attunement_owner: None,
        });
    }
    Ok(())
}
fn stage_using(
    current: &Checkpoint,
    input: &GameInput,
    supplier: DiceSource<'_>,
) -> Result<Checkpoint, RepositoryError> {
    current
        .validate_resume(current.basis(), &model::pins()?)
        .map_err(bad)?;
    df_model::commands::validate_client_command(
        input,
        current,
        ReferenceInventory {
            rules: &[model::rule()?, rule()?],
            content: &model::contents()?,
            resources: &resources()?,
            assets: &[],
        },
        df_model::commands::CommandLimits {
            maximum_records: 32,
            maximum_text_bytes: 128,
            maximum_retained_bytes: 8192,
        },
    )
    .map_err(bad)?;
    let command = command(input)?;
    let GameCommand::ProposeAction {
        actor: who,
        action,
        targets,
        choices,
    } = &command.command
    else {
        return Err(RepositoryError::InvalidCandidate);
    };
    if *who != player_entity(command.member, current)? || !targets.is_empty() {
        return Err(RepositoryError::Unauthorized);
    }
    let kind = wire::action_kind(action.entry.as_str()).ok_or(RepositoryError::InvalidCandidate)?;
    if !offered(current, command.member)?
        .iter()
        .any(|(offered, _)| *offered == kind)
    {
        return Err(RepositoryError::InvalidCandidate);
    }
    let mut basis = current.basis();
    basis.revision = basis.revision.next_sequence().map_err(bad)?;
    let mut state = current.state().clone();
    let mut draws = Vec::new();
    let mut outcomes = Vec::new();
    let mut private = false;
    match kind {
        rpc::GameplayActionKind::CreateCharacter => {
            accept_character(&mut state, *who, command.member, choices)?
        }
        rpc::GameplayActionKind::BeginStory => {
            if !choices.is_empty() {
                return Err(RepositoryError::InvalidCandidate);
            }
            beat(&mut state, "opening")?;
            state
                .narrative
                .open_threads
                .push(model::content(PACKET_THREAD)?);
        }
        rpc::GameplayActionKind::AskCourier | rpc::GameplayActionKind::EscortCourier => {
            if !choices.is_empty() {
                return Err(RepositoryError::InvalidCandidate);
            }
            let answer = if kind == rpc::GameplayActionKind::AskCourier {
                "courier-answer-seal"
            } else {
                "courier-answer-escort"
            };
            beat(&mut state, answer)?;
            private = kind == rpc::GameplayActionKind::AskCourier;
        }
        rpc::GameplayActionKind::DefendCourier => {
            if !choices.is_empty() {
                return Err(RepositoryError::InvalidCandidate);
            }
            state
                .narrative
                .open_threads
                .push(model::content(THREAT_THREAD)?);
            start_combat(
                &mut state,
                command.operation,
                &mut draws,
                &mut outcomes,
                supplier,
            )?;
        }
        rpc::GameplayActionKind::GreatswordAttack => {
            let savage = choice(choices, "savage-attacker")? == "yes";
            let graze = choice(choices, "graze")? == "yes";
            let economy = rules::ActionEconomy {
                action_used: value(&state, *who, "action-used")? != 0,
                bonus_action_used: value(&state, *who, "bonus-used")? != 0,
            }
            .attack()
            .map_err(bad)?;
            outcomes.push(attack(
                &mut state,
                command.operation,
                *who,
                entity(BANDIT)?,
                AttackOptions { savage, graze },
                &mut draws,
                supplier,
            )?);
            set(
                &mut state,
                *who,
                "action-used",
                u32::from(economy.action_used),
            )?;
            finish_combat(&mut state)?;
        }
        rpc::GameplayActionKind::SecondWind => {
            if !choices.is_empty() {
                return Err(RepositoryError::InvalidCandidate);
            }
            let economy = rules::ActionEconomy {
                action_used: value(&state, *who, "action-used")? != 0,
                bonus_action_used: value(&state, *who, "bonus-used")? != 0,
            }
            .second_wind(value(&state, *who, "second-wind")?)
            .map_err(bad)?;
            let die = draw(command.operation, 10, &mut draws, supplier)?;
            let (hp, uses) = rules::heal_second_wind(
                die,
                value(&state, *who, "hit-points")?,
                13,
                value(&state, *who, "second-wind")?,
            )
            .map_err(bad)?;
            set(&mut state, *who, "hit-points", hp)?;
            set(&mut state, *who, "second-wind", uses)?;
            set(
                &mut state,
                *who,
                "bonus-used",
                u32::from(economy.bonus_action_used),
            )?;
        }
        rpc::GameplayActionKind::EndTurn => {
            if !choices.is_empty() {
                return Err(RepositoryError::InvalidCandidate);
            }
            advance(&mut state)?;
            enemy_turn(
                &mut state,
                command.operation,
                &mut draws,
                &mut outcomes,
                supplier,
            )?;
        }
        _ => return Err(RepositoryError::InvalidCandidate),
    }
    let result = rpc::AcceptedAction {
        check: None,
        scene: None,
        character: if kind == rpc::GameplayActionKind::CreateCharacter {
            Some(character_sheet(&state, *who)?)
        } else {
            None
        },
        combat: outcomes,
        phase: phase_from_state(&state)? as i32,
    };
    let semantic = Some(hex(&result.encode_to_vec()));
    let entry = if private {
        "private-courier-note"
    } else {
        action.entry.as_str()
    };
    finish(
        current,
        basis,
        state,
        command.operation,
        draws,
        DecisionContent {
            entry,
            semantic,
            policy: THREAD_POLICY,
            audience: if private {
                AudienceScope::Members(vec![command.member])
            } else {
                AudienceScope::Shared
            },
        },
    )
}
fn phase_from_state(state: &GameState) -> Result<rpc::JourneyPhase, RepositoryError> {
    match state
        .narrative
        .active_beats
        .first()
        .map(|beat| beat.entry.as_str())
    {
        Some("room") => Ok(rpc::JourneyPhase::Room),
        Some("opening") => Ok(rpc::JourneyPhase::Opening),
        Some("courier-answer-seal" | "courier-answer-escort") => Ok(rpc::JourneyPhase::Dialogue),
        Some("combat") => Ok(rpc::JourneyPhase::Combat),
        Some("complete") => Ok(rpc::JourneyPhase::Complete),
        _ => Err(RepositoryError::InvalidCandidate),
    }
}

struct JourneyHandler {
    pins: CheckpointPins,
}
impl df_rules::RulesCommandHandler for JourneyHandler {
    type Rejection = RepositoryError;
    fn pins(&self) -> &CheckpointPins {
        &self.pins
    }
    fn stage(
        &self,
        input: df_rules::RulesCommandInput<'_>,
        current: &Checkpoint,
    ) -> Result<Checkpoint, RepositoryError> {
        let operation = command(input.command)?.operation;
        let mut supplied = input.supplied_draws.iter();
        let mut ordinal = 0;
        let mut consume = |sides| {
            let draw = supplied.next().ok_or(RepositoryError::InvalidCandidate)?;
            if draw.operation != operation
                || draw.ordinal != ordinal
                || draw.sides != sides
                || draw.source != rule()?
                || !(1..=sides).contains(&draw.value)
            {
                return Err(RepositoryError::InvalidCandidate);
            }
            ordinal += 1;
            Ok(draw.value)
        };
        let staged = stage_using(current, input.command, &mut consume)?;
        if supplied.next().is_some() {
            return Err(RepositoryError::InvalidCandidate);
        }
        Ok(staged)
    }
}
/// Native sampling is separate from the registered pure transition. Every actual
/// source-qualified draw is supplied explicitly; retries stop at the PG operation lookup.
pub(super) fn stage(
    current: &Checkpoint,
    input: &GameInput,
) -> Result<Checkpoint, RepositoryError> {
    stage_with_supplier(current, input, &mut sample_face)
}

fn stage_with_supplier(
    current: &Checkpoint,
    input: &GameInput,
    supplier: DiceSource<'_>,
) -> Result<Checkpoint, RepositoryError> {
    let prepared = stage_using(current, input, supplier)?;
    let operation = command(input)?.operation;
    let draws = prepared
        .state()
        .draws
        .iter()
        .filter(|draw| draw.operation == operation)
        .cloned()
        .collect::<Vec<_>>();
    let pins = model::pins()?;
    let source = rule()?;
    let selector = model::label("local-journey-handler-1")?;
    let manifest = model::source_manifest();
    let entries = [df_content::catalog::CatalogEntry::new(&source, &manifest)];
    let catalog = df_content::catalog::CatalogSnapshot::from_published(
        &pins.rules.catalog,
        &pins,
        &manifest,
        &entries,
        df_content::catalog::CatalogLimits {
            max_complete_bytes: 4096,
            max_entries: 1,
            max_item_bytes: 4096,
            max_total_item_bytes: 4096,
        },
    )
    .map_err(bad)?;
    let handler = JourneyHandler { pins: pins.clone() };
    let registrations = [df_rules::HandlerRegistration::new(
        &selector, &source, &handler,
    )];
    let registry =
        df_rules::DispatchRegistry::from_catalog(catalog, &registrations, 1).map_err(bad)?;
    df_engine::command_entry::decide_registered_command(
        df_rules::RulesCommandInput {
            command: input,
            supplied_draws: &draws,
        },
        current,
        df_engine::command_entry::CommandEntryContext {
            current_basis: current.basis(),
            admitted_pins: &pins,
            inventory: ReferenceInventory {
                rules: &[model::rule()?, source.clone()],
                content: &model::contents()?,
                resources: &resources()?,
                assets: &[],
            },
            limits: df_engine::command_entry::CommandEntryLimits {
                command: df_model::commands::CommandLimits {
                    maximum_records: 32,
                    maximum_text_bytes: 128,
                    maximum_retained_bytes: 8192,
                },
                maximum_staged_bytes: 1024 * 1024,
            },
        },
        &registry,
        &selector,
        &source,
    )
    .map_err(bad)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input(
        current: &Checkpoint,
        member: MemberId,
        operation: u8,
        entry: &str,
        choices: Vec<(df_types::RevisionLabel, df_types::RevisionLabel)>,
    ) -> GameInput {
        GameInput::Game(CommandInput {
            basis: current.basis(),
            operation: OperationId::from_bytes(&[operation; 16]).expect("operation"),
            member,
            observed_revision: current.basis().revision,
            command: GameCommand::ProposeAction {
                actor: if member == bootstrap_member().expect("bootstrap") {
                    room_entity().expect("room")
                } else {
                    player_entity(member, current).expect("joined")
                },
                action: model::content(entry).expect("admitted content"),
                targets: vec![],
                choices,
            },
        })
    }
    fn build(name: &str) -> Vec<(df_types::RevisionLabel, df_types::RevisionLabel)> {
        [
            ("name", hex(name.as_bytes())),
            ("species", "dwarf".to_owned()),
            ("class", "fighter-1".to_owned()),
            ("background", "soldier".to_owned()),
            ("array", "stalwart".to_owned()),
            ("alignment", "lawful-good".to_owned()),
            ("language-1", "dwarvish".to_owned()),
            ("language-2", "elvish".to_owned()),
            ("skill-1", "perception".to_owned()),
            ("skill-2", "survival".to_owned()),
            ("gaming-set", "dice".to_owned()),
            ("equipment", "fighter-a-soldier-a".to_owned()),
            (
                "style-masteries",
                "defense-greatsword-flail-javelin".to_owned(),
            ),
            ("allied-ties", "join-order".to_owned()),
        ]
        .into_iter()
        .map(|(key, value)| {
            (
                model::label(key).expect("key"),
                model::label(&value).expect("choice"),
            )
        })
        .collect()
    }
    #[test]
    fn empty_character_name_crosses_codec_but_cannot_create_or_draw() {
        let initial = initial().expect("empty room");
        let current = stage_join(
            &initial,
            &input(
                &initial,
                bootstrap_member().expect("bootstrap"),
                1,
                "join-room",
                vec![],
            ),
        )
        .expect("real joined member");
        let member = MemberId::from_bytes(&MEMBERS[0]).expect("joined member");
        let basis = current.basis();
        let request = rpc::SubmitActionRequest {
            session_id: Some(rpc::SessionId {
                value: Some(basis.session.as_bytes().to_vec()),
            }),
            run_id: Some(rpc::RunId {
                value: Some(basis.run.as_bytes().to_vec()),
            }),
            operation_id: Some(rpc::OperationId {
                value: Some(vec![2; 16]),
            }),
            observed_revision: Some(super::super::wire::revision(basis.revision)),
            offer_id: offer_id(&current, rpc::GameplayActionKind::CreateCharacter),
            action_kind: rpc::GameplayActionKind::CreateCharacter as i32,
            character: Some(rpc::CharacterSelection::default()),
            ..Default::default()
        };
        let converted = super::super::wire::journey_input(&request, member, &current)
            .expect("bounded semantic-invalid request must reach server validation");
        df_model::commands::validate_client_command(
            &converted,
            &current,
            ReferenceInventory {
                rules: &[
                    model::rule().expect("existing rule"),
                    rule().expect("journey rule"),
                ],
                content: &model::contents().expect("content"),
                resources: &resources().expect("resources"),
                assets: &[],
            },
            df_model::commands::CommandLimits {
                maximum_records: 32,
                maximum_text_bytes: 128,
                maximum_retained_bytes: 8192,
            },
        )
        .expect("canonical structural admission");
        assert_eq!(
            rules::validate_character("", &[]),
            Err(rules::JourneyRuleError::InvalidName)
        );
        let before = current.clone();
        let mut calls = 0;
        assert!(
            stage_using(&current, &converted, &mut |_| {
                calls += 1;
                Ok(1)
            })
            .is_err()
        );
        assert_eq!(calls, 0);
        assert_eq!(current, before);
        assert!(current.state().characters.is_empty());
    }
    #[test]
    fn reverse_character_finalization_preserves_join_order_initiative_and_private_dialogue() {
        let mut current = initial().expect("empty room");
        for op in [1, 2] {
            current = stage_join(
                &current,
                &input(
                    &current,
                    bootstrap_member().expect("bootstrap"),
                    op,
                    "join-room",
                    vec![],
                ),
            )
            .expect("actual join");
        }
        let first = MemberId::from_bytes(&MEMBERS[0]).expect("first");
        let second = MemberId::from_bytes(&MEMBERS[1]).expect("second");
        let no_draw = |_: u32| Err(RepositoryError::InvalidCandidate);
        let mut supply = no_draw;
        current = stage_using(
            &current,
            &input(&current, second, 3, "create-character", build("Vale")),
            &mut supply,
        )
        .expect("second creates first");
        current = stage_using(
            &current,
            &input(&current, first, 4, "create-character", build("Brynn")),
            &mut supply,
        )
        .expect("first creates second");
        current = stage_using(
            &current,
            &input(&current, first, 5, "begin-story", vec![]),
            &mut supply,
        )
        .expect("opening");
        current = stage_using(
            &current,
            &input(&current, first, 6, "ask-courier", vec![]),
            &mut supply,
        )
        .expect("dialogue saved");
        let private = current.state().facts.last().expect("fact");
        assert_eq!(private.audience, AudienceScope::Members(vec![first]));
        let mut equal = |_: u32| Ok(10);
        current = stage_using(
            &current,
            &input(&current, first, 7, "defend-courier", vec![]),
            &mut equal,
        )
        .expect("initiative");
        let encounter = current.state().encounters.first().expect("encounter");
        assert_eq!(
            encounter.turn_order,
            vec![
                entity(ENTITIES[0]).expect("first entity"),
                entity(ENTITIES[1]).expect("second entity"),
                entity(BANDIT).expect("bandit")
            ]
        );
        assert_eq!(
            encounter.active_turn,
            Some(entity(ENTITIES[0]).expect("first entity"))
        );
        assert_eq!(current.state().draws.len(), 3);
        let wrong = input(
            &current,
            second,
            8,
            "greatsword-attack",
            vec![
                (
                    model::label("savage-attacker").expect("key"),
                    model::label("no").expect("option"),
                ),
                (
                    model::label("graze").expect("key"),
                    model::label("no").expect("option"),
                ),
            ],
        );
        let mut forbid = |_: u32| panic!("wrong turn must never draw");
        assert!(stage_using(&current, &wrong, &mut forbid).is_err());
        assert_eq!(current.state().draws.len(), 3);
    }

    fn prepared_story() -> Checkpoint {
        let mut current = initial().expect("initial room");
        for op in [1, 2] {
            current = stage_join(
                &current,
                &input(
                    &current,
                    bootstrap_member().unwrap(),
                    op,
                    "join-room",
                    vec![],
                ),
            )
            .expect("joined member");
        }
        for (op, bytes, name) in [(3, MEMBERS[0], "Brynn"), (4, MEMBERS[1], "Vale")] {
            current = stage_with_supplier(
                &current,
                &input(
                    &current,
                    MemberId::from_bytes(&bytes).unwrap(),
                    op,
                    "create-character",
                    build(name),
                ),
                &mut |_| panic!("creation cannot draw"),
            )
            .expect("registered character outcome");
        }
        current
    }

    fn opening_story() -> Checkpoint {
        let current = prepared_story();
        stage_with_supplier(
            &current,
            &input(
                &current,
                MemberId::from_bytes(&MEMBERS[0]).unwrap(),
                5,
                "begin-story",
                vec![],
            ),
            &mut |_| panic!("opening cannot draw"),
        )
        .expect("registered opening")
    }

    #[test]
    fn registered_narrative_tracks_private_packet_and_ignores_unmapped_events() {
        let ready = prepared_story();
        assert!(ready.state().narrative.accepted_facts.is_empty());
        assert!(ready.state().narrative.open_threads.is_empty());
        let current = opening_story();
        assert_eq!(
            current.state().narrative.open_threads,
            vec![model::content(PACKET_THREAD).unwrap()]
        );
        assert_eq!(current.state().narrative.accepted_facts.len(), 1);
        let member = MemberId::from_bytes(&MEMBERS[0]).unwrap();
        let command = input(&current, member, 6, "ask-courier", vec![]);
        let private =
            stage_with_supplier(&current, &command, &mut |_| panic!("dialogue cannot draw"))
                .expect("private accepted source");
        let replay = stage_with_supplier(&current, &command, &mut |_| panic!("replay cannot draw"))
            .expect("same immutable source");
        assert_eq!(private, replay);
        assert_eq!(
            private.state().narrative.open_threads,
            current.state().narrative.open_threads
        );
        assert_eq!(private.state().narrative.accepted_facts.len(), 2);
        let source = private.state().facts.last().unwrap();
        assert_eq!(source.audience, AudienceScope::Members(vec![member]));
        assert!(private.state().knowledge.is_empty());
        assert!(private.state().draws.is_empty());
        assert_eq!(
            private
                .state()
                .decisions
                .last()
                .unwrap()
                .source_policy
                .as_str(),
            THREAD_POLICY
        );
        assert_eq!(
            accepted(private.state().decisions.last().unwrap())
                .unwrap()
                .phase,
            rpc::JourneyPhase::Dialogue as i32
        );
        assert_eq!(
            stage_authored_threads(private.clone(), source.id),
            Err(RepositoryError::InvalidCandidate)
        );
        assert!(
            stage_with_supplier(&private, &command, &mut |_| panic!(
                "stale command cannot draw"
            ))
            .is_err()
        );
        let untouched = private.clone();
        let mut old_pins = private.pins().clone();
        old_pins.content.package_digest = ContentDigest([99; 32]);
        let stale = Checkpoint::new(
            private.schema(),
            private.basis(),
            old_pins,
            private.state().clone(),
            ReferenceInventory {
                rules: &[model::rule().unwrap(), rule().unwrap()],
                content: &model::contents().unwrap(),
                resources: &resources().unwrap(),
                assets: &[],
            },
            model::limits(),
        )
        .unwrap();
        assert!(
            stage_with_supplier(
                &stale,
                &input(&stale, member, 7, "defend-courier", vec![]),
                &mut |_| Ok(10)
            )
            .is_err()
        );
        assert_eq!(private, untouched);
    }

    #[test]
    fn registered_hit_preserves_threat_until_actual_knockout_then_retains_packet_branch() {
        let member = MemberId::from_bytes(&MEMBERS[0]).unwrap();
        let mut current = opening_story();
        current = stage_with_supplier(
            &current,
            &input(&current, member, 6, "escort-courier", vec![]),
            &mut |_| panic!("escort cannot draw"),
        )
        .unwrap();
        current = stage_with_supplier(
            &current,
            &input(&current, member, 7, "defend-courier", vec![]),
            &mut |_| Ok(10),
        )
        .unwrap();
        let threads = vec![
            model::content(PACKET_THREAD).unwrap(),
            model::content(THREAT_THREAD).unwrap(),
        ];
        assert_eq!(current.state().narrative.open_threads, threads);
        assert_eq!(current.state().draws.len(), 3);
        let attack_choices = || {
            vec![
                (
                    model::label("savage-attacker").unwrap(),
                    model::label("no").unwrap(),
                ),
                (model::label("graze").unwrap(), model::label("no").unwrap()),
            ]
        };
        let command = input(&current, member, 8, "greatsword-attack", attack_choices());
        current = stage_with_supplier(&current, &command, &mut |sides| {
            Ok(if sides == 20 { 10 } else { 1 })
        })
        .unwrap();
        assert_eq!(
            value(current.state(), entity(BANDIT).unwrap(), "unconscious").unwrap(),
            0
        );
        assert_eq!(current.state().narrative.open_threads, threads);
        assert_eq!(current.state().draws.len(), 6);
        current = stage_with_supplier(
            &current,
            &input(&current, member, 9, "end-turn", vec![]),
            &mut |_| panic!("next player needs no enemy draw"),
        )
        .unwrap();
        let second = MemberId::from_bytes(&MEMBERS[1]).unwrap();
        let before = current.clone();
        let command = input(&current, second, 10, "greatsword-attack", attack_choices());
        let defeated = stage_with_supplier(&current, &command, &mut |sides| {
            Ok(if sides == 20 { 10 } else { 6 })
        })
        .unwrap();
        assert_eq!(
            value(defeated.state(), entity(BANDIT).unwrap(), "unconscious").unwrap(),
            1
        );
        assert_eq!(
            value(defeated.state(), entity(BANDIT).unwrap(), "hit-points").unwrap(),
            1
        );
        assert_eq!(
            defeated.state().narrative.open_threads,
            vec![model::content(PACKET_THREAD).unwrap()]
        );
        assert_eq!(phase(&defeated).unwrap(), rpc::JourneyPhase::Complete);
        assert_eq!(defeated.state().draws.len(), before.state().draws.len() + 3);
        assert_eq!(current, before);
        let repeated = stage_with_supplier(&current, &command, &mut |sides| {
            Ok(if sides == 20 { 10 } else { 6 })
        })
        .unwrap();
        assert_eq!(defeated, repeated);
        let decision = defeated.state().decisions.last().unwrap();
        let receipt = accepted(decision).unwrap();
        assert!(receipt.combat[0].knocked_out);
        assert_eq!(decision.source_policy.as_str(), THREAD_POLICY);
        assert!(defeated.state().facts.iter().any(|fact| decision.facts.contains(&fact.id)
            && matches!(&fact.value, FactValue::ResourceChanged { entity: who, resource, before: 0, after: 1, .. }
                if *who == entity(BANDIT).unwrap() && resource.as_str() == "unconscious")));
        assert!(defeated.state().continuity.catch_up.is_none());
    }

    #[test]
    fn registered_party_defeat_keeps_both_threads_unresolved() {
        let first = MemberId::from_bytes(&MEMBERS[0]).unwrap();
        let second = MemberId::from_bytes(&MEMBERS[1]).unwrap();
        let opening = opening_story();
        let dialogue = stage_with_supplier(
            &opening,
            &input(&opening, first, 6, "escort-courier", vec![]),
            &mut |_| panic!("escort cannot draw"),
        )
        .unwrap();
        let mut initiative_count = 0;
        let combat = stage_with_supplier(
            &dialogue,
            &input(&dialogue, first, 7, "defend-courier", vec![]),
            &mut |sides| {
                if sides == 20 {
                    initiative_count += 1;
                    Ok(if initiative_count <= 2 { 1 } else { 20 })
                } else {
                    Ok(6)
                }
            },
        )
        .unwrap();
        assert_eq!(
            value(combat.state(), entity(ENTITIES[0]).unwrap(), "unconscious").unwrap(),
            1
        );
        assert_eq!(
            value(combat.state(), entity(BANDIT).unwrap(), "unconscious").unwrap(),
            0
        );
        let defeated = stage_with_supplier(
            &combat,
            &input(&combat, second, 8, "end-turn", vec![]),
            &mut |sides| Ok(if sides == 20 { 20 } else { 6 }),
        )
        .unwrap();
        assert_eq!(phase(&defeated).unwrap(), rpc::JourneyPhase::Complete);
        assert_eq!(
            value(
                defeated.state(),
                entity(ENTITIES[1]).unwrap(),
                "unconscious"
            )
            .unwrap(),
            1
        );
        assert_eq!(
            value(defeated.state(), entity(BANDIT).unwrap(), "unconscious").unwrap(),
            0
        );
        assert_eq!(
            defeated.state().narrative.open_threads,
            vec![
                model::content(PACKET_THREAD).unwrap(),
                model::content(THREAT_THREAD).unwrap()
            ]
        );
    }
}
