//! Integration tests verifying the Cortex Coding Agent inside an intentionally broken fixture repository.

use cortex_core::CortexError;
use cortex_runtime::tools::fs::{ListDirTool, ReadFileTool, WriteFileTool};
use cortex_runtime::tools::git::{
    GitBranchTool, GitCommitTool, GitDiffTool, GitLogTool, GitPushTool, GitStatusTool,
};
use cortex_runtime::tools::shell::ShellTool;
use cortex_runtime::{
    AgentContext, AgentLoop, ChatMessage, MockModelProvider, ModelOutput, ToolCall, ToolRegistry,
    Workspace,
};
use serde_json::json;
use std::fs;
use std::process::Command;
use std::sync::Arc;

fn setup_broken_fixture_repo() -> std::path::PathBuf {
    let repo_dir = std::env::temp_dir().join(format!("cortex_fixture_test_{}", std::process::id()));
    if repo_dir.exists() {
        let _ = fs::remove_dir_all(&repo_dir);
    }
    fs::create_dir_all(&repo_dir).unwrap();

    // Initialize git repository
    let run_git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(&repo_dir)
            .status()
            .expect("Failed to execute git");
        assert!(status.success(), "git command failed: {:?}", args);
    };

    run_git(&["init", "-b", "main"]);
    run_git(&["config", "user.name", "Cortex Test Agent"]);
    run_git(&["config", "user.email", "agent@cortex.ai"]);

    // Create broken calculator.py
    fs::write(
        repo_dir.join("calculator.py"),
        "def add(a, b):\n    return a - b  # Bug: subtraction instead of addition\n",
    )
    .unwrap();

    // Create test_calculator.py
    fs::write(
        repo_dir.join("test_calculator.py"),
        "from calculator import add\nassert add(2, 2) == 4, '2 + 2 must equal 4'\nprint('TEST_PASSED')\n",
    )
    .unwrap();

    // Create unrelated file that must remain untouched
    fs::write(
        repo_dir.join("unrelated.txt"),
        "CRITICAL_SYSTEM_CONFIG=DO_NOT_MODIFY\n",
    )
    .unwrap();

    run_git(&["add", "-A"]);
    run_git(&["commit", "-m", "initial broken repository state"]);

    repo_dir
}

