use codebeat_lib::providers::{parse_usage, rpc::RpcClient};
use serde_json::json;
#[tokio::test]
async fn handshake_ids_notifications_stderr_and_pagination() {
    let mut rpc = RpcClient::connect(std::path::Path::new(env!(
        "CARGO_BIN_EXE_codebeat-protocol-fixture"
    )))
    .await
    .unwrap();
    let data = rpc
        .request("account/rateLimits/read", json!({}))
        .await
        .unwrap();
    assert_eq!(parse_usage(&data)[0].used_percent, Some(17.0));
    let page = rpc
        .request("thread/list", json!({"limit":10}))
        .await
        .unwrap();
    assert_eq!(page["nextCursor"], "cursor-2");
    rpc.close().await;
}
#[tokio::test]
async fn exited_server_is_not_success() {
    let mut rpc = RpcClient::connect(std::path::Path::new(env!(
        "CARGO_BIN_EXE_codebeat-protocol-fixture"
    )))
    .await
    .unwrap();
    assert!(rpc.request("fixture/exit", json!({})).await.is_err());
    rpc.close().await;
}
