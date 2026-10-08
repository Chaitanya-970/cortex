//! Session persistence and history management for Cortex TUI.

use cortex_core::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// A persisted chat session record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSession {
    /// Unique session identifier.
    pub id: String,
    /// Brief title or first prompt snippet.
    pub title: String,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
    /// ISO 8601 last update timestamp.
    pub updated_at: String,
    /// Model name used during this session.
    pub model: String,
    /// Serialized conversation messages.
    pub messages: Vec<SavedChatMessage>,
    /// Total tokens consumed.
    pub tokens: u64,
    /// Total estimated cost in USD.
    pub cost: f64,
}

/// A single message persisted in session history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedChatMessage {
    /// Role string ("user", "assistant", "system", "tool", "thinking", "error").
    pub role: String,
    /// Text content of the message.
    pub content: String,
    /// Recorded timestamp.
    pub timestamp: String,
}

/// Resolve the directory for storing session files (`~/.cortex/sessions`).
pub fn sessions_dir() -> PathBuf {
    cortex_core::settings::cortex_home_dir().join("sessions")
}

/// Save a session to disk under `~/.cortex/sessions/<id>.json`.
pub fn save_session(session: &SavedSession) -> Result<()> {
    let dir = sessions_dir();
    fs::create_dir_all(&dir).map_err(|e| {
        cortex_core::CortexError::Internal(format!("failed to create sessions dir: {}", e))
    })?;

    let file_path = dir.join(format!("{}.json", session.id));
    let json = serde_json::to_string_pretty(session).map_err(|e| {
        cortex_core::CortexError::Internal(format!("failed to serialize session: {}", e))
    })?;

    fs::write(file_path, json).map_err(|e| {
        cortex_core::CortexError::Internal(format!("failed to write session file: {}", e))
    })?;

    Ok(())
}

/// List all saved sessions sorted by last updated timestamp descending.
pub fn list_sessions() -> Result<Vec<SavedSession>> {
    let dir = sessions_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut sessions = Vec::new();
    let entries = fs::read_dir(dir).map_err(|e| {
        cortex_core::CortexError::Internal(format!("failed to read sessions dir: {}", e))
    })?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "json") {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(session) = serde_json::from_str::<SavedSession>(&content) {
                    sessions.push(session);
                }
            }
        }
    }

    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(sessions)
}

/// Load a specific session by its identifier.
pub fn load_session(id: &str) -> Result<Option<SavedSession>> {
    let file_path = sessions_dir().join(format!("{}.json", id));
    if !file_path.exists() {
        // Try prefix match
        let all = list_sessions()?;
        if let Some(matching) = all.into_iter().find(|s| s.id.starts_with(id)) {
            return Ok(Some(matching));
        }
        return Ok(None);
    }

    let content = fs::read_to_string(file_path).map_err(|e| {
        cortex_core::CortexError::Internal(format!("failed to read session: {}", e))
    })?;

    let session = serde_json::from_str(&content).map_err(|e| {
        cortex_core::CortexError::Internal(format!("failed to parse session: {}", e))
    })?;

    Ok(Some(session))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_save_and_load_session_roundtrip() {
        let temp_dir = std::env::temp_dir().join("cortex_test_sessions");
        let _ = fs::remove_dir_all(&temp_dir);
        let _ = fs::create_dir_all(&temp_dir);

        let session = SavedSession {
            id: "session_test_123".to_string(),
            title: "Test prompt".to_string(),
            created_at: "2026-10-09T00:00:00Z".to_string(),
            updated_at: "2026-10-09T00:01:00Z".to_string(),
            model: "gpt-4o-mini".to_string(),
            messages: vec![
                SavedChatMessage {
                    role: "user".to_string(),
                    content: "Fix the bug".to_string(),
                    timestamp: "00:00:00".to_string(),
                },
                SavedChatMessage {
                    role: "assistant".to_string(),
                    content: "Fixed.".to_string(),
                    timestamp: "00:01:00".to_string(),
                },
            ],
            tokens: 150,
            cost: 0.0003,
        };

        let file_path = temp_dir.join(format!("{}.json", session.id));
        let json = serde_json::to_string_pretty(&session).unwrap();
        fs::write(&file_path, json).unwrap();

        let loaded: SavedSession =
            serde_json::from_str(&fs::read_to_string(&file_path).unwrap()).unwrap();
        assert_eq!(loaded.id, session.id);
        assert_eq!(loaded.messages.len(), 2);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
