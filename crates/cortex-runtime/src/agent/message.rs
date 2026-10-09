//! Inter-agent messaging schema, routing keys, and typed payloads.
//!
//! Provides validation and serialization primitives for messages exchanged
//! between autonomous agents across coordination boundaries.

use cortex_core::{CortexError, Result};
use serde::{Deserialize, Serialize};

/// Strongly typed routing key used to direct messages to appropriate agent handlers.
///
/// A valid routing key contains 1 through 128 bytes, each consisting exclusively of
/// ASCII letters (`a-z`, `A-Z`), digits (`0-9`), dots (`.`), underscores (`_`), or
/// hyphens (`-`). Whitespace, non-ASCII characters, empty strings, oversized values,
/// and wildcards are strictly rejected.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RoutingKey(String);

impl RoutingKey {
    /// Create and validate a new [`RoutingKey`].
    ///
    /// # Errors
    /// Returns [`CortexError::Validation`] if the string is empty, longer than 128
    /// bytes, or contains characters other than ASCII letters, digits, dots,
    /// underscores, or hyphens.
    pub fn new(value: impl Into<String>) -> Result<Self> {
        let s = value.into();
        validate_routing_key(&s)?;
        Ok(Self(s))
    }

    /// Return the routing key as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for RoutingKey {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for RoutingKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl TryFrom<String> for RoutingKey {
    type Error = CortexError;

    fn try_from(value: String) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<&str> for RoutingKey {
    type Error = CortexError;

    fn try_from(value: &str) -> Result<Self> {
        Self::new(value)
    }
}

impl From<RoutingKey> for String {
    fn from(key: RoutingKey) -> Self {
        key.0
    }
}

fn validate_routing_key(s: &str) -> Result<()> {
    if s.is_empty() {
        return Err(CortexError::Validation(
            "routing key cannot be empty".to_string(),
        ));
    }
    if s.len() > 128 {
        return Err(CortexError::Validation(format!(
            "routing key length exceeds maximum of 128 bytes (got {})",
            s.len()
        )));
    }
    for (i, b) in s.bytes().enumerate() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'_' | b'-' => {}
            _ => {
                return Err(CortexError::Validation(format!(
                    "invalid character in routing key at byte offset {}: ASCII letter, digit, dot, underscore, or hyphen required",
                    i
                )));
            }
        }
    }
    Ok(())
}

