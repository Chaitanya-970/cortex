//! Multi-agent declarative workflow manifest schema definitions and core types.

use crate::error::{CortexError, Result};
use crate::id::WorkflowId;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Reference to a participating agent worker in a multi-agent workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowAgentRef {
    /// Unique identifier for this agent within the workflow.
    pub id: String,
    /// Relative or absolute path to the agent's manifest file (e.g., `./agents/coder.yaml`).
    pub manifest: String,
    /// Functional role descriptor for this agent (e.g., `coordinator`, `implementer`).
    pub role: String,
}

impl WorkflowAgentRef {
    /// Create a new [`WorkflowAgentRef`].
    pub fn new(
        id: impl Into<String>,
        manifest: impl Into<String>,
        role: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            manifest: manifest.into(),
            role: role.into(),
        }
    }
}

/// A single execution stage within a multi-agent workflow pipeline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStage {
    /// Unique identifier for this stage.
    pub id: String,
    /// Identifier of the agent assigned to execute this stage.
    pub agent: String,
    /// Natural language prompt, instruction, or goal to be performed by the agent.
    pub action: String,
    /// List of stage IDs that must successfully complete before this stage can execute.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub depends_on: Vec<String>,
    /// Artifact keys or named outputs produced by this stage.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<String>,
    /// Optional bound on maximum iterations or execution loopbacks for this stage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_iterations: Option<usize>,
}

impl WorkflowStage {
    /// Create a new [`WorkflowStage`].
    pub fn new(id: impl Into<String>, agent: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            agent: agent.into(),
            action: action.into(),
            depends_on: Vec::new(),
            outputs: Vec::new(),
            max_iterations: None,
        }
    }

    /// Add dependency stage IDs that this stage relies upon.
    pub fn with_depends_on(mut self, depends_on: Vec<String>) -> Self {
        self.depends_on = depends_on;
        self
    }

    /// Add output identifiers produced by this stage.
    pub fn with_outputs(mut self, outputs: Vec<String>) -> Self {
        self.outputs = outputs;
        self
    }

    /// Set an iteration limit on this stage.
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = Some(max_iterations);
        self
    }
}

/// Declarative multi-agent workflow specification (`workflow.yaml`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowManifest {
    /// Schema specification version (e.g. `"1"`).
    pub version: String,
    /// Unique workflow name or pipeline identifier.
    pub name: String,
    /// Optional descriptive summary of the workflow's purpose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Participating agents configured for this workflow team.
    pub agents: Vec<WorkflowAgentRef>,
    /// Directed stages comprising the execution pipeline.
    pub stages: Vec<WorkflowStage>,
}

