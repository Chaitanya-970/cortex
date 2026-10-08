//! Integration tests for AgentManager and lifecycle state machine.

use cortex_core::AgentId;
use cortex_runtime::agent::{
    AgentManager, AgentManifest, AgentModelConfig, AgentPermissions, AgentState,
};
use std::sync::Arc;
use std::thread;

fn make_manifest(name: &str, role: &str) -> AgentManifest {
    AgentManifest::new(
        name,
        role,
        "/tmp/cortex-agent-workspace",
        AgentModelConfig::new("anthropic", "claude-3-5-sonnet")
            .with_temperature(0.2)
            .with_max_tokens(4096),
    )
}

#[test]
fn test_manifest_validation() {
    let manager = AgentManager::new();

    // Empty name fails
    let bad_name = AgentManifest::new(
        "   ",
        "Engineer",
        "/tmp",
        AgentModelConfig::new("mock", "model"),
    );
    let err = manager.create(bad_name).unwrap_err();
    assert!(err.to_string().contains("agent name cannot be empty"));

    // Empty role fails
    let bad_role = AgentManifest::new(
        "alice",
        "   ",
        "/tmp",
        AgentModelConfig::new("mock", "model"),
    );
    let err = manager.create(bad_role).unwrap_err();
    assert!(err.to_string().contains("agent role cannot be empty"));

    // Explicit ID succeeds
    let custom_id = AgentId::from("agent_custom_001");
    let manifest = make_manifest("alice", "Engineer").with_id(custom_id.clone());
    let agent = manager.create(manifest).unwrap();
    assert_eq!(agent.id, custom_id);

    // Duplicate ID fails
    let dup_manifest = make_manifest("alice_dup", "Engineer").with_id(custom_id);
    let dup_err = manager.create(dup_manifest).unwrap_err();
    assert!(dup_err.to_string().contains("already exists"));
}

#[test]
fn test_complete_valid_lifecycle_transitions() {
    let manager = AgentManager::new();
    let rx = manager.subscribe();

    let manifest = make_manifest("lead-reviewer", "Reviewer");
    let agent = manager.create(manifest).unwrap();
    let id = agent.id;

    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Created);

    // Created -> Running
    manager.start(&id).unwrap();
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Running);

    // Running -> Paused
    manager.pause(&id).unwrap();
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Paused);

    // Paused -> Running
    manager.resume(&id).unwrap();
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Running);

    // Running -> Stopped
    manager.stop(&id).unwrap();
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Stopped);

    // Stopped -> Ready (restart)
    manager.restart(&id).unwrap();
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Ready);

    // Ready -> Running
    manager.start(&id).unwrap();
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Running);

    // Running -> Failed
    manager
        .fail(&id, "Simulated unrecoverable out-of-memory error")
        .unwrap();
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Failed);

    // Failed -> Ready (restart)
    manager.restart(&id).unwrap();
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Ready);

    // Ready -> Stopped
    manager.stop(&id).unwrap();
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Stopped);

    // Verify stream of events
    let mut event_types = Vec::new();
    while let Ok(event) = rx.try_recv() {
        assert_eq!(event.agent_id(), &id);
        event_types.push(event.event_type().to_string());
    }

    assert_eq!(
        event_types,
        vec![
            "AgentCreated",
            "AgentStarted",
            "AgentPaused",
            "AgentResumed",
            "AgentStopped",
            "AgentReady",
            "AgentStarted",
            "AgentFailed",
            "AgentReady",
            "AgentStopped",
        ]
    );
}

#[test]
fn test_prohibited_state_transitions() {
    let manager = AgentManager::new();
    let agent = manager
        .create(make_manifest("test-guard", "Tester"))
        .unwrap();
    let id = agent.id;

    // Cannot pause while Created
    let pause_err = manager.pause(&id).unwrap_err();
    assert!(pause_err
        .to_string()
        .contains("only running agents can be paused"));

    // Cannot resume while Created
    let resume_err = manager.resume(&id).unwrap_err();
    assert!(resume_err
        .to_string()
        .contains("only paused agents can be resumed"));

    // Move to Stopped
    manager.stop(&id).unwrap();

    // Stopped cannot transition directly to Running without restart
    let start_err = manager.start(&id).unwrap_err();
    assert!(start_err.to_string().contains("cannot start stopped agent"));

    // Stopped cannot pause
    let pause_stopped_err = manager.pause(&id).unwrap_err();
    assert!(pause_stopped_err
        .to_string()
        .contains("only running agents can be paused"));

    // Restart to Ready, start to Running, then fail
    manager.restart(&id).unwrap();
    manager.start(&id).unwrap();
    manager.fail(&id, "fatal error").unwrap();

    // Failed cannot start directly without restart
    let start_failed_err = manager.start(&id).unwrap_err();
    assert!(start_failed_err
        .to_string()
        .contains("cannot start failed agent"));
}

