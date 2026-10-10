//! Multi-agent resilience test suite covering cycles, failed workers,
//! backpressure, cancellation, and sequenced handoffs.

use cortex_core::{AgentId, CortexError, RunId};
use cortex_runtime::{
    AgentEndpoint, AgentManager, AgentManifest, AgentMessagePayload, AgentModelConfig,
    AgentPermissions, BusMessage, DeadLetterReason, MessageBus, MessageType, RoutingKey,
};

fn work_key() -> RoutingKey {
    RoutingKey::new("work").unwrap()
}

fn test_run() -> RunId {
    RunId::from("resilience_run")
}

fn task_req(id: &str, instructions: &str) -> AgentMessagePayload {
    AgentMessagePayload::TaskRequest {
        task_id: id.into(),
        instructions: instructions.into(),
    }
}

fn task_res(id: &str, output: &str) -> AgentMessagePayload {
    AgentMessagePayload::TaskResult {
        task_id: id.into(),
        output: output.into(),
    }
}

fn setup_agent(manager: &AgentManager, name: &str, capacity: usize) -> AgentEndpoint {
    let manifest = AgentManifest::new(
        name,
        "specialist",
        ".",
        AgentModelConfig::new("mock", "mock"),
    )
    .with_id(AgentId::from(name))
    .with_permissions(AgentPermissions::read_only());

    let agent = manager.create(manifest).unwrap();
    manager.start(&agent.id).unwrap();
    manager
        .connect_agent(&agent.id, vec![work_key()], capacity)
        .unwrap()
}

#[test]
fn test_sequenced_four_agent_handoff() {
    let manager = AgentManager::new();
    let mut supervisor = setup_agent(&manager, "manager", 10);
    let mut researcher = setup_agent(&manager, "researcher", 10);
    let mut coder = setup_agent(&manager, "coder", 10);
    let mut reviewer = setup_agent(&manager, "reviewer", 10);

    let run_id = test_run();

    // Configure hierarchy: manager supervisors researcher, coder, and reviewer
    manager
        .assign_worker(supervisor.agent_id(), researcher.agent_id())
        .unwrap();
    manager
        .assign_worker(supervisor.agent_id(), coder.agent_id())
        .unwrap();
    manager
        .assign_worker(supervisor.agent_id(), reviewer.agent_id())
        .unwrap();

    // Stage 1: Manager delegates research to researcher
    supervisor
        .send(
            researcher.agent_id(),
            &run_id,
            work_key(),
            task_req("task-01", "research auth pattern"),
        )
        .unwrap();

    let req1 = researcher
        .try_recv()
        .unwrap()
        .expect("researcher inbox message");
    assert_eq!(&req1.sender, supervisor.agent_id());
    assert_eq!(req1.payload, task_req("task-01", "research auth pattern"));

    // Researcher replies with findings
    researcher
        .send(
            supervisor.agent_id(),
            &run_id,
            work_key(),
            task_res("task-01", "use JWT with Ed25519"),
        )
        .unwrap();

    let res1 = supervisor
        .try_recv()
        .unwrap()
        .expect("supervisor receives research");
    assert_eq!(&res1.sender, researcher.agent_id());

    // Stage 2: Manager delegates implementation to coder
    supervisor
        .send(
            coder.agent_id(),
            &run_id,
            work_key(),
            task_req("task-02", "implement Ed25519 auth verification"),
        )
        .unwrap();

    let req2 = coder.try_recv().unwrap().expect("coder inbox message");
    assert_eq!(&req2.sender, supervisor.agent_id());

    coder
        .send(
            supervisor.agent_id(),
            &run_id,
            work_key(),
            task_res("task-02", "implemented in src/auth.rs"),
        )
        .unwrap();

    let res2 = supervisor
        .try_recv()
        .unwrap()
        .expect("supervisor receives code");
    assert_eq!(&res2.sender, coder.agent_id());

    // Stage 3: Manager delegates review to reviewer
    supervisor
        .send(
            reviewer.agent_id(),
            &run_id,
            work_key(),
            task_req("task-03", "review src/auth.rs for boundary safety"),
        )
        .unwrap();

    let req3 = reviewer
        .try_recv()
        .unwrap()
        .expect("reviewer inbox message");
    assert_eq!(&req3.sender, supervisor.agent_id());

    reviewer
        .send(
            supervisor.agent_id(),
            &run_id,
            work_key(),
            task_res("task-03", "LGTM all tests pass"),
        )
        .unwrap();

    let res3 = supervisor
        .try_recv()
        .unwrap()
        .expect("supervisor receives review");
    assert_eq!(&res3.sender, reviewer.agent_id());

    // Finalize run
    manager.finish_coordination_run(&run_id).unwrap();
}

