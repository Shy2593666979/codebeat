//! 通知收件箱、主题替换、已读和去重。
use super::*;
use entities::{notification_dedup, notification_inbox};

impl Database {
    pub fn notify_once(&self, key: &str) -> Result<bool> {
        let count = notification_dedup::Entity::insert(notification_dedup::ActiveModel {
            dedup_key: Set(key.into()),
            emitted_at: Set(Utc::now().to_rfc3339()),
        })
        .on_conflict(
            OnConflict::column(notification_dedup::Column::DedupKey)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(&self.conn)?;
        Ok(count == 1)
    }

    pub fn undo_notification(&self, key: &str) -> Result<()> {
        notification_dedup::Entity::delete_by_id(key).exec(&self.conn)?;
        Ok(())
    }

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
        let replaced = self.write(|db| {
            if notification_inbox::Entity::find_by_id(&notice.id)
                .one(&db.conn)?
                .is_some()
            {
                return Ok(None);
            }
            let previous = if let Some(topic) = &notice.topic {
                notification_inbox::Entity::find()
                    .filter(notification_inbox::Column::Topic.eq(topic))
                    .one(&db.conn)?
            } else {
                None
            };
            let at = notice.at.to_rfc3339();
            let replaced = if let Some(old) = previous {
                // 未读主题更新保留展示顺序；已读主题收到新版本则重新计时。
                let created = if old.acknowledged { at } else { old.created_at };
                notification_inbox::Entity::update_many()
                    .col_expr(notification_inbox::Column::Id, Expr::value(&notice.id))
                    .col_expr(
                        notification_inbox::Column::DataJson,
                        Expr::value(serde_json::to_string(notice)?),
                    )
                    .col_expr(notification_inbox::Column::Acknowledged, Expr::value(false))
                    .col_expr(notification_inbox::Column::CreatedAt, Expr::value(created))
                    .filter(notification_inbox::Column::Id.eq(&old.id))
                    .exec(&db.conn)?;
                Some(old.id)
            } else {
                db.insert_notice(notice)?;
                None
            };
            notification_inbox::Entity::delete_many()
                .filter(notification_inbox::Column::Acknowledged.eq(true))
                .filter(
                    notification_inbox::Column::CreatedAt
                        .lt((Utc::now() - Duration::days(30)).to_rfc3339()),
                )
                .exec(&db.conn)?;
            Ok(replaced)
        })?;
        if let Some(old) = replaced {
            let directory = self.directory.join("notification-acks");
            if std::fs::create_dir_all(&directory).is_ok() {
                let _ = std::fs::write(directory.join(format!("{old}.dismiss")), b"");
            }
        }
        Ok(())
    }

    pub(super) fn insert_notice(&self, notice: &InboxNotice) -> Result<()> {
        notification_inbox::Entity::insert(notification_inbox::ActiveModel {
            id: Set(notice.id.clone()),
            data_json: Set(serde_json::to_string(notice)?),
            created_at: Set(notice.at.to_rfc3339()),
            acknowledged: Set(false),
            topic: Set(notice.topic.clone()),
        })
        .exec_without_returning(&self.conn)?;
        Ok(())
    }

    pub fn pending_notices(&self) -> Result<Vec<InboxNotice>> {
        notification_inbox::Entity::find()
            .filter(notification_inbox::Column::Acknowledged.eq(false))
            .order_by_desc(notification_inbox::Column::CreatedAt)
            .order_by_desc(Expr::cust("rowid"))
            .limit(200)
            .all(&self.conn)?
            .into_iter()
            .map(|row| Ok(serde_json::from_str(&row.data_json)?))
            .collect()
    }

    pub fn acknowledge_notices(&mut self, ids: &[String]) -> Result<()> {
        if ids.len() > 200 {
            bail!("INVALID_NOTIFICATION");
        }
        for id in ids {
            Uuid::parse_str(id)?;
        }
        if ids.is_empty() {
            return Ok(());
        }
        notification_inbox::Entity::update_many()
            .col_expr(notification_inbox::Column::Acknowledged, Expr::value(true))
            .filter(notification_inbox::Column::Id.is_in(ids.iter().cloned()))
            .exec(&self.conn)?;
        Ok(())
    }

    pub fn acknowledge_all_notices(&mut self) -> Result<Vec<String>> {
        self.write(|db| {
            let ids = notification_inbox::Entity::find()
                .select_only()
                .column(notification_inbox::Column::Id)
                .filter(notification_inbox::Column::Acknowledged.eq(false))
                .into_tuple::<String>()
                .all(&db.conn)?;
            notification_inbox::Entity::update_many()
                .col_expr(notification_inbox::Column::Acknowledged, Expr::value(true))
                .filter(notification_inbox::Column::Acknowledged.eq(false))
                .exec(&db.conn)?;
            Ok(ids)
        })
    }
}
