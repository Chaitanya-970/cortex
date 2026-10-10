# Coding Agent Guide

Cortex's coding agent operates as an autonomous worker confined to an explicitly configured repository workspace.

---

## 1. Core Operational Principles

The coding agent adheres to a strict policy:

1. **Inspect before modifying**: Read existing files, directory structures, and configurations before proposing changes.
2. **Understand existing code**: Respect the project's architecture, dependencies, and patterns.
3. **Make minimal changes**: Solve the task with the smallest, most targeted edits possible.
4. **Preserve project conventions**: Follow the formatting, linting, and naming rules established in the repository.
5. **Run relevant tests**: Execute tests via the shell tool to evaluate changes.
6. **Inspect failures**: Analyze test failure outputs carefully instead of guessing fixes.
7. **Iterate**: Refine changes until all relevant checks pass.
8. **Review final diff**: Inspect `git diff` before finalizing a commit.
9. **Never claim success without verification**: Confirm that automated tests pass before reporting completion.

---

## 2. Workspace Boundary Enforcement

All filesystem and process operations are bound to a [`Workspace`](crates/cortex-runtime/src/workspace.rs):

- Every file path passed to tools is canonicalized and verified against the workspace root.
- Path traversal attempts (`../../`, symlink escapes, or absolute paths outside the workspace) are blocked with `CortexError::PermissionDenied`.
- Unrelated files outside the workspace cannot be inspected or altered.

---

## 3. Available Tools

### Filesystem
- `read_file`: Reads content from a file within the workspace with optional offset and line limits.
- `write_file`: Writes content to a file, safely creating parent directories if needed.
- `list_directory`: Lists directory entries within the workspace.

### Shell Execution
- `shell`: Executes commands in the workspace directory with output capture and sensitive environment variable scrubbing (`AWS_SECRET_ACCESS_KEY`, API tokens).

### Git Operations
- `git_status`: Displays modified, untracked, and staged files.
- `git_diff`: Displays uncommitted changes.
- `git_log`: Displays recent commit history.
- `git_branch`: Creates or checks out a git branch.
- `git_commit`: Stages specified files and commits with a message.
- `git_push`: **Strictly disabled** by security policy. Any call to push returns `CortexError::PermissionDenied`.

---

## 4. End-to-End Fixture Verification

The coding agent workflow is verified through an automated integration test (`tests/coding_agent_fixture_test.rs`) that:
1. Provisions an intentionally broken test repository.
2. Instructs the agent to inspect the code, execute tests, and observe initial failure.
3. Fixes the underlying code, re-runs tests to observe success, and commits the result.
4. Verifies that unrelated files remain untouched and workspace boundaries are preserved.

---

## 5. Running the Agent with Live LLMs

Use the `cortex run` CLI to dispatch real coding instructions to model providers:

### Local Google Gemma 4 (Recommended: 100% Private, Zero Cost)
```bash
# Gemma 4 12B for autonomous bug diagnosis and repair
cortex run "Run tests, find broken calculator functions, fix them, and commit" \
  --model ollama/gemma4:12b

# Gemma 4 26B for complex refactoring and deep reasoning
cortex run "Refactor configuration parsing to support environment overrides" \
  --model ollama/gemma4:26b

# Hosted Gemma 4 via Google AI Studio
export GEMINI_API_KEY="AIzaSy..."
cortex run "Audit error types and replace unwrap() with proper Result handling" \
  --model gemma-4-26b-it \
  --base-url https://generativelanguage.googleapis.com/v1beta/openai/
```

### Cloud Providers
```bash
# OpenAI GPT-4o-mini
export OPENAI_API_KEY="sk-..."
cortex run "Implement health check endpoint and verify with cargo test"

# Anthropic Claude 3.5 Sonnet
export ANTHROPIC_API_KEY="sk-ant-..."
cortex run "Refactor configuration parsing to support environment overrides" \
  --model claude-3-5-sonnet-20241022
```

All tool calls, command outputs, token usage, and costs are persisted to `~/.cortex/cortex.db` and can be inspected live in `cortex` or with `cortex runs show <run-id>`.

