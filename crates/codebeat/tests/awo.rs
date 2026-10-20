use chrono::{Duration, TimeZone, Utc};
use codebeat_lib::{db::Database, pretrigger::*, *};
fn settings() -> PretriggerSettings {
    PretriggerSettings {
        work_start: "09:00".into(),
        work_end: "18:00".into(),
        days: "daily".into(),
        auto_trigger: true,
        ..Default::default()
    }
}
fn at(h: u32, m: u32) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, 19, h, m, 0).unwrap() - Duration::hours(8)
}
fn quota(
    now: chrono::DateTime<Utc>,
    used: f64,
    reset: Option<chrono::DateTime<Utc>>,
) -> UsageReport {
    UsageReport {
        provider: ProviderId::Codex,
        status: "available".into(),
        message: String::new(),
        reset_credits: None,
        windows: vec![UsageWindow {
            label: "primary".into(),
            used_percent: Some(used),
            resets_at_utc: reset,
            duration_minutes: Some(300),
            observed_at_utc: now,
            source: "isolated fixture".into(),
            reliability: "official".into(),
        }],
    }
}
// 单元测试显式提供已核实的历史；快照稳定性由数据库测试验证。
fn verified_plan(
    s: &PretriggerSettings,
    now: chrono::DateTime<Utc>,
    report: &UsageReport,
) -> RuntimePlan {
    let mut seed = runtime_plan(s, ProviderId::Codex, now, None, None).unwrap();
    seed.actual_reset = report.windows[0].resets_at_utc;
    seed.window_confirmed = true;
    seed.day_start = seed.actual_reset.map(|r| r - Duration::hours(5));
    runtime_plan(s, ProviderId::Codex, now, Some(report), Some(&seed)).unwrap()
}
#[test]
fn lunch_intersections_do_not_pause_the_five_hour_clock() {
    for end in ["13:00", "13:30", "14:00"] {
        let mut s = settings();
        s.lunch_enabled = true;
        s.lunch_end = end.into();
        let p = plan(&s).unwrap();
        let lunch = if end == "13:00" {
            60
        } else if end == "13:30" {
            90
        } else {
            120
        };
        assert_eq!(p.work_minutes_per_window.iter().sum::<i32>(), 540 - lunch);
        assert!(p.starts.windows(2).all(|w| w[1] - w[0] == 300));
        assert!(p
            .work_minutes_per_window
            .iter()
            .all(|&n| n >= s.min_work_minutes));
    }
}
#[test]
fn disabled_lunch_ignores_invalid_saved_values_and_times_are_required() {
    let mut s = settings();
    s.lunch_start = String::new();
    s.lunch_end = "bad".into();
    assert!(plan(&s).is_ok());
    s.lunch_enabled = true;
    assert!(plan(&s).is_err());
    s.lunch_enabled = false;
    s.work_start.clear();
    assert!(plan(&s).is_err());
}
#[test]
fn overnight_lunch_and_shift_weekday_belong_to_starting_date() {
    let mut s = settings();
    s.work_start = "22:00".into();
    s.work_end = "08:00".into();
    s.lunch_enabled = true;
    s.lunch_start = "02:00".into();
    s.lunch_end = "03:00".into();
    s.days = "weekdays".into();
    assert_eq!(
        work_intervals(&s).unwrap(),
        vec![(1320, 1560), (1620, 1920)]
    );
    let saturday = Utc.with_ymd_and_hms(2026, 10, 17, 1, 0, 0).unwrap() - Duration::hours(8);
    let p = runtime_plan(&s, ProviderId::Codex, saturday, None, None).unwrap();
    assert_eq!(p.date.to_string(), "2026-10-16");
    assert!(p.work_end > saturday);
}
#[test]
fn early_real_usage_cancels_original_slot_and_reanchors_rest_of_day() {
    let s = settings();
    let initial = runtime_plan(&s, ProviderId::Codex, at(5, 0), None, None).unwrap();
    assert_eq!(initial.recommendations[0], at(6, 0));
    let report = quota(at(5, 0), 1.0, Some(at(10, 0)));
    let adjusted = verified_plan(&s, at(5, 0), &report);
    assert_eq!(adjusted.status, "active");
    assert_eq!(adjusted.recommendations, vec![at(10, 1), at(15, 2)]);
    assert_eq!(adjusted.display_times, vec![at(5, 0), at(10, 0), at(15, 0)]);
    assert!(due_for_plan(&s, &adjusted, at(6, 0)).is_none());
    assert!(!adjusted.automatic_allowed);
    assert_eq!(s.work_start, "09:00");
    // 过期的快照不能让已取消的 06:00 请求重新生效。
    let stale = runtime_plan(
        &s,
        ProviderId::Codex,
        at(6, 0),
        Some(&report),
        Some(&adjusted),
    )
    .unwrap();
    assert!(due_for_plan(&s, &stale, at(6, 0)).is_none());
    let idle = quota(at(10, 0), 0.0, None);
    let next = runtime_plan(
        &s,
        ProviderId::Codex,
        at(10, 0),
        Some(&idle),
        Some(&adjusted),
    )
    .unwrap();
    assert!(due_for_plan(&s, &next, at(10, 0)).is_none());
    let ready = runtime_plan(&s, ProviderId::Codex, at(10, 1), Some(&idle), Some(&next)).unwrap();
    assert!(due_for_plan(&s, &ready, at(10, 1)).is_some());
}
#[test]
fn unknown_stale_and_rounded_zero_never_authorize_automatic_requests() {
    let s = settings();
    let now = at(6, 0);
    for mut report in [
        quota(now, 0.0, Some(at(10, 0))),
        quota(now - Duration::minutes(3), 0.0, None),
    ] {
        let p = runtime_plan(&s, ProviderId::Codex, now, Some(&report), None).unwrap();
        assert!(!p.automatic_allowed);
        report.status = "unavailable".into();
        assert!(
            !runtime_plan(&s, ProviderId::Codex, now, Some(&report), None)
                .unwrap()
                .automatic_allowed
        );
    }
    assert!(
        !runtime_plan(&s, ProviderId::ClaudeCode, now, None, None)
            .unwrap()
            .automatic_allowed
    );
}
#[test]
fn lunch_usage_real_reset_and_temporary_changes_replan_without_changing_defaults() {
    let mut s = settings();
    s.lunch_enabled = true;
    s.lunch_end = "13:30".into();
    let report = quota(at(12, 20), 1.0, Some(at(17, 20)));
    let p = verified_plan(&s, at(12, 20), &report);
    assert_eq!(p.recommendations, vec![at(17, 21)]);
    s.day_override = Some(DayOverride {
        date: at(9, 0).date_naive(),
        work_start: "09:00".into(),
        work_end: "22:00".into(),
        lunch_enabled: false,
    });
    let changed = runtime_plan(&s, ProviderId::Codex, at(12, 20), Some(&report), Some(&p)).unwrap();
    assert_eq!(changed.work_end, at(22, 0));
    assert_eq!(changed.intervals.len(), 1);
    assert_eq!(s.work_end, "18:00");
    assert!(s.lunch_enabled);
    let tomorrow = runtime_plan(&s, ProviderId::Codex, at(23, 0), None, None).unwrap();
    assert_eq!((tomorrow.work_end - tomorrow.work_start).num_hours(), 9);
    assert_eq!(tomorrow.intervals.len(), 2);
}
#[test]
fn missed_slots_and_too_short_remaining_work_are_skipped() {
    let s = settings();
    let p = runtime_plan(&s, ProviderId::Codex, at(6, 0), None, None).unwrap();
    assert!(scheduled_slot(&s, &p, at(6, 9)).is_some());
    assert!(due_for_plan(&s, &p, at(6, 9)).is_none());
    assert!(due_for_plan(&s, &p, at(6, 11)).is_none());
    assert!(due_for_plan(&s, &p, at(18, 0)).is_none());
    let mut short = s.clone();
    short.work_end = "09:30".into();
    let p = runtime_plan(&short, ProviderId::Codex, at(9, 0), None, None).unwrap();
    assert!(due_for_plan(&short, &p, at(9, 0)).is_none());
}
#[test]
fn save_mode_and_cost_protection_are_independent_of_window_count() {
    let mut s = settings();
    s.mode = "save".into();
    assert_eq!(plan(&s).unwrap().times[0], "09:00");
    let r = quota(at(9, 0), 0.0, None);
    assert!(
        !runtime_plan(&s, ProviderId::Codex, at(9, 0), Some(&r), None)
            .unwrap()
            .automatic_allowed
    );
    let after = quota(at(9, 1), 6.0, Some(at(14, 1)));
    assert!(cost_anomaly(&r, &after));
    assert!(!cost_anomaly(&after, &r));
}
#[test]
fn durable_attempt_lock_survives_processes_and_plan_revisions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("codebeat.db");
    let db = Database::open(&path).unwrap();
    let mut settings = db.settings().unwrap();
    settings.pretrigger_enabled = true;
    settings.pretrigger.auto_trigger = true;
    db.save_settings(&settings).unwrap();
    let now = Utc::now();
    let mut r = ProbeRecord {
        provider: ProviderId::Codex,
        slot: "first".into(),
        state: "running".into(),
        message: String::new(),
        attempted_at: now,
        finished_at: None,
        next_check_at: None,
        before: None,
        after: None,
        tokens: None,
        request_sent: true,
        sent_at: None,
    };
    assert!(db.reserve_probe_request(&r, now).unwrap());
    let other = Database::open(&path).unwrap();
    r.slot = "edited-plan".into();
    assert!(!other
        .reserve_probe_request(&r, now + Duration::hours(1))
        .unwrap());
    assert!(other
        .reserve_probe_request(&r, now + Duration::hours(6))
        .unwrap());
}
#[test]
fn quota_refresh_persists_dynamic_plan_and_old_schema_is_upgraded() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("codebeat.db");
    let old = rusqlite::Connection::open(&path).unwrap();
    old.execute_batch(include_str!("../upgrades/001_initial.sql"))
        .unwrap();
    drop(old);
    let db = Database::open(&path).unwrap();
    let mut s = db.settings().unwrap();
    s.pretrigger = settings();
    let now = Utc::now();
    s.pretrigger.timezone = "UTC".into();
    s.pretrigger.work_start = (now - Duration::hours(6)).format("%H:%M").to_string();
    s.pretrigger.work_end = (now + Duration::hours(6)).format("%H:%M").to_string();
    db.save_settings(&s).unwrap();
    db.cache_usage(&quota(
        now - Duration::seconds(60),
        1.0,
        Some(now + Duration::hours(5)),
    ))
    .unwrap();
    let r = quota(now, 1.0, Some(now + Duration::hours(5)));
    db.cache_usage(&r).unwrap();
    let p = db.runtime_pretrigger(ProviderId::Codex).unwrap().unwrap();
    assert_eq!(p.actual_reset, r.windows[0].resets_at_utc);
    assert_eq!(p.status, "active");
}

