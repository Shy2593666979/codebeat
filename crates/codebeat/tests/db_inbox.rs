use chrono::Utc;
use codebeat_lib::db::Database;
use codebeat_lib::InboxNotice;
fn notice(message: &str) -> InboxNotice {
    InboxNotice {
        topic: None,
        id: uuid::Uuid::new_v4().to_string(),
        title: "Task finished".into(),
        message: message.into(),
        kind: "success".into(),
        task_id: None,
        page: "tasks".into(),
        at: Utc::now(),
    }
}
#[test]
fn inbox_is_durable_deduplicated_and_acknowledgements_are_scoped() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.db");
    let mut db = Database::open(&path).unwrap();
    let first = notice("first");
    let second = notice("second");
    db.add_notice(&first).unwrap();
    db.add_notice(&first).unwrap();
    db.add_notice(&second).unwrap();
    assert_eq!(db.pending_notices().unwrap().len(), 2);
    drop(db);
    db = Database::open(&path).unwrap();
    assert_eq!(db.pending_notices().unwrap().len(), 2);
    db.acknowledge_notices(&[first.id]).unwrap();
    assert_eq!(db.pending_notices().unwrap(), vec![second]);
    db.acknowledge_all_notices().unwrap();
    assert!(db.pending_notices().unwrap().is_empty());
    let third = notice("new after ignore all");
    db.add_notice(&third).unwrap();
    assert_eq!(db.pending_notices().unwrap(), vec![third]);
}
#[test]
fn invalid_targets_and_acknowledgements_cannot_mutate_the_inbox() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Database::open(&dir.path().join("test.db")).unwrap();
    let good = notice("kept");
    db.add_notice(&good).unwrap();
    let mut bad = notice("invalid");
    bad.task_id = Some("not-a-uuid".into());
    assert!(db.add_notice(&bad).is_err());
    bad.task_id = None;
    bad.page = "https://example.com".into();
    assert!(db.add_notice(&bad).is_err());
    assert!(db
        .acknowledge_notices(&[good.id.clone(), "bad".into()])
        .is_err());
    assert_eq!(db.pending_notices().unwrap(), vec![good]);
}
#[test]
fn upgrades_existing_version_two_database_without_changing_settings() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("old.db");
    let db = Database::open(&path).unwrap();
    let mut settings = db.settings().unwrap();
    settings.default_time = "09:45".into();
    db.save_settings(&settings).unwrap();
    drop(db);
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("DROP TABLE notification_inbox; PRAGMA user_version=2;")
        .unwrap();
    drop(conn);
    let db = Database::open(&path).unwrap();
    assert_eq!(db.settings().unwrap().default_time, "09:45");
    db.add_notice(&notice("migrated")).unwrap();
    assert_eq!(db.pending_notices().unwrap().len(), 1);
}

#[test]
fn ignore_all_includes_messages_beyond_the_visible_batch() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Database::open(&dir.path().join("test.db")).unwrap();
    for _ in 0..205 {
        db.add_notice(&notice("queued")).unwrap();
    }
    assert_eq!(db.pending_notices().unwrap().len(), 200);
    assert_eq!(db.acknowledge_all_notices().unwrap().len(), 205);
    assert!(db.pending_notices().unwrap().is_empty());
    db.add_notice(&notice("after ignore")).unwrap();
    assert_eq!(db.pending_notices().unwrap().len(), 1);
}

#[test]
fn topics_replace_in_place_and_old_acknowledgements_cannot_dismiss_the_new_revision() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Database::open(&dir.path().join("topics.db")).unwrap();
    let mut first = notice("enabled");
    first.topic = Some("provider:codex:scheduling".into());
    db.add_notice(&first).unwrap();
    let mut quota = notice("quota exhausted");
    quota.topic = Some("provider:codex:quota-exhausted".into());
    db.add_notice(&quota).unwrap();
    let before = db.pending_notices().unwrap();
    let index = before.iter().position(|n| n.id == first.id).unwrap();
    let mut changed = notice("disabled");
    changed.topic = first.topic.clone();
    db.add_notice(&changed).unwrap();
    let after = db.pending_notices().unwrap();
    assert_eq!(after.len(), 2);
    assert_eq!(after[index], changed);
    assert!(dir
        .path()
        .join("notification-acks")
        .join(format!("{}.dismiss", first.id))
        .is_file());
    db.acknowledge_notices(&[first.id]).unwrap();
    assert_eq!(db.pending_notices().unwrap().len(), 2);
    let mut cc = notice("claude enabled");
    cc.topic = Some("provider:claude_code:scheduling".into());
    db.add_notice(&cc).unwrap();
    assert_eq!(db.pending_notices().unwrap().len(), 3);
    db.acknowledge_notices(&[changed.id]).unwrap();
    let mut again = notice("enabled again");
    again.topic = changed.topic;
    db.add_notice(&again).unwrap();
    assert_eq!(db.pending_notices().unwrap().len(), 3);
}
