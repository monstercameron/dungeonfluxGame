use std::cell::Cell;

use df_combat::candidates::{
    CandidateContext, CandidateError, CandidateLimits, LegalOfferOwner, enumerate_candidates,
};
use df_model::checkpoint::{
    Basis, CheckpointPins, ContentDigest, ContentPins, EntityId, OfferedResponse, RuleReference,
    RulesMode, RulesPins,
};
use df_types::{
    BuildIdentity, MemberId, RecoveryEpoch, RevisionLabel, RunId, SessionId, SessionRevision,
};

fn label(value: &str) -> RevisionLabel {
    RevisionLabel::new(Some(value)).unwrap()
}

fn basis() -> Basis {
    Basis {
        session: SessionId::from_bytes(&[1; 16]).unwrap(),
        run: RunId::from_bytes(&[2; 16]).unwrap(),
        revision: SessionRevision::new(RecoveryEpoch::new(1).unwrap(), 3),
    }
}

fn pins() -> CheckpointPins {
    CheckpointPins {
        rules: RulesPins {
            mode: RulesMode::Standard2024,
            ruleset: label("fixture-rules"),
            catalog: label("fixture-catalog"),
            catalog_digest: ContentDigest([1; 32]),
            source_manifest: label("fixture-source"),
            source_manifest_digest: ContentDigest([2; 32]),
            handler: label("fixture-handler"),
            handler_digest: ContentDigest([3; 32]),
        },
        content: ContentPins {
            content: label("fixture-content"),
            content_digest: ContentDigest([4; 32]),
            package: label("fixture-package"),
            package_digest: ContentDigest([5; 32]),
        },
        build: BuildIdentity::new(
            Some("fixture-source"),
            Some("fixture-native"),
            Some("fixture-wasm"),
            Some("fixture-config"),
            Some("fixture-content"),
        )
        .unwrap(),
    }
}

fn actor(value: u8) -> EntityId {
    EntityId::from_bytes(&[value; 16]).unwrap()
}

fn offer(value: &str) -> OfferedResponse {
    OfferedResponse {
        participant: MemberId::from_bytes(&[3; 16]).unwrap(),
        offer: label(value),
        options: vec![label("fixture-option")],
        source: RuleReference {
            catalog: label("fixture-catalog"),
            source: label("fixture-source"),
            entry: label("fixture-entry"),
            clause: label("fixture-clause"),
        },
    }
}

fn limits() -> CandidateLimits {
    CandidateLimits {
        max_offers: 4,
        max_identity_comparisons: 6,
        max_candidates: 3,
        max_candidate_bytes: 30,
    }
}

#[derive(Debug, Eq, PartialEq)]
enum OwnerError {
    WrongActor,
    RulesUnavailable,
    SourceUnavailable,
    CostUnavailable,
}

// The fixture uses existing model responses as opaque canonical payloads. These
// admitted labels/costs are test inputs, not NPC action definitions or game rules.
struct Authority {
    observation_calls: Cell<usize>,
    identity_calls: Cell<usize>,
    legality_calls: Cell<usize>,
    cost_calls: Cell<usize>,
    fail_legality: Option<&'static str>,
    fail_identity: bool,
    fail_cost: bool,
    cost: usize,
}

impl Authority {
    fn new() -> Self {
        Self {
            observation_calls: Cell::new(0),
            identity_calls: Cell::new(0),
            legality_calls: Cell::new(0),
            cost_calls: Cell::new(0),
            fail_legality: None,
            fail_identity: false,
            fail_cost: false,
            cost: 10,
        }
    }

    fn assert_no_callbacks(&self) {
        assert_eq!(self.observation_calls.get(), 0);
        assert_eq!(self.identity_calls.get(), 0);
        assert_eq!(self.legality_calls.get(), 0);
        assert_eq!(self.cost_calls.get(), 0);
    }
}

impl LegalOfferOwner for Authority {
    type Observation = EntityId;
    type Offer = OfferedResponse;
    type Error = OwnerError;

    fn validate_observation(
        &self,
        observation: &EntityId,
        _: CandidateContext<'_>,
    ) -> Result<(), OwnerError> {
        self.observation_calls.set(self.observation_calls.get() + 1);
        if *observation != actor(4) {
            return Err(OwnerError::WrongActor);
        }
        Ok(())
    }

    fn is_current_legal(
        &self,
        _: &EntityId,
        _: CandidateContext<'_>,
        offer: &OfferedResponse,
    ) -> Result<bool, OwnerError> {
        self.legality_calls.set(self.legality_calls.get() + 1);
        if self.fail_legality == Some(offer.offer.as_str()) {
            return Err(OwnerError::RulesUnavailable);
        }
        Ok(matches!(
            offer.offer.as_str(),
            "legal-a" | "legal-b" | "legal-c"
        ))
    }

    fn same_offer(
        &self,
        left: &OfferedResponse,
        right: &OfferedResponse,
    ) -> Result<bool, OwnerError> {
        self.identity_calls.set(self.identity_calls.get() + 1);
        if self.fail_identity {
            return Err(OwnerError::SourceUnavailable);
        }
        Ok(left.participant == right.participant && left.offer == right.offer)
    }

