//! Thread-safe catalog for agent worker discovery, capability matching, and role indexing.

use super::lifecycle::{AgentLifecycleEvent, AgentState};
use super::manager::Agent;
use cortex_core::{AgentId, CortexError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Declared capability or domain qualification of an agent worker.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AgentCapability {
    /// Tool or tool family available to the agent (e.g. `read_file`, `shell`, `git_*`).
    Tool(String),
    /// Inference model performance tier (e.g. `fast`, `reasoning`, `coding`).
    ModelTier(String),
    /// Operational domain specialization (e.g. `researcher`, `coder`, `reviewer`, `planner`).
    Domain(String),
    /// Custom operational capability tag.
    Custom(String),
}

impl AgentCapability {
    /// Create a tool capability.
    pub fn tool(name: impl Into<String>) -> Self {
        Self::Tool(name.into())
    }

    /// Create a model tier capability.
    pub fn model_tier(tier: impl Into<String>) -> Self {
        Self::ModelTier(tier.into())
    }

    /// Create a domain specialization capability.
    pub fn domain(domain: impl Into<String>) -> Self {
        Self::Domain(domain.into())
    }

    /// Create a custom capability.
    pub fn custom(tag: impl Into<String>) -> Self {
        Self::Custom(tag.into())
    }

    /// Underlying string representation of this capability.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Tool(s) => s,
            Self::ModelTier(s) => s,
            Self::Domain(s) => s,
            Self::Custom(s) => s,
        }
    }

    /// Check whether this capability matches a search query.
    ///
    /// Supports exact matching (case-insensitive), typed prefixes (`tool:`, `tier:`, `domain:`),
    /// and wildcard suffix matching (e.g., `git_*` matching `git_commit` or `git_status`).
    pub fn matches(&self, query: &str) -> bool {
        let q = query.trim();
        if q.is_empty() {
            return false;
        }

        let inner = self.as_str();
        if inner.eq_ignore_ascii_case(q) {
            return true;
        }

        match self {
            Self::Tool(tool) => {
                if let Some(tool_query) = q.strip_prefix("tool:") {
                    if tool.eq_ignore_ascii_case(tool_query) {
                        return true;
                    }
                }
                // Handle wildcards: e.g. "git_*" matching "git_commit"
                if let Some(prefix) = q.strip_suffix('*') {
                    if tool
                        .to_ascii_lowercase()
                        .starts_with(&prefix.to_ascii_lowercase())
                    {
                        return true;
                    }
                }
                if let Some(prefix) = tool.strip_suffix('*') {
                    if q.to_ascii_lowercase()
                        .starts_with(&prefix.to_ascii_lowercase())
                    {
                        return true;
                    }
                }
            }
            Self::ModelTier(tier) => {
                if let Some(tier_query) = q
                    .strip_prefix("tier:")
                    .or_else(|| q.strip_prefix("model_tier:"))
                {
                    if tier.eq_ignore_ascii_case(tier_query) {
                        return true;
                    }
                }
            }
            Self::Domain(domain) => {
                if let Some(domain_query) = q.strip_prefix("domain:") {
                    if domain.eq_ignore_ascii_case(domain_query) {
                        return true;
                    }
                }
            }
            Self::Custom(_) => {}
        }

        false
    }
}

impl std::fmt::Display for AgentCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tool(s) => write!(f, "tool:{}", s),
            Self::ModelTier(s) => write!(f, "tier:{}", s),
            Self::Domain(s) => write!(f, "domain:{}", s),
            Self::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// Public, read-optimized snapshot of an agent worker's metadata and capabilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDescriptor {
    /// Unique identifier of the agent.
    pub id: AgentId,
    /// Human-readable agent worker name.
    pub name: String,
    /// Operational role definition.
    pub role: String,
    /// Current lifecycle state of the agent.
    pub status: AgentState,
    /// Declared tool, tier, and domain capabilities.
    pub capabilities: Vec<AgentCapability>,
    /// Root path of the assigned workspace.
    pub workspace_root: PathBuf,
    /// Descriptive operational tags.
    pub tags: Vec<String>,
}

