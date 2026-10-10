//! Search tools for workspace files, code symbols, glob patterns, and text inspection.

use crate::agent::CancellationToken;
use crate::tool::{PermissionLevel, Tool, ToolDefinition, ToolResult};
use crate::workspace::Workspace;
use cortex_core::{CortexError, Result};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Ignored directory names during workspace traversal.
const IGNORED_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "dist",
    "build",
    ".idea",
    ".vscode",
    ".next",
    "__pycache__",
];

/// Helper to recursively collect files within a directory, pruning ignored directories.
fn check_cancelled(token: Option<&CancellationToken>) -> Result<()> {
    if token.is_some_and(CancellationToken::is_cancelled) {
        return Err(CortexError::Cancelled(
            "workspace search cancelled by request".to_string(),
        ));
    }
    Ok(())
}

fn walk_dir_pruned(
    current: &Path,
    files: &mut Vec<PathBuf>,
    token: Option<&CancellationToken>,
) -> Result<()> {
    check_cancelled(token)?;
    if !current.exists() {
        return Ok(());
    }

    let entries = fs::read_dir(current).map_err(|e| {
        CortexError::Internal(format!(
            "failed to read directory '{}': {}",
            current.display(),
            e
        ))
    })?;

    for entry in entries {
        check_cancelled(token)?;
        let entry = entry.map_err(|e| CortexError::Internal(e.to_string()))?;
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            if !IGNORED_DIRS.contains(&file_name.as_str()) && !file_name.starts_with('.') {
                walk_dir_pruned(&path, files, token)?;
            }
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

/// Helper to check if a file appears to be binary.
fn is_binary_file(path: &Path) -> bool {
    if let Ok(mut file) = fs::File::open(path) {
        use std::io::Read;
        let mut buffer = [0u8; 1024];
        if let Ok(n) = file.read(&mut buffer) {
            return buffer[..n].contains(&0);
        }
    }
    false
}

/// Tool to search text patterns or regular expressions across workspace files.
pub struct GrepTool {
    workspace: Arc<Workspace>,
    def: ToolDefinition,
}

impl GrepTool {
    /// Create a new [`GrepTool`].
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            workspace,
            def: ToolDefinition::new(
                "grep",
                "Searches for text pattern or regex across files in workspace",
                json!({
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string" },
                        "path": { "type": "string" },
                        "case_sensitive": { "type": "boolean" },
                        "max_results": { "type": "integer" }
                    },
                    "required": ["pattern"]
                }),
            )
            .with_permission(PermissionLevel::ReadOnly),
        }
    }
}

impl Tool for GrepTool {
    fn definition(&self) -> &ToolDefinition {
        &self.def
    }

    fn execute(&self, input: &serde_json::Value) -> Result<ToolResult> {
        self.execute_with_cancellation(input, None)
    }

    fn execute_with_cancellation(
        &self,
        input: &serde_json::Value,
        token: Option<&CancellationToken>,
    ) -> Result<ToolResult> {
        check_cancelled(token)?;
        let pattern = input["pattern"].as_str().ok_or_else(|| {
            CortexError::Validation("missing required 'pattern' parameter".to_string())
        })?;
        let rel_path = input.get("path").and_then(|p| p.as_str()).unwrap_or(".");
        let case_sensitive = input
            .get("case_sensitive")
            .and_then(|c| c.as_bool())
            .unwrap_or(true);
        let max_results = input
            .get("max_results")
            .and_then(|m| m.as_i64())
            .unwrap_or(100)
            .max(1) as usize;

        let start_dir = self.workspace.resolve_path(rel_path)?;
        if !start_dir.exists() {
            return Err(CortexError::NotFound(format!(
                "path not found: '{}'",
                rel_path
            )));
        }

        let mut files = Vec::new();
        if start_dir.is_file() {
            files.push(start_dir.clone());
        } else {
            walk_dir_pruned(&start_dir, &mut files, token)?;
        }

        let query = if case_sensitive {
            pattern.to_string()
        } else {
            pattern.to_lowercase()
        };

        let mut matches = Vec::new();
        for file in files {
            check_cancelled(token)?;
            if is_binary_file(&file) {
                continue;
            }

            if let Ok(content) = fs::read_to_string(&file) {
                let display_path = file
                    .strip_prefix(self.workspace.root())
                    .unwrap_or(&file)
                    .to_string_lossy()
                    .replace('\\', "/");

                for (idx, line) in content.lines().enumerate() {
                    check_cancelled(token)?;
                    let matched = if case_sensitive {
                        line.contains(&query)
                    } else {
                        line.to_lowercase().contains(&query)
                    };

                    if matched {
                        matches.push(format!("{}:{}: {}", display_path, idx + 1, line.trim()));
                        if matches.len() >= max_results {
                            break;
                        }
                    }
                }
            }

            if matches.len() >= max_results {
                break;
            }
        }

        if matches.is_empty() {
            Ok(ToolResult::success(format!(
                "No matches found for pattern '{}' in '{}'",
                pattern, rel_path
            )))
        } else {
            Ok(ToolResult::success(matches.join("\n")))
        }
    }
}

