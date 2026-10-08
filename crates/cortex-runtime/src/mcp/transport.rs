//! MCP transport implementations for STDIO, Server-Sent Events (SSE), and mock testing.

use crate::mcp::protocol::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};
use cortex_core::{CortexError, Result};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Abstract communication transport for exchanging JSON-RPC 2.0 messages with an MCP server.
pub trait McpTransport: Send + Sync {
    /// Send a JSON-RPC request and await the corresponding response.
    fn send_request(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse>;

    /// Send a one-way JSON-RPC notification.
    fn send_notification(&self, notification: &JsonRpcNotification) -> Result<()>;

    /// Gracefully terminate and close the transport.
    fn close(&self) -> Result<()>;
}

/// Standard I/O (STDIO) transport communicating with a child process over piped stdin and stdout.
pub struct StdioTransport {
    stdin: Mutex<ChildStdin>,
    reader: Mutex<BufReader<ChildStdout>>,
    child: Mutex<Option<Child>>,
}

impl StdioTransport {
    /// Spawn an external process and establish a STDIO transport connection.
    pub fn spawn(command: &str, args: &[String], env: &HashMap<String, String>) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::inherit());

        for (k, v) in env {
            cmd.env(k, v);
        }

        let mut child = cmd.spawn().map_err(|e| {
            CortexError::Internal(format!(
                "failed to spawn MCP child process '{}': {}",
                command, e
            ))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            CortexError::Internal("failed to capture child process stdin".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            CortexError::Internal("failed to capture child process stdout".to_string())
        })?;

        Ok(Self {
            stdin: Mutex::new(stdin),
            reader: Mutex::new(BufReader::new(stdout)),
            child: Mutex::new(Some(child)),
        })
    }
}

impl McpTransport for StdioTransport {
    fn send_request(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        let mut payload = serde_json::to_string(request)
            .map_err(|e| CortexError::Internal(format!("failed to serialize request: {}", e)))?;
        payload.push('\n');

        {
            let mut stdin = self
                .stdin
                .lock()
                .map_err(|_| CortexError::Internal("failed to acquire stdin lock".to_string()))?;
            stdin.write_all(payload.as_bytes()).map_err(|e| {
                CortexError::Internal(format!("failed to write to child stdin: {}", e))
            })?;
            stdin.flush().map_err(|e| {
                CortexError::Internal(format!("failed to flush child stdin: {}", e))
            })?;
        }

        let mut reader = self
            .reader
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire stdout lock".to_string()))?;

        // Read line by line until we encounter a response matching our request id
        let mut line = String::new();
        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line).map_err(|e| {
                CortexError::Internal(format!("failed to read from child stdout: {}", e))
            })?;

