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
