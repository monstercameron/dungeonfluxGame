#[path = "../src/qualification_verdict.rs"]
mod qualification_verdict;

use df_rpc_bridge::{
    BufferSnapshot, CONCURRENT_STREAMS, ConnectionMetrics, ConnectionSnapshot, FRAME_BYTES,
    MESSAGE_BYTES, RECEIVE_BYTES, RPC_MESSAGE_BYTES,
};
use qualification_verdict::{
    CALLBACK_BYTES_LIMIT, CALLBACK_ITEMS_LIMIT, CREDIT_REPORT_DEADLINE_MS, OWNED_RUN_DEADLINE_MS,
    PAYLOAD_ENVELOPE_BYTES, Verdict, evaluate,
};

fn base_snapshot() -> ConnectionSnapshot {
    ConnectionMetrics::new(true)
        .snapshot()
        .expect("fresh fixture snapshot")
}

fn render_case(name: &str, verdict: Verdict) -> String {
    let display = qualification_verdict::format_verdict(verdict).replace('\n', "; ");
    format!(
        "    {{\"case\":\"{name}\",\"observed_subcase\":\"{}\",\"full_g02\":\"{}\",\"stop_owned_run\":{},\"block_dependents\":{},\"display\":\"{display}\"}}",
        verdict.observed_subcase.as_str(),
        verdict.full_g02.as_str(),
        verdict.stop_owned_run,
        verdict.block_dependents,
    )
}

fn emitted_cases() -> String {
    let mut below = base_snapshot();
    below.receive_reservation_bytes = PAYLOAD_ENVELOPE_BYTES - 1;
    below.callback_bytes = BufferSnapshot {
        current: 0,
        peak: CALLBACK_BYTES_LIMIT - 1,
        total: 0,
    };
    below.callback_items = BufferSnapshot {
        current: 0,
        peak: CALLBACK_ITEMS_LIMIT - 1,
        total: 0,
    };

    let mut at = base_snapshot();
    at.receive_reservation_bytes = PAYLOAD_ENVELOPE_BYTES;
    at.callback_bytes = BufferSnapshot {
        current: CALLBACK_BYTES_LIMIT,
        peak: CALLBACK_BYTES_LIMIT,
        total: CALLBACK_BYTES_LIMIT,
    };
    at.callback_items = BufferSnapshot {
        current: CALLBACK_ITEMS_LIMIT,
        peak: CALLBACK_ITEMS_LIMIT,
        total: CALLBACK_ITEMS_LIMIT,
    };

    let mut bytes_breach = base_snapshot();
    bytes_breach.callback_bytes.peak = CALLBACK_BYTES_LIMIT + 1;
    let mut items_breach = base_snapshot();
    items_breach.callback_items.peak = CALLBACK_ITEMS_LIMIT + 1;
    let mut reservation_breach = base_snapshot();
    reservation_breach.receive_reservation_bytes = PAYLOAD_ENVELOPE_BYTES + 1;
    let mut driver_failure = base_snapshot();
    driver_failure.driver_failed = true;

    [
        render_case("below_existing_bounds", evaluate(Some(&below), true, false)),
        render_case("at_existing_bounds", evaluate(Some(&at), true, false)),
        render_case(
            "callback_bytes_breach_precedes_missing_full_evidence",
            evaluate(Some(&bytes_breach), true, false),
        ),
        render_case(
            "callback_items_breach_stops_owned_run",
            evaluate(Some(&items_breach), true, false),
        ),
        render_case(
            "planned_payload_reservation_breach_stops_owned_run",
            evaluate(Some(&reservation_breach), true, false),
        ),
        render_case(
            "owned_driver_failure_stops_run",
            evaluate(Some(&driver_failure), true, false),
        ),
        render_case(
            "missing_measurement_is_inconclusive",
            evaluate(None, false, false),
        ),
        render_case(
            "local_pass_does_not_pass_full_g02",
            evaluate(Some(&at), true, false),
        ),
        render_case(
            "failed_scenario_stops_owned_run",
            evaluate(None, false, true),
        ),
    ]
    .join(",\n")
}

