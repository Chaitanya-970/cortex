//! AgentManager coordinator for managing persistent agent worker lifecycles.

use super::lifecycle::{AgentLifecycleEvent, AgentState};
use chrono::Utc;
use cortex_core::{AgentId, CortexError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, RwLock};

/// Configuration for an agent's model provider and inference parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentModelConfig {
    /// Model identifier or name (e.g., "claude-3-5-sonnet", "gpt-4o", "mock").
    pub model: String,
    /// Model provider identifier (e.g., "anthropic", "openai", "mock").
    pub provider: String,
    /// Sampling temperature (0.0 to 2.0).
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    /// Maximum completion tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,
    /// Additional provider-specific parameters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

fn default_temperature() -> f32 {
    0.7
}

impl AgentModelConfig {
    /// Create a new [`AgentModelConfig`] with default temperature and parameters.
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
            temperature: default_temperature(),
            max_tokens: None,
            parameters: None,
        }
    }

    /// Set a custom sampling temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    /// Set a maximum token limit.
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }
}

/// Operational permission boundaries for an agent.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AgentPermissions {
    /// Allowed filesystem operations (e.g. `["read", "write"]`).
    #[serde(default)]
    pub filesystem: Vec<String>,
    /// Allowed network destinations or hosts.
    #[serde(default)]
    pub network: Vec<String>,
    /// Allowed shell commands or command patterns.
    #[serde(default)]
    pub shell: Vec<String>,
}

impl AgentPermissions {
    /// Create full permissions with read/write filesystem and network access.
    pub fn standard() -> Self {
        Self {
            filesystem: vec!["read".to_string(), "write".to_string()],
            network: vec!["allow".to_string()],
            shell: vec!["*".to_string()],
        }
    }

    /// Create read-only permissions restricting mutations.
    pub fn read_only() -> Self {
        Self {
            filesystem: vec!["read".to_string()],
            network: Vec::new(),
            shell: Vec::new(),
        }
    }
}

/// Declarative manifest used to instantiate a new agent worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentManifest {
    /// Optional explicitly specified ID (generated if not provided).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<AgentId>,
    /// Human-readable agent worker name.
    pub name: String,
    /// Operational role and task domain (e.g., "Code Reviewer", "Software Engineer").
    pub role: String,
    /// System prompt or custom operational policy instructions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Assigned workspace directory boundary.
    pub workspace: PathBuf,
    /// Model provider and parameter configuration.
    pub model: AgentModelConfig,
    /// Allowed tool definitions or names.
    #[serde(default)]
    pub tools: Vec<String>,
    /// Capability and operational permission boundaries.
    #[serde(default)]
    pub permissions: AgentPermissions,
}

impl AgentManifest {
    /// Create a new basic manifest with default configurations.
    pub fn new(
        name: impl Into<String>,
        role: impl Into<String>,
        workspace: impl AsRef<Path>,
        model: AgentModelConfig,
    ) -> Self {
        Self {
            id: None,
            name: name.into(),
            role: role.into(),
            system_prompt: None,
            workspace: workspace.as_ref().to_path_buf(),
            model,
            tools: Vec::new(),
            permissions: AgentPermissions::standard(),
        }
    }

    /// Attach an explicit [`AgentId`].
    pub fn with_id(mut self, id: AgentId) -> Self {
        self.id = Some(id);
        self
    }

    /// Attach a custom system prompt.
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Attach tool names to the manifest.
    pub fn with_tools(mut self, tools: Vec<String>) -> Self {
        self.tools = tools;
        self
    }

    /// Attach permissions to the manifest.
    pub fn with_permissions(mut self, permissions: AgentPermissions) -> Self {
        self.permissions = permissions;
        self
    }

    /// Deserialize an [`AgentManifest`] from a JSON string.
    pub fn from_json_str(json_str: &str) -> Result<Self> {
        serde_json::from_str(json_str)
            .map_err(|e| CortexError::Validation(format!("invalid json manifest: {}", e)))
    }

    /// Deserialize an [`AgentManifest`] from a TOML string.
    pub fn from_toml_str(toml_str: &str) -> Result<Self> {
        toml::from_str(toml_str)
            .map_err(|e| CortexError::Validation(format!("invalid toml manifest: {}", e)))
    }

    /// Serialize this [`AgentManifest`] to a pretty-printed JSON string.
    pub fn to_json_string(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| {
            CortexError::Internal(format!("failed to serialize manifest to json: {}", e))
        })
    }

    /// Serialize this [`AgentManifest`] to a TOML string.
    pub fn to_toml_string(&self) -> Result<String> {
        toml::to_string_pretty(self).map_err(|e| {
            CortexError::Internal(format!("failed to serialize manifest to toml: {}", e))
        })
    }
}

