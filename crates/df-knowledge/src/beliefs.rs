/// Access to an existing caller-owned belief record, without defining its persistent shape.
///
/// The basis must bind the relevant world, knowledge, content and policy versions,
/// logical-time basis and causal input. Identity values alone confer no authority.
pub trait BeliefUpdate {
    type Basis: Eq;
    type Actor;
    type Subject;
    type Claim;
    type Evidence;

    fn basis(&self) -> &Self::Basis;
    fn actor(&self) -> &Self::Actor;
    fn subject(&self) -> &Self::Subject;
    fn claim(&self) -> &Self::Claim;
    fn evidence(&self) -> &Self::Evidence;
}

/// Immutable, trusted owner view of the current basis and permitted observation scope.
///
/// Checks must be side-effect-free and use the owner's actual attribution and evidence
/// records. A claim may
/// contradict canonical truth: these checks authorize its attributed provenance,
/// rather than treating the claim as a canonical fact. This port grants no access to
/// mutable world state and does not publish a disclosure or commit a belief.
pub trait BeliefBasis<Update: BeliefUpdate> {
    fn current_basis(&self) -> &Update::Basis;
    fn permits_subject(&self, subject: &Update::Subject) -> bool;
    fn permits_attribution(&self, actor: &Update::Actor, subject: &Update::Subject) -> bool;
    fn permits_evidence(&self, update: &Update) -> bool;
}

/// Safe rejection categories contain no private claims, fact identifiers or evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeliefValidationError {
    StaleBasis,
    UnknownSubject,
    InvalidAttribution,
    UndisclosedEvidence,
}

/// One validated caller-owned belief update; never a canonical-world mutation.
///
/// Only validation can construct this wrapper. The session owner must revalidate
/// its basis before committing the record; this receipt does not authorize a commit.
///
/// ```compile_fail
/// use df_knowledge::beliefs::ValidatedBeliefUpdate;
/// let _ = ValidatedBeliefUpdate { update: () };
/// ```
pub struct ValidatedBeliefUpdate<Update> {
    update: Update,
}

impl<Update> ValidatedBeliefUpdate<Update> {
    pub fn update(&self) -> &Update {
        &self.update
    }

    pub fn into_update(self) -> Update {
        self.update
    }
}

/// Validate one attributed belief against an immutable owner view.
///
/// Work is four owner checks over one supplied record, with no copying, allocation,
/// I/O, clock access or mutation. Owners must bound their record sizes and checks.
/// Canonical truth is neither compared with the claim nor returned to the caller.
pub fn validate_belief_update<Update: BeliefUpdate>(
    basis: &impl BeliefBasis<Update>,
    update: Update,
) -> Result<ValidatedBeliefUpdate<Update>, BeliefValidationError> {
    if update.basis() != basis.current_basis() {
        return Err(BeliefValidationError::StaleBasis);
    }
    if !basis.permits_subject(update.subject()) {
        return Err(BeliefValidationError::UnknownSubject);
    }
    if !basis.permits_attribution(update.actor(), update.subject()) {
        return Err(BeliefValidationError::InvalidAttribution);
    }
    if !basis.permits_evidence(&update) {
        return Err(BeliefValidationError::UndisclosedEvidence);
    }
    Ok(ValidatedBeliefUpdate { update })
}
