-- 003_multi_agent_tables.sql
-- Inter-agent messages and multi-agent coordination traces

CREATE TABLE IF NOT EXISTS inter_agent_messages (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL,
    task_id TEXT,
    sender TEXT NOT NULL,
    recipient TEXT NOT NULL,
    routing_key TEXT NOT NULL,
    message_type TEXT NOT NULL,
    payload TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    FOREIGN KEY (run_id) REFERENCES runs(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_messages_run_id ON inter_agent_messages(run_id);
CREATE INDEX IF NOT EXISTS idx_messages_sender ON inter_agent_messages(sender);
CREATE INDEX IF NOT EXISTS idx_messages_recipient ON inter_agent_messages(recipient);
CREATE INDEX IF NOT EXISTS idx_messages_task_id ON inter_agent_messages(task_id);
