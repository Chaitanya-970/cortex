# Multi-Agent Guide

Cortex supports deterministic, inspectable collaboration between agents.

Example:

```text
manager
   ↓
researcher
   ↓
coder
   ↓
reviewer
   ↓
coder
```

Messages should include:

- sender
- recipient
- timestamp
- message ID
- task/run ID
- payload

Avoid uncontrolled autonomous swarms. Orchestration should be inspectable and testable.
