//! Cortex CLI entrypoint.
//!
//! Provides the primary command-line interface for the Cortex runtime harness,
//! including execution run inspection and tracing queries.

use clap::{Parser, Subcommand};
use cortex_core::{JobId, RunId, VERSION};
use cortex_runtime::{
    create_model_provider, scheduler::OverlapPolicy, tools, AgentContext, AgentLoop, CortexConfig,
    McpManager, RunStore, RunSummary, SchedulerEngine, ToolRegistry, Workspace,
};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

/// Cortex - An open-source runtime and harness for autonomous AI workers.
#[derive(Parser, Debug)]
#[command(
    name = "cortex",
    author = "Cortex Contributors",
    version = VERSION,
    about = "An open-source runtime and harness for autonomous AI workers",
    long_about = "Cortex is a runtime and harness for autonomous AI workers, providing sandboxed \
                  tool execution, persistent agents, execution tracing, and multi-agent coordination."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Inspect the current runtime bootstrap status.
    Status,

    /// Check system environment readiness.
    Check,

    /// Run an agent task.
    Run {
        /// Task prompt or instructions for the agent.
        prompt: String,

        /// Model identifier (e.g., gpt-4o-mini, gpt-4o, claude-3-5-sonnet-20241022, deepseek-chat, or ollama/llama3.1).
        #[arg(short, long, env = "CORTEX_MODEL", default_value = "gpt-4o-mini")]
        model: String,

        /// Provider API key (or set OPENAI_API_KEY / ANTHROPIC_API_KEY).
        #[arg(long, env = "CORTEX_API_KEY")]
        api_key: Option<String>,

        /// Custom provider base URL (or set OPENAI_BASE_URL / ANTHROPIC_BASE_URL).
        #[arg(long, env = "CORTEX_BASE_URL")]
        base_url: Option<String>,

        /// Working directory boundary for the agent. Defaults to current directory.
        #[arg(short, long)]
        workspace: Option<PathBuf>,

        /// Maximum autonomous loop iterations allowed.
        #[arg(short = 'i', long, default_value = "15")]
        max_iterations: usize,

        /// Path to SQLite runs database. Defaults to ~/.cortex/cortex.db.
        #[arg(long)]
        db: Option<PathBuf>,

        /// Suppress interactive progress and only output final answer.
        #[arg(short, long)]
        quiet: bool,

        /// Output the final run result in JSON format.
        #[arg(long)]
        json: bool,

        /// Path to cortex.toml configuration file.
        #[arg(short, long)]
        config: Option<PathBuf>,
    },

    /// Manage Model Context Protocol (MCP) servers and external tools.
    Mcp {
        #[command(subcommand)]
        action: McpCommands,
    },

    /// Manage and inspect recorded execution runs.
    Runs {
        #[command(subcommand)]
        action: Option<RunsCommands>,
    },

    /// Benchmark and evaluate agent performance against verifiable ground truth tasks.
    Bench {
        #[command(subcommand)]
        action: BenchCommands,
    },

    /// Launch the interactive terminal control plane (Claude Code / Codex CLI harness).
    #[command(alias = "dashboard", alias = "chat")]
    Tui {
        /// Optional path to SQLite database.
        #[arg(short, long)]
        db: Option<PathBuf>,
    },

    /// Manage scheduled cron jobs and persistent triggers.
    Cron {
        #[command(subcommand)]
        action: CronCommands,
    },
}

#[derive(Subcommand, Debug)]
enum McpCommands {
    /// List configured MCP servers and discover available tools.
    List {
        /// Optional path to cortex.toml configuration file.
        #[arg(short, long)]
        config: Option<PathBuf>,
    },

