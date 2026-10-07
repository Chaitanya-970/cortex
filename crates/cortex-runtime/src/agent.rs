//! Agent execution context, iteration coordinator, cancellation, and system policies.

use crate::model::{ModelOutput, ModelProvider, ToolCall};
use crate::storage::RunStore;
use crate::tool::{ToolDefinition, ToolRegistry};
use crate::workspace::Workspace;
use chrono::Utc;
use cortex_core::{CortexError, EventRecord, ExecutionEvent, Redactor, Result, RunId};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Standard coding-agent policy guidelines from the architecture specification.
pub const CODING_AGENT_POLICY: &str = "\
You are Cortex, an autonomous coding agent operating inside a configured repository workspace.
Follow these operational principles strictly:
1. Inspect before modifying: Read files and directory contents before editing.
2. Understand existing code: Respect established patterns and architecture.
3. Make minimal changes: Change only what is required to achieve the goal.
4. Preserve project conventions: Follow existing style, naming, and structure.
5. Run relevant tests: Always verify behavior using the shell test runner.
6. Inspect failures: Analyze errors carefully rather than guessing fixes.
7. Iterate: Refine changes until all relevant checks pass.
8. Review final diff: Verify changes with git diff before committing.
9. Never claim success without verification: Do not declare completion until verified.\
";

/// Types of chat messages recorded during agent execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChatMessage {
    /// System instruction or operational policy.
    System(String),
    /// User prompt or task assignment.
    User(String),
    /// Assistant response message.
    Assistant(String),
    /// Tool execution request proposed by the model.
    ToolCall(ToolCall),
    /// Result returned by executing a tool.
    ToolResult {
        /// Identifier linking back to the [`ToolCall`].
        call_id: String,
        /// Name of the tool executed.
        tool_name: String,
        /// Output content returned by the tool.
        output: String,
        /// Flag indicating whether the tool reported an error.
        is_error: bool,
    },
}

/// Execution context maintaining conversation history and workspace state.
#[derive(Debug, Clone)]
pub struct AgentContext {
    /// Unique identifier for this execution run.
    pub run_id: RunId,
    /// Initial task assigned to the agent.
    pub task: String,
    /// History of messages exchanged during this run.
    pub messages: Vec<ChatMessage>,
    /// Optional workspace boundary for filesystem and subprocess operations.
    pub workspace: Option<Arc<Workspace>>,
    /// Current iteration count.
    pub iterations: usize,
    /// Available tool definitions provided to the model.
    pub tools: Vec<ToolDefinition>,
}

impl AgentContext {
    /// Create a new [`AgentContext`] with an initial user task and generated [`RunId`].
    pub fn new(task: impl Into<String>) -> Self {
        let task_str = task.into();
        Self {
            run_id: RunId::generate(),
            messages: vec![
                ChatMessage::System(CODING_AGENT_POLICY.to_string()),
                ChatMessage::User(task_str.clone()),
            ],
            task: task_str,
            workspace: None,
            iterations: 0,
            tools: Vec::new(),
        }
    }

    /// Set an explicit [`RunId`] for this execution context.
    pub fn with_run_id(mut self, run_id: RunId) -> Self {
        self.run_id = run_id;
        self
    }

    /// Attach a [`Workspace`] boundary to this context.
    pub fn with_workspace(mut self, workspace: Arc<Workspace>) -> Self {
        self.workspace = Some(workspace);
        self
    }

    /// Attach tool definitions to this context.
    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = tools;
        self
    }

    /// Append a message to the context history.
    pub fn push_message(&mut self, message: ChatMessage) {
        self.messages.push(message);
    }
}

