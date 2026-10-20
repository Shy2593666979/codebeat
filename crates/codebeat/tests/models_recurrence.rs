use chrono::{TimeZone, Utc};
use codebeat_lib::*;
fn request() -> CreateTaskRequest {
    CreateTaskRequest {
        locked_session_behavior: Default::default(),
        recurrence: Some(Recurrence::Daily),
        conversation: Conversation {
            lock_state: Default::default(),
            provider: ProviderId::Codex,
            session_id: "d27dfaa2-8399-401d-9428-e2cd4fe18190".into(),
            title: "test".into(),
            cwd: ".".into(),
            last_updated_utc: None,
            resumable: true,
            source: "test".into(),
        },
        title: "daily".into(),
        prompt: "read".into(),
        scheduled_at_utc: Utc.with_ymd_and_hms(2026, 3, 7, 11, 0, 0).unwrap(),
        scheduled_local: "2026-03-07T06:00".into(),
        timezone_id: "America/New_York".into(),
        wake_to_run: false,
        catch_up_policy: CatchUpPolicy::Ask,
        catch_up_ttl_minutes: 720,
        permission_profile: PermissionProfile::ReadOnly,
        consent: true,
        notify: false,
    }
}
#[test]
fn daily_preserves_wall_clock_across_dst() {
    let r = request();
    let next = next_occurrence(&r, r.scheduled_at_utc).unwrap().unwrap();
    assert_eq!(next.scheduled_local, "2026-03-08T06:00");
    assert_eq!((next.scheduled_at_utc - r.scheduled_at_utc).num_hours(), 23);
}
#[test]
fn intervals_skip_elapsed_occurrences_without_burst_replay() {
    let mut r = request();
    r.recurrence = Some(Recurrence::EveryHours { hours: 6 });
    let now = r.scheduled_at_utc + chrono::Duration::hours(25);
    let next = next_occurrence(&r, now).unwrap().unwrap();
    assert_eq!((next.scheduled_at_utc - r.scheduled_at_utc).num_hours(), 30);
    r.recurrence = Some(Recurrence::EveryHours { hours: 0 });
    assert!(next_occurrence(&r, now).is_err());
}
#[test]
fn delayed_daily_keeps_next_future_wall_time() {
    let r = request();
    let now = r.scheduled_at_utc + chrono::Duration::hours(22);
    let next = next_occurrence(&r, now).unwrap().unwrap();
    assert_eq!(next.scheduled_local, "2026-03-08T06:00");
}
#[test]
fn one_off_does_not_requeue() {
    let mut r = request();
    r.recurrence = None;
    assert!(next_occurrence(&r, r.scheduled_at_utc).unwrap().is_none());
}
