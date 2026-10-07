//! Sandbox boundary trait and configuration types.

use cortex_core::Result;

/// Isolation mode for sandboxed execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SandboxMode {
    /// Non-sandboxed host environment (typically restricted to local development).
    #[default]
    Host,
    /// Containerized environment (e.g., Docker/OCI).
    Container,
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
}
