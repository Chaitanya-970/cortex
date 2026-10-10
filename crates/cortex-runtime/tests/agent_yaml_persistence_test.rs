//! Integration and persistence tests for YAML agent manifests, SQLite storage,
//! and daemon restart recovery.

use cortex_core::AgentId;
use cortex_runtime::agent::{
    AgentManager, AgentManifest, AgentModelConfig, AgentState, AgentYamlManifest,
};
use cortex_runtime::storage::{
    AgentCheckpointRecord, AgentRecord, RunStore, CURRENT_SCHEMA_VERSION,
};
use std::path::PathBuf;
use std::sync::Arc;

// ============================================================================
// 1. YAML Manifest Parsing and Validation Tests
// ============================================================================

#[test]
fn test_yaml_manifest_spec_parsing() {
    let yaml = r#"
name: "code-reviewer"
model: "claude-3-5-sonnet-20241022"
workspace: "./src"
policy: "Inspect pull requests and review code changes"
tools:
  - read_file
  - git_diff
  - git_log
permissions:
  filesystem: "workspace_only"
  shell: false
  git_push: false
"#;

    let yaml_manifest = AgentYamlManifest::from_yaml_str(yaml).expect("parse YAML manifest");
    assert_eq!(yaml_manifest.name, "code-reviewer");
    assert_eq!(yaml_manifest.workspace, PathBuf::from("./src"));
    assert_eq!(
        yaml_manifest.policy.as_deref(),
        Some("Inspect pull requests and review code changes")
    );
    assert_eq!(
        yaml_manifest.tools,
        vec!["read_file", "git_diff", "git_log"]
    );
    assert_eq!(yaml_manifest.permissions.git_push, Some(false));

    // Convert to runtime AgentManifest
    let runtime_manifest = yaml_manifest
        .into_manifest()
        .expect("convert to runtime manifest");
    assert_eq!(runtime_manifest.name, "code-reviewer");
    assert_eq!(
        runtime_manifest.role,
        "Inspect pull requests and review code changes"
    );
    assert_eq!(runtime_manifest.model.provider, "anthropic");
    assert_eq!(runtime_manifest.model.model, "claude-3-5-sonnet-20241022");
    assert_eq!(
        runtime_manifest.system_prompt.as_deref(),
        Some("Inspect pull requests and review code changes")
    );
    assert_eq!(
        runtime_manifest.tools,
        vec!["read_file", "git_diff", "git_log"]
    );
    assert!(runtime_manifest
        .permissions
        .filesystem
        .contains(&"workspace_only".to_string()));
    assert!(runtime_manifest.permissions.shell.is_empty());
}

#[test]
fn test_yaml_manifest_provider_inference() {
    // OpenAI model inference
    let yaml_openai = r#"
name: "gpt-coder"
model: "gpt-4o"
workspace: "/tmp/workspace"
policy: "Write clean Rust code"
"#;
    let manifest_openai = AgentManifest::from_yaml_str(yaml_openai).expect("parse openai");
    assert_eq!(manifest_openai.model.provider, "openai");
    assert_eq!(manifest_openai.model.model, "gpt-4o");

    // Google model inference
    let yaml_google = r#"
name: "gemini-researcher"
model: "gemini-1.5-pro"
workspace: "/tmp/workspace"
policy: "Analyze code repos"
"#;
    let manifest_google = AgentManifest::from_yaml_str(yaml_google).expect("parse google");
    assert_eq!(manifest_google.model.provider, "google");
    assert_eq!(manifest_google.model.model, "gemini-1.5-pro");

    // Mock model inference
    let yaml_mock = r#"
name: "mock-worker"
model: "mock-model"
workspace: "/tmp/workspace"
policy: "Test execution"
"#;
    let manifest_mock = AgentManifest::from_yaml_str(yaml_mock).expect("parse mock");
    assert_eq!(manifest_mock.model.provider, "mock");
}

#[test]
fn test_yaml_manifest_detailed_model_and_permissions() {
    let yaml = r#"
name: "full-config-agent"
model:
  model: "custom-llm-v1"
  provider: "ollama"
  temperature: 0.2
  max_tokens: 2048
workspace: "./data"
role: "Data Processor"
tools:
  - run_query
  - export_csv
permissions:
  filesystem:
    - "/data/input"
    - "/data/output"
  shell:
    - "psql"
  network: true
auto_resume: true
"#;

    let manifest = AgentManifest::from_yaml_str(yaml).expect("parse full yaml");
    assert_eq!(manifest.name, "full-config-agent");
    assert_eq!(manifest.role, "Data Processor");
    assert_eq!(manifest.model.provider, "ollama");
    assert_eq!(manifest.model.model, "custom-llm-v1");
    assert_eq!(manifest.model.temperature, 0.2);
    assert_eq!(manifest.model.max_tokens, Some(2048));
    assert!(manifest.auto_resume);
    assert_eq!(
        manifest.permissions.filesystem,
        vec!["/data/input".to_string(), "/data/output".to_string()]
    );
    assert_eq!(manifest.permissions.shell, vec!["psql".to_string()]);
    assert_eq!(manifest.permissions.network, vec!["allow".to_string()]);
}

