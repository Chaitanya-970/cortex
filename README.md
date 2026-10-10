# Cortex

<p align="center">
  <strong>Open-source Rust runtime and interactive control plane for autonomous AI workers</strong>
</p>

<p align="center">
  <em>Built for Google Gemma 4, open models, sandboxed tool execution, and multi-agent coordination.</em>
</p>

<p align="center">
  <a href="https://github.com/x1-xh/cortex/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License: Apache-2.0"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/rust-1.75%2B-orange.svg" alt="Rust: 1.75+"></a>
  <a href="https://github.com/x1-xh/cortex/actions"><img src="https://img.shields.io/badge/build-passing-brightgreen.svg" alt="Build Status"></a>
  <a href="https://cortex-ai.github.io"><img src="https://img.shields.io/badge/docs-cortex--ai.github.io-0284c7.svg" alt="Documentation"></a>
</p>

<p align="center">
  <img src="assets/cortex-tui.png" alt="Cortex Interactive Terminal Control Plane" width="850">
</p>

---

## Runtime Authority & Security Invariants

> **Model output is untrusted input. The runtime is the sole execution authority.**

Cortex treats all LLM responses as unverified advisory proposals. The Rust runtime guarantees strict operational isolation:

- **Strict Schema Validation**: Tool calls are verified against typed schemas before execution; arbitrary or unparsed model commands are rejected.
- **Workspace Boundary Enforcement**: Canonicalized path resolution prevents directory traversal (`../`) and symlink escapes outside the workspace root.
- **Subprocess Environment Scrubbing**: Shell tools execute in sanitized environments with sensitive credentials (`*_API_KEY`, tokens) stripped, bounded by process timeouts.
- **Destructive Operation Denial**: High-risk operations (such as unauthorized `git push` or arbitrary root writes) are explicitly blocked by runtime security policy.
- **Append-Only Tracing & Redaction**: Every prompt, tool execution, and state transition is captured in SQLite with automated regex secret redaction.

---

## Key Features

- 🖥️ **Interactive Terminal Control Plane (TUI)**: Launch `cortex` for an ultra-responsive, full-featured terminal workspace powered by Ratatui, featuring live streaming reasoning traces, slash commands, and real-time token/cost accounting.
- ⚡ **Google Gemma 4 & Open Models Native**: First-class integration for Google Gemma 4 (`gemma4:12b`, `gemma-4-26b-it`, `codegemma`) both locally offline via Ollama and hosted via Google AI Studio, alongside OpenAI and Anthropic.
- 🔒 **Defense-in-Depth Sandboxing**: Host isolation and optional container sandboxing with CPU/memory limits, read-only root filesystems, and network restrictions.
- 🐝 **Multi-Agent Coordination & DAG Pipelines**: Define and coordinate specialized agent swarms with directed acyclic graph (DAG) pipelines, mailbox message buses, and supervisor-worker hierarchies.
- ⏱️ **Persistent Scheduler & Cron**: Schedule autonomous tasks with standard 5-field cron syntax, one-shot timers, and configurable overlap policies (`skip`, `queue`, `replace`).
- 🔌 **Model Context Protocol (MCP)**: Discover, register, and query external tools from any standard stdio or SSE MCP server.
- 🎯 **Ground-Truth Benchmarks**: Built-in deterministic benchmark evaluation harness (`cortex bench`) to test coding and agent capabilities against verifiable ground truth.

---

## Quickstart

### Prerequisites

- **Rust**: $\ge$ 1.75 (`cargo`, `rustc`)
- **Git**: Installed and available in `$PATH`
- **SQLite**: Local database support (bundled via `rusqlite`)

### Installation

Install the Cortex CLI globally using Cargo:

```bash
# Install globally from GitHub
cargo install --git https://github.com/x1-xh/cortex.git cortex-cli

# Or install from a local checkout
git clone https://github.com/x1-xh/cortex.git
cd cortex
cargo install --path crates/cortex-cli
```

Ensure your Cargo binary directory is present in your shell's `$PATH`:

```bash
# Add to current shell session
export PATH="$HOME/.cargo/bin:$PATH"

# Persist to shell configuration
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc   # zsh
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc  # bash
```

Verify your installation:

```bash
cortex --version
cortex check
cortex status
```

---

## Interactive Terminal Control Plane (TUI)

Launch the interactive control plane by running `cortex` without arguments:

```bash
# Launch interactive TUI in current directory
cortex

# Launch with custom SQLite persistence path
cortex --db /path/to/cortex.db
```

### Views & Navigation

| Key | Tab View | Description |
|---|---|---|
| `1` | **Chat / Agent** | Live interactive coding agent, streaming reasoning traces, and diff viewer |
| `2` | **Portals** | Configured inference endpoints, active model selection, and API key management |
| `3` | **Coordination** | Multi-agent swarms, DAG execution pipelines, and inter-agent message feeds |
| `4` | **Runs** | Searchable history of past execution runs, durations, token counts, and costs |
| `5` | **Settings** | Runtime preferences, default model, base URL, and workspace sandbox settings |

