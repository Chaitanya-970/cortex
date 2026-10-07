# cortex

cortex is an open source agent runtime and harness for autonomous ai workers

## status

implemented
- core domain identifiers agent id run id session id
- error taxonomy cortex error
- trait contracts model provider tool sandbox
- autonomous coding agent loop with filesystem, shell, and git tool registry
- structured event tracing and sqlite persistence (run store & replay)
- reproducible evaluation harness with curated coding/refactor/cli benchmark suites
- terminal-native interactive control plane (ratatui / crossterm)
- cli harness cortex (status, check, runs, bench, tui)
- ci checks fmt check clippy test doc deny

planned
- docker container sandboxing and capability permissions
- multi agent message bus
- distributed scheduler and cron jobs
- mcp protocol integration

## runtime authority

model output is untrusted input

the runtime is the sole execution authority

models never execute system operations directly

all actions pass through validation permissions and sandboxed isolation

## local verification

cargo fmt --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- --deny warnings
cargo test --workspace --all-targets --all-features
cargo doc --workspace --no-deps --all-features
cargo build --workspace --all-targets --all-features

## run

cargo run -- status
cargo run -- check

## license

apache 2
