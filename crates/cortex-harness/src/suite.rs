//! Curated benchmark suites and ground-truth task definitions.

use crate::task::BenchmarkTask;
use cortex_core::Result;
use cortex_runtime::model::{ModelDescriptor, ModelOutput, ModelProvider, ToolCall};
use cortex_runtime::AgentContext;
use serde_json::json;
use std::sync::OnceLock;

/// Reference baseline model provider with ground-truth solutions for benchmark tasks.
pub struct BenchmarkBaselineProvider;

impl ModelProvider for BenchmarkBaselineProvider {
    fn descriptor(&self) -> &ModelDescriptor {
        static DESC: OnceLock<ModelDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ModelDescriptor::new("cortex", "benchmark-baseline-v1"))
    }

    fn is_configured(&self) -> Result<bool> {
        Ok(true)
    }

    fn generate(&self, context: &AgentContext) -> Result<ModelOutput> {
        if context.iterations <= 1 {
            if context.task.contains("src/lib.rs") {
                return Ok(ModelOutput::ToolCalls(vec![ToolCall::new(
                    "fix_syntax",
                    "write_file",
                    json!({
                        "path": "src/lib.rs",
                        "content": "pub fn greet(name: &str) -> String {\n    format!(\"Hello, {}!\", name)\n}\n"
                    }),
                )]));
            }
            if context.task.contains("search.py") {
                return Ok(ModelOutput::ToolCalls(vec![ToolCall::new(
                    "fix_boundary",
                    "write_file",
                    json!({
                        "path": "search.py",
                        "content": "def binary_search(arr, target):\n    low = 0\n    high = len(arr) - 1\n    while low <= high:\n        mid = (low + high) // 2\n        if arr[mid] == target:\n            return mid\n        elif arr[mid] < target:\n            low = mid + 1\n        else:\n            high = mid - 1\n    return -1\n"
                    }),
                )]));
            }
            if context.task.contains("src/calc.rs") {
                return Ok(ModelOutput::ToolCalls(vec![ToolCall::new(
                    "fix_logic",
                    "write_file",
                    json!({
                        "path": "src/calc.rs",
                        "content": "pub fn add(a: i32, b: i32) -> i32 {\n    a + b\n}\n\n#[test]\nfn test_add() {\n    assert_eq!(add(2, 3), 5);\n}\n"
                    }),
                )]));
            }
            if context.task.contains("geometry.py") {
                return Ok(ModelOutput::ToolCalls(vec![ToolCall::new(
                    "fix_import",
                    "write_file",
                    json!({
                        "path": "geometry.py",
                        "content": "import math\n\ndef hypotenuse(a, b):\n    return math.sqrt(a * a + b * b)\n"
                    }),
                )]));
            }
            if context.task.contains("parser.py") {
                return Ok(ModelOutput::ToolCalls(vec![ToolCall::new(
                    "fix_key_error",
                    "write_file",
                    json!({
                        "path": "parser.py",
                        "content": "def parse_event(data):\n    meta = data.get('meta', {})\n    return {'user': data['user'], 'tags': meta.get('tags', [])}\n"
                    }),
                )]));
            }
            if context.task.contains("auth.py") {
                return Ok(ModelOutput::ToolCalls(vec![ToolCall::new(
                    "fix_auth",
                    "write_file",
                    json!({
                        "path": "auth.py",
                        "content": "def register_user(email, password):\n    if '@' not in email or '.' not in email:\n        return False\n    return len(password) >= 8\n"
                    }),
                )]));
            }
            if context.task.contains("cli.py") {
                return Ok(ModelOutput::ToolCalls(vec![ToolCall::new(
                    "fix_cli",
                    "write_file",
                    json!({
                        "path": "cli.py",
                        "content": "import sys\ndef parse_args(args):\n    verbose = '--verbose' in args\n    return {'verbose': verbose}\n"
                    }),
                )]));
            }
        }

        Ok(ModelOutput::FinalAnswer(
            "Task completed and verified".to_string(),
        ))
    }
}

/// Return all tasks configured for the specified benchmark suite.
pub fn get_suite_tasks(suite_name: &str) -> Vec<BenchmarkTask> {
    match suite_name {
        "coding" => coding_benchmark_suite(),
        "refactor" => refactor_benchmark_suite(),
        "cli" => cli_benchmark_suite(),
        _ => Vec::new(),
    }
}

/// Return all known benchmark suite names.
pub fn available_suites() -> Vec<&'static str> {
    vec!["coding", "refactor", "cli"]
}

