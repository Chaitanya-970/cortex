# Configuration Reference

Cortex supports both global user settings (`~/.cortex/settings.json`) and declarative agent manifests (`agents/*.yaml`). All configurations are strictly validated before execution.

---

## 1. Global User Settings (`~/.cortex/settings.json`)

Global defaults apply to all `cortex run` invocations when CLI flags are not provided:

```json
{
  "model": "ollama/gemma2:9b",
  "base_url": "http://localhost:11434/v1",
  "temperature": 0.2,
  "max_tokens": 4096,
  "max_iterations": 15,
  "db_path": "~/.cortex/cortex.db"
}
```

### Supported Settings Fields

| Field | Type | Default | Description |
|---|---|---|---|
| `model` | string | `gpt-4o-mini` | Default model identifier (e.g. `ollama/gemma2:9b`, `ollama/codegemma`, `gpt-4o-mini`, `claude-3-5-sonnet-20241022`). |
| `base_url` | string | `null` | Base URL for OpenAI-compatible REST endpoints (auto-detected for `ollama/*` as `http://localhost:11434/v1`). |
| `temperature` | float | `0.2` | Sampling temperature (`0.0` - `1.0`). Keep low (`0.1`-`0.3`) for deterministic coding agents. |
| `max_tokens` | integer | `4096` | Maximum generation tokens per model response step. |
| `max_iterations` | integer | `15` | Default maximum tool-loop iterations before aborting. |
| `db_path` | string | `~/.cortex/cortex.db` | Path to the SQLite append-only trace database. |

Manage settings directly via the CLI:

```bash
cortex settings get model
cortex settings set model ollama/gemma2:9b
cortex settings set base_url http://localhost:11434/v1
```

---

## 2. Agent Manifest Specification (`agent.yaml`)

Autonomous workers can be defined declaratively in YAML, JSON, or TOML files.

### Example: Google Gemma 2 Coding Agent

```yaml
version: "1.0"
id: gemma-coder
name: "Gemma Autonomous Coder"
role: "Software Engineer"
description: "Specialized coding worker powered by Google Gemma 2 9B"

workspace: "./my-project"

model:
  provider: "ollama"
  model: "gemma2:9b"
  base_url: "http://localhost:11434/v1"
  temperature: 0.2
  max_tokens: 4096

tools:
  - filesystem
  - shell
  - git

permissions:
  filesystem:
    read: true
    write: true
    allow_paths: ["./src", "./tests", "Cargo.toml"]
    deny_paths: [".git", ".env", "*.pem"]
  shell:
    allowed: true
    blocked_commands: ["curl", "wget", "rm -rf /"]
  git:
    read: true
    commit: true
    push: false

memory:
  enabled: true
  max_history_tokens: 8192
```

### Example: Google CodeGemma Patch Specialist

```yaml
version: "1.0"
id: codegemma-refactorer
name: "CodeGemma Refactorer"
role: "Code Specialist"
description: "Precision code modification using Google CodeGemma"

workspace: "."

model:
  provider: "ollama"
  model: "codegemma:7b"
  temperature: 0.1

tools:
  - filesystem
  - shell

permissions:
  filesystem:
    read: true
    write: true
  shell:
    allowed: true
```

### Example: Cloud-Hosted Gemma 2 (Google AI Studio)

```yaml
version: "1.0"
id: gemma-cloud-reviewer
name: "Gemma Cloud Reviewer"
role: "Architecture Reviewer"
workspace: "."

model:
  provider: "google"
  model: "gemma-2-27b-it"
  base_url: "https://generativelanguage.googleapis.com/v1beta/openai/"
  api_key_env: "GEMINI_API_KEY"
  temperature: 0.3

tools:
  - filesystem
  - git

permissions:
  filesystem:
    read: true
    write: false
  git:
    read: true
```

---

## 3. Security & Permission Boundaries

Cortex implements capability-based security. Tools cannot execute actions outside their explicit permissions:

### Filesystem Permissions
- `read`: Allow reading files within the workspace.
- `write`: Allow creating or modifying files within the workspace.
- `allow_paths`: Optional whitelist of workspace subpaths.
- `deny_paths`: Blacklist of patterns that will always be denied (e.g. `.env`, `.git`, `id_rsa`).

### Shell Permissions
- `allowed`: Enable or disable process execution.
- `blocked_commands`: Commands that will be rejected immediately upon inspection.
- Environment variables: Sensitive keys (`AWS_SECRET_ACCESS_KEY`, `*API_KEY*`, `*TOKEN*`) are scrubbed before command execution.

### Git Permissions
- `read`: `git status`, `git diff`, `git log`, `git branch`.
- `commit`: `git commit` to stage and record modifications.
- `push`: **Strictly denied** by default. Any attempt to invoke `git push` returns `CortexError::PermissionDenied`.

---

## 4. Multi-Agent Team Configuration (`workflow.yaml`)

Coordinate multiple Gemma-powered agents in a structured workflow:

```yaml
version: "1.0"
name: gemma-triage-and-fix

agents:
  - id: manager
    manifest: agents/manager.yaml
    role: supervisor

  - id: coder
    manifest: agents/coder.yaml
    role: specialist

stages:
  - id: diagnosis
    agent: manager
    instructions: "Inspect test failure logs and isolate root causes."

  - id: fix
    agent: coder
    depends_on: [diagnosis]
    instructions: "Apply minimal code patch and ensure test suite passes."
```

Execute with:

```bash
cortex workflow run workflow.yaml
```
