//! Model Context Protocol (MCP) client and external tool integration.
//!
//! Provides JSON-RPC 2.0 communication over standard input/output and Server-Sent Events,
//! configuration management via `cortex.toml`, and adaptation of remote MCP tools
//! into Cortex's native [`crate::tool::Tool`] registry.

pub mod client;
pub mod config;
pub mod protocol;
pub mod tool;
pub mod transport;

pub use client::McpClient;
pub use config::{CortexConfig, McpConfig, McpManager, McpServerConfig};
pub use protocol::{
    CallToolParams, CallToolResult, InitializeParams, InitializeResult, JsonRpcError,
    JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, McpContent, McpPrompt, McpResource,
    McpToolDefinition, PromptMessage, RequestId, ResourceContent, ServerCapabilities, ServerInfo,
};
pub use tool::McpTool;
pub use transport::{McpTransport, MockTransport, SseTransport, StdioTransport};
