use crate::emit;
use anyhow::{bail, Context, Result};
use codebeat_lib::db::Database;
use codebeat_lib::*;
use serde_json::{json, Value};

pub(crate) type IpcResult<T> = std::result::Result<T, String>;
pub(crate) fn error(e: anyhow::Error) -> String {
    codebeat_lib::providers::redact(&e.to_string())
}
pub(crate) fn db() -> Result<Database> {
    Database::open_default()
}
pub(crate) fn notification_target(args: &[String]) -> Option<String> {
    for pair in args.windows(2) {
        if pair[0] == "--open-task" {
            if let Ok(id) = uuid::Uuid::parse_str(&pair[1]) {
                return Some(format!("/tasks?task={id}"));
            }
        }
        if pair[0] == "--open-page" && matches!(pair[1].as_str(), "tasks" | "usage") {
            return Some(format!("/{}", pair[1]));
        }
    }
    None
}
#[tauri::command]
pub(crate) fn get_launch_target() -> Option<String> {
    notification_target(&std::env::args().collect::<Vec<_>>())
}
#[cfg(test)]
mod notification_tests {
    use super::notification_target;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }
    #[test]
    fn accepts_only_known_pages_and_uuid_task_links() {
        assert_eq!(
            notification_target(&args(&["app", "--open-page", "usage"])),
            Some("/usage".into())
        );
        let id = "b27dfaa2-8399-401d-9428-000000000003";
        assert_eq!(
            notification_target(&args(&["app", "--open-task", id])),
            Some(format!("/tasks?task={id}"))
        );
        assert_eq!(
            notification_target(&args(&["app", "--open-task", "x&run=1"])),
            None
        );
        assert_eq!(
            notification_target(&args(&["app", "--open-page", "https://example.com"])),
            None
        );
        assert_eq!(notification_target(&args(&["app", "--open-task"])), None);
    }
}

#[tauri::command]
pub(crate) async fn list_providers() -> IpcResult<Vec<ProviderHealth>> {
    let (a, b) = tokio::join!(
        codebeat_lib::providers::detect(ProviderId::Codex),
        codebeat_lib::providers::detect(ProviderId::ClaudeCode)
    );
    let providers = vec![a, b];
    if let Ok(db) = db() {
        for p in &providers {
            let _ = db.cache_health(p);
        }
    }
    Ok(providers)
}
#[tauri::command]
pub(crate) async fn detect_provider(id: ProviderId) -> ProviderHealth {
    codebeat_lib::providers::detect(id).await
}
#[tauri::command]
pub(crate) async fn refresh_usage(id: ProviderId, app: tauri::AppHandle) -> UsageReport {
    crate::usage_refresh::refresh(&app, id, false).await
}

