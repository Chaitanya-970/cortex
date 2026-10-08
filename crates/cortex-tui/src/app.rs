//! Application state management and navigation model for the Cortex TUI.

use cortex_core::{EventRecord, ExecutionEvent, RunId};
use cortex_harness::suite::get_suite_tasks;
use cortex_harness::task::BenchmarkTask;
use cortex_runtime::agent::{AgentContext, AgentLoop, AgentRunResult, CancellationToken};
use cortex_runtime::providers::create_model_provider;
use cortex_runtime::storage::{RunStore, RunSummary};
use cortex_runtime::tool::ToolRegistry;
use cortex_runtime::tools;
use cortex_runtime::workspace::Workspace;
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{channel, Receiver, Sender};
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
    /// Interactive chat interface for autonomous agent execution.
    Chat,
    /// Model provider and custom inference portal configuration.
    Portals,
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
            ActiveTab::Chat,
            ActiveTab::Portals,
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
            ActiveTab::Chat => "7: Agent Chat",
            ActiveTab::Portals => "8: Portals",
        }
    }
}

/// Lifecycle execution states for persistent agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AgentLifecycleState {
    /// Agent initialized and waiting for tasks.
    #[default]
    Ready,
    /// Agent currently executing an assigned task.
    Running,
    /// Agent paused by user or coordinator.
    Paused,
    /// Agent stopped and inactive.
    Stopped,
    /// Agent encountered an unrecoverable failure.
    Failed,
}

impl AgentLifecycleState {
    /// Formatted string representation of the state.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Running => "Running",
            Self::Paused => "Paused",
            Self::Stopped => "Stopped",
            Self::Failed => "Failed",
        }
    }
}

/// Information representing an agent configured in the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentView {
    /// Identifier of the agent.
    pub id: String,
    /// Agent display name.
    pub name: String,
    /// Agent role description.
    pub role: String,
    /// Assigned operational policy or instruction summary.
    pub policy: String,
    /// Current execution state badge.
    pub status: String,
    /// Model identifier configured for this agent.
    pub model: String,
    /// Working directory boundary path.
    pub workspace: String,
    /// Authorized tool names.
    pub tools: Vec<String>,
    /// Number of runs executed by this agent.
    pub runs_count: usize,
}

/// Configuration for a model inference provider portal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelPortal {
    /// Unique identifier for this portal.
    pub id: String,
    /// Display name shown in UI tables and selectors.
    pub name: String,
    /// Underlying provider family (e.g., "openai", "anthropic", "ollama", "custom").
    pub provider_kind: String,
    /// Model identifier sent to the provider.
    pub model_name: String,
    /// Custom API base URL if different from provider defaults.
    pub base_url: Option<String>,
    /// Optional explicit API key override.
    pub api_key: Option<String>,
    /// Whether this portal is currently selected for agent execution runs.
    pub is_active: bool,
}

impl ModelPortal {
    /// Create a new portal configuration.
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        provider_kind: impl Into<String>,
        model_name: impl Into<String>,
        base_url: Option<String>,
        api_key: Option<String>,
        is_active: bool,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            provider_kind: provider_kind.into(),
            model_name: model_name.into(),
            base_url,
            api_key,
            is_active,
        }
    }

    /// Check if the portal has credentials or local endpoint availability configured.
    pub fn is_configured(&self) -> bool {
        let lower = self.model_name.to_lowercase();
        if lower.starts_with("ollama/") || self.provider_kind == "ollama" {
            return true;
        }
        if lower.starts_with("claude") || self.provider_kind == "anthropic" {
            return self.api_key.is_some()
                || std::env::var("ANTHROPIC_API_KEY").is_ok()
                || std::env::var("anthropic_api_key").is_ok();
        }
        self.api_key.is_some()
            || std::env::var("OPENAI_API_KEY").is_ok()
            || std::env::var("openai_api_key").is_ok()
    }

    /// Status badge text describing the configuration state.
    pub fn status_text(&self) -> &'static str {
        let lower = self.model_name.to_lowercase();
        if lower.starts_with("ollama/") || self.provider_kind == "ollama" {
            "Ready (Local)"
        } else if self.api_key.is_some() {
            "Configured (Custom Key)"
        } else if (lower.starts_with("claude")
            && (std::env::var("ANTHROPIC_API_KEY").is_ok()
                || std::env::var("anthropic_api_key").is_ok()))
            || std::env::var("OPENAI_API_KEY").is_ok()
            || std::env::var("openai_api_key").is_ok()
        {
            "Configured (Env Var)"
        } else {
            "Needs API Key"
        }
    }
}

/// Interactive mode for portal configuration inputs in the TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PortalInputMode {
    /// Normal navigation mode.
    #[default]
    Normal,
    /// Multi-step form for creating a new custom portal.
    Adding {
        /// Current field being edited.
        field: PortalField,
    },
    /// Editing the API key of the selected portal.
    EditingKey,
    /// Editing the Base URL of the selected portal.
    EditingBaseUrl,
}

/// Form fields for creating a new portal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalField {
    /// Display name of the portal.
    Name,
    /// Model identifier.
    Model,
    /// Base endpoint URL.
    BaseUrl,
    /// API authentication key.
    ApiKey,
}

/// Draft values collected during interactive portal creation.
#[derive(Debug, Clone, Default)]
pub struct NewPortalDraft {
    /// Display name.
    pub name: String,
    /// Model name.
    pub model_name: String,
    /// Base URL.
    pub base_url: String,
    /// API key.
    pub api_key: String,
}

