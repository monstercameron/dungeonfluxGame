/// The independent directional axes of a relationship, not mechanical bonuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationshipAxis {
    Trust,
    Affection,
    Respect,
    Fear,
    Suspicion,
    Debt,
    Familiarity,
}

/// One caller-owned policy value together with its causal provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipAxisValue<Value, Provenance> {
    value: Value,
    provenance: Provenance,
}

impl<Value, Provenance> RelationshipAxisValue<Value, Provenance> {
    pub fn new(value: Value, provenance: Provenance) -> Self {
        Self { value, provenance }
    }

    pub fn value(&self) -> &Value {
        &self.value
    }

    pub fn provenance(&self) -> &Provenance {
        &self.provenance
    }
}

/// Bounded policy input, separate from the caller's persistent relationship model.
/// Values and provenance must already satisfy the owning model's size bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipAxes<Value, Provenance> {
    trust: RelationshipAxisValue<Value, Provenance>,
    affection: RelationshipAxisValue<Value, Provenance>,
    respect: RelationshipAxisValue<Value, Provenance>,
    fear: RelationshipAxisValue<Value, Provenance>,
    suspicion: RelationshipAxisValue<Value, Provenance>,
    debt: RelationshipAxisValue<Value, Provenance>,
    familiarity: RelationshipAxisValue<Value, Provenance>,
}

impl<Value, Provenance> RelationshipAxes<Value, Provenance> {
    /// Accepts exactly seven axes in enum order: trust through familiarity.
    pub fn new(axes: [RelationshipAxisValue<Value, Provenance>; 7]) -> Self {
        let [
            trust,
            affection,
            respect,
            fear,
            suspicion,
            debt,
            familiarity,
        ] = axes;
        Self {
            trust,
            affection,
            respect,
            fear,
            suspicion,
            debt,
            familiarity,
        }
    }

    pub fn axis(&self, axis: RelationshipAxis) -> &RelationshipAxisValue<Value, Provenance> {
        match axis {
            RelationshipAxis::Trust => &self.trust,
            RelationshipAxis::Affection => &self.affection,
            RelationshipAxis::Respect => &self.respect,
            RelationshipAxis::Fear => &self.fear,
            RelationshipAxis::Suspicion => &self.suspicion,
            RelationshipAxis::Debt => &self.debt,
            RelationshipAxis::Familiarity => &self.familiarity,
        }
    }

    fn replace(
        &mut self,
        axis: RelationshipAxis,
        replacement: RelationshipAxisValue<Value, Provenance>,
    ) {
        let destination = match axis {
            RelationshipAxis::Trust => &mut self.trust,
            RelationshipAxis::Affection => &mut self.affection,
            RelationshipAxis::Respect => &mut self.respect,
            RelationshipAxis::Fear => &mut self.fear,
            RelationshipAxis::Suspicion => &mut self.suspicion,
            RelationshipAxis::Debt => &mut self.debt,
            RelationshipAxis::Familiarity => &mut self.familiarity,
        };
        *destination = replacement;
    }
}

/// Supplies actual model identities, basis, bounded values and source policy.
/// Implementations validate both the proposed value and its causal provenance;
/// this primitive supplies no score units, relationship arithmetic or authority.
pub trait RelationshipOwner {
    type Subject: Clone + PartialEq;
    type Basis: Clone + PartialEq;
    type Value: Clone;
    type Provenance: Clone;
    type Refusal;

    fn validate_axis_change(
        &self,
        axis: RelationshipAxis,
        current: &RelationshipAxisValue<Self::Value, Self::Provenance>,
        proposed: &RelationshipAxisValue<Self::Value, Self::Provenance>,
    ) -> Result<(), Self::Refusal>;
}

/// Immutable caller-supplied directional basis for one staged policy operation.
pub struct RelationshipView<'a, Owner: RelationshipOwner> {
    pub subject: &'a Owner::Subject,
    pub target: &'a Owner::Subject,
    pub basis: &'a Owner::Basis,
    pub axes: &'a RelationshipAxes<Owner::Value, Owner::Provenance>,
}

/// A single named-axis replacement bound to an expected directional basis.
pub struct RelationshipChange<'a, Owner: RelationshipOwner> {
    pub subject: &'a Owner::Subject,
    pub target: &'a Owner::Subject,
    pub expected_basis: &'a Owner::Basis,
    pub axis: RelationshipAxis,
    pub replacement: RelationshipAxisValue<Owner::Value, Owner::Provenance>,
}

/// Refusals leave the immutable input intact and produce no candidate changes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelationshipRefusal<Refusal> {
    WrongDirection,
    StaleBasis,
    PolicyRefusal(Refusal),
}

/// Owned candidate data; the session/model owner must revalidate and commit it.
/// This result never commits or publishes authoritative relationship state.
pub struct RelationshipProposal<Owner: RelationshipOwner> {
    subject: Owner::Subject,
    target: Owner::Subject,
    expected_basis: Owner::Basis,
    changed_axis: RelationshipAxis,
    axes: RelationshipAxes<Owner::Value, Owner::Provenance>,
}

impl<Owner: RelationshipOwner> RelationshipProposal<Owner> {
    pub fn subject(&self) -> &Owner::Subject {
        &self.subject
    }

    pub fn target(&self) -> &Owner::Subject {
        &self.target
    }

    pub fn expected_basis(&self) -> &Owner::Basis {
        &self.expected_basis
    }

    pub fn changed_axis(&self) -> RelationshipAxis {
        self.changed_axis
    }

    pub fn axes(&self) -> &RelationshipAxes<Owner::Value, Owner::Provenance> {
        &self.axes
    }

    pub fn into_axes(self) -> RelationshipAxes<Owner::Value, Owner::Provenance> {
        self.axes
    }
}

/// Proposes exactly one validated axis replacement, preserving the other six
/// values and their provenance. Binding refusals precede policy validation.
/// Work is fixed at seven axes; caller-owned values remain subject to owner bounds.
pub fn propose_relationship_change<Owner: RelationshipOwner>(
    owner: &Owner,
    current: RelationshipView<'_, Owner>,
    change: RelationshipChange<'_, Owner>,
) -> Result<RelationshipProposal<Owner>, RelationshipRefusal<Owner::Refusal>> {
    if current.subject != change.subject || current.target != change.target {
        return Err(RelationshipRefusal::WrongDirection);
    }
    if current.basis != change.expected_basis {
        return Err(RelationshipRefusal::StaleBasis);
    }
    owner
        .validate_axis_change(
            change.axis,
            current.axes.axis(change.axis),
            &change.replacement,
        )
        .map_err(RelationshipRefusal::PolicyRefusal)?;
    let mut axes = current.axes.clone();
    axes.replace(change.axis, change.replacement);
    Ok(RelationshipProposal {
        subject: current.subject.clone(),
        target: current.target.clone(),
        expected_basis: current.basis.clone(),
        changed_axis: change.axis,
        axes,
    })
}