    /// Test connection to an MCP server and query its capabilities.
    Test {
        /// Server name from cortex.toml to test.
        server: String,

        /// Optional path to cortex.toml configuration file.
        #[arg(short, long)]
        config: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
enum RunsCommands {
    /// List recorded execution runs.
    List {
        /// Maximum number of runs to display.
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    /// Inspect details and event trace for a specific execution run.
    Show {
        /// Run identifier to inspect.
        run_id: String,

        /// Display verbose JSON payloads for each event.
        #[arg(short, long)]
        verbose: bool,
    },

    /// Replay a recorded execution run deterministically.
    Replay {
        /// Run identifier to replay.
        run_id: String,
    },
}

#[derive(Subcommand, Debug)]
enum BenchCommands {
    /// Execute a benchmark evaluation suite.
    Run {
        /// Target benchmark suite to evaluate (e.g., "coding", "refactor", "cli").
        #[arg(short, long, default_value = "coding")]
        suite: String,

        /// Output results in JSON format to stdout.
        #[arg(long)]
        json: bool,

        /// Write Markdown evaluation report to the specified file path.
        #[arg(short, long)]
        report: Option<PathBuf>,

        /// Maximum agent iterations allowed per task.
        #[arg(short, long, default_value = "10")]
        max_iterations: usize,
    },
}

#[derive(Subcommand, Debug)]
enum CronCommands {
    /// List all registered scheduled cron jobs.
    List {
        /// Optional path to SQLite database. Defaults to ~/.cortex/cortex.db.
        #[arg(long)]
        db: Option<PathBuf>,

        /// Output registered jobs in JSON format.
        #[arg(long)]
        json: bool,
    },

    /// Create and register a new scheduled cron job.
    Create {
        /// Human-readable name for the job.
        #[arg(short, long)]
        name: Option<String>,

        /// 5-field cron expression (e.g., "*/15 * * * *") or one-shot ISO timestamp.
        #[arg(short, long)]
        schedule: String,

        /// Task prompt to execute when triggered.
        #[arg(short, long, alias = "task")]
        prompt: String,

        /// Target agent name (e.g., coding, monitor).
        #[arg(short, long)]
        agent: Option<String>,

        /// Overlap policy when previous run is active: skip, queue, or replace.
        #[arg(short, long, default_value = "skip")]
        overlap: String,

        /// Optional path to SQLite database. Defaults to ~/.cortex/cortex.db.
        #[arg(long)]
        db: Option<PathBuf>,
    },

    /// Delete a registered scheduled cron job by its identifier.
    Delete {
        /// Identifier of the job to delete.
        id: String,

        /// Optional path to SQLite database. Defaults to ~/.cortex/cortex.db.
        #[arg(long)]
        db: Option<PathBuf>,
    },
}

fn default_db_path() -> PathBuf {
    if let Ok(env_path) = std::env::var("CORTEX_DB_PATH") {
        return PathBuf::from(env_path);
    }
    cortex_core::settings::cortex_home_dir().join("cortex.db")
}

fn list_runs(store: &RunStore, limit: usize) -> Result<(), Box<dyn std::error::Error>> {
    let runs = store.list_runs(limit)?;
    if runs.is_empty() {
        println!("No recorded execution runs found.");
        return Ok(());
    }

    println!(
        "{:<28} {:<12} {:<24} {:<10} TASK",
        "RUN ID", "STATUS", "STARTED", "DURATION"
    );
    println!("{:-<90}", "");

    for run in runs {
        let duration_str = run
            .duration_ms
            .map(|d| format!("{}ms", d))
            .unwrap_or_else(|| "-".to_string());

        let task_preview = if run.task.len() > 30 {
            format!("{}...", &run.task[..27])
        } else {
            run.task.clone()
        };

        println!(
            "{:<28} {:<12} {:<24} {:<10} {}",
            run.id.as_str(),
            run.status,
            run.started_at,
            duration_str,
            task_preview
        );
    }

    Ok(())
}

fn show_run(
    store: &RunStore,
    run_id_str: &str,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let run_id = RunId::from(run_id_str);
    let maybe_run = store.get_run(&run_id)?;

    let run: RunSummary = match maybe_run {
        Some(r) => r,
        None => {
            eprintln!("Error: Run '{}' not found.", run_id_str);
            std::process::exit(1);
        }
    };

    println!("Run:        {}", run.id);
    println!("Status:     {}", run.status);
    println!("Started:    {}", run.started_at);
    if let Some(fin) = &run.finished_at {
        println!("Finished:   {}", fin);
    }
    if let Some(dur) = run.duration_ms {
        println!("Duration:   {} ms", dur);
    }
    println!(
        "Tokens:     {} prompt / {} completion ({} total)",
        run.tokens_prompt, run.tokens_completion, run.tokens_total
    );
    println!("Cost (USD): ${:.6}", run.estimated_cost_usd);
    if let Some(err) = &run.error {
        println!("Error:      {}", err);
    }
    println!("Task:       {}", run.task);
    println!();

    let events = store.get_events(&run_id)?;
    println!("Event Trace ({} events):", events.len());

    for record in &events {
        println!(
            "  [{}] {} {:<14}",
            record.sequence,
            record.timestamp,
            record.event.event_type()
        );
        if verbose {
            let json_str = serde_json::to_string_pretty(&record.event).unwrap_or_default();
            for line in json_str.lines() {
                println!("      {}", line);
            }
        }
    }

    Ok(())
}

fn replay_run(store: &RunStore, run_id_str: &str) -> Result<(), Box<dyn std::error::Error>> {
    let run_id = RunId::from(run_id_str);
    let run = match store.get_run(&run_id)? {
        Some(r) => r,
        None => {
            eprintln!("Error: Run '{}' not found.", run_id_str);
            std::process::exit(1);
        }
    };

    println!("Replaying run:   {}", run.id);
    println!("Original Task:   {}", run.task);
    println!("Original Status: {}", run.status);

    let replay_provider = cortex_runtime::ReplayModelProvider::from_store(store, &run_id)?;
    let mut replay_context = cortex_runtime::AgentContext::new(&run.task);
    let registry = cortex_runtime::ToolRegistry::new();
    let agent = cortex_runtime::AgentLoop::new(10);

    let res = agent.run(&mut replay_context, &replay_provider, &registry)?;
    println!("\nReplay Result:");
    println!("Status:       completed");
    println!("Iterations:   {}", res.iterations);
    println!("Final Answer: {}", res.final_answer);

    Ok(())
}

fn run_bench(
    suite: &str,
    json: bool,
    report: Option<PathBuf>,
    max_iterations: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let runner = cortex_harness::BenchmarkRunner::new(max_iterations);
    let model = cortex_harness::BenchmarkBaselineProvider;

    let metrics = runner.run_suite(suite, &model)?;

    if json {
        println!("{}", metrics.to_json());
    } else {
        println!("{}", metrics.to_markdown());
    }

    if let Some(report_path) = report {
        if let Some(parent) = report_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&report_path, metrics.to_markdown())?;
        println!("Report saved to: {}", report_path.display());
    }

    if metrics.successful_tasks < metrics.total_tasks {
        std::process::exit(1);
    }

    Ok(())
}

fn handle_cron_list(
    engine: &SchedulerEngine,
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let jobs = engine.list_jobs()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&jobs)?);
        return Ok(());
    }

    if jobs.is_empty() {
        println!("No scheduled cron jobs found. Create one with 'cortex cron create'.");
        return Ok(());
    }

    println!(
        "{:<28} {:<20} {:<16} {:<9} {:<10} {:<24} LAST RUN",
        "JOB ID", "NAME", "SCHEDULE", "OVERLAP", "STATUS", "NEXT RUN"
    );
    println!("{:-<110}", "");

    for job in jobs {
        let next_str = job
            .next_run_at
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_else(|| "-".to_string());
        let last_str = job
            .last_run_at
            .map(|dt| dt.to_rfc3339())
            .unwrap_or_else(|| "-".to_string());

        let name_display = if job.name.len() > 18 {
            format!("{}...", &job.name[..15])
        } else {
            job.name.clone()
        };

        println!(
            "{:<28} {:<20} {:<16} {:<9} {:<10} {:<24} {}",
            job.id.as_str(),
            name_display,
            job.schedule,
            job.overlap_policy.as_str(),
            job.status.as_str(),
            next_str,
            last_str,
        );
    }

    Ok(())
}

