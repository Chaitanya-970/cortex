//! Strongly typed identifiers for agents, runs, and sessions.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Strongly typed identifier for an agent.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AgentId(String);

static AGENT_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static RUN_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static SESSION_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static JOB_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl AgentId {
    /// Create a new [`AgentId`] from a string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate a unique [`AgentId`] based on current timestamp and atomic counter.
    pub fn generate() -> Self {
        let ts = chrono::Utc::now().timestamp_micros();
        let cnt = AGENT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self(format!("agent_{}_{:04x}", ts, cnt & 0xffff))
    }

    /// Access the underlying string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::generate()
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct RunId(String);

impl RunId {
    /// Create a new [`RunId`] from a string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate a unique [`RunId`] based on current timestamp and atomic counter.
    pub fn generate() -> Self {
        let ts = chrono::Utc::now().timestamp_micros();
        let cnt = RUN_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self(format!("run_{}_{:04x}", ts, cnt & 0xffff))
    }

    /// Access the underlying string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for RunId {
    fn default() -> Self {
        Self::generate()
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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    /// Create a new [`SessionId`] from a string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate a unique [`SessionId`] based on current timestamp and atomic counter.
    pub fn generate() -> Self {
        let ts = chrono::Utc::now().timestamp_micros();
        let cnt = SESSION_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self(format!("session_{}_{:04x}", ts, cnt & 0xffff))
    }

    /// Access the underlying string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::generate()
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

/// Strongly typed identifier for a scheduled cron or one-shot job.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct JobId(String);

impl JobId {
    /// Create a new [`JobId`] from a string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Generate a unique [`JobId`] based on current timestamp and atomic counter.
    pub fn generate() -> Self {
        let ts = chrono::Utc::now().timestamp_micros();
        let cnt = JOB_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self(format!("job_{}_{:04x}", ts, cnt & 0xffff))
    }

    /// Access the underlying string representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::generate()
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for JobId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for JobId {
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

        let job_id = JobId::from("job-cron-1");
        assert_eq!(job_id.as_str(), "job-cron-1");
        assert_eq!(job_id.to_string(), "job-cron-1");

        let generated_job = JobId::generate();
        assert!(generated_job.as_str().starts_with("job_"));
    }
}
