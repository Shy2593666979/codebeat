use chrono::{Duration, Utc};
use codebeat_lib::db::Database;
use codebeat_lib::*;
fn setup() -> (tempfile::TempDir, Database, Task) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("bin")).unwrap();
    std::fs::copy(
        env!("CARGO_BIN_EXE_codebeat-runner-fixture"),
        dir.path().join("bin/codex.exe"),
    )
    .unwrap();
    let db = Database::open(&dir.path().join("codebeat.db")).unwrap();
    db.save_settings(&Settings {
        notifications: false,
        authorized_roots: vec![],
        ..Default::default()
    })
    .unwrap();
    let utc = Utc::now() + Duration::minutes(5);
    let request = CreateTaskRequest {
        locked_session_behavior: Default::default(),
        recurrence: None,
        conversation: Conversation {
            lock_state: Default::default(),
            provider: ProviderId::Codex,
            session_id: "d27dfaa2-8399-401d-9428-e2cd4fe18190".into(),
            title: "fixture".into(),
            cwd: dir.path().to_owned(),
            last_updated_utc: None,
            resumable: true,
            source: "test".into(),
        },
        title: "isolated runner test".into(),
        prompt: "中文\n\" ; $() & do-not-execute".into(),
        scheduled_at_utc: utc,
        scheduled_local: utc.format("%Y-%m-%dT%H:%M").to_string(),
        timezone_id: "UTC".into(),
        wake_to_run: false,
        catch_up_policy: CatchUpPolicy::Skip,
        catch_up_ttl_minutes: 720,
        permission_profile: PermissionProfile::ReadOnly,
        consent: false,
        notify: false,
    };
    let task = db.insert_task(request).unwrap();
    db.transition(&task.id, &["pending_registration"], "scheduled", None)
        .unwrap();
    (dir, db, task)
}
fn start(dir: &std::path::Path, task: &Task, outcome: &str) -> std::process::Child {
    let separator = if cfg!(windows) { ";" } else { ":" };
    let path = format!(
        "{}{separator}{}",
        dir.join("bin").display(),
        std::env::var("PATH").unwrap_or_default()
    );
    std::process::Command::new(env!("CARGO_BIN_EXE_codebeat-runner"))
        .arg("--database")
        .arg(dir.join("codebeat.db"))
        .args(["run", "--task-id", &task.id, "--now"])
        .env("PATH", path)
        .env("CODEBEAT_FIXTURE_CWD", dir)
        .env("CODEX_HOME", dir.join("codex-home"))
        .env(
            "CODEBEAT_FIXTURE_ARGS_PATH",
            dir.join("execution-args.json"),
        )
        .env("CODEBEAT_FIXTURE_EXPECT_PROMPT", &task.request.prompt)
        .env("CODEBEAT_FIXTURE_OUTCOME", outcome)
        .spawn()
        .unwrap()
}
#[test]
fn independent_runner_requires_success_event_and_preserves_prompt() {
    for (outcome, expected) in [
        ("succeeded", "succeeded"),
        ("unverified", "completed_unverified"),
        ("failed", "failed"),
        ("blocked_approval", "blocked_approval"),
    ] {
        let (dir, db, task) = setup();
        assert!(start(dir.path(), &task, outcome).wait().unwrap().success());
        assert_eq!(
            db.task(&task.id).unwrap().state,
            expected,
            "runs: {:?}",
            db.runs().unwrap()
        );
        let run = &db.runs().unwrap()[0];
        let log = std::fs::read_to_string(run.log_path.as_ref().unwrap()).unwrap();
        assert!(log.contains("\"event\":\"start\""));
        assert!(log.contains("\"event\":\"end\""));
        assert!(!log.contains(&task.request.prompt));
        assert!(std::path::Path::new(run.log_path.as_ref().unwrap()).starts_with(dir.path()));
    }
}

