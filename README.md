# Cortex

[![CI](https://github.com/x1-xh/cortex/actions/workflows/pr.yml/badge.svg)](https://github.com/x1-xh/cortex/actions)
[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Version](https://img.shields.io/badge/version-0.1.0--alpha.2-green.svg)](https://github.com/x1-xh/cortex/releases)

**Cortex** is an open-source Rust runtime and harness for autonomous AI workers.

It provides a sandboxed execution environment with workspace boundary enforcement, live model inference (OpenAI, Anthropic Claude, DeepSeek, Groq, Ollama), structured tool execution, real-time token and USD cost tracking, append-only SQLite execution event tracing, deterministic replay, benchmark evaluations, and an interactive terminal control plane (TUI).

---

## Core Security Principle: Runtime Authority

In Cortex, **model output is untrusted input**. The runtime is the sole execution authority:
- **Models propose actions** via structured schemas.
- **The runtime validates, authorizes, and executes** them within strict workspace boundaries.
- **Path traversal prevention**: All filesystem and shell operations are confined to the configured workspace root; directory traversal (`../`) and external symlinks are strictly denied.
- **Dangerous operations restricted**: `git push` is blocked by default; secrets and API keys are redacted from execution logs.

---

## Installation & Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) (version 1.75 or newer)
- [Git](https://git-scm.com/)
- SQLite (bundled automatically via `rusqlite`)

### 1. Cloning the Repository

Clone the Cortex repository from GitHub:

```bash
git clone https://github.com/x1-xh/cortex.git
cd cortex
```

### 2. Building from Source

Build the workspace using Cargo:

```bash
# Debug build (faster compilation)
cargo build

# Release build (optimized for performance)
cargo build --release
```

The compiled binary will be located at:
```bash
./target/release/cortex
```

### 3. Installing the CLI Binary (`cargo install`)

#### Option A: Install from Local Source
From within the repository root, install the `cortex` binary globally into your Cargo bin path (`~/.cargo/bin`):

```bash
cargo install --path crates/cortex-cli
```

#### Option B: Install Directly from GitHub
Install directly from the remote repository without cloning manually:

```bash
cargo install --git https://github.com/x1-xh/cortex.git cortex-cli
```

Verify that the CLI is accessible:

```bash
cortex --version
cortex check
cortex status
```

### 4. Using Cortex as a Library Dependency

To embed Cortex runtime components into your own Rust projects, add the crates to your `Cargo.toml`:

```toml
[dependencies]
cortex-core = { git = "https://github.com/x1-xh/cortex.git" }
cortex-runtime = { git = "https://github.com/x1-xh/cortex.git" }
cortex-harness = { git = "https://github.com/x1-xh/cortex.git" }
cortex-tui = { git = "https://github.com/x1-xh/cortex.git" }
```

---

## Quickstart Guide

### 1. Configure Model API Keys (or Use Local Ollama)

Cortex supports cloud model providers as well as local offline models:

```bash
# For OpenAI / DeepSeek / Groq:
export OPENAI_API_KEY="sk-..."

# For Anthropic Claude:
export ANTHROPIC_API_KEY="sk-ant-..."

# Or use local Ollama (zero API key required):
# Ensure Ollama is running at http://localhost:11434
```

### 2. Running Autonomous Agent Tasks (`cortex run`)

Execute autonomous tasks confined to your workspace repository:

```bash
# Run with OpenAI (default: gpt-4o-mini)
cortex run "Inspect the git status and summarize uncommitted changes"

# Run with Anthropic Claude 3.5 Sonnet
cortex run "Refactor error handling in src/lib.rs" \
  --model claude-3-5-sonnet-20241022

# Run locally and offline with Ollama
cortex run "Run cargo test and fix any failing unit tests" \
  --model ollama/llama3.1 \
  --base-url http://localhost:11434/v1

# Confine execution to a custom workspace
cortex run "Audit dependency licenses" --workspace /path/to/project

# Output structured JSON with token metrics and USD cost breakdown
cortex run "Review recent git diff" --json
```

### 3. Inspecting Historical Runs & Traces (`cortex runs`)

All agent runs, tool invocations, and token metrics are saved to SQLite (`~/.cortex/cortex.db`):

```bash
# List recent execution runs
cortex runs list --limit 10

# Show full event timeline, tool calls, and model outputs
cortex runs show <run-id> --verbose

# Replay an execution run deterministically using recorded outputs
cortex runs replay <run-id>
```

### 4. Interactive Terminal Control Plane (`cortex tui`)

Launch the terminal UI built with Ratatui to monitor live agent runs, browse historical traces, inspect metrics, and review evaluations:

```bash
cortex tui
```

**Keybindings**:
- `1` - `6` or `Tab` / `Shift+Tab`: Switch between views (Dashboard, Agents, Active Run, Events, History, Tasks/Bench)
- `↑` / `↓` or `k` / `j`: Navigate lists and run records
- `Enter`: Inspect selected run details
- `r`: Reload state from SQLite
- `q` / `Ctrl+C`: Exit

### 5. Running Evaluations & Benchmarks (`cortex bench`)

Evaluate agent performance against ground truth benchmarks:

```bash
# Run the coding benchmark suite
cortex bench run --suite coding

# Generate a Markdown report
cortex bench run --suite coding --report reports/coding.md
```

---

## Workspace Structure

The project is organized as a modular Cargo workspace:

| Crate | Purpose |
|---|---|
| [`cortex-core`](crates/cortex-core) | Domain identifiers (`AgentId`, `RunId`, `SessionId`), error taxonomy (`CortexError`), execution events (`ExecutionEvent`), and secret redactor. |
| [`cortex-runtime`](crates/cortex-runtime) | Execution engine, model provider abstraction (OpenAI, Anthropic, Ollama), tool registry, workspace boundaries, and SQLite storage (`RunStore`). |
| [`cortex-cli`](crates/cortex-cli) | User-facing CLI entrypoint (`cortex run`, `cortex runs`, `cortex bench`, `cortex tui`). |
| [`cortex-harness`](crates/cortex-harness) | Deterministic benchmark runner and coding/refactoring evaluation suites. |
| [`cortex-tui`](crates/cortex-tui) | Ratatui terminal dashboard and interactive execution inspector. |

---

## Local Development & Quality Gates

Before contributing or submitting pull requests, run the project verification suite:

```bash
# Check formatting
cargo fmt --check

# Compile-check workspace
cargo check --workspace --all-targets --all-features

# Strict clippy lints (warnings treated as errors)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Execute all unit, integration, and security tests
cargo test --workspace --all-targets --all-features

# Verify documentation generation
cargo doc --workspace --no-deps --all-features

# Audit licenses and dependency bans
cargo deny check
```

---

## Project Status & Roadmap

### Implemented
- [x] **Core Runtime**: Model provider abstraction, tool trait, agent context, and iterative execution loop.
- [x] **Live Inference Providers**: OpenAI-compatible API, Anthropic Messages API, and local Ollama.
- [x] **Token & Cost Tracking**: Real-time accumulation of prompt/completion tokens and estimated USD costs.
- [x] **Tool Registry**: Filesystem (`read`, `write`, `list`), shell execution with token scrubbing, and Git operations (`status`, `diff`, `log`, `branch`, `commit`).
- [x] **Workspace Sandboxing**: Boundary enforcement preventing path traversal and external modifications.
- [x] **Observability & Tracing**: SQLite-backed structured event log (`RunStore`) and deterministic replay.
- [x] **Terminal UI Control Plane**: Ratatui interface with dashboard, run inspector, event viewer, and history.
- [x] **Evaluation Harness**: Automated benchmark runner with verifiable coding suites.

### Planned
- [ ] **Docker Sandbox Execution**: Container-level isolation with non-root execution and explicit mounts.
- [ ] **Multi-Agent Message Bus**: Inter-agent communication (`AgentManager`, `MessageBus`, `TaskQueue`).
- [ ] **Persistent Scheduler**: Cron automation and durable background tasks.
- [ ] **Model Context Protocol (MCP)**: Native integration for MCP client/server tool discovery.

---

## Contributing

We welcome contributions! Please review our [Contributing Guidelines](CONTRIBUTING.md) and [Code of Conduct](CODE_OF_CONDUCT.md) before submitting pull requests.

For coding agent guidelines and architectural principles, see [AGENTS.md](AGENTS.md).

---

## Security

Security is a primary architectural priority. Please report security vulnerabilities according to our [Security Policy](SECURITY.md).

---

## License

Cortex is licensed under the [Apache License, Version 2.0](LICENSE).
