# Scheduler Architecture

Scheduled work references existing Cortex agents.

```text
Schedule
   ↓
Persistent job state
   ↓
Load agent
   ↓
Create normal Forge/Cortex run
   ↓
Execute
   ↓
Persist result/state
```

Every scheduled execution should use the same runtime, permission, tracing and sandbox systems as interactive executions.

Overlap policies:

- skip
- queue
- replace
