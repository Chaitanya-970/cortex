use cortex_core::{AgentId, CortexError, EventRecord, ExecutionEvent, RunId};
use cortex_runtime::{
    AgentEndpoint, AgentManager, AgentManifest, AgentMessagePayload, AgentModelConfig,
    AgentPermissions, RoutingKey, RunStore,
};
use std::future::Future;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Barrier,
};
use std::task::{Context, Poll, Wake, Waker};

fn key() -> RoutingKey {
    RoutingKey::new("work").unwrap()
}
fn run() -> RunId {
    RunId::from("coordination_test")
}
fn notice(text: &str) -> AgentMessagePayload {
    AgentMessagePayload::Notification {
        content: text.into(),
    }
}
fn request(id: &str) -> AgentMessagePayload {
    AgentMessagePayload::TaskRequest {
        task_id: id.into(),
        instructions: "inspect repository".into(),
    }
}
fn result(id: &str) -> AgentMessagePayload {
    AgentMessagePayload::TaskResult {
        task_id: id.into(),
        output: "done".into(),
    }
}
fn endpoint(manager: &AgentManager, name: &str, capacity: usize) -> AgentEndpoint {
    let agent = manager
        .create(
            AgentManifest::new(
                name,
                "specialist",
                ".",
                AgentModelConfig::new("mock", "mock"),
            )
            .with_id(AgentId::from(name))
            .with_permissions(AgentPermissions::read_only()),
        )
        .unwrap();
    manager.start(&agent.id).unwrap();
    manager
        .connect_agent(&agent.id, vec![key()], capacity)
        .unwrap()
}

#[derive(Default)]
struct WakeCount(AtomicUsize);
impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn bounded_fifo_routing_and_sender_identity() {
    let manager = AgentManager::new();
    let a = endpoint(&manager, "a", 2);
    let mut b = endpoint(&manager, "b", 2);
    let missing = AgentId::from("missing");
    assert!(matches!(
        a.send(&missing, &run(), key(), notice("x")),
        Err(CortexError::NotFound(_))
    ));
    assert!(a
        .send(
            b.agent_id(),
            &run(),
            RoutingKey::new("other").unwrap(),
            notice("x")
        )
        .is_err());
    let first = a
        .send(b.agent_id(), &run(), key(), notice("first"))
        .unwrap();
    let second = a
        .send(b.agent_id(), &run(), key(), notice("second"))
        .unwrap();
    assert_ne!(first, second);
    assert!(a
        .send(b.agent_id(), &run(), key(), notice("overflow"))
        .is_err());
    let message = b.try_recv().unwrap().unwrap();
    assert_eq!(message.id, first);
    assert_eq!(&message.sender, a.agent_id());
    assert_eq!(&message.recipient, b.agent_id());
    assert_eq!(message.payload, notice("first"));
    assert!(!message.timestamp.is_empty());
    assert_eq!(b.try_recv().unwrap().unwrap().id, second);
    assert!(b.try_recv().unwrap().is_none());
    assert_eq!(manager.drain_message_events().unwrap().len(), 2);
}

#[test]
fn asynchronous_receive_is_woken_by_delivery_and_preserves_cancelled_future_messages() {
    let manager = AgentManager::new();
    let a = endpoint(&manager, "a", 2);
    let mut b = endpoint(&manager, "b", 2);
    let id = b.agent_id().clone();
    let count = Arc::new(WakeCount::default());
    let waker = Waker::from(count.clone());
    let mut cx = Context::from_waker(&waker);
    {
        let mut future = std::pin::pin!(b.recv());
        assert!(future.as_mut().poll(&mut cx).is_pending());
        a.send(&id, &run(), key(), notice("delivered")).unwrap();
        assert_eq!(count.0.load(Ordering::SeqCst), 1);
        assert!(
            matches!(future.as_mut().poll(&mut cx), Poll::Ready(Ok(message)) if message.payload == notice("delivered"))
        );
    }
    {
        let mut future = std::pin::pin!(b.recv());
        assert!(future.as_mut().poll(&mut cx).is_pending());
    }
    a.send(&id, &run(), key(), notice("still queued")).unwrap();
    assert_eq!(
        b.try_recv().unwrap().unwrap().payload,
        notice("still queued")
    );
}

