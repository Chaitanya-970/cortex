//! Integration and stress tests for AgentManager lifecycle boundaries,
//! invalid state transitions, crash recovery reconciliation, and concurrency.

use cortex_core::{EventRecord, ExecutionEvent, RunId};
use cortex_runtime::agent::{AgentManager, AgentManifest, AgentModelConfig, AgentState};
use cortex_runtime::storage::RunStore;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

fn make_test_manifest(name: &str, role: &str) -> AgentManifest {
    AgentManifest::new(
        name,
        role,
        "/tmp/cortex-test-workspace",
        AgentModelConfig::new("mock", "gpt-4o"),
    )
}

// ============================================================================
// 1. Invalid State Transitions
// ============================================================================

#[test]
fn test_invalid_transition_cannot_pause_stopped_or_created() {
    let manager = AgentManager::new();
    let agent = manager
        .create(make_test_manifest("worker-alpha", "Tester"))
        .expect("create agent");
    let id = agent.id;

    // Pausing a Created agent must fail
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Created);
    let err_created = manager.pause(&id).unwrap_err();
    assert!(
        err_created
            .to_string()
            .contains("only running agents can be paused"),
        "unexpected error message: {}",
        err_created
    );

    // Transition to Stopped
    manager.stop(&id).expect("stop agent");
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Stopped);

    // Pausing a Stopped agent must fail
    let err_stopped = manager.pause(&id).unwrap_err();
    assert!(
        err_stopped
            .to_string()
            .contains("only running agents can be paused"),
        "unexpected error message: {}",
        err_stopped
    );

    // Restart, start, then fail
    manager.restart(&id).expect("restart agent");
    manager.start(&id).expect("start agent");
    manager.fail(&id, "fatal test error").expect("fail agent");
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Failed);

    // Pausing a Failed agent must fail
    let err_failed = manager.pause(&id).unwrap_err();
    assert!(
        err_failed
            .to_string()
            .contains("only running agents can be paused"),
        "unexpected error message: {}",
        err_failed
    );
}

#[test]
fn test_invalid_transition_cannot_start_stopped_or_failed_directly() {
    let manager = AgentManager::new();
    let agent = manager
        .create(make_test_manifest("worker-beta", "Deployer"))
        .expect("create agent");
    let id = agent.id;

    // Stop the agent
    manager.stop(&id).expect("stop agent");

    // Starting a Stopped agent directly must fail
    let err_start_stopped = manager.start(&id).unwrap_err();
    assert!(
        err_start_stopped
            .to_string()
            .contains("cannot start stopped agent"),
        "unexpected error message: {}",
        err_start_stopped
    );

    // After restart to Ready, starting succeeds
    manager.restart(&id).expect("restart agent");
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Ready);
    assert!(manager.start(&id).is_ok());

    // Fail the agent
    manager
        .fail(&id, "unrecoverable memory fault")
        .expect("fail agent");

    // Starting a Failed agent directly must fail
    let err_start_failed = manager.start(&id).unwrap_err();
    assert!(
        err_start_failed
            .to_string()
            .contains("cannot start failed agent"),
        "unexpected error message: {}",
        err_start_failed
    );

    // After restart, starting succeeds again
    manager.restart(&id).expect("restart agent");
    assert!(manager.start(&id).is_ok());
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Running);
}

