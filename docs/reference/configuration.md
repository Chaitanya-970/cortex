# Configuration Reference

Example agent:

```yaml
name: coder
model:
  provider: gemma
workspace: ./project

tools:
  - filesystem
  - shell
  - git

memory:
  enabled: true

permissions:
  filesystem: workspace
  shell: true
  network: false
  git:
    read: true
    write: true
    push: false
```

Configuration should be validated before execution.