#[test]
fn lifecycle_changes_wake_receivers_and_revoke_old_endpoints() {
    for action in ["stop", "fail", "restart", "prepare", "pause"] {
        let manager = AgentManager::new();
        let a = endpoint(&manager, "a", 2);
        let mut b = endpoint(&manager, "b", 2);
        let id = b.agent_id().clone();
        let count = Arc::new(WakeCount::default());
        let waker = Waker::from(count.clone());
        let mut cx = Context::from_waker(&waker);
        {
            let mut future = std::pin::pin!(b.recv());
            assert!(future.as_mut().poll(&mut cx).is_pending());
            match action {
                "stop" => manager.stop(&id).unwrap(),
                "fail" => manager.fail(&id, "crash").unwrap(),
                "restart" => manager.restart(&id).unwrap(),
                "prepare" => manager.prepare(&id).unwrap(),
                "pause" => manager.pause(&id).unwrap(),
                _ => unreachable!(),
            }
            assert_eq!(count.0.load(Ordering::SeqCst), 1);
            assert!(matches!(future.as_mut().poll(&mut cx), Poll::Ready(Err(_))));
        }
        assert!(a.send(&id, &run(), key(), notice("inactive")).is_err());
        if action == "pause" {
            manager.resume(&id).unwrap();
        } else {
            if action == "stop" || action == "fail" {
                manager.restart(&id).unwrap();
            }
            manager.start(&id).unwrap();
            let mut replacement = manager.connect_agent(&id, vec![key()], 2).unwrap();
            assert!(b
                .send(a.agent_id(), &run(), key(), notice("stale"))
                .is_err());
            drop(b); // A stale endpoint must not close its replacement.
            a.send(&id, &run(), key(), notice("new incarnation"))
                .unwrap();
            assert!(replacement.try_recv().unwrap().is_some());
        }
    }
}

#[test]
fn hierarchy_and_task_correlation_enforce_ownership_and_single_completion() {
    let manager = AgentManager::new();
    let mut supervisor = endpoint(&manager, "supervisor", 2);
    let mut worker = endpoint(&manager, "worker", 1);
    let stranger = endpoint(&manager, "stranger", 2);
    manager
        .assign_worker(supervisor.agent_id(), worker.agent_id())
        .unwrap();
    assert!(manager
        .assign_worker(worker.agent_id(), supervisor.agent_id())
        .is_err());
    assert!(manager
        .assign_worker(worker.agent_id(), worker.agent_id())
        .is_err());
    assert_eq!(
        manager.workers(supervisor.agent_id()).unwrap(),
        vec![worker.agent_id().clone()]
    );
    assert!(stranger
        .send(worker.agent_id(), &run(), key(), request("job"))
        .is_err());
    supervisor
        .send(worker.agent_id(), &run(), key(), request("job"))
        .unwrap();
    assert!(manager
        .assign_worker(stranger.agent_id(), worker.agent_id())
        .is_err());
    assert!(manager.finish_coordination_run(&run()).is_err());
    assert!(stranger
        .send(supervisor.agent_id(), &run(), key(), result("job"))
        .is_err());
    assert!(worker
        .send(stranger.agent_id(), &run(), key(), result("job"))
        .is_err());
    assert!(worker
        .send(
            supervisor.agent_id(),
            &RunId::from("wrong_run"),
            key(),
            result("job")
        )
        .is_err());
    assert_eq!(worker.try_recv().unwrap().unwrap().payload, request("job"));
    worker
        .send(supervisor.agent_id(), &run(), key(), result("job"))
        .unwrap();
    assert!(worker
        .send(supervisor.agent_id(), &run(), key(), result("job"))
        .is_err());
    assert_eq!(
        supervisor.try_recv().unwrap().unwrap().payload,
        result("job")
    );
    assert!(supervisor
        .send(worker.agent_id(), &run(), key(), request("job"))
        .is_err());
    let permissions = manager.inspect(worker.agent_id()).unwrap().permissions;
    assert_eq!(permissions, AgentPermissions::read_only());
    manager.finish_coordination_run(&run()).unwrap();
}

