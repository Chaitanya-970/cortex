//! Structured execution events and event record schemas.

use crate::id::RunId;
use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Structured execution events emitted during an agent lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum ExecutionEvent {
    /// A new agent run has started.
    RunStarted {
        /// Identifier of the run.
        run_id: RunId,
        /// Original prompt or assigned task.
        task: String,
        /// Optional workspace directory path.
        workspace_root: Option<String>,
    },

    /// An inference request was sent to a model provider.
    ModelRequest {
        /// Identifier of the run.
        run_id: RunId,
        /// Preview of the context or request submitted.
        prompt_preview: String,
    },

    /// A structured response was received from a model provider.
    ModelResponse {
        /// Identifier of the run.
        run_id: RunId,
        /// Summary of the model response or proposed actions.
        output_summary: String,
    },

    /// A tool execution has begun.
    ToolStarted {
        /// Identifier of the run.
        run_id: RunId,
        /// Name of the tool.
        tool_name: String,
        /// Parameters passed to the tool.
        arguments: serde_json::Value,
    },

    /// A tool execution completed successfully.
    ToolCompleted {
        /// Identifier of the run.
        run_id: RunId,
        /// Name of the tool.
        tool_name: String,
        /// Output returned by the tool.
        output: String,
    },

    /// A tool execution failed.
    ToolFailed {
        /// Identifier of the run.
        run_id: RunId,
        /// Name of the tool.
        tool_name: String,
        /// Failure error message.
        error: String,
    },

    /// An agent communication or status message.
    AgentMessage {
        /// Identifier of the run.
        run_id: RunId,
        /// Role of the sender (e.g., "user", "assistant", "system").
        role: String,
        /// Message body content.
        content: String,
    },

    /// An agent recovery retry attempt.
    AgentRetry {
        /// Identifier of the run.
        run_id: RunId,
        /// Sequential attempt number.
        attempt: usize,
        /// Reason for retry.
        reason: String,
    },

    /// An unrecoverable error encountered by the agent.
    AgentError {
        /// Identifier of the run.
        run_id: RunId,
        /// Error description.
        error: String,
    },

    /// The agent run concluded successfully.
    RunCompleted {
        /// Identifier of the run.
        run_id: RunId,
        /// Final answer or outcome report.
        final_answer: String,
        /// Total iterations executed.
        iterations: usize,
        /// Elapsed execution time in milliseconds.
        duration_ms: u64,
    },

    /// The agent run was cancelled.
    RunCancelled {
        /// Identifier of the run.
        run_id: RunId,
        /// Reason for cancellation.
        reason: String,
    },
}

impl ExecutionEvent {
    /// Return the string identifier representing the event variant name.
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::RunStarted { .. } => "RunStarted",
            Self::ModelRequest { .. } => "ModelRequest",
            Self::ModelResponse { .. } => "ModelResponse",
            Self::ToolStarted { .. } => "ToolStarted",
            Self::ToolCompleted { .. } => "ToolCompleted",
            Self::ToolFailed { .. } => "ToolFailed",
            Self::AgentMessage { .. } => "AgentMessage",
            Self::AgentRetry { .. } => "AgentRetry",
            Self::AgentError { .. } => "AgentError",
            Self::RunCompleted { .. } => "RunCompleted",
            Self::RunCancelled { .. } => "RunCancelled",
        }
    }

    /// Access the [`RunId`] associated with this event.
    pub fn run_id(&self) -> &RunId {
        match self {
            Self::RunStarted { run_id, .. }
            | Self::ModelRequest { run_id, .. }
            | Self::ModelResponse { run_id, .. }
            | Self::ToolStarted { run_id, .. }
            | Self::ToolCompleted { run_id, .. }
            | Self::ToolFailed { run_id, .. }
            | Self::AgentMessage { run_id, .. }
            | Self::AgentRetry { run_id, .. }
            | Self::AgentError { run_id, .. }
            | Self::RunCompleted { run_id, .. }
            | Self::RunCancelled { run_id, .. } => run_id,
        }
    }
}

/// Persistent record wrapping an event with sequence numbering and timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRecord {
    /// Unique identifier for this event entry.
    pub id: String,
    /// Associated execution run identifier.
    pub run_id: RunId,
    /// Monotonically increasing sequence index within the run.
    pub sequence: u64,
    /// ISO 8601 UTC timestamp of emission.
    pub timestamp: String,
    /// Inner structured event payload.
    pub event: ExecutionEvent,
}

impl EventRecord {
    /// Create a new [`EventRecord`] stamped with current UTC time.
    pub fn new(sequence: u64, event: ExecutionEvent) -> Self {
        let run_id = event.run_id().clone();
        let timestamp = Utc::now().to_rfc3339();
        let id = format!("{}_{}", run_id, sequence);
        Self {
            id,
            run_id,
            sequence,
            timestamp,
            event,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_types_and_record_creation() {
        let run_id = RunId::from("run_100");
        let event = ExecutionEvent::RunStarted {
            run_id: run_id.clone(),
            task: "Inspect repo".to_string(),
            workspace_root: Some("/tmp/test".to_string()),
        };

        assert_eq!(event.event_type(), "RunStarted");
        assert_eq!(event.run_id(), &run_id);

        let record = EventRecord::new(1, event);
        assert_eq!(record.sequence, 1);
        assert_eq!(record.run_id, run_id);
        assert!(!record.timestamp.is_empty());
    }
}