#[test]
fn wake_timer_is_kept_for_the_next_shift_after_the_last_slot() {
    let mut s = settings();
    s.wake = true;
    s.days = "weekdays".into();
    let friday = Utc.with_ymd_and_hms(2026, 10, 16, 9, 30, 0).unwrap(); // Friday 17:30
    let p = runtime_plan(&s, ProviderId::Codex, friday, None, None).unwrap();
    let wake = next_wake_at(&s, &[p], friday).unwrap().unwrap();
    assert_eq!(wake, Utc.with_ymd_and_hms(2026, 10, 18, 22, 0, 0).unwrap()); // Monday 06:00
    s.mode = "save".into();
    assert!(next_wake_at(&s, &[], friday).unwrap().is_none());
}

#[test]
fn reset_at_lunch_searches_candidates_instead_of_always_waiting_until_lunch_ends() {
    let mut s = settings();
    s.work_start = "10:00".into();
    s.work_end = "20:00".into();
    s.lunch_enabled = true;
    s.lunch_start = "12:00".into();
    s.lunch_end = "14:00".into();
    let r = quota(at(7, 0), 1.0, Some(at(12, 0)));
    let p = verified_plan(&s, at(7, 0), &r);
    assert_eq!(p.recommendations, vec![at(12, 1), at(17, 2)]);
    assert!(due_for_plan(&s, &p, at(12, 0)).is_none()); // still needs a fresh reset check
}

