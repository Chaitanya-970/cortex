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

#[test]
fn test_cli_runs_list_and_show() {
    let tmp_dir = std::env::temp_dir().join(format!("cortex_cli_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    let db_path = tmp_dir.join("cortex.db");

    // Initialize db with a run
    let store = cortex_runtime::RunStore::open(&db_path).unwrap();
    let run_id = cortex_core::RunId::from("run_cli_test_01");
    store
        .record_run_start(&run_id, "Inspect CLI functionality", "2026-10-08T00:00:00Z")
        .unwrap();
    store
        .record_run_completion(
            &run_id,
            "completed",
            "2026-10-08T00:00:03Z",
            3000,
            100,
            20,
            0.0001,
            None,
        )
        .unwrap();
    let event = cortex_core::ExecutionEvent::RunStarted {
        run_id: run_id.clone(),
        task: "Inspect CLI functionality".to_string(),
        workspace_root: None,
    };
    store
        .record_event(&cortex_core::EventRecord::new(1, event))
        .unwrap();

    // Test cortex runs list
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_DB_PATH", &db_path)
        .args(["runs", "list"])
        .output()
        .expect("Failed to execute cortex runs list");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("run_cli_test_01"));
    assert!(stdout.contains("completed"));

    // Test cortex runs show
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_DB_PATH", &db_path)
        .args(["runs", "show", "run_cli_test_01", "--verbose"])
        .output()
        .expect("Failed to execute cortex runs show");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("run_cli_test_01"));
    assert!(stdout.contains("RunStarted"));

    // Record a model response for replay
    let replay_event = cortex_core::ExecutionEvent::ModelResponse {
        run_id: run_id.clone(),
        output_summary: "Final answer".to_string(),
        structured_output: Some(
            serde_json::to_value(cortex_runtime::ModelOutput::FinalAnswer(
                "replayed answer verify".to_string(),
            ))
            .unwrap(),
        ),
    };
    store
        .record_event(&cortex_core::EventRecord::new(2, replay_event))
        .unwrap();

    // Test cortex runs replay
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_DB_PATH", &db_path)
        .args(["runs", "replay", "run_cli_test_01"])
        .output()
        .expect("Failed to execute cortex runs replay");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Replaying run:   run_cli_test_01"));
    assert!(stdout.contains("replayed answer verify"));

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_cli_bench_run_command() {
    let tmp_dir =
        std::env::temp_dir().join(format!("cortex_bench_cli_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    let _ = std::fs::create_dir_all(&tmp_dir);
    let report_path = tmp_dir.join("report.md");

    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args([
            "bench",
            "run",
            "--suite",
            "coding",
            "--report",
            report_path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute cortex bench run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Benchmark Report: coding"));
    assert!(stdout.contains("**Success Rate**: 100.0% (5/5)"));
    assert!(report_path.exists());

    let _ = std::fs::remove_dir_all(&tmp_dir);
}
