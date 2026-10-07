//! Metric collection, aggregation, and export formatting for benchmark runs.

use crate::task::TaskOutcome;
use serde::{Deserialize, Serialize};

/// Aggregated metrics across a suite of evaluated benchmark tasks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkMetrics {
    /// Evaluated suite name.
    pub suite: String,
    /// Total number of tasks executed.
    pub total_tasks: usize,
    /// Number of tasks that completed successfully.
    pub successful_tasks: usize,
    /// Proportion of tasks succeeded (0.0 to 1.0).
    pub success_rate: f64,
    /// Total agent iterations executed.
    pub total_steps: usize,
    /// Average iterations per task.
    pub avg_steps_per_task: f64,
    /// Total wall-clock time in milliseconds.
    pub total_duration_ms: u64,
    /// Total tokens consumed.
    pub total_tokens: u32,
    /// Total tool errors encountered across all tasks.
    pub tool_error_count: usize,
    /// Tool error rate per executed step.
    pub tool_error_rate: f64,
    /// Individual task outcomes.
    pub outcomes: Vec<TaskOutcome>,
}

impl BenchmarkMetrics {
    /// Aggregate individual task outcomes into a [`BenchmarkMetrics`] summary.
    pub fn from_outcomes(suite: impl Into<String>, outcomes: Vec<TaskOutcome>) -> Self {
        let suite_name = suite.into();
        let total_tasks = outcomes.len();
        let successful_tasks = outcomes.iter().filter(|o| o.success).count();
        let success_rate = if total_tasks > 0 {
            successful_tasks as f64 / total_tasks as f64
        } else {
            0.0
        };

        let total_steps: usize = outcomes.iter().map(|o| o.step_count).sum();
        let avg_steps_per_task = if total_tasks > 0 {
            total_steps as f64 / total_tasks as f64
        } else {
            0.0
        };

        let total_duration_ms: u64 = outcomes.iter().map(|o| o.duration_ms).sum();
        let total_tokens: u32 = outcomes.iter().map(|o| o.tokens_consumed).sum();
        let tool_error_count: usize = outcomes.iter().map(|o| o.tool_error_count).sum();
        let tool_error_rate = if total_steps > 0 {
            tool_error_count as f64 / total_steps as f64
        } else {
            0.0
        };

        Self {
            suite: suite_name,
            total_tasks,
            successful_tasks,
            success_rate,
            total_steps,
            avg_steps_per_task,
            total_duration_ms,
            total_tokens,
            tool_error_count,
            tool_error_rate,
            outcomes,
        }
    }

    /// Render metrics as formatted JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Render metrics as a Markdown summary report.
    pub fn to_markdown(&self) -> String {
        let mut md = String::new();
        md.push_str(&format!("# Benchmark Report: {}\n\n", self.suite));
        md.push_str(&format!(
            "- **Success Rate**: {:.1}% ({}/{})\n",
            self.success_rate * 100.0,
            self.successful_tasks,
            self.total_tasks
        ));
        md.push_str(&format!(
            "- **Total Wall-Clock Time**: {} ms\n",
            self.total_duration_ms
        ));
        md.push_str(&format!(
            "- **Average Iterations / Task**: {:.1}\n",
            self.avg_steps_per_task
        ));
        md.push_str(&format!(
            "- **Tool Error Rate**: {:.1}% ({} errors / {} steps)\n\n",
            self.tool_error_rate * 100.0,
            self.tool_error_count,
            self.total_steps
        ));

        md.push_str("| Task ID | Task Name | Status | Steps | Duration (ms) | Tool Errors |\n");
        md.push_str("|---|---|---|---|---|---|\n");

        for outcome in &self.outcomes {
            let status = if outcome.success { "PASSED" } else { "FAILED" };
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                outcome.task_id,
                outcome.task_name,
                status,
                outcome.step_count,
                outcome.duration_ms,
                outcome.tool_error_count
            ));
        }

        md
    }
}
