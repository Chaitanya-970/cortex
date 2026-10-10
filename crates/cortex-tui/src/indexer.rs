//! Background workspace indexing for fast file querying and diagnostics.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;

/// Status and metadata of the workspace file index.
#[derive(Debug, Clone, Default)]
pub struct WorkspaceIndex {
    /// Total count of indexed workspace files.
    pub total_files: usize,
    /// Whether indexing has finished initial scan.
    pub is_ready: bool,
    /// Relative paths of indexed files.
    pub files: Vec<String>,
}

/// Handle for background file indexing.
#[derive(Clone)]
pub struct BackgroundIndexer {
    index: Arc<RwLock<WorkspaceIndex>>,
    cancelled: Arc<AtomicBool>,
}

impl Default for BackgroundIndexer {
    fn default() -> Self {
        Self::new()
    }
}

impl BackgroundIndexer {
    /// Create a new background indexer.
    pub fn new() -> Self {
        Self {
            index: Arc::new(RwLock::new(WorkspaceIndex::default())),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Read access to current index snapshot.
    pub fn read(&self) -> WorkspaceIndex {
        self.index.read().map(|idx| idx.clone()).unwrap_or_default()
    }

    /// Spawn a background thread to index files under `root_dir`.
    pub fn start_indexing(&self, root_dir: PathBuf) {
        let index_arc = self.index.clone();
        let cancel_flag = self.cancelled.clone();

        thread::spawn(move || {
            let mut collected = Vec::new();
            scan_dir(&root_dir, &root_dir, &mut collected, &cancel_flag);

            if !cancel_flag.load(Ordering::Relaxed) {
                if let Ok(mut lock) = index_arc.write() {
                    lock.total_files = collected.len();
                    lock.files = collected;
                    lock.is_ready = true;
                }
            }
        });
    }

    /// Signal cancellation for ongoing indexing.
    pub fn stop(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    /// Check if cancellation has been requested for this indexer.
    pub fn is_stopped(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

fn scan_dir(root: &Path, current: &Path, collected: &mut Vec<String>, cancelled: &AtomicBool) {
    if cancelled.load(Ordering::Relaxed) {
        return;
    }

    let entries = match std::fs::read_dir(current) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        if cancelled.load(Ordering::Relaxed) {
            return;
        }

        let path = entry.path();
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) => name,
            None => continue,
        };

        // Skip hidden files, version control, build outputs, and dependencies
        if file_name.starts_with('.')
            || file_name == "target"
            || file_name == "node_modules"
            || file_name == "dist"
            || file_name == "build"
            || file_name == "vendor"
            || file_name == ".git"
        {
            continue;
        }

        if path.is_dir() {
            scan_dir(root, &path, collected, cancelled);
        } else if path.is_file() {
            if let Ok(rel) = path.strip_prefix(root) {
                collected.push(rel.to_string_lossy().to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_indexer_initialization_and_read() {
        let indexer = BackgroundIndexer::new();
        let current = indexer.read();
        assert_eq!(current.total_files, 0);
        assert!(!current.is_ready);
    }

    #[test]
    fn test_indexer_stop() {
        let indexer = BackgroundIndexer::new();
        assert!(!indexer.is_stopped());
        indexer.stop();
        assert!(indexer.is_stopped());
    }
}