fn handle_cron_create(
    engine: &SchedulerEngine,
    name: Option<String>,
    schedule: &str,
    prompt: &str,
    overlap_str: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let policy = OverlapPolicy::from_str(overlap_str)?;
    let job_name = name.unwrap_or_else(|| {
        if prompt.len() > 24 {
            format!("{}...", &prompt[..21])
        } else {
            prompt.to_string()
        }
    });

    let job = engine.register_job(job_name, schedule, prompt, policy)?;
    let next_run = job
        .next_run_at
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_else(|| "none".to_string());

    println!("Created cron job '{}' ({}).", job.id, job.name);
    println!("  Schedule: {}", job.schedule);
    println!("  Overlap:  {}", job.overlap_policy);
    println!("  Next Run: {}", next_run);
    println!("  Prompt:   {}", job.prompt);

    Ok(())
}

fn handle_cron_delete(
    engine: &SchedulerEngine,
    id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let job_id = JobId::from(id);
    let deleted = engine.delete_job(&job_id)?;
    if deleted {
        println!("Deleted cron job '{}'.", id);
        Ok(())
    } else {
        eprintln!("Error: Cron job '{}' not found.", id);
        std::process::exit(1);
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_run(
    prompt: &str,
    model: &str,
    api_key: Option<String>,
    base_url: Option<String>,
    workspace: Option<PathBuf>,
    max_iterations: usize,
    db: Option<PathBuf>,
    quiet: bool,
    json: bool,
    config: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let ws_path = match workspace {
        Some(p) => p,
        None => std::env::current_dir()?,
    };

    let ws = Arc::new(Workspace::new(&ws_path)?);

    // Build tool registry with standard agent tools (filesystem, search, shell, git)
    let registry = ToolRegistry::new();
    tools::register_standard_tools(&registry, ws.clone())?;

    // Load MCP configuration and register external tools if available
    let config_file = config.or_else(|| {
        let ws_config = ws_path.join("cortex.toml");
        if ws_config.is_file() {
            Some(ws_config)
        } else {
            None
        }
    });

    if let Some(cfg_path) = config_file {
        if cfg_path.is_file() {
            match CortexConfig::from_file(&cfg_path) {
                Ok(cfg) => match McpManager::start(&cfg) {
                    Ok(manager) => match manager.register_all(&registry) {
                        Ok(count) => {
                            if !quiet && !json && count > 0 {
                                println!("Loaded {} external tools from MCP servers", count);
                            }
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to register MCP tools: {}", e);
                        }
                    },
                    Err(e) => {
                        eprintln!("Warning: Failed to initialize MCP servers: {}", e);
                    }
                },
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to load configuration '{}': {}",
                        cfg_path.display(),
                        e
                    );
                }
            }
        }
    }

    // Instantiate model provider
    let provider = create_model_provider(model, api_key, base_url)?;
    if !provider.is_configured()? {
        eprintln!(
            "Error: Model provider '{}' for model '{}' is not configured.",
            provider.descriptor().provider,
            model
        );
        eprintln!("\nTo configure authentication for this model provider:");
        eprintln!("  • Via settings:    Configure 'api_key' in ~/.cortex/settings.json");
        eprintln!("  • Via environment: export OPENAI_API_KEY=\"sk-...\"       # OpenAI / DeepSeek / Groq");
        eprintln!(
            "                     export ANTHROPIC_API_KEY=\"sk-ant-...\" # Anthropic Claude"
        );
        eprintln!(
            "  • Via CLI flag:    cortex run \"{}\" --api-key \"...\"",
            prompt
        );
        eprintln!(
            "  • For local Ollama: cortex run \"{}\" --model ollama/llama3.1",
            prompt
        );
        std::process::exit(1);
    }

    let db_path = db.unwrap_or_else(default_db_path);
    let store = Arc::new(RunStore::open(&db_path)?);

    let mut context = AgentContext::new(prompt).with_workspace(ws.clone());
    let run_id = context.run_id.clone();

    if !quiet && !json {
        println!("{:=<80}", "");
        println!("Cortex Autonomous Agent Execution");
        println!("Run ID:     {}", run_id);
        println!("Model:      {} ({})", model, provider.descriptor().provider);
        println!("Workspace:  {}", ws.root().display());
        println!("Task:       {}", prompt);
        println!("{:-<80}", "");
    }

    let agent = AgentLoop::new(max_iterations).with_store(store);
    let result = agent.run(&mut context, provider.as_ref(), &registry)?;

    if json {
        let json_output = serde_json::json!({
            "run_id": result.run_id.as_str(),
            "task": prompt,
            "status": if result.completed { "completed" } else { "max_iterations_reached" },
            "final_answer": result.final_answer,
            "iterations": result.iterations,
            "duration_ms": result.duration_ms,
            "tokens": {
                "prompt": result.tokens_prompt,
                "completion": result.tokens_completion,
                "total": result.tokens_total,
            },
            "estimated_cost_usd": result.estimated_cost_usd,
        });
        println!("{}", serde_json::to_string_pretty(&json_output)?);
    } else {
        if !quiet {
            println!("\n[Final Answer]");
        }
        println!("{}", result.final_answer);
        if !quiet {
            println!("\n{:-<80}", "");
            println!("Execution Summary:");
            println!(
                "  Status:           {}",
                if result.completed {
                    "completed"
                } else {
                    "max iterations reached"
                }
            );
            println!("  Iterations:       {}", result.iterations);
            println!("  Duration:         {} ms", result.duration_ms);
            println!("  Tokens (prompt):  {}", result.tokens_prompt);
            println!("  Tokens (compl):   {}", result.tokens_completion);
            println!("  Tokens (total):   {}", result.tokens_total);
            println!("  Estimated Cost:   ${:.6}", result.estimated_cost_usd);
            println!(
                "  Inspect Trace:    cortex runs show {} --verbose",
                result.run_id
            );
            println!("{:=<80}", "");
        }
    }

    Ok(())
}

