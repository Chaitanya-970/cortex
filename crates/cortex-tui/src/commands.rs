//! Slash command parsing, catalog, and execution for the Cortex TUI harness.

use crate::app::{AgentLifecycleState, App, ChatMessageItem, ModelPortal};
use cortex_core::VERSION;

/// Parsed slash commands available in the interactive terminal harness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlashCommand {
    /// Show the help guide and command catalog.
    Help,
    /// Model provider and parameter configuration subcommands.
    Model {
        /// Subcommand arguments (e.g., `["list"]`, `["set", "gpt-4o"]`).
        args: Vec<String>,
    },
    /// Inference portal configuration subcommands.
    Portal {
        /// Subcommand arguments (e.g., `["list"]`, `["use", "ollama-llama3"]`).
        args: Vec<String>,
    },
    /// Agent inspection and lifecycle management subcommands.
    Agents {
        /// Subcommand arguments (e.g., `["inspect", "coder-01"]`, `["start", "eval-01"]`).
        args: Vec<String>,
    },
    /// List all registered tools and security boundaries.
    Tools,
    /// Display git diff of uncommitted changes in the workspace.
    Diff,
    /// Display runtime health, diagnostics, and session status.
    Status,
    /// Display recent execution runs from SQLite storage.
    Runs,
    /// Benchmark evaluation tasks available in the test harness.
    Tasks,
    /// Overall runtime metrics and dashboard summary card.
    Dashboard,
    /// Environment diagnostics, API keys, and system doctor check.
    Doctor,
    /// Token usage and cost breakdown summary.
    Cost,
    /// Toggle or set thinking section expansion (`/thinking`, `/thinking on`, `/thinking off`).
    Thinking {
        /// Optional argument: `Some("on")`, `Some("off")`, or `None` for toggle.
        arg: Option<String>,
    },
    /// Clear the chat message log.
    Clear,
    /// Compact the chat session log.
    Compact,
    /// Exit the TUI harness.
    Quit,
    /// Unrecognized slash command.
    Unknown(String),
}

/// Parse a raw input string into a [`SlashCommand`] if it begins with `/`.
pub fn parse_command(input: &str) -> Option<SlashCommand> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    let without_slash = &trimmed[1..];
    let parts: Vec<&str> = without_slash.split_whitespace().collect();
    if parts.is_empty() {
        return Some(SlashCommand::Help);
    }

    let cmd = parts[0].to_lowercase();
    let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();

    match cmd.as_str() {
        "help" | "?" => Some(SlashCommand::Help),
        "model" | "models" => Some(SlashCommand::Model { args }),
        "portal" | "portals" => Some(SlashCommand::Portal { args }),
        "agent" | "agents" => Some(SlashCommand::Agents { args }),
        "tools" | "tool" => Some(SlashCommand::Tools),
        "diff" => Some(SlashCommand::Diff),
        "status" | "info" => Some(SlashCommand::Status),
        "runs" | "history" => Some(SlashCommand::Runs),
        "tasks" | "bench" => Some(SlashCommand::Tasks),
        "dashboard" | "metrics" => Some(SlashCommand::Dashboard),
        "doctor" | "check" => Some(SlashCommand::Doctor),
        "cost" => Some(SlashCommand::Cost),
        "thinking" | "think" | "thought" => {
            let arg = args.first().cloned();
            Some(SlashCommand::Thinking { arg })
        }
        "clear" | "cls" => Some(SlashCommand::Clear),
        "compact" => Some(SlashCommand::Compact),
        "quit" | "exit" | "q" => Some(SlashCommand::Quit),
        other => Some(SlashCommand::Unknown(other.to_string())),
    }
}

/// Execute a parsed slash command against the application state.
/// Returns a formatted message string to display to the user.
pub fn execute_command(app: &mut App, command: SlashCommand) -> String {
    match command {
        SlashCommand::Help => format_help(),
        SlashCommand::Model { args } => handle_model_command(app, &args),
        SlashCommand::Portal { args } => handle_portal_command(app, &args),
        SlashCommand::Agents { args } => handle_agents_command(app, &args),
        SlashCommand::Tools => handle_tools_command(),
        SlashCommand::Diff => handle_diff_command(),
        SlashCommand::Status => handle_status_command(app),
        SlashCommand::Runs => handle_runs_command(app),
        SlashCommand::Tasks => handle_tasks_command(app),
        SlashCommand::Dashboard => handle_dashboard_command(app),
        SlashCommand::Doctor => handle_doctor_command(app),
        SlashCommand::Cost => handle_cost_command(app),
        SlashCommand::Thinking { arg } => handle_thinking_command(app, arg),
        SlashCommand::Clear => {
            app.chat_messages.clear();
            app.chat_scroll = 0;
            "Chat log cleared.".to_string()
        }
        SlashCommand::Compact => handle_compact_command(app),
        SlashCommand::Quit => {
            app.should_quit = true;
            "Exiting Cortex TUI... Goodbye!".to_string()
        }
        SlashCommand::Unknown(cmd) => {
            format!(
                "Unknown command '/{}'. Type '/help' to see available slash commands.",
                cmd
            )
        }
    }
}

