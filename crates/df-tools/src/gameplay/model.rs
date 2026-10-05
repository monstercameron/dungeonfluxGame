use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_engine::command_entry::{
    CommandEntryContext, CommandEntryLimits, decide_registered_command,
};
use df_model::checkpoint::*;
use df_model::commands::{CommandLimits, validate_client_command};
use df_persistence::local_demo_scope::{LocalDemoAuthority, PLAYER};
use df_persistence::{NativeRecoverySource, NativeScope};
use df_rules::ability_check::{AbilityCheckInput, resolve};
use df_rules::{DispatchRegistry, HandlerRegistration, RulesCommandHandler, RulesCommandInput};
use df_session::submission::{RepositoryError, SessionEngine};
use df_types::{
    BuildIdentity, MemberId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision,
};
use sha2::{Digest, Sha256};
use std::io::Read;

pub(super) const SOURCE: &[u8] = b"SRD5.2.1 page6 D20 Tests Ability Checks; https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.1.pdf; Wizards of the Coast LLC; CC BY 4.0; normal single-d20 check only";
const CONTENT: &[u8] = b"DungeonFlux authored local journey4: share Lantern room, two actual independently joined members create source-selected Dwarf Fighter/Soldier heroes; their accepted selections and starting equipment persist. Begin at Lantern Wharf only after both builds; choosing to ask the courier reveals a member-private delivery clue, or escorting elicits a different shared response. Defending begins a normal adjacent nonlethal Bandit encounter with actual initiative, player-selected attacks/SecondWind/endturn, source-qualified damage, unconscious one-HP knockout and no wall-time rest completion. Source knockout starts persist due schedules; after combat a joined member may propose an uninterrupted campaign Short Rest (one hour, no Hit Point Dice/HP healing, Fighter one-use SecondWind recharge capped2). Knockout waking retains Prone and dropped held gear, and preserves the accepted battle outcome. Versioned local-journey-rpc-3-threads-1-rest-1 maps begin-story/private-courier-note/escort-courier events to continued sealed-packet-delivery-thread; defend-courier/greatsword-attack/second-wind/end-turn events continue dockside-threat-thread, resolving it only on the current operation accepted bandit unconscious transition. Delivery remains unresolved after combat; unmapped events consume no narrative source. Preconfigured Investigation/next-scene draft remains an earlier source lineage, not the active browser room.";

