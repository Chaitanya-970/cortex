//! Agent execution context, iteration coordinator, cancellation, and persistent lifecycle management.

pub mod execution;
pub mod lifecycle;
pub mod manager;
pub mod manifest;

pub use execution::{
    AgentContext, AgentLoop, AgentRunResult, CancellationToken, ChatMessage, PermissionState,
    SessionMetadata, TodoItem, CODING_AGENT_POLICY,
};
pub use lifecycle::{AgentLifecycleEvent, AgentState};
pub use manager::{Agent, AgentManager, AgentManifest, AgentModelConfig, AgentPermissions};
pub use manifest::{AgentYamlManifest, AgentYamlModel, AgentYamlPermissions, YamlPermissionValue};