#[tauri::command]
pub(crate) async fn consume_reset_credit(
    idempotency_key: String,
    account_key: String,
    app: tauri::AppHandle,
) -> IpcResult<ResetOutcome> {
    crate::usage_refresh::consume_reset(&app, &idempotency_key, &account_key)
        .await
        .map_err(error)
}
#[tauri::command]
pub(crate) async fn get_session_lock_states(
    conversations: Vec<codebeat_lib::providers::session_lock::SessionIdentity>,
) -> IpcResult<std::collections::BTreeMap<String, SessionLockState>> {
    if conversations.len() > 1000 {
        return Err("TOO_MANY_SESSIONS".into());
    }
    tokio::task::spawn_blocking(move || {
        conversations
            .into_iter()
            .map(|c| {
                (
                    format!("{}:{}", c.provider.as_str(), c.session_id),
                    codebeat_lib::providers::session_lock::state(c.provider, &c.session_id),
                )
            })
            .collect()
    })
    .await
    .map_err(|e| e.to_string())
}
#[tauri::command]
pub(crate) async fn list_conversations(
    provider: ProviderId,
    search: String,
    cursor: Option<String>,
    limit: usize,
) -> IpcResult<SessionPage> {
    codebeat_lib::providers::sessions(provider, &search, cursor, limit)
        .await
        .map_err(error)
}
#[tauri::command]
pub(crate) fn get_monitoring(days: i64) -> IpcResult<Value> {
    db().and_then(|d| d.monitoring(days)).map_err(error)
}
#[tauri::command]
pub(crate) async fn get_token_usage(
    days: i64,
    timezone: String,
) -> IpcResult<codebeat_lib::usage::TokenReport> {
    codebeat_lib::usage::local_usage(days, timezone)
        .await
        .map_err(error)
}
#[tauri::command]
pub(crate) fn get_default_settings() -> Settings {
    Settings::default()
}
#[tauri::command]
pub(crate) fn open_logs_folder() -> IpcResult<()> {
    (|| {
        let path = codebeat_lib::db::data_dir()?.join("logs");
        std::fs::create_dir_all(&path)?;
        codebeat_lib::providers::command(std::path::Path::new("explorer.exe"))
            .arg(path)
            .spawn()?;
        Ok(())
    })()
    .map_err(error)
}
#[tauri::command]
pub(crate) fn open_provider(provider: ProviderId, cwd: String) -> IpcResult<()> {
    (|| {
        let cwd = local_directory(std::path::Path::new(&cwd))?;
        let mut command =
            std::process::Command::new(codebeat_lib::providers::executable(provider)?);
        command.current_dir(cwd);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x00000010);
        }
        command.spawn()?;
        Ok(())
    })()
    .map_err(error)
}
#[tauri::command]
pub(crate) async fn get_conversation_detail(conversation: Conversation) -> IpcResult<Value> {
    codebeat_lib::providers::conversation_detail(&conversation)
        .await
        .map_err(error)
}
#[tauri::command]
pub(crate) fn list_tasks() -> IpcResult<Vec<Task>> {
    db().and_then(|d| d.tasks()).map_err(error)
}
#[tauri::command]
pub(crate) fn list_run_history() -> IpcResult<Vec<RunRecord>> {
    db().and_then(|d| d.runs()).map_err(error)
}
#[tauri::command]
pub(crate) fn get_settings() -> IpcResult<Settings> {
    db().and_then(|d| d.settings()).map_err(error)
}
#[tauri::command]
pub(crate) fn preview_pretrigger(
    settings: pretrigger::PretriggerSettings,
) -> IpcResult<pretrigger::Plan> {
    pretrigger::plan(&settings).map_err(error)
}
#[tauri::command]
pub(crate) fn get_pretrigger_runs() -> IpcResult<Vec<pretrigger::ProbeRecord>> {
    db().and_then(|d| d.pretrigger_runs()).map_err(error)
}
#[tauri::command]
pub(crate) fn get_optimizer_status() -> IpcResult<Value> {
    (|| {
        let d = db()?;
        let now = chrono::Utc::now();
        Ok(serde_json::json!({"plans":d.optimizer_display_status(now)?,"windows":d.optimizer_confirmed_windows(now)?,"habits":d.optimizer_habit()?}))
    })()
    .map_err(error)
}
#[tauri::command]
pub(crate) async fn get_codex_compatibility() -> Value {
    let chatgpt_installed = codebeat_lib::providers::discovery::chatgpt_installed().await;
    let health = codebeat_lib::providers::detect(ProviderId::Codex).await;
    serde_json::json!({"chatgpt_installed":chatgpt_installed,"engine_available":health.executable_path.is_some(),"authenticated":health.auth_status=="authenticated","message":health.message,"execution_verified":false})
}
#[tauri::command]
pub(crate) fn update_settings(settings: Settings) -> IpcResult<Settings> {
    (move || {
        validate_settings(&settings)?;
        let db = db()?;
        let old = db.settings()?;
        if settings.autostart != old.autostart {
            codebeat_lib::platform::autostart(settings.autostart)?;
        }
        let sync_probe = settings.pretrigger_enabled != old.pretrigger_enabled
            || settings.pretrigger.wake != old.pretrigger.wake;
        if sync_probe {
            if let Err(e) = codebeat_lib::platform::runner_path().and_then(|p| {
                codebeat_lib::platform::configure_pretrigger(
                    &p,
                    settings.pretrigger_enabled,
                    settings.pretrigger.wake,
                )
            }) {
                if settings.autostart != old.autostart {
                    let _ = codebeat_lib::platform::autostart(old.autostart);
                }
                return Err(e);
            }
        }
        if let Err(e) = db.save_settings(&settings) {
            if sync_probe {
                let _ = codebeat_lib::platform::runner_path().and_then(|p| {
                    codebeat_lib::platform::configure_pretrigger(
                        &p,
                        old.pretrigger_enabled,
                        old.pretrigger.wake,
                    )
                });
            }
            if settings.autostart != old.autostart {
                let _ = codebeat_lib::platform::autostart(old.autostart);
            }
            return Err(e);
        }
        if serde_json::to_value(&settings.pretrigger)? != serde_json::to_value(&old.pretrigger)?
            || sync_probe
        {
            if let Err(e) = codebeat_lib::platform::runner_path()
                .and_then(|p| codebeat_lib::platform::sync_optimizer_wake(&db, &p))
            {
                eprintln!("AWO_WAKE: {e}");
            }
        }
        Ok(settings)
    })()
    .map_err(error)
}
#[tauri::command]
pub(crate) async fn create_task(
    request: CreateTaskRequest,
    app: tauri::AppHandle,
) -> IpcResult<Task> {
    let path = codebeat_lib::db::data_dir()
        .map_err(error)?
        .join("codebeat.db");
    let task = codebeat_lib::tasks::service::create_task(path, request)
        .await
        .map_err(error)?;
    emit(&app, &task);
    Ok(task)
}

