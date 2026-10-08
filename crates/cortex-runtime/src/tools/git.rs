//! Git tools for workspace repository operations.
//!
//! Enforces the Phase 2 security contract: status, diff, log, branch, and commit
//! are supported inside the workspace. Push operations are strictly disabled.

use crate::tool::{Tool, ToolDefinition, ToolResult};
use crate::workspace::Workspace;
use cortex_core::{CortexError, Result};
use serde_json::json;
use std::process::Command;
use std::sync::Arc;

/// Tool to check git working tree status.
pub struct GitStatusTool {
    workspace: Arc<Workspace>,
    def: ToolDefinition,
}

impl GitStatusTool {
    /// Create a new [`GitStatusTool`].
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            workspace,
            def: ToolDefinition::new(
                "git_status",
                "Displays the status of the repository working tree",
                json!({ "type": "object" }),
            )
            .with_permission(crate::tool::PermissionLevel::ReadOnly),
        }
    }
}

impl Tool for GitStatusTool {
    fn definition(&self) -> &ToolDefinition {
        &self.def
    }

    fn execute(&self, _input: &serde_json::Value) -> Result<ToolResult> {
        let output = Command::new("git")
            .args(["status", "--short"])
            .current_dir(self.workspace.root())
            .output()
            .map_err(|e| CortexError::Internal(format!("failed to run git status: {}", e)))?;

        let res = String::from_utf8_lossy(&output.stdout);
        Ok(ToolResult::success(res.trim().to_string()))
    }
}

/// Tool to inspect git working tree diff.
pub struct GitDiffTool {
    workspace: Arc<Workspace>,
    def: ToolDefinition,
}

impl GitDiffTool {
    /// Create a new [`GitDiffTool`].
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            workspace,
            def: ToolDefinition::new(
                "git_diff",
                "Shows changes between commits, commit and working tree",
                json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" }
                    }
                }),
            )
            .with_permission(crate::tool::PermissionLevel::ReadOnly),
        }
    }
}

impl Tool for GitDiffTool {
    fn definition(&self) -> &ToolDefinition {
        &self.def
    }

    fn execute(&self, input: &serde_json::Value) -> Result<ToolResult> {
        let mut cmd = Command::new("git");
        cmd.arg("diff");
        if let Some(path) = input.get("path").and_then(|p| p.as_str()) {
            let resolved = self.workspace.resolve_path(path)?;
            let rel = self.workspace.relative_path(resolved)?;
            cmd.arg(rel);
        }
        cmd.current_dir(self.workspace.root());

        let output = cmd
            .output()
            .map_err(|e| CortexError::Internal(format!("failed to run git diff: {}", e)))?;

        let res = String::from_utf8_lossy(&output.stdout);
        Ok(ToolResult::success(res.to_string()))
    }
}

/// Tool to inspect recent git commit log.
pub struct GitLogTool {
    workspace: Arc<Workspace>,
    def: ToolDefinition,
}

impl GitLogTool {
    /// Create a new [`GitLogTool`].
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            workspace,
            def: ToolDefinition::new(
                "git_log",
                "Displays commit history within the repository",
                json!({
                    "type": "object",
                    "properties": {
                        "max_count": { "type": "integer" }
                    }
                }),
            )
            .with_permission(crate::tool::PermissionLevel::ReadOnly),
        }
    }
}

impl Tool for GitLogTool {
    fn definition(&self) -> &ToolDefinition {
        &self.def
    }

    fn execute(&self, input: &serde_json::Value) -> Result<ToolResult> {
        let limit = input.get("max_count").and_then(|m| m.as_i64()).unwrap_or(5);
        let output = Command::new("git")
            .args(["log", &format!("-n{}", limit), "--oneline"])
            .current_dir(self.workspace.root())
            .output()
            .map_err(|e| CortexError::Internal(format!("failed to run git log: {}", e)))?;

        let res = String::from_utf8_lossy(&output.stdout);
        Ok(ToolResult::success(res.trim().to_string()))
    }
}

/// Tool to create or switch git branches.
pub struct GitBranchTool {
    workspace: Arc<Workspace>,
    def: ToolDefinition,
}

impl GitBranchTool {
    /// Create a new [`GitBranchTool`].
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            workspace,
            def: ToolDefinition::new(
                "git_branch",
                "Creates or switches to a specified branch in the repository",
                json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "create": { "type": "boolean" }
                    },
                    "required": ["name"]
                }),
            )
            .with_permission(crate::tool::PermissionLevel::ReadOnly),
        }
    }
}

impl Tool for GitBranchTool {
    fn definition(&self) -> &ToolDefinition {
        &self.def
    }