fn invalid<T>(_: T) -> RepositoryError {
    RepositoryError::InvalidCandidate
}
pub(super) fn label(value: &str) -> Result<RevisionLabel, RepositoryError> {
    RevisionLabel::new(Some(value)).map_err(invalid)
}
pub(super) fn rule() -> Result<RuleReference, RepositoryError> {
    Ok(RuleReference {
        catalog: label("srd521-journey-subset-2")?,
        source: label("srd521-page6")?,
        entry: label("d20-ability-check")?,
        clause: label("normal-proficient-total-versus-dc")?,
    })
}
pub(super) fn content(entry: &str) -> Result<ContentReference, RepositoryError> {
    Ok(ContentReference {
        package: label("harbor-investigation-demo-4-threads-1-rest-1")?,
        entry: label(entry)?,
    })
}
pub(super) fn contents() -> Result<Vec<ContentReference>, RepositoryError> {
    [
        "harbor",
        "mara-preconfigured",
        "inspect-seal",
        "clue-loading-pier",
        "seal-obscured",
        "choose-harbor-scene",
        "loading-pier",
        "lantern-wharf",
        "harbor-inn",
    ]
    .into_iter()
    .map(content)
    .chain(super::journey::CONTENT_ENTRIES.iter().copied().map(content))
    .collect()
}
pub(super) fn source_manifest() -> Vec<u8> {
    [SOURCE, df_rules::local_journey::SOURCE.as_bytes()].concat()
}
pub(super) fn pins() -> Result<CheckpointPins, RepositoryError> {
    Ok(CheckpointPins {
        rules: RulesPins {
            mode: RulesMode::Standard2024,
            ruleset: label("srd521-journey-subset-2")?,
            catalog: label("srd521-journey-subset-2")?,
            catalog_digest: ContentDigest(Sha256::digest(source_manifest()).into()),
            source_manifest: label("srd521-creation-combat-rest-reviewed-subset")?,
            source_manifest_digest: ContentDigest(Sha256::digest(source_manifest()).into()),
            handler: label("source-qualified-journey-compiled-2")?,
            handler_digest: ContentDigest(
                Sha256::digest(
                    [
                        include_bytes!("../../../df-rules/src/ability_check.rs").as_slice(),
                        include_bytes!("../../../df-rules/src/local_journey.rs").as_slice(),
                        include_bytes!("journey.rs").as_slice(),
                    ]
                    .concat(),
                )
                .into(),
            ),
        },
        content: ContentPins {
            content: label("harbor-investigation-demo-4-threads-1-rest-1")?,
            content_digest: ContentDigest(Sha256::digest(CONTENT).into()),
            package: label("harbor-investigation-demo-4-threads-1-rest-1")?,
            package_digest: ContentDigest(Sha256::digest(CONTENT).into()),
        },
        build: BuildIdentity::new(
            Some(crate::BUILD_ID),
            Some(crate::BUILD_ID),
            Some(crate::BUILD_ID),
            Some("local-gameplay-demo-config-1"),
            Some("harbor-investigation-demo-4-threads-1-rest-1"),
        )
        .map_err(invalid)?,
    })
}
pub(super) fn basis() -> Result<Basis, RepositoryError> {
    Ok(Basis {
        session: SessionId::from_bytes(&[0x41; 16]).map_err(invalid)?,
        run: RunId::from_bytes(&[0x42; 16]).map_err(invalid)?,
        revision: SessionRevision::new(RecoveryEpoch::new(1).map_err(invalid)?, 0),
    })
}
pub(super) fn actor() -> Result<EntityId, RepositoryError> {
    EntityId::from_bytes(&[0x44; 16]).map_err(invalid)
}
pub(super) fn member() -> Result<MemberId, RepositoryError> {
    MemberId::from_bytes(&PLAYER).map_err(invalid)
}
pub(super) fn limits() -> CheckpointLimits {
    CheckpointLimits {
        maximum_records: 512,
        maximum_text_bytes: 4096,
        maximum_total_text_bytes: 65536,
        maximum_retained_bytes: 1024 * 1024,
    }
}
pub(super) fn recovery() -> Result<NativeRecoverySource, RepositoryError> {
    Ok(NativeRecoverySource {
        rules: vec![rule()?, super::journey::rule()?],
        content: contents()?,
        resources: super::journey::resources()?,
        assets: vec![],
        limits: limits(),
    })
}
pub(super) fn checkpoint(basis: Basis, state: GameState) -> Result<Checkpoint, RepositoryError> {
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        basis,
        pins()?,
        state,
        ReferenceInventory {
            rules: &[rule()?, super::journey::rule()?],
            content: &contents()?,
            resources: &super::journey::resources()?,
            assets: &[],
        },
        limits(),
    )
    .map_err(invalid)
}

