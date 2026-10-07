# Cortex Engineering Contract

Cortex is a long-lived open-source Rust project.

It is NOT a throwaway hackathon project.

## Priorities

Prioritize:

- maintainability
- security
- correctness
- testability
- backwards compatibility
- contributor experience
- observability
- documentation

## Before coding

1. Inspect the repository.
2. Read relevant architecture documentation.
3. Inspect related issues.
4. Understand existing abstractions.
5. Avoid duplicate functionality.

## Implementation rules

1. Prefer small composable modules.
2. Minimize dependencies.
3. Avoid unnecessary global mutable state.
4. Do not bypass existing abstractions.
5. Do not create duplicate execution paths.
6. Keep security boundaries explicit.
7. Keep model output separate from runtime authority.
8. Never allow model output to bypass permissions.
9. Never weaken tests or CI.
10. Never silently ignore failures.
11. Avoid unrelated edits.

## Testing

Every feature needs appropriate tests:

- unit tests
- integration tests
- security tests
- deterministic evaluations
- benchmarks where appropriate

Before submitting:

```bash
cargo fmt --check
cargo check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo doc
```

## Documentation

User-facing changes should update documentation.

Architecture changes should update architecture documentation.

Security-sensitive changes should update security documentation.

## Truthfulness

Never claim something works without testing it.

Never claim CI passes unless CI actually passed.

## Commits

Use focused, descriptive commits.

```text
feat: add agent message bus
fix: prevent workspace path traversal
test: cover scheduler restart recovery
docs: document tool permissions
ci: add nightly evaluation workflow
security: restrict inherited environment variables
```
