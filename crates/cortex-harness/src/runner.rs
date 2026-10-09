//! Evaluation runner coordinating agent execution on benchmark tasks.

use crate::metrics::BenchmarkMetrics;
use crate::suite::get_suite_tasks;
use crate::task::{BenchmarkTask, TaskOutcome};
use cortex_core::{CortexError, Result};
use cortex_runtime::model::ModelProvider;
use cortex_runtime::tools::fs::{ListDirTool, ReadFileTool, WriteFileTool};
use cortex_runtime::tools::git::{
    GitBranchTool, GitCommitTool, GitDiffTool, GitLogTool, GitStatusTool,
};
use cortex_runtime::tools::shell::ShellTool;
use cortex_runtime::{AgentContext, AgentLoop, ChatMessage, ToolRegistry, Workspace};
use std::fs;
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

/// Evaluation runner for executing agent benchmarks.
pub struct BenchmarkRunner {
    max_iterations: usize,
}

impl BenchmarkRunner {
    /// Create a new [`BenchmarkRunner`] with the specified iteration cap.
    pub fn new(max_iterations: usize) -> Self {
        Self { max_iterations }
    }

    /// Run an agent against a single benchmark task.
    pub fn run_task(&self, task: &BenchmarkTask, model: &dyn ModelProvider) -> Result<TaskOutcome> {
        let tmp_dir =
            std::env::temp_dir().join(format!("cortex_bench_{}_{}", task.id, std::process::id()));
        if tmp_dir.exists() {
            let _ = fs::remove_dir_all(&tmp_dir);
        }
        fs::create_dir_all(&tmp_dir).map_err(|e| {
            CortexError::Internal(format!("failed to create benchmark workspace: {}", e))
        })?;

        // Initialize git repository in workspace for git tools
        let _ = Command::new("git")
            .args(["init", "-b", "main"])
            .current_dir(&tmp_dir)
            .output();
        let _ = Command::new("git")
            .args(["config", "user.name", "Cortex Benchmark"])
            .current_dir(&tmp_dir)
            .output();
        let _ = Command::new("git")
            .args(["config", "user.email", "bench@cortex.ai"])
            .current_dir(&tmp_dir)
            .output();

        // Populate initial files
        for file in &task.initial_files {
            let target_path = tmp_dir.join(&file.path);
            if let Some(parent) = target_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            fs::write(&target_path, &file.content).map_err(|e| {
                CortexError::Internal(format!("failed to write task file '{}': {}", file.path, e))
            })?;
        }

        // Commit initial task files
        let _ = Command::new("git")
            .args(["add", "-A"])
            .current_dir(&tmp_dir)
            .output();
        let _ = Command::new("git")
            .args(["commit", "-m", "initial benchmark files"])
            .current_dir(&tmp_dir)
            .output();

        let workspace = Arc::new(Workspace::new(&tmp_dir)?);
        let registry = create_standard_registry(&workspace)?;

        let mut context = AgentContext::new(&task.prompt).with_workspace(Arc::clone(&workspace));
        let agent_loop = AgentLoop::new(self.max_iterations);

        let start_time = Instant::now();
        let agent_result = agent_loop.run(&mut context, model, &registry);
        let duration_ms = start_time.elapsed().as_millis() as u64;

        let tool_error_count = context
            .messages
            .iter()
            .filter(|m| matches!(m, ChatMessage::ToolResult { is_error: true, .. }))
            .count();

        let (success, step_count, error_message) = match agent_result {
            Ok(run_res) => {
                // Execute verification command
                let prepared_cmd = prepare_verification_command(&task.verification_command);
                let mut cmd = if cfg!(target_os = "windows") {
                    let mut c = Command::new("cmd");
                    c.args(["/C", &prepared_cmd]);
                    c
                } else {
                    let mut c = Command::new("sh");
                    c.args(["-c", &prepared_cmd]);
                    c
                };
                let output = cmd.current_dir(&tmp_dir).output();

                match output {
                    Ok(out) => {
                        let code = out.status.code().unwrap_or(-1);
                        if code == task.expected_exit_code {
                            (true, run_res.iterations, None)
                        } else {
                            let stderr = String::from_utf8_lossy(&out.stderr);
                            let stdout = String::from_utf8_lossy(&out.stdout);
                            let err = format!(
                                "verification command failed (code {}): {}{}",
                                code, stdout, stderr
                            );
                            (false, run_res.iterations, Some(err))
                        }
                    }
                    Err(e) => (
                        false,
                        run_res.iterations,
                        Some(format!("failed to execute verification command: {}", e)),
                    ),
                }
            }
            Err(e) => (false, context.iterations, Some(e.to_string())),
        };

        // Cleanup temporary directory
        let _ = fs::remove_dir_all(&tmp_dir);

        Ok(TaskOutcome {
            task_id: task.id.clone(),
            task_name: task.name.clone(),
            suite: task.suite.clone(),
            success,
            duration_ms,
            step_count,
            tokens_consumed: 0,
            tool_error_count,
            error_message,
        })
    }

