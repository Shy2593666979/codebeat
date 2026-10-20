//! 原生轮询/重试的唯一所有者，自动刷新和手动刷新共用。
use crate::db;
use chrono::{DateTime, Duration, Utc};
use codebeat_lib::{ProviderId, UsageReport};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{Emitter, Manager};
use tokio::sync::Mutex as AsyncMutex;

const PROVIDERS: [ProviderId; 2] = [ProviderId::Codex, ProviderId::ClaudeCode];
fn index(id: ProviderId) -> usize {
    if id == ProviderId::Codex {
        0
    } else {
        1
    }
}
#[derive(Clone, Serialize)]
pub struct UsageRefreshState {
    pub provider: ProviderId,
    pub report: Option<UsageReport>,
    pub refreshing: bool,
    pub retrying: bool,
    pub next_retry_at: Option<DateTime<Utc>>,
    pub next_check_at: DateTime<Utc>,
    pub last_error: Option<String>,
}
struct Schedule {
    start: DateTime<Utc>,
    deadline: DateTime<Utc>,
    retry: Option<DateTime<Utc>>,
    failures: u32,
}
impl Schedule {
    fn new(now: DateTime<Utc>, minutes: i64) -> Self {
        Self {
            start: now,
            deadline: now + Duration::minutes(minutes),
            retry: None,
            failures: 0,
        }
    }
    fn configure(&mut self, minutes: i64) {
        self.deadline = self.start + Duration::minutes(minutes);
    }
    fn due(&self, now: DateTime<Utc>) -> bool {
        now >= self.deadline || self.retry.is_some_and(|at| at <= now)
    }
    fn begin(&mut self, now: DateTime<Utc>, minutes: i64) -> bool {
        let regular = now >= self.deadline;
        if regular {
            *self = Self::new(now, minutes);
        }
        self.retry = None;
        regular
    }
    fn finish(&mut self, now: DateTime<Utc>, retryable_failure: bool) {
        if !retryable_failure {
            self.failures = 0;
            self.retry = None;
            return;
        }
        self.failures = self.failures.saturating_add(1);
        let seconds = match self.failures {
            1 => 5,
            2 => 15,
            3 => 30,
            _ => 60,
        };
        let next = now + Duration::seconds(seconds);
        self.retry = (next < self.deadline).then_some(next);
    }
}
struct Data {
    schedule: Schedule,
    public: UsageRefreshState,
    latest: Option<UsageReport>,
    revision: u64,
}
impl Data {
    fn complete(&mut self, report: &UsageReport, now: DateTime<Utc>) {
        let success = report.status == "available" && !report.windows.is_empty();
        let retry = report.provider == ProviderId::Codex && !success;
        self.schedule.finish(now, retry);
        if success
            || self
                .public
                .report
                .as_ref()
                .is_none_or(|r| r.status != "available")
        {
            self.public.report = Some(report.clone());
        }
        self.public.last_error = retry.then(|| report.message.clone());
        self.public.refreshing = false;
        self.public.retrying = retry;
        self.public.next_retry_at = self.schedule.retry;
        self.public.next_check_at = self.schedule.deadline;
        self.latest = Some(report.clone());
        self.revision += 1;
    }
}
struct Slot {
    gate: AsyncMutex<()>,
    data: Mutex<Data>,
}
pub struct UsageMonitor {
    slots: [Slot; 2],
}
impl Slot {
    fn snapshot(&self) -> UsageRefreshState {
        self.data
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .public
            .clone()
    }
}
#[tauri::command]
pub fn get_usage_refresh(app: tauri::AppHandle) -> Vec<UsageRefreshState> {
    app.state::<UsageMonitor>()
        .slots
        .iter()
        .map(Slot::snapshot)
        .collect()
}
fn emit(app: &tauri::AppHandle, slot: &Slot) {
    let _ = app.emit("usage-updated", slot.snapshot());
}

pub fn start(app: &tauri::AppHandle) -> anyhow::Result<()> {
    let database = db()?;
    let minutes = database.settings()?.connection_interval_minutes;
    let slots = PROVIDERS.map(|provider| {
        let latest = database.latest_usage(provider).ok().flatten();
        let report = database
            .latest_successful_usage(provider)
            .ok()
            .flatten()
            .or_else(|| latest.clone());
        let mut schedule = Schedule::new(Utc::now(), minutes);
        let failed = provider == ProviderId::Codex
            && latest.as_ref().is_some_and(|r| r.status != "available");
        schedule.finish(Utc::now(), failed);
        let last_error = failed.then(|| latest.as_ref().unwrap().message.clone());
        Slot {
            gate: AsyncMutex::new(()),
            data: Mutex::new(Data {
                public: UsageRefreshState {
                    provider,
                    report,
                    refreshing: false,
                    retrying: failed,
                    next_retry_at: schedule.retry,
                    next_check_at: schedule.deadline,
                    last_error,
                },
                schedule,
                latest,
                revision: 0,
            }),
        }
    });
    app.manage(UsageMonitor { slots });
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(1));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            let minutes = db()
                .and_then(|d| d.settings())
                .map(|s| s.connection_interval_minutes)
                .unwrap_or(10);
            for provider in PROVIDERS {
                let state = app.state::<UsageMonitor>();
                let slot = &state.slots[index(provider)];
                let due = {
                    let mut data = slot.data.lock().unwrap_or_else(|e| e.into_inner());
                    data.schedule.configure(minutes);
                    data.public.next_check_at = data.schedule.deadline;
                    !data.public.refreshing && data.schedule.due(Utc::now())
                };
                if due {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = refresh(&app, provider, true).await;
                    });
                }
            }
        }
    });
    Ok(())
}

