//! Build script for cortex-cli.
//!
//! Automatically ensures the canonical `~/.cortex` directory and default `settings.json`
//! configuration exist upon crate installation.

fn main() {
    // Attempt to ensure ~/.cortex and ~/.cortex/settings.json exist during crate installation.
    // We catch and ignore any permission/environment errors so that packaging or sandboxed builds never fail.
    let cortex_dir = if let Ok(dir) = std::env::var("CORTEX_HOME") {
        if !dir.trim().is_empty() {
            Some(std::path::PathBuf::from(dir))
        } else {
            None
        }
    } else if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        if !home.trim().is_empty() {
            Some(std::path::PathBuf::from(home).join(".cortex"))
        } else {
            None
        }
    } else {
        None
    };

    if let Some(dir) = cortex_dir {
        if !dir.exists() {
            let _ = std::fs::create_dir_all(&dir);
        }
        let settings_file = dir.join("settings.json");
        if !settings_file.exists() {
            let default_settings = r#"{
  "$schema": "https://raw.githubusercontent.com/cortex-ai/cortex/main/schemas/settings.json",
  "_comment": "Cortex settings: configure model, api_key, base_url, or provider keys below",
  "model": "gpt-4o-mini",
  "api_key": null,
  "base_url": null,
  "provider": null,
  "openai_api_key": null,
  "anthropic_api_key": null,
  "ollama_base_url": null,
  "temperature": null,
  "max_iterations": 25,
  "auto_save_sessions": true
}
"#;
            let _ = std::fs::write(&settings_file, default_settings);
        }
    }
}
