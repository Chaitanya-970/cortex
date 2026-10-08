# Contributing to Cortex

Thank you for your interest in contributing to Cortex!

Cortex is an open-source runtime and harness for autonomous AI workers. All contributions should optimize for maintainability, security, correctness, testability, and contributor experience.

---

## 1. Issue-First Development

Every non-trivial code change should begin with an issue:

1. **Search First**: Check existing GitHub Issues and Pull Requests to avoid duplicate work.
2. **Open an Issue**: File a bug report, feature request, or task describing the problem and proposed design before writing code.
3. **Wait for Alignment**: Discuss substantial architectural additions with maintainers to ensure alignment with our roadmap.
4. **Claiming Work**: Comment `.take` on the issue. The automation bot will automatically assign the issue to you.

---

## 2. Branch Naming Conventions

Always develop on a dedicated branch created from `main`. Name your branches according to their purpose:

| Prefix | Description | Example |
| :--- | :--- | :--- |
| `feat/` | New features or capabilities | `feat/tool-registry` |
| `fix/` | Bug fixes | `fix/workspace-path-traversal` |
| `test/` | Adding or improving tests | `test/scheduler-recovery` |
| `docs/` | Documentation improvements | `docs/agent-lifecycle` |
| `ci/` | CI/CD and automation workflows | `ci/nightly-evals` |
| `security/` | Security controls and sandboxing | `security/env-filtering` |
| `refactor/` | Code refactoring without behavior change | `refactor/runtime-interfaces` |

---

## 3. Commit Conventions

We follow the [Conventional Commits](https://www.conventionalcommits.org/) standard. Commits should be atomic and focused.

```text
<type>(<optional scope>): <description>

[optional body]

[optional footer(s)]
```

### Allowed Types
- `feat`: A new feature
- `fix`: A bug fix
- `docs`: Documentation only changes
- `test`: Adding missing tests or correcting existing tests
- `ci`: Changes to CI configuration files and scripts
- `security`: Security patches, sandboxing, and permission checks
- `refactor`: A code change that neither fixes a bug nor adds a feature
- `chore`: Maintenance tasks and dependency updates

### Examples
```text
feat: add structured tool registry
fix: prevent workspace path traversal
test: add shell tool integration tests
docs: document agent lifecycle
ci: add cargo-deny license verification
security: restrict inherited environment variables
refactor: decouple runtime abstractions from CLI
```

---

## 4. Pull Requests

1. **Open Draft Early**: Feel free to open a Draft PR early if you want feedback on architecture or approach.
2. **Follow PR Template**: Fill out all sections in the pull request template:
   - Summary of changes
   - Closes / references related issue (`Closes #123`)
   - Verification and testing completed
   - Security implications
   - Documentation updates
   - Breaking changes, if any
3. **No Direct Pushes to Main**: All code merges into `main` via reviewed Pull Requests.
4. **Never Weaken CI**: Do not disable, weaken, or ignore CI checks to make a PR pass.

---

## 5. Code Review

- **Respectful & Constructive**: All reviews follow our [Code of Conduct](CODE_OF_CONDUCT.md). Technical feedback is directed at the code, not the author.
- **Architectural Integrity**: Reviewers verify adherence to the [Cortex Engineering Contract](AGENTS.md) and security boundaries.
- **Prompt Iteration**: Authors are expected to address feedback and keep PRs rebased against `main`.

---

## 6. Testing Requirements

Every feature, bug fix, or behavioral change requires corresponding tests:

- **Unit Tests**: Verify individual functions, state transitions, and error handling.
- **Integration Tests**: Verify cross-crate interaction and end-to-end workflows in `tests/`.
- **Security Tests**: Validate authorization checks, path traversal boundaries, and sandbox isolation.
- **Deterministic Evaluations**: When adding agent or tool logic, add deterministic evals under `evals/`.

Never claim tests passed unless you have executed them locally.

---

## 7. Documentation Requirements

- **User-Facing Changes**: Update README, guides, or CLI reference.
- **Architecture Changes**: Update `docs/architecture/` documents.
- **Security Changes**: Update `SECURITY.md` and `docs/security/threat-model.md`.
- **Code Comments**: All public APIs must have doc comments (`#![deny(missing_docs)]`).
- **Changelog**: Document noteworthy changes in `CHANGELOG.md` under `[Unreleased]`.

---

## 8. CI Requirements & Local Verification

Before pushing your branch or submitting a PR, ensure all required checks pass locally:

```bash
# 1. Format check
cargo fmt --check

# 2. Workspace compilation check
cargo check --workspace --all-targets --all-features

# 3. Strict clippy lints (zero warnings allowed)
cargo clippy --workspace --all-targets --all-features -- -D warnings

# 4. Workspace test suite
cargo test --workspace --all-targets --all-features

# 5. Documentation build
cargo doc --workspace --no-deps --all-features

# 6. Workspace build
cargo build --workspace --all-targets --all-features
```

If your change introduces new dependencies, verify them with `cargo-deny`:
```bash
cargo deny check
```