#[test]
fn test_yaml_manifest_roundtrip_serialization() {
    let orig = AgentManifest::new(
        "roundtrip-agent",
        "Integration Tester",
        "/tmp/roundtrip",
        AgentModelConfig::new("anthropic", "claude-3-5-sonnet"),
    )
    .with_tools(vec!["grep".to_string(), "read_file".to_string()])
    .with_system_prompt("Be concise.")
    .with_auto_resume(true);

    let serialized = orig.to_yaml_string().expect("serialize to YAML");
    assert!(serialized.contains("roundtrip-agent"));
    assert!(serialized.contains("Integration Tester"));

    let deserialized = AgentManifest::from_yaml_str(&serialized).expect("deserialize from YAML");
    assert_eq!(orig.name, deserialized.name);
    assert_eq!(orig.role, deserialized.role);
    assert_eq!(orig.workspace, deserialized.workspace);
    assert_eq!(orig.model, deserialized.model);
    assert_eq!(orig.tools, deserialized.tools);
    assert_eq!(orig.auto_resume, deserialized.auto_resume);
}

#[test]
fn test_yaml_schema_validation_errors() {
    // 1. Missing / empty name
    let empty_name = r#"
name: "   "
model: "gpt-4o"
workspace: "./"
policy: "Do work"
"#;
    let err = AgentYamlManifest::from_yaml_str(empty_name).unwrap_err();
    assert!(
        err.to_string().contains("'name' is required"),
        "err: {}",
        err
    );

    // 2. Invalid characters in name
    let invalid_name = r#"
name: "bad name with spaces!"
model: "gpt-4o"
workspace: "./"
policy: "Do work"
"#;
    let err = AgentYamlManifest::from_yaml_str(invalid_name).unwrap_err();
    assert!(
        err.to_string().contains("contains invalid characters"),
        "err: {}",
        err
    );

    // 3. Missing workspace
    let empty_ws = r#"
name: "valid-name"
model: "gpt-4o"
workspace: ""
policy: "Do work"
"#;
    let err = AgentYamlManifest::from_yaml_str(empty_ws).unwrap_err();
    assert!(
        err.to_string().contains("'workspace' path is required"),
        "err: {}",
        err
    );

    // 4. Missing policy, role, and system_prompt
    let missing_policy = r#"
name: "valid-name"
model: "gpt-4o"
workspace: "./src"
"#;
    let err = AgentYamlManifest::from_yaml_str(missing_policy).unwrap_err();
    assert!(
        err.to_string()
            .contains("at least one of 'policy', 'role', or 'system_prompt'"),
        "err: {}",
        err
    );

    // 5. Corrupt syntax
    let bad_syntax = "name: [this is invalid yaml: :::";
    let err = AgentYamlManifest::from_yaml_str(bad_syntax).unwrap_err();
    assert!(
        err.to_string().contains("failed to parse YAML manifest"),
        "err: {}",
        err
    );
}

// ============================================================================
// 2. Database Schema and Migrations Tests
// ============================================================================

