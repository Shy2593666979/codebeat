use serde_json::{json, Value};
use std::io::{self, BufRead, Write};
fn main() {
    let first = std::env::args().nth(1).unwrap_or_default();
    if first == "--pretrigger-args" {
        println!(
            "{}",
            serde_json::to_string(&codebeat_lib::providers::pretrigger_args(
                codebeat_lib::ProviderId::Codex
            ))
            .unwrap()
        );
        return;
    }
    if first == "--discover-codex" {
        println!(
            "{}",
            codebeat_lib::providers::executable(codebeat_lib::ProviderId::Codex)
                .unwrap()
                .display()
        );
        return;
    }
    if first == "--version" {
        println!("fixture 1.0");
        return;
    }
    if first == "auth" {
        println!("{}", json!({"loggedIn":true}));
        return;
    }
    if first == "--help" || (first == "exec" && std::env::args().any(|a| a == "--help")) {
        println!("--ephemeral --sandbox --json --ignore-user-config --ignore-rules --max-turns --tools --no-session-persistence --model");
        return;
    }
    if first == "exec" || first == "-p" {
        if let Ok(path) = std::env::var("CODEBEAT_FIXTURE_ARGS_PATH") {
            std::fs::write(
                path,
                serde_json::to_vec(&std::env::args().skip(1).collect::<Vec<_>>()).unwrap(),
            )
            .unwrap();
        }
        use std::io::Read;
        let mut prompt = String::new();
        std::io::stdin().read_to_string(&mut prompt).unwrap();
        if let Ok(expected) = std::env::var("CODEBEAT_FIXTURE_EXPECT_PROMPT") {
            assert_eq!(prompt, expected);
        }
        if let Ok(path) = std::env::var("CODEBEAT_FIXTURE_PROBE_MARKER") {
            let mut marker = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap();
            marker.write_all(b"x").unwrap();
        }
        eprintln!("isolated fixture stderr");
        let mut outcome =
            std::env::var("CODEBEAT_FIXTURE_OUTCOME").unwrap_or_else(|_| "succeeded".into());
        if let Ok(first) = std::env::var("CODEBEAT_FIXTURE_FAIL_FIRST") {
            let attempts = std::fs::read(std::env::var("CODEBEAT_FIXTURE_PROBE_MARKER").unwrap())
                .unwrap()
                .len();
            if attempts <= first.parse::<usize>().unwrap() {
                outcome = "failed".into();
            }
        }
        match outcome.as_str() {
            "reconnected" => {
                println!(
                    "{}",
                    json!({"type":"error","message":"Reconnecting... 2/5"})
                );
                println!(
                    "{}",
                    json!({"type":"turn.completed","usage":{"output_tokens":5}})
                );
            }
            "succeeded" => {
                if first == "-p" {
                    println!(
                        "{}",
                        json!({"type":"result","subtype":"success","is_error":false})
                    );
                } else {
                    println!(
                        "{}",
                        json!({"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":80,"output_tokens":5}})
                    );
                }
            }
            "blocked_approval" => println!("{}", json!({"type":"approval_needed"})),
            "unverified" => println!("{}", json!({"type":"item.completed"})),
            _ => {
                println!("{}", json!({"type":"turn.failed"}));
                std::process::exit(1);
            }
        }
        return;
    }
    if std::env::args().nth(1).as_deref() != Some("app-server") {
        return;
    }
    eprintln!("fixture stderr must never be parsed as JSON");
    let mut initialized = false;
    let mut redeemed = std::collections::HashSet::new();
    let mut redemption_calls = 0;
    let mut reset_outcome = "reset".to_string();
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let v: Value = serde_json::from_str(&line).unwrap();
        if v["method"] == "initialized" {
            initialized = true;
            continue;
        }
        if v["id"].is_null() {
            continue;
        }
        let method = v["method"].as_str().unwrap_or("");
        if let Ok(path) = std::env::var("CODEBEAT_FIXTURE_RPC_LOG") {
            let mut log = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .unwrap();
            writeln!(log, "{method}").unwrap();
        }
        if method == "account/rateLimits/read" && initialized {
            if let Ok(path) = std::env::var("CODEBEAT_FIXTURE_USAGE_CONTROL") {
                let control: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(
                    control["delay_ms"].as_u64().unwrap_or(0),
                ));
                let response = if control["fail"] == true {
                    json!({"id":v["id"],"error":{"code":-32603,"message":"failed to fetch codex rate limits: error sending request for url (https://chatgpt.com/backend-api/wham/usage)"}})
                } else {
                    json!({"id":v["id"],"result":{"rateLimits":{"primary":{"usedPercent":control["used_percent"].as_f64().unwrap_or(17.),"windowDurationMins":300,"resetsAt":chrono::Utc::now().timestamp()+18000}},"rateLimitResetCredits":{"availableCount":2}}})
                };
                println!("{response}");
                io::stdout().flush().unwrap();
                continue;
            }
        }
        if method == "fixture/exit" {
            std::process::exit(0);
        }
        if method == "fixture/hang" {
            std::thread::sleep(std::time::Duration::from_secs(60));
            continue;
        }
        let result = match method {
            "initialize" => json!({"userAgent":"fixture"}),
            "account/read" => {
                json!({"account":{"type":"chatgpt","email":"fixture@example.invalid","planType":"plus"}})
            }
            "fixture/reset_outcome" => {
                reset_outcome = v["params"]["outcome"].as_str().unwrap().into();
                json!({"ok":true})
            }
            "fixture/redemption_stats" => {
                json!({"calls":redemption_calls,"consumed":redeemed.len()})
            }
            "account/rateLimitResetCredit/consume" if initialized => {
                redemption_calls += 1;
                let key = v["params"]["idempotencyKey"].as_str().unwrap();
                if redeemed.contains(key) {
                    json!({"outcome":"alreadyRedeemed"})
                } else {
                    if reset_outcome == "reset" {
                        redeemed.insert(key.to_string());
                    }
                    json!({"outcome":reset_outcome})
                }
            }
            "account/rateLimits/read" if initialized => {
                if let Ok(path) = std::env::var("CODEBEAT_FIXTURE_PROBE_MARKER") {
                    let mut sent = std::path::Path::new(&path).exists();
                    if let Ok(first) = std::env::var("CODEBEAT_FIXTURE_FAIL_FIRST") {
                        sent = std::fs::read(&path).unwrap_or_default().len()
                            > first.parse::<usize>().unwrap();
                    }
                    let active = std::env::var("CODEBEAT_FIXTURE_ACTIVE").is_ok();
                    let floating = std::env::var("CODEBEAT_FIXTURE_FLOATING").is_ok();
                    let zero_after = std::env::var("CODEBEAT_FIXTURE_ZERO_AFTER").is_ok();
                    json!({"rateLimits":{"primary":{"usedPercent":if active || (sent && std::env::var("CODEBEAT_FIXTURE_HIGH_COST").is_ok()){17.0}else if sent&&!zero_after{0.2}else{0.0},"windowDurationMins":300,"resetsAt":if sent||active||floating{chrono::Utc::now().timestamp()+18000}else{chrono::Utc::now().timestamp()-60}}}})
                } else {
                    json!({"rateLimits":{"primary":{"usedPercent":17,"windowDurationMins":300,"resetsAt":1792195200}},"rateLimitResetCredits":{"availableCount":2,"credits":null}})
                }
            }
            "thread/list" if initialized => json!({"data":[],"nextCursor":"cursor-2"}),
            "thread/read" if initialized => {
                json!({"thread":{"id":v["params"]["threadId"],"cwd":std::env::var("CODEBEAT_FIXTURE_CWD").unwrap_or_else(|_|".".into()),"name":"fixture","ephemeral":false}})
            }
            _ => json!({"ok":initialized}),
        };
        println!(
            "{}",
            json!({"method":"account/rateLimits/updated","params":{}})
        );
        println!("{}", json!({"id":v["id"],"result":result}));
        io::stdout().flush().unwrap();
    }
}
