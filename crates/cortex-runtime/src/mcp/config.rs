//! Configuration schema and lifecycle manager for MCP servers.

use crate::mcp::client::McpClient;
use crate::mcp::tool::McpTool;
use crate::tool::ToolRegistry;
use cortex_core::{CortexError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Top-level configuration loaded from `cortex.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CortexConfig {
    /// MCP section containing server definitions.
    #[serde(default)]
    pub mcp: McpConfig,

    /// Optional alias allowing servers to be placed under `[mcp_servers.<name>]`.
    #[serde(default, rename = "mcp_servers")]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

impl std::str::FromStr for CortexConfig {
    type Err = CortexError;

    fn from_str(content: &str) -> Result<Self> {
        Self::from_toml(content)
    }
}

impl CortexConfig {
    /// Parse configuration from a TOML string.
    pub fn from_toml(content: &str) -> Result<Self> {
        toml::from_str(content)
            .map_err(|e| CortexError::Validation(format!("failed to parse cortex.toml: {}", e)))
    }

    /// Load and parse configuration from a file path.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let p = path.as_ref();
        let content = fs::read_to_string(p).map_err(|e| {
            CortexError::Internal(format!(
                "failed to read configuration file '{}': {}",
                p.display(),
                e
            ))
        })?;
        Self::from_toml(&content)
    }

    /// Search for `cortex.toml` in the given directory or its parents.
    pub fn find_and_load(start_dir: Option<&Path>) -> Result<Option<(PathBuf, Self)>> {
        let current = match start_dir {
            Some(d) => d.to_path_buf(),
            None => std::env::current_dir().map_err(|e| {
                CortexError::Internal(format!("failed to determine current directory: {}", e))
            })?,
        };

        let mut check_dir = current.as_path();
        loop {
            let candidate = check_dir.join("cortex.toml");
            if candidate.is_file() {
                let cfg = Self::from_file(&candidate)?;
                return Ok(Some((candidate, cfg)));
            }
            match check_dir.parent() {
                Some(p) => check_dir = p,
                None => break,
            }
        }

        Ok(None)
    }

    /// Return combined map of configured servers.
    pub fn servers(&self) -> HashMap<String, McpServerConfig> {
        let mut map = self.mcp_servers.clone();
        for (name, srv) in &self.mcp.servers {
            map.insert(name.clone(), srv.clone());
        }
        map
    }
}

/// MCP group configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpConfig {
    /// Servers mapped by identifier name.
    #[serde(default)]
    pub servers: HashMap<String, McpServerConfig>,
}

/// Configuration settings for an individual MCP server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpServerConfig {
    /// Subprocess command executable for stdio transport.
    #[serde(default)]
    pub command: Option<String>,

    /// Subprocess command arguments.
    #[serde(default)]
    pub args: Vec<String>,

    /// Environment variables for subprocess execution.
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Remote endpoint URL for SSE transport.
    #[serde(default)]
    pub url: Option<String>,

    /// Namespace prefix applied to tool names.
    #[serde(default)]
    pub prefix: Option<String>,

    /// Whether this server is disabled.
    #[serde(default)]
    pub disabled: bool,
}

struct ManagedServer {
    client: Arc<McpClient>,
    prefix: Option<String>,
}

/// Manager orchestrating active MCP client connections.
pub struct McpManager {
    servers: HashMap<String, ManagedServer>,
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

impl McpManager {
    /// Create an empty [`McpManager`].
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
        }
    }

    /// Initialize connections to all enabled servers defined in the configuration.
    pub fn start(config: &CortexConfig) -> Result<Self> {
        let mut manager = Self::new();
        let servers = config.servers();

        for (name, srv) in servers {
            if srv.disabled {
                continue;
            }

            let client = if let Some(cmd) = &srv.command {
                McpClient::connect_stdio(cmd, &srv.args, &srv.env)?
            } else if let Some(url) = &srv.url {
                McpClient::connect_sse(url)?
            } else {
                return Err(CortexError::Validation(format!(
                    "MCP server '{}' must specify either 'command' or 'url'",
                    name
                )));
            };

            // Complete protocol handshake
            client.initialize()?;

            let prefix = srv.prefix.or_else(|| Some(name.clone()));
            manager.add_client(&name, Arc::new(client), prefix);
        }

        Ok(manager)
    }

    /// Add an active client to the manager.
    pub fn add_client(&mut self, name: &str, client: Arc<McpClient>, prefix: Option<String>) {
        self.servers
            .insert(name.to_string(), ManagedServer { client, prefix });
    }

    /// Return an active client by server name.
    pub fn get_client(&self, name: &str) -> Option<Arc<McpClient>> {
        self.servers.get(name).map(|s| Arc::clone(&s.client))
    }

    /// Return names of all managed servers.
    pub fn server_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.servers.keys().cloned().collect();
        names.sort();
        names
    }

    /// Discover tools from all servers and register them into the tool registry.
    ///
    /// Returns the total count of successfully registered tools.
    pub fn register_all(&self, registry: &ToolRegistry) -> Result<usize> {
        let mut registered_count = 0;

        for server in self.servers.values() {
            let tools = server.client.list_tools()?;
            for remote_def in tools {
                let tool = McpTool::new(
                    Arc::clone(&server.client),
                    remote_def,
                    server.prefix.as_deref(),
                );
                registry.register_tool(tool)?;
                registered_count += 1;
            }
        }

        Ok(registered_count)
    }

    /// Gracefully close all managed client connections.
    pub fn close_all(&self) -> Result<()> {
        for server in self.servers.values() {
            let _ = server.client.close();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::protocol::{
        InitializeResult, JsonRpcResponse, ListToolsResult, McpToolDefinition, ServerCapabilities,
        ServerInfo, LATEST_PROTOCOL_VERSION,
    };
    use crate::mcp::transport::MockTransport;
    use serde_json::json;

    #[test]
    fn test_parse_cortex_toml_both_formats() {
        let toml_content = r#"
[mcp.servers.github]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-github"]
prefix = "gh"

[mcp_servers.remote_docs]
url = "http://localhost:8080/sse"
prefix = "docs"
disabled = true
"#;

        let cfg = CortexConfig::from_toml(toml_content).unwrap();
        let servers = cfg.servers();

        assert_eq!(servers.len(), 2);

        let gh = servers.get("github").unwrap();
        assert_eq!(gh.command.as_deref(), Some("npx"));
        assert_eq!(gh.args, vec!["-y", "@modelcontextprotocol/server-github"]);
        assert_eq!(gh.prefix.as_deref(), Some("gh"));
        assert!(!gh.disabled);

        let docs = servers.get("remote_docs").unwrap();
        assert_eq!(docs.url.as_deref(), Some("http://localhost:8080/sse"));
        assert_eq!(docs.prefix.as_deref(), Some("docs"));
        assert!(docs.disabled);
    }

    #[test]
    fn test_mcp_manager_discovery_and_registration() {
        let transport = MockTransport::new().with_handler(|req| match req.method.as_str() {
            "initialize" => Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(req.id.clone()),
                result: Some(json!(InitializeResult {
                    protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
                    capabilities: ServerCapabilities::default(),
                    server_info: ServerInfo {
                        name: "test-server".to_string(),
                        version: "1.0.0".to_string(),
                    },
                    instructions: None,
                })),
                error: None,
            }),
            "notifications/initialized" => Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(req.id.clone()),
                result: Some(json!({})),
                error: None,
            }),
            "tools/list" => Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(req.id.clone()),
                result: Some(json!(ListToolsResult {
                    tools: vec![
                        McpToolDefinition {
                            name: "read_doc".to_string(),
                            description: Some("Read documentation".to_string()),
                            input_schema: json!({ "type": "object" }),
                        },
                        McpToolDefinition {
                            name: "search_doc".to_string(),
                            description: Some("Search documentation".to_string()),
                            input_schema: json!({ "type": "object" }),
                        },
                    ],
                    next_cursor: None,
                })),
                error: None,
            }),
            other => panic!("unexpected method {}", other),
        });

        let client = Arc::new(McpClient::new(Arc::new(transport)));
        client.initialize().unwrap();

        let mut manager = McpManager::new();
        manager.add_client("docs", client, Some("docs".to_string()));

        let registry = ToolRegistry::new();
        let registered = manager.register_all(&registry).unwrap();
        assert_eq!(registered, 2);
        assert!(registry.contains("docs_read_doc"));
        assert!(registry.contains("docs_search_doc"));
    }
}
