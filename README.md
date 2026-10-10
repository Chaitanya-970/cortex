# Cortex

<p align="center">
  <strong>Deterministic Rust runtime and execution harness for autonomous AI workers</strong>
</p>

<p align="center">
  <a href="https://github.com/x1-xh/cortex/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License: Apache-2.0"></a>
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/rust-1.75%2B-orange.svg" alt="Rust: 1.75+"></a>
  <a href="https://github.com/x1-xh/cortex/actions"><img src="https://img.shields.io/badge/build-passing-brightgreen.svg" alt="Build Status"></a>
  <a href="https://cortex-ai.github.io"><img src="https://img.shields.io/badge/docs-cortex--ai.github.io-0284c7.svg" alt="Documentation"></a>
</p>

<p align="center">
  <img src="assets/cortex-tui.png" alt="Cortex Terminal Interface" width="850">
</p>

---

## Architectural Principles

```
  +-------------------------------------------------------------------------+
  |                             Untrusted Model                             |
  |                (Gemma 4 / Ollama / Google AI Studio / Claude / OpenAI)   |
  +-----------------------------------+-------------------------------------+
                                      | Structured JSON Tool Proposals
                                      v
  +-------------------------------------------------------------------------+
  |                         Cortex Runtime Authority                        |
  |  +------------------------+  +-------------------+  +----------------+  |
  |  | Schema & Policy Filter |  | Path Canonicalizer|  | Env Sanitizer  |  |
  |  +------------------------+  +-------------------+  +----------------+  |
  +------------------+-----------------------+--------------------+---------+
                     |                       |                    |
                     v                       v                    v
          +--------------------+   +-------------------+   +---------------+
          | Sandboxed Filesys  |   | Subprocess Runner |   | MCP Bridge    |
          | (Boundary Checked) |   | (Env Scrubbed)    |   | (JSON-RPC)    |
          +--------------------+   +-------------------+   +---------------+
                     |                       |                    |
                     +-----------------------+--------------------+
                                             |
                                             v Execution Events
  +-------------------------------------------------------------------------+
  |                SQLite WAL Append-Only Trace Storage                     |
  |              (~/.cortex/cortex.db with regex secret redaction)          |
  +-------------------------------------------------------------------------+
```

### Security Invariants

1. **Model output is untrusted proposal data**: LLMs never hold execution authority. Every model completion is parsed into structured tool call proposals and evaluated against explicit runtime policy filters before dispatch.
2. **Strict workspace containment**: File system tools resolve paths against canonical absolute roots via `std::fs::canonicalize`. Path traversal sequences (`../`), relative directory breakouts, and symlink escapes outside the target root trigger non-recoverable security policy violations.
3. **Subprocess isolation and credential scrubbing**: Shell tasks run under bounded execution timeouts (default 60s) with stripped environment variables. Ambient authentication tokens (`*_API_KEY`, `*_SECRET`, `*_TOKEN`, `AWS_*`, `GITHUB_*`) are scrubbed before child process instantiation.
4. **Destructive command mitigation**: High-risk operations (e.g., unauthorized `git push`, arbitrary root write attempts) are denied at the runtime interceptor boundary.
5. **Deterministic trace auditability**: All state transitions, prompts, tool inputs, outputs, and token counts are persisted to SQLite with automatic credential redaction prior to serialization.

---

## System Architecture

The codebase is organized into modular crates with clean separation of domain boundaries:

```
crates/
├── cortex-core/       # Domain primitives, IDs, events, errors, secret redactor
├── cortex-runtime/    # Execution loop, sandbox policies, tool registry, SQLite store
├── cortex-tui/        # Ratatui terminal UI, event loop, slash command parser
├── cortex-cli/        # Binary CLI entrypoint, argument parsing, workflow dispatch
└── cortex-harness/    # Deterministic ground-truth evaluation suites & baselines
```

### Runtime Loop (`cortex-runtime`)

The core execution engine implements an iterative evaluation state machine:

$$\text{State}_{t+1} = \text{Execute}(\text{Runtime}, \text{Model}(\text{Context}_t))$$

