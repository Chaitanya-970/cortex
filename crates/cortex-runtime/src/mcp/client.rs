//! High-level Model Context Protocol (MCP) client.

use crate::mcp::protocol::{
    CallToolParams, CallToolResult, GetPromptResult, InitializeParams, InitializeResult,
    JsonRpcNotification, JsonRpcRequest, ListPromptsResult, ListResourcesResult, ListToolsResult,
    McpPrompt, McpResource, McpToolDefinition, ReadResourceResult, RequestId, ResourceContent,
    ServerCapabilities, ServerInfo,
};
use crate::mcp::transport::{McpTransport, SseTransport, StdioTransport};
use cortex_core::{CortexError, Result};
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

/// High-level client for communicating with an MCP server over an abstract transport.
pub struct McpClient {
    transport: Arc<dyn McpTransport>,
    next_id: AtomicU64,
    server_info: RwLock<Option<ServerInfo>>,
    capabilities: RwLock<Option<ServerCapabilities>>,
}

impl McpClient {
    /// Construct a new [`McpClient`] wrapping the given transport.
    pub fn new(transport: Arc<dyn McpTransport>) -> Self {
        Self {
            transport,
            next_id: AtomicU64::new(1),
            server_info: RwLock::new(None),
            capabilities: RwLock::new(None),
        }
    }

    /// Connect to a local subprocess via standard I/O pipes.
    pub fn connect_stdio(
        command: &str,
        args: &[String],
        env: &HashMap<String, String>,
    ) -> Result<Self> {
        let transport = StdioTransport::spawn(command, args, env)?;
        Ok(Self::new(Arc::new(transport)))
    }

    /// Connect to a remote MCP server via HTTP and Server-Sent Events (SSE).
    pub fn connect_sse(url: &str) -> Result<Self> {
        let transport = SseTransport::new(url);
        Ok(Self::new(Arc::new(transport)))
    }

    /// Generate an incremental request identifier.
    fn next_request_id(&self) -> RequestId {
        RequestId::Number(self.next_id.fetch_add(1, Ordering::SeqCst) as i64)
    }

    /// Execute the initialization handshake with the remote server.
    pub fn initialize(&self) -> Result<InitializeResult> {
        let id = self.next_request_id();
        let params = InitializeParams::default();
        let req = JsonRpcRequest::new(id, "initialize", Some(json!(params)));

        let resp = self.transport.send_request(&req)?;

        if let Some(err) = resp.error {
            return Err(CortexError::Internal(format!(
                "MCP initialization failed [{}]: {}",
                err.code, err.message
            )));
        }

        let result_val = resp.result.ok_or_else(|| {
            CortexError::Internal("missing result in initialize response".to_string())
        })?;

        let init_result: InitializeResult = serde_json::from_value(result_val).map_err(|e| {
            CortexError::Internal(format!("failed to parse initialize result: {}", e))
        })?;

        // Cache server metadata
        if let Ok(mut info_guard) = self.server_info.write() {
            *info_guard = Some(init_result.server_info.clone());
        }
        if let Ok(mut caps_guard) = self.capabilities.write() {
            *caps_guard = Some(init_result.capabilities.clone());
        }

        // Send notifications/initialized acknowledgment
        let notify = JsonRpcNotification::new("notifications/initialized", None);
        let _ = self.transport.send_notification(&notify);

        Ok(init_result)
    }

    /// Return cached server metadata if initialization has succeeded.
    pub fn server_info(&self) -> Option<ServerInfo> {
        self.server_info.read().ok()?.clone()
    }

    /// Return cached server capabilities if initialization has succeeded.
    pub fn capabilities(&self) -> Option<ServerCapabilities> {
        self.capabilities.read().ok()?.clone()
    }

    /// Discover available tools provided by the server.
    pub fn list_tools(&self) -> Result<Vec<McpToolDefinition>> {
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let id = self.next_request_id();
            let params = cursor.as_ref().map(|c| json!({ "cursor": c }));
            let req = JsonRpcRequest::new(id, "tools/list", params);

            let resp = self.transport.send_request(&req)?;

            if let Some(err) = resp.error {
                return Err(CortexError::Internal(format!(
                    "tools/list failed [{}]: {}",
                    err.code, err.message
                )));
            }

            let result_val = resp.result.ok_or_else(|| {
                CortexError::Internal("missing result in tools/list response".to_string())
            })?;

            let list_res: ListToolsResult = serde_json::from_value(result_val).map_err(|e| {
                CortexError::Internal(format!("failed to parse tools/list result: {}", e))
            })?;

            tools.extend(list_res.tools);

            match list_res.next_cursor {
                Some(next) if !next.is_empty() => cursor = Some(next),
                _ => break,
            }
        }

