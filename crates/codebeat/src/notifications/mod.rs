use crate::{db::Database, models::*};
use anyhow::Result;

pub(crate) fn notify(db: &Database, key: &str, title: &str, body: &str) -> Result<()> {
    deliver_notification(
        db,
        key,
        &crate::platform::DesktopNotification {
            title,
            body,
            kind: "info",
            task_id: None,
            page: "usage",
        },
    )
}
fn task_notice(task: &crate::Task, started: bool) -> (String, String, &'static str) {
    let provider = match task.request.conversation.provider {
        ProviderId::Codex => "Codex",
        ProviderId::ClaudeCode => "Claude Code",
    };
    if started {
        (
            format!("{provider} 已开始继续任务"),
            format!("任务“{}”正在原会话中继续执行。", task.request.title),
            "info",
        )
    } else if task.state == "succeeded" {
        (
            format!("{provider} 任务已完成"),
            format!("任务“{}”已在原会话中完成执行。", task.request.title),
            "success",
        )
    } else {
        let state = match task.state.as_str() {
            "cancelled" => "已取消",
            "needs_review" | "blocked_approval" => "需要人工确认",
            _ => "执行失败",
        };
        (
            format!("{provider} 任务{state}"),
            format!(
                "任务“{}”{state}，请查看任务与运行历史。",
                task.request.title
            ),
            "error",
        )
    }
}
pub(crate) fn notify_task(
    db: &Database,
    key: &str,
    task: &crate::Task,
    started: bool,
) -> Result<()> {
    let (title, body, kind) = task_notice(task, started);
    deliver_notification(
        db,
        key,
        &crate::platform::DesktopNotification {
            title: &title,
            body: &body,
            kind,
            task_id: Some(&task.id),
            page: "tasks",
        },
    )
}
fn deliver_notification(
    db: &Database,
    key: &str,
    card: &crate::platform::DesktopNotification<'_>,
) -> Result<()> {
    if db.notify_once(key)? {
        let topic = card.task_id.map_or_else(
            || {
                format!(
                    "system:{}",
                    key.split(':').take(2).collect::<Vec<_>>().join(":")
                )
            },
            |id| format!("task:{id}:execution"),
        );
        if let Err(e) = crate::platform::notify_card_with_topic(
            card,
            db.settings()?.notification_sound,
            Some(&topic),
        ) {
            db.undo_notification(key)?;
            eprintln!("NOTIFICATION_FAILED: {e}");
        }
    }
    Ok(())
}
