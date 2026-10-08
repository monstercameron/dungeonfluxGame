//! Private G12 D04 contract example, mounted only by its boundary test.
//! Identity continuity and critical celebration have separate eligibility owners. Failure or
//! suppression of an optional celebration cannot erase valid committed identity continuity.
//! These borrowed admissions do not authenticate source rights, success, audience or asset bytes.

use df_media::continuity::{
    AdmittedPreparedBytes, ContinuityBinding, ContinuityContext, ContinuityError, ContinuityLimits,
    ContinuitySelection, PreparedRepresentation, PreparedTier, select_continuity,
};
use df_model::checkpoint::{Checkpoint, CriticalCueEligibility, ReferenceInventory};
use df_presentation::critical::{
    CriticalCueError, CriticalCueRequest, PermittedCriticalCue, ResolvedOutcomeAdmission,
    select_committed_celebration,
};
use df_presentation::moment_selection::{
    MomentAlternative, MomentContext, MomentError, MomentLimits, MomentSelection, PermittedMoment,
    select_committed_moment_plan,
};

pub(crate) struct EligibilityRequest<'a> {
    pub critical: CriticalCueRequest<'a>,
    pub outcome: Option<ResolvedOutcomeAdmission<'a>>,
    pub critical_permission: Option<PermittedCriticalCue<'a>>,
    pub moment: MomentContext<'a>,
    pub moment_permission: Option<PermittedMoment<'a>>,
    pub alternatives: &'a [MomentAlternative<'a>],
    pub continuity: ContinuityBinding<'a>,
    pub tiers: &'a [PreparedTier],
    pub prepared: &'a [PreparedRepresentation<'a>],
    pub admitted_bytes: &'a [AdmittedPreparedBytes<'a>],
}

pub(crate) struct EligibilityLimits {
    pub moment: MomentLimits,
    pub continuity: ContinuityLimits,
}

/// Keep each owner's actual refusal/disposition rather than inventing one global readiness flag.
/// A valid identity still can accompany a refused critical cue or an unavailable moment plan.
pub(crate) struct CommittedEligibility<'a> {
    pub critical: Result<&'a CriticalCueEligibility, CriticalCueError>,
    pub moment: Result<MomentSelection<'a>, MomentError>,
    pub continuity: Result<ContinuitySelection<'a>, ContinuityError>,
}

/// Execute the existing bounded selectors over one immutable checkpoint and current inventory.
/// Critical eligibility requires exact accepted decision/fact and explicit resolved-source
/// admission. Continuity requires every exact canonical lineage fact in an accepted decision.
/// The moment selector additionally checks that the supplied plan records are actually present.
/// No die value, tentative record, model text or ready-looking reference replaces these owners.
/// This example makes no fact, cue, demand, bytes, dispatch, publication or commit mutation.
pub(crate) fn select_committed_eligibility<'a>(
    current: &'a Checkpoint,
    inventory: ReferenceInventory<'a>,
    request: EligibilityRequest<'a>,
    limits: EligibilityLimits,
) -> CommittedEligibility<'a> {
    let critical = select_committed_celebration(
        current,
        request.critical,
        ReferenceInventory {
            rules: inventory.rules,
            content: inventory.content,
            resources: inventory.resources,
            assets: inventory.assets,
        },
        limits.continuity.checkpoint,
        request.outcome,
        request.critical_permission,
    );
    let moment = select_committed_moment_plan(
        current,
        request.moment,
        request.critical.now,
        request.moment_permission,
        request.alternatives,
        limits.moment,
    );
    let continuity = select_continuity(
        ContinuityContext {
            current,
            expected: request.critical.context.basis,
            pins: request.critical.context.pins,
            inventory,
        },
        request.continuity,
        request.tiers,
        request.prepared,
        request.admitted_bytes,
        limits.continuity,
    );
    CommittedEligibility {
        critical,
        moment,
        continuity,
    }
}
