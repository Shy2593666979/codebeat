//! 每次 runner 检查都留下决策记录，不依赖会被覆盖的运行计划。
use super::*;
use std::io::Write;
pub(super) fn log(
    db: &crate::db::Database,
    provider: Option<ProviderId>,
    event: &str,
    plan: Option<&RuntimePlan>,
    report: Option<&UsageReport>,
) {
    let write = || -> Result<()> {
        let dir = db.directory().join("logs");
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("pretrigger-decisions.jsonl");
        if std::fs::metadata(&path).is_ok_and(|m| m.len() > 2 * 1024 * 1024) {
            let old = dir.join("pretrigger-decisions.previous.jsonl");
            if old.exists() {
                let _ = std::fs::remove_file(&old);
            }
            let _ = std::fs::rename(&path, old);
        }
        let executable = std::env::current_exe().ok();
        let modified = executable
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .and_then(|m| m.modified().ok())
            .map(chrono::DateTime::<Utc>::from);
        let data = serde_json::json!({"at":Utc::now(),"event":event,"provider":provider,"pid":std::process::id(),"executable":executable,"executable_modified":modified,"version":env!("CARGO_PKG_VERSION"),"plan":plan,"quota_status":report.map(|r|&r.status),"quota_message":report.map(|r|crate::providers::redact(&r.message)),"windows":report.map(|r|&r.windows)});
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        file.write_all(format!("{data}\n").as_bytes())?;
        Ok(())
    };
    if let Err(error) = write() {
        eprintln!("PRETRIGGER_DECISION_LOG: {error}");
    }
}
