//! # cortex-harness
//!
//! Benchmark evaluation harness, metrics tracking, and reproducibility framework
//! for the Cortex autonomous agent runtime.
//!
//! Provides curated benchmark suites for code repair, multi-step refactoring,
//! and CLI tasks, along with metric aggregation and reporting.

#![deny(missing_docs)]

pub mod metrics;
pub mod runner;
pub mod suite;
pub mod task;

pub use metrics::BenchmarkMetrics;
pub use runner::BenchmarkRunner;
pub use suite::{available_suites, get_suite_tasks, BenchmarkBaselineProvider};
pub use task::{BenchmarkTask, TaskFile, TaskOutcome};

/// Current semantic version of the Cortex harness crate.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;
    use cortex_runtime::model::{MockModelProvider, ModelOutput, ToolCall};
    use serde_json::json;

    #[test]
    fn test_suite_definitions() {
        let coding_tasks = get_suite_tasks("coding");
        assert_eq!(coding_tasks.len(), 5);

        let refactor_tasks = get_suite_tasks("refactor");
        assert_eq!(refactor_tasks.len(), 1);

        let cli_tasks = get_suite_tasks("cli");
        assert_eq!(cli_tasks.len(), 1);

        let suites = available_suites();
        assert!(suites.contains(&"coding"));
        assert!(suites.contains(&"refactor"));
        assert!(suites.contains(&"cli"));
    }

    #[test]
    fn test_metrics_aggregation_and_markdown() {
        let outcomes = vec![
            TaskOutcome {
                task_id: "task-01".to_string(),
                task_name: "Rust syntax".to_string(),
                suite: "coding".to_string(),
                success: true,
                duration_ms: 1200,
                step_count: 2,
                tokens_consumed: 300,
                tool_error_count: 0,
                error_message: None,
            },
            TaskOutcome {
                task_id: "task-02".to_string(),
                task_name: "Python boundary".to_string(),
                suite: "coding".to_string(),
                success: false,
                duration_ms: 2500,
                step_count: 5,
                tokens_consumed: 700,
                tool_error_count: 1,
                error_message: Some("assertion error".to_string()),
            },
        ];

        let metrics = BenchmarkMetrics::from_outcomes("coding", outcomes);
        assert_eq!(metrics.total_tasks, 2);
        assert_eq!(metrics.successful_tasks, 1);
        assert!((metrics.success_rate - 0.5).abs() < 1e-6);
        assert_eq!(metrics.total_steps, 7);
        assert_eq!(metrics.tool_error_count, 1);

        let json = metrics.to_json();
        assert!(json.contains("\"success_rate\": 0.5"));

        let md = metrics.to_markdown();
        assert!(md.contains("# Benchmark Report: coding"));
        assert!(md.contains("PASSED"));
        assert!(md.contains("FAILED"));
    }

    #[test]
    fn test_benchmark_runner_executes_task_successfully() {
        let task = BenchmarkTask::new(
            "test-simple-repair",
            "Simple Repair",
            "coding",
            "Fix test failure",
            "Fix solution.py",
            "python3 test.py",
        )
        .with_file("solution.py", "def value(): return 0\n")
        .with_file(
            "test.py",
            "from solution import value\nassert value() == 1\n",
        );

        let model = MockModelProvider::new();
        // Model writes the fixed file
        model.queue_response(ModelOutput::ToolCalls(vec![ToolCall::new(
            "c1",
            "write_file",
            json!({
                "path": "solution.py",
                "content": "def value(): return 1\n"
            }),
        )]));
        model.queue_response(ModelOutput::FinalAnswer("Fixed value".to_string()));

        let runner = BenchmarkRunner::new(5);
        let outcome = runner.run_task(&task, &model).unwrap();

        assert!(outcome.success);
        assert_eq!(outcome.step_count, 2);
        assert_eq!(outcome.tool_error_count, 0);
        assert!(outcome.error_message.is_none());
    }

    #[test]
    fn test_benchmark_baseline_provider_coding_suite() {
        let runner = BenchmarkRunner::new(5);
        let model = BenchmarkBaselineProvider;
        let metrics = runner.run_suite("coding", &model).unwrap();

        assert_eq!(metrics.total_tasks, 5);
        assert_eq!(metrics.successful_tasks, 5);
        assert!((metrics.success_rate - 1.0).abs() < 1e-6);
        assert_eq!(metrics.tool_error_count, 0);
    }
}
