# CLI Reference

Available commands:

```bash
cortex status
cortex check
cortex run "<prompt>"

cortex runs
cortex runs list [--limit <N>]
cortex runs show <id> [--verbose]
cortex runs replay <id>

cortex bench run [--suite <suite>] [--json] [--report <path>] [--max-iterations <N>]

cortex tui [--db <path>]
```

## `cortex tui`

Launch the terminal user interface (TUI) control plane for interactive inspection of agent executions, metrics, and benchmark tasks.

```bash
# Connect using default database location (~/.cortex/cortex.db)
cortex tui

# Connect to custom SQLite database
cortex tui --db /path/to/cortex.db
```

See [TUI Control Plane Guide](../guides/tui.md) for full keyboard shortcuts and view documentation.

