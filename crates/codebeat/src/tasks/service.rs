use crate::{db::Database, models::*, tasks::validate_task};
use anyhow::{bail, Result};
use std::path::PathBuf;

pub async fn create_task(database_path: PathBuf, mut request: CreateTaskRequest) -> Result<Task> {
    // 同步 ORM 连接不跨越网络 await；只在实际读写阶段打开。
    let settings = Database::open(&database_path)?.settings()?;
    validate_task(&request, &settings, chrono::Utc::now(), true)?;
    crate::providers::verify(&request.conversation).await?;
    request.conversation.lock_state = crate::providers::session_lock::require_known(
        request.conversation.provider,
        &request.conversation.session_id,
    )?;
    let db = Database::open(&database_path)?;
    let task = register_task(&db, request)?;
    Ok(task)
}

fn register_task(db: &Database, request: CreateTaskRequest) -> Result<Task> {
    let task = db.insert_task(request)?;
    complete_registration(db, &task)?;
    db.task(&task.id)
}

fn complete_registration(db: &Database, task: &Task) -> Result<()> {
    match crate::platform::runner_path().and_then(|p| crate::platform::register(task, &p)) {
        Ok(()) => db.transition(&task.id, &["pending_registration"], "scheduled", None),
        Err(e) => {
            db.transition(
                &task.id,
                &["pending_registration"],
                "registration_failed",
                Some(&e.to_string()),
            )?;
            Err(e)
        }
    }
}

pub async fn update_task(
    database_path: PathBuf,
    id: String,
    mut request: CreateTaskRequest,
) -> Result<Task> {
    let settings = Database::open(&database_path)?.settings()?;
    validate_task(&request, &settings, chrono::Utc::now(), true)?;
    crate::providers::verify(&request.conversation).await?;
    request.conversation.lock_state = crate::providers::session_lock::require_known(
        request.conversation.provider,
        &request.conversation.session_id,
    )?;
    let db = Database::open(&database_path)?;
    db.transition(
        &id,
        &[
            "scheduled",
            "paused",
            "registration_failed",
            "needs_review",
            "failed",
            "expired",
            "blocked_approval",
            "completed_unverified",
        ],
        "pending_registration",
        None,
    )?;
    if let Err(e) = crate::platform::unregister(&id) {
        db.transition(
            &id,
            &["pending_registration"],
            "registration_failed",
            Some(&e.to_string()),
        )?;
        return Err(e);
    }
    db.replace_request(&id, &request)?;
    let task = db.task(&id)?;
    complete_registration(&db, &task)?;
    let task = db.task(&id)?;
    Ok(task)
}

pub fn cancel_task(db: &Database, id: String) -> Result<Task> {
    let task = db.task(&id)?;
    if ["claimed", "running", "cancelling"].contains(&task.state.as_str()) {
        db.transition(
            &id,
            &["claimed", "running", "cancelling"],
            "cancelling",
            Some("正在停止执行器子进程"),
        )?;
    } else {
        crate::platform::unregister(&id)?;
        db.transition(
            &id,
            &[
                "scheduled",
                "paused",
                "registration_failed",
                "needs_review",
                "failed",
                "expired",
                "blocked_approval",
                "completed_unverified",
                "succeeded",
            ],
            "cancelled",
            None,
        )?;
    }
    db.task(&id)
}

pub fn pause_task(db: &Database, id: String, paused: bool) -> Result<Task> {
    if paused {
        db.transition(&id, &["scheduled"], "paused", None)?;
    } else {
        let t = db.task(&id)?;
        if t.request.scheduled_at_utc <= chrono::Utc::now() {
            bail!("TIME_IN_PAST: 请编辑任务并选择未来时间");
        }
        db.transition(&id, &["paused"], "scheduled", None)?;
    }
    db.task(&id)
}

pub fn run_task_now(database: &Database, id: String) -> Result<()> {
    uuid::Uuid::parse_str(&id)?;
    let t = database.task(&id)?;
    let settings = database.settings()?;
    if settings.paused
        || settings
            .paused_providers
            .contains(&t.request.conversation.provider)
    {
        bail!("SCHEDULING_PAUSED");
    }
    if t.state != "scheduled" {
        bail!("INVALID_STATE: 仅可立即执行已排程任务");
    }
    let path = crate::platform::runner_path()?;
    crate::providers::command(&path)
        .args(["run", "--task-id", &id, "--now"])
        .spawn()?;
    Ok(())
}