impl AgentDescriptor {
    /// Create a new [`AgentDescriptor`].
    pub fn new(
        id: AgentId,
        name: impl Into<String>,
        role: impl Into<String>,
        status: AgentState,
        workspace_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            id,
            name: name.into(),
            role: role.into(),
            status,
            capabilities: Vec::new(),
            workspace_root: workspace_root.into(),
            tags: Vec::new(),
        }
    }

    /// Attach capabilities to the descriptor.
    pub fn with_capabilities(
        mut self,
        capabilities: impl IntoIterator<Item = AgentCapability>,
    ) -> Self {
        self.capabilities.extend(capabilities);
        self
    }

    /// Attach a single capability.
    pub fn with_capability(mut self, capability: AgentCapability) -> Self {
        self.capabilities.push(capability);
        self
    }

    /// Attach operational tags to the descriptor.
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    /// Attach a single tag.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Check whether this descriptor matches a specific capability or tag query.
    pub fn matches_capability(&self, query: &str) -> bool {
        self.capabilities.iter().any(|c| c.matches(query))
            || self
                .tags
                .iter()
                .any(|t| t.eq_ignore_ascii_case(query.trim()))
    }

    /// Count how many of the specified capability or tag queries this descriptor satisfies.
    pub fn match_score(&self, queries: &[impl AsRef<str>]) -> usize {
        queries
            .iter()
            .filter(|q| self.matches_capability(q.as_ref()))
            .count()
    }

    /// Construct an [`AgentDescriptor`] snapshot from an [`Agent`] runtime instance.
    pub fn from_agent(agent: &Agent) -> Self {
        let mut capabilities = Vec::new();

        // Populate tools
        for tool in &agent.tools {
            capabilities.push(AgentCapability::tool(tool.clone()));
        }

        // Infer model tier
        let model_name = agent.model.model.to_ascii_lowercase();
        let tier = if model_name.contains("haiku")
            || model_name.contains("mini")
            || model_name.contains("flash")
        {
            "fast"
        } else if model_name.contains("o1")
            || model_name.contains("r1")
            || model_name.contains("reason")
        {
            "reasoning"
        } else if model_name.contains("coder") || model_name.contains("sonnet") {
            "coding"
        } else {
            "fast"
        };
        capabilities.push(AgentCapability::model_tier(tier));

        // Domain tags inferred from role
        let role_lower = agent.role.to_ascii_lowercase();
        let mut matched_domain = false;
        if role_lower.contains("research") {
            capabilities.push(AgentCapability::domain("researcher"));
            matched_domain = true;
        }
        if role_lower.contains("review") {
            capabilities.push(AgentCapability::domain("reviewer"));
            matched_domain = true;
        }
        if role_lower.contains("code")
            || role_lower.contains("engineer")
            || role_lower.contains("developer")
        {
            capabilities.push(AgentCapability::domain("coder"));
            matched_domain = true;
        }
        if role_lower.contains("plan") {
            capabilities.push(AgentCapability::domain("planner"));
            matched_domain = true;
        }
        if !matched_domain {
            capabilities.push(AgentCapability::domain(&agent.role));
        }

        let tags = vec![agent.role.clone(), agent.model.provider.clone()];

        Self {
            id: agent.id.clone(),
            name: agent.name.clone(),
            role: agent.role.clone(),
            status: agent.state,
            capabilities,
            workspace_root: agent.workspace.clone(),
            tags,
        }
    }
}

impl From<&Agent> for AgentDescriptor {
    fn from(agent: &Agent) -> Self {
        Self::from_agent(agent)
    }
}

impl From<Agent> for AgentDescriptor {
    fn from(agent: Agent) -> Self {
        Self::from_agent(&agent)
    }
}

/// Read-optimized, thread-safe catalog for agent discovery and capability matching.
#[derive(Debug, Clone, Default)]
pub struct AgentRegistry {
    descriptors: Arc<RwLock<HashMap<AgentId, AgentDescriptor>>>,
}