/// A persistent agent worker managed by the runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Agent {
    /// Unique stable identifier.
    pub id: AgentId,
    /// Human-readable name.
    pub name: String,
    /// Operational role definition.
    pub role: String,
    /// Custom system prompt or operational instructions.
    pub system_prompt: Option<String>,
    /// Current lifecycle state.
    pub state: AgentState,
    /// Assigned workspace boundary path.
    pub workspace: PathBuf,
    /// Model provider configuration.
    pub model: AgentModelConfig,
    /// Configured tool capabilities.
    pub tools: Vec<String>,
    /// Granted permission boundaries.
    pub permissions: AgentPermissions,
    /// Creation timestamp in ISO 8601 UTC.
    pub created_at: String,
    /// Last state transition timestamp in ISO 8601 UTC.
    pub updated_at: String,
}

/// Thread-safe manager responsible for agent workers and their lifecycle transitions.
#[derive(Debug, Clone, Default)]
pub struct AgentManager {
    pub(super) agents: Arc<RwLock<HashMap<AgentId, Agent>>>,
    pub(super) coordination: Arc<Mutex<super::coordination::CoordinationState>>,
    events: Arc<RwLock<Vec<AgentLifecycleEvent>>>,
    subscribers: Arc<Mutex<Vec<Sender<AgentLifecycleEvent>>>>,
}

impl AgentManager {
    /// Create a new, empty [`AgentManager`].
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            coordination: Arc::new(Mutex::new(super::coordination::CoordinationState::default())),
            events: Arc::new(RwLock::new(Vec::new())),
            subscribers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Register a new agent worker from a manifest.
    ///
    /// Validates the manifest and transitions the new agent to [`AgentState::Created`].
    pub fn create(&self, manifest: AgentManifest) -> Result<Agent> {
        let name = manifest.name.trim();
        if name.is_empty() {
            return Err(CortexError::Validation(
                "agent name cannot be empty".to_string(),
            ));
        }

        let role = manifest.role.trim();
        if role.is_empty() {
            return Err(CortexError::Validation(
                "agent role cannot be empty".to_string(),
            ));
        }

        let id = manifest.id.unwrap_or_else(AgentId::generate);
        let now = Utc::now().to_rfc3339();

        let agent = Agent {
            id: id.clone(),
            name: name.to_string(),
            role: role.to_string(),
            system_prompt: manifest.system_prompt,
            state: AgentState::Created,
            workspace: manifest.workspace,
            model: manifest.model,
            tools: manifest.tools,
            permissions: manifest.permissions,
            created_at: now.clone(),
            updated_at: now.clone(),
        };

        {
            let mut lock = self
                .agents
                .write()
                .map_err(|e| CortexError::Internal(format!("agents lock poisoned: {}", e)))?;
            if lock.contains_key(&id) {
                return Err(CortexError::Validation(format!(
                    "agent with id '{}' already exists",
                    id
                )));
            }
            lock.insert(id.clone(), agent.clone());
        }

        self.emit_event(AgentLifecycleEvent::AgentCreated {
            agent_id: id,
            name: name.to_string(),
            role: role.to_string(),
            timestamp: now,
        });

        Ok(agent)
    }

    /// Prepare an agent for execution, moving it from [`AgentState::Created`] to [`AgentState::Ready`].
    pub fn prepare(&self, agent_id: &AgentId) -> Result<()> {
        let mut lock = self
            .agents
            .write()
            .map_err(|e| CortexError::Internal(format!("agents lock poisoned: {}", e)))?;
        let agent = lock
            .get_mut(agent_id)
            .ok_or_else(|| CortexError::NotFound(format!("agent '{}' not found", agent_id)))?;

        if agent.state == AgentState::Ready {
            return Ok(());
        }

        let mut coordination = self
            .coordination
            .lock()
            .map_err(super::coordination::lock_error)?;
        agent.state.transition_to(AgentState::Ready)?;
        let now = Utc::now().to_rfc3339();
        agent.updated_at = now.clone();

        let waker = coordination.revoke(agent_id);
        drop(coordination);
        self.emit_event(AgentLifecycleEvent::AgentReady {
            agent_id: agent_id.clone(),
            timestamp: now,
        });
        drop(lock);
        if let Some(waker) = waker {
            waker.wake();
        }

        Ok(())
    }

