//! Shell tool for executing commands within the workspace boundary.

use crate::tool::{Tool, ToolDefinition, ToolResult};
use crate::workspace::Workspace;
use cortex_core::{CortexError, Result};
use serde_json::json;
use std::process::Command;
use std::sync::Arc;

/// Tool for executing shell commands inside the workspace root.
pub struct ShellTool {
    workspace: Arc<Workspace>,
    def: ToolDefinition,
}

impl ShellTool {
    /// Create a new [`ShellTool`].
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            workspace,
            def: ToolDefinition::new(
                "shell",
                "Executes a command inside the workspace directory and returns output",
                json!({
                    "type": "object",
                    "properties": {
                        "command": { "type": "string" }
                    },
                    "required": ["command"]
                }),
            ),
        }
    }
}

impl Tool for ShellTool {
    fn definition(&self) -> &ToolDefinition {
        &self.def
    }

    fn execute(&self, input: &serde_json::Value) -> Result<ToolResult> {
        execute_shell_command(&self.workspace, input)
    }
}

/// Helper function to execute shell command within workspace bounds.
fn execute_shell_command(workspace: &Workspace, input: &serde_json::Value) -> Result<ToolResult> {
    let command_str = input["command"].as_str().ok_or_else(|| {
        CortexError::Validation("missing required 'command' parameter".to_string())
    })?;

    let workdir = if let Some(sub) = input.get("working_dir").and_then(|w| w.as_str()) {
        workspace.resolve_path(sub)?
    } else {
        workspace.root().to_path_buf()
    };

    let mut cmd = if cfg!(target_os = "windows") {
        let mut c = Command::new("cmd");
        c.args(["/C", command_str]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-c", command_str]);
        c
    };

    // Pin working directory to workspace bounds
    cmd.current_dir(workdir);

    // Scrub sensitive host environment variables
    cmd.env_remove("AWS_SECRET_ACCESS_KEY");
    cmd.env_remove("OPENAI_API_KEY");
    cmd.env_remove("ANTHROPIC_API_KEY");
    cmd.env_remove("GITHUB_TOKEN");
    cmd.env_remove("GH_TOKEN");

    let output = cmd.output().map_err(|e| {
        CortexError::Internal(format!(
            "failed to execute command '{}': {}",
            command_str, e
        ))
    })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let combined = if stderr.is_empty() {
        stdout.to_string()
    } else if stdout.is_empty() {
        stderr.to_string()
    } else {
        format!("{}\n{}", stdout, stderr)
    };

    if output.status.success() {
        Ok(ToolResult::success(combined))
    } else {
        let exit_code = output.status.code().unwrap_or(-1);
        Ok(ToolResult::error(format!(
            "Command exited with code {}:\n{}",
            exit_code, combined
        )))
    }
}

/// Tool for executing bash commands (Claude Code standard tool 'bash').
pub struct BashTool {
    workspace: Arc<Workspace>,
    def: ToolDefinition,
}

impl BashTool {
    /// Create a new [`BashTool`].
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            workspace,
            def: ToolDefinition::new(
                "bash",
                "Executes a bash command within the workspace boundary and returns stdout/stderr",
                json!({
                    "type": "object",
                    "properties": {
                        "command": { "type": "string" },
                        "working_dir": { "type": "string" }
                    },
                    "required": ["command"]
                }),
            )
            .with_permission(crate::tool::PermissionLevel::Execute),
        }
    }
}

impl Tool for BashTool {
    fn definition(&self) -> &ToolDefinition {
        &self.def
    }

    fn execute(&self, input: &serde_json::Value) -> Result<ToolResult> {
        execute_shell_command(&self.workspace, input)
    }
}

/// Tool for executing CLI commands ('execute_command').
pub struct ExecuteCommandTool {
    workspace: Arc<Workspace>,
    def: ToolDefinition,
}

impl ExecuteCommandTool {
    /// Create a new [`ExecuteCommandTool`].
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            workspace,
            def: ToolDefinition::new(
                "execute_command",
                "Executes a CLI command within workspace bounds",
                json!({
                    "type": "object",
                    "properties": {
                        "command": { "type": "string" },
                        "working_dir": { "type": "string" }
                    },
                    "required": ["command"]
                }),
            )
            .with_permission(crate::tool::PermissionLevel::Execute),
        }
    }
}

impl Tool for ExecuteCommandTool {
    fn definition(&self) -> &ToolDefinition {
        &self.def
    }

    fn execute(&self, input: &serde_json::Value) -> Result<ToolResult> {
        execute_shell_command(&self.workspace, input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_tool_execution() {
        let temp_dir =
            std::env::temp_dir().join(format!("cortex_test_shell_{}", std::process::id()));
        let ws = Arc::new(Workspace::new(&temp_dir).unwrap());
        let shell = ShellTool::new(ws);

        let res = shell
            .execute(&json!({
                "command": "echo 'cortex test'"
            }))
            .unwrap();

        assert!(!res.is_error);
        assert!(res.output.contains("cortex test"));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_shell_tool_failure() {
        let temp_dir =
            std::env::temp_dir().join(format!("cortex_test_shell_fail_{}", std::process::id()));
        let ws = Arc::new(Workspace::new(&temp_dir).unwrap());
        let shell = ShellTool::new(ws);

        let res = shell
            .execute(&json!({
                "command": "exit 1"
            }))
            .unwrap();

        assert!(res.is_error);
        assert!(res.output.contains("Command exited with code 1"));

        // Clean up
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
