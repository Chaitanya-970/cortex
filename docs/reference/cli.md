# CLI Reference

Planned commands include:

```bash
cortex run "Fix the failing tests"

cortex agent list
cortex agent create
cortex agent start
cortex agent stop
cortex agent inspect

cortex runs
cortex runs list [--limit <N>]
cortex runs show <id> [--verbose]
cortex runs replay <id>

cortex bench run [--suite <suite>] [--json] [--report <path>]
```

Commands should expose runtime state rather than duplicating business logic.