        Ok(tools)
    }

    /// Invoke a tool on the server with input arguments.
    pub fn call_tool(&self, name: &str, arguments: serde_json::Value) -> Result<CallToolResult> {
        let id = self.next_request_id();
        let params = CallToolParams {
            name: name.to_string(),
            arguments: Some(arguments),
        };
        let req = JsonRpcRequest::new(id, "tools/call", Some(json!(params)));

        let resp = self.transport.send_request(&req)?;

        if let Some(err) = resp.error {
            return Err(CortexError::Internal(format!(
                "tools/call failed [{}]: {}",
                err.code, err.message
            )));
        }

        let result_val = resp.result.ok_or_else(|| {
            CortexError::Internal("missing result in tools/call response".to_string())
        })?;

        let call_res: CallToolResult = serde_json::from_value(result_val).map_err(|e| {
            CortexError::Internal(format!("failed to parse tools/call result: {}", e))
        })?;

        Ok(call_res)
    }

    /// Discover available resources on the server.
    pub fn list_resources(&self) -> Result<Vec<McpResource>> {
        let mut resources = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let id = self.next_request_id();
            let params = cursor.as_ref().map(|c| json!({ "cursor": c }));
            let req = JsonRpcRequest::new(id, "resources/list", params);

            let resp = self.transport.send_request(&req)?;

            if let Some(err) = resp.error {
                return Err(CortexError::Internal(format!(
                    "resources/list failed [{}]: {}",
                    err.code, err.message
                )));
            }

            let result_val = resp.result.ok_or_else(|| {
                CortexError::Internal("missing result in resources/list response".to_string())
            })?;

            let list_res: ListResourcesResult =
                serde_json::from_value(result_val).map_err(|e| {
                    CortexError::Internal(format!("failed to parse resources/list result: {}", e))
                })?;

            resources.extend(list_res.resources);

            match list_res.next_cursor {
                Some(next) if !next.is_empty() => cursor = Some(next),
                _ => break,
            }
        }

        Ok(resources)
    }

    /// Read resource content identified by URI.
    pub fn read_resource(&self, uri: &str) -> Result<Vec<ResourceContent>> {
        let id = self.next_request_id();
        let req = JsonRpcRequest::new(id, "resources/read", Some(json!({ "uri": uri })));

        let resp = self.transport.send_request(&req)?;

        if let Some(err) = resp.error {
            return Err(CortexError::Internal(format!(
                "resources/read failed [{}]: {}",
                err.code, err.message
            )));
        }

        let result_val = resp.result.ok_or_else(|| {
            CortexError::Internal("missing result in resources/read response".to_string())
        })?;

        let read_res: ReadResourceResult = serde_json::from_value(result_val).map_err(|e| {
            CortexError::Internal(format!("failed to parse resources/read result: {}", e))
        })?;

        Ok(read_res.contents)
    }

    /// Discover available prompt templates on the server.
    pub fn list_prompts(&self) -> Result<Vec<McpPrompt>> {
        let mut prompts = Vec::new();
        let mut cursor: Option<String> = None;

        loop {
            let id = self.next_request_id();
            let params = cursor.as_ref().map(|c| json!({ "cursor": c }));
            let req = JsonRpcRequest::new(id, "prompts/list", params);

            let resp = self.transport.send_request(&req)?;

            if let Some(err) = resp.error {
                return Err(CortexError::Internal(format!(
                    "prompts/list failed [{}]: {}",
                    err.code, err.message
                )));
            }

            let result_val = resp.result.ok_or_else(|| {
                CortexError::Internal("missing result in prompts/list response".to_string())
            })?;

            let list_res: ListPromptsResult = serde_json::from_value(result_val).map_err(|e| {
                CortexError::Internal(format!("failed to parse prompts/list result: {}", e))
            })?;

            prompts.extend(list_res.prompts);

            match list_res.next_cursor {
                Some(next) if !next.is_empty() => cursor = Some(next),
                _ => break,
            }
        }

        Ok(prompts)
    }

    /// Retrieve a rendered prompt template by name.
    pub fn get_prompt(
        &self,
        name: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<GetPromptResult> {
        let id = self.next_request_id();
        let mut params_obj = json!({ "name": name });
        if let Some(args) = arguments {
            params_obj["arguments"] = args;
        }

        let req = JsonRpcRequest::new(id, "prompts/get", Some(params_obj));
        let resp = self.transport.send_request(&req)?;

        if let Some(err) = resp.error {
            return Err(CortexError::Internal(format!(
                "prompts/get failed [{}]: {}",
                err.code, err.message
            )));
        }

        let result_val = resp.result.ok_or_else(|| {
            CortexError::Internal("missing result in prompts/get response".to_string())
        })?;

        let prompt_res: GetPromptResult = serde_json::from_value(result_val).map_err(|e| {
            CortexError::Internal(format!("failed to parse prompts/get result: {}", e))
        })?;

        Ok(prompt_res)
    }

    /// Gracefully terminate and close the client connection.
    pub fn close(&self) -> Result<()> {
        self.transport.close()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::protocol::{
        McpContent, ServerCapabilities, ServerInfo, LATEST_PROTOCOL_VERSION,
    };
    use crate::mcp::transport::MockTransport;

    #[test]
    fn test_client_handshake_and_tool_calls() {
        let transport = MockTransport::new().with_handler(|req| match req.method.as_str() {
            "initialize" => Ok(crate::mcp::protocol::JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(req.id.clone()),
                result: Some(json!(InitializeResult {
                    protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
                    capabilities: ServerCapabilities::default(),
                    server_info: ServerInfo {
                        name: "mock-server".to_string(),
                        version: "1.0".to_string(),
                    },
                    instructions: None,
                })),
                error: None,
            }),
            "tools/list" => Ok(crate::mcp::protocol::JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(req.id.clone()),
                result: Some(json!({
                    "tools": [
                        {
                            "name": "calculate",
                            "description": "Performs arithmetic",
                            "inputSchema": {
                                "type": "object",
                                "properties": { "expr": { "type": "string" } },
                                "required": ["expr"]
                            }
                        }
                    ]
                })),
                error: None,
            }),
            "tools/call" => {
                let params: CallToolParams =
                    serde_json::from_value(req.params.clone().unwrap()).unwrap();
                assert_eq!(params.name, "calculate");
                Ok(crate::mcp::protocol::JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(req.id.clone()),
                    result: Some(json!(CallToolResult {
                        content: vec![McpContent::Text {
                            text: "42".to_string(),
                        }],
                        is_error: Some(false),
                    })),
                    error: None,
                })
            }
            _ => Err(CortexError::NotFound(req.method.clone())),
        });

        let client = McpClient::new(Arc::new(transport));
        let init = client.initialize().unwrap();
        assert_eq!(init.server_info.name, "mock-server");
        assert_eq!(client.server_info().unwrap().name, "mock-server");

        let tools = client.list_tools().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "calculate");

        let call_res = client
            .call_tool("calculate", json!({ "expr": "6 * 7" }))
            .unwrap();
        assert_eq!(call_res.text_content(), "42");
    }

    #[test]
    fn test_client_resources_and_prompts() {
        let transport = MockTransport::new().with_handler(|req| match req.method.as_str() {
            "resources/list" => Ok(crate::mcp::protocol::JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(req.id.clone()),
                result: Some(json!({
                    "resources": [
                        {
                            "uri": "memo://notes/1",
                            "name": "Note 1",
                            "mimeType": "text/plain"
                        }
                    ]
                })),
                error: None,
            }),
            "resources/read" => Ok(crate::mcp::protocol::JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(req.id.clone()),
                result: Some(json!({
                    "contents": [
                        {
                            "uri": "memo://notes/1",
                            "mimeType": "text/plain",
                            "text": "Meeting notes content"
                        }
                    ]
                })),
                error: None,
            }),
            "prompts/list" => Ok(crate::mcp::protocol::JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(req.id.clone()),
                result: Some(json!({
                    "prompts": [
                        {
                            "name": "summarize",
                            "description": "Summarize text"
                        }
                    ]
                })),
                error: None,
            }),
            _ => Err(CortexError::NotFound(req.method.clone())),
        });

        let client = McpClient::new(Arc::new(transport));

        let res_list = client.list_resources().unwrap();
        assert_eq!(res_list.len(), 1);
        assert_eq!(res_list[0].uri, "memo://notes/1");

        let read_res = client.read_resource("memo://notes/1").unwrap();
        assert_eq!(read_res.len(), 1);
        assert_eq!(read_res[0].text.as_deref(), Some("Meeting notes content"));

        let prompts = client.list_prompts().unwrap();
        assert_eq!(prompts.len(), 1);
        assert_eq!(prompts[0].name, "summarize");
    }
}
