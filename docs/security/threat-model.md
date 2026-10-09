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

## Inter-Agent Boundaries

- `AgentManager` and endpoint issuance are restricted to trusted host code.
- Sender identity is bound to the issued `AgentEndpoint`; agents cannot forge senders.
- Model text does not grant supervisor roles, assign workers, or expand permissions.
- Delegating tasks cannot copy, elevate, or transfer capabilities to workers.
- Receiving a message returns typed data; receipt does not execute tools.
- Deserialized message envelopes carry no authentication or capability authority.
- In-process coordination does not defend against malicious host code with manager access.