#[test]
fn test_missing_recipient_error_handling() {
    let manager = AgentManager::new();
    let sender = setup_agent(&manager, "sender", 5);
    let ghost = AgentId::from("non-existent-agent");

    let result = sender.send(
        &ghost,
        &test_run(),
        work_key(),
        task_req("task-x", "do work"),
    );

    assert!(
        matches!(result, Err(CortexError::NotFound(_))),
        "Expected NotFound error for missing recipient, got {:?}",
        result
    );
}

#[test]
fn test_cyclic_delegation_rejection() {
    let manager = AgentManager::new();
    let a = setup_agent(&manager, "agent-a", 5);
    let b = setup_agent(&manager, "agent-b", 5);
    let c = setup_agent(&manager, "agent-c", 5);

    // a -> b
    manager.assign_worker(a.agent_id(), b.agent_id()).unwrap();
    // b -> c
    manager.assign_worker(b.agent_id(), c.agent_id()).unwrap();

    // Direct cycle: b -> a must be rejected
    let err_direct = manager.assign_worker(b.agent_id(), a.agent_id());
    assert!(
        err_direct.is_err(),
        "Direct supervisor cycle must be rejected"
    );

    // Indirect cycle: c -> a must be rejected
    let err_indirect = manager.assign_worker(c.agent_id(), a.agent_id());
    assert!(
        err_indirect.is_err(),
        "Transitive supervisor cycle must be rejected"
    );

    // Self assignment: a -> a must be rejected
    let err_self = manager.assign_worker(a.agent_id(), a.agent_id());
    assert!(err_self.is_err(), "Self assignment must be rejected");
}

#[test]
fn test_unauthorized_third_party_delegation_rejected() {
    let manager = AgentManager::new();
    let supervisor = setup_agent(&manager, "supervisor", 5);
    let worker = setup_agent(&manager, "worker", 5);
    let rogue = setup_agent(&manager, "rogue", 5);

    manager
        .assign_worker(supervisor.agent_id(), worker.agent_id())
        .unwrap();

    // Rogue agent attempts to delegate to worker (not rogue's worker)
    let rogue_send = rogue.send(
        worker.agent_id(),
        &test_run(),
        work_key(),
        task_req("rogue-task", "bypass hierarchy"),
    );

    assert!(
        rogue_send.is_err(),
        "Unauthorized delegation from non-supervisor must be rejected"
    );
}