pub(super) fn initial() -> Result<Checkpoint, RepositoryError> {
    let harbor = content("harbor")?;
    let state = GameState {
        mode: ExecutionMode::PreparedOnly,
        logical_time: LogicalTime {
            ticks: 0,
            ticks_per_second: 1,
        },
        members: vec![MembershipLink {
            member: member()?,
            character: Some(actor()?),
        }],
        entities: vec![WorldEntity {
            id: actor()?,
            definition: content("mara-preconfigured")?,
            location: None,
            position: None,
            identity_revision: label("mara-preconfigured-1")?,
        }],
        characters: vec![CharacterState {
            entity: actor()?,
            build: content("mara-preconfigured")?,
            owner: member()?,
            choices: vec![],
        }],
        resources: vec![],
        inventory: vec![],
        facts: vec![],
        draws: vec![],
        decisions: vec![],
        pending: vec![],
        intents: vec![],
        timers: vec![],
        active_effects: vec![],
        knowledge: vec![],
        beliefs: vec![],
        memories: vec![],
        schedules: vec![],
        threats: vec![],
        relationships: vec![],
        conversations: vec![],
        obligations: vec![],
        narrative: NarrativeState {
            definition: harbor.clone(),
            active_beats: vec![harbor.clone()],
            completed_beats: vec![],
            open_threads: vec![],
            accepted_facts: vec![],
            remaining_budget: 1,
        },
        encounters: vec![],
        activity: vec![],
        tempo: TempoState {
            policy: harbor,
            presentation_ticks: 0,
            intensity: 0,
            inertia: 0,
            fatigue: vec![],
        },
        presentation: vec![],
        continuity: ContinuityState {
            creation: vec![],
            simulation: vec![],
            catch_up: None,
            environment: vec![],
            travel: vec![],
            witnesses: vec![],
            rumors: vec![],
            journal: vec![],
            summaries: vec![],
            retrieval: vec![],
            retrieved: vec![],
            consolidation: vec![],
            npcs: vec![],
            hooks: vec![],
            arcs: vec![],
            remote: None,
            presence: vec![],
            audio: None,
            private_offers: vec![],
            knowledge_cues: vec![],
            moments: vec![],
            demands: vec![],
            asset_jobs: vec![],
            asset_dependencies: vec![],
            canonical_packs: vec![],
            shots: vec![],
            prefetch: None,
            scenes: vec![],
            item_origins: vec![],
            bookends: vec![],
            exports: vec![],
            critical_cues: vec![],
            content_candidates: vec![],
            content_admissions: vec![],
            recovery: RecoveryState {
                origin: None,
                retired_epochs: vec![],
                lost_ranges: vec![],
                suppression_generation: 0,
                redacted_records: vec![],
                unavailable_sources: vec![],
            },
        },
    };
    checkpoint(basis()?, state)
}