fn load_config_or_exit(config_path: Option<PathBuf>) -> CortexConfig {
    let resolved = match config_path {
        Some(p) => {
            if !p.is_file() {
                eprintln!("Error: Configuration file '{}' not found.", p.display());
                std::process::exit(1);
            }
            p
        }
        None => match CortexConfig::find_and_load(None) {
            Ok(Some((path, cfg))) => {
                println!("Using configuration file: {}", path.display());
                return cfg;
            }
            Ok(None) => {
                eprintln!("Error: No cortex.toml configuration file found.");
                eprintln!("Create cortex.toml in your workspace or specify --config <path>.");
                std::process::exit(1);
            }
            Err(e) => {
                eprintln!("Error searching for cortex.toml: {}", e);
                std::process::exit(1);
            }
        },
    };

    match CortexConfig::from_file(&resolved) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Error parsing '{}': {}", resolved.display(), e);
            std::process::exit(1);
        }
    }
}

fn mcp_list(config_path: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config_or_exit(config_path);
    let servers = config.servers();

    if servers.is_empty() {
        println!("No MCP servers configured in cortex.toml.");
        return Ok(());
    }

    println!("Configured MCP Servers ({})", servers.len());
    println!("{}", "-".repeat(50));

    for (name, srv) in servers {
        let status = if srv.disabled { "disabled" } else { "enabled" };
        let transport_desc = if let Some(cmd) = &srv.command {
            format!("stdio: {} {}", cmd, srv.args.join(" "))
        } else if let Some(url) = &srv.url {
            format!("sse: {}", url)
        } else {
            "unspecified".to_string()
        };

        let prefix_desc = srv.prefix.as_deref().unwrap_or(&name);
        println!("Server: {} [{}]", name, status);
        println!("  Transport: {}", transport_desc);
        println!("  Prefix:    {}", prefix_desc);

        if srv.disabled {
            println!();
            continue;
        }

        let client_res = if let Some(cmd) = &srv.command {
            cortex_runtime::mcp::McpClient::connect_stdio(cmd, &srv.args, &srv.env)
        } else if let Some(url) = &srv.url {
            cortex_runtime::mcp::McpClient::connect_sse(url)
        } else {
            continue;
        };

        match client_res {
            Ok(client) => match client.initialize() {
                Ok(init) => match client.list_tools() {
                    Ok(tools) => {
                        println!(
                            "  Discovered Tools ({}) [Server v{}]:",
                            tools.len(),
                            init.server_info.version
                        );
                        for t in tools {
                            let desc = t.description.as_deref().unwrap_or("no description");
                            println!("    - {}_{}: {}", prefix_desc, t.name, desc);
                        }
                    }
                    Err(e) => println!("  Failed to list tools: {}", e),
                },
                Err(e) => println!("  Handshake failed: {}", e),
            },
            Err(e) => println!("  Failed to connect: {}", e),
        }
        println!();
    }

    Ok(())
}

