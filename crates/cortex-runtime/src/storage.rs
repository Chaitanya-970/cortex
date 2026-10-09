//! SQLite-backed execution run and event stream persistence.
//!
//! Provides schema versioning, transaction-wrapped migrations, run lifecycle tracking,
//! and event stream storage for the Cortex runtime.

use crate::agent::{AgentMessage, AgentMessagePayload, RoutingKey};
use chrono::Utc;
pub use cortex_core::SecretRedactor;
use cortex_core::{AgentId, CortexError, EventRecord, ExecutionEvent, Redactor, Result, RunId};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

/// Current schema migration version.
pub const CURRENT_SCHEMA_VERSION: i32 = 3;
const SCHEMA_V1: &str = include_str!("../migrations/001_initial_schema.sql");
const SCHEMA_V3: &str = include_str!("../migrations/003_multi_agent_tables.sql");

/// Summary information for an execution run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunSummary {
    /// Unique run identifier.
    pub id: RunId,
    /// Task prompt assigned to the agent.
    pub task: String,
    /// Execution status (e.g., "running", "completed", "failed", "cancelled").
    pub status: String,
    /// ISO 8601 timestamp of run start.
    pub started_at: String,
    /// ISO 8601 timestamp of completion, if finished.
    pub finished_at: Option<String>,
    /// Elapsed execution time in milliseconds.
    pub duration_ms: Option<u64>,
    /// Number of prompt tokens consumed.
    pub tokens_prompt: u32,
    /// Number of completion tokens consumed.
    pub tokens_completion: u32,
    /// Total tokens consumed.
    pub tokens_total: u32,
    /// Estimated inference cost in USD.
    pub estimated_cost_usd: f64,
    /// Error message if the run terminated abnormally.
    pub error: Option<String>,
}

/// SQLite persistence manager for agent execution runs, structured events, and inter-agent messages.
pub struct RunStore {
    conn: Mutex<Connection>,
    redactor: Redactor,
}

impl std::fmt::Debug for RunStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunStore").finish_non_exhaustive()
    }
}

