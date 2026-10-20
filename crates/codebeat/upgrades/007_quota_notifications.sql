BEGIN IMMEDIATE;
CREATE TABLE IF NOT EXISTS quota_notice_state(provider_id TEXT PRIMARY KEY, data_json TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS quota_reset_consumptions(attempt_id TEXT PRIMARY KEY, account_key TEXT NOT NULL, consumed_at TEXT NOT NULL, reconciled INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS quota_credit_changes(account_key TEXT PRIMARY KEY, before_json TEXT NOT NULL, observed_at TEXT NOT NULL);
PRAGMA user_version=7;
COMMIT;
