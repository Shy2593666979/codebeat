use chrono::{TimeZone, Utc};
use codebeat_lib::{pretrigger::*, *};
#[test]
fn balances_work_and_aligns_lunch_to_7_12_17() {
    let s = PretriggerSettings {
        lunch_enabled: true,
        ..Default::default()
    };
    let p = plan(&s).unwrap();
    assert_eq!(p.times, vec!["07:00", "12:00", "17:00"]);
    assert_eq!(p.work_minutes_per_window, vec![120, 180, 180]);
    assert_eq!(p.max_work_minutes, 180);
    let mut no_lunch = s.clone();
    no_lunch.lunch_enabled = false;
    assert_eq!(
        plan(&no_lunch).unwrap().work_minutes_per_window,
        vec![150, 300, 150]
    );
}
#[test]
fn weekdays_timezone_and_missed_round_only() {
    let mut s = PretriggerSettings {
        work_start: "09:00".into(),
        work_end: "18:00".into(),
        ..Default::default()
    };
    assert!(
        due(&s, Utc.with_ymd_and_hms(2026, 10, 17, 0, 0, 0).unwrap())
            .unwrap()
            .is_none()
    );
    s.days = "daily".into();
    let now = Utc.with_ymd_and_hms(2026, 10, 17, 3, 1, 0).unwrap(); // 11:00
    assert!(due(&s, now).unwrap().is_some());
    assert!(due(&s, now + chrono::Duration::minutes(11))
        .unwrap()
        .is_none());
    s.timezone = "America/New_York".into();
    s.work_start = "01:00".into();
    s.work_end = "04:00".into();
    assert!(due(&s, Utc.with_ymd_and_hms(2026, 11, 1, 6, 30, 0).unwrap()).is_err());
}
#[test]
fn invalid_ranges_and_empty_tools_are_rejected() {
    for (key, value) in [
        ("work_start", "20:00"),
        ("work_end", "10:00"),
        ("lunch_start", "09:00"),
        ("lunch_end", "21:00"),
        ("timezone", "bogus"),
        ("days", "bogus"),
    ] {
        let mut v = serde_json::to_value(PretriggerSettings {
            lunch_enabled: true,
            ..Default::default()
        })
        .unwrap();
        v[key] = value.into();
        let s: PretriggerSettings = serde_json::from_value(v).unwrap();
        assert!(plan(&s).is_err(), "{key}");
    }
    let mut s = PretriggerSettings::default();
    s.providers.clear();
    assert!(validate(&s).is_err());
}
#[test]
fn only_fresh_official_five_hour_windows_are_active() {
    let now = Utc::now();
    let mut r = UsageReport {
        reset_credits: None,
        provider: ProviderId::Codex,
        status: "available".into(),
        message: String::new(),
        windows: vec![UsageWindow {
            label: "primary".into(),
            used_percent: Some(1.0),
            resets_at_utc: Some(now + chrono::Duration::hours(3)),
            duration_minutes: Some(300),
            observed_at_utc: now,
            source: "fixture".into(),
            reliability: "official".into(),
        }],
    };
    assert!(active_reset(&r, now).is_some());
    r.windows[0].used_percent = Some(0.0);
    assert!(active_reset(&r, now).is_none());
    r.windows[0].used_percent = Some(100.0);
    assert!(exhausted(&r, now));
    r.windows[0].duration_minutes = Some(10080);
    assert!(active_reset(&r, now).is_none());
    r.windows[0].duration_minutes = Some(300);
    r.windows[0].observed_at_utc = now - chrono::Duration::minutes(3);
    assert!(active_reset(&r, now).is_none());
}
