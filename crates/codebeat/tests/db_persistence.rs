use chrono::{Duration, Utc};
use codebeat_lib::db::Database;
use codebeat_lib::*;
use std::sync::{Arc, Barrier};

#[test]
fn child_claim_worker() {
    let Ok(path) = std::env::var("CODEBEAT_TEST_DATABASE") else {
        return;
    };
    let id = std::env::var("CODEBEAT_TEST_TASK").unwrap();
    let mut db = Database::open(std::path::Path::new(&path)).unwrap();
    let claimed = db.claim(&id, false).unwrap().is_some();
    std::process::exit(if claimed { 42 } else { 0 });
}

#[test]
fn orm_opens_relative_unicode_paths_and_in_memory_databases() {
    let dir = tempfile::Builder::new()
        .prefix(".orm-path-")
        .tempdir_in(".")
        .unwrap();
    let path = std::path::PathBuf::from(dir.path().file_name().unwrap()).join("中文 # quota.db");
    assert!(path.is_relative());
    let db = Database::open(&path).unwrap();
    assert!(db.notify_once("relative").unwrap());
    drop(db);
    assert!(!Database::open(&path)
        .unwrap()
        .notify_once("relative")
        .unwrap());
    let memory = Database::open(std::path::Path::new(":memory:")).unwrap();
    assert!(memory.notify_once("memory").unwrap());
}

#[test]
fn skipped_recurring_occurrence_advances_once_without_running() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Database::open(&dir.path().join("repeat.db")).unwrap();
    let mut r = request();
    r.recurrence = Some(Recurrence::Daily);
    r.catch_up_policy = CatchUpPolicy::Skip;
    r.scheduled_at_utc = Utc::now() - Duration::days(2);
    let task = db.insert_task(r).unwrap();
    db.transition(&task.id, &["pending_registration"], "scheduled", None)
        .unwrap();
    assert!(db.claim(&task.id, false).unwrap().is_none());
    assert_eq!(db.task(&task.id).unwrap().state, "expired");
    let next = db.prepare_next_occurrence(&task.id).unwrap().unwrap();
    assert!(next.request.scheduled_at_utc > Utc::now());
    assert!(db.prepare_next_occurrence(&task.id).unwrap().is_none());
    assert!(db.runs().unwrap().is_empty());
}
#[test]
fn multiple_processes_claim_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("process.db");
    let db = Database::open(&path).unwrap();
    let task = db.insert_task(request()).unwrap();
    db.transition(&task.id, &["pending_registration"], "scheduled", None)
        .unwrap();
    let mut children: Vec<_> = (0..4)
        .map(|_| {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "child_claim_worker", "--nocapture"])
                .env("CODEBEAT_TEST_DATABASE", &path)
                .env("CODEBEAT_TEST_TASK", &task.id)
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap()
        })
        .collect();
    let count = children
        .iter_mut()
        .map(|child| child.wait().unwrap().code())
        .filter(|code| *code == Some(42))
        .count();
    assert_eq!(count, 1);
    assert_eq!(db.runs().unwrap().len(), 1);
}
fn request() -> CreateTaskRequest {
    CreateTaskRequest {
        locked_session_behavior: Default::default(),
        recurrence: None,
        conversation: Conversation {
            lock_state: Default::default(),
            provider: ProviderId::Codex,
            session_id: uuid::Uuid::new_v4().to_string(),
            title: "fixture".into(),
            cwd: ".".into(),
            last_updated_utc: None,
            resumable: true,
            source: "test".into(),
        },
        title: "test".into(),
        prompt: "test".into(),
        scheduled_at_utc: Utc::now() - Duration::seconds(20),
        scheduled_local: "2026-10-16T12:00".into(),
        timezone_id: "Asia/Shanghai".into(),
        wake_to_run: false,
        catch_up_policy: CatchUpPolicy::Ask,
        catch_up_ttl_minutes: 720,
        permission_profile: PermissionProfile::ReadOnly,
        consent: true,
        notify: false,
    }
}
#[test]
fn concurrent_connections_claim_once() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("codebeat.db");
    let db = Database::open(&path).unwrap();
    let task = db.insert_task(request()).unwrap();
    db.transition(&task.id, &["pending_registration"], "scheduled", None)
        .unwrap();
    let barrier = Arc::new(Barrier::new(6));
    let handles: Vec<_> = (0..6)
        .map(|_| {
            let path = path.clone();
            let id = task.id.clone();
            let b = barrier.clone();
            std::thread::spawn(move || {
                let mut db = Database::open(&path).unwrap();
                b.wait();
                db.claim(&id, false).unwrap().is_some()
            })
        })
        .collect();
    let count = handles
        .into_iter()
        .filter_map(|h| h.join().ok())
        .filter(|v| *v)
        .count();
    assert_eq!(count, 1);
    assert_eq!(db.runs().unwrap().len(), 1);
}