pub(super) fn random<const N: usize>() -> Result<[u8; N], RepositoryError> {
    let mut bytes = [0; N];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|_| RepositoryError::Unavailable)?;
    if N > 1 && bytes.iter().all(|byte| *byte == 0) {
        return Err(RepositoryError::Unavailable);
    }
    Ok(bytes)
}
fn draw_d20() -> Result<u8, RepositoryError> {
    // Reject the final incomplete bucket; modulo of all 256 byte values is biased.
    for _ in 0..16 {
        let [byte] = random::<1>()?;
        if let Some(face) = d20_face(byte) {
            return Ok(face);
        }
    }
    Err(RepositoryError::Unavailable)
}
fn d20_face(byte: u8) -> Option<u8> {
    (byte < 240).then_some(byte % 20 + 1)
}
pub(super) struct HarborHandler {
    pins: CheckpointPins,
}
impl RulesCommandHandler for HarborHandler {
    type Rejection = RepositoryError;
    fn pins(&self) -> &CheckpointPins {
        &self.pins
    }
    fn stage(
        &self,
        input: RulesCommandInput<'_>,
        current: &Checkpoint,
    ) -> Result<Checkpoint, RepositoryError> {
        let GameInput::Game(command) = input.command else {
            return Err(RepositoryError::InvalidCandidate);
        };
        let GameCommand::ProposeAction {
            actor: who,
            action,
            targets,
            choices,
        } = &command.command
        else {
            return Err(RepositoryError::InvalidCandidate);
        };
        if *who != actor()?
            || command.member != member()?
            || *action != content("inspect-seal")?
            || !targets.is_empty()
            || !choices.is_empty()
            || !current.state().decisions.is_empty()
        {
            return Err(RepositoryError::InvalidCandidate);
        }
        let [draw] = input.supplied_draws else {
            return Err(RepositoryError::InvalidCandidate);
        };
        if draw.sides != 20 || draw.source != rule()? {
            return Err(RepositoryError::InvalidCandidate);
        }
        let result = resolve(AbilityCheckInput {
            die: u8::try_from(draw.value).map_err(invalid)?,
            ability_modifier: 2,
            proficiency_bonus: 2,
            difficulty_class: 15,
        })
        .map_err(invalid)?;
        let mut next = current.basis();
        next.revision = next.revision.next_sequence().map_err(invalid)?;
        let mut state = current.state().clone();
        let draw_fact = FactId::from_bytes(&[0x51; 16]).map_err(invalid)?;
        let result_fact = FactId::from_bytes(&[0x52; 16]).map_err(invalid)?;
        state.draws.push(draw.clone());
        state.facts.push(GameFact {
            id: draw_fact,
            revision: next.revision,
            operation: command.operation,
            ordinal: 0,
            cause: None,
            audience: AudienceScope::Members(vec![member()?]),
            value: FactValue::DrawAccepted {
                operation: command.operation,
                ordinal: 0,
            },
        });
        state.facts.push(GameFact {
            id: result_fact,
            revision: next.revision,
            operation: command.operation,
            ordinal: 1,
            cause: Some(draw_fact),
            audience: AudienceScope::Members(vec![member()?]),
            value: FactValue::ContentEvent {
                definition: content(if result.succeeded {
                    "clue-loading-pier"
                } else {
                    "seal-obscured"
                })?,
                subjects: vec![actor()?],
            },
        });
        state.decisions.push(AcceptedDecision {
            operation: command.operation,
            revision: next.revision,
            facts: vec![draw_fact, result_fact],
            draws: vec![0],
            effects: vec![],
            source_policy: label("srd521-journey-subset-2")?,
            semantic_output: None,
        });
        state.narrative.remaining_budget = 0;
        checkpoint(next, state)
    }
}

pub(super) struct HarborEngine;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HarborScene {
    Harbor,
    LoadingPier,
    LanternWharf,
    HarborInn,
}
impl HarborScene {
    pub fn entry(self) -> &'static str {
        match self {
            Self::Harbor => "harbor",
            Self::LoadingPier => "loading-pier",
            Self::LanternWharf => "lantern-wharf",
            Self::HarborInn => "harbor-inn",
        }
    }
    pub fn from_entry(entry: &str) -> Result<Self, RepositoryError> {
        match entry {
            "harbor" => Ok(Self::Harbor),
            "loading-pier" => Ok(Self::LoadingPier),
            "lantern-wharf" => Ok(Self::LanternWharf),
            "harbor-inn" => Ok(Self::HarborInn),
            _ => Err(RepositoryError::InvalidCandidate),
        }
    }
}

pub(super) fn scene(current: &Checkpoint) -> Result<HarborScene, RepositoryError> {
    let [active] = current.state().narrative.active_beats.as_slice() else {
        return Err(RepositoryError::InvalidCandidate);
    };
    HarborScene::from_entry(active.entry.as_str())
}

pub(super) fn investigation_succeeded(
    current: &Checkpoint,
) -> Result<Option<bool>, RepositoryError> {
    if current.state().decisions.is_empty() {
        return Ok(None);
    }
    let result = current
        .state()
        .facts
        .iter()
        .find(|fact| {
            matches!(&fact.value, FactValue::ContentEvent { definition, .. }
            if definition.entry.as_str() == "clue-loading-pier"
                || definition.entry.as_str() == "seal-obscured")
        })
        .ok_or(RepositoryError::InvalidCandidate)?;
    match &result.value {
        FactValue::ContentEvent { definition, .. } => {
            Ok(Some(*definition == content("clue-loading-pier")?))
        }
        _ => Err(RepositoryError::InvalidCandidate),
    }
}

