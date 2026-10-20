#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use anyhow::Result;
use clap::{Parser, Subcommand};
use codebeat_lib::db::Database;
use codebeat_lib::*;
use serde_json::json;
use std::path::PathBuf;

#[derive(Parser)]
#[command(version, about = "codebeat independent task runner")]
struct Cli {
    #[arg(long, global = true)]
    database: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    Run {
        #[arg(long)]
        task_id: String,
        #[arg(long)]
        now: bool,
    },
    RecoverMissed,
    CheckHealth,
    Maintenance,
    Pretrigger,
    /// 根据本地缓存检查窗口与计划，不连接服务商、不发送模型请求。
    PretriggerStatus,
    /// 清理当前用户的系统任务和自启，不打开数据库。
    CleanupRegistrations,
}
#[tokio::main]
async fn main() {
    if let Err(e) = entry().await {
        eprintln!("{}", codebeat_lib::providers::redact(&e.to_string()));
        std::process::exit(1);
    }
}
async fn entry() -> Result<()> {
    let cli = Cli::parse();
    if matches!(cli.command, Commands::CleanupRegistrations) {
        return codebeat_lib::platform::cleanup_registrations();
    }
    let manage_wake = cli.database.is_none()
        && cfg!(windows)
        && !matches!(cli.command, Commands::PretriggerStatus);
    let mut db = if let Some(path) = cli.database {
        Database::open(&path)?
    } else {
        Database::open_default()?
    };
    db.recover_stale()?;
    match cli.command {
        Commands::CleanupRegistrations => unreachable!(),
        Commands::Run { task_id, now } => {
            codebeat_lib::tasks::execution::run(&mut db, &task_id, now).await?
        }
        Commands::Pretrigger => {
            pretrigger::tick(&mut db).await?;
        }
        Commands::PretriggerStatus => {
            let now = chrono::Utc::now();
            let plans = db.optimizer_display_status(now)?;
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &json!({"plans":plans,"windows":db.optimizer_confirmed_windows(now)?,"next_wake":pretrigger::next_wake_at(&db.settings()?.pretrigger,&plans,now)?})
                )?
            );
        }
        Commands::RecoverMissed | Commands::Maintenance => {
            codebeat_lib::tasks::execution::maintenance(&mut db).await?
        }
        Commands::CheckHealth => {
            let c = codebeat_lib::providers::detect(ProviderId::Codex).await;
            let a = codebeat_lib::providers::detect(ProviderId::ClaudeCode).await;
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &json!({"providers":[c,a],"windows":codebeat_lib::platform::diagnostics().ok()})
                )?
            );
        }
    }
    if manage_wake {
        if let Err(e) = codebeat_lib::platform::sync_optimizer_wake(&db, &std::env::current_exe()?)
        {
            eprintln!("AWO_WAKE: {e}");
        }
    }
    Ok(())
}
