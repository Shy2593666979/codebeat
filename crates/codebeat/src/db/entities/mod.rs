//! 数据库实体：目录名是项目约定，每张表独立映射。
pub mod app_settings;
pub mod awo_activity;
pub mod awo_attempts;
pub mod awo_runtime;
pub mod conversations;
pub mod notification_dedup;
pub mod notification_inbox;
pub mod pretrigger_runs;
pub mod providers;
pub mod quota_credit_changes;
pub mod quota_notice_state;
pub mod quota_reset_consumptions;
pub mod task_runs;
pub mod tasks;
pub mod usage_snapshots;