#[test]
fn test_database_schema_v2_migration() {
    let store = RunStore::in_memory().expect("open in-memory store");
    assert_eq!(store.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
    assert!(store.schema_version().unwrap() >= 2);

    // Verify agents and agent_checkpoints tables are queryable
    let agents = store.list_agents().expect("list agents on fresh DB");
    assert!(agents.is_empty());
}

#[test]
fn test_database_migration_from_v1_preserves_runs() {
    // Manually create a v1 SQLite database
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE schema_version (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
        CREATE TABLE runs (
            id TEXT PRIMARY KEY,
            task TEXT NOT NULL,
            status TEXT NOT NULL,
            started_at TEXT NOT NULL,
            finished_at TEXT,
            duration_ms INTEGER,
            tokens_prompt INTEGER NOT NULL DEFAULT 0,
            tokens_completion INTEGER NOT NULL DEFAULT 0,
            tokens_total INTEGER NOT NULL DEFAULT 0,
            estimated_cost_usd REAL NOT NULL DEFAULT 0.0,
            error TEXT
        );
        CREATE TABLE events (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL,
            sequence INTEGER NOT NULL,
            timestamp TEXT NOT NULL,
            event_type TEXT NOT NULL,
            payload_json TEXT NOT NULL
        );
        INSERT INTO schema_version (version, applied_at) VALUES (1, '2026-10-08T00:00:00Z');
        INSERT INTO runs (id, task, status, started_at) VALUES ('run_legacy', 'legacy task', 'completed', '2026-10-08T00:00:00Z');
        "#,
    )
    .unwrap();

    // Now instantiate RunStore on that existing connection or apply migrations
    let v2_script = include_str!("../migrations/002_agents_and_checkpoints.sql");
    conn.execute_batch(v2_script).unwrap();
    conn.execute(
        "INSERT INTO schema_version (version, applied_at) VALUES (2, '2026-10-09T00:00:00Z')",
        [],
    )
    .unwrap();

    // Verify runs table data is preserved
    let run_task: String = conn
        .query_row("SELECT task FROM runs WHERE id = 'run_legacy'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(run_task, "legacy task");

    // Verify agents table is created and empty
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM agents", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

// ============================================================================
// 3. SQLite Persistence: Save, Load, Update, Checkpoints
// ============================================================================

#[test]
fn test_agent_persistence_crud_operations() {
    let store = RunStore::in_memory().expect("open store");
    let agent_id = AgentId::generate();

    let record = AgentRecord {
        id: agent_id.clone(),
        name: "test-agent-alpha".to_string(),
        manifest_yaml: "name: test-agent-alpha\nworkspace: /tmp\n".to_string(),
        status: "created".to_string(),
        created_at: "2026-10-09T10:00:00Z".to_string(),
        updated_at: "2026-10-09T10:00:00Z".to_string(),
    };

    // 1. Save agent
    store.save_agent(&record).expect("save agent");

    // 2. Get agent
    let loaded = store
        .get_agent(&agent_id)
        .expect("get agent")
        .expect("found");
    assert_eq!(loaded.name, "test-agent-alpha");
    assert_eq!(loaded.status, "created");

    // 3. Update status
    store
        .update_agent_status(&agent_id, "running")
        .expect("update status");
    let updated = store.get_agent(&agent_id).unwrap().unwrap();
    assert_eq!(updated.status, "running");

    // 4. List agents
    let list = store.list_agents().expect("list agents");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, agent_id);

    // 5. Delete agent
    let deleted = store.delete_agent(&agent_id).expect("delete agent");
    assert!(deleted);
    assert!(store.get_agent(&agent_id).unwrap().is_none());
}

#[test]
fn test_agent_checkpoints_persistence() {
    let store = RunStore::in_memory().expect("open store");
    let agent_id = AgentId::generate();

    let record = AgentRecord {
        id: agent_id.clone(),
        name: "checkpoint-agent".to_string(),
        manifest_yaml: "name: checkpoint-agent\nworkspace: /tmp\n".to_string(),
        status: "running".to_string(),
        created_at: "2026-10-09T10:00:00Z".to_string(),
        updated_at: "2026-10-09T10:00:00Z".to_string(),
    };
    store.save_agent(&record).unwrap();

    // Save checkpoints
    let cp1 = AgentCheckpointRecord {
        id: None,
        agent_id: agent_id.clone(),
        step: 1,
        state: "running".to_string(),
        data_json: Some(r#"{"memory": "step 1 finished"}"#.to_string()),
        created_at: "2026-10-09T10:01:00Z".to_string(),
    };
    let cp2 = AgentCheckpointRecord {
        id: None,
        agent_id: agent_id.clone(),
        step: 2,
        state: "paused".to_string(),
        data_json: Some(r#"{"memory": "step 2 paused"}"#.to_string()),
        created_at: "2026-10-09T10:02:00Z".to_string(),
    };

    let id1 = store.save_agent_checkpoint(&cp1).expect("save cp1");
    let id2 = store.save_agent_checkpoint(&cp2).expect("save cp2");
    assert!(id2 > id1);

    // List checkpoints
    let checkpoints = store
        .list_agent_checkpoints(&agent_id)
        .expect("list checkpoints");
    assert_eq!(checkpoints.len(), 2);
    assert_eq!(checkpoints[0].step, 1);
    assert_eq!(checkpoints[1].step, 2);
    assert_eq!(checkpoints[1].state, "paused");
}

// ============================================================================
// 4. Daemon Restart Recovery Tests
// ============================================================================

#[test]
fn test_daemon_restart_reconciles_running_to_stopped() {
    let store = Arc::new(RunStore::in_memory().expect("open store"));

    let id1 = AgentId::generate();
    let id2 = AgentId::generate();

    // Agent 1: standard (auto_resume: false), left in 'running'
    let yaml1 = r#"
name: "interrupted-agent"
model: "claude-3-5-sonnet-20241022"
workspace: "./src"
policy: "Review code"
auto_resume: false
"#;
    store
        .save_agent(&AgentRecord {
            id: id1.clone(),
            name: "interrupted-agent".to_string(),
            manifest_yaml: yaml1.to_string(),
            status: "running".to_string(),
            created_at: "2026-10-09T10:00:00Z".to_string(),
            updated_at: "2026-10-09T10:00:00Z".to_string(),
        })
        .unwrap();

    // Agent 2: auto_resume: true, left in 'running'
    let yaml2 = r#"
name: "resuming-agent"
model: "gpt-4o"
workspace: "./src"
policy: "Background sync"
auto_resume: true
"#;
    store
        .save_agent(&AgentRecord {
            id: id2.clone(),
            name: "resuming-agent".to_string(),
            manifest_yaml: yaml2.to_string(),
            status: "running".to_string(),
            created_at: "2026-10-09T10:00:00Z".to_string(),
            updated_at: "2026-10-09T10:00:00Z".to_string(),
        })
        .unwrap();

    // Rehydrate AgentManager from store simulating daemon restart
    let recovered_manager =
        AgentManager::load_from_store(Arc::clone(&store)).expect("recover from store");

    // Agent 1 should be reconciled to Stopped
    let agent1 = recovered_manager.inspect(&id1).expect("agent 1 found");
    assert_eq!(agent1.state, AgentState::Stopped);

    // Agent 2 should remain in Running (auto-resumed)
    let agent2 = recovered_manager.inspect(&id2).expect("agent 2 found");
    assert_eq!(agent2.state, AgentState::Running);

    // Verify database state matches
    let db_agent1 = store.get_agent(&id1).unwrap().unwrap();
    assert_eq!(db_agent1.status, "stopped");
    let db_agent2 = store.get_agent(&id2).unwrap().unwrap();
    assert_eq!(db_agent2.status, "running");
}

#[test]
fn test_agent_manager_with_store_automatic_persistence() {
    let store = Arc::new(RunStore::in_memory().expect("open store"));
    let manager = AgentManager::new_with_store(Arc::clone(&store));

    let yaml = r#"
name: "persisted-worker"
model: "claude-3-5-sonnet-20241022"
workspace: "./workspace"
policy: "Execute integration benchmarks"
tools:
  - run_bench
"#;
    let manifest = AgentManifest::from_yaml_str(yaml).expect("parse manifest");

    // Create
    let agent = manager.create(manifest).expect("create agent");
    let id = agent.id.clone();

    // Verify automatically stored in DB
    let in_db = store.get_agent(&id).unwrap().expect("found in DB");
    assert_eq!(in_db.name, "persisted-worker");
    assert_eq!(in_db.status, "created");

    // Transition to Ready
    manager.prepare(&id).expect("prepare");
    assert_eq!(store.get_agent(&id).unwrap().unwrap().status, "ready");

    // Start (Running)
    manager.start(&id).expect("start");
    assert_eq!(store.get_agent(&id).unwrap().unwrap().status, "running");

    // Save checkpoint
    let cp_id = manager
        .save_checkpoint(&id, 10, Some(r#"{"status": "checkpoint-1"}"#.to_string()))
        .expect("save checkpoint");
    assert_eq!(cp_id, 1);

    let checkpoints = manager.list_checkpoints(&id).expect("list checkpoints");
    assert_eq!(checkpoints.len(), 1);
    assert_eq!(checkpoints[0].step, 10);

    // Pause
    manager.pause(&id).expect("pause");
    assert_eq!(store.get_agent(&id).unwrap().unwrap().status, "paused");

    // Resume
    manager.resume(&id).expect("resume");
    assert_eq!(store.get_agent(&id).unwrap().unwrap().status, "running");

    // Stop
    manager.stop(&id).expect("stop");
    assert_eq!(store.get_agent(&id).unwrap().unwrap().status, "stopped");

    // Restart
    manager.restart(&id).expect("restart");
    assert_eq!(store.get_agent(&id).unwrap().unwrap().status, "ready");

    // Fail
    manager.start(&id).expect("start again");
    manager.fail(&id, "fatal test abort").expect("fail");
    assert_eq!(store.get_agent(&id).unwrap().unwrap().status, "failed");

    // Remove
    manager.stop(&id).expect("stop before removal");
    manager.remove(&id).expect("remove");
    assert!(store.get_agent(&id).unwrap().is_none());
}
