//! 额度计划、预触发抢占与发送预留。
use super::*;
use entities::{awo_activity, awo_attempts, awo_runtime, pretrigger_runs, tasks};

impl Database {
    pub fn runtime_pretrigger(&self, id: ProviderId) -> Result<Option<allocation::RuntimePlan>> {
        awo_runtime::Entity::find_by_id(id.as_str())
            .one(&self.conn)?
            .map(|row| Ok(serde_json::from_str(&row.data_json)?))
            .transpose()
    }

    pub fn replan_provider(
        &self,
        id: ProviderId,
        report: Option<&UsageReport>,
        now: chrono::DateTime<Utc>,
    ) -> Result<allocation::RuntimePlan> {
        let old = self.runtime_pretrigger(id)?;
        let settings = self.settings()?.pretrigger;
        let initial = allocation::runtime_plan(&settings, id, now, None, None)?;
        let mut seed = old
            .filter(|p| p.date == initial.date && !p.display_times.is_empty())
            .unwrap_or(initial);
        let history = self.optimizer_confirmed_windows(now)?;
        let own: Vec<_> = history.iter().filter(|w| w.provider == id).collect();
        if let Some(first) = own.first() {
            seed.day_start = Some(first.start);
        }
        if let Some(last) = own.last() {
            seed.actual_reset = Some(last.closed_at.unwrap_or(last.end));
            seed.latest_start = Some(last.start);
            seed.window_confirmed = last.source != "app_unconfirmed";
        }
        let mut plan = allocation::runtime_plan(&settings, id, now, report, Some(&seed))?;
        // 尚未核实的成功发送仍占用本轮，不能重新领取同一个理论节点。
        if let Some(last) = own.last().filter(|last| last.source == "app_unconfirmed") {
            let earliest = last.end + Duration::seconds(allocation::RESET_SETTLE_SECONDS);
            plan.slots.retain(|slot| slot.execute_at >= earliest);
            plan.recommendations = plan.slots.iter().map(|slot| slot.execute_at).collect();
            plan.automatic_allowed &= now >= earliest;
        }
        awo_runtime::Entity::insert(awo_runtime::ActiveModel {
            provider_id: Set(id.as_str().into()),
            data_json: Set(serde_json::to_string(&plan)?),
        })
        .on_conflict(
            OnConflict::column(awo_runtime::Column::ProviderId)
                .update_column(awo_runtime::Column::DataJson)
                .to_owned(),
        )
        .exec_without_returning(&self.conn)?;
        Ok(plan)
    }

    pub fn optimizer_status(&self) -> Result<Vec<allocation::RuntimePlan>> {
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
    ) -> Result<Vec<allocation::RuntimePlan>> {
        let selected = self.settings()?.pretrigger.providers;
        self.clear_unselected_plans(&selected)?;
        selected
            .iter()
            .map(|&id| self.replan_provider(id, self.latest_usage(id)?.as_ref(), now))
            .collect()
    }

    pub fn reserve_probe_request(
        &self,
        record: &allocation::ProbeRecord,
        now: chrono::DateTime<Utc>,
    ) -> Result<bool> {
        self.write(|db| {
            let settings = db.settings()?;
            if !settings.pretrigger_enabled
                || !settings.pretrigger.auto_trigger
                || settings.pretrigger.mode == "save"
                || settings.paused
                || settings.paused_providers.contains(&record.provider)
                || !settings.pretrigger.providers.contains(&record.provider)
            {
                return Ok(false);
            }
            if tasks::Entity::find()
                .filter(tasks::Column::State.is_in(["claimed", "running", "cancelling"]))
                .count(&db.conn)?
                > 0
            {
                return Ok(false);
            }
            if awo_attempts::Entity::find()
                .filter(awo_attempts::Column::ProviderId.eq(record.provider.as_str()))
                .filter(
                    awo_attempts::Column::AttemptedAt.gt((now - Duration::hours(5)).to_rfc3339()),
                )
                .count(&db.conn)?
                > 0
            {
                return Ok(false);
            }
            let inserted = awo_attempts::Entity::insert(awo_attempts::ActiveModel {
                provider_id: Set(record.provider.as_str().into()),
                slot: Set(record.slot.clone()),
                attempted_at: Set(now.to_rfc3339()),
            })
            .on_conflict(
                OnConflict::columns([awo_attempts::Column::ProviderId, awo_attempts::Column::Slot])
                    .do_nothing()
                    .to_owned(),
            )
            .exec_without_returning(&db.conn)?;
            awo_attempts::Entity::delete_many()
                .filter(
                    awo_attempts::Column::AttemptedAt.lt((now - Duration::days(30)).to_rfc3339()),
                )
                .exec(&db.conn)?;
            Ok(inserted == 1)
        })
    }

    pub fn persist_probe_progress(&self, record: &allocation::ProbeRecord) -> Result<()> {
        pretrigger_runs::Entity::update_many()
            .col_expr(
                pretrigger_runs::Column::DataJson,
                Expr::value(serde_json::to_string(record)?),
            )
            .filter(pretrigger_runs::Column::ProviderId.eq(record.provider.as_str()))
            .filter(pretrigger_runs::Column::Slot.eq(&record.slot))
            .filter(pretrigger_runs::Column::State.eq("running"))
            .exec(&self.conn)?;
        Ok(())
    }

