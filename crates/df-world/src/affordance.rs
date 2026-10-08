//! Bounded lookup of source-admitted actions over canonical WorldEntity identities.

use df_model::affordance::{Affordance, AffordanceSet};
use df_model::checkpoint::{
    Basis, Checkpoint, CheckpointError, CheckpointPins, ContentReference, EntityId,
    ReferenceInventory,
};

pub struct AffordanceQuery<'a> {
    pub basis: Basis,
    pub pins: &'a CheckpointPins,
    pub actor: EntityId,
    pub action: &'a ContentReference,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AffordanceLookupLimits {
    pub input_records: usize,
    pub candidates: usize,
    pub output_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AffordanceLookupError {
    Snapshot(CheckpointError),
    Capacity,
    AllocationCapacity,
    UnknownActor,
    UnknownAction,
    UnknownSource,
    UnknownTarget,
    StaleTarget,
    AmbiguousSource,
    ConflictingTargetPolicy,
}

/// Looks up immutable bindings supplied by the trusted native content/rules owner.
/// Catalog membership alone never makes an action legal: the caller must supply only bindings
/// whose actual source handler and current conditions have been admitted. Every supplied record
/// is checked before any output is returned. Entity definitions stay opaque; no geometry, entity
/// kind, resource cost, physics or success is inferred. Empty matches require an explicit ruling.
pub fn lookup_affordances<'a>(
    current: &'a Checkpoint,
    query: AffordanceQuery<'_>,
    source_admitted: &[Affordance],
    inventory: ReferenceInventory<'_>,
    limits: AffordanceLookupLimits,
) -> Result<AffordanceSet<'a>, AffordanceLookupError> {
    current
        .validate_resume(query.basis, query.pins)
        .map_err(AffordanceLookupError::Snapshot)?;
    let records = current
        .state()
        .entities
        .len()
        .checked_add(source_admitted.len())
        .and_then(|n| n.checked_add(inventory.content.len()))
        .and_then(|n| n.checked_add(inventory.rules.len()))
        .ok_or(AffordanceLookupError::Capacity)?;
    if limits.input_records == 0
        || limits.candidates == 0
        || limits.output_bytes == 0
        || records > limits.input_records
    {
        return Err(AffordanceLookupError::Capacity);
    }
    if !current
        .state()
        .entities
        .iter()
        .any(|entity| entity.id == query.actor)
    {
        return Err(AffordanceLookupError::UnknownActor);
    }
    if query.action.package != current.pins().content.package
        || !inventory.content.contains(query.action)
    {
        return Err(AffordanceLookupError::UnknownAction);
    }
    let matches = Vec::new();
    let mut result = AffordanceSet {
        basis: current.basis(),
        pins: current.pins(),
        actor: query.actor,
        action: query.action.clone(),
        matches,
    };
    let mut bytes = result
        .retained_bytes()
        .ok_or(AffordanceLookupError::Capacity)?;
    if bytes > limits.output_bytes {
        return Err(AffordanceLookupError::Capacity);
    }
    let mut count = 0usize;
    for (index, record) in source_admitted.iter().enumerate() {
        if !current
            .state()
            .entities
            .iter()
            .any(|entity| entity.id == record.actor)
        {
            return Err(AffordanceLookupError::UnknownActor);
        }
        if record.action.package != current.pins().content.package
            || !inventory.content.contains(&record.action)
        {
            return Err(AffordanceLookupError::UnknownAction);
        }
        if record.source.catalog != current.pins().rules.catalog
            || !inventory.rules.contains(&record.source)
        {
            return Err(AffordanceLookupError::UnknownSource);
        }
        if let Some(target) = &record.target {
            let actual = current
                .state()
                .entities
                .iter()
                .find(|entity| entity.id == target.id)
                .ok_or(AffordanceLookupError::UnknownTarget)?;
            if actual != target {
                return Err(AffordanceLookupError::StaleTarget);
            }
            if !inventory.content.contains(&target.definition) {
                return Err(AffordanceLookupError::UnknownTarget);
            }
        }
        if record.actor != query.actor || record.action != *query.action {
            continue;
        }
        if let Some(previous) = source_admitted.iter().take(index).find(|previous| {
            previous.actor == query.actor
                && previous.action == *query.action
                && previous.target.as_ref().map(|target| target.id)
                    == record.target.as_ref().map(|target| target.id)
        }) {
            if previous.source != record.source {
                return Err(AffordanceLookupError::AmbiguousSource);
            }
            continue;
        }
        if source_admitted.iter().take(index).any(|previous| {
            previous.actor == query.actor
                && previous.action == *query.action
                && previous.target.is_some() != record.target.is_some()
        }) {
            return Err(AffordanceLookupError::ConflictingTargetPolicy);
        }
        bytes = bytes
            .checked_add(
                record
                    .retained_bytes()
                    .ok_or(AffordanceLookupError::Capacity)?,
            )
            .ok_or(AffordanceLookupError::Capacity)?;
        if bytes > limits.output_bytes || count >= limits.candidates {
            return Err(AffordanceLookupError::Capacity);
        }
        count += 1;
    }
    result
        .matches
        .try_reserve_exact(count)
        .map_err(|_| AffordanceLookupError::AllocationCapacity)?;
    for (index, record) in source_admitted.iter().enumerate() {
        if record.actor == query.actor
            && record.action == *query.action
            && !source_admitted.iter().take(index).any(|previous| {
                previous.actor == query.actor
                    && previous.action == *query.action
                    && previous.target == record.target
            })
        {
            result.matches.push(record.clone());
        }
    }
    result
        .matches
        .sort_unstable_by_key(|entry| entry.target.as_ref().map(|target| *target.id.as_bytes()));
    Ok(result)
}
