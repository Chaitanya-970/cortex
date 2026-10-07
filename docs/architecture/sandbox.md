# Sandbox Architecture

The sandbox is a security boundary between model-generated actions and the host system.

Capabilities may include:

```yaml
permissions:
  filesystem: workspace
  shell: true
  network: false
  git:
    read: true
    write: true
    push: false
```

Docker-backed sandboxing should support:

- workspace mounts
- network policy
- timeouts
- resource limits
- environment filtering
- non-root execution where practical

Sandboxing should be treated as defense in depth, not as a claim of perfect isolation.
