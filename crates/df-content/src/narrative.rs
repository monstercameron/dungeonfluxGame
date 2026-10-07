//! Immutable owner-published narrative budget policy; values are not calibrated by this crate.

/// An authored event's intervention-unit cost or approved free-play credit.
/// Units are narrative policy units, never money, work capacity, game time or D&D resources.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NarrativeBudgetRule<Reference> {
    pub event: Reference,
    pub units: u64,
}

/// The source owner pins and admits this whole value with the content publication.
/// No implicit rate, wall-clock accrual or default free-play trigger exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NarrativeBudgetPolicy<Reference> {
    pub definition: Reference,
    pub maximum: u64,
    pub strong_events: Vec<NarrativeBudgetRule<Reference>>,
    pub free_play_events: Vec<NarrativeBudgetRule<Reference>>,
}