/// Typed payload carried by an inter-agent message.
///
/// Defines structured message variants representing delegation, completion,
/// failure, or informational notifications between collaborating agents.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum AgentMessagePayload {
    /// Request delegating a task to an agent worker.
    TaskRequest {
        /// Unique task identifier correlating request and reply.
        task_id: String,
        /// Instructions describing the task to execute.
        instructions: String,
    },
    /// Result payload returned upon successful completion of a task.
    TaskResult {
        /// Correlated task identifier matching the original request.
        task_id: String,
        /// Output produced by the completed task.
        output: String,
    },
    /// Error notification returned when a task fails to complete.
    TaskFailed {
        /// Correlated task identifier matching the original request.
        task_id: String,
        /// Error message or diagnostic explanation of the failure.
        error: String,
    },
    /// Informational notification sent to an explicit recipient.
    Notification {
        /// Textual content of the notification.
        content: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_routing_keys() {
        assert!(RoutingKey::new("agent.worker").is_ok());
        assert!(RoutingKey::new("task_update-1").is_ok());
        assert!(RoutingKey::new("coordinator.coder.review-01").is_ok());
        assert!(RoutingKey::new("a").is_ok());

        // Exactly 128 bytes
        let max_key = "a".repeat(128);
        let key = RoutingKey::new(&max_key).expect("128-byte key must be valid");
        assert_eq!(key.as_str(), max_key);
        assert_eq!(key.as_str().len(), 128);
    }

    #[test]
    fn test_invalid_routing_keys() {
        // Empty
        assert!(matches!(
            RoutingKey::new(""),
            Err(CortexError::Validation(_))
        ));

        // 129 bytes (oversized)
        let oversized = "a".repeat(129);
        assert!(matches!(
            RoutingKey::new(oversized),
            Err(CortexError::Validation(_))
        ));

        // Whitespace
        assert!(matches!(
            RoutingKey::new("worker one"),
            Err(CortexError::Validation(_))
        ));
        assert!(matches!(
            RoutingKey::new(" agent"),
            Err(CortexError::Validation(_))
        ));
        assert!(matches!(
            RoutingKey::new("agent "),
            Err(CortexError::Validation(_))
        ));
        assert!(matches!(
            RoutingKey::new("agent\n"),
            Err(CortexError::Validation(_))
        ));
        assert!(matches!(
            RoutingKey::new("\t"),
            Err(CortexError::Validation(_))
        ));

        // Non-ASCII
        assert!(matches!(
            RoutingKey::new("agent.🦀"),
            Err(CortexError::Validation(_))
        ));
        assert!(matches!(
            RoutingKey::new("tëst"),
            Err(CortexError::Validation(_))
        ));

        // Wildcards and invalid punctuation
        assert!(matches!(
            RoutingKey::new("agent.*"),
            Err(CortexError::Validation(_))
        ));
        assert!(matches!(
            RoutingKey::new("agent.?"),
            Err(CortexError::Validation(_))
        ));
        assert!(matches!(
            RoutingKey::new("worker.#"),
            Err(CortexError::Validation(_))
        ));
        assert!(matches!(
            RoutingKey::new("agent:worker"),
            Err(CortexError::Validation(_))
        ));
        assert!(matches!(
            RoutingKey::new("agent/worker"),
            Err(CortexError::Validation(_))
        ));
    }

    #[test]
    fn test_routing_key_serde_and_json_validation() {
        let key = RoutingKey::new("agent.coordinator-01").unwrap();

        // Serializes as plain JSON string
        let serialized = serde_json::to_string(&key).expect("serialize routing key");
        assert_eq!(serialized, "\"agent.coordinator-01\"");

        // Deserializes valid string
        let deserialized: RoutingKey =
            serde_json::from_str(&serialized).expect("deserialize routing key");
        assert_eq!(deserialized, key);

        // Deserialization rejects invalid string
        assert!(serde_json::from_str::<RoutingKey>("\"\"").is_err());
        assert!(serde_json::from_str::<RoutingKey>("\"with space\"").is_err());
        assert!(serde_json::from_str::<RoutingKey>("\"wildcard.*\"").is_err());
        assert!(serde_json::from_str::<RoutingKey>(&format!("\"{}\"", "a".repeat(129))).is_err());
    }

    #[test]
    fn test_payload_variants_roundtrip_including_multiline() {
        let variants = vec![
            AgentMessagePayload::TaskRequest {
                task_id: "task-001".to_string(),
                instructions:
                    "Line 1: Refactor auth module\nLine 2: Add test coverage\n\nLine 4: Done."
                        .to_string(),
            },
            AgentMessagePayload::TaskResult {
                task_id: "task-001".to_string(),
                output: "Status: Success\nArtifacts:\n- auth.rs\n- tests/auth_test.rs".to_string(),
            },
            AgentMessagePayload::TaskFailed {
                task_id: "task-001".to_string(),
                error: "Compilation error:\nerror[E0425]: cannot find value `x` in this scope"
                    .to_string(),
            },
            AgentMessagePayload::Notification {
                content: "System alert:\nNode maintenance scheduled for 02:00 UTC.".to_string(),
            },
        ];

        for payload in variants {
            let json = serde_json::to_string(&payload).expect("serialization failed");
            let recovered: AgentMessagePayload =
                serde_json::from_str(&json).expect("deserialization failed");
            assert_eq!(recovered, payload);
        }
    }

    #[test]
    fn test_payload_concrete_json_fixture() {
        let payload = AgentMessagePayload::TaskRequest {
            task_id: "task-42".to_string(),
            instructions: "run verification tests".to_string(),
        };

        let json = serde_json::to_string(&payload).expect("serialize fixture");
        let expected_json = r#"{"type":"task_request","payload":{"task_id":"task-42","instructions":"run verification tests"}}"#;
        assert_eq!(json, expected_json);

        let parsed: AgentMessagePayload =
            serde_json::from_str(expected_json).expect("parse fixture");
        assert_eq!(parsed, payload);
    }

    #[test]
    fn test_payload_deserialization_rejections() {
        // Unknown payload variant
        let unknown_variant = r#"{"type":"unknown_type","payload":{"task_id":"task-1"}}"#;
        assert!(serde_json::from_str::<AgentMessagePayload>(unknown_variant).is_err());

        // TaskRequest missing task_id
        let missing_task_id =
            r#"{"type":"task_request","payload":{"instructions":"do something"}}"#;
        assert!(serde_json::from_str::<AgentMessagePayload>(missing_task_id).is_err());

        // TaskRequest missing instructions
        let missing_instructions = r#"{"type":"task_request","payload":{"task_id":"task-1"}}"#;
        assert!(serde_json::from_str::<AgentMessagePayload>(missing_instructions).is_err());

        // Missing payload field entirely
        let missing_payload = r#"{"type":"task_request"}"#;
        assert!(serde_json::from_str::<AgentMessagePayload>(missing_payload).is_err());
    }
}
