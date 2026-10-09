# Agent Architecture

A Cortex agent is a persistent worker with:

- stable ID (`AgentId`)
- human-readable name and operational role
- declarative configuration (`AgentManifest`, `AgentModelConfig`)
- explicit lifecycle state (`AgentState`)
- workspace boundary path (`PathBuf`)
- tool capabilities (`tools`)
- operational permissions (`AgentPermissions`)

## Lifecycle State Machine

```text
                  ┌─────────┐
                  │ Created │
                  └────┬────┘
                       │
         ┌─────────────┼─────────────┐
         ▼             ▼             ▼
    ┌─────────┐   ┌─────────┐   ┌─────────┐
    │  Ready  │◄──┤ Stopped │   │ Failed  │
    └────┬────┘   └────▲────┘   └────▲────┘
         │             │             │
         ▼             │             │
    ┌─────────┐        │             │
 ┌─►│ Running ├────────┴─────────────┘
 │  └────┬────┘
 │       │
 │       ▼
 │  ┌─────────┐
 └──┤ Paused  │
    └─────────┘
```

### States

| State | Description |
| :--- | :--- |
| `Created` | Agent registered from manifest; configuration validated. |
| `Ready` | Initialized, verified, and ready for execution. |
| `Running` | Actively executing tasks or the run loop. |
| `Paused` | Temporarily suspended; can resume back to `Running`. |
| `Stopped` | Halted or completed cleanly. Requires `restart()` to transition to `Ready` before running again. |
| `Failed` | Encountered an unrecoverable execution error. Requires `restart()` to reset to `Ready`. |

### Valid Transitions

- **`Created`** $\to$ `Ready`, `Running`, `Stopped`, `Failed`
- **`Ready`** $\to$ `Running`, `Stopped`, `Failed`
- **`Running`** $\to$ `Paused`, `Stopped`, `Ready`, `Failed`
- **`Paused`** $\to$ `Running`, `Stopped`, `Ready`, `Failed`
- **`Stopped`** $\to$ `Ready` (via `restart`)
- **`Failed`** $\to$ `Ready` (via `restart`), `Stopped`

Direct transitions from `Stopped` or `Failed` directly to `Running` without going through `restart()` / `Ready` are rejected with `CortexError::Validation`.

## AgentManager

`AgentManager` owns lifecycle transitions, thread-safe concurrency, and event streaming:

- `create(manifest)`: Register a new agent in `Created` state.
- `start(agent_id)`: Move agent to `Running`.
- `pause(agent_id)`: Suspend a running agent to `Paused`.
- `resume(agent_id)`: Resume a paused agent to `Running`.
- `stop(agent_id)`: Stop an agent, moving it to `Stopped`.
- `restart(agent_id)`: Reset a stopped or failed agent to `Ready`.
- `inspect(agent_id)`: Query agent configuration and state.
- `list()`: Return all registered agents.
- `registry()`: Access the thread-safe `AgentRegistry` catalog for capability discovery.
- `remove(agent_id)`: Remove a stopped, failed, or created agent. Active agents cannot be removed until stopped.
- `subscribe()`: Receive a stream of `AgentLifecycleEvent` updates (`AgentCreated`, `AgentStarted`, `AgentPaused`, `AgentResumed`, `AgentStopped`, `AgentFailed`).

Future multi-agent systems and CLI interfaces build directly on this manager.

## AgentRegistry & Capability Discovery

`AgentRegistry` provides a read-optimized, thread-safe directory service (`Arc<RwLock<HashMap<AgentId, AgentDescriptor>>>`) for peer discovery and capability querying:

- **`AgentDescriptor`**: Read-only public snapshot containing `id`, `name`, `role`, `status`, `capabilities`, `workspace_root`, and `tags`.
- **`AgentCapability`**: Typed qualifications covering tools (`Tool("read_file")`, wildcard `Tool("git_*")`), model tiers (`ModelTier("fast")`, `ModelTier("reasoning")`, `ModelTier("coding")`), and domain specializations (`Domain("researcher")`, `Domain("coder")`, `Domain("reviewer")`, `Domain("planner")`).
- **Query APIs**:
  - `find_by_role(role)`: Search agents by operational role (case-insensitive substring and exact matching).
  - `find_by_capability(query)`: Search agents matching tool names, wildcard patterns, model tiers, or domain specializations.
  - `find_by_all_capabilities(queries)`: Multi-predicate conjunction query (AND) returning agents that satisfy every capability or tag.
  - `find_by_any_capability(queries)`: Multi-predicate disjunction query (OR) returning agents that satisfy at least one capability or tag.
  - `rank_by_capabilities(queries)` / `find_best_match(queries)`: Score and rank candidate agents by number of matched capabilities for optimal task delegation.
  - `find_active_by_capability(query)` / `find_active_by_all_capabilities(queries)`: Capability discovery filtered strictly for active workers (`Running`, `Paused`).
  - `find_by_tag(tag)`: Search agents by operational tags.
  - `list_active()`: Filter for agents currently in active execution states (`Running`, `Paused`).
  - `list_all()`: Enumerate all registered agent descriptors.
