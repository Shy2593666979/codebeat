//! 额度变化与重置来源通知。失败观测不覆盖基线，卡片过期不等于兑换成功。
use super::*;
use chrono::{DateTime, Utc};

fn observed(report: &UsageReport) -> Option<DateTime<Utc>> {
    report
        .windows
        .iter()
        .filter(|w| w.reliability == "official")
        .map(|w| w.observed_at_utc)
        .max()
}
fn account(report: &UsageReport) -> Option<&str> {
    report
        .reset_credits
        .as_ref()?
        .account_key
        .as_deref()
        .filter(|s| !s.is_empty())
}
fn reset_message(
    old: &UsageReport,
    new: &UsageReport,
    card_before: Option<&UsageReport>,
) -> Option<(&'static str, String)> {
    let (before_at, after_at) = (observed(old)?, observed(new)?);
    if after_at <= before_at || account(old)? != account(new)? {
        return None;
    }
    let mut restored = Vec::new();
    let mut early = false;
    for n in &new.windows {
        if n.reliability != "official" || !matches!(n.duration_minutes, Some(300 | 10080)) {
            continue;
        }
        let Some(o) = old.windows.iter().find(|o| {
            o.reliability == "official"
                && o.duration_minutes == n.duration_minutes
                && o.label == n.label
        }) else {
            continue;
        };
        let (Some(a), Some(b)) = (o.used_percent, n.used_percent) else {
            continue;
        };
        let new_cycle = a > 0.0
            && b > 0.0
            && o.resets_at_utc.is_some_and(|reset| {
                reset <= n.observed_at_utc && n.resets_at_utc.is_some_and(|next| next > reset)
            });
        if !a.is_finite()
            || !b.is_finite()
            || !(0.0..=100.0).contains(&a)
            || !(0.0..=100.0).contains(&b)
            || n.observed_at_utc <= o.observed_at_utc
            || !(new_cycle || (a - b >= 1.0 && (b == 0.0 || n.resets_at_utc > o.resets_at_utc)))
        {
            continue;
        }
        restored.push(if n.duration_minutes == Some(300) {
            "五小时额度"
        } else {
            "周额度"
        });
        early |= o.resets_at_utc.is_some_and(|r| r > after_at);
    }
    if restored.is_empty() {
        return None;
    }
    let scope = restored.join("、");
    let card_before = card_before.unwrap_or(old);
    let card_observed = observed(card_before)?;
    let (old_cards, new_cards) = (
        card_before.reset_credits.as_ref()?,
        new.reset_credits.as_ref()?,
    );
    let decrease = old_cards
        .available_count
        .saturating_sub(new_cards.available_count);
    if decrease > 0 {
        let expired = old_cards.credits.as_ref().map_or(0, |cards| {
            cards
                .iter()
                .filter(|c| {
                    c.status == "available"
                        && c.expiry_known
                        && c.expires_at
                            .is_some_and(|at| card_observed < at && at <= after_at)
                })
                .count() as u64
        });
        if decrease > expired {
            let complete = old_cards
                .credits
                .as_ref()
                .is_some_and(|v| v.len() as u64 == old_cards.available_count)
                && new_cards
                    .credits
                    .as_ref()
                    .is_some_and(|v| v.len() as u64 == new_cards.available_count);
            if complete {
                let missing_live = old_cards
                    .credits
                    .as_ref()?
                    .iter()
                    .filter(|c| {
                        c.status == "available"
                            && c.expiry_known
                            && c.expires_at.is_none_or(|at| at > after_at)
                            && !new_cards
                                .credits
                                .as_ref()
                                .unwrap()
                                .iter()
                                .any(|n| n.id == c.id)
                    })
                    .count() as u64;
                if missing_live >= decrease - expired {
                    return Some((
                        "Codex 重置卡重置提醒",
                        format!("{scope}已恢复；检测到未过期重置卡减少，判断为使用重置卡重置。"),
                    ));
                }
            }
            return Some(("Codex 额度已恢复", format!("{scope}已恢复，可用重置卡减少了 {decrease} 张；卡片明细或过期信息不完整，无法确认是使用还是到期。")));
        }
        return Some((
            "Codex 额度已恢复",
            format!("{scope}已恢复；减少的重置卡已到期，无法仅根据数量变化确认重置来源。"),
        ));
    }
    Some((
        if early {
            "Codex 官方额度已重置"
        } else {
            "Codex 额度窗口已恢复"
        },
        format!("{scope}已恢复，未观察到可用重置卡减少。"),
    ))
}

