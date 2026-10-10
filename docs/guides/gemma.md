# Running Autonomous AI Workers with Google Gemma

Cortex provides first-class support for **Google Gemma 4** (the latest open multimodal and agentic reasoning family: Gemma 4 12B, 26B, 31B, and Edge E4B/E2B) as well as Gemma 2 and CodeGemma.

By pairing Google's frontier-grade open-weight models with Cortex's runtime execution boundaries, deterministic workspace sandboxing, and multi-agent coordination, developers can build **100% private, air-gapped, zero-cost autonomous AI workers**.

---

## Why Cortex + Gemma 4?

- **Frontier Reasoning & Thinking Modes**: Gemma 4 introduces native agentic reasoning and step-by-step thinking modes, allowing autonomous workers to plan complex codebase modifications before executing tool calls.
- **Multi-Token Prediction (MTP)**: Gemma 4 leverages multi-token prediction to achieve 2-3x faster token generation on local GPUs and Apple Silicon, dramatically reducing agent iteration latency.
- **100% Local Privacy & Air-Gapped Security**: Run agents on confidential codebases without sending tokens or IP to third-party cloud APIs.
- **Strict Runtime Authority**: Gemma models propose structured tool actions, while the Cortex runtime validates schemas, enforces canonical workspace path confinement, and blocks dangerous operations (such as unauthorized network requests or `git push`).
- **Zero Inference Cost**: Execute continuous background schedulers, multi-agent pipelines, and coding tasks with zero per-token API charges.
- **Permissive Open Licensing**: Gemma 4 is distributed under the permissive Apache 2.0 license, making it ideal for enterprise and open-source deployment.

---

## Supported Gemma Models

| Model Family | Variant | Parameters | Context | Best Suited For | Minimum RAM |
|---|---|---|---|---|---|
| **Gemma 4** | **Gemma 4 12B** (`gemma4:12b` or `gemma4`) | 12 Billion | 128k tokens | **Recommended Default**: General coding tasks, bug repair, and multi-agent specialist roles. | 16 GB |
| **Gemma 4** | **Gemma 4 26B / 31B** (`gemma4:26b`, `gemma4:31b`) | 26B / 31B | 128k tokens | Complex architectural refactoring, multi-agent supervisor/manager coordination, deep reasoning. | 32 GB |
| **Gemma 4 Edge** | **Gemma 4 E4B / E2B** (`gemma4:e4b`, `gemma4:e2b`) | 2B / 4B | 32k tokens | Git diff summaries, commit message generation, classification, and edge/mobile devices. | 4 - 8 GB |
| **Gemma 2** | **Gemma 2 9B** (`gemma2:9b`) | 9 Billion | 8k tokens | Lightweight local coding and instruction following. | 8 GB |
| **Gemma 2** | **Gemma 2 27B** (`gemma2:27b`) | 27 Billion | 8k tokens | High-parameter reasoning on workstations. | 32 GB |
| **CodeGemma** | **CodeGemma 7B** (`codegemma:7b`) | 7 Billion | 8k tokens | Specialized code editing, unit test generation, and syntax refactoring. | 8 GB |

---

## Deployment & Execution Modes

Cortex connects to Gemma through OpenAI-compatible REST endpoints, local runtimes, and hosted cloud providers.

### Mode 1: Local Offline via Ollama (Zero Setup)

The fastest way to get started on macOS, Linux, or Windows:

1. **Pull Gemma 4 with Ollama**:
   ```bash
   # Pull Gemma 4 12B (recommended default)
   ollama pull gemma4:12b

   # Or pull Gemma 4 high-reasoning variant
   ollama pull gemma4:26b
   ```

2. **Execute an autonomous coding task with Cortex**:
   ```bash
   cortex run "Inspect tests in src/ and fix failing assertions" \
     --model ollama/gemma4:12b
   ```

3. **Or run Gemma 4 Edge for ultra-fast git operations**:
   ```bash
   cortex run "Inspect git status and write descriptive commit message" \
     --model ollama/gemma4:e4b
   ```

Cortex automatically detects `ollama/*` model identifiers and routes requests to `http://localhost:11434/v1` with zero API key requirement.

---

### Mode 2: High-Performance Local Server (llama.cpp / vLLM)

For maximum inference throughput, Metal acceleration on Apple Silicon, or vLLM on NVIDIA GPUs:

1. **Start `llama-server`**:
   ```bash
   ./llama-server -m gemma-4-12b-it-Q4_K_M.gguf \
     --port 8080 \
     --ctx-size 16384 \
     --n-gpu-layers 99
   ```

2. **Run Cortex targeting the local endpoint**:
   ```bash
   cortex run "Audit dependency vulnerabilities and run cargo check" \
     --model gemma-4-12b-it \
     --base-url http://localhost:8080/v1
   ```

---

### Mode 3: Google AI Studio & Gemini API

For developers running Gemma through Google Cloud and Google AI Studio:

1. **Set your Google AI Studio API key**:
   ```bash
   export GEMINI_API_KEY="AIzaSy..."
   ```

2. **Run Gemma through the Google Generative Language endpoint**:
   ```bash
   cortex run "Analyze git commit history and summarize release notes" \
     --model gemma-4-26b-it \
     --base-url https://generativelanguage.googleapis.com/v1beta/openai/
   ```

---

### Mode 4: Ultra-Fast Cloud Inference (Groq / OpenRouter)

For sub-second tool dispatch loops during intensive evaluations:

```bash
# Run Gemma 4 on Groq LPU inference
export OPENAI_API_KEY="gsk_..."
cortex run "Fix calculator bug and commit" \
  --model gemma4-12b-it \
  --base-url https://api.groq.com/openai/v1
```

---

## Setting Gemma 4 as the Default Model

Configure Gemma 4 as your default engine so you can simply type `cortex run "<prompt>"` or use `cortex tui` without passing model flags:

Edit `~/.cortex/settings.json`:

```json
{
  "model": "ollama/gemma4:12b",
  "base_url": "http://localhost:11434/v1",
  "temperature": 0.2,
  "max_tokens": 8192
}
```

Or configure via the Cortex CLI:

```bash
cortex settings set model ollama/gemma4:12b
cortex settings set base_url http://localhost:11434/v1
```

Verify your active runtime configuration:

```bash
cortex check
cortex status
```

---

## Multi-Agent Team Orchestration with Gemma 4

Cortex enables multi-agent teams where each specialist agent is powered by an appropriately sized Gemma model:

```yaml
version: "1.0"
name: gemma4-code-review-team
description: "Autonomous code review pipeline powered by Google Gemma 4 models"

agents:
  # Supervisor requires deep reasoning capacity
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
name: "Gemma 4 Specialist Coder"
role: "Autonomous Bug Fixer"
workspace: "."
model:
  provider: "ollama"
  model: "gemma4:12b"
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

## Best Practices for Gemma 4 Agents

1. **Lower Temperature (`0.1` - `0.3`)**: For deterministic tool execution, parameter adherence, and syntax accuracy, keep temperature low.
2. **Utilize Thinking / Reasoning**: For complex tasks, Gemma 4's reasoning mode allows the model to analyze failure traces before calling edit tools.
3. **Explicit Workspace Boundaries**: Gemma performs best when given focused instructions and explicit workspace boundaries.
4. **Structured Verification Loops**: Instruct Gemma to run tests (`cargo test`, `pytest`, `npm test`) via the `shell` tool after making modifications, verifying the fix before reporting completion.
5. **Inspection Before Edits**: Direct Gemma to use `read_file` or `list_directory` before invoking `write_file` to prevent hallucinating file structures.
