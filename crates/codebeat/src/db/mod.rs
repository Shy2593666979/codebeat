//! SQLite 持久化入口。实体、事务和各业务仓储分开维护。
use crate::pretrigger as allocation;
use crate::*;
use anyhow::{bail, Context, Result};
use chrono::{Duration, Utc};
use sea_orm::{
    sea_query::{Expr, OnConflict},
    ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, Statement,
};
use std::path::{Path, PathBuf};
use uuid::Uuid;

mod connection;
pub mod entities;
mod migrations;
mod notifications;
mod pretrigger;
mod quota_notifications;
mod settings;
mod tasks;
mod usage;
mod windows;

pub fn data_dir() -> Result<PathBuf> {
    Ok(
        PathBuf::from(std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA unavailable")?)
            .join("codebeat"),
    )
}

pub struct Database {
    conn: connection::Connection,
    directory: PathBuf,
}

impl Database {
    pub fn open_default() -> Result<Self> {
        Self::open(&data_dir()?.join("codebeat.db"))
    }

    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        // SQLite URL 的主机部分不能是相对目录，先规范成绝对文件路径。
        let absolute = std::path::absolute(path)?;
        let url = if path == Path::new(":memory:") {
            "sqlite::memory:".to_owned()
        } else {
            format!(
                "sqlite://{}",
                absolute
                    .to_str()
                    .context("DATABASE_PATH_UTF8")?
                    .replace('\\', "/")
            )
        };
        let conn = connection::Connection::Database(sea_orm::Database::connect(url)?);
        conn.execute_unprepared(
            "PRAGMA busy_timeout=10000; PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;",
        )?;
        migrations::run(&conn)?;
        entities::app_settings::Entity::insert(entities::app_settings::ActiveModel {
            key: Set("settings".into()),
            value_json: Set(serde_json::to_string(&Settings::default())?),
        })
        .on_conflict(
            OnConflict::column(entities::app_settings::Column::Key)
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(&conn)?;
        Ok(Self {
            conn,
            directory: absolute
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf(),
        })
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    /// 写事务使用 SQLite IMMEDIATE，防止多个进程同时读到可抢占状态。
    /// 闭包只接触事务连接；出错时由 SeaORM 回滚。
    fn write<T>(&self, action: impl FnOnce(&Self) -> Result<T>) -> Result<T> {
        let scoped = Self {
            conn: connection::Connection::Transaction(self.conn.begin()?),
            directory: self.directory.clone(),
        };
        let value = action(&scoped)?;
        if let connection::Connection::Transaction(tx) = scoped.conn {
            tx.commit()?;
        }
        Ok(value)
    }
}