#[test]
fn display_switches_at_work_end_instead_of_waiting_for_midnight() {
    let s = settings();
    let execution = runtime_plan(&s, ProviderId::Codex, at(23, 0), None, None).unwrap();
    assert_eq!(execution.date.to_string(), "2026-10-20");
    let shown = display_plan(&s, ProviderId::Codex, at(23, 0), None, Some(&execution)).unwrap();
    assert_eq!(shown.date.to_string(), "2026-10-20");
    assert_eq!(shown.work_end, at(18, 0) + Duration::days(1));
    let before_end = display_plan(&s, ProviderId::Codex, at(17, 59), None, None).unwrap();
    assert_eq!(before_end.date.to_string(), "2026-10-19");
    let at_end = display_plan(&s, ProviderId::Codex, at(18, 0), None, None).unwrap();
    assert_eq!(at_end.date.to_string(), "2026-10-20");
    let tomorrow = display_plan(
        &s,
        ProviderId::Codex,
        at(0, 0) + Duration::days(1),
        None,
        None,
    )
    .unwrap();
    assert_eq!(tomorrow.date.to_string(), "2026-10-20");
}

#[test]
fn reset_boundary_waits_a_full_minute_including_seconds_and_wake_timer() {
    let mut s = settings();
    s.wake = true;
    let reset = at(11, 0) + Duration::seconds(35);
    let active = quota(at(10, 0), 1.0, Some(reset));
    let previous = verified_plan(&s, at(10, 0), &active);
    assert_eq!(previous.recommendations[0], reset + Duration::minutes(1));
    assert_eq!(
        next_wake_at(&s, std::slice::from_ref(&previous), at(10, 0)).unwrap(),
        Some(reset + Duration::minutes(1))
    );
    let idle = quota(reset, 0.0, Some(reset));
    let plan = runtime_plan(&s, ProviderId::Codex, reset, Some(&idle), Some(&previous)).unwrap();
    assert!(due_for_plan(&s, &plan, reset + Duration::seconds(59)).is_none());
    let ready = runtime_plan(
        &s,
        ProviderId::Codex,
        reset + Duration::seconds(60),
        Some(&idle),
        Some(&plan),
    )
    .unwrap();
    assert!(due_for_plan(&s, &ready, reset + Duration::seconds(60)).is_some());
}

