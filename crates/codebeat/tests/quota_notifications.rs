//! 隔离数据库验证通知分类与去重，不兑换真实重置卡、不发送模型请求。
use chrono::{Duration, Utc};
use codebeat_lib::{db::Database, providers::reset::parse_credits, *};
use serde_json::json;

fn report(minutes: i64, used: f64, count: u64, details: serde_json::Value) -> UsageReport {
    let now = Utc::now() - Duration::hours(1) + Duration::minutes(minutes);
    let mut cards =
        parse_credits(&json!({"rateLimitResetCredits":{"availableCount":count,"credits":details}}))
            .unwrap();
    cards.account_key = Some("account-a".into());
    UsageReport {
        provider: ProviderId::Codex,
        status: "available".into(),
        message: String::new(),
        reset_credits: Some(cards),
        windows: vec![UsageWindow {
            label: "primary".into(),
            used_percent: Some(used),
            resets_at_utc: Some(now + Duration::hours(5)),
            duration_minutes: Some(300),
            observed_at_utc: now,
            source: "fixture".into(),
            reliability: "official".into(),
        }],
    }
}
fn card(expiry: serde_json::Value) -> serde_json::Value {
    json!([{"id":"card-1","status":"available","expiresAt":expiry}])
}

#[test]
fn official_early_reset_survives_a_failed_read_and_is_not_repeated() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("quota.db");
    let db = Database::open(&path).unwrap();
    let before = report(0, 63., 1, serde_json::Value::Null);
    db.cache_usage(&before).unwrap();
    let mut failed = before.clone();
    failed.status = "unavailable".into();
    failed.windows.clear();
    db.cache_usage(&failed).unwrap();
    let after = report(10, 0., 1, serde_json::Value::Null);
    db.cache_usage(&after).unwrap();
    db.cache_usage(&after).unwrap();
    drop(db);
    let db = Database::open(&path).unwrap();
    db.cache_usage(&report(11, 0., 1, serde_json::Value::Null))
        .unwrap();
    let notices = db.pending_notices().unwrap();
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].title, "Codex 官方额度已重置");
    assert!(notices[0].message.contains("未观察到可用重置卡减少"));
}

#[test]
fn expired_missing_and_truncated_card_details_do_not_prove_redemption() {
    for details in [
        serde_json::Value::Null,
        card(json!((Utc::now() - Duration::minutes(55)).timestamp())),
        json!([{"id":"card-1","status":"available"}]),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("quota.db")).unwrap();
        db.cache_usage(&report(0, 63., 1, details)).unwrap();
        db.cache_usage(&report(10, 0., 0, json!([]))).unwrap();
        let notices = db.pending_notices().unwrap();
        assert_eq!(notices.len(), 1);
        assert_eq!(notices[0].title, "Codex 额度已恢复");
        assert!(!notices[0].title.contains("卡"));
    }
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(&dir.path().join("quota.db")).unwrap();
    db.cache_usage(&report(0, 63., 3, card(serde_json::Value::Null)))
        .unwrap();
    db.cache_usage(&report(10, 0., 2, json!([]))).unwrap();
    assert_eq!(db.pending_notices().unwrap()[0].title, "Codex 额度已恢复");
}

#[test]
fn a_missing_unexpired_card_with_quota_recovery_is_classified_separately() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(&dir.path().join("quota.db")).unwrap();
    db.cache_usage(&report(
        0,
        63.,
        1,
        card(json!((Utc::now() + Duration::days(1)).timestamp())),
    ))
    .unwrap();
    db.cache_usage(&report(10, 0., 0, json!([]))).unwrap();
    let n = db.pending_notices().unwrap();
    assert_eq!(n.len(), 1);
    assert_eq!(n[0].title, "Codex 重置卡重置提醒");
}

#[test]
fn credit_count_and_quota_can_update_in_separate_polls() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(&dir.path().join("quota.db")).unwrap();
    db.cache_usage(&report(
        0,
        63.,
        1,
        card(json!((Utc::now() + Duration::days(1)).timestamp())),
    ))
    .unwrap();
    db.cache_usage(&report(1, 63., 0, json!([]))).unwrap();
    assert!(db.pending_notices().unwrap().is_empty());
    db.cache_usage(&report(13, 0., 0, json!([]))).unwrap();
    assert_eq!(
        db.pending_notices().unwrap()[0].title,
        "Codex 重置卡重置提醒"
    );
}

