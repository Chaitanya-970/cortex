# Execution Tracing

Cortex should emit structured events for every significant execution.

Core events:

- RunStarted
- ModelRequest
- ModelResponse
- ToolStarted
- ToolCompleted
- ToolFailed
- AgentMessage
- AgentRetry
- AgentError
- RunCompleted
- RunCancelled

Events should be persisted in SQLite and remain usable by:

- CLI
- TUI
- evaluator
- replay system
- future web interfaces

Sensitive values must be redacted.
