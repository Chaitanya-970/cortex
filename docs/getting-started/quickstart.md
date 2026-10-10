# Quickstart

Get up and running with Cortex in under 2 minutes. Cortex is designed to provide observable, sandboxed execution environments for autonomous coding agents and multi-agent teams.

---

## 1. Prerequisites

- **Rust**: Version >= 1.75 (`rustup update stable`)
- **Git** & **SQLite**
- *(Optional for 100% free local execution)* **Ollama**: [ollama.com](https://ollama.com) with Google Gemma 4 (`ollama pull gemma4:12b`)

---

## 2. Installation

Clone and install the Cortex unified CLI:

```bash
git clone https://github.com/x1-xh/cortex.git
cd cortex
cargo install --path crates/cortex-cli
```

Verify the installation:

```bash
cortex check
```

---

## 3. Run Your First Agent Task

### Option A: Local Google Gemma 4 (Recommended: 100% Private, Zero Cost)

Cortex provides first-class support for Google Gemma 4 open models. Run completely offline without an API key:

```bash
# Pull Gemma 4 12B
ollama pull gemma4:12b

# Dispatch an autonomous coding task
cortex run "Inspect the git status, review recent commits, and verify cargo check passes" \
  --model ollama/gemma4:12b
```

You can also run Gemma 4 reasoning variants or specialized coding models:

```bash
# Gemma 4 26B for complex refactoring
cortex run "Refactor error handling to use thiserror" --model ollama/gemma4:26b

# CodeGemma for targeted syntax edits
ollama pull codegemma
cortex run "Refactor error handling to use thiserror" --model ollama/codegemma
```

See the [Google Gemma Guide](../guides/gemma.md) for full deployment details (Ollama, llama.cpp, Google AI Studio).

### Option B: Google AI Studio / Gemini API

Run Gemma 4 or Gemini models via Google AI Studio's OpenAI-compatible endpoint:

```bash
export GEMINI_API_KEY="AIzaSy..."
cortex run "Inspect tests and fix any failing assertions" \
  --model gemma-4-26b-it \
  --base-url https://generativelanguage.googleapis.com/v1beta/openai/
```

### Option C: OpenAI or Anthropic

```bash
# OpenAI
export OPENAI_API_KEY="sk-..."
cortex run "Find broken tests in src/ and fix them" --model gpt-4o-mini

# Anthropic Claude
export ANTHROPIC_API_KEY="sk-ant-..."
cortex run "Write comprehensive unit tests for tool parsing" --model claude-3-5-sonnet-20241022
```

---

## 4. Confining the Agent to a Workspace

By default, Cortex confines the agent's filesystem and process execution to the specified `--workspace` path. Path traversal (`../../`) or symlink escapes outside this boundary are blocked:

```bash
cortex run "Implement health check endpoint" \
  --workspace /path/to/my-project \
  --model ollama/gemma4:12b
```

---

## 5. Inspect Traces & Control Plane

All executions are recorded as structured event logs in SQLite (`~/.cortex/cortex.db`).

### View Recent Runs
```bash
cortex runs list --limit 5
cortex runs show <run-id> --verbose
```

### Launch Interactive Terminal UI (TUI)
Simply run `cortex` to launch the Ratatui-powered control plane:

```bash
cortex
```

---

## 6. Run Deterministic Benchmark Evaluations

Verify agent performance using deterministic offline test fixtures:

```bash
cortex bench run --suite coding
```

---

## Next Steps

- [Google Gemma Guide](../guides/gemma.md) — Multi-agent teams, quantized local inference, and hardware acceleration with Gemma.
- [Coding Agent Guide](../guides/coding-agent.md) — Autonomous bug repair workflows and tool authority.
- [CLI Reference](../reference/cli.md) — Comprehensive command syntax and options.
- [Configuration Reference](../reference/configuration.md) — YAML agent manifests and `settings.json`.