- **Tab Navigation**: `Tab` / `Shift+Tab` or number keys `1`–`5`
- **Command Palette**: Type `/` to open slash command autocomplete
- **Cancel / Interrupt**: `Ctrl+C` cancels active execution without quitting
- **Quit**: `Ctrl+Q` or `/exit`

### Built-in Slash Commands

| Command | Usage | Description |
|---|---|---|
| `/help` | `/help` | Display command catalog and keyboard shortcuts |
| `/model` | `/model [list \| set <name>]` | Switch active inference model or list curated options |
| `/settings` | `/settings [set <key> <val> \| reload]` | View or update `~/.cortex/settings.json` |
| `/status` | `/status` | Display runtime health, active model, and storage status |
| `/diff` | `/diff` | Inspect Git diff of workspace modifications |
| `/sessions` | `/sessions` | List and resume saved conversation sessions |
| `/clear` | `/clear` | Clear message buffer and archive session |
| `/cost` | `/cost` | Detailed token usage and cost accounting breakdown |

---

## CLI Usage & Workflows

### Autonomous Task Execution (`cortex run`)

Execute autonomous agent tasks in a single command with workspace confinement, live telemetry, and cost accounting:

```bash
# Google Gemma 4 (Local offline via Ollama — zero API key required)
cortex run "inspect src/lib.rs and fix compiler warnings" --model ollama/gemma4:12b

# Google Gemma 4 (Hosted via Google AI Studio / Gemini API)
export GEMINI_API_KEY="AIza..."
cortex run "audit repository security" \
  --model gemma-4-26b-it \
  --base-url https://generativelanguage.googleapis.com/v1beta/openai/

# CodeGemma / Gemma 2 for specialized code editing
cortex run "refactor error handling to use thiserror" --model ollama/codegemma

# Anthropic Claude / OpenAI
cortex run "implement user authentication module" --model claude-3-5-sonnet-20241022

# Explicit workspace confinement boundary
cortex run "run cargo check and fix lints" --workspace /path/to/project

# Structured JSON output for headless pipelines and CI/CD
cortex run "verify test suite" --json
```

### Execution History & Deterministic Replay (`cortex runs`)

All execution traces, tool invocations, and model responses are stored in SQLite (`~/.cortex/cortex.db`):

```bash
# List recent execution runs
cortex runs list --limit 20

# Inspect full event trace with verbose tool arguments
cortex runs show <run-id> --verbose

# Bit-exact deterministic replay from recorded model responses
cortex runs replay <run-id>
```

### Deterministic Benchmark Evaluations (`cortex bench`)

Evaluate agent capabilities against verifiable ground-truth problem suites:

```bash
# Run coding evaluation suite
cortex bench run --suite coding

# Run benchmark suite and emit markdown report
cortex bench run --suite coding --report reports/coding.md --json
```

---

## Crate Workspace Topology

| Crate | Path | Responsibility |
|---|---|---|
| **`cortex-core`** | [`crates/cortex-core`](crates/cortex-core) | Domain identifiers (`AgentId`, `RunId`, `SessionId`), error taxonomy, event types, secret redactor |
| **`cortex-runtime`** | [`crates/cortex-runtime`](crates/cortex-runtime) | Execution loop, model providers, sandbox isolation, tool registry, multi-agent bus, SQLite store |
| **`cortex-tui`** | [`crates/cortex-tui`](crates/cortex-tui) | Ratatui terminal user interface, streaming chat, DAG viewer, and slash command harness |
| **`cortex-cli`** | [`crates/cortex-cli`](crates/cortex-cli) | User-facing CLI entrypoint (`cortex`, `cortex run`, `cortex bench`, `cortex runs`) |
| **`cortex-harness`** | [`crates/cortex-harness`](crates/cortex-harness) | Deterministic benchmark evaluation runner and baseline provider implementations |

To embed Cortex primitives inside your own Rust projects:

```toml
[dependencies]
cortex-core = { git = "https://github.com/x1-xh/cortex.git" }
cortex-runtime = { git = "https://github.com/x1-xh/cortex.git" }
cortex-tui = { git = "https://github.com/x1-xh/cortex.git" }
```

---

## Verification & Quality Gates

Cortex adheres to strict automated testing and static analysis standards:

```bash
# Format check
cargo fmt --check

# Type checking
cargo check --workspace --all-targets --all-features

# Strict linting (zero warnings permitted)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Comprehensive workspace test suite
cargo test --workspace --all-targets --all-features

# Documentation build
cargo doc --workspace --no-deps --all-features
```

---

## Documentation

Full architectural documentation, guides, and tutorials are available at:

- **Documentation Portal**: [https://cortex-ai.github.io](https://cortex-ai.github.io)
- **Local Documentation**: [`docs/`](docs/)

---

## License

Licensed under the **Apache License, Version 2.0**. See [`LICENSE`](LICENSE) for details.
