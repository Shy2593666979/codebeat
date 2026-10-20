//! 计划、首次检查和发送前复查共用的窗口判定。
use super::*;
use chrono::Duration;

#[derive(Debug, Clone)]
pub struct WindowCheck {
    pub status: &'static str,
    pub reset: Option<DateTime<Utc>>,
    pub idle_observed: Option<DateTime<Utc>>,
    pub idle_reset: Option<DateTime<Utc>>,
    pub idle_confirmed: bool,
}
pub fn check_window(
    report: Option<&UsageReport>,
    provider: ProviderId,
    now: DateTime<Utc>,
    old: Option<&RuntimePlan>,
) -> WindowCheck {
    let mut result = WindowCheck {
        status: "unknown",
        reset: None,
        idle_observed: None,
        idle_reset: None,
        idle_confirmed: false,
    };
    let Some(report) = report.filter(|r| r.provider == provider) else {
        return result;
    };
    let Some(window) = fresh_window(report, now) else {
        return result;
    };
    if let Some(reset) = window.resets_at_utc {
        if window.used_percent.is_some_and(|u| u > 0.0) && reset > now {
            result.status = "active";
            result.reset = Some(reset);
            return result;
        }
        if old.is_some_and(|p| {
            p.window_confirmed
                && p.actual_reset
                    .is_some_and(|r| (r - reset).num_seconds().abs() <= 30)
        }) && reset > now
        {
            result.status = "active";
            result.reset = Some(reset);
            return result;
        }
        if reset <= now && reset + Duration::seconds(RESET_SETTLE_SECONDS) > now {
            result.status = "settling";
            result.reset = Some(reset);
            return result;
        }
    }
    if window.used_percent != Some(0.0) {
        return result;
    }
    let Some(reset) = window.resets_at_utc.filter(|r| *r > now) else {
        result.status = "idle";
        result.idle_confirmed = true;
        return result;
    };
    // 空闲浮动值必须随两次独立读取一起移动；固定的未来恢复时间不能当作空闲。
    if ((reset - window.observed_at_utc).num_seconds() - 18000).abs() > 120 {
        return result;
    }
    result.idle_observed = Some(window.observed_at_utc);
    result.idle_reset = Some(reset);
    result.idle_confirmed = old.is_some_and(|p| {
        p.idle_observed.zip(p.idle_reset).is_some_and(|(at, end)| {
            let elapsed = (window.observed_at_utc - at).num_seconds();
            (p.idle_confirmed
                && (0..2).contains(&elapsed)
                && (end - reset).num_seconds().abs() <= 2)
                || (elapsed >= 2
                    && (reset - end).num_seconds() >= 1
                    && ((end - at).num_seconds() - 18000).abs() <= 120
                    && ((reset - end).num_seconds() - elapsed).abs() <= 10)
        })
    });
    if result.idle_confirmed {
        result.status = "idle";
    }
    result
}
