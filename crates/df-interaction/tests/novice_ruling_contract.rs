//! Executable Design examples: supplied mechanics differ from adjudicator discretion.
//! These examples establish no native issuer, authentication, durable resume or public UI.
#[path = "../../df-rules/src/ability_check.rs"]
mod reviewed_ability_check;

// Reuse the canonical checkpoint fixture and its actual interaction/model consumers.
mod canonical_fixture {
    include!("listener_safe_speech_contract.rs");

    mod novice_ruling_cases {
        use super::*;
        use crate::reviewed_ability_check::{
            AbilityCheckError, AbilityCheckInput, SOURCE_REVISION, resolve,
        };
        use df_model::commands::{CommandError, CommandLimits, validate_client_command};

        fn normal_check(die: u8, difficulty_class: u8) -> AbilityCheckInput {
            AbilityCheckInput {
                die,
                ability_modifier: 2,
                proficiency_bonus: 2,
                difficulty_class,
            }
        }

        fn resolution() -> ResolutionId {
            ResolutionId::from_bytes(&[40; 16]).unwrap()
        }

        fn window() -> WindowId {
            WindowId::from_bytes(&[41; 16]).unwrap()
        }

        fn pending_checkpoint() -> Checkpoint {
            let mut current = state();
            let causal = fact(10, 0);
            current.facts.push(causal.clone());
            current.resources.push(ResourceState {
                owner: entity(4),
                resource: label("synthetic-already-spent-resource"),
                value: 1,
                minimum: 0,
                maximum: 2,
                source: rule(),
            });
            current.pending.push(PendingResolution {
                id: resolution(),
                basis: basis(),
                continuation: label("synthetic-uncommitted-continuation"),
                window: ResolutionWindow {
                    id: window(),
                    phase: TriggerPhase::BeforeDraw,
                    causal_fact: causal.id,
                    source: rule(),
                    timer: None,
                },
                next: PendingInput::Ruling {
                    permitted: vec![OfferedResponse {
                        participant: member(3),
                        offer: label("synthetic-ruling-offer"),
                        // Opaque source options demonstrate structural admission only.
                        options: vec![label("synthetic-set-dc"), label("synthetic-decline")],
                        source: rule(),
                    }],
                    source: rule(),
                },
                choices: vec![],
                draw_ordinals: vec![],
                spent: vec![ResourceSpend {
                    owner: entity(4),
                    resource: label("synthetic-already-spent-resource"),
                    amount: 1,
                    source: rule(),
                }],
                rulings: vec![ScopedRuling {
                    adjudicator: member(3),
                    selected: label("synthetic-prior-ruling"),
                    source: rule(),
                    audience: AudienceScope::Shared,
                }],
            });
            let rules = [rule()];
            let content_entries = [content()];
            let resources = [ResourceConstraint {
                owner: entity(4),
                resource: label("synthetic-already-spent-resource"),
                minimum: 0,
                maximum: 2,
                source: rule(),
            }];
            Checkpoint::new(
                CHECKPOINT_SCHEMA,
                basis(),
                pins(),
                current,
                ReferenceInventory {
                    rules: &rules,
                    content: &content_entries,
                    resources: &resources,
                    assets: &[],
                },
                limits(),
            )
            .unwrap()
        }

        fn response(option: &str) -> HostInput {
            HostInput {
                basis: basis(),
                operation: OperationId::from_bytes(&[42; 16]).unwrap(),
                host: member(3),
                command: HostCommand::ResolveRuling {
                    resolution: resolution(),
                    window: window(),
                    offer: label("synthetic-ruling-offer"),
                    option: label(option),
                },
            }
        }

        fn admission_limits() -> CommandLimits {
            CommandLimits {
                maximum_records: 100,
                maximum_text_bytes: 256,
                maximum_retained_bytes: 1024 * 1024,
            }
        }

        fn admit(
            current: &Checkpoint,
            request: HostInput,
            bounds: CommandLimits,
        ) -> Result<(), CommandError> {
            validate_client_command(
                &GameInput::Host(request),
                current,
                ReferenceInventory {
                    rules: &[rule()],
                    content: &[content()],
                    resources: &[],
                    assets: &[],
                },
                bounds,
            )
        }

        #[test]
        fn represented_mechanics_compare_trusted_inputs_without_a_host_response() {
            assert_eq!(SOURCE_REVISION, "SRD-5.2.1:page-6:D20-Tests:Ability-Checks");
            let matched = resolve(normal_check(11, 15)).unwrap();
            let below = resolve(normal_check(10, 15)).unwrap();
            assert_eq!(matched.total, 15);
            assert!(matched.succeeded);
            assert_eq!(below.total, 14);
            assert!(!below.succeeded);
        }