/// Thread-safe cancellation token for terminating running agent loops.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Create a new, active [`CancellationToken`].
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Trigger cancellation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Check if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// Final summary outcome produced by an agent execution run.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentRunResult {
    /// Associated execution run identifier.
    pub run_id: RunId,
    /// Final text message or report from the agent.
    pub final_answer: String,
    /// Number of iterations executed during the run.
    pub iterations: usize,
    /// Whether the agent reached normal completion.
    pub completed: bool,
    /// Elapsed execution time in milliseconds.
    pub duration_ms: u64,
    /// Prompt tokens consumed across all model inference steps.
    pub tokens_prompt: usize,
    /// Completion tokens generated across all model inference steps.
    pub tokens_completion: usize,
    /// Total tokens consumed.
    pub tokens_total: usize,
    /// Estimated cost in USD.
    pub estimated_cost_usd: f64,
}

/// Coordinates the iterative model $\to$ tool $\to$ result $\to$ model execution loop.
pub struct AgentLoop {
    max_iterations: usize,
    cancellation_token: CancellationToken,
    store: Option<Arc<RunStore>>,
    redactor: Arc<Redactor>,
}

impl AgentLoop {
    /// Create a new [`AgentLoop`] with the specified iteration cap.
    pub fn new(max_iterations: usize) -> Self {
        Self {
            max_iterations,
            cancellation_token: CancellationToken::new(),
            store: None,
            redactor: Arc::new(Redactor::default()),
        }
    }

    /// Attach a custom [`CancellationToken`].
    pub fn with_cancellation_token(mut self, token: CancellationToken) -> Self {
        self.cancellation_token = token;
        self
    }

    /// Attach a persistent [`RunStore`] for recording structured events and execution runs.
    pub fn with_store(mut self, store: Arc<RunStore>) -> Self {
        self.store = Some(store);
        self
    }

    /// Attach an optional persistent [`RunStore`].
    pub fn with_store_optional(mut self, store: Option<Arc<RunStore>>) -> Self {
        self.store = store;
        self
    }

    /// Attach a custom [`Redactor`] for scrubbing sensitive secrets from traces.
    pub fn with_redactor(mut self, redactor: Arc<Redactor>) -> Self {
        self.redactor = redactor;
        self
    }

