#[path = "../src/grounded_claim_contract.rs"]
mod grounded_claim_contract;

// Reuse the existing source-bound canonical checkpoint fixture rather than
// introducing a parallel Model or Knowledge implementation.
mod canonical_fixture {
    include!("listener_safe_speech_contract.rs");

    mod grounded_claim_cases {
        use super::*;
        use crate::grounded_claim_contract::{
            GroundedCandidate, GroundedLimits, GroundingError, validate_grounded_claim,
        };

        // A faithful local source-owner fixture. The real native issuer remains
        // separate: caller flags, fluent text and matching pins confer no authority.
        struct ReviewedSource<'a> {
            checkpoint: &'a Checkpoint,
            access: bool,
            deceit: bool,
            false_belief: bool,
        }

        impl SpeechSourceOwner for ReviewedSource<'_> {
            fn validate(
                &self,
                current: Basis,
                admitted: &CheckpointPins,
                version: &SpeechVersions,
                request: &SpeechProposal,
            ) -> Result<(), SpeechSourceError> {
                RegisteredSource {
                    access: self.access,
                    deceit: self.deceit,
                }
                .validate(current, admitted, version, request)?;
                for item in &request.claims {
                    let record = self
                        .checkpoint
                        .state()
                        .beliefs
                        .iter()
                        .find(|record| record.id == item.claim)
                        .ok_or(SpeechSourceError::Unsupported)?;
                    if item.claim == claim_id(30) && record.claim != "Ada has 13 coins." {
                        return Err(SpeechSourceError::Unsupported);
                    }
                    if item.intent == SpeechIntent::FalseBelief && !self.false_belief {
                        return Err(SpeechSourceError::Unsupported);
                    }
                }
                Ok(())
            }
        }

        fn reviewed(checkpoint: &Checkpoint) -> ReviewedSource<'_> {
            ReviewedSource {
                checkpoint,
                access: true,
                deceit: false,
                false_belief: false,
            }
        }

        fn bounds() -> GroundedLimits {
            GroundedLimits {
                maximum_text_bytes: 256,
                maximum_slots: 3,
            }
        }

        fn expected_slots() -> Vec<ExpressionSlot<'static>> {
            vec![
                ExpressionSlot {
                    kind: SlotKind::Name,
                    value: "Ada",
                },
                ExpressionSlot {
                    kind: SlotKind::Outcome,
                    value: "has",
                },
                ExpressionSlot {
                    kind: SlotKind::Number,
                    value: "13",
                },
            ]
        }

        fn candidate<'a>(text: &'a str, slots: &'a [ExpressionSlot<'a>]) -> GroundedCandidate<'a> {
            GroundedCandidate {
                claim: claim_id(30),
                text,
                slots,
            }
        }

        #[test]
        fn admitted_outcome_name_number_are_borrowed_from_canonical_claim() {
            let checkpoint = checkpoint(speech_state("SECRET")).unwrap();
            let before = checkpoint.clone();
            let versions = versions();
            let owner = reviewed(&checkpoint);
            let current = observation(&checkpoint, &versions, Some(&owner), ObserverScope::Shared);
            let plan = admit_speech(proposal(), &current).unwrap();
            let context = project_expression_context(&plan, &current).unwrap();
            let slots = expected_slots();
            let result = validate_grounded_claim(
                &context,
                &current,
                candidate("Ada has 13 coins.", &slots),
                bounds(),
            )
            .unwrap();
            assert_eq!(
                result.text.as_ptr(),
                checkpoint.state().beliefs[0].claim.as_ptr()
            );
            assert_eq!(result.slots, slots);
            assert_eq!(result.evidence, checkpoint.state().beliefs[0].evidence);
            assert_eq!(result.holder, entity(4));
            assert_eq!(result.subject, entity(4));
            assert_eq!(checkpoint, before);
        }

        #[test]
        fn fluent_changed_name_number_outcome_and_negation_are_refused() {
            let checkpoint = checkpoint(speech_state("SECRET")).unwrap();
            let versions = versions();
            let owner = reviewed(&checkpoint);
            let current = observation(&checkpoint, &versions, Some(&owner), ObserverScope::Shared);
            let plan = admit_speech(proposal(), &current).unwrap();
            let context = project_expression_context(&plan, &current).unwrap();
            let slots = expected_slots();
            for invented in [
                "Bea has 13 coins.",
                "Ada has 99 coins.",
                "Ada lost 13 coins.",
                "Ada does not have 13 coins.",
                "Ada has 13 coins and owns the harbor.",
            ] {
                assert!(matches!(
                    validate_grounded_claim(
                        &context,
                        &current,
                        candidate(invented, &slots),
                        bounds()
                    ),
                    Err(GroundingError::TextMismatch)
                ));
            }
        }

        #[test]
        fn fabricated_slot_values_kinds_order_and_omission_are_refused() {
            let checkpoint = checkpoint(speech_state("SECRET")).unwrap();
            let versions = versions();
            let owner = reviewed(&checkpoint);
            let current = observation(&checkpoint, &versions, Some(&owner), ObserverScope::Shared);
            let plan = admit_speech(proposal(), &current).unwrap();
            let context = project_expression_context(&plan, &current).unwrap();
            for change in 0..4 {
                let mut slots = expected_slots();
                match change {
                    0 => slots[2].value = "99",
                    1 => slots[2].kind = SlotKind::Name,
                    2 => slots.swap(0, 2),
                    _ => {
                        slots.pop();
                    }
                }
                assert!(matches!(
                    validate_grounded_claim(
                        &context,
                        &current,
                        candidate("Ada has 13 coins.", &slots),
                        bounds(),
                    ),
                    Err(GroundingError::SlotMismatch)
                ));
            }
        }

        #[test]
        fn missing_private_and_private_evidence_claims_cannot_supply_grounded_text() {
            let checkpoint = checkpoint(speech_state("SECRET")).unwrap();
            let versions = versions();
            let owner = reviewed(&checkpoint);
            let current = observation(&checkpoint, &versions, Some(&owner), ObserverScope::Shared);
            let plan = admit_speech(proposal(), &current).unwrap();
            let context = project_expression_context(&plan, &current).unwrap();
            for id in [31, 32, 33, 90] {
                let result = validate_grounded_claim(
                    &context,
                    &current,
                    GroundedCandidate {
                        claim: claim_id(id),
                        text: "claimed private text",
                        slots: &[],
                    },
                    bounds(),
                );
                assert!(matches!(result, Err(GroundingError::ClaimUnavailable)));
                assert!(!format!("{:?}", result.err().unwrap()).contains("claimed private text"));
            }
        }

        #[test]
        fn invalid_and_semantically_unreviewed_spans_fail_real_speech_admission() {
            let checkpoint = checkpoint(speech_state("SECRET")).unwrap();
            let versions = versions();
            let owner = reviewed(&checkpoint);
            let current = observation(&checkpoint, &versions, Some(&owner), ObserverScope::Shared);
            let mut invalid = proposal();
            invalid.claims[0].slots[2].end = 999;
            assert!(matches!(
                admit_speech(invalid, &current),
                Err(SpeechError::InvalidSlot)
            ));
            let mut mismatch = proposal();
            mismatch.claims[0].slots[2].start = 9;
            assert!(matches!(
                admit_speech(mismatch, &current),
                Err(SpeechError::Source(SpeechSourceError::Unsupported))
            ));
            let mut source_mismatch = proposal();
            source_mismatch.source.entry = label("unreviewed-entry");
            assert!(matches!(
                admit_speech(source_mismatch, &current),
                Err(SpeechError::SourceMismatch)
            ));
            let mut unknown = proposal();
            unknown.claims[0].claim = claim_id(90);
            assert!(matches!(
                admit_speech(unknown, &current),
                Err(SpeechError::UnknownClaim)
            ));
        }

        #[test]
        fn source_owner_rejects_an_initially_fabricated_canonical_claim() {
            let mut state = speech_state("SECRET");
            state.beliefs[0].claim = "Ada has 99 coins.".to_owned();
            let checkpoint = checkpoint(state).unwrap();
            let versions = versions();
            let owner = reviewed(&checkpoint);
            let current = observation(&checkpoint, &versions, Some(&owner), ObserverScope::Shared);
            assert!(matches!(
                admit_speech(proposal(), &current),
                Err(SpeechError::Source(SpeechSourceError::Unsupported))
            ));
        }

        #[test]
        fn missing_or_denied_owner_refuses_both_admission_and_later_consumption() {
            let checkpoint = checkpoint(speech_state("SECRET")).unwrap();
            let versions = versions();
            let owner = reviewed(&checkpoint);
            let current = observation(&checkpoint, &versions, Some(&owner), ObserverScope::Shared);
            let plan = admit_speech(proposal(), &current).unwrap();
            let context = project_expression_context(&plan, &current).unwrap();
            let slots = expected_slots();
            let missing = observation(&checkpoint, &versions, None, ObserverScope::Shared);
            assert!(matches!(
                admit_speech(proposal(), &missing),
                Err(SpeechError::SourceUnavailable)
            ));
            assert!(matches!(
                validate_grounded_claim(
                    &context,
                    &missing,
                    candidate("Ada has 13 coins.", &slots),
                    bounds()
                ),
                Err(GroundingError::Speech(SpeechError::SourceUnavailable))
            ));
            let denied = ReviewedSource {
                access: false,
                ..reviewed(&checkpoint)
            };
            let denied_current =
                observation(&checkpoint, &versions, Some(&denied), ObserverScope::Shared);
            assert!(matches!(
                admit_speech(proposal(), &denied_current),
                Err(SpeechError::Source(SpeechSourceError::AccessDenied))
            ));
            assert!(matches!(
                validate_grounded_claim(
                    &context,
                    &denied_current,
                    candidate("Ada has 13 coins.", &slots),
                    bounds()
                ),
                Err(GroundingError::Speech(SpeechError::Source(
                    SpeechSourceError::AccessDenied
                )))
            ));
        }

        #[test]
        fn stale_basis_wrong_pins_and_source_versions_refuse_cached_claims() {
            let checkpoint = checkpoint(speech_state("SECRET")).unwrap();
            let versions = versions();
            let owner = reviewed(&checkpoint);
            let current = observation(&checkpoint, &versions, Some(&owner), ObserverScope::Shared);
            let plan = admit_speech(proposal(), &current).unwrap();
            let context = project_expression_context(&plan, &current).unwrap();
            let slots = expected_slots();
            let mut stale =
                observation(&checkpoint, &versions, Some(&owner), ObserverScope::Shared);
            stale.basis.revision = revision(2, 9);
            assert!(matches!(
                validate_grounded_claim(
                    &context,
                    &stale,
                    candidate("Ada has 13 coins.", &slots),
                    bounds()
                ),
                Err(GroundingError::Speech(SpeechError::Checkpoint(_)))
            ));
            let mut wrong_pins = checkpoint.pins().clone();
            wrong_pins.content.content_digest = ContentDigest([99; 32]);
            let mut wrong =
                observation(&checkpoint, &versions, Some(&owner), ObserverScope::Shared);
            wrong.pins = &wrong_pins;
            assert!(matches!(
                validate_grounded_claim(
                    &context,
                    &wrong,
                    candidate("Ada has 13 coins.", &slots),
                    bounds()
                ),
                Err(GroundingError::Speech(SpeechError::Checkpoint(_)))
            ));
            for channel in 0..3 {
                let mut changed = versions.clone();
                match channel {
                    0 => changed.access = label("access-2"),
                    1 => changed.contract = label("contract-2"),
                    _ => changed.locale = LocaleTag::parse("fr-FR").unwrap(),
                }
                let changed_current =
                    observation(&checkpoint, &changed, Some(&owner), ObserverScope::Shared);
                assert!(matches!(
                    validate_grounded_claim(
                        &context,
                        &changed_current,
                        candidate("Ada has 13 coins.", &slots),
                        bounds()
                    ),
                    Err(GroundingError::Speech(SpeechError::StalePlan))
                ));
            }
        }

        #[test]
        fn changed_current_claim_text_cannot_reuse_a_prior_context() {
            let checkpoint = checkpoint(speech_state("SECRET")).unwrap();
            let versions = versions();
            let owner = reviewed(&checkpoint);
            let current = observation(&checkpoint, &versions, Some(&owner), ObserverScope::Shared);
            let plan = admit_speech(proposal(), &current).unwrap();
            let context = project_expression_context(&plan, &current).unwrap();
            let mut changed_state = speech_state("SECRET");
            changed_state.beliefs[0].claim = "Ada has 99 coins.".to_owned();
            let changed = super::checkpoint(changed_state).unwrap();
            let changed_current =
                observation(&changed, &versions, Some(&owner), ObserverScope::Shared);
            let slots = expected_slots();
            assert!(matches!(
                validate_grounded_claim(
                    &context,
                    &changed_current,
                    candidate("Ada has 13 coins.", &slots),
                    bounds()
                ),
                Err(GroundingError::Speech(SpeechError::ContextChanged))
            ));
        }

        #[test]
        fn exact_text_and_slot_limits_refuse_without_partial_output() {
            let checkpoint = checkpoint(speech_state("SECRET")).unwrap();
            let versions = versions();
            let owner = reviewed(&checkpoint);
            let current = observation(&checkpoint, &versions, Some(&owner), ObserverScope::Shared);
            let plan = admit_speech(proposal(), &current).unwrap();
            let context = project_expression_context(&plan, &current).unwrap();
            let slots = expected_slots();
            for tiny in [
                GroundedLimits {
                    maximum_text_bytes: 0,
                    maximum_slots: 3,
                },
                GroundedLimits {
                    maximum_text_bytes: 256,
                    maximum_slots: 2,
                },
            ] {
                assert!(matches!(
                    validate_grounded_claim(
                        &context,
                        &current,
                        candidate("Ada has 13 coins.", &slots),
                        tiny
                    ),
                    Err(GroundingError::Capacity)
                ));
            }
            let exact = GroundedLimits {
                maximum_text_bytes: "Ada has 13 coins.".len(),
                maximum_slots: 3,
            };
            assert!(
                validate_grounded_claim(
                    &context,
                    &current,
                    candidate("Ada has 13 coins.", &slots),
                    exact
                )
                .is_ok()
            );
        }

        #[test]
        fn admitted_deceit_false_belief_and_uncertainty_remain_attributed_not_truth() {
            let checkpoint = checkpoint(speech_state("SECRET")).unwrap();
            let before = checkpoint.clone();
            let versions = versions();
            let owner = ReviewedSource {
                deceit: true,
                false_belief: true,
                ..reviewed(&checkpoint)
            };
            let current = observation(&checkpoint, &versions, Some(&owner), ObserverScope::Shared);
            let slots = expected_slots();
            for intent in [SpeechIntent::ApprovedDeceit, SpeechIntent::FalseBelief] {
                let mut request = proposal();
                request.claims[0].intent = intent;
                request.claims[0].uncertainty = DeclaredUncertainty::Explicit;
                let plan = admit_speech(request, &current).unwrap();
                let context = project_expression_context(&plan, &current).unwrap();
                let result = validate_grounded_claim(
                    &context,
                    &current,
                    candidate("Ada has 13 coins.", &slots),
                    bounds(),
                )
                .unwrap();
                assert_eq!(result.intent, intent);
                assert_eq!(result.uncertainty, DeclaredUncertainty::Explicit);
                assert_eq!(result.holder, entity(4));
                assert_eq!(checkpoint, before);
            }
            let unapproved = reviewed(&checkpoint);
            let denied = observation(
                &checkpoint,
                &versions,
                Some(&unapproved),
                ObserverScope::Shared,
            );
            for intent in [SpeechIntent::ApprovedDeceit, SpeechIntent::FalseBelief] {
                let mut request = proposal();
                request.claims[0].intent = intent;
                assert!(matches!(
                    admit_speech(request, &denied),
                    Err(SpeechError::Source(SpeechSourceError::Unsupported))
                ));
            }
        }

        #[test]
        fn changing_listener_does_not_reuse_the_first_listeners_expression() {
            let checkpoint = checkpoint(speech_state("SECRET")).unwrap();
            let versions = versions();
            let owner = reviewed(&checkpoint);
            let current = observation(&checkpoint, &versions, Some(&owner), ObserverScope::Shared);
            let plan = admit_speech(proposal(), &current).unwrap();
            let context = project_expression_context(&plan, &current).unwrap();
            let next = observation(
                &checkpoint,
                &versions,
                Some(&owner),
                ObserverScope::Member(member(3)),
            );
            let slots = expected_slots();
            assert!(matches!(
                validate_grounded_claim(
                    &context,
                    &next,
                    candidate("Ada has 13 coins.", &slots),
                    bounds()
                ),
                Err(GroundingError::Speech(SpeechError::ContextChanged))
            ));
        }
    }
}
