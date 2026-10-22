//! 官方额度快照、活动观测与提供方状态缓存。
use super::*;
use entities::{
    awo_activity, awo_runtime, notification_dedup, pretrigger_runs, providers, usage_snapshots,
};

impl Database {
    pub fn cache_health(&self, provider: &ProviderHealth) -> Result<()> {
        providers::Entity::insert(providers::ActiveModel {
            id: Set(provider.id.as_str().into()),
            data_json: Set(serde_json::to_string(provider)?),
            last_checked_at: Set(Utc::now().to_rfc3339()),
        })
        .on_conflict(
            OnConflict::column(providers::Column::Id)
                .update_columns([
                    providers::Column::DataJson,
                    providers::Column::LastCheckedAt,
                ])
                .to_owned(),
        )
        .exec_without_returning(&self.conn)?;
        Ok(())
    }

    pub fn cache_usage(&self, report: &UsageReport) -> Result<()> {
        // 基线、收件箱、快照和新计划必须使用同一个事务连接。
        let notice = self.write(|db| {
            let old_account = db
                .latest_successful_usage(report.provider)?
                .and_then(|r| r.reset_credits.and_then(|credits| credits.account_key));
            let new_account = report
                .reset_credits
                .as_ref()
                .and_then(|c| c.account_key.as_ref());
            if old_account
                .as_ref()
                .zip(new_account)
                .is_some_and(|(a, b)| a != b)
            {
                awo_runtime::Entity::delete_by_id(report.provider.as_str()).exec(&db.conn)?;
            }
            let notice = db.observe_quota_reset(report)?;
            let now = Utc::now();
            let own_probe = pretrigger_runs::Entity::find()
                .filter(pretrigger_runs::Column::ProviderId.eq(report.provider.as_str()))
                .filter(pretrigger_runs::Column::State.eq("running"))
                .all(&db.conn)?
                .into_iter()
                .map(|row| serde_json::from_str::<allocation::ProbeRecord>(&row.data_json))
                .collect::<std::result::Result<Vec<_>, _>>()?
                .iter()
                .any(|r| r.request_sent);
            if let Some(window) = allocation::fresh_window(report, now).filter(|_| !own_probe) {
                let old = db.latest_usage(report.provider)?;
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
                        w.used_percent < window.used_percent
                            || w.resets_at_utc != window.resets_at_utc
                    });
                if changed && window.used_percent.is_some_and(|u| u > 0.0) {
                    awo_activity::Entity::insert(awo_activity::ActiveModel {
                        provider_id: Set(report.provider.as_str().into()),
                        observed_at: Set(window.observed_at_utc.to_rfc3339()),
                    })
                    .on_conflict(
                        OnConflict::columns([
                            awo_activity::Column::ProviderId,
                            awo_activity::Column::ObservedAt,
                        ])
                        .do_nothing()
                        .to_owned(),
                    )
                    .exec_without_returning(&db.conn)?;
                }
                awo_activity::Entity::delete_many()
                    .filter(
                        awo_activity::Column::ObservedAt
                            .lt((now - Duration::days(30)).to_rfc3339()),
                    )
                    .exec(&db.conn)?;
            }
            usage_snapshots::Entity::insert(usage_snapshots::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                provider_id: Set(report.provider.as_str().into()),
                data_json: Set(serde_json::to_string(report)?),
                observed_at_utc: Set(now.to_rfc3339()),
            })
            .exec_without_returning(&db.conn)?;
            usage_snapshots::Entity::delete_many()
                .filter(
                    usage_snapshots::Column::ObservedAtUtc
                        .lt((now - Duration::days(30)).to_rfc3339()),
                )
                .exec(&db.conn)?;
            if db
                .settings()?
                .pretrigger
                .providers
                .contains(&report.provider)
            {
                db.replan_provider(report.provider, Some(report), now)?;
            } else {
                awo_runtime::Entity::delete_by_id(report.provider.as_str()).exec(&db.conn)?;
            }
            Ok(notice)
        })?;
        if let Some(notice) = notice {
            self.show_quota_notice(&notice);
        }
        Ok(())
    }

    pub fn latest_usage(&self, provider: ProviderId) -> Result<Option<UsageReport>> {
        usage_snapshots::Entity::find()
            .filter(usage_snapshots::Column::ProviderId.eq(provider.as_str()))
            .order_by_desc(usage_snapshots::Column::ObservedAtUtc)
            .order_by_desc(Expr::cust("rowid"))
            .one(&self.conn)?
            .map(|row| Ok(serde_json::from_str(&row.data_json)?))
            .transpose()
    }

    /// 只用于展示兜底，不能把失败前的快照冒充为新额度。
    pub fn latest_successful_usage(&self, provider: ProviderId) -> Result<Option<UsageReport>> {
        usage_snapshots::Entity::find().filter(usage_snapshots::Column::ProviderId.eq(provider.as_str()))
            // 窗口是现有 JSON 文档，保留 SQLite JSON 谓词以避免全量反序列化历史。
            .filter(Expr::cust("json_extract(data_json,'$.status')='available' AND json_array_length(data_json,'$.windows')>0"))
            .order_by_desc(usage_snapshots::Column::ObservedAtUtc).order_by_desc(Expr::cust("rowid")).one(&self.conn)?
            .map(|row| Ok(serde_json::from_str(&row.data_json)?)).transpose()
    }

    pub fn monitoring(&self, days: i64) -> Result<serde_json::Value> {
        let cutoff = (Utc::now() - Duration::days(days.clamp(1, 30))).to_rfc3339();
        let snapshots = usage_snapshots::Entity::find().filter(usage_snapshots::Column::ObservedAtUtc.gte(&cutoff))
            .order_by_desc(usage_snapshots::Column::ObservedAtUtc).limit(10000).all(&self.conn)?
            .into_iter().map(|row| Ok(serde_json::json!({"report":serde_json::from_str::<serde_json::Value>(&row.data_json)?,"observed_at":row.observed_at_utc})))
            .collect::<Result<Vec<_>>>()?;
        let notifications: Vec<_> = notification_dedup::Entity::find()
            .filter(notification_dedup::Column::EmittedAt.gte(cutoff))
            .order_by_desc(notification_dedup::Column::EmittedAt)
            .limit(1000)
            .all(&self.conn)?
            .into_iter()
            .map(|row| serde_json::json!({"key":row.dedup_key,"at":row.emitted_at}))
            .collect();
        Ok(serde_json::json!({"snapshots":snapshots,"notifications":notifications}))
    }
}
