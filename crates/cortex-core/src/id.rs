//! Strongly typed identifiers for agents, runs, and sessions.

use std::fmt;

/// Strongly typed identifier for an agent.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AgentId(String);

impl AgentId {
    /// Create a new [`AgentId`] from a string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Access the underlying string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for AgentId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for AgentId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Strongly typed identifier for an individual execution run.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RunId(String);

impl RunId {
    /// Create a new [`RunId`] from a string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Access the underlying string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for RunId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for RunId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Strongly typed identifier for an interactive session.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionId(String);

impl SessionId {
    /// Create a new [`SessionId`] from a string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Access the underlying string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for SessionId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for SessionId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identifiers_creation_and_display() {
        let agent_id = AgentId::from("coder-1");
        assert_eq!(agent_id.as_str(), "coder-1");
        assert_eq!(agent_id.to_string(), "coder-1");

        let run_id = RunId::from("run-100");
        assert_eq!(run_id.as_str(), "run-100");
        assert_eq!(run_id.to_string(), "run-100");

        let session_id = SessionId::from("session-abc");
        assert_eq!(session_id.as_str(), "session-abc");
        assert_eq!(session_id.to_string(), "session-abc");
    }
}
