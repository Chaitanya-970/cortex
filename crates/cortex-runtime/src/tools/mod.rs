//! Built-in agent tools for filesystem, search, shell, and Git operations.

pub mod fs;
pub mod git;
pub mod search;
pub mod shell;

pub use fs::{
    DeleteFileTool, EditFileTool, ListDirTool, ListDirectoryTool, ReadFileTool, WriteFileTool,
};
pub use git::{GitBranchTool, GitCommitTool, GitDiffTool, GitLogTool, GitPushTool, GitStatusTool};
pub use search::{FindTool, GlobTool, GrepTool, SearchCodeTool};
pub use shell::{BashTool, ExecuteCommandTool, ShellTool};

use crate::tool::ToolRegistry;
use crate::workspace::Workspace;
use cortex_core::Result;
use std::sync::Arc;

/// Register the complete suite of standard coding agent tools into a [`ToolRegistry`].
pub fn register_standard_tools(registry: &ToolRegistry, workspace: Arc<Workspace>) -> Result<()> {
    // Filesystem tools
    registry.register(Arc::new(ReadFileTool::new(workspace.clone())))?;
    registry.register(Arc::new(WriteFileTool::new(workspace.clone())))?;
    registry.register(Arc::new(EditFileTool::new(workspace.clone())))?;
    registry.register(Arc::new(DeleteFileTool::new(workspace.clone())))?;
    registry.register(Arc::new(ListDirTool::new(workspace.clone())))?;

    // Search tools
    registry.register(Arc::new(GrepTool::new(workspace.clone())))?;
    registry.register(Arc::new(GlobTool::new(workspace.clone())))?;
    registry.register(Arc::new(FindTool::new(workspace.clone())))?;
    registry.register(Arc::new(SearchCodeTool::new(workspace.clone())))?;

    // Shell tools
    registry.register(Arc::new(BashTool::new(workspace.clone())))?;
    registry.register(Arc::new(ExecuteCommandTool::new(workspace.clone())))?;
    registry.register(Arc::new(ShellTool::new(workspace.clone())))?;

    // Git tools
    registry.register(Arc::new(GitStatusTool::new(workspace.clone())))?;
    registry.register(Arc::new(GitDiffTool::new(workspace.clone())))?;
    registry.register(Arc::new(GitLogTool::new(workspace.clone())))?;
    registry.register(Arc::new(GitBranchTool::new(workspace.clone())))?;
    registry.register(Arc::new(GitCommitTool::new(workspace)))?;

    Ok(())
}