    pub fn upcoming_real_task(&self, id: ProviderId, now: chrono::DateTime<Utc>) -> Result<bool> {
        Ok(self.tasks()?.iter().any(|task| {
            task.request.conversation.provider == id
                && matches!(task.state.as_str(), "scheduled" | "claimed" | "running")
                && task.request.scheduled_at_utc <= now + Duration::minutes(15)
                && task.request.scheduled_at_utc >= now - Duration::minutes(10)
        }))
    }

    pub fn optimizer_habit(&self) -> Result<Vec<serde_json::Value>> {
        use chrono::{Datelike, Timelike};
        let settings = self.settings()?.pretrigger;
        let zone = settings.timezone.parse::<chrono_tz::Tz>()?;
        let rows = awo_activity::Entity::find()
            .filter(
                awo_activity::Column::ObservedAt.gt((Utc::now() - Duration::days(21)).to_rfc3339()),
            )
            .order_by_asc(awo_activity::Column::ObservedAt)
            .all(&self.conn)?;
        let mut days = std::collections::BTreeMap::new();
        for row in rows {
            let at = chrono::DateTime::parse_from_rfc3339(&row.observed_at)?.with_timezone(&zone);
            if settings.days == "weekdays" && at.weekday().number_from_monday() > 5 {
                continue;
            }
            days.entry((row.provider_id, at.date_naive()))
                .or_insert((at.hour() * 60 + at.minute()) as i32);
        }
        let intervals = allocation::work_intervals(&settings)?;
        let start = intervals[0].0;
        let overnight = intervals.last().is_some_and(|(_, end)| *end > 1440);
        Ok(settings
            .providers
            .iter()
            .map(|id| {
                let early = days
                    .iter()
                    .filter(|((provider, _), minute)| {
                        !overnight && provider == id.as_str() && **minute < start - 30
                    })
                    .count();
                serde_json::json!({"provider":id,"early_days":early,"suggest_adjustment":early>=5})
            })
            .collect())
    }

    pub fn pretrigger_runs(&self) -> Result<Vec<allocation::ProbeRecord>> {
        pretrigger_runs::Entity::find()
            .order_by_desc(pretrigger_runs::Column::StartedAt)
            .limit(100)
            .all(&self.conn)?
            .into_iter()
            .map(|row| Ok(serde_json::from_str(&row.data_json)?))
            .collect()
    }

    pub fn claim_pretrigger(
        &mut self,
        provider: ProviderId,
        slot: &str,
        now: chrono::DateTime<Utc>,
    ) -> Result<bool> {
        self.write(|db| {
            let settings = db.settings()?;
            if !settings.pretrigger_enabled
                || settings.paused
                || settings.paused_providers.contains(&provider)
                || !settings.pretrigger.providers.contains(&provider)
            {
                return Ok(false);
            }
            let tasks_busy = tasks::Entity::find()
                .filter(tasks::Column::State.is_in(["claimed", "running", "cancelling"]))
                .count(&db.conn)?
                > 0;
            let probes_busy = pretrigger_runs::Entity::find()
                .filter(pretrigger_runs::Column::State.eq("running"))
                .filter(
                    pretrigger_runs::Column::StartedAt
                        .gt((now - Duration::minutes(5)).to_rfc3339()),
                )
                .count(&db.conn)?
                > 0;
            if tasks_busy || probes_busy {
                return Ok(false);
            }
            if let Some(old) =
                pretrigger_runs::Entity::find_by_id((provider.as_str().to_owned(), slot.to_owned()))
                    .one(&db.conn)?
            {
                let old: allocation::ProbeRecord = serde_json::from_str(&old.data_json)?;
                if old.state != "waiting" || old.next_check_at.is_none_or(|time| time > now) {
                    return Ok(false);
                }
            }
            let record = allocation::ProbeRecord {
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
            pretrigger_runs::Entity::insert(pretrigger_runs::ActiveModel {
                provider_id: Set(provider.as_str().into()),
                slot: Set(slot.into()),
                state: Set("running".into()),
                data_json: Set(serde_json::to_string(&record)?),
                started_at: Set(now.to_rfc3339()),
            })
            .on_conflict(
                OnConflict::columns([
                    pretrigger_runs::Column::ProviderId,
                    pretrigger_runs::Column::Slot,
                ])
                .update_columns([
                    pretrigger_runs::Column::State,
                    pretrigger_runs::Column::DataJson,
                    pretrigger_runs::Column::StartedAt,
                ])
                .to_owned(),
            )
            .exec_without_returning(&db.conn)?;
            pretrigger_runs::Entity::delete_many()
                .filter(
                    pretrigger_runs::Column::StartedAt.lt((now - Duration::days(30)).to_rfc3339()),
                )
                .exec(&db.conn)?;
            Ok(true)
        })
    }

    pub fn finish_pretrigger(&self, record: &allocation::ProbeRecord) -> Result<()> {
        pretrigger_runs::Entity::update_many()
            .col_expr(pretrigger_runs::Column::State, Expr::value(&record.state))
            .col_expr(
                pretrigger_runs::Column::DataJson,
                Expr::value(serde_json::to_string(record)?),
            )
            .filter(pretrigger_runs::Column::ProviderId.eq(record.provider.as_str()))
            .filter(pretrigger_runs::Column::Slot.eq(&record.slot))
            .filter(pretrigger_runs::Column::State.eq("running"))
            .exec(&self.conn)?;
        Ok(())
    }
}
