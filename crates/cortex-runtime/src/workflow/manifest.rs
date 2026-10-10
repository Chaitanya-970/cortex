//! YAML manifest parser, schema validation, and workspace resolution for multi-agent workflows.

use cortex_core::{CortexError, Result, WorkflowManifest};
use std::path::{Path, PathBuf};

/// Parser and validator for declarative workflow manifests (`workflow.yaml`).
pub struct WorkflowYamlParser;

impl WorkflowYamlParser {
    /// Parse and validate a [`WorkflowManifest`] from a YAML string.
    ///
    /// Returns descriptive line-and-column diagnostic errors if YAML syntax is malformed,
    /// and semantic validation errors if the workflow schema, agent references, or DAG constraints fail.
    pub fn parse_str(yaml_str: &str) -> Result<WorkflowManifest> {
        let manifest: WorkflowManifest = serde_yaml::from_str(yaml_str).map_err(|e| {
            if let Some(location) = e.location() {
                CortexError::Validation(format!(
                    "invalid workflow YAML at line {}, column {}: {}",
                    location.line(),
                    location.column(),
                    e
                ))
            } else {
                CortexError::Validation(format!("invalid workflow YAML: {}", e))
            }
        })?;

        manifest.validate()?;
        Ok(manifest)
    }

    /// Parse and validate a [`WorkflowManifest`] from a YAML file.
    pub fn parse_file(path: impl AsRef<Path>) -> Result<WorkflowManifest> {
        let p = path.as_ref();
        if !p.is_file() {
            return Err(CortexError::NotFound(format!(
                "workflow file '{}' not found",
                p.display()
            )));
        }

        let content = std::fs::read_to_string(p).map_err(|e| {
            CortexError::Internal(format!(
                "failed to read workflow file '{}': {}",
                p.display(),
                e
            ))
        })?;

        Self::parse_str(&content)
    }

    /// Serialize a [`WorkflowManifest`] into a formatted YAML string.
    pub fn to_yaml_string(manifest: &WorkflowManifest) -> Result<String> {
        serde_yaml::to_string(manifest).map_err(|e| {
            CortexError::Internal(format!(
                "failed to serialize workflow manifest to yaml: {}",
                e
            ))
        })
    }