pub async fn refresh(app: &tauri::AppHandle, provider: ProviderId, scheduled: bool) -> UsageReport {
    let monitor = app.state::<UsageMonitor>();
    let slot = &monitor.slots[index(provider)];
    let revision = slot.data.lock().unwrap_or_else(|e| e.into_inner()).revision;
    let _guard = slot.gate.lock().await;
    let minutes = db()
        .and_then(|d| d.settings())
        .map(|s| s.connection_interval_minutes)
        .unwrap_or(10);
    let regular = {
        let mut data = slot.data.lock().unwrap_or_else(|e| e.into_inner());
        // 等待期间已有请求完成：直接共享那次结果。
        if data.revision != revision || (scheduled && !data.schedule.due(Utc::now())) {
            return data.latest.clone().unwrap_or_else(|| UsageReport {
                provider,
                windows: vec![],
                status: "unknown".into(),
                message: String::new(),
                reset_credits: None,
            });
        }
        data.schedule.configure(minutes);
        let regular = data.schedule.begin(Utc::now(), minutes);
        data.public.refreshing = true;
        data.public.next_retry_at = None;
        regular
    };
    emit(app, slot);
    if regular {
        let health = codebeat_lib::providers::detect(provider).await;
        if let Ok(database) = db() {
            let _ = database.cache_health(&health);
        }
        let _ = app.emit("provider-updated", health);
    }
    let report = codebeat_lib::providers::usage(provider).await;
    if let Ok(database) = db() {
        // 保留失败时的原始观测供规划使用；只有界面展示层继续用旧数据。
        let _ = database.cache_usage(&report);
        if database
            .settings()
            .is_ok_and(|s| s.pretrigger_enabled && s.pretrigger.wake)
        {
            let _ = codebeat_lib::platform::runner_path()
                .and_then(|p| codebeat_lib::platform::sync_optimizer_wake(&database, &p));
        }
    }
    {
        let mut data = slot.data.lock().unwrap_or_else(|e| e.into_inner());
        data.complete(&report, Utc::now());
    }
    emit(app, slot);
    report
}

pub async fn consume_reset(
    app: &tauri::AppHandle,
    key: &str,
    account: &str,
) -> anyhow::Result<codebeat_lib::ResetOutcome> {
    let monitor = app.state::<UsageMonitor>();
    let slot = &monitor.slots[index(ProviderId::Codex)];
    let _guard = slot.gate.lock().await;
    let outcome = codebeat_lib::providers::reset::consume(key, account).await?;
    if matches!(
        outcome,
        codebeat_lib::ResetOutcome::Reset | codebeat_lib::ResetOutcome::AlreadyRedeemed
    ) {
        db()?.record_reset_consumption(key, account)?;
    }
    {
        let mut data = slot.data.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(report) = &mut data.public.report {
            report.reset_credits = None;
        }
        if let Some(report) = &mut data.latest {
            report.reset_credits = None;
        }
        data.revision += 1;
    }
    emit(app, slot);
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retries_continue_until_deadline_then_regular_poll_takes_over() {
        let now = DateTime::from_timestamp(0, 0).unwrap();
        let mut s = Schedule::new(now, 5);
        let mut at = now;
        for delay in [5, 15, 30, 60, 60, 60, 60] {
            s.finish(at, true);
            assert_eq!(s.retry, Some(at + Duration::seconds(delay)));
            at += Duration::seconds(delay);
            assert!(s.due(at));
            assert!(!s.begin(at, 5));
        }
        s.finish(at, true);
        assert_eq!(s.retry, None); // Cannot schedule a retry beyond this cycle.
        assert!(!s.due(now + Duration::seconds(299)));
        assert!(s.due(now + Duration::minutes(5)));
        assert!(s.begin(now + Duration::minutes(5), 5));
        assert_eq!(s.deadline, now + Duration::minutes(10));
        assert_eq!(s.failures, 0);
    }
    #[test]
    fn success_cancels_retries_and_interval_changes_preserve_cycle_anchor() {
        let now = DateTime::from_timestamp(0, 0).unwrap();
        let mut s = Schedule::new(now, 10);
        s.finish(now, true);
        s.finish(now + Duration::seconds(5), false);
        assert_eq!(s.retry, None);
        assert!(!s.due(now + Duration::minutes(9)));
        s.configure(5);
        assert_eq!(s.deadline, now + Duration::minutes(5));
        assert!(s.begin(now + Duration::hours(8), 5)); // Resume after sleep: one cycle, no catch-up storm.
        assert!(!s.due(now + Duration::hours(8)));
    }
}
