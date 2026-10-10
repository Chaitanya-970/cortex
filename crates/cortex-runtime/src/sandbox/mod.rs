//! Sandbox boundary trait and container/host execution backends.

pub mod docker;
pub mod host;

pub use docker::{DockerSandbox, DockerSandboxConfig};
pub use host::HostSandbox;

use cortex_core::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Isolation mode for sandboxed execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxMode {
    /// Non-sandboxed host environment (typically restricted to local development).
    #[default]
    Host,
    /// Containerized environment (e.g., Docker/OCI).
    Container,
}

/// Network isolation policy for sandboxed container execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkIsolationPolicy {
    /// Disables all network egress (`--network none`).
    #[default]
    Offline,
    /// Intranet-only network access restricted to container networks.
    IntranetOnly,
    /// Standard network access.
    FullInternet,
}

/// Result of a command execution within a sandboxed environment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxExecutionResult {
    /// Exit code of the executed process.
    pub exit_code: i32,
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
    /// Total execution duration in milliseconds.
    pub duration_ms: u64,
}

/// Abstract contract for sandboxed execution environments.
///
/// Sandboxes enforce process isolation, workspace bounds, and environment restrictions
/// for commands proposed by models.
pub trait Sandbox: Send + Sync {
    /// Return the execution mode of this sandbox.
    fn mode(&self) -> SandboxMode;

    /// Verify sandbox health and environment availability.
    fn is_healthy(&self) -> Result<bool>;

    /// Execute a command within the sandboxed environment with capability boundaries.
    fn execute(&self, command: &str, working_dir: Option<&Path>) -> Result<SandboxExecutionResult>;
}