/// Tool to match files using glob patterns.
pub struct GlobTool {
    workspace: Arc<Workspace>,
    def: ToolDefinition,
}

impl GlobTool {
    /// Create a new [`GlobTool`].
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            workspace,
            def: ToolDefinition::new(
                "glob",
                "Finds files matching a glob pattern (e.g. '**/*.rs', 'src/**/*.ts') within workspace",
                json!({
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string" },
                        "path": { "type": "string" }
                    },
                    "required": ["pattern"]
                }),
            )
            .with_permission(PermissionLevel::ReadOnly),
        }
    }
}

/// Match simple glob pattern against path string.
fn matches_glob(pattern: &str, path_str: &str) -> bool {
    if pattern == "**" || pattern == "*" {
        return true;
    }

    if let Some(suffix) = pattern.strip_prefix("*.") {
        return path_str.ends_with(&format!(".{}", suffix));
    }

    if let Some(suffix) = pattern.strip_prefix("**/*.") {
        return path_str.ends_with(&format!(".{}", suffix));
    }

    if let Some(prefix) = pattern.strip_suffix("/**") {
        return path_str.starts_with(prefix);
    }

    if pattern.contains('*') {
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            let (head, tail) = (parts[0], parts[1]);
            return path_str.starts_with(head) && path_str.ends_with(tail);
        }
    }

    path_str.contains(pattern) || path_str == pattern
}

impl Tool for GlobTool {
    fn definition(&self) -> &ToolDefinition {
        &self.def
    }

    fn execute(&self, input: &serde_json::Value) -> Result<ToolResult> {
        self.execute_with_cancellation(input, None)
    }

    fn execute_with_cancellation(
        &self,
        input: &serde_json::Value,
        token: Option<&CancellationToken>,
    ) -> Result<ToolResult> {
        check_cancelled(token)?;
        let pattern = input["pattern"].as_str().ok_or_else(|| {
            CortexError::Validation("missing required 'pattern' parameter".to_string())
        })?;
        let rel_path = input.get("path").and_then(|p| p.as_str()).unwrap_or(".");
        let start_dir = self.workspace.resolve_path(rel_path)?;

        if !start_dir.exists() {
            return Err(CortexError::NotFound(format!(
                "path not found: '{}'",
                rel_path
            )));
        }

        let mut files = Vec::new();
        walk_dir_pruned(&start_dir, &mut files, token)?;

        let mut matched = Vec::new();
        for file in files {
            check_cancelled(token)?;
            let rel = file
                .strip_prefix(self.workspace.root())
                .unwrap_or(&file)
                .to_string_lossy()
                .replace('\\', "/");

            if matches_glob(pattern, &rel) {
                matched.push(rel);
            }
        }

        matched.sort();
        if matched.is_empty() {
            Ok(ToolResult::success(format!(
                "No files matched glob pattern '{}'",
                pattern
            )))
        } else {
            Ok(ToolResult::success(matched.join("\n")))
        }
    }
}

/// Tool to find files or directories by name within workspace bounds.
pub struct FindTool {
    workspace: Arc<Workspace>,
    def: ToolDefinition,
}

impl FindTool {
    /// Create a new [`FindTool`].
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            workspace,
            def: ToolDefinition::new(
                "find",
                "Finds files or directories by name substring within workspace",
                json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "path": { "type": "string" },
                        "kind": { "type": "string", "enum": ["all", "file", "directory"] }
                    },
                    "required": ["name"]
                }),
            )
            .with_permission(PermissionLevel::ReadOnly),
        }
    }
}

impl Tool for FindTool {
    fn definition(&self) -> &ToolDefinition {
        &self.def
    }

    fn execute(&self, input: &serde_json::Value) -> Result<ToolResult> {
        self.execute_with_cancellation(input, None)
    }