#[test]
fn full_inbox_does_not_consume_a_delegation_or_reply() {
    let manager = AgentManager::new();
    let mut supervisor = endpoint(&manager, "supervisor", 1);
    let mut worker = endpoint(&manager, "worker", 1);
    manager
        .assign_worker(supervisor.agent_id(), worker.agent_id())
        .unwrap();
    supervisor
        .send(worker.agent_id(), &run(), key(), notice("fill"))
        .unwrap();
    assert!(supervisor
        .send(worker.agent_id(), &run(), key(), request("job"))
        .is_err());
    worker.try_recv().unwrap();
    supervisor
        .send(worker.agent_id(), &run(), key(), request("job"))
        .unwrap();
    worker.try_recv().unwrap();
    worker
        .send(supervisor.agent_id(), &run(), key(), notice("fill"))
        .unwrap();
    assert!(worker
        .send(supervisor.agent_id(), &run(), key(), result("job"))
        .is_err());
    supervisor.try_recv().unwrap();
    worker
        .send(
            supervisor.agent_id(),
            &run(),
            key(),
            AgentMessagePayload::TaskFailed {
                task_id: "job".into(),
                error: "failed verification".into(),
            },
        )
        .unwrap();
    assert!(supervisor.try_recv().unwrap().is_some());
    manager.finish_coordination_run(&run()).unwrap();
}

#[test]
fn stop_cancels_tasks_discards_queue_and_remove_cleans_topology() {
    let manager = AgentManager::new();
    let supervisor = endpoint(&manager, "supervisor", 2);
    let mut worker = endpoint(&manager, "worker", 2);
    let id = worker.agent_id().clone();
    manager.assign_worker(supervisor.agent_id(), &id).unwrap();
    supervisor.send(&id, &run(), key(), request("job")).unwrap();
    manager.stop(&id).unwrap();
    manager.restart(&id).unwrap();
    manager.start(&id).unwrap();
    assert!(worker.try_recv().is_err());
    let mut replacement = manager.connect_agent(&id, vec![key()], 2).unwrap();
    assert!(replacement.try_recv().unwrap().is_none());
    assert!(replacement
        .send(supervisor.agent_id(), &run(), key(), result("job"))
        .is_err());
    manager.stop(&id).unwrap();
    manager.remove(&id).unwrap();
    assert!(manager.workers(supervisor.agent_id()).unwrap().is_empty());
    manager.finish_coordination_run(&run()).unwrap();
}

#[test]
fn redacted_message_events_round_trip_through_existing_run_store() {
    let manager = AgentManager::new();
    let a = endpoint(&manager, "a", 2);
    let mut b = endpoint(&manager, "b", 2);
    let secret = "sk-1234567890abcdef1234567890";
    let id = a.send(b.agent_id(), &run(), key(), notice(secret)).unwrap();
    assert_eq!(b.try_recv().unwrap().unwrap().payload, notice(secret));
    let store = RunStore::in_memory().unwrap();
    store
        .record_run_start(&run(), "coordination", "2026-10-09T00:00:00Z")
        .unwrap();
    for (index, event) in manager
        .drain_message_events()
        .unwrap()
        .into_iter()
        .enumerate()
    {
        store
            .record_event(&EventRecord::new(index as u64 + 1, event))
            .unwrap();
    }
    let events = store.get_events(&run()).unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].event.event_type(), "InterAgentMessage");
    assert!(!serde_json::to_string(&events).unwrap().contains(secret));
    assert!(
        matches!(&events[0].event, ExecutionEvent::InterAgentMessage { message_id, .. } if message_id == &id)
    );
}

