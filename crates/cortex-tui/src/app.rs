//! Application state management and navigation model for the Cortex TUI.

use cortex_core::EventRecord;
use cortex_harness::suite::get_suite_tasks;
use cortex_harness::task::BenchmarkTask;
use cortex_runtime::storage::{RunStore, RunSummary};
use std::sync::Arc;

/// Available navigation tabs in the Cortex control plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveTab {
    /// Overview dashboard with health indicators and metrics.
    #[default]
    Dashboard,
    /// Agent list and active policy inspector.
    Agents,
    /// Detailed inspector for current or selected execution run.
    ActiveRun,
    /// Chronological event and execution trace viewer.
    Events,
    /// Historical execution runs stored in SQLite.
    History,
    /// Curated benchmark tasks and evaluations.
    Tasks,
}

impl ActiveTab {
    /// Return all tabs in standard display order.
    pub fn all() -> &'static [ActiveTab] {
        &[
            ActiveTab::Dashboard,
            ActiveTab::Agents,
            ActiveTab::ActiveRun,
            ActiveTab::Events,
            ActiveTab::History,
            ActiveTab::Tasks,
        ]
    }

    /// Tab label displayed in the top navigation bar.
    pub fn label(&self) -> &'static str {
        match self {
            ActiveTab::Dashboard => "1: Dashboard",
            ActiveTab::Agents => "2: Agents",
            ActiveTab::ActiveRun => "3: Active Run",
            ActiveTab::Events => "4: Events",
            ActiveTab::History => "5: History",
            ActiveTab::Tasks => "6: Tasks & Bench",
        }
    }
}

/// Information representing an agent configured in the workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentView {
    /// Identifier of the agent.
    pub id: String,
    /// Agent role or display name.
    pub name: String,
    /// Assigned operational policy or instruction summary.
    pub policy: String,
    /// Current execution state.
    pub status: String,
}

/// Primary application state container for the TUI client.
pub struct App {
    /// Currently focused navigation tab.
    pub active_tab: ActiveTab,
    /// Persistent SQLite storage client.
    pub store: Option<Arc<RunStore>>,
    /// Retrieved execution runs from storage.
    pub runs: Vec<RunSummary>,
    /// Index of currently selected run in tables.
    pub selected_run_idx: usize,
    /// Structured event stream for selected run.
    pub events: Vec<EventRecord>,
    /// Index of currently selected event in the log viewer.
    pub selected_event_idx: usize,
    /// Benchmark tasks available in the system.
    pub tasks: Vec<BenchmarkTask>,
    /// Index of currently selected benchmark task.
    pub selected_task_idx: usize,
    /// List of configured agents.
    pub agents: Vec<AgentView>,
    /// Index of currently selected agent.
    pub selected_agent_idx: usize,
    /// Flag indicating whether the TUI loop should exit.
    pub should_quit: bool,
    /// Transient status or notification message.
    pub status_message: Option<String>,
}

impl App {
    /// Create a new [`App`] initialized with an optional storage handle.
    pub fn new(store: Option<Arc<RunStore>>) -> Self {
        let mut app = Self {
            active_tab: ActiveTab::Dashboard,
            store,
            runs: Vec::new(),
            selected_run_idx: 0,
            events: Vec::new(),
            selected_event_idx: 0,
            tasks: Vec::new(),
            selected_task_idx: 0,
            agents: vec![
                AgentView {
                    id: "coder-01".to_string(),
                    name: "Coding Agent".to_string(),
                    policy: "Autonomous repository bug fixing and test verification".to_string(),
                    status: "Ready".to_string(),
                },
                AgentView {
                    id: "eval-01".to_string(),
                    name: "Benchmark Evaluator".to_string(),
                    policy: "Executes reproducible evaluation suites and collects metrics"
                        .to_string(),
                    status: "Idle".to_string(),
                },
            ],
            selected_agent_idx: 0,
            should_quit: false,
            status_message: Some("Cortex TUI Control Plane active. Press 'q' to quit.".to_string()),
        };

        app.load_tasks();
        app.refresh();
        app
    }

    /// Load benchmark tasks from the harness suite definitions.
    pub fn load_tasks(&mut self) {
        let mut tasks = get_suite_tasks("coding");
        tasks.extend(get_suite_tasks("refactor"));
        tasks.extend(get_suite_tasks("cli"));
        self.tasks = tasks;
    }

    /// Refresh state by querying the persistent [`RunStore`].
    pub fn refresh(&mut self) {
        if let Some(store) = &self.store {
            if let Ok(runs) = store.list_runs(50) {
                self.runs = runs;
                if self.selected_run_idx >= self.runs.len() && !self.runs.is_empty() {
                    self.selected_run_idx = self.runs.len() - 1;
                }
            }

            self.refresh_events();
        }
    }