impl AgentRegistry {
    /// Create a new, empty [`AgentRegistry`].
    pub fn new() -> Self {
        Self {
            descriptors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create an [`AgentRegistry`] backed by a shared descriptor store.
    pub fn from_shared(descriptors: Arc<RwLock<HashMap<AgentId, AgentDescriptor>>>) -> Self {
        Self { descriptors }
    }

    /// Access the underlying shared descriptors map.
    pub fn descriptors(&self) -> Arc<RwLock<HashMap<AgentId, AgentDescriptor>>> {
        Arc::clone(&self.descriptors)
    }

    /// Register a new [`AgentDescriptor`].
    ///
    /// Returns [`CortexError::Validation`] if an agent with the same ID is already registered.
    pub fn register(&self, descriptor: AgentDescriptor) -> Result<()> {
        let mut lock = self.descriptors.write().map_err(|e| {
            CortexError::Internal(format!(
                "failed to acquire agent registry write lock: {}",
                e
            ))
        })?;
        if lock.contains_key(&descriptor.id) {
            return Err(CortexError::Validation(format!(
                "agent '{}' is already registered in registry",
                descriptor.id
            )));
        }
        lock.insert(descriptor.id.clone(), descriptor);
        Ok(())
    }

    /// Upsert an agent descriptor, inserting it if absent or replacing it if present.
    pub fn upsert(&self, descriptor: AgentDescriptor) -> Result<()> {
        let mut lock = self.descriptors.write().map_err(|e| {
            CortexError::Internal(format!(
                "failed to acquire agent registry write lock: {}",
                e
            ))
        })?;
        lock.insert(descriptor.id.clone(), descriptor);
        Ok(())
    }

    /// Deregister an agent by [`AgentId`].
    ///
    /// Returns [`CortexError::NotFound`] if the agent is not registered.
    pub fn deregister(&self, id: &AgentId) -> Result<()> {
        let mut lock = self.descriptors.write().map_err(|e| {
            CortexError::Internal(format!(
                "failed to acquire agent registry write lock: {}",
                e
            ))
        })?;
        lock.remove(id).ok_or_else(|| {
            CortexError::NotFound(format!("agent '{}' not found in registry", id))
        })?;
        Ok(())
    }

    /// Look up an agent descriptor by [`AgentId`].
    pub fn get(&self, id: &AgentId) -> Option<AgentDescriptor> {
        let lock = self.descriptors.read().unwrap_or_else(|e| e.into_inner());
        lock.get(id).cloned()
    }

    /// Check if the registry contains an agent with the specified [`AgentId`].
    pub fn contains(&self, id: &AgentId) -> bool {
        let lock = self.descriptors.read().unwrap_or_else(|e| e.into_inner());
        lock.contains_key(id)
    }

    /// Find all agents matching an operational role (case-insensitive substring or exact match).
    pub fn find_by_role(&self, role: &str) -> Vec<AgentDescriptor> {
        let lock = self.descriptors.read().unwrap_or_else(|e| e.into_inner());
        let target = role.trim().to_ascii_lowercase();
        let mut results: Vec<AgentDescriptor> = lock
            .values()
            .filter(|d| {
                let r = d.role.to_ascii_lowercase();
                r == target || r.contains(&target)
            })
            .cloned()
            .collect();
        results.sort_by(|a, b| a.id.cmp(&b.id));
        results
    }

    /// Find all agents exposing a matching capability or tag.
    pub fn find_by_capability(&self, capability: &str) -> Vec<AgentDescriptor> {
        let lock = self.descriptors.read().unwrap_or_else(|e| e.into_inner());
        let mut results: Vec<AgentDescriptor> = lock
            .values()
            .filter(|d| d.matches_capability(capability))
            .cloned()
            .collect();
        results.sort_by(|a, b| a.id.cmp(&b.id));
        results
    }

    /// Find all agents matching every capability or tag in `queries` (conjunction / AND).
    ///
    /// If `queries` is empty, returns all registered agents.
    pub fn find_by_all_capabilities(&self, queries: &[impl AsRef<str>]) -> Vec<AgentDescriptor> {
        if queries.is_empty() {
            return self.list_all();
        }
        let lock = self.descriptors.read().unwrap_or_else(|e| e.into_inner());
        let mut results: Vec<AgentDescriptor> = lock
            .values()
            .filter(|d| queries.iter().all(|q| d.matches_capability(q.as_ref())))
            .cloned()
            .collect();
        results.sort_by(|a, b| a.id.cmp(&b.id));
        results
    }

    /// Find all agents matching at least one capability or tag in `queries` (disjunction / OR).
    ///
    /// If `queries` is empty, returns an empty vector.
    pub fn find_by_any_capability(&self, queries: &[impl AsRef<str>]) -> Vec<AgentDescriptor> {
        if queries.is_empty() {
            return Vec::new();
        }
        let lock = self.descriptors.read().unwrap_or_else(|e| e.into_inner());
        let mut results: Vec<AgentDescriptor> = lock
            .values()
            .filter(|d| queries.iter().any(|q| d.matches_capability(q.as_ref())))
            .cloned()
            .collect();
        results.sort_by(|a, b| a.id.cmp(&b.id));
        results
    }

    /// Rank registered agents by how many capabilities or tags in `queries` they satisfy.
    ///
    /// Returns pairs of `(descriptor, score)` where `score > 0`, ordered descending by score
    /// with ties broken deterministically by `AgentId`.
    pub fn rank_by_capabilities(
        &self,
        queries: &[impl AsRef<str>],
    ) -> Vec<(AgentDescriptor, usize)> {
        if queries.is_empty() {
            return Vec::new();
        }
        let lock = self.descriptors.read().unwrap_or_else(|e| e.into_inner());
        let mut scored: Vec<(AgentDescriptor, usize)> = lock
            .values()
            .filter_map(|d| {
                let score = d.match_score(queries);
                if score > 0 {
                    Some((d.clone(), score))
                } else {
                    None
                }
            })
            .collect();
        scored.sort_by(|(a_desc, a_score), (b_desc, b_score)| {
            b_score.cmp(a_score).then_with(|| a_desc.id.cmp(&b_desc.id))
        });
        scored
    }

    /// Find the highest-scoring candidate agent matching `queries`.
    ///
    /// In case of score ties, picks the candidate with the smallest `AgentId`.
    pub fn find_best_match(&self, queries: &[impl AsRef<str>]) -> Option<AgentDescriptor> {
        self.rank_by_capabilities(queries)
            .into_iter()
            .next()
            .map(|(desc, _)| desc)
    }

    /// Find all currently active agents ([`AgentState::Running`] or [`AgentState::Paused`])
    /// exposing a matching capability or tag.
    pub fn find_active_by_capability(&self, capability: &str) -> Vec<AgentDescriptor> {
        let lock = self.descriptors.read().unwrap_or_else(|e| e.into_inner());
        let mut results: Vec<AgentDescriptor> = lock
            .values()
            .filter(|d| d.status.is_active() && d.matches_capability(capability))
            .cloned()
            .collect();
        results.sort_by(|a, b| a.id.cmp(&b.id));
        results
    }

    /// Find all currently active agents matching all capabilities or tags in `queries`.
    pub fn find_active_by_all_capabilities(
        &self,
        queries: &[impl AsRef<str>],
    ) -> Vec<AgentDescriptor> {
        if queries.is_empty() {
            return self.list_active();
        }
        let lock = self.descriptors.read().unwrap_or_else(|e| e.into_inner());
        let mut results: Vec<AgentDescriptor> = lock
            .values()
            .filter(|d| {
                d.status.is_active() && queries.iter().all(|q| d.matches_capability(q.as_ref()))
            })
            .cloned()
            .collect();
        results.sort_by(|a, b| a.id.cmp(&b.id));
        results
    }

    /// Find all agents with a matching operational tag (case-insensitive).
    pub fn find_by_tag(&self, tag: &str) -> Vec<AgentDescriptor> {
        let lock = self.descriptors.read().unwrap_or_else(|e| e.into_inner());
        let target = tag.trim().to_ascii_lowercase();
        let mut results: Vec<AgentDescriptor> = lock
            .values()
            .filter(|d| d.tags.iter().any(|t| t.to_ascii_lowercase() == target))
            .cloned()
            .collect();
        results.sort_by(|a, b| a.id.cmp(&b.id));
        results
    }

    /// List all currently active agents ([`AgentState::Running`] or [`AgentState::Paused`]).
    pub fn list_active(&self) -> Vec<AgentDescriptor> {
        let lock = self.descriptors.read().unwrap_or_else(|e| e.into_inner());
        let mut results: Vec<AgentDescriptor> = lock
            .values()
            .filter(|d| d.status.is_active())
            .cloned()
            .collect();
        results.sort_by(|a, b| a.id.cmp(&b.id));
        results
    }

    /// List all registered agent descriptors.
    pub fn list_all(&self) -> Vec<AgentDescriptor> {
        let lock = self.descriptors.read().unwrap_or_else(|e| e.into_inner());
        let mut results: Vec<AgentDescriptor> = lock.values().cloned().collect();
        results.sort_by(|a, b| a.id.cmp(&b.id));
        results
    }

    /// Number of agents registered in the catalog.
    pub fn len(&self) -> usize {
        let lock = self.descriptors.read().unwrap_or_else(|e| e.into_inner());
        lock.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clear all registered agent descriptors.
    pub fn clear(&self) {
        if let Ok(mut lock) = self.descriptors.write() {
            lock.clear();
        }
    }

    /// Update the lifecycle state of a registered agent.
    pub fn update_status(&self, id: &AgentId, status: AgentState) -> Result<()> {
        let mut lock = self.descriptors.write().map_err(|e| {
            CortexError::Internal(format!(
                "failed to acquire agent registry write lock: {}",
                e
            ))
        })?;
        if let Some(desc) = lock.get_mut(id) {
            desc.status = status;
            Ok(())
        } else {
            Err(CortexError::NotFound(format!(
                "agent '{}' not found in registry",
                id
            )))
        }
    }

    /// Synchronize registry state with an [`AgentLifecycleEvent`].
    pub fn handle_event(&self, event: &AgentLifecycleEvent) {
        match event {
            AgentLifecycleEvent::AgentCreated {
                agent_id,
                name,
                role,
                ..
            } => {
                let _ = self.upsert(AgentDescriptor::new(
                    agent_id.clone(),
                    name.clone(),
                    role.clone(),
                    AgentState::Created,
                    PathBuf::new(),
                ));
            }
            AgentLifecycleEvent::AgentReady { agent_id, .. } => {
                let _ = self.update_status(agent_id, AgentState::Ready);
            }
            AgentLifecycleEvent::AgentStarted { agent_id, .. }
            | AgentLifecycleEvent::AgentResumed { agent_id, .. } => {
                let _ = self.update_status(agent_id, AgentState::Running);
            }
            AgentLifecycleEvent::AgentPaused { agent_id, .. } => {
                let _ = self.update_status(agent_id, AgentState::Paused);
            }
            AgentLifecycleEvent::AgentStopped { agent_id, .. } => {
                let _ = self.update_status(agent_id, AgentState::Stopped);
            }
            AgentLifecycleEvent::AgentFailed { agent_id, .. } => {
                let _ = self.update_status(agent_id, AgentState::Failed);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_capability_matching() {
        let tool_cap = AgentCapability::tool("read_file");
        assert!(tool_cap.matches("read_file"));
        assert!(tool_cap.matches("READ_FILE"));
        assert!(tool_cap.matches("tool:read_file"));
        assert!(!tool_cap.matches("write_file"));

        let git_cap = AgentCapability::tool("git_commit");
        assert!(git_cap.matches("git_*"));
        assert!(git_cap.matches("git_commit"));

        let tier_cap = AgentCapability::model_tier("fast");
        assert!(tier_cap.matches("fast"));
        assert!(tier_cap.matches("tier:fast"));
        assert!(tier_cap.matches("model_tier:fast"));
        assert!(!tier_cap.matches("reasoning"));

        let domain_cap = AgentCapability::domain("researcher");
        assert!(domain_cap.matches("researcher"));
        assert!(domain_cap.matches("domain:researcher"));
        assert!(!domain_cap.matches("coder"));
    }

    #[test]
    fn test_agent_registry_crud_and_lookups() {
        let registry = AgentRegistry::new();
        assert!(registry.is_empty());

        let id1 = AgentId::generate();
        let desc1 = AgentDescriptor::new(
            id1.clone(),
            "Researcher One",
            "Lead Researcher",
            AgentState::Ready,
            PathBuf::from("/tmp/research"),
        )
        .with_capability(AgentCapability::tool("read_file"))
        .with_capability(AgentCapability::domain("researcher"))
        .with_capability(AgentCapability::model_tier("reasoning"))
        .with_tag("nlp");

        assert!(registry.register(desc1.clone()).is_ok());
        assert_eq!(registry.len(), 1);
        assert!(!registry.is_empty());
        assert!(registry.contains(&id1));

        // Duplicate registration fails
        assert!(registry.register(desc1).is_err());

        // Get by ID
        let fetched = registry.get(&id1).expect("descriptor should exist");
        assert_eq!(fetched.name, "Researcher One");
        assert_eq!(fetched.status, AgentState::Ready);

        // Find by role
        let researchers = registry.find_by_role("Researcher");
        assert_eq!(researchers.len(), 1);
        assert_eq!(researchers[0].id, id1);

        let non_existent_role = registry.find_by_role("Accountant");
        assert!(non_existent_role.is_empty());

        // Find by capability
        let tool_matches = registry.find_by_capability("read_file");
        assert_eq!(tool_matches.len(), 1);

        let domain_matches = registry.find_by_capability("domain:researcher");
        assert_eq!(domain_matches.len(), 1);

        let tag_matches = registry.find_by_tag("nlp");
        assert_eq!(tag_matches.len(), 1);

        // Update status
        assert!(registry.update_status(&id1, AgentState::Running).is_ok());
        assert_eq!(registry.get(&id1).unwrap().status, AgentState::Running);

        // List active
        let active = registry.list_active();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, id1);

        // Update to stopped -> no longer active
        assert!(registry.update_status(&id1, AgentState::Stopped).is_ok());
        assert!(registry.list_active().is_empty());

        // Deregister
        assert!(registry.deregister(&id1).is_ok());
        assert_eq!(registry.len(), 0);
        assert!(registry.get(&id1).is_none());

        // Deregister non-existent fails
        assert!(registry.deregister(&id1).is_err());
    }

    #[test]
    fn test_concurrent_registry_access() {
        use std::thread;

        let registry = Arc::new(AgentRegistry::new());
        let mut handles = Vec::new();

        // Spawn 10 writer threads each registering an agent
        for i in 0..10 {
            let reg = Arc::clone(&registry);
            handles.push(thread::spawn(move || {
                let id = AgentId::generate();
                let desc = AgentDescriptor::new(
                    id.clone(),
                    format!("Agent-{}", i),
                    "Worker",
                    AgentState::Running,
                    PathBuf::from("/workspace"),
                )
                .with_capability(AgentCapability::tool("shell"));
                reg.register(desc).unwrap();
            }));
        }

        // Spawn 10 reader threads querying concurrently
        for _ in 0..10 {
            let reg = Arc::clone(&registry);
            handles.push(thread::spawn(move || {
                let _ = reg.find_by_role("Worker");
                let _ = reg.find_by_capability("shell");
                let _ = reg.list_active();
            }));
        }

        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(registry.len(), 10);
        assert_eq!(registry.list_active().len(), 10);
    }

    #[test]
    fn test_compound_capability_queries_and_ranking() {
        let registry = AgentRegistry::new();

        let id1 = AgentId::generate();
        let desc1 = AgentDescriptor::new(
            id1.clone(),
            "Fullstack Coder",
            "Software Engineer",
            AgentState::Running,
            PathBuf::from("/ws1"),
        )
        .with_capability(AgentCapability::tool("read_file"))
        .with_capability(AgentCapability::tool("git_diff"))
        .with_capability(AgentCapability::model_tier("coding"))
        .with_capability(AgentCapability::domain("coder"))
        .with_tag("backend");

        let id2 = AgentId::generate();
        let desc2 = AgentDescriptor::new(
            id2.clone(),
            "Junior Reviewer",
            "Code Reviewer",
            AgentState::Paused,
            PathBuf::from("/ws2"),
        )
        .with_capability(AgentCapability::tool("read_file"))
        .with_capability(AgentCapability::model_tier("fast"))
        .with_capability(AgentCapability::domain("reviewer"))
        .with_tag("qa");

        let id3 = AgentId::generate();
        let desc3 = AgentDescriptor::new(
            id3.clone(),
            "Offline Specialist",
            "Security Auditor",
            AgentState::Stopped,
            PathBuf::from("/ws3"),
        )
        .with_capability(AgentCapability::tool("read_file"))
        .with_capability(AgentCapability::tool("shell"))
        .with_capability(AgentCapability::model_tier("reasoning"))
        .with_tag("security");

        registry.register(desc1).unwrap();
        registry.register(desc2).unwrap();
        registry.register(desc3).unwrap();

        // 1. find_by_all_capabilities (AND)
        // Only desc1 has both "git_diff" and "domain:coder"
        let and_matches = registry.find_by_all_capabilities(&["git_diff", "domain:coder"]);
        assert_eq!(and_matches.len(), 1);
        assert_eq!(and_matches[0].id, id1);

        // All 3 have "read_file"
        let all_readers = registry.find_by_all_capabilities(&["read_file"]);
        assert_eq!(all_readers.len(), 3);

        // Empty queries returns all
        assert_eq!(registry.find_by_all_capabilities(&[] as &[&str]).len(), 3);

        // None match all of these
        let no_matches = registry.find_by_all_capabilities(&["git_diff", "shell"]);
        assert!(no_matches.is_empty());

        // 2. find_by_any_capability (OR)
        let or_matches = registry.find_by_any_capability(&["git_diff", "shell"]);
        assert_eq!(or_matches.len(), 2);
        let or_ids: Vec<AgentId> = or_matches.into_iter().map(|d| d.id).collect();
        assert!(or_ids.contains(&id1));
        assert!(or_ids.contains(&id3));

        // 3. rank_by_capabilities
        // Query: ["read_file", "git_diff", "coding", "backend"]
        // desc1 matches all 4 (score 4)
        // desc2 matches "read_file" (score 1)
        // desc3 matches "read_file" (score 1)
        let ranked = registry.rank_by_capabilities(&["read_file", "git_diff", "coding", "backend"]);
        assert_eq!(ranked.len(), 3);
        assert_eq!(ranked[0].0.id, id1);
        assert_eq!(ranked[0].1, 4);

        // find_best_match
        let best = registry.find_best_match(&["git_diff", "coder"]);
        assert_eq!(best.unwrap().id, id1);

        // 4. find_active_by_capability & find_active_by_all_capabilities
        // Active = desc1 (Running) and desc2 (Paused); desc3 is Stopped
        let active_readers = registry.find_active_by_capability("read_file");
        assert_eq!(active_readers.len(), 2);
        let active_ids: Vec<AgentId> = active_readers.into_iter().map(|d| d.id).collect();
        assert!(active_ids.contains(&id1));
        assert!(active_ids.contains(&id2));
        assert!(!active_ids.contains(&id3));

        let active_shell = registry.find_active_by_capability("shell");
        assert!(active_shell.is_empty()); // desc3 has shell, but is Stopped

        let active_and = registry.find_active_by_all_capabilities(&["read_file", "coding"]);
        assert_eq!(active_and.len(), 1);
        assert_eq!(active_and[0].id, id1);
    }
}
