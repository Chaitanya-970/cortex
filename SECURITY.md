# Security Policy

Security is a foundational concern in Cortex because autonomous agents will eventually execute model-generated commands across filesystems, shells, networks, Git repositories, and external tools.

---

## 1. Preliminary Threat Model

In Cortex, **model output is always treated as untrusted input**. The runtime is the sole execution authority.

```text
┌─────────────────────────────────┐
│     Model Output (Untrusted)    │
└────────────────┬────────────────┘
                 │ Proposes structured action
                 ▼
┌─────────────────────────────────┐
│  Runtime Schema Validation      │
└────────────────┬────────────────┘
                 │ Valid action payload
                 ▼
┌─────────────────────────────────┐
│ Capability & Permission Policy  │
└────────────────┬────────────────┘
                 │ Authorized capability
                 ▼
┌─────────────────────────────────┐
│  Sandbox Boundary (Isolation)   │
└────────────────┬────────────────┘
                 │ Confined execution
                 ▼
┌─────────────────────────────────┐
│         Tool Execution          │
└─────────────────────────────────┘
```

### Core Tenet
> **A model must never directly bypass the permission or sandbox layer.**
> Prompt instructions and system prompts are *not* a security boundary. All constraints and boundaries must be enforced by the runtime.

---

## 2. High-Risk Capabilities

Because Cortex executes actions autonomously, several capabilities present high security risks:

- **Arbitrary Shell Execution**: Running commands that could modify system state or escape process confines.
- **Filesystem Writes & Deletions**: Potential directory traversal, modifying files outside the designated workspace.
- **Network Access**: Outbound requests that could leak credentials or exfiltrate private repository data.
- **Package Installation**: Executing setup scripts with arbitrary host execution.
- **Git Operations**: Unauthorized commits, branch modifications, or pushing to remote repositories.
- **Model Context Protocol (MCP)**: Connecting to untrusted external MCP servers that declare unverified tools.
- **Third-Party Agent Skills**: Loading executable scripts or hooks from external sources.

All high-risk capabilities require explicit configuration, strict capability checks, and, where configured, interactive human approval.

---

## 3. Sandboxing & Isolation Strategy

Cortex employs defense-in-depth through containerized and operating-system-level isolation:

- **Isolated Filesystems**: The agent is restricted to mounting only the explicit workspace directory.
- **Environment Scrubbing**: Sensitive host environment variables (API tokens, SSH keys, credentials) are stripped before spawning subprocesses.
- **Network Policy**: Sandboxes default to no network access unless explicitly authorized for a task.
- **Resource Limits & Timeouts**: Strict memory caps, CPU quotas, and timeout enforcement prevent resource exhaustion and hanging processes.
- **Non-Root Execution**: Commands run with minimal non-root privileges inside the container.

---

## 4. Inter-Agent Communication Boundaries

Cortex provides in-process messaging and supervisor/worker task delegation through `AgentManager` and `AgentEndpoint`:

- **Trusted Host Authority**: Access to `AgentManager` (registration, endpoint issuance, worker assignment, and run completion) remains strictly in trusted host code. Model outputs cannot access the manager or issue endpoints.
- **Endpoint-Bound Sender Identity**: Senders cannot forge identity; each `AgentEndpoint` strictly stamps the authenticated `AgentId` of the sending agent upon enqueueing.
- **Model Output Separation**: Model-generated text cannot grant supervisor roles, assign workers, or expand permissions.
- **No Capability Transfer**: Delegating a task does not copy, elevate, or transfer permissions from supervisor to worker. Host execution must still use existing tool permission and workspace checks.
- **No Automatic Tool Execution**: Messages carry typed data. Receiving an `AgentMessage` or `TaskRequest` does not execute tools or perform external side effects.
- **Envelope Deserialization**: Deserializing a message envelope creates data, not authority or verified identity. Envelopes carry no cryptographic authentication.
- **In-Process Boundary Scope**: Inter-agent messaging is an in-process runtime coordination abstraction; it does not isolate malicious host code with direct memory access to `AgentManager`.

---

## 5. Security Testing Requirements

Every component that touches I/O, process execution, or file paths must include dedicated security test coverage:

- Path traversal attempts (`../../` outside workspace bounds)
- Shell injection and escaping vulnerabilities
- Network isolation enforcement
- Timeout and cancellation enforcement
- Capability permission bypass prevention
- Environment variable leak prevention
- Malformed tool arguments and schema fuzzing

---

## 6. Reporting a Vulnerability

If you discover a security vulnerability in Cortex:

1. **Do not open a public issue.**
2. Report the vulnerability privately to maintainers via GitHub Private Vulnerability Reporting or by contacting `security@cortex-ai.org` (or configured repository security contact).
3. Provide detailed steps to reproduce, impact assessment, and any proposed mitigation.
4. The maintainers will respond promptly to acknowledge the report and coordinate a patch before public disclosure.
