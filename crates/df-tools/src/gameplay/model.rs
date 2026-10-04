use df_content::catalog::{CatalogEntry, CatalogLimits, CatalogSnapshot};
use df_engine::command_entry::{
    CommandEntryContext, CommandEntryLimits, decide_registered_command,
};
use df_model::checkpoint::*;
use df_model::commands::CommandLimits;
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

pub(super) const OFFER: &str = "harbor-seal-investigation-1";
pub(super) const SOURCE: &[u8] = b"SRD5.2.1 page6 D20 Tests Ability Checks; https://media.dndbeyond.com/compendium-images/srd/5.2/SRD_CC_v5.2.1.pdf; Wizards of the Coast LLC; CC BY 4.0; normal single-d20 check only";
const CONTENT: &[u8] = b"DungeonFlux harbor seal authored challenge: Mara Intelligence modifier +2, Investigation proficiency +2; medium DC15; success reveals loading-pier shipment clue; failure yields obscured seal and no clue; one attempt.";

fn invalid<T>(_: T) -> RepositoryError {
    RepositoryError::InvalidCandidate
}
pub(super) fn label(value: &str) -> Result<RevisionLabel, RepositoryError> {
    RevisionLabel::new(Some(value)).map_err(invalid)
}
pub(super) fn rule() -> Result<RuleReference, RepositoryError> {
    Ok(RuleReference {
        catalog: label("srd521-normal-ability-check-1")?,
        source: label("srd521-page6")?,
        entry: label("d20-ability-check")?,
        clause: label("normal-proficient-total-versus-dc")?,
    })
}
pub(super) fn content(entry: &str) -> Result<ContentReference, RepositoryError> {
    Ok(ContentReference {
        package: label("harbor-investigation-demo-1")?,
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
    ]
    .into_iter()
    .map(content)
    .collect()
}
pub(super) fn pins() -> Result<CheckpointPins, RepositoryError> {
    Ok(CheckpointPins {
        rules: RulesPins {
            mode: RulesMode::Standard2024,
            ruleset: label("srd521-normal-ability-check-1")?,
            catalog: label("srd521-normal-ability-check-1")?,
            catalog_digest: ContentDigest(Sha256::digest(SOURCE).into()),
            source_manifest: label("srd521-page6-reviewed-subset")?,
            source_manifest_digest: ContentDigest(Sha256::digest(SOURCE).into()),
            handler: label("normal-ability-check-compiled-1")?,
            handler_digest: ContentDigest(
                Sha256::digest(include_bytes!("../../../df-rules/src/ability_check.rs")).into(),
            ),
        },
        content: ContentPins {
            content: label("harbor-investigation-demo-1")?,
            content_digest: ContentDigest(Sha256::digest(CONTENT).into()),
            package: label("harbor-investigation-demo-1")?,
            package_digest: ContentDigest(Sha256::digest(CONTENT).into()),
        },
        build: BuildIdentity::new(
            Some(crate::BUILD_ID),
            Some(crate::BUILD_ID),
            Some(crate::BUILD_ID),
            Some("local-gameplay-demo-config-1"),
            Some("harbor-investigation-demo-1"),
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
        maximum_records: 128,
        maximum_text_bytes: 1024,
        maximum_total_text_bytes: 8192,
        maximum_retained_bytes: 1024 * 1024,
    }
}
pub(super) fn recovery() -> Result<NativeRecoverySource, RepositoryError> {
    Ok(NativeRecoverySource {
        rules: vec![rule()?],
        content: contents()?,
        resources: vec![],
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
            rules: &[rule()?],
            content: &contents()?,
            resources: &[],
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
            source_policy: label("srd521-normal-ability-check-1")?,
            semantic_output: None,
        });
        state.narrative.remaining_budget = 0;
        checkpoint(next, state)
    }
}

pub(super) struct HarborEngine;
impl SessionEngine<NativeScope<LocalDemoAuthority>> for HarborEngine {
    fn decide(
        &mut self,
        current: &Checkpoint,
        scope: &NativeScope<LocalDemoAuthority>,
        input: &GameInput,
    ) -> Result<Checkpoint, RepositoryError> {
        use df_session::submission::OperationScope;
        scope.validate_input(input)?;
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
        let entries = [CatalogEntry::new(&source, SOURCE)];
        let catalog = CatalogSnapshot::from_published(
            &pins.rules.catalog,
            &pins,
            SOURCE,
            &entries,
            CatalogLimits {
                max_complete_bytes: 1024,
                max_entries: 1,
                max_item_bytes: 1024,
                max_total_item_bytes: 1024,
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
}