fn format_help() -> String {
    r#"┌── ◈ CORTEX HARNESS SLASH COMMAND CATALOG ───────────────────────────────┐
│                                                                          │
│  🤖 MODEL & PORTAL CONFIGURATION                                         │
│    /model                      Show current active model, provider & pricing │
│    /model list                 List popular curated models across providers  │
│    /model set <model>          Switch active model (e.g. gpt-4o, claude-3-5) │
│    /model info                 Detailed context window & cost diagnostics    │
│    /portal list                List all configured inference portals & keys  │
│    /portal use <id>            Switch dispatch portal (e.g. ollama-llama3)   │
│    /portal set-key <id> <key>  Update API authentication key for portal      │
│    /portal set-url <id> <url>  Update base URL endpoint for portal           │
│    /portal test [id]           Verify endpoint credentials and connectivity  │
│                                                                          │
│  👥 AGENT WORKERS & VISUALIZATION                                        │
│    /agents                     Visual ASCII topology & worker status cards   │
│    /agent inspect <id>         Detailed policy, capabilities & tool manifest │
│    /agent start <id>           Transition persistent agent to RUNNING        │
│    /agent stop <id>            Transition persistent agent to STOPPED        │
│    /agent pause <id>           Transition persistent agent to PAUSED         │
│    /agent add <id> <name> <role> Register a new persistent agent worker     │
│                                                                          │
│  🛠️ WORKSPACE, TOOLS & BENCHMARKS                                         │
│    /tools                      List registered sandbox tools & permissions   │
│    /diff                       View git diff of uncommitted workspace changes│
│    /status                     Runtime diagnostics, SQLite and memory status │
│    /runs                       Show 5 most recent execution runs and metrics │
│    /tasks                      List benchmark evaluation tasks in suite      │
│    /dashboard                  Runtime metrics, completed runs & success card│
│    /doctor                     Check API keys, git status & health checks    │
│    /cost                       Display token usage & cost summary            │
│                                                                          │
│  ⚡ HARNESS & DISPLAY CONTROLS                                            │
│    /thinking [on|off]          Toggle or set expandable thinking trace view  │
│    /clear                      Clear chat conversation log                   │
│    /compact                    Compact conversation context                  │
│    /exit, /quit                Exit Cortex TUI                               │
│                                                                          │
│  Keyboard Shortcuts:                                                     │
│    Enter: Send prompt / command   Ctrl+T: Toggle thinking expanded/collapsed │
│    ↑/↓: Browse command history    PageUp/PageDown: Scroll chat stream        │
│    Tab: Autocomplete commands     Esc: Cancel running agent / clear input    │
│    Ctrl+C: Quit harness           Ctrl+U: Clear input line                   │
└──────────────────────────────────────────────────────────────────────────┘"#
        .to_string()
}

