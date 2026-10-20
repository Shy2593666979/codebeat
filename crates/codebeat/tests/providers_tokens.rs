use chrono::{DateTime, Utc};
use codebeat_lib::usage::{TokenReport, TokenScanner};
use codebeat_lib::ProviderId;
use serde_json::{json, Value};
use std::io::Write;

fn now() -> DateTime<Utc> {
    "2026-10-16T14:00:00Z".parse().unwrap()
}
fn codex(at: &str, input: u64, output: u64, last_input: u64, last_output: u64) -> Value {
    json!({"type":"event_msg","timestamp":at,"payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":input,"output_tokens":output,"cached_input_tokens":input/2,"reasoning_output_tokens":output/2},"last_token_usage":{"input_tokens":last_input,"output_tokens":last_output,"cached_input_tokens":last_input/2,"reasoning_output_tokens":last_output/2}}}})
}
fn claude(at: &str, id: &str, output: u64) -> Value {
    json!({"type":"assistant","timestamp":at,"message":{"id":id,"model":"test","usage":{"input_tokens":10,"output_tokens":output,"cache_read_input_tokens":20,"cache_creation_input_tokens":30}}})
}
fn write(path: &std::path::Path, rows: &[Value]) {
    std::fs::write(
        path,
        rows.iter().map(|v| format!("{v}\n")).collect::<String>(),
    )
    .unwrap();
}
fn total(report: &TokenReport, provider: ProviderId) -> (u64, u64, u64) {
    report
        .days
        .iter()
        .filter(|d| d.provider == provider)
        .fold((0, 0, 0), |a, d| {
            (
                a.0 + d.counts.input,
                a.1 + d.counts.output,
                a.2 + d.requests,
            )
        })
}
fn meta() -> Value {
    json!({"type":"session_meta","payload":{"id":"thread-a"}})
}

