//! End-to-end integration tests for Model Context Protocol (MCP) client,
//! server configuration, tool discovery, and runtime ToolRegistry execution.

use cortex_runtime::mcp::protocol::{
    CallToolParams, CallToolResult, InitializeParams, InitializeResult, JsonRpcResponse,
    ListPromptsResult, ListResourcesResult, ListToolsResult, McpContent, McpPrompt, McpResource,
    McpToolDefinition, PromptMessage, ReadResourceResult, ResourceContent, ServerCapabilities,
    ServerInfo, LATEST_PROTOCOL_VERSION,
};
use cortex_runtime::mcp::transport::MockTransport;
use cortex_runtime::mcp::{CortexConfig, McpClient, McpManager};
use cortex_runtime::ToolRegistry;
use serde_json::json;
use std::sync::Arc;

#[test]
fn test_mcp_config_parsing_and_defaults() {
    let toml_data = r#"
[mcp.servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/workspace"]
prefix = "fs"

[mcp.servers.github]
command = "docker"
args = ["run", "-i", "mcp/github"]
env = { GITHUB_TOKEN = "test-token" }

[mcp.servers.remote_eval]
url = "http://127.0.0.1:9090/sse"
prefix = "eval"
disabled = true
"#;

    let config = CortexConfig::from_toml(toml_data).expect("valid config");
    let servers = config.servers();
    assert_eq!(servers.len(), 3);

    let fs_srv = servers.get("filesystem").unwrap();
    assert_eq!(fs_srv.command.as_deref(), Some("npx"));
    assert_eq!(fs_srv.args.len(), 3);
    assert_eq!(fs_srv.prefix.as_deref(), Some("fs"));
    assert!(!fs_srv.disabled);

    let gh_srv = servers.get("github").unwrap();
    assert_eq!(gh_srv.env.get("GITHUB_TOKEN").unwrap(), "test-token");
    assert_eq!(gh_srv.prefix, None);

    let remote_srv = servers.get("remote_eval").unwrap();
    assert_eq!(remote_srv.url.as_deref(), Some("http://127.0.0.1:9090/sse"));
    assert!(remote_srv.disabled);
}

#[test]
fn test_mcp_end_to_end_discovery_execution_and_registry() {
    let transport = MockTransport::new().with_handler(|req| match req.method.as_str() {
        "initialize" => {
            let _params: InitializeParams =
                serde_json::from_value(req.params.clone().unwrap()).unwrap();
            Ok(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Some(req.id.clone()),
                result: Some(json!(InitializeResult {
                    protocol_version: LATEST_PROTOCOL_VERSION.to_string(),
                    capabilities: ServerCapabilities::default(),
                    server_info: ServerInfo {
                        name: "test-database-server".to_string(),
                        version: "2.1.0".to_string(),
                    },
                    instructions: Some("Execute SQL queries against demo database".to_string()),
                })),
                error: None,
            })
        }
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
                        name: "query".to_string(),
                        description: Some("Run query".to_string()),
                        input_schema: json!({
                            "type": "object",
                            "properties": {
                                "sql": { "type": "string" }
                            },
                            "required": ["sql"]
                        }),
                    },
                    McpToolDefinition {
                        name: "schema".to_string(),
                        description: Some("Inspect table schema".to_string()),
                        input_schema: json!({
                            "type": "object",
                            "properties": {
                                "table": { "type": "string" }
                            }
                        }),
                    },
                ],
                next_cursor: None,
            })),
            error: None,
        }),
        "tools/call" => {
            let params: CallToolParams =
                serde_json::from_value(req.params.clone().unwrap()).unwrap();
            match params.name.as_str() {
                "query" => {
                    let sql = params
                        .arguments
                        .as_ref()
                        .and_then(|a| a.get("sql"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if sql.contains("FAIL") {
                        Ok(JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: Some(req.id.clone()),
                            result: Some(json!(CallToolResult {
                                content: vec![McpContent::text("syntax error at line 1")],
                                is_error: Some(true),
                            })),
                            error: None,
                        })
                    } else {
                        Ok(JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: Some(req.id.clone()),
                            result: Some(json!(CallToolResult {
                                content: vec![McpContent::text("row_count: 42")],
                                is_error: Some(false),
                            })),
                            error: None,
                        })
                    }
                }
                "schema" => Ok(JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(req.id.clone()),
                    result: Some(json!(CallToolResult {
                        content: vec![McpContent::text("columns: id, name, created_at")],
                        is_error: Some(false),
                    })),
                    error: None,
                }),
                other => panic!("unknown tool call: {}", other),
            }
        }
        "resources/list" => Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(req.id.clone()),
            result: Some(json!(ListResourcesResult {
                resources: vec![McpResource {
                    uri: "db://schema/users".to_string(),
                    name: "users schema".to_string(),
                    description: Some("Table layout for users".to_string()),
                    mime_type: Some("text/plain".to_string()),
                }],
                next_cursor: None,
            })),
            error: None,
        }),
        "resources/read" => Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(req.id.clone()),
            result: Some(json!(ReadResourceResult {
                contents: vec![ResourceContent {
                    uri: "db://schema/users".to_string(),
                    mime_type: Some("text/plain".to_string()),
                    text: Some("CREATE TABLE users (id INT, name TEXT);".to_string()),
                    blob: None,
                }],
            })),
            error: None,
        }),
        "prompts/list" => Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(req.id.clone()),
            result: Some(json!(ListPromptsResult {
                prompts: vec![McpPrompt {
                    name: "optimize_query".to_string(),
                    description: Some("Analyze query plan".to_string()),
                    arguments: None,
                }],
                next_cursor: None,
            })),
            error: None,
        }),
        "prompts/get" => Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(req.id.clone()),
            result: Some(json!(cortex_runtime::mcp::protocol::GetPromptResult {
                description: Some("Optimization advice".to_string()),
                messages: vec![PromptMessage {
                    role: "user".to_string(),
                    content: McpContent::text("Add an index on users(created_at)"),
                }],
            })),
            error: None,
        }),
        other => panic!("unexpected method: {}", other),
    });

    let client = Arc::new(McpClient::new(Arc::new(transport)));
    let init_res = client.initialize().expect("initialize succeeds");
    assert_eq!(init_res.server_info.name, "test-database-server");

    // Test resources
    let resources = client.list_resources().expect("resources listed");
    assert_eq!(resources.len(), 1);
    assert_eq!(resources[0].uri, "db://schema/users");

    let contents = client
        .read_resource("db://schema/users")
        .expect("read succeeds");
    assert_eq!(contents.len(), 1);
    assert_eq!(
        contents[0].text.as_deref(),
        Some("CREATE TABLE users (id INT, name TEXT);")
    );

    // Test prompts
    let prompts = client.list_prompts().expect("prompts listed");
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].name, "optimize_query");

    let prompt_content = client
        .get_prompt("optimize_query", None)
        .expect("prompt retrieved");
    assert_eq!(prompt_content.messages.len(), 1);
    assert_eq!(
        prompt_content.messages[0].content.as_text(),
        Some("Add an index on users(created_at)")
    );

    // Test manager and registry integration with namespace prefix
    let mut manager = McpManager::new();
    manager.add_client("db", Arc::clone(&client), Some("db".to_string()));

    let registry = ToolRegistry::new();
    let count = manager
        .register_all(&registry)
        .expect("register all succeeds");
    assert_eq!(count, 2);

    // Verify tools are present with prefixed names
    assert!(registry.contains("db_query"));
    assert!(registry.contains("db_schema"));

    // Execute successful query
    let ok_res = registry
        .execute("db_query", &json!({ "sql": "SELECT COUNT(*) FROM users;" }))
        .expect("execution succeeds");
    assert_eq!(ok_res.output, "row_count: 42");
    assert!(!ok_res.is_error);

    // Execute failing query and check error propagation
    let fail_res = registry
        .execute("db_query", &json!({ "sql": "FAIL SELECT" }))
        .expect("execution succeeds with error flag");
    assert!(fail_res.is_error);
    assert!(fail_res.output.contains("syntax error"));

    // Schema validation prevents invalid parameters before reaching tool
    let invalid_res = registry.execute("db_query", &json!({ "wrong_arg": 123 }));
    assert!(invalid_res.is_err());
}
