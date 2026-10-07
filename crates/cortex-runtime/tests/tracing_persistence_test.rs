//! Integration tests for execution tracing, event ordering, secret redaction,
//! and SQLite persistence.

use cortex_core::{CortexError, ExecutionEvent, Redactor, RunId};
use cortex_runtime::model::{MockModelProvider, ModelOutput, ToolCall};
use cortex_runtime::storage::RunStore;
use cortex_runtime::tool::{Tool, ToolDefinition, ToolRegistry, ToolResult};
use cortex_runtime::{AgentContext, AgentLoop, CancellationToken};
use serde_json::json;
use std::sync::Arc;

struct EchoTool;
impl Tool for EchoTool {
    fn definition(&self) -> &ToolDefinition {
        static DEF: std::sync::OnceLock<ToolDefinition> = std::sync::OnceLock::new();
        DEF.get_or_init(|| {
            ToolDefinition::new(
                "echo_tool",
                "Echoes input message back",
                json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    },
                    "required": ["message"]
                }),
            )
        })
    }

    fn execute(&self, input: &serde_json::Value) -> cortex_core::Result<ToolResult> {
        let msg = input
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("no message");
        Ok(ToolResult::success(format!("Echo: {}", msg)))
    }
}

struct FailingTool;
impl Tool for FailingTool {
    fn definition(&self) -> &ToolDefinition {
        static DEF: std::sync::OnceLock<ToolDefinition> = std::sync::OnceLock::new();
        DEF.get_or_init(|| {
            ToolDefinition::new(
                "failing_tool",
                "Always fails with an error",
                json!({ "type": "object" }),
            )
        })
    }

    fn execute(&self, _input: &serde_json::Value) -> cortex_core::Result<ToolResult> {
        Ok(ToolResult::error("command crashed with exit code 1"))
    }
}

#[test]
fn test_event_ordering_and_sequence_integrity() {
    let store = Arc::new(RunStore::in_memory().unwrap());
    let run_id = RunId::from("run_order_verify");
    let mut context = AgentContext::new("Perform sequence testing").with_run_id(run_id.clone());

    let registry = ToolRegistry::new();
    registry.register_tool(EchoTool).unwrap();

    let model = MockModelProvider::new();
    // Step 1: Tool call
    model.queue_response(ModelOutput::ToolCalls(vec![ToolCall::new(
        "call_1",
        "echo_tool",
        json!({ "message": "hello world" }),
    )]));
    // Step 2: Final answer
    model.queue_response(ModelOutput::FinalAnswer("Sequence completed".to_string()));

    let agent = AgentLoop::new(5).with_store(Arc::clone(&store));
    let res = agent.run(&mut context, &model, &registry).unwrap();
    assert!(res.completed);

    let events = store.get_events(&run_id).unwrap();
    assert_eq!(events.len(), 8);

    // Verify monotonic sequencing: 1, 2, 3, 4, 5, 6, 7, 8
    for (idx, record) in events.iter().enumerate() {
        assert_eq!(record.sequence, (idx + 1) as u64);
    }

    // Verify lifecycle progression across both iterations
    assert_eq!(events[0].event.event_type(), "RunStarted");
    assert_eq!(events[1].event.event_type(), "ModelRequest");
    assert_eq!(events[2].event.event_type(), "ModelResponse");
    assert_eq!(events[3].event.event_type(), "ToolStarted");
    assert_eq!(events[4].event.event_type(), "ToolCompleted");
    assert_eq!(events[5].event.event_type(), "ModelRequest");
    assert_eq!(events[6].event.event_type(), "ModelResponse");
    assert_eq!(events[7].event.event_type(), "RunCompleted");

    // Verify run summary record
    let run = store.get_run(&run_id).unwrap().unwrap();
    assert_eq!(run.status, "completed");
    assert!(run.finished_at.is_some());
    assert!(run.duration_ms.is_some());
}

