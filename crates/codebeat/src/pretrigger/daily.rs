//! 展示固定为当天首个起点的五小时刻度，执行每轮累加一分钟。
use super::*;
use chrono::{Duration, NaiveDate};

#[allow(clippy::too_many_arguments)]
pub(super) fn build(
    effective: &PretriggerSettings,
    provider: ProviderId,
    now: DateTime<Utc>,
    report: Option<&UsageReport>,
    previous: Option<&RuntimePlan>,
    date: NaiveDate,
    plan: Plan,
    end: DateTime<Utc>,
) -> Result<RuntimePlan> {
    let zone = effective.timezone.parse::<chrono_tz::Tz>()?;
    let intervals = plan
        .intervals
        .iter()
        .map(|&(a, b)| {
            Ok((
                optimizer::instant(date, a, zone)?,
                optimizer::instant(date, b, zone)?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    let previous = previous.filter(|p| p.date == date);
    let check = check_window(report, provider, now, previous);
    let day_start = previous.and_then(|p| p.day_start);
    let base = day_start.unwrap_or(optimizer::instant(date, plan.starts[0], zone)?);
    let known = previous
        .filter(|p| p.window_confirmed)
        .and_then(|p| p.actual_reset);
    let verified = check
        .reset
        .is_some_and(|r| known.is_some_and(|k| (k - r).num_seconds().abs() <= 30));
    let actual_reset = if verified { check.reset } else { known };
    let current_index = actual_reset.map(|reset| {
        ((previous
            .and_then(|p| p.latest_start)
            .unwrap_or(reset - Duration::hours(5))
            - base)
            .num_seconds()
            .max(0)
            / 18000) as usize
    });
    let mut display_times = Vec::new();
    let mut slots = Vec::new();
    let mut index = 0usize;
    loop {
        let display_at = base + Duration::hours(5 * index as i64);
        if display_at >= end {
            break;
        }
        display_times.push(display_at);
        if current_index.is_none_or(|i| index > i) {
            let mut execute_at = base + Duration::minutes(301 * index as i64);
            if let Some(reset) = actual_reset {
                let next = current_index.unwrap() + 1;
                execute_at = execute_at.max(
                    reset
                        + Duration::seconds(RESET_SETTLE_SECONDS)
                        + Duration::minutes(301 * index.saturating_sub(next) as i64),
                );
            }
            if execute_at < end {
                slots.push(PlannedSlot {
                    display_at,
                    execute_at,
                });
            }
        }
        index += 1;
    }
    let status = match check.status {
        "active" if !verified => "verifying",
        other => other,
    };
    let reason = match status {
        "active" => "官方窗口仍在计时，等待恢复后再执行",
        "verifying" => "观察到窗口变化，等待独立快照确认；暂不发送请求",
        "settling" => "官方恢复时间已到，等待一分钟后复查额度",
        "idle" => "已确认空闲额度，按当天固定节点执行",
        _ => "额度证据不足，等待刷新确认",
    };
    let automatic_allowed = effective.auto_trigger
        && effective.mode != "save"
        && provider == ProviderId::Codex
        && check.status == "idle"
        && report.is_some_and(|r| !exhausted(r, now))
        && actual_reset.is_none_or(|r| now >= r + Duration::seconds(RESET_SETTLE_SECONDS));
    Ok(RuntimePlan {
        provider,
        date,
        work_start: intervals[0].0,
        work_end: end,
        intervals,
        recommendations: slots.iter().map(|s| s.execute_at).collect(),
        slots,
        display_times,
        day_start,
        actual_reset,
        window_confirmed: actual_reset.is_some(),
        status: status.into(),
        reason: reason.into(),
        updated_at: now,
        automatic_allowed,
        idle_observed: check.idle_observed,
        idle_reset: check.idle_reset,
        idle_confirmed: check.idle_confirmed,
        latest_start: previous.and_then(|p| p.latest_start),
    })
}
