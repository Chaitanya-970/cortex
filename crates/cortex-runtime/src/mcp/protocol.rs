//! JSON-RPC 2.0 and Model Context Protocol (MCP) data contracts.
//!
//! Conforms to the Model Context Protocol (2024-11-05 specification) for client-server
//! handshakes, tool discovery, tool execution, resource reading, and prompt template retrieval.

use serde::{Deserialize, Serialize};

/// Latest MCP protocol specification version supported by Cortex.
pub const LATEST_PROTOCOL_VERSION: &str = "2024-11-05";

/// JSON-RPC 2.0 request identifier, which may be either an integer or string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RequestId {
    /// Integer request identifier.
    Number(i64),
    /// String request identifier.
    String(String),
}

impl From<i32> for RequestId {
    fn from(val: i32) -> Self {
        Self::Number(val as i64)
    }
}

impl From<i64> for RequestId {
    fn from(val: i64) -> Self {
        Self::Number(val)
    }
}

impl From<u64> for RequestId {
    fn from(val: u64) -> Self {
        Self::Number(val as i64)
    }
}

impl From<&str> for RequestId {
    fn from(val: &str) -> Self {
        Self::String(val.to_string())
    }
}

impl From<String> for RequestId {
    fn from(val: String) -> Self {
        Self::String(val)
    }
}

/// JSON-RPC 2.0 Request message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// Must be exactly `"2.0"`.
    pub jsonrpc: String,
    /// Unique identifier for this request.
    pub id: RequestId,
    /// Remote method name to invoke.
    pub method: String,
    /// Optional parameter object or array.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    /// Construct a new JSON-RPC request with the given method and parameters.
    pub fn new(
        id: impl Into<RequestId>,
        method: impl Into<String>,
        params: Option<serde_json::Value>,
    ) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: id.into(),
            method: method.into(),
            params,
        }
    }
}

/// JSON-RPC 2.0 Notification message (does not expect a reply).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    /// Must be exactly `"2.0"`.
    pub jsonrpc: String,
    /// Notification method name.
    pub method: String,
    /// Optional parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcNotification {
    /// Construct a new JSON-RPC notification.
    pub fn new(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.into(),
            params,
        }
    }
}

/// JSON-RPC 2.0 Error payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Numeric error code.
    pub code: i64,
    /// Short description of the error.
    pub message: String,
    /// Optional structured error data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// JSON-RPC 2.0 Response message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// Must be exactly `"2.0"`.
    pub jsonrpc: String,
    /// Identifier matching the original request, if available.
    #[serde(default)]
    pub id: Option<RequestId>,
    /// Result payload on successful execution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error payload if execution failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// Client metadata provided during the MCP initialization handshake.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientInfo {
    /// Client name.
    pub name: String,
    /// Client version.
    pub version: String,
}

impl Default for ClientInfo {
    fn default() -> Self {
        Self {
            name: "cortex".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Client capabilities declared during initialization.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientCapabilities {
    /// Declares root listing capabilities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roots: Option<serde_json::Value>,
    /// Declares sampling capabilities.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<serde_json::Value>,
    /// Additional experimental capability flags.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental: Option<serde_json::Value>,
}

/// Parameters for the `initialize` method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    /// Client protocol version.
    pub protocol_version: String,
    /// Client capabilities.
    pub capabilities: ClientCapabilities,
    /// Metadata identifying the client application.
    pub client_info: ClientInfo,
}

impl Default for InitializeParams {
    fn default() -> Self {
        Self {
            protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: ClientInfo::default(),
        }
    }
}

/// Server metadata returned during handshake.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerInfo {
    /// Server name.
    pub name: String,
    /// Server version.
    #[serde(default)]
    pub version: String,
}

/// Server capabilities declared during initialization.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerCapabilities {
    /// Server supports tool calls.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<serde_json::Value>,
    /// Server supports resources.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<serde_json::Value>,
    /// Server supports prompt templates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<serde_json::Value>,
    /// Server supports logging notifications.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<serde_json::Value>,
}

/// Result returned from `initialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    /// Protocol version selected by the server.
    pub protocol_version: String,
    /// Server capabilities.
    pub capabilities: ServerCapabilities,
    /// Metadata describing the server.
    pub server_info: ServerInfo,
    /// Optional operational instructions provided by the server.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
}

/// Tool definition discovered from an MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolDefinition {
    /// Unique name of the tool.
    pub name: String,
    /// Optional human-readable description.
    #[serde(default)]
    pub description: Option<String>,
    /// JSON schema describing the tool parameters.
    pub input_schema: serde_json::Value,
}

/// Response returned from `tools/list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListToolsResult {
    /// Discovered tools.
    pub tools: Vec<McpToolDefinition>,
    /// Pagination cursor if more tools are available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Parameters for `tools/call`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallToolParams {
    /// Name of the tool to execute.
    pub name: String,
    /// Input arguments for the tool.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<serde_json::Value>,
}

