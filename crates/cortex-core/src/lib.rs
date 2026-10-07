//! # cortex-core
//!
//! Foundational domain types, identifiers, and error abstractions for the Cortex
//! agent runtime and harness.
//!
//! Cortex is a long-lived open-source runtime for autonomous AI workers.
//! This crate provides shared kernel primitives that higher-level crates
//! (`cortex-runtime`, `cortex-cli`) depend upon.

#![deny(missing_docs)]

pub mod error;
pub mod id;

pub use error::{CortexError, Result};
pub use id::{AgentId, RunId, SessionId};

/// Current semantic version of the Cortex core library.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_available() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_error_formatting() {
        let err = CortexError::NotFound("test-resource".to_string());
        assert_eq!(err.to_string(), "resource not found: test-resource");
    }
}