#[test]
fn concurrent_send_and_stop_are_serialized_without_stale_delivery() {
    for iteration in 0..32 {
        let manager = AgentManager::new();
        let a = endpoint(&manager, "a", 1);
        let b = endpoint(&manager, "b", 1);
        let id = b.agent_id().clone();
        let barrier = Arc::new(Barrier::new(2));
        let send_barrier = barrier.clone();
        let destination = id.clone();
        let send = std::thread::spawn(move || {
            send_barrier.wait();
            a.send(&destination, &run(), key(), notice(&iteration.to_string()))
        });
        barrier.wait();
        manager.stop(&id).unwrap();
        let accepted = send.join().unwrap().is_ok();
        assert_eq!(
            manager.drain_message_events().unwrap().len(),
            usize::from(accepted)
        );
        manager.restart(&id).unwrap();
        manager.start(&id).unwrap();
        let mut replacement = manager.connect_agent(&id, vec![key()], 1).unwrap();
        assert!(replacement.try_recv().unwrap().is_none());
    }
}

#[test]
fn trace_backpressure_and_payload_limits_preserve_delivery() {
    let manager = AgentManager::new();
    let a = endpoint(&manager, "a", 1);
    let mut b = endpoint(&manager, "b", 1);
    assert!(manager.connect_agent(b.agent_id(), vec![key()], 1).is_err());
    assert!(a
        .send(b.agent_id(), &run(), key(), notice(&"x".repeat(65536)))
        .is_err());
    assert!(b.try_recv().unwrap().is_none());
    for _ in 0..4096 {
        a.send(b.agent_id(), &run(), key(), notice("bounded"))
            .unwrap();
        b.try_recv().unwrap().unwrap();
    }
    assert!(a
        .send(b.agent_id(), &run(), key(), notice("trace full"))
        .is_err());
    assert!(b.try_recv().unwrap().is_none());
    assert_eq!(manager.drain_message_events().unwrap().len(), 4096);
    a.send(b.agent_id(), &run(), key(), notice("after drain"))
        .unwrap();
    assert_eq!(
        b.try_recv().unwrap().unwrap().payload,
        notice("after drain")
    );
}

#[test]
fn nested_supervisors_delegate_independently_and_endpoint_drop_cancels_tasks() {
    let manager = AgentManager::new();
    let root = endpoint(&manager, "root", 2);
    let mut middle = endpoint(&manager, "middle", 2);
    let mut leaf = endpoint(&manager, "leaf", 2);
    manager
        .assign_worker(root.agent_id(), middle.agent_id())
        .unwrap();
    manager
        .assign_worker(middle.agent_id(), leaf.agent_id())
        .unwrap();
    assert!(manager
        .assign_worker(leaf.agent_id(), root.agent_id())
        .is_err());
    root.send(middle.agent_id(), &run(), key(), request("root-task"))
        .unwrap();
    middle.try_recv().unwrap().unwrap();
    middle
        .send(leaf.agent_id(), &run(), key(), request("leaf-task"))
        .unwrap();
    leaf.try_recv().unwrap().unwrap();
    leaf.send(middle.agent_id(), &run(), key(), result("leaf-task"))
        .unwrap();
    assert_eq!(
        middle.try_recv().unwrap().unwrap().payload,
        result("leaf-task")
    );
    let middle_id = middle.agent_id().clone();
    drop(middle);
    let replacement = manager.connect_agent(&middle_id, vec![key()], 2).unwrap();
    assert!(replacement
        .send(root.agent_id(), &run(), key(), result("root-task"))
        .is_err());
    manager.finish_coordination_run(&run()).unwrap();
}