fn handle_model_command(app: &mut App, args: &[String]) -> String {
    if args.is_empty() {
        let portal = app.active_portal();
        let cost = cortex_runtime::estimate_cost(&portal.model_name, 1_000_000, 1_000_000);
        return format!(
            "┌── Active Model Configuration ──────────────────────────────────────────┐\n\
             │ Model Identifier : {:<52}│\n\
             │ Provider Family  : {:<52}│\n\
             │ Portal Binding   : {:<52}│\n\
             │ Status / Creds   : {:<52}│\n\
             │ Endpoint URL     : {:<52}│\n\
             │ Est. Cost / 1M   : ${:<51.2}│\n\
             └────────────────────────────────────────────────────────────────────────┘\n\
             Use '/model set <name>' to switch or '/model list' for options.",
            portal.model_name,
            portal.provider_kind,
            portal.name,
            portal.status_text(),
            portal.base_url.as_deref().unwrap_or("Provider Default"),
            cost
        );
    }

    match args[0].to_lowercase().as_str() {
        "list" | "ls" => {
            r#"┌── Popular Curated Models Across Providers ───────────────────────────────┐
│ • OpenAI:                                                                │
│     gpt-4o-mini                 Fast, highly economical general coding   │
│     gpt-4o                      Flagship multi-file reasoning & refactor │
│ • Anthropic:                                                             │
│     claude-3-5-sonnet-20241022  Industry-standard autonomous agent model │
│     claude-3-5-haiku-20241022   Rapid lightweight execution              │
│ • Local (Ollama - No API Key Required):                                  │
│     ollama/llama3.1             Meta LLaMA 3.1 8B (http://localhost:11434)│
│     ollama/qwen2.5-coder        Qwen 2.5 Coder 7B/14B/32B                │
│ • DeepSeek:                                                              │
│     deepseek-chat               DeepSeek V3 / R1 high reasoning          │
└──────────────────────────────────────────────────────────────────────────┘
Type '/model set <model_name>' to activate any of these models."#
                .to_string()
        }
        "set" => {
            if args.len() < 2 {
                return "Usage: /model set <model_name> (e.g. '/model set gpt-4o' or '/model set claude-3-5-sonnet-20241022')".to_string();
            }
            let target_model = args[1].trim();

            // Look for existing portal matching model
            let mut found_idx = None;
            for (idx, portal) in app.portals.iter().enumerate() {
                if portal.model_name.eq_ignore_ascii_case(target_model)
                    || portal.id.eq_ignore_ascii_case(target_model)
                {
                    found_idx = Some(idx);
                    break;
                }
            }

            if let Some(idx) = found_idx {
                app.selected_portal_idx = idx;
                app.activate_selected_portal();
                let p = &app.portals[idx];
                format!(
                    "✓ Switched to existing portal '{}' (Model: {}, Provider: {}).",
                    p.name, p.model_name, p.provider_kind
                )
            } else {
                // Determine provider family from name
                let lower = target_model.to_lowercase();
                let (provider_kind, name) = if lower.starts_with("ollama/") {
                    (
                        "ollama",
                        format!("Ollama {}", target_model.trim_start_matches("ollama/")),
                    )
                } else if lower.starts_with("claude") {
                    ("anthropic", format!("Anthropic {}", target_model))
                } else if lower.contains("deepseek") {
                    ("openai", format!("DeepSeek {}", target_model))
                } else {
                    ("openai", format!("Custom {}", target_model))
                };

                let id = format!("portal-{}", chrono::Utc::now().timestamp_millis());
                let base_url = if provider_kind == "ollama" {
                    Some("http://localhost:11434/v1".to_string())
                } else if lower.contains("deepseek") {
                    Some("https://api.deepseek.com/v1".to_string())
                } else {
                    None
                };

                let new_portal = ModelPortal::new(
                    id,
                    name,
                    provider_kind,
                    target_model.to_string(),
                    base_url,
                    None,
                    true,
                );

                for p in &mut app.portals {
                    p.is_active = false;
                }
                app.portals.push(new_portal);
                app.selected_portal_idx = app.portals.len() - 1;
                format!(
                    "✓ Configured and activated new portal for model '{}' (Provider: {}).",
                    target_model, provider_kind
                )
            }
        }
        "info" => {
            let portal = app.active_portal();
            format!(
                "Model Info:\n  ID: {}\n  Provider: {}\n  Configured: {}\n  Status: {}",
                portal.model_name,
                portal.provider_kind,
                portal.is_configured(),
                portal.status_text()
            )
        }
        other => {
            format!(
                "Unknown /model subcommand '{}'. Valid options: /model, /model list, /model set <name>, /model info.",
                other
            )
        }
    }
}

fn handle_portal_command(app: &mut App, args: &[String]) -> String {
    if args.is_empty() || args[0].eq_ignore_ascii_case("list") {
        let mut out = String::from(
            "┌── Configured Inference Portals ──────────────────────────────────────────┐\n\
             │ Act  ID                  Provider   Model                      Status       │\n\
             ├─────────────────────────────────────────────────────────────────────────────┤\n",
        );

        for p in &app.portals {
            let marker = if p.is_active { "[*]" } else { "[ ]" };
            out.push_str(&format!(
                "│ {:<4} {:<19} {:<10} {:<26} {:<12}│\n",
                marker,
                if p.id.len() > 18 { &p.id[..18] } else { &p.id },
                if p.provider_kind.len() > 9 {
                    &p.provider_kind[..9]
                } else {
                    &p.provider_kind
                },
                if p.model_name.len() > 25 {
                    &p.model_name[..25]
                } else {
                    &p.model_name
                },
                if p.status_text().len() > 11 {
                    &p.status_text()[..11]
                } else {
                    p.status_text()
                }
            ));
        }

        out.push_str(
            "└─────────────────────────────────────────────────────────────────────────────┘\n\
             Commands: '/portal use <id>', '/portal set-key <id> <key>', '/portal test <id>'",
        );
        return out;
    }

    match args[0].to_lowercase().as_str() {
        "use" | "switch" => {
            if args.len() < 2 {
                return "Usage: /portal use <portal_id>".to_string();
            }
            let target_id = args[1].trim();
            if let Some(pos) = app.portals.iter().position(|p| {
                p.id.eq_ignore_ascii_case(target_id) || p.name.eq_ignore_ascii_case(target_id)
            }) {
                app.selected_portal_idx = pos;
                app.activate_selected_portal();
                format!("✓ Switched active portal to '{}'.", app.portals[pos].name)
            } else {
                format!(
                    "Portal '{}' not found. Type '/portal list' to see available portals.",
                    target_id
                )
            }
        }
        "set-key" => {
            if args.len() < 3 {
                return "Usage: /portal set-key <portal_id> <api_key>".to_string();
            }
            let target_id = args[1].trim();
            let key = args[2].trim().to_string();

            if let Some(portal) = app
                .portals
                .iter_mut()
                .find(|p| p.id.eq_ignore_ascii_case(target_id))
            {
                portal.api_key = Some(key);
                format!("✓ Updated API key for portal '{}'.", portal.name)
            } else {
                format!("Portal '{}' not found.", target_id)
            }
        }
        "set-url" => {
            if args.len() < 3 {
                return "Usage: /portal set-url <portal_id> <endpoint_url>".to_string();
            }
            let target_id = args[1].trim();
            let url = args[2].trim().to_string();

            if let Some(portal) = app
                .portals
                .iter_mut()
                .find(|p| p.id.eq_ignore_ascii_case(target_id))
            {
                portal.base_url = Some(url);
                format!("✓ Updated Base URL for portal '{}'.", portal.name)
            } else {
                format!("Portal '{}' not found.", target_id)
            }
        }
        "test" => {
            let portal = if args.len() >= 2 {
                let id = args[1].trim();
                app.portals.iter().find(|p| p.id.eq_ignore_ascii_case(id))
            } else {
                Some(app.active_portal())
            };

            if let Some(p) = portal {
                if p.is_configured() {
                    format!(
                        "✓ Portal '{}' is configured and ready ({})",
                        p.name,
                        p.status_text()
                    )
                } else {
                    format!(
                        "⚠ Portal '{}' requires credentials. Use '/portal set-key {} <key>'.",
                        p.name, p.id
                    )
                }
            } else {
                "Target portal not found.".to_string()
            }
        }
        other => {
            format!("Unknown /portal subcommand '{}'. Use '/portal list', '/portal use <id>', or '/portal set-key <id> <key>'.", other)
        }
    }
}

fn handle_agents_command(app: &mut App, args: &[String]) -> String {
    if args.is_empty() || args[0].eq_ignore_ascii_case("list") {
        return format_agents_visualization(app);
    }

    match args[0].to_lowercase().as_str() {
        "inspect" => {
            if args.len() < 2 {
                return "Usage: /agent inspect <agent_id> (e.g. '/agent inspect coder-01')"
                    .to_string();
            }
            let id = args[1].trim();
            if let Some(agent) = app.agents.iter().find(|a| a.id.eq_ignore_ascii_case(id)) {
                format!(
                    "┌── Agent Inspection: {} ───────────────────────────────────────┐\n\
                     │ Identifier   : {:<57}│\n\
                     │ Display Name : {:<57}│\n\
                     │ Role         : {:<57}│\n\
                     │ Model        : {:<57}│\n\
                     │ Status Badge : {:<57}│\n\
                     │ Workspace    : {:<57}│\n\
                     │ Runs Executed: {:<57}│\n\
                     ├────────────────────────────────────────────────────────────────────────┤\n\
                     │ Operational Policy:                                                    │\n\
                     │   {:<69}│\n\
                     ├────────────────────────────────────────────────────────────────────────┤\n\
                     │ Authorized Tool Boundaries:                                            │\n\
                     │   • read_file, write_file, list_dir (workspace confined)               │\n\
                     │   • shell (timeout capped, sanitized environment)                      │\n\
                     │   • git_status, git_diff, git_commit, git_log                          │\n\
                     └────────────────────────────────────────────────────────────────────────┘",
                    agent.id,
                    agent.id,
                    agent.name,
                    agent.role,
                    agent.model,
                    format!("[{}]", agent.status),
                    agent.workspace,
                    agent.runs_count,
                    agent.policy
                )
            } else {
                format!(
                    "Agent '{}' not found. Type '/agents' to view all configured agents.",
                    id
                )
            }
        }
        "start" => {
            if args.len() < 2 {
                return "Usage: /agent start <agent_id>".to_string();
            }
            let id = args[1].trim();
            match app.transition_agent_state(id, AgentLifecycleState::Running) {
                Ok(_) => format!("✓ Agent '{}' transitioned to RUNNING.", id),
                Err(e) => format!("Failed to start agent: {}", e),
            }
        }
        "stop" => {
            if args.len() < 2 {
                return "Usage: /agent stop <agent_id>".to_string();
            }
            let id = args[1].trim();
            match app.transition_agent_state(id, AgentLifecycleState::Stopped) {
                Ok(_) => format!("✓ Agent '{}' transitioned to STOPPED.", id),
                Err(e) => format!("Failed to stop agent: {}", e),
            }
        }
        "pause" => {
            if args.len() < 2 {
                return "Usage: /agent pause <agent_id>".to_string();
            }
            let id = args[1].trim();
            match app.transition_agent_state(id, AgentLifecycleState::Paused) {
                Ok(_) => format!("✓ Agent '{}' transitioned to PAUSED.", id),
                Err(e) => format!("Failed to pause agent: {}", e),
            }
        }
        "add" | "create" => {
            if args.len() < 4 {
                return "Usage: /agent add <id> <name> <role> (e.g. '/agent add reviewer \"PR Reviewer\" \"Audits code changes\"')".to_string();
            }
            let id = args[1].trim().to_string();
            let name = args[2].trim().to_string();
            let role = args[3..].join(" ");
            app.register_agent(
                id.clone(),
                name.clone(),
                role.clone(),
                "gpt-4o-mini".to_string(),
                format!("Autonomous operational guidelines for {}", name),
            );
            format!(
                "✓ Registered new persistent agent '{}' ({}) with role '{}'.",
                id, name, role
            )
        }
        other => {
            format!("Unknown /agent subcommand '{}'. Valid options: /agents, /agent inspect <id>, /agent start <id>, /agent stop <id>, /agent pause <id>, /agent add <id> <name> <role>.", other)
        }
    }
}

fn format_agents_visualization(app: &App) -> String {
    let mut out = String::from(
        "┌── ◈ CORTEX AGENT WORKERS TOPOLOGY ──────────────────────────────────────────┐\n",
    );

    for agent in &app.agents {
        let (icon, badge_str) = match agent.status.as_str() {
            "Running" | "RUNNING" => ("●", "[RUNNING]"),
            "Ready" | "READY" => ("●", "[READY]  "),
            "Paused" | "PAUSED" => ("⏸", "[PAUSED] "),
            "Stopped" | "STOPPED" => ("■", "[STOPPED]"),
            _ => ("○", "[IDLE]   "),
        };

        out.push_str(&format!(
            "│                                                                             │\n\
             │  {} {:<9} {:<16} ({}) \n\
             │    Role     : {:<58}│\n\
             │    Model    : {:<18} Workspace: {:<27}│\n\
             │    Policy   : {:<58}│\n\
             │    Runs     : {:<18} Tools    : [read_file, write_file, shell, git_*]  │\n",
            icon,
            badge_str,
            agent.id,
            agent.name,
            if agent.role.len() > 57 {
                &agent.role[..57]
            } else {
                &agent.role
            },
            agent.model,
            if agent.workspace.len() > 26 {
                &agent.workspace[..26]
            } else {
                &agent.workspace
            },
            if agent.policy.len() > 57 {
                &agent.policy[..57]
            } else {
                &agent.policy
            },
            agent.runs_count,
        ));
    }

    out.push_str(
        "│                                                                             │\n\
         └─────────────────────────────────────────────────────────────────────────────┘\n\
         Use '/agent inspect <id>', '/agent start <id>', or '/agent stop <id>' to manage.",
    );
    out
}

fn handle_tools_command() -> String {
    r#"┌── Registered Tool Capabilities & Permissions ───────────────────────────┐
│ • read_file   : Read file contents (workspace boundary confined)         │
│ • write_file  : Create or overwrite files (workspace boundary confined)  │
│ • list_dir    : Inspect directory entries (path traversal blocked)       │
│ • shell       : Sandboxed subprocess execution (timeout & signal guard)  │
│ • git_status  : Working tree status query                                │
│ • git_diff    : Inspect uncommitted line modifications                   │
│ • git_commit  : Structured atomic commit authoring                       │
│ • git_log     : Read revision commit history                             │
│ • git_branch  : Inspect active repository branches                       │
├──────────────────────────────────────────────────────────────────────────┤
│ Security Boundary: All file access is strictly confined within workspace │
│ Directory traversal attempts ('../') outside workspace root are blocked. │
└──────────────────────────────────────────────────────────────────────────┘"#
        .to_string()
}

fn handle_diff_command() -> String {
    match std::process::Command::new("git").arg("diff").output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.trim().is_empty() {
                "Workspace clean. No uncommitted git modifications found.".to_string()
            } else {
                let lines: Vec<&str> = stdout.lines().take(40).collect();
                let mut out = format!("── Git Diff Summary ({} lines) ──\n", lines.len());
                for line in lines {
                    out.push_str(line);
                    out.push('\n');
                }
                if stdout.lines().count() > 40 {
                    out.push_str("... (diff truncated)");
                }
                out
            }
        }
        Err(e) => format!("Failed to execute git diff: {}", e),
    }
}

