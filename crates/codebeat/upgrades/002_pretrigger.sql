BEGIN IMMEDIATE;
CREATE TABLE IF NOT EXISTS pretrigger_runs (
 provider_id TEXT NOT NULL, slot TEXT NOT NULL, state TEXT NOT NULL,
 data_json TEXT NOT NULL, started_at TEXT NOT NULL,
 PRIMARY KEY(provider_id,slot)
);
PRAGMA user_version=2;
COMMIT;
