//! Lifecycle state machine and structured lifecycle events for persistent agents.

use cortex_core::{AgentId, CortexError, Result};
use serde::{Deserialize, Serialize};

/// Explicit lifecycle states for a persistent Cortex agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentState {
    /// Agent manifest has been registered but not yet initialized or started.
    Created,
    /// Agent configuration, workspace, and tools are verified and ready to run.
    Ready,
    /// Agent is actively executing tasks.
    Running,
    /// Agent execution has been temporarily suspended.
    Paused,
    /// Agent has stopped execution cleanly or finished.
    Stopped,
    /// Agent has encountered an unrecoverable failure.
    Failed,
}

impl AgentState {
    /// Validate whether a transition from `self` to `target` is permitted by the state machine.
    pub fn can_transition_to(&self, target: AgentState) -> bool {
        if *self == target {
            return true;
        }

        match (*self, target) {
            // From Created: can move to Ready, Running, Stopped, or Failed.
            (AgentState::Created, AgentState::Ready) => true,
            (AgentState::Created, AgentState::Running) => true,
            (AgentState::Created, AgentState::Stopped) => true,
            (AgentState::Created, AgentState::Failed) => true,
            (AgentState::Created, AgentState::Paused) => false,

            // From Ready: can start Running, be Stopped, or enter Failed.
            (AgentState::Ready, AgentState::Running) => true,
            (AgentState::Ready, AgentState::Stopped) => true,
            (AgentState::Ready, AgentState::Failed) => true,
            (AgentState::Ready, AgentState::Paused) => false,
            (AgentState::Ready, AgentState::Created) => false,

            // From Running: can be Paused, Stopped, return to Ready, or Fail.
            (AgentState::Running, AgentState::Paused) => true,
            (AgentState::Running, AgentState::Stopped) => true,
            (AgentState::Running, AgentState::Ready) => true,
            (AgentState::Running, AgentState::Failed) => true,
            (AgentState::Running, AgentState::Created) => false,

            // From Paused: can Resume to Running, be Stopped, or Fail.
            (AgentState::Paused, AgentState::Running) => true,
            (AgentState::Paused, AgentState::Stopped) => true,
            (AgentState::Paused, AgentState::Ready) => true,
            (AgentState::Paused, AgentState::Failed) => true,
            (AgentState::Paused, AgentState::Created) => false,

            // From Stopped: can only transition to Ready via restart; direct Running/Paused forbidden.
            (AgentState::Stopped, AgentState::Ready) => true,
            (AgentState::Stopped, AgentState::Running) => false,
            (AgentState::Stopped, AgentState::Paused) => false,
            (AgentState::Stopped, AgentState::Failed) => false,
            (AgentState::Stopped, AgentState::Created) => false,

            // From Failed: can only be re-initialized to Ready via restart, or transition to Stopped.
            (AgentState::Failed, AgentState::Ready) => true,
            (AgentState::Failed, AgentState::Stopped) => true,
            (AgentState::Failed, AgentState::Running) => false,
            (AgentState::Failed, AgentState::Paused) => false,
            (AgentState::Failed, AgentState::Created) => false,

            _ => false,
        }
    }

    /// Transition to `target`, returning a descriptive validation error if the transition is prohibited.
    pub fn transition_to(&mut self, target: AgentState) -> Result<()> {
        if self.can_transition_to(target) {
            *self = target;
            Ok(())
        } else {
            Err(CortexError::Validation(format!(
                "invalid state transition: cannot transition agent from '{:?}' to '{:?}'",
                self, target
            )))
        }
    }

    /// Check if the agent is currently in an active execution state.
    pub fn is_active(&self) -> bool {
        matches!(self, AgentState::Running | AgentState::Paused)
    }

    /// Check if the agent is in a terminal or idle state.
    pub fn is_terminal(&self) -> bool {
        matches!(self, AgentState::Stopped | AgentState::Failed)
    }
}

impl std::fmt::Display for AgentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentState::Created => write!(f, "created"),
            AgentState::Ready => write!(f, "ready"),
            AgentState::Running => write!(f, "running"),
            AgentState::Paused => write!(f, "paused"),
            AgentState::Stopped => write!(f, "stopped"),
            AgentState::Failed => write!(f, "failed"),
        }
    }
}

/// Structured lifecycle event emitted when an agent transitions state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum AgentLifecycleEvent {
    /// Agent was created from a manifest.
    AgentCreated {
        /// Identifier of the agent.
        agent_id: AgentId,
        /// Name of the agent.
        name: String,
        /// Operational role of the agent.
        role: String,
        /// Timestamp in ISO 8601 UTC.
        timestamp: String,
    },
    /// Agent transitioned to Ready state.
    AgentReady {
        /// Identifier of the agent.
        agent_id: AgentId,
        /// Timestamp in ISO 8601 UTC.
        timestamp: String,
    },
    /// Agent was started or resumed into Running state.
    AgentStarted {
        /// Identifier of the agent.
        agent_id: AgentId,
        /// Timestamp in ISO 8601 UTC.
        timestamp: String,
    },
    /// Agent execution was paused.
    AgentPaused {
        /// Identifier of the agent.
        agent_id: AgentId,
        /// Timestamp in ISO 8601 UTC.
        timestamp: String,
    },
    /// Agent execution was resumed from paused state.
    AgentResumed {
        /// Identifier of the agent.
        agent_id: AgentId,
        /// Timestamp in ISO 8601 UTC.
        timestamp: String,
    },
    /// Agent was stopped.
    AgentStopped {
        /// Identifier of the agent.
        agent_id: AgentId,
        /// Optional reason for stopping.
        reason: Option<String>,
        /// Timestamp in ISO 8601 UTC.
        timestamp: String,
    },
    /// Agent encountered an unrecoverable failure.
    AgentFailed {
        /// Identifier of the agent.
        agent_id: AgentId,
        /// Failure error description.
        error: String,
        /// Timestamp in ISO 8601 UTC.
        timestamp: String,
    },
}

