BEGIN IMMEDIATE;
ALTER TABLE notification_inbox ADD COLUMN topic TEXT;
CREATE UNIQUE INDEX inbox_topic ON notification_inbox(topic) WHERE topic IS NOT NULL;
PRAGMA user_version=5;
COMMIT;