#[test]
fn confirmed_manual_windows_survive_failed_refresh_restart_and_later_windows() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("codebeat.db");
    let db = Database::open(&file).unwrap();
    let now = Utc::now();
    let mut s = db.settings().unwrap();
    s.pretrigger.timezone = "UTC".into();
    s.pretrigger.days = "daily".into();
    s.pretrigger.work_start = (now - Duration::hours(6)).format("%H:%M").to_string();
    s.pretrigger.work_end = (now + Duration::hours(6)).format("%H:%M").to_string();
    db.save_settings(&s).unwrap();
    db.cache_usage(&quota(
        now - Duration::seconds(120),
        2.0,
        Some(now + Duration::hours(1)),
    ))
    .unwrap();
    db.cache_usage(&quota(
        now - Duration::seconds(80),
        2.0,
        Some(now + Duration::hours(1)),
    ))
    .unwrap();
    db.cache_usage(&quota(
        now - Duration::seconds(60),
        3.0,
        Some(now + Duration::hours(2)),
    ))
    .unwrap();
    db.cache_usage(&quota(now, 3.0, Some(now + Duration::hours(2))))
        .unwrap();
    let mut unavailable = quota(now, 0.0, None);
    unavailable.status = "unavailable".into();
    db.cache_usage(&unavailable).unwrap();
    drop(db);
    let reopened = Database::open(&file).unwrap();
    let windows = reopened.optimizer_confirmed_windows(now).unwrap();
    assert_eq!(windows.len(), 2);
    let shown = reopened.optimizer_display_status(now).unwrap();
    assert_eq!(shown[0].actual_reset, Some(now + Duration::hours(2)));
    assert!(windows
        .iter()
        .all(|w| w.end - w.start == Duration::hours(5)));
    assert!(reopened
        .optimizer_confirmed_windows(now + Duration::days(2))
        .unwrap()
        .is_empty());
}

