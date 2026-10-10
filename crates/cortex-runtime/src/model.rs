//! Model provider trait, structured action outputs, and test adapters.

use crate::agent::AgentContext;
use cortex_core::{CortexError, Result};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Mutex;

/// Metadata describing a supported model or model family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelDescriptor {
    /// Identifier of the model provider (e.g., "gemma", "google", "mock").
    pub provider: String,
    /// Model name or variant identifier.
    pub name: String,
}

impl ModelDescriptor {
    /// Create a new [`ModelDescriptor`].
    pub fn new(provider: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            name: name.into(),
        }
    }
}

/// A structured tool call proposed by a model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this tool call invocation.
    pub id: String,
    /// Name of the tool to be invoked.
    pub name: String,
    /// Input arguments for the tool.
    pub arguments: serde_json::Value,
}

impl ToolCall {
    /// Create a new [`ToolCall`].
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            arguments,
        }
    }
}

/// Structured response output from an inference provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelOutput {
    /// The model proposes one or more tool calls to execute.
    ToolCalls(Vec<ToolCall>),
    /// The model has finished its task and produced a final answer message.
    FinalAnswer(String),
}

/// Token usage metrics returned by model inference calls.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelUsage {
    /// Number of tokens in the prompt context.
    pub prompt_tokens: usize,
    /// Number of tokens generated in the completion.
    pub completion_tokens: usize,
    /// Total tokens consumed.
    pub total_tokens: usize,
}

impl ModelUsage {
    /// Create a new [`ModelUsage`] record.
    pub fn new(prompt_tokens: usize, completion_tokens: usize) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        }
    }
}

/// Abstract contract for LLM inference providers.
///
/// In Cortex, the model layer proposes structured actions.
/// The runtime holds sole execution authority, validates schemas, checks permissions,
/// and executes tools within sandboxed boundaries.
pub trait ModelProvider: Send + Sync {
    /// Return the descriptor for this provider.
    fn descriptor(&self) -> &ModelDescriptor;

    /// Validate whether the provider credentials and endpoint are configured.
    fn is_configured(&self) -> Result<bool>;

    /// Generate the next structured response given current agent context.
    fn generate(&self, context: &AgentContext) -> Result<ModelOutput>;

    /// Stream the next structured response token-by-token given current agent context.
    /// Progressive text tokens are sent to `on_token`.
    fn stream(
        &self,
        context: &AgentContext,
        on_token: &mut dyn FnMut(&str) -> Result<()>,
    ) -> Result<ModelOutput> {
        if context.is_cancelled() {
            return Err(CortexError::Cancelled(
                "agent execution cancelled by request".to_string(),
            ));
        }
        let output = self.generate(context)?;
        if context.is_cancelled() {
            return Err(CortexError::Cancelled(
                "agent execution cancelled by request".to_string(),
            ));
        }
        if let ModelOutput::FinalAnswer(ref ans) = output {
            on_token(ans)?;
        }
        Ok(output)
    }

    /// Return token usage reported from the most recent inference call, if available.
    fn last_usage(&self) -> Option<ModelUsage> {
        None
    }
}

/// Scripted mock model provider for deterministic testing and integration evaluation.
pub struct MockModelProvider {
    descriptor: ModelDescriptor,
    scripted_responses: Mutex<VecDeque<ModelOutput>>,
}

impl MockModelProvider {
    /// Create a new [`MockModelProvider`].
    pub fn new() -> Self {
        Self {
            descriptor: ModelDescriptor::new("mock", "scripted-agent-v1"),
            scripted_responses: Mutex::new(VecDeque::new()),
        }
    }

    /// Queue a scripted [`ModelOutput`] to be returned on subsequent `generate` calls.
    pub fn queue_response(&self, response: ModelOutput) {
        let mut queue = self.scripted_responses.lock().unwrap();
        queue.push_back(response);
    }
}

impl Default for MockModelProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelProvider for MockModelProvider {
    fn descriptor(&self) -> &ModelDescriptor {
        &self.descriptor
    }

    fn is_configured(&self) -> Result<bool> {
        Ok(true)
    }

    fn generate(&self, _context: &AgentContext) -> Result<ModelOutput> {
        let mut queue = self.scripted_responses.lock().unwrap();
        queue.pop_front().ok_or_else(|| {
            CortexError::Internal("MockModelProvider ran out of scripted responses".to_string())
        })
    }

