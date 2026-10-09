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
fn test_cli_agent_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .args(["agent", "--help"])
        .output()
        .expect("Failed to execute cortex agent --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("list"));
    assert!(stdout.contains("create"));
    assert!(stdout.contains("start"));
    assert!(stdout.contains("stop"));
    assert!(stdout.contains("pause"));
    assert!(stdout.contains("inspect"));
}

#[test]
fn test_cli_agent_lifecycle_workflow() {
    let tmp_dir =
        std::env::temp_dir().join(format!("cortex_agent_cli_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();

    let agents_file = tmp_dir.join("agents.json");
    let manifest_file = tmp_dir.join("reviewer.yaml");

    let manifest_content = r#"
name: "reviewer"
role: "Code Reviewer"
workspace: "./src"
policy: "Review pull requests and ensure quality"
model:
  provider: "anthropic"
  model: "claude-3-5-sonnet"
tools:
  - read_file
  - git_diff
"#;
    std::fs::write(&manifest_file, manifest_content).unwrap();

    // 1. Initially empty list
    let list_empty = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_AGENTS_PATH", &agents_file)
        .args(["agent", "list"])
        .output()
        .expect("Failed to run cortex agent list");
    assert!(list_empty.status.success());
    let empty_stdout = String::from_utf8_lossy(&list_empty.stdout);
    assert!(empty_stdout.contains("No registered agents found"));

    // 2. Create agent from manifest
    let create_out = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_AGENTS_PATH", &agents_file)
        .args([
            "agent",
            "create",
            "--manifest",
            manifest_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run cortex agent create");
    assert!(create_out.status.success());
    let create_stdout = String::from_utf8_lossy(&create_out.stdout);
    assert!(create_stdout.contains("Agent 'reviewer' created successfully"));
    assert!(create_stdout.contains("ID:"));
    assert!(create_stdout.contains("Role:      Code Reviewer"));
    assert!(create_stdout.contains("Status:    created"));

    // Extract generated ID
    let id_line = create_stdout
        .lines()
        .find(|l| l.trim().starts_with("ID:"))
        .expect("ID line in create output");
    let agent_id = id_line.split_whitespace().nth(1).unwrap().to_string();

    // 3. List agents - table formatting
    let list_out = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_AGENTS_PATH", &agents_file)
        .args(["agent", "list"])
        .output()
        .expect("Failed to run cortex agent list");
    assert!(list_out.status.success());
    let list_stdout = String::from_utf8_lossy(&list_out.stdout);
    assert!(list_stdout.contains("ID"));
    assert!(list_stdout.contains("NAME"));
    assert!(list_stdout.contains("STATUS"));
    assert!(list_stdout.contains("MODEL"));
    assert!(list_stdout.contains("WORKSPACE"));
    assert!(list_stdout.contains(&agent_id));
    assert!(list_stdout.contains("reviewer"));
    assert!(list_stdout.contains("created"));
    assert!(list_stdout.contains("claude-3-5-sonnet"));

    // 4. Inspect agent (plain text)
    let inspect_out = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_AGENTS_PATH", &agents_file)
        .args(["agent", "inspect", &agent_id])
        .output()
        .expect("Failed to run cortex agent inspect");
    assert!(inspect_out.status.success());
    let inspect_stdout = String::from_utf8_lossy(&inspect_out.stdout);
    assert!(inspect_stdout.contains(&agent_id));
    assert!(inspect_stdout.contains("reviewer"));
    assert!(inspect_stdout.contains("Code Reviewer"));
    assert!(inspect_stdout.contains("Review pull requests"));
    assert!(inspect_stdout.contains("read_file, git_diff"));

    // 5. Inspect agent (JSON)
    let json_out = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_AGENTS_PATH", &agents_file)
        .args(["agent", "inspect", &agent_id, "--json"])
        .output()
        .expect("Failed to run cortex agent inspect --json");
    assert!(json_out.status.success());
    let json_stdout = String::from_utf8_lossy(&json_out.stdout);
    assert!(json_stdout.contains(&format!("\"id\": \"{}\"", agent_id)));
    assert!(json_stdout.contains("\"name\": \"reviewer\""));
    assert!(json_stdout.contains("\"state\": \"created\""));

    // 6. Start agent (created -> running)
    let start_out = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_AGENTS_PATH", &agents_file)
        .args(["agent", "start", &agent_id])
        .output()
        .expect("Failed to run cortex agent start");
    assert!(start_out.status.success());
    assert!(String::from_utf8_lossy(&start_out.stdout).contains("is now running"));

    // 7. Pause agent (running -> paused)
    let pause_out = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_AGENTS_PATH", &agents_file)
        .args(["agent", "pause", &agent_id])
        .output()
        .expect("Failed to run cortex agent pause");
    assert!(pause_out.status.success());
    assert!(String::from_utf8_lossy(&pause_out.stdout).contains("is now paused"));

    // 8. Resume agent via start (paused -> running)
    let resume_out = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_AGENTS_PATH", &agents_file)
        .args(["agent", "start", &agent_id])
        .output()
        .expect("Failed to run cortex agent start (resume)");
    assert!(resume_out.status.success());
    assert!(String::from_utf8_lossy(&resume_out.stdout).contains("is now running"));

    // 9. Stop agent (running -> stopped)
    let stop_out = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_AGENTS_PATH", &agents_file)
        .args(["agent", "stop", &agent_id])
        .output()
        .expect("Failed to run cortex agent stop");
    assert!(stop_out.status.success());
    assert!(String::from_utf8_lossy(&stop_out.stdout).contains("is now stopped"));

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_cli_agent_errors() {
    let tmp_dir =
        std::env::temp_dir().join(format!("cortex_agent_err_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();
    let agents_file = tmp_dir.join("agents.json");

    // Non-existent agent inspect fails gracefully
    let inspect_missing = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_AGENTS_PATH", &agents_file)
        .args(["agent", "inspect", "agent_nonexistent_999"])
        .output()
        .expect("Failed to run cortex agent inspect");
    assert!(!inspect_missing.status.success());
    let stderr = String::from_utf8_lossy(&inspect_missing.stderr);
    assert!(stderr.contains("not found"));

    // Non-existent manifest create fails gracefully
    let create_missing = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_AGENTS_PATH", &agents_file)
        .args(["agent", "create", "--manifest", "non_existent_file.yaml"])
        .output()
        .expect("Failed to run cortex agent create");
    assert!(!create_missing.status.success());
    let create_err = String::from_utf8_lossy(&create_missing.stderr);
    assert!(create_err.contains("Failed to load manifest") || create_err.contains("not found"));

    // Create an agent and test invalid state transition (pausing a created agent)
    let manifest_file = tmp_dir.join("agent_err.yaml");
    std::fs::write(
        &manifest_file,
        "name: err-agent\nworkspace: .\nmodel: gpt-4o\n",
    )
    .unwrap();
    let create_ok = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_AGENTS_PATH", &agents_file)
        .args([
            "agent",
            "create",
            "--manifest",
            manifest_file.to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run cortex agent create");
    assert!(create_ok.status.success());
    let create_stdout = String::from_utf8_lossy(&create_ok.stdout);
    let agent_id = create_stdout
        .lines()
        .find(|l| l.trim().starts_with("ID:"))
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap();

    // Pausing a created (non-running) agent must fail gracefully with descriptive error
    let pause_err = Command::new(env!("CARGO_BIN_EXE_cortex"))
        .env("CORTEX_AGENTS_PATH", &agents_file)
        .args(["agent", "pause", agent_id])
        .output()
        .expect("Failed to run cortex agent pause");
    assert!(!pause_err.status.success());
    let pause_stderr = String::from_utf8_lossy(&pause_err.stderr);
    assert!(pause_stderr.contains("only running agents can be paused"));

    let _ = std::fs::remove_dir_all(&tmp_dir);
}
