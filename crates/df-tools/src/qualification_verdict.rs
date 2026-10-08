use df_rpc_bridge::ConnectionSnapshot;

pub(super) const PAYLOAD_ENVELOPE_BYTES: usize = 8 * 1024 * 1024;
pub(super) const CALLBACK_BYTES_LIMIT: usize = 1024 * 1024;
pub(super) const CALLBACK_ITEMS_LIMIT: usize = 256;
pub(super) const OWNED_RUN_DEADLINE_MS: u32 = 30_000;
pub(super) const CREDIT_REPORT_DEADLINE_MS: u32 = 29_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Outcome {
    Pass,
    Fail,
    Inconclusive,
}

impl Outcome {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Inconclusive => "INCONCLUSIVE",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Verdict {
    pub(crate) observed_subcase: Outcome,
    pub(crate) full_g02: Outcome,
    pub(crate) stop_owned_run: bool,
    pub(crate) block_dependents: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalDisposition {
    ReplaceCurrentReport,
    KeepCurrentReport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalOutcome {
    verdict: Verdict,
    detail: String,
    disposition: TerminalDisposition,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum TerminalPresentation {
    Replace(String),
    KeepCurrent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalContext {
    Qualification,
    CallbackCapacity,
    ConnectionCredit,
}

impl TerminalContext {
    fn description(self) -> &'static str {
        match self {
            Self::Qualification => {
                "G02 qualification remains incomplete until all required evidence is available."
            }
            Self::CallbackCapacity => "Callback capacity and G02 qualification remain unverified.",
            Self::ConnectionCredit => {
                "Connection-credit attribution remains unverified. Hostile over-credit bursts, full G02/D03, total 8 MiB, pre-callback allocation, 128-frame/256 KiB control queue and fairness, physical phones, audio, gameplay and production remain pending. The run owner and its streams were dropped."
            }
        }
    }
}

impl TerminalOutcome {
    pub(crate) fn from_verdict(verdict: Verdict, detail: impl Into<String>) -> Self {
        Self {
            verdict,
            detail: detail.into(),
            disposition: TerminalDisposition::ReplaceCurrentReport,
        }
    }

    pub(crate) fn inconclusive(detail: impl Into<String>) -> Self {
        Self::from_verdict(evaluate(None, false, false), detail)
    }

    pub(crate) fn retain_current_report(verdict: Verdict) -> Self {
        Self {
            verdict,
            detail: String::new(),
            disposition: TerminalDisposition::KeepCurrentReport,
        }
    }

    pub(crate) fn verdict(&self) -> Verdict {
        self.verdict
    }

    pub(crate) fn detail(&self) -> &str {
        &self.detail
    }
}

impl From<String> for TerminalOutcome {
    fn from(detail: String) -> Self {
        Self::from_verdict(
            Verdict {
                observed_subcase: Outcome::Fail,
                full_g02: Outcome::Inconclusive,
                stop_owned_run: false,
                block_dependents: true,
            },
            detail,
        )
    }
}

impl From<&str> for TerminalOutcome {
    fn from(detail: &str) -> Self {
        Self::from(detail.to_owned())
    }
}

pub(crate) fn present_terminal_outcome(
    outcome: &TerminalOutcome,
    build: &str,
    context: TerminalContext,
) -> TerminalPresentation {
    match outcome.disposition {
        TerminalDisposition::KeepCurrentReport => TerminalPresentation::KeepCurrent,
        TerminalDisposition::ReplaceCurrentReport => TerminalPresentation::Replace(format!(
            "{}\n{}\nBuild: {build}\n{}\n",
            format_verdict(outcome.verdict),
            outcome.detail,
            context.description(),
        )),
    }
}

pub(crate) fn evaluate(
    snapshot: Option<&ConnectionSnapshot>,
    subcase_observed: bool,
    scenario_failed: bool,
) -> Verdict {
    let actual_breach = scenario_failed
        || snapshot.is_some_and(|value| {
            value.receive_reservation_bytes > PAYLOAD_ENVELOPE_BYTES
                || value.callback_bytes.peak > CALLBACK_BYTES_LIMIT
                || value.callback_items.peak > CALLBACK_ITEMS_LIMIT
                || value.driver_failed
        });
    let missing_measurement = snapshot.is_none();

    let observed_subcase = if actual_breach {
        Outcome::Fail
    } else if missing_measurement || !subcase_observed {
        Outcome::Inconclusive
    } else {
        Outcome::Pass
    };
    let full_g02 = if actual_breach {
        Outcome::Fail
    } else {
        Outcome::Inconclusive
    };

    Verdict {
        observed_subcase,
        full_g02,
        stop_owned_run: actual_breach,
        block_dependents: full_g02 != Outcome::Pass,
    }
}

pub(crate) fn format_verdict(verdict: Verdict) -> String {
    format!(
        "Observed bounded subcase: {}\nFull G02: {}\nOwned resource stop: {}\nDependent qualification blocked: {}",
        verdict.observed_subcase.as_str(),
        verdict.full_g02.as_str(),
        verdict.stop_owned_run,
        verdict.block_dependents,
    )
}