    fn execute_with_cancellation(
        &self,
        input: &serde_json::Value,
        token: Option<&CancellationToken>,
    ) -> Result<ToolResult> {
        check_cancelled(token)?;
        let name_query = input["name"].as_str().ok_or_else(|| {
            CortexError::Validation("missing required 'name' parameter".to_string())
        })?;
        let rel_path = input.get("path").and_then(|p| p.as_str()).unwrap_or(".");
        let kind = input.get("kind").and_then(|k| k.as_str()).unwrap_or("all");

        let start_dir = self.workspace.resolve_path(rel_path)?;
        if !start_dir.exists() {
            return Err(CortexError::NotFound(format!(
                "path not found: '{}'",
                rel_path
            )));
        }

        let mut matches = Vec::new();
        let query_lower = name_query.to_lowercase();

        fn walk_find(
            root: &Path,
            current: &Path,
            query: &str,
            kind: &str,
            matches: &mut Vec<String>,
            token: Option<&CancellationToken>,
        ) -> Result<()> {
            check_cancelled(token)?;
            if !current.exists() {
                return Ok(());
            }
            let entries = fs::read_dir(current).map_err(|e| {
                CortexError::Internal(format!("failed to read '{}': {}", current.display(), e))
            })?;

            for entry in entries {
                check_cancelled(token)?;
                let entry = entry.map_err(|e| CortexError::Internal(e.to_string()))?;
                let path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_string();

                if IGNORED_DIRS.contains(&file_name.as_str()) || file_name.starts_with('.') {
                    continue;
                }

                let is_dir = path.is_dir();
                let name_matches = file_name.to_lowercase().contains(query);

                let rel_str = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");

                if name_matches {
                    match kind {
                        "file" if !is_dir => matches.push(rel_str.clone()),
                        "directory" if is_dir => matches.push(format!("{}/", rel_str)),
                        "all" => {
                            if is_dir {
                                matches.push(format!("{}/", rel_str));
                            } else {
                                matches.push(rel_str.clone());
                            }
                        }
                        _ => {}
                    }
                }

                if is_dir {
                    walk_find(root, &path, query, kind, matches, token)?;
                }
            }
            Ok(())
        }

        walk_find(
            self.workspace.root(),
            &start_dir,
            &query_lower,
            kind,
            &mut matches,
            token,
        )?;

        matches.sort();
        if matches.is_empty() {
            Ok(ToolResult::success(format!(
                "No entries found matching '{}'",
                name_query
            )))
        } else {
            Ok(ToolResult::success(matches.join("\n")))
        }
    }
}

/// Tool for searching code symbols, function declarations, and definitions.
pub struct SearchCodeTool {
    workspace: Arc<Workspace>,
    def: ToolDefinition,
}

impl SearchCodeTool {
    /// Create a new [`SearchCodeTool`].
    pub fn new(workspace: Arc<Workspace>) -> Self {
        Self {
            workspace,
            def: ToolDefinition::new(
                "search_code",
                "Searches source code files for symbols, function definitions, or declarations with context",
                json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string" },
                        "language": { "type": "string" },
                        "path": { "type": "string" }
                    },
                    "required": ["query"]
                }),
            )
            .with_permission(PermissionLevel::ReadOnly),
        }
    }
}

impl Tool for SearchCodeTool {
    fn definition(&self) -> &ToolDefinition {
        &self.def
    }

    fn execute(&self, input: &serde_json::Value) -> Result<ToolResult> {
        self.execute_with_cancellation(input, None)
    }