fn mcp_test(
    server_name: &str,
    config_path: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config_or_exit(config_path);
    let servers = config.servers();

    let srv = match servers.get(server_name) {
        Some(s) => s,
        None => {
            let available: Vec<String> = servers.keys().cloned().collect();
            eprintln!(
                "Error: Server '{}' not found in configuration.",
                server_name
            );
            eprintln!("Available servers: {}", available.join(", "));
            std::process::exit(1);
        }
    };

    println!("Testing MCP Server '{}'...", server_name);
    let client = if let Some(cmd) = &srv.command {
        println!("Spawning stdio process: {} {}", cmd, srv.args.join(" "));
        cortex_runtime::mcp::McpClient::connect_stdio(cmd, &srv.args, &srv.env)?
    } else if let Some(url) = &srv.url {
        println!("Connecting via SSE: {}", url);
        cortex_runtime::mcp::McpClient::connect_sse(url)?
    } else {
        eprintln!(
            "Error: Server '{}' has neither command nor url configured.",
            server_name
        );
        std::process::exit(1);
    };

    print!("Performing handshake (initialize)... ");
    let init = client.initialize()?;
    println!("OK");
    println!("  Server Name:    {}", init.server_info.name);
    println!("  Server Version: {}", init.server_info.version);
    println!("  Protocol:       {}", init.protocol_version);
    if let Some(instructions) = init.instructions {
        println!("  Instructions:   {}", instructions);
    }

    print!("Querying tools (tools/list)... ");
    match client.list_tools() {
        Ok(tools) => {
            println!("OK ({} tools discovered)", tools.len());
            for t in tools {
                let desc = t.description.as_deref().unwrap_or("no description");
                println!("  - {}: {}", t.name, desc);
            }
        }
        Err(e) => println!("Failed: {}", e),
    }

    print!("Querying resources (resources/list)... ");
    match client.list_resources() {
        Ok(resources) => {
            println!("OK ({} resources discovered)", resources.len());
            for r in resources {
                println!("  - {} ({})", r.name, r.uri);
            }
        }
        Err(e) => println!("Failed: {}", e),
    }

    print!("Querying prompts (prompts/list)... ");
    match client.list_prompts() {
        Ok(prompts) => {
            println!("OK ({} prompts discovered)", prompts.len());
            for p in prompts {
                let desc = p.description.as_deref().unwrap_or("no description");
                println!("  - {}: {}", p.name, desc);
            }
        }
        Err(e) => println!("Failed: {}", e),
    }

    let _ = client.close();
    println!("\nServer test completed successfully.");
    Ok(())
}