pub(super) fn destinations(current: &Checkpoint) -> Result<Vec<HarborScene>, RepositoryError> {
    if scene(current)? != HarborScene::Harbor {
        return Ok(Vec::new());
    }
    match investigation_succeeded(current)? {
        None => Ok(Vec::new()),
        Some(true) => Ok(vec![HarborScene::LoadingPier, HarborScene::LanternWharf]),
        Some(false) => Ok(vec![HarborScene::LanternWharf, HarborScene::HarborInn]),
    }
}

fn stage_scene(current: &Checkpoint, input: &GameInput) -> Result<Checkpoint, RepositoryError> {
    let GameInput::Game(command) = input else {
        return Err(RepositoryError::InvalidCandidate);
    };
    let GameCommand::ProposeAction {
        actor: who,
        action,
        targets,
        choices,
    } = &command.command
    else {
        return Err(RepositoryError::InvalidCandidate);
    };
    let [(input_id, selected)] = choices.as_slice() else {
        return Err(RepositoryError::InvalidCandidate);
    };
    let destination = HarborScene::from_entry(selected.as_str())?;
    if *who != actor()?
        || command.member != member()?
        || *action != content("choose-harbor-scene")?
        || !targets.is_empty()
        || *input_id != label("harbor-destination")?
        || current.state().decisions.len() != 1
        || !destinations(current)?.contains(&destination)
    {
        return Err(RepositoryError::InvalidCandidate);
    }
    validate_client_command(
        input,
        current,
        ReferenceInventory {
            rules: &[rule()?],
            content: &contents()?,
            resources: &[],
            assets: &[],
        },
        CommandLimits {
            maximum_records: 16,
            maximum_text_bytes: 128,
            maximum_retained_bytes: 8192,
        },
    )
    .map_err(invalid)?;
    let mut next = current.basis();
    next.revision = next.revision.next_sequence().map_err(invalid)?;
    let mut state = current.state().clone();
    let fact = FactId::from_bytes(&[0x53; 16]).map_err(invalid)?;
    state.facts.push(GameFact {
        id: fact,
        revision: next.revision,
        operation: command.operation,
        ordinal: 0,
        cause: Some(FactId::from_bytes(&[0x52; 16]).map_err(invalid)?),
        audience: AudienceScope::Shared,
        value: FactValue::ContentEvent {
            definition: content(destination.entry())?,
            subjects: vec![actor()?],
        },
    });
    state.decisions.push(AcceptedDecision {
        operation: command.operation,
        revision: next.revision,
        facts: vec![fact],
        draws: vec![],
        effects: vec![],
        source_policy: label("harbor-authored-scene-choice-1")?,
        semantic_output: Some(destination.entry().to_owned()),
    });
    state.narrative.completed_beats.push(content("harbor")?);
    state.narrative.active_beats = vec![content(destination.entry())?];
    state.narrative.accepted_facts.push(fact);
    checkpoint(next, state)
}