    /// Run the agent execution loop until completion, cancellation, or max iterations.
    pub fn run(
        &self,
        context: &mut AgentContext,
        model: &dyn ModelProvider,
        registry: &ToolRegistry,
    ) -> Result<AgentRunResult> {
        context.tools = registry.list();
        let start_time = Instant::now();
        let started_at = Utc::now().to_rfc3339();
        let mut sequence: u64 = 1;
        let mut total_prompt_tokens = 0usize;
        let mut total_completion_tokens = 0usize;

        // Record run start in store
        if let Some(store) = &self.store {
            store.record_run_start(&context.run_id, &context.task, &started_at)?;

            let ws_root = context
                .workspace
                .as_ref()
                .map(|w| w.root().to_string_lossy().to_string());

            let run_started_event = ExecutionEvent::RunStarted {
                run_id: context.run_id.clone(),
                task: self.redactor.redact_text(&context.task),
                workspace_root: ws_root,
            };
            store.record_event(&EventRecord::new(sequence, run_started_event))?;
            sequence += 1;
        }

        while context.iterations < self.max_iterations {
            if self.cancellation_token.is_cancelled() {
                let duration_ms = start_time.elapsed().as_millis() as u64;
                let finished_at = Utc::now().to_rfc3339();
                let estimated_cost = crate::providers::estimate_cost(
                    &model.descriptor().name,
                    total_prompt_tokens,
                    total_completion_tokens,
                );
                if let Some(store) = &self.store {
                    let cancel_event = ExecutionEvent::RunCancelled {
                        run_id: context.run_id.clone(),
                        reason: "agent execution cancelled by request".to_string(),
                    };
                    store.record_event(&EventRecord::new(sequence, cancel_event))?;
                    store.record_run_completion(
                        &context.run_id,
                        "cancelled",
                        &finished_at,
                        duration_ms,
                        total_prompt_tokens as u32,
                        total_completion_tokens as u32,
                        estimated_cost,
                        Some("agent execution cancelled by request"),
                    )?;
                }
                return Err(CortexError::Cancelled(
                    "agent execution cancelled by request".to_string(),
                ));
            }

            context.iterations += 1;

            // Emit ModelRequest event
            if let Some(store) = &self.store {
                let preview = context
                    .messages
                    .last()
                    .map(|m| format!("{:?}", m))
                    .unwrap_or_default();
                let event = ExecutionEvent::ModelRequest {
                    run_id: context.run_id.clone(),
                    prompt_preview: self.redactor.redact_text(&preview),
                };
                store.record_event(&EventRecord::new(sequence, event))?;
                sequence += 1;
            }

            // Generate next action from model
            let output = model.generate(context)?;
            if let Some(usage) = model.last_usage() {
                total_prompt_tokens += usage.prompt_tokens;
                total_completion_tokens += usage.completion_tokens;
            }

            // Emit ModelResponse event
            if let Some(store) = &self.store {
                let summary = match &output {
                    ModelOutput::FinalAnswer(ans) => format!("FinalAnswer: {}", ans),
                    ModelOutput::ToolCalls(calls) => {
                        let names: Vec<_> = calls.iter().map(|c| c.name.as_str()).collect();
                        format!("ToolCalls: {}", names.join(", "))
                    }
                };
                let structured_output = serde_json::to_value(&output).ok();
                let event = ExecutionEvent::ModelResponse {
                    run_id: context.run_id.clone(),
                    output_summary: self.redactor.redact_text(&summary),
                    structured_output,
                };
                store.record_event(&EventRecord::new(sequence, event))?;
                sequence += 1;
            }

            match output {
                ModelOutput::FinalAnswer(answer) => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    let finished_at = Utc::now().to_rfc3339();

                    context.push_message(ChatMessage::Assistant(answer.clone()));

                    if let Some(store) = &self.store {
                        let comp_event = ExecutionEvent::RunCompleted {
                            run_id: context.run_id.clone(),
                            final_answer: self.redactor.redact_text(&answer),
                            iterations: context.iterations,
                            duration_ms,
                        };
                        let estimated_cost = crate::providers::estimate_cost(
                            &model.descriptor().name,
                            total_prompt_tokens,
                            total_completion_tokens,
                        );
                        store.record_event(&EventRecord::new(sequence, comp_event))?;
                        store.record_run_completion(
                            &context.run_id,
                            "completed",
                            &finished_at,
                            duration_ms,
                            total_prompt_tokens as u32,
                            total_completion_tokens as u32,
                            estimated_cost,
                            None,
                        )?;
                    }

                    let estimated_cost = crate::providers::estimate_cost(
                        &model.descriptor().name,
                        total_prompt_tokens,
                        total_completion_tokens,
                    );

                    return Ok(AgentRunResult {
                        run_id: context.run_id.clone(),
                        final_answer: answer,
                        iterations: context.iterations,
                        completed: true,
                        duration_ms,
                        tokens_prompt: total_prompt_tokens,
                        tokens_completion: total_completion_tokens,
                        tokens_total: total_prompt_tokens + total_completion_tokens,
                        estimated_cost_usd: estimated_cost,
                    });
                }
                ModelOutput::ToolCalls(calls) => {
                    for call in calls {
                        if self.cancellation_token.is_cancelled() {
                            let duration_ms = start_time.elapsed().as_millis() as u64;
                            let finished_at = Utc::now().to_rfc3339();
                            if let Some(store) = &self.store {
                                let cancel_event = ExecutionEvent::RunCancelled {
                                    run_id: context.run_id.clone(),
                                    reason: "agent execution cancelled during tool processing"
                                        .to_string(),
                                };
                                store.record_event(&EventRecord::new(sequence, cancel_event))?;
                                store.record_run_completion(
                                    &context.run_id,
                                    "cancelled",
                                    &finished_at,
                                    duration_ms,
                                    0,
                                    0,
                                    0.0,
                                    Some("agent execution cancelled during tool processing"),
                                )?;
                            }
                            return Err(CortexError::Cancelled(
                                "agent execution cancelled during tool processing".to_string(),
                            ));
                        }

                        // Emit ToolStarted event
                        if let Some(store) = &self.store {
                            let start_event = ExecutionEvent::ToolStarted {
                                run_id: context.run_id.clone(),
                                tool_name: call.name.clone(),
                                arguments: self.redactor.redact_json(&call.arguments),
                            };
                            store.record_event(&EventRecord::new(sequence, start_event))?;
                            sequence += 1;
                        }

                        context.push_message(ChatMessage::ToolCall(call.clone()));

                        // Execute tool through registry (validates schema before dispatch)
                        let tool_result = registry.execute(&call.name, &call.arguments);

                        match tool_result {
                            Ok(res) => {
                                let redacted_out = self.redactor.redact_text(&res.output);
                                if let Some(store) = &self.store {
                                    let ev = if res.is_error {
                                        ExecutionEvent::ToolFailed {
                                            run_id: context.run_id.clone(),
                                            tool_name: call.name.clone(),
                                            error: redacted_out.clone(),
                                        }
                                    } else {
                                        ExecutionEvent::ToolCompleted {
                                            run_id: context.run_id.clone(),
                                            tool_name: call.name.clone(),
                                            output: redacted_out.clone(),
                                        }
                                    };
                                    store.record_event(&EventRecord::new(sequence, ev))?;
                                    sequence += 1;
                                }

                                context.push_message(ChatMessage::ToolResult {
                                    call_id: call.id,
                                    tool_name: call.name,
                                    output: res.output,
                                    is_error: res.is_error,
                                });
                            }
                            Err(e) => {
                                let err_msg = format!("execution error: {}", e);
                                let redacted_err = self.redactor.redact_text(&err_msg);
                                if let Some(store) = &self.store {
                                    let ev = ExecutionEvent::ToolFailed {
                                        run_id: context.run_id.clone(),
                                        tool_name: call.name.clone(),
                                        error: redacted_err,
                                    };
                                    store.record_event(&EventRecord::new(sequence, ev))?;
                                    sequence += 1;
                                }

                                context.push_message(ChatMessage::ToolResult {
                                    call_id: call.id,
                                    tool_name: call.name,
                                    output: err_msg,
                                    is_error: true,
                                });
                            }
                        }
                    }
                }
            }
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let finished_at = Utc::now().to_rfc3339();
        if let Some(store) = &self.store {
            let error_event = ExecutionEvent::AgentError {
                run_id: context.run_id.clone(),
                error: format!("exceeded max iterations limit ({})", self.max_iterations),
            };
            let estimated_cost = crate::providers::estimate_cost(
                &model.descriptor().name,
                total_prompt_tokens,
                total_completion_tokens,
            );
            store.record_event(&EventRecord::new(sequence, error_event))?;
            store.record_run_completion(
                &context.run_id,
                "failed",
                &finished_at,
                duration_ms,
                total_prompt_tokens as u32,
                total_completion_tokens as u32,
                estimated_cost,
                Some("exceeded max iterations limit"),
            )?;
        }

        Err(CortexError::Timeout(self.max_iterations as u64))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::MockModelProvider;
    use crate::tool::{Tool, ToolDefinition, ToolResult};
    use serde_json::json;

    struct DummyTool;
    impl Tool for DummyTool {
        fn definition(&self) -> &ToolDefinition {
            static DEF: std::sync::OnceLock<ToolDefinition> = std::sync::OnceLock::new();
            DEF.get_or_init(|| {
                ToolDefinition::new("dummy", "test dummy tool", json!({ "type": "object" }))
            })
        }

        fn execute(&self, _input: &serde_json::Value) -> Result<ToolResult> {
            Ok(ToolResult::success("dummy called"))
        }
    }

    struct SecretLeakingTool;
    impl Tool for SecretLeakingTool {
        fn definition(&self) -> &ToolDefinition {
            static DEF: std::sync::OnceLock<ToolDefinition> = std::sync::OnceLock::new();
            DEF.get_or_init(|| {
                ToolDefinition::new("secret_tool", "leaks secret", json!({ "type": "object" }))
            })
        }

        fn execute(&self, _input: &serde_json::Value) -> Result<ToolResult> {
            Ok(ToolResult::success(
                "API Token: sk-live1234567890abcdef12345678",
            ))
        }
    }

    #[test]
    fn test_agent_loop_successful_completion() {
        let mut context = AgentContext::new("solve task");
        let model = MockModelProvider::new();
        let registry = ToolRegistry::new();

        registry.register_tool(DummyTool).unwrap();

        // Step 1: Model calls tool
        model.queue_response(ModelOutput::ToolCalls(vec![ToolCall::new(
            "call_1",
            "dummy",
            json!({}),
        )]));

        // Step 2: Model finishes with answer
        model.queue_response(ModelOutput::FinalAnswer("task resolved".to_string()));

        let agent_loop = AgentLoop::new(5);
        let result = agent_loop.run(&mut context, &model, &registry).unwrap();

        assert!(result.completed);
        assert_eq!(result.final_answer, "task resolved");
        assert_eq!(result.iterations, 2);
    }

    #[test]
    fn test_agent_loop_cancellation() {
        let mut context = AgentContext::new("infinite task");
        let model = MockModelProvider::new();
        let registry = ToolRegistry::new();

        let token = CancellationToken::new();
        token.cancel();

        let agent_loop = AgentLoop::new(5).with_cancellation_token(token);
        let err = agent_loop.run(&mut context, &model, &registry).unwrap_err();

        match err {
            CortexError::Cancelled(_) => {}
            _ => panic!("expected Cancelled error, got {:?}", err),
        }
    }

    #[test]
    fn test_agent_loop_with_store_and_secret_redaction() {
        let store = Arc::new(RunStore::in_memory().unwrap());
        let mut context = AgentContext::new("Process with sk-secretkey1234567890abcdef");
        let model = MockModelProvider::new();
        let registry = ToolRegistry::new();

        registry.register_tool(SecretLeakingTool).unwrap();

        model.queue_response(ModelOutput::ToolCalls(vec![ToolCall::new(
            "c1",
            "secret_tool",
            json!({ "token": "ghp_1234567890abcdef1234567890abcdef1234" }),
        )]));
        model.queue_response(ModelOutput::FinalAnswer("all done".to_string()));

        let agent_loop = AgentLoop::new(5).with_store(Arc::clone(&store));
        let res = agent_loop.run(&mut context, &model, &registry).unwrap();
        assert!(res.completed);

        // Verify stored run
        let run = store.get_run(&res.run_id).unwrap().unwrap();
        assert_eq!(run.status, "completed");

        // Verify stored events and redactions
        let events = store.get_events(&res.run_id).unwrap();
        assert!(!events.is_empty());

        for record in &events {
            match &record.event {
                ExecutionEvent::RunStarted { task, .. } => {
                    assert!(!task.contains("sk-secretkey"));
                    assert!(task.contains("[REDACTED]"));
                }
                ExecutionEvent::ToolStarted { arguments, .. } => {
                    let arg_str = arguments.to_string();
                    assert!(!arg_str.contains("ghp_"));
                    assert!(arg_str.contains("[REDACTED]"));
                }
                ExecutionEvent::ToolCompleted { output, .. } => {
                    assert!(!output.contains("sk-live"));
                    assert!(output.contains("[REDACTED]"));
                }
                _ => {}
            }
        }
    }
}