    /// Evaluate all tasks in a named suite and return aggregated metrics.
    pub fn run_suite(
        &self,
        suite_name: &str,
        model: &dyn ModelProvider,
    ) -> Result<BenchmarkMetrics> {
        let tasks = get_suite_tasks(suite_name);
        if tasks.is_empty() {
            return Err(CortexError::NotFound(format!(
                "benchmark suite '{}' not found or contains no tasks",
                suite_name
            )));
        }

        let mut outcomes = Vec::new();
        for task in &tasks {
            let outcome = self.run_task(task, model)?;
            outcomes.push(outcome);
        }

        Ok(BenchmarkMetrics::from_outcomes(suite_name, outcomes))
    }
}

fn create_standard_registry(workspace: &Arc<Workspace>) -> Result<ToolRegistry> {
    let registry = ToolRegistry::new();
    registry.register_tool(ReadFileTool::new(Arc::clone(workspace)))?;
    registry.register_tool(WriteFileTool::new(Arc::clone(workspace)))?;
    registry.register_tool(ListDirTool::new(Arc::clone(workspace)))?;
    registry.register_tool(ShellTool::new(Arc::clone(workspace)))?;
    registry.register_tool(GitStatusTool::new(Arc::clone(workspace)))?;
    registry.register_tool(GitDiffTool::new(Arc::clone(workspace)))?;
    registry.register_tool(GitLogTool::new(Arc::clone(workspace)))?;
    registry.register_tool(GitBranchTool::new(Arc::clone(workspace)))?;
    registry.register_tool(GitCommitTool::new(Arc::clone(workspace)))?;
    Ok(registry)
}

/// Detect the available Python executable name on the host system.
///
/// Probes `python3`, `python`, and `py` in order, returning the first candidate
/// that successfully responds to `--version`.
pub fn detect_python() -> &'static str {
    static PYTHON: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
    PYTHON.get_or_init(|| {
        let candidates = if cfg!(target_os = "windows") {
            &["python", "python3", "py"][..]
        } else {
            &["python3", "python"][..]
        };

        for &candidate in candidates {
            if let Ok(output) = Command::new(candidate).arg("--version").output() {
                if output.status.success() {
                    return candidate;
                }
            }
        }

        if cfg!(target_os = "windows") {
            "python"
        } else {
            "python3"
        }
    })
}

/// Prepare a verification command for execution on the host operating system.
///
/// On Windows:
/// - Adapts `python3` invocations to the detected Python binary (`python` or `py`).
/// - Replaces POSIX `/tmp/calc_test` references with `calc_test.exe`.
pub fn prepare_verification_command(command: &str) -> String {
    let mut cmd = command.to_string();
    if cfg!(target_os = "windows") {
        let py = detect_python();
        if py != "python3" {
            if let Some(rest) = cmd.strip_prefix("python3 ") {
                cmd = format!("{} {}", py, rest);
            } else if cmd == "python3" {
                cmd = py.to_string();
            }

            if cmd.contains("&& python3 ") {
                cmd = cmd.replace("&& python3 ", &format!("&& {} ", py));
            }
            if cmd.contains("; python3 ") {
                cmd = cmd.replace("; python3 ", &format!("; {} ", py));
            }
        }

        if cmd.contains("/tmp/calc_test") {
            cmd = cmd.replace("/tmp/calc_test", "calc_test.exe");
        }
    }
    cmd
}
