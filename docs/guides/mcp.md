# MCP Guide

MCP integrations must enter Cortex through the existing Tool abstraction.

They automatically inherit:

- permissions
- tracing
- timeouts
- cancellation
- error handling

This prevents external tools from bypassing Cortex's security and observability layers.
