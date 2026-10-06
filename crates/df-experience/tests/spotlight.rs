use std::time::Duration;

use df_experience::cooldown::{RefusalMemory, RefusalScope};
use df_experience::spotlight::{
    SpotlightCandidate, SpotlightError, SpotlightLimits, SpotlightOutcome, SpotlightParticipant,
    SpotlightReason, SpotlightRequest, recommend,
};
use df_experience::window::{AcceptedActivity, ActivityWindow, ActivityWindowLimits};
use df_model::checkpoint::{
    AudienceScope, Basis, CharacterHook, ContentReference, FactId, PresenceKind, RecordId,
};
use df_types::{
    MemberId, OperationId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision,
};

fn member(value: u8) -> MemberId {
    MemberId::from_bytes(&[value; 16]).unwrap()
}

fn record(value: u8) -> RecordId {
    RecordId::from_bytes(&[value; 16]).unwrap()
}

fn basis() -> Basis {
    Basis {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        revision: SessionRevision::new(RecoveryEpoch::new(1).unwrap(), 4),
    }
}

fn source(entry: &str) -> ContentReference {
    ContentReference {
        package: RevisionLabel::new(Some("pack-v1")).unwrap(),
        entry: RevisionLabel::new(Some(entry)).unwrap(),
    }
}

struct Fixture {
    policy: ContentReference,
    activity: ActivityWindow<MemberId, OperationId>,
    refusals: RefusalMemory<MemberId, RecordId>,
    participants: Vec<SpotlightParticipant>,
    hooks: Vec<CharacterHook>,
}

impl Fixture {
    fn new() -> Self {
        let participants = (1..=3)
            .map(|id| SpotlightParticipant {
                member: member(id),
                opted_in: true,
                consent_generation: 7,
                presence: PresenceKind::Connected,
            })
            .collect();
        let hooks = (1..=3)
            .map(|id| CharacterHook {
                id: record(id + 10),
                member: member(id),
                definition: source("character-hook"),
                source_facts: vec![FactId::from_bytes(&[id; 16]).unwrap()],
                consent_generation: 7,
                audience: AudienceScope::Shared,
            })
            .collect();
        Self {
            policy: source("experience-policy-v1"),
            activity: ActivityWindow::new(
                basis().session,
                basis().run,
                ActivityWindowLimits {
                    max_events: 16,
                    horizon: Duration::from_secs(60),
                },
            )
            .unwrap(),
            refusals: RefusalMemory::new(
                RefusalScope {
                    session: basis().session,
                    run: basis().run,
                },
                16,
                Duration::from_secs(10),
            )
            .unwrap(),
            participants,
            hooks,
        }
    }

    fn candidates(&self) -> Vec<SpotlightCandidate<'_>> {
        self.hooks
            .iter()
            .enumerate()
            .map(|(index, hook)| SpotlightCandidate {
                id: record(index as u8 + 20),
                hook,
                basis: basis(),
                policy: &self.policy,
                expires: Duration::from_secs(30),
                relevant: true,
            })
            .collect()
    }

    fn run(
        &self,
        candidates: &[SpotlightCandidate<'_>],
        now: u64,
        maximum: usize,
    ) -> Result<SpotlightOutcome, SpotlightError> {
        recommend(SpotlightRequest {
            basis: basis(),
            now: Duration::from_secs(now),
            policy: &self.policy,
            participants: &self.participants,
            candidates,
            activity: &self.activity,
            refusals: &self.refusals,
            limits: SpotlightLimits {
                max_participants: 4,
                max_candidates: 8,
                max_recommendations: maximum,
            },
        })
    }

    fn observe(&mut self, participant: u8, id: u8) {
        self.activity
            .observe(
                AcceptedActivity {
                    session: basis().session,
                    run: basis().run,
                    operation: OperationId::from_bytes(&[id; 16]).unwrap(),
                    revision: basis().revision,
                    participant: member(participant),
                    event: OperationId::from_bytes(&[id; 16]).unwrap(),
                    at: Duration::ZERO,
                },
                basis().revision,
                Duration::ZERO,
            )
            .unwrap();
    }
}

#[test]
fn current_accepted_counts_rank_quiet_members_without_rewarding_spam() {
    let mut fixture = Fixture::new();
    fixture.observe(1, 1);
    fixture.observe(1, 2);
    fixture.observe(2, 3);
    let result = fixture.run(&fixture.candidates(), 0, 3).unwrap();
    let order: Vec<_> = result
        .recommendations
        .iter()
        .map(|proposal| proposal.member)
        .collect();
    assert_eq!(order, vec![member(3), member(2), member(1)]);
    assert_eq!(result.recommendations[0].accepted_activity, 0);
    assert_eq!(result.recommendations[0].basis, basis());
    assert_eq!(
        result.recommendations[0].source,
        fixture.hooks[2].definition
    );
    assert_eq!(fixture.activity.len(), 3);
    assert!(fixture.refusals.is_empty());
}

