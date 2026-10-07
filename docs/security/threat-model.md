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
