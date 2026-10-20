#[cfg(windows)]
mod autostart;
#[cfg(windows)]
mod diagnostics;
pub(crate) mod job;
#[cfg(windows)]
pub(crate) mod packages;
#[cfg(windows)]
mod scheduler;
#[cfg(windows)]
mod security;
use crate::Task;
use anyhow::{bail, Context, Result};
#[cfg(windows)]
use scheduler::ScheduleKind;
use serde_json::{json, Value};
use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

// Windows 宽字符 API 要求以零结尾的 UTF-16；保留路径中的非 Unicode 编码单元。
#[cfg(windows)]
fn wide(value: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    value.encode_wide().chain(Some(0)).collect()
}

pub fn runner_path() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    Ok(exe
        .parent()
        .context("No executable directory")?
        .join("codebeat-runner.exe"))
}
pub fn register(task: &Task, runner: &Path) -> Result<()> {
    uuid::Uuid::parse_str(&task.id)?;
    if !runner.is_file() {
        bail!("RUNNER_MISSING: {}", runner.display());
    }
    #[cfg(windows)]
    scheduler::schedule(
        &task.id,
        runner,
        ScheduleKind::Once,
        Some(&task.request.scheduled_at_utc.to_rfc3339()),
        task.request.wake_to_run,
        task.request.catch_up_policy != crate::CatchUpPolicy::Skip,
    )?;
    #[cfg(not(windows))]
    bail!("UNSUPPORTED_PLATFORM: Windows required");
    Ok(())
}
pub fn unregister(id: &str) -> Result<()> {
    uuid::Uuid::parse_str(id)?;
    #[cfg(windows)]
    scheduler::delete_task(id)?;
    #[cfg(not(windows))]
    bail!("UNSUPPORTED_PLATFORM: Windows required");
    Ok(())
}
pub fn register_maintenance(runner: &Path) -> Result<()> {
    if !runner.is_file() {
        bail!("RUNNER_MISSING");
    }
    #[cfg(windows)]
    scheduler::schedule(
        "maintenance",
        runner,
        ScheduleKind::Maintenance,
        None,
        false,
        true,
    )?;
    #[cfg(not(windows))]
    bail!("UNSUPPORTED_PLATFORM: Windows required");
    Ok(())
}
pub fn configure_pretrigger(runner: &Path, enabled: bool, wake: bool) -> Result<()> {
    if enabled && !runner.is_file() {
        bail!("RUNNER_MISSING");
    }
    #[cfg(windows)]
    {
        if enabled {
            // 每分钟轮询本身不得唤醒机器；唤醒由独立的一次性任务负责。
            scheduler::schedule(
                "pretrigger",
                runner,
                ScheduleKind::Pretrigger,
                None,
                false,
                true,
            )?;
        } else {
            scheduler::delete_task("pretrigger")?;
        }
        if !enabled || !wake {
            scheduler::delete_task("pretrigger-wake")?;
        }
    }
    #[cfg(not(windows))]
    bail!("UNSUPPORTED_PLATFORM: Windows required");
    Ok(())
}
pub fn sync_optimizer_wake(db: &crate::db::Database, runner: &Path) -> Result<()> {
    let s = db.settings()?;
    let next = if s.pretrigger_enabled
        && s.pretrigger.wake
        && s.pretrigger.auto_trigger
        && s.pretrigger.mode != "save"
        && !s.paused
        && !s.paused_providers.contains(&crate::ProviderId::Codex)
    {
        crate::pretrigger::next_wake_at(&s.pretrigger, &db.optimizer_status()?, chrono::Utc::now())?
    } else {
        None
    };
    #[cfg(windows)]
    match next {
        Some(time) => scheduler::schedule(
            "pretrigger-wake",
            runner,
            ScheduleKind::OptimizerWake,
            Some(&time.to_rfc3339()),
            true,
            true,
        )?,
        None => scheduler::delete_task("pretrigger-wake")?,
    }
    #[cfg(not(windows))]
    {
        let _ = (next, runner);
        bail!("UNSUPPORTED_PLATFORM: Windows required");
    }
    Ok(())
}
pub fn notify(title: &str, body: &str) -> Result<()> {
    notify_with_sound(title, body, true)
}
pub fn notify_with_sound(title: &str, body: &str, sound: bool) -> Result<()> {
    notify_card(
        &DesktopNotification {
            title,
            body,
            kind: "info",
            task_id: None,
            page: "tasks",
        },
        sound,
    )
}
#[derive(serde::Serialize)]
pub struct DesktopNotification<'a> {
    pub title: &'a str,
    pub body: &'a str,
    pub kind: &'a str,
    pub task_id: Option<&'a str>,
    pub page: &'a str,
}
pub fn notify_card(card: &DesktopNotification<'_>, sound: bool) -> Result<()> {
    notify_card_with_topic(card, sound, None)
}
pub fn notify_card_with_topic(
    card: &DesktopNotification<'_>,
    sound: bool,
    topic: Option<&str>,
) -> Result<()> {
    if !cfg!(windows) {
        bail!("UNSUPPORTED_PLATFORM: Windows required");
    }
    if let Some(id) = card.task_id {
        uuid::Uuid::parse_str(id)?;
    }
    if !matches!(card.kind, "success" | "error" | "info") || !matches!(card.page, "tasks" | "usage")
    {
        bail!("INVALID_NOTIFICATION");
    }
    let id = uuid::Uuid::new_v4().to_string();
    let database = crate::db::Database::open_default()?;
    database.add_notice(&crate::InboxNotice {
        topic: Some(topic.map(str::to_owned).unwrap_or_else(|| {
            card.task_id.map_or_else(
                || format!("system:{}", card.title),
                |id| format!("task:{id}:execution"),
            )
        })),
        id: id.clone(),
        title: card.title.into(),
        message: card.body.into(),
        kind: card.kind.into(),
        task_id: card.task_id.map(str::to_owned),
        page: card.page.into(),
        at: chrono::Utc::now(),
    })?;
    show_stored_notice(&database, card, sound, &id)
}