#[test]
fn rolling_zero_percent_snapshots_never_become_trigger_history_or_verified_anchors() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rolling-zero.db");
    let db = Database::open(&path).unwrap();
    let now = Utc::now();
    let mut settings = db.settings().unwrap();
    settings.pretrigger.timezone = "UTC".into();
    settings.pretrigger.days = "daily".into();
    settings.pretrigger.work_start = (now - Duration::hours(6)).format("%H:%M").to_string();
    settings.pretrigger.work_end = (now + Duration::hours(6)).format("%H:%M").to_string();
    db.save_settings(&settings).unwrap();
    for seconds in 0..30 {
        let observed = now + Duration::seconds(seconds);
        db.cache_usage(&quota(observed, 0.0, Some(observed + Duration::hours(5))))
            .unwrap();
    }
    assert!(db.optimizer_confirmed_windows(now).unwrap().is_empty());
    assert!(db.pretrigger_runs().unwrap().is_empty());
    let report = quota(now, 0.0, Some(now + Duration::hours(5)));
    let mut legacy =
        runtime_plan(&settings.pretrigger, ProviderId::Codex, now, None, None).unwrap();
    legacy.actual_reset = report.windows[0].resets_at_utc;
    legacy.status = "active".into();
    let corrected = runtime_plan(
        &settings.pretrigger,
        ProviderId::Codex,
        now,
        Some(&report),
        Some(&legacy),
    )
    .unwrap();
    assert!(!corrected.window_confirmed);
    assert!(corrected.actual_reset.is_none());
    assert_eq!(corrected.status, "unknown");
    assert!(!corrected.automatic_allowed);
}

#[test]
fn zero_percent_can_retain_a_previously_verified_window_but_cannot_move_it() {
    let s = settings();
    let report = quota(at(9, 0), 1.0, Some(at(14, 0)));
    let verified = verified_plan(&s, at(9, 0), &report);
    assert!(verified.window_confirmed);
    let rounded = quota(at(9, 1), 0.0, Some(at(14, 0)));
    let retained = runtime_plan(
        &s,
        ProviderId::Codex,
        at(9, 1),
        Some(&rounded),
        Some(&verified),
    )
    .unwrap();
    assert_eq!(retained.actual_reset, Some(at(14, 0)));
    assert_eq!(retained.status, "active");
    let moving = quota(at(9, 10), 0.0, Some(at(14, 10)));
    let unchanged = runtime_plan(
        &s,
        ProviderId::Codex,
        at(9, 10),
        Some(&moving),
        Some(&retained),
    )
    .unwrap();
    assert_eq!(unchanged.actual_reset, Some(at(14, 0)));
    assert_eq!(unchanged.status, "unknown");
    assert!(!unchanged.automatic_allowed);
}

#[test]
fn day_baseline_stays_fixed_and_a_single_overlapping_outlier_is_not_a_window() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("windows.db");
    let db = Database::open(&path).unwrap();
    let mut s = db.settings().unwrap();
    s.pretrigger.work_start = "10:00".into();
    s.pretrigger.work_end = "20:00".into();
    s.pretrigger.days = "daily".into();
    db.save_settings(&s).unwrap();
    for (h, m, reset_h, reset_m) in [
        (7, 10, 12, 10),
        (7, 11, 12, 10),
        (14, 1, 19, 1),
        (15, 4, 20, 4),
        (15, 5, 20, 4),
    ] {
        db.cache_usage(&quota(at(h, m), 1., Some(at(reset_h, reset_m))))
            .unwrap();
    }
    let windows = db.optimizer_confirmed_windows(at(15, 6)).unwrap();
    assert_eq!(windows.len(), 2);
    assert!(windows.iter().all(|w| w.start != at(14, 1)));
    let p = db.optimizer_display_status(at(15, 6)).unwrap().remove(0);
    assert_eq!(p.actual_reset, Some(at(20, 4)));
    assert_eq!(p.day_start, Some(at(7, 10)));
    assert_eq!(p.display_times, vec![at(7, 10), at(12, 10), at(17, 10)]);
    assert!(p.recommendations.is_empty());
    drop(db);
    let reopened = Database::open(&path).unwrap();
    assert_eq!(
        reopened.optimizer_display_status(at(15, 6)).unwrap()[0].display_times,
        p.display_times
    );
}

