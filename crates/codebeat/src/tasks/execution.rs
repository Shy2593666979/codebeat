use crate::notifications::{notify, notify_task};
use crate::{
    db::Database,
    models::*,
    platform::job,
    tasks::{local_directory, validate_task},
};
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{io::Write, process::Stdio, time::Duration};
use tokio::io::{AsyncWriteExt, BufReader};

pub async fn maintenance(db: &mut Database) -> Result<()> {
    for task in db.tasks()? {
        if task.state == "scheduled" && task.request.scheduled_at_utc <= chrono::Utc::now() {
            run(db, &task.id, false).await?;
        }
    }
    reminders(db).await?;
    cleanup_logs(db)
}

pub async fn run(db: &mut Database, id: &str, manual: bool) -> Result<()> {
    let Some(record) = db.claim(id, manual)? else {
        register_next_occurrence(db, id)?;
        return Ok(());
    };
    let result = execute(db, &record).await;
    if let Err(e) = result {
        let msg = crate::providers::redact(&e.to_string());
        db.finish(
            &record.id,
            "failed",
            None,
            Some(msg.split(':').next().unwrap_or("RUN_FAILED")),
            Some(&msg),
        )?;
    }
    let task = db.task(id)?;
    let settings = db.settings()?;
    let terminal_state = task.state.clone();
    if settings.pretrigger_enabled
        && settings
            .pretrigger
            .providers
            .contains(&task.request.conversation.provider)
    {
        let report = crate::providers::usage(task.request.conversation.provider).await;
        db.cache_usage(&report)?;
    }
    if settings.notifications
        && task.request.notify
        && (if terminal_state == "succeeded" {
            settings.notify_success
        } else {
            settings.notify_failure
        })
    {
        notify_task(db, &format!("run:{}", record.id), &task, false)?;
    }
    if terminal_state == "succeeded" {
        register_next_occurrence(db, id)?;
    }
    Ok(())
}
fn register_next_occurrence(db: &Database, id: &str) -> Result<()> {
    if let Some(next) = db.prepare_next_occurrence(id)? {
        match crate::platform::runner_path()
            .and_then(|path| crate::platform::register(&next, &path))
        {
            Ok(()) => db.transition(id, &["pending_registration"], "scheduled", None)?,
            Err(e) => db.transition(
                id,
                &["pending_registration"],
                "registration_failed",
                Some(&e.to_string()),
            )?,
        }
    }
    Ok(())
}
async fn execute(db: &mut Database, record: &RunRecord) -> Result<()> {
    let task = db.task(&record.task_id)?;
    let settings = db.settings()?;
    validate_task(&task.request, &settings, chrono::Utc::now(), false)?;
    crate::providers::verify(&task.request.conversation).await?;
    let exe = crate::providers::executable(task.request.conversation.provider)?;
    // 使用执行当时的系统锁状态，不能信任创建任务时保存的快照。
    let lock = crate::providers::session_lock::state(
        task.request.conversation.provider,
        &task.request.conversation.session_id,
    );
    if lock == SessionLockState::Unknown
        || (lock == SessionLockState::InUse
            && task.request.locked_session_behavior == LockedSessionBehavior::Pause)
    {
        let message = if lock == SessionLockState::Unknown {
            "无法确认会话占用状态，任务已暂停，请检查后手动恢复"
        } else {
            "会话正在被其他进程使用，任务已暂停，请在会话空闲后手动恢复"
        };
        db.finish(
            &record.id,
            "paused",
            None,
            Some(if lock == SessionLockState::Unknown {
                "SESSION_LOCK_UNKNOWN"
            } else {
                "SESSION_IN_USE"
            }),
            Some(message),
        )?;
        return Ok(());
    }
    let fork = lock == SessionLockState::InUse;
    let args = crate::providers::execution_args(&task.request, fork);
    if settings.notifications && settings.notify_start && task.request.notify {
        notify_task(db, &format!("run-start:{}", record.id), &task, true)?;
    }
    let logdir = db.directory().join("logs");
    std::fs::create_dir_all(&logdir)?;
    let logpath = logdir.join(format!("{}.jsonl", record.id));
    let mut log = std::fs::File::create(&logpath)?;
    writeln!(
        log,
        "{}",
        json!({"event":"start","at":chrono::Utc::now(),"run_id":record.id,"session_mode":if fork {"fork"} else {"resume"},"source_session_id":task.request.conversation.session_id})
    )?;
    let mut child = crate::providers::command(&exe)
        .args(&args)
        .current_dir(local_directory(&task.request.conversation.cwd)?)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("CLI_START_FAILED")?;
    let _job = job::ProcessJob::assign(child.id().context("PID_MISSING")?)
        .context("PROCESS_TREE_GUARD_FAILED")?;
    db.start_run(
        &record.id,
        &task.id,
        &logpath.to_string_lossy(),
        child.id().context("PID_MISSING")?,
    )?;
    let mut stdin = child.stdin.take().context("STDIN_MISSING")?;
    stdin.write_all(task.request.prompt.as_bytes()).await?;
    stdin.shutdown().await?;
    drop(stdin);
    let stdout = child.stdout.take().context("STDOUT_MISSING")?;
    let stderr = child.stderr.take().context("STDERR_MISSING")?;
    let (tx, mut rx) = tokio::sync::mpsc::channel(128);
    let tx2 = tx.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout);
        while let Ok(Some(l)) = crate::providers::stream::next_line(&mut lines, 1024 * 1024).await {
            if tx.send((false, l)).await.is_err() {
                break;
            }
        }
    });
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr);
        while let Ok(Some(l)) = crate::providers::stream::next_line(&mut lines, 1024 * 1024).await {
            if tx2.send((true, l)).await.is_err() {
                break;
            }
        }
    });
    let deadline = tokio::time::sleep(Duration::from_secs(30 * 60));
    tokio::pin!(deadline);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(10));
    let mut terminal = None;
    let mut cancelled = false;
    let mut timed_out = false;
    let mut bytes = 0usize;
    loop {
        tokio::select! {
            line=rx.recv()=>{let Some((stderr,line))=line else{break;};
                if line.len()>1024*1024{terminal=Some("failed");continue;}
                if !stderr {if let Ok(v)=serde_json::from_str::<Value>(&line){if let Some(s)=crate::providers::classify_event(&v){if terminal!=Some("blocked_approval") && terminal!=Some("failed"){terminal=Some(s);}}}}
                let event=if settings.save_output {json!({"event":if stderr{"stderr"}else{"model_output"},"at":chrono::Utc::now(),"data":crate::providers::redact(&line)})}else{json!({"event":if stderr{"stderr_redacted"}else{"progress"},"at":chrono::Utc::now(),"bytes":line.len()})};
                let encoded=serde_json::to_string(&event)?;
                if bytes+encoded.len()>10*1024*1024 {drop(log);for n in (1..5).rev(){let from=logpath.with_extension(format!("jsonl.{n}"));let to=logpath.with_extension(format!("jsonl.{}",n+1));if from.exists(){let _=std::fs::rename(from,to);}}let _=std::fs::rename(&logpath,logpath.with_extension("jsonl.1"));log=std::fs::File::create(&logpath)?;bytes=0;}
                writeln!(log,"{encoded}")?;bytes+=encoded.len()+1;
            },
            _=heartbeat.tick()=>{if db.heartbeat(&record.id)?{cancelled=true;stop_tree(&mut child).await;break;}},
            _=&mut deadline=>{timed_out=true;stop_tree(&mut child).await;break;},
        }
    }
    let status = tokio::time::timeout(Duration::from_secs(10), child.wait())
        .await
        .context("CLI_EXIT_TIMEOUT")??;
    let state = if cancelled {
        "cancelled"
    } else if timed_out {
        "needs_review"
    } else if !status.success() {
        "failed"
    } else {
        terminal.unwrap_or("completed_unverified")
    };
    writeln!(
        log,
        "{}",
        json!({"event":"end","at":chrono::Utc::now(),"state":state,"exit_code":status.code()})
    )?;
    log.sync_all()?;
    db.finish(
        &record.id,
        state,
        status.code(),
        if timed_out {
            Some("EXECUTION_TIMEOUT")
        } else {
            None
        },
        None,
    )?;
    Ok(())
}
pub(crate) async fn stop_tree(child: &mut tokio::process::Child) {
    #[cfg(windows)]
    if let Some(pid) = child.id() {
        let mut c = crate::providers::command(std::path::Path::new("taskkill.exe"));
        let _ = c.args(["/PID", &pid.to_string(), "/T"]).output().await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        let _ = c.arg("/F").output().await;
    }
    let _ = child.kill().await;
}
async fn reminders(db: &Database) -> Result<()> {
    let settings = db.settings()?;
    if !settings.notifications {
        return Ok(());
    }
    let fresh = crate::providers::usage(ProviderId::Codex).await;
    if fresh.status != "available" {
        return Ok(());
    }
    for w in &fresh.windows {
        if let Some(reset) = w.resets_at_utc {
            let mins = reset
                .signed_duration_since(chrono::Utc::now())
                .num_minutes();
            if mins > 0 && mins <= settings.reminder_minutes {
                notify(
                    db,
                    &format!("reset-before:{}:{reset}", w.label),
                    "额度窗口提醒",
                    &format!("{} 预计于 {} 重置；届时需再次查询确认", w.label, reset),
                )?;
            }
        }
    }
    // 恢复通知统一由 cache_usage 比较官方快照生成，避免重复或漏掉提前重置。
    db.cache_usage(&fresh)?;
    Ok(())
}
fn cleanup_logs(db: &Database) -> Result<()> {
    let dir = db.directory().join("logs");
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.metadata()?.modified()?.elapsed().unwrap_or_default()
                > Duration::from_secs(14 * 86400)
        {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}
