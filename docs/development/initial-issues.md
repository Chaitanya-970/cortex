# Initial Cortex GitHub Issues

These issues define the immediate work required to progress from the initial workspace bootstrap into core runtime and agent development.

---

### Issue 1: Model Provider Abstraction & Initial Adapter
- **Title**: `feat(runtime): define ModelProvider trait and Gemma adapter`
- **Labels**: `type: feature`, `area: runtime`
- **Description**: Implement the foundational LLM provider abstraction in `cortex-runtime`. Add support for model configuration, prompt submission, token generation, and structured action parsing, beginning with Gemma (local and API endpoints).
- **Acceptance Criteria**:
  - Trait definition with streaming and non-streaming completion interfaces.
  - Integration with mock provider for deterministic unit testing.
  - Concrete adapter for Gemma endpoint.
  - Zero unhandled errors; model output marked untrusted.

---

### Issue 2: Tool Registry & Schema Validation
- **Title**: `feat(runtime): implement ToolRegistry and schema validation`
- **Labels**: `type: feature`, `area: tools`
- **Description**: Build `cortex-runtime::tool::ToolRegistry` to register, discover, and validate tool invocations. Tools must define JSON/typed schemas for arguments and return structured results.
- **Acceptance Criteria**:
  - Thread-safe registry for registering and looking up tools.
  - Argument validation against tool schema before invocation.
  - Structured output types (`ToolResult::Success`, `ToolResult::Failure`).
  - Unit tests covering duplicate tool names, missing tools, and invalid schemas.

---

### Issue 3: Workspace Filesystem Tools
- **Title**: `feat(tools): implement sandboxed filesystem tools`
- **Labels**: `type: feature`, `area: tools`, `security`
- **Description**: Implement basic filesystem tools (`read_file`, `write_file`, `list_dir`) that enforce strict workspace root confinement to prevent directory traversal.
- **Acceptance Criteria**:
  - `read_file`: Reads text files with offset/limit limits.
  - `write_file`: Writes files safely, creating parent directories.
  - `list_dir`: Lists files and subdirectories.
  - Security tests verifying path traversal attempts (`../../`) are blocked.

---

### Issue 4: Shell Execution Tool with Timeouts & Cancellation
- **Title**: `feat(tools): implement bounded shell execution tool`
- **Labels**: `type: feature`, `area: tools`, `security`
- **Description**: Implement a shell command runner with explicit timeout enforcement, environment variable filtering, and cancellation token support.
- **Acceptance Criteria**:
  - Captures stdout and stderr with configurable buffer limits.
  - Kills runaway processes on timeout.
  - Cleanses sensitive host environment variables.
  - Unit and integration tests covering timeouts and exit code reporting.

---

### Issue 5: Agent Execution Loop
- **Title**: `feat(runtime): implement single-agent execution coordinator`
- **Labels**: `type: feature`, `area: runtime`
- **Description**: Implement the iterative execution loop: load context, call model provider, parse tool calls, execute authorized tools, append results to context, and loop until completion or iteration limit.
- **Acceptance Criteria**:
  - Configurable max iterations and timeout.
  - Cancellation token support for graceful termination.
  - Unit tests using mock model provider verifying step transitions.

---

### Issue 6: Structured Event Tracing and SQLite Persistence
- **Title**: `feat(tracing): implement structured run event logger with SQLite`
- **Labels**: `type: feature`, `area: observability`
- **Description**: Record execution events (`RunStarted`, `ModelRequest`, `ToolCompleted`, `AgentError`, etc.) to an append-only SQLite database. Include automatic redacting of sensitive environment values.
- **Acceptance Criteria**:
  - Structured event schema with timestamps and run IDs.
  - Non-blocking event emission.
  - SQLite storage migrations.
  - Integration test verifying event replay.