#[test]
fn test_worker_failure_and_state_invalidation() {
    let manager = AgentManager::new();
    let supervisor = setup_agent(&manager, "supervisor", 5);
    let worker = setup_agent(&manager, "worker", 5);
    let worker_id = worker.agent_id().clone();
    let run_id = test_run();

    manager
        .assign_worker(supervisor.agent_id(), &worker_id)
        .unwrap();

    // Delegate task
    supervisor
        .send(
            &worker_id,
            &run_id,
            work_key(),
            task_req("task-fail-test", "flaky operation"),
        )
        .unwrap();

    // Worker fails mid-task
    manager
        .fail(&worker_id, "out of memory simulation")
        .unwrap();

    // Sending to failed worker must be rejected
    let send_failed = supervisor.send(
        &worker_id,
        &run_id,
        work_key(),
        task_req("task-post-fail", "try again"),
    );
    assert!(
        send_failed.is_err(),
        "Sending to failed worker must be rejected"
    );

    // Failed worker's endpoint is revoked and cannot send
    let worker_send_after_fail = worker.send(
        supervisor.agent_id(),
        &run_id,
        work_key(),
        task_res("task-fail-test", "attempt late result"),
    );
    assert!(
        worker_send_after_fail.is_err(),
        "Revoked worker endpoint cannot send replies"
    );

    // Recovery: restart and start worker, connect new endpoint
    manager.restart(&worker_id).unwrap();
    manager.start(&worker_id).unwrap();
    let mut recovered_worker = manager
        .connect_agent(&worker_id, vec![work_key()], 5)
        .unwrap();

    // Supervisor can now delegate a new recovery task
    supervisor
        .send(
            &worker_id,
            &run_id,
            work_key(),
            task_req("task-retry", "resumed operation"),
        )
        .unwrap();

    let retry_msg = recovered_worker
        .try_recv()
        .unwrap()
        .expect("received retry task");
    assert_eq!(&retry_msg.sender, supervisor.agent_id());

    // Recovered worker completes task
    recovered_worker
        .send(
            supervisor.agent_id(),
            &run_id,
            work_key(),
            task_res("task-retry", "operation succeeded"),
        )
        .unwrap();

    // Finalize run cleanly
    manager.finish_coordination_run(&run_id).unwrap();
}

#[test]
fn test_high_throughput_backpressure_and_queue_saturation() {
    let manager = AgentManager::new();
    let sender = setup_agent(&manager, "producer", 2);
    let mut receiver = setup_agent(&manager, "consumer", 3);
    let run_id = test_run();

    // Fill receiver inbox to exact capacity (3)
    let m1 = sender
        .send(
            receiver.agent_id(),
            &run_id,
            work_key(),
            AgentMessagePayload::Notification {
                content: "msg-1".into(),
            },
        )
        .unwrap();
    let m2 = sender
        .send(
            receiver.agent_id(),
            &run_id,
            work_key(),
            AgentMessagePayload::Notification {
                content: "msg-2".into(),
            },
        )
        .unwrap();
    let m3 = sender
        .send(
            receiver.agent_id(),
            &run_id,
            work_key(),
            AgentMessagePayload::Notification {
                content: "msg-3".into(),
            },
        )
        .unwrap();

    // 4th message should hit backpressure limit (inbox capacity exceeded)
    let overflow = sender.send(
        receiver.agent_id(),
        &run_id,
        work_key(),
        AgentMessagePayload::Notification {
            content: "overflow".into(),
        },
    );
    assert!(
        overflow.is_err(),
        "Inbox saturation must reject additional messages with backpressure error"
    );

    // Drain one message
    let rec1 = receiver.try_recv().unwrap().expect("msg-1");
    assert_eq!(rec1.id, m1);

    // Now sending another message should succeed cleanly
    let m4 = sender
        .send(
            receiver.agent_id(),
            &run_id,
            work_key(),
            AgentMessagePayload::Notification {
                content: "msg-4".into(),
            },
        )
        .unwrap();

    let rec2 = receiver.try_recv().unwrap().expect("msg-2");
    let rec3 = receiver.try_recv().unwrap().expect("msg-3");
    let rec4 = receiver.try_recv().unwrap().expect("msg-4");

    assert_eq!(rec2.id, m2);
    assert_eq!(rec3.id, m3);
    assert_eq!(rec4.id, m4);
    assert!(receiver.try_recv().unwrap().is_none());
}