    /// Parse a workflow manifest from a file supporting YAML, JSON, or TOML formats based on file extension.
    pub fn parse_auto(path: impl AsRef<Path>) -> Result<WorkflowManifest> {
        let p = path.as_ref();
        if !p.is_file() {
            return Err(CortexError::NotFound(format!(
                "workflow file '{}' not found",
                p.display()
            )));
        }

        let content = std::fs::read_to_string(p).map_err(|e| {
            CortexError::Internal(format!(
                "failed to read workflow file '{}': {}",
                p.display(),
                e
            ))
        })?;

        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        let manifest = match ext.to_ascii_lowercase().as_str() {
            "json" => WorkflowManifest::from_json_str(&content)?,
            "toml" => toml::from_str(&content).map_err(|e| {
                CortexError::Validation(format!("invalid toml workflow manifest: {}", e))
            })?,
            "yaml" | "yml" => Self::parse_str(&content)?,
            _ => {
                // Attempt YAML first, then JSON, then TOML
                if let Ok(m) = Self::parse_str(&content) {
                    m
                } else if let Ok(m) = WorkflowManifest::from_json_str(&content) {
                    m
                } else {
                    toml::from_str(&content).map_err(|e| {
                        CortexError::Validation(format!(
                            "unable to parse workflow manifest (tried YAML, JSON, TOML): {}",
                            e
                        ))
                    })?
                }
            }
        };

        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate the workflow manifest against a workspace root directory.
    ///
    /// Verifies that all agent manifest paths referenced in `agents` exist, are readable files,
    /// and contain valid manifest data (JSON, YAML, or TOML).
    pub fn validate_workspace(
        manifest: &WorkflowManifest,
        workspace_root: impl AsRef<Path>,
    ) -> Result<()> {
        manifest.validate()?;
        let ws = workspace_root.as_ref();

        for agent in &manifest.agents {
            let manifest_path = if Path::new(&agent.manifest).is_absolute() {
                PathBuf::from(&agent.manifest)
            } else {
                ws.join(&agent.manifest)
            };

            if !manifest_path.is_file() {
                return Err(CortexError::NotFound(format!(
                    "agent manifest file '{}' referenced by agent '{}' not found in workspace '{}'",
                    agent.manifest,
                    agent.id,
                    ws.display()
                )));
            }

            // Verify readability and syntax of referenced agent manifest
            let content = std::fs::read_to_string(&manifest_path).map_err(|e| {
                CortexError::Internal(format!(
                    "failed to read agent manifest '{}': {}",
                    manifest_path.display(),
                    e
                ))
            })?;

            if content.trim().is_empty() {
                return Err(CortexError::Validation(format!(
                    "agent manifest file '{}' for agent '{}' is empty",
                    manifest_path.display(),
                    agent.id
                )));
            }

            // Validate that content is parseable as JSON, YAML, or TOML
            let is_valid_yaml = serde_yaml::from_str::<serde_json::Value>(&content).is_ok();
            let is_valid_json = serde_json::from_str::<serde_json::Value>(&content).is_ok();
            let is_valid_toml = toml::from_str::<toml::Value>(&content).is_ok();

            if !is_valid_yaml && !is_valid_json && !is_valid_toml {
                return Err(CortexError::Validation(format!(
                    "agent manifest file '{}' for agent '{}' contains malformed syntax",
                    manifest_path.display(),
                    agent.id
                )));
            }
        }

        Ok(())
    }
}

/// Extension trait providing ergonomic YAML methods on [`WorkflowManifest`].
pub trait WorkflowManifestExt {
    /// Deserialize and validate a [`WorkflowManifest`] from a YAML string.
    fn from_yaml_str(yaml_str: &str) -> Result<WorkflowManifest>;

    /// Deserialize and validate a [`WorkflowManifest`] from a YAML file.
    fn from_yaml_file(path: impl AsRef<Path>) -> Result<WorkflowManifest>;

    /// Serialize this [`WorkflowManifest`] to a YAML string.
    fn to_yaml_string(&self) -> Result<String>;

    /// Validate this [`WorkflowManifest`] against an active workspace root directory.
    fn validate_with_workspace(&self, workspace_root: impl AsRef<Path>) -> Result<()>;
}

impl WorkflowManifestExt for WorkflowManifest {
    fn from_yaml_str(yaml_str: &str) -> Result<WorkflowManifest> {
        WorkflowYamlParser::parse_str(yaml_str)
    }

    fn from_yaml_file(path: impl AsRef<Path>) -> Result<WorkflowManifest> {
        WorkflowYamlParser::parse_file(path)
    }

    fn to_yaml_string(&self) -> Result<String> {
        WorkflowYamlParser::to_yaml_string(self)
    }