/// Content item returned in a tool call result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpContent {
    /// Text content output.
    Text {
        /// Text string.
        text: String,
    },
    /// Base64-encoded image or binary content.
    Image {
        /// Base64 payload data.
        data: String,
        /// Media MIME type.
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    /// Embedded resource content.
    Resource {
        /// Embedded resource representation.
        resource: serde_json::Value,
    },
}

impl McpContent {
    /// Create a text content item.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Extract text content if this item is a text block.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text { text } => Some(text),
            _ => None,
        }
    }
}

/// Result returned from `tools/call`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    /// Content items produced by tool execution.
    pub content: Vec<McpContent>,
    /// Whether the tool execution resulted in an error.
    #[serde(default)]
    pub is_error: Option<bool>,
}

impl CallToolResult {
    /// Aggregate all text content items into a single unified output string.
    pub fn text_content(&self) -> String {
        let mut out = String::new();
        for item in &self.content {
            if let Some(txt) = item.as_text() {
                if !out.is_empty() {
                    out.push('\n');
                }
                out.push_str(txt);
            }
        }
        out
    }
}

/// Resource metadata discovered from an MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResource {
    /// Uniform Resource Identifier for this resource.
    pub uri: String,
    /// Human-readable name of the resource.
    pub name: String,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// MIME type if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Response returned from `resources/list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResourcesResult {
    /// Discovered resources.
    pub resources: Vec<McpResource>,
    /// Pagination cursor if more resources are available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Content payload of a read resource.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceContent {
    /// URI of the resource read.
    pub uri: String,
    /// Media MIME type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Text content if resource is textual.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Base64 blob if resource is binary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

/// Response returned from `resources/read`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadResourceResult {
    /// Resource content entries.
    pub contents: Vec<ResourceContent>,
}

/// Argument definition for an MCP prompt template.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptArgument {
    /// Argument name.
    pub name: String,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether this argument is strictly required.
    #[serde(default)]
    pub required: Option<bool>,
}

/// Prompt template metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpPrompt {
    /// Unique prompt template name.
    pub name: String,
    /// Human-readable description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional parameters accepted by the prompt template.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<PromptArgument>>,
}

/// Response returned from `prompts/list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPromptsResult {
    /// Available prompt templates.
    pub prompts: Vec<McpPrompt>,
    /// Pagination cursor if more prompts exist.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// A rendered message inside a prompt template result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptMessage {
    /// Author role (e.g., "user" or "assistant").
    pub role: String,
    /// Content of the message.
    pub content: McpContent,
}

/// Response returned from `prompts/get`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetPromptResult {
    /// Optional description of the rendered prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Messages that comprise the rendered prompt.
    pub messages: Vec<PromptMessage>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_jsonrpc_request_serialization() {
        let req = JsonRpcRequest::new(1, "tools/list", None);
        let serialized = serde_json::to_string(&req).unwrap();
        assert!(serialized.contains("\"jsonrpc\":\"2.0\""));
        assert!(serialized.contains("\"id\":1"));
        assert!(serialized.contains("\"method\":\"tools/list\""));

        let deserialized: JsonRpcRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.id, RequestId::Number(1));
        assert_eq!(deserialized.method, "tools/list");
    }

    #[test]
    fn test_initialize_roundtrip() {
        let init_result = InitializeResult {
            protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
            capabilities: ServerCapabilities {
                tools: Some(json!({"listChanged": true})),
                resources: None,
                prompts: None,
                logging: None,
            },
            server_info: ServerInfo {
                name: "test-server".to_string(),
                version: "1.0.0".to_string(),
            },
            instructions: Some("Handle with care".to_string()),
        };

        let val = serde_json::to_value(&init_result).unwrap();
        assert_eq!(val["protocolVersion"], "2024-11-05");
        assert_eq!(val["serverInfo"]["name"], "test-server");

        let parsed: InitializeResult = serde_json::from_value(val).unwrap();
        assert_eq!(parsed.server_info.name, "test-server");
    }

    #[test]
    fn test_tool_definition_and_call_result() {
        let tool_json = json!({
            "name": "read_file",
            "description": "Read file contents",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }
        });

        let tool_def: McpToolDefinition = serde_json::from_value(tool_json).unwrap();
        assert_eq!(tool_def.name, "read_file");
        assert_eq!(tool_def.description.as_deref(), Some("Read file contents"));

        let call_res = CallToolResult {
            content: vec![
                McpContent::Text {
                    text: "line 1".to_string(),
                },
                McpContent::Text {
                    text: "line 2".to_string(),
                },
            ],
            is_error: Some(false),
        };

        assert_eq!(call_res.text_content(), "line 1\nline 2");
    }
}