#[test]
fn test_tool_execution_prohibited_on_paused_and_stopped_agents() {
    let manager = AgentManager::new();
    let manifest = make_test_manifest("tool-worker", "Coder")
        .with_tools(vec!["read_file".to_string(), "git_diff".to_string()]);
    let agent = manager.create(manifest).expect("create agent");
    let id = agent.id;

    // 1. While Created: tool execution is prohibited
    assert!(!manager.can_execute_tool(&id).unwrap());
    let err_created = manager
        .authorize_tool_execution(&id, "read_file")
        .unwrap_err();
    assert!(err_created
        .to_string()
        .contains("only running agents can execute tools"));

    // 2. While Running: authorized tools succeed
    manager.start(&id).expect("start agent");
    assert!(manager.can_execute_tool(&id).unwrap());
    assert!(manager.authorize_tool_execution(&id, "read_file").is_ok());
    assert!(manager.authorize_tool_execution(&id, "git_diff").is_ok());

    // Unauthorized tool is rejected even while Running
    let err_unauth = manager.authorize_tool_execution(&id, "rm_rf").unwrap_err();
    assert!(err_unauth.to_string().contains("not authorized"));

    // 3. While Paused: tool execution is strictly prohibited
    manager.pause(&id).expect("pause agent");
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Paused);
    assert!(!manager.can_execute_tool(&id).unwrap());
    let err_paused = manager
        .authorize_tool_execution(&id, "read_file")
        .unwrap_err();
    assert!(
        err_paused
            .to_string()
            .contains("only running agents can execute tools"),
        "expected paused validation error, got: {}",
        err_paused
    );

    // 4. Resume to Running: tool execution is permitted again
    manager.resume(&id).expect("resume agent");
    assert!(manager.can_execute_tool(&id).unwrap());
    assert!(manager.authorize_tool_execution(&id, "read_file").is_ok());

    // 5. While Stopped: tool execution is prohibited
    manager.stop(&id).expect("stop agent");
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Stopped);
    assert!(!manager.can_execute_tool(&id).unwrap());
    let err_stopped = manager
        .authorize_tool_execution(&id, "read_file")
        .unwrap_err();
    assert!(err_stopped
        .to_string()
        .contains("only running agents can execute tools"));
}

#[test]
fn test_idempotent_start_on_already_running_agent() {
    let manager = AgentManager::new();
    let rx = manager.subscribe();
    let agent = manager
        .create(make_test_manifest("idempotent-starter", "Runner"))
        .expect("create agent");
    let id = agent.id;

    // Initial start
    manager.start(&id).expect("first start");
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Running);

    // Repeated starts on an already running agent succeed idempotently
    assert!(manager.start(&id).is_ok());
    assert!(manager.start(&id).is_ok());
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Running);

    // Verify only one AgentStarted event was emitted (no duplicate spam)
    let mut started_events = 0;
    while let Ok(event) = rx.try_recv() {
        if event.event_type() == "AgentStarted" {
            started_events += 1;
        }
    }
    assert_eq!(started_events, 1, "expected exactly 1 AgentStarted event");
}

// ============================================================================
// 2. Crash Recovery & Reconciliation
// ============================================================================