#[test]
fn test_agent_removal_lifecycle_rules() {
    let manager = AgentManager::new();
    let agent = manager
        .create(make_manifest("deletable", "Janitor"))
        .unwrap();
    let id = agent.id.clone();

    // Created agent can be removed
    assert!(manager.remove(&id).is_ok());
    assert!(manager.inspect(&id).is_err());

    // Re-create and start
    let agent2 = manager
        .create(make_manifest("running-agent", "Worker"))
        .unwrap();
    let id2 = agent2.id.clone();
    manager.start(&id2).unwrap();

    // Active Running agent cannot be removed
    let err = manager.remove(&id2).unwrap_err();
    assert!(err.to_string().contains("cannot remove agent"));

    // Active Paused agent cannot be removed
    manager.pause(&id2).unwrap();
    let err_paused = manager.remove(&id2).unwrap_err();
    assert!(err_paused.to_string().contains("cannot remove agent"));

    // Once Stopped, agent can be removed cleanly
    manager.stop(&id2).unwrap();
    assert!(manager.remove(&id2).is_ok());
    assert!(manager.inspect(&id2).is_err());
}

#[test]
fn test_multi_agent_concurrent_execution() {
    let manager = Arc::new(AgentManager::new());
    let mut handles = Vec::new();

    for i in 0..20 {
        let m = Arc::clone(&manager);
        handles.push(thread::spawn(move || {
            let manifest = make_manifest(&format!("worker_{:02}", i), "Autonomous Unit")
                .with_permissions(AgentPermissions::standard());
            let agent = m.create(manifest).unwrap();
            let id = agent.id;

            m.start(&id).unwrap();
            m.pause(&id).unwrap();
            m.resume(&id).unwrap();
            m.inspect(&id).unwrap();
            m.stop(&id).unwrap();
            m.restart(&id).unwrap();
            m.start(&id).unwrap();
            m.stop(&id).unwrap();
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let agents = manager.list();
    assert_eq!(agents.len(), 20);
    for a in agents {
        assert_eq!(a.state, AgentState::Stopped);
    }
}

#[test]
fn test_complete_state_transition_matrix_36_pairs() {
    let all_states = [
        AgentState::Created,
        AgentState::Ready,
        AgentState::Running,
        AgentState::Paused,
        AgentState::Stopped,
        AgentState::Failed,
    ];

    for &from in &all_states {
        for &to in &all_states {
            let mut current = from;
            let allowed = current.can_transition_to(to);
            let res = current.transition_to(to);

            if allowed {
                assert!(
                    res.is_ok(),
                    "Transition from {:?} to {:?} was expected to succeed but failed",
                    from,
                    to
                );
                assert_eq!(current, to);
            } else {
                assert!(
                    res.is_err(),
                    "Transition from {:?} to {:?} was expected to fail but succeeded",
                    from,
                    to
                );
                assert_eq!(current, from, "State should remain unchanged on failure");
            }
        }
    }
}

#[test]
fn test_idempotent_operations() {
    let manager = AgentManager::new();
    let agent = manager
        .create(make_manifest("idempotent", "Worker"))
        .unwrap();
    let id = agent.id;

    // Idempotent prepare
    assert!(manager.prepare(&id).is_ok());
    assert!(manager.prepare(&id).is_ok());
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Ready);

    // Idempotent start
    assert!(manager.start(&id).is_ok());
    assert!(manager.start(&id).is_ok());
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Running);

    // Idempotent pause
    assert!(manager.pause(&id).is_ok());
    assert!(manager.pause(&id).is_ok());
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Paused);

    // Idempotent resume
    assert!(manager.resume(&id).is_ok());
    assert!(manager.resume(&id).is_ok());
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Running);

    // Idempotent stop
    assert!(manager.stop(&id).is_ok());
    assert!(manager.stop(&id).is_ok());
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Stopped);
}

#[test]
fn test_nonexistent_agent_operations() {
    let manager = AgentManager::new();
    let unknown = AgentId::from("nonexistent_agent_999");

    assert!(manager.inspect(&unknown).is_err());
    assert!(manager.prepare(&unknown).is_err());
    assert!(manager.start(&unknown).is_err());
    assert!(manager.pause(&unknown).is_err());
    assert!(manager.resume(&unknown).is_err());
    assert!(manager.stop(&unknown).is_err());
    assert!(manager.restart(&unknown).is_err());
    assert!(manager.fail(&unknown, "some error").is_err());
    assert!(manager.remove(&unknown).is_err());

    // Querying events for non-existent agent returns empty vector without error
    let events = manager.events_for_agent(&unknown);
    assert!(events.is_empty());
}

