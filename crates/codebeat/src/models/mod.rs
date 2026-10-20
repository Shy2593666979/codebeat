use crate::pretrigger;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderId {
    Codex,
    ClaudeCode,
}
impl ProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude_code",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Capabilities {
    pub usage: bool,
    pub sessions: bool,
    pub resume: bool,
    pub unattended: bool,
    pub structured_events: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealth {
    pub id: ProviderId,
    pub executable_path: Option<PathBuf>,
    pub version: Option<String>,
    pub status: String,
    pub auth_status: String,
    pub message: String,
    pub capabilities: Capabilities,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageWindow {
    pub label: String,
    pub used_percent: Option<f64>,
    pub resets_at_utc: Option<DateTime<Utc>>,
    pub duration_minutes: Option<i64>,
    pub observed_at_utc: DateTime<Utc>,
    pub source: String,
    pub reliability: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageReport {
    pub provider: ProviderId,
    pub windows: Vec<UsageWindow>,
    pub status: String,
    pub message: String,
    #[serde(default)]
    pub reset_credits: Option<ResetCredits>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetCredits {
    pub available_count: u64,
    /// 不透明的身份校验值，绝不是 bearer token 或登录凭据。
    pub account_key: Option<String>,
    /// 官方明细可能缺失或被截断，不能用明细数量代替 available_count。
    #[serde(default)]
    pub credits: Option<Vec<ResetCredit>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResetCredit {
    pub id: String,
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub expiry_known: bool,
    pub status: String,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ResetOutcome {
    Reset,
    AlreadyRedeemed,
    NothingToReset,
    NoCredit,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    #[serde(default)]
    pub lock_state: SessionLockState,
    pub provider: ProviderId,
    pub session_id: String,
    pub title: String,
    pub cwd: PathBuf,
    pub last_updated_utc: Option<DateTime<Utc>>,
    pub resumable: bool,
    pub source: String,
}
/// 文件是否正被其他写入者锁定；未知状态不能当作空闲。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionLockState {
    Available,
    InUse,
    #[default]
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LockedSessionBehavior {
    #[default]
    Fork,
    Pause,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPage {
    pub data: Vec<Conversation>,
    pub next_cursor: Option<String>,
    pub warnings: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CatchUpPolicy {
    Skip,
    Ask,
    RunOnNextStart,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionProfile {
    ReadOnly,
    WorkspaceWrite,
    FullAccess,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Recurrence {
    Daily,
    Weekly,
    EveryHours { hours: u16 },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    #[serde(default)]
    pub locked_session_behavior: LockedSessionBehavior,
    #[serde(default)]
    pub recurrence: Option<Recurrence>,
    pub conversation: Conversation,
    pub title: String,
    pub prompt: String,
    pub scheduled_at_utc: DateTime<Utc>,
    pub scheduled_local: String,
    pub timezone_id: String,
    pub wake_to_run: bool,
    pub catch_up_policy: CatchUpPolicy,
    pub catch_up_ttl_minutes: i64,
    pub permission_profile: PermissionProfile,
    #[serde(default = "default_task_consent")]
    pub consent: bool,
    pub notify: bool,
}
fn default_task_consent() -> bool {
    true
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub request: CreateTaskRequest,
    pub state: String,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: String,
    pub task_id: String,
    pub attempt: i64,
    pub state: String,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub exit_code: Option<i32>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub log_path: Option<String>,
    pub pid: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub startup_minimized: bool,
    pub default_time: String,
    pub default_wake: bool,
    pub default_catch_up: bool,
    pub notify_start: bool,
    pub notify_success: bool,
    pub notify_failure: bool,
    pub notification_sound: bool,
    pub connection_interval_minutes: i64,
    pub appearance: String,
    pub paused_providers: Vec<ProviderId>,
    pub language: String,
    pub theme: String,
    pub close_to_tray: bool,
    pub autostart: bool,
    pub paused: bool,
    pub notifications: bool,
    pub reminder_minutes: i64,
    pub pretrigger_enabled: bool,
    pub pretrigger: pretrigger::PretriggerSettings,
    pub authorized_roots: Vec<PathBuf>,
    pub save_output: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            startup_minimized: false,
            default_time: "06:00".into(),
            default_wake: false,
            default_catch_up: false,
            notify_start: false,
            notify_success: true,
            notify_failure: true,
            notification_sound: true,
            connection_interval_minutes: 10,
            appearance: "system".into(),
            paused_providers: vec![],
            language: "zh-CN".into(),
            theme: "indigo".into(),
            close_to_tray: true,
            autostart: false,
            paused: false,
            notifications: true,
            reminder_minutes: 10,
            pretrigger_enabled: true,
            pretrigger: Default::default(),
            authorized_roots: vec![],
            save_output: false,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InboxNotice {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub topic: Option<String>,
    pub id: String,
    pub title: String,
    pub message: String,
    pub kind: String,
    pub task_id: Option<String>,
    pub page: String,
    pub at: DateTime<Utc>,
}