- **Synchronization**: `AgentManager` automatically keeps the registry synchronized across all worker creation, lifecycle state transitions, and removal operations. Registries can also ingest `AgentLifecycleEvent` streams directly via `handle_event()`.

## Inter-Agent Coordination & Message Bus

`AgentManager` integrates deterministic inter-agent messaging and task delegation directly into its coordination state:

### Endpoint Issuance
- **Authenticity**: Communication handles (`AgentEndpoint`) are created by trusted host code calling `manager.connect_agent(agent_id, routes, capacity)`.
- **State Constraint**: Only agents in the `Running` lifecycle state can be connected. Exactly one active endpoint is allowed per agent at a time.
- **Capacity & Routes**: Each endpoint specifies an inbox capacity between 1 and 1024 messages, and 1 to 128 accepted `RoutingKey` values.

### Bounded FIFO Delivery & Exact Routes
- **Enqueue Ordering**: Messages are delivered to recipient queues in the order of successful enqueueing.
- **Exact Routing**: Routing keys are 1..=128 bytes containing only ASCII alphanumeric characters, dots, underscores, or hyphens (`RoutingKey`). Senders must target an exact routing key registered by the recipient; wildcards, broadcasts, and pattern matching are rejected.
- **Backpressure & Limits**: Full inboxes, a full trace buffer, and payloads exceeding 64 KiB reject sends with `CortexError::Validation`. Accepted messages remain queued until received or discarded by endpoint revocation.

### Hierarchy & Supervisor Relationships
- **Topology**: `manager.assign_worker(supervisor, worker)` establishes directed supervisor/worker trees. Nested supervisor hierarchies are supported.
- **Cycle Prevention**: The hierarchy graph is traversed upon assignment; circular dependencies and self-assignment are strictly rejected.
- **Pending Work Invariants**: A worker cannot be reassigned to a new supervisor while it has pending task delegations.

### Task Correlation & Validation
- **Correlation Lifecycle**: Task delegations (`TaskRequest`, `TaskResult`, `TaskFailed`) require a `task_id` (1..=128 bytes) unique to the execution `RunId`.
- **Delegation Rules**:
  - `TaskRequest` may only be sent by a direct supervisor to its assigned worker.
  - `TaskResult` and `TaskFailed` must be sent by the worker back to its original supervisor, matching the active delegation and `RunId` exactly once.
  - Informational `Notification` messages do not require supervisor relationships.
- **Run Completion**: `manager.finish_coordination_run(run_id)` removes completed and cancelled task correlations once a run permanently finishes; pending work prevents cleanup. Host code must never reuse that run ID. The manager does not retain completed-run tombstones.

### Lifecycle Invalidation & Revocation
- **Pause**: Pausing an agent wakes any pending `recv()` future with an error and preserves queued messages. When resumed, the endpoint can continue receiving.
- **Stop, Fail, Restart, Prepare**: These operations revoke existing endpoints, cancel pending task correlations involving the agent, discard its queued messages, and wake waiting receivers. Subsequent receives fail lifecycle validation or report endpoint cancellation. This does not undo work already performed by another agent.
- **Drop**: Dropping an `AgentEndpoint` closes its inbox, revoking delivery.
- **Removal**: Removing an agent also severs its hierarchy parent/child relationships.

### Lock Ordering
To prevent deadlocks under heavy multi-agent concurrency, Cortex enforces a strict hierarchy:
1. `AgentManager.agents` (`RwLock`): Acquired first for agent lifecycle validation and state checks.
2. `AgentManager.coordination` (`Mutex`): Acquired second for inbox operations, topology modifications, task correlation, and event emission.
3. Wakers (`Waker::wake`): Invoked only **after** releasing both locks, ensuring receivers never resume while locks are held.