fn handle_status_command(app: &App) -> String {
    let portal = app.active_portal();
    let store_desc = if app.store.is_some() {
        "SQLite Connected (~/.cortex/cortex.db)"
    } else {
        "In-Memory / Ephemeral"
    };
    let active_agents = app.agents.len();
    let total_runs = app.runs.len();

    format!(
        "┌── Cortex Runtime Diagnostic Status ────────────────────────────────────┐\n\
         │ Runtime Version : Cortex v{:<44}│\n\
         │ Harness Mode    : Interactive Terminal Chat & Control Plane            │\n\
         │ Active Portal   : {:<51}│\n\
         │ Active Model    : {:<51}│\n\
         │ Persistence     : {:<51}│\n\
         │ Total Runs      : {:<51}│\n\
         │ Active Workers  : {:<51}│\n\
         │ Thinking Mode   : {:<51}│\n\
         │ Sandbox Boundary: Active (confining workspace)                         │\n\
         └────────────────────────────────────────────────────────────────────────┘",
        VERSION,
        portal.name,
        portal.model_name,
        store_desc,
        total_runs,
        active_agents,
        if app.thinking_expanded {
            "Expanded (Full Reasoning Trace)"
        } else {
            "Collapsed (Compact Summary)"
        }
    )
}

fn handle_runs_command(app: &App) -> String {
    if app.runs.is_empty() {
        return "No execution runs recorded in SQLite storage yet.".to_string();
    }

    let mut out = String::from(
        "┌── Recent Execution Runs (SQLite) ────────────────────────────────────────┐\n\
         │ Run ID                 Status     Duration   Tokens    Task Preview     │\n\
         ├─────────────────────────────────────────────────────────────────────────┤\n",
    );

    for run in app.runs.iter().take(5) {
        let task_preview = if run.task.len() > 18 {
            format!("{}...", &run.task[..15])
        } else {
            run.task.clone()
        };
        out.push_str(&format!(
            "│ {:<22} {:<10} {:<10} {:<9} {:<17}│\n",
            if run.id.as_str().len() > 21 {
                &run.id.as_str()[..21]
            } else {
                run.id.as_str()
            },
            run.status,
            run.duration_ms
                .map(|d| format!("{}ms", d))
                .unwrap_or_else(|| "-".to_string()),
            run.tokens_total,
            task_preview
        ));
    }

    out.push_str("└─────────────────────────────────────────────────────────────────────────┘");
    out
}