    fn execute(&self, input: &serde_json::Value) -> Result<ToolResult> {
        let branch_name = input["name"].as_str().ok_or_else(|| {
            CortexError::Validation("missing required 'name' parameter".to_string())
        })?;
        let create = input
            .get("create")
            .and_then(|c| c.as_bool())
            .unwrap_or(false);

        let mut cmd = Command::new("git");
        if create {
            cmd.args(["checkout", "-b", branch_name]);
        } else {
            cmd.args(["checkout", branch_name]);
        }
        cmd.current_dir(self.workspace.root());

        let output = cmd.output().map_err(|e| {
            CortexError::Internal(format!("failed to run git branch/checkout: {}", e))
        })?;

        if output.status.success() {
            Ok(ToolResult::success(format!(
                "Successfully switched to branch '{}'",
                branch_name
            )))
        } else {
            let err = String::from_utf8_lossy(&output.stderr);
            Ok(ToolResult::error(format!(
                "Failed to switch to branch '{}': {}",
                branch_name, err
            )))
        }
    }
}

/// Tool to commit staged changes.
pub struct GitCommitTool {
    workspace: Arc<Workspace>,
    def: ToolDefinition,
}

impl GitCommitTool {
    /// Create a new [`GitCommitTool`].
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            workspace,
            def: ToolDefinition::new(
                "git_commit",
                "Stages specified files and creates a commit with the provided message",
                json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" },
                        "paths": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Optional list of file paths to stage and commit"
                        }
                    },
                    "required": ["message"]
                }),
            )
            .with_permission(crate::tool::PermissionLevel::WorkspaceWrite),
        }
    }
}

impl Tool for GitCommitTool {
    fn definition(&self) -> &ToolDefinition {
        &self.def
    }

    fn execute(&self, input: &serde_json::Value) -> Result<ToolResult> {
        let message = input["message"].as_str().ok_or_else(|| {
            CortexError::Validation("missing required 'message' parameter".to_string())
        })?;

        // Stage files
        let mut add_cmd = Command::new("git");
        add_cmd.arg("add");

        if let Some(paths) = input.get("paths").and_then(|p| p.as_array()) {
            if paths.is_empty() {
                add_cmd.arg("-A");
            } else {
                for p in paths {
                    if let Some(path_str) = p.as_str() {
                        let resolved = self.workspace.resolve_path(path_str)?;
                        let rel = self.workspace.relative_path(resolved)?;
                        add_cmd.arg(rel);
                    }
                }
            }
        } else {
            add_cmd.arg("-A");
        }
        add_cmd.current_dir(self.workspace.root());

        let add_out = add_cmd
            .output()
            .map_err(|e| CortexError::Internal(format!("failed to run git add: {}", e)))?;

        if !add_out.status.success() {
            let err = String::from_utf8_lossy(&add_out.stderr);
            return Ok(ToolResult::error(format!("failed to stage files: {}", err)));
        }

        // Commit staged files
        let commit_out = Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(self.workspace.root())
            .output()
            .map_err(|e| CortexError::Internal(format!("failed to run git commit: {}", e)))?;

        if commit_out.status.success() {
            let stdout = String::from_utf8_lossy(&commit_out.stdout);
            Ok(ToolResult::success(stdout.trim().to_string()))
        } else {
            let err = String::from_utf8_lossy(&commit_out.stderr);
            Ok(ToolResult::error(format!("git commit failed: {}", err)))
        }
    }
}

/// Tool representing git push, strictly prohibited by Phase 2 security policy.
pub struct GitPushTool {
    def: ToolDefinition,
}

impl GitPushTool {
    /// Create a new [`GitPushTool`].
    pub fn new() -> Self {
        Self {
            def: ToolDefinition::new(
                "git_push",
                "Pushes commits to a remote repository (strictly disabled)",
                json!({ "type": "object" }),
            )
            .with_permission(crate::tool::PermissionLevel::Danger),
        }
    }
}

impl Default for GitPushTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for GitPushTool {
    fn definition(&self) -> &ToolDefinition {
        &self.def
    }

    fn execute(&self, _input: &serde_json::Value) -> Result<ToolResult> {
        Err(CortexError::PermissionDenied(
            "git push is strictly disabled in current permissions policy".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_push_strictly_denied() {
        let push_tool = GitPushTool::new();
        let err = push_tool.execute(&json!({})).unwrap_err();
        match err {
            CortexError::PermissionDenied(msg) => {
                assert!(msg.contains("git push is strictly disabled"));
            }
            _ => panic!("expected PermissionDenied, got {:?}", err),
        }
    }
}
