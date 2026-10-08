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
