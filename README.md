# cortex

cortex is an open source agent runtime and harness for autonomous ai workers

## status

implemented
- core domain identifiers agent id run id session id
- error taxonomy cortex error
- trait contracts model provider tool sandbox
- cli harness cortex
- ci checks fmt check clippy test doc build

planned
- agent loop and execution coordinator
- model providers gemma and external apis
- tool registry filesystem shell and git tools
- docker container sandboxing and capability permissions
- structured event tracing and sqlite persistence
- multi agent message bus
- deterministic evaluation harness

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
