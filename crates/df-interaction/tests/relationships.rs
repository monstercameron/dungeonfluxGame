use std::cell::Cell;

use df_interaction::relationships::{
    RelationshipAxes, RelationshipAxis, RelationshipAxisValue, RelationshipChange,
    RelationshipOwner, RelationshipRefusal, RelationshipView, propose_relationship_change,
};

const AXES: [RelationshipAxis; 7] = [
    RelationshipAxis::Trust,
    RelationshipAxis::Affection,
    RelationshipAxis::Respect,
    RelationshipAxis::Fear,
    RelationshipAxis::Suspicion,
    RelationshipAxis::Debt,
    RelationshipAxis::Familiarity,
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct Subject(&'static str);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Basis {
    state_revision: u64,
    policy_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Provenance {
    event: &'static str,
    witness: Subject,
    policy_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PolicyRefusal {
    UnsupportedValue,
    MissingWitness,
    StalePolicy,
}

struct Policy {
    calls: Cell<usize>,
    revision: u64,
}

impl RelationshipOwner for Policy {
    type Subject = Subject;
    type Basis = Basis;
    type Value = String;
    type Provenance = Provenance;
    type Refusal = PolicyRefusal;

    fn validate_axis_change(
        &self,
        _: RelationshipAxis,
        _: &RelationshipAxisValue<String, Provenance>,
        proposed: &RelationshipAxisValue<String, Provenance>,
    ) -> Result<(), PolicyRefusal> {
        self.calls.set(self.calls.get() + 1);
        if proposed.value().is_empty() || proposed.value().len() > 64 {
            return Err(PolicyRefusal::UnsupportedValue);
        }
        if proposed.provenance().witness != Subject("witness") {
            return Err(PolicyRefusal::MissingWitness);
        }
        if proposed.provenance().policy_revision != self.revision {
            return Err(PolicyRefusal::StalePolicy);
        }
        Ok(())
    }
}

fn policy() -> Policy {
    Policy {
        calls: Cell::new(0),
        revision: 3,
    }
}

fn basis() -> Basis {
    Basis {
        state_revision: 21,
        policy_revision: 3,
    }
}

fn provenance(event: &'static str) -> Provenance {
    Provenance {
        event,
        witness: Subject("witness"),
        policy_revision: 3,
    }
}

fn initial_axes() -> RelationshipAxes<String, Provenance> {
    RelationshipAxes::new([
        RelationshipAxisValue::new("trust-existing".into(), provenance("trust-event")),
        RelationshipAxisValue::new("affection-existing".into(), provenance("affection-event")),
        RelationshipAxisValue::new("respect-existing".into(), provenance("respect-event")),
        RelationshipAxisValue::new("fear-existing".into(), provenance("fear-event")),
        RelationshipAxisValue::new("suspicion-existing".into(), provenance("suspicion-event")),
        RelationshipAxisValue::new("debt-existing".into(), provenance("debt-event")),
        RelationshipAxisValue::new(
            "familiarity-existing".into(),
            provenance("familiarity-event"),
        ),
    ])
}

#[test]
fn each_axis_update_preserves_the_other_six_values_and_provenance() {
    let policy = policy();
    let subject = Subject("npc");
    let target = Subject("player");
    let basis = basis();
    let original = initial_axes();
    for changed_axis in AXES {
        let replacement =
            RelationshipAxisValue::new("new-approved-value".into(), provenance("new-event"));
        let proposal = propose_relationship_change(
            &policy,
            RelationshipView {
                subject: &subject,
                target: &target,
                basis: &basis,
                axes: &original,
            },
            RelationshipChange {
                subject: &subject,
                target: &target,
                expected_basis: &basis,
                axis: changed_axis,
                replacement: replacement.clone(),
            },
        )
        .unwrap();
        assert_eq!(proposal.subject(), &subject);
        assert_eq!(proposal.target(), &target);
        assert_eq!(proposal.expected_basis(), &basis);
        assert_eq!(proposal.changed_axis(), changed_axis);
        for inspected_axis in AXES {
            let expected = if inspected_axis == changed_axis {
                &replacement
            } else {
                original.axis(inspected_axis)
            };
            assert_eq!(proposal.axes().axis(inspected_axis), expected);
        }
        assert_eq!(original, initial_axes());
    }
    assert_eq!(policy.calls.get(), 7);
}

#[test]
fn changing_debt_does_not_change_trust_or_the_reverse_direction() {
    let policy = policy();
    let npc = Subject("npc");
    let player = Subject("player");
    let basis = basis();
    let forward = initial_axes();
    let reverse = initial_axes();
    let reverse_before = reverse.clone();
    let proposal = propose_relationship_change(
        &policy,
        RelationshipView {
            subject: &npc,
            target: &player,
            basis: &basis,
            axes: &forward,
        },
        RelationshipChange {
            subject: &npc,
            target: &player,
            expected_basis: &basis,
            axis: RelationshipAxis::Debt,
            replacement: RelationshipAxisValue::new(
                "debt-settled".into(),
                provenance("settlement"),
            ),
        },
    )
    .unwrap();
    let staged_forward = proposal.into_axes();
    assert_eq!(
        staged_forward.axis(RelationshipAxis::Debt).value(),
        "debt-settled"
    );
    assert_eq!(
        staged_forward.axis(RelationshipAxis::Trust),
        forward.axis(RelationshipAxis::Trust)
    );
    assert_eq!(reverse, reverse_before);
    assert_eq!(forward, initial_axes());
}

#[test]
fn wrong_subject_target_or_basis_refuses_before_policy_validation() {
    let policy = policy();
    let npc = Subject("npc");
    let player = Subject("player");
    let other = Subject("unrelated");
    let basis = basis();
    let stale_state = Basis {
        state_revision: 20,
        policy_revision: 3,
    };
    let stale_policy = Basis {
        state_revision: 21,
        policy_revision: 2,
    };
    let axes = initial_axes();
    let cases = [
        (&player, &npc, &basis, RelationshipRefusal::WrongDirection),
        (&other, &player, &basis, RelationshipRefusal::WrongDirection),
        (&npc, &other, &basis, RelationshipRefusal::WrongDirection),
        (&npc, &player, &stale_state, RelationshipRefusal::StaleBasis),
        (
            &npc,
            &player,
            &stale_policy,
            RelationshipRefusal::StaleBasis,
        ),
    ];
    for (subject, target, expected_basis, expected) in cases {
        let result = propose_relationship_change(
            &policy,
            RelationshipView {
                subject: &npc,
                target: &player,
                basis: &basis,
                axes: &axes,
            },
            RelationshipChange {
                subject,
                target,
                expected_basis,
                axis: RelationshipAxis::Trust,
                replacement: RelationshipAxisValue::new("new".into(), provenance("event")),
            },
        );
        assert!(matches!(result, Err(ref error) if error == &expected));
        assert_eq!(axes, initial_axes());
    }
    assert_eq!(policy.calls.get(), 0);
}

#[test]
fn unsupported_or_oversized_values_and_unwitnessed_or_stale_provenance_refuse_without_mutation() {
    let policy = policy();
    let npc = Subject("npc");
    let player = Subject("player");
    let basis = basis();
    let axes = initial_axes();
    let cases = [
        (
            String::new(),
            provenance("empty"),
            PolicyRefusal::UnsupportedValue,
        ),
        (
            "x".repeat(65),
            provenance("oversized"),
            PolicyRefusal::UnsupportedValue,
        ),
        (
            "new".into(),
            Provenance {
                witness: Subject("unknown"),
                ..provenance("unwitnessed")
            },
            PolicyRefusal::MissingWitness,
        ),
        (
            "new".into(),
            Provenance {
                policy_revision: 2,
                ..provenance("stale")
            },
            PolicyRefusal::StalePolicy,
        ),
    ];
    for (value, provenance, expected) in cases {
        let result = propose_relationship_change(
            &policy,
            RelationshipView {
                subject: &npc,
                target: &player,
                basis: &basis,
                axes: &axes,
            },
            RelationshipChange {
                subject: &npc,
                target: &player,
                expected_basis: &basis,
                axis: RelationshipAxis::Affection,
                replacement: RelationshipAxisValue::new(value, provenance),
            },
        );
        assert!(
            matches!(result, Err(RelationshipRefusal::PolicyRefusal(error)) if error == expected)
        );
        assert_eq!(axes, initial_axes());
    }
    assert_eq!(policy.calls.get(), 4);
}

#[test]
fn an_explicit_same_value_update_retains_the_new_causal_provenance() {
    let policy = policy();
    let npc = Subject("npc");
    let player = Subject("player");
    let basis = basis();
    let axes = initial_axes();
    let proposal = propose_relationship_change(
        &policy,
        RelationshipView {
            subject: &npc,
            target: &player,
            basis: &basis,
            axes: &axes,
        },
        RelationshipChange {
            subject: &npc,
            target: &player,
            expected_basis: &basis,
            axis: RelationshipAxis::Respect,
            replacement: RelationshipAxisValue::new(
                axes.axis(RelationshipAxis::Respect).value().clone(),
                provenance("new-respect-event"),
            ),
        },
    )
    .unwrap();
    assert_eq!(
        proposal.axes().axis(RelationshipAxis::Respect).value(),
        axes.axis(RelationshipAxis::Respect).value()
    );
    assert_eq!(
        proposal.axes().axis(RelationshipAxis::Respect).provenance(),
        &provenance("new-respect-event")
    );
    for axis in AXES {
        if axis != RelationshipAxis::Respect {
            assert_eq!(proposal.axes().axis(axis), axes.axis(axis));
        }
    }
}
