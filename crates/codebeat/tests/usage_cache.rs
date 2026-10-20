use chrono::Utc;
use codebeat_lib::{db::Database, *};
#[test]
fn failed_read_keeps_last_success_for_display_but_not_for_automatic_execution() {
    let dir = tempfile::tempdir().unwrap();
    let database = Database::open(&dir.path().join("quota.db")).unwrap();
    let good = UsageReport {
        provider: ProviderId::Codex,
        windows: vec![UsageWindow {
            label: "codex".into(),
            used_percent: Some(17.),
            resets_at_utc: Some(Utc::now() + chrono::Duration::hours(5)),
            duration_minutes: Some(300),
            observed_at_utc: Utc::now(),
            source: "fixture".into(),
            reliability: "official".into(),
        }],
        status: "available".into(),
        message: String::new(),
        reset_credits: None,
    };
    database.cache_usage(&good).unwrap();
    let mut failed = good.clone();
    failed.status = "unavailable".into();
    failed.windows.clear();
    failed.message = "RPC_ERROR: network".into();
    database.cache_usage(&failed).unwrap();
    assert_eq!(
        database
            .latest_usage(ProviderId::Codex)
            .unwrap()
            .unwrap()
            .status,
        "unavailable"
    );
    let cached = database
        .latest_successful_usage(ProviderId::Codex)
        .unwrap()
        .unwrap();
    assert_eq!(cached.windows[0].used_percent, Some(17.));
    assert!(database
        .latest_successful_usage(ProviderId::ClaudeCode)
        .unwrap()
        .is_none());
    let runtime = database
        .runtime_pretrigger(ProviderId::Codex)
        .unwrap()
        .unwrap();
    assert_eq!(runtime.status, "unknown");
    assert!(!runtime.automatic_allowed);
}
