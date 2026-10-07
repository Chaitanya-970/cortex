//! Cortex CLI entrypoint.
//!
//! Provides the primary command-line interface for the Cortex runtime harness,
//! including execution run inspection and tracing queries.

use clap::{Parser, Subcommand};
use cortex_core::{RunId, VERSION};
use cortex_runtime::{RunStore, RunSummary};
use std::path::PathBuf;

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

    /// Launch the interactive terminal control plane.
    Tui {
        /// Optional path to SQLite database.
        #[arg(short, long)]
        db: Option<PathBuf>,
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

fn default_db_path() -> PathBuf {
    if let Ok(env_path) = std::env::var("CORTEX_DB_PATH") {
        return PathBuf::from(env_path);
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".cortex").join("cortex.db");
    }
    PathBuf::from(".cortex").join("cortex.db")
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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Status) => {
            println!("Cortex Agent Runtime v{}", VERSION);
            println!("Status: Workspace & Architecture Bootstrap");
            println!("Core Interfaces: Loaded (cortex-core, cortex-runtime)");
            println!("Planned Features: See docs/roadmap.md for upcoming milestones");
        }
        Some(Commands::Check) => {
            println!("Cortex v{} environment check:", VERSION);
            println!("  [✓] Workspace crates initialized");
            println!("  [✓] Architecture traits defined");
            println!("  [✓] Ready for runtime development");
        }
        Some(Commands::Run { prompt }) => {
            eprintln!("cortex: 'run' command received task: \"{}\"", prompt);
            eprintln!(
                "Note: Agent execution loop is planned for upcoming milestones. \
                 Initial workspace provides foundations and interfaces."
            );
            std::process::exit(1);
        }
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
        None => {
            println!("Cortex Agent Runtime v{}", VERSION);
            println!("Run 'cortex --help' for usage instructions.");
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
}
