# Benchmark Evaluation Harness

The Cortex benchmark harness (`cortex-harness`) provides reproducible, automated evaluation of agent task performance across curated problem suites.

## Evaluation Workflow

```
Benchmark Task Definition (Prompt + Fixtures + Verification Command)
                           │
                           ▼
             Isolated Workspace Sandbox
                           │
                           ▼
                  Agent Execution Loop
                           │
                           ▼
          Ground Truth Verification Command
                           │
                           ▼
               TaskOutcome & Metrics
```

1. **Isolation**: Each benchmark task runs in a clean temporary workspace initialized as a Git repository.
2. **Execution**: The agent interacts with workspace files using standard filesystem, git, and shell tools.
3. **Verification**: After the agent yields completion, the task's deterministic verification command (e.g., `rustc`, `python3 test_solution.py`) runs.
4. **Metrics**: Metrics are aggregated across the suite and formatted as JSON or Markdown.

## Curated Benchmark Suites

### 1. `coding` Suite (Code Repair)

Contains 5 realistic bug-fix tasks with automated test assertions:

- `coding-01-rust-syntax`: Fix syntax delimiter and semicolon error in Rust module.
- `coding-02-python-boundary`: Fix off-by-one boundary bug in binary search algorithm.
- `coding-03-rust-logic`: Fix arithmetic inversion in helper function.
- `coding-04-python-import`: Add missing standard library import to resolve undefined name.
- `coding-05-python-exception`: Safely handle missing optional dictionary key in event parser.

### 2. `refactor` Suite

- `refactor-01-extract-helper`: Extract duplicated validation logic into helper function.

### 3. `cli` Suite

- `cli-01-flag-handling`: Add `--verbose` flag handling to command argument processor.

## CLI Usage

Run a benchmark suite and view results in the terminal:

```bash
cortex bench run --suite coding
```

### Export Markdown Report

```bash
cortex bench run --suite coding --report eval-report.md
```

### Export JSON Summary

```bash
cortex bench run --suite coding --json
```

## Metrics Collected

| Metric | Description |
|---|---|
| `success_rate` | Percentage of tasks passing all verification tests |
| `step_count` | Number of model-tool iterations consumed |
| `duration_ms` | Elapsed wall-clock time in milliseconds |
| `tokens_consumed` | Total prompt and completion tokens |
| `tool_error_rate` | Ratio of tool failures per step |

## CI Integration

In continuous integration, `cortex bench run` exits with code `0` if all tasks pass and code `1` if any task fails.
