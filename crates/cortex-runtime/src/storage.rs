//! SQLite-backed execution run and event stream persistence.
//!
//! Provides schema versioning, transaction-wrapped migrations, run lifecycle tracking,
//! and event stream storage for the Cortex runtime.

use chrono::Utc;
use cortex_core::{AgentId, CortexError, EventRecord, Result, RunId};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Mutex;

/// Current schema migration version.
pub const CURRENT_SCHEMA_VERSION: i32 = 2;
const SCHEMA_V1: &str = include_str!("../migrations/001_initial_schema.sql");
const SCHEMA_V2: &str = include_str!("../migrations/002_agents_and_checkpoints.sql");

/// Persistent database record for an agent worker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentRecord {
    /// Unique agent identifier.
    pub id: AgentId,
    /// Human-readable agent name.
    pub name: String,
    /// Declarative YAML manifest content.
    pub manifest_yaml: String,
    /// Current lifecycle status string (e.g., "created", "running", "paused", "stopped", "failed").
    pub status: String,
    /// Creation timestamp in ISO 8601 UTC.
    pub created_at: String,
    /// Last update timestamp in ISO 8601 UTC.
    pub updated_at: String,
}

/// Persistent execution checkpoint for an agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentCheckpointRecord {
    /// Auto-incrementing database ID.
    pub id: Option<i64>,
    /// Associated agent ID.
    pub agent_id: AgentId,
    /// Execution step or sequence number.
    pub step: i64,
    /// Snapshot lifecycle state (e.g., "running", "paused", "stopped").
    pub state: String,
    /// Serialized context, memories, or checkpoint payload JSON.
    pub data_json: Option<String>,
    /// Checkpoint timestamp in ISO 8601 UTC.
    pub created_at: String,
}

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

/// SQLite persistence manager for agent execution runs and structured events.
pub struct RunStore {
    conn: Mutex<Connection>,
}