#[test]
fn test_cascading_cancellation_on_supervisor_stop() {
    let manager = AgentManager::new();
    let supervisor = setup_agent(&manager, "lead", 5);
    let worker1 = setup_agent(&manager, "w1", 5);
    let worker2 = setup_agent(&manager, "w2", 5);
    let lead_id = supervisor.agent_id().clone();
    let w1_id = worker1.agent_id().clone();
    let w2_id = worker2.agent_id().clone();
    let run_id = test_run();

    manager.assign_worker(&lead_id, &w1_id).unwrap();
    manager.assign_worker(&lead_id, &w2_id).unwrap();

    supervisor
        .send(&w1_id, &run_id, work_key(), task_req("task-w1", "job 1"))
        .unwrap();
    supervisor
        .send(&w2_id, &run_id, work_key(), task_req("task-w2", "job 2"))
        .unwrap();

    // Stop supervisor
    manager.stop(&lead_id).unwrap();

    // Supervisor endpoint is revoked
    assert!(supervisor
        .send(&w1_id, &run_id, work_key(), task_req("task-w3", "job 3"))
        .is_err());

    // Workers stop or disconnect cleanly without deadlock
    manager.stop(&w1_id).unwrap();
    manager.stop(&w2_id).unwrap();
}

#[test]
fn test_duplicate_task_id_rejection() {
    let manager = AgentManager::new();
    let supervisor = setup_agent(&manager, "lead-dup", 5);
    let worker = setup_agent(&manager, "worker-dup", 5);
    let lead_id = supervisor.agent_id().clone();
    let worker_id = worker.agent_id().clone();
    let run_id = test_run();

    manager.assign_worker(&lead_id, &worker_id).unwrap();

    // First task with ID "dup-task-1" succeeds
    supervisor
        .send(
            &worker_id,
            &run_id,
            work_key(),
            task_req("dup-task-1", "first assignment"),
        )
        .unwrap();

    // Submitting duplicate task ID in the same run must be rejected
    let dup_res = supervisor.send(
        &worker_id,
        &run_id,
        work_key(),
        task_req("dup-task-1", "second assignment with same ID"),
    );
    assert!(
        dup_res.is_err(),
        "Duplicate task ID in the same run must be rejected"
    );
    assert!(matches!(dup_res, Err(CortexError::Validation(_))));
}

#[tokio::test]
async fn test_bus_deduplication_and_dlq_routing() {
    let bus = MessageBus::new();
    let recipient_id = AgentId::from("dest_worker");
    let route = RoutingKey::new("team.task").unwrap();

    let _rx = bus
        .register_agent(recipient_id.clone(), Some(10), vec![route.clone()])
        .unwrap();

    let msg = BusMessage::new(
        "dedup-msg-99",
        AgentId::from("supervisor"),
        recipient_id.clone(),
        RunId::from("run-dedup"),
        route.clone(),
        MessageType::Request,
        serde_json::json!({"action": "run_audit"}),
    );

    // Initial send succeeds
    bus.publish(msg.clone()).await.unwrap();

    // Duplicate message submission with identical ID is rejected
    let dup_err = bus.publish(msg).await;
    assert!(dup_err.is_err());
    assert!(
        dup_err
            .unwrap_err()
            .to_string()
            .contains("duplicate message rejected"),
        "Duplicate message submission must be rejected by MessageBus"
    );

    // Unmapped recipient routes cleanly to DLQ without panic
    let ghost = AgentId::from("ghost_agent");
    let unroutable_msg = BusMessage::new(
        "dlq-msg-100",
        AgentId::from("supervisor"),
        ghost.clone(),
        RunId::from("run-dedup"),
        route.clone(),
        MessageType::Request,
        serde_json::json!({"action": "ghost_task"}),
    );

    let res = bus.publish(unroutable_msg).await;
    assert!(res.is_err(), "Publishing to unmapped agent returns error");

    assert_eq!(bus.dlq_len(), 1);
    let dlq = bus.drain_dlq();
    assert_eq!(dlq.len(), 1);
    assert_eq!(dlq[0].message.id, "dlq-msg-100");
    assert_eq!(dlq[0].reason, DeadLetterReason::RecipientNotFound(ghost));
}