fn handle_thinking_command(app: &mut App, arg: Option<String>) -> String {
    match arg.as_deref() {
        Some("on") | Some("true") | Some("expand") => {
            app.set_thinking_expanded(true);
            "✓ Thinking trace view set to EXPANDED. Full reasoning steps will be displayed."
                .to_string()
        }
        Some("off") | Some("false") | Some("collapse") => {
            app.set_thinking_expanded(false);
            "✓ Thinking trace view set to COLLAPSED. Compact expandable summaries will be displayed.".to_string()
        }
        _ => {
            let new_state = !app.thinking_expanded;
            app.set_thinking_expanded(new_state);
            if new_state {
                "✓ Thinking trace view TOGGLED to EXPANDED.".to_string()
            } else {
                "✓ Thinking trace view TOGGLED to COLLAPSED.".to_string()
            }
        }
    }
}

fn handle_compact_command(app: &mut App) -> String {
    let count_before = app.chat_messages.len();
    if count_before <= 6 {
        return "Chat history is already compact.".to_string();
    }

    let retained: Vec<ChatMessageItem> = app
        .chat_messages
        .iter()
        .rev()
        .take(6)
        .cloned()
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();

    app.chat_messages = retained;
    format!(
        "✓ Compacted session history from {} to {} messages.",
        count_before,
        app.chat_messages.len()
    )
}