    fn offer_bytes(&self, _: &OfferedResponse) -> Result<usize, OwnerError> {
        self.cost_calls.set(self.cost_calls.get() + 1);
        if self.fail_cost {
            return Err(OwnerError::CostUnavailable);
        }
        Ok(self.cost)
    }
}

#[test]
fn only_admitted_current_legal_offers_retain_exact_identity_order_and_pins() {
    let authority = Authority::new();
    let basis = basis();
    let pins = pins();
    let context = CandidateContext {
        basis: &basis,
        pins: &pins,
    };
    let offers = [
        offer("legal-b"),
        offer("expired"),
        offer("legal-a"),
        offer("unknown"),
    ];
    let original = offers.clone();
    let result =
        enumerate_candidates(&authority, &actor(4), context, context, &offers, limits()).unwrap();
    assert_eq!(result.offers().len(), 2);
    assert!(std::ptr::eq(result.offers()[0], &offers[0]));
    assert!(std::ptr::eq(result.offers()[1], &offers[2]));
    assert!(std::ptr::eq(result.basis(), &basis));
    assert!(std::ptr::eq(result.pins(), &pins));
    assert_eq!(offers, original);
    assert_eq!(authority.legality_calls.get(), 4);
    assert_eq!(authority.cost_calls.get(), 2);
    assert_eq!(authority.identity_calls.get(), 6);
}

#[test]
fn changed_session_run_epoch_or_sequence_prevents_every_owner_callback() {
    let current = basis();
    let pins = pins();
    let context = CandidateContext {
        basis: &current,
        pins: &pins,
    };
    let offers = [offer("legal-a")];
    let mut changed = [current; 4];
    changed[0].session = SessionId::from_bytes(&[9; 16]).unwrap();
    changed[1].run = RunId::from_bytes(&[9; 16]).unwrap();
    changed[2].revision = SessionRevision::new(RecoveryEpoch::new(2).unwrap(), 3);
    changed[3].revision = current.revision.next_sequence().unwrap();
    for offered_basis in &changed {
        let authority = Authority::new();
        let offered = CandidateContext {
            basis: offered_basis,
            pins: &pins,
        };
        assert_eq!(
            enumerate_candidates(&authority, &actor(4), offered, context, &offers, limits()).err(),
            Some(CandidateError::StaleBasis)
        );
        authority.assert_no_callbacks();
    }
}

#[test]
fn every_rules_content_and_build_pin_component_is_rechecked() {
    let basis = basis();
    let current_pins = pins();
    let context = CandidateContext {
        basis: &basis,
        pins: &current_pins,
    };
    let offers = [offer("legal-a")];
    let mut changed = Vec::new();
    for index in 0..17 {
        let mut pins = current_pins.clone();
        match index {
            0 => pins.rules.mode = RulesMode::DisclosedCustom,
            1 => pins.rules.ruleset = label("changed"),
            2 => pins.rules.catalog = label("changed"),
            3 => pins.rules.catalog_digest = ContentDigest([9; 32]),
            4 => pins.rules.source_manifest = label("changed"),
            5 => pins.rules.source_manifest_digest = ContentDigest([9; 32]),
            6 => pins.rules.handler = label("changed"),
            7 => pins.rules.handler_digest = ContentDigest([9; 32]),
            8 => pins.content.content = label("changed"),
            9 => pins.content.content_digest = ContentDigest([9; 32]),
            10 => pins.content.package = label("changed"),
            11 => pins.content.package_digest = ContentDigest([9; 32]),
            12..=16 => {
                let mut labels = [
                    "fixture-source",
                    "fixture-native",
                    "fixture-wasm",
                    "fixture-config",
                    "fixture-content",
                ];
                labels[index - 12] = "changed";
                pins.build = BuildIdentity::new(
                    Some(labels[0]),
                    Some(labels[1]),
                    Some(labels[2]),
                    Some(labels[3]),
                    Some(labels[4]),
                )
                .unwrap();
            }
            _ => continue,
        }
        changed.push(pins);
    }
    assert_eq!(changed.len(), 17);
    for offered_pins in &changed {
        let authority = Authority::new();
        let offered = CandidateContext {
            basis: &basis,
            pins: offered_pins,
        };
        assert_eq!(
            enumerate_candidates(&authority, &actor(4), offered, context, &offers, limits()).err(),
            Some(CandidateError::StalePins)
        );
        authority.assert_no_callbacks();
    }
}

#[test]
fn input_and_comparison_caps_stop_before_owner_work() {
    let basis = basis();
    let pins = pins();
    let context = CandidateContext {
        basis: &basis,
        pins: &pins,
    };
    let offers = [offer("legal-a"), offer("legal-b")];
    for (limits, error) in [
        (
            CandidateLimits {
                max_offers: 1,
                ..limits()
            },
            CandidateError::OfferCapacity {
                required: 2,
                limit: 1,
            },
        ),
        (
            CandidateLimits {
                max_identity_comparisons: 0,
                ..limits()
            },
            CandidateError::ComparisonCapacity {
                required: 1,
                limit: 0,
            },
        ),
    ] {
        let authority = Authority::new();
        assert_eq!(
            enumerate_candidates(&authority, &actor(4), context, context, &offers, limits).err(),
            Some(error)
        );
        authority.assert_no_callbacks();
    }
}