    /// Refresh event stream for the currently selected run.
    pub fn refresh_events(&mut self) {
        if let Some(store) = &self.store {
            if let Some(run) = self.runs.get(self.selected_run_idx) {
                if let Ok(events) = store.get_events(&run.id) {
                    self.events = events;
                    if self.selected_event_idx >= self.events.len() && !self.events.is_empty() {
                        self.selected_event_idx = self.events.len() - 1;
                    }
                } else {
                    self.events.clear();
                }
            } else {
                self.events.clear();
            }
        }
    }

    /// Switch to next navigation tab.
    pub fn next_tab(&mut self) {
        let tabs = ActiveTab::all();
        let current_pos = tabs.iter().position(|t| *t == self.active_tab).unwrap_or(0);
        let next_pos = (current_pos + 1) % tabs.len();
        self.active_tab = tabs[next_pos];
    }

    /// Switch to previous navigation tab.
    pub fn prev_tab(&mut self) {
        let tabs = ActiveTab::all();
        let current_pos = tabs.iter().position(|t| *t == self.active_tab).unwrap_or(0);
        let prev_pos = if current_pos == 0 {
            tabs.len() - 1
        } else {
            current_pos - 1
        };
        self.active_tab = tabs[prev_pos];
    }

    /// Set navigation tab directly.
    pub fn set_tab(&mut self, tab: ActiveTab) {
        self.active_tab = tab;
    }

    /// Navigate to next item in current tab view.
    pub fn next_item(&mut self) {
        match self.active_tab {
            ActiveTab::History | ActiveTab::Dashboard => {
                if !self.runs.is_empty() && self.selected_run_idx + 1 < self.runs.len() {
                    self.selected_run_idx += 1;
                    self.refresh_events();
                }
            }
            ActiveTab::Events => {
                if !self.events.is_empty() && self.selected_event_idx + 1 < self.events.len() {
                    self.selected_event_idx += 1;
                }
            }
            ActiveTab::Tasks => {
                if !self.tasks.is_empty() && self.selected_task_idx + 1 < self.tasks.len() {
                    self.selected_task_idx += 1;
                }
            }
            ActiveTab::Agents => {
                if !self.agents.is_empty() && self.selected_agent_idx + 1 < self.agents.len() {
                    self.selected_agent_idx += 1;
                }
            }
            ActiveTab::ActiveRun => {}
        }
    }

    /// Navigate to previous item in current tab view.
    pub fn prev_item(&mut self) {
        match self.active_tab {
            ActiveTab::History | ActiveTab::Dashboard => {
                if self.selected_run_idx > 0 {
                    self.selected_run_idx -= 1;
                    self.refresh_events();
                }
            }
            ActiveTab::Events => {
                if self.selected_event_idx > 0 {
                    self.selected_event_idx -= 1;
                }
            }
            ActiveTab::Tasks => {
                if self.selected_task_idx > 0 {
                    self.selected_task_idx -= 1;
                }
            }
            ActiveTab::Agents => {
                if self.selected_agent_idx > 0 {
                    self.selected_agent_idx -= 1;
                }
            }
            ActiveTab::ActiveRun => {}
        }
    }

    /// Action triggered by pressing Enter.
    pub fn select_current(&mut self) {
        match self.active_tab {
            ActiveTab::Dashboard | ActiveTab::History if !self.runs.is_empty() => {
                self.active_tab = ActiveTab::ActiveRun;
            }
            _ => {}
        }
    }

    /// Currently selected run summary, if any.
    pub fn selected_run(&self) -> Option<&RunSummary> {
        self.runs.get(self.selected_run_idx)
    }

    /// Currently selected event record, if any.
    pub fn selected_event(&self) -> Option<&EventRecord> {
        self.events.get(self.selected_event_idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_initialization_and_tab_cycling() {
        let mut app = App::new(None);
        assert_eq!(app.active_tab, ActiveTab::Dashboard);

        app.next_tab();
        assert_eq!(app.active_tab, ActiveTab::Agents);

        app.next_tab();
        assert_eq!(app.active_tab, ActiveTab::ActiveRun);

        app.prev_tab();
        assert_eq!(app.active_tab, ActiveTab::Agents);

        app.set_tab(ActiveTab::Tasks);
        assert_eq!(app.active_tab, ActiveTab::Tasks);
        assert!(!app.tasks.is_empty());
    }

    #[test]
    fn test_app_navigation_bounds() {
        let mut app = App::new(None);
        assert_eq!(app.selected_run_idx, 0);

        // Underflow safe
        app.prev_item();
        assert_eq!(app.selected_run_idx, 0);

        // Overflow safe on empty list
        app.next_item();
        assert_eq!(app.selected_run_idx, 0);
    }
}
