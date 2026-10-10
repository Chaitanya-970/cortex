# Running Autonomous AI Workers with Google Gemma

Cortex provides first-class support for **Google Gemma** open models (Gemma 2 2B, 9B, 27B, and CodeGemma). By pairing Google's lightweight, state-of-the-art open models with Cortex's runtime execution boundaries, sandboxing, and multi-agent coordination, developers can build **100% private, air-gapped, zero-cost autonomous AI workers**.

---

## Why Cortex + Gemma?

- **100% Local Privacy & Air-Gapped Security**: Run agents on private repositories and confidential files without sending tokens or source code to third-party cloud APIs.
- **Strict Runtime Authority**: Gemma models propose structured tool actions, while the Cortex runtime validates schemas, enforces canonical workspace path confinement, and blocks dangerous operations (such as unauthorized network requests or `git push`).
- **Zero Inference Cost**: Execute continuous background schedulers, multi-agent pipelines, and coding tasks with zero per-token API charges.
- **Hardware Optimized**: Leverage Apple Silicon (Metal), NVIDIA CUDA, or CPU quantization via Ollama, llama.cpp, or vLLM with instant sub-millisecond response loops.

---

## Supported Gemma Models

| Model | Parameters | Context | Best Suited For | Minimum RAM |
|---|---|---|---|---|
| **Gemma 2 9B** (`gemma2:9b`) | 9 Billion | 8k tokens | **Recommended Default**: General coding tasks, bug repair, and multi-agent specialist roles. | 8 GB |
| **Gemma 2 27B** (`gemma2:27b`) | 27 Billion | 8k tokens | Complex architectural refactoring, multi-agent supervisor/manager coordination. | 32 GB |
| **Gemma 2 2B** (`gemma2:2b`) | 2 Billion | 8k tokens | Git diff summaries, commit message generation, classification, and edge devices. | 4 GB |
| **CodeGemma** (`codegemma:7b`) | 7 Billion | 8k tokens | Specialized code editing, unit test generation, and syntax refactoring. | 8 GB |

---

## Deployment & Execution Modes

Cortex connects to Gemma through OpenAI-compatible REST endpoints, local runtimes, and hosted cloud providers.

### Mode 1: Local Offline via Ollama (Zero Setup)

The fastest way to get started on macOS, Linux, or Windows:

1. **Pull Gemma model with Ollama**:
   ```bash
   ollama pull gemma2:9b
   ```

2. **Execute an autonomous coding task with Cortex**:
   ```bash
   cortex run "Inspect tests in src/ and fix failing assertions" \
     --model ollama/gemma2:9b
   ```

3. **Or run CodeGemma for targeted patch generation**:
   ```bash
   cortex run "Refactor error handling to use thiserror" \
     --model ollama/codegemma
   ```

Cortex automatically detects `ollama/*` model identifiers and routes requests to `http://localhost:11434/v1` with zero API key requirement.

---

### Mode 2: High-Performance Local Server (llama.cpp / vLLM)

For maximum inference throughput, Metal acceleration on Apple Silicon, or vLLM on NVIDIA GPUs:

1. **Start `llama-server`**:
   ```bash
   ./llama-server -m gemma-2-9b-it-Q4_K_M.gguf \
     --port 8080 \
     --ctx-size 8192 \
     --n-gpu-layers 99
   ```

2. **Run Cortex targeting the local endpoint**:
   ```bash
   cortex run "Audit dependency vulnerabilities and run cargo check" \
     --model gemma-2-9b-it \
     --base-url http://localhost:8080/v1
   ```

---

### Mode 3: Google AI Studio & Gemini API

For developers running Gemma through Google's hosted infrastructure:

1. **Set your Google AI Studio API key**:
   ```bash
   export GEMINI_API_KEY="AIzaSy..."
   ```

2. **Run Gemma 2 through the Google Generative Language endpoint**:
   ```bash
   cortex run "Analyze git commit history and summarize release notes" \
     --model gemma-2-27b-it \
     --base-url https://generativelanguage.googleapis.com/v1beta/openai/
   ```

---

### Mode 4: Ultra-Fast Cloud Inference (Groq / OpenRouter)

For sub-second tool dispatch loops during intensive evaluations:

```bash
# Run Gemma 2 on Groq LPU inference
export OPENAI_API_KEY="gsk_..."
cortex run "Fix calculator bug and commit" \
  --model gemma2-9b-it \
  --base-url https://api.groq.com/openai/v1
```

---

## Setting Gemma as the Default Model

Configure Gemma as your default engine so you can simply type `cortex run "<prompt>"` or use `cortex tui` without passing model flags:

Edit `~/.cortex/settings.json`:

```json
{
  "model": "ollama/gemma2:9b",
  "base_url": "http://localhost:11434/v1",
  "temperature": 0.2,
  "max_tokens": 4096
}
```

Or configure via the Cortex CLI:

```bash
cortex settings set model ollama/gemma2:9b
cortex settings set base_url http://localhost:11434/v1
```

Verify your active runtime configuration:

```bash
cortex check
cortex status
```

---

## Multi-Agent Team Orchestration with Gemma

Cortex enables multi-agent teams where each specialist agent is powered by an appropriately sized Gemma model:

```yaml
version: "1.0"
name: gemma-code-review-team
description: "Autonomous code review pipeline powered by Google Gemma models"

agents:
  # Supervisor requires higher reasoning capacity
  - id: manager
    manifest: agents/manager.yaml
    role: supervisor

  # Specialist for code search and documentation audit
  - id: researcher
    manifest: agents/researcher.yaml
    role: specialist

  # Specialized coding agent
  - id: coder
    manifest: agents/coder.yaml
    role: specialist

  # Policy and security compliance reviewer
  - id: reviewer
    manifest: agents/reviewer.yaml
    role: specialist

stages:
  - id: research
    agent: researcher
    instructions: "Audit test failure logs and trace broken functions."
    routing_key: team.research

  - id: implementation
    agent: coder
    depends_on: [research]
    instructions: "Apply minimal diff patch to fix failing tests."
    routing_key: team.code

  - id: review
    agent: reviewer
    depends_on: [implementation]
    instructions: "Verify git diff adheres to security and formatting policy."
    routing_key: team.review
```

### Agent Manifest Example (`agents/coder.yaml`):

```yaml
id: coder
name: "Gemma Specialist Coder"
role: "Autonomous Bug Fixer"
workspace: "."
model:
  provider: "ollama"
  model: "gemma2:9b"
  temperature: 0.2

permissions:
  filesystem:
    read: true
    write: true
  shell:
    allowed: true
  git:
    read: true
    commit: true
    push: false
```

Execute the workflow:

```bash
cortex workflow run --workflow workflow.yaml
```

---

## Best Practices for Gemma Agents

1. **Lower Temperature (`0.1` - `0.3`)**: For deterministic tool execution, parameter adherence, and syntax accuracy, keep temperature low.
2. **Explicit Workspace Boundaries**: Gemma performs best when given focused instructions and explicit workspace directories.
3. **Structured Verification Loops**: Instruct Gemma to run tests (`cargo test`, `pytest`, `npm test`) via the `shell` tool after making modifications, verifying the fix before reporting completion.
4. **Inspection Before Edits**: Direct Gemma to use `read_file` or `list_directory` before invoking `write_file` to prevent hallucinating file structures.
