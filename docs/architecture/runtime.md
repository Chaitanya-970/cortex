# Runtime Architecture

The minimal runtime lifecycle is:

```text
User task
   ↓
Agent context
   ↓
Model request
   ↓
Structured model response
   ↓
Tool validation
   ↓
Permission check
   ↓
Tool execution
   ↓
Structured result
   ↓
Model continuation
   ↓
Final result
```

## Core interfaces

The first runtime should contain:

- model provider abstraction
- Tool trait
- ToolRegistry
- Agent context
- execution loop
- structured tool calls
- structured tool results
- cancellation
- timeouts

## Initial tools

- read_file
- write_file
- list_directory
- shell

The runtime owns execution authority. Models do not receive direct access to process execution APIs.

## Cancellation during I/O

`AgentLoop` shares its cancellation token through `AgentContext` and tool
dispatch. The synchronous model provider interface uses a private, per-request
Tokio runtime and asynchronous HTTP transport for OpenAI-compatible and
Anthropic requests. A 25 ms cancellation check runs alongside pending network
I/O, including connection setup, response headers, JSON bodies, and SSE chunks.
Cancellation drops the request future and response, then shuts down the runtime;
it does not wait for the normal HTTP timeout. An OS DNS lookup already running
in Tokio's blocking pool may finish separately, without retaining the HTTP
operation. Streaming callbacks remain on the calling thread.

Shell execution checks cancellation until both the process and its output
readers finish. Unix shells use a dedicated process group. Windows shells are
attached to a job object whose descendants are terminated on cancellation or
job closure, including descendants whose direct parent has exited. These
mechanisms provide process cleanup, rather than an additional sandbox boundary.

The TUI signals the active run and background indexer during shutdown. A second
Ctrl+C during pending chat cancellation exits the TUI without waiting for an
agent worker. Third-party model and tool implementations must cooperate with
cancellation to interrupt their own blocking work.

Built-in recursive search tools check cancellation between directory entries,
files, and matching lines. A single blocking filesystem operation must still
return before its next cooperative check.
