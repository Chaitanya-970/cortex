# Threat Model

## Assets

- source code
- credentials
- environment variables
- Git repositories
- filesystem data
- network access
- agent state
- execution history

## Adversaries

- malicious or compromised model output
- malicious tool arguments
- malicious MCP server
- malicious Agent Skill
- untrusted repository contents
- prompt injection
- compromised dependencies

## Core defenses

- capability-based permissions
- workspace boundaries
- sandboxing
- environment filtering
- tool validation
- timeouts
- resource limits
- audit/event tracing
- security CI

## Important principle

Prompt instructions are not a security boundary.

Security must be enforced by the runtime.

## Cancellation cleanup

Cancelling a built-in model request drops pending HTTP I/O. Cancelling shell
execution terminates its Unix process group or Windows job and reaps the direct
child. Output readers do not block the caller's cancellation indefinitely.
Already-cancelled tool dispatch is rejected before execution. Cancellation does
not grant capabilities or replace workspace and permission validation.

Process groups and job objects support cleanup; they are not containment against
malicious descendants that escape a group or spawn during Windows job attachment.
Container isolation remains a separate security boundary.

## Inter-Agent Boundaries & Multi-Agent Threat Model

- **Boundary Containment**: Delegating a task cannot grant capabilities the child worker lacks. Agent permissions are strictly bound to individual manifests (`AgentPermissions`); task delegation never copies, inherits, or expands capabilities across hierarchy boundaries.
- **Privilege Escalation Prevention**: An agent cannot forge its `sender` identity in the message envelope. `AgentManager` binds sender identities exclusively through issued `AgentEndpoint` handles in trusted host code; deserialized envelopes cannot claim unverified authority.
- **Prompt Injection Defense**: Inter-agent messages are treated as untrusted data inputs, never as runtime execution authority. Enqueued message payloads (`AgentMessagePayload`) do not trigger automatic tool dispatch or shell execution; all actions pass through runtime capability policies.
- **Hierarchy Integrity**: Supervisor-worker relationships form a directed acyclic graph. Direct and transitive cycles (A → B → A) are rejected at configuration time. Rogue agents cannot delegate tasks to workers assigned to other supervisors.
- **In-Process Scope**: Inter-agent messaging is an in-process runtime coordination abstraction; it does not isolate malicious host code with direct memory access to `AgentManager`.
