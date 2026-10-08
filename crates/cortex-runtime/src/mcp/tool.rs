//! Tool adapter for external Model Context Protocol (MCP) tools.

use crate::mcp::client::McpClient;
use crate::mcp::protocol::McpToolDefinition;
use crate::tool::{Tool, ToolDefinition, ToolResult};
use cortex_core::Result;
use std::fmt;
use std::sync::Arc;

/// Tool adapter that exposes a remote MCP tool through Cortex's [`Tool`] trait.
pub struct McpTool {
    client: Arc<McpClient>,
    remote_name: String,
    definition: ToolDefinition,
}

impl McpTool {
    /// Create a new adapter for an MCP tool definition.
    ///
    /// If `prefix` is provided, the registered name is prefixed as `{prefix}_{remote_name}`.
    pub fn new(
        client: Arc<McpClient>,
        remote_def: McpToolDefinition,
        prefix: Option<&str>,
    ) -> Self {
        let remote_name = remote_def.name;
        let qualified_name = match prefix {
            Some(p) if !p.trim().is_empty() => {
                let p = p.trim();
                if p.ends_with('_') {
                    format!("{}{}", p, remote_name)
                } else {
                    format!("{}_{}", p, remote_name)
                }
            }
            _ => remote_name.clone(),
        };

        let description = remote_def
            .description
            .unwrap_or_else(|| format!("MCP tool: {}", remote_name));

        let definition = ToolDefinition::new(qualified_name, description, remote_def.input_schema);

        Self {
            client,
            remote_name,
            definition,
        }
    }

    /// Return the remote tool name as registered on the MCP server.
    pub fn remote_name(&self) -> &str {
        &self.remote_name
    }

    /// Return the underlying MCP client reference.
    pub fn client(&self) -> &Arc<McpClient> {
        &self.client
    }
}

impl fmt::Debug for McpTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("McpTool")
            .field("remote_name", &self.remote_name)
            .field("definition", &self.definition)
            .finish()
    }
}

impl Tool for McpTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    fn execute(&self, input: &serde_json::Value) -> Result<ToolResult> {
        let call_res = self.client.call_tool(&self.remote_name, input.clone())?;
        let mut output = call_res.text_content();
        if output.is_empty() && !call_res.content.is_empty() {
            output = serde_json::to_string(&call_res.content).unwrap_or_default();
        }
        let is_error = call_res.is_error.unwrap_or(false);
        Ok(ToolResult { output, is_error })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::protocol::{CallToolParams, CallToolResult, JsonRpcResponse, McpContent};
    use crate::mcp::transport::MockTransport;
    use crate::tool::ToolRegistry;
    use serde_json::json;

    #[test]
    fn test_mcp_tool_execution_and_registry() {
        let transport = MockTransport::new().with_handler(|req| {
            if req.method == "tools/call" {
                let params: CallToolParams =
                    serde_json::from_value(req.params.clone().unwrap()).unwrap();
                assert_eq!(params.name, "search");
                let q = params
                    .arguments
                    .as_ref()
                    .and_then(|a| a.get("query"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(req.id.clone()),
                    result: Some(json!(CallToolResult {
                        content: vec![McpContent::text(format!("found results for {}", q))],
                        is_error: Some(false),
                    })),
                    error: None,
                })
            } else {
                panic!("unexpected method {}", req.method);
            }
        });

        let client = Arc::new(McpClient::new(Arc::new(transport)));
        let tool_def = McpToolDefinition {
            name: "search".to_string(),
            description: Some("Search the web".to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"]
            }),
        };

        let mcp_tool = McpTool::new(client, tool_def, Some("web"));
        assert_eq!(mcp_tool.name(), "web_search");
        assert_eq!(mcp_tool.remote_name(), "search");

        let registry = ToolRegistry::new();
        registry.register_tool(mcp_tool).unwrap();

        let res = registry
            .execute("web_search", &json!({ "query": "rust mcp" }))
            .unwrap();
        assert_eq!(res.output, "found results for rust mcp");
        assert!(!res.is_error);
    }
}