/// 消息已写入收件箱，仅负责前台可见性判断和原生弹窗。
pub(crate) fn show_stored_notice(
    database: &crate::db::Database,
    card: &DesktopNotification<'_>,
    sound: bool,
    id: &str,
) -> Result<()> {
    let visible = database.directory().join("notification-acks/main-visible");
    if std::fs::read(&visible).is_ok_and(|bytes| bytes == b"visible")
        && std::fs::metadata(&visible)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|time| time.elapsed().ok())
            .is_some_and(|age| age.as_secs_f64() < 2.)
    {
        return Ok(());
    }
    // 收件箱负责持久化投递；弹窗失败不能触发重试并产生重复消息。
    if let Err(error) = notify_popup(card, sound, id) {
        let log = database.directory().join("logs/notifications.log");
        if let Some(parent) = log.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
        {
            let _ = writeln!(file, "Desktop popup unavailable: {error}");
        }
    }
    Ok(())
}
fn notify_popup(card: &DesktopNotification<'_>, sound: bool, id: &str) -> Result<()> {
    let app = std::env::current_exe()?.with_file_name("codebeat.exe");
    let mut payload = serde_json::to_value(card)?;
    payload["sound"] = json!(sound);
    let ack_dir = crate::db::data_dir()?.join("notification-acks");
    std::fs::create_dir_all(&ack_dir)?;
    payload["ack_path"] = json!(ack_dir.join(format!("{id}.read")));
    payload["dismiss_path"] = json!(ack_dir.join(format!("{id}.dismiss")));

    // 辅助进程只承载通知窗口，不启动主程序的单实例插件和后台任务。
    if !app.is_file() {
        bail!("Desktop application unavailable: {}", app.display());
    }
    let logs = PathBuf::from(std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA missing")?)
        .join("codebeat/logs");
    std::fs::create_dir_all(&logs)?;
    let log = logs.join("notifications.log");
    if std::fs::metadata(&log).is_ok_and(|m| m.len() > 512 * 1024) {
        std::fs::remove_file(&log)?;
    }
    let errors = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)?;
    let mut command = Command::new(app);
    command
        .arg("--notification-helper")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(errors);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn()?;
    child
        .stdin
        .take()
        .context("stdin unavailable")?
        .write_all(&serde_json::to_vec(&payload)?)?;
    let stdout = child.stdout.take().context("stdout unavailable")?;
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        use std::io::BufRead;
        let mut line = String::new();
        let _ = std::io::BufReader::new(stdout).read_line(&mut line);
        let _ = sender.send(line);
    });
    if receiver
        .recv_timeout(Duration::from_secs(8))
        .is_ok_and(|line| line.trim() == "READY")
    {
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    } else {
        let _ = child.kill();
        let _ = child.wait();
        bail!("NOTIFICATION_WINDOW_FAILED: see logs/notifications.log");
    }
    Ok(())
}
pub fn autostart(enabled: bool) -> Result<()> {
    #[cfg(windows)]
    {
        autostart::autostart(enabled)
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        bail!("UNSUPPORTED_PLATFORM: Windows required");
    }
}
pub fn diagnostics() -> Result<Value> {
    #[cfg(windows)]
    {
        diagnostics::diagnostics()
    }
    #[cfg(not(windows))]
    bail!("UNSUPPORTED_PLATFORM: Windows required")
}
pub fn restrict_data_directory(path: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        security::restrict_directory(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        bail!("UNSUPPORTED_PLATFORM: Windows required");
    }
}
/// 供卸载器在删除 runner 前调用，清理系统任务及自启。
/// 此过程不打开数据库，也不执行维护、应用发现或模型请求。
pub fn cleanup_registrations() -> Result<()> {
    #[cfg(windows)]
    {
        scheduler::cleanup_tasks()?;
        autostart::autostart(false)
    }
    #[cfg(not(windows))]
    bail!("UNSUPPORTED_PLATFORM: Windows required")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_id_rejected() {
        assert!(unregister("../x;evil").is_err());
    }
}