#[test]
fn duplicate_canonical_identity_rejects_changed_payload_before_legality() {
    let authority = Authority::new();
    let basis = basis();
    let pins = pins();
    let context = CandidateContext {
        basis: &basis,
        pins: &pins,
    };
    let mut duplicate = offer("legal-a");
    duplicate.options = vec![label("different-option")];
    let offers = [offer("legal-a"), duplicate];
    assert_eq!(
        enumerate_candidates(&authority, &actor(4), context, context, &offers, limits()).err(),
        Some(CandidateError::Duplicate {
            first: 0,
            duplicate: 1
        })
    );
    assert_eq!(authority.legality_calls.get(), 0);
}

#[test]
fn output_count_and_byte_caps_reject_the_whole_legal_batch() {
    let basis = basis();
    let pins = pins();
    let context = CandidateContext {
        basis: &basis,
        pins: &pins,
    };
    let offers = [offer("legal-a"), offer("legal-b")];
    for (limits, error) in [
        (
            CandidateLimits {
                max_candidates: 1,
                ..limits()
            },
            CandidateError::CandidateCapacity { limit: 1 },
        ),
        (
            CandidateLimits {
                max_candidate_bytes: 19,
                ..limits()
            },
            CandidateError::ByteCapacity { limit: 19 },
        ),
    ] {
        let authority = Authority::new();
        assert_eq!(
            enumerate_candidates(&authority, &actor(4), context, context, &offers, limits).err(),
            Some(error)
        );
        assert_eq!(authority.legality_calls.get(), 2);
    }
}

#[test]
fn byte_sum_overflow_is_capacity_even_with_maximum_byte_budget() {
    let authority = Authority {
        cost: usize::MAX,
        ..Authority::new()
    };
    let basis = basis();
    let pins = pins();
    let context = CandidateContext {
        basis: &basis,
        pins: &pins,
    };
    let offers = [offer("legal-a"), offer("legal-b")];
    let limits = CandidateLimits {
        max_candidate_bytes: usize::MAX,
        ..limits()
    };
    assert_eq!(
        enumerate_candidates(&authority, &actor(4), context, context, &offers, limits).err(),
        Some(CandidateError::ByteCapacity { limit: usize::MAX })
    );
}

#[test]
fn owner_errors_preserve_typed_failure_without_partial_receipt() {
    let basis = basis();
    let pins = pins();
    let context = CandidateContext {
        basis: &basis,
        pins: &pins,
    };
    let offers = [offer("legal-a"), offer("legal-b")];
    for (authority, observation, expected) in [
        (Authority::new(), actor(9), OwnerError::WrongActor),
        (
            Authority {
                fail_identity: true,
                ..Authority::new()
            },
            actor(4),
            OwnerError::SourceUnavailable,
        ),
        (
            Authority {
                fail_legality: Some("legal-b"),
                ..Authority::new()
            },
            actor(4),
            OwnerError::RulesUnavailable,
        ),
        (
            Authority {
                fail_cost: true,
                ..Authority::new()
            },
            actor(4),
            OwnerError::CostUnavailable,
        ),
    ] {
        assert_eq!(
            enumerate_candidates(
                &authority,
                &observation,
                context,
                context,
                &offers,
                limits()
            )
            .err(),
            Some(CandidateError::Owner(expected))
        );
    }
}

#[test]
fn empty_or_all_nonlegal_offers_are_explicit_successful_no_action() {
    let basis = basis();
    let pins = pins();
    let context = CandidateContext {
        basis: &basis,
        pins: &pins,
    };
    let empty = [];
    let nonlegal = [offer("expired"), offer("unknown")];
    for offers in [&empty[..], &nonlegal[..]] {
        let authority = Authority::new();
        let limits = CandidateLimits {
            max_candidates: 0,
            max_candidate_bytes: 0,
            ..limits()
        };
        let result =
            enumerate_candidates(&authority, &actor(4), context, context, offers, limits).unwrap();
        assert!(result.offers().is_empty());
        assert_eq!(authority.cost_calls.get(), 0);
        assert_eq!(authority.observation_calls.get(), 1);
    }
}

#[test]
fn exact_input_work_output_and_byte_limits_succeed() {
    let authority = Authority::new();
    let basis = basis();
    let pins = pins();
    let context = CandidateContext {
        basis: &basis,
        pins: &pins,
    };
    let offers = [offer("legal-a"), offer("legal-b")];
    let limits = CandidateLimits {
        max_offers: 2,
        max_identity_comparisons: 1,
        max_candidates: 2,
        max_candidate_bytes: 20,
    };
    let result =
        enumerate_candidates(&authority, &actor(4), context, context, &offers, limits).unwrap();
    assert_eq!(result.offers().len(), 2);
}
