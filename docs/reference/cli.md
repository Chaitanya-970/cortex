# CLI Reference

Cortex provides a unified command-line interface for running autonomous AI agents, inspecting execution traces, running benchmark evaluations, and launching the interactive terminal control plane.

```bash
cortex status
cortex check
cortex run "<prompt>" [--model <model>] [--api-key <key>] [--base-url <url>] [--workspace <path>] [--max-iterations <N>] [--json] [--quiet]

cortex runs list [--limit <N>]
cortex runs show <id> [--verbose]
cortex runs replay <id>

cortex bench run [--suite <suite>] [--json] [--report <path>] [--max-iterations <N>]

cortex tui [--db <path>]
```

---

## `cortex run`

Execute an autonomous agent task against real model APIs or local LLM instances.

```bash
cortex run "<prompt>" [OPTIONS]
```

### Options

| Option | Environment Variable | Default | Description |
|---|---|---|---|
| `-m, --model <model>` | `CORTEX_MODEL` | `gpt-4o-mini` | Model identifier (e.g. `gpt-4o-mini`, `gpt-4o`, `claude-3-5-sonnet-20241022`, `deepseek-chat`, `ollama/<model>`). |
| `--api-key <key>` | `CORTEX_API_KEY` | None | API authentication key. Falls back to `OPENAI_API_KEY` or `ANTHROPIC_API_KEY`. |
| `--base-url <url>` | `CORTEX_BASE_URL` | None | Base API URL for OpenAI-compatible endpoints or local servers (`http://localhost:11434/v1`). |
| `-w, --workspace <path>` | None | `.` (current dir) | Confined workspace directory boundary for agent operations. |
| `-i, --max-iterations <N>`| None | `15` | Maximum autonomous model-tool iteration loops. |
| `--db <path>` | `CORTEX_DB_PATH` | `~/.cortex/cortex.db` | SQLite database file for recording execution runs and traces. |
| `-q, --quiet` | None | `false` | Suppress execution banners and print only the final answer. |
| `--json` | None | `false` | Output structured JSON with token counts and estimated USD cost. |

### Examples

#### 1. OpenAI / OpenAI-Compatible
```bash
export OPENAI_API_KEY="sk-..."
cortex run "Refactor error handling in src/model.rs" --model gpt-4o-mini
```

#### 2. Anthropic Claude
```bash
export ANTHROPIC_API_KEY="sk-ant-..."
cortex run "Add comprehensive unit tests for tool registry" --model claude-3-5-sonnet-20241022
```

#### 3. Local Ollama (Zero API Key Required)
```bash
cortex run "Inspect the git status and summarize recent commits" \
  --model ollama/llama3.1 \
  --base-url http://localhost:11434/v1
```

#### 4. JSON Output with Cost Tracking
```bash
cortex run "Fix calculator bug" --json
```

Output:
```json
{
  "run_id": "run_01j7abc...",
  "task": "Fix calculator bug",
  "status": "completed",
  "final_answer": "Fixed bug in add function and verified all tests pass.",
  "iterations": 3,
  "duration_ms": 2840,
  "tokens": {
    "prompt": 1250,
    "completion": 310,
    "total": 1560
  },
  "estimated_cost_usd": 0.000373
}
```

---

## `cortex runs`

Inspect and replay historical execution traces recorded in SQLite.

```bash
# List recent executions
cortex runs list --limit 10

# Inspect run details and full event history
cortex runs show run_01j7abc... --verbose

# Replay a past run deterministically using recorded model outputs
cortex runs replay run_01j7abc...
```

---

## `cortex bench`

Execute deterministic benchmark suites to evaluate autonomous coding capabilities.

```bash
# Run coding benchmark suite
cortex bench run --suite coding

# Output JSON metrics and save Markdown report
cortex bench run --suite coding --json --report reports/coding.md
```

---

## `cortex tui`

Launch the interactive terminal control plane (Ratatui) to navigate live and historical runs, inspect traces, and monitor token usage.

```bash
# Default database
cortex tui

# Custom database
cortex tui --db /path/to/cortex.db
```

See the [TUI Control Plane Guide](../guides/tui.md) for keyboard shortcuts and views.