#[cfg(windows)]
#[test]
fn occupied_session_forks_or_pauses_without_touching_the_source() {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::{Storage::FileSystem::*, System::IO::OVERLAPPED};
    for behavior in [LockedSessionBehavior::Fork, LockedSessionBehavior::Pause] {
        let (dir, db, task) = setup();
        let mut request = task.request.clone();
        request.locked_session_behavior = behavior;
        request.permission_profile = PermissionProfile::FullAccess;
        db.transition(&task.id, &["scheduled"], "pending_registration", None)
            .unwrap();
        db.replace_request(&task.id, &request).unwrap();
        db.transition(&task.id, &["pending_registration"], "scheduled", None)
            .unwrap();
        let locks = dir.path().join("codex-home/thread-writer-locks");
        std::fs::create_dir_all(&locks).unwrap();
        let path = locks.join(format!("{}.lock", request.conversation.session_id));
        let held = std::fs::File::create(&path).unwrap();
        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
        assert_ne!(
            unsafe {
                LockFileEx(
                    held.as_raw_handle(),
                    LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
                    0,
                    u32::MAX,
                    u32::MAX,
                    &mut overlapped,
                )
            },
            0
        );
        assert!(start(dir.path(), &task, "succeeded")
            .wait()
            .unwrap()
            .success());
        let updated = db.task(&task.id).unwrap();
        let args_path = dir.path().join("execution-args.json");
        if behavior == LockedSessionBehavior::Pause {
            assert_eq!(updated.state, "paused");
            assert_eq!(
                db.runs().unwrap()[0].error_code.as_deref(),
                Some("SESSION_IN_USE")
            );
            assert!(!args_path.exists(), "暂停策略不能启动执行 CLI");
        } else {
            assert_eq!(updated.state, "succeeded");
            let args: Vec<String> =
                serde_json::from_slice(&std::fs::read(args_path).unwrap()).unwrap();
            assert!(args.contains(&"fork".into()));
            assert!(!args.contains(&"resume".into()));
            assert!(args.contains(&"danger-full-access".into()));
            assert_eq!(
                updated.request.conversation.session_id,
                task.request.conversation.session_id
            );
        }
        // 检测和执行不能释放其他进程持有的原始会话锁。
        assert_ne!(
            unsafe { UnlockFileEx(held.as_raw_handle(), 0, u32::MAX, u32::MAX, &mut overlapped) },
            0
        );
    }
}

