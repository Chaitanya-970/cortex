//! Filesystem tools confined to workspace boundaries.

use crate::tool::{Tool, ToolDefinition, ToolResult};
use crate::workspace::Workspace;
use cortex_core::{CortexError, Result};
use serde_json::json;
use std::fs;
use std::sync::Arc;

/// Tool to read file contents inside workspace bounds.
pub struct ReadFileTool {
    workspace: Arc<Workspace>,
    def: ToolDefinition,
}

impl ReadFileTool {
    /// Create a new [`ReadFileTool`].
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            workspace,
            def: ToolDefinition::new(
                "read_file",
                "Reads contents from a file within the workspace boundary",
                json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "offset": { "type": "integer" },
                        "limit": { "type": "integer" }
                    },
                    "required": ["path"]
                }),
            ),
        }
    }
}

impl Tool for ReadFileTool {
    fn definition(&self) -> &ToolDefinition {
        &self.def
    }

    fn execute(&self, input: &serde_json::Value) -> Result<ToolResult> {
        let requested_path = input["path"].as_str().ok_or_else(|| {
            CortexError::Validation("missing required 'path' parameter".to_string())
        })?;

        let path = self.workspace.resolve_path(requested_path)?;

        if !path.exists() {
            return Err(CortexError::NotFound(format!(
                "file not found: '{}'",
                requested_path
            )));
        }

        if path.is_dir() {
            return Err(CortexError::Validation(format!(
                "path '{}' is a directory, not a file",
                requested_path
            )));
        }

        let content = fs::read_to_string(&path).map_err(|e| {
            CortexError::Internal(format!("failed to read file '{}': {}", path.display(), e))
        })?;

        let offset = input
            .get("offset")
            .and_then(|o| o.as_i64())
            .unwrap_or(0)
            .max(0) as usize;
        let limit = input.get("limit").and_then(|l| l.as_i64());

        let lines: Vec<&str> = content.lines().collect();
        let selected_lines = if let Some(limit_val) = limit {
            let max_lines = limit_val.max(0) as usize;
            lines
                .into_iter()
                .skip(offset)
                .take(max_lines)
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            lines
                .into_iter()
                .skip(offset)
                .collect::<Vec<_>>()
                .join("\n")
        };

        Ok(ToolResult::success(selected_lines))
    }
}

/// Tool to write or update file contents inside workspace bounds.
pub struct WriteFileTool {
    workspace: Arc<Workspace>,
    def: ToolDefinition,
}

impl WriteFileTool {
    /// Create a new [`WriteFileTool`].
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            workspace,
            def: ToolDefinition::new(
                "write_file",
                "Writes or updates content of a file within the workspace boundary",
                json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" },
                        "content": { "type": "string" }
                    },
                    "required": ["path", "content"]
                }),
            ),
        }
    }
}

impl Tool for WriteFileTool {
    fn definition(&self) -> &ToolDefinition {
        &self.def
    }

    fn execute(&self, input: &serde_json::Value) -> Result<ToolResult> {
        let requested_path = input["path"].as_str().ok_or_else(|| {
            CortexError::Validation("missing required 'path' parameter".to_string())
        })?;
        let content = input["content"].as_str().ok_or_else(|| {
            CortexError::Validation("missing required 'content' parameter".to_string())
        })?;

        let path = self.workspace.resolve_path(requested_path)?;

        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent).map_err(|e| {
                    CortexError::Internal(format!(
                        "failed to create parent directories for '{}': {}",
                        path.display(),
                        e
                    ))
                })?;
            }
        }

        fs::write(&path, content).map_err(|e| {
            CortexError::Internal(format!("failed to write file '{}': {}", path.display(), e))
        })?;

        Ok(ToolResult::success(format!(
            "Successfully wrote {} bytes to '{}'",
            content.len(),
            requested_path
        )))
    }
}

/// Tool to list files and subdirectories within workspace bounds.
pub struct ListDirTool {
    workspace: Arc<Workspace>,
    def: ToolDefinition,
}

impl ListDirTool {
    /// Create a new [`ListDirTool`].
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            workspace,
            def: ToolDefinition::new(
                "list_directory",
                "Lists directory entries within the workspace boundary",
                json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string" }
                    }
                }),
            ),
        }
    }
}

impl Tool for ListDirTool {
    fn definition(&self) -> &ToolDefinition {
        &self.def
    }

    fn execute(&self, input: &serde_json::Value) -> Result<ToolResult> {
        let requested_path = input.get("path").and_then(|p| p.as_str()).unwrap_or(".");
        let path = self.workspace.resolve_path(requested_path)?;

        if !path.exists() {
            return Err(CortexError::NotFound(format!(
                "directory not found: '{}'",
                requested_path
            )));
        }

        if !path.is_dir() {
            return Err(CortexError::Validation(format!(
                "path '{}' is not a directory",
                requested_path
            )));
        }

        let read_dir = fs::read_dir(&path).map_err(|e| {
            CortexError::Internal(format!(
                "failed to list directory '{}': {}",
                path.display(),
                e
            ))
        })?;

        let mut entries = Vec::new();
        for entry in read_dir {
            let entry = entry.map_err(|e| CortexError::Internal(e.to_string()))?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            let file_type = entry
                .file_type()
                .map_err(|e| CortexError::Internal(e.to_string()))?;

            if file_type.is_dir() {
                entries.push(format!("{}/", file_name));
            } else {
                entries.push(file_name);
            }
        }

        entries.sort();
        Ok(ToolResult::success(entries.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filesystem_tools_lifecycle() {
        let temp_dir = std::env::temp_dir().join(format!("cortex_test_fs_{}", std::process::id()));
        let ws = Arc::new(Workspace::new(&temp_dir).unwrap());

        let write_tool = WriteFileTool::new(Arc::clone(&ws));
        let read_tool = ReadFileTool::new(Arc::clone(&ws));
        let list_tool = ListDirTool::new(Arc::clone(&ws));

        // Write file
        let write_res = write_tool
            .execute(&json!({
                "path": "src/lib.rs",
                "content": "pub fn test() {}"
            }))
            .unwrap();
        assert!(!write_res.is_error);

        // Read file
        let read_res = read_tool
            .execute(&json!({
                "path": "src/lib.rs"
            }))
            .unwrap();
        assert_eq!(read_res.output, "pub fn test() {}");

        // List directory
        let list_res = list_tool
            .execute(&json!({
                "path": "src"
            }))
            .unwrap();
        assert!(list_res.output.contains("lib.rs"));

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