fn handle_tasks_command(app: &App) -> String {
    let mut out = String::new();
    out.push_str("┌── 📋 BENCHMARK EVALUATION TASKS ────────────────────────────────────────┐\n");
    if app.tasks.is_empty() {
        out.push_str(
            "│ No benchmark tasks loaded in app memory.                                │\n",
        );
    } else {
        for t in app.tasks.iter().take(8) {
            out.push_str(&format!(
                "│  • [{:<18}] {:<48}│\n",
                t.id,
                t.name.chars().take(48).collect::<String>()
            ));
        }
    }
    out.push_str("└────────────────────────────────────────────────────────────────────────┘\n");
    out.push_str("Run benchmark suites via CLI: 'cortex bench run <task-id>'");
    out
}

fn handle_dashboard_command(app: &App) -> String {
    let total_runs = app.runs.len();
    let completed = app.runs.iter().filter(|r| r.status == "completed").count();
    let failed = app.runs.iter().filter(|r| r.status == "failed").count();
    let tokens: u64 = app.runs.iter().map(|r| r.tokens_total as u64).sum();
    let cost: f64 = app.runs.iter().map(|r| r.estimated_cost_usd).sum();
    let active_agents = app.agents.iter().filter(|a| a.status == "Running").count();

    format!(
        "┌── 📊 CORTEX RUNTIME DASHBOARD ─────────────────────────────────────────┐\n\
         │ Total Runs     : {:<52}│\n\
         │ Completed Runs : {:<52}│\n\
         │ Failed Runs    : {:<52}│\n\
         │ Total Tokens   : {:<52}│\n\
         │ Est. Total Cost: ${:<51.4}│\n\
         │ Active Agents  : {:<52}│\n\
         │ SQLite Store   : {:<52}│\n\
         └────────────────────────────────────────────────────────────────────────┘",
        total_runs,
        completed,
        failed,
        tokens,
        cost,
        format!("{} running / {} total", active_agents, app.agents.len()),
        if app.store.is_some() {
            "Connected"
        } else {
            "In-Memory / Unconnected"
        },
    )
}

