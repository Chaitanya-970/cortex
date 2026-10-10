# Cortex Architecture Overview

Cortex is an open-source agent runtime and harness for autonomous AI workers, rather than a single hardcoded agent prompt or prototype script.

It is designed to provide observable, sandboxed execution environments for coding agents and multi-agent teams.

---

## Current Status: Workspace Bootstrap

The architecture is currently established through:
- Workspace crate topology:
  - `cortex-core`: Core identifiers (`AgentId`, `RunId`, `SessionId`) and error types (`CortexError`).
  - `cortex-runtime`: Abstract trait boundaries (`ModelProvider`, `Tool`, `Sandbox`).
  - `cortex-cli`: Command-line harness (`cortex`).
- CI/CD quality gates, dependency policies, and security specifications.

No fake agent execution loops or placeholder mock runtimes are deployed in the bootstrap stage. The architectural interfaces define the contract that upcoming implementation milestones will realize.

---

## Intended Architectural Layers

The complete Cortex runtime consists of 10 decoupled layers:

1. **Control Plane & CLI/TUI**: User interface, configuration loader, and task dispatcher. The CLI and future TUI act strictly as presentation consumers of runtime state; execution logic lives entirely in the runtime engine.
2. **Agent Runtime Engine**: The core execution coordinator managing the iterative loop (prompt construction, model invocation, action decoding, tool dispatch, error recovery).
3. **Model Layer**: Provider-agnostic inference abstraction supporting local models—specifically optimized for **Google Gemma** (Gemma 2 9B/27B/2B and CodeGemma via Ollama, llama.cpp, and vLLM) with zero inference cost, privacy, and sub-millisecond local execution, as well as hosted endpoints (Google AI Studio, OpenAI, Anthropic).
4. **Tool Layer**: Registry and typed invocation harness for agent tools (filesystem, shell, git, search).
5. **Agent Manager**: Registry managing persistent worker state, lifecycle transitions (`created`, `running`, `paused`, `stopped`), and state recovery.
6. **Permission Layer**: Capability-based security policy engine evaluating tool execution requests against configured permissions.
7. **Sandbox Boundary**: Process and container isolation (Docker/OCI) ensuring commands cannot access unmounted host resources.
8. **Persistence & Tracing**: SQLite-backed append-only structured event log recording every run, model invocation, tool execution, and retry.
9. **Scheduler**: Persistent cron and one-shot job runner executing recurring agent tasks.
10. **Evaluation System**: Deterministic benchmarks and regression test harness measuring agent task completion, tool call accuracy, and error recovery.

---

## Runtime Authority & Security Boundary

In Cortex, **the model proposes actions, but the runtime holds execution authority**.

```text
┌─────────────────────────────────┐
│              Model              │
└────────────────┬────────────────┘
                 │ Proposes structured action
                 ▼
┌─────────────────────────────────┐
│       Runtime Validation        │
└────────────────┬────────────────┘
                 │ Validates schema & parameters
                 ▼
┌─────────────────────────────────┐
│     Capability / Permission     │
└────────────────┬────────────────┘
                 │ Checks authorization policy
                 ▼
┌─────────────────────────────────┐
│        Sandbox Execution        │
└────────────────┬────────────────┘
                 │ Executes tool in bounded container
                 ▼
┌─────────────────────────────────┐
│        Structured Result        │
└────────────────┬────────────────┘
                 │ Emits trace event & returns result
                 ▼
┌─────────────────────────────────┐
│      Model Context Update       │
└─────────────────────────────────┘
```

This separation is non-negotiable: prompt instructions are never trusted as a security boundary, and models never receive direct unmediated access to host OS APIs.

---

## Event Architecture & Observability

Every execution generates structured, timestamped events:
- `RunStarted`
- `ModelRequest` / `ModelResponse`
- `ToolStarted` / `ToolCompleted` / `ToolFailed`
- `AgentMessage`
- `AgentError` / `AgentRetry`
- `RunCompleted` / `RunCancelled`

These events flow into SQLite storage and are streamed to observability consumers (CLI, TUI, evaluator, replay system) without duplicating execution code. Sensitive variables and credentials are automatically redacted prior to logging.

---

## Core Design Principles

1. **Small, Composable Modules**: Avoid monoliths. Each crate has a distinct responsibility.
2. **Minimal Dependencies**: Every external dependency is evaluated for maintenance, security, and build impact.
3. **No Duplicate Execution Paths**: Interactive runs, scheduled jobs, and evaluation replays use the exact same runtime harness and permission checks.
4. **Stable Interfaces & Explicit Migrations**: Prefer versioned state schemas and backward-compatible evolutions over rapid, breaking shortcuts.
