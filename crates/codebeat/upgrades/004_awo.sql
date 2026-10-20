BEGIN IMMEDIATE;
CREATE TABLE IF NOT EXISTS awo_runtime(provider_id TEXT PRIMARY KEY, data_json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS awo_attempts(provider_id TEXT NOT NULL, slot TEXT NOT NULL, attempted_at TEXT NOT NULL, PRIMARY KEY(provider_id,slot));
CREATE TABLE IF NOT EXISTS awo_activity(provider_id TEXT NOT NULL, observed_at TEXT NOT NULL, PRIMARY KEY(provider_id,observed_at));
PRAGMA user_version=4;
COMMIT;