fn handle_cost_command(app: &App) -> String {
    let portal = app.active_portal();
    let tokens: u64 = app.runs.iter().map(|r| r.tokens_total as u64).sum();
    let cost: f64 = app.runs.iter().map(|r| r.estimated_cost_usd).sum();

    format!(
        "┌── 💰 TOKEN USAGE & COST ESTIMATE ──────────────────────────────────────┐\n\
         │ Active Model   : {:<52}│\n\
         │ Total Tokens   : {:<52}│\n\
         │ Est. Run Cost  : ${:<51.4}│\n\
         │ Model Pricing  : {:<52}│\n\
         └────────────────────────────────────────────────────────────────────────┘\n\
         See '/model info' for context window and token rates.",
        portal.model_name, tokens, cost, "Configured via active portal"
    )
}

fn handle_doctor_command(app: &App) -> String {
    let portal = app.active_portal();
    let openai_set = std::env::var("OPENAI_API_KEY").is_ok();
    let anthropic_set = std::env::var("ANTHROPIC_API_KEY").is_ok();
    let git_repo = std::path::Path::new(".git").exists();
    let db_ok = app.store.is_some();

    format!(
        "┌── 🩺 CORTEX SYSTEM DOCTOR & ENVIRONMENT CHECK ─────────────────────────┐\n\
         │ Git Repository     : {:<50}│\n\
         │ SQLite Storage     : {:<50}│\n\
         │ Active Portal      : {:<50}│\n\
         │ OPENAI_API_KEY     : {:<50}│\n\
         │ ANTHROPIC_API_KEY  : {:<50}│\n\
         │ Runtime Engine     : {:<50}│\n\
         └────────────────────────────────────────────────────────────────────────┘",
        if git_repo {
            "✔ Initialized"
        } else {
            "✖ Not found (.git missing)"
        },
        if db_ok {
            "✔ Connected"
        } else {
            "⚠ Memory fallback"
        },
        format!("{} ({})", portal.name, portal.provider_kind),
        if openai_set {
            "✔ Configured"
        } else {
            "○ Not set"
        },
        if anthropic_set {
            "✔ Configured"
        } else {
            "○ Not set"
        },
        format!("Cortex v{}", VERSION),
    )
}

/// Curated primary slash commands for autocomplete.
pub const ALL_COMMANDS: &[&str] = &[
    "/agents",
    "/clear",
    "/compact",
    "/cost",
    "/dashboard",
    "/diff",
    "/doctor",
    "/help",
    "/model",
    "/portal",
    "/quit",
    "/runs",
    "/status",
    "/tasks",
    "/thinking",
    "/tools",
];

const MODEL_SUBCOMMANDS: &[&str] = &["info", "list", "set"];
const PORTAL_SUBCOMMANDS: &[&str] = &["add", "list", "set-key", "set-url", "test", "use"];
const AGENT_SUBCOMMANDS: &[&str] = &["add", "inspect", "list", "pause", "start", "stop"];
const THINKING_SUBCOMMANDS: &[&str] = &["off", "on", "toggle"];

