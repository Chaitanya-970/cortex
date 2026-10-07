//! Rendering implementation for all TUI screens and control widgets.

use crate::app::{ActiveTab, App};
use cortex_core::ExecutionEvent;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, Tabs, Wrap};
use ratatui::Frame;

/// Render the complete user interface for the current frame.
pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header & Tabs
            Constraint::Min(12),   // Main content
            Constraint::Length(3), // Footer & Keybindings
        ])
        .split(frame.area());

    render_header(frame, app, chunks[0]);

    match app.active_tab {
        ActiveTab::Dashboard => render_dashboard(frame, app, chunks[1]),
        ActiveTab::Agents => render_agents(frame, app, chunks[1]),
        ActiveTab::ActiveRun => render_active_run(frame, app, chunks[1]),
        ActiveTab::Events => render_events(frame, app, chunks[1]),
        ActiveTab::History => render_history(frame, app, chunks[1]),
        ActiveTab::Tasks => render_tasks(frame, app, chunks[1]),
    }

    render_footer(frame, app, chunks[2]);
}

fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = ActiveTab::all()
        .iter()
        .map(|tab| {
            let style = if *tab == app.active_tab {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Line::from(Span::styled(tab.label(), style))
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Cortex Agent Runtime Control Plane ")
                .title_style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .select(
            ActiveTab::all()
                .iter()
                .position(|t| *t == app.active_tab)
                .unwrap_or(0),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::UNDERLINED),
        );

    frame.render_widget(tabs, area);
}

fn render_dashboard(frame: &mut Frame, app: &App, area: Rect) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4), // Metrics tiles
            Constraint::Min(8),    // Recent activity & health
        ])
        .split(area);

    // Top metrics tiles
    let tile_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(main_chunks[0]);

    let total_runs = app.runs.len();
    let completed_runs = app.runs.iter().filter(|r| r.status == "completed").count();
    let failed_runs = app
        .runs
        .iter()
        .filter(|r| r.status == "failed" || r.status == "cancelled")
        .count();
    let active_agents = app.agents.len();

    render_tile(
        frame,
        tile_chunks[0],
        "Total Runs",
        &total_runs.to_string(),
        Color::Blue,
    );
    render_tile(
        frame,
        tile_chunks[1],
        "Completed",
        &completed_runs.to_string(),
        Color::Green,
    );
    render_tile(
        frame,
        tile_chunks[2],
        "Failed / Aborted",
        &failed_runs.to_string(),
        if failed_runs > 0 {
            Color::Red
        } else {
            Color::DarkGray
        },
    );
    render_tile(
        frame,
        tile_chunks[3],
        "Active Workers",
        &active_agents.to_string(),
        Color::Magenta,
    );

    // Lower half: Recent runs table + System Status
    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(main_chunks[1]);

    // Recent runs table
    let rows: Vec<Row> = app
        .runs
        .iter()
        .take(10)
        .enumerate()
        .map(|(idx, run)| {
            let is_selected = idx == app.selected_run_idx;
            let style = if is_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            let status_style = status_to_style(&run.status);
            Row::new(vec![
                Cell::from(run.id.as_str()),
                Cell::from(run.status.clone()).style(status_style),
                Cell::from(
                    run.duration_ms
                        .map(|d| format!("{}ms", d))
                        .unwrap_or_else(|| "-".to_string()),
                ),
                Cell::from(run.task.clone()),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(24),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Min(20),
        ],
    )
    .header(
        Row::new(vec!["Run ID", "Status", "Duration", "Task Prompt"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Recent Executions (Press Enter to inspect) "),
    );

    frame.render_widget(table, bottom_chunks[0]);

    // System status panel
    let status_text = vec![
        Line::from(vec![
            Span::styled("Runtime Status:  ", Style::default().fg(Color::White)),
            Span::styled("ONLINE (Persistent)", Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled("Storage Backend: ", Style::default().fg(Color::White)),
            Span::styled(
                if app.store.is_some() {
                    "SQLite Connected"
                } else {
                    "In-Memory / None"
                },
                Style::default().fg(Color::Cyan),
            ),
        ]),
        Line::from(vec![
            Span::styled("Sandbox Guard:   ", Style::default().fg(Color::White)),
            Span::styled(
                "Active (Workspace Bounded)",
                Style::default().fg(Color::Green),
            ),
        ]),
        Line::from(vec![
            Span::styled("Secret Redactor: ", Style::default().fg(Color::White)),
            Span::styled(
                "Enabled (sk-, ghp-, bearer)",
                Style::default().fg(Color::Yellow),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Quick Navigation:",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  [1] Dashboard    [2] Agents"),
        Line::from("  [3] Active Run   [4] Events"),
        Line::from("  [5] Run History  [6] Benchmark"),
    ];

    let status_panel = Paragraph::new(status_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Runtime Health & Status "),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(status_panel, bottom_chunks[1]);
}

fn render_tile(frame: &mut Frame, area: Rect, title: &str, value: &str, color: Color) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .title(Span::styled(title, Style::default().fg(Color::White)));

    let text = vec![
        Line::from(""),
        Line::from(Span::styled(
            value,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )),
    ];

    let p = Paragraph::new(text)
        .block(block)
        .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(p, area);
}

fn render_agents(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    let rows: Vec<Row> = app
        .agents
        .iter()
        .enumerate()
        .map(|(idx, agent)| {
            let is_selected = idx == app.selected_agent_idx;
            let style = if is_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(agent.id.clone()),
                Cell::from(agent.name.clone()),
                Cell::from(agent.status.clone()).style(Style::default().fg(Color::Green)),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Length(22),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["Agent ID", "Name", "Status"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Registered Agents "),
    );

    frame.render_widget(table, chunks[0]);

    if let Some(agent) = app.agents.get(app.selected_agent_idx) {
        let details = vec![
            Line::from(vec![
                Span::styled(
                    "Agent Identifier: ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&agent.id, Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled(
                    "Name / Role:      ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&agent.name, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled(
                    "Status:           ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&agent.status, Style::default().fg(Color::Green)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Assigned Operational Policy:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(agent.policy.clone()),
            Line::from(""),
            Line::from(Span::styled(
                "Authorized Tool Permissions:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  • read_file, write_file, list_dir (workspace boundary confined)"),
            Line::from("  • shell (timeout capped, sanitized environment)"),
            Line::from("  • git_status, git_diff, git_log, git_commit (push denied)"),
        ];

        let detail_panel = Paragraph::new(details)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Agent Manifest & Capabilities "),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(detail_panel, chunks[1]);
    }
}

fn render_active_run(frame: &mut Frame, app: &App, area: Rect) {
    if let Some(run) = app.selected_run() {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8), // Metrics summary
                Constraint::Min(6),    // Event stream
            ])
            .split(area);

        let status_style = status_to_style(&run.status);
        let summary_text = vec![
            Line::from(vec![
                Span::styled(
                    "Run ID:         ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(run.id.as_str(), Style::default().fg(Color::Cyan)),
                Span::raw("    "),
                Span::styled(
                    "Status: ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&run.status, status_style),
            ]),
            Line::from(vec![
                Span::styled("Started:        ", Style::default().fg(Color::White)),
                Span::raw(&run.started_at),
                Span::raw("    "),
                Span::styled("Duration: ", Style::default().fg(Color::White)),
                Span::raw(
                    run.duration_ms
                        .map(|d| format!("{} ms", d))
                        .unwrap_or_else(|| "-".to_string()),
                ),
            ]),
            Line::from(vec![
                Span::styled("Tokens:         ", Style::default().fg(Color::White)),
                Span::raw(format!(
                    "{} prompt / {} completion ({} total)",
                    run.tokens_prompt, run.tokens_completion, run.tokens_total
                )),
                Span::raw("    "),
                Span::styled("Estimated Cost: ", Style::default().fg(Color::White)),
                Span::raw(format!("${:.6}", run.estimated_cost_usd)),
            ]),
            Line::from(vec![
                Span::styled(
                    "Task Prompt:    ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&run.task),
            ]),
            Line::from(if let Some(err) = &run.error {
                vec![
                    Span::styled(
                        "Error:          ",
                        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(err, Style::default().fg(Color::Red)),
                ]
            } else {
                vec![]
            }),
        ];

        let summary_panel = Paragraph::new(summary_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Run Execution Overview "),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(summary_panel, chunks[0]);

        // Event stream for this run
        let event_rows: Vec<Row> = app
            .events
            .iter()
            .map(|record| {
                let event_type = record.event.event_type();
                let event_style = event_type_to_style(event_type);
                Row::new(vec![
                    Cell::from(record.sequence.to_string()),
                    Cell::from(record.timestamp.clone()),
                    Cell::from(event_type).style(event_style),
                    Cell::from(event_preview(&record.event)),
                ])
            })
            .collect();

        let event_table = Table::new(
            event_rows,
            [
                Constraint::Length(6),
                Constraint::Length(24),
                Constraint::Length(16),
                Constraint::Min(30),
            ],
        )
        .header(
            Row::new(vec!["Seq", "Timestamp", "Event Type", "Preview"]).style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(format!(
                    " Chronological Event Stream ({} events) ",
                    app.events.len()
                )),
        );

        frame.render_widget(event_table, chunks[1]);
    } else {
        let empty = Paragraph::new("No run selected. Navigate to 'Run History' and select a run.")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Active Run Inspector "),
            )
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(empty, area);
    }
}

fn render_events(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let rows: Vec<Row> = app
        .events
        .iter()
        .enumerate()
        .map(|(idx, record)| {
            let is_selected = idx == app.selected_event_idx;
            let style = if is_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            let ev_style = event_type_to_style(record.event.event_type());
            Row::new(vec![
                Cell::from(record.sequence.to_string()),
                Cell::from(record.timestamp[11..19].to_string()), // HH:MM:SS
                Cell::from(record.event.event_type()).style(ev_style),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(10),
            Constraint::Min(16),
        ],
    )
    .header(
        Row::new(vec!["Seq", "Time", "Event Variant"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(format!(" Event Log ({} records) ", app.events.len())),
    );

    frame.render_widget(table, chunks[0]);

    if let Some(record) = app.selected_event() {
        let json_pretty = serde_json::to_string_pretty(&record.event).unwrap_or_default();
        let detail_text = vec![
            Line::from(vec![
                Span::styled(
                    "Sequence:  ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    record.sequence.to_string(),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
            Line::from(vec![
                Span::styled(
                    "Timestamp: ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&record.timestamp),
            ]),
            Line::from(vec![
                Span::styled(
                    "Run ID:    ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(record.run_id.as_str()),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Structured Payload (Redacted):",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
        ];

        let mut lines = detail_text;
        for l in json_pretty.lines() {
            lines.push(Line::from(l));
        }

        let payload_widget = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Event Payload Inspector "),
            )
            .wrap(Wrap { trim: false });

        frame.render_widget(payload_widget, chunks[1]);
    } else {
        let empty = Paragraph::new("Select an event from the list to view its full JSON payload.")
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Event Payload Inspector "),
            );
        frame.render_widget(empty, chunks[1]);
    }
}

fn render_history(frame: &mut Frame, app: &App, area: Rect) {
    let rows: Vec<Row> = app
        .runs
        .iter()
        .enumerate()
        .map(|(idx, run)| {
            let is_selected = idx == app.selected_run_idx;
            let style = if is_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            let status_style = status_to_style(&run.status);
            Row::new(vec![
                Cell::from(run.id.as_str()),
                Cell::from(run.status.clone()).style(status_style),
                Cell::from(run.started_at.clone()),
                Cell::from(
                    run.duration_ms
                        .map(|d| format!("{} ms", d))
                        .unwrap_or_else(|| "-".to_string()),
                ),
                Cell::from(run.tokens_total.to_string()),
                Cell::from(run.task.clone()),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(26),
            Constraint::Length(12),
            Constraint::Length(24),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Min(25),
        ],
    )
    .header(
        Row::new(vec![
            "Run ID",
            "Status",
            "Started At",
            "Duration",
            "Tokens",
            "Task Prompt",
        ])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(format!(
                " Historical Runs ({} total - Press Enter to inspect) ",
                app.runs.len()
            )),
    );

    frame.render_widget(table, area);
}

fn render_tasks(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(area);

    let rows: Vec<Row> = app
        .tasks
        .iter()
        .enumerate()
        .map(|(idx, task)| {
            let is_selected = idx == app.selected_task_idx;
            let style = if is_selected {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(task.id.clone()),
                Cell::from(task.suite.clone()).style(Style::default().fg(Color::Cyan)),
                Cell::from(task.name.clone()),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(26),
            Constraint::Length(10),
            Constraint::Min(20),
        ],
    )
    .header(
        Row::new(vec!["Task ID", "Suite", "Name"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(format!(" Curated Benchmark Tasks ({}) ", app.tasks.len())),
    );

    frame.render_widget(table, chunks[0]);

    if let Some(task) = app.tasks.get(app.selected_task_idx) {
        let details = vec![
            Line::from(vec![
                Span::styled(
                    "Task ID:     ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&task.id, Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::styled(
                    "Name:        ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(&task.name),
            ]),
            Line::from(vec![
                Span::styled(
                    "Suite:       ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(&task.suite, Style::default().fg(Color::Yellow)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Description:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(task.description.clone()),
            Line::from(""),
            Line::from(Span::styled(
                "Task Prompt:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(task.prompt.clone()),
            Line::from(""),
            Line::from(Span::styled(
                "Verification Command:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                &task.verification_command,
                Style::default().fg(Color::Green),
            )),
        ];

        let task_panel = Paragraph::new(details)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Benchmark Task Details "),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(task_panel, chunks[1]);
    }
}

fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let status_text = app
        .status_message
        .clone()
        .unwrap_or_else(|| "Cortex Control Plane".to_string());

    let footer_text = vec![Line::from(vec![
        Span::styled(status_text, Style::default().fg(Color::White)),
        Span::raw(" | "),
        Span::styled("Tab / 1-6", Style::default().fg(Color::Yellow)),
        Span::raw(": Switch Tabs | "),
        Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
        Span::raw(": Navigate | "),
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::raw(": Inspect | "),
        Span::styled("r", Style::default().fg(Color::Yellow)),
        Span::raw(": Refresh | "),
        Span::styled("q", Style::default().fg(Color::Yellow)),
        Span::raw(": Quit"),
    ])];

    let footer = Paragraph::new(footer_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded),
    );

    frame.render_widget(footer, area);
}

fn status_to_style(status: &str) -> Style {
    match status {
        "completed" => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        "running" => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        "failed" => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        "cancelled" => Style::default().fg(Color::Yellow),
        _ => Style::default().fg(Color::White),
    }
}

fn event_type_to_style(event_type: &str) -> Style {
    match event_type {
        "RunStarted" | "RunCompleted" => Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
        "ModelRequest" | "ModelResponse" => Style::default().fg(Color::Blue),
        "ToolStarted" | "ToolCompleted" => Style::default().fg(Color::Yellow),
        "ToolFailed" | "AgentError" => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        "RunCancelled" => Style::default().fg(Color::Magenta),
        _ => Style::default().fg(Color::White),
    }
}

fn event_preview(event: &ExecutionEvent) -> String {
    match event {
        ExecutionEvent::RunStarted { task, .. } => format!("Task: {}", task),
        ExecutionEvent::ModelRequest { prompt_preview, .. } => {
            format!("Prompt: {}", prompt_preview)
        }
        ExecutionEvent::ModelResponse { output_summary, .. } => {
            format!("Output: {}", output_summary)
        }
        ExecutionEvent::ToolStarted { tool_name, .. } => format!("Invoke: {}", tool_name),
        ExecutionEvent::ToolCompleted {
            tool_name, output, ..
        } => {
            let out_preview = if output.len() > 40 {
                format!("{}...", &output[..37])
            } else {
                output.clone()
            };
            format!("{} -> {}", tool_name, out_preview)
        }
        ExecutionEvent::ToolFailed {
            tool_name, error, ..
        } => format!("{} FAIL: {}", tool_name, error),
        ExecutionEvent::AgentMessage { role, content, .. } => format!("{}: {}", role, content),
        ExecutionEvent::AgentRetry {
            attempt, reason, ..
        } => format!("Retry #{}: {}", attempt, reason),
        ExecutionEvent::AgentError { error, .. } => format!("Error: {}", error),
        ExecutionEvent::RunCompleted { final_answer, .. } => format!("Completed: {}", final_answer),
        ExecutionEvent::RunCancelled { reason, .. } => format!("Cancelled: {}", reason),
    }
}
