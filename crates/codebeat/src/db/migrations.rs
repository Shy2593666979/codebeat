//! 已有版本化 SQL 只用于结构升级；业务读写由实体和仓储方法处理。
use super::*;

pub(super) fn run(connection: &connection::Connection) -> Result<()> {
    let tx = connection.begin()?;
    let version: i64 = tx
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "PRAGMA user_version",
        ))?
        .context("DATABASE_VERSION_MISSING")?
        .try_get("", "user_version")?;
    if version > 8 {
        bail!("DATABASE_TOO_NEW: 请升级应用，不能降级此数据库");
    }
    let scripts = [
        include_str!("../../upgrades/001_initial.sql"),
        include_str!("../../upgrades/002_pretrigger.sql"),
        include_str!("../../upgrades/003_notification_inbox.sql"),
        include_str!("../../upgrades/004_awo.sql"),
        include_str!("../../upgrades/005_notification_topics.sql"),
        include_str!("../../upgrades/006_coding_allocation.sql"),
        include_str!("../../upgrades/007_quota_notifications.sql"),
        include_str!("../../upgrades/008_daily_allocation.sql"),
    ];
    for (index, script) in scripts.iter().enumerate().skip(version as usize) {
        // 某些旧数据库/夹具已存在此列，但版本号尚未前进。
        if index == 4
            && tx
                .query_all_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    "PRAGMA table_info(notification_inbox)",
                ))?
                .iter()
                .any(|row| {
                    row.try_get::<String>("", "name")
                        .is_ok_and(|name| name == "topic")
                })
        {
            tx.execute_unprepared("PRAGMA user_version=5")?;
            continue;
        }
        let script = script
            .replace("BEGIN IMMEDIATE;", "")
            .replace("COMMIT;", "");
        tx.execute_unprepared(&script)?;
    }
    tx.commit()?;
    Ok(())
}