#[test]
fn test_secret_redaction_in_sqlite_events() {
    let store = Arc::new(RunStore::in_memory().unwrap());
    let run_id = RunId::from("run_secret_redact");
    let mut context =
        AgentContext::new("Process confidential data with token sk-testsecretkey12345678901234")
            .with_run_id(run_id.clone());

    let registry = ToolRegistry::new();
    registry.register_tool(EchoTool).unwrap();

    let model = MockModelProvider::new();
    model.queue_response(ModelOutput::ToolCalls(vec![ToolCall::new(
        "call_sec",
        "echo_tool",
        json!({
            "message": "Authorization: Bearer mysecrettokenstring123456",
            "password": "supersecretpassword123"
        }),
    )]));
    model.queue_response(ModelOutput::FinalAnswer(
        "Finished processing key ghp_1234567890abcdef1234567890abcdef1234".to_string(),
    ));

    let redactor = Arc::new(Redactor::default());
    let agent = AgentLoop::new(5)
        .with_store(Arc::clone(&store))
        .with_redactor(redactor);

    let res = agent.run(&mut context, &model, &registry).unwrap();
    assert!(res.completed);

    let events = store.get_events(&run_id).unwrap();
    for record in events {
        match record.event {
            ExecutionEvent::RunStarted { task, .. } => {
                assert!(!task.contains("sk-testsecretkey"));
                assert!(task.contains("[REDACTED]"));
            }
            ExecutionEvent::ToolStarted { arguments, .. } => {
                let s = arguments.to_string();
                assert!(!s.contains("Bearer mysecrettokenstring"));
                assert!(!s.contains("supersecretpassword123"));
                assert!(s.contains("[REDACTED]"));
            }
            ExecutionEvent::RunCompleted { final_answer, .. } => {
                assert!(!final_answer.contains("ghp_"));
                assert!(final_answer.contains("[REDACTED]"));
            }
            _ => {}
        }
    }
}

#[test]
fn test_schema_migration_idempotency_and_file_persistence() {
    let tmp_dir = std::env::temp_dir().join(format!("cortex_persist_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp_dir);
    let db_path = tmp_dir.join("cortex_test.db");

    let run_id = RunId::from("run_file_persist");

    // Phase 1: Open store, insert data, and drop
    {
        let store = RunStore::open(&db_path).unwrap();
        assert_eq!(store.schema_version().unwrap(), 1);

        store
            .record_run_start(&run_id, "File persistence test", "2026-10-08T00:00:00Z")
            .unwrap();
        let ev = ExecutionEvent::RunStarted {
            run_id: run_id.clone(),
            task: "File persistence test".to_string(),
            workspace_root: None,
        };
        store
            .record_event(&cortex_core::EventRecord::new(1, ev))
            .unwrap();
    }

    // Phase 2: Reopen same database file, verify schema migrations run idempotently and data preserved
    {
        let store = RunStore::open(&db_path).unwrap();
        assert_eq!(store.schema_version().unwrap(), 1);

        let run = store.get_run(&run_id).unwrap().expect("run must persist");
        assert_eq!(run.task, "File persistence test");

        let events = store.get_events(&run_id).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 1);
        assert_eq!(events[0].event.event_type(), "RunStarted");
    }

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_failed_run_and_cancellation_tracking() {
    let store = Arc::new(RunStore::in_memory().unwrap());
    let run_id = RunId::from("run_cancelled_test");
    let mut context = AgentContext::new("Test cancellation").with_run_id(run_id.clone());

    let registry = ToolRegistry::new();
    let model = MockModelProvider::new();

    let token = CancellationToken::new();
    token.cancel();

    let agent = AgentLoop::new(5)
        .with_store(Arc::clone(&store))
        .with_cancellation_token(token);

    let err = agent.run(&mut context, &model, &registry).unwrap_err();
    match err {
        CortexError::Cancelled(_) => {}
        _ => panic!("expected Cancelled error, got {:?}", err),
    }

    let run = store.get_run(&run_id).unwrap().unwrap();
    assert_eq!(run.status, "cancelled");
    assert!(run.error.is_some());

    let events = store.get_events(&run_id).unwrap();
    assert!(events
        .iter()
        .any(|e| e.event.event_type() == "RunCancelled"));
}

#[test]
fn test_tool_failure_event_recording() {
    let store = Arc::new(RunStore::in_memory().unwrap());
    let run_id = RunId::from("run_tool_failure");
    let mut context = AgentContext::new("Test tool failure").with_run_id(run_id.clone());

    let registry = ToolRegistry::new();
    registry.register_tool(FailingTool).unwrap();

    let model = MockModelProvider::new();
    model.queue_response(ModelOutput::ToolCalls(vec![ToolCall::new(
        "fail_1",
        "failing_tool",
        json!({}),
    )]));
    model.queue_response(ModelOutput::FinalAnswer("Handled failure".to_string()));

    let agent = AgentLoop::new(5).with_store(Arc::clone(&store));
    let res = agent.run(&mut context, &model, &registry).unwrap();
    assert!(res.completed);

    let events = store.get_events(&run_id).unwrap();
    let failed_event = events
        .iter()
        .find(|e| e.event.event_type() == "ToolFailed")
        .expect("ToolFailed event must be recorded");

    match &failed_event.event {
        ExecutionEvent::ToolFailed {
            error, tool_name, ..
        } => {
            assert_eq!(tool_name, "failing_tool");
            assert!(error.contains("command crashed"));
        }
        _ => panic!("expected ToolFailed variant"),
    }
}