fn coding_benchmark_suite() -> Vec<BenchmarkTask> {
    vec![
        // Task 1: Rust Syntax Repair
        BenchmarkTask::new(
            "coding-01-rust-syntax",
            "Rust Syntax Error Repair",
            "coding",
            "Fix missing semicolon and mismatched closing delimiter in Rust module",
            "Fix the syntax error in src/lib.rs so it compiles cleanly.",
            "rustc --crate-type lib src/lib.rs",
        )
        .with_file(
            "src/lib.rs",
            "pub fn greet(name: &str) -> String {\n    format!(\"Hello, {}!\", name\n}\n",
        ),

        // Task 2: Python Boundary Repair
        BenchmarkTask::new(
            "coding-02-python-boundary",
            "Python Boundary Off-by-One Repair",
            "coding",
            "Fix off-by-one boundary bug in binary search implementation",
            "Fix the boundary condition in binary_search in search.py to pass test_search.py.",
            "python3 test_search.py",
        )
        .with_file(
            "search.py",
            "def binary_search(arr, target):\n    low = 0\n    high = len(arr) - 2  # bug: off by one\n    while low <= high:\n        mid = (low + high) // 2\n        if arr[mid] == target:\n            return mid\n        elif arr[mid] < target:\n            low = mid + 1\n        else:\n            high = mid - 1\n    return -1\n",
        )
        .with_file(
            "test_search.py",
            "import sys\nfrom search import binary_search\n\narr = [1, 3, 5, 7, 9]\nassert binary_search(arr, 9) == 4, 'Failed to find last element'\nassert binary_search(arr, 1) == 0, 'Failed to find first element'\nassert binary_search(arr, 6) == -1, 'Failed on missing element'\nprint('All tests passed.')\n",
        ),

        // Task 3: Rust Logic Repair
        BenchmarkTask::new(
            "coding-03-rust-logic",
            "Rust Logic Inversion Repair",
            "coding",
            "Correct arithmetic inversion in calculation helper",
            "Fix the logic bug in src/calc.rs so test suite passes.",
            "rustc --test src/calc.rs -o /tmp/calc_test && /tmp/calc_test",
        )
        .with_file(
            "src/calc.rs",
            "pub fn add(a: i32, b: i32) -> i32 {\n    a - b // bug: subtraction instead of addition\n}\n\n#[test]\nfn test_add() {\n    assert_eq!(add(2, 3), 5);\n}\n",
        ),

        // Task 4: Python Missing Import Repair
        BenchmarkTask::new(
            "coding-04-python-import",
            "Python Missing Import Repair",
            "coding",
            "Add missing standard library import in geometry helper",
            "Fix the undefined name error in geometry.py so test_geometry.py passes.",
            "python3 test_geometry.py",
        )
        .with_file(
            "geometry.py",
            "# missing: import math\ndef hypotenuse(a, b):\n    return math.sqrt(a * a + b * b)\n",
        )
        .with_file(
            "test_geometry.py",
            "from geometry import hypotenuse\nassert abs(hypotenuse(3, 4) - 5.0) < 1e-6\nprint('Geometry tests passed.')\n",
        ),

        // Task 5: Python KeyError Exception Repair
        BenchmarkTask::new(
            "coding-05-python-exception",
            "Python KeyError Exception Repair",
            "coding",
            "Safely handle optional dictionary key in event parser",
            "Fix the KeyError exception when parsing payload without 'meta' in parser.py.",
            "python3 test_parser.py",
        )
        .with_file(
            "parser.py",
            "def parse_event(data):\n    # bug: direct access instead of get\n    meta = data['meta']\n    return {'user': data['user'], 'tags': meta.get('tags', [])}\n",
        )
        .with_file(
            "test_parser.py",
            "from parser import parse_event\nres = parse_event({'user': 'alice'})\nassert res['user'] == 'alice'\nassert res['tags'] == []\nprint('Parser tests passed.')\n",
        ),
    ]
}

fn refactor_benchmark_suite() -> Vec<BenchmarkTask> {
    vec![
        // Multi-step refactoring task 1: Extract helper
        BenchmarkTask::new(
            "refactor-01-extract-helper",
            "Extract Helper Function",
            "refactor",
            "Extract duplicated email validation regex into dedicated helper module",
            "Refactor auth.py to use validate_email from validator.py.",
            "python3 test_auth.py",
        )
        .with_file(
            "auth.py",
            "def register_user(email, password):\n    if '@' not in email or '.' not in email:\n        return False\n    return len(password) >= 8\n",
        )
        .with_file(
            "test_auth.py",
            "from auth import register_user\nassert register_user('test@example.com', 'secret123') is True\nassert register_user('invalid', 'secret123') is False\nprint('Auth tests passed.')\n",
        ),
    ]
}

fn cli_benchmark_suite() -> Vec<BenchmarkTask> {
    vec![
        // CLI Benchmark task 1: Flag parsing
        BenchmarkTask::new(
            "cli-01-flag-handling",
            "CLI Verbose Flag Handling",
            "cli",
            "Support --verbose flag in argument processor",
            "Update cli.py to parse --verbose flag correctly.",
            "python3 test_cli.py",
        )
        .with_file(
            "cli.py",
            "import sys\ndef parse_args(args):\n    verbose = '--verbose' in args\n    return {'verbose': verbose}\n",
        )
        .with_file(
            "test_cli.py",
            "from cli import parse_args\nassert parse_args(['--verbose'])['verbose'] is True\nassert parse_args([])['verbose'] is False\nprint('CLI tests passed.')\n",
        ),
    ]
}
