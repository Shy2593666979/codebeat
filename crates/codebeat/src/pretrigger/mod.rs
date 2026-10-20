mod daily;
mod decisions;
mod optimizer;
mod window;
mod worker;
use crate::{ProviderId, UsageReport};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Datelike, NaiveTime, Timelike, Utc};
pub use optimizer::*;
use serde::{Deserialize, Serialize};
pub use window::*;
pub use worker::tick;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PretriggerSettings {
    pub days: String,
    pub work_start: String,
    pub work_end: String,
    pub lunch_enabled: bool,
    pub lunch_start: String,
    pub lunch_end: String,
    pub timezone: String,
    pub providers: Vec<ProviderId>,
    pub mode: String,
    pub min_work_minutes: i32,
    pub auto_trigger: bool,
    pub wake: bool,
    pub day_override: Option<DayOverride>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayOverride {
    pub date: chrono::NaiveDate,
    pub work_start: String,
    pub work_end: String,
    pub lunch_enabled: bool,
}
impl Default for PretriggerSettings {
    fn default() -> Self {
        Self {
            days: "weekdays".into(),
            work_start: "10:00".into(),
            work_end: "20:00".into(),
            lunch_enabled: false,
            lunch_start: "12:00".into(),
            lunch_end: "14:00".into(),
            timezone: "Asia/Shanghai".into(),
            providers: vec![ProviderId::Codex],
            mode: "balanced".into(),
            min_work_minutes: 60,
            auto_trigger: true,
            wake: true,
            day_override: None,
        }
    }
}
fn minute(value: &str) -> Result<i32> {
    if value.len() != 5 {
        bail!("PRETRIGGER_TIME: 时间格式应为 HH:mm");
    }
    let t = NaiveTime::parse_from_str(value, "%H:%M").context("PRETRIGGER_TIME")?;
    Ok((t.hour() * 60 + t.minute()) as i32)
}
pub fn validate(s: &PretriggerSettings) -> Result<()> {
    if !["weekdays", "daily"].contains(&s.days.as_str()) {
        bail!("PRETRIGGER_DAYS");
    }
    s.timezone
        .parse::<chrono_tz::Tz>()
        .context("PRETRIGGER_TIMEZONE")?;
    let (start, end) = (minute(&s.work_start)?, minute(&s.work_end)?);
    if start == end {
        bail!("PRETRIGGER_TIME: 上下班时间不能相同");
    }
    work_intervals(s)?;
    if !["balanced", "windows", "save"].contains(&s.mode.as_str())
        || !(15..=300).contains(&s.min_work_minutes)
    {
        bail!("PRETRIGGER_MODE: 优化模式或最短有效时长无效");
    }
    if let Some(o) = &s.day_override {
        let mut temporary = s.clone();
        temporary.day_override = None;
        temporary.work_start = o.work_start.clone();
        temporary.work_end = o.work_end.clone();
        temporary.lunch_enabled = o.lunch_enabled;
        validate(&temporary)?;
    }
    if s.providers.is_empty()
        || s.providers.len() > 2
        || (s.providers.len() == 2 && s.providers[0] == s.providers[1])
    {
        bail!("PRETRIGGER_PROVIDERS: 请选择工具");
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize)]
pub struct Plan {
    pub times: Vec<String>,
    pub max_work_minutes: i32,
    pub work_minutes_per_window: Vec<i32>,
    pub starts: Vec<i32>,
    pub intervals: Vec<(i32, i32)>,
    pub effective_windows: usize,
    pub explanation: String,
}
/// 先最小化单个额度窗口内的峰值工作分钟，再最小化不均衡度。
/// 这里假设工作强度均匀且窗口为五小时，不做 token 用量预测。
pub fn plan(s: &PretriggerSettings) -> Result<Plan> {
    optimize(s)
}
#[derive(Debug, Clone, Serialize)]
pub struct DueSlot {
    pub key: String,
    pub until: DateTime<Utc>,
}
pub fn due(s: &PretriggerSettings, now: DateTime<Utc>) -> Result<Option<DueSlot>> {
    due_dynamic(s, now, None)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeRecord {
    pub provider: ProviderId,
    pub slot: String,
    pub state: String,
    pub message: String,
    pub attempted_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub next_check_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub before: Option<UsageReport>,
    #[serde(default)]
    pub after: Option<UsageReport>,
    #[serde(default)]
    pub tokens: Option<serde_json::Value>,
    #[serde(default)]
    pub request_sent: bool,
    #[serde(default)]
    pub sent_at: Option<DateTime<Utc>>,
}
pub fn active_reset(report: &UsageReport, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    if report.status != "available" {
        return None;
    }
    report
        .windows
        .iter()
        .filter(|w| {
            w.reliability == "official"
                && w.duration_minutes == Some(300)
                && w.used_percent.is_some_and(|u| u > 0.0)
                && w.observed_at_utc >= now - chrono::Duration::minutes(2)
        })
        .filter_map(|w| w.resets_at_utc.filter(|r| *r > now))
        .max()
}
pub fn exhausted(report: &UsageReport, now: DateTime<Utc>) -> bool {
    report.status == "available"
        && report.windows.iter().any(|w| {
            w.reliability == "official"
                && w.used_percent.is_some_and(|u| u >= 100.0)
                && w.resets_at_utc.is_none_or(|r| r > now)
        })
}