impl AgentLifecycleEvent {
    /// Retrieve the [`AgentId`] associated with this lifecycle event.
    pub fn agent_id(&self) -> &AgentId {
        match self {
            AgentLifecycleEvent::AgentCreated { agent_id, .. }
            | AgentLifecycleEvent::AgentReady { agent_id, .. }
            | AgentLifecycleEvent::AgentStarted { agent_id, .. }
            | AgentLifecycleEvent::AgentPaused { agent_id, .. }
            | AgentLifecycleEvent::AgentResumed { agent_id, .. }
            | AgentLifecycleEvent::AgentStopped { agent_id, .. }
            | AgentLifecycleEvent::AgentFailed { agent_id, .. } => agent_id,
        }
    }

    /// Retrieve the timestamp string of this lifecycle event.
    pub fn timestamp(&self) -> &str {
        match self {
            AgentLifecycleEvent::AgentCreated { timestamp, .. }
            | AgentLifecycleEvent::AgentReady { timestamp, .. }
            | AgentLifecycleEvent::AgentStarted { timestamp, .. }
            | AgentLifecycleEvent::AgentPaused { timestamp, .. }
            | AgentLifecycleEvent::AgentResumed { timestamp, .. }
            | AgentLifecycleEvent::AgentStopped { timestamp, .. }
            | AgentLifecycleEvent::AgentFailed { timestamp, .. } => timestamp,
        }
    }

    /// Return the string identifier of the event variant.
    pub fn event_type(&self) -> &'static str {
        match self {
            AgentLifecycleEvent::AgentCreated { .. } => "AgentCreated",
            AgentLifecycleEvent::AgentReady { .. } => "AgentReady",
            AgentLifecycleEvent::AgentStarted { .. } => "AgentStarted",
            AgentLifecycleEvent::AgentPaused { .. } => "AgentPaused",
            AgentLifecycleEvent::AgentResumed { .. } => "AgentResumed",
            AgentLifecycleEvent::AgentStopped { .. } => "AgentStopped",
            AgentLifecycleEvent::AgentFailed { .. } => "AgentFailed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_lifecycle_transitions() {
        let mut state = AgentState::Created;
        assert_eq!(state, AgentState::Created);

        // Created -> Ready
        assert!(state.transition_to(AgentState::Ready).is_ok());
        assert_eq!(state, AgentState::Ready);

        // Ready -> Running
        assert!(state.transition_to(AgentState::Running).is_ok());
        assert_eq!(state, AgentState::Running);

        // Running -> Paused
        assert!(state.transition_to(AgentState::Paused).is_ok());
        assert_eq!(state, AgentState::Paused);

        // Paused -> Running
        assert!(state.transition_to(AgentState::Running).is_ok());
        assert_eq!(state, AgentState::Running);

        // Running -> Stopped
        assert!(state.transition_to(AgentState::Stopped).is_ok());
        assert_eq!(state, AgentState::Stopped);

        // Stopped -> Ready (restart)
        assert!(state.transition_to(AgentState::Ready).is_ok());
        assert_eq!(state, AgentState::Ready);

        // Ready -> Failed
        assert!(state.transition_to(AgentState::Failed).is_ok());
        assert_eq!(state, AgentState::Failed);

        // Failed -> Ready (restart)
        assert!(state.transition_to(AgentState::Ready).is_ok());
        assert_eq!(state, AgentState::Ready);
    }

    #[test]
    fn test_invalid_lifecycle_transitions() {
        // Stopped cannot transition directly to Running without restart
        let mut stopped = AgentState::Stopped;
        let err = stopped.transition_to(AgentState::Running).unwrap_err();
        assert!(err.to_string().contains("invalid state transition"));

        // Stopped cannot transition to Paused
        assert!(stopped.transition_to(AgentState::Paused).is_err());

        // Failed cannot transition directly to Running
        let mut failed = AgentState::Failed;
        assert!(failed.transition_to(AgentState::Running).is_err());

        // Failed cannot transition to Paused
        assert!(failed.transition_to(AgentState::Paused).is_err());

        // Created cannot transition directly to Paused
        let mut created = AgentState::Created;
        assert!(created.transition_to(AgentState::Paused).is_err());
    }

    #[test]
    fn test_identity_transitions() {
        let mut running = AgentState::Running;
        assert!(running.transition_to(AgentState::Running).is_ok());

        let mut paused = AgentState::Paused;
        assert!(paused.transition_to(AgentState::Paused).is_ok());

        let mut stopped = AgentState::Stopped;
        assert!(stopped.transition_to(AgentState::Stopped).is_ok());
    }

    #[test]
    fn test_lifecycle_event_accessors() {
        let id = AgentId::from("agent_123");
        let event = AgentLifecycleEvent::AgentCreated {
            agent_id: id.clone(),
            name: "test-agent".to_string(),
            role: "coder".to_string(),
            timestamp: "2026-10-08T00:00:00Z".to_string(),
        };

        assert_eq!(event.agent_id(), &id);
        assert_eq!(event.event_type(), "AgentCreated");
        assert_eq!(event.timestamp(), "2026-10-08T00:00:00Z");
    }
}
