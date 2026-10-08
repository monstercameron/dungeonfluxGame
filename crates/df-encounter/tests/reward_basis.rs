//! Qualification of source-addressed consequences, not execution of rewards.
use df_encounter::challenge::*;
use df_encounter::participants::{ParticipantOwner, ParticipantStatus};
use std::cell::Cell;
include!("objectives/fixture.rs");

fn definitions() -> Vec<ContentReference> {
    vec![content()]
}

fn current() -> Checkpoint {
    let mut initial = state();
    initial.facts.push(fact(8, 0));
    checkpoint(initial).unwrap()
}

fn authored(entry: &str) -> AuthoredChallengeReference {
    let mut definition = content();
    let mut source = rule();
    definition.entry = label(entry);
    source.entry = label(entry);
    AuthoredChallengeReference { definition, source }
}

struct Owner {
    consequence: SourceAdmission,
    calls: Cell<usize>,
}

impl Owner {
    fn admitted() -> Self {
        Self {
            consequence: SourceAdmission::Admitted,
            calls: Cell::new(0),
        }
    }
}

impl ParticipantOwner for Owner {
    type Actor = EntityId;
    type Basis = Basis;
    type Error = ();

    fn validate_basis(&self, proposal: &Basis, current: &Basis) -> Result<(), ()> {
        if proposal == current { Ok(()) } else { Err(()) }
    }

    fn participant_status(
        &self,
        _current: &Basis,
        actor: &EntityId,
    ) -> Result<ParticipantStatus, ()> {
        Ok(if *actor == entity(4) {
            ParticipantStatus::Available
        } else {
            ParticipantStatus::Unknown
        })
    }
}

impl EncounterSourceOwner for Owner {
    fn admit_reference(
        &self,
        current: &Checkpoint,
        _mode: ChallengeMode,
        purpose: ChallengeReferenceKind,
        _reference: &AuthoredChallengeReference,
    ) -> Result<SourceAdmission, ()> {
        assert_eq!(current.basis(), basis());
        assert_eq!(current.pins(), &pins());
        self.calls.set(self.calls.get() + 1);
        Ok(if purpose == ChallengeReferenceKind::Consequence {
            self.consequence
        } else {
            SourceAdmission::Admitted
        })
    }
}

struct Example {
    current: Checkpoint,
    definition: AuthoredChallengeReference,
    consequences: [AuthoredChallengeReference; 2],
    actors: [EntityId; 1],
    rules: Vec<RuleReference>,
    contents: Vec<ContentReference>,
}

impl Example {
    fn new() -> Self {
        let definition = authored("encounter");
        let consequences = [
            authored("required-consequence-one"),
            authored("required-consequence-two"),
        ];
        let references = [&definition, &consequences[0], &consequences[1]];
        let rules = references
            .iter()
            .map(|reference| reference.source.clone())
            .collect();
        let contents = references
            .iter()
            .map(|reference| reference.definition.clone())
            .collect();
        Self {
            current: current(),
            definition,
            consequences,
            actors: [entity(4)],
            rules,
            contents,
        }
    }

    fn inventory(&self) -> ReferenceInventory<'_> {
        ReferenceInventory {
            rules: &self.rules,
            content: &self.contents,
            resources: &[],
            assets: &[],
        }
    }

    fn budget(&self) -> ChallengeBudget {
        ChallengeBudget {
            max_participants: 1,
            max_objectives: 0,
            max_escape_references: 0,
            max_consequence_references: 2,
            max_comparisons: 100,
            max_input_bytes: 100_000,
            max_output_bytes: 100_000,
        }
    }

    fn request(&self, mode: ChallengeMode) -> EncounterRequest<'_> {
        EncounterRequest {
            current: &self.current,
            expected_basis: self.current.basis(),
            admitted_pins: self.current.pins(),
            inventory: self.inventory(),
            mode,
            definition: &self.definition,
            participants: &self.actors,
            objectives: &[],
            escape_condition: None,
            consequences: &self.consequences,
            budget: self.budget(),
        }
    }
}

#[test]
fn absent_optional_media_preserves_ordered_admitted_consequences_in_all_modes() {
    let example = Example::new();
    let before = example.current.clone();
    assert!(example.current.state().continuity.asset_jobs.is_empty());
    assert!(
        example
            .current
            .state()
            .continuity
            .asset_dependencies
            .is_empty()
    );
    assert!(example.inventory().assets.is_empty());
    for mode in [
        ChallengeMode::Combat,
        ChallengeMode::Social,
        ChallengeMode::Chase,
        ChallengeMode::Hazard,
        ChallengeMode::Puzzle,
        ChallengeMode::Survival,
    ] {
        let owner = Owner::admitted();
        let plan = EncounterDirector::propose(&owner, example.request(mode)).unwrap();
        assert_eq!(plan.basis, example.current.basis());
        assert_eq!(plan.pins, *example.current.pins());
        assert_eq!(plan.mode, mode);
        assert_eq!(plan.definition, example.definition);
        assert_eq!(plan.participants, example.actors);
        assert_eq!(
            plan.consequences
                .iter()
                .map(|item| &item.authored)
                .collect::<Vec<_>>(),
            example.consequences.iter().collect::<Vec<_>>()
        );
        assert!(plan.source_gaps.is_empty());
        EncounterDirector::validate(
            &owner,
            &example.current,
            &plan,
            example.current.basis(),
            example.current.pins(),
            example.inventory(),
            example.budget(),
        )
        .unwrap();
        assert_eq!(owner.calls.get(), 6);
        // Proposal and validation neither award resources nor commit any checkpoint change.
        assert_eq!(example.current, before);
    }
}