#[test]
fn test_coding_agent_fixture_repair_workflow() {
    let repo_dir = setup_broken_fixture_repo();
    let workspace = Arc::new(Workspace::new(&repo_dir).unwrap());

    // Register all Phase 2 tools
    let registry = ToolRegistry::new();
    registry
        .register_tool(ReadFileTool::new(Arc::clone(&workspace)))
        .unwrap();
    registry
        .register_tool(WriteFileTool::new(Arc::clone(&workspace)))
        .unwrap();
    registry
        .register_tool(ListDirTool::new(Arc::clone(&workspace)))
        .unwrap();
    registry
        .register_tool(ShellTool::new(Arc::clone(&workspace)))
        .unwrap();
    registry
        .register_tool(GitStatusTool::new(Arc::clone(&workspace)))
        .unwrap();
    registry
        .register_tool(GitDiffTool::new(Arc::clone(&workspace)))
        .unwrap();
    registry
        .register_tool(GitLogTool::new(Arc::clone(&workspace)))
        .unwrap();
    registry
        .register_tool(GitBranchTool::new(Arc::clone(&workspace)))
        .unwrap();
    registry
        .register_tool(GitCommitTool::new(Arc::clone(&workspace)))
        .unwrap();
    registry.register_tool(GitPushTool::new()).unwrap();

    // Setup scripted model simulating an autonomous coding agent
    let model = MockModelProvider::new();

    // Step 1: Agent inspects directory contents
    model.queue_response(ModelOutput::ToolCalls(vec![ToolCall::new(
        "call_1",
        "list_directory",
        json!({ "path": "." }),
    )]));

    // Step 2: Agent runs test runner and observes failure
    model.queue_response(ModelOutput::ToolCalls(vec![ToolCall::new(
        "call_2",
        "shell",
        json!({ "command": "python3 test_calculator.py" }),
    )]));

    // Step 3: Agent reads the broken file to understand the bug
    model.queue_response(ModelOutput::ToolCalls(vec![ToolCall::new(
        "call_3",
        "read_file",
        json!({ "path": "calculator.py" }),
    )]));

    // Step 4: Agent writes the minimal fix
    model.queue_response(ModelOutput::ToolCalls(vec![ToolCall::new(
        "call_4",
        "write_file",
        json!({
            "path": "calculator.py",
            "content": "def add(a, b):\n    return a + b\n"
        }),
    )]));

    // Step 5: Agent re-runs tests to verify fix
    model.queue_response(ModelOutput::ToolCalls(vec![ToolCall::new(
        "call_5",
        "shell",
        json!({ "command": "python3 test_calculator.py" }),
    )]));

    // Step 6: Agent inspects git diff
    model.queue_response(ModelOutput::ToolCalls(vec![ToolCall::new(
        "call_6",
        "git_diff",
        json!({}),
    )]));

    // Step 7: Agent creates branch and commits fix
    model.queue_response(ModelOutput::ToolCalls(vec![
        ToolCall::new(
            "call_7a",
            "git_branch",
            json!({ "name": "fix/add-bug", "create": true }),
        ),
        ToolCall::new(
            "call_7b",
            "git_commit",
            json!({ "message": "fix: correct addition operator in calculator" }),
        ),
    ]));

    // Step 8: Agent tries git_push (must be blocked by policy)
    model.queue_response(ModelOutput::ToolCalls(vec![ToolCall::new(
        "call_8",
        "git_push",
        json!({}),
    )]));

    // Step 9: Agent concludes run with verified report
    model.queue_response(ModelOutput::FinalAnswer(
        "Repaired calculator.py, verified all tests pass, and committed changes to fix/add-bug branch."
            .to_string(),
    ));

    let mut context =
        AgentContext::new("Fix broken tests in repository").with_workspace(Arc::clone(&workspace));

    let agent_loop = AgentLoop::new(15);
    let run_result = agent_loop.run(&mut context, &model, &registry).unwrap();

    assert!(run_result.completed);
    assert!(run_result.final_answer.contains("Repaired calculator.py"));

    // Verify calculator.py content was fixed
    let fixed_content = fs::read_to_string(repo_dir.join("calculator.py")).unwrap();
    assert_eq!(fixed_content, "def add(a, b):\n    return a + b\n");

    // Verify unrelated file was NOT touched
    let unrelated = fs::read_to_string(repo_dir.join("unrelated.txt")).unwrap();
    assert_eq!(unrelated, "CRITICAL_SYSTEM_CONFIG=DO_NOT_MODIFY\n");

    // Verify git commit was created
    let log_out = Command::new("git")
        .args(["log", "-n1", "--oneline"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    let log_str = String::from_utf8_lossy(&log_out.stdout);
    assert!(log_str.contains("fix: correct addition operator"));

    // Verify git branch is fix/add-bug
    let branch_out = Command::new("git")
        .args(["branch", "--show-current"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    let branch_str = String::from_utf8_lossy(&branch_out.stdout);
    assert_eq!(branch_str.trim(), "fix/add-bug");

    // Verify message history captured both failure and success
    let mut saw_test_failure = false;
    let mut saw_test_pass = false;
    let mut saw_push_denial = false;

    for msg in &context.messages {
        if let ChatMessage::ToolResult {
            output, is_error, ..
        } = msg
        {
            if *is_error && output.contains("2 + 2 must equal 4") {
                saw_test_failure = true;
            }
            if !*is_error && output.contains("TEST_PASSED") {
                saw_test_pass = true;
            }
            if *is_error && output.contains("git push is strictly disabled") {
                saw_push_denial = true;
            }
        }
    }

    assert!(
        saw_test_failure,
        "Agent loop should record initial test failure"
    );
    assert!(
        saw_test_pass,
        "Agent loop should record subsequent test pass"
    );
    assert!(
        saw_push_denial,
        "Agent loop should record git push permission denial"
    );

    // Verify workspace boundary enforcement prevents directory traversal
    let escape_err = workspace.resolve_path("../../etc/shadow").unwrap_err();
    match escape_err {
        CortexError::PermissionDenied(_) => {}
        _ => panic!("Expected PermissionDenied on workspace escape attempt"),
    }

    // Clean up temporary fixture
    let _ = fs::remove_dir_all(&repo_dir);
}
