//! AWO 在候选评分时以本地时间（分钟）为单位，在执行时使用绝对时刻。
//! 预测出的边界永远不能证明服务商已经重置。
use super::*;
use chrono::{Duration, NaiveDate, TimeZone};

pub fn work_intervals(s: &PretriggerSettings) -> Result<Vec<(i32, i32)>> {
    let start = minute(&s.work_start)?;
    let mut end = minute(&s.work_end)?;
    if end == start {
        bail!("PRETRIGGER_TIME: 上下班时间不能相同");
    }
    if end < start {
        end += 1440;
    }
    if !s.lunch_enabled {
        return Ok(vec![(start, end)]);
    }
    let mut ls = minute(&s.lunch_start)?;
    let mut le = minute(&s.lunch_end)?;
    if ls < start {
        ls += 1440;
    }
    if le <= ls {
        le += 1440;
    }
    if !(start < ls && ls < le && le < end) {
        bail!("PRETRIGGER_LUNCH: 午休须在工作时间内");
    }
    Ok(vec![(start, ls), (le, end)])
}
pub fn overlap(intervals: &[(i32, i32)], a: i32, b: i32) -> i32 {
    intervals
        .iter()
        .map(|&(s, e)| (e.min(b) - s.max(a)).max(0))
        .sum()
}
pub fn clock(t: i32) -> String {
    let day = t.div_euclid(1440);
    let t = t.rem_euclid(1440);
    format!(
        "{}{:02}:{:02}",
        if day < 0 {
            "前日 "
        } else if day > 0 {
            "次日 "
        } else {
            ""
        },
        t / 60,
        t % 60
    )
}
fn window_score(s: &PretriggerSettings, work: &[i32], idle: i64) -> i64 {
    let count = work.iter().filter(|&&n| n >= s.min_work_minutes).count() as i64;
    let short: i64 = work
        .iter()
        .filter(|&&n| n < s.min_work_minutes)
        .map(|&n| i64::from(s.min_work_minutes - n))
        .sum();
    let squares: i64 = work.iter().map(|&n| i64::from(n) * i64::from(n)).sum();
    count
        * if s.mode == "windows" {
            1_000_000
        } else {
            100_000
        }
        - short * 2_000
        - squares
        - idle * 10
        - (work.len() as i64) * 500
}
pub fn optimize(s: &PretriggerSettings) -> Result<Plan> {
    validate(s)?;
    let intervals = work_intervals(s)?;
    let start = intervals[0].0;
    let end = intervals.last().context("PRETRIGGER_INTERVAL")?.1;
    let mut candidates: Vec<i32> = (start - 295..=start).step_by(5).collect();
    // 连续公式只提供一个候选起点，所有候选仍要参与比较。
    let length = end - start;
    if !s.lunch_enabled && length >= 2 * s.min_work_minutes {
        let n = 2 + (length - 2 * s.min_work_minutes) / 300;
        candidates.push(start + (length - (n - 2) * 300) / 2 - 300);
    }
    if s.mode == "save" {
        candidates = vec![start];
    }
    let mut best: Option<(i64, i32, Vec<i32>, Vec<i32>)> = None;
    for first in candidates {
        let times: Vec<_> = (first..end).step_by(300).collect();
        let work: Vec<_> = times
            .iter()
            .map(|&t| overlap(&intervals, t, t + 300))
            .collect();
        let idle: i64 = times
            .iter()
            .map(|&t| {
                i64::from((t + 300).min(end) - t.max(start) - overlap(&intervals, t, t + 300))
            })
            .sum();
        let score = window_score(s, &work, idle);
        if best
            .as_ref()
            .is_none_or(|b| score > b.0 || (score == b.0 && first > b.1))
        {
            best = Some((score, first, times, work));
        }
    }
    let (_, _, starts, work) = best.context("PRETRIGGER_PLAN")?;
    Ok(Plan {
        times: starts.iter().map(|&t| clock(t)).collect(),
        max_work_minutes: *work.iter().max().unwrap_or(&0),
        effective_windows: work.iter().filter(|&&n| n >= s.min_work_minutes).count(),
        work_minutes_per_window: work,
        starts,
        intervals,
        explanation: if s.mode == "save" {
            "节省额度模式仅提供建议，不发送额外请求"
        } else {
            "按实际工作交集均衡窗口；后续边界为理论预测，执行前须重新验证官方额度"
        }
        .into(),
    })
}
pub fn effective_settings(s: &PretriggerSettings, date: NaiveDate) -> PretriggerSettings {
    let mut result = s.clone();
    if let Some(o) = &s.day_override {
        if o.date == date {
            result.work_start = o.work_start.clone();
            result.work_end = o.work_end.clone();
            result.lunch_enabled = o.lunch_enabled;
        }
    }
    result.day_override = None;
    result
}
pub(super) fn instant(date: NaiveDate, minute: i32, zone: chrono_tz::Tz) -> Result<DateTime<Utc>> {
    let wall = date.and_hms_opt(0, 0, 0).context("PRETRIGGER_DATE")?
        + Duration::minutes(i64::from(minute));
    zone.from_local_datetime(&wall)
        .single()
        .map(|t| t.with_timezone(&Utc))
        .context("PRETRIGGER_DST: 当地时间不存在或重复，跳过自动触发")
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimePlan {
    pub provider: ProviderId,
    pub date: NaiveDate,
    pub work_start: DateTime<Utc>,
    pub work_end: DateTime<Utc>,
    pub intervals: Vec<(DateTime<Utc>, DateTime<Utc>)>,
    pub recommendations: Vec<DateTime<Utc>>,
    pub actual_reset: Option<DateTime<Utc>>,
    #[serde(default)]
    pub window_confirmed: bool,
    pub status: String,
    pub reason: String,
    pub updated_at: DateTime<Utc>,
    pub automatic_allowed: bool,
    #[serde(default)]
    pub day_start: Option<DateTime<Utc>>,
    #[serde(default)]
    pub display_times: Vec<DateTime<Utc>>,
    #[serde(default)]
    pub idle_observed: Option<DateTime<Utc>>,
    #[serde(default)]
    pub idle_reset: Option<DateTime<Utc>>,
    #[serde(default)]
    pub idle_confirmed: bool,
    #[serde(default)]
    pub slots: Vec<PlannedSlot>,
    #[serde(default)]
    pub latest_start: Option<DateTime<Utc>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedSlot {
    pub display_at: DateTime<Utc>,
    pub execute_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmedWindow {
    pub provider: ProviderId,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub display_start: Option<DateTime<Utc>>,
    #[serde(default)]
    pub closed_at: Option<DateTime<Utc>>,
}

/// 官方重置边界后留出一分钟，再读取额度并决定是否发送请求。
pub const RESET_SETTLE_SECONDS: i64 = 60;
pub fn fresh_window(report: &UsageReport, now: DateTime<Utc>) -> Option<&crate::UsageWindow> {
    if report.status != "available" {
        return None;
    }
    report
        .windows
        .iter()
        .filter(|w| {
            w.reliability == "official"
                && w.duration_minutes == Some(300)
                && w.observed_at_utc <= now + Duration::seconds(30)
                && w.observed_at_utc >= now - Duration::minutes(2)
                && w.used_percent
                    .is_some_and(|u| u.is_finite() && (0.0..=100.0).contains(&u))
        })
        .max_by(|a, b| {
            a.resets_at_utc
                .is_some_and(|r| r > now)
                .cmp(&b.resets_at_utc.is_some_and(|r| r > now))
                .then_with(|| {
                    a.used_percent
                        .unwrap_or(0.0)
                        .total_cmp(&b.used_percent.unwrap_or(0.0))
                })
        })
}
pub fn cost_anomaly(before: &UsageReport, after: &UsageReport) -> bool {
    before.provider == after.provider
        && before.windows.iter().any(|b| {
            b.reliability == "official"
                && b.used_percent.is_some_and(|u| {
                    after.windows.iter().any(|a| {
                        a.reliability == "official"
                            && a.label == b.label
                            && a.duration_minutes == b.duration_minutes
                            && a.used_percent.is_some_and(|v| v.is_finite() && v - u > 5.0)
                    })
                })
        })
}
/// 计算星期时使用班次的起始日期，包括从前一夜开始的班次。
pub fn runtime_plan(
    s: &PretriggerSettings,
    provider: ProviderId,
    now: DateTime<Utc>,
    report: Option<&UsageReport>,
    previous: Option<&RuntimePlan>,
) -> Result<RuntimePlan> {
    validate(s)?;
    let zone = s.timezone.parse::<chrono_tz::Tz>()?;
    let today = now.with_timezone(&zone).date_naive();
    let mut chosen = None;
    for offset in -1..=7 {
        let date = today + Duration::days(offset);
        if s.days == "weekdays" && date.weekday().number_from_monday() > 5 {
            continue;
        }
        let effective = effective_settings(s, date);
        let p = plan(&effective)?;
        let end = instant(date, p.intervals.last().context("INTERVAL")?.1, zone)?;
        if end <= now {
            continue;
        }
        chosen = Some((date, effective, p, end));
        break;
    }
    let (date, effective, p, end) = chosen.context("PRETRIGGER_DATE")?;
    super::daily::build(&effective, provider, now, report, previous, date, p, end)
}

/// 工作结束后展示下一工作日，当前实际窗口优先于理论节点。
pub fn display_plan(
    s: &PretriggerSettings,
    provider: ProviderId,
    now: DateTime<Utc>,
    report: Option<&UsageReport>,
    previous: Option<&RuntimePlan>,
) -> Result<RuntimePlan> {
    runtime_plan(s, provider, now, report, previous)
}

pub fn scheduled_slot(
    s: &PretriggerSettings,
    p: &RuntimePlan,
    now: DateTime<Utc>,
) -> Option<DueSlot> {
    if now >= p.work_end || p.status == "active" {
        return None;
    }
    let slot = p.slots.iter().rev().find(|s| s.execute_at <= now)?;
    let target = slot.execute_at;
    // 只恢复近期仍然有用的触发点；不要在中午重放清晨的预启动。
    let remaining: i64 = p
        .intervals
        .iter()
        .map(|(a, b)| (*b - now.max(*a)).num_minutes().max(0))
        .sum();
    let useful: i64 = p
        .intervals
        .iter()
        .map(|(a, b)| {
            ((*b).min(now + Duration::hours(5)) - now.max(*a))
                .num_minutes()
                .max(0)
        })
        .sum();
    if now - target > Duration::minutes(10)
        || remaining < i64::from(s.min_work_minutes)
        || useful < i64::from(s.min_work_minutes)
    {
        return None;
    }
    Some(DueSlot {
        key: format!("{}:{}:{}", p.date, slot.display_at.timestamp(), s.timezone),
        until: p.work_end,
    })
}
pub fn due_for_plan(
    s: &PretriggerSettings,
    p: &RuntimePlan,
    now: DateTime<Utc>,
) -> Option<DueSlot> {
    if !p.automatic_allowed {
        return None;
    }
    scheduled_slot(s, p, now)
}
pub fn due_dynamic(
    s: &PretriggerSettings,
    now: DateTime<Utc>,
    previous: Option<&RuntimePlan>,
) -> Result<Option<DueSlot>> {
    let p = runtime_plan(s, ProviderId::Codex, now, None, previous)?;
    Ok(scheduled_slot(s, &p, now))
}

/// 今天最后一次推荐已过期后，仍要为明天保留唤醒定时器。
pub fn next_wake_at(
    s: &PretriggerSettings,
    plans: &[RuntimePlan],
    now: DateTime<Utc>,
) -> Result<Option<DateTime<Utc>>> {
    if !s.wake || !s.auto_trigger || s.mode == "save" || !s.providers.contains(&ProviderId::Codex) {
        return Ok(None);
    }
    let current = plans.iter().find(|p| p.provider == ProviderId::Codex);
    if let Some(next) = current
        .and_then(|p| p.recommendations.iter().filter(|&&t| t > now).min())
        .copied()
    {
        return Ok(Some(next));
    }
    let after = current.map_or(now, |p| p.work_end.max(now));
    let next = runtime_plan(s, ProviderId::Codex, after, None, None)?;
    Ok(next.recommendations.into_iter().filter(|&t| t > now).min())
}
