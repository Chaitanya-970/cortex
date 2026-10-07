# Execution Tracing & Run Persistence

Cortex records structured execution events for every agent run, persisting the event streams and summary metrics into an embedded SQLite database. This foundation powers post-run inspection, deterministic replay, automated benchmark evaluations, and secret redaction.

## Architecture Overview

The observability architecture follows a decoupled stream model:

```
Agent Execution Loop
        │
        ├── ExecutionEvent emission
        │        │
        │        ▼
        │   Secret Redactor (Regex + Key sanitization)
        │        │
        │        ▼
        └── RunStore (SQLite)
                 ├── Table: schema_version
                 ├── Table: runs
                 └── Table: events
```

## Schema & Migrations

Database migrations are tracked in `schema_version` and executed within atomic transactions upon opening the store.

### `schema_version` Table

| Column | Type | Description |
|---|---|---|
| `version` | INTEGER PRIMARY KEY | Migration version level |
| `applied_at` | TEXT NOT NULL | ISO 8601 UTC timestamp of application |

### `runs` Table

Tracks top-level run lifecycle state, token consumption, and duration:

| Column | Type | Description |
|---|---|---|
| `id` | TEXT PRIMARY KEY | Strongly typed `RunId` |
| `task` | TEXT NOT NULL | Original user task prompt (redacted) |
| `status` | TEXT NOT NULL | `running`, `completed`, `failed`, or `cancelled` |
| `started_at` | TEXT NOT NULL | ISO 8601 UTC timestamp of start |
| `finished_at` | TEXT | ISO 8601 UTC timestamp of completion |
| `duration_ms` | INTEGER | Elapsed execution time in milliseconds |
| `tokens_prompt` | INTEGER DEFAULT 0 | Consumed prompt tokens |
| `tokens_completion` | INTEGER DEFAULT 0 | Consumed completion tokens |
| `tokens_total` | INTEGER DEFAULT 0 | Total token consumption |
| `estimated_cost_usd` | REAL DEFAULT 0.0 | Estimated cost based on model pricing |
| `error` | TEXT | Error description if terminated abnormally |

### `events` Table

Appends individual structured lifecycle events with monotonic sequencing:

| Column | Type | Description |
|---|---|---|
| `id` | INTEGER PRIMARY KEY AUTOINCREMENT | Monotonic primary key |
| `run_id` | TEXT NOT NULL REFERENCES runs(id) | Associated run identifier |
| `sequence` | INTEGER NOT NULL | 1-indexed sequence number within the run |
| `timestamp` | TEXT NOT NULL | ISO 8601 UTC timestamp |
| `event_type` | TEXT NOT NULL | Tagged event variant name |
| `payload_json` | TEXT NOT NULL | Full serialized `EventRecord` JSON |

## Event Lifecycle Variants

Cortex defines 11 structured execution events in `crates/cortex-core/src/event.rs`:

1. `RunStarted`: Emitted when an agent context is initialized with its assigned task.
2. `ModelRequest`: Emitted before dispatching context to the model provider.
3. `ModelResponse`: Emitted upon receiving structured actions or final answers. Contains `structured_output` for replay.
4. `ToolStarted`: Emitted before tool execution begins with arguments.
5. `ToolCompleted`: Emitted when tool execution succeeds.
6. `ToolFailed`: Emitted when tool execution encounters an error.
7. `AgentMessage`: Emitted for conversational agent messages.
8. `AgentRetry`: Emitted during recovery attempts.
9. `AgentError`: Emitted upon unrecoverable runtime errors.
10. `RunCompleted`: Emitted when the agent finishes normally.
11. `RunCancelled`: Emitted when cancellation is triggered via `CancellationToken`.

## Secret Redaction

Before events are written to the database or output to logs, strings and structured JSON values pass through `Redactor`:

- **OpenAI API keys**: Matches `sk-[a-zA-Z0-9_\-]{20,}`
- **GitHub PATs**: Matches `gh[pousr]_[a-zA-Z0-9]{36,}` and fine-grained `github_pat_`
- **AWS Credentials**: Matches `AKIA[0-9A-Z]{16}`
- **Authorization Headers**: Matches `(?i)bearer\s+[a-zA-Z0-9_\-\.]{15,}`
- **Generic Key-Value Assignments**: Matches `password`, `token`, `secret`, `api_key` in text and JSON maps

Matches are replaced with `[REDACTED]`.

## Deterministic Replay

Runs with recorded `ModelResponse` events can be replayed deterministically without querying external model APIs:

```bash
# Replay run using recorded outputs
cortex runs replay <run-id>
```

The `ReplayModelProvider` loads recorded `structured_output` payloads from `RunStore` and supplies them sequentially to the `AgentLoop`, enabling reproducible regression testing and debugging.

## CLI Commands

```bash
# List recent execution runs
cortex runs list --limit 20

# Show detailed run metrics and event trace
cortex runs show <run-id> --verbose

# Deterministically replay an execution run
cortex runs replay <run-id>
```
