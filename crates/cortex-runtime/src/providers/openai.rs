//! OpenAI-compatible model provider implementation.
//!
//! Supports OpenAI, DeepSeek, OpenRouter, Groq, local Ollama, LM Studio,
//! and other providers implementing the `/chat/completions` protocol.

use crate::agent::{AgentContext, ChatMessage};
use crate::model::{ModelDescriptor, ModelOutput, ModelProvider, ModelUsage, ToolCall};
use cortex_core::{CortexError, Result};
use std::sync::Mutex;
use std::time::Duration;

/// Model provider communicating via the standard OpenAI-compatible `/chat/completions` endpoint.
pub struct OpenAiCompatibleProvider {
    descriptor: ModelDescriptor,
    api_key: Option<String>,
    base_url: String,
    model: String,
    timeout_secs: u64,
    last_usage: Mutex<Option<ModelUsage>>,
}

impl OpenAiCompatibleProvider {
    /// Create a new [`OpenAiCompatibleProvider`].
    pub fn new(
        model: impl Into<String>,
        api_key: Option<String>,
        base_url: Option<String>,
    ) -> Self {
        let model_str = model.into();
        let base = base_url
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string())
            .trim_end_matches('/')
            .to_string();

        let provider_name =
            if base.contains("localhost") || base.contains("127.0.0.1") || base.contains("11434") {
                "ollama"
            } else if base.contains("openrouter") {
                "openrouter"
            } else if base.contains("deepseek") {
                "deepseek"
            } else if base.contains("groq") {
                "groq"
            } else {
                "openai"
            };

        Self {
            descriptor: ModelDescriptor::new(provider_name, &model_str),
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

    fn format_messages(&self, context: &AgentContext) -> Vec<serde_json::Value> {
        let mut messages = Vec::new();

        for msg in &context.messages {
            match msg {
                ChatMessage::System(text) => {
                    messages.push(serde_json::json!({
                        "role": "system",
                        "content": text
                    }));
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
                        "content": null,
                        "tool_calls": [{
                            "id": call.id,
                            "type": "function",
                            "function": {
                                "name": call.name,
                                "arguments": call.arguments.to_string()
                            }
                        }]
                    }));
                }
                ChatMessage::ToolResult {
                    call_id, output, ..
                } => {
                    messages.push(serde_json::json!({
                        "role": "tool",
                        "tool_call_id": call_id,
                        "content": output
                    }));
                }
            }
        }

        messages
    }

    fn format_tools(&self, context: &AgentContext) -> Vec<serde_json::Value> {
        context
            .tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters
                    }
                })
            })
            .collect()
    }
}

impl ModelProvider for OpenAiCompatibleProvider {
    fn descriptor(&self) -> &ModelDescriptor {
        &self.descriptor
    }

    fn is_configured(&self) -> Result<bool> {
        if self.api_key.is_some() {
            return Ok(true);
        }
        // Local providers like Ollama do not require an API key
        if self.base_url.contains("localhost")
            || self.base_url.contains("127.0.0.1")
            || self.base_url.contains("11434")
        {
            return Ok(true);
        }
        Ok(false)
    }

    fn generate(&self, context: &AgentContext) -> Result<ModelOutput> {
        let messages = self.format_messages(context);
        let tools = self.format_tools(context);

        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
        });

        if !tools.is_empty() {
            body["tools"] = serde_json::Value::Array(tools);
            body["tool_choice"] = serde_json::Value::String("auto".to_string());
        }

        let endpoint = format!("{}/chat/completions", self.base_url);
        let mut req = ureq::post(&endpoint)
            .timeout(Duration::from_secs(self.timeout_secs))
            .set("Content-Type", "application/json");

        if let Some(key) = &self.api_key {
            req = req.set("Authorization", &format!("Bearer {}", key));
        }

        let response = req.send_json(body).map_err(|e| {
            CortexError::Internal(format!(
                "HTTP request to model API ({}) failed: {}",
                endpoint, e
            ))
        })?;

        let resp_json: serde_json::Value = response.into_json().map_err(|e| {
            CortexError::Internal(format!("failed to parse model response JSON: {}", e))
        })?;

        // Extract token usage
        if let Some(usage) = resp_json.get("usage") {
            let prompt = usage
                .get("prompt_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let completion = usage
                .get("completion_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            *self.last_usage.lock().unwrap() = Some(ModelUsage::new(prompt, completion));
        }

        // Parse choices
        let choice = resp_json
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|a| a.first())
            .ok_or_else(|| {
                CortexError::Internal("malformed response: missing choices array".to_string())
            })?;

        let message = choice
            .get("message")
            .ok_or_else(|| CortexError::Internal("missing message in choice".to_string()))?;

        // Check for tool calls
        if let Some(tool_calls) = message.get("tool_calls").and_then(|c| c.as_array()) {
            if !tool_calls.is_empty() {
                let mut parsed_calls = Vec::new();
                for (i, tc) in tool_calls.iter().enumerate() {
                    let id = tc
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&format!("call_{}", i))
                        .to_string();

                    let func = tc.get("function").ok_or_else(|| {
                        CortexError::Internal("missing function object in tool_call".to_string())
                    })?;

                    let name = func
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();

                    let raw_args = func
                        .get("arguments")
                        .and_then(|v| v.as_str())
                        .unwrap_or("{}");

                    let args: serde_json::Value =
                        serde_json::from_str(raw_args).unwrap_or_else(|_| serde_json::json!({}));

                    parsed_calls.push(ToolCall::new(id, name, args));
                }
                return Ok(ModelOutput::ToolCalls(parsed_calls));
            }
        }

        // Fall back to text content
        let content = message
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        Ok(ModelOutput::FinalAnswer(content))
    }

    fn last_usage(&self) -> Option<ModelUsage> {
        *self.last_usage.lock().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::ToolDefinition;

    #[test]
    fn test_openai_provider_configuration_check() {
        let p_no_key = OpenAiCompatibleProvider::new("gpt-4o", None, None);
        assert!(!p_no_key.is_configured().unwrap());

        let p_with_key =
            OpenAiCompatibleProvider::new("gpt-4o", Some("sk-test-key".to_string()), None);
        assert!(p_with_key.is_configured().unwrap());

        let p_local = OpenAiCompatibleProvider::new(
            "qwen2.5-coder",
            None,
            Some("http://localhost:11434/v1".to_string()),
        );
        assert!(p_local.is_configured().unwrap());
        assert_eq!(p_local.descriptor().provider, "ollama");
    }

    #[test]
    fn test_message_and_tools_formatting() {
        let provider = OpenAiCompatibleProvider::new("gpt-4o", None, None);
        let mut ctx = AgentContext::new("Test task");
        ctx.tools = vec![ToolDefinition::new(
            "read_file",
            "Reads a file",
            serde_json::json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        )];

        let msgs = provider.format_messages(&ctx);
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[1]["role"], "user");

        let tools = provider.format_tools(&ctx);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["function"]["name"], "read_file");
    }
}