impl Database {
    /// 由 cache_usage 的写事务调用，基线、去重记录和收件箱同步提交。
    pub(super) fn observe_quota_reset(&self, report: &UsageReport) -> Result<Option<InboxNotice>> {
        if report.provider != ProviderId::Codex
            || report.status != "available"
            || observed(report).is_none()
            || account(report).is_none()
        {
            return Ok(None);
        }
        let saved: Option<String> = self
            .conn
            .query_row(
                "SELECT data_json FROM quota_notice_state WHERE provider_id='codex'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        let old = saved
            .map(|s| serde_json::from_str::<UsageReport>(&s))
            .transpose()?
            .or(self.latest_successful_usage(ProviderId::Codex)?);
        if old
            .as_ref()
            .is_some_and(|old| observed(old) >= observed(report))
        {
            return Ok(None);
        }
        let account_key = account(report).unwrap();
        let correlation =
            Duration::minutes((self.settings()?.connection_interval_minutes * 2 + 5).max(30));
        let pending: Option<String> = self.conn.query_row("SELECT before_json FROM quota_credit_changes WHERE account_key=?1 AND observed_at>=?2", params![account_key,(observed(report).unwrap()-correlation).to_rfc3339()], |r| r.get(0)).optional()?;
        let card_before = pending
            .map(|s| serde_json::from_str::<UsageReport>(&s))
            .transpose()?;
        let event = old
            .as_ref()
            .and_then(|old| reset_message(old, report, card_before.as_ref()));
        self.conn.execute(
            "DELETE FROM quota_credit_changes WHERE observed_at<?1",
            [(observed(report).unwrap() - correlation).to_rfc3339()],
        )?;
        if event.is_none() {
            if let Some(before) = old.as_ref().filter(|before| {
                account(before) == Some(account_key)
                    && before
                        .reset_credits
                        .as_ref()
                        .zip(report.reset_credits.as_ref())
                        .is_some_and(|(a, b)| a.available_count > b.available_count)
            }) {
                self.conn.execute(
                    "INSERT OR IGNORE INTO quota_credit_changes VALUES(?1,?2,?3)",
                    params![
                        account_key,
                        serde_json::to_string(before)?,
                        observed(report).unwrap().to_rfc3339()
                    ],
                )?;
            }
        } else {
            self.conn.execute(
                "DELETE FROM quota_credit_changes WHERE account_key=?1",
                [account_key],
            )?;
        }
        self.conn.execute("INSERT INTO quota_notice_state VALUES('codex',?1) ON CONFLICT(provider_id) DO UPDATE SET data_json=excluded.data_json", [serde_json::to_string(report)?])?;
        let at = observed(report).unwrap().to_rfc3339();
        let consumed: bool = self.conn.query_row("SELECT EXISTS(SELECT 1 FROM quota_reset_consumptions WHERE account_key=?1 AND reconciled=0 AND consumed_at>=?2 AND consumed_at<=?3)", params![account_key,(Utc::now()-correlation).to_rfc3339(),at], |r| r.get(0))?;
        if consumed {
            self.conn.execute("UPDATE quota_reset_consumptions SET reconciled=1 WHERE account_key=?1 AND reconciled=0 AND consumed_at<=?2", params![account_key,at])?;
            return Ok(None);
        }
        let Some((title, message)) = event else {
            return Ok(None);
        };
        let key = format!(
            "quota-reset:{account_key}:{}",
            observed(old.as_ref().unwrap()).unwrap().timestamp_millis()
        );
        self.persist_quota_notice(&key, title, &message)
    }

    fn persist_quota_notice(
        &self,
        key: &str,
        title: &str,
        message: &str,
    ) -> Result<Option<InboxNotice>> {
        if !self.settings()?.notifications || !self.notify_once(key)? {
            return Ok(None);
        }
        let notice = InboxNotice {
            id: Uuid::new_v4().to_string(),
            title: title.into(),
            message: message.into(),
            kind: "success".into(),
            task_id: None,
            page: "usage".into(),
            topic: Some(format!("system:{key}")),
            at: Utc::now(),
        };
        self.conn.execute(
            "INSERT INTO notification_inbox(id,data_json,created_at,topic) VALUES(?1,?2,?3,?4)",
            params![
                notice.id,
                serde_json::to_string(&notice)?,
                notice.at.to_rfc3339(),
                notice.topic
            ],
        )?;
        Ok(Some(notice))
    }

    /// 只有官方兑换结果确认成功时调用；重试同一幂等键不会再发一条。
    pub fn record_reset_consumption(&self, attempt: &str, account: &str) -> Result<()> {
        Uuid::parse_str(attempt)?;
        if account.is_empty() {
            bail!("RESET_ACCOUNT_CHANGED");
        }
        let tx = rusqlite::Transaction::new_unchecked(&self.conn, TransactionBehavior::Immediate)?;
        let inserted = self.conn.execute("INSERT OR IGNORE INTO quota_reset_consumptions(attempt_id,account_key,consumed_at) VALUES(?1,?2,?3)", params![attempt,account,Utc::now().to_rfc3339()])?;
        let notice = if inserted == 1 {
            self.persist_quota_notice(
                &format!("quota-card:{attempt}"),
                "Codex 重置卡已使用",
                "官方已确认重置卡兑换成功，额度已重置；最新用量将继续刷新。",
            )?
        } else {
            None
        };
        tx.commit()?;
        if let Some(notice) = notice {
            self.show_quota_notice(&notice);
        }
        Ok(())
    }
    pub(super) fn show_quota_notice(&self, notice: &InboxNotice) {
        // 隔离数据库测试只验证收件箱，不操作用户桌面。
        if data_dir().is_ok_and(|dir| dir == self.directory) {
            let card = crate::platform::DesktopNotification {
                title: &notice.title,
                body: &notice.message,
                kind: &notice.kind,
                task_id: None,
                page: "usage",
            };
            if let Err(error) = crate::platform::show_stored_notice(
                self,
                &card,
                self.settings()
                    .map(|s| s.notification_sound)
                    .unwrap_or(false),
                &notice.id,
            ) {
                eprintln!("QUOTA_NOTIFICATION: {error}");
            }
        }
    }
}