            if bytes_read == 0 {
                return Err(CortexError::Internal(
                    "unexpected EOF from MCP process stdout".to_string(),
                ));
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Ok(resp) = serde_json::from_str::<JsonRpcResponse>(trimmed) {
                if let Some(resp_id) = &resp.id {
                    if resp_id == &request.id {
                        return Ok(resp);
                    }
                }
            }
        }
    }

    fn send_notification(&self, notification: &JsonRpcNotification) -> Result<()> {
        let mut payload = serde_json::to_string(notification).map_err(|e| {
            CortexError::Internal(format!("failed to serialize notification: {}", e))
        })?;
        payload.push('\n');

        let mut stdin = self
            .stdin
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire stdin lock".to_string()))?;
        stdin
            .write_all(payload.as_bytes())
            .map_err(|e| CortexError::Internal(format!("failed to write notification: {}", e)))?;
        stdin
            .flush()
            .map_err(|e| CortexError::Internal(format!("failed to flush notification: {}", e)))?;

        Ok(())
    }

    fn close(&self) -> Result<()> {
        let mut child_guard = self
            .child
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire child lock".to_string()))?;
        if let Some(mut child) = child_guard.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        Ok(())
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// HTTP and Server-Sent Events (SSE) transport for remote MCP servers.
pub struct SseTransport {
    base_url: String,
    post_endpoint: Mutex<Option<String>>,
    timeout: Duration,
}

impl SseTransport {
    /// Create a new SSE / HTTP transport connecting to the given URL.
    pub fn new(url: impl Into<String>) -> Self {
        Self {
            base_url: url.into(),
            post_endpoint: Mutex::new(None),
            timeout: Duration::from_secs(30),
        }
    }

    /// Set request timeout duration.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Resolve or retrieve the active POST endpoint URL.
    fn get_post_url(&self) -> Result<String> {
        let mut endpoint_guard = self
            .post_endpoint
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire endpoint lock".to_string()))?;

        if let Some(url) = &*endpoint_guard {
            return Ok(url.clone());
        }

        // If the URL ends with /sse, attempt initial GET handshake to discover message endpoint
        if self.base_url.ends_with("/sse") {
            let agent = ureq::builder().timeout(self.timeout).build();
            let res = agent.get(&self.base_url).call();

            if let Ok(response) = res {
                let mut reader = BufReader::new(response.into_reader());
                let mut line = String::new();
                for _ in 0..20 {
                    line.clear();
                    if reader.read_line(&mut line).is_ok() && !line.is_empty() {
                        let trimmed = line.trim();
                        if let Some(endpoint_rel) = trimmed.strip_prefix("data: ") {
                            let full_url = if endpoint_rel.starts_with("http://")
                                || endpoint_rel.starts_with("https://")
                            {
                                endpoint_rel.to_string()
                            } else {
                                let base = self.base_url.trim_end_matches("/sse");
                                format!("{}/{}", base, endpoint_rel.trim_start_matches('/'))
                            };
                            *endpoint_guard = Some(full_url.clone());
                            return Ok(full_url);
                        }
                    }
                }
            }
        }

        // Default to base_url as direct JSON-RPC HTTP POST endpoint
        *endpoint_guard = Some(self.base_url.clone());
        Ok(self.base_url.clone())
    }
}

impl McpTransport for SseTransport {
    fn send_request(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        let url = self.get_post_url()?;
        let agent = ureq::builder().timeout(self.timeout).build();

        let response = agent
            .post(&url)
            .set("Content-Type", "application/json")
            .send_json(request)
            .map_err(|e| {
                CortexError::Internal(format!("MCP HTTP request failed to '{}': {}", url, e))
            })?;

        let rpc_res: JsonRpcResponse = response.into_json().map_err(|e| {
            CortexError::Internal(format!(
                "failed to deserialize MCP JSON-RPC response: {}",
                e
            ))
        })?;

        Ok(rpc_res)
    }

    fn send_notification(&self, notification: &JsonRpcNotification) -> Result<()> {
        let url = self.get_post_url()?;
        let agent = ureq::builder().timeout(self.timeout).build();

        let _ = agent
            .post(&url)
            .set("Content-Type", "application/json")
            .send_json(notification);

        Ok(())
    }

    fn close(&self) -> Result<()> {
        Ok(())
    }
}

/// Type alias for mock request handler closures.
type MockHandler = Box<dyn Fn(&JsonRpcRequest) -> Result<JsonRpcResponse> + Send + Sync>;

/// In-memory mock transport for deterministic testing without external dependencies.
#[derive(Default)]
pub struct MockTransport {
    handler: Arc<Mutex<Option<MockHandler>>>,
    notifications: Arc<Mutex<Vec<JsonRpcNotification>>>,
}

impl MockTransport {
    /// Create a new [`MockTransport`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a custom callback handler for incoming requests.
    pub fn with_handler<F>(self, handler: F) -> Self
    where
        F: Fn(&JsonRpcRequest) -> Result<JsonRpcResponse> + Send + Sync + 'static,
    {
        *self.handler.lock().unwrap() = Some(Box::new(handler));
        self
    }

    /// Inspect notifications received by this transport.
    pub fn notifications(&self) -> Vec<JsonRpcNotification> {
        self.notifications.lock().unwrap().clone()
    }
}

impl McpTransport for MockTransport {
    fn send_request(&self, request: &JsonRpcRequest) -> Result<JsonRpcResponse> {
        let guard = self.handler.lock().unwrap();
        if let Some(h) = guard.as_ref() {
            h(request)
        } else {
            Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(request.id.clone()),
                result: Some(serde_json::json!({})),
                error: None,
            })
        }
    }

    fn send_notification(&self, notification: &JsonRpcNotification) -> Result<()> {
        self.notifications
            .lock()
            .unwrap()
            .push(notification.clone());
        Ok(())
    }

    fn close(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::protocol::RequestId;
    use serde_json::json;

    #[test]
    fn test_mock_transport_request_response() {
        let transport = MockTransport::new().with_handler(|req| {
            if req.method == "tools/list" {
                Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(req.id.clone()),
                    result: Some(json!({
                        "tools": [
                            {
                                "name": "mock_tool",
                                "description": "A mock tool",
                                "inputSchema": { "type": "object" }
                            }
                        ]
                    })),
                    error: None,
                })
            } else {
                Err(CortexError::NotFound(req.method.clone()))
            }
        });

        let req = JsonRpcRequest::new(42, "tools/list", None);
        let resp = transport.send_request(&req).unwrap();
        assert_eq!(resp.id, Some(RequestId::Number(42)));
        assert!(resp.result.is_some());

        let notify = JsonRpcNotification::new("notifications/initialized", None);
        transport.send_notification(&notify).unwrap();
        assert_eq!(transport.notifications().len(), 1);
        assert_eq!(
            transport.notifications()[0].method,
            "notifications/initialized"
        );
    }
}
