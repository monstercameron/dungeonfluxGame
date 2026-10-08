#[path = "../src/output_evidence.rs"]
mod output_evidence;

use df_types::{BuildIdentity, RevisionLabel};
use output_evidence::*;

fn label(value: &str) -> RevisionLabel {
    RevisionLabel::new(Some(value)).unwrap()
}

struct Fixture {
    labels: [RevisionLabel; 11],
    build: BuildIdentity,
}

impl Fixture {
    fn new() -> Self {
        Self {
            labels: [
                "task",
                "a1",
                "assets",
                "worker",
                "evaluator",
                "model",
                "criterion",
                "output",
                "digest",
                "handle",
                "other",
            ]
            .map(label),
            build: BuildIdentity::new(
                Some("source"),
                Some("native"),
                Some("wasm"),
                Some("configuration"),
                Some("content"),
            )
            .unwrap(),
        }
    }

    fn scope(&self) -> EvidenceScope<'_> {
        EvidenceScope {
            task: &self.labels[0],
            attempt: &self.labels[1],
            assets: &self.labels[2],
            build: &self.build,
        }
    }
}

struct Case<'a> {
    context: ReviewContext<'a>,
    required: [RequiredCriterion<'a>; 1],
    criteria: [CriterionEvidence<'a>; 1],
    outputs: [RetainedOutput<'a>; 1],
    handles: [OwnedHandle<'a>; 1],
    limits: EvidenceLimits,
}

impl<'a> Case<'a> {
    fn new(fixture: &'a Fixture) -> Self {
        let scope = fixture.scope();
        let evaluator = &fixture.labels[4];
        let output = OutputReference {
            id: &fixture.labels[7],
            revision: &fixture.labels[8],
            bytes: 42,
        };
        Self {
            context: ReviewContext {
                scope,
                worker: &fixture.labels[3],
                evaluator,
                model: &fixture.labels[5],
                computer_use: true,
                vision: true,
                audio: true,
            },
            required: [RequiredCriterion {
                id: &fixture.labels[6],
                observation: ObservationKind::NativeBoundary,
            }],
            criteria: [CriterionEvidence {
                criterion: &fixture.labels[6],
                procedure: "Run current boundary",
                input: "bounded input",
                expected: "independent current result",
                state: CriterionState::Observed {
                    result: ObservedResult::Pass,
                    observed: "actual output",
                    output,
                },
            }],
            outputs: [RetainedOutput {
                scope,
                observer: evaluator,
                observation: ObservationKind::NativeBoundary,
                output,
            }],
            handles: [OwnedHandle {
                scope,
                id: &fixture.labels[9],
                owner: evaluator,
                terminal: true,
            }],
            limits: EvidenceLimits {
                maximum_records: 8,
                maximum_text_bytes: 1024,
            },
        }
    }

    fn report(&self) -> EvidenceReport<'_> {
        EvidenceReport {
            scope: self.context.scope,
            observer: self.context.evaluator,
            model: self.context.model,
            criteria: &self.criteria,
        }
    }

    fn inspect(&self) -> Result<EvidenceSummary, EvidenceError> {
        inspect_evidence(
            self.context,
            &self.required,
            &self.report(),
            &self.outputs,
            &self.handles,
            self.limits,
        )
    }
}

#[test]
fn actual_independent_pass_and_failure_are_counts_without_an_approval_verdict() {
    let fixture = Fixture::new();
    let mut case = Case::new(&fixture);
    assert_eq!(
        case.inspect(),
        Ok(EvidenceSummary {
            passed: 1,
            ..EvidenceSummary::default()
        })
    );
    assert!(!case.inspect().unwrap().has_unobserved_criteria());
    case.criteria[0].state = CriterionState::Observed {
        result: ObservedResult::Fail,
        observed: "actual failed result",
        output: case.outputs[0].output,
    };
    assert_eq!(
        case.inspect(),
        Ok(EvidenceSummary {
            failed: 1,
            ..EvidenceSummary::default()
        })
    );
}

#[test]
fn pending_unsupported_unperformed_and_worker_claims_remain_unobserved() {
    let fixture = Fixture::new();
    let mut case = Case::new(&fixture);
    let states = [
        CriterionState::Pending {
            reason: "pending check",
        },
        CriterionState::Unsupported {
            reason: "unsupported boundary",
        },
        CriterionState::Inconclusive {
            reason: "ambiguous observation",
        },
        CriterionState::Unperformed {
            reason: "capability unavailable",
        },
        CriterionState::WorkerSupportingClaim {
            reference: &fixture.labels[7],
        },
    ];
    for state in states {
        case.criteria[0].state = state;
        let summary = inspect_evidence(
            case.context,
            &case.required,
            &case.report(),
            &[],
            &[],
            case.limits,
        )
        .unwrap();
        assert!(summary.has_unobserved_criteria());
        assert_eq!(summary.passed + summary.failed, 0);
        assert_eq!(summary.unobserved, 1);
    }
}

#[test]
fn worker_self_review_wrong_observer_and_wrong_model_are_refused() {
    let fixture = Fixture::new();
    let mut case = Case::new(&fixture);
    case.context.worker = case.context.evaluator;
    assert_eq!(case.inspect(), Err(EvidenceError::SelfReview));
    case.context.worker = &fixture.labels[3];
    for report in [
        EvidenceReport {
            observer: case.context.worker,
            ..case.report()
        },
        EvidenceReport {
            model: &fixture.labels[10],
            ..case.report()
        },
    ] {
        assert_eq!(
            inspect_evidence(
                case.context,
                &case.required,
                &report,
                &case.outputs,
                &case.handles,
                case.limits
            ),
            Err(EvidenceError::IdentityMismatch)
        );
    }
}

#[test]
fn current_task_attempt_assets_and_every_build_component_are_bound() {
    let fixture = Fixture::new();
    let case = Case::new(&fixture);
    for scope in [
        EvidenceScope {
            task: &fixture.labels[10],
            ..fixture.scope()
        },
        EvidenceScope {
            attempt: &fixture.labels[10],
            ..fixture.scope()
        },
        EvidenceScope {
            assets: &fixture.labels[10],
            ..fixture.scope()
        },
    ] {
        let report = EvidenceReport {
            scope,
            ..case.report()
        };
        assert_eq!(
            inspect_evidence(
                case.context,
                &case.required,
                &report,
                &case.outputs,
                &case.handles,
                case.limits
            ),
            Err(EvidenceError::IdentityMismatch)
        );
    }
    for changed in 0..5 {
        let mut labels = ["source", "native", "wasm", "configuration", "content"];
        labels[changed] = "other";
        let build = BuildIdentity::new(
            Some(labels[0]),
            Some(labels[1]),
            Some(labels[2]),
            Some(labels[3]),
            Some(labels[4]),
        )
        .unwrap();
        let report = EvidenceReport {
            scope: EvidenceScope {
                build: &build,
                ..fixture.scope()
            },
            ..case.report()
        };
        assert_eq!(
            inspect_evidence(
                case.context,
                &case.required,
                &report,
                &case.outputs,
                &case.handles,
                case.limits
            ),
            Err(EvidenceError::IdentityMismatch)
        );
    }
}

#[test]
fn required_criterion_map_cannot_omit_duplicate_or_substitute_a_criterion() {
    let fixture = Fixture::new();
    let mut case = Case::new(&fixture);
    let missing = EvidenceReport {
        criteria: &[],
        ..case.report()
    };
    assert_eq!(
        inspect_evidence(
            case.context,
            &case.required,
            &missing,
            &case.outputs,
            &case.handles,
            case.limits
        ),
        Err(EvidenceError::InvalidCriterionMap)
    );
    let required = [case.required[0], case.required[0]];
    let criteria = [case.criteria[0], case.criteria[0]];
    let duplicate = EvidenceReport {
        criteria: &criteria,
        ..case.report()
    };
    assert_eq!(
        inspect_evidence(
            case.context,
            &required,
            &duplicate,
            &case.outputs,
            &case.handles,
            case.limits
        ),
        Err(EvidenceError::InvalidCriterionMap)
    );
    case.criteria[0].criterion = &fixture.labels[10];
    assert_eq!(case.inspect(), Err(EvidenceError::InvalidCriterionMap));
}

#[test]
fn missing_stale_zero_byte_and_ambiguous_output_custody_are_refused() {
    let fixture = Fixture::new();
    let mut case = Case::new(&fixture);
    assert_eq!(
        inspect_evidence(
            case.context,
            &case.required,
            &case.report(),
            &[],
            &case.handles,
            case.limits
        ),
        Err(EvidenceError::MissingOutput)
    );
    let duplicate = [case.outputs[0], case.outputs[0]];
    assert_eq!(
        inspect_evidence(
            case.context,
            &case.required,
            &case.report(),
            &duplicate,
            &case.handles,
            case.limits
        ),
        Err(EvidenceError::StaleOutput)
    );
    let original = case.outputs[0];
    for changed in 0..6 {
        case.outputs[0] = original;
        match changed {
            0 => case.outputs[0].output.revision = &fixture.labels[10],
            1 => case.outputs[0].output.bytes = 0,
            2 => case.outputs[0].observation = ObservationKind::BrowserInteractionAndVision,
            3 => case.outputs[0].scope.attempt = &fixture.labels[10],
            4 => case.outputs[0].observer = case.context.worker,
            _ => case.outputs[0].output.bytes += 1,
        }
        assert_eq!(case.inspect(), Err(EvidenceError::StaleOutput));
    }
}

#[test]
fn browser_needs_actual_computer_use_and_vision_and_audio_needs_audible_observation() {
    let fixture = Fixture::new();
    let mut case = Case::new(&fixture);
    case.required[0].observation = ObservationKind::BrowserInteractionAndVision;
    case.outputs[0].observation = ObservationKind::BrowserInteractionAndVision;
    case.context.computer_use = false;
    assert_eq!(case.inspect(), Err(EvidenceError::MissingCapability));
    case.context.computer_use = true;
    case.context.vision = false;
    assert_eq!(case.inspect(), Err(EvidenceError::MissingCapability));
    case.context.vision = true;
    assert_eq!(case.inspect().unwrap().passed, 1);
    case.required[0].observation = ObservationKind::AudioPlayback;
    case.outputs[0].observation = ObservationKind::AudioPlayback;
    case.context.audio = false;
    assert_eq!(case.inspect(), Err(EvidenceError::MissingCapability));
    case.context.audio = true;
    assert_eq!(case.inspect().unwrap().passed, 1);
}

#[test]
fn native_output_needs_no_invented_ui_gate_and_zero_owned_handles_is_explicit() {
    let fixture = Fixture::new();
    let mut case = Case::new(&fixture);
    case.context.computer_use = false;
    case.context.vision = false;
    case.context.audio = false;
    assert_eq!(
        inspect_evidence(
            case.context,
            &case.required,
            &case.report(),
            &case.outputs,
            &[],
            case.limits
        )
        .unwrap()
        .passed,
        1
    );
}

#[test]
fn foreign_stale_unterminated_and_duplicate_owned_handles_are_refused() {
    let fixture = Fixture::new();
    let mut case = Case::new(&fixture);
    case.handles[0].owner = case.context.worker;
    assert_eq!(case.inspect(), Err(EvidenceError::ForeignHandle));
    case.handles[0].owner = case.context.evaluator;
    case.handles[0].scope.attempt = &fixture.labels[10];
    assert_eq!(case.inspect(), Err(EvidenceError::ForeignHandle));
    case.handles[0].scope = fixture.scope();
    case.handles[0].terminal = false;
    assert_eq!(case.inspect(), Err(EvidenceError::UnterminatedHandle));
    case.handles[0].terminal = true;
    let duplicate = [case.handles[0], case.handles[0]];
    assert_eq!(
        inspect_evidence(
            case.context,
            &case.required,
            &case.report(),
            &case.outputs,
            &duplicate,
            case.limits
        ),
        Err(EvidenceError::DuplicateHandle)
    );
}

#[test]
fn positive_aggregate_record_and_text_bounds_are_required() {
    let fixture = Fixture::new();
    let mut case = Case::new(&fixture);
    case.limits.maximum_records = 0;
    assert_eq!(case.inspect(), Err(EvidenceError::Capacity));
    case.limits.maximum_records = 3;
    assert_eq!(case.inspect(), Err(EvidenceError::Capacity));
    case.limits.maximum_records = 8;
    case.limits.maximum_text_bytes = 0;
    assert_eq!(case.inspect(), Err(EvidenceError::Capacity));
    case.limits.maximum_text_bytes = 1;
    assert_eq!(case.inspect(), Err(EvidenceError::Capacity));
}

#[test]
fn procedure_input_expectation_observation_and_gap_reason_are_required() {
    let fixture = Fixture::new();
    for missing in 0..5 {
        let mut case = Case::new(&fixture);
        match missing {
            0 => case.criteria[0].procedure = "",
            1 => case.criteria[0].input = " ",
            2 => case.criteria[0].expected = "",
            3 => {
                case.criteria[0].state = CriterionState::Observed {
                    result: ObservedResult::Pass,
                    observed: "",
                    output: case.outputs[0].output,
                }
            }
            _ => case.criteria[0].state = CriterionState::Unperformed { reason: " " },
        }
        assert_eq!(case.inspect(), Err(EvidenceError::MissingDescription));
    }
}
