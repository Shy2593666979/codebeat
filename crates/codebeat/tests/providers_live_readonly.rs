use codebeat_lib::ProviderId;
#[tokio::test]
#[ignore = "Read-only desktop engine discovery, quota and metadata; NEVER consumes reset credits or sends prompts"]
async fn installed_desktop_readonly_probe() {
    use codebeat_lib::providers::{discovery, read_usage_with_rpc, rpc::RpcClient};
    use serde_json::json;
    let engine = discovery::desktop_executable().expect("Desktop native engine must be installed");
    // 子进程没有 npm 安装，且 PATH 只含系统目录，用以证明仅靠桌面端的
    // 发现流程即可工作，同时不改动真实用户的环境和已安装的 CLI。
    let empty_appdata = tempfile::tempdir().unwrap();
    let system_root = std::env::var_os("SystemRoot").unwrap();
    let root = std::path::PathBuf::from(system_root);
    let path = std::env::join_paths([
        root.join("System32"),
        root.join("System32/WindowsPowerShell/v1.0"),
    ])
    .unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_codebeat-protocol-fixture"))
        .arg("--discover-codex")
        .env("APPDATA", empty_appdata.path())
        .env("PATH", path)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        std::path::PathBuf::from(String::from_utf8(output.stdout).unwrap().trim()),
        engine
    );
    let mut rpc = RpcClient::connect(&engine).await.unwrap();
    let (windows, credits) = read_usage_with_rpc(&mut rpc).await.unwrap();
    assert!(!windows.is_empty());
    let page = rpc
        .request(
            "thread/list",
            json!({"limit":1,"sourceKinds":["appServer"]}),
        )
        .await
        .unwrap();
    assert!(page["data"].is_array());
    let interactive = rpc
        .request("thread/list", json!({"limit":1,"sourceKinds":[]}))
        .await
        .unwrap();
    let mut detail_ok = false;
    if let Some(id) = interactive["data"].get(0).and_then(|t| t["id"].as_str()) {
        let detail = rpc
            .request("thread/read", json!({"threadId":id,"includeTurns":false}))
            .await
            .unwrap();
        detail_ok = detail["thread"]["id"] == id;
        assert!(detail_ok);
    }
    println!("desktop_only_discovery=true quota_windows={} reset_count_available={} reset_identity_available={} app_server_source_sessions={} interactive_sessions={} detail_ok={} model_requests=0 reset_consumptions=0",windows.len(),credits.is_some(),credits.as_ref().is_some_and(|c|c.account_key.is_some()),page["data"].as_array().unwrap().len(),interactive["data"].as_array().unwrap().len(),detail_ok);
    rpc.close().await;
}
#[tokio::test]
#[ignore = "Uses installed official CLI for read-only health, quota and session queries; never sends model prompts"]
async fn installed_cli_readonly_probe() {
    for id in [ProviderId::Codex, ProviderId::ClaudeCode] {
        let health = codebeat_lib::providers::detect(id).await;
        let usage = codebeat_lib::providers::usage(id).await;
        let page = codebeat_lib::providers::sessions(id, "", None, 5).await;
        println!(
            "provider={} health={} auth={} quota={} windows={} session_query={}",
            id.as_str(),
            health.status,
            health.auth_status,
            usage.status,
            usage.windows.len(),
            match &page {
                Ok(p) => format!("{} entries, next={}", p.data.len(), p.next_cursor.is_some()),
                Err(e) => codebeat_lib::providers::redact(&e.to_string()),
            }
        );
        if id == ProviderId::Codex && usage.status != "available" {
            println!("quota_diagnostic={}", usage.message);
        }
        assert!(!health.message.is_empty());
    }
}
