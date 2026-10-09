//! Integration tests for multi-agent message persistence, secret redaction,
//! and deterministic execution replay from SQLite RunStore.

use cortex_core::{AgentId, ExecutionEvent, RunId};
use cortex_runtime::agent::{AgentMessage, AgentMessagePayload, RoutingKey};
use cortex_runtime::storage::{RunStore, SecretRedactor};
use std::sync::Arc;

fn temp_db_path(name: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "cortex_multi_agent_{}_{}",
        std::process::id(),
        name
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("cortex.db");
    (dir, db)
}

#[test]
fn test_multi_agent_message_lifecycle_and_sqlite_round_trip() {
    let (dir, db_path) = temp_db_path("lifecycle");

    let run_id = RunId::from("run_lifecycle_001");
    let supervisor = AgentId::from("supervisor");
    let coder = AgentId::from("coder");
    let reviewer = AgentId::from("reviewer");

    let route_task = RoutingKey::new("workflow.task").unwrap();
    let route_review = RoutingKey::new("workflow.review").unwrap();

    // 1. Open persistent SQLite database and record messages
    {
        let store = RunStore::open(&db_path).expect("open persistent store");
        store
            .record_run_start(
                &run_id,
                "Multi-agent implementation task",
                "2026-10-10T00:00:00Z",
            )
            .expect("record run start");

        let msg1 = AgentMessage {
            id: "msg_init_task".into(),
            run_id: run_id.clone(),
            sender: supervisor.clone(),
            recipient: coder.clone(),
            routing_key: route_task.clone(),
            timestamp: "2026-10-10T00:00:01Z".into(),
            payload: AgentMessagePayload::TaskRequest {
                task_id: "task-01".into(),
                instructions: "Write QuickSort algorithm in Rust".into(),
            },
        };

        let msg2 = AgentMessage {
            id: "msg_task_done".into(),
            run_id: run_id.clone(),
            sender: coder.clone(),
            recipient: supervisor.clone(),
            routing_key: route_task.clone(),
            timestamp: "2026-10-10T00:00:02Z".into(),
            payload: AgentMessagePayload::TaskResult {
                task_id: "task-01".into(),
                output: "fn quicksort<T: Ord>(slice: &mut [T]) { ... }".into(),
            },
        };

        let msg3 = AgentMessage {
            id: "msg_review_req".into(),
            run_id: run_id.clone(),
            sender: supervisor.clone(),
            recipient: reviewer.clone(),
            routing_key: route_review.clone(),
            timestamp: "2026-10-10T00:00:03Z".into(),
            payload: AgentMessagePayload::TaskRequest {
                task_id: "task-02".into(),
                instructions: "Review quicksort for boundary conditions and panic safety".into(),
            },
        };

        let msg4 = AgentMessage {
            id: "msg_review_done".into(),
            run_id: run_id.clone(),
            sender: reviewer.clone(),
            recipient: supervisor.clone(),
            routing_key: route_review.clone(),
            timestamp: "2026-10-10T00:00:04Z".into(),
            payload: AgentMessagePayload::Notification {
                content: "All tests pass, code is approved".into(),
            },
        };

        store
            .record_messages(&[msg1, msg2, msg3, msg4])
            .expect("record messages");
    }

    // 2. Re-open SQLite database from disk to verify restart persistence
    {
        let store = RunStore::open(&db_path).expect("re-open persistent store");
        assert_eq!(store.schema_version().unwrap(), 3);

        let run_messages = store
            .get_messages_for_run(&run_id)
            .expect("query run messages");
        assert_eq!(run_messages.len(), 4);

        assert_eq!(run_messages[0].id, "msg_init_task");
        assert_eq!(run_messages[0].sender, supervisor);
        assert_eq!(run_messages[0].recipient, coder);

        assert_eq!(run_messages[1].id, "msg_task_done");
        assert_eq!(run_messages[1].sender, coder);
        assert_eq!(run_messages[1].recipient, supervisor);

        assert_eq!(run_messages[2].id, "msg_review_req");
        assert_eq!(run_messages[2].sender, supervisor);
        assert_eq!(run_messages[2].recipient, reviewer);

        assert_eq!(run_messages[3].id, "msg_review_done");
        assert_eq!(run_messages[3].sender, reviewer);
        assert_eq!(run_messages[3].recipient, supervisor);

        // Verify task-specific queries
        let task1_msgs = store
            .get_messages_for_task("task-01")
            .expect("query task-01");
        assert_eq!(task1_msgs.len(), 2);
        assert_eq!(task1_msgs[0].id, "msg_init_task");
        assert_eq!(task1_msgs[1].id, "msg_task_done");

        // Verify pairwise queries
        let sup_coder_msgs = store
            .get_messages_between(&supervisor, &coder)
            .expect("query between supervisor and coder");
        assert_eq!(sup_coder_msgs.len(), 2);

        let sup_reviewer_msgs = store
            .get_messages_between(&supervisor, &reviewer)
            .expect("query between supervisor and reviewer");
        assert_eq!(sup_reviewer_msgs.len(), 2);

        let coder_reviewer_msgs = store
            .get_messages_between(&coder, &reviewer)
            .expect("query between coder and reviewer");
        assert!(coder_reviewer_msgs.is_empty());
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_secret_redaction_before_writing_to_disk() {
    let (dir, db_path) = temp_db_path("redaction");

    let run_id = RunId::from("run_redaction_002");
    let redactor = SecretRedactor::new();
    let store = RunStore::open(&db_path)
        .expect("open persistent store")
        .with_redactor(redactor);

    let openai_key = "sk-live1234567890abcdef1234567890";
    let github_pat = "ghp_1234567890abcdef1234567890abcdef1234";
    let aws_access = "AKIAIOSFODNN7EXAMPLE";

    let sensitive_instructions = format!(
        "Deploy cluster using OpenAI key: {}, GitHub token: {}, AWS key: {}",
        openai_key, github_pat, aws_access
    );

    let msg = AgentMessage {
        id: "msg_secure_dispatch".into(),
        run_id: run_id.clone(),
        sender: AgentId::from("supervisor"),
        recipient: AgentId::from("worker"),
        routing_key: RoutingKey::new("secure.dispatch").unwrap(),
        timestamp: "2026-10-10T00:00:01Z".into(),
        payload: AgentMessagePayload::TaskRequest {
            task_id: "sec-task-1".into(),
            instructions: sensitive_instructions,
        },
    };

    store
        .record_message(&msg)
        .expect("record message with secret");

    // 1. Verify through RunStore query API that payloads are redacted
    let fetched = store.get_messages_for_run(&run_id).expect("get messages");
    assert_eq!(fetched.len(), 1);

    if let AgentMessagePayload::TaskRequest { instructions, .. } = &fetched[0].payload {
        assert!(!instructions.contains(openai_key));
        assert!(!instructions.contains(github_pat));
        assert!(!instructions.contains(aws_access));
        assert!(instructions.contains("[REDACTED]"));
    } else {
        panic!("expected TaskRequest payload");
    }

    // 2. Query SQLite raw bytes directly from the file to confirm zero secrets on disk
    let raw_conn = rusqlite::Connection::open(&db_path).expect("open raw sqlite");
    let raw_payload: String = raw_conn
        .query_row(
            "SELECT payload FROM inter_agent_messages WHERE id = 'msg_secure_dispatch'",
            [],
            |row| row.get(0),
        )
        .expect("query raw payload");

    assert!(!raw_payload.contains(openai_key));
    assert!(!raw_payload.contains(github_pat));
    assert!(!raw_payload.contains(aws_access));
    assert!(raw_payload.contains("[REDACTED]"));

    drop(raw_conn);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_multi_agent_deterministic_replay_reconstruction() {
    let store = RunStore::in_memory().expect("in-memory store");
    let run_id = RunId::from("run_replay_reconstruct");

    let manager_id = AgentId::from("manager");
    let worker_1 = AgentId::from("worker_1");
    let worker_2 = AgentId::from("worker_2");

    let route_pipe = RoutingKey::new("pipeline.step").unwrap();

    // Generate a multi-step orchestrated workflow execution stream
    let original_stream = vec![
        AgentMessage {
            id: "step_1_req".into(),
            run_id: run_id.clone(),
            sender: manager_id.clone(),
            recipient: worker_1.clone(),
            routing_key: route_pipe.clone(),
            timestamp: "2026-10-10T00:01:00Z".into(),
            payload: AgentMessagePayload::TaskRequest {
                task_id: "t_data_fetch".into(),
                instructions: "Fetch dataset from remote endpoint".into(),
            },
        },
        AgentMessage {
            id: "step_1_res".into(),
            run_id: run_id.clone(),
            sender: worker_1.clone(),
            recipient: manager_id.clone(),
            routing_key: route_pipe.clone(),
            timestamp: "2026-10-10T00:01:05Z".into(),
            payload: AgentMessagePayload::TaskResult {
                task_id: "t_data_fetch".into(),
                output: "1000 records loaded successfully".into(),
            },
        },
        AgentMessage {
            id: "step_2_req".into(),
            run_id: run_id.clone(),
            sender: manager_id.clone(),
            recipient: worker_2.clone(),
            routing_key: route_pipe.clone(),
            timestamp: "2026-10-10T00:01:10Z".into(),
            payload: AgentMessagePayload::TaskRequest {
                task_id: "t_data_transform".into(),
                instructions: "Run data normalization on dataset".into(),
            },
        },
        AgentMessage {
            id: "step_2_err".into(),
            run_id: run_id.clone(),
            sender: worker_2.clone(),
            recipient: manager_id.clone(),
            routing_key: route_pipe.clone(),
            timestamp: "2026-10-10T00:01:15Z".into(),
            payload: AgentMessagePayload::TaskFailed {
                task_id: "t_data_transform".into(),
                error: "Out of memory in matrix normalization".into(),
            },
        },
        AgentMessage {
            id: "step_3_notice".into(),
            run_id: run_id.clone(),
            sender: manager_id.clone(),
            recipient: worker_1.clone(),
            routing_key: route_pipe.clone(),
            timestamp: "2026-10-10T00:01:20Z".into(),
            payload: AgentMessagePayload::Notification {
                content: "Aborting pipeline run due to transform failure".into(),
            },
        },
    ];

    store
        .record_messages(&original_stream)
        .expect("record stream");

    // Replay reconstruction: retrieve all messages from SQLite
    let replayed_stream = store
        .get_messages_for_run(&run_id)
        .expect("get replayed messages");

    assert_eq!(replayed_stream.len(), original_stream.len());

    // Deterministic verification: every message preserves ID, participants, routing, and payload
    for (i, (orig, rep)) in original_stream
        .iter()
        .zip(replayed_stream.iter())
        .enumerate()
    {
        assert_eq!(orig.id, rep.id, "Step {} message ID mismatch", i);
        assert_eq!(orig.run_id, rep.run_id, "Step {} run ID mismatch", i);
        assert_eq!(orig.sender, rep.sender, "Step {} sender mismatch", i);
        assert_eq!(
            orig.recipient, rep.recipient,
            "Step {} recipient mismatch",
            i
        );
        assert_eq!(
            orig.routing_key, rep.routing_key,
            "Step {} routing key mismatch",
            i
        );
        assert_eq!(
            orig.timestamp, rep.timestamp,
            "Step {} timestamp mismatch",
            i
        );
        assert_eq!(orig.payload, rep.payload, "Step {} payload mismatch", i);
    }

    // Verify conversions between AgentMessage and ExecutionEvent
    for msg in &replayed_stream {
        let event = ExecutionEvent::from(msg);
        assert_eq!(event.event_type(), "InterAgentMessage");

        let converted_back = AgentMessage::try_from(&event).expect("convert back to AgentMessage");
        assert_eq!(converted_back.id, msg.id);
        assert_eq!(converted_back.run_id, msg.run_id);
        assert_eq!(converted_back.sender, msg.sender);
        assert_eq!(converted_back.recipient, msg.recipient);
        assert_eq!(converted_back.payload, msg.payload);
    }
}

#[test]
fn test_concurrent_message_recording_no_locks_or_data_races() {
    let (dir, db_path) = temp_db_path("concurrent");

    let store = Arc::new(RunStore::open(&db_path).expect("open concurrent store"));
    let run_id = RunId::from("run_concurrent_stress");

    store
        .record_run_start(&run_id, "Concurrent stress test", "2026-10-10T00:00:00Z")
        .expect("record run start");

    let mut handles = Vec::new();
    let thread_count = 8;
    let messages_per_thread = 50;

    for thread_idx in 0..thread_count {
        let store_clone = Arc::clone(&store);
        let run_clone = run_id.clone();

        handles.push(std::thread::spawn(move || {
            let sender = AgentId::from(format!("agent_sender_{}", thread_idx));
            let recipient = AgentId::from(format!("agent_recv_{}", thread_idx));
            let route = RoutingKey::new("stress.event").unwrap();

            for msg_idx in 0..messages_per_thread {
                let msg = AgentMessage {
                    id: format!("msg_{}_{}", thread_idx, msg_idx),
                    run_id: run_clone.clone(),
                    sender: sender.clone(),
                    recipient: recipient.clone(),
                    routing_key: route.clone(),
                    timestamp: format!("2026-10-10T00:00:{:02}Z", msg_idx % 60),
                    payload: AgentMessagePayload::TaskRequest {
                        task_id: format!("task_{}_{}", thread_idx, msg_idx),
                        instructions: format!("Concurrent payload {}", msg_idx),
                    },
                };

                store_clone
                    .record_message(&msg)
                    .expect("concurrent record message");
            }
        }));
    }

    for handle in handles {
        handle.join().expect("thread join");
    }

    let all_messages = store
        .get_messages_for_run(&run_id)
        .expect("query all concurrent messages");
    assert_eq!(
        all_messages.len(),
        thread_count * messages_per_thread,
        "All concurrent messages must be persisted without loss or lock failure"
    );

    drop(store);
    let _ = std::fs::remove_dir_all(&dir);
}
