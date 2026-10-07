//! Integration tests for the Cortex CLI binary.

use std::process::Command;

#[test]
fn test_cli_version_flag() {
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .arg("--version")
        .output()
        .expect("Failed to execute cortex binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cortex"));
}

#[test]
fn test_cli_status_subcommand() {
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .arg("status")
        .output()
        .expect("Failed to execute cortex binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Workspace & Architecture Bootstrap"));
}

#[test]
fn test_cli_check_subcommand() {
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .arg("check")
        .output()
        .expect("Failed to execute cortex binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("environment check"));
}