/// Role classification for messages displayed in the agent chat interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatRole {
    /// Message sent by the human user.
    User,
    /// Response or final answer from the autonomous agent.
    Assistant,
    /// Informational system notification or execution lifecycle event.
    System,
    /// Tool invocation or execution observation.
    Tool,
    /// Internal reasoning trace or thinking tokens.
    Thinking,
    /// Error message.
    Error,
}

/// An individual message item displayed in the agent chat interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessageItem {
    /// Role of the message sender.
    pub role: ChatRole,
    /// Message content text.
    pub content: String,
    /// Timestamp when the message was recorded.
    pub timestamp: String,
    /// Whether this message (such as a thinking trace) is expanded in the UI.
    pub is_expanded: bool,
}

impl ChatMessageItem {
    /// Create a new message item with default expansion and current timestamp.
    pub fn new(role: ChatRole, content: impl Into<String>) -> Self {
        let now = chrono::Utc::now().format("%H:%M:%S").to_string();
        Self {
            role,
            content: content.into(),
            timestamp: now,
            is_expanded: false,
        }
    }

    /// Set an explicit timestamp on the message item.
    pub fn with_timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = timestamp.into();
        self
    }

    /// Set whether the item is expanded.
    pub fn with_expanded(mut self, is_expanded: bool) -> Self {
        self.is_expanded = is_expanded;
        self
    }
}

/// Updates emitted by background agent execution worker threads.
#[derive(Debug)]
pub enum ChatAgentUpdate {
    /// Agent execution initiated.
    Started {
        /// Associated run identifier.
        run_id: String,
        /// Model descriptor name.
        model: String,
    },
    /// Agent completed normally.
    Completed(Box<AgentRunResult>),
    /// Agent was cancelled.
    Cancelled(String),
    /// Agent encountered a fatal error.
    Error(String),
}

/// Execution handle for an asynchronous agent execution thread.
pub struct AgentExecutionHandle {
    /// Cancellation token to stop the running agent.
    pub cancel_token: CancellationToken,
    /// Channel receiver for agent updates and terminal outcomes.
    pub receiver: Receiver<ChatAgentUpdate>,
    /// Unique run ID assigned to this execution.
    pub run_id: Option<String>,
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

    // Chat interface state
    /// Recorded conversation and execution stream items.
    pub chat_messages: Vec<ChatMessageItem>,
    /// Current prompt text in the input box.
    pub chat_input: String,
    /// Insertion cursor position within the chat prompt input buffer.
    pub chat_cursor: usize,
    /// Prompt and slash command input history.
    pub chat_history: Vec<String>,
    /// Active index within the command history during Up/Down browsing.
    pub chat_history_idx: Option<usize>,
    /// Flag indicating whether the chat should automatically pin scroll to the newest message.
    pub chat_auto_scroll: bool,
    /// Global expansion state for thinking traces.
    pub thinking_expanded: bool,
    /// Message stream scroll offset when manual scrolling is active.
    pub chat_scroll: usize,
    /// Whether an agent task is actively executing in background.
    pub chat_is_running: bool,
    /// Active agent background execution handle.
    pub chat_handle: Option<AgentExecutionHandle>,
    /// Highest event sequence number already rendered in chat stream.
    pub chat_last_event_seq: u64,

    // Portals configuration state
    /// List of configured model portals.
    pub portals: Vec<ModelPortal>,
    /// Index of selected portal in table.
    pub selected_portal_idx: usize,
    /// Active interactive input mode in portals tab.
    pub portal_input_mode: PortalInputMode,
    /// Text buffer for editing portal fields.
    pub portal_input_buffer: String,
    /// Draft state for multi-step portal creation.
    pub new_portal_draft: NewPortalDraft,
}

impl App {
    /// Create a new [`App`] initialized with an optional storage handle.
    pub fn new(store: Option<Arc<RunStore>>) -> Self {
        let ws = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());