    fn stream(
        &self,
        context: &AgentContext,
        on_token: &mut dyn FnMut(&str) -> Result<()>,
    ) -> Result<ModelOutput> {
        if context.is_cancelled() {
            return Err(CortexError::Cancelled(
                "agent execution cancelled by request".to_string(),
            ));
        }
        let output = self.generate(context)?;
        if let ModelOutput::FinalAnswer(ref ans) = output {
            for word in ans.split_inclusive(' ') {
                if context.is_cancelled() {
                    return Err(CortexError::Cancelled(
                        "agent execution cancelled by request".to_string(),
                    ));
                }
                on_token(word)?;
            }
        }
        Ok(output)
    }
}

/// Deterministic replay model provider that re-executes runs from recorded event logs.
pub struct ReplayModelProvider {
    descriptor: ModelDescriptor,
    recorded_responses: Mutex<VecDeque<ModelOutput>>,
}

impl ReplayModelProvider {
    /// Create a [`ReplayModelProvider`] from a list of recorded [`ModelOutput`]s.
    pub fn new(responses: Vec<ModelOutput>) -> Self {
        Self {
            descriptor: ModelDescriptor::new("replay", "deterministic-replay-v1"),
            recorded_responses: Mutex::new(VecDeque::from(responses)),
        }
    }

    /// Construct a [`ReplayModelProvider`] by extracting recorded model responses from a [`crate::storage::RunStore`].
    pub fn from_store(
        store: &crate::storage::RunStore,
        run_id: &cortex_core::RunId,
    ) -> Result<Self> {
        let events = store.get_events(run_id)?;
        let mut outputs = Vec::new();
        for record in events {
            if let cortex_core::ExecutionEvent::ModelResponse {
                structured_output: Some(val),
                ..
            } = record.event
            {
                let model_out: ModelOutput = serde_json::from_value(val).map_err(|e| {
                    CortexError::Internal(format!(
                        "failed to deserialize recorded model output: {}",
                        e
                    ))
                })?;
                outputs.push(model_out);
            }
        }
        if outputs.is_empty() {
            return Err(CortexError::NotFound(format!(
                "no replayable model responses found for run '{}'",
                run_id
            )));
        }
        Ok(Self::new(outputs))
    }
}

impl ModelProvider for ReplayModelProvider {
    fn descriptor(&self) -> &ModelDescriptor {
        &self.descriptor
    }

    fn is_configured(&self) -> Result<bool> {
        Ok(true)
    }

    fn generate(&self, _context: &AgentContext) -> Result<ModelOutput> {
        let mut queue = self.recorded_responses.lock().map_err(|_| {
            CortexError::Internal("failed to acquire replay provider lock".to_string())
        })?;
        queue.pop_front().ok_or_else(|| {
            CortexError::Internal("ReplayModelProvider ran out of recorded responses".to_string())
        })
    }

    fn stream(
        &self,
        context: &AgentContext,
        on_token: &mut dyn FnMut(&str) -> Result<()>,
    ) -> Result<ModelOutput> {
        if context.is_cancelled() {
            return Err(CortexError::Cancelled(
                "agent execution cancelled by request".to_string(),
            ));
        }
        let output = self.generate(context)?;
        if let ModelOutput::FinalAnswer(ref ans) = output {
            for word in ans.split_inclusive(' ') {
                if context.is_cancelled() {
                    return Err(CortexError::Cancelled(
                        "agent execution cancelled by request".to_string(),
                    ));
                }
                on_token(word)?;
            }
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_model_provider() {
        let provider = MockModelProvider::new();
        assert_eq!(provider.descriptor().name, "scripted-agent-v1");

        provider.queue_response(ModelOutput::FinalAnswer("all done".to_string()));
        let context = AgentContext::new("do something");

        let res = provider.generate(&context).unwrap();
        match res {
            ModelOutput::FinalAnswer(ans) => assert_eq!(ans, "all done"),
            _ => panic!("unexpected response"),
        }
    }

    #[test]
    fn test_replay_model_provider() {
        let responses = vec![
            ModelOutput::ToolCalls(vec![ToolCall::new("c1", "dummy", serde_json::json!({}))]),
            ModelOutput::FinalAnswer("replayed finish".to_string()),
        ];
        let replay = ReplayModelProvider::new(responses);
        let ctx = AgentContext::new("replay test");

        let r1 = replay.generate(&ctx).unwrap();
        match r1 {
            ModelOutput::ToolCalls(calls) => assert_eq!(calls[0].id, "c1"),
            _ => panic!("expected ToolCalls"),
        }

        let r2 = replay.generate(&ctx).unwrap();
        match r2 {
            ModelOutput::FinalAnswer(ans) => assert_eq!(ans, "replayed finish"),
            _ => panic!("expected FinalAnswer"),
        }
    }
}