fn main() {
    let settings = cortex_core::settings::UserSettings::load_or_create().unwrap_or_default();
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Status) => {
            println!("Cortex Agent Runtime v{}", VERSION);
            println!("Status: Workspace & Architecture Bootstrap");
            let settings_file = cortex_core::settings::settings_path();
            println!(
                "Settings: {} (model: {})",
                settings_file.display(),
                settings.model
            );
            if let Some(url) = &settings.base_url {
                println!("Base URL: {}", url);
            }
            let key_status = if settings.api_key.is_some()
                || settings.openai_api_key.is_some()
                || settings.anthropic_api_key.is_some()
            {
                "Configured"
            } else {
                "Not set"
            };
            println!("API Key: {}", key_status);
            println!("Core Interfaces: Loaded (cortex-core, cortex-runtime)");
            println!("Database: {}", default_db_path().display());
            println!("Planned Features: See docs/roadmap.md for upcoming milestones");
        }
        Some(Commands::Check) => {
            println!("Cortex v{} environment check:", VERSION);
            println!("  [✓] Workspace crates initialized");
            println!("  [✓] Architecture traits defined");
            let settings_file = cortex_core::settings::settings_path();
            if settings_file.is_file() {
                println!(
                    "  [✓] User settings loaded from {}",
                    settings_file.display()
                );
            } else {
                println!("  [!] User settings missing at {}", settings_file.display());
            }
            println!("  [✓] Ready for runtime development");
        }
        Some(Commands::Run {
            prompt,
            model,
            api_key,
            base_url,
            workspace,
            max_iterations,
            db,
            quiet,
            json,
            config,
        }) => {
            let effective_model = if model == "gpt-4o-mini" && settings.model != "gpt-4o-mini" {
                settings.model.clone()
            } else {
                model
            };
            let effective_api_key = api_key.or_else(|| settings.resolve_api_key(&effective_model));
            let effective_base_url =
                base_url.or_else(|| settings.resolve_base_url(&effective_model));
            let effective_max_iter = if max_iterations == 15 && settings.max_iterations != 15 {
                settings.max_iterations
            } else {
                max_iterations
            };
            if let Err(e) = execute_run(
                &prompt,
                &effective_model,
                effective_api_key,
                effective_base_url,
                workspace,
                effective_max_iter,
                db,
                quiet,
                json,
                config,
            ) {
                eprintln!("Error executing agent task: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Mcp { action }) => match action {
            McpCommands::List { config } => {
                if let Err(e) = mcp_list(config) {
                    eprintln!("Error listing MCP servers: {}", e);
                    std::process::exit(1);
                }
            }
            McpCommands::Test { server, config } => {
                if let Err(e) = mcp_test(&server, config) {
                    eprintln!("Error testing MCP server '{}': {}", server, e);
                    std::process::exit(1);
                }
            }
        },
        Some(Commands::Runs { action }) => {
            let db_path = default_db_path();
            let store = match RunStore::open(&db_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!(
                        "Error: Failed to open run database at '{}': {}",
                        db_path.display(),
                        e
                    );
                    std::process::exit(1);
                }
            };

            match action.unwrap_or(RunsCommands::List { limit: 20 }) {
                RunsCommands::List { limit } => {
                    if let Err(e) = list_runs(&store, limit) {
                        eprintln!("Error listing runs: {}", e);
                        std::process::exit(1);
                    }
                }
                RunsCommands::Show { run_id, verbose } => {
                    if let Err(e) = show_run(&store, &run_id, verbose) {
                        eprintln!("Error displaying run: {}", e);
                        std::process::exit(1);
                    }
                }
                RunsCommands::Replay { run_id } => {
                    if let Err(e) = replay_run(&store, &run_id) {
                        eprintln!("Error replaying run: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
        Some(Commands::Bench { action }) => match action {
            BenchCommands::Run {
                suite,
                json,
                report,
                max_iterations,
            } => {
                if let Err(e) = run_bench(&suite, json, report, max_iterations) {
                    eprintln!("Error executing benchmark suite: {}", e);
                    std::process::exit(1);
                }
            }
        },
        Some(Commands::Tui { db }) => {
            let db_path = db.unwrap_or_else(default_db_path);
            if let Err(e) = cortex_tui::run_tui(&db_path) {
                eprintln!("Error running TUI control plane: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Cron { action }) => match action {
            CronCommands::List { db, json } => {
                let db_path = db.unwrap_or_else(default_db_path);
                let store = Arc::new(match RunStore::open(&db_path) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Error opening database: {}", e);
                        std::process::exit(1);
                    }
                });
                let engine = SchedulerEngine::new(store);
                if let Err(e) = handle_cron_list(&engine, json) {
                    eprintln!("Error listing cron jobs: {}", e);
                    std::process::exit(1);
                }
            }
            CronCommands::Create {
                name,
                schedule,
                prompt,
                agent: _,
                overlap,
                db,
            } => {
                let db_path = db.unwrap_or_else(default_db_path);
                let store = Arc::new(match RunStore::open(&db_path) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Error opening database: {}", e);
                        std::process::exit(1);
                    }
                });
                let engine = SchedulerEngine::new(store);
                if let Err(e) = handle_cron_create(&engine, name, &schedule, &prompt, &overlap) {
                    eprintln!("Error creating cron job: {}", e);
                    std::process::exit(1);
                }
            }
            CronCommands::Delete { id, db } => {
                let db_path = db.unwrap_or_else(default_db_path);
                let store = Arc::new(match RunStore::open(&db_path) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("Error opening database: {}", e);
                        std::process::exit(1);
                    }
                });
                let engine = SchedulerEngine::new(store);
                if let Err(e) = handle_cron_delete(&engine, &id) {
                    eprintln!("Error deleting cron job: {}", e);
                    std::process::exit(1);
                }
            }
        },
        None => {
            use std::io::IsTerminal;
            if std::io::stdin().is_terminal() {
                let db_path = default_db_path();
                if let Err(e) = cortex_tui::run_tui(&db_path) {
                    eprintln!("Error running interactive agent: {}", e);
                    std::process::exit(1);
                }
            } else {
                println!("Cortex Agent Runtime v{}", VERSION);
                println!("Run 'cortex --help' or 'agent --help' for usage instructions.");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing_status() {
        let args = vec!["cortex", "status"];
        let parsed = Cli::try_parse_from(args);
        assert!(parsed.is_ok());
    }

    #[test]
    fn test_cli_parsing_check() {
        let args = vec!["cortex", "check"];
        let parsed = Cli::try_parse_from(args);
        assert!(parsed.is_ok());
    }

    #[test]
    fn test_cli_parsing_runs_subcommands() {
        let args = vec!["cortex", "runs"];
        let parsed = Cli::try_parse_from(args).unwrap();
        match parsed.command {
            Some(Commands::Runs { action }) => assert!(action.is_none()),
            _ => panic!("unexpected command parsed"),
        }

        let args = vec!["cortex", "runs", "list", "--limit", "10"];
        let parsed = Cli::try_parse_from(args).unwrap();
        match parsed.command {
            Some(Commands::Runs {
                action: Some(RunsCommands::List { limit }),
            }) => assert_eq!(limit, 10),
            _ => panic!("unexpected command parsed"),
        }

        let args = vec!["cortex", "runs", "show", "run_123", "--verbose"];
        let parsed = Cli::try_parse_from(args).unwrap();
        match parsed.command {
            Some(Commands::Runs {
                action:
                    Some(RunsCommands::Show {
                        run_id,
                        verbose: true,
                    }),
            }) => assert_eq!(run_id, "run_123"),
            _ => panic!("unexpected command parsed"),
        }
    }

    #[test]
    fn test_default_db_path() {
        let path = default_db_path();
        assert!(path.to_string_lossy().contains("cortex.db"));
    }

    #[test]
    fn test_cli_parsing_bench_subcommands() {
        let args = vec!["cortex", "bench", "run", "--suite", "coding", "--json"];
        let parsed = Cli::try_parse_from(args).unwrap();
        match parsed.command {
            Some(Commands::Bench {
                action:
                    BenchCommands::Run {
                        suite,
                        json: true,
                        report: None,
                        ..
                    },
            }) => assert_eq!(suite, "coding"),
            _ => panic!("unexpected command parsed"),
        }
    }

    #[test]
    fn test_cli_parsing_tui() {
        let args = vec!["cortex", "tui", "--db", "/tmp/cortex.db"];
        let parsed = Cli::try_parse_from(args).unwrap();
        match parsed.command {
            Some(Commands::Tui { db: Some(db) }) => {
                assert_eq!(db, PathBuf::from("/tmp/cortex.db"));
            }
            _ => panic!("unexpected command parsed"),
        }

        let args_no_db = vec!["cortex", "tui"];
        let parsed_no_db = Cli::try_parse_from(args_no_db).unwrap();
        match parsed_no_db.command {
            Some(Commands::Tui { db: None }) => {}
            _ => panic!("unexpected command parsed"),
        }
    }

    #[test]
    fn test_cli_parsing_run_default() {
        let args = vec!["cortex", "run", "Fix the failing test in main.rs"];
        let parsed = Cli::try_parse_from(args).unwrap();
        match parsed.command {
            Some(Commands::Run {
                prompt,
                model,
                api_key,
                base_url,
                workspace,
                max_iterations,
                quiet,
                json,
                ..
            }) => {
                assert_eq!(prompt, "Fix the failing test in main.rs");
                assert_eq!(model, "gpt-4o-mini");
                assert!(api_key.is_none());
                assert!(base_url.is_none());
                assert!(workspace.is_none());
                assert_eq!(max_iterations, 15);
                assert!(!quiet);
                assert!(!json);
            }
            _ => panic!("unexpected command parsed"),
        }
    }

    #[test]
    fn test_cli_parsing_run_custom_flags() {
        let args = vec![
            "cortex",
            "run",
            "Refactor auth logic",
            "--model",
            "claude-3-5-sonnet-20241022",
            "--api-key",
            "sk-ant-test",
            "--workspace",
            "/tmp/workspace",
            "--max-iterations",
            "25",
            "--quiet",
            "--json",
        ];
        let parsed = Cli::try_parse_from(args).unwrap();
        match parsed.command {
            Some(Commands::Run {
                prompt,
                model,
                api_key,
                workspace,
                max_iterations,
                quiet,
                json,
                ..
            }) => {
                assert_eq!(prompt, "Refactor auth logic");
                assert_eq!(model, "claude-3-5-sonnet-20241022");
                assert_eq!(api_key.as_deref(), Some("sk-ant-test"));
                assert_eq!(workspace, Some(PathBuf::from("/tmp/workspace")));
                assert_eq!(max_iterations, 25);
                assert!(quiet);
                assert!(json);
            }
            _ => panic!("unexpected command parsed"),
        }
    }

    #[test]
    fn test_cli_parsing_run_ollama() {
        let args = vec![
            "cortex",
            "run",
            "Analyze logs",
            "--model",
            "ollama/llama3.1",
            "--base-url",
            "http://localhost:11434/v1",
        ];
        let parsed = Cli::try_parse_from(args).unwrap();
        match parsed.command {
            Some(Commands::Run {
                prompt,
                model,
                base_url,
                ..
            }) => {
                assert_eq!(prompt, "Analyze logs");
                assert_eq!(model, "ollama/llama3.1");
                assert_eq!(base_url.as_deref(), Some("http://localhost:11434/v1"));
            }
            _ => panic!("unexpected command parsed"),
        }
    }

    #[test]
    fn test_cli_parsing_run_with_config() {
        let args = vec![
            "cortex",
            "run",
            "Fix test",
            "--config",
            "custom-cortex.toml",
        ];
        let parsed = Cli::try_parse_from(args).unwrap();
        match parsed.command {
            Some(Commands::Run {
                prompt,
                config: Some(cfg),
                ..
            }) => {
                assert_eq!(prompt, "Fix test");
                assert_eq!(cfg, PathBuf::from("custom-cortex.toml"));
            }
            _ => panic!("unexpected command parsed"),
        }
    }

    #[test]
    fn test_cli_parsing_mcp_subcommands() {
        let args_list = vec!["cortex", "mcp", "list", "--config", "cortex.toml"];
        let parsed_list = Cli::try_parse_from(args_list).unwrap();
        match parsed_list.command {
            Some(Commands::Mcp {
                action: McpCommands::List { config: Some(cfg) },
            }) => {
                assert_eq!(cfg, PathBuf::from("cortex.toml"));
            }
            _ => panic!("unexpected command parsed"),
        }

        let args_test = vec!["cortex", "mcp", "test", "github-srv"];
        let parsed_test = Cli::try_parse_from(args_test).unwrap();
        match parsed_test.command {
            Some(Commands::Mcp {
                action:
                    McpCommands::Test {
                        server,
                        config: None,
                    },
            }) => {
                assert_eq!(server, "github-srv");
            }
            _ => panic!("unexpected command parsed"),
        }
    }

    #[test]
    fn test_cli_parsing_tui_and_dashboard() {
        let args_tui = vec!["cortex", "tui"];
        let parsed_tui = Cli::try_parse_from(args_tui).unwrap();
        assert!(matches!(parsed_tui.command, Some(Commands::Tui { .. })));

        let args_dash = vec!["cortex", "dashboard"];
        let parsed_dash = Cli::try_parse_from(args_dash).unwrap();
        assert!(matches!(parsed_dash.command, Some(Commands::Tui { .. })));
    }

    #[test]
    fn test_cli_parsing_cron_subcommands() {
        let args_list = vec!["cortex", "cron", "list", "--json"];
        let parsed_list = Cli::try_parse_from(args_list).unwrap();
        match parsed_list.command {
            Some(Commands::Cron {
                action: CronCommands::List { json: true, .. },
            }) => {}
            _ => panic!("unexpected command parsed"),
        }

        let args_create = vec![
            "cortex",
            "cron",
            "create",
            "--name",
            "Nightly Test",
            "--schedule",
            "0 2 * * *",
            "--prompt",
            "Run regression tests",
            "--overlap",
            "queue",
        ];
        let parsed_create = Cli::try_parse_from(args_create).unwrap();
        match parsed_create.command {
            Some(Commands::Cron {
                action:
                    CronCommands::Create {
                        name: Some(name),
                        schedule,
                        prompt,
                        overlap,
                        ..
                    },
            }) => {
                assert_eq!(name, "Nightly Test");
                assert_eq!(schedule, "0 2 * * *");
                assert_eq!(prompt, "Run regression tests");
                assert_eq!(overlap, "queue");
            }
            _ => panic!("unexpected command parsed"),
        }

        let args_delete = vec!["cortex", "cron", "delete", "job_12345"];
        let parsed_delete = Cli::try_parse_from(args_delete).unwrap();
        match parsed_delete.command {
            Some(Commands::Cron {
                action: CronCommands::Delete { id, .. },
            }) => {
                assert_eq!(id, "job_12345");
            }
            _ => panic!("unexpected command parsed"),
        }
    }
}
