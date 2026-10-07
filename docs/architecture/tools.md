# Tool Architecture

Tools are the controlled interface between an agent and the external world.

Every tool should define:

- stable name
- input schema
- execution function
- permission requirements
- timeout behavior
- error behavior

All tools must pass through the ToolRegistry and common runtime controls.

MCP tools should use the same abstraction rather than creating a separate execution path.