#[test]
fn test_crash_recovery_mid_iteration_sqlite_reconciliation() {
    let tmp_dir = std::env::temp_dir().join(format!("cortex_crash_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();
    let db_path = tmp_dir.join("cortex_recovery.db");

    let run_id = RunId::from("run_mid_iteration_abort_001");
    let started_at = "2026-10-09T08:00:00Z";

    // 1. Initial process execution: start a run and record events
    {
        let store = RunStore::open(&db_path).expect("open initial store");
        store
            .record_run_start(&run_id, "Simulate heavy refactoring task", started_at)
            .expect("record run start");

        let event = ExecutionEvent::RunStarted {
            run_id: run_id.clone(),
            task: "Simulate heavy refactoring task".to_string(),
            workspace_root: Some("/tmp/workspace".to_string()),
        };
        store
            .record_event(&EventRecord::new(1, event))
            .expect("record event 1");

        // Verify initial state is "running" with no finish timestamp
        let run = store.get_run(&run_id).unwrap().expect("run exists");
        assert_eq!(run.status, "running");
        assert!(run.finished_at.is_none());

        // Process abruptly crashes/drops here without calling record_run_completion
    }

    // 2. Daemon restart & recovery: open database and reconcile dangling runs
    {
        let recovery_store = RunStore::open(&db_path).expect("open recovery store");

        // Before reconciliation, the run was left in "running"
        let unrecovered = recovery_store.get_run(&run_id).unwrap().unwrap();
        assert_eq!(unrecovered.status, "running");
        assert!(unrecovered.finished_at.is_none());

        // Reconcile crashed runs
        let reconciled_count = recovery_store
            .reconcile_crashed_runs("daemon terminated unexpectedly during active iteration")
            .expect("reconcile runs");
        assert_eq!(reconciled_count, 1);

        // After reconciliation, verify deterministic state
        let recovered = recovery_store.get_run(&run_id).unwrap().unwrap();
        assert_eq!(recovered.status, "failed");
        assert!(recovered.finished_at.is_some());
        assert!(
            recovered
                .error
                .as_deref()
                .unwrap()
                .contains("daemon terminated unexpectedly"),
            "expected crash diagnostics in error field"
        );

        // Running reconciliation again is idempotent (0 further updates)
        let second_reconcile = recovery_store
            .reconcile_crashed_runs("subsequent pass")
            .expect("idempotent reconcile");
        assert_eq!(second_reconcile, 0);
    }

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_agent_manager_reconciliation_after_unexpected_process_abort() {
    let manager = AgentManager::new();

    // Register 3 agents in different states
    let a1 = manager
        .create(make_test_manifest("agent-1", "Worker"))
        .unwrap();
    let a2 = manager
        .create(make_test_manifest("agent-2", "Reviewer"))
        .unwrap();
    let a3 = manager
        .create(make_test_manifest("agent-3", "Janitor"))
        .unwrap();

    manager.start(&a1.id).unwrap();
    manager.start(&a2.id).unwrap();
    manager.stop(&a3.id).unwrap();

    assert_eq!(manager.inspect(&a1.id).unwrap().state, AgentState::Running);
    assert_eq!(manager.inspect(&a2.id).unwrap().state, AgentState::Running);
    assert_eq!(manager.inspect(&a3.id).unwrap().state, AgentState::Stopped);

    // Simulate crash reconciliation on recovery boot
    let reconciled = manager.reconcile_crashed_agents().unwrap();
    assert_eq!(reconciled, 2, "expected 2 running agents to be reconciled");

    // All previously running agents are now Stopped
    assert_eq!(manager.inspect(&a1.id).unwrap().state, AgentState::Stopped);
    assert_eq!(manager.inspect(&a2.id).unwrap().state, AgentState::Stopped);
    assert_eq!(manager.inspect(&a3.id).unwrap().state, AgentState::Stopped);

    // They can all be safely restarted via restart() -> Ready -> Running
    manager.restart(&a1.id).unwrap();
    manager.start(&a1.id).unwrap();
    assert_eq!(manager.inspect(&a1.id).unwrap().state, AgentState::Running);
}

#[test]
fn test_corrupt_manifest_validation_does_not_poison_state() {
    let manager = AgentManager::new();

    // 1. Corrupt JSON syntax
    let bad_json = r#"{"name": "broken", "role": "tester", "#;
    let json_err = AgentManifest::from_json_str(bad_json).unwrap_err();
    assert!(json_err.to_string().contains("invalid json manifest"));

    // 2. Corrupt TOML syntax
    let bad_toml = "name = [unclosed syntax";
    let toml_err = AgentManifest::from_toml_str(bad_toml).unwrap_err();
    assert!(toml_err.to_string().contains("invalid toml manifest"));

    // 3. Manifest with empty name rejected
    let empty_name_manifest = make_test_manifest("   ", "Valid Role");
    let create_name_err = manager.create(empty_name_manifest).unwrap_err();
    assert!(create_name_err
        .to_string()
        .contains("agent name cannot be empty"));

    // 4. Manifest with empty role rejected
    let empty_role_manifest = make_test_manifest("Valid Name", "   ");
    let create_role_err = manager.create(empty_role_manifest).unwrap_err();
    assert!(create_role_err
        .to_string()
        .contains("agent role cannot be empty"));

    // 5. Manager state was not poisoned: list is still empty
    assert!(manager.list().is_empty());

    // 6. Valid creation succeeds immediately after corrupt attempts
    let good = manager
        .create(make_test_manifest("healthy-agent", "Engineer"))
        .expect("create healthy agent");
    assert_eq!(good.name, "healthy-agent");
    assert_eq!(manager.list().len(), 1);
}

// ============================================================================
// 3. Concurrent Workers & Lock Contention
// ============================================================================

#[test]
fn test_concurrent_persistent_agents_independent_workspaces() {
    let tmp_dir = std::env::temp_dir().join(format!(
        "cortex_concurrent_workspaces_{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    std::fs::create_dir_all(&tmp_dir).unwrap();

    let db_path = tmp_dir.join("shared_runs.db");
    let store = Arc::new(RunStore::open(&db_path).expect("open shared store"));
    let manager = Arc::new(AgentManager::new());

    let worker_count = 16;
    let completed_counter = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    for i in 0..worker_count {
        let mgr = Arc::clone(&manager);
        let str_ref = Arc::clone(&store);
        let counter = Arc::clone(&completed_counter);
        let ws_dir = tmp_dir.join(format!("workspace_{:02}", i));
        std::fs::create_dir_all(&ws_dir).unwrap();

        handles.push(thread::spawn(move || {
            let manifest = AgentManifest::new(
                format!("worker_{:02}", i),
                "Concurrent Engineer",
                &ws_dir,
                AgentModelConfig::new("mock", "gpt-4o"),
            )
            .with_tools(vec!["read_file".to_string()]);

            let agent = mgr.create(manifest).expect("create concurrent agent");
            let id = agent.id;
            let run_id = RunId::from(format!("run_concurrent_{:02}", i));

            // Record run start in shared SQLite database
            str_ref
                .record_run_start(
                    &run_id,
                    &format!("Task for worker {}", i),
                    "2026-10-09T08:00:00Z",
                )
                .expect("record run start");

            // Execute full lifecycle flow
            mgr.prepare(&id).expect("prepare");
            mgr.start(&id).expect("start");

            // Verify tool authorization during active execution
            assert!(mgr.can_execute_tool(&id).unwrap());
            assert!(mgr.authorize_tool_execution(&id, "read_file").is_ok());

            // Record events in shared SQLite
            let event = ExecutionEvent::RunStarted {
                run_id: run_id.clone(),
                task: format!("Task for worker {}", i),
                workspace_root: Some(ws_dir.to_string_lossy().to_string()),
            };
            str_ref
                .record_event(&EventRecord::new(1, event))
                .expect("record event");

            // Pause and resume
            mgr.pause(&id).expect("pause");
            assert!(!mgr.can_execute_tool(&id).unwrap());
            mgr.resume(&id).expect("resume");

            // Stop cleanly
            mgr.stop(&id).expect("stop");

            // Record run completion in shared SQLite database
            str_ref
                .record_run_completion(
                    &run_id,
                    "completed",
                    "2026-10-09T08:00:05Z",
                    5000,
                    120,
                    45,
                    0.0003,
                    None,
                )
                .expect("record run completion");

            counter.fetch_add(1, Ordering::SeqCst);
        }));
    }

    for h in handles {
        h.join().expect("thread join succeeded without panic");
    }

    // Verify all workers completed without contention
    assert_eq!(completed_counter.load(Ordering::SeqCst), worker_count);

    // Verify all agents in manager
    let all_agents = manager.list();
    assert_eq!(all_agents.len(), worker_count);
    for a in all_agents {
        assert_eq!(a.state, AgentState::Stopped);
    }

    // Verify all runs in shared SQLite database
    let all_runs = store.list_runs(worker_count * 2).expect("list runs");
    assert_eq!(all_runs.len(), worker_count);
    for run in all_runs {
        assert_eq!(run.status, "completed");
    }

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_high_concurrency_manager_lifecycle_stress() {
    let manager = Arc::new(AgentManager::new());
    let stress_threads = 20;
    let mut handles = Vec::new();

    for i in 0..stress_threads {
        let m = Arc::clone(&manager);
        handles.push(thread::spawn(move || {
            for step in 0..5 {
                let name = format!("stress_{}_{}", i, step);
                let manifest = make_test_manifest(&name, "Stress Unit");
                let agent = m.create(manifest).unwrap();
                let id = agent.id;

                m.start(&id).unwrap();
                m.pause(&id).unwrap();
                m.resume(&id).unwrap();
                m.stop(&id).unwrap();
                m.restart(&id).unwrap();
                m.start(&id).unwrap();
                m.stop(&id).unwrap();
                m.remove(&id).unwrap();
            }
        }));
    }

    for h in handles {
        h.join().expect("stress thread joined without panic");
    }

    // All created agents were cleaned up
    assert_eq!(manager.list().len(), 0);
}
