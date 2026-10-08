# Installation

Cortex is installed strictly from source or via the Cargo command. Pre-built release binaries and packaging bundles are not distributed.

## 1. Install via Cargo

```bash
# Install globally from local checkout
cargo install --path crates/cortex-cli

# Install globally from git
cargo install --git https://github.com/x1-xh/cortex.git cortex-cli
```

## 2. Build from Source

Prerequisites: Rust >= 1.75, Git, SQLite.

```bash
git clone https://github.com/x1-xh/cortex.git
cd cortex
cargo build --release
```

Binary output: `./target/release/cortex`.