#[test]
fn failed_run_insert_rolls_back_the_task_claim_and_releases_the_connection() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rollback.db");
    let mut db = Database::open(&path).unwrap();
    let task = db.insert_task(request()).unwrap();
    db.transition(&task.id, &["pending_registration"], "scheduled", None)
        .unwrap();
    let sql = rusqlite::Connection::open(&path).unwrap();
    sql.execute_batch("CREATE TRIGGER reject_run BEFORE INSERT ON task_runs BEGIN SELECT RAISE(ABORT,'injected run failure'); END;").unwrap();
    assert!(db.claim(&task.id, true).is_err());
    assert_eq!(db.task(&task.id).unwrap().state, "scheduled");
    assert!(db.runs().unwrap().is_empty());
    sql.execute_batch("DROP TRIGGER reject_run;").unwrap();
    assert!(db.claim(&task.id, true).unwrap().is_some());
}
#[test]
fn finished_task_never_replayed() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Database::open(&dir.path().join("k.db")).unwrap();
    let task = db.insert_task(request()).unwrap();
    db.transition(&task.id, &["pending_registration"], "scheduled", None)
        .unwrap();
    let run = db.claim(&task.id, true).unwrap().unwrap();
    db.finish(&run.id, "succeeded", Some(0), None, None)
        .unwrap();
    assert!(db.claim(&task.id, true).unwrap().is_none());
    assert_eq!(db.runs().unwrap()[0].state, "succeeded");
}

#[test]
fn stale_run_is_reviewed_and_late_success_cannot_override_review() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("stale.db");
    let mut db = Database::open(&path).unwrap();
    let task = db.insert_task(request()).unwrap();
    db.transition(&task.id, &["pending_registration"], "scheduled", None)
        .unwrap();
    let run = db.claim(&task.id, true).unwrap().unwrap();
    let other = rusqlite::Connection::open(&path).unwrap();
    other
        .execute(
            "UPDATE task_runs SET heartbeat_at=?1",
            [(Utc::now() - Duration::minutes(4)).to_rfc3339()],
        )
        .unwrap();
    assert_eq!(db.recover_stale().unwrap(), 1);
    assert_eq!(db.task(&task.id).unwrap().state, "needs_review");
    db.finish(&run.id, "succeeded", Some(0), None, None)
        .unwrap();
    assert_eq!(db.task(&task.id).unwrap().state, "needs_review");
    assert!(db.claim(&task.id, true).unwrap().is_none());
}
#[test]
fn missed_policy_and_ttl() {
    let mut r = request();
    r.scheduled_at_utc = Utc::now() - Duration::minutes(10);
    assert_eq!(missed_state(&r, Utc::now()), "needs_review");
    r.catch_up_policy = CatchUpPolicy::Skip;
    assert_eq!(missed_state(&r, Utc::now()), "expired");
    r.catch_up_policy = CatchUpPolicy::RunOnNextStart;
    assert_eq!(missed_state(&r, Utc::now()), "scheduled");
    r.scheduled_at_utc = Utc::now() - Duration::hours(13);
    assert_eq!(missed_state(&r, Utc::now()), "expired");
}
#[test]
fn persisted_settings_and_notification_dedup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("k.db");
    {
        let db = Database::open(&path).unwrap();
        let mut s = db.settings().unwrap();
        assert!(s.pretrigger_enabled);
        s.theme = "teal".into();
        db.save_settings(&s).unwrap();
        assert!(db.notify_once("reset-1").unwrap());
    }
    let db = Database::open(&path).unwrap();
    assert_eq!(db.settings().unwrap().theme, "teal");
    assert!(!db.notify_once("reset-1").unwrap());
}

#[test]
fn ten_minute_interval_roundtrips_and_legacy_fifteen_is_normalized() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("interval.db");
    let db = Database::open(&path).unwrap();
    let mut settings = Settings {
        connection_interval_minutes: 10,
        ..Default::default()
    };
    db.save_settings(&settings).unwrap();
    assert_eq!(db.settings().unwrap().connection_interval_minutes, 10);
    settings.connection_interval_minutes = 15;
    assert!(db.save_settings(&settings).is_err());
    drop(db);
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute(
        "UPDATE app_settings SET value_json=?1 WHERE key='settings'",
        [serde_json::to_string(&settings).unwrap()],
    )
    .unwrap();
    drop(conn);
    let db = Database::open(&path).unwrap();
    let loaded = db.settings().unwrap();
    assert_eq!(loaded.connection_interval_minutes, 10);
    db.save_settings(&loaded).unwrap();
    drop(db);
    assert_eq!(
        Database::open(&path)
            .unwrap()
            .settings()
            .unwrap()
            .connection_interval_minutes,
        10
    );
}