    fn execute_with_cancellation(
        &self,
        input: &serde_json::Value,
        token: Option<&CancellationToken>,
    ) -> Result<ToolResult> {
        check_cancelled(token)?;
        let query = input["query"].as_str().ok_or_else(|| {
            CortexError::Validation("missing required 'query' parameter".to_string())
        })?;
        let language = input.get("language").and_then(|l| l.as_str());
        let rel_path = input.get("path").and_then(|p| p.as_str()).unwrap_or(".");

        let start_dir = self.workspace.resolve_path(rel_path)?;
        if !start_dir.exists() {
            return Err(CortexError::NotFound(format!(
                "path not found: '{}'",
                rel_path
            )));
        }

        let allowed_extensions: Option<Vec<&str>> =
            language.map(|lang| match lang.to_lowercase().as_str() {
                "rust" | "rs" => vec!["rs"],
                "python" | "py" => vec!["py"],
                "typescript" | "ts" => vec!["ts", "tsx"],
                "javascript" | "js" => vec!["js", "jsx"],
                "go" => vec!["go"],
                "c" => vec!["c", "h"],
                "cpp" | "c++" => vec!["cpp", "hpp", "cc", "h"],
                _ => vec![],
            });

        let mut files = Vec::new();
        walk_dir_pruned(&start_dir, &mut files, token)?;

        let mut results = Vec::new();
        for file in files {
            check_cancelled(token)?;
            if let Some(exts) = &allowed_extensions {
                let ext = file.extension().and_then(|e| e.to_str()).unwrap_or("");
                if !exts.contains(&ext) {
                    continue;
                }
            }

            if is_binary_file(&file) {
                continue;
            }

            if let Ok(content) = fs::read_to_string(&file) {
                let lines: Vec<&str> = content.lines().collect();
                let rel = file
                    .strip_prefix(self.workspace.root())
                    .unwrap_or(&file)
                    .to_string_lossy()
                    .replace('\\', "/");

                for (idx, line) in lines.iter().enumerate() {
                    check_cancelled(token)?;
                    if line.contains(query) {
                        let start = idx.saturating_sub(1);
                        let end = (idx + 2).min(lines.len());

                        let mut snippet = format!("--- {}:{} ---\n", rel, idx + 1);
                        for (i, line_item) in lines.iter().enumerate().take(end).skip(start) {
                            let prefix = if i == idx { ">" } else { " " };
                            snippet.push_str(&format!("{} {:4} | {}\n", prefix, i + 1, line_item));
                        }
                        results.push(snippet);

                        if results.len() >= 25 {
                            break;
                        }
                    }
                }
            }

            if results.len() >= 25 {
                break;
            }
        }

        if results.is_empty() {
            Ok(ToolResult::success(format!(
                "No code symbols found matching '{}'",
                query
            )))
        } else {
            Ok(ToolResult::success(results.join("\n")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_searches_stop_before_traversal() {
        let root = std::env::temp_dir().join(cortex_core::RunId::generate().as_str());
        let workspace = Arc::new(Workspace::new(&root).unwrap());
        let token = CancellationToken::new();
        token.cancel();
        let searches: Vec<Box<dyn Tool>> = vec![
            Box::new(GrepTool::new(workspace.clone())),
            Box::new(GlobTool::new(workspace.clone())),
            Box::new(FindTool::new(workspace.clone())),
            Box::new(SearchCodeTool::new(workspace)),
        ];
        let input = json!({"pattern": "*", "name": "file", "query": "symbol"});
        for tool in searches {
            assert!(matches!(
                tool.execute_with_cancellation(&input, Some(&token)),
                Err(CortexError::Cancelled(_))
            ));
        }
        assert!(matches!(
            walk_dir_pruned(&root, &mut Vec::new(), Some(&token)),
            Err(CortexError::Cancelled(_))
        ));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn test_search_tools_lifecycle() {
        let temp_dir =
            std::env::temp_dir().join(format!("cortex_test_search_{}", std::process::id()));
        let ws = Arc::new(Workspace::new(&temp_dir).unwrap());

        // Create sample structure
        let src_dir = temp_dir.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(
            src_dir.join("main.rs"),
            "fn main() {\n    println!(\"Hello World\");\n}\n",
        )
        .unwrap();
        fs::write(
            src_dir.join("util.rs"),
            "pub fn calculate(x: i32) -> i32 {\n    x * 2\n}\n",
        )
        .unwrap();

        // 1. Grep
        let grep = GrepTool::new(Arc::clone(&ws));
        let grep_res = grep
            .execute(&json!({
                "pattern": "calculate"
            }))
            .unwrap();
        assert!(!grep_res.is_error);
        assert!(grep_res.output.contains("src/util.rs:1: pub fn calculate"));

        // 2. Glob
        let glob = GlobTool::new(Arc::clone(&ws));
        let glob_res = glob
            .execute(&json!({
                "pattern": "*.rs"
            }))
            .unwrap();
        assert!(!glob_res.is_error);
        assert!(glob_res.output.contains("src/main.rs"));
        assert!(glob_res.output.contains("src/util.rs"));

        // 3. Find
        let find = FindTool::new(Arc::clone(&ws));
        let find_res = find
            .execute(&json!({
                "name": "util"
            }))
            .unwrap();
        assert!(!find_res.is_error);
        assert!(find_res.output.contains("src/util.rs"));

        // 4. Search Code
        let search_code = SearchCodeTool::new(Arc::clone(&ws));
        let code_res = search_code
            .execute(&json!({
                "query": "println",
                "language": "rust"
            }))
            .unwrap();
        assert!(!code_res.is_error);
        assert!(code_res.output.contains("src/main.rs:2"));

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
