CREATE TABLE IF NOT EXISTS tasks (
 id TEXT PRIMARY KEY, request_json TEXT NOT NULL, state TEXT NOT NULL,
 message TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_tasks_due ON tasks(state,updated_at);
CREATE TABLE IF NOT EXISTS task_runs (
 id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id), attempt INTEGER NOT NULL,
 state TEXT NOT NULL, started_at TEXT NOT NULL, finished_at TEXT, exit_code INTEGER,
 error_code TEXT,error_message TEXT,log_path TEXT,pid INTEGER,heartbeat_at TEXT NOT NULL,
 UNIQUE(task_id,attempt)
);
CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY,value_json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS notification_dedup (dedup_key TEXT PRIMARY KEY,emitted_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS providers (id TEXT PRIMARY KEY,data_json TEXT NOT NULL,last_checked_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS conversations (provider_id TEXT NOT NULL,session_id TEXT NOT NULL,data_json TEXT NOT NULL,PRIMARY KEY(provider_id,session_id));
CREATE TABLE IF NOT EXISTS usage_snapshots (id TEXT PRIMARY KEY,provider_id TEXT NOT NULL,data_json TEXT NOT NULL,observed_at_utc TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS idx_usage_recent ON usage_snapshots(provider_id,observed_at_utc DESC);
PRAGMA user_version=1;
