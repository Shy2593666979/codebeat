//! 应用设置持久化与提供方计划刷新。
use super::*;
use entities::{app_settings, awo_runtime};

impl Database {
    pub fn settings(&self) -> Result<Settings> {
        let row = app_settings::Entity::find_by_id("settings").one(&self.conn)?;
        let mut settings: Settings = row
            .map(|r| serde_json::from_str(&r.value_json))
            .transpose()?
            .unwrap_or_default();
        if settings.connection_interval_minutes == 15 {
            settings.connection_interval_minutes = 10;
        }
        Ok(settings)
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        validate_settings(settings)?;
        self.write(|db| {
            app_settings::Entity::insert(app_settings::ActiveModel {
                key: Set("settings".into()),
                value_json: Set(serde_json::to_string(settings)?),
            })
            .on_conflict(
                OnConflict::column(app_settings::Column::Key)
                    .update_column(app_settings::Column::ValueJson)
                    .to_owned(),
            )
            .exec_without_returning(&db.conn)?;
            db.clear_unselected_plans(&settings.pretrigger.providers)?;
            for &provider in &settings.pretrigger.providers {
                // 夏令时歧义只禁用执行，不能导致部分保存。
                let _ =
                    db.replan_provider(provider, db.latest_usage(provider)?.as_ref(), Utc::now());
            }
            Ok(())
        })
    }

    pub(super) fn clear_unselected_plans(&self, providers: &[ProviderId]) -> Result<()> {
        let selected: Vec<_> = providers.iter().map(|p| p.as_str()).collect();
        let mut query = awo_runtime::Entity::delete_many();
        if !selected.is_empty() {
            query = query.filter(awo_runtime::Column::ProviderId.is_not_in(selected));
        }
        query.exec(&self.conn)?;
        Ok(())
    }
}
