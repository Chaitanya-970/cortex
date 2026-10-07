//! Tool trait and definition contracts.

use cortex_core::Result;

/// Specification and metadata of an agent tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDefinition {
    /// Unique name of the tool (e.g., "read_file", "shell").
    pub name: String,
    /// Human-readable description of the tool functionality.
    pub description: String,
}

impl ToolDefinition {
    /// Create a new [`ToolDefinition`].
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
        }
    }
}

/// Abstract contract for runtime tools.
///
/// All tools must pass through capability checks and runtime validation
/// before execution.
pub trait Tool: Send + Sync {
    /// Return the metadata definition of this tool.
    fn definition(&self) -> &ToolDefinition;

    /// Unique name of the tool.
    fn name(&self) -> &str {
        &self.definition().name
    }

    /// Check if the tool is supported on the current platform/runtime environment.
    fn is_available(&self) -> Result<bool> {
        Ok(true)
    }
}