    /// Transition an agent to [`AgentState::Running`].
    ///
    /// Permitted from [`AgentState::Created`], [`AgentState::Ready`], or [`AgentState::Paused`].
    /// Stopped agents cannot be started directly and must be restarted first.
    pub fn start(&self, agent_id: &AgentId) -> Result<()> {
        let mut lock = self
            .agents
            .write()
            .map_err(|e| CortexError::Internal(format!("agents lock poisoned: {}", e)))?;
        let agent = lock
            .get_mut(agent_id)
            .ok_or_else(|| CortexError::NotFound(format!("agent '{}' not found", agent_id)))?;

        match agent.state {
            AgentState::Running => {
                // Already running, idempotent success.
                return Ok(());
            }
            AgentState::Stopped => {
                return Err(CortexError::Validation(format!(
                    "cannot start stopped agent '{}'; use restart() to reset before running",
                    agent_id
                )));
            }
            AgentState::Failed => {
                return Err(CortexError::Validation(format!(
                    "cannot start failed agent '{}'; use restart() to reset before running",
                    agent_id
                )));
            }
            AgentState::Created | AgentState::Ready | AgentState::Paused => {
                agent.state.transition_to(AgentState::Running)?;
                let now = Utc::now().to_rfc3339();
                agent.updated_at = now.clone();

                self.emit_event(AgentLifecycleEvent::AgentStarted {
                    agent_id: agent_id.clone(),
                    timestamp: now,
                });
            }
        }

        Ok(())
    }

    /// Transition an agent to [`AgentState::Paused`].
    ///
    /// Only running agents may be paused.
    pub fn pause(&self, agent_id: &AgentId) -> Result<()> {
        let mut lock = self
            .agents
            .write()
            .map_err(|e| CortexError::Internal(format!("agents lock poisoned: {}", e)))?;
        let agent = lock
            .get_mut(agent_id)
            .ok_or_else(|| CortexError::NotFound(format!("agent '{}' not found", agent_id)))?;

        if agent.state == AgentState::Paused {
            return Ok(());
        }

        if agent.state != AgentState::Running {
            return Err(CortexError::Validation(format!(
                "cannot pause agent '{}' in state '{}'; only running agents can be paused",
                agent_id, agent.state
            )));
        }

        let mut coordination = self
            .coordination
            .lock()
            .map_err(super::coordination::lock_error)?;
        agent.state.transition_to(AgentState::Paused)?;
        let now = Utc::now().to_rfc3339();
        agent.updated_at = now.clone();

        let waker = coordination.notify(agent_id);
        drop(coordination);
        self.emit_event(AgentLifecycleEvent::AgentPaused {
            agent_id: agent_id.clone(),
            timestamp: now,
        });
        drop(lock);
        if let Some(waker) = waker {
            waker.wake();
        }

        Ok(())
    }

    /// Resume a paused agent back to [`AgentState::Running`].
    pub fn resume(&self, agent_id: &AgentId) -> Result<()> {
        let mut lock = self
            .agents
            .write()
            .map_err(|e| CortexError::Internal(format!("agents lock poisoned: {}", e)))?;
        let agent = lock
            .get_mut(agent_id)
            .ok_or_else(|| CortexError::NotFound(format!("agent '{}' not found", agent_id)))?;

        if agent.state == AgentState::Running {
            return Ok(());
        }

        if agent.state != AgentState::Paused {
            return Err(CortexError::Validation(format!(
                "cannot resume agent '{}' in state '{}'; only paused agents can be resumed",
                agent_id, agent.state
            )));
        }

        agent.state.transition_to(AgentState::Running)?;
        let now = Utc::now().to_rfc3339();
        agent.updated_at = now.clone();

        self.emit_event(AgentLifecycleEvent::AgentResumed {
            agent_id: agent_id.clone(),
            timestamp: now,
        });

        Ok(())
    }

    /// Stop an agent worker, moving it to [`AgentState::Stopped`].
    pub fn stop(&self, agent_id: &AgentId) -> Result<()> {
        let mut lock = self
            .agents
            .write()
            .map_err(|e| CortexError::Internal(format!("agents lock poisoned: {}", e)))?;
        let agent = lock
            .get_mut(agent_id)
            .ok_or_else(|| CortexError::NotFound(format!("agent '{}' not found", agent_id)))?;

        if agent.state == AgentState::Stopped {
            return Ok(());
        }

        let mut coordination = self
            .coordination
            .lock()
            .map_err(super::coordination::lock_error)?;
        agent.state.transition_to(AgentState::Stopped)?;
        let now = Utc::now().to_rfc3339();
        agent.updated_at = now.clone();

        let waker = coordination.revoke(agent_id);
        drop(coordination);
        self.emit_event(AgentLifecycleEvent::AgentStopped {
            agent_id: agent_id.clone(),
            reason: None,
            timestamp: now,
        });
        drop(lock);
        if let Some(waker) = waker {
            waker.wake();
        }

        Ok(())
    }