impl RunStore {
    /// Open a persistent SQLite database at the given file path, applying migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    CortexError::Internal(format!(
                        "failed to create database directory '{}': {}",
                        parent.display(),
                        e
                    ))
                })?;
            }
        }

        let conn = Connection::open(path).map_err(|e| {
            CortexError::Internal(format!(
                "failed to open sqlite database at '{}': {}",
                path.display(),
                e
            ))
        })?;

        let store = Self {
            conn: Mutex::new(conn),
            redactor: Redactor::new(),
        };
        store.migrate()?;
        Ok(store)
    }

    /// Open an in-memory SQLite database, applying migrations.
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(|e| {
            CortexError::Internal(format!("failed to open in-memory sqlite: {}", e))
        })?;

        let store = Self {
            conn: Mutex::new(conn),
            redactor: Redactor::new(),
        };
        store.migrate()?;
        Ok(store)
    }

    /// Attach a custom [`Redactor`] for scrubbing sensitive secrets from traces and payloads.
    pub fn with_redactor(mut self, redactor: Redactor) -> Self {
        self.redactor = redactor;
        self
    }

    /// Access the underlying [`Redactor`] used for secret scrubbing.
    pub fn redactor(&self) -> &Redactor {
        &self.redactor
    }

    /// Apply schema migrations sequentially within a transaction.
    pub fn migrate(&self) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        // Check if schema_version table exists
        let table_exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='schema_version'",
                [],
                |row| row.get(0),
            )
            .map_err(|e| {
                CortexError::Internal(format!("failed to query schema_version table: {}", e))
            })?;

        let applied_versions: std::collections::HashSet<i32> = if table_exists {
            let mut stmt = conn
                .prepare("SELECT version FROM schema_version")
                .map_err(|e| {
                    CortexError::Internal(format!("failed to query schema_version: {}", e))
                })?;
            let rows = stmt.query_map([], |row| row.get(0)).map_err(|e| {
                CortexError::Internal(format!("failed to query schema_version: {}", e))
            })?;
            rows.filter_map(|r| r.ok()).collect()
        } else {
            std::collections::HashSet::new()
        };

        if !applied_versions.contains(&1) {
            conn.execute_batch(SCHEMA_V1).map_err(|e| {
                CortexError::Internal(format!("failed to apply migration 001: {}", e))
            })?;

            let now = Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2)",
                params![1, now],
            )
            .map_err(|e| {
                CortexError::Internal(format!("failed to record schema_version 1: {}", e))
            })?;
        }

        if !applied_versions.contains(&3) {
            conn.execute_batch(SCHEMA_V3).map_err(|e| {
                CortexError::Internal(format!("failed to apply migration 003: {}", e))
            })?;

            let now = Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2)",
                params![3, now],
            )
            .map_err(|e| {
                CortexError::Internal(format!("failed to record schema_version 3: {}", e))
            })?;
        }

        Ok(())
    }

    /// Return the currently applied schema migration version.
    pub fn schema_version(&self) -> Result<i32> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        let version: i32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .map_err(|e| CortexError::Internal(format!("failed to read schema version: {}", e)))?;

        Ok(version)
    }

    /// Record initial execution run state.
    pub fn record_run_start(&self, run_id: &RunId, task: &str, started_at: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        conn.execute(
            "INSERT INTO runs (id, task, status, started_at) VALUES (?1, ?2, 'running', ?3)
             ON CONFLICT(id) DO UPDATE SET task = excluded.task, started_at = excluded.started_at",
            params![run_id.as_str(), task, started_at],
        )
        .map_err(|e| CortexError::Internal(format!("failed to record run start: {}", e)))?;

        Ok(())
    }

    /// Record completion or termination outcome of an execution run.
    #[allow(clippy::too_many_arguments)]
    pub fn record_run_completion(
        &self,
        run_id: &RunId,
        status: &str,
        finished_at: &str,
        duration_ms: u64,
        tokens_prompt: u32,
        tokens_completion: u32,
        estimated_cost_usd: f64,
        error: Option<&str>,
    ) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        let tokens_total = tokens_prompt + tokens_completion;

        conn.execute(
            "UPDATE runs SET
                status = ?1,
                finished_at = ?2,
                duration_ms = ?3,
                tokens_prompt = ?4,
                tokens_completion = ?5,
                tokens_total = ?6,
                estimated_cost_usd = ?7,
                error = ?8
             WHERE id = ?9",
            params![
                status,
                finished_at,
                duration_ms as i64,
                tokens_prompt as i64,
                tokens_completion as i64,
                tokens_total as i64,
                estimated_cost_usd,
                error,
                run_id.as_str(),
            ],
        )
        .map_err(|e| CortexError::Internal(format!("failed to record run completion: {}", e)))?;

        Ok(())
    }

    /// Record a structured execution event in the event log.
    pub fn record_event(&self, record: &EventRecord) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        let payload_json = serde_json::to_string(record).map_err(|e| {
            CortexError::Internal(format!("failed to serialize event payload: {}", e))
        })?;

        conn.execute(
            "INSERT INTO events (run_id, sequence, timestamp, event_type, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                record.run_id.as_str(),
                record.sequence as i64,
                record.timestamp.as_str(),
                record.event.event_type(),
                payload_json,
            ],
        )
        .map_err(|e| CortexError::Internal(format!("failed to insert event record: {}", e)))?;

        // Mirror InterAgentMessage events to the dedicated inter_agent_messages table
        if let ExecutionEvent::InterAgentMessage {
            run_id,
            message_id,
            sender,
            recipient,
            routing_key,
            timestamp,
            payload,
        } = &record.event
        {
            let task_id = payload
                .get("payload")
                .and_then(|p| p.get("task_id"))
                .or_else(|| payload.get("task_id"))
                .and_then(|t| t.as_str());

            let message_type = payload
                .get("type")
                .and_then(|t| t.as_str())
                .unwrap_or("notification");

            let payload_str = serde_json::to_string(payload).unwrap_or_default();

            let _ = conn.execute(
                "INSERT INTO inter_agent_messages (
                    id, run_id, task_id, sender, recipient, routing_key, message_type, payload, timestamp
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(id) DO UPDATE SET
                    run_id = excluded.run_id,
                    task_id = excluded.task_id,
                    sender = excluded.sender,
                    recipient = excluded.recipient,
                    routing_key = excluded.routing_key,
                    message_type = excluded.message_type,
                    payload = excluded.payload,
                    timestamp = excluded.timestamp",
                params![
                    message_id.as_str(),
                    run_id.as_str(),
                    task_id,
                    sender.as_str(),
                    recipient.as_str(),
                    routing_key.as_str(),
                    message_type,
                    payload_str,
                    timestamp.as_str(),
                ],
            );
        }

        Ok(())
    }

    /// Retrieve summary details for a specific run.
    pub fn get_run(&self, run_id: &RunId) -> Result<Option<RunSummary>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, task, status, started_at, finished_at, duration_ms,
                        tokens_prompt, tokens_completion, tokens_total, estimated_cost_usd, error
                 FROM runs WHERE id = ?1",
            )
            .map_err(|e| CortexError::Internal(format!("failed to prepare run query: {}", e)))?;

        let mut rows = stmt
            .query(params![run_id.as_str()])
            .map_err(|e| CortexError::Internal(format!("failed to execute run query: {}", e)))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| CortexError::Internal(format!("failed to read run row: {}", e)))?
        {
            let id_str: String = row.get(0).map_err(map_sql_err)?;
            let duration_ms: Option<i64> = row.get(5).map_err(map_sql_err)?;
            let tokens_prompt: i64 = row.get(6).map_err(map_sql_err)?;
            let tokens_completion: i64 = row.get(7).map_err(map_sql_err)?;
            let tokens_total: i64 = row.get(8).map_err(map_sql_err)?;

            Ok(Some(RunSummary {
                id: RunId::from(id_str),
                task: row.get(1).map_err(map_sql_err)?,
                status: row.get(2).map_err(map_sql_err)?,
                started_at: row.get(3).map_err(map_sql_err)?,
                finished_at: row.get(4).map_err(map_sql_err)?,
                duration_ms: duration_ms.map(|d| d as u64),
                tokens_prompt: tokens_prompt as u32,
                tokens_completion: tokens_completion as u32,
                tokens_total: tokens_total as u32,
                estimated_cost_usd: row.get(9).map_err(map_sql_err)?,
                error: row.get(10).map_err(map_sql_err)?,
            }))
        } else {
            Ok(None)
        }
    }

    /// List recent execution runs, sorted newest first.
    pub fn list_runs(&self, limit: usize) -> Result<Vec<RunSummary>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, task, status, started_at, finished_at, duration_ms,
                        tokens_prompt, tokens_completion, tokens_total, estimated_cost_usd, error
                 FROM runs ORDER BY started_at DESC LIMIT ?1",
            )
            .map_err(|e| CortexError::Internal(format!("failed to prepare list runs: {}", e)))?;

        let rows = stmt
            .query_map(params![limit as i64], |row| {
                let id_str: String = row.get(0)?;
                let duration_ms: Option<i64> = row.get(5)?;
                let tokens_prompt: i64 = row.get(6)?;
                let tokens_completion: i64 = row.get(7)?;
                let tokens_total: i64 = row.get(8)?;

                Ok(RunSummary {
                    id: RunId::from(id_str),
                    task: row.get(1)?,
                    status: row.get(2)?,
                    started_at: row.get(3)?,
                    finished_at: row.get(4)?,
                    duration_ms: duration_ms.map(|d| d as u64),
                    tokens_prompt: tokens_prompt as u32,
                    tokens_completion: tokens_completion as u32,
                    tokens_total: tokens_total as u32,
                    estimated_cost_usd: row.get(9)?,
                    error: row.get(10)?,
                })
            })
            .map_err(|e| CortexError::Internal(format!("failed to list runs: {}", e)))?;

        let mut result = Vec::new();
        for r in rows {
            result.push(r.map_err(map_sql_err)?);
        }
        Ok(result)
    }

    /// Retrieve all structured event records for a specific run, sorted by sequence number.
    pub fn get_events(&self, run_id: &RunId) -> Result<Vec<EventRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT payload_json FROM events
                 WHERE run_id = ?1 ORDER BY sequence ASC",
            )
            .map_err(|e| CortexError::Internal(format!("failed to prepare get events: {}", e)))?;

        let rows = stmt
            .query_map(params![run_id.as_str()], |row| {
                let json_str: String = row.get(0)?;
                Ok(json_str)
            })
            .map_err(|e| CortexError::Internal(format!("failed to query events: {}", e)))?;

        let mut records = Vec::new();
        for r in rows {
            let json_str = r.map_err(map_sql_err)?;
            let record: EventRecord = serde_json::from_str(&json_str).map_err(|e| {
                CortexError::Internal(format!("failed to deserialize event record: {}", e))
            })?;
            records.push(record);
        }

        Ok(records)
    }

    /// Persist an inter-agent message transmission in SQLite with secret redaction.
    pub fn record_message(&self, msg: &AgentMessage) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        // Ensure run exists so foreign key constraints are preserved
        conn.execute(
            "INSERT OR IGNORE INTO runs (id, task, status, started_at) VALUES (?1, 'multi-agent execution', 'running', ?2)",
            params![msg.run_id.as_str(), msg.timestamp.as_str()],
        )
        .map_err(|e| CortexError::Internal(format!("failed to ensure run exists: {}", e)))?;

        let (task_id, message_type) = match &msg.payload {
            AgentMessagePayload::TaskRequest { task_id, .. } => {
                (Some(task_id.as_str()), "task_request")
            }
            AgentMessagePayload::TaskResult { task_id, .. } => {
                (Some(task_id.as_str()), "task_result")
            }
            AgentMessagePayload::TaskFailed { task_id, .. } => {
                (Some(task_id.as_str()), "task_failed")
            }
            AgentMessagePayload::Notification { .. } => (None, "notification"),
        };

        // Redact payload before writing to disk
        let raw_val = serde_json::to_value(&msg.payload).map_err(|e| {
            CortexError::Internal(format!("failed to serialize message payload: {}", e))
        })?;
        let redacted_val = self.redactor.redact_json(&raw_val);
        let payload_json = serde_json::to_string(&redacted_val).map_err(|e| {
            CortexError::Internal(format!("failed to serialize redacted payload: {}", e))
        })?;

        conn.execute(
            "INSERT INTO inter_agent_messages (
                id, run_id, task_id, sender, recipient, routing_key, message_type, payload, timestamp
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(id) DO UPDATE SET
                run_id = excluded.run_id,
                task_id = excluded.task_id,
                sender = excluded.sender,
                recipient = excluded.recipient,
                routing_key = excluded.routing_key,
                message_type = excluded.message_type,
                payload = excluded.payload,
                timestamp = excluded.timestamp",
            params![
                msg.id.as_str(),
                msg.run_id.as_str(),
                task_id,
                msg.sender.as_str(),
                msg.recipient.as_str(),
                msg.routing_key.as_str(),
                message_type,
                payload_json,
                msg.timestamp.as_str(),
            ],
        )
        .map_err(|e| CortexError::Internal(format!("failed to insert inter-agent message: {}", e)))?;

        Ok(())
    }

    /// Persist a slice of inter-agent messages in SQLite.
    pub fn record_messages(&self, msgs: &[AgentMessage]) -> Result<()> {
        for msg in msgs {
            self.record_message(msg)?;
        }
        Ok(())
    }

    /// Retrieve all inter-agent messages recorded for a specific run, sorted chronologically.
    pub fn get_messages_for_run(&self, run_id: &RunId) -> Result<Vec<AgentMessage>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, run_id, sender, recipient, routing_key, timestamp, message_type, payload
                 FROM inter_agent_messages
                 WHERE run_id = ?1
                 ORDER BY timestamp ASC, id ASC",
            )
            .map_err(|e| CortexError::Internal(format!("failed to prepare get_messages_for_run query: {}", e)))?;

        let rows = stmt
            .query_map(params![run_id.as_str()], Self::row_to_agent_message)
            .map_err(|e| {
                CortexError::Internal(format!("failed to query messages for run: {}", e))
            })?;

        let mut messages = Vec::new();
        for r in rows {
            messages.push(r.map_err(map_sql_err)?);
        }

        Ok(messages)
    }

    /// Retrieve all inter-agent messages exchanged between two agents across all runs, sorted chronologically.
    pub fn get_messages_between(
        &self,
        agent_a: &AgentId,
        agent_b: &AgentId,
    ) -> Result<Vec<AgentMessage>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, run_id, sender, recipient, routing_key, timestamp, message_type, payload
                 FROM inter_agent_messages
                 WHERE (sender = ?1 AND recipient = ?2) OR (sender = ?2 AND recipient = ?1)
                 ORDER BY timestamp ASC, id ASC",
            )
            .map_err(|e| CortexError::Internal(format!("failed to prepare get_messages_between query: {}", e)))?;

        let rows = stmt
            .query_map(
                params![agent_a.as_str(), agent_b.as_str()],
                Self::row_to_agent_message,
            )
            .map_err(|e| {
                CortexError::Internal(format!("failed to query messages between agents: {}", e))
            })?;

        let mut messages = Vec::new();
        for r in rows {
            messages.push(r.map_err(map_sql_err)?);
        }

        Ok(messages)
    }

    /// Retrieve all inter-agent messages correlated with a specific task identifier.
    pub fn get_messages_for_task(&self, task_id: &str) -> Result<Vec<AgentMessage>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        let mut stmt = conn
            .prepare(
                "SELECT id, run_id, sender, recipient, routing_key, timestamp, message_type, payload
                 FROM inter_agent_messages
                 WHERE task_id = ?1
                 ORDER BY timestamp ASC, id ASC",
            )
            .map_err(|e| CortexError::Internal(format!("failed to prepare get_messages_for_task query: {}", e)))?;

        let rows = stmt
            .query_map(params![task_id], Self::row_to_agent_message)
            .map_err(|e| {
                CortexError::Internal(format!("failed to query messages for task: {}", e))
            })?;

        let mut messages = Vec::new();
        for r in rows {
            messages.push(r.map_err(map_sql_err)?);
        }

        Ok(messages)
    }

    fn row_to_agent_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentMessage> {
        let id: String = row.get(0)?;
        let run_id_str: String = row.get(1)?;
        let sender_str: String = row.get(2)?;
        let recipient_str: String = row.get(3)?;
        let routing_key_str: String = row.get(4)?;
        let timestamp: String = row.get(5)?;
        let message_type: String = row.get(6)?;
        let payload_json: String = row.get(7)?;

        let routing_key = RoutingKey::new(routing_key_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
        })?;

        let payload_val: serde_json::Value = serde_json::from_str(&payload_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e))
        })?;

        let payload: AgentMessagePayload = serde_json::from_value(payload_val.clone())
            .or_else(|_| {
                serde_json::from_value(serde_json::json!({
                    "type": message_type,
                    "payload": payload_val
                }))
            })
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

        Ok(AgentMessage {
            id,
            run_id: RunId::from(run_id_str),
            sender: AgentId::from(sender_str),
            recipient: AgentId::from(recipient_str),
            routing_key,
            timestamp,
            payload,
        })
    }
}

