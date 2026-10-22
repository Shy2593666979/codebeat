//! 稳定快照与真实发送记录重建窗口，单条正用量快照只能用于暂缓发送。
use super::*;
use chrono::{DateTime, TimeZone};

struct Candidate {
    provider: ProviderId,
    label: String,
    account: Option<String>,
    reset: DateTime<Utc>,
    reference: DateTime<Utc>,
    first: DateTime<Utc>,
    last: DateTime<Utc>,
}
struct ZeroSample {
    provider: ProviderId,
    label: String,
    account: Option<String>,
    observed: DateTime<Utc>,
    reset: Option<DateTime<Utc>>,
}

/// 仅核实已经成功发送的请求；零用量本身不能证明窗口开启。
fn stable_zero_reset(
    samples: &[ZeroSample],
    provider: ProviderId,
    account: Option<&str>,
    sent: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let expected = sent + Duration::hours(5);
    let matching: Vec<_> = samples
        .iter()
        .filter(|sample| {
            sample.provider == provider
                && sample.account.as_deref() == account
                && sample.observed >= sent
                && sample.reset.is_some_and(|reset| {
                    reset > sample.observed && (reset - expected).num_seconds().abs() <= 120
                })
        })
        .collect();
    matching
        .iter()
        .filter(|last| {
            matching.iter().any(|first| {
                last.label == first.label
                    && last.observed - first.observed >= Duration::seconds(30)
                    && (last.reset.unwrap() - first.reset.unwrap())
                        .num_seconds()
                        .abs()
                        <= 2
            })
        })
        .max_by_key(|sample| sample.observed)
        .and_then(|sample| sample.reset)
}
impl Database {
    pub fn optimizer_confirmed_windows(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<allocation::ConfirmedWindow>> {
        let settings = self.settings()?.pretrigger;
        let date =
            allocation::runtime_plan(&settings, settings.providers[0], now, None, None)?.date;
        let zone = settings.timezone.parse::<chrono_tz::Tz>()?;
        let start = zone
            .from_local_datetime(&date.and_hms_opt(0, 0, 0).context("DAY_START")?)
            .earliest()
            .context("DAY_START")?
            .with_timezone(&Utc);
        let range_end = start
            + Duration::minutes(i64::from(
                allocation::work_intervals(&allocation::effective_settings(&settings, date))?
                    .last()
                    .context("WORK_INTERVAL")?
                    .1
                    .max(1440),
            ));
        let current_account = self
            .latest_successful_usage(ProviderId::Codex)?
            .and_then(|r| r.reset_credits.and_then(|c| c.account_key));
        let mut rows = entities::usage_snapshots::Entity::find()
            .order_by_desc(Expr::cust("rowid"))
            .limit(10000)
            .all(&self.conn)?;
        rows.reverse();
        let mut candidates: Vec<Candidate> = Vec::new();
        let mut zero_samples = Vec::new();
        for row in rows {
            let report: UsageReport = serde_json::from_str(&row.data_json)?;
            if report.status != "available" || !settings.providers.contains(&report.provider) {
                continue;
            }
            let account = report
                .reset_credits
                .as_ref()
                .and_then(|c| c.account_key.clone());
            if report.provider == ProviderId::Codex
                && current_account.is_some()
                && account != current_account
            {
                continue;
            }
            for window in &report.windows {
                let observed = window.observed_at_utc;
                if window.reliability == "official"
                    && window.duration_minutes == Some(300)
                    && window.used_percent == Some(0.0)
                    && observed <= now + Duration::seconds(30)
                {
                    zero_samples.push(ZeroSample {
                        provider: report.provider,
                        label: window.label.clone(),
                        account: account.clone(),
                        observed,
                        reset: window.resets_at_utc,
                    });
                }
                if window.reliability != "official"
                    || window.duration_minutes != Some(300)
                    || !window
                        .used_percent
                        .is_some_and(|u| u.is_finite() && u > 0.0 && u <= 100.0)
                    || observed > now + Duration::seconds(30)
                {
                    continue;
                }
                let Some(reset) = window
                    .resets_at_utc
                    .filter(|r| *r > observed && *r > start && *r - Duration::hours(5) < range_end)
                else {
                    continue;
                };
                if let Some(candidate) = candidates.iter_mut().find(|c| {
                    c.provider == report.provider
                        && c.label == window.label
                        && c.account == account
                        && (c.reference - reset).num_seconds().abs() <= 2
                }) {
                    candidate.first = candidate.first.min(observed);
                    candidate.last = candidate.last.max(observed);
                    if observed == candidate.last {
                        candidate.reset = reset;
                    }
                } else {
                    candidates.push(Candidate {
                        provider: report.provider,
                        label: window.label.clone(),
                        account: account.clone(),
                        reset,
                        reference: reset,
                        first: observed,
                        last: observed,
                    });
                }
            }
        }
        let closures: Vec<_> = candidates
            .iter()
            .map(|c| {
                let closed = zero_samples
                    .iter()
                    .filter(|z| {
                        z.provider == c.provider
                            && z.label == c.label
                            && z.account == c.account
                            && z.observed > c.last
                            && z.observed < c.reset
                            && z.reset
                                .is_none_or(|r| (r - c.reset).num_seconds().abs() > 30)
                    })
                    .map(|z| z.observed)
                    .min();
                (c.provider, c.reset, closed)
            })
            .collect();
        let mut windows: Vec<allocation::ConfirmedWindow> = candidates
            .into_iter()
            .filter(|c| c.last - c.first >= Duration::seconds(30))
            .map(|c| allocation::ConfirmedWindow {
                provider: c.provider,
                start: c.reset - Duration::hours(5),
                end: c.reset,
                source: "observed".into(),
                display_start: None,
                closed_at: closures
                    .iter()
                    .find(|(provider, reset, _)| *provider == c.provider && *reset == c.reset)
                    .and_then(|(_, _, end)| *end),
            })
            .collect();
        for run in self.pretrigger_runs()? {
            if !settings.providers.contains(&run.provider)
                || !run.request_sent
                || !matches!(run.state.as_str(), "confirmed" | "sent_unconfirmed")
            {
                continue;
            }
            let sent = run.sent_at.unwrap_or(run.attempted_at);
            let run_account = run
                .after
                .as_ref()
                .or(run.before.as_ref())
                .and_then(|report| report.reset_credits.as_ref())
                .and_then(|credits| credits.account_key.as_deref());
            if current_account
                .as_deref()
                .is_some_and(|account| run_account != Some(account))
            {
                continue;
            }
            let expected = run.next_check_at.unwrap_or(sent + Duration::hours(5));
            let official_end = windows
                .iter()
                .filter(|window| {
                    window.provider == run.provider
                        && window.source == "observed"
                        && (window.end - expected).num_seconds().abs() <= 120
                })
                .map(|window| window.end)
                .max()
                .or_else(|| stable_zero_reset(&zero_samples, run.provider, run_account, sent));
            let end = official_end.unwrap_or(expected);
            let confirmed = run.state == "confirmed" || official_end.is_some();
            let actual_start = if confirmed {
                end - Duration::hours(5)
            } else {
                sent
            };
            if actual_start < start || actual_start >= range_end || sent > now {
                continue;
            }
            let display_start = run
                .slot
                .split(':')
                .nth(1)
                .and_then(|s| s.parse::<i64>().ok())
                .and_then(|t| DateTime::from_timestamp(t, 0));
            windows
                .retain(|w| w.provider != run.provider || (w.end - end).num_seconds().abs() > 120);
            windows.push(allocation::ConfirmedWindow {
                provider: run.provider,
                start: actual_start,
                end,
                source: if confirmed { "app" } else { "app_unconfirmed" }.into(),
                display_start,
                closed_at: closures
                    .iter()
                    .find(|(provider, reset, _)| {
                        *provider == run.provider && (*reset - end).num_seconds().abs() <= 120
                    })
                    .and_then(|(_, _, closed)| *closed),
            });
        }
        windows.retain(|w| w.start >= start && w.start < range_end && w.start <= now);
        windows.sort_by_key(|w| (w.start, w.end));
        windows
            .dedup_by(|a, b| a.provider == b.provider && (a.end - b.end).num_seconds().abs() <= 30);
        Ok(windows)
    }
}
