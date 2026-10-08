use df_model::checkpoint::{
    AssetDemand, AssetKind, AssetReference, AssetRequestKey, AudienceScope, Basis, CheckpointPins,
    ContentDigest, ContentPins, ContentReference, DemandPriority, EntityId, ExecutionMode, FactId,
    LogicalTime, NarrativeMoment, PerformanceHint, PresentationDemand, RecordId, RulesMode,
    RulesPins, ShotPlan,
};
use df_presentation::moment_selection::{
    MomentAlternative, MomentContext, MomentLimits, PermittedMoment,
};
use df_types::{BuildIdentity, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision};

pub fn label(value: &str) -> RevisionLabel {
    RevisionLabel::new(Some(value)).unwrap()
}

pub fn record(value: u8) -> RecordId {
    RecordId::from_bytes(&[value; 16]).unwrap()
}

pub fn fact(value: u8) -> FactId {
    FactId::from_bytes(&[value; 16]).unwrap()
}

pub fn entity(value: u8) -> EntityId {
    EntityId::from_bytes(&[value; 16]).unwrap()
}

pub fn basis() -> Basis {
    Basis {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        revision: SessionRevision::new(RecoveryEpoch::new(1).unwrap(), 4),
    }
}

pub fn now() -> LogicalTime {
    LogicalTime {
        ticks: 9,
        ticks_per_second: 1,
    }
}

pub fn limits() -> MomentLimits {
    MomentLimits {
        maximum_alternatives: 8,
        maximum_items: 1024,
        maximum_shots: 8,
        maximum_demands: 8,
        maximum_duration_ticks: 100,
        maximum_reference_bytes: 1024,
        maximum_demand_bytes: 1024,
    }
}

pub struct Fixture {
    pub basis: Basis,
    pub mode: ExecutionMode,
    pub pins: CheckpointPins,
    pub moment: NarrativeMoment,
    pub presentation: PresentationDemand,
    pub shots: Vec<ShotPlan>,
    pub demands: Vec<AssetDemand>,
    pub facts: Vec<FactId>,
    pub claims: Vec<RecordId>,
    pub entities: Vec<EntityId>,
    pub content: Vec<ContentReference>,
    pub assets: Vec<AssetReference>,
}

impl Fixture {
    pub fn new() -> Self {
        let definition = ContentReference {
            package: label("package-v1"),
            entry: label("policy-v1"),
        };
        let asset = AssetReference {
            key: label("still-v1"),
            digest: ContentDigest([6; 32]),
            byte_length: 4,
            kind: AssetKind::Image,
        };
        let pins = CheckpointPins {
            rules: RulesPins {
                mode: RulesMode::Standard2024,
                ruleset: label("rules-v1"),
                catalog: label("catalog-v1"),
                catalog_digest: ContentDigest([1; 32]),
                source_manifest: label("sources-v1"),
                source_manifest_digest: ContentDigest([2; 32]),
                handler: label("handler-v1"),
                handler_digest: ContentDigest([3; 32]),
            },
            content: ContentPins {
                content: label("content-v1"),
                content_digest: ContentDigest([4; 32]),
                package: label("package-v1"),
                package_digest: ContentDigest([5; 32]),
            },
            build: BuildIdentity::new(
                Some("source-v1"),
                Some("native-v1"),
                Some("wasm-v1"),
                Some("config-v1"),
                Some("content-v1"),
            )
            .unwrap(),
        };
        let moment = NarrativeMoment {
            id: record(7),
            location: entity(8),
            characters: vec![entity(9)],
            facts: vec![fact(10)],
            attributed_claims: vec![record(11)],
            audience: AudienceScope::Shared,
            semantic_focus: definition.clone(),
            identity_revision: label("identity-v1"),
        };
        let presentation = PresentationDemand {
            id: record(12),
            definition: definition.clone(),
            audience: AudienceScope::Shared,
            causal_facts: vec![fact(10)],
            source_revision: basis().revision,
        };
        let shot = ShotPlan {
            id: record(13),
            moment: moment.id,
            subjects: moment.characters.clone(),
            audience: AudienceScope::Shared,
            duration_ticks: 5,
            definition: definition.clone(),
            references: vec![asset.clone()],
            performance: PerformanceHint {
                definition: definition.clone(),
                voice: None,
                emphasis_facts: moment.facts.clone(),
            },
        };
        let demand = AssetDemand {
            id: record(14),
            basis: basis(),
            key: AssetRequestKey {
                schema: df_model::checkpoint::CHECKPOINT_SCHEMA,
                source: pins.content.content_digest,
                moment: moment.id,
                identity: moment.identity_revision.clone(),
                style: label("style-v1"),
                voice: None,
                provider: label("fixture"),
                model: label("fixture-v1"),
                format: label("still-v1"),
                references: vec![asset.clone()],
                audience: AudienceScope::Shared,
                parameters: label("parameters-v1"),
            },
            priority: DemandPriority::InteractionCritical,
            mode: ExecutionMode::PreparedOnly,
            expires: LogicalTime {
                ticks: 10,
                ticks_per_second: 1,
            },
            budget_reservation: label("no-live-spend"),
            maximum_bytes: 64,
            policy: definition.clone(),
        };
        Self {
            basis: basis(),
            mode: ExecutionMode::PreparedOnly,
            pins,
            moment,
            presentation,
            shots: vec![shot],
            demands: vec![demand],
            facts: vec![fact(10)],
            claims: vec![record(11)],
            entities: vec![entity(8), entity(9)],
            content: vec![definition],
            assets: vec![asset],
        }
    }

    pub fn context(&self) -> MomentContext<'_> {
        MomentContext {
            basis: self.basis,
            pins: &self.pins,
            mode: self.mode,
            moment: &self.moment,
            presentation: &self.presentation,
        }
    }

    pub fn permitted(&self) -> PermittedMoment<'_> {
        PermittedMoment {
            context: self.context(),
            facts: &self.facts,
            attributed_claims: &self.claims,
            entities: &self.entities,
            content: &self.content,
            assets: &self.assets,
        }
    }

    pub fn alternative(&self) -> MomentAlternative<'_> {
        MomentAlternative {
            context: self.context(),
            shots: &self.shots,
            demands: &self.demands,
        }
    }
}