        let mut app = Self {
            active_tab: ActiveTab::Chat,
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
                    role: "Autonomous Bug Fixing & Implementation".to_string(),
                    policy: "Inspect files before modifying, write minimal tests, verify with cargo test".to_string(),
                    status: "Ready".to_string(),
                    model: "gpt-4o-mini".to_string(),
                    workspace: ws.clone(),
                    tools: vec![
                        "read_file".into(),
                        "write_file".into(),
                        "list_dir".into(),
                        "shell".into(),
                        "git_*".into(),
                    ],
                    runs_count: 0,
                },
                AgentView {
                    id: "eval-01".to_string(),
                    name: "Benchmark Evaluator".to_string(),
                    role: "Deterministic Benchmark & Test Runner".to_string(),
                    policy: "Executes reproducible evaluation suites and collects ground-truth metrics".to_string(),
                    status: "Ready".to_string(),
                    model: "gpt-4o".to_string(),
                    workspace: "Isolated".to_string(),
                    tools: vec!["read_file".into(), "shell".into()],
                    runs_count: 0,
                },
                AgentView {
                    id: "reviewer-01".to_string(),
                    name: "Policy Reviewer".to_string(),
                    role: "PR Policy & Security Compliance".to_string(),
                    policy: "Audits git diffs, detects secret leaks, enforces code conventions".to_string(),
                    status: "Paused".to_string(),
                    model: "claude-3-5-sonnet".to_string(),
                    workspace: "Read-Only".to_string(),
                    tools: vec!["read_file".into(), "git_diff".into(), "git_log".into()],
                    runs_count: 0,
                },
            ],
            selected_agent_idx: 0,
            should_quit: false,
            status_message: Some("Cortex Harness active. Type /help for slash commands, 'q' to quit.".to_string()),

            chat_messages: Vec::new(),
            chat_input: String::new(),
            chat_cursor: 0,
            chat_history: Vec::new(),
            chat_history_idx: None,
            chat_auto_scroll: true,
            thinking_expanded: false,
            chat_scroll: 0,
            chat_is_running: false,
            chat_handle: None,
            chat_last_event_seq: 0,

            portals: Self::default_portals(),
            selected_portal_idx: 0,
            portal_input_mode: PortalInputMode::Normal,
            portal_input_buffer: String::new(),
            new_portal_draft: NewPortalDraft::default(),
        };

        app.load_tasks();
        app.refresh();
        app
    }

    /// Default set of preconfigured model portals.
    pub fn default_portals() -> Vec<ModelPortal> {
        vec![
            ModelPortal::new(
                "openai-gpt4o-mini",
                "OpenAI GPT-4o-mini",
                "openai",
                "gpt-4o-mini",
                None,
                None,
                true,
            ),
            ModelPortal::new(
                "openai-gpt4o",
                "OpenAI GPT-4o",
                "openai",
                "gpt-4o",
                None,
                None,
                false,
            ),
            ModelPortal::new(
                "anthropic-sonnet",
                "Anthropic Claude 3.5 Sonnet",
                "anthropic",
                "claude-3-5-sonnet-20241022",
                None,
                None,
                false,
            ),
            ModelPortal::new(
                "anthropic-haiku",
                "Anthropic Claude 3.5 Haiku",
                "anthropic",
                "claude-3-5-haiku-20241022",
                None,
                None,
                false,
            ),
            ModelPortal::new(
                "ollama-llama3",
                "Ollama LLaMA 3.1 (Local)",
                "ollama",
                "ollama/llama3.1",
                Some("http://localhost:11434/v1".to_string()),
                None,
                false,
            ),
            ModelPortal::new(
                "ollama-qwen",
                "Ollama Qwen 2.5 Coder (Local)",
                "ollama",
                "ollama/qwen2.5-coder",
                Some("http://localhost:11434/v1".to_string()),
                None,
                false,
            ),
            ModelPortal::new(
                "deepseek-chat",
                "DeepSeek Chat",
                "openai",
                "deepseek-chat",
                Some("https://api.deepseek.com/v1".to_string()),
                None,
                false,
            ),
        ]
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
            ActiveTab::Portals => {
                self.next_portal();
            }
            ActiveTab::Chat => {
                self.chat_scroll_down();
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
            ActiveTab::Portals => {
                self.prev_portal();
            }
            ActiveTab::Chat => {
                self.chat_scroll_up();
            }
            ActiveTab::ActiveRun => {}
        }
    }

    /// Action triggered by pressing Enter in item tables.
    pub fn select_current(&mut self) {
        match self.active_tab {
            ActiveTab::Dashboard | ActiveTab::History if !self.runs.is_empty() => {
                self.active_tab = ActiveTab::ActiveRun;
            }
            ActiveTab::Portals => {
                self.activate_selected_portal();
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

    // ==========================================
    // Chat & Autonomous Agent Execution Methods
    // ==========================================

    /// Reference to the currently active [`ModelPortal`].
    pub fn active_portal(&self) -> &ModelPortal {
        self.portals
            .iter()
            .find(|p| p.is_active)
            .unwrap_or(&self.portals[0])
    }

    /// Insert a character into the chat input buffer at current cursor position.
    pub fn chat_input_insert(&mut self, c: char) {
        if self.chat_cursor >= self.chat_input.len() {
            self.chat_input.push(c);
            self.chat_cursor = self.chat_input.len();
        } else {
            self.chat_input.insert(self.chat_cursor, c);
            self.chat_cursor += 1;
        }
    }

    /// Delete character immediately preceding the cursor in chat input.
    pub fn chat_input_backspace(&mut self) {
        if self.chat_cursor > 0 && !self.chat_input.is_empty() {
            self.chat_cursor -= 1;
            if self.chat_cursor < self.chat_input.len() {
                self.chat_input.remove(self.chat_cursor);
            }
        }
    }

    /// Delete character at current cursor position in chat input.
    pub fn chat_input_delete(&mut self) {
        if self.chat_cursor < self.chat_input.len() {
            self.chat_input.remove(self.chat_cursor);
        }
    }

    /// Move input cursor one position to the left.
    pub fn chat_input_left(&mut self) {
        self.chat_cursor = self.chat_cursor.saturating_sub(1);
    }

    /// Move input cursor one position to the right.
    pub fn chat_input_right(&mut self) {
        if self.chat_cursor < self.chat_input.len() {
            self.chat_cursor += 1;
        }
    }

    /// Move input cursor to beginning of input line.
    pub fn chat_input_home(&mut self) {
        self.chat_cursor = 0;
    }

    /// Move input cursor to end of input line.
    pub fn chat_input_end(&mut self) {
        self.chat_cursor = self.chat_input.len();
    }

    /// Clear chat prompt input line and reset cursor.
    pub fn chat_input_clear(&mut self) {
        self.chat_input.clear();
        self.chat_cursor = 0;
        self.chat_history_idx = None;
    }

    /// Recall previous command from history into input.
    pub fn chat_history_prev(&mut self) {
        if self.chat_history.is_empty() {
            return;
        }
        let next_idx = match self.chat_history_idx {
            None => self.chat_history.len() - 1,
            Some(idx) => idx.saturating_sub(1),
        };
        self.chat_history_idx = Some(next_idx);
        self.chat_input = self.chat_history[next_idx].clone();
        self.chat_cursor = self.chat_input.len();
    }

    /// Recall next command from history into input.
    pub fn chat_history_next(&mut self) {
        if self.chat_history.is_empty() {
            return;
        }
        if let Some(idx) = self.chat_history_idx {
            if idx + 1 < self.chat_history.len() {
                let next_idx = idx + 1;
                self.chat_history_idx = Some(next_idx);
                self.chat_input = self.chat_history[next_idx].clone();
                self.chat_cursor = self.chat_input.len();
            } else {
                self.chat_history_idx = None;
                self.chat_input.clear();
                self.chat_cursor = 0;
            }
        }
    }

    /// Set thinking trace expansion state globally and update all existing thinking items.
    pub fn set_thinking_expanded(&mut self, expanded: bool) {
        self.thinking_expanded = expanded;
        for msg in &mut self.chat_messages {
            if msg.role == ChatRole::Thinking {
                msg.is_expanded = expanded;
            }
        }
    }

    /// Toggle thinking trace expansion.
    pub fn toggle_thinking_expanded(&mut self) {
        let new_state = !self.thinking_expanded;
        self.set_thinking_expanded(new_state);
    }

    /// Scroll chat messages view upwards.
    pub fn chat_scroll_up(&mut self) {
        self.chat_auto_scroll = false;
        self.chat_scroll = self.chat_scroll.saturating_add(3);
    }

    /// Scroll chat messages view downwards.
    pub fn chat_scroll_down(&mut self) {
        self.chat_scroll = self.chat_scroll.saturating_sub(3);
        if self.chat_scroll == 0 {
            self.chat_auto_scroll = true;
        }
    }

    /// Submit current chat input prompt or slash command.
    pub fn dispatch_chat(&mut self) {
        let prompt = self.chat_input.trim().to_string();
        if prompt.is_empty() {
            return;
        }

        // Record prompt into input history
        if self.chat_history.last().map(|s| s.as_str()) != Some(&prompt) {
            self.chat_history.push(prompt.clone());
        }
        self.chat_history_idx = None;
        self.chat_input.clear();
        self.chat_cursor = 0;
        self.chat_auto_scroll = true;
        self.chat_scroll = 0;

        // Route slash commands
        if prompt.starts_with('/') {
            let now = chrono::Utc::now().format("%H:%M:%S").to_string();
            self.chat_messages.push(ChatMessageItem {
                role: ChatRole::User,
                content: prompt.clone(),
                timestamp: now.clone(),
                is_expanded: false,
            });

            if let Some(command) = crate::commands::parse_command(&prompt) {
                let response = crate::commands::execute_command(self, command);
                self.chat_messages.push(ChatMessageItem {
                    role: ChatRole::System,
                    content: response,
                    timestamp: now,
                    is_expanded: false,
                });
            }
            return;
        }

        if self.chat_is_running {
            self.status_message = Some(
                "An agent is already running. Press Esc to cancel it before launching a new task."
                    .to_string(),
            );
            return;
        }

        let now = chrono::Utc::now().format("%H:%M:%S").to_string();
        self.chat_messages.push(ChatMessageItem {
            role: ChatRole::User,
            content: prompt.clone(),
            timestamp: now,
            is_expanded: false,
        });

        let active_portal = self.active_portal().clone();
        let cancel_token = CancellationToken::new();
        let (sender, receiver) = channel();

        self.chat_is_running = true;
        self.chat_last_event_seq = 0;
        self.chat_handle = Some(AgentExecutionHandle {
            cancel_token: cancel_token.clone(),
            receiver,
            run_id: None,
        });

        let store_clone = self.store.clone();
        let model = active_portal.model_name.clone();
        let api_key = active_portal.api_key.clone();
        let base_url = active_portal.base_url.clone();

        std::thread::spawn(move || {
            execute_chat_agent(
                prompt,
                model,
                api_key,
                base_url,
                store_clone,
                cancel_token,
                sender,
            );
        });
    }

    /// Cancel the currently executing agent run, if any.
    pub fn cancel_chat_agent(&mut self) {
        if let Some(handle) = &self.chat_handle {
            handle.cancel_token.cancel();
            let now = chrono::Utc::now().format("%H:%M:%S").to_string();
            self.chat_messages.push(ChatMessageItem {
                role: ChatRole::System,
                content: "Cancellation requested... Waiting for agent to abort gracefully."
                    .to_string(),
                timestamp: now,
                is_expanded: false,
            });
            self.status_message = Some("Sent cancellation signal to running agent.".to_string());
        }
    }

    /// Poll for real-time updates from background agent executions and event store.
    pub fn poll_chat_updates(&mut self) {
        let mut finished = false;
        let mut new_run_id: Option<String> = None;

        if let Some(handle) = &mut self.chat_handle {
            while let Ok(update) = handle.receiver.try_recv() {
                let now = chrono::Utc::now().format("%H:%M:%S").to_string();
                match update {
                    ChatAgentUpdate::Started { run_id, model } => {
                        handle.run_id = Some(run_id.clone());
                        new_run_id = Some(run_id.clone());
                        self.chat_messages.push(ChatMessageItem {
                            role: ChatRole::System,
                            content: format!(
                                "Agent execution started (Run ID: {}, Model: {})",
                                run_id, model
                            ),
                            timestamp: now,
                            is_expanded: false,
                        });
                    }
                    ChatAgentUpdate::Completed(res) => {
                        let answer = &res.final_answer;
                        if let (Some(start), Some(end)) =
                            (answer.find("<think>"), answer.find("</think>"))
                        {
                            let thought = answer[start + 7..end].trim().to_string();
                            let remaining = answer[end + 8..].trim().to_string();
                            if !thought.is_empty() {
                                self.chat_messages.push(ChatMessageItem {
                                    role: ChatRole::Thinking,
                                    content: thought,
                                    timestamp: now.clone(),
                                    is_expanded: self.thinking_expanded,
                                });
                            }
                            self.chat_messages.push(ChatMessageItem {
                                role: ChatRole::Assistant,
                                content: remaining,
                                timestamp: now.clone(),
                                is_expanded: false,
                            });
                        } else {
                            self.chat_messages.push(ChatMessageItem {
                                role: ChatRole::Assistant,
                                content: res.final_answer.clone(),
                                timestamp: now.clone(),
                                is_expanded: false,
                            });
                        }

                        self.chat_messages.push(ChatMessageItem {
                            role: ChatRole::System,
                            content: format!(
                                "Run completed in {}ms across {} iterations | Tokens: {} | Cost: ${:.4}",
                                res.duration_ms,
                                res.iterations,
                                res.tokens_total,
                                res.estimated_cost_usd
                            ),
                            timestamp: now,
                            is_expanded: false,
                        });
                        finished = true;
                    }
                    ChatAgentUpdate::Cancelled(reason) => {
                        self.chat_messages.push(ChatMessageItem {
                            role: ChatRole::System,
                            content: format!("Agent run cancelled: {}", reason),
                            timestamp: now,
                            is_expanded: false,
                        });
                        finished = true;
                    }
                    ChatAgentUpdate::Error(err) => {
                        self.chat_messages.push(ChatMessageItem {
                            role: ChatRole::Error,
                            content: format!("Agent execution error: {}", err),
                            timestamp: now,
                            is_expanded: false,
                        });
                        finished = true;
                    }
                }
            }

            // Stream new intermediate events from SQLite store if available
            if let (Some(store), Some(run_id)) = (&self.store, &handle.run_id) {
                let r_id = RunId::from(run_id.clone());
                if let Ok(events) = store.get_events(&r_id) {
                    for event in events {
                        if event.sequence > self.chat_last_event_seq {
                            self.chat_last_event_seq = event.sequence;
                            let ts = event
                                .timestamp
                                .split('T')
                                .nth(1)
                                .unwrap_or(&event.timestamp)
                                .to_string();
                            match &event.event {
                                ExecutionEvent::ToolStarted {
                                    tool_name,
                                    arguments,
                                    ..
                                } => {
                                    let args_str = arguments.to_string();
                                    let preview = if args_str.len() > 60 {
                                        format!("{}...", &args_str[..57])
                                    } else {
                                        args_str
                                    };
                                    self.chat_messages.push(ChatMessageItem {
                                        role: ChatRole::Tool,
                                        content: format!("Tool call: {} ({})", tool_name, preview),
                                        timestamp: ts,
                                        is_expanded: false,
                                    });
                                }
                                ExecutionEvent::ToolCompleted {
                                    tool_name, output, ..
                                } => {
                                    let preview = if output.len() > 60 {
                                        format!("{}...", &output[..57])
                                    } else {
                                        output.clone()
                                    };
                                    self.chat_messages.push(ChatMessageItem {
                                        role: ChatRole::Tool,
                                        content: format!(
                                            "Tool {} finished -> {}",
                                            tool_name, preview
                                        ),
                                        timestamp: ts,
                                        is_expanded: false,
                                    });
                                }
                                ExecutionEvent::ToolFailed {
                                    tool_name, error, ..
                                } => {
                                    self.chat_messages.push(ChatMessageItem {
                                        role: ChatRole::Error,
                                        content: format!("Tool {} failed: {}", tool_name, error),
                                        timestamp: ts,
                                        is_expanded: false,
                                    });
                                }
                                ExecutionEvent::ModelRequest { prompt_preview, .. } => {
                                    let preview = if prompt_preview.len() > 60 {
                                        format!("{}...", &prompt_preview[..57])
                                    } else {
                                        prompt_preview.clone()
                                    };
                                    self.chat_messages.push(ChatMessageItem {
                                        role: ChatRole::Thinking,
                                        content: format!("Planning next action:\n{}", preview),
                                        timestamp: ts,
                                        is_expanded: self.thinking_expanded,
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        if finished {
            self.chat_handle = None;
            self.chat_is_running = false;
            self.refresh();
        } else if new_run_id.is_some() {
            self.refresh();
        }
    }

    // ==========================================
    // Persistent Agent State Machine & Management
    // ==========================================

    /// Register a new persistent agent in the workspace.
    pub fn register_agent(
        &mut self,
        id: String,
        name: String,
        role: String,
        model: String,
        policy: String,
    ) {
        let ws = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());
        let agent = AgentView {
            id,
            name,
            role,
            policy,
            status: "Ready".to_string(),
            model,
            workspace: ws,
            tools: vec![
                "read_file".into(),
                "write_file".into(),
                "list_dir".into(),
                "shell".into(),
                "git_*".into(),
            ],
            runs_count: 0,
        };
        self.agents.push(agent);
    }

    /// Transition a persistent agent to a new lifecycle state.
    pub fn transition_agent_state(
        &mut self,
        id: &str,
        new_state: AgentLifecycleState,
    ) -> Result<(), String> {
        if let Some(agent) = self
            .agents
            .iter_mut()
            .find(|a| a.id.eq_ignore_ascii_case(id))
        {
            agent.status = new_state.as_str().to_string();
            self.status_message = Some(format!(
                "Agent '{}' transitioned to {}.",
                id,
                new_state.as_str()
            ));
            Ok(())
        } else {
            Err(format!("Agent '{}' not found", id))
        }
    }

    /// Transition currently selected agent to RUNNING.
    pub fn start_selected_agent(&mut self) {
        if let Some(agent) = self.agents.get(self.selected_agent_idx) {
            let id = agent.id.clone();
            let _ = self.transition_agent_state(&id, AgentLifecycleState::Running);
        }
    }

    /// Transition currently selected agent to STOPPED.
    pub fn stop_selected_agent(&mut self) {
        if let Some(agent) = self.agents.get(self.selected_agent_idx) {
            let id = agent.id.clone();
            let _ = self.transition_agent_state(&id, AgentLifecycleState::Stopped);
        }
    }

    /// Transition currently selected agent to PAUSED.
    pub fn pause_selected_agent(&mut self) {
        if let Some(agent) = self.agents.get(self.selected_agent_idx) {
            let id = agent.id.clone();
            let _ = self.transition_agent_state(&id, AgentLifecycleState::Paused);
        }
    }

    // ==========================================
    // Portals & Provider Configuration Methods
    // ==========================================

    /// Navigate to next portal in table.
    pub fn next_portal(&mut self) {
        if !self.portals.is_empty() && self.selected_portal_idx + 1 < self.portals.len() {
            self.selected_portal_idx += 1;
        }
    }

    /// Navigate to previous portal in table.
    pub fn prev_portal(&mut self) {
        if self.selected_portal_idx > 0 {
            self.selected_portal_idx -= 1;
        }
    }

    /// Mark the currently selected portal as active.
    pub fn activate_selected_portal(&mut self) {
        if self.portals.is_empty() {
            return;
        }
        for (i, p) in self.portals.iter_mut().enumerate() {
            p.is_active = i == self.selected_portal_idx;
        }
        let name = self.portals[self.selected_portal_idx].name.clone();
        self.status_message = Some(format!("Activated portal: {}", name));
    }

    /// Delete the selected portal if more than one exists.
    pub fn delete_selected_portal(&mut self) {
        if self.portals.len() <= 1 {
            self.status_message = Some("Cannot delete the only remaining portal.".to_string());
            return;
        }
        let was_active = self.portals[self.selected_portal_idx].is_active;
        let name = self.portals.remove(self.selected_portal_idx).name;
        if self.selected_portal_idx >= self.portals.len() {
            self.selected_portal_idx = self.portals.len() - 1;
        }
        if was_active {
            self.portals[0].is_active = true;
        }
        self.status_message = Some(format!("Deleted portal '{}'.", name));
    }

    /// Begin interactive flow for adding a new portal.
    pub fn start_adding_portal(&mut self) {
        self.portal_input_mode = PortalInputMode::Adding {
            field: PortalField::Name,
        };
        self.portal_input_buffer.clear();
        self.new_portal_draft = NewPortalDraft::default();
        self.status_message = Some("Creating new portal: Enter Portal Name.".to_string());
    }

    /// Begin editing API key for currently selected portal.
    pub fn start_editing_key(&mut self) {
        if let Some(portal) = self.portals.get(self.selected_portal_idx) {
            self.portal_input_mode = PortalInputMode::EditingKey;
            self.portal_input_buffer = portal.api_key.clone().unwrap_or_default();
            self.status_message = Some(format!(
                "Editing API key for '{}'. Press Enter to save, Esc to cancel.",
                portal.name
            ));
        }
    }

    /// Begin editing Base URL for currently selected portal.
    pub fn start_editing_base_url(&mut self) {
        if let Some(portal) = self.portals.get(self.selected_portal_idx) {
            self.portal_input_mode = PortalInputMode::EditingBaseUrl;
            self.portal_input_buffer = portal.base_url.clone().unwrap_or_default();
            self.status_message = Some(format!(
                "Editing Base URL for '{}'. Press Enter to save, Esc to cancel.",
                portal.name
            ));
        }
    }

    /// Confirm the current interactive input field in Portals tab.
    pub fn confirm_portal_input(&mut self) {
        match self.portal_input_mode {
            PortalInputMode::Normal => {}
            PortalInputMode::EditingKey => {
                let trimmed = self.portal_input_buffer.trim().to_string();
                let key_opt = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                };
                if let Some(portal) = self.portals.get_mut(self.selected_portal_idx) {
                    portal.api_key = key_opt;
                    self.status_message = Some(format!("Updated API key for '{}'.", portal.name));
                }
                self.portal_input_mode = PortalInputMode::Normal;
                self.portal_input_buffer.clear();
            }
            PortalInputMode::EditingBaseUrl => {
                let trimmed = self.portal_input_buffer.trim().to_string();
                let url_opt = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                };
                if let Some(portal) = self.portals.get_mut(self.selected_portal_idx) {
                    portal.base_url = url_opt;
                    self.status_message = Some(format!("Updated Base URL for '{}'.", portal.name));
                }
                self.portal_input_mode = PortalInputMode::Normal;
                self.portal_input_buffer.clear();
            }
            PortalInputMode::Adding { field } => match field {
                PortalField::Name => {
                    let val = self.portal_input_buffer.trim().to_string();
                    self.new_portal_draft.name = if val.is_empty() {
                        "Custom Portal".to_string()
                    } else {
                        val
                    };
                    self.portal_input_buffer.clear();
                    self.portal_input_mode = PortalInputMode::Adding {
                        field: PortalField::Model,
                    };
                    self.status_message = Some(
                        "Enter Model identifier (e.g. gpt-4o, claude-3-5-sonnet, ollama/llama3.1):"
                            .to_string(),
                    );
                }
                PortalField::Model => {
                    let val = self.portal_input_buffer.trim().to_string();
                    self.new_portal_draft.model_name = if val.is_empty() {
                        "gpt-4o-mini".to_string()
                    } else {
                        val
                    };
                    self.portal_input_buffer.clear();
                    self.portal_input_mode = PortalInputMode::Adding {
                        field: PortalField::BaseUrl,
                    };
                    self.status_message =
                        Some("Enter Base URL (or leave blank for provider default):".to_string());
                }
                PortalField::BaseUrl => {
                    let val = self.portal_input_buffer.trim().to_string();
                    self.new_portal_draft.base_url = val;
                    self.portal_input_buffer.clear();
                    self.portal_input_mode = PortalInputMode::Adding {
                        field: PortalField::ApiKey,
                    };
                    self.status_message = Some(
                        "Enter API Key (or leave blank if using environment variable):".to_string(),
                    );
                }
                PortalField::ApiKey => {
                    let key_val = self.portal_input_buffer.trim().to_string();
                    self.new_portal_draft.api_key = key_val;
                    let id = format!("portal-{}", chrono::Utc::now().timestamp_millis());
                    let lower = self.new_portal_draft.model_name.to_lowercase();
                    let provider_kind = if lower.starts_with("ollama/") {
                        "ollama".to_string()
                    } else if lower.starts_with("claude") {
                        "anthropic".to_string()
                    } else {
                        "openai".to_string()
                    };
                    let base_url = if self.new_portal_draft.base_url.is_empty() {
                        None
                    } else {
                        Some(self.new_portal_draft.base_url.clone())
                    };
                    let api_key = if self.new_portal_draft.api_key.is_empty() {
                        None
                    } else {
                        Some(self.new_portal_draft.api_key.clone())
                    };

                    let new_portal = ModelPortal::new(
                        id,
                        self.new_portal_draft.name.clone(),
                        provider_kind,
                        self.new_portal_draft.model_name.clone(),
                        base_url,
                        api_key,
                        true,
                    );

                    for p in &mut self.portals {
                        p.is_active = false;
                    }
                    self.portals.push(new_portal);
                    self.selected_portal_idx = self.portals.len() - 1;
                    self.portal_input_mode = PortalInputMode::Normal;
                    self.portal_input_buffer.clear();
                    self.new_portal_draft = NewPortalDraft::default();
                    self.status_message = Some("Created and activated new portal.".to_string());
                }
            },
        }
    }

    /// Cancel any active portal input mode and discard buffer.
    pub fn cancel_portal_input(&mut self) {
        self.portal_input_mode = PortalInputMode::Normal;
        self.portal_input_buffer.clear();
        self.new_portal_draft = NewPortalDraft::default();
        self.status_message = Some("Cancelled portal edit.".to_string());
    }

    /// Test configuration of currently selected portal.
    pub fn test_selected_portal(&mut self) {
        if let Some(portal) = self.portals.get(self.selected_portal_idx) {
            if portal.is_configured() {
                self.status_message = Some(format!(
                    "Portal '{}' ({}) is configured and ready.",
                    portal.name,
                    portal.status_text()
                ));
            } else {
                self.status_message = Some(format!(
                    "Portal '{}' requires credentials! Press 'e' to set API key or export env var.",
                    portal.name
                ));
            }
        }
    }
}

/// Asynchronous worker executing the agent iteration loop in background.
fn execute_chat_agent(
    prompt: String,
    model: String,
    api_key: Option<String>,
    base_url: Option<String>,
    store: Option<Arc<RunStore>>,
    cancel_token: CancellationToken,
    sender: Sender<ChatAgentUpdate>,
) {
    let ws_path = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let ws = match Workspace::new(&ws_path) {
        Ok(w) => Arc::new(w),
        Err(e) => {
            let _ = sender.send(ChatAgentUpdate::Error(format!(
                "Failed to initialize workspace at {}: {}",
                ws_path.display(),
                e
            )));
            return;
        }
    };

    let registry = ToolRegistry::new();
    let reg_res = (|| -> cortex_core::Result<()> {
        registry.register(Arc::new(tools::ReadFileTool::new(ws.clone())))?;
        registry.register(Arc::new(tools::WriteFileTool::new(ws.clone())))?;
        registry.register(Arc::new(tools::ListDirTool::new(ws.clone())))?;
        registry.register(Arc::new(tools::ShellTool::new(ws.clone())))?;
        registry.register(Arc::new(tools::GitStatusTool::new(ws.clone())))?;
        registry.register(Arc::new(tools::GitDiffTool::new(ws.clone())))?;
        registry.register(Arc::new(tools::GitCommitTool::new(ws.clone())))?;
        registry.register(Arc::new(tools::GitLogTool::new(ws.clone())))?;
        registry.register(Arc::new(tools::GitBranchTool::new(ws.clone())))?;
        Ok(())
    })();

    if let Err(e) = reg_res {
        let _ = sender.send(ChatAgentUpdate::Error(format!(
            "Failed to configure tools: {}",
            e
        )));
        return;
    }

    let provider = match create_model_provider(&model, api_key, base_url) {
        Ok(p) => p,
        Err(e) => {
            let _ = sender.send(ChatAgentUpdate::Error(format!(
                "Failed to instantiate model provider: {}",
                e
            )));
            return;
        }
    };

    match provider.is_configured() {
        Ok(true) => {}
        Ok(false) => {
            let _ = sender.send(ChatAgentUpdate::Error(format!(
                "Model provider '{}' for model '{}' is not configured.\n\
                 Please configure your API key in Portals (Tab 8) or set OPENAI_API_KEY / ANTHROPIC_API_KEY.",
                provider.descriptor().provider,
                model
            )));
            return;
        }
        Err(e) => {
            let _ = sender.send(ChatAgentUpdate::Error(format!(
                "Provider configuration check failed: {}",
                e
            )));
            return;
        }
    }

    let mut context = AgentContext::new(&prompt).with_workspace(ws);
    let run_id = context.run_id.to_string();

    let _ = sender.send(ChatAgentUpdate::Started {
        run_id: run_id.clone(),
        model: model.clone(),
    });

    let agent_loop = AgentLoop::new(15)
        .with_cancellation_token(cancel_token)
        .with_store_optional(store);

    match agent_loop.run(&mut context, &*provider, &registry) {
        Ok(res) => {
            let _ = sender.send(ChatAgentUpdate::Completed(Box::new(res)));
        }
        Err(cortex_core::CortexError::Cancelled(reason)) => {
            let _ = sender.send(ChatAgentUpdate::Cancelled(reason));
        }
        Err(e) => {
            let _ = sender.send(ChatAgentUpdate::Error(e.to_string()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_initialization_and_tab_cycling() {
        let mut app = App::new(None);
        assert_eq!(app.active_tab, ActiveTab::Chat);

        app.set_tab(ActiveTab::Dashboard);
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

        app.set_tab(ActiveTab::Chat);
        assert_eq!(app.active_tab, ActiveTab::Chat);

        app.set_tab(ActiveTab::Portals);
        assert_eq!(app.active_tab, ActiveTab::Portals);
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

    #[test]
    fn test_portal_activation_and_deletion() {
        let mut app = App::new(None);
        assert!(!app.portals.is_empty());
        assert!(app.portals[0].is_active);

        // Switch active portal
        app.selected_portal_idx = 1;
        app.activate_selected_portal();
        assert!(!app.portals[0].is_active);
        assert!(app.portals[1].is_active);

        // Delete portal
        let count_before = app.portals.len();
        app.delete_selected_portal();
        assert_eq!(app.portals.len(), count_before - 1);
    }

    #[test]
    fn test_portal_editing_flow() {
        let mut app = App::new(None);
        app.selected_portal_idx = 0;

        app.start_editing_key();
        assert_eq!(app.portal_input_mode, PortalInputMode::EditingKey);
        app.portal_input_buffer = "sk-test-custom-key".to_string();
        app.confirm_portal_input();

        assert_eq!(app.portal_input_mode, PortalInputMode::Normal);
        assert_eq!(
            app.portals[0].api_key,
            Some("sk-test-custom-key".to_string())
        );
    }

    #[test]
    fn test_portal_adding_flow() {
        let mut app = App::new(None);
        let count_before = app.portals.len();

        app.start_adding_portal();
        assert_eq!(
            app.portal_input_mode,
            PortalInputMode::Adding {
                field: PortalField::Name
            }
        );

        app.portal_input_buffer = "My Custom Ollama".to_string();
        app.confirm_portal_input();
        assert_eq!(
            app.portal_input_mode,
            PortalInputMode::Adding {
                field: PortalField::Model
            }
        );

        app.portal_input_buffer = "ollama/deepseek-r1".to_string();
        app.confirm_portal_input();
        assert_eq!(
            app.portal_input_mode,
            PortalInputMode::Adding {
                field: PortalField::BaseUrl
            }
        );

        app.portal_input_buffer = "http://127.0.0.1:11434/v1".to_string();
        app.confirm_portal_input();
        assert_eq!(
            app.portal_input_mode,
            PortalInputMode::Adding {
                field: PortalField::ApiKey
            }
        );

        app.portal_input_buffer = "".to_string();
        app.confirm_portal_input();

        assert_eq!(app.portal_input_mode, PortalInputMode::Normal);
        assert_eq!(app.portals.len(), count_before + 1);
        let added = app.portals.last().unwrap();
        assert_eq!(added.name, "My Custom Ollama");
        assert_eq!(added.model_name, "ollama/deepseek-r1");
        assert_eq!(
            added.base_url,
            Some("http://127.0.0.1:11434/v1".to_string())
        );
        assert!(added.is_active);
    }
}