#[test]
fn card_expiration_alone_and_account_switch_do_not_trigger_a_reset_notice() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(&dir.path().join("quota.db")).unwrap();
    db.cache_usage(&report(
        0,
        63.,
        1,
        card(json!((Utc::now() - Duration::minutes(55)).timestamp())),
    ))
    .unwrap();
    db.cache_usage(&report(10, 63., 0, json!([]))).unwrap();
    let mut changed = report(20, 0., 0, json!([]));
    changed.reset_credits.as_mut().unwrap().account_key = Some("account-b".into());
    db.cache_usage(&changed).unwrap();
    assert!(db.pending_notices().unwrap().is_empty());
}

#[test]
fn confirmed_consumption_is_one_notice_even_if_refresh_fails_or_retries() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("quota.db");
    let db = Database::open(&path).unwrap();
    db.cache_usage(&report(0, 63., 1, serde_json::Value::Null))
        .unwrap();
    let attempt = uuid::Uuid::new_v4().to_string();
    db.record_reset_consumption(&attempt, "account-a").unwrap();
    db.record_reset_consumption(&attempt, "account-a").unwrap();
    drop(db);
    let db = Database::open(&path).unwrap();
    db.record_reset_consumption(&attempt, "account-a").unwrap();
    let mut after = report(10, 0., 0, json!([]));
    after.windows[0].observed_at_utc = Utc::now();
    db.cache_usage(&after).unwrap();
    let n = db.pending_notices().unwrap();
    assert_eq!(n.len(), 1);
    assert_eq!(n[0].title, "Codex 重置卡已使用");
}

#[test]
fn notifications_off_still_advances_the_baseline_without_late_replays() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(&dir.path().join("quota.db")).unwrap();
    let mut settings = db.settings().unwrap();
    settings.notifications = false;
    db.save_settings(&settings).unwrap();
    db.cache_usage(&report(0, 63., 1, serde_json::Value::Null))
        .unwrap();
    db.cache_usage(&report(10, 0., 1, serde_json::Value::Null))
        .unwrap();
    settings.notifications = true;
    db.save_settings(&settings).unwrap();
    db.cache_usage(&report(20, 0., 1, serde_json::Value::Null))
        .unwrap();
    assert!(db.pending_notices().unwrap().is_empty());
}

#[test]
fn a_new_official_cycle_is_not_missed_when_usage_has_already_increased() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(&dir.path().join("quota.db")).unwrap();
    let mut before = report(0, 1., 1, serde_json::Value::Null);
    before.windows[0].resets_at_utc =
        Some(before.windows[0].observed_at_utc + Duration::minutes(5));
    db.cache_usage(&before).unwrap();
    db.cache_usage(&report(10, 5., 1, serde_json::Value::Null))
        .unwrap();
    let n = db.pending_notices().unwrap();
    assert_eq!(n.len(), 1);
    assert_eq!(n[0].title, "Codex 额度窗口已恢复");
}

#[test]
fn concurrent_refreshes_produce_one_message_and_late_observations_cannot_replay() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("quota.db");
    let db = Database::open(&path).unwrap();
    let before = report(0, 63., 1, serde_json::Value::Null);
    let after = report(10, 0., 1, serde_json::Value::Null);
    db.cache_usage(&before).unwrap();
    std::thread::scope(|scope| {
        for _ in 0..3 {
            let path = &path;
            let after = &after;
            scope.spawn(move || Database::open(path).unwrap().cache_usage(after).unwrap());
        }
    });
    db.cache_usage(&before).unwrap();
    db.cache_usage(&report(20, 0., 1, serde_json::Value::Null))
        .unwrap();
    assert_eq!(db.pending_notices().unwrap().len(), 1);
}

#[test]
fn different_buckets_with_the_same_duration_never_compare_each_others_counters() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(&dir.path().join("quota.db")).unwrap();
    let mut before = report(0, 63., 1, serde_json::Value::Null);
    let mut other = before.windows[0].clone();
    other.label = "other-primary".into();
    other.used_percent = Some(0.);
    before.windows.push(other);
    db.cache_usage(&before).unwrap();
    let mut after = before.clone();
    for window in &mut after.windows {
        window.observed_at_utc += Duration::minutes(1);
    }
    db.cache_usage(&after).unwrap();
    assert!(db.pending_notices().unwrap().is_empty());
}
