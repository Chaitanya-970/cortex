# Agent Architecture

A Cortex agent is a persistent worker with:

- stable ID
- configuration
- lifecycle state
- workspace
- model
- tools
- permissions
- memory configuration

## Lifecycle

```text
created → stopped → started → running
                    ↓
                 paused
                    ↓
                 resumed
                    ↓
                  stopped
```

AgentManager owns lifecycle transitions.

Future multi-agent systems should build on this manager rather than creating independent agent implementations.