fn render_terminal_case(wrapper: &str, trigger: &str) -> String {
    use qualification_verdict::{TerminalContext, TerminalOutcome, TerminalPresentation};

    let (outcome, expected_presentation) = match trigger {
        "pressure_failure" => (
            TerminalOutcome::retain_current_report(evaluate(None, false, true)),
            "keep_current",
        ),
        "direct_resource_breach" => {
            let mut breached = base_snapshot();
            breached.callback_bytes.peak = CALLBACK_BYTES_LIMIT + 1;
            (
                TerminalOutcome::from_verdict(
                    evaluate(Some(&breached), false, false),
                    "Resource bound exceeded; owned browser connection closed immediately.",
                ),
                "replace",
            )
        }
        "timeout" => (
            TerminalOutcome::inconclusive("owned observation deadline reached"),
            "replace",
        ),
        _ => unreachable!("contract supplies known terminal triggers"),
    };
    let verdict = outcome.verdict();
    let existing_pressure_report = format!(
        "{}\nFAIL · unchanged simultaneous pressure workload",
        qualification_verdict::format_verdict(verdict)
    );
    let context = match wrapper {
        "qualification" => TerminalContext::Qualification,
        "callback_capacity" => TerminalContext::CallbackCapacity,
        "connection_credit" => TerminalContext::ConnectionCredit,
        _ => unreachable!("contract supplies known terminal wrappers"),
    };
    let presentation =
        qualification_verdict::present_terminal_outcome(&outcome, "contract-build", context);
    let (actual_presentation, first_line) = match presentation {
        TerminalPresentation::KeepCurrent => (
            "keep_current",
            existing_pressure_report
                .lines()
                .next()
                .unwrap_or_default()
                .to_owned(),
        ),
        TerminalPresentation::Replace(report) => (
            "replace",
            report.lines().next().unwrap_or_default().to_owned(),
        ),
    };
    assert_eq!(actual_presentation, expected_presentation);
    match trigger {
        "pressure_failure" | "direct_resource_breach" => {
            assert_eq!(verdict.observed_subcase.as_str(), "FAIL");
            assert_eq!(verdict.full_g02.as_str(), "FAIL");
            assert!(verdict.stop_owned_run);
        }
        "timeout" => {
            assert_eq!(verdict.observed_subcase.as_str(), "INCONCLUSIVE");
            assert_eq!(verdict.full_g02.as_str(), "INCONCLUSIVE");
            assert!(!verdict.stop_owned_run);
        }
        _ => unreachable!("contract supplies known terminal triggers"),
    }
    let detail = outcome.detail().replace('"', "'");
    format!(
        "    {{\"wrapper\":\"{wrapper}\",\"trigger\":\"{trigger}\",\"presentation\":\"{actual_presentation}\",\"observed_subcase\":\"{}\",\"full_g02\":\"{}\",\"stop_owned_run\":{},\"block_dependents\":{},\"first_line\":\"{first_line}\",\"detail\":\"{detail}\"}}",
        verdict.observed_subcase.as_str(),
        verdict.full_g02.as_str(),
        verdict.stop_owned_run,
        verdict.block_dependents,
    )
}

fn emitted_terminal_cases() -> String {
    ["qualification", "callback_capacity", "connection_credit"]
        .into_iter()
        .flat_map(|wrapper| {
            ["pressure_failure", "direct_resource_breach", "timeout"]
                .into_iter()
                .map(move |trigger| render_terminal_case(wrapper, trigger))
        })
        .collect::<Vec<_>>()
        .join(",\n")
}

#[test]
fn emitted_verdict_cases_match_reviewed_json_contract() {
    let actual = format!(
        "{{\n  \"task\":\"B-G02-D04\",\n  \"decision\":\"An observed numeric bound breach fails and stops owned work; missing measurements remain inconclusive. A bounded local pass never proves full physical G02.\",\n  \"governing\":[\"planning/rpc-transport.md: Browser resource proof and feasibility result\",\"development/qualify-g02.md: What the experiment measures and bounds; Required checks and evidence\",\"crates/df-rpc-bridge/tests/flow_credit_contract.json: bounds and browser_fixture\"],\n  \"limits\":{{\"http2_frame_bytes\":{},\"tunnel_message_bytes\":{},\"connection_receive_window_bytes\":{},\"encoded_rpc_message_bytes\":{},\"concurrent_streams\":{},\"callback_bytes\":{},\"callback_items\":{},\"planned_payload_envelope_bytes\":{},\"owned_run_deadline_ms\":{},\"credit_report_deadline_ms\":{}}},\n  \"alternatives\":[\"Treat the planned payload reservation as proof of total connection allocation: rejected; it excludes allocator, h2, browser engine and pre-callback storage.\",\"Treat a small-subcase PASS as global G02 PASS: rejected; physical devices, whole-process CPU/memory, audio and action-to-view evidence remain absent.\",\"Invent latency thresholds from local p95/p99 samples: rejected; these are observations without approved production budgets.\"],\n  \"unperformed\":[\"physical iOS and Android devices\",\"supported desktop and network matrix\",\"whole-process CPU and memory\",\"browser engine and pre-callback allocations\",\"audio playback and action-to-view evidence\"],\n  \"cases\":[\n{}\n  ],\n  \"terminal_cases\":[\n{}\n  ]\n}}\n",
        FRAME_BYTES,
        MESSAGE_BYTES,
        RECEIVE_BYTES,
        RPC_MESSAGE_BYTES,
        CONCURRENT_STREAMS,
        CALLBACK_BYTES_LIMIT,
        CALLBACK_ITEMS_LIMIT,
        PAYLOAD_ENVELOPE_BYTES,
        OWNED_RUN_DEADLINE_MS,
        CREDIT_REPORT_DEADLINE_MS,
        emitted_cases(),
        emitted_terminal_cases(),
    );
    println!("{actual}");
    assert_eq!(
        actual,
        include_str!("fixtures/qualification_verdict_contract.json")
    );
}
