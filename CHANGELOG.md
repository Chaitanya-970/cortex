# Changelog

All notable changes to Cortex will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

### Graceful Cancellation Latency & Process Teardown

#### Fixed
- Interruptible HTTP streaming in `ModelProvider` implementations: `OpenAiCompatibleProvider` and trait methods poll cooperative cancellation tokens and immediately terminate socket reads and token streaming on cancel.
- Added `execute_with_cancellation` to `Tool` trait and `ToolRegistry`, making subprocess tools (`shell`, `bash`, `execute_command`) interruptible without blocking the agent loop.
- Child process tree termination for `shell` execution: killing spawned process trees immediately upon cancellation.
- Double `Ctrl+C` force-quit support in interactive TUI chat: a second keystroke while cancellation is pending immediately exits the application.
- Workspace `BackgroundIndexer` cancellation and graceful shutdown during application teardown.

### AgentManager & Agent Lifecycle State Machine

#### Added
- Thread-safe `AgentManager` in `cortex-runtime` for persistent agent worker registration, execution, and lifecycle coordination.
- `AgentState` lifecycle state machine with explicit validated transitions: `Created`, `Ready`, `Running`, `Paused`, `Stopped`, and `Failed`.
- Persistent agent worker representation (`Agent`, `AgentManifest`, `AgentModelConfig`, `AgentPermissions`).
- Lifecycle management operations: `create`, `start`, `pause`, `resume`, `stop`, `restart`, `inspect`, `list`, and `remove`.
- Real-time event streaming and subscription channel for structured `AgentLifecycleEvent` records (`AgentCreated`, `AgentStarted`, `AgentPaused`, `AgentResumed`, `AgentStopped`, `AgentFailed`).
- Comprehensive unit and integration test suites covering valid transitions, prohibited state violations, concurrent multi-agent executions, and event streams.

### Model Context Protocol (MCP) Client & Tool Discovery

#### Added
- Model Context Protocol (MCP) client implementation adhering to JSON-RPC 2.0 and the 2024-11-05 protocol contract.
- Subprocess standard I/O transport (`StdioTransport`) and Server-Sent Events HTTP transport (`SseTransport`), with mock transport for test suites.
- Dynamic tool discovery via `tools/list` and adaptation into the native `Tool` trait via `McpTool`, registering external tools directly into `ToolRegistry`.
- Resource reading (`resources/list`, `resources/read`) and prompt template retrieval (`prompts/list`, `prompts/get`).
- Multi-server configuration management via `cortex.toml` using `toml = "0.8"`, supporting namespace prefixes, subprocess environment variables, and disabled flags.
- CLI subcommands `cortex mcp list` and `cortex mcp test <server>`, plus automatic configuration loading in `cortex run`.

### GitHub Automation & Workflows

#### Added
- Automated issue assignment workflow (`.github/workflows/auto-assign.yml`) triggered when contributors comment `.take` on open issues.

### Live Model Providers & Autonomous CLI Harness

#### Added
- Live model provider abstraction supporting OpenAI-compatible endpoints (`/chat/completions`) and Anthropic Claude (`/messages` tool calling).
- Native support for OpenAI (`gpt-4o`, `gpt-4o-mini`), Anthropic Claude (`claude-3-5-sonnet`, `claude-3-5-haiku`), DeepSeek, Groq, and local Ollama (`ollama/<model>` at `http://localhost:11434/v1`).
- Real-time token usage tracking (`prompt_tokens`, `completion_tokens`, `total_tokens`) and USD cost calculation integrated into `AgentLoop` and SQLite persistence.
- Full autonomous agent execution via `cortex run "<prompt>"` with workspace boundaries, tool registration, configurable iterations, and JSON output mode.

### Repository & Workspace Bootstrap

#### Added
- Cargo workspace with `resolver = "2"` containing:
  - `cortex-core`: Shared types, domain identifiers (`AgentId`, `RunId`, `SessionId`), and error definitions (`CortexError`).
  - `cortex-runtime`: Core trait contracts (`ModelProvider`, `Tool`, `Sandbox`).
  - `cortex-cli`: Command-line executable (`cortex`) with status and environment check subcommands.
- Standard Apache-2.0 `LICENSE`.
- Repository governance files: `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `SECURITY.md`, and `.github/CODEOWNERS`.
- Strict GitHub Actions CI workflows:
  - Format checking (`cargo fmt --check`)
  - Compilation check (`cargo check --workspace --all-targets --all-features`)
  - Strict clippy lints (`cargo clippy --workspace --all-targets --all-features -- -D warnings`)
  - Workspace test execution (`cargo test --workspace --all-targets --all-features`)
  - Documentation generation (`cargo doc --workspace --no-deps --all-features`)
  - Release artifact compilation (`cargo build --workspace --all-targets --all-features`)
- Dependency & security policies: `deny.toml` (for `cargo-deny`) and `cargo-audit` security workflows.
- Contributor tooling: `.editorconfig`, `.gitignore`, `rust-toolchain.toml` (stable toolchain pinned).
- GitHub templates: issue templates (bug, feature, task) and pull request template.
- Comprehensive architecture specifications in `docs/architecture/` and initial issue breakdown in `docs/development/initial-issues.md`.

### Planned (Future Milestones)
- **v0.1**: Core runtime loop, initial model provider (Gemma), tool registry, and basic filesystem/shell tools.
- **v0.2**: Coding agent workspace boundaries, Git integration, and coding integration tests.
- **v0.3**: Structured event tracing, SQLite persistence, and run replay.
- **v0.4**: Multi-agent orchestration, agent manager, and inter-agent message bus.
- **v0.5**: Persistent scheduler and cron automation.
- **v0.6**: Docker sandbox and capability-based permissions.
- **v0.7**: Model Context Protocol (MCP) and Agent Skills integration.
- **v0.8**: Evaluation harness and deterministic coding benchmarks.
- **v0.9**: Terminal UI (TUI) and developer tooling.
- **v1.0**: Stable API guarantees and production-ready cross-platform releases.

> *Note: In accordance with project standards, features are not marked as released until they are implemented and verified.*
