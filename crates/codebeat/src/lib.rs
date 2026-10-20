//! CodeBeat 共享的领域模型、持久化、任务执行与 Windows 集成。
//! 桌面应用和独立 runner 都依赖这个库。
pub mod db;
pub mod models;
pub mod notifications;
pub mod platform;
pub mod pretrigger;
pub mod providers;
pub mod tasks;
pub mod usage;
pub use models::*;
pub use tasks::{local_directory, missed_state, next_occurrence, validate_settings, validate_task};
