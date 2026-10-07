# Terminal UI (TUI) Control Plane

Cortex provides a terminal-native interactive control plane (`cortex-tui`) built with [Ratatui](https://ratatui.rs) and [Crossterm](https://github.com/crossterm-rs/crossterm).

The TUI acts as a completely decoupled observability and inspection interface over Cortex's persistent execution state, connecting directly to SQLite via `RunStore` without duplicating runtime logic or interfering with agent processes.

```
┌────────────────────────────────────────────────────────────────────────┐
│ Cortex Agent Runtime Control Plane                                     │
├────────────────────────────────────────────────────────────────────────┤
│ [1: Dashboard] [2: Agents] [3: Active Run] [4: Events] [5: History]   │
├────────────────────────────────────────────────────────────────────────┤
│                                                                        │
│   ┌───────────────┐ ┌───────────────┐ ┌───────────────┐                │
│   │  Total Runs   │ │   Completed   │ │    Failed     │                │
│   │      12       │ │      11       │ │       1       │                │
│   └───────────────┘ └───────────────┘ └───────────────┘                │
│                                                                        │
│   Execution Log & Event Trace Inspector                                │
│   [#1] 12:00:01  RunStarted        Solve coding task                   │
│   [#2] 12:00:05  ToolStarted       file_read {"path":"src/lib.rs"}     │
│   [#3] 12:00:10  RunCompleted      Solved                              │
│                                                                        │
├────────────────────────────────────────────────────────────────────────┤
│ Tab/1-6: Tabs | ↑/↓: Navigate | Enter: Inspect | r: Refresh | q: Quit   │
└────────────────────────────────────────────────────────────────────────┘
```

## Launching the Control Plane

Launch the TUI from any terminal:

```bash
cortex tui
```

### Custom Database Path

To connect to a specific database (e.g. during testing or multi-environment management):

```bash
cortex tui --db /path/to/cortex.db
```

Alternatively, set the `CORTEX_DB_PATH` environment variable:

```bash
CORTEX_DB_PATH=/path/to/cortex.db cortex tui
```

---

## Views and Navigation

The control plane contains 6 primary views accessible via numeric keys (`1` through `6`) or `Tab` / `Shift+Tab`:

### 1. Dashboard (`1`)
- **Metric Tiles**: Total runs, completed runs, failure/abort counts, and active worker count.
- **Recent Executions**: Quick view of latest agent tasks with duration and status badges.
- **System Status**: SQLite connection status and runtime engine health.

### 2. Agents (`2`)
- **Agent Roster**: Configured agent personas, roles, and status.
- **Policy Inspector**: Operational policy definitions, safety constraints, and assigned capabilities for the selected agent.

### 3. Active Run (`3`)
- **Run Overview**: Detailed execution metadata for the selected run (Run ID, status, start/finish timestamps, duration).
- **Resource Usage**: Token consumption breakdown (prompt, completion, total) and estimated inference cost (USD).
- **Execution Outcome**: Error logs, stack traces, and final answers.

### 4. Events (`4`)
- **Event Trace**: Chronological sequence table of all events emitted during the run (`RunStarted`, `ToolStarted`, `ToolCompleted`, `ToolFailed`, `ModelRequest`, `ModelResponse`, `RunCompleted`).
- **Structured Payload Inspector**: Formatted JSON inspector showing exact arguments, outputs, and parameters passed to tools and models.

### 5. History (`5`)
- **Runs Table**: Complete historical log of execution runs recorded in the persistent SQLite database.
- Navigate rows with `↑`/`↓` and press `Enter` to switch to the Active Run and Event views for that execution.

### 6. Tasks & Bench (`6`)
- **Curated Tasks**: Interactive browser of benchmark evaluation tasks across suites (`coding`, `refactor`, `cli`).
- Inspect task prompts, test files, and verification commands.

---

## Keyboard Controls

| Key | Action |
|---|---|
| `Tab` | Cycle to next tab |
| `Shift+Tab` / `BackTab` | Cycle to previous tab |
| `1` ..= `6` | Jump directly to tab 1 through 6 |
| `↑` / `k` | Move selection up in list/table |
| `↓` / `j` | Move selection down in list/table |
| `Enter` | Inspect selected run / drill down |
| `r` | Refresh state from SQLite database |
| `Esc` | Return to Dashboard |
| `q` / `Ctrl+C` | Cleanly exit TUI |

---

## Terminal Safety and Panic Recovery

The TUI is engineered to prevent terminal corruption:

1. **RAII Terminal Guard**: `TerminalGuard` automatically restores the terminal out of raw mode, leaves the alternate screen buffer, and restores the cursor on drop.
2. **Panic Hook Restoration**: If an unhandled panic occurs anywhere in the process, a custom panic hook restores standard terminal mode before printing the panic unwinding message, preventing terminal freezing or invisible typing.
3. **Headless Test Backend**: Automated testing utilizes `ratatui::backend::TestBackend` to verify all views and keyboard transitions without spawning terminal sessions or requiring interactive input.