#[test]
fn equal_counts_use_stable_identity_and_ignore_candidate_input_order() {
    let fixture = Fixture::new();
    let mut candidates = fixture.candidates();
    let first = fixture.run(&candidates, 0, 2).unwrap();
    candidates.reverse();
    assert_eq!(fixture.run(&candidates, 0, 2).unwrap(), first);
    assert_eq!(
        first
            .recommendations
            .iter()
            .map(|value| value.member)
            .collect::<Vec<_>>(),
        vec![member(1), member(2)]
    );
    assert_eq!(first.decisions[2].reason, SpotlightReason::OutputLimit);
}

#[test]
fn opt_out_consent_revocation_and_afk_are_valid_no_op_outcomes() {
    let mut fixture = Fixture::new();
    fixture.participants[0].opted_in = false;
    fixture.participants[1].consent_generation = 8;
    fixture.participants[2].presence = PresenceKind::VoluntaryAfk;
    let result = fixture.run(&fixture.candidates(), 0, 3).unwrap();
    assert!(result.recommendations.is_empty());
    assert_eq!(
        result
            .decisions
            .iter()
            .map(|value| value.reason)
            .collect::<Vec<_>>(),
        vec![
            SpotlightReason::NotParticipating,
            SpotlightReason::ConsentChanged,
            SpotlightReason::Away
        ]
    );
    assert!(fixture.activity.is_empty());
    assert!(fixture.refusals.is_empty());
}

#[test]
fn private_hook_contents_cannot_change_a_public_recommendation() {
    let mut fixture = Fixture::new();
    fixture.hooks[0].audience = AudienceScope::Members(vec![member(1)]);
    let first = fixture.run(&fixture.candidates(), 0, 3).unwrap();
    fixture.hooks[0].definition = source("unrevealed-backstory-secret");
    fixture.hooks[0].source_facts.clear();
    fixture.hooks[0].member = member(99);
    fixture.hooks[0].consent_generation = 0;
    assert_eq!(fixture.run(&fixture.candidates(), 0, 3).unwrap(), first);
    assert_eq!(first.decisions[0].reason, SpotlightReason::PrivateHook);
    assert!(
        first
            .recommendations
            .iter()
            .all(|value| value.member != member(1))
    );
}

#[test]
fn stale_irrelevant_and_exact_expiry_candidates_are_not_delivered() {
    let fixture = Fixture::new();
    let mut candidates = fixture.candidates();
    candidates[0].basis.revision = basis().revision.next_sequence().unwrap();
    candidates[1].relevant = false;
    candidates[2].expires = Duration::ZERO;
    let result = fixture.run(&candidates, 0, 3).unwrap();
    assert!(result.recommendations.is_empty());
    assert_eq!(
        result
            .decisions
            .iter()
            .map(|value| value.reason)
            .collect::<Vec<_>>(),
        vec![
            SpotlightReason::StaleBasis,
            SpotlightReason::Irrelevant,
            SpotlightReason::Expired
        ]
    );
}

#[test]
fn changed_policy_and_hooks_without_evidence_are_excluded() {
    let mut fixture = Fixture::new();
    fixture.hooks[1].source_facts.clear();
    let other_policy = source("experience-policy-v2");
    let mut candidates = fixture.candidates();
    candidates[0].policy = &other_policy;
    let result = fixture.run(&candidates, 0, 3).unwrap();
    assert_eq!(result.decisions[0].reason, SpotlightReason::PolicyChanged);
    assert_eq!(result.decisions[1].reason, SpotlightReason::Ungrounded);
    assert_eq!(result.recommendations.len(), 1);
}

#[test]
fn explicit_refusal_cools_down_all_opportunities_and_retry_does_not_extend_it() {
    let mut fixture = Fixture::new();
    let scope = RefusalScope {
        session: basis().session,
        run: basis().run,
    };
    fixture
        .refusals
        .record_decline(scope, member(1), record(20), Duration::ZERO)
        .unwrap();
    let blocked = fixture.run(&fixture.candidates(), 0, 3).unwrap();
    assert_eq!(blocked.decisions[0].reason, SpotlightReason::CoolingDown);
    assert_eq!(fixture.run(&fixture.candidates(), 0, 3).unwrap(), blocked);
    fixture.activity.advance(Duration::from_secs(10)).unwrap();
    let restored = fixture.run(&fixture.candidates(), 10, 3).unwrap();
    assert_eq!(restored.decisions[0].reason, SpotlightReason::Selected);
    assert_eq!(fixture.refusals.len(), 1);
}

