use chrono::{Duration, Utc};
use codebeat_lib::db::Database;
use codebeat_lib::*;
#[test]
fn migration_preserves_legacy_settings_and_probe_claim_is_atomic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v1.db");
    let old = rusqlite::Connection::open(&path).unwrap();
    old.execute_batch(include_str!("../upgrades/001_initial.sql"))
        .unwrap();
    old.execute(
        "INSERT INTO app_settings VALUES('settings','{\"theme\":\"teal\"}')",
        [],
    )
    .unwrap();
    drop(old);
    let mut db = Database::open(&path).unwrap();
    let mut settings = db.settings().unwrap();
    assert_eq!(settings.theme, "teal");
    assert!(settings.pretrigger_enabled);
    settings.pretrigger_enabled = false;
    db.save_settings(&settings).unwrap();
    assert!(!db
        .claim_pretrigger(ProviderId::Codex, "round", Utc::now())
        .unwrap());
    settings.pretrigger_enabled = true;
    db.save_settings(&settings).unwrap();
    assert!(db
        .claim_pretrigger(ProviderId::Codex, "round", Utc::now())
        .unwrap());
    let mut second = Database::open(&path).unwrap();
    assert!(!second
        .claim_pretrigger(ProviderId::Codex, "round", Utc::now())
        .unwrap());
    let mut r = db.pretrigger_runs().unwrap().remove(0);
    r.state = "waiting".into();
    r.next_check_at = Some(Utc::now() + Duration::hours(1));
    db.finish_pretrigger(&r).unwrap();
    assert!(!second
        .claim_pretrigger(ProviderId::Codex, "round", Utc::now())
        .unwrap());
    assert!(second
        .claim_pretrigger(ProviderId::Codex, "round", Utc::now() + Duration::hours(2))
        .unwrap());
    r.state = "sent_unconfirmed".into();
    r.next_check_at = None;
    db.finish_pretrigger(&r).unwrap();
    assert!(!second
        .claim_pretrigger(ProviderId::Codex, "round", Utc::now() + Duration::hours(3))
        .unwrap());
    settings.paused = true;
    db.save_settings(&settings).unwrap();
    assert!(!second
        .claim_pretrigger(ProviderId::Codex, "newround", Utc::now())
        .unwrap());
}

#[test]
fn coding_allocation_defaults_are_persisted_and_upgrade_runs_only_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("allocation.db");
    let db = Database::open(&path).unwrap();
    let mut settings = db.settings().unwrap();
    assert!(
        settings.pretrigger_enabled && settings.pretrigger.auto_trigger && settings.pretrigger.wake
    );
    settings.pretrigger_enabled = false;
    settings.pretrigger.auto_trigger = false;
    settings.pretrigger.wake = false;
    settings.pretrigger.mode = "save".into();
    db.save_settings(&settings).unwrap();
    drop(db);
    let old = rusqlite::Connection::open(&path).unwrap();
    old.execute_batch("PRAGMA user_version=5;").unwrap();
    drop(old);
    let db = Database::open(&path).unwrap();
    let mut settings = db.settings().unwrap();
    assert!(
        settings.pretrigger_enabled && settings.pretrigger.auto_trigger && settings.pretrigger.wake
    );
    assert_eq!(settings.pretrigger.mode, "balanced");
    settings.pretrigger_enabled = false;
    db.save_settings(&settings).unwrap();
    drop(db);
    assert!(
        !Database::open(&path)
            .unwrap()
            .settings()
            .unwrap()
            .pretrigger_enabled
    );
}

#[test]
fn coding_allocation_upgrade_preserves_a_protection_pause() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("protected.db");
    drop(Database::open(&path).unwrap());
    let old = rusqlite::Connection::open(&path).unwrap();
    old.execute("UPDATE app_settings SET value_json=json_set(value_json,'$.pretrigger.auto_trigger',json('false'))",[]).unwrap();
    old.execute(
        "INSERT INTO pretrigger_runs VALUES('codex','protected','cost_paused','{}',?1)",
        [Utc::now().to_rfc3339()],
    )
    .unwrap();
    old.execute_batch("PRAGMA user_version=5;").unwrap();
    drop(old);
    let settings = Database::open(&path).unwrap().settings().unwrap();
    assert!(settings.pretrigger_enabled && settings.pretrigger.wake);
    assert!(!settings.pretrigger.auto_trigger);
}
