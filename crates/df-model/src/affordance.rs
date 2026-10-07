//! Ephemeral, source-admitted entity affordances shared by World and Intent.
//! These records grant no actor permission, source rights or mechanical authority.

use crate::checkpoint::{
    Basis, CheckpointPins, ContentReference, EntityId, RuleReference, WorldEntity,
};
use std::mem::size_of;

/// One action binding admitted by the native content/rules owner against current state.
/// An absent target denotes an authored action without a selected entity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Affordance {
    pub actor: EntityId,
    pub action: ContentReference,
    pub source: RuleReference,
    pub target: Option<WorldEntity>,
}

impl Affordance {
    /// Owned allocation accounting, including every retained source and target label.
    pub fn retained_bytes(&self) -> Option<usize> {
        let mut bytes = size_of::<Self>();
        for label in [
            &self.action.package,
            &self.action.entry,
            &self.source.catalog,
            &self.source.source,
            &self.source.entry,
            &self.source.clause,
        ] {
            bytes = bytes.checked_add(label.as_str().len())?;
        }
        if let Some(target) = &self.target {
            for label in [
                &target.definition.package,
                &target.definition.entry,
                &target.identity_revision,
            ] {
                bytes = bytes.checked_add(label.as_str().len())?;
            }
        }
        Some(bytes)
    }
}

/// A bounded lookup result bound to the immutable current checkpoint's complete pins.
/// Pins are borrowed rather than copied; the set cannot extend its source snapshot's lifetime.
#[derive(Debug, Eq, PartialEq)]
pub struct AffordanceSet<'a> {
    pub basis: Basis,
    pub pins: &'a CheckpointPins,
    pub actor: EntityId,
    pub action: ContentReference,
    pub matches: Vec<Affordance>,
}

impl AffordanceSet<'_> {
    pub fn retained_bytes(&self) -> Option<usize> {
        self.matches.iter().try_fold(
            size_of::<Self>()
                .checked_add(self.action.package.as_str().len())?
                .checked_add(self.action.entry.as_str().len())?,
            |bytes, entry| bytes.checked_add(entry.retained_bytes()?),
        )
    }
}

/// Only an exact typed identity confirms a target. No display name or confidence selects one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetSelection {
    Unspecified,
    Explicit(EntityId),
}

/// Lookup and interpretation do not execute or commit an action.
#[derive(Debug, Eq, PartialEq)]
pub enum TargetResolution<'a> {
    Selected(&'a Affordance),
    NeedsClarification(Vec<EntityId>),
    UnsupportedTarget,
    NeedsRuling,
}
