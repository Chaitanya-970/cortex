# Multi-Agent Coordination Guide

Cortex provides an in-process, deterministic inter-agent coordination system built directly into `AgentManager`. It enables structured communication and supervised task delegation between persistent agent workers without introducing an external message broker or async executor dependency.

---

## Core Concepts

- **`AgentEndpoint`**: An authenticated communication handle issued by `AgentManager` to an active (`Running`) agent. All messages sent through an endpoint automatically bind the agent's verified `AgentId` as the sender.
- **`RoutingKey`**: Exact-match ASCII routing identifiers (1..=128 bytes) accepted by recipient inboxes. Wildcards and topic patterns are not permitted.
- **`AgentMessagePayload`**: Strongly typed message variants:
  - `TaskRequest`: Delegating work from a supervisor to an assigned worker.
  - `TaskResult`: Returning successful task output back to the supervisor.
  - `TaskFailed`: Reporting an execution failure back to the supervisor.
  - `Notification`: General informational messages that do not require a supervisor/worker relationship.
- **Supervisor/Worker Hierarchy**: Explicit parent-child relationships established via `assign_worker`. Task delegations require direct supervisor authority; cycle formation and cross-supervisor interference are prevented.
- **Separation of Concerns**: Message delivery only enqueues typed data into an inbox. The host coordinates execution separately through standard runtime and tool boundaries; receiving a message never automatically executes code or grants permissions.

---

## Practical Example: Supervisor and Worker Delegation

Below is a complete, self-contained example demonstrating how trusted host code initializes agents, connects endpoints, delegates a task, and receives the result.

```rust
use cortex_core::{AgentId, RunId};
use cortex_runtime::{
    AgentManager, AgentManifest, AgentModelConfig, AgentPermissions,
    AgentMessagePayload, RoutingKey,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manager = AgentManager::new();

    // 1. Define manifests for a supervisor and a specialist worker
    let supervisor_manifest = AgentManifest::new(
        "supervisor",
        "coordinator",
        ".",
        AgentModelConfig::new("anthropic", "claude-3-5-sonnet"),
    )
    .with_id(AgentId::from("supervisor-agent"))
    .with_permissions(AgentPermissions::read_only());

    let worker_manifest = AgentManifest::new(
        "worker",
        "coder",
        ".",
        AgentModelConfig::new("anthropic", "claude-3-5-sonnet"),
    )
    .with_id(AgentId::from("worker-agent"))
    .with_permissions(AgentPermissions::read_only());

    // 2. Register and start agents (endpoints require agents to be in Running state)
    let supervisor = manager.create(supervisor_manifest)?;
    let worker = manager.create(worker_manifest)?;

    manager.start(&supervisor.id)?;
    manager.start(&worker.id)?;

    // 3. Configure exact routing keys
    let task_route = RoutingKey::new("tasks.code")?;
    let result_route = RoutingKey::new("results.code")?;

    // 4. Issue bounded endpoints (capacity: 1..=1024 messages)
    let mut supervisor_endpoint = manager.connect_agent(
        &supervisor.id,
        vec![result_route.clone()],
        16,
    )?;

    let mut worker_endpoint = manager.connect_agent(
        &worker.id,
        vec![task_route.clone()],
        16,
    )?;

    // 5. Establish supervisor/worker delegation relationship
    manager.assign_worker(&supervisor.id, &worker.id)?;

    // 6. Supervisor sends a TaskRequest to the worker
    let run_id = RunId::from("run_coord_01");
    let task_id = "task-search-index";

    let message_id = supervisor_endpoint.send(
        &worker.id,
        &run_id,
        task_route,
        AgentMessagePayload::TaskRequest {
            task_id: task_id.to_string(),
            instructions: "Audit search indexing latency".to_string(),
        },
    )?;
    println!("Delegated task message ID: {message_id}");

    // 7. Worker checks its inbox synchronously with try_recv()
    if let Some(msg) = worker_endpoint.try_recv()? {
        match msg.payload {
            AgentMessagePayload::TaskRequest { task_id, instructions } => {
                println!("Worker received task '{task_id}': {instructions}");

                // Host performs actual execution through existing tool/runtime boundaries...

                // 8. Worker reports completion back to the supervisor
                worker_endpoint.send(
                    &supervisor.id,
                    &run_id,
                    result_route,
                    AgentMessagePayload::TaskResult {
                        task_id,
                        output: "Audit complete: latency is nominal.".to_string(),
                    },
                )?;
            }
            _ => unreachable!(),
        }
    }

    // 9. Supervisor checks inbox for task completion
    if let Some(result_msg) = supervisor_endpoint.try_recv()? {
        if let AgentMessagePayload::TaskResult { task_id, output } = result_msg.payload {
            println!("Supervisor received result for '{task_id}': {output}");
        }
    }

    // 10. Release completed task correlation records after the run concludes
    manager.finish_coordination_run(&run_id)?;

    Ok(())
}
```

---

## Asynchronous Receiving (`recv().await`)

For asynchronous runtimes, `AgentEndpoint` implements `recv(&mut self)`:

```rust
let message = worker_endpoint.recv().await?;
```

- **Executor Independent**: `recv()` uses a custom `poll_fn` implementation and standard `std::task::Waker`. It yields cooperatively on any Rust asynchronous executor (e.g. Tokio, smol, async-std) without introducing an external runtime dependency.
- **Cancellation Safe**: If an in-flight `recv().await` future is dropped before delivery, already-queued messages remain intact in the inbox.
- **Lifecycle Integration**: Pause wakes a waiting receiver and preserves its inbox. If the agent is still paused when polled, receiving returns an error; after resume, the same endpoint can receive again. Stop, failure, restart, and prepare revoke existing endpoints and wake receivers. Old endpoints remain unusable even after the agent starts again.

## Limits and tracing

Payloads may contain up to 64 KiB of serialized JSON. The manager retains at most 4096 task correlations and 4096 undrained message events. Full buffers reject sends instead of silently overwriting messages or traces. Task IDs are unique within a run, including after completion. Once `finish_coordination_run` releases correlations, host code must never reuse that run ID.

`drain_message_events()` returns redacted `InterAgentMessage` events in enqueue order. Host code wraps each event in an `EventRecord` using its existing per-run sequence allocator and writes it to `RunStore`; the manager does not persist traces automatically. An event confirms enqueueing, not processing or execution. Drain the buffer regularly. See [execution tracing](../architecture/tracing.md).