#[tauri::command]
pub(crate) async fn update_task(
    id: String,
    request: CreateTaskRequest,
    app: tauri::AppHandle,
) -> IpcResult<Task> {
    let path = codebeat_lib::db::data_dir()
        .map_err(error)?
        .join("codebeat.db");
    let task = codebeat_lib::tasks::service::update_task(path, id, request)
        .await
        .map_err(error)?;
    emit(&app, &task);
    Ok(task)
}
#[tauri::command]
pub(crate) fn cancel_task(id: String, app: tauri::AppHandle) -> IpcResult<()> {
    let task =
        codebeat_lib::tasks::service::cancel_task(&db().map_err(error)?, id).map_err(error)?;
    emit(&app, &task);
    Ok(())
}
#[tauri::command]
pub(crate) fn pause_task(id: String, paused: bool) -> IpcResult<Task> {
    codebeat_lib::tasks::service::pause_task(&db().map_err(error)?, id, paused).map_err(error)
}
#[tauri::command]
pub(crate) fn run_task_now(id: String) -> IpcResult<()> {
    codebeat_lib::tasks::service::run_task_now(&db().map_err(error)?, id).map_err(error)
}
#[tauri::command]
pub(crate) fn get_run_log(run_id: String, offset: usize, limit: usize) -> IpcResult<Value> {
    (||{use std::io::{Read,Seek,SeekFrom};
    let run=db()?.runs()?.into_iter().find(|r|r.id==run_id).context("RUN_NOT_FOUND")?;
    let path=std::path::PathBuf::from(run.log_path.context("LOG_UNAVAILABLE")?);
    let root=codebeat_lib::db::data_dir()?.join("logs").canonicalize()?;
    if !path.canonicalize()?.starts_with(&root){bail!("LOG_PATH_DENIED");}
    let mut f=std::fs::File::open(path)?;let size=f.metadata()?.len();f.seek(SeekFrom::Start(offset as u64))?;let mut bytes=vec![];f.take(limit.min(128*1024) as u64).read_to_end(&mut bytes)?;
    Ok(json!({"text":String::from_utf8_lossy(&bytes),"next_offset":offset+bytes.len(),"total_bytes":size}))})().map_err(error)
}
#[tauri::command]
pub(crate) async fn get_wake_diagnostics() -> IpcResult<Value> {
    tauri::async_runtime::spawn_blocking(codebeat_lib::platform::diagnostics)
        .await
        .map_err(|e| e.to_string())?
        .map_err(error)
}
#[tauri::command]
pub(crate) fn install_maintenance() -> IpcResult<()> {
    codebeat_lib::platform::runner_path()
        .and_then(|p| codebeat_lib::platform::register_maintenance(&p))
        .map_err(error)
}
#[tauri::command]
pub(crate) fn get_dashboard() -> IpcResult<Value> {
    (|| {
        let db = db()?;
        Ok(json!({"tasks":db.tasks()?,"runs":db.runs()?,"settings":db.settings()?}))
    })()
    .map_err(error)
}
#[tauri::command]
pub(crate) fn exit_app(cancel_scheduled: bool, app: tauri::AppHandle) -> IpcResult<()> {
    (|| {
        if cancel_scheduled {
            let db = db()?;
            for t in db.tasks()? {
                if ["scheduled", "paused"].contains(&t.state.as_str()) {
                    codebeat_lib::platform::unregister(&t.id)?;
                    db.transition(&t.id, &["scheduled", "paused"], "cancelled", None)?;
                }
            }
        }
        app.exit(0);
        Ok(())
    })()
    .map_err(error)
}
