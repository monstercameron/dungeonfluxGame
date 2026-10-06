use df_model::checkpoint::*;
use df_tempo::elapsed::*;
use df_types::{
    BuildIdentity, MemberId, OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId,
    SessionRevision,
};
use std::time::Duration;

pub(crate) fn label(value: &str) -> RevisionLabel {
    RevisionLabel::new(Some(value)).unwrap()
}
pub(crate) fn content() -> ContentReference {
    ContentReference {
        package: label("fixture-package-1"),
        entry: label("fixture-entry-1"),
    }
}
pub(crate) fn revision(sequence: u64) -> SessionRevision {
    SessionRevision::new(RecoveryEpoch::new(2).unwrap(), sequence)
}
pub(crate) fn basis() -> Basis {
    Basis {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        revision: revision(8),
    }
}
pub(crate) fn fact_id() -> FactId {
    FactId::from_bytes(&[5; 16]).unwrap()
}
pub(crate) fn pins() -> CheckpointPins {
    CheckpointPins {
        rules: RulesPins {
            mode: RulesMode::Standard2024,
            ruleset: label("fixture-rules-1"),
            catalog: label("fixture-catalog-1"),
            catalog_digest: ContentDigest([1; 32]),
            source_manifest: label("fixture-sources-1"),
            source_manifest_digest: ContentDigest([2; 32]),
            handler: label("fixture-handler-1"),
            handler_digest: ContentDigest([3; 32]),
        },
        content: ContentPins {
            content: label("fixture-content-1"),
            content_digest: ContentDigest([4; 32]),
            package: label("fixture-package-1"),
            package_digest: ContentDigest([5; 32]),
        },
        build: BuildIdentity::new(
            Some("fixture-source-1"),
            Some("fixture-native-1"),
            Some("fixture-wasm-1"),
            Some("fixture-config-1"),
            Some("fixture-content-1"),
        )
        .unwrap(),
    }
}
pub(crate) fn continuity() -> ContinuityState {
    ContinuityState {
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
    }
}

pub(crate) fn checkpoint_limits() -> CheckpointLimits {
    CheckpointLimits {
        maximum_records: 100,
        maximum_text_bytes: 256,
        maximum_total_text_bytes: 1024,
        maximum_retained_bytes: 1024 * 1024,
    }
}
pub(crate) fn checkpoint_for(
    mode: ExecutionMode,
    fatigue: Vec<(ContentReference, u64)>,
    presentation_ticks: u64,
) -> Checkpoint {
    let member = MemberId::from_bytes(&[3; 16]).unwrap();
    let operation = OperationId::from_bytes(&[4; 16]).unwrap();
    let state = GameState {
        mode,
        logical_time: LogicalTime {
            ticks: 9,
            ticks_per_second: 10,
        },
        members: vec![MembershipLink {
            member,
            character: None,
        }],
        entities: vec![],
        characters: vec![],
        resources: vec![],
        inventory: vec![],
        facts: vec![GameFact {
            id: fact_id(),
            revision: revision(8),
            operation,
            ordinal: 0,
            cause: None,
            audience: AudienceScope::Shared,
            value: FactValue::ContentEvent {
                definition: content(),
                subjects: vec![],
            },
        }],
        draws: vec![],
        decisions: vec![AcceptedDecision {
            operation,
            revision: revision(8),
            facts: vec![fact_id()],
            draws: vec![],
            effects: vec![],
            source_policy: label("fixture-policy-1"),
            semantic_output: None,
        }],
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
            definition: content(),
            active_beats: vec![],
            completed_beats: vec![],
            open_threads: vec![],
            accepted_facts: vec![],
            remaining_budget: 0,
        },
        encounters: vec![],
        activity: vec![ActivityWindow {
            member,
            started: LogicalTime {
                ticks: 1,
                ticks_per_second: 10,
            },
            ends: LogicalTime {
                ticks: 9,
                ticks_per_second: 10,
            },
            observed_facts: vec![fact_id()],
            spotlight_opt_in: false,
        }],
        tempo: TempoState {
            policy: content(),
            presentation_ticks,
            intensity: 31,
            inertia: -2,
            fatigue,
        },
        presentation: vec![],
        continuity: continuity(),
    };
    checkpoint_from(state)
}
pub(crate) fn checkpoint_from(state: GameState) -> Checkpoint {
    let mut contents = vec![content()];
    for (definition, _) in &state.tempo.fatigue {
        if !contents.contains(definition) {
            contents.push(definition.clone());
        }
    }
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        basis(),
        pins(),
        state,
        ReferenceInventory {
            rules: &[],
            content: &contents,
            resources: &[],
            assets: &[],
        },
        checkpoint_limits(),
    )
    .unwrap()
}
pub(crate) fn request(
    checkpoint: &Checkpoint,
    elapsed: Duration,
    paused: bool,
) -> TempoAdvanceRequest {
    TempoAdvanceRequest {
        expected_basis: checkpoint.basis(),
        observed_logical_time: checkpoint.state().logical_time,
        from_presentation_ticks: checkpoint.state().tempo.presentation_ticks,
        elapsed,
        paused,
    }
}
pub(crate) fn policy(checkpoint: &Checkpoint) -> TempoElapsedPolicy {
    TempoElapsedPolicy {
        definition: checkpoint.state().tempo.policy.clone(),
        ticks_per_second: 10,
        maximum_elapsed_ticks: 100,
    }
}
pub(crate) fn limits() -> TempoElapsedLimits {
    TempoElapsedLimits {
        maximum_checkpoint_bytes: 1024 * 1024,
        maximum_fatigue_entries: 10,
    }
}
