#!/usr/bin/env bash
set -euo pipefail

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

echo "=== Running Cortex Multi-Agent Code Review Workflow ==="
cd "${SCRIPT_DIR}"

cargo run --manifest-path "${ROOT_DIR}/Cargo.toml" -p cortex-cli --bin cortex -- \
    workflow run workflow.yaml \
    --task "Inspect tests/test_calc.py, identify why safe_divide fails when b is 0, implement fix in src/calc.py, and verify review"
