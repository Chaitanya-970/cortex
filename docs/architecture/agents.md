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
- `remove(agent_id)`: Remove a stopped, failed, or created agent. Active agents cannot be removed until stopped.
- `subscribe()`: Receive a stream of `AgentLifecycleEvent` updates (`AgentCreated`, `AgentStarted`, `AgentPaused`, `AgentResumed`, `AgentStopped`, `AgentFailed`).

Future multi-agent systems and CLI interfaces build directly on this manager.
