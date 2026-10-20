use codebeat_lib::providers::*;
use codebeat_lib::*;
use serde_json::json;
#[test]
fn usage_multiple_windows_and_milliseconds() {
    let v = serde_json::from_str(include_str!("fixtures/rate-limits.json")).unwrap();
    let windows = parse_usage(&v);
    assert_eq!(windows.len(), 3);
    assert!(windows.iter().any(|w| w.duration_minutes == Some(10080)));
    let extra = windows
        .iter()
        .find(|w| w.label.starts_with("extra"))
        .unwrap();
    assert_eq!(extra.used_percent, None);
    assert_eq!(extra.resets_at_utc.unwrap().timestamp(), 1792195200);
}
#[test]
fn malformed_percent_is_unknown() {
    for value in [json!(-1), json!(101), json!("25"), json!(null)] {
        let w = parse_usage(&json!({"rateLimits":{"primary":{"usedPercent":value}}}));
        assert_eq!(w[0].used_percent, None);
    }
    assert!(parse_usage(&json!({})).is_empty());
}
#[test]
fn zero_exit_is_not_success() {
    assert_eq!(classify_event(&json!({"type":"process_started"})), None);
    assert_eq!(
        classify_event(&json!({"type":"turn.completed"})),
        Some("succeeded")
    );
    assert_eq!(
        classify_event(&json!({"type":"result","subtype":"success","permission_denials":[{}]})),
        Some("blocked_approval")
    );
}
#[test]
fn secret_redaction() {
    let s = redact("Bearer abc.def.ghi sk-abcdefgh12345678 api_key=supersecret");
    assert!(!s.contains("abc.def"));
    assert!(!s.contains("supersecret"));
    assert!(!s.contains("sk-"));
    assert!(
        !redact(r#"{"apiKey":"supersecret","access_token":"tokenvalue"}"#).contains("supersecret")
    );
    assert!(!redact(r#"{"access_token":"tokenvalue"}"#).contains("tokenvalue"));
}
#[test]
fn claude_bom_unicode_truncated_and_pagination() {
    let dir = tempfile::tempdir().unwrap();
    let project = dir.path().join("project");
    std::fs::create_dir(&project).unwrap();
    let id = uuid::Uuid::new_v4().to_string();
    let file = project.join(format!("{id}.jsonl"));
    let v = json!({"sessionId":id,"cwd":dir.path(),"type":"user","message":{"content":"检查中文路径测试"},"timestamp":"2026-10-16T00:00:00Z"});
    std::fs::write(&file, format!("\u{feff}{v}\n{{truncated\n")).unwrap();
    let page = scan_claude(dir.path(), "中文", None, 1).unwrap();
    assert_eq!(page.data.len(), 1);
    assert!(page.data[0].resumable);
    assert_eq!(page.data[0].title, "检查中文路径测试");
    assert!(scan_claude(dir.path(), "absent", None, 10)
        .unwrap()
        .data
        .is_empty());
    assert!(scan_claude(dir.path(), "", Some("invalid".into()), 10).is_err());
}
#[test]
fn no_shell_prompt_arguments() {
    let c = Conversation {
        lock_state: Default::default(),
        provider: ProviderId::Codex,
        session_id: uuid::Uuid::new_v4().to_string(),
        title: "测试".into(),
        cwd: ".".into(),
        last_updated_utc: None,
        resumable: true,
        source: "fixture".into(),
    };
    let mut r = CreateTaskRequest {
        locked_session_behavior: Default::default(),
        recurrence: None,
        conversation: c,
        title: "test".into(),
        prompt: "中文 ` $() \"; & del C:\\*\nline".into(),
        scheduled_at_utc: chrono::Utc::now(),
        scheduled_local: "2026-10-16T12:00".into(),
        timezone_id: "Asia/Shanghai".into(),
        wake_to_run: false,
        catch_up_policy: CatchUpPolicy::Ask,
        catch_up_ttl_minutes: 720,
        permission_profile: PermissionProfile::ReadOnly,
        consent: true,
        notify: false,
    };
    let args = resume_args(&r);
    assert!(!args.iter().any(|a| a.contains(&r.prompt)));
    assert!(!args.iter().any(|a| a.contains("dangerously")));
    assert_eq!(args.last().unwrap(), "-");
    r.permission_profile = PermissionProfile::FullAccess;
    let fork = execution_args(&r, true);
    assert!(fork.iter().any(|arg| arg == "fork"));
    assert!(!fork.iter().any(|arg| arg == "resume"));
    assert!(fork.iter().any(|arg| arg == "danger-full-access"));
    assert!(!fork.iter().any(|arg| arg.contains(&r.prompt)));
    let mut legacy = serde_json::to_value(&r).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("locked_session_behavior");
    legacy["conversation"]
        .as_object_mut()
        .unwrap()
        .remove("lock_state");
    let legacy: CreateTaskRequest = serde_json::from_value(legacy).unwrap();
    assert_eq!(legacy.locked_session_behavior, LockedSessionBehavior::Fork);
    assert_eq!(legacy.conversation.lock_state, SessionLockState::Unknown);
    r.permission_profile = PermissionProfile::ReadOnly;
    r.conversation.provider = ProviderId::ClaudeCode;
    let args = resume_args(&r);
    assert!(args.iter().any(|a| a == "dontAsk"));
    assert!(!args.iter().any(|a| a.contains("Bash")));
    assert!(!args.iter().any(|a| a.contains("dangerously")));
}