1. **Context Assembly**: Current workspace state, file index, tool definitions, and prior trajectory are formatted into structured system/user frames.
2. **Model Invocation**: Dispatched to the configured `ModelProvider` endpoint via streaming SSE or standard completion calls.
3. **Proposal Validation**: Proposed tool calls are deserialized against strongly typed JSON schemas registered in `ToolRegistry`.
4. **Policy Enforcement**: Sandboxing boundary verification, path canonicalization, and argument inspection.
5. **Execution & Event Recording**: Tools execute; structured `ExecutionEvent` records are appended to SQLite in WAL mode.
6. **Termination Conditions**: The loop terminates when the model produces a final answer without tool calls, hits `max_iterations`, or receives a cancellation signal (`SIGINT` / `Ctrl+C`).

### Storage Engine (`RunStore`)

Persistence is handled by SQLite (`~/.cortex/cortex.db`) configured with:

- `PRAGMA journal_mode = WAL;` (Write-Ahead Logging for concurrent non-blocking reads)
- `PRAGMA synchronous = NORMAL;`
- `PRAGMA busy_timeout = 5000;`

Schema tables:
- `runs`: Run metadata, task prompt, status (`running`, `completed`, `failed`, `cancelled`), timestamps, token counters, estimated USD cost.
- `events`: Monotonic sequenced append-only event stream recording `RunStarted`, `ToolStarted`, `ToolCompleted`, `ToolFailed`, `ModelRequest`, `ModelResponse`, and `InterAgentMessage`.

---

## Installation

### Prerequisites

- Rust toolchain $\ge$ 1.75 (`cargo`, `rustc`)
- Git
- SQLite 3 (bundled at compile-time via `rusqlite`)

### Building from Source

```bash
git clone https://github.com/x1-xh/cortex.git
cd cortex
cargo build --release
```

Binary output: `./target/release/cortex`

### Global Installation

```bash
# Install from local checkout
cargo install --path crates/cortex-cli

# Install directly from Git repository
cargo install --git https://github.com/x1-xh/cortex.git cortex-cli
```

Verify binary availability:

```bash
cortex --version
cortex check
cortex status
```

---

## CLI Reference

### Interactive Terminal Control Plane (`cortex`)

Running `cortex` without arguments launches the terminal control plane implemented with Ratatui:

```bash
cortex
cortex --db /path/to/cortex.db
```

#### View Tabs

- `1` **Chat**: Interactive session prompt, real-time reasoning trace streaming, tool invocation diffs.
- `2` **Portals**: Inference provider catalog, base URLs, credentials, active model dispatch.
- `3` **Coordination**: Inter-agent message bus feed, DAG pipeline status, agent topology cards.
- `4` **Runs**: Paginated historical execution runs queried from SQLite.
- `5` **Settings**: User settings view (`~/.cortex/settings.json`), sandbox boundaries, token telemetry.

#### Navigation Keybindings

- `Tab` / `Shift+Tab`: Cycle active view tabs.
- `1` – `5`: Direct tab jumps.
- `/`: Open slash command autocomplete popup.
- `Ctrl+C`: Cancel running agent iteration (double `Ctrl+C` terminates the harness).
- `Ctrl+Q`: Exit application.

#### Supported Slash Commands

- `/help`: Display command catalog and keyboard shortcuts.
- `/model [list | set <name>]`: Inspect or switch active model provider.
- `/settings [set <key> <val> | reload]`: Inspect or mutate `~/.cortex/settings.json`.
- `/status`: Runtime diagnostics, workspace root, index status, and active portal.
- `/diff`: Output uncommitted Git working tree diff within workspace boundary.
- `/clear`: Flush current conversation buffer and archive session state to disk.
- `/sessions` / `/resume <id>`: Session serialization and resumption.
- `/cost`: Cumulative session token accounting and pricing breakdown.

---

### Autonomous Execution (`cortex run`)

Executes bounded agent tasks programmatically:

