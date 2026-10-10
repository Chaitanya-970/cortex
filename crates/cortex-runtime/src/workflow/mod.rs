//! Declarative multi-agent workflows, schema parser, and execution planning.

pub mod manifest;

pub use cortex_core::workflow::{WorkflowAgentRef, WorkflowManifest, WorkflowStage};
pub use manifest::{WorkflowManifestExt, WorkflowYamlParser};
