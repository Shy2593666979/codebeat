use crate::*;
use anyhow::{bail, Context, Result};
use chrono::{Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{
    path::{Path, PathBuf},
    time::Duration as StdDuration,
};
use uuid::Uuid;
mod quota_notifications;
mod windows;

pub fn data_dir() -> Result<PathBuf> {
    Ok(
        PathBuf::from(std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA unavailable")?)
            .join("codebeat"),
    )
}
pub struct Database {
    conn: Connection,
    directory: PathBuf,
}
impl Database {
    pub fn runtime_pretrigger(&self, id: ProviderId) -> Result<Option<pretrigger::RuntimePlan>> {
        let data: Option<String> = self
            .conn
            .query_row(
                "SELECT data_json FROM awo_runtime WHERE provider_id=?1",
                [id.as_str()],
                |r| r.get(0),
            )
            .optional()?;
        data.map(|v| Ok(serde_json::from_str(&v)?)).transpose()
    }
    pub fn replan_provider(
        &self,
        id: ProviderId,
        report: Option<&UsageReport>,
        now: chrono::DateTime<Utc>,
    ) -> Result<pretrigger::RuntimePlan> {
        let old = self.runtime_pretrigger(id)?;
        let settings = self.settings()?.pretrigger;
        let initial = pretrigger::runtime_plan(&settings, id, now, None, None)?;
        let mut seed = old
            .filter(|p| p.date == initial.date && !p.display_times.is_empty())
            .unwrap_or(initial);
        let history = self.optimizer_confirmed_windows(now)?;
        let own: Vec<_> = history.iter().filter(|w| w.provider == id).collect();
        if let Some(first) = own.first() {
            seed.day_start = Some(first.display_start.unwrap_or(first.start));
        }
        if let Some(last) = own.last() {
            seed.actual_reset = Some(last.closed_at.unwrap_or(last.end));
            seed.latest_start = Some(last.start);
            seed.window_confirmed = true;
        }
        let p = pretrigger::runtime_plan(&settings, id, now, report, Some(&seed))?;
        self.conn.execute("INSERT INTO awo_runtime VALUES(?1,?2) ON CONFLICT(provider_id) DO UPDATE SET data_json=excluded.data_json",params![id.as_str(),serde_json::to_string(&p)?])?;
        Ok(p)
    }
    pub fn optimizer_status(&self) -> Result<Vec<pretrigger::RuntimePlan>> {
        self.settings()?
            .pretrigger
            .providers
            .iter()
            .map(|&id| self.replan_provider(id, self.latest_usage(id)?.as_ref(), Utc::now()))
            .collect()
    }
    pub fn optimizer_display_status(
        &self,
        now: chrono::DateTime<Utc>,
    ) -> Result<Vec<pretrigger::RuntimePlan>> {
        let selected = self.settings()?.pretrigger.providers;
        self.conn.execute(
            "DELETE FROM awo_runtime WHERE provider_id NOT IN (SELECT value FROM json_each(?1))",
            [serde_json::to_string(&selected)?],
        )?;
        selected
            .iter()
            .map(|&id| self.replan_provider(id, self.latest_usage(id)?.as_ref(), now))
            .collect()
    }
    pub fn reserve_probe_request(
        &self,
        r: &pretrigger::ProbeRecord,
        now: chrono::DateTime<Utc>,
    ) -> Result<bool> {
        let n=self.conn.execute("INSERT OR IGNORE INTO awo_attempts SELECT ?1,?2,?3 WHERE NOT EXISTS(SELECT 1 FROM awo_attempts WHERE provider_id=?1 AND attempted_at>?4) AND NOT EXISTS(SELECT 1 FROM tasks WHERE state IN ('claimed','running','cancelling')) AND EXISTS(SELECT 1 FROM app_settings WHERE key='settings' AND json_extract(value_json,'$.pretrigger_enabled')=1 AND json_extract(value_json,'$.pretrigger.auto_trigger')=1 AND coalesce(json_extract(value_json,'$.pretrigger.mode'),'balanced')!='save' AND coalesce(json_extract(value_json,'$.paused'),0)=0 AND NOT EXISTS(SELECT 1 FROM json_each(value_json,'$.paused_providers') WHERE value=?1) AND EXISTS(SELECT 1 FROM json_each(value_json,'$.pretrigger.providers') WHERE value=?1))",params![r.provider.as_str(),r.slot,now.to_rfc3339(),(now-Duration::hours(5)).to_rfc3339()])?;
        self.conn.execute(
            "DELETE FROM awo_attempts WHERE attempted_at<?1",
            [(now - Duration::days(30)).to_rfc3339()],
        )?;
        Ok(n == 1)
    }
    pub fn persist_probe_progress(&self, r: &pretrigger::ProbeRecord) -> Result<()> {
        self.conn.execute("UPDATE pretrigger_runs SET data_json=?1 WHERE provider_id=?2 AND slot=?3 AND state='running'",params![serde_json::to_string(r)?,r.provider.as_str(),r.slot])?;
        Ok(())
    }
    pub fn upcoming_real_task(&self, id: ProviderId, now: chrono::DateTime<Utc>) -> Result<bool> {
        Ok(self.tasks()?.iter().any(|t| {
            t.request.conversation.provider == id
                && matches!(t.state.as_str(), "scheduled" | "claimed" | "running")
                && t.request.scheduled_at_utc <= now + Duration::minutes(15)
                && t.request.scheduled_at_utc >= now - Duration::minutes(10)
        }))
    }
    pub fn optimizer_habit(&self) -> Result<Vec<serde_json::Value>> {
        use chrono::Datelike;
        use chrono::Timelike;
        let s = self.settings()?.pretrigger;
        let zone = s.timezone.parse::<chrono_tz::Tz>()?;
        let cutoff = (Utc::now() - Duration::days(21)).to_rfc3339();
        let mut stmt=self.conn.prepare("SELECT provider_id,observed_at FROM awo_activity WHERE observed_at>?1 ORDER BY observed_at")?;
        let rows = stmt.query_map([cutoff], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut days = std::collections::BTreeMap::new();
        for row in rows {
            let (id, at) = row?;
            let at = chrono::DateTime::parse_from_rfc3339(&at)?.with_timezone(&zone);
            if s.days == "weekdays" && at.weekday().number_from_monday() > 5 {
                continue;
            }
            days.entry((id, at.date_naive()))
                .or_insert((at.hour() * 60 + at.minute()) as i32);
        }
        let intervals = pretrigger::work_intervals(&s)?;
        let start = intervals[0].0;
        let overnight = intervals.last().is_some_and(|(_, end)| *end > 1440);
        Ok(s.providers
            .iter()
            .map(|id| {
                let early = days
                    .iter()
                    .filter(|((p, _), m)| !overnight && p == id.as_str() && **m < start - 30)
                    .count();
                serde_json::json!({"provider":id,"early_days":early,"suggest_adjustment":early>=5})
            })
            .collect())
    }
    pub fn open_default() -> Result<Self> {
        Self::open(&data_dir()?.join("codebeat.db"))
    }
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(StdDuration::from_secs(10))?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > 8 {
            bail!("DATABASE_TOO_NEW: 请升级应用，不能降级此数据库");
        }
        if version == 0 {
            conn.execute_batch(include_str!("../../upgrades/001_initial.sql"))?;
        }
        if version < 2 {
            conn.execute_batch(include_str!("../../upgrades/002_pretrigger.sql"))?;
        }
        if version < 3 {
            conn.execute_batch(include_str!("../../upgrades/003_notification_inbox.sql"))?;
        }
        if version < 4 {
            conn.execute_batch(include_str!("../../upgrades/004_awo.sql"))?;
        }
        if version < 5 {
            // 旧版测试夹具可能只重建收件箱表，而保留后续版本才有的表。
            let has_topic: bool = conn
                .prepare("PRAGMA table_info(notification_inbox)")?
                .query_map([], |r| r.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()?
                .iter()
                .any(|name| name == "topic");
            if !has_topic {
                conn.execute_batch(include_str!("../../upgrades/005_notification_topics.sql"))?;
            } else {
                conn.execute_batch("PRAGMA user_version=5;")?;
            }
        }
        if version < 6 {
            conn.execute_batch(include_str!("../../upgrades/006_coding_allocation.sql"))?;
        }
        if version < 7 {
            conn.execute_batch(include_str!("../../upgrades/007_quota_notifications.sql"))?;
        }
        if version < 8 {
            conn.execute_batch(include_str!("../../upgrades/008_daily_allocation.sql"))?;
        }
        conn.execute(
            "INSERT OR IGNORE INTO app_settings VALUES('settings',?1)",
            [serde_json::to_string(&Settings::default())?],
        )?;
        Ok(Self {
            conn,
            directory: path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf(),
        })
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn pretrigger_runs(&self) -> Result<Vec<pretrigger::ProbeRecord>> {
        let mut stmt = self
            .conn
            .prepare("SELECT data_json FROM pretrigger_runs ORDER BY started_at DESC LIMIT 100")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    pub fn claim_pretrigger(
        &mut self,
        provider: ProviderId,
        slot: &str,
        now: chrono::DateTime<Utc>,
    ) -> Result<bool> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let settings: Option<String> = tx
            .query_row(
                "SELECT value_json FROM app_settings WHERE key='settings'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let s: Settings = settings
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_default();
        if !s.pretrigger_enabled
            || s.paused
            || s.paused_providers.contains(&provider)
            || !s.pretrigger.providers.contains(&provider)
        {
            return Ok(false);
        }
        let busy:i64=tx.query_row("SELECT (SELECT count(*) FROM tasks WHERE state IN ('claimed','running','cancelling')) + (SELECT count(*) FROM pretrigger_runs WHERE state='running' AND started_at>?1)",[(now-Duration::minutes(5)).to_rfc3339()],|r|r.get(0))?;
        if busy > 0 {
            return Ok(false);
        }
        let old: Option<String> = tx
            .query_row(
                "SELECT data_json FROM pretrigger_runs WHERE provider_id=?1 AND slot=?2",
                params![provider.as_str(), slot],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(old) = old {
            let old: pretrigger::ProbeRecord = serde_json::from_str(&old)?;
            if old.state != "waiting" || old.next_check_at.is_none_or(|t| t > now) {
                return Ok(false);
            }
        }
        let r = pretrigger::ProbeRecord {
            provider,
            slot: slot.into(),
            state: "running".into(),
            message: "正在检查并预启动额度窗口".into(),
            attempted_at: now,
            finished_at: None,
            next_check_at: None,
            before: None,
            after: None,
            tokens: None,
            request_sent: false,
            sent_at: None,
        };
        tx.execute("INSERT INTO pretrigger_runs VALUES(?1,?2,'running',?3,?4) ON CONFLICT(provider_id,slot) DO UPDATE SET state='running',data_json=excluded.data_json,started_at=excluded.started_at",params![provider.as_str(),slot,serde_json::to_string(&r)?,now.to_rfc3339()])?;
        tx.execute(
            "DELETE FROM pretrigger_runs WHERE started_at<?1",
            [(now - Duration::days(30)).to_rfc3339()],
        )?;
        tx.commit()?;
        Ok(true)
    }
    pub fn finish_pretrigger(&self, record: &pretrigger::ProbeRecord) -> Result<()> {
        self.conn.execute("UPDATE pretrigger_runs SET state=?1,data_json=?2 WHERE provider_id=?3 AND slot=?4 AND state='running'",params![record.state,serde_json::to_string(record)?,record.provider.as_str(),record.slot])?;
        Ok(())
    }
    pub fn settings(&self) -> Result<Settings> {
        let s: Option<String> = self
            .conn
            .query_row(
                "SELECT value_json FROM app_settings WHERE key='settings'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let mut settings: Settings = s
            .map(|v| serde_json::from_str(&v))
            .transpose()?
            .unwrap_or_default();
        // 15 分钟选项已改为 10 分钟，这里兼容旧安装中保存的值。
        if settings.connection_interval_minutes == 15 {
            settings.connection_interval_minutes = 10;
        }
        Ok(settings)
    }
    pub fn save_settings(&self, s: &Settings) -> Result<()> {
        validate_settings(s)?;
        self.conn.execute("INSERT INTO app_settings VALUES('settings',?1) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json",[serde_json::to_string(s)?])?;
        self.conn.execute(
            "DELETE FROM awo_runtime WHERE provider_id NOT IN (SELECT value FROM json_each(?1))",
            [serde_json::to_string(&s.pretrigger.providers)?],
        )?;
        for &provider in &s.pretrigger.providers {
            // 夏令时歧义只会禁用执行，不能让设置保存出现部分失败。
            let _ =
                self.replan_provider(provider, self.latest_usage(provider)?.as_ref(), Utc::now());
        }
        Ok(())
    }
    pub fn insert_task(&self, request: CreateTaskRequest) -> Result<Task> {
        let t = Task {
            id: Uuid::new_v4().to_string(),
            request,
            state: "pending_registration".into(),
            message: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        self.conn.execute(
            "INSERT INTO tasks VALUES(?1,?2,?3,NULL,?4,?4)",
            params![
                t.id,
                serde_json::to_string(&t.request)?,
                t.state,
                t.created_at.to_rfc3339()
            ],
        )?;
        Ok(t)
    }
    pub fn tasks(&self) -> Result<Vec<Task>> {
        let mut stmt=self.conn.prepare("SELECT id,request_json,state,message,created_at,updated_at FROM tasks ORDER BY json_extract(request_json,'$.scheduled_at_utc')")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })?;
        rows.map(|r| {
            let (id, json, state, message, c, u) = r?;
            Ok(Task {
                id,
                request: serde_json::from_str(&json)?,
                state,
                message,
                created_at: c.parse()?,
                updated_at: u.parse()?,
            })
        })
        .collect()
    }
    pub fn task(&self, id: &str) -> Result<Task> {
        self.tasks()?
            .into_iter()
            .find(|t| t.id == id)
            .context("TASK_NOT_FOUND")
    }
    pub fn transition(
        &self,
        id: &str,
        from: &[&str],
        to: &str,
        message: Option<&str>,
    ) -> Result<()> {
        let t = self.task(id)?;
        if !from.contains(&t.state.as_str()) {
            bail!("INVALID_STATE: {}", t.state);
        }
        if self.conn.execute(
            "UPDATE tasks SET state=?1,message=?2,updated_at=?3 WHERE id=?4 AND state=?5",
            params![to, message, Utc::now().to_rfc3339(), id, t.state],
        )? != 1
        {
            bail!("STATE_CONFLICT");
        }
        Ok(())
    }
    pub fn replace_request(&self, id: &str, r: &CreateTaskRequest) -> Result<()> {
        if self.conn.execute("UPDATE tasks SET request_json=?1,updated_at=?2 WHERE id=?3 AND state='pending_registration'",params![serde_json::to_string(r)?,Utc::now().to_rfc3339(),id])?!=1 { bail!("STATE_CONFLICT"); }
        Ok(())
    }
    pub fn claim(&mut self, id: &str, manual: bool) -> Result<Option<RunRecord>> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let settings: Option<String> = tx
            .query_row(
                "SELECT value_json FROM app_settings WHERE key='settings'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let settings = settings
            .map(|v| serde_json::from_str::<Settings>(&v))
            .transpose()?
            .unwrap_or_default();
        if settings.paused {
            return Ok(None);
        }
        let active: i64 = tx.query_row(
            "SELECT count(*) FROM tasks WHERE state IN ('claimed','running')",
            [],
            |r| r.get(0),
        )?;
        if active > 0 {
            return Ok(None);
        }
        let row: Option<(String, String)> = tx
            .query_row(
                "SELECT request_json,state FROM tasks WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((json, state)) = row else {
            bail!("TASK_NOT_FOUND");
        };
        if state != "scheduled" {
            return Ok(None);
        }
        let request: CreateTaskRequest = serde_json::from_str(&json)?;
        if settings
            .paused_providers
            .contains(&request.conversation.provider)
        {
            return Ok(None);
        }
        if !manual && request.scheduled_at_utc > Utc::now() {
            return Ok(None);
        }
        if !manual {
            let next = missed_state(&request, Utc::now());
            if next != "scheduled" {
                tx.execute(
                    "UPDATE tasks SET state=?1,updated_at=?2 WHERE id=?3",
                    params![next, Utc::now().to_rfc3339(), id],
                )?;
                tx.commit()?;
                return Ok(None);
            }
        }
        let now = Utc::now();
        tx.execute(
            "UPDATE tasks SET state='claimed',updated_at=?1 WHERE id=?2 AND state='scheduled'",
            params![now.to_rfc3339(), id],
        )?;
        let attempt: i64 = tx.query_row(
            "SELECT COALESCE(MAX(attempt),0)+1 FROM task_runs WHERE task_id=?1",
            [id],
            |r| r.get(0),
        )?;
        let run = RunRecord {
            id: Uuid::new_v4().to_string(),
            task_id: id.into(),
            attempt,
            state: "claimed".into(),
            started_at: now,
            finished_at: None,
            exit_code: None,
            error_code: None,
            error_message: None,
            log_path: None,
            pid: Some(std::process::id()),
        };
        tx.execute("INSERT INTO task_runs(id,task_id,attempt,state,started_at,pid,heartbeat_at) VALUES(?1,?2,?3,'claimed',?4,?5,?4)",params![run.id,id,attempt,now.to_rfc3339(),run.pid])?;
        tx.commit()?;
        Ok(Some(run))
    }
    pub fn start_run(&self, run: &str, task: &str, log: &str, pid: u32) -> Result<()> {
        self.conn.execute(
            "UPDATE task_runs SET state='running',log_path=?1,pid=?2 WHERE id=?3",
            params![log, pid, run],
        )?;
        self.transition(task, &["claimed"], "running", None)
    }
    pub fn heartbeat(&self, run: &str) -> Result<bool> {
        self.conn.execute(
            "UPDATE task_runs SET heartbeat_at=?1 WHERE id=?2",
            params![Utc::now().to_rfc3339(), run],
        )?;
        let state: String = self.conn.query_row(
            "SELECT t.state FROM tasks t JOIN task_runs r ON r.task_id=t.id WHERE r.id=?1",
            [run],
            |r| r.get(0),
        )?;
        Ok(state == "cancelling")
    }
    pub fn finish(
        &mut self,
        run: &str,
        state: &str,
        exit: Option<i32>,
        code: Option<&str>,
        message: Option<&str>,
    ) -> Result<()> {
        let tx = self.conn.transaction()?;
        let current: String =
            tx.query_row("SELECT state FROM task_runs WHERE id=?1", [run], |r| {
                r.get(0)
            })?;
        if !["claimed", "running"].contains(&current.as_str()) {
            return Ok(());
        }
        let task: String =
            tx.query_row("SELECT task_id FROM task_runs WHERE id=?1", [run], |r| {
                r.get(0)
            })?;
        let now = Utc::now().to_rfc3339();
        tx.execute("UPDATE task_runs SET state=?1,finished_at=?2,exit_code=?3,error_code=?4,error_message=?5 WHERE id=?6",params![state,now,exit,code,message,run])?;
        tx.execute(
            "UPDATE tasks SET state=?1,message=?2,updated_at=?3 WHERE id=?4",
            params![state, message, now, task],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn recover_stale(&self) -> Result<usize> {
        for mut r in self.pretrigger_runs()? {
            if r.state == "running" && r.attempted_at < Utc::now() - Duration::minutes(5) {
                r.state = "uncertain".into();
                r.message = "上次执行中断，结果未知；本轮不重复发送".into();
                r.finished_at = Some(Utc::now());
                self.finish_pretrigger(&r)?;
            }
        }
        let cutoff = (Utc::now() - Duration::minutes(3)).to_rfc3339();
        let count=self.conn.execute("UPDATE tasks SET state='needs_review',message='执行器心跳失联，请审查后手动重试' WHERE state IN ('claimed','running','cancelling') AND id IN (SELECT task_id FROM task_runs WHERE finished_at IS NULL AND heartbeat_at<?1)",[&cutoff])?;
        self.conn.execute("UPDATE task_runs SET state='needs_review',finished_at=?1,error_code='RUNNER_LOST' WHERE finished_at IS NULL AND heartbeat_at<?2",params![Utc::now().to_rfc3339(),cutoff])?;
        Ok(count)
    }
    pub fn runs(&self) -> Result<Vec<RunRecord>> {
        let mut s=self.conn.prepare("SELECT id,task_id,attempt,state,started_at,finished_at,exit_code,error_code,error_message,log_path,pid FROM task_runs ORDER BY started_at DESC LIMIT 500")?;
        let rows = s.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
                r.get(9)?,
                r.get(10)?,
            ))
        })?;
        rows.map(|r| {
            let (
                id,
                task_id,
                attempt,
                state,
                start,
                end,
                exit_code,
                error_code,
                error_message,
                log_path,
                pid,
            ) = r?;
            Ok(RunRecord {
                id,
                task_id,
                attempt,
                state,
                started_at: start.parse()?,
                finished_at: end.map(|s| s.parse()).transpose()?,
                exit_code,
                error_code,
                error_message,
                log_path,
                pid,
            })
        })
        .collect()
    }
    pub fn notify_once(&self, key: &str) -> Result<bool> {
        Ok(self.conn.execute(
            "INSERT OR IGNORE INTO notification_dedup VALUES(?1,?2)",
            params![key, Utc::now().to_rfc3339()],
        )? == 1)
    }
    pub fn undo_notification(&self, key: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM notification_dedup WHERE dedup_key=?1", [key])?;
        Ok(())
    }
    pub fn cache_health(&self, p: &ProviderHealth) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO providers VALUES(?1,?2,?3)",
            params![
                p.id.as_str(),
                serde_json::to_string(p)?,
                Utc::now().to_rfc3339()
            ],
        )?;
        Ok(())
    }
    pub fn cache_usage(&self, p: &UsageReport) -> Result<()> {
        // 桌面轮询与独立 runner 共用事务，只有首个观察到变化的进程生成消息。
        let tx = rusqlite::Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let old_account = self
            .latest_successful_usage(p.provider)?
            .and_then(|r| r.reset_credits.and_then(|c| c.account_key));
        let new_account = p
            .reset_credits
            .as_ref()
            .and_then(|c| c.account_key.as_ref());
        if old_account
            .as_ref()
            .zip(new_account)
            .is_some_and(|(a, b)| a != b)
        {
            self.conn.execute(
                "DELETE FROM awo_runtime WHERE provider_id=?1",
                [p.provider.as_str()],
            )?;
        }
        let notice = self.observe_quota_reset(p)?;
        let now = Utc::now();
        let own_probe:i64=self.conn.query_row("SELECT count(*) FROM pretrigger_runs WHERE provider_id=?1 AND state='running' AND json_extract(data_json,'$.request_sent')=1",[p.provider.as_str()],|r|r.get(0))?;
        if let Some(window) = pretrigger::fresh_window(p, now).filter(|_| own_probe == 0) {
            let old = self.latest_usage(p.provider)?;
            // 只统计观察到的增长，不包括打开应用或首次看到非零计数。
            // 不要据此推断精确的请求时间戳。
            let changed = old
                .as_ref()
                .filter(|r| r.status == "available")
                .and_then(|r| {
                    r.windows.iter().find(|w| {
                        w.duration_minutes == Some(300)
                            && w.reliability == "official"
                            && now - w.observed_at_utc < Duration::minutes(65)
                    })
                })
                .is_some_and(|w| {
                    w.used_percent < window.used_percent || w.resets_at_utc != window.resets_at_utc
                });
            if changed && window.used_percent.is_some_and(|u| u > 0.0) {
                self.conn.execute(
                    "INSERT OR IGNORE INTO awo_activity VALUES(?1,?2)",
                    params![p.provider.as_str(), window.observed_at_utc.to_rfc3339()],
                )?;
            }
            self.conn.execute(
                "DELETE FROM awo_activity WHERE observed_at<?1",
                [(now - Duration::days(30)).to_rfc3339()],
            )?;
        }
        self.conn.execute(
            "INSERT INTO usage_snapshots VALUES(?1,?2,?3,?4)",
            params![
                Uuid::new_v4().to_string(),
                p.provider.as_str(),
                serde_json::to_string(p)?,
                Utc::now().to_rfc3339()
            ],
        )?;
        self.conn.execute(
            "DELETE FROM usage_snapshots WHERE observed_at_utc<?1",
            [(Utc::now() - Duration::days(30)).to_rfc3339()],
        )?;
        if self.settings()?.pretrigger.providers.contains(&p.provider) {
            self.replan_provider(p.provider, Some(p), now)?;
        } else {
            self.conn.execute(
                "DELETE FROM awo_runtime WHERE provider_id=?1",
                [p.provider.as_str()],
            )?;
        }
        tx.commit()?;
        if let Some(notice) = notice {
            self.show_quota_notice(&notice);
        }
        Ok(())
    }
    pub fn latest_usage(&self, id: ProviderId) -> Result<Option<UsageReport>> {
        let s:Option<String>=self.conn.query_row("SELECT data_json FROM usage_snapshots WHERE provider_id=?1 ORDER BY observed_at_utc DESC LIMIT 1",[id.as_str()],|r|r.get(0)).optional()?;
        s.map(|v| Ok(serde_json::from_str(&v)?)).transpose()
    }
    /// 仅用于界面展示兜底。读取失败时仍以最新的原始快照为准，
    /// 以免优化器和自动执行把这个缓存误认为新数据。
    pub fn latest_successful_usage(&self, id: ProviderId) -> Result<Option<UsageReport>> {
        let value: Option<String> = self.conn.query_row(
            "SELECT data_json FROM usage_snapshots WHERE provider_id=?1 AND json_extract(data_json,'$.status')='available' AND json_array_length(data_json,'$.windows')>0 ORDER BY observed_at_utc DESC LIMIT 1",
            [id.as_str()], |row| row.get(0),
        ).optional()?;
        value
            .map(|json| Ok(serde_json::from_str(&json)?))
            .transpose()
    }
    pub fn monitoring(&self, days: i64) -> Result<serde_json::Value> {
        let cutoff = (Utc::now() - Duration::days(days.clamp(1, 30))).to_rfc3339();
        let mut stmt=self.conn.prepare("SELECT data_json,observed_at_utc FROM usage_snapshots WHERE observed_at_utc>=?1 ORDER BY observed_at_utc DESC LIMIT 10000")?;
        let rows = stmt.query_map([&cutoff], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let snapshots:Vec<serde_json::Value>=rows.map(|r|{let(data,at)=r?;Ok(serde_json::json!({"report":serde_json::from_str::<serde_json::Value>(&data)?,"observed_at":at}))}).collect::<Result<_>>()?;
        let mut stmt=self.conn.prepare("SELECT dedup_key,emitted_at FROM notification_dedup WHERE emitted_at>=?1 ORDER BY emitted_at DESC LIMIT 1000")?;
        let notifications = stmt
            .query_map([cutoff], |r| {
                Ok(serde_json::json!({"key":r.get::<_,String>(0)?,"at":r.get::<_,String>(1)?}))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(serde_json::json!({"snapshots":snapshots,"notifications":notifications}))
    }
    pub fn prepare_next_occurrence(&self, id: &str) -> Result<Option<Task>> {
        let task = self.task(id)?;
        if !matches!(task.state.as_str(), "succeeded" | "expired") {
            return Ok(None);
        }
        let Some(next) = next_occurrence(&task.request, Utc::now())? else {
            return Ok(None);
        };
        let changed=self.conn.execute("UPDATE tasks SET request_json=?1,state='pending_registration',updated_at=?2 WHERE id=?3 AND state=?4",params![serde_json::to_string(&next)?,Utc::now().to_rfc3339(),id,task.state])?;
        if changed != 1 {
            return Ok(None);
        }
        Ok(Some(self.task(id)?))
    }
}

impl Database {
    pub fn add_notice(&self, notice: &InboxNotice) -> Result<()> {
        if notice.topic.as_ref().is_some_and(|key| {
            key.is_empty() || key.len() > 256 || key.chars().any(char::is_control)
        }) {
            bail!("INVALID_NOTIFICATION_TOPIC");
        }
        Uuid::parse_str(&notice.id)?;
        if !["success", "error", "info"].contains(&notice.kind.as_str())
            || !["tasks", "usage"].contains(&notice.page.as_str())
            || notice.title.trim().is_empty()
            || notice.title.len() > 512
            || notice.message.trim().is_empty()
            || notice.message.len() > 16384
        {
            bail!("INVALID_NOTIFICATION");
        }
        if let Some(id) = &notice.task_id {
            Uuid::parse_str(id)?;
        }
        let tx = rusqlite::Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let replaced: Option<String> = if let Some(topic) = &notice.topic {
            tx.query_row(
                "SELECT id FROM notification_inbox WHERE topic=?1",
                [topic],
                |r| r.get(0),
            )
            .optional()?
        } else {
            None
        };
        tx.execute(
            "INSERT INTO notification_inbox(id,data_json,created_at,topic) VALUES(?1,?2,?3,?4) ON CONFLICT(topic) WHERE topic IS NOT NULL DO UPDATE SET id=excluded.id,data_json=excluded.data_json,acknowledged=0,created_at=CASE WHEN notification_inbox.acknowledged=0 THEN notification_inbox.created_at ELSE excluded.created_at END ON CONFLICT(id) DO NOTHING",
            params![
                notice.id,
                serde_json::to_string(notice)?,
                notice.at.to_rfc3339()
                ,notice.topic
            ],
        )?;
        tx.execute(
            "DELETE FROM notification_inbox WHERE acknowledged=1 AND created_at<?1",
            [(Utc::now() - Duration::days(30)).to_rfc3339()],
        )?;
        tx.commit()?;
        if let Some(old) = replaced.filter(|id| id != &notice.id) {
            let acks = self.directory.join("notification-acks");
            if std::fs::create_dir_all(&acks).is_ok() {
                let _ = std::fs::write(acks.join(format!("{old}.dismiss")), b"");
            }
        }
        Ok(())
    }
    pub fn pending_notices(&self) -> Result<Vec<InboxNotice>> {
        let mut stmt=self.conn.prepare("SELECT data_json FROM notification_inbox WHERE acknowledged=0 ORDER BY created_at DESC,rowid DESC LIMIT 200")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    pub fn acknowledge_notices(&mut self, ids: &[String]) -> Result<()> {
        if ids.len() > 200 {
            bail!("INVALID_NOTIFICATION");
        }
        for id in ids {
            Uuid::parse_str(id)?;
        }
        let tx = self.conn.transaction()?;
        for id in ids {
            tx.execute(
                "UPDATE notification_inbox SET acknowledged=1 WHERE id=?1",
                [id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn acknowledge_all_notices(&mut self) -> Result<Vec<String>> {
        let tx = self.conn.transaction()?;
        let ids = {
            let mut statement =
                tx.prepare("SELECT id FROM notification_inbox WHERE acknowledged=0")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        tx.execute(
            "UPDATE notification_inbox SET acknowledged=1 WHERE acknowledged=0",
            [],
        )?;
        tx.commit()?;
        Ok(ids)
    }
}
