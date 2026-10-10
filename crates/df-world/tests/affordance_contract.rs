#[path = "../../df-intent/tests/support/candidate_fixture.rs"]
mod fixture;

use df_model::affordance::{Affordance, AffordanceSet};
use df_model::checkpoint::*;
use df_world::affordance::*;
use fixture::*;

fn bounds() -> AffordanceLookupLimits {
    AffordanceLookupLimits {
        input_records: 256,
        candidates: 16,
        output_bytes: 16384,
    }
}
fn records(current: &Checkpoint) -> Vec<Affordance> {
    current
        .state()
        .entities
        .iter()
        .filter(|target| target.id != entity(4))
        .map(|target| Affordance {
            actor: entity(4),
            action: content(),
            source: rule(),
            target: Some(target.clone()),
        })
        .collect()
}
fn invoke<'a>(
    current: &'a Checkpoint,
    entries: &[Affordance],
    limits: AffordanceLookupLimits,
    rules: &[RuleReference],
) -> Result<AffordanceSet<'a>, AffordanceLookupError> {
    let contents = current
        .state()
        .entities
        .iter()
        .map(|entity| entity.definition.clone())
        .collect::<Vec<_>>();
    let action = content();
    lookup_affordances(
        current,
        AffordanceQuery {
            basis: current.basis(),
            pins: current.pins(),
            actor: entity(4),
            action: &action,
        },
        entries,
        ReferenceInventory {
            rules,
            content: &contents,
            resources: &[],
            assets: &[],
        },
        limits,
    )
}
fn entities() -> Checkpoint {
    let mut supplied = state();
    for (index, kind) in [
        "npc",
        "object",
        "location",
        "creature",
        "faction",
        "environmental-system",
    ]
    .iter()
    .enumerate()
    {
        supplied.entities.push(WorldEntity {
            id: entity(u8::try_from(index + 5).unwrap()),
            definition: ContentReference {
                package: content().package,
                entry: label(kind),
            },
            location: None,
            position: None,
            identity_revision: label("source-entity-1"),
        });
    }
    let contents = supplied
        .entities
        .iter()
        .map(|entity| entity.definition.clone())
        .collect::<Vec<_>>();
    Checkpoint::new(
        CHECKPOINT_SCHEMA,
        basis(),
        pins(),
        supplied,
        ReferenceInventory {
            rules: &[rule()],
            content: &contents,
            resources: &resource_constraints(),
            assets: &[],
        },
        limits(),
    )
    .unwrap()
}

#[test]
fn source_admitted_lookup_handles_all_entity_definitions_without_inventing_mechanics() {
    let current = entities();
    let before = current.clone();
    let mut entries = records(&current);
    entries.reverse();
    let result = invoke(&current, &entries, bounds(), &[rule()]).unwrap();
    assert_eq!(result.matches.len(), 6);
    assert_eq!(
        result
            .matches
            .iter()
            .map(|entry| entry.target.as_ref().unwrap().id)
            .collect::<Vec<_>>(),
        (5..=10).map(entity).collect::<Vec<_>>()
    );
    assert_eq!(result.basis, current.basis());
    assert_eq!(result.pins, current.pins());
    assert_eq!(current, before);
}

#[test]
fn source_absence_and_duplicate_same_source_preserve_explicit_lookup_state() {
    let mut supplied = state();
    supplied.facts.push(fact(7, 0));
    supplied.pending.push(pending());
    let current = checkpoint(supplied).unwrap();
    let before = current.clone();
    assert!(
        invoke(&current, &[], bounds(), &[rule()])
            .unwrap()
            .matches
            .is_empty()
    );
    let record = Affordance {
        actor: entity(4),
        action: content(),
        source: rule(),
        target: Some(current.state().entities[0].clone()),
    };
    let set = invoke(&current, &[record.clone(), record], bounds(), &[rule()]).unwrap();
    assert_eq!(set.matches.len(), 1);
    assert_eq!(current, before);
    assert_eq!(current.state().pending.len(), 1);
    assert_eq!(current.state().facts.len(), 1);
}

#[test]
fn conflicting_sources_never_select_the_first_record() {
    let current = entities();
    let first = records(&current).remove(0);
    let mut other = first.clone();
    other.source.clause = label("different-reviewed-clause");
    let source = other.source.clone();
    for entries in [vec![first.clone(), other.clone()], vec![other, first]] {
        assert_eq!(
            invoke(&current, &entries, bounds(), &[rule(), source.clone()]),
            Err(AffordanceLookupError::AmbiguousSource)
        );
    }
}