#[test]
fn codex_deltas_duplicates_and_cached_subset_are_counted_once() {
    let root = tempfile::tempdir().unwrap();
    let rows = [
        meta(),
        codex("2026-10-15T10:00:00Z", 100, 20, 100, 20),
        codex("2026-10-15T10:00:01Z", 100, 20, 100, 20),
        codex("2026-10-16T10:00:00Z", 150, 30, 50, 10),
    ];
    write(&root.path().join("a.jsonl"), &rows);
    write(&root.path().join("copied.jsonl"), &rows);
    let r = TokenScanner::default()
        .report(
            &[(ProviderId::Codex, root.path().into())],
            7,
            "Asia/Shanghai",
            now(),
        )
        .unwrap();
    assert_eq!(total(&r, ProviderId::Codex), (150, 30, 2));
    assert_eq!(r.days.iter().map(|d| d.counts.cache_read).sum::<u64>(), 75);
    assert_eq!(r.sources[0].status, "available");
}
#[test]
fn baseline_before_range_is_not_assigned_to_today_and_midnight_uses_requested_zone() {
    let root = tempfile::tempdir().unwrap();
    write(
        &root.path().join("a.jsonl"),
        &[
            meta(),
            codex("2026-09-01T00:00:00Z", 100, 20, 100, 20),
            codex("2026-10-15T16:01:00Z", 150, 30, 50, 10),
        ],
    );
    let r = TokenScanner::default()
        .report(
            &[(ProviderId::Codex, root.path().into())],
            1,
            "Asia/Shanghai",
            now(),
        )
        .unwrap();
    assert_eq!(total(&r, ProviderId::Codex), (50, 10, 1));
    assert_eq!(r.days[0].date, "2026-10-16");
}
#[test]
fn claude_streamed_duplicates_and_subagents_use_message_id_and_final_maximum() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("subagents")).unwrap();
    write(
        &root.path().join("a.jsonl"),
        &[
            claude("2026-10-16T10:00:00Z", "m1", 0),
            claude("2026-10-16T10:00:01Z", "m1", 12),
        ],
    );
    write(
        &root.path().join("subagents/agent.jsonl"),
        &[
            claude("2026-10-16T10:00:00Z", "m1", 12),
            claude("2026-10-16T10:01:00Z", "m2", 8),
        ],
    );
    let r = TokenScanner::default()
        .report(
            &[(ProviderId::ClaudeCode, root.path().into())],
            7,
            "UTC",
            now(),
        )
        .unwrap();
    assert_eq!(total(&r, ProviderId::ClaudeCode), (120, 20, 2));
    assert_eq!(r.days.iter().map(|d| d.counts.cache_read).sum::<u64>(), 40);
}
#[test]
fn append_partial_line_and_truncation_refresh_without_double_counting() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("a.jsonl");
    let roots = [(ProviderId::ClaudeCode, root.path().into())];
    let mut scanner = TokenScanner::default();
    write(&path, &[claude("2026-10-16T10:00:00Z", "m1", 5)]);
    assert_eq!(
        total(
            &scanner.report(&roots, 7, "UTC", now()).unwrap(),
            ProviderId::ClaudeCode
        ),
        (60, 5, 1)
    );
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
    write!(file, "{}", claude("2026-10-16T10:01:00Z", "m2", 7)).unwrap();
    assert_eq!(
        total(
            &scanner.report(&roots, 7, "UTC", now()).unwrap(),
            ProviderId::ClaudeCode
        ),
        (60, 5, 1)
    );
    writeln!(file).unwrap();
    assert_eq!(
        total(
            &scanner.report(&roots, 7, "UTC", now()).unwrap(),
            ProviderId::ClaudeCode
        ),
        (120, 12, 2)
    );
    write(&path, &[claude("2026-10-16T10:02:00Z", "new", 1)]);
    assert_eq!(
        total(
            &scanner.report(&roots, 7, "UTC", now()).unwrap(),
            ProviderId::ClaudeCode
        ),
        (60, 1, 1)
    );
}
#[test]
fn missing_logs_are_distinguished_from_zero_and_invalid_timezone_is_rejected() {
    let root = tempfile::tempdir().unwrap();
    let mut scanner = TokenScanner::default();
    let r = scanner
        .report(
            &[(ProviderId::Codex, root.path().join("missing"))],
            7,
            "UTC",
            now(),
        )
        .unwrap();
    assert_eq!(r.sources[0].status, "missing");
    assert_eq!(total(&r, ProviderId::Codex), (0, 0, 0));
    assert!(scanner.report(&[], 7, "bad/timezone", now()).is_err());
}
#[test]
fn missing_codex_prefix_and_regressing_counter_are_explicitly_partial() {
    let root = tempfile::tempdir().unwrap();
    write(
        &root.path().join("a.jsonl"),
        &[
            meta(),
            codex("2026-10-16T10:00:00Z", 500, 100, 50, 10),
            codex("2026-10-16T10:01:00Z", 400, 90, 40, 9),
        ],
    );
    let r = TokenScanner::default()
        .report(&[(ProviderId::Codex, root.path().into())], 7, "UTC", now())
        .unwrap();
    assert_eq!(r.sources[0].status, "partial");
    assert_eq!(total(&r, ProviderId::Codex), (50, 10, 1));
}
#[tokio::test]
#[ignore = "Reads local usage counters only; never sends a model request"]
async fn live_local_token_probe() {
    let mut report = codebeat_lib::usage::local_usage(7, "Asia/Shanghai".into())
        .await
        .unwrap();
    for _ in 0..8 {
        if !report
            .sources
            .iter()
            .any(|s| s.warnings.iter().any(|w| w.contains("增量扫描")))
        {
            break;
        }
        report = codebeat_lib::usage::local_usage(7, "Asia/Shanghai".into())
            .await
            .unwrap();
    }
    assert!(report.sources.iter().any(|s| s.files > 0));
    println!("{}", serde_json::to_string(&report).unwrap());
}

#[test]
fn independent_threads_with_identical_counters_are_distinct_and_fork_prefix_is_skipped() {
    let root = tempfile::tempdir().unwrap();
    let event = codex("2026-10-16T10:00:00Z", 100, 20, 100, 20);
    write(&root.path().join("a.jsonl"), &[meta(), event.clone()]);
    write(
        &root.path().join("b.jsonl"),
        &[
            json!({"type":"session_meta","payload":{"id":"thread-b"}}),
            event.clone(),
        ],
    );
    write(
        &root.path().join("fork.jsonl"),
        &[
            json!({"type":"session_meta","payload":{"id":"fork","timestamp":"2026-10-16T10:30:00Z"}}),
            event,
            codex("2026-10-16T11:00:00Z", 150, 30, 50, 10),
        ],
    );
    let r = TokenScanner::default()
        .report(&[(ProviderId::Codex, root.path().into())], 7, "UTC", now())
        .unwrap();
    assert_eq!(total(&r, ProviderId::Codex), (250, 50, 3));
}