#[test]
fn multiple_hooks_for_one_member_do_not_consume_multiple_spotlights() {
    let fixture = Fixture::new();
    let mut candidates = fixture.candidates();
    candidates.push(SpotlightCandidate {
        id: record(99),
        hook: &fixture.hooks[0],
        basis: basis(),
        policy: &fixture.policy,
        expires: Duration::from_secs(30),
        relevant: true,
    });
    let result = fixture.run(&candidates, 0, 3).unwrap();
    assert_eq!(result.recommendations.len(), 3);
    assert_eq!(
        result.decisions.last().unwrap().reason,
        SpotlightReason::Alternative
    );
}

#[test]
fn bounded_inputs_and_duplicate_identities_refuse_the_whole_request() {
    let mut fixture = Fixture::new();
    let candidates = fixture.candidates();
    let mut oversized: Vec<_> = candidates
        .iter()
        .cycle()
        .take(9)
        .map(|value| SpotlightCandidate {
            id: value.id,
            hook: value.hook,
            basis: value.basis,
            policy: value.policy,
            expires: value.expires,
            relevant: value.relevant,
        })
        .collect();
    assert_eq!(fixture.run(&oversized, 0, 3), Err(SpotlightError::Capacity));
    oversized.truncate(4);
    assert_eq!(
        fixture.run(&oversized, 0, 3),
        Err(SpotlightError::DuplicateOpportunity)
    );
    drop(oversized);
    drop(candidates);
    fixture.participants.push(fixture.participants[0]);
    assert_eq!(
        fixture.run(&fixture.candidates(), 0, 3),
        Err(SpotlightError::DuplicateParticipant)
    );
    assert_eq!(fixture.run(&[], 0, 0), Err(SpotlightError::InvalidLimits));
}

#[test]
fn mismatched_refusal_scope_returns_a_typed_failure_without_selection() {
    let mut fixture = Fixture::new();
    fixture.refusals = RefusalMemory::new(
        RefusalScope {
            session: SessionId::from_bytes(&[9; 16]).unwrap(),
            run: basis().run,
        },
        16,
        Duration::from_secs(10),
    )
    .unwrap();
    assert_eq!(
        fixture.run(&fixture.candidates(), 0, 3),
        Err(SpotlightError::Refusal(
            df_experience::cooldown::RefusalError::WrongSession
        ))
    );
}

#[test]
fn recommendation_rejects_foreign_unadvanced_or_stale_activity_windows() {
    use df_experience::window::WindowError;
    let mut fixture = Fixture::new();
    assert_eq!(
        fixture.run(&fixture.candidates(), 1, 3),
        Err(SpotlightError::Activity(WindowError::StaleBasis))
    );
    fixture.activity = ActivityWindow::new(
        SessionId::from_bytes(&[9; 16]).unwrap(),
        basis().run,
        ActivityWindowLimits {
            max_events: 16,
            horizon: Duration::from_secs(60),
        },
    )
    .unwrap();
    assert_eq!(
        fixture.run(&fixture.candidates(), 0, 3),
        Err(SpotlightError::Activity(WindowError::WrongSession))
    );
    fixture.activity = ActivityWindow::new(
        basis().session,
        RunId::from_bytes(&[9; 16]).unwrap(),
        ActivityWindowLimits {
            max_events: 16,
            horizon: Duration::from_secs(60),
        },
    )
    .unwrap();
    assert_eq!(
        fixture.run(&fixture.candidates(), 0, 3),
        Err(SpotlightError::Activity(WindowError::WrongRun))
    );

    let mut fixture = Fixture::new();
    let stale = SessionRevision::new(basis().revision.epoch(), 3);
    fixture
        .activity
        .observe(
            AcceptedActivity {
                session: basis().session,
                run: basis().run,
                operation: OperationId::from_bytes(&[1; 16]).unwrap(),
                revision: stale,
                participant: member(1),
                event: OperationId::from_bytes(&[1; 16]).unwrap(),
                at: Duration::ZERO,
            },
            stale,
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(
        fixture.run(&fixture.candidates(), 0, 3),
        Err(SpotlightError::Activity(WindowError::StaleBasis))
    );
}

#[test]
fn empty_window_remains_scoped_and_regressing_recommendation_time_is_rejected() {
    use df_experience::window::WindowError;
    let mut fixture = Fixture::new();
    assert_eq!(
        fixture
            .run(&fixture.candidates(), 0, 3)
            .unwrap()
            .recommendations
            .len(),
        3
    );
    fixture.activity.advance(Duration::from_secs(1)).unwrap();
    assert_eq!(
        fixture
            .run(&fixture.candidates(), 1, 3)
            .unwrap()
            .recommendations
            .len(),
        3
    );
    assert_eq!(
        fixture.run(&fixture.candidates(), 0, 3),
        Err(SpotlightError::Activity(WindowError::TimeRegression))
    );
}