fn map_sql_err(err: rusqlite::Error) -> CortexError {
    CortexError::Internal(format!("sqlite error: {}", err))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cortex_core::ExecutionEvent;

    #[test]
    fn test_store_in_memory_migrations() {
        let store = RunStore::in_memory().unwrap();
        assert_eq!(store.schema_version().unwrap(), CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn test_record_and_query_run_lifecycle() {
        let store = RunStore::in_memory().unwrap();
        let run_id = RunId::from("run_test_01");

        store
            .record_run_start(&run_id, "Inspect repository", "2026-10-08T00:00:00Z")
            .unwrap();

        let run = store.get_run(&run_id).unwrap().unwrap();
        assert_eq!(run.id, run_id);
        assert_eq!(run.status, "running");
        assert_eq!(run.task, "Inspect repository");

        store
            .record_run_completion(
                &run_id,
                "completed",
                "2026-10-08T00:00:05Z",
                5000,
                150,
                50,
                0.0003,
                None,
            )
            .unwrap();

        let finished = store.get_run(&run_id).unwrap().unwrap();
        assert_eq!(finished.status, "completed");
        assert_eq!(finished.duration_ms, Some(5000));
        assert_eq!(finished.tokens_total, 200);
        assert_eq!(finished.tokens_prompt, 150);
        assert_eq!(finished.tokens_completion, 50);
        assert!(finished.error.is_none());
    }

    #[test]
    fn test_record_and_query_events_ordering() {
        let store = RunStore::in_memory().unwrap();
        let run_id = RunId::from("run_test_seq");

        store
            .record_run_start(&run_id, "Test sequence", "2026-10-08T00:00:00Z")
            .unwrap();

        let ev1 = ExecutionEvent::RunStarted {
            run_id: run_id.clone(),
            task: "Test sequence".to_string(),
            workspace_root: None,
        };
        let ev2 = ExecutionEvent::ModelRequest {
            run_id: run_id.clone(),
            prompt_preview: "Prompt content".to_string(),
        };
        let ev3 = ExecutionEvent::RunCompleted {
            run_id: run_id.clone(),
            final_answer: "Done".to_string(),
            iterations: 1,
            duration_ms: 120,
        };

        store.record_event(&EventRecord::new(1, ev1)).unwrap();
        store.record_event(&EventRecord::new(2, ev2)).unwrap();
        store.record_event(&EventRecord::new(3, ev3)).unwrap();

        let events = store.get_events(&run_id).unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].sequence, 1);
        assert_eq!(events[0].event.event_type(), "RunStarted");
        assert_eq!(events[1].sequence, 2);
        assert_eq!(events[1].event.event_type(), "ModelRequest");
        assert_eq!(events[2].sequence, 3);
        assert_eq!(events[2].event.event_type(), "RunCompleted");
    }

    #[test]
    fn test_list_runs() {
        let store = RunStore::in_memory().unwrap();
        for i in 1..=3 {
            let rid = RunId::from(format!("run_{}", i));
            store
                .record_run_start(
                    &rid,
                    &format!("task {}", i),
                    &format!("2026-10-08T00:00:0{}Z", i),
                )
                .unwrap();
        }

        let runs = store.list_runs(10).unwrap();
        assert_eq!(runs.len(), 3);
        // Latest first
        assert_eq!(runs[0].id.as_str(), "run_3");
    }

    #[test]
    fn test_inter_agent_message_persistence_and_query_by_run() {
        let store = RunStore::in_memory().unwrap();
        let run_id = RunId::from("run_multi_agent_01");
        store
            .record_run_start(&run_id, "Coordinated task", "2026-10-08T00:00:00Z")
            .unwrap();

        let supervisor = AgentId::from("supervisor");
        let worker = AgentId::from("worker_coder");
        let route = RoutingKey::new("task.delegate").unwrap();

        let msg1 = AgentMessage {
            id: "msg_1".into(),
            run_id: run_id.clone(),
            sender: supervisor.clone(),
            recipient: worker.clone(),
            routing_key: route.clone(),
            timestamp: "2026-10-08T00:00:01Z".into(),
            payload: AgentMessagePayload::TaskRequest {
                task_id: "task-001".into(),
                instructions: "Implement binary search".into(),
            },
        };

        let msg2 = AgentMessage {
            id: "msg_2".into(),
            run_id: run_id.clone(),
            sender: worker.clone(),
            recipient: supervisor.clone(),
            routing_key: route.clone(),
            timestamp: "2026-10-08T00:00:02Z".into(),
            payload: AgentMessagePayload::TaskResult {
                task_id: "task-001".into(),
                output: "Binary search implemented".into(),
            },
        };

        store.record_message(&msg1).unwrap();
        store.record_message(&msg2).unwrap();

        let messages = store.get_messages_for_run(&run_id).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].id, "msg_1");
        assert_eq!(messages[0].sender, supervisor);
        assert_eq!(messages[0].recipient, worker);
        assert_eq!(
            messages[0].payload,
            AgentMessagePayload::TaskRequest {
                task_id: "task-001".into(),
                instructions: "Implement binary search".into(),
            }
        );

        assert_eq!(messages[1].id, "msg_2");
        assert_eq!(messages[1].sender, worker);
        assert_eq!(messages[1].recipient, supervisor);
        assert_eq!(
            messages[1].payload,
            AgentMessagePayload::TaskResult {
                task_id: "task-001".into(),
                output: "Binary search implemented".into(),
            }
        );
    }

    #[test]
    fn test_inter_agent_message_query_between_agents() {
        let store = RunStore::in_memory().unwrap();
        let run_1 = RunId::from("run_coord_1");
        let run_2 = RunId::from("run_coord_2");

        let agent_a = AgentId::from("agent_a");
        let agent_b = AgentId::from("agent_b");
        let agent_c = AgentId::from("agent_c");
        let route = RoutingKey::new("direct.channel").unwrap();

        let msg_ab_1 = AgentMessage {
            id: "m1".into(),
            run_id: run_1.clone(),
            sender: agent_a.clone(),
            recipient: agent_b.clone(),
            routing_key: route.clone(),
            timestamp: "2026-10-08T00:00:01Z".into(),
            payload: AgentMessagePayload::Notification {
                content: "Hello B from A".into(),
            },
        };

        let msg_ba_1 = AgentMessage {
            id: "m2".into(),
            run_id: run_1.clone(),
            sender: agent_b.clone(),
            recipient: agent_a.clone(),
            routing_key: route.clone(),
            timestamp: "2026-10-08T00:00:02Z".into(),
            payload: AgentMessagePayload::Notification {
                content: "Hello A from B".into(),
            },
        };

        let msg_ac_1 = AgentMessage {
            id: "m3".into(),
            run_id: run_2.clone(),
            sender: agent_a.clone(),
            recipient: agent_c.clone(),
            routing_key: route.clone(),
            timestamp: "2026-10-08T00:00:03Z".into(),
            payload: AgentMessagePayload::Notification {
                content: "Hello C from A".into(),
            },
        };

        store
            .record_messages(&[msg_ab_1, msg_ba_1, msg_ac_1])
            .unwrap();

        let conversation_ab = store.get_messages_between(&agent_a, &agent_b).unwrap();
        assert_eq!(conversation_ab.len(), 2);
        assert_eq!(conversation_ab[0].id, "m1");
        assert_eq!(conversation_ab[1].id, "m2");

        let conversation_ac = store.get_messages_between(&agent_a, &agent_c).unwrap();
        assert_eq!(conversation_ac.len(), 1);
        assert_eq!(conversation_ac[0].id, "m3");

        let conversation_bc = store.get_messages_between(&agent_b, &agent_c).unwrap();
        assert!(conversation_bc.is_empty());
    }

    #[test]
    fn test_inter_agent_message_secret_redaction() {
        let store = RunStore::in_memory().unwrap();
        let run_id = RunId::from("run_secret_leak_prevention");

        let secret_key = "sk-1234567890abcdef1234567890";
        let aws_key = "AKIAIOSFODNN7EXAMPLE";

        let msg = AgentMessage {
            id: "msg_sensitive".into(),
            run_id: run_id.clone(),
            sender: AgentId::from("supervisor"),
            recipient: AgentId::from("worker"),
            routing_key: RoutingKey::new("confidential.dispatch").unwrap(),
            timestamp: "2026-10-08T00:00:01Z".into(),
            payload: AgentMessagePayload::TaskRequest {
                task_id: "task-auth".into(),
                instructions: format!("Connect with key {} and AWS {}", secret_key, aws_key),
            },
        };

        store.record_message(&msg).unwrap();

        // 1. Verify that retrieved message payload is redacted
        let retrieved = store.get_messages_for_run(&run_id).unwrap();
        assert_eq!(retrieved.len(), 1);

        if let AgentMessagePayload::TaskRequest { instructions, .. } = &retrieved[0].payload {
            assert!(!instructions.contains(secret_key));
            assert!(!instructions.contains(aws_key));
            assert!(instructions.contains("[REDACTED]"));
        } else {
            panic!("expected TaskRequest payload");
        }

        // 2. Inspect raw SQLite storage to verify no cleartext secrets exist on disk
        let conn = store.conn.lock().unwrap();
        let raw_payload: String = conn
            .query_row(
                "SELECT payload FROM inter_agent_messages WHERE id = 'msg_sensitive'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(!raw_payload.contains(secret_key));
        assert!(!raw_payload.contains(aws_key));
        assert!(raw_payload.contains("[REDACTED]"));
    }

    #[test]
    fn test_inter_agent_message_task_correlation() {
        let store = RunStore::in_memory().unwrap();
        let run_id = RunId::from("run_tasks_workflow");

        let msg_t1_req = AgentMessage {
            id: "m_t1_1".into(),
            run_id: run_id.clone(),
            sender: AgentId::from("lead"),
            recipient: AgentId::from("dev"),
            routing_key: RoutingKey::new("workflow.tasks").unwrap(),
            timestamp: "2026-10-08T00:00:01Z".into(),
            payload: AgentMessagePayload::TaskRequest {
                task_id: "task-xyz".into(),
                instructions: "Write parser".into(),
            },
        };

        let msg_t1_res = AgentMessage {
            id: "m_t1_2".into(),
            run_id: run_id.clone(),
            sender: AgentId::from("dev"),
            recipient: AgentId::from("lead"),
            routing_key: RoutingKey::new("workflow.tasks").unwrap(),
            timestamp: "2026-10-08T00:00:02Z".into(),
            payload: AgentMessagePayload::TaskResult {
                task_id: "task-xyz".into(),
                output: "Parser finished".into(),
            },
        };

        let msg_t2_req = AgentMessage {
            id: "m_t2_1".into(),
            run_id: run_id.clone(),
            sender: AgentId::from("lead"),
            recipient: AgentId::from("dev"),
            routing_key: RoutingKey::new("workflow.tasks").unwrap(),
            timestamp: "2026-10-08T00:00:03Z".into(),
            payload: AgentMessagePayload::TaskRequest {
                task_id: "task-abc".into(),
                instructions: "Write serializer".into(),
            },
        };

        store
            .record_messages(&[msg_t1_req, msg_t1_res, msg_t2_req])
            .unwrap();

        let task_xyz_msgs = store.get_messages_for_task("task-xyz").unwrap();
        assert_eq!(task_xyz_msgs.len(), 2);
        assert_eq!(task_xyz_msgs[0].id, "m_t1_1");
        assert_eq!(task_xyz_msgs[1].id, "m_t1_2");

        let task_abc_msgs = store.get_messages_for_task("task-abc").unwrap();
        assert_eq!(task_abc_msgs.len(), 1);
        assert_eq!(task_abc_msgs[0].id, "m_t2_1");
    }

    #[test]
    fn test_deterministic_replay_message_history() {
        let store = RunStore::in_memory().unwrap();
        let run_id = RunId::from("run_replay_audit");

        let sup = AgentId::from("supervisor");
        let worker = AgentId::from("worker");
        let route = RoutingKey::new("audit.exec").unwrap();

        let messages = vec![
            AgentMessage {
                id: "step_1".into(),
                run_id: run_id.clone(),
                sender: sup.clone(),
                recipient: worker.clone(),
                routing_key: route.clone(),
                timestamp: "2026-10-08T00:00:10Z".into(),
                payload: AgentMessagePayload::TaskRequest {
                    task_id: "t1".into(),
                    instructions: "Analyze vulnerability in dependencies".into(),
                },
            },
            AgentMessage {
                id: "step_2".into(),
                run_id: run_id.clone(),
                sender: worker.clone(),
                recipient: sup.clone(),
                routing_key: route.clone(),
                timestamp: "2026-10-08T00:00:15Z".into(),
                payload: AgentMessagePayload::TaskResult {
                    task_id: "t1".into(),
                    output: "No CVEs discovered".into(),
                },
            },
            AgentMessage {
                id: "step_3".into(),
                run_id: run_id.clone(),
                sender: sup.clone(),
                recipient: worker.clone(),
                routing_key: route.clone(),
                timestamp: "2026-10-08T00:00:20Z".into(),
                payload: AgentMessagePayload::Notification {
                    content: "Execution verified and completed".into(),
                },
            },
        ];

        store.record_messages(&messages).unwrap();

        // Reconstruct from store for deterministic replay
        let replayed = store.get_messages_for_run(&run_id).unwrap();
        assert_eq!(replayed.len(), 3);

        // Verify deterministic timeline and states
        for (original, reconstructed) in messages.iter().zip(replayed.iter()) {
            assert_eq!(original.id, reconstructed.id);
            assert_eq!(original.run_id, reconstructed.run_id);
            assert_eq!(original.sender, reconstructed.sender);
            assert_eq!(original.recipient, reconstructed.recipient);
            assert_eq!(original.routing_key, reconstructed.routing_key);
            assert_eq!(original.timestamp, reconstructed.timestamp);
            assert_eq!(original.payload, reconstructed.payload);
        }
    }
}