        #[test]
        fn adjudicated_dc_is_an_explicit_input_and_has_no_fabricated_default() {
            let current = pending_checkpoint();
            let before = current.clone();
            assert!(resolve(normal_check(11, 15)).unwrap().succeeded);
            assert!(!resolve(normal_check(11, 16)).unwrap().succeeded);
            assert_eq!(
                resolve(normal_check(11, 0)),
                Err(AbilityCheckError::InvalidDifficultyClass)
            );
            assert_eq!(
                resolve(normal_check(0, 15)),
                Err(AbilityCheckError::InvalidDie)
            );
            assert_eq!(current, before);
        }

        #[test]
        fn offered_adjudicator_choice_and_decline_are_admitted_without_resolving_mechanics() {
            let current = pending_checkpoint();
            let before = current.clone();
            for option in ["synthetic-set-dc", "synthetic-decline"] {
                assert_eq!(
                    admit(&current, response(option), admission_limits()),
                    Ok(())
                );
                assert_eq!(current, before);
            }
        }

        #[test]
        fn another_participant_cannot_answer_the_designated_adjudicator_offer() {
            let current = pending_checkpoint();
            let before = current.clone();
            let mut request = response("synthetic-set-dc");
            request.host = member(5);
            assert_eq!(
                admit(&current, request, admission_limits()),
                Err(CommandError::UnofferedResponse)
            );
            assert_eq!(current, before);
            assert_eq!(current.state().pending[0].spent.len(), 1);
            assert_eq!(current.state().pending[0].rulings.len(), 1);
            assert!(current.state().draws.is_empty());
            assert!(current.state().decisions.is_empty());
            assert!(current.state().timers.is_empty());
        }

        #[test]
        fn stale_epoch_wrong_window_unoffered_option_and_zero_bounds_refuse() {
            let current = pending_checkpoint();
            let before = current.clone();
            let mut stale = response("synthetic-set-dc");
            stale.basis.revision = revision(1, 8);
            assert_eq!(
                admit(&current, stale, admission_limits()),
                Err(CommandError::StaleRevision)
            );
            let mut wrong_window = response("synthetic-set-dc");
            let HostCommand::ResolveRuling { window, .. } = &mut wrong_window.command else {
                unreachable!();
            };
            *window = WindowId::from_bytes(&[99; 16]).unwrap();
            assert_eq!(
                admit(&current, wrong_window, admission_limits()),
                Err(CommandError::StaleWindow)
            );
            let mut wrong_offer = response("synthetic-set-dc");
            let HostCommand::ResolveRuling { offer, .. } = &mut wrong_offer.command else {
                unreachable!();
            };
            *offer = label("unoffered-ruling");
            assert_eq!(
                admit(&current, wrong_offer, admission_limits()),
                Err(CommandError::UnofferedResponse)
            );
            assert_eq!(
                admit(&current, response("invented-rule"), admission_limits()),
                Err(CommandError::UnofferedResponse)
            );
            let mut zero = admission_limits();
            zero.maximum_records = 0;
            assert_eq!(
                admit(&current, response("synthetic-set-dc"), zero),
                Err(CommandError::Capacity)
            );
            assert_eq!(current, before);
        }

        #[test]
        fn canonical_rule_source_and_build_basis_are_revalidated_separately_from_admission() {
            let current = pending_checkpoint();
            let before = current.clone();
            let mut source_changed = pins();
            source_changed.rules.source_manifest = label("different-source-manifest");
            assert_eq!(
                current.validate_resume(basis(), &source_changed),
                Err(CheckpointError::RulesMismatch)
            );
            let mut build_changed = pins();
            build_changed.build = BuildIdentity::new(
                Some("different-source-build"),
                Some("fixture-native-1"),
                Some("fixture-wasm-1"),
                Some("fixture-config-1"),
                Some("fixture-content-1"),
            )
            .unwrap();
            assert_eq!(
                current.validate_resume(basis(), &build_changed),
                Err(CheckpointError::BuildMismatch)
            );
            assert_eq!(current, before);
        }

        #[test]
        fn host_ruling_input_cannot_be_reused_for_a_roll_pending_kind() {
            let mut current = pending_checkpoint().state().clone();
            current.pending[0].next = PendingInput::Roll {
                participant: member(3),
                sides: vec![20],
                source: rule(),
            };
            // Reuse the validated resource inventory from the canonical fixture.
            let resource = ResourceConstraint {
                owner: entity(4),
                resource: label("synthetic-already-spent-resource"),
                minimum: 0,
                maximum: 2,
                source: rule(),
            };
            let checkpoint = Checkpoint::new(
                CHECKPOINT_SCHEMA,
                basis(),
                pins(),
                current,
                ReferenceInventory {
                    rules: &[rule()],
                    content: &[content()],
                    resources: &[resource],
                    assets: &[],
                },
                limits(),
            )
            .unwrap();
            let before = checkpoint.clone();
            assert_eq!(
                admit(
                    &checkpoint,
                    response("synthetic-set-dc"),
                    admission_limits()
                ),
                Err(CommandError::WrongPendingKind)
            );
            assert_eq!(checkpoint, before);
        }
    }
}
