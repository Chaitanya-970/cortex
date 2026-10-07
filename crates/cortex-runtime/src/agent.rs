//! Agent execution context, iteration coordinator, cancellation, and system policies.

use crate::model::{ModelOutput, ModelProvider, ToolCall};
use crate::tool::ToolRegistry;
use crate::workspace::Workspace;
use cortex_core::{CortexError, Result};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
    /// Initial task assigned to the agent.
    pub task: String,
    /// History of messages exchanged during this run.
    pub messages: Vec<ChatMessage>,
    /// Optional workspace boundary for filesystem and subprocess operations.
    pub workspace: Option<Arc<Workspace>>,
    /// Current iteration count.
    pub iterations: usize,
}

impl AgentContext {
    /// Create a new [`AgentContext`] with an initial user task.
    pub fn new(task: impl Into<String>) -> Self {
        let task_str = task.into();
        Self {
            messages: vec![
                ChatMessage::System(CODING_AGENT_POLICY.to_string()),
                ChatMessage::User(task_str.clone()),
            ],
            task: task_str,
            workspace: None,
            iterations: 0,
        }
    }

    /// Attach a [`Workspace`] boundary to this context.
    pub fn with_workspace(mut self, workspace: Arc<Workspace>) -> Self {
        self.workspace = Some(workspace);
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRunResult {
    /// Final text message or report from the agent.
    pub final_answer: String,
    /// Number of iterations executed during the run.
    pub iterations: usize,
    /// Whether the agent reached normal completion.
    pub completed: bool,
}

/// Coordinates the iterative model $\to$ tool $\to$ result $\to$ model execution loop.
pub struct AgentLoop {
    max_iterations: usize,
    cancellation_token: CancellationToken,
}

impl AgentLoop {
    /// Create a new [`AgentLoop`] with the specified iteration cap.
    pub fn new(max_iterations: usize) -> Self {
        Self {
            max_iterations,
            cancellation_token: CancellationToken::new(),
        }
    }

    /// Attach a custom [`CancellationToken`].
    pub fn with_cancellation_token(mut self, token: CancellationToken) -> Self {
        self.cancellation_token = token;
        self
    }

    /// Run the agent execution loop until completion, cancellation, or max iterations.
    pub fn run(
        &self,
        context: &mut AgentContext,
        model: &dyn ModelProvider,
        registry: &ToolRegistry,
    ) -> Result<AgentRunResult> {
        while context.iterations < self.max_iterations {
            if self.cancellation_token.is_cancelled() {
                return Err(CortexError::Cancelled(
                    "agent execution cancelled by request".to_string(),
                ));
            }

            context.iterations += 1;

            // Generate next action from model
            let output = model.generate(context)?;

            match output {
                ModelOutput::FinalAnswer(answer) => {
                    context.push_message(ChatMessage::Assistant(answer.clone()));
                    return Ok(AgentRunResult {
                        final_answer: answer,
                        iterations: context.iterations,
                        completed: true,
                    });
                }
                ModelOutput::ToolCalls(calls) => {
                    for call in calls {
                        if self.cancellation_token.is_cancelled() {
                            return Err(CortexError::Cancelled(
                                "agent execution cancelled during tool processing".to_string(),
                            ));
                        }

                        context.push_message(ChatMessage::ToolCall(call.clone()));

                        // Execute tool through registry (validates schema before dispatch)
                        let tool_result = registry.execute(&call.name, &call.arguments);

                        match tool_result {
                            Ok(res) => {
                                context.push_message(ChatMessage::ToolResult {
                                    call_id: call.id,
                                    tool_name: call.name,
                                    output: res.output,
                                    is_error: res.is_error,
                                });
                            }
                            Err(e) => {
                                context.push_message(ChatMessage::ToolResult {
                                    call_id: call.id,
                                    tool_name: call.name,
                                    output: format!("execution error: {}", e),
                                    is_error: true,
                                });
                            }
                        }
                    }
                }
            }
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
}
