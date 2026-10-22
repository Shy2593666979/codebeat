use crate::db::Database;
use crate::{pretrigger::*, *};
use anyhow::{bail, Context, Result};
use chrono::Utc;
use serde_json::Value;
use std::{process::Stdio, time::Duration};
use tokio::io::{AsyncWriteExt, BufReader};

pub const PROMPT: &str =
    "Reply with exactly OK. Do not use any tools, read files, or perform any other action.";
const MAX_REQUEST_ATTEMPTS: usize = 4;
pub async fn tick(db: &mut Database) -> Result<()> {
    let result = tick_inner(db).await;
    if let Err(error) = &result {
        decisions::log(
            db,
            None,
            &format!(
                "tick_error:{}",
                crate::providers::redact(&error.to_string())
            ),
            None,
            None,
        );
    }
    result
}
async fn tick_inner(db: &mut Database) -> Result<()> {
    let settings = db.settings()?;
    decisions::log(db, None, "tick", None, None);
    if !settings.pretrigger_enabled || settings.paused {
        decisions::log(db, None, "disabled_or_paused", None, None);
        return Ok(());
    }
    validate(&settings.pretrigger)?;
    for provider in &settings.pretrigger.providers {
        let now = Utc::now();
        let cached = db.latest_usage(*provider)?;
        let mut planned = db.replan_provider(*provider, cached.as_ref(), now)?;
        let check_due = scheduled_slot(&settings.pretrigger, &planned, now);
        let refresh_due = cached
            .as_ref()
            .and_then(|r| r.windows.iter().map(|w| w.observed_at_utc).max())
            .is_none_or(|at| {
                now - at >= chrono::Duration::minutes(settings.connection_interval_minutes)
            });
        if refresh_due || check_due.is_some() {
            let fresh = crate::providers::usage(*provider).await;
            db.cache_usage(&fresh)?;
            planned = db
                .runtime_pretrigger(*provider)?
                .context("PRETRIGGER_PLAN_MISSING")?;
            if check_due.is_some()
                && !planned.automatic_allowed
                && planned.idle_observed.is_some()
                && !planned.idle_confirmed
            {
                tokio::time::sleep(Duration::from_secs(2)).await;
                let second = crate::providers::usage(*provider).await;
                db.cache_usage(&second)?;
                planned = db
                    .runtime_pretrigger(*provider)?
                    .context("PRETRIGGER_PLAN_MISSING")?;
            }
        }
        if !settings.pretrigger.auto_trigger || settings.pretrigger.mode == "save" {
            decisions::log(
                db,
                Some(*provider),
                "automatic_disabled",
                Some(&planned),
                None,
            );
            continue;
        }
        let Some(slot) = due_for_plan(&settings.pretrigger, &planned, Utc::now()) else {
            decisions::log(
                db,
                Some(*provider),
                if scheduled_slot(&settings.pretrigger, &planned, Utc::now()).is_some() {
                    "quota_not_ready"
                } else {
                    "not_due"
                },
                Some(&planned),
                db.latest_usage(*provider)?.as_ref(),
            );
            continue;
        };
        if !db.claim_pretrigger(*provider, &slot.key, Utc::now())? {
            let existing = db
                .pretrigger_runs()?
                .into_iter()
                .find(|r| r.provider == *provider && r.slot == slot.key);
            let reason = existing
                .map(|r| format!("claim_declined:{}:next={:?}", r.state, r.next_check_at))
                .unwrap_or_else(|| "claim_declined:paused_or_busy".into());
            decisions::log(db, Some(*provider), &reason, Some(&planned), None);
            continue;
        }
        let mut record = ProbeRecord {
            provider: *provider,
            slot: slot.key.clone(),
            state: "running".into(),
            message: String::new(),
            attempted_at: Utc::now(),
            finished_at: None,
            next_check_at: None,
            before: None,
            after: None,
            tokens: None,
            request_sent: false,
            sent_at: None,
        };
        if let Err(error) = process(db, &mut record, slot.until).await {
            record.state = "failed".into();
            record.message = crate::providers::redact(&error.to_string());
        }
        record.finished_at = Some(Utc::now());
        db.finish_pretrigger(&record)?;
        db.replan_provider(
            *provider,
            record.after.as_ref().or(record.before.as_ref()),
            Utc::now(),
        )?;
        decisions::log(
            db,
            Some(*provider),
            &format!("{}:{}", record.state, record.message),
            db.runtime_pretrigger(*provider)?.as_ref(),
            record.before.as_ref(),
        );
        if matches!(
            record.state.as_str(),
            "confirmed" | "failed" | "sent_unconfirmed" | "cost_paused"
        ) {
            let title = if record.state == "confirmed" {
                "额度窗口预启动完成"
            } else {
                "额度窗口预启动提醒"
            };
            if db.settings()?.notifications {
                db.add_notice(&InboxNotice {
                    topic: Some(format!("system:awo:{}", provider.as_str())),
                    id: uuid::Uuid::new_v4().to_string(),
                    title: title.into(),
                    message: record.message.clone(),
                    kind: if record.state == "confirmed" {
                        "success"
                    } else {
                        "error"
                    }
                    .into(),
                    task_id: None,
                    page: "usage".into(),
                    at: Utc::now(),
                })?;
                let _ = crate::notifications::notify(
                    db,
                    &format!("awo:{}:{}", provider.as_str(), record.slot),
                    title,
                    &record.message,
                );
            }
        }
    }
    Ok(())
}
async fn process(db: &Database, r: &mut ProbeRecord, until: chrono::DateTime<Utc>) -> Result<()> {
    if let Some(sent) = db.pretrigger_runs()?.into_iter().find(|old| {
        old.provider == r.provider
            && matches!(old.state.as_str(), "confirmed" | "sent_unconfirmed")
            && old
                .next_check_at
                .unwrap_or(old.attempted_at + chrono::Duration::hours(5))
                + chrono::Duration::seconds(RESET_SETTLE_SECONDS)
                > Utc::now()
    }) {
        r.state = "waiting".into();
        r.next_check_at = Some(
            sent.next_check_at
                .unwrap_or(sent.attempted_at + chrono::Duration::hours(5))
                + chrono::Duration::seconds(RESET_SETTLE_SECONDS),
        );
        r.message = if sent.state == "confirmed" {
            "已确认的窗口仍在计时，等待恢复"
        } else {
            "请求已发送，按五小时间隔暂缓重复请求；窗口尚未确认"
        }
        .into();
        return Ok(());
    }
    let mut before = crate::providers::usage(r.provider).await;
    db.cache_usage(&before)?;
    r.before = Some(before.clone());
    let plan = db
        .runtime_pretrigger(r.provider)?
        .context("PRETRIGGER_PLAN_MISSING")?;
    if let Some(reset) = plan.actual_reset.filter(|end| {
        !plan.automatic_allowed
            && *end + chrono::Duration::seconds(RESET_SETTLE_SECONDS) > Utc::now()
    }) {
        r.state = "waiting".into();
        r.message = "已有有效五小时窗口，沿用官方恢复时间".into();
        r.next_check_at = Some(reset + chrono::Duration::seconds(RESET_SETTLE_SECONDS));
        return Ok(());
    }
    if exhausted(&before, Utc::now()) {
        r.state = "blocked".into();
        r.message = "官方额度不足，已跳过本次预触发".into();
        return Ok(());
    }
    if fresh_window(&before, Utc::now()).is_none() || r.provider == ProviderId::ClaudeCode {
        r.state = "waiting".into();
        r.message = "无法确认官方五小时额度或轻量触发能力，禁止无人值守请求".into();
        r.next_check_at = Some(Utc::now() + chrono::Duration::minutes(5));
        return Ok(());
    }
    if !plan.automatic_allowed {
        r.state = "waiting".into();
        r.message = "官方数据尚不能确认旧窗口已结束，跳过轻量请求".into();
        r.next_check_at = Some(Utc::now() + chrono::Duration::minutes(5));
        return Ok(());
    }
    let s = db.settings()?;
    if !s.pretrigger_enabled
        || !s.pretrigger.auto_trigger
        || s.pretrigger.mode == "save"
        || s.paused
        || s.paused_providers.contains(&r.provider)
        || !s.pretrigger.providers.contains(&r.provider)
        || Utc::now() >= until
        || db
            .runtime_pretrigger(r.provider)?
            .and_then(|p| due_for_plan(&s.pretrigger, &p, Utc::now()))
            .is_none_or(|slot| slot.key != r.slot)
    {
        r.state = "skipped".into();
        r.message = "计划已关闭、变更或工作时间已结束".into();
        return Ok(());
    }
    if db.upcoming_real_task(r.provider, Utc::now())? {
        r.state = "skipped".into();
        r.message = "即将执行真实任务，跳过额外轻量请求".into();
        return Ok(());
    }
    let health = crate::providers::detect(r.provider).await;
    if health.auth_status != "authenticated" {
        bail!("PRETRIGGER_AUTH: {}", health.message);
    }
    let exe = crate::providers::executable(r.provider)?;
    verify_cli(&exe, r.provider).await?;
    let workspace_root = db.directory().join("pretrigger-workspace");
    std::fs::create_dir_all(&workspace_root)?;
    let workspace = tempfile::tempdir_in(&workspace_root)?;
    let cwd = workspace.path();
    let latest = db.settings()?;
    if !latest.pretrigger_enabled
        || !latest.pretrigger.auto_trigger
        || latest.pretrigger.mode == "save"
        || latest.paused
        || latest.paused_providers.contains(&r.provider)
        || !latest.pretrigger.providers.contains(&r.provider)
    {
        r.state = "skipped".into();
        r.message = "预启动已暂停".into();
        return Ok(());
    }
    // 健康检查/帮助检查可能耗时数秒。在持久化发送预留之前再检查一次，
    // 包括期间出现的真实任务。
    before = crate::providers::usage(r.provider).await;
    db.cache_usage(&before)?;
    r.before = Some(before.clone());
    if db
        .runtime_pretrigger(r.provider)?
        .is_none_or(|p| !p.automatic_allowed)
        || exhausted(&before, Utc::now())
        || db.upcoming_real_task(r.provider, Utc::now())?
    {
        r.state = "skipped".into();
        r.message = "执行前状态已改变，取消额外轻量触发".into();
        return Ok(());
    }
    if !db.reserve_probe_request(r, Utc::now())? {
        r.state = "skipped".into();
        r.message = "该计划已尝试或近期已有请求，禁止重复消耗".into();
        return Ok(());
    }
    r.request_sent = true;
    r.sent_at = Some(Utc::now());
    db.persist_probe_progress(r)?;
    // 同一条已抢占的记录内执行首次请求和三次重试，不另建预留。
    // 每次失败后先核实官方窗口，避免请求实际成功却因连接中断重复消耗。
    let mut attempt = 0;
    let after = loop {
        attempt += 1;
        r.sent_at = Some(Utc::now());
        r.message = format!("正在执行轻量请求，第 {attempt}/{MAX_REQUEST_ATTEMPTS} 次");
        db.persist_probe_progress(r)?;
        decisions::log(
            db,
            Some(r.provider),
            &format!(
                "request_attempt:{attempt}/{MAX_REQUEST_ATTEMPTS}:{}",
                r.slot
            ),
            None,
            None,
        );
        let sent = send(&exe, r.provider, cwd).await;
        if let Ok(tokens) = &sent {
            r.tokens = tokens.clone();
        }
        let after = crate::providers::usage(r.provider).await;
        db.cache_usage(&after)?;
        r.after = Some(after.clone());
        if cost_anomaly(&before, &after) {
            let mut settings = db.settings()?;
            settings.pretrigger.auto_trigger = false;
            db.save_settings(&settings)?;
            r.state = "cost_paused".into();
            r.message = "触发额度消耗超过保护阈值，已暂停未来自动轻量触发".into();
            return Ok(());
        }
        match sent {
            Ok(_) => break after,
            Err(error) => {
                let message = crate::providers::redact(&error.to_string());
                decisions::log(
                    db,
                    Some(r.provider),
                    &format!("request_failed:{attempt}:{message}"),
                    None,
                    Some(&after),
                );
                if let Some(reset) = active_reset(&after, Utc::now()) {
                    r.state = "confirmed".into();
                    r.message = "请求未确认完成，但官方窗口已经开启，停止额外重试".into();
                    r.next_check_at = Some(reset);
                    return Ok(());
                }
                if attempt == MAX_REQUEST_ATTEMPTS {
                    let mut settings = db.settings()?;
                    settings.pretrigger.auto_trigger = false;
                    db.save_settings(&settings)?;
                    bail!("PRETRIGGER_RETRIES_EXHAUSTED: 首次执行及三次重试均失败；最后错误：{message}");
                }
                if exhausted(&after, Utc::now()) || fresh_window(&after, Utc::now()).is_none() {
                    r.state = "blocked".into();
                    r.message = "重试前无法确认可用额度，停止本轮请求".into();
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_secs(2_u64.pow(attempt as u32))).await;
                let settings = db.settings()?;
                if !settings.pretrigger_enabled
                    || !settings.pretrigger.auto_trigger
                    || settings.pretrigger.mode == "save"
                    || settings.paused
                    || settings.paused_providers.contains(&r.provider)
                    || !settings.pretrigger.providers.contains(&r.provider)
                    || Utc::now() >= until
                    || db.upcoming_real_task(r.provider, Utc::now())?
                {
                    r.state = "skipped".into();
                    r.message = "重试前计划已暂停、过期或有真实任务，停止本轮请求".into();
                    return Ok(());
                }
            }
        }
    };
    if let Some(reset) = active_reset(&after, Utc::now()) {
        r.state = "confirmed".into();
        r.message = format!("请求成功，官方五小时窗口恢复时间：{}", reset.to_rfc3339());
        r.next_check_at = Some(reset);
    } else {
        r.state = "sent_unconfirmed".into();
        r.next_check_at = Some(r.sent_at.unwrap_or(r.attempted_at) + chrono::Duration::hours(5));
        if fresh_window(&after, Utc::now()).is_some_and(|w| w.used_percent == Some(0.0)) {
            r.message =
                "极简请求已成功，用量仍显示 0%；已记录真实发送，下一轮至少等待五小时再加一分钟"
                    .into();
        } else {
            let mut settings = db.settings()?;
            settings.pretrigger.auto_trigger = false;
            db.save_settings(&settings)?;
            r.message = "极简请求已成功；无法读取官方额度，已暂停自动触发".into();
        }
    }
    Ok(())
}
async fn verify_cli(exe: &std::path::Path, provider: ProviderId) -> Result<()> {
    let args = if provider == ProviderId::Codex {
        vec!["exec", "--help"]
    } else {
        vec!["--help"]
    };
    let output = tokio::time::timeout(
        Duration::from_secs(15),
        crate::providers::command(exe)
            .args(args)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .context("PRETRIGGER_HELP_TIMEOUT")??;
    let help = String::from_utf8_lossy(&output.stdout);
    let required = if provider == ProviderId::Codex {
        vec![
            "--ephemeral",
            "--sandbox",
            "--json",
            "--ignore-user-config",
            "--ignore-rules",
        ]
    } else {
        vec![
            "--max-turns",
            "--tools",
            "--no-session-persistence",
            "--model",
        ]
    };
    if !output.status.success() || required.iter().any(|flag| !help.contains(flag)) {
        bail!("PRETRIGGER_UNSUPPORTED: 当前 CLI 不支持隔离轻量请求，已跳过");
    }
    Ok(())
}
async fn send(
    exe: &std::path::Path,
    provider: ProviderId,
    cwd: &std::path::Path,
) -> Result<Option<Value>> {
    let mut child = crate::providers::command(exe)
        .args(crate::providers::pretrigger_args(provider))
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context("PRETRIGGER_CLI_START")?;
    let _job = crate::platform::job::ProcessJob::assign(child.id().context("PID_MISSING")?)?;
    let result = tokio::time::timeout(Duration::from_secs(90), async {
        let mut stdin = child.stdin.take().context("STDIN_MISSING")?;
        stdin.write_all(PROMPT.as_bytes()).await?;
        stdin.shutdown().await?;
        drop(stdin);
        let stdout = child.stdout.take().context("STDOUT_MISSING")?;
        let stderr = child.stderr.take().context("STDERR_MISSING")?;
        let drain = tokio::spawn(async move {
            let mut r = BufReader::new(stderr);
            let mut bytes = 0;
            while let Some(line) = crate::providers::stream::next_line(&mut r, 1024 * 1024).await? {
                bytes += line.len();
                if bytes > 4 * 1024 * 1024 {
                    bail!("PRETRIGGER_OUTPUT_LIMIT");
                }
            }
            Ok::<_, anyhow::Error>(())
        });
        let mut reader = BufReader::new(stdout);
        let mut success = false;
        let mut failed = false;
        let mut bytes = 0;
        let mut tokens = None;
        while let Some(line) = crate::providers::stream::next_line(&mut reader, 1024 * 1024).await?
        {
            bytes += line.len();
            if bytes > 4 * 1024 * 1024 {
                bail!("PRETRIGGER_OUTPUT_LIMIT");
            }
            if let Ok(event) = serde_json::from_str::<Value>(&line) {
                if let Some(usage) = event.get("usage") {
                    tokens = Some(usage.clone());
                }
                match crate::providers::classify_event(&event) {
                    Some("succeeded") => success = true,
                    // Codex 重连也会发出 error；最终完成事件才决定请求成功。
                    Some("failed") if event["type"] == "error" => {}
                    Some("failed" | "blocked_approval") => failed = true,
                    _ => {}
                }
            }
        }
        let status = child.wait().await?;
        drain.await??;
        if !status.success() || !success || failed {
            bail!("PRETRIGGER_UNVERIFIED: 本次请求未收到成功完成事件");
        }
        Ok(tokens)
    })
    .await;
    match result {
        Ok(Ok(tokens)) => Ok(tokens),
        other => {
            crate::tasks::execution::stop_tree(&mut child).await;
            match other {
                Ok(Err(e)) => Err(e),
                _ => bail!("PRETRIGGER_TIMEOUT: 本次请求超时"),
            }
        }
    }
}