#[test]
fn test_manifest_serialization_json_and_toml_roundtrip() {
    let mut manifest = make_manifest("serializer-test", "Compiler Engineer")
        .with_id(AgentId::from("agent_manifest_roundtrip"))
        .with_system_prompt("You are a specialized compiler engineer.")
        .with_tools(vec!["rustc".to_string(), "cargo_check".to_string()])
        .with_permissions(AgentPermissions::standard());

    manifest.model.parameters = Some(serde_json::json!({
        "top_p": 0.95,
        "seed": 42
    }));

    // JSON round-trip
    let json_str = manifest.to_json_string().expect("serialize json");
    let from_json = AgentManifest::from_json_str(&json_str).expect("deserialize json");
    assert_eq!(manifest, from_json);

    // TOML round-trip
    let toml_str = manifest.to_toml_string().expect("serialize toml");
    let from_toml = AgentManifest::from_toml_str(&toml_str).expect("deserialize toml");
    assert_eq!(manifest, from_toml);

    // Invalid JSON validation error
    assert!(AgentManifest::from_json_str("{ invalid_json }").is_err());

    // Invalid TOML validation error
    assert!(AgentManifest::from_toml_str("invalid = [to m l").is_err());
}

#[test]
fn test_subscriber_disconnection_resilience() {
    let manager = AgentManager::new();

    let rx1 = manager.subscribe();
    let rx2 = manager.subscribe();

    // Drop rx1 immediately to simulate a disconnected client
    drop(rx1);

    // Perform agent operations; manager should not panic and should clean up rx1
    let agent = manager
        .create(make_manifest("disconnect-test", "Resilience"))
        .unwrap();
    manager.start(&agent.id).unwrap();
    manager.stop(&agent.id).unwrap();

    // rx2 should still receive all three events
    let ev1 = rx2.recv().unwrap();
    assert_eq!(ev1.event_type(), "AgentCreated");

    let ev2 = rx2.recv().unwrap();
    assert_eq!(ev2.event_type(), "AgentStarted");

    let ev3 = rx2.recv().unwrap();
    assert_eq!(ev3.event_type(), "AgentStopped");
}

#[test]
fn test_heavy_concurrency_same_agent_race_conditions() {
    let manager = Arc::new(AgentManager::new());
    let agent = manager
        .create(make_manifest("hot-agent", "Concurrent Target"))
        .unwrap();
    let id = Arc::new(agent.id);

    let mut handles = Vec::new();

    // Spawn 30 threads all attempting competing transitions on the SAME agent
    for i in 0..30 {
        let m = Arc::clone(&manager);
        let agent_id = Arc::clone(&id);

        handles.push(thread::spawn(move || match i % 4 {
            0 => {
                let _ = m.start(&agent_id);
            }
            1 => {
                let _ = m.pause(&agent_id);
            }
            2 => {
                let _ = m.resume(&agent_id);
            }
            _ => {
                let _ = m.inspect(&agent_id);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    // Inspect final state: must be valid without corrupting memory
    let inspected = manager.inspect(&id).unwrap();
    assert!(
        inspected.state == AgentState::Running
            || inspected.state == AgentState::Paused
            || inspected.state == AgentState::Created
    );

    // Cleanly stopping the hot agent must succeed
    assert!(manager.stop(&id).is_ok());
    assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Stopped);
}

#[test]
fn test_event_history_filtering_and_ordering() {
    let manager = AgentManager::new();

    let a1 = manager
        .create(make_manifest("agent-1", "Worker 1"))
        .unwrap();
    let a2 = manager
        .create(make_manifest("agent-2", "Worker 2"))
        .unwrap();
    let a3 = manager
        .create(make_manifest("agent-3", "Worker 3"))
        .unwrap();

    manager.start(&a1.id).unwrap();
    manager.start(&a2.id).unwrap();
    manager.stop(&a1.id).unwrap();
    manager.start(&a3.id).unwrap();
    manager.stop(&a2.id).unwrap();
    manager.stop(&a3.id).unwrap();

    let all_events = manager.events();
    assert_eq!(all_events.len(), 9); // 3 creates + 3 starts + 3 stops

    let a1_events = manager.events_for_agent(&a1.id);
    assert_eq!(a1_events.len(), 3);
    assert_eq!(a1_events[0].event_type(), "AgentCreated");
    assert_eq!(a1_events[1].event_type(), "AgentStarted");
    assert_eq!(a1_events[2].event_type(), "AgentStopped");

    let a2_events = manager.events_for_agent(&a2.id);
    assert_eq!(a2_events.len(), 3);

    let a3_events = manager.events_for_agent(&a3.id);
    assert_eq!(a3_events.len(), 3);
}
