BEGIN IMMEDIATE;
CREATE TABLE IF NOT EXISTS notification_inbox (
 id TEXT PRIMARY KEY, data_json TEXT NOT NULL, created_at TEXT NOT NULL,
 acknowledged INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS inbox_pending ON notification_inbox(acknowledged,created_at);
PRAGMA user_version=3;
COMMIT;
