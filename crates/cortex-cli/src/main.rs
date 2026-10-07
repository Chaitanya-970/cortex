//! Cortex CLI entrypoint.
//!
//! Provides the primary command-line interface for the Cortex runtime harness.

use clap::{Parser, Subcommand};
use cortex_core::VERSION;

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
}