#[cfg(windows)]
#[test]
fn unknown_session_lock_pauses_before_sending_any_prompt() {
    let (dir, db, task) = setup();
    std::fs::create_dir_all(
        dir.path()
            .join("codex-home/thread-writer-locks")
            .join(format!("{}.lock", task.request.conversation.session_id)),
    )
    .unwrap();
    assert!(start(dir.path(), &task, "succeeded")
        .wait()
        .unwrap()
        .success());
    assert_eq!(db.task(&task.id).unwrap().state, "paused");
    assert_eq!(
        db.runs().unwrap()[0].error_code.as_deref(),
        Some("SESSION_LOCK_UNKNOWN")
    );
    assert!(!dir.path().join("execution-args.json").exists());
}
#[test]
fn duplicate_process_triggers_never_repeat_provider() {
    let (dir, db, task) = setup();
    let mut a = start(dir.path(), &task, "succeeded");
    let mut b = start(dir.path(), &task, "succeeded");
    assert!(a.wait().unwrap().success());
    assert!(b.wait().unwrap().success());
    assert_eq!(db.runs().unwrap().len(), 1);
    assert_eq!(
        db.task(&task.id).unwrap().state,
        "succeeded",
        "runs: {:?}",
        db.runs().unwrap()
    );
}
fn probe_setup() -> (tempfile::TempDir, Database) {
    use chrono::Timelike;
    let (dir, db, task) = setup();
    db.transition(&task.id, &["scheduled"], "cancelled", None)
        .unwrap();
    let now = Utc::now();
    let mut s = db.settings().unwrap();
    s.pretrigger_enabled = true;
    s.pretrigger.auto_trigger = true;
    s.pretrigger.days = "daily".into();
    s.pretrigger.mode = "save".into();
    s.pretrigger.work_start = format!("{:02}:{:02}", now.hour(), now.minute());
    s.pretrigger.work_end = (now + Duration::hours(9)).format("%H:%M").to_string();
    s.pretrigger.timezone = "UTC".into();
    // balanced 模式下的 9 小时班次，推荐第一个边界在开始前 3 小时。
    s.pretrigger.mode = "balanced".into();
    s.pretrigger.work_start = (now + Duration::hours(3)).format("%H:%M").to_string();
    s.pretrigger.work_end = (now + Duration::hours(12)).format("%H:%M").to_string();
    db.save_settings(&s).unwrap();
    (dir, db)
}
fn probe_command(dir: &std::path::Path) -> std::process::Command {
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_codebeat-runner"));
    cmd.arg("--database")
        .arg(dir.join("codebeat.db"))
        .arg("pretrigger")
        .env(
            "PATH",
            format!(
                "{};{}",
                dir.join("bin").display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        )
        .env("CODEBEAT_FIXTURE_PROBE_MARKER", dir.join("sent-marker"));
    cmd
}
#[test]
fn pretrigger_requires_success_and_never_repeats_even_after_plan_changes() {
    for (outcome, expected) in [
        ("succeeded", "confirmed"),
        ("unverified", "failed"),
        ("failed", "failed"),
    ] {
        let (dir, db) = probe_setup();
        for _ in 0..2 {
            let o = probe_command(dir.path())
                .env("CODEBEAT_FIXTURE_OUTCOME", outcome)
                .output()
                .unwrap();
            assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        }
        let records = db.pretrigger_runs().unwrap();
        assert_eq!(records.len(), 1, "{records:?}");
        assert_eq!(records[0].state, expected, "{records:?}");
        assert!(records[0].before.is_some());
        assert!(records[0].after.is_some());
        assert!(records[0].request_sent);
        assert_eq!(std::fs::read(dir.path().join("sent-marker")).unwrap(), b"x");
        if outcome == "succeeded" {
            assert_eq!(records[0].tokens.as_ref().unwrap()["output_tokens"], 5);
        }
    }
}
#[test]
fn pretrigger_reuses_active_official_window_without_sending() {
    let (dir, db) = probe_setup();
    let o = probe_command(dir.path())
        .env("CODEBEAT_FIXTURE_ACTIVE", "1")
        .output()
        .unwrap();
    assert!(o.status.success());
    assert_eq!(db.optimizer_status().unwrap()[0].status, "verifying");
    assert!(!dir.path().join("sent-marker").exists());
}
#[test]
fn excessive_cost_pauses_future_automatic_requests() {
    let (dir, db) = probe_setup();
    let o = probe_command(dir.path())
        .env("CODEBEAT_FIXTURE_HIGH_COST", "1")
        .output()
        .unwrap();
    assert!(o.status.success());
    assert_eq!(db.pretrigger_runs().unwrap()[0].state, "cost_paused");
    assert!(!db.settings().unwrap().pretrigger.auto_trigger);
}
#[test]
fn unknown_claude_quota_never_sends_a_request() {
    let (dir, db) = probe_setup();
    std::fs::copy(
        env!("CARGO_BIN_EXE_codebeat-runner-fixture"),
        dir.path().join("bin/claude.exe"),
    )
    .unwrap();
    let mut s = db.settings().unwrap();
    s.pretrigger.providers = vec![ProviderId::ClaudeCode];
    db.save_settings(&s).unwrap();
    let o = probe_command(dir.path()).output().unwrap();
    assert!(o.status.success());
    assert!(!dir.path().join("sent-marker").exists());
    assert!(db
        .pretrigger_runs()
        .unwrap()
        .iter()
        .all(|r| !r.request_sent));
}

#[test]
fn idle_floating_reset_can_reach_send_and_rounded_zero_does_not_repeat() {
    for zero_after in [false, true] {
        let (dir, db) = probe_setup();
        for _ in 0..2 {
            let mut command = probe_command(dir.path());
            command.env("CODEBEAT_FIXTURE_FLOATING", "1");
            if zero_after {
                command.env("CODEBEAT_FIXTURE_ZERO_AFTER", "1");
            }
            let output = command.output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        assert!(
            dir.path().join("sent-marker").exists(),
            "records={:?}; plan={:?}; log={}",
            db.pretrigger_runs(),
            db.runtime_pretrigger(ProviderId::Codex),
            std::fs::read_to_string(dir.path().join("logs/pretrigger-decisions.jsonl"))
                .unwrap_or_default()
        );
        assert_eq!(std::fs::read(dir.path().join("sent-marker")).unwrap(), b"x");
        let records = db.pretrigger_runs().unwrap();
        assert_eq!(records.len(), 1);
        assert!(records[0].request_sent);
        assert_eq!(
            records[0].state,
            if zero_after {
                "sent_unconfirmed"
            } else {
                "confirmed"
            }
        );
        assert!(db.settings().unwrap().pretrigger.auto_trigger);
        let windows = db.optimizer_confirmed_windows(Utc::now()).unwrap();
        assert_eq!(windows.len(), 1);
        assert!(windows[0].source.starts_with("app"));
        let log =
            std::fs::read_to_string(dir.path().join("logs/pretrigger-decisions.jsonl")).unwrap();
        assert!(log.contains("tick") && log.contains("executable"));
    }
}
