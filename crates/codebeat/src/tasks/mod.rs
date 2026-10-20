use crate::{models::*, pretrigger};
use anyhow::{bail, Context, Result};
use chrono::{DateTime, LocalResult, NaiveDateTime, TimeZone, Utc};
use std::path::{Path, PathBuf};
pub mod execution;
pub mod service;

pub fn validate_settings(s: &Settings) -> Result<()> {
    chrono::NaiveTime::parse_from_str(&s.default_time, "%H:%M").context("INVALID_DEFAULT_TIME")?;
    if ![5, 10, 30, 60].contains(&s.connection_interval_minutes)
        || !["system", "light", "dark"].contains(&s.appearance.as_str())
    {
        bail!("INVALID_SETTINGS: appearance/connection interval");
    }
    if !["zh-CN", "en-US"].contains(&s.language.as_str())
        || !["indigo", "teal", "navy", "warm"].contains(&s.theme.as_str())
    {
        bail!("INVALID_SETTINGS: language/theme");
    }
    if !(0..=60).contains(&s.reminder_minutes) {
        bail!("INVALID_SETTINGS: reminder_minutes");
    }
    pretrigger::validate(&s.pretrigger)?;
    Ok(())
}
pub fn local_directory(path: &Path) -> Result<PathBuf> {
    let canonical = path.canonicalize().context("CWD_MISSING: 工作目录不存在")?;
    if !canonical.is_dir() {
        bail!("CWD_MISSING: 必须是目录");
    }
    Ok(canonical)
}
pub fn validate_task(
    r: &CreateTaskRequest,
    _settings: &Settings,
    now: DateTime<Utc>,
    check_time: bool,
) -> Result<()> {
    if let Some(Recurrence::EveryHours { hours }) = r.recurrence {
        if !(1..=168).contains(&hours) {
            bail!("INVALID_RECURRENCE");
        }
    }
    uuid::Uuid::parse_str(&r.conversation.session_id)
        .context("SESSION_INVALID: 需要有效会话 ID")?;
    if !r.conversation.resumable {
        bail!("SESSION_NOT_RESUMABLE: 此会话不能续聊");
    }
    if r.prompt.trim().is_empty()
        || r.prompt.chars().count() > 4000
        || r.title.trim().is_empty()
        || r.title.chars().count() > 120
    {
        bail!("INVALID_INPUT: 标题或提示词长度无效");
    }
    if !(1..=720).contains(&r.catch_up_ttl_minutes) {
        bail!("INVALID_TTL: 补执行期限须为 1–720 分钟");
    }
    if check_time && r.scheduled_at_utc <= now {
        bail!("TIME_IN_PAST: 执行时间必须晚于当前时间");
    }
    let timezone: chrono_tz::Tz = r.timezone_id.parse().context("INVALID_TIMEZONE")?;
    let wall = NaiveDateTime::parse_from_str(&r.scheduled_local, "%Y-%m-%dT%H:%M")
        .context("INVALID_LOCAL_TIME")?;
    match timezone.from_local_datetime(&wall) {
        LocalResult::Single(instant)
            if instant.with_timezone(&Utc).timestamp() / 60
                == r.scheduled_at_utc.timestamp() / 60 => {}
        LocalResult::Ambiguous(_, _) => bail!("AMBIGUOUS_TIME: 夏令时重复时刻，请选择其他时间"),
        _ => bail!("INVALID_LOCAL_TIME: 当地时间不存在或与 UTC 不一致"),
    }
    local_directory(&r.conversation.cwd)?;
    Ok(())
}
pub fn missed_state(r: &CreateTaskRequest, now: DateTime<Utc>) -> &'static str {
    let age = now.signed_duration_since(r.scheduled_at_utc).num_seconds();
    if age <= 120 {
        return "scheduled";
    }
    if age > r.catch_up_ttl_minutes * 60 {
        return "expired";
    }
    match r.catch_up_policy {
        CatchUpPolicy::Skip => "expired",
        CatchUpPolicy::Ask => "needs_review",
        CatchUpPolicy::RunOnNextStart => "scheduled",
    }
}

pub fn next_occurrence(
    r: &CreateTaskRequest,
    now: DateTime<Utc>,
) -> Result<Option<CreateTaskRequest>> {
    let Some(repeat) = r.recurrence else {
        return Ok(None);
    };
    let timezone: chrono_tz::Tz = r.timezone_id.parse().context("INVALID_TIMEZONE")?;
    let next = match repeat {
        Recurrence::EveryHours { hours } => {
            if !(1..=168).contains(&hours) {
                bail!("INVALID_RECURRENCE");
            }
            let interval = chrono::Duration::hours(i64::from(hours));
            let steps =
                (now - r.scheduled_at_utc).num_seconds().max(0) / interval.num_seconds() + 1;
            r.scheduled_at_utc + interval * i32::try_from(steps).context("RECURRENCE_OVERFLOW")?
        }
        Recurrence::Daily | Recurrence::Weekly => {
            let days = if repeat == Recurrence::Daily { 1 } else { 7 };
            let initial = r.scheduled_at_utc.with_timezone(&timezone).naive_local();
            let elapsed = (now.with_timezone(&timezone).date_naive() - initial.date())
                .num_days()
                .max(0);
            let mut wall = initial + chrono::Duration::days((elapsed / days).max(1) * days);
            // 保持本地时间不变；夏令时中不存在的时间顺延到第一个有效分钟。
            loop {
                match timezone.from_local_datetime(&wall) {
                    LocalResult::Single(t) if t.with_timezone(&Utc) > now => {
                        break t.with_timezone(&Utc)
                    }
                    LocalResult::Ambiguous(a, b) if a.max(b).with_timezone(&Utc) > now => {
                        break [a, b]
                            .into_iter()
                            .map(|t| t.with_timezone(&Utc))
                            .filter(|t| *t > now)
                            .min()
                            .unwrap();
                    }
                    LocalResult::None => wall += chrono::Duration::minutes(1),
                    _ => wall += chrono::Duration::days(days),
                }
            }
        }
    };
    let mut result = r.clone();
    result.scheduled_at_utc = next;
    result.scheduled_local = next
        .with_timezone(&timezone)
        .format("%Y-%m-%dT%H:%M")
        .to_string();
    Ok(Some(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_safe() {
        let s = Settings::default();
        assert!(s.pretrigger_enabled);
        assert!(s.pretrigger.auto_trigger);
        assert!(s.pretrigger.wake);
        assert!(!s.autostart);
        assert!(!s.save_output);
    }
    #[test]
    fn network_rejected() {
        assert!(local_directory(Path::new("\\\\server\\share")).is_err());
    }
    #[test]
    fn dst_ambiguous_and_missing() {
        let tz: chrono_tz::Tz = "America/New_York".parse().unwrap();
        let d = |s| NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M").unwrap();
        assert!(matches!(
            tz.from_local_datetime(&d("2026-11-01T01:30")),
            LocalResult::Ambiguous(_, _)
        ));
        assert!(matches!(
            tz.from_local_datetime(&d("2026-03-08T02:30")),
            LocalResult::None
        ));
    }
}
