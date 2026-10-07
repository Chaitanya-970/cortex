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
