use chrono::{Duration, Utc};
use codebeat_lib::{db::Database, tasks::service, *};
fn request() -> CreateTaskRequest {
    let at = Utc::now() + Duration::hours(1);
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
        scheduled_at_utc: at,
        scheduled_local: at.format("%Y-%m-%dT%H:%M").to_string(),
        timezone_id: "UTC".into(),
        wake_to_run: false,
        catch_up_policy: CatchUpPolicy::Ask,
        catch_up_ttl_minutes: 720,
        permission_profile: PermissionProfile::ReadOnly,
        consent: true,
        notify: false,
    }
}
#[test]
fn shared_task_service_preserves_pause_guards_without_spawning_a_runner() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(&dir.path().join("service.db")).unwrap();
    let task = db.insert_task(request()).unwrap();
    db.transition(&task.id, &["pending_registration"], "scheduled", None)
        .unwrap();
    assert_eq!(
        service::pause_task(&db, task.id.clone(), true)
            .unwrap()
            .state,
        "paused"
    );
    assert!(service::run_task_now(&db, task.id.clone())
        .unwrap_err()
        .to_string()
        .contains("INVALID_STATE"));
    assert_eq!(
        service::pause_task(&db, task.id.clone(), false)
            .unwrap()
            .state,
        "scheduled"
    );
    let mut settings = db.settings().unwrap();
    settings.paused = true;
    db.save_settings(&settings).unwrap();
    assert!(service::run_task_now(&db, task.id.clone())
        .unwrap_err()
        .to_string()
        .contains("SCHEDULING_PAUSED"));
    settings.paused = false;
    settings.paused_providers = vec![ProviderId::Codex];
    db.save_settings(&settings).unwrap();
    assert!(service::run_task_now(&db, task.id)
        .unwrap_err()
        .to_string()
        .contains("SCHEDULING_PAUSED"));
    assert!(db.runs().unwrap().is_empty());
}
#[test]
fn shared_task_service_cancellation_reaches_the_independent_executor_heartbeat() {
    let dir = tempfile::tempdir().unwrap();
    let mut db = Database::open(&dir.path().join("cancel.db")).unwrap();
    let task = db.insert_task(request()).unwrap();
    db.transition(&task.id, &["pending_registration"], "scheduled", None)
        .unwrap();
    let run = db.claim(&task.id, true).unwrap().unwrap();
    assert_eq!(
        service::cancel_task(&db, task.id).unwrap().state,
        "cancelling"
    );
    assert!(db.heartbeat(&run.id).unwrap());
}