    /// Restart a stopped or failed agent, resetting its lifecycle state to [`AgentState::Ready`].
    pub fn restart(&self, agent_id: &AgentId) -> Result<()> {
        let mut lock = self
            .agents
            .write()
            .map_err(|e| CortexError::Internal(format!("agents lock poisoned: {}", e)))?;
        let agent = lock
            .get_mut(agent_id)
            .ok_or_else(|| CortexError::NotFound(format!("agent '{}' not found", agent_id)))?;

        let mut coordination = self
            .coordination
            .lock()
            .map_err(super::coordination::lock_error)?;
        agent.state.transition_to(AgentState::Ready)?;
        let now = Utc::now().to_rfc3339();
        agent.updated_at = now.clone();

        let waker = coordination.revoke(agent_id);
        drop(coordination);
        self.emit_event(AgentLifecycleEvent::AgentReady {
            agent_id: agent_id.clone(),
            timestamp: now,
        });
        drop(lock);
        if let Some(waker) = waker {
            waker.wake();
        }

        Ok(())
    }

    /// Mark an agent as failed due to an unrecoverable execution error.
    pub fn fail(&self, agent_id: &AgentId, error: impl Into<String>) -> Result<()> {
        let err_str = error.into();
        let mut lock = self
            .agents
            .write()
            .map_err(|e| CortexError::Internal(format!("agents lock poisoned: {}", e)))?;
        let agent = lock
            .get_mut(agent_id)
            .ok_or_else(|| CortexError::NotFound(format!("agent '{}' not found", agent_id)))?;

        let mut coordination = self
            .coordination
            .lock()
            .map_err(super::coordination::lock_error)?;
        agent.state.transition_to(AgentState::Failed)?;
        let now = Utc::now().to_rfc3339();
        agent.updated_at = now.clone();

        let waker = coordination.revoke(agent_id);
        drop(coordination);
        self.emit_event(AgentLifecycleEvent::AgentFailed {
            agent_id: agent_id.clone(),
            error: err_str,
            timestamp: now,
        });
        drop(lock);
        if let Some(waker) = waker {
            waker.wake();
        }

        Ok(())
    }

    /// Inspect details, configuration, and state for a single agent.
    pub fn inspect(&self, agent_id: &AgentId) -> Result<Agent> {
        let lock = self
            .agents
            .read()
            .map_err(|e| CortexError::Internal(format!("agents lock poisoned: {}", e)))?;
        lock.get(agent_id)
            .cloned()
            .ok_or_else(|| CortexError::NotFound(format!("agent '{}' not found", agent_id)))
    }

    /// List all registered agents.
    pub fn list(&self) -> Vec<Agent> {
        let lock = self.agents.read().unwrap_or_else(|e| e.into_inner());
        let mut list: Vec<Agent> = lock.values().cloned().collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        list
    }

    /// Remove a stopped or created agent from management.
    ///
    /// Running or paused agents cannot be removed until stopped.
    pub fn remove(&self, agent_id: &AgentId) -> Result<Agent> {
        let mut lock = self
            .agents
            .write()
            .map_err(|e| CortexError::Internal(format!("agents lock poisoned: {}", e)))?;

        if let Some(agent) = lock.get(agent_id) {
            if agent.state.is_active() {
                return Err(CortexError::Validation(format!(
                    "cannot remove agent '{}' while in active state '{}'; stop it first",
                    agent_id, agent.state
                )));
            }
        }

        let mut coordination = self
            .coordination
            .lock()
            .map_err(super::coordination::lock_error)?;
        let agent = lock
            .remove(agent_id)
            .ok_or_else(|| CortexError::NotFound(format!("agent '{}' not found", agent_id)))?;
        let waker = coordination.remove(agent_id);
        drop(coordination);
        drop(lock);
        if let Some(waker) = waker {
            waker.wake();
        }
        Ok(agent)
    }

    /// Subscribe to the live stream of agent lifecycle events.
    pub fn subscribe(&self) -> Receiver<AgentLifecycleEvent> {
        let (tx, rx) = channel();
        if let Ok(mut subs) = self.subscribers.lock() {
            subs.push(tx);
        }
        rx
    }

