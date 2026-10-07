# Cron Guide

Example:

```bash
cortex cron create --schedule "*/30 * * * *" --agent monitor --task "Check GitHub issues"
```

Jobs should be persistent and recoverable.

Supported concepts:

- cron
- one-shot jobs
- recurring jobs
- history
- cancellation
- restart recovery
- overlap policies
- timezone handling