#[test]
fn unknown_actor_source_action_and_target_are_typed_refusals() {
    let current = entities();
    let valid = records(&current).remove(0);
    for mutation in 0..4 {
        let mut entry = valid.clone();
        let expected = match mutation {
            0 => {
                entry.actor = entity(99);
                AffordanceLookupError::UnknownActor
            }
            1 => {
                entry.source.clause = label("unregistered-clause");
                AffordanceLookupError::UnknownSource
            }
            2 => {
                entry.action.entry = label("unsupported-physics");
                AffordanceLookupError::UnknownAction
            }
            _ => {
                entry.target.as_mut().unwrap().id = entity(99);
                AffordanceLookupError::UnknownTarget
            }
        };
        assert_eq!(
            invoke(&current, &[entry], bounds(), &[rule()]),
            Err(expected)
        );
    }
}

#[test]
fn changed_target_identity_definition_or_position_never_reuses_old_admission() {
    let current = entities();
    for mutation in 0..3 {
        let mut entry = records(&current).remove(0);
        let target = entry.target.as_mut().unwrap();
        match mutation {
            0 => target.identity_revision = label("source-entity-2"),
            1 => target.definition.entry = label("other-object"),
            _ => target.position = Some(Position { x: 1, y: 0, z: 0 }),
        }
        assert_eq!(
            invoke(&current, &[entry], bounds(), &[rule()]),
            Err(AffordanceLookupError::StaleTarget)
        );
    }
}

#[test]
fn stale_complete_basis_or_pins_refuse_before_lookup() {
    let current = entities();
    let action = content();
    let contents = current
        .state()
        .entities
        .iter()
        .map(|entity| entity.definition.clone())
        .collect::<Vec<_>>();
    let mut changed_basis = current.basis();
    changed_basis.revision = changed_basis.revision.next_sequence().unwrap();
    let mut changed_pins = current.pins().clone();
    changed_pins.content.content_digest.0[0] ^= 1;
    for (basis, pins) in [
        (changed_basis, current.pins()),
        (current.basis(), &changed_pins),
    ] {
        let result = lookup_affordances(
            &current,
            AffordanceQuery {
                basis,
                pins,
                actor: entity(4),
                action: &action,
            },
            &records(&current),
            ReferenceInventory {
                rules: &[rule()],
                content: &contents,
                resources: &[],
                assets: &[],
            },
            bounds(),
        );
        assert!(matches!(result, Err(AffordanceLookupError::Snapshot(_))));
    }
}

#[test]
fn exact_output_and_candidate_bounds_refuse_without_partial_result() {
    let current = entities();
    let entries = records(&current);
    let bytes = invoke(&current, &entries, bounds(), &[rule()])
        .unwrap()
        .retained_bytes()
        .unwrap();
    let mut exact = bounds();
    exact.output_bytes = bytes;
    assert!(invoke(&current, &entries, exact, &[rule()]).is_ok());
    exact.output_bytes -= 1;
    assert_eq!(
        invoke(&current, &entries, exact, &[rule()]),
        Err(AffordanceLookupError::Capacity)
    );
    for bounded in [
        AffordanceLookupLimits {
            input_records: 1,
            ..bounds()
        },
        AffordanceLookupLimits {
            candidates: 5,
            ..bounds()
        },
        AffordanceLookupLimits {
            candidates: 0,
            ..bounds()
        },
    ] {
        assert_eq!(
            invoke(&current, &entries, bounded, &[rule()]),
            Err(AffordanceLookupError::Capacity)
        );
    }
}

#[test]
fn targeted_and_untargeted_policies_cannot_be_silently_combined() {
    let current = entities();
    let targeted = records(&current).remove(0);
    let mut untargeted = targeted.clone();
    untargeted.target = None;
    assert_eq!(
        invoke(&current, &[targeted, untargeted], bounds(), &[rule()]),
        Err(AffordanceLookupError::ConflictingTargetPolicy)
    );
}

#[test]
fn invalid_nonmatching_source_refuses_the_whole_lookup_without_partial_output() {
    let current = entities();
    let before = current.clone();
    let mut entries = records(&current);
    let mut unrelated = entries[0].clone();
    unrelated.actor = entity(5);
    unrelated.source.clause = label("unadmitted-physical-heuristic");
    entries.push(unrelated);
    let saved = entries.clone();
    assert_eq!(
        invoke(&current, &entries, bounds(), &[rule()]),
        Err(AffordanceLookupError::UnknownSource)
    );
    assert_eq!(current, before);
    assert_eq!(entries, saved);
}
