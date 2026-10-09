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
    assert!(stdout.contains("Settings:"));
}

#[test]
fn test_cli_settings_auto_creation_and_config() {
    let tmp_home = std::env::temp_dir().join(format!("cortex_home_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_home);

    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_HOME", &tmp_home)
        .arg("status")
        .output()
        .expect("Failed to execute cortex binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Settings:"));

    // Verify settings.json was automatically created in the home folder
    let settings_file = tmp_home.join("settings.json");
    assert!(settings_file.is_file());

    let content = std::fs::read_to_string(&settings_file).unwrap();
    assert!(content.contains("\"model\": \"gpt-4o-mini\""));

    // Configure a custom model, url, and api key in settings.json
    let custom_settings = r#"{
        "model": "my-custom-model",
        "url": "http://localhost:8000/v1",
        "api_key": "sk-custom-123"
    }"#;
    std::fs::write(&settings_file, custom_settings).unwrap();

    let output2 = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_HOME", &tmp_home)
        .arg("status")
        .output()
        .expect("Failed to execute cortex binary");

    assert!(output2.status.success());
    let stdout2 = String::from_utf8_lossy(&output2.stdout);
    assert!(stdout2.contains("my-custom-model"));
    assert!(stdout2.contains("http://localhost:8000/v1"));
    assert!(stdout2.contains("API Key: Configured"));

    let _ = std::fs::remove_dir_all(&tmp_home);
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
#[cfg(not(windows))]
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

#[test]
fn test_cli_tui_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args(["tui", "--help"])
        .output()
        .expect("Failed to execute cortex tui --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Launch the interactive terminal control plane"));
    assert!(stdout.contains("--db"));
}

#[test]
fn test_cli_run_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args(["run", "--help"])
        .output()
        .expect("Failed to execute cortex run --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--model"));
    assert!(stdout.contains("--api-key"));
    assert!(stdout.contains("--base-url"));
    assert!(stdout.contains("--workspace"));
    assert!(stdout.contains("--max-iterations"));
    assert!(stdout.contains("--quiet"));
    assert!(stdout.contains("--json"));
}

#[test]
fn test_cli_run_unconfigured_error_guidance() {
    let tmp_dir =
        std::env::temp_dir().join(format!("cortex_unconfigured_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_HOME", &tmp_dir)
        .env_remove("OPENAI_API_KEY")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("CORTEX_API_KEY")
        .args(["run", "Inspect codebase", "--model", "gpt-4o"])
        .output()
        .expect("Failed to execute cortex run");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not configured"));
    assert!(stderr.contains("OPENAI_API_KEY"));

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_cli_cron_lifecycle() {
    let tmp_dir = std::env::temp_dir().join(format!("cortex_cli_cron_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    let _ = std::fs::create_dir_all(&tmp_dir);
    let db_path = tmp_dir.join("cortex.db");

    // 1. Initially empty
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args(["cron", "list", "--db", db_path.to_str().unwrap()])
        .output()
        .expect("Failed to execute cortex cron list");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No scheduled cron jobs found"));

    // 2. Create job
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args([
            "cron",
            "create",
            "--name",
            "Nightly Backup",
            "--schedule",
            "0 2 * * *",
            "--prompt",
            "Backup workspace databases",
            "--overlap",
            "queue",
            "--db",
            db_path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute cortex cron create");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Created cron job"));
    assert!(stdout.contains("Nightly Backup"));
    assert!(stdout.contains("0 2 * * *"));

    // Extract job id
    let job_id_start = stdout.find('\'').unwrap() + 1;
    let job_id_end = stdout[job_id_start..].find('\'').unwrap() + job_id_start;
    let job_id = &stdout[job_id_start..job_id_end];

    // 3. List formatted
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args(["cron", "list", "--db", db_path.to_str().unwrap()])
        .output()
        .expect("Failed to execute cortex cron list");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(job_id));
    assert!(stdout.contains("Nightly Backup"));
    assert!(stdout.contains("0 2 * * *"));
    assert!(stdout.contains("queue"));

    // 4. List json
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args(["cron", "list", "--json", "--db", db_path.to_str().unwrap()])
        .output()
        .expect("Failed to execute cortex cron list --json");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"name\": \"Nightly Backup\""));

    // 5. Create invalid cron
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args([
            "cron",
            "create",
            "--schedule",
            "invalid cron expr",
            "--prompt",
            "Fails",
            "--db",
            db_path.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to execute cortex cron create invalid");

    assert!(!output.status.success());

    // 6. Delete job
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args(["cron", "delete", job_id, "--db", db_path.to_str().unwrap()])
        .output()
        .expect("Failed to execute cortex cron delete");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&format!("Deleted cron job '{}'", job_id)));

    // 7. Verify empty again
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args(["cron", "list", "--db", db_path.to_str().unwrap()])
        .output()
        .expect("Failed to execute cortex cron list");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("No scheduled cron jobs found"));

    let _ = std::fs::remove_dir_all(&tmp_dir);
}
