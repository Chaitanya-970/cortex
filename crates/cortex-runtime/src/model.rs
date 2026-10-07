//! Model provider trait and related types.

use cortex_core::Result;

/// Metadata describing a supported model or model family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDescriptor {
    /// Identifier of the model provider (e.g., "gemma", "google", "local").
    pub provider: String,
    /// Model name or variant identifier.
    pub name: String,
}

impl ModelDescriptor {
    /// Create a new [`ModelDescriptor`].
    pub fn new(provider: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            name: name.into(),
        }
    }
}

/// Abstract contract for LLM inference providers.
///
/// In Cortex, the model layer produces proposed structured actions.
/// The runtime holds sole execution authority and validates all model responses.
pub trait ModelProvider: Send + Sync {
    /// Return the descriptor for this provider.
    fn descriptor(&self) -> &ModelDescriptor;

    /// Validate whether the provider credentials and endpoint are configured.
    fn is_configured(&self) -> Result<bool>;
}
