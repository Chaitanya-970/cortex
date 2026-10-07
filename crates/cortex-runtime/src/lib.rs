//! # cortex-runtime
//!
//! Runtime abstractions, tool execution contracts, model interfaces, and
//! sandboxing boundaries for the Cortex agent harness.
//!
//! Cortex strictly separates model generation from runtime execution authority:
//! models propose actions, and the runtime validates, authorizes, and executes
//! them within bounded sandboxes.

#![deny(missing_docs)]

pub mod model;
pub mod sandbox;
pub mod tool;

pub use model::{ModelDescriptor, ModelProvider};
pub use sandbox::{Sandbox, SandboxMode};
pub use tool::{validate_schema, Tool, ToolDefinition, ToolRegistry, ToolResult};

/// Current semantic version of the Cortex runtime crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyTool;
    impl Tool for DummyTool {
        fn definition(&self) -> &ToolDefinition {
            static DEF: std::sync::OnceLock<ToolDefinition> = std::sync::OnceLock::new();
            DEF.get_or_init(|| {
                ToolDefinition::new(
                    "dummy",
                    "A dummy test tool",
                    serde_json::json!({ "type": "object" }),
                )
            })
        }

        fn execute(&self, _input: &serde_json::Value) -> cortex_core::Result<ToolResult> {
            Ok(ToolResult::success("dummy output"))
        }
    }

    #[test]
    fn test_runtime_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_dummy_tool_contract() {
        let tool = DummyTool;
        assert_eq!(tool.name(), "dummy");
        assert!(tool.is_available().unwrap());
        let res = tool.execute(&serde_json::json!({})).unwrap();
        assert_eq!(res.output, "dummy output");
    }

    #[test]
    fn test_tool_definition() {
        let def = ToolDefinition::new(
            "read_file",
            "Reads file contents from disk",
            serde_json::json!({ "type": "object" }),
        );
        assert_eq!(def.name, "read_file");
        assert_eq!(def.description, "Reads file contents from disk");
    }

    #[test]
    fn test_model_descriptor() {
        let desc = ModelDescriptor::new("google", "gemma-2-9b");
        assert_eq!(desc.provider, "google");
        assert_eq!(desc.name, "gemma-2-9b");
    }
}
