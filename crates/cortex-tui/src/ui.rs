//! Rendering implementation for all TUI screens and control widgets.

#![allow(dead_code)]

use crate::app::{ActiveTab, App, ChatRole, PortalField, PortalInputMode};
use crate::theme;
use cortex_core::ExecutionEvent;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table, Tabs, Wrap,
};
use ratatui::Frame;

/// Render the complete user interface for the current frame.
///
/// In Cortex's modern CLI architecture (inspired by Claude Code, Cursor CLI, and Gemini CLI),
/// the interface is a dedicated conversational harness with slash commands and zero top tabs.
pub fn render(frame: &mut Frame, app: &App) {
    render_chat(frame, app, frame.area());
}

#[allow(dead_code)]
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
        .constraints([Constraint::Percentage(48), Constraint::Percentage(52)])
        .split(area);

    let rows: Vec<Row> = app
        .agents
        .iter()
        .enumerate()
        .map(|(idx, agent)| {
            let is_selected = idx == app.selected_agent_idx;
            let style = if is_selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let (badge, badge_style) = match agent.status.as_str() {
                "Running" | "RUNNING" => (
                    "[RUNNING]",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                "Ready" | "READY" => (
                    "[READY]",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                "Paused" | "PAUSED" => (
                    "[PAUSED]",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                "Stopped" | "STOPPED" => ("[STOPPED]", Style::default().fg(Color::Red)),
                _ => ("[IDLE]", Style::default().fg(Color::DarkGray)),
            };

            Row::new(vec![
                Cell::from(agent.id.clone()),
                Cell::from(agent.name.clone()),
                Cell::from(agent.model.clone()).style(Style::default().fg(Color::Yellow)),
                Cell::from(Span::styled(badge, badge_style)),
                Cell::from(agent.runs_count.to_string()),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(14),
            Constraint::Min(16),
            Constraint::Length(14),
            Constraint::Length(11),
            Constraint::Length(6),
        ],
    )
    .header(
        Row::new(vec!["Agent ID", "Name", "Model", "Status", "Runs"]).style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(format!(" Persistent Agent Workers ({}) ", app.agents.len()))
            .title_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
    );

    frame.render_widget(table, chunks[0]);

    if let Some(agent) = app.agents.get(app.selected_agent_idx) {
        let (badge, badge_style) = match agent.status.as_str() {
            "Running" | "RUNNING" => (
                "[RUNNING]",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            "Ready" | "READY" => (
                "[READY]",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            "Paused" | "PAUSED" => (
                "[PAUSED]",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            "Stopped" | "STOPPED" => ("[STOPPED]", Style::default().fg(Color::Red)),
            _ => ("[IDLE]", Style::default().fg(Color::DarkGray)),
        };

        let details = vec![
            Line::from(vec![
                Span::styled("Agent Identifier: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    &agent.id,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("    "),
                Span::styled("Lifecycle: ", Style::default().fg(Color::DarkGray)),
                Span::styled(badge, badge_style),
            ]),
            Line::from(vec![
                Span::styled("Display Name:     ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    &agent.name,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Role Description: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&agent.role, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("Active Model:     ", Style::default().fg(Color::DarkGray)),
                Span::styled(&agent.model, Style::default().fg(Color::Yellow)),
                Span::raw("    "),
                Span::styled("Runs Recorded: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    agent.runs_count.to_string(),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::styled("Workspace Root:   ", Style::default().fg(Color::DarkGray)),
                Span::styled(&agent.workspace, Style::default().fg(Color::White)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Operational Policy / Instruction:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(format!("  \"{}\"", agent.policy)),
            Line::from(""),
            Line::from(Span::styled(
                "Authorized Tools & Capabilities:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  • read_file, write_file, list_dir (strictly workspace confined)"),
            Line::from("  • shell (timeout capped, sanitized environment)"),
            Line::from("  • git_status, git_diff, git_log, git_commit"),
            Line::from(""),
            Line::from(Span::styled(
                "Agent Lifecycle Actions:",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(vec![
                Span::styled(
                    "  [s] ",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Start / Resume Agent    "),
                Span::styled(
                    "[x] ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::raw("Stop Agent    "),
                Span::styled(
                    "[p] ",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("Pause Agent"),
            ]),
        ];

        let detail_panel = Paragraph::new(details)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Agent Manifest & Runtime Inspector ")
                    .title_style(
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
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

fn render_chat(frame: &mut Frame, app: &App, area: Rect) {
    let active_portal = app.active_portal();
    let input_line_count = if app.chat_input.is_empty() {
        1
    } else {
        app.chat_input.split('\n').count().max(1)
    };
    let input_height = ((input_line_count as u16) + 1).clamp(2, 6);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Top Header: ~/project  git:main* · gpt-4o-mini · ● Ready
            Constraint::Length(1), // Divider space
            Constraint::Min(4),    // Message & execution stream
            Constraint::Length(input_height), // Dynamic prompt >
            Constraint::Length(1), // Minimal status bar
        ])
        .split(area);

    // 1. Top banner: ~/project  git:main* · gpt-4o-mini · ● Ready
    let cwd = std::env::current_dir()
        .map(|p| {
            p.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| p.display().to_string())
        })
        .unwrap_or_else(|_| "cortex".to_string());

    let mut header_spans = vec![Span::styled(
        format!("~/{} ", cwd),
        Style::default()
            .fg(theme::COLOR_PRIMARY)
            .add_modifier(Modifier::BOLD),
    )];

    if let Some(git_info) = &app.git_branch_info {
        header_spans.push(Span::styled(
            format!("{} ", git_info),
            Style::default().fg(theme::COLOR_SECONDARY),
        ));
    }

    header_spans.push(Span::styled("· ", Style::default().fg(theme::COLOR_SUBTLE)));
    header_spans.push(Span::styled(
        &active_portal.model_name,
        Style::default().fg(theme::COLOR_FG),
    ));
    header_spans.push(Span::styled(
        " · ",
        Style::default().fg(theme::COLOR_SUBTLE),
    ));

    if app.chat_is_running {
        header_spans.push(Span::styled(
            format!(
                "{} {} (Ctrl+C to cancel)",
                theme::spinner_frame(app.anim_tick),
                app.current_verb
            ),
            Style::default()
                .fg(theme::thinking_shimmer_color(app.anim_tick))
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        header_spans.push(Span::styled(
            "● Ready",
            Style::default().fg(theme::COLOR_SUCCESS),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(header_spans)), chunks[0]);

    // 2. Chat messages & execution stream
    let mut text_lines: Vec<Line> = Vec::new();
    if app.chat_messages.is_empty() {
        let indexed = app.indexer.read();
        let index_desc = if indexed.is_ready {
            format!("{} files indexed", indexed.total_files)
        } else {
            "indexing workspace...".to_string()
        };

        text_lines.push(Line::from(""));
        text_lines.push(Line::from(vec![
            Span::styled(
                "  ◈ Cortex Code",
                Style::default()
                    .fg(theme::COLOR_PRIMARY)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" v{}", cortex_core::VERSION),
                Style::default().fg(theme::COLOR_MUTED),
            ),
            Span::styled(" · ", Style::default().fg(theme::COLOR_SUBTLE)),
            Span::styled(
                &active_portal.model_name,
                Style::default().fg(theme::COLOR_FG),
            ),
            Span::styled(" · ", Style::default().fg(theme::COLOR_SUBTLE)),
            Span::styled(index_desc, Style::default().fg(theme::COLOR_MUTED)),
        ]));
        text_lines.push(Line::from(""));
        text_lines.push(Line::from(vec![
            Span::styled("  Type your task and press Enter to start (e.g. \"fix tests\", \"implement feature\").", Style::default().fg(theme::COLOR_MUTED)),
        ]));
        text_lines.push(Line::from(vec![
            Span::styled("  Type / for slash commands (/help, /clear, /model, /status, /diff, /sessions, /exit).", Style::default().fg(theme::COLOR_MUTED)),
        ]));
        text_lines.push(Line::from(""));
    } else {
        for msg in &app.chat_messages {
            match msg.role {
                ChatRole::User => {
                    text_lines.push(Line::from(vec![
                        Span::styled(
                            theme::prompt_glyph(),
                            Style::default()
                                .fg(theme::COLOR_PRIMARY)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            &msg.content,
                            Style::default()
                                .fg(theme::COLOR_FG)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ]));
                    text_lines.push(Line::from(""));
                }
                ChatRole::Assistant => {
                    text_lines.extend(crate::markdown::render_markdown(&msg.content));
                    text_lines.push(Line::from(""));
                }
                ChatRole::Thinking => {
                    if msg.is_expanded {
                        text_lines.push(Line::from(Span::styled(
                            "  ┌─ Thinking (Ctrl+T to collapse) ───────────────────────",
                            Style::default().fg(theme::COLOR_MUTED),
                        )));
                        for l in msg.content.lines() {
                            text_lines.push(Line::from(vec![
                                Span::styled("  │ ", Style::default().fg(theme::COLOR_MUTED)),
                                Span::styled(l, Style::default().fg(theme::COLOR_MUTED)),
                            ]));
                        }
                        text_lines.push(Line::from(Span::styled(
                            "  └───────────────────────────────────────────────────────",
                            Style::default().fg(theme::COLOR_MUTED),
                        )));
                    } else {
                        let lines = msg.content.lines().count();
                        text_lines.push(Line::from(vec![
                            Span::styled("  ⠋ ", Style::default().fg(theme::COLOR_PRIMARY)),
                            Span::styled(
                                format!("Thinking ({} lines, press Ctrl+T to expand)", lines),
                                Style::default().fg(theme::COLOR_MUTED),
                            ),
                        ]));
                    }
                    text_lines.push(Line::from(""));
                }
                ChatRole::Tool => {
                    let trimmed = msg.content.trim();
                    if trimmed.starts_with('✓') {
                        text_lines.push(Line::from(vec![
                            Span::styled(
                                "  ✓ ",
                                Style::default()
                                    .fg(theme::COLOR_SUCCESS)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                trimmed.trim_start_matches('✓').trim_start(),
                                Style::default().fg(theme::COLOR_FG),
                            ),
                        ]));
                    } else if trimmed.starts_with('✗') {
                        text_lines.push(Line::from(vec![
                            Span::styled(
                                "  ✗ ",
                                Style::default()
                                    .fg(theme::COLOR_ERROR)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                trimmed.trim_start_matches('✗').trim_start(),
                                Style::default().fg(theme::COLOR_ERROR),
                            ),
                        ]));
                    } else {
                        text_lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(trimmed, Style::default().fg(theme::COLOR_SECONDARY)),
                        ]));
                    }
                    text_lines.push(Line::from(""));
                }
                ChatRole::System => {
                    for line in msg.content.lines() {
                        let trimmed = line.trim_start();
                        if trimmed.starts_with('✓') {
                            text_lines.push(Line::from(vec![
                                Span::styled(
                                    "  ✓ ",
                                    Style::default()
                                        .fg(theme::COLOR_SUCCESS)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(
                                    trimmed.trim_start_matches('✓').trim_start(),
                                    Style::default().fg(theme::COLOR_FG),
                                ),
                            ]));
                        } else if trimmed.starts_with('✗') {
                            text_lines.push(Line::from(vec![
                                Span::styled(
                                    "  ✗ ",
                                    Style::default()
                                        .fg(theme::COLOR_ERROR)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(
                                    trimmed.trim_start_matches('✗').trim_start(),
                                    Style::default().fg(theme::COLOR_ERROR),
                                ),
                            ]));
                        } else {
                            text_lines.push(Line::from(Span::styled(
                                format!("  {}", line),
                                Style::default().fg(theme::COLOR_MUTED),
                            )));
                        }
                    }
                    text_lines.push(Line::from(""));
                }
                ChatRole::Error => {
                    text_lines.push(Line::from(vec![
                        Span::styled(
                            "  ✗ ",
                            Style::default()
                                .fg(theme::COLOR_ERROR)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(&msg.content, Style::default().fg(theme::COLOR_ERROR)),
                    ]));
                    text_lines.push(Line::from(""));
                }
            }
        }
    }

    if app.chat_is_running {
        let spinner = theme::spinner_frame(app.anim_tick);
        let verb = &app.current_verb;
        text_lines.push(Line::from(vec![
            Span::styled(
                format!("  {} ", spinner),
                Style::default()
                    .fg(theme::COLOR_PRIMARY)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("{} (Ctrl+C to cancel)", verb),
                Style::default().fg(theme::COLOR_PRIMARY),
            ),
        ]));
        text_lines.push(Line::from(""));
    }

    // Dynamic Autoscroll calculation
    let total_lines = text_lines.len();
    let visible_height = chunks[2].height as usize;
    let scroll_y = if app.chat_auto_scroll {
        total_lines.saturating_sub(visible_height)
    } else {
        let max_scroll = total_lines.saturating_sub(visible_height);
        max_scroll.saturating_sub(app.chat_scroll)
    };

    let chat_panel = Paragraph::new(text_lines)
        .scroll((scroll_y as u16, 0))
        .wrap(Wrap { trim: false });
    frame.render_widget(chat_panel, chunks[2]);

    // 3. Prompt input line: > [input text with cursor]
    let cursor_pos = app.chat_cursor.min(app.chat_input.len());
    let raw_lines: Vec<&str> = if app.chat_input.is_empty() {
        vec![""]
    } else {
        app.chat_input.split('\n').collect()
    };

    let mut rendered_input_lines: Vec<Line> = Vec::new();
    let mut current_offset = 0;

    for (i, line) in raw_lines.iter().enumerate() {
        let line_len = line.len();
        let line_start = current_offset;
        let line_end = current_offset + line_len;

        let prefix = if i == 0 { theme::prompt_glyph() } else { "  " };
        let mut spans = vec![Span::styled(
            prefix,
            Style::default()
                .fg(theme::COLOR_PRIMARY)
                .add_modifier(Modifier::BOLD),
        )];

        if cursor_pos >= line_start && (cursor_pos <= line_end || i == raw_lines.len() - 1) {
            let col = (cursor_pos.saturating_sub(line_start)).min(line_len);
            let before = &line[..col];
            let after = &line[col..];
            spans.push(Span::styled(before, Style::default().fg(theme::COLOR_FG)));
            spans.push(Span::styled(
                "█",
                Style::default()
                    .fg(theme::COLOR_PRIMARY)
                    .add_modifier(Modifier::RAPID_BLINK),
            ));
            spans.push(Span::styled(after, Style::default().fg(theme::COLOR_FG)));
        } else {
            spans.push(Span::styled(*line, Style::default().fg(theme::COLOR_FG)));
        }

        rendered_input_lines.push(Line::from(spans));
        current_offset += line_len + 1;
    }

    let input_widget = Paragraph::new(rendered_input_lines);
    frame.render_widget(input_widget, chunks[3]);

    // 4. Floating Autocomplete Popup (rendered above chunks[3] if open)
    if app.autocomplete_state.is_open && !app.autocomplete_state.matches.is_empty() {
        let matches_count = app.autocomplete_state.matches.len().min(6);
        let popup_h = (matches_count as u16) + 2;
        let popup_w = 64.min(area.width.saturating_sub(4));
        let popup_y = chunks[3].y.saturating_sub(popup_h);
        let popup_rect = Rect::new(chunks[3].x + 2, popup_y, popup_w, popup_h);

        let mut popup_lines: Vec<Line> = Vec::new();
        for (idx, cmd) in app
            .autocomplete_state
            .matches
            .iter()
            .take(matches_count)
            .enumerate()
        {
            let is_sel = idx == app.autocomplete_state.selected_idx;
            let (prefix, style_name, style_desc) = if is_sel {
                (
                    "> ",
                    Style::default()
                        .fg(theme::COLOR_PRIMARY)
                        .add_modifier(Modifier::BOLD),
                    Style::default()
                        .fg(theme::COLOR_FG)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                (
                    "  ",
                    Style::default().fg(theme::COLOR_FG),
                    Style::default().fg(theme::COLOR_MUTED),
                )
            };

            popup_lines.push(Line::from(vec![
                Span::styled(prefix, style_name),
                Span::styled(format!("/{: <12} ", cmd.name), style_name),
                Span::styled(cmd.description, style_desc),
            ]));
        }

        let popup_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme::COLOR_PRIMARY))
            .title(" Slash Commands (Tab/Enter: Select · ↑/↓: Navigate · Esc: Close) ")
            .title_style(
                Style::default()
                    .fg(theme::COLOR_PRIMARY)
                    .add_modifier(Modifier::BOLD),
            );

        let popup_widget = Paragraph::new(popup_lines).block(popup_block);
        frame.render_widget(Clear, popup_rect);
        frame.render_widget(popup_widget, popup_rect);
    }

    // 5. Minimal Status Bar
    let status_line = Line::from(vec![
        Span::styled(
            &active_portal.model_name,
            Style::default().fg(theme::COLOR_MUTED),
        ),
        Span::styled(" · ", Style::default().fg(theme::COLOR_SUBTLE)),
        Span::styled(
            format!("{} tokens", app.session_tokens),
            Style::default().fg(theme::COLOR_MUTED),
        ),
        Span::styled(" · ", Style::default().fg(theme::COLOR_SUBTLE)),
        Span::styled(
            format!("${:.4}", app.session_cost),
            Style::default().fg(theme::COLOR_MUTED),
        ),
        Span::styled(" · ", Style::default().fg(theme::COLOR_SUBTLE)),
        Span::styled(
            "/help for commands",
            Style::default().fg(theme::COLOR_MUTED),
        ),
    ]);
    frame.render_widget(Paragraph::new(status_line), chunks[4]);
}

fn render_portals(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(55), // Table of portals
            Constraint::Percentage(45), // Inspector & Edit Form
        ])
        .split(area);

    // Left pane: Table of Portals
    let header_row = Row::new(vec![
        Cell::from("Active"),
        Cell::from("Name"),
        Cell::from("Model"),
        Cell::from("Provider"),
        Cell::from("Status"),
    ])
    .style(
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    )
    .bottom_margin(1);

    let rows: Vec<Row> = app
        .portals
        .iter()
        .enumerate()
        .map(|(idx, p)| {
            let active_marker = if p.is_active { "[*]" } else { "[ ]" };
            let active_style = if p.is_active {
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let status_style = match p.status_text() {
                "Ready (Local)" => Style::default().fg(Color::Green),
                "Configured (Custom Key)" | "Configured (Env Var)" => {
                    Style::default().fg(Color::Cyan)
                }
                _ => Style::default().fg(Color::Yellow),
            };

            let row_style = if idx == app.selected_portal_idx {
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            Row::new(vec![
                Cell::from(Span::styled(active_marker, active_style)),
                Cell::from(p.name.clone()),
                Cell::from(p.model_name.clone()),
                Cell::from(p.provider_kind.clone()),
                Cell::from(Span::styled(p.status_text(), status_style)),
            ])
            .style(row_style)
        })
        .collect();

    let widths = [
        Constraint::Length(8),
        Constraint::Min(20),
        Constraint::Min(20),
        Constraint::Length(12),
        Constraint::Length(18),
    ];

    let table = Table::new(rows, widths).header(header_row).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(" Configured Model Portals ")
            .title_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
    );
    frame.render_widget(table, chunks[0]);

    // Right pane: Inspector or Input mode
    match app.portal_input_mode {
        PortalInputMode::Normal => {
            render_portal_inspector(frame, app, chunks[1]);
        }
        PortalInputMode::EditingKey => {
            render_portal_edit_key(frame, app, chunks[1]);
        }
        PortalInputMode::EditingBaseUrl => {
            render_portal_edit_base_url(frame, app, chunks[1]);
        }
        PortalInputMode::Adding { field } => {
            render_portal_adding_form(frame, app, field, chunks[1]);
        }
    }
}

fn render_portal_inspector(frame: &mut Frame, app: &App, area: Rect) {
    if let Some(portal) = app.portals.get(app.selected_portal_idx) {
        let est_cost = cortex_runtime::estimate_cost(&portal.model_name, 1_000_000, 1_000_000);
        let api_key_display = if let Some(key) = &portal.api_key {
            if key.len() > 8 {
                format!("{}...{}", &key[..4], &key[key.len() - 4..])
            } else {
                "********".to_string()
            }
        } else if portal.provider_kind == "ollama" {
            "Not Required (Local endpoint)".to_string()
        } else if portal.is_configured() {
            "Configured via Environment Variable".to_string()
        } else {
            "Not Configured (Press 'e' to set)".to_string()
        };

        let base_url_display = portal
            .base_url
            .clone()
            .unwrap_or_else(|| "Provider Default Endpoint".to_string());

        let lines = vec![
            Line::from(vec![
                Span::styled("Portal ID: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&portal.id, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("Name: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    &portal.name,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Provider Type: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&portal.provider_kind, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("Model: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    &portal.model_name,
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("Base URL: ", Style::default().fg(Color::DarkGray)),
                Span::styled(base_url_display, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("API Key: ", Style::default().fg(Color::DarkGray)),
                Span::styled(api_key_display, Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::styled("Est. Cost: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("${:.2} / 1M prompt + 1M completion", est_cost),
                    Style::default().fg(Color::Green),
                ),
            ]),
            Line::from(vec![
                Span::styled("Active Portal: ", Style::default().fg(Color::DarkGray)),
                if portal.is_active {
                    Span::styled(
                        "YES (Current Dispatch Target)",
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled("No", Style::default().fg(Color::DarkGray))
                },
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "Quick Actions:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(vec![
                Span::styled("[Enter / Space] ", Style::default().fg(Color::Cyan)),
                Span::raw("Set as active portal"),
            ]),
            Line::from(vec![
                Span::styled("[e] ", Style::default().fg(Color::Cyan)),
                Span::raw("Set / Edit custom API key"),
            ]),
            Line::from(vec![
                Span::styled("[b] ", Style::default().fg(Color::Cyan)),
                Span::raw("Set / Edit base endpoint URL"),
            ]),
            Line::from(vec![
                Span::styled("[a] ", Style::default().fg(Color::Cyan)),
                Span::raw("Add a new custom model portal"),
            ]),
            Line::from(vec![
                Span::styled("[d] ", Style::default().fg(Color::Cyan)),
                Span::raw("Delete selected portal"),
            ]),
            Line::from(vec![
                Span::styled("[t] ", Style::default().fg(Color::Cyan)),
                Span::raw("Test configuration & credentials"),
            ]),
        ];

        let panel = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Portal Inspector "),
            )
            .wrap(Wrap { trim: true });
        frame.render_widget(panel, area);
    }
}

fn render_portal_edit_key(frame: &mut Frame, app: &App, area: Rect) {
    let portal_name = app
        .portals
        .get(app.selected_portal_idx)
        .map(|p| p.name.as_str())
        .unwrap_or("Portal");

    let lines = vec![
        Line::from(Span::styled(
            format!("Set API Key for: {}", portal_name),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Enter your API key below. Leave blank to clear and use environment variables.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Key: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&app.portal_input_buffer, Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Controls: Press Enter to save | Esc to cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let widget = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Edit API Key ")
            .title_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
    );
    frame.render_widget(widget, area);
}

fn render_portal_edit_base_url(frame: &mut Frame, app: &App, area: Rect) {
    let portal_name = app
        .portals
        .get(app.selected_portal_idx)
        .map(|p| p.name.as_str())
        .unwrap_or("Portal");

    let lines = vec![
        Line::from(Span::styled(
            format!("Set Base URL for: {}", portal_name),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Enter custom endpoint URL (e.g. http://localhost:11434/v1). Leave blank for default.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "Base URL: ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&app.portal_input_buffer, Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Controls: Press Enter to save | Esc to cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let widget = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Edit Base URL ")
            .title_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
    );
    frame.render_widget(widget, area);
}

fn render_portal_adding_form(frame: &mut Frame, app: &App, current_field: PortalField, area: Rect) {
    let (step_num, field_name, hint) = match current_field {
        PortalField::Name => (
            "1/4",
            "Portal Display Name",
            "e.g. 'My Ollama Llama 3', 'Production Anthropic'",
        ),
        PortalField::Model => (
            "2/4",
            "Model Identifier",
            "e.g. 'gpt-4o', 'claude-3-5-sonnet', 'ollama/llama3.1'",
        ),
        PortalField::BaseUrl => (
            "3/4",
            "Base Endpoint URL",
            "e.g. 'http://localhost:11434/v1' (or leave blank for default)",
        ),
        PortalField::ApiKey => (
            "4/4",
            "API Authentication Key",
            "e.g. 'sk-...' (or leave blank to inherit from env vars)",
        ),
    };

    let lines = vec![
        Line::from(Span::styled(
            format!("Step {} - Create New Model Portal", step_num),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Current Field: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                field_name,
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(hint, Style::default().fg(Color::DarkGray))),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "> ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(&app.portal_input_buffer, Style::default().fg(Color::White)),
            Span::styled("█", Style::default().fg(Color::Cyan)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Draft Summary:",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::UNDERLINED),
        )),
        Line::from(vec![
            Span::styled("  Name: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if app.new_portal_draft.name.is_empty() {
                    "(editing...)"
                } else {
                    &app.new_portal_draft.name
                },
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Model: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if app.new_portal_draft.model_name.is_empty() {
                    "(pending...)"
                } else {
                    &app.new_portal_draft.model_name
                },
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Base URL: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                if app.new_portal_draft.base_url.is_empty() {
                    "(default)"
                } else {
                    &app.new_portal_draft.base_url
                },
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Controls: Press Enter to proceed | Esc to cancel creation",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let widget = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Add New Portal ")
            .title_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
    );
    frame.render_widget(widget, area);
}

fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let status_text = app
        .status_message
        .clone()
        .unwrap_or_else(|| "Cortex Control Plane".to_string());

    let hints = match app.active_tab {
        ActiveTab::Chat => vec![
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(": Send | "),
            Span::styled("Ctrl+T", Style::default().fg(Color::Yellow)),
            Span::raw(": Thinking | "),
            Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
            Span::raw(": History | "),
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::raw(": Tabs | "),
            Span::styled("PageUp/Dn", Style::default().fg(Color::Yellow)),
            Span::raw(": Scroll | "),
            Span::styled("Ctrl+C", Style::default().fg(Color::Yellow)),
            Span::raw(": Quit"),
        ],
        ActiveTab::Agents => vec![
            Span::styled(
                "s",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Start | "),
            Span::styled(
                "x",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Stop | "),
            Span::styled(
                "p",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": Pause | "),
            Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
            Span::raw(": Select | "),
            Span::styled("Tab", Style::default().fg(Color::Yellow)),
            Span::raw(": Tabs | "),
            Span::styled("q", Style::default().fg(Color::Yellow)),
            Span::raw(": Quit"),
        ],
        ActiveTab::Portals => {
            if app.portal_input_mode != crate::app::PortalInputMode::Normal {
                vec![
                    Span::styled("Enter", Style::default().fg(Color::Yellow)),
                    Span::raw(": Confirm Field | "),
                    Span::styled("Esc", Style::default().fg(Color::Yellow)),
                    Span::raw(": Cancel Edit | "),
                    Span::styled("Ctrl+C", Style::default().fg(Color::Yellow)),
                    Span::raw(": Quit"),
                ]
            } else {
                vec![
                    Span::styled("Enter/Space", Style::default().fg(Color::Yellow)),
                    Span::raw(": Activate | "),
                    Span::styled("e", Style::default().fg(Color::Yellow)),
                    Span::raw(": API Key | "),
                    Span::styled("b", Style::default().fg(Color::Yellow)),
                    Span::raw(": Base URL | "),
                    Span::styled("a", Style::default().fg(Color::Yellow)),
                    Span::raw(": Add | "),
                    Span::styled("d", Style::default().fg(Color::Yellow)),
                    Span::raw(": Delete | "),
                    Span::styled("t", Style::default().fg(Color::Yellow)),
                    Span::raw(": Test | "),
                    Span::styled("Tab", Style::default().fg(Color::Yellow)),
                    Span::raw(": Tabs | "),
                    Span::styled("q", Style::default().fg(Color::Yellow)),
                    Span::raw(": Quit"),
                ]
            }
        }
        _ => vec![
            Span::styled("Tab / 1-8", Style::default().fg(Color::Yellow)),
            Span::raw(": Switch Tabs | "),
            Span::styled("↑/↓", Style::default().fg(Color::Yellow)),
            Span::raw(": Navigate | "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(": Inspect | "),
            Span::styled("r", Style::default().fg(Color::Yellow)),
            Span::raw(": Refresh | "),
            Span::styled("q", Style::default().fg(Color::Yellow)),
            Span::raw(": Quit"),
        ],
    };

    let mut line_spans = vec![
        Span::styled(status_text, Style::default().fg(Color::White)),
        Span::raw(" | "),
    ];
    line_spans.extend(hints);

    let footer = Paragraph::new(vec![Line::from(line_spans)]).block(
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
