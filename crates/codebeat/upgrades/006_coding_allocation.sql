BEGIN IMMEDIATE;
UPDATE app_settings SET value_json=json_set(value_json,
 '$.pretrigger_enabled',json('true'),
 '$.pretrigger.mode','balanced',
 '$.pretrigger.wake',json('true'),
 '$.pretrigger.auto_trigger',json(CASE WHEN EXISTS(
   SELECT 1 FROM pretrigger_runs WHERE state IN ('cost_paused','sent_unconfirmed','failed')
 ) THEN 'false' ELSE 'true' END)) WHERE key='settings';
PRAGMA user_version=6;
COMMIT;
