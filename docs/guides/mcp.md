# Model Context Protocol (MCP) Guide

Cortex provides a native Model Context Protocol (MCP) client conforming to JSON-RPC 2.0 and the 2024-11-05 protocol specification. External tools exposed by MCP servers are adapted into Cortex's standard Tool abstraction and registered into the ToolRegistry.

External tools automatically inherit:

- Runtime schema validation
- Permission boundaries
- Event tracing and SQLite run logging
- Timeouts and cancellation tokens
- Error propagation and handling

External tools share the same execution path as built-in tools.

## Configuration

MCP servers are configured in `cortex.toml` at the root of a workspace or passed with the `--config` flag.

### Configuration Format

```toml
[mcp.servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/dir"]
prefix = "fs"

[mcp.servers.github]
command = "docker"
args = ["run", "-i", "mcp/github"]
env = { GITHUB_PERSONAL_ACCESS_TOKEN = "ghp_xxx" }
prefix = "gh"

[mcp.servers.remote_service]
url = "http://localhost:8080/sse"
prefix = "remote"
disabled = false
```

Configuration fields:

- `command`: Path to executable for standard input/output subprocess communication.
- `args`: Command-line arguments passed to the subprocess.
- `env`: Key-value table of environment variables provided to the subprocess.
- `url`: Remote HTTP endpoint for Server-Sent Events (SSE) transport.
- `prefix`: Tool namespace prefix (for example, `fs` produces `fs_read_file`). Defaults to the server identifier if omitted.
- `disabled`: Set to `true` to skip connecting to the server.

Both `[mcp.servers.<name>]` and `[mcp_servers.<name>]` table syntax are supported.

## Transports

Cortex implements two communication transports:

1. StdioTransport: Spawns local subprocesses with piped standard input and output streams. Communicates using line-delimited JSON-RPC messages. Process handles are monitored and terminated on drop.
2. SseTransport: Communicates with remote servers over HTTP with Server-Sent Events for server-to-client streaming and standard HTTP POST for client requests.

In unit tests and continuous integration, MockTransport provides deterministic in-memory request-response handling.

## Discovery and Registration

When Cortex initializes:

1. `McpManager` connects to all enabled servers and completes the `initialize` handshake to negotiate protocol versions and capabilities.
2. Discovered tools are retrieved via `tools/list` with pagination support.
3. Each remote tool is wrapped into an `McpTool` implementing Cortex's `Tool` trait and registered into `ToolRegistry`.
4. Discovered resources (`resources/list`, `resources/read`) and prompt templates (`prompts/list`, `prompts/get`) are accessible through `McpClient`.

## CLI Commands

### List Configured Servers and Tools

```bash
cortex mcp list
cortex mcp list --config /path/to/cortex.toml
```

Displays each configured server, its transport method, operational status, and all discovered tools with parameter descriptions.

### Test Server Connectivity

```bash
cortex mcp test github
cortex mcp test github --config /path/to/cortex.toml
```

Performs the initialization handshake, queries capabilities, lists available tools, and inspects resources and prompts.

### Run Agents with External Tools

When executing an agent task, Cortex automatically detects `cortex.toml` in the current workspace or accepts an explicit `--config` flag:

```bash
cortex run "Search repository issues and summarize open pull requests" --config cortex.toml
```