#[test]
fn required_source_refusals_never_return_partial_success() {
    let example = Example::new();
    let before = example.current.clone();
    let plan =
        EncounterDirector::propose(&Owner::admitted(), example.request(ChallengeMode::Combat))
            .unwrap();
    for reason in [
        SourceAdmission::Denied,
        SourceAdmission::Unavailable,
        SourceAdmission::Unsupported,
    ] {
        let owner = Owner {
            consequence: reason,
            calls: Cell::new(0),
        };
        let expected = EncounterError::SourceGap(ChallengeSourceGap {
            purpose: ChallengeReferenceKind::Consequence,
            index: 0,
            reason,
        });
        assert_eq!(
            EncounterDirector::propose(&owner, example.request(ChallengeMode::Combat)),
            Err(expected)
        );
        assert_eq!(owner.calls.get(), 2);
        owner.calls.set(0);
        assert_eq!(
            EncounterDirector::validate(
                &owner,
                &example.current,
                &plan,
                example.current.basis(),
                example.current.pins(),
                example.inventory(),
                example.budget()
            ),
            Err(EncounterError::SourceGap(ChallengeSourceGap {
                purpose: ChallengeReferenceKind::Consequence,
                index: 0,
                reason
            }))
        );
        assert_eq!(owner.calls.get(), 2);
        assert_eq!(example.current, before);
    }
}

#[test]
fn absent_media_does_not_admit_missing_required_content_or_rules() {
    let mut example = Example::new();
    let before = example.current.clone();
    let owner = Owner::admitted();
    example.contents.pop();
    assert_eq!(
        EncounterDirector::propose(&owner, example.request(ChallengeMode::Social)),
        Err(EncounterError::UnadmittedContent)
    );
    assert_eq!(owner.calls.get(), 0);
    example
        .contents
        .push(example.consequences[1].definition.clone());
    example.rules.pop();
    assert_eq!(
        EncounterDirector::propose(&owner, example.request(ChallengeMode::Social)),
        Err(EncounterError::UnadmittedSource)
    );
    assert_eq!(owner.calls.get(), 0);
    assert_eq!(example.current, before);
}

#[test]
fn stale_basis_and_rules_content_build_pins_refuse_before_source_callbacks() {
    let example = Example::new();
    let before = example.current.clone();
    let plan =
        EncounterDirector::propose(&Owner::admitted(), example.request(ChallengeMode::Hazard))
            .unwrap();
    let mut stale = plan.clone();
    stale.basis.revision = revision(2, 9);
    let mut rules = plan.clone();
    rules.pins.rules.handler = label("other-handler");
    let mut content = plan.clone();
    content.pins.content.content = label("other-content");
    let mut build = plan.clone();
    build.pins.build = BuildIdentity::new(
        Some("other-source"),
        Some("fixture-native-1"),
        Some("fixture-wasm-1"),
        Some("fixture-config-1"),
        Some("fixture-content-1"),
    )
    .unwrap();
    for (candidate, expected) in [
        (stale, CheckpointError::StaleBasis),
        (rules, CheckpointError::RulesMismatch),
        (content, CheckpointError::ContentMismatch),
        (build, CheckpointError::BuildMismatch),
    ] {
        let owner = Owner::admitted();
        assert_eq!(
            EncounterDirector::validate(
                &owner,
                &example.current,
                &candidate,
                example.current.basis(),
                example.current.pins(),
                example.inventory(),
                example.budget()
            ),
            Err(EncounterError::Checkpoint(expected))
        );
        assert_eq!(owner.calls.get(), 0);
        assert_eq!(example.current, before);
    }
    let owner = Owner::admitted();
    let mut request = example.request(ChallengeMode::Hazard);
    request.expected_basis.revision = revision(2, 9);
    assert_eq!(
        EncounterDirector::propose(&owner, request),
        Err(EncounterError::Checkpoint(CheckpointError::StaleBasis))
    );
    assert_eq!(owner.calls.get(), 0);
}

#[test]
fn consequence_and_work_capacity_refuse_before_source_callbacks() {
    let example = Example::new();
    let before = example.current.clone();
    for budget in [
        ChallengeBudget {
            max_consequence_references: 1,
            ..example.budget()
        },
        ChallengeBudget {
            max_comparisons: 0,
            ..example.budget()
        },
        ChallengeBudget {
            max_input_bytes: 0,
            ..example.budget()
        },
        ChallengeBudget {
            max_output_bytes: 0,
            ..example.budget()
        },
    ] {
        let owner = Owner::admitted();
        let mut request = example.request(ChallengeMode::Puzzle);
        request.budget = budget;
        assert_eq!(
            EncounterDirector::propose(&owner, request),
            Err(EncounterError::Capacity)
        );
        assert_eq!(owner.calls.get(), 0);
        assert_eq!(example.current, before);
    }
}
