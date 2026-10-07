//! Benchmark task specifications and individual execution outcomes.

use serde::{Deserialize, Serialize};

/// An individual file to be placed in the task workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskFile {
    /// Relative path within the workspace root.
    pub path: String,
    /// UTF-8 content of the file.
    pub content: String,
}

impl TaskFile {
    /// Create a new [`TaskFile`].
    pub fn new(path: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            content: content.into(),
        }
    }
}

/// Specification for an automated evaluation task with verifiable ground truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkTask {
    /// Unique task identifier.
    pub id: String,
    /// Human-readable task name.
    pub name: String,
    /// Benchmark suite category (e.g., "coding", "refactor", "cli").
    pub suite: String,
    /// Detailed description of the task requirements.
    pub description: String,
    /// Task prompt presented to the agent.
    pub prompt: String,
    /// Initial files populated into the workspace before agent execution.
    pub initial_files: Vec<TaskFile>,
    /// Shell command executed to verify task completion (e.g., "cargo test").
    pub verification_command: String,
    /// Expected process exit code (default: 0).
    pub expected_exit_code: i32,
}

impl BenchmarkTask {
    /// Create a new [`BenchmarkTask`].
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        suite: impl Into<String>,
        description: impl Into<String>,
        prompt: impl Into<String>,
        verification_command: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            suite: suite.into(),
            description: description.into(),
            prompt: prompt.into(),
            initial_files: Vec::new(),
            verification_command: verification_command.into(),
            expected_exit_code: 0,
        }
    }

    /// Add an initial file to the task definition.
    pub fn with_file(mut self, path: impl Into<String>, content: impl Into<String>) -> Self {
        self.initial_files.push(TaskFile {
            path: path.into(),
            content: content.into(),
        });
        self
    }
}

/// Outcome of evaluating an agent against a single benchmark task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskOutcome {
    /// Identifier of the executed task.
    pub task_id: String,
    /// Name of the executed task.
    pub task_name: String,
    /// Suite category of the task.
    pub suite: String,
    /// Whether the task met all verification criteria.
    pub success: bool,
    /// Total execution duration in milliseconds.
    pub duration_ms: u64,
    /// Number of iterations executed by the agent.
    pub step_count: usize,
    /// Total tokens consumed during evaluation.
    pub tokens_consumed: u32,
    /// Number of tool execution failures encountered.
    pub tool_error_count: usize,
    /// Optional failure or error description.
    pub error_message: Option<String>,
}