impl SessionEngine<NativeScope<LocalDemoAuthority>> for HarborEngine {
    fn decide(
        &mut self,
        current: &Checkpoint,
        scope: &NativeScope<LocalDemoAuthority>,
        input: &GameInput,
    ) -> Result<Checkpoint, RepositoryError> {
        use df_session::submission::OperationScope;
        scope.validate_input(input)?;
        if let GameInput::Game(command) = input
            && let GameCommand::ProposeAction { action, .. } = &command.command
        {
            if action.entry.as_str() == "join-room" {
                return super::journey::stage_join(current, input);
            }
            if super::journey::CONTENT_ENTRIES.contains(&action.entry.as_str()) {
                return super::journey::stage(current, input);
            }
        }

        if matches!(input, GameInput::Game(CommandInput {
            command: GameCommand::ProposeAction { action, .. }, ..
        }) if *action == content("choose-harbor-scene")?)
        {
            return stage_scene(current, input);
        }
        // Lookup runs in DurableOwner first, so a retry never reaches this draw owner.
        if !current.state().decisions.is_empty() {
            return Err(RepositoryError::InvalidCandidate);
        }
        let GameInput::Game(command) = input else {
            return Err(RepositoryError::InvalidCandidate);
        };
        let source = rule()?;
        let pins = pins()?;
        let selector = label("harbor-normal-ability-check-1")?;
        let handler = HarborHandler { pins: pins.clone() };
        let manifest = source_manifest();
        let entries = [CatalogEntry::new(&source, &manifest)];
        let catalog = CatalogSnapshot::from_published(
            &pins.rules.catalog,
            &pins,
            &manifest,
            &entries,
            CatalogLimits {
                max_complete_bytes: 4096,
                max_entries: 1,
                max_item_bytes: 4096,
                max_total_item_bytes: 4096,
            },
        )
        .map_err(invalid)?;
        let registrations = [HandlerRegistration::new(&selector, &source, &handler)];
        let registry =
            DispatchRegistry::from_catalog(catalog, &registrations, 1).map_err(invalid)?;
        let draw = ActualDraw {
            operation: command.operation,
            ordinal: 0,
            resolution: ResolutionId::from_bytes(&[0x61; 16]).map_err(invalid)?,
            window: WindowId::from_bytes(&[0x62; 16]).map_err(invalid)?,
            sides: 20,
            value: u32::from(draw_d20()?),
            source: source.clone(),
        };
        decide_registered_command(
            RulesCommandInput {
                command: input,
                supplied_draws: &[draw],
            },
            current,
            CommandEntryContext {
                current_basis: current.basis(),
                admitted_pins: &pins,
                inventory: ReferenceInventory {
                    rules: std::slice::from_ref(&source),
                    content: &contents()?,
                    resources: &[],
                    assets: &[],
                },
                limits: CommandEntryLimits {
                    command: CommandLimits {
                        maximum_records: 16,
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
        .map_err(invalid)
    }
    fn validate_recovery(&mut self, current: &Checkpoint) -> Result<(), RepositoryError> {
        current
            .validate_resume(current.basis(), &pins()?)
            .map(|_| ())
            .map_err(invalid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use df_types::OperationId;

    #[test]
    fn every_d20_face_has_twelve_equally_likely_byte_inputs() {
        for face in 1..=20 {
            assert_eq!(
                (0..=u8::MAX)
                    .filter(|byte| d20_face(*byte) == Some(face))
                    .count(),
                12
            );
        }
        assert_eq!(d20_face(0), Some(1));
        assert_eq!(
            (0..=u8::MAX)
                .filter(|byte| d20_face(*byte).is_none())
                .count(),
            16
        );
    }

    fn staged(die: u32, sides: u32) -> Result<Checkpoint, RepositoryError> {
        let current = initial()?;
        let operation = OperationId::from_bytes(&[0x81; 16]).map_err(invalid)?;
        let command = GameInput::Game(CommandInput {
            basis: current.basis(),
            operation,
            member: member()?,
            observed_revision: current.basis().revision,
            command: GameCommand::ProposeAction {
                actor: actor()?,
                action: content("inspect-seal")?,
                targets: vec![],
                choices: vec![],
            },
        });
        let draw = ActualDraw {
            operation,
            ordinal: 0,
            resolution: ResolutionId::from_bytes(&[0x61; 16]).map_err(invalid)?,
            window: WindowId::from_bytes(&[0x62; 16]).map_err(invalid)?,
            sides,
            value: die,
            source: rule()?,
        };
        df_rules::stage_handler(
            &HarborHandler { pins: pins()? },
            &pins()?,
            RulesCommandInput {
                command: &command,
                supplied_draws: &[draw],
            },
            &current,
            1024 * 1024,
        )
        .map_err(invalid)
    }

    #[test]
    fn success_and_failure_commit_distinct_information_facts_with_one_actual_draw() {
        let success = staged(11, 20).unwrap();
        let failure = staged(10, 20).unwrap();
        for checkpoint in [&success, &failure] {
            assert_eq!(checkpoint.basis().revision.sequence(), 1);
            assert_eq!(checkpoint.state().draws.len(), 1);
            assert_eq!(checkpoint.state().decisions.len(), 1);
            assert_eq!(checkpoint.state().facts.len(), 2);
        }
        assert!(
            matches!(&success.state().facts[1].value,FactValue::ContentEvent { definition,.. } if *definition==content("clue-loading-pier").unwrap())
        );
        assert!(
            matches!(&failure.state().facts[1].value,FactValue::ContentEvent { definition,.. } if *definition==content("seal-obscured").unwrap())
        );
        assert!(staged(11, 12).is_err());
    }

    fn choose(
        current: &Checkpoint,
        destination: HarborScene,
    ) -> Result<Checkpoint, RepositoryError> {
        stage_scene(
            current,
            &GameInput::Game(CommandInput {
                basis: current.basis(),
                operation: OperationId::from_bytes(&[0x82; 16]).map_err(invalid)?,
                member: member()?,
                observed_revision: current.basis().revision,
                command: GameCommand::ProposeAction {
                    actor: actor()?,
                    action: content("choose-harbor-scene")?,
                    targets: vec![],
                    choices: vec![(label("harbor-destination")?, label(destination.entry())?)],
                },
            }),
        )
    }

    #[test]
    fn each_outcome_has_two_real_scene_choices_and_preserves_original_roll() {
        for (die, allowed) in [
            (
                11,
                vec![HarborScene::LoadingPier, HarborScene::LanternWharf],
            ),
            (10, vec![HarborScene::LanternWharf, HarborScene::HarborInn]),
        ] {
            let checked = staged(die, 20).unwrap();
            assert_eq!(destinations(&checked).unwrap(), allowed);
            for destination in allowed {
                let next = choose(&checked, destination).unwrap();
                assert_eq!(scene(&next).unwrap(), destination);
                assert_eq!(next.basis().revision.sequence(), 2);
                assert_eq!(next.state().draws, checked.state().draws);
                assert_eq!(next.state().logical_time, checked.state().logical_time);
                assert_eq!(next.state().resources, checked.state().resources);
                assert_eq!(next.state().facts.len(), 3);
                assert_eq!(next.state().decisions.len(), 2);
                assert_eq!(next.state().decisions[0], checked.state().decisions[0]);
                assert!(next.state().decisions[1].draws.is_empty());
                assert_eq!(
                    next.state().decisions[1].semantic_output.as_deref(),
                    Some(destination.entry())
                );
                assert_eq!(next.state().facts[2].audience, AudienceScope::Shared);
                assert_eq!(
                    next.state().narrative.completed_beats,
                    vec![content("harbor").unwrap()]
                );
                assert_eq!(
                    next.state().narrative.accepted_facts,
                    vec![next.state().facts[2].id]
                );
                assert!(destinations(&next).unwrap().is_empty());
            }
        }
    }

    #[test]
    fn failed_seal_does_not_unlock_private_pier_lead() {
        let checked = staged(10, 20).unwrap();
        let before = checked.clone();
        assert!(choose(&checked, HarborScene::LoadingPier).is_err());
        assert_eq!(checked, before);
        assert_eq!(investigation_succeeded(&checked).unwrap(), Some(false));
    }

    #[test]
    fn next_scene_requires_completed_check_and_cannot_be_selected_twice() {
        let initial = initial().unwrap();
        assert!(choose(&initial, HarborScene::LanternWharf).is_err());
        let checked = staged(11, 20).unwrap();
        let selected = choose(&checked, HarborScene::LoadingPier).unwrap();
        assert!(choose(&selected, HarborScene::LanternWharf).is_err());
        assert_eq!(selected.state().draws, checked.state().draws);
    }
}