#[test]
fn only_moving_idle_evidence_authorizes_the_first_request() {
    let s = PretriggerSettings {
        days: "daily".into(),
        ..Default::default()
    };
    let now = at(7, 30);
    let first = quota(now, 0., Some(now + Duration::hours(5)));
    let p = runtime_plan(&s, ProviderId::Codex, now, Some(&first), None).unwrap();
    assert!(!p.automatic_allowed);
    assert!(due_for_plan(&s, &p, now).is_none());
    let later = now + Duration::seconds(3);
    let stationary = quota(later, 0., Some(now + Duration::hours(5)));
    assert!(
        !runtime_plan(&s, ProviderId::Codex, later, Some(&stationary), Some(&p))
            .unwrap()
            .automatic_allowed
    );
    let moving = quota(later, 0., Some(later + Duration::hours(5)));
    let ready = runtime_plan(&s, ProviderId::Codex, later, Some(&moving), Some(&p)).unwrap();
    assert!(ready.automatic_allowed);
    assert!(due_for_plan(&s, &ready, later).is_some());
    assert!(ready.actual_reset.is_none());
    assert!(!ready.window_confirmed);
    assert_eq!(ready.display_times, vec![at(7, 30), at(12, 30), at(17, 30)]);
    assert_eq!(
        ready.recommendations,
        vec![at(7, 30), at(12, 31), at(17, 32)]
    );
}

#[test]
fn disabled_provider_cache_cannot_recreate_a_stale_runtime() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(&dir.path().join("disabled.db")).unwrap();
    db.replan_provider(ProviderId::ClaudeCode, None, Utc::now())
        .unwrap();
    let mut report = quota(Utc::now(), 0., None);
    report.provider = ProviderId::ClaudeCode;
    db.cache_usage(&report).unwrap();
    assert!(db
        .runtime_pretrigger(ProviderId::ClaudeCode)
        .unwrap()
        .is_none());
}

#[test]
fn official_early_reset_closes_a_real_window_without_erasing_its_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(&dir.path().join("early-reset.db")).unwrap();
    let mut s = db.settings().unwrap();
    s.pretrigger.days = "daily".into();
    db.save_settings(&s).unwrap();
    for (h, m, used, rh, rm) in [
        (7, 10, 1., 12, 10),
        (7, 11, 2., 12, 10),
        (14, 11, 1., 19, 1),
        (14, 21, 1., 19, 1),
        (14, 31, 1., 19, 1),
        (14, 43, 0., 19, 43),
        (15, 4, 1., 20, 4),
        (15, 5, 2., 20, 4),
    ] {
        db.cache_usage(&quota(at(h, m), used, Some(at(rh, rm))))
            .unwrap();
    }
    let windows = db.optimizer_confirmed_windows(at(15, 6)).unwrap();
    assert_eq!(windows.len(), 3);
    let middle = windows.iter().find(|w| w.start == at(14, 1)).unwrap();
    assert_eq!(middle.end, at(19, 1));
    assert_eq!(middle.closed_at, Some(at(14, 43)));
    let before_next = db.optimizer_display_status(at(14, 45)).unwrap().remove(0);
    assert_eq!(before_next.latest_start, Some(at(14, 1)));
    assert_eq!(before_next.actual_reset, Some(at(14, 43)));
    assert_eq!(before_next.recommendations, vec![at(17, 12)]);
    let after = db.optimizer_display_status(at(15, 6)).unwrap().remove(0);
    assert_eq!(after.actual_reset, Some(at(20, 4)));
}
