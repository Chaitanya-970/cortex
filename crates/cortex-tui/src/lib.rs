//! # cortex-tui
//!
//! Terminal user interface and control plane for Cortex.

#![deny(missing_docs)]

pub mod app;
pub mod commands;
pub mod event;
pub mod indexer;
pub mod markdown;
pub mod session;
pub mod terminal;
pub mod theme;
pub mod ui;

use app::App;
use cortex_core::Result;
use crossterm::event::{Event, KeyEventKind};
use ratatui::backend::Backend;
use ratatui::Terminal;
use std::path::Path;
use std::sync::Arc;
use terminal::TerminalGuard;

/// Current semantic version of the Cortex TUI crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Launch the interactive terminal control plane connected to the specified SQLite database path.
pub fn run_tui(db_path: &Path) -> Result<()> {
    let store = match cortex_runtime::RunStore::open(db_path) {
        Ok(s) => Some(Arc::new(s)),
        Err(e) => {
            tracing::warn!("Failed to open RunStore at {:?}: {}", db_path, e);
            None
        }
    };

    let mut app = App::new(store);
    app.set_tab(app::ActiveTab::Chat);
    let (mut terminal, _guard) = TerminalGuard::init()?;
    run_app(&mut terminal, app)
}

/// Run the TUI event loop on an initialized [`Terminal`] instance.
pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    loop {
        app.poll_chat_updates();

        terminal
            .draw(|frame| ui::render(frame, &app))
            .map_err(|e| {
                cortex_core::CortexError::Internal(format!("terminal draw error: {}", e))
            })?;

        if app.should_quit {
            break;
        }

        if crossterm::event::poll(std::time::Duration::from_millis(100))
            .map_err(|e| cortex_core::CortexError::Internal(format!("event poll error: {}", e)))?
        {
            if let Event::Key(key) = crossterm::event::read().map_err(|e| {
                cortex_core::CortexError::Internal(format!("event read error: {}", e))
            })? {
                if key.kind == KeyEventKind::Press {
                    event::handle_key(&mut app, key);
                }
            }
        }
    }

    app.shutdown();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use app::{ActiveTab, ChatMessageItem, ChatRole};
    use cortex_core::{EventRecord, ExecutionEvent, RunId};
    use cortex_runtime::storage::RunSummary;
    use ratatui::backend::TestBackend;

    #[test]
    fn test_headless_rendering_all_tabs_empty() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("create test terminal");
        let mut app = App::new(None);

        for tab in ActiveTab::all() {
            app.set_tab(*tab);
            terminal
                .draw(|f| ui::render(f, &app))
                .expect("draw tab in test backend");
        }
    }

    #[test]
    fn test_headless_rendering_with_populated_data() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("create test terminal");
        let mut app = App::new(None);

        let run_id = RunId::generate();
        app.runs = vec![RunSummary {
            id: run_id.clone(),
            task: "Solve coding puzzle".to_string(),
            status: "completed".to_string(),
            started_at: "2026-10-07T12:00:00Z".to_string(),
            finished_at: Some("2026-10-07T12:01:00Z".to_string()),
            duration_ms: Some(60000),
            tokens_prompt: 1500,
            tokens_completion: 400,
            tokens_total: 1900,
            estimated_cost_usd: 0.0035,
            error: None,
        }];

        app.events = vec![
            EventRecord::new(
                1,
                ExecutionEvent::RunStarted {
                    run_id: run_id.clone(),
                    task: "Solve coding puzzle".to_string(),
                    workspace_root: None,
                },
            ),
            EventRecord::new(
                2,
                ExecutionEvent::ToolStarted {
                    run_id: run_id.clone(),
                    tool_name: "file_read".to_string(),
                    arguments: serde_json::json!({"path": "src/main.rs"}),
                },
            ),
            EventRecord::new(
                3,
                ExecutionEvent::RunCompleted {
                    run_id,
                    final_answer: "Solved".to_string(),
                    iterations: 2,
                    duration_ms: 60000,
                },
            ),
        ];

        app.chat_messages = vec![
            ChatMessageItem {
                role: ChatRole::User,
                content: "Run test suite".to_string(),
                timestamp: "12:00:00".to_string(),
                is_expanded: false,
            },
            ChatMessageItem {
                role: ChatRole::Thinking,
                content: "Analyzing workspace crates and locating tests".to_string(),
                timestamp: "12:00:02".to_string(),
                is_expanded: true,
            },
            ChatMessageItem {
                role: ChatRole::Tool,
                content: "Executed shell: cargo test".to_string(),
                timestamp: "12:00:05".to_string(),
                is_expanded: false,
            },
            ChatMessageItem {
                role: ChatRole::Assistant,
                content: "All tests pass successfully.".to_string(),
                timestamp: "12:00:10".to_string(),
                is_expanded: false,
            },
        ];

        for tab in ActiveTab::all() {
            app.set_tab(*tab);
            terminal
                .draw(|f| ui::render(f, &app))
                .expect("draw populated tab in test backend");
        }
    }

    #[test]
    fn test_run_app_terminates_on_should_quit() {
        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("create test terminal");
        let mut app = App::new(None);
        app.should_quit = true;

        let res = run_app(&mut terminal, app);
        assert!(res.is_ok());
    }
}