impl WorkflowManifest {
    /// Create a new [`WorkflowManifest`] with default version `"1"`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            version: "1".to_string(),
            name: name.into(),
            description: None,
            agents: Vec::new(),
            stages: Vec::new(),
        }
    }

    /// Set the schema specification version.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Set a human-readable description for the workflow.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Append an agent reference to the workflow.
    pub fn with_agent(mut self, agent: WorkflowAgentRef) -> Self {
        self.agents.push(agent);
        self
    }

    /// Append a stage definition to the workflow.
    pub fn with_stage(mut self, stage: WorkflowStage) -> Self {
        self.stages.push(stage);
        self
    }

    /// Generate a typed [`WorkflowId`] based on this workflow's name.
    pub fn workflow_id(&self) -> WorkflowId {
        WorkflowId::new(&self.name)
    }

    /// Find an agent reference by its identifier.
    pub fn get_agent(&self, id: &str) -> Option<&WorkflowAgentRef> {
        self.agents.iter().find(|a| a.id == id)
    }

    /// Find a stage by its identifier.
    pub fn get_stage(&self, id: &str) -> Option<&WorkflowStage> {
        self.stages.iter().find(|s| s.id == id)
    }

    /// Deserialize a [`WorkflowManifest`] from a JSON string.
    pub fn from_json_str(json_str: &str) -> Result<Self> {
        serde_json::from_str(json_str)
            .map_err(|e| CortexError::Validation(format!("invalid json workflow manifest: {}", e)))
    }

    /// Serialize this [`WorkflowManifest`] to a pretty-printed JSON string.
    pub fn to_json_string(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| {
            CortexError::Internal(format!(
                "failed to serialize workflow manifest to json: {}",
                e
            ))
        })
    }

    /// Validate the workflow manifest schema, referential integrity, and stage DAG acyclicity.
    pub fn validate(&self) -> Result<()> {
        // 1. Version check
        if self.version.trim() != "1" {
            return Err(CortexError::Validation(format!(
                "unsupported workflow version '{}'; expected '1'",
                self.version
            )));
        }

        // 2. Name check
        let name = self.name.trim();
        if name.is_empty() {
            return Err(CortexError::Validation(
                "workflow name cannot be empty".to_string(),
            ));
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(CortexError::Validation(format!(
                "invalid workflow name '{}': only alphanumeric characters, hyphens, and underscores are allowed",
                name
            )));
        }

        // 3. Agents check
        if self.agents.is_empty() {
            return Err(CortexError::Validation(
                "workflow must define at least one agent in 'agents'".to_string(),
            ));
        }

        let mut seen_agent_ids = HashSet::new();
        for agent in &self.agents {
            let aid = agent.id.trim();
            if aid.is_empty() {
                return Err(CortexError::Validation(
                    "agent id cannot be empty".to_string(),
                ));
            }
            if !seen_agent_ids.insert(aid.to_string()) {
                return Err(CortexError::Validation(format!(
                    "duplicate agent id '{}' in workflow",
                    aid
                )));
            }
            if agent.manifest.trim().is_empty() {
                return Err(CortexError::Validation(format!(
                    "agent '{}' manifest path cannot be empty",
                    aid
                )));
            }
            if agent.role.trim().is_empty() {
                return Err(CortexError::Validation(format!(
                    "agent '{}' role cannot be empty",
                    aid
                )));
            }
        }

        // 4. Stages check
        if self.stages.is_empty() {
            return Err(CortexError::Validation(
                "workflow must define at least one stage in 'stages'".to_string(),
            ));
        }

        let mut seen_stage_ids = HashSet::new();
        for stage in &self.stages {
            let sid = stage.id.trim();
            if sid.is_empty() {
                return Err(CortexError::Validation(
                    "stage id cannot be empty".to_string(),
                ));
            }
            if !seen_stage_ids.insert(sid.to_string()) {
                return Err(CortexError::Validation(format!(
                    "duplicate stage id '{}' in workflow",
                    sid
                )));
            }
            if stage.action.trim().is_empty() {
                return Err(CortexError::Validation(format!(
                    "stage '{}' action cannot be empty",
                    sid
                )));
            }
            if !seen_agent_ids.contains(&stage.agent) {
                return Err(CortexError::Validation(format!(
                    "stage '{}' references undefined agent '{}'",
                    sid, stage.agent
                )));
            }

            // Max iterations bounds
            if let Some(max_iter) = stage.max_iterations {
                if max_iter == 0 {
                    return Err(CortexError::Validation(format!(
                        "stage '{}' max_iterations must be greater than 0",
                        sid
                    )));
                }
                if max_iter > 1000 {
                    return Err(CortexError::Validation(format!(
                        "stage '{}' max_iterations ({}) exceeds maximum allowed limit of 1000",
                        sid, max_iter
                    )));
                }
            }

            // Dependencies check
            let mut seen_deps = HashSet::new();
            for dep in &stage.depends_on {
                if dep == sid {
                    return Err(CortexError::Validation(format!(
                        "stage '{}' cannot depend on itself",
                        sid
                    )));
                }
                if !seen_deps.insert(dep.as_str()) {
                    return Err(CortexError::Validation(format!(
                        "stage '{}' has duplicate dependency '{}'",
                        sid, dep
                    )));
                }
            }
        }

        // Verify that all depends_on refer to valid stage IDs
        for stage in &self.stages {
            for dep in &stage.depends_on {
                if !seen_stage_ids.contains(dep) {
                    return Err(CortexError::Validation(format!(
                        "stage '{}' depends on unknown stage '{}'",
                        stage.id, dep
                    )));
                }
            }
        }

        // 5. DAG Cycle Detection
        self.detect_cycle()?;

        Ok(())
    }

    /// Detect circular stage dependencies and return an error with the complete cycle path if found.
    fn detect_cycle(&self) -> Result<()> {
        let stage_map: HashMap<&str, &WorkflowStage> =
            self.stages.iter().map(|s| (s.id.as_str(), s)).collect();

        // 0 = unvisited, 1 = visiting (in recursion stack), 2 = visited
        let mut state: HashMap<&str, u8> = HashMap::new();
        let mut stack: Vec<&str> = Vec::new();

        for stage in &self.stages {
            let id = stage.id.as_str();
            if state.get(id).copied().unwrap_or(0) == 0 {
                Self::dfs_cycle(id, &stage_map, &mut state, &mut stack)?;
            }
        }

        Ok(())
    }

    fn dfs_cycle<'a>(
        curr: &'a str,
        stage_map: &HashMap<&str, &'a WorkflowStage>,
        state: &mut HashMap<&'a str, u8>,
        stack: &mut Vec<&'a str>,
    ) -> Result<()> {
        state.insert(curr, 1);
        stack.push(curr);

        if let Some(stage) = stage_map.get(curr) {
            for dep in &stage.depends_on {
                let dep_str = dep.as_str();
                match state.get(dep_str).copied().unwrap_or(0) {
                    1 => {
                        // Cycle detected! Extract the cycle path from stack
                        let start_idx = stack.iter().position(|&x| x == dep_str).unwrap_or(0);
                        let mut cycle_path = stack[start_idx..].to_vec();
                        cycle_path.push(dep_str);
                        return Err(CortexError::Validation(format!(
                            "cyclic dependency detected in workflow stages: {}",
                            cycle_path.join(" -> ")
                        )));
                    }
                    0 => {
                        Self::dfs_cycle(dep_str, stage_map, state, stack)?;
                    }
                    _ => {}
                }
            }
        }

        stack.pop();
        state.insert(curr, 2);
        Ok(())
    }

    /// Compute a linear topological execution sequence for the stages.
    ///
    /// Dependencies are guaranteed to appear before dependent stages.
    pub fn execution_order(&self) -> Result<Vec<String>> {
        self.validate()?;

        // in_degree: number of unsatisfied dependencies for each stage
        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        // dependents: map from stage -> list of stages waiting on it
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

        for stage in &self.stages {
            in_degree.insert(stage.id.as_str(), stage.depends_on.len());
            for dep in &stage.depends_on {
                dependents
                    .entry(dep.as_str())
                    .or_default()
                    .push(stage.id.as_str());
            }
        }

        // Stages with 0 dependencies can execute immediately
        let mut queue: Vec<&str> = self
            .stages
            .iter()
            .filter(|s| s.depends_on.is_empty())
            .map(|s| s.id.as_str())
            .collect();
        queue.sort(); // Deterministic ordering

        let mut order = Vec::with_capacity(self.stages.len());

        while !queue.is_empty() {
            let curr = queue.remove(0);
            order.push(curr.to_string());

            if let Some(deps) = dependents.get(curr) {
                for &dep_stage in deps {
                    if let Some(deg) = in_degree.get_mut(dep_stage) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push(dep_stage);
                            queue.sort();
                        }
                    }
                }
            }
        }

        if order.len() < self.stages.len() {
            return Err(CortexError::Validation(
                "cannot compute execution order: cyclic dependency in workflow".to_string(),
            ));
        }

        Ok(order)
    }

    /// Compute execution stages grouped into concurrent waves.
    ///
    /// Each wave contains stages that can execute in parallel because all their
    /// dependencies have completed in preceding waves.
    pub fn execution_waves(&self) -> Result<Vec<Vec<String>>> {
        self.validate()?;

        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();

        for stage in &self.stages {
            in_degree.insert(stage.id.as_str(), stage.depends_on.len());
            for dep in &stage.depends_on {
                dependents
                    .entry(dep.as_str())
                    .or_default()
                    .push(stage.id.as_str());
            }
        }

        let mut current_wave: Vec<&str> = self
            .stages
            .iter()
            .filter(|s| s.depends_on.is_empty())
            .map(|s| s.id.as_str())
            .collect();
        current_wave.sort();

        let mut waves = Vec::new();
        let mut total_scheduled = 0;

        while !current_wave.is_empty() {
            total_scheduled += current_wave.len();
            let mut next_wave = Vec::new();

            for &curr in &current_wave {
                if let Some(deps) = dependents.get(curr) {
                    for &dep_stage in deps {
                        if let Some(deg) = in_degree.get_mut(dep_stage) {
                            *deg -= 1;
                            if *deg == 0 {
                                next_wave.push(dep_stage);
                            }
                        }
                    }
                }
            }

            waves.push(current_wave.into_iter().map(String::from).collect());
            next_wave.sort();
            current_wave = next_wave;
        }

        if total_scheduled < self.stages.len() {
            return Err(CortexError::Validation(
                "cannot compute execution waves: cyclic dependency in workflow".to_string(),
            ));
        }

        Ok(waves)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_valid_workflow() -> WorkflowManifest {
        WorkflowManifest::new("code-refactor-team")
            .with_description("Autonomous code research and review pipeline")
            .with_agent(WorkflowAgentRef::new(
                "manager",
                "./agents/manager.yaml",
                "coordinator",
            ))
            .with_agent(WorkflowAgentRef::new(
                "researcher",
                "./agents/researcher.yaml",
                "researcher",
            ))
            .with_agent(WorkflowAgentRef::new(
                "coder",
                "./agents/coder.yaml",
                "implementer",
            ))
            .with_agent(WorkflowAgentRef::new(
                "reviewer",
                "./agents/reviewer.yaml",
                "evaluator",
            ))
            .with_stage(WorkflowStage::new(
                "research",
                "researcher",
                "Analyze codebase architecture",
            ))
            .with_stage(
                WorkflowStage::new("implementation", "coder", "Write patch")
                    .with_depends_on(vec!["research".to_string()])
                    .with_outputs(vec!["code_diff".to_string()]),
            )
            .with_stage(
                WorkflowStage::new("review", "reviewer", "Validate patch")
                    .with_depends_on(vec!["implementation".to_string()])
                    .with_max_iterations(3),
            )
    }

    #[test]
    fn test_valid_manifest_passes_validation() {
        let wf = sample_valid_workflow();
        assert!(wf.validate().is_ok());

        let order = wf.execution_order().unwrap();
        assert_eq!(order, vec!["research", "implementation", "review"]);

        let waves = wf.execution_waves().unwrap();
        assert_eq!(waves.len(), 3);
        assert_eq!(waves[0], vec!["research"]);
        assert_eq!(waves[1], vec!["implementation"]);
        assert_eq!(waves[2], vec!["review"]);
    }

    #[test]
    fn test_validation_rejects_empty_name() {
        let mut wf = sample_valid_workflow();
        wf.name = "   ".to_string();
        let err = wf.validate().unwrap_err();
        assert!(err.to_string().contains("name cannot be empty"));
    }

    #[test]
    fn test_validation_rejects_invalid_version() {
        let mut wf = sample_valid_workflow();
        wf.version = "2".to_string();
        let err = wf.validate().unwrap_err();
        assert!(err.to_string().contains("unsupported workflow version"));
    }

    #[test]
    fn test_validation_rejects_empty_agents() {
        let mut wf = sample_valid_workflow();
        wf.agents.clear();
        let err = wf.validate().unwrap_err();
        assert!(err.to_string().contains("at least one agent"));
    }

    #[test]
    fn test_validation_rejects_duplicate_agent_id() {
        let mut wf = sample_valid_workflow();
        wf.agents.push(WorkflowAgentRef::new(
            "coder",
            "./agents/coder2.yaml",
            "implementer",
        ));
        let err = wf.validate().unwrap_err();
        assert!(err.to_string().contains("duplicate agent id 'coder'"));
    }

    #[test]
    fn test_validation_rejects_undefined_agent_in_stage() {
        let mut wf = sample_valid_workflow();
        wf.stages[0].agent = "ghost_agent".to_string();
        let err = wf.validate().unwrap_err();
        assert!(err
            .to_string()
            .contains("references undefined agent 'ghost_agent'"));
    }

    #[test]
    fn test_validation_rejects_self_dependency() {
        let mut wf = sample_valid_workflow();
        wf.stages[0].depends_on = vec!["research".to_string()];
        let err = wf.validate().unwrap_err();
        assert!(err
            .to_string()
            .contains("stage 'research' cannot depend on itself"));
    }

    #[test]
    fn test_validation_rejects_unknown_dependency() {
        let mut wf = sample_valid_workflow();
        wf.stages[0].depends_on = vec!["non_existent_stage".to_string()];
        let err = wf.validate().unwrap_err();
        assert!(err
            .to_string()
            .contains("depends on unknown stage 'non_existent_stage'"));
    }

    #[test]
    fn test_validation_rejects_cyclic_dependency_two_nodes() {
        let mut wf = sample_valid_workflow();
        // research -> implementation -> research
        wf.stages[0].depends_on = vec!["implementation".to_string()];
        wf.stages[1].depends_on = vec!["research".to_string()];
        let err = wf.validate().unwrap_err();
        assert!(err.to_string().contains("cyclic dependency detected"));
    }

    #[test]
    fn test_validation_rejects_cyclic_dependency_three_nodes() {
        let mut wf = sample_valid_workflow();
        // research -> review -> implementation -> research
        wf.stages[0].depends_on = vec!["review".to_string()];
        wf.stages[1].depends_on = vec!["research".to_string()];
        wf.stages[2].depends_on = vec!["implementation".to_string()];
        let err = wf.validate().unwrap_err();
        assert!(err.to_string().contains("cyclic dependency detected"));
    }

    #[test]
    fn test_validation_rejects_invalid_max_iterations() {
        let mut wf = sample_valid_workflow();
        wf.stages[0].max_iterations = Some(0);
        let err = wf.validate().unwrap_err();
        assert!(err
            .to_string()
            .contains("max_iterations must be greater than 0"));

        wf.stages[0].max_iterations = Some(2000);
        let err = wf.validate().unwrap_err();
        assert!(err.to_string().contains("exceeds maximum allowed limit"));
    }
}