    /// Access the historical sequence of recorded lifecycle events.
    pub fn events(&self) -> Vec<AgentLifecycleEvent> {
        let lock = self.events.read().unwrap_or_else(|e| e.into_inner());
        lock.clone()
    }

    /// Retrieve recorded lifecycle events specific to the given [`AgentId`].
    pub fn events_for_agent(&self, agent_id: &AgentId) -> Vec<AgentLifecycleEvent> {
        let lock = self.events.read().unwrap_or_else(|e| e.into_inner());
        lock.iter()
            .filter(|e| e.agent_id() == agent_id)
            .cloned()
            .collect()
    }

    fn emit_event(&self, event: AgentLifecycleEvent) {
        if let Ok(mut events) = self.events.write() {
            events.push(event.clone());
        }

        if let Ok(mut subs) = self.subscribers.lock() {
            subs.retain(|tx| tx.send(event.clone()).is_ok());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manifest(name: &str) -> AgentManifest {
        AgentManifest::new(
            name,
            "Rust Developer",
            "/tmp/workspace",
            AgentModelConfig::new("mock", "gpt-4o"),
        )
    }

    #[test]
    fn test_agent_creation_and_inspect() {
        let manager = AgentManager::new();
        let manifest = test_manifest("reviewer");
        let agent = manager.create(manifest).expect("agent creation");

        assert_eq!(agent.name, "reviewer");
        assert_eq!(agent.role, "Rust Developer");
        assert_eq!(agent.state, AgentState::Created);

        let inspected = manager.inspect(&agent.id).expect("inspect");
        assert_eq!(inspected.id, agent.id);
        assert_eq!(inspected.state, AgentState::Created);
    }

    #[test]
    fn test_lifecycle_flow_start_pause_resume_stop() {
        let manager = AgentManager::new();
        let agent = manager.create(test_manifest("worker")).unwrap();
        let id = agent.id;

        // Start
        manager.start(&id).unwrap();
        assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Running);

        // Pause
        manager.pause(&id).unwrap();
        assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Paused);

        // Resume
        manager.resume(&id).unwrap();
        assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Running);

        // Stop
        manager.stop(&id).unwrap();
        assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Stopped);

        // Cannot start directly when stopped
        let err = manager.start(&id).unwrap_err();
        assert!(err.to_string().contains("cannot start stopped agent"));

        // Restart to Ready
        manager.restart(&id).unwrap();
        assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Ready);

        // Start from Ready
        manager.start(&id).unwrap();
        assert_eq!(manager.inspect(&id).unwrap().state, AgentState::Running);
    }

    #[test]
    fn test_event_streaming_subscription() {
        let manager = AgentManager::new();
        let rx = manager.subscribe();

        let agent = manager.create(test_manifest("stream-test")).unwrap();
        let id = agent.id;
        manager.start(&id).unwrap();
        manager.stop(&id).unwrap();

        let ev1 = rx.recv().unwrap();
        assert_eq!(ev1.event_type(), "AgentCreated");

        let ev2 = rx.recv().unwrap();
        assert_eq!(ev2.event_type(), "AgentStarted");

        let ev3 = rx.recv().unwrap();
        assert_eq!(ev3.event_type(), "AgentStopped");

        let history = manager.events_for_agent(&id);
        assert_eq!(history.len(), 3);
    }

    #[test]
    fn test_thread_safe_concurrent_operations() {
        use std::thread;

        let manager = Arc::new(AgentManager::new());
        let mut handles = Vec::new();

        for i in 0..10 {
            let m = Arc::clone(&manager);
            handles.push(thread::spawn(move || {
                let manifest = test_manifest(&format!("worker_{}", i));
                let agent = m.create(manifest).unwrap();
                m.start(&agent.id).unwrap();
                m.inspect(&agent.id).unwrap();
                m.stop(&agent.id).unwrap();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(manager.list().len(), 10);
    }

    #[test]
    fn test_remove_validation() {
        let manager = AgentManager::new();
        let agent = manager.create(test_manifest("removable")).unwrap();

        // Start agent
        manager.start(&agent.id).unwrap();

        // Removing active agent fails
        let err = manager.remove(&agent.id).unwrap_err();
        assert!(err.to_string().contains("cannot remove agent"));

        // Stop and remove succeeds
        manager.stop(&agent.id).unwrap();
        assert!(manager.remove(&agent.id).is_ok());
        assert!(manager.inspect(&agent.id).is_err());
    }
}