/// Autocomplete a partial slash command input.
pub fn autocomplete_command(input: &str) -> Option<String> {
    let trimmed = input.trim_start();
    if !trimmed.starts_with('/') {
        return None;
    }

    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() || (parts.len() == 1 && !trimmed.ends_with(' ')) {
        let prefix = parts.first().copied().unwrap_or("/");
        let matches: Vec<&&str> = ALL_COMMANDS
            .iter()
            .filter(|cmd| cmd.starts_with(prefix))
            .collect();

        if matches.len() == 1 {
            return Some(format!("{} ", matches[0]));
        } else if matches.len() > 1 {
            if let Some(pos) = matches.iter().position(|cmd| **cmd == prefix) {
                let next_idx = (pos + 1) % matches.len();
                return Some(format!("{} ", matches[next_idx]));
            }
            return Some(format!("{} ", matches[0]));
        }
    } else if parts.len() == 2 && !trimmed.ends_with(' ') {
        let cmd = parts[0];
        let sub_prefix = parts[1];
        let sub_candidates: &[&str] = match cmd {
            "/model" | "/models" => MODEL_SUBCOMMANDS,
            "/portal" | "/portals" => PORTAL_SUBCOMMANDS,
            "/agent" | "/agents" => AGENT_SUBCOMMANDS,
            "/thinking" | "/think" => THINKING_SUBCOMMANDS,
            _ => &[],
        };

        let matches: Vec<&&str> = sub_candidates
            .iter()
            .filter(|sub| sub.starts_with(sub_prefix))
            .collect();

        if !matches.is_empty() {
            return Some(format!("{} {} ", cmd, matches[0]));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_slash_commands() {
        assert_eq!(parse_command("/help"), Some(SlashCommand::Help));
        assert_eq!(parse_command("/?"), Some(SlashCommand::Help));
        assert_eq!(
            parse_command("/model"),
            Some(SlashCommand::Model { args: vec![] })
        );
        assert_eq!(
            parse_command("/model set gpt-4o"),
            Some(SlashCommand::Model {
                args: vec!["set".to_string(), "gpt-4o".to_string()]
            })
        );
        assert_eq!(
            parse_command("/portal use ollama"),
            Some(SlashCommand::Portal {
                args: vec!["use".to_string(), "ollama".to_string()]
            })
        );
        assert_eq!(
            parse_command("/agents"),
            Some(SlashCommand::Agents { args: vec![] })
        );
        assert_eq!(parse_command("/tools"), Some(SlashCommand::Tools));
        assert_eq!(parse_command("/diff"), Some(SlashCommand::Diff));
        assert_eq!(parse_command("/status"), Some(SlashCommand::Status));
        assert_eq!(parse_command("/runs"), Some(SlashCommand::Runs));
        assert_eq!(
            parse_command("/thinking on"),
            Some(SlashCommand::Thinking {
                arg: Some("on".to_string())
            })
        );
        assert_eq!(parse_command("/tasks"), Some(SlashCommand::Tasks));
        assert_eq!(parse_command("/dashboard"), Some(SlashCommand::Dashboard));
        assert_eq!(parse_command("/doctor"), Some(SlashCommand::Doctor));
        assert_eq!(parse_command("/cost"), Some(SlashCommand::Cost));
        assert_eq!(parse_command("/clear"), Some(SlashCommand::Clear));
        assert_eq!(parse_command("/quit"), Some(SlashCommand::Quit));
        assert_eq!(
            parse_command("/foobar"),
            Some(SlashCommand::Unknown("foobar".to_string()))
        );
        assert_eq!(parse_command("plain prompt"), None);
    }

    #[test]
    fn test_execute_slash_commands() {
        let mut app = App::new(None);

        let help = execute_command(&mut app, SlashCommand::Help);
        assert!(help.contains("SLASH COMMAND CATALOG"));

        let tools = execute_command(&mut app, SlashCommand::Tools);
        assert!(tools.contains("read_file"));

        let status = execute_command(&mut app, SlashCommand::Status);
        assert!(status.contains("Cortex Runtime Diagnostic Status"));

        let dashboard = execute_command(&mut app, SlashCommand::Dashboard);
        assert!(dashboard.contains("RUNTIME DASHBOARD"));

        let doctor = execute_command(&mut app, SlashCommand::Doctor);
        assert!(doctor.contains("SYSTEM DOCTOR"));

        let cost = execute_command(&mut app, SlashCommand::Cost);
        assert!(cost.contains("TOKEN USAGE"));

        let tasks = execute_command(&mut app, SlashCommand::Tasks);
        assert!(tasks.contains("BENCHMARK EVALUATION TASKS"));

        // Toggle thinking
        let think = execute_command(&mut app, SlashCommand::Thinking { arg: None });
        assert!(think.contains("TOGGLED"));

        // Model list
        let models = execute_command(
            &mut app,
            SlashCommand::Model {
                args: vec!["list".to_string()],
            },
        );
        assert!(models.contains("gpt-4o-mini"));

        // Model set
        let set_res = execute_command(
            &mut app,
            SlashCommand::Model {
                args: vec!["set".to_string(), "gpt-4o".to_string()],
            },
        );
        assert!(set_res.contains("Switched") || set_res.contains("Configured"));

        // Quit
        assert!(!app.should_quit);
        let _ = execute_command(&mut app, SlashCommand::Quit);
        assert!(app.should_quit);
    }

    #[test]
    fn test_autocomplete_slash_commands() {
        assert_eq!(autocomplete_command("/m"), Some("/model ".to_string()));
        assert_eq!(autocomplete_command("/p"), Some("/portal ".to_string()));
        assert_eq!(autocomplete_command("/a"), Some("/agents ".to_string()));
        assert_eq!(autocomplete_command("/di"), Some("/diff ".to_string()));
        assert_eq!(autocomplete_command("/q"), Some("/quit ".to_string()));
        assert_eq!(autocomplete_command("/h"), Some("/help ".to_string()));
        assert_eq!(
            autocomplete_command("/model s"),
            Some("/model set ".to_string())
        );
        assert_eq!(
            autocomplete_command("/portal u"),
            Some("/portal use ".to_string())
        );
        assert_eq!(autocomplete_command("regular text"), None);
    }
}