impl std::fmt::Debug for RunStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RunStore").finish()
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
        };
        store.migrate()?;
        Ok(store)
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

        let current_version: i32 = if table_exists {
            conn.query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
        } else {
            0
        };

        if current_version < 1 {
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

        if current_version < 2 {
            conn.execute_batch(SCHEMA_V2).map_err(|e| {
                CortexError::Internal(format!("failed to apply migration 002: {}", e))
            })?;

            let now = Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO schema_version (version, applied_at) VALUES (?1, ?2)",
                params![2, now],
            )
            .map_err(|e| {
                CortexError::Internal(format!("failed to record schema_version 2: {}", e))
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

    /// Reconcile runs left in 'running' state after an unhandled process termination or crash.
    pub fn reconcile_crashed_runs(&self, error_message: &str) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        let now = Utc::now().to_rfc3339();
        let updated = conn
            .execute(
                "UPDATE runs SET status = 'failed', finished_at = ?1, error = ?2 WHERE status = 'running'",
                params![now, error_message],
            )
            .map_err(|e| CortexError::Internal(format!("failed to reconcile crashed runs: {}", e)))?;

        Ok(updated)
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

    /// Save or update an agent record in SQLite.
    pub fn save_agent(&self, agent: &AgentRecord) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        conn.execute(
            "INSERT INTO agents (id, name, manifest_yaml, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                manifest_yaml = excluded.manifest_yaml,
                status = excluded.status,
                updated_at = excluded.updated_at",
            params![
                agent.id.as_str(),
                agent.name,
                agent.manifest_yaml,
                agent.status,
                agent.created_at,
                agent.updated_at,
            ],
        )
        .map_err(|e| CortexError::Internal(format!("failed to save agent: {}", e)))?;

        Ok(())
    }

    /// Retrieve an agent record by ID.
    pub fn get_agent(&self, id: &AgentId) -> Result<Option<AgentRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        let mut stmt = conn
            .prepare("SELECT id, name, manifest_yaml, status, created_at, updated_at FROM agents WHERE id = ?1")
            .map_err(|e| CortexError::Internal(format!("failed to prepare get_agent query: {}", e)))?;

        let mut rows = stmt
            .query(params![id.as_str()])
            .map_err(|e| CortexError::Internal(format!("failed to query agent: {}", e)))?;

        if let Some(row) = rows
            .next()
            .map_err(|e| CortexError::Internal(format!("failed to fetch agent row: {}", e)))?
        {
            let raw_id: String = row.get(0).map_err(map_sql_err)?;
            let name: String = row.get(1).map_err(map_sql_err)?;
            let manifest_yaml: String = row.get(2).map_err(map_sql_err)?;
            let status: String = row.get(3).map_err(map_sql_err)?;
            let created_at: String = row.get(4).map_err(map_sql_err)?;
            let updated_at: String = row.get(5).map_err(map_sql_err)?;

            Ok(Some(AgentRecord {
                id: AgentId::from(raw_id),
                name,
                manifest_yaml,
                status,
                created_at,
                updated_at,
            }))
        } else {
            Ok(None)
        }
    }

    /// List all agent records ordered by creation time.
    pub fn list_agents(&self) -> Result<Vec<AgentRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        let mut stmt = conn
            .prepare("SELECT id, name, manifest_yaml, status, created_at, updated_at FROM agents ORDER BY created_at ASC")
            .map_err(|e| CortexError::Internal(format!("failed to prepare list_agents query: {}", e)))?;

        let rows = stmt
            .query_map([], |row| {
                let raw_id: String = row.get(0)?;
                let name: String = row.get(1)?;
                let manifest_yaml: String = row.get(2)?;
                let status: String = row.get(3)?;
                let created_at: String = row.get(4)?;
                let updated_at: String = row.get(5)?;

                Ok(AgentRecord {
                    id: AgentId::from(raw_id),
                    name,
                    manifest_yaml,
                    status,
                    created_at,
                    updated_at,
                })
            })
            .map_err(|e| CortexError::Internal(format!("failed to query agents list: {}", e)))?;

        let mut agents = Vec::new();
        for agent in rows {
            agents.push(agent.map_err(map_sql_err)?);
        }

        Ok(agents)
    }

    /// Update the lifecycle status of an agent.
    pub fn update_agent_status(&self, id: &AgentId, status: &str) -> Result<()> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        let now = Utc::now().to_rfc3339();
        let rows_affected = conn
            .execute(
                "UPDATE agents SET status = ?1, updated_at = ?2 WHERE id = ?3",
                params![status, now, id.as_str()],
            )
            .map_err(|e| CortexError::Internal(format!("failed to update agent status: {}", e)))?;

        if rows_affected == 0 {
            return Err(CortexError::NotFound(format!("agent '{}' not found", id)));
        }

        Ok(())
    }

    /// Delete an agent and its checkpoints by ID.
    pub fn delete_agent(&self, id: &AgentId) -> Result<bool> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        let rows = conn
            .execute("DELETE FROM agents WHERE id = ?1", params![id.as_str()])
            .map_err(|e| CortexError::Internal(format!("failed to delete agent: {}", e)))?;

        Ok(rows > 0)
    }

    /// Record an execution checkpoint for an agent.
    pub fn save_agent_checkpoint(&self, checkpoint: &AgentCheckpointRecord) -> Result<i64> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        conn.execute(
            "INSERT INTO agent_checkpoints (agent_id, step, state, data_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                checkpoint.agent_id.as_str(),
                checkpoint.step,
                checkpoint.state,
                checkpoint.data_json,
                checkpoint.created_at,
            ],
        )
        .map_err(|e| CortexError::Internal(format!("failed to save agent checkpoint: {}", e)))?;

        Ok(conn.last_insert_rowid())
    }

    /// List all checkpoints recorded for an agent.
    pub fn list_agent_checkpoints(&self, agent_id: &AgentId) -> Result<Vec<AgentCheckpointRecord>> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        let mut stmt = conn
            .prepare("SELECT id, agent_id, step, state, data_json, created_at FROM agent_checkpoints WHERE agent_id = ?1 ORDER BY step ASC, id ASC")
            .map_err(|e| CortexError::Internal(format!("failed to prepare list_agent_checkpoints query: {}", e)))?;

        let rows = stmt
            .query_map(params![agent_id.as_str()], |row| {
                let id: i64 = row.get(0)?;
                let raw_agent_id: String = row.get(1)?;
                let step: i64 = row.get(2)?;
                let state: String = row.get(3)?;
                let data_json: Option<String> = row.get(4)?;
                let created_at: String = row.get(5)?;

                Ok(AgentCheckpointRecord {
                    id: Some(id),
                    agent_id: AgentId::from(raw_agent_id),
                    step,
                    state,
                    data_json,
                    created_at,
                })
            })
            .map_err(|e| {
                CortexError::Internal(format!("failed to query agent checkpoints: {}", e))
            })?;

        let mut checkpoints = Vec::new();
        for cp in rows {
            checkpoints.push(cp.map_err(map_sql_err)?);
        }

        Ok(checkpoints)
    }

    /// Reconcile agents left in 'running' state after daemon shutdown or crash.
    ///
    /// Interrupted agents are cleanly marked as 'stopped', or remain 'running' if their ID
    /// is included in `auto_resume_ids`.
    pub fn reconcile_crashed_agents_in_store(&self, auto_resume_ids: &[AgentId]) -> Result<usize> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| CortexError::Internal("failed to acquire store lock".to_string()))?;

        let now = Utc::now().to_rfc3339();
        let mut stopped_count = 0;

        let mut stmt = conn
            .prepare("SELECT id FROM agents WHERE status = 'running'")
            .map_err(|e| CortexError::Internal(format!("failed to prepare query: {}", e)))?;

        let running_ids: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| CortexError::Internal(format!("failed to query running agents: {}", e)))?
            .filter_map(|r| r.ok())
            .collect();

        for id_str in running_ids {
            let agent_id = AgentId::from(id_str.as_str());
            if !auto_resume_ids.contains(&agent_id) {
                conn.execute(
                    "UPDATE agents SET status = 'stopped', updated_at = ?1 WHERE id = ?2",
                    params![now, id_str],
                )
                .map_err(|e| CortexError::Internal(format!("failed to reconcile agent: {}", e)))?;
                stopped_count += 1;
            }
        }

        Ok(stopped_count)
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
}