```bash
# Google Gemma 4 via local Ollama endpoint (zero external network dependency)
cortex run "fix lifetime errors in src/agent/loop.rs" --model ollama/gemma4:12b

# Google Gemma 4 via Google AI Studio / Gemini API endpoint
export GEMINI_API_KEY="AIza..."
cortex run "audit memory bounds across crates" \
  --model gemma-4-26b-it \
  --base-url https://generativelanguage.googleapis.com/v1beta/openai/

# CodeGemma local refactoring
cortex run "convert error enum to thiserror derive" --model ollama/codegemma

# Anthropic Claude / OpenAI
cortex run "implement DAG topological sorting" --model claude-3-5-sonnet-20241022

# Confined to explicit workspace directory
cortex run "cargo clippy --fix" --workspace /path/to/target/repo

# Structured JSON output for automated test harnesses and CI
cortex run "cargo test" --json
```

### Run Inspection & Deterministic Replay (`cortex runs`)

```bash
# Query execution history
cortex runs list --limit 20

# Detailed event telemetry inspection
cortex runs show <run-id> --verbose

# Replay run deterministically against cached model responses
cortex runs replay <run-id>
```

### Benchmark Evaluation Harness (`cortex bench`)

Executes evaluation suites against deterministic ground-truth tasks:

```bash
cortex bench run --suite coding
cortex bench run --suite coding --report reports/coding.md --json
```

---

## Configuration (`~/.cortex/settings.json`)

Runtime defaults are configured via `~/.cortex/settings.json`:

```json
{
  "model": "gpt-4o-mini",
  "base_url": null,
  "api_key": null,
  "theme": "blue",
  "max_iterations": 30,
  "auto_save_sessions": true
}
```

Authentication keys are resolved with the following precedence:
1. Command-line flags (`--api-key`, `--base-url`)
2. Environment variables (`CORTEX_API_KEY`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GEMINI_API_KEY`)
3. User configuration file (`~/.cortex/settings.json`)

---

## Workspace Crate Specifications

| Crate | Directory | Primary Types & Functions |
|---|---|---|
| `cortex-core` | [`crates/cortex-core`](crates/cortex-core) | `RunId`, `AgentId`, `ExecutionEvent`, `EventRecord`, `CortexError`, `Redactor`, `UserSettings` |
| `cortex-runtime` | [`crates/cortex-runtime`](crates/cortex-runtime) | `AgentLoop`, `Workspace`, `ToolRegistry`, `ModelProvider`, `RunStore`, `SchedulerEngine`, `McpManager` |
| `cortex-tui` | [`crates/cortex-tui`](crates/cortex-tui) | `App`, `ui::render`, `event::handle_key`, `commands::execute_command`, `theme` |
| `cortex-cli` | [`crates/cortex-cli`](crates/cortex-cli) | `Cli`, `Commands::Run`, `Commands::Runs`, `Commands::Bench`, `Commands::Tui` |
| `cortex-harness` | [`crates/cortex-harness`](crates/cortex-harness) | `BenchmarkRunner`, `BenchmarkBaselineProvider`, `EvaluationSuite` |

### Embedding Cortex as a Dependency

```toml
[dependencies]
cortex-core = { git = "https://github.com/x1-xh/cortex.git" }
cortex-runtime = { git = "https://github.com/x1-xh/cortex.git" }
cortex-tui = { git = "https://github.com/x1-xh/cortex.git" }
```

---

## Verification & Testing

Every commit must satisfy all static analysis and deterministic verification checks:

```bash
# Code formatting compliance
cargo fmt --check

# Type check across all targets and features
cargo check --workspace --all-targets --all-features

# Static analysis and linter validation (-D warnings)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Unit, integration, and security test suites
cargo test --workspace --all-targets --all-features

# API documentation compilation
cargo doc --workspace --no-deps --all-features
```

---

## Documentation

- Architecture and security threat model: [`docs/architecture/`](docs/architecture/)
- Configuration and tool interfaces: [`docs/reference/`](docs/reference/)
- Documentation portal: [https://cortex-ai.github.io](https://cortex-ai.github.io)

---

## License

Apache License, Version 2.0. See [`LICENSE`](LICENSE) for complete text.
