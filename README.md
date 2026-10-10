# cortex

open source rust runtime and harness for autonomous ai workers — built for Google Gemma 4, open models, and multi-agent coordination

## runtime authority & security invariants

model output is untrusted input

the runtime is the sole execution authority

- models propose structured tool calls; runtime validates schemas, evaluates permissions, and executes
- workspace boundary enforcement: canonicalized path resolution, traversal denial (`../`), symlink escape prevention
- process isolation: subprocess execution confined to workspace root with sensitive environment variable scrubbing
- destructive operations restricted: `git push` denied by runtime policy
- append-only trace logging with automated secret redaction in sqlite

## clone & build

prerequisites: rust >= 1.75, git, sqlite

```bash
git clone https://github.com/x1-xh/cortex.git
cd cortex
cargo build --release
```

binary output: `./target/release/cortex`

## install

```bash
# install globally from local checkout
cargo install --path crates/cortex-cli

# install globally from git
cargo install --git https://github.com/x1-xh/cortex.git cortex-cli
```

ensure cargo binary directory (`~/.cargo/bin`) is in `$PATH`:

```bash
# current session
export PATH="$HOME/.cargo/bin:$PATH"

# persistent configuration (zsh / bash)
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc   # zsh
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc  # bash
```

verification:

```bash
cortex --version
cortex check
cortex status
```

## crate dependencies

to embed cortex runtime primitives in a cargo workspace:

```toml
[dependencies]
cortex-core = { git = "https://github.com/x1-xh/cortex.git" }
cortex-runtime = { git = "https://github.com/x1-xh/cortex.git" }
cortex-harness = { git = "https://github.com/x1-xh/cortex.git" }
cortex-tui = { git = "https://github.com/x1-xh/cortex.git" }
```

## cli harness

### run agent tasks

autonomous iterative loop (model -> tool -> result -> model) with workspace confinement, token metrics, and usd cost calculation:

```bash
# google gemma 4 (local offline via ollama, zero api key required)
cortex run "inspect src/lib.rs and fix compiler warnings" --model ollama/gemma4:12b

# google gemma 4 (hosted via google ai studio / gemini api)
export GEMINI_API_KEY="AIza..."
cortex run "audit repository security" --model gemma-4-26b-it --base-url https://generativelanguage.googleapis.com/v1beta/openai/

# codegemma / gemma 2 for specialized editing
cortex run "refactor error handling to use thiserror" --model ollama/codegemma

# openai / anthropic
cortex run "refactor error handling" --model claude-3-5-sonnet-20241022

# explicit workspace boundary
cortex run "run cargo check and fix lints" --workspace /path/to/project

# structured json output with token metrics and usd cost estimation
cortex run "verify test suite" --json
```

### inspect execution runs & replay

execution runs, tool invocations, and metrics persisted to sqlite (`~/.cortex/cortex.db`):

```bash
# list execution history
cortex runs list --limit 20

# inspect full event trace, tool calls, and model outputs
cortex runs show <run-id> --verbose

# deterministic replay from recorded model responses
cortex runs replay <run-id>
```

### terminal control plane (tui)

ratatui terminal control plane for live and historical execution inspection:

```bash
cortex tui
cortex tui --db /path/to/cortex.db
```

views:
- `1` dashboard: active run telemetry, sqlite health, run counter
- `2` agents: agent registry, sandbox policies, tool permissions
- `3` active run: execution metadata, prompt/completion token consumption, duration, estimated cost
- `4` events: chronological event log table with formatted json payload inspector
- `5` history: searchable sqlite execution runs
- `6` tasks & bench: ground truth task browser and verification command viewer

navigation: `tab` / `shift+tab`, `1`-`6`, `j`/`k`, `enter`, `r` (reload), `q` (quit)

### benchmark evaluations

deterministic evaluation suites against verifiable ground truth:

```bash
cortex bench run --suite coding
cortex bench run --suite coding --report reports/coding.md --json
```

## crate topology

- `crates/cortex-core`: domain identifiers (`agent_id`, `run_id`, `session_id`), error taxonomy (`cortex_error`), execution events (`execution_event`), secret redactor
- `crates/cortex-runtime`: agent execution loop (`agent_loop`), model provider abstractions (openai `/chat/completions`, anthropic `/v1/messages`, ollama), workspace isolation (`workspace`), tool registry (`read_file`, `write_file`, `list_directory`, `shell`, git tools), sqlite storage (`run_store`)
- `crates/cortex-cli`: user-facing cli entrypoint (`cortex`)
- `crates/cortex-harness`: benchmark evaluation runner and baseline providers
- `crates/cortex-tui`: ratatui terminal user interface and execution inspector

## verification

```bash
cargo fmt --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- --deny warnings
cargo test --workspace --all-targets --all-features
cargo doc --workspace --no-deps --all-features
cargo deny check
```

## status

implemented:
- core domain types and error taxonomy
- model provider abstraction (openai `/chat/completions`, anthropic `/messages`, ollama)
- token accumulation (`prompt_tokens`, `completion_tokens`, `total_tokens`) and usd cost calculation
- workspace path containment and boundary enforcement
- tool registry: filesystem, shell execution with token scrubbing, git tools
- persistent sqlite event tracing (`run_store`) and deterministic replay
- ratatui terminal control plane (`cortex tui`)
- benchmark evaluation harness (`cortex bench`)
- cli harness (`cortex run`, `cortex runs`, `cortex bench`, `cortex tui`)

planned:
- docker container sandbox execution
- multi-agent message bus and task queues
- persistent cron scheduler
- model context protocol (mcp) discovery

## license

apache-2.0
