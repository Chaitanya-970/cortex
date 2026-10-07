//! Common error types for Cortex.

use thiserror::Error;

/// Root error type representing failure modes across Cortex subsystems.
#[derive(Debug, Error)]
pub enum CortexError {
    /// An entity or resource was not found.
    #[error("resource not found: {0}")]
    NotFound(String),

    /// Validation error for input parameters or configurations.
    #[error("validation error: {0}")]
    Validation(String),

    /// Permission or capability check failure.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// Operation timed out.
    #[error("operation timed out after {0} ms")]
    Timeout(u64),

    /// Subsystem or execution aborted.
    #[error("operation cancelled: {0}")]
    Cancelled(String),

    /// Internal runtime or environment error.
    #[error("internal error: {0}")]
    Internal(String),
}

/// Convenience alias for results returning [`CortexError`].
pub type Result<T> = std::result::Result<T, CortexError>;
