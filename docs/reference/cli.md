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
cortex runs list
cortex runs show <id>

cortex cron list
cortex cron create
cortex cron delete
cortex cron inspect
cortex cron history

cortex eval ./evals
```

Commands should expose runtime state rather than duplicating business logic.