    fn validate_with_workspace(&self, workspace_root: impl AsRef<Path>) -> Result<()> {
        WorkflowYamlParser::validate_workspace(self, workspace_root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cortex_core::{WorkflowAgentRef, WorkflowStage};

    const SPEC_WORKFLOW_YAML: &str = r#"
version: "1"
name: "code-refactor-team"
description: "Autonomous code research, implementation, and pull request review pipeline"

agents:
  - id: "manager"
    manifest: "./agents/manager.yaml"
    role: "coordinator"
  - id: "researcher"
    manifest: "./agents/researcher.yaml"
    role: "researcher"
  - id: "coder"
    manifest: "./agents/coder.yaml"
    role: "implementer"
  - id: "reviewer"
    manifest: "./agents/reviewer.yaml"
    role: "evaluator"

stages:
  - id: "research"
    agent: "researcher"
    action: "Analyze workspace and identify required modifications"
    outputs: ["research_notes"]
  - id: "implementation"
    agent: "coder"
    depends_on: ["research"]
    action: "Implement refactoring per research_notes"
    outputs: ["code_diff"]
  - id: "review"
    agent: "reviewer"
    depends_on: ["implementation"]
    action: "Review code_diff, run test suite, and approve or request revisions"
    max_iterations: 3
"#;

    #[test]
    fn test_spec_workflow_yaml_parsing_and_roundtrip() {
        let manifest = WorkflowYamlParser::parse_str(SPEC_WORKFLOW_YAML).expect("valid yaml parse");
        assert_eq!(manifest.version, "1");
        assert_eq!(manifest.name, "code-refactor-team");
        assert_eq!(manifest.agents.len(), 4);
        assert_eq!(manifest.stages.len(), 3);

        let review_stage = manifest.get_stage("review").unwrap();
        assert_eq!(review_stage.agent, "reviewer");
        assert_eq!(review_stage.depends_on, vec!["implementation"]);
        assert_eq!(review_stage.max_iterations, Some(3));

        // Serialization roundtrip
        let serialized_yaml = WorkflowYamlParser::to_yaml_string(&manifest).unwrap();
        let parsed_again = WorkflowYamlParser::parse_str(&serialized_yaml).unwrap();
        assert_eq!(manifest, parsed_again);
    }

    #[test]
    fn test_trait_extension_methods() {
        let manifest = WorkflowManifest::from_yaml_str(SPEC_WORKFLOW_YAML).unwrap();
        assert_eq!(manifest.name, "code-refactor-team");
        let yaml = manifest.to_yaml_string().unwrap();
        assert!(yaml.contains("code-refactor-team"));
    }

    #[test]
    fn test_yaml_syntax_error_with_line_and_column() {
        let bad_yaml =
            "version: '1'\nname: test\nagents:\n  - id: 'a'\n    manifest: [broken syntax\n";
        let err = WorkflowYamlParser::parse_str(bad_yaml).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("invalid workflow YAML at line"),
            "expected line in error, got: {}",
            msg
        );
        assert!(
            msg.contains("column"),
            "expected column in error, got: {}",
            msg
        );
    }

    #[test]
    fn test_workspace_validation() {
        let tmp_dir =
            std::env::temp_dir().join(format!("cortex_wf_ws_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp_dir);
        let agents_dir = tmp_dir.join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        std::fs::write(
            agents_dir.join("coder.yaml"),
            "name: coder\nrole: developer\n",
        )
        .unwrap();
        std::fs::write(
            agents_dir.join("reviewer.yaml"),
            "name: reviewer\nrole: tester\n",
        )
        .unwrap();

        let manifest = WorkflowManifest::new("dev-team")
            .with_agent(WorkflowAgentRef::new(
                "coder",
                "./agents/coder.yaml",
                "developer",
            ))
            .with_agent(WorkflowAgentRef::new(
                "reviewer",
                "./agents/reviewer.yaml",
                "tester",
            ))
            .with_stage(WorkflowStage::new("code", "coder", "Write code"))
            .with_stage(
                WorkflowStage::new("test", "reviewer", "Test code")
                    .with_depends_on(vec!["code".to_string()]),
            );

        // Valid workspace passes
        assert!(WorkflowYamlParser::validate_workspace(&manifest, &tmp_dir).is_ok());

        // Missing manifest fails
        let mut broken_manifest = manifest.clone();
        broken_manifest.agents.push(WorkflowAgentRef::new(
            "missing",
            "./agents/ghost.yaml",
            "ghost",
        ));
        broken_manifest
            .stages
            .push(WorkflowStage::new("ghost_task", "missing", "Do ghost work"));
        let err = WorkflowYamlParser::validate_workspace(&broken_manifest, &tmp_dir).unwrap_err();
        assert!(err.to_string().contains("not found in workspace"));

        let _ = std::fs::remove_dir_all(&tmp_dir);
    }
}
