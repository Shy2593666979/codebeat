#[test]
#[ignore = "Registers a temporary current-user Windows Task Scheduler entry; run explicitly on Windows"]
fn register_query_remove_current_user_task() {
    use codebeat_lib::*;
    let runner = std::env::current_exe().unwrap();
    let request = CreateTaskRequest {
        locked_session_behavior: Default::default(),
        recurrence: None,
        conversation: Conversation {
            lock_state: Default::default(),
            provider: ProviderId::Codex,
            session_id: uuid::Uuid::new_v4().to_string(),
            title: "fixture".into(),
            cwd: std::env::current_dir().unwrap(),
            last_updated_utc: None,
            resumable: true,
            source: "test".into(),
        },
        title: "isolated scheduler integration test".into(),
        prompt: "not transmitted".into(),
        scheduled_at_utc: chrono::Utc::now() + chrono::Duration::hours(2),
        scheduled_local: "2026-10-16T12:00".into(),
        timezone_id: "Asia/Shanghai".into(),
        wake_to_run: false,
        catch_up_policy: CatchUpPolicy::Skip,
        catch_up_ttl_minutes: 60,
        permission_profile: PermissionProfile::ReadOnly,
        consent: true,
        notify: false,
    };
    let id = uuid::Uuid::new_v4().to_string();
    let task = Task {
        id: id.clone(),
        request,
        state: "pending_registration".into(),
        message: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    struct Cleanup(String);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = codebeat_lib::platform::unregister(&self.0);
        }
    }
    let _cleanup = Cleanup(id.clone());
    codebeat_lib::platform::register(&task, &runner).unwrap();
    let report = codebeat_lib::platform::diagnostics().unwrap();
    assert!(report.to_string().contains(&id));
    codebeat_lib::platform::unregister(&id).unwrap();
}
