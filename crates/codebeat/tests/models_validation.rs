use chrono::{Duration, Timelike, Utc};
use codebeat_lib::*;
#[test]
fn project_directories_and_request_consent_are_enabled_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let utc = (Utc::now() + Duration::hours(1))
        .with_second(0)
        .unwrap()
        .with_nanosecond(0)
        .unwrap();
    let mut s = Settings {
        authorized_roots: vec![dir.path().to_owned()],
        ..Default::default()
    };
    let mut r = CreateTaskRequest {
        locked_session_behavior: Default::default(),
        recurrence: None,
        conversation: Conversation {
            lock_state: Default::default(),
            provider: ProviderId::Codex,
            session_id: uuid::Uuid::new_v4().to_string(),
            title: "测试".into(),
            cwd: dir.path().to_owned(),
            last_updated_utc: None,
            resumable: true,
            source: "test".into(),
        },
        title: "test".into(),
        prompt: "检查中文测试".into(),
        scheduled_at_utc: utc,
        scheduled_local: utc.format("%Y-%m-%dT%H:%M").to_string(),
        timezone_id: "UTC".into(),
        wake_to_run: false,
        catch_up_policy: CatchUpPolicy::Ask,
        catch_up_ttl_minutes: 720,
        permission_profile: PermissionProfile::ReadOnly,
        consent: true,
        notify: false,
    };
    assert!(validate_task(&r, &s, Utc::now(), true).is_ok());
    r.consent = false;
    assert!(validate_task(&r, &s, Utc::now(), true).is_ok());
    let mut json = serde_json::to_value(&r).unwrap();
    json.as_object_mut().unwrap().remove("consent");
    let implicit: CreateTaskRequest = serde_json::from_value(json).unwrap();
    assert!(implicit.consent);
    assert!(validate_task(&implicit, &Settings::default(), Utc::now(), true).is_ok());
    r.consent = true;
    r.conversation.resumable = false;
    assert!(validate_task(&r, &s, Utc::now(), true).is_err());
    r.conversation.resumable = true;
    s.authorized_roots.clear();
    assert!(validate_task(&r, &s, Utc::now(), true).is_ok());
    let other = tempfile::tempdir().unwrap();
    s.authorized_roots.push(other.path().to_owned());
    assert!(validate_task(&r, &s, Utc::now(), true).is_ok());
    r.conversation.cwd = dir.path().join("missing");
    assert!(validate_task(&r, &s, Utc::now(), true)
        .unwrap_err()
        .to_string()
        .contains("CWD_MISSING"));
    std::fs::write(dir.path().join("file.txt"), "fixture").unwrap();
    r.conversation.cwd = dir.path().join("file.txt");
    assert!(validate_task(&r, &s, Utc::now(), true).is_err());
}
#[test]
fn backend_accepts_valid_pretrigger_and_rejects_invalid_work_times() {
    let s = Settings {
        pretrigger_enabled: true,
        ..Default::default()
    };
    assert!(validate_settings(&s).is_ok());
    let mut invalid = s;
    invalid.pretrigger.work_end = "10:00".into();
    assert!(validate_settings(&invalid).is_err());
}
