//! Execute the real composition root: startup refusals must never announce readiness.
#![cfg(not(target_arch = "wasm32"))]
use std::{net::TcpListener, process::Command};

fn fixture() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_df-transport-fixture"));
    command.args([env!("CARGO_MANIFEST_DIR"), "43195"]);
    for variable in [
        "DF_PREVIEW_STATE_ROOT",
        "DF_PREVIEW_ATTEMPT_ID",
        "DF_PREVIEW_WEB_SOURCE_ID",
    ] {
        command.env_remove(variable);
    }
    command
}

#[test]
fn actual_fixture_rejects_partial_and_invalid_managed_configuration() {
    for (name, value) in [
        ("DF_PREVIEW_STATE_ROOT", "/foreign"),
        ("DF_PREVIEW_ATTEMPT_ID", "valid"),
        ("DF_PREVIEW_WEB_SOURCE_ID", "valid"),
    ] {
        let output = fixture().env(name, value).output().unwrap();
        assert!(!output.status.success());
        assert!(!String::from_utf8_lossy(&output.stdout).contains("fixture ready"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("IncompleteConfiguration"));
    }
    let output = fixture()
        .env("DF_PREVIEW_STATE_ROOT", "/foreign")
        .env("DF_PREVIEW_ATTEMPT_ID", "bad label")
        .env("DF_PREVIEW_WEB_SOURCE_ID", "valid")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("fixture ready"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("InvalidLabel"));
}

#[test]
fn actual_fixture_bind_failure_never_announces_ready_or_registers() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let mut command = fixture();
    // A fresh command selects the occupied socket's actual nonzero port.
    command = Command::new(command.get_program());
    command.args([env!("CARGO_MANIFEST_DIR"), &port]);
    for variable in [
        "DF_PREVIEW_STATE_ROOT",
        "DF_PREVIEW_ATTEMPT_ID",
        "DF_PREVIEW_WEB_SOURCE_ID",
    ] {
        command.env_remove(variable);
    }
    let output = command.output().unwrap();
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("fixture ready"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("AddrInUse"));
}
