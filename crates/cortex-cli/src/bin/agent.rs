//! Dedicated terminal AI coding agent binary entrypoint (`agent`).
//!
//! Provides direct, immediate startup into the interactive AI coding agent
//! environment without subcommands.

fn main() {
    let _ = cortex_core::settings::UserSettings::load_or_create();
    let db_path = cortex_core::settings::cortex_home_dir().join("cortex.db");
    if let Err(e) = cortex_tui::run_tui(&db_path) {
        eprintln!("Error running agent: {}", e);
        std::process::exit(1);
    }
}
