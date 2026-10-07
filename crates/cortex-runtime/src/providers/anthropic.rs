//! Anthropic Claude model provider implementation.
//!
//! Communicates with the `/v1/messages` endpoint using tool_use and tool_result blocks.

use crate::agent::{AgentContext, ChatMessage};
use crate::model::{ModelDescriptor, ModelOutput, ModelProvider, ModelUsage, ToolCall};
use cortex_core::{CortexError, Result};
use std::sync::Mutex;
use std::time::Duration;

/// Model provider communicating via the Anthropic Claude Messages API.
pub struct AnthropicProvider {
    descriptor: ModelDescriptor,
    api_key: Option<String>,
    base_url: String,
    model: String,
    timeout_secs: u64,
    last_usage: Mutex<Option<ModelUsage>>,
}

impl AnthropicProvider {
    /// Create a new [`AnthropicProvider`].
    pub fn new(
        model: impl Into<String>,
        api_key: Option<String>,
        base_url: Option<String>,
    ) -> Self {
        let model_str = model.into();
        let base = base_url
            .unwrap_or_else(|| "https://api.anthropic.com/v1".to_string())
            .trim_end_matches('/')
            .to_string();

        Self {
            descriptor: ModelDescriptor::new("anthropic", &model_str),
            api_key,
            base_url: base,
            model: model_str,
            timeout_secs: 60,
            last_usage: Mutex::new(None),
        }
    }

    /// Set HTTP request timeout in seconds.
    pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
        self.timeout_secs = timeout_secs;
        self
    }

    fn format_payload(
        &self,
        context: &AgentContext,
    ) -> (
        Option<String>,
        Vec<serde_json::Value>,
        Vec<serde_json::Value>,
    ) {
        let mut system = None;
        let mut messages = Vec::new();

        for msg in &context.messages {
            match msg {
                ChatMessage::System(text) => {
                    system = Some(text.clone());
                }
                ChatMessage::User(text) => {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": text
                    }));
                }
                ChatMessage::Assistant(text) => {
                    messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": text
                    }));
                }
                ChatMessage::ToolCall(call) => {
                    messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": [{
                            "type": "tool_use",
                            "id": call.id,
                            "name": call.name,
                            "input": call.arguments
                        }]
                    }));
                }
                ChatMessage::ToolResult {
                    call_id,
                    output,
                    is_error,
                    ..
                } => {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": call_id,
                            "content": output,
                            "is_error": is_error
                        }]
                    }));
                }
            }
        }

        let tools = context
            .tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters
                })
            })
            .collect();

        (system, messages, tools)
    }
}

impl ModelProvider for AnthropicProvider {
    fn descriptor(&self) -> &ModelDescriptor {
        &self.descriptor
    }

    fn is_configured(&self) -> Result<bool> {
        Ok(self.api_key.is_some())
    }

    fn generate(&self, context: &AgentContext) -> Result<ModelOutput> {
        let api_key = self.api_key.as_ref().ok_or_else(|| {
            CortexError::Internal(
                "Anthropic API key is not configured. Set ANTHROPIC_API_KEY environment variable."
                    .to_string(),
            )
        })?;

        let (system, messages, tools) = self.format_payload(context);

        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": messages,
        });

        if let Some(sys) = system {
            body["system"] = serde_json::Value::String(sys);
        }

        if !tools.is_empty() {
            body["tools"] = serde_json::Value::Array(tools);
        }

        let endpoint = format!("{}/messages", self.base_url);
        let req = ureq::post(&endpoint)
            .timeout(Duration::from_secs(self.timeout_secs))
            .set("x-api-key", api_key)
            .set("anthropic-version", "2023-06-01")
            .set("Content-Type", "application/json");

        let response = req.send_json(body).map_err(|e| {
            CortexError::Internal(format!(
                "HTTP request to Anthropic API ({}) failed: {}",
                endpoint, e
            ))
        })?;

        let resp_json: serde_json::Value = response.into_json().map_err(|e| {
            CortexError::Internal(format!("failed to parse Anthropic response JSON: {}", e))
        })?;

        // Extract usage
        if let Some(usage) = resp_json.get("usage") {
            let prompt = usage
                .get("input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let completion = usage
                .get("output_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            *self.last_usage.lock().unwrap() = Some(ModelUsage::new(prompt, completion));
        }

        // Check content blocks
        let content_blocks = resp_json
            .get("content")
            .and_then(|c| c.as_array())
            .ok_or_else(|| {
                CortexError::Internal(
                    "malformed Anthropic response: missing content array".to_string(),
                )
            })?;

        let mut tool_calls = Vec::new();
        let mut text_parts = Vec::new();

        for block in content_blocks {
            let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if block_type == "tool_use" {
                let id = block
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = block
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let input = block
                    .get("input")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                tool_calls.push(ToolCall::new(id, name, input));
            } else if block_type == "text" {
                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                    text_parts.push(text.to_string());
                }
            }
        }

        if !tool_calls.is_empty() {
            Ok(ModelOutput::ToolCalls(tool_calls))
        } else {
            Ok(ModelOutput::FinalAnswer(text_parts.join("\n")))
        }
    }

    fn last_usage(&self) -> Option<ModelUsage> {
        *self.last_usage.lock().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_provider_config() {
        let p_none = AnthropicProvider::new("claude-3-5-sonnet", None, None);
        assert!(!p_none.is_configured().unwrap());

        let p_key =
            AnthropicProvider::new("claude-3-5-sonnet", Some("ant-test-key".to_string()), None);
        assert!(p_key.is_configured().unwrap());
        assert_eq!(p_key.descriptor().provider, "anthropic");
    }

    #[test]
    fn test_anthropic_payload_formatting() {
        let provider = AnthropicProvider::new("claude-3-5-sonnet", None, None);
        let ctx = AgentContext::new("Audit codebase");
        let (sys, msgs, _tools) = provider.format_payload(&ctx);
        assert!(sys.is_some());
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
    }
}
