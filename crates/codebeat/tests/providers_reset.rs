//! 这里所有兑换都针对测试夹具的可执行文件，绝不针对已安装的 Codex。
use codebeat_lib::providers::{read_usage_with_rpc, reset::*, rpc::RpcClient};
use codebeat_lib::ResetOutcome;
use serde_json::json;

async fn fixture() -> RpcClient {
    RpcClient::connect(std::path::Path::new(env!(
        "CARGO_BIN_EXE_codebeat-protocol-fixture"
    )))
    .await
    .unwrap()
}
async fn identity(rpc: &mut RpcClient) -> String {
    account_key(&rpc.request("account/read", json!({})).await.unwrap()).unwrap()
}
#[test]
fn count_is_authoritative_and_missing_is_not_zero() {
    for v in [
        json!({}),
        json!({"rateLimitResetCredits":null}),
        json!({"rateLimitResetCredits":{"availableCount":-1}}),
    ] {
        assert!(parse_credits(&v).is_none());
    }
    for credits in [json!(null), json!([]), json!([{"id":"one"}])] {
        assert_eq!(
            parse_credits(&json!({"rateLimitResetCredits":{"availableCount":3,"credits":credits}}))
                .unwrap()
                .available_count,
            3
        );
    }
    assert_eq!(
        parse_credits(&json!({"rateLimitResetCredits":{"availableCount":0}}))
            .unwrap()
            .available_count,
        0
    );
    assert!(account_key(&json!({"account":{"type":"apiKey"}})).is_none());
}

#[test]
fn credit_expiration_preserves_unknown_and_never_expiring_as_different_states() {
    let credits = parse_credits(
        &json!({"rateLimitResetCredits":{"availableCount":4,"credits":[
            {"id":"expiring","status":"available","expiresAt":1792500000},
            {"id":"forever","status":"available","expiresAt":null},
            {"id":"unknown","status":"available"},
            {"id":"invalid","status":"available","expiresAt":"bad"}
        ]}}),
    )
    .unwrap()
    .credits
    .unwrap();
    assert!(credits[0].expiry_known && credits[0].expires_at.is_some());
    assert!(credits[1].expiry_known && credits[1].expires_at.is_none());
    assert!(!credits[2].expiry_known && !credits[3].expiry_known);
}
#[tokio::test]
async fn polling_reads_count_without_ever_redeeming() {
    let mut rpc = fixture().await;
    for _ in 0..3 {
        let (windows, credits) = read_usage_with_rpc(&mut rpc).await.unwrap();
        assert_eq!(windows.len(), 1);
        assert_eq!(credits.unwrap().available_count, 2);
    }
    let stats = rpc
        .request("fixture/redemption_stats", json!({}))
        .await
        .unwrap();
    assert_eq!(stats, json!({"calls":0,"consumed":0}));
    rpc.close().await;
}
#[tokio::test]
async fn fixture_redemption_retries_same_key_only_once() {
    let mut rpc = fixture().await;
    let account = identity(&mut rpc).await;
    let key = uuid::Uuid::new_v4().to_string();
    assert_eq!(
        consume_with_rpc(&mut rpc, &key, &account).await.unwrap(),
        ResetOutcome::Reset
    );
    assert_eq!(
        consume_with_rpc(&mut rpc, &key, &account).await.unwrap(),
        ResetOutcome::AlreadyRedeemed
    );
    let stats = rpc
        .request("fixture/redemption_stats", json!({}))
        .await
        .unwrap();
    assert_eq!(stats["consumed"], 1);
    rpc.close().await;
}
#[tokio::test]
async fn rejects_bad_identity_and_attempt_before_consume() {
    let mut rpc = fixture().await;
    assert!(consume_with_rpc(
        &mut rpc,
        &uuid::Uuid::new_v4().to_string(),
        "another-account"
    )
    .await
    .is_err());
    assert!(consume_with_rpc(&mut rpc, "not-a-uuid", "account")
        .await
        .is_err());
    assert_eq!(
        rpc.request("fixture/redemption_stats", json!({}))
            .await
            .unwrap()["calls"],
        0
    );
    rpc.close().await;
}
#[tokio::test]
async fn all_official_outcomes_and_unknown_are_handled() {
    let mut rpc = fixture().await;
    let account = identity(&mut rpc).await;
    for (name, expected) in [
        ("nothingToReset", ResetOutcome::NothingToReset),
        ("noCredit", ResetOutcome::NoCredit),
    ] {
        rpc.request("fixture/reset_outcome", json!({"outcome":name}))
            .await
            .unwrap();
        assert_eq!(
            consume_with_rpc(&mut rpc, &uuid::Uuid::new_v4().to_string(), &account)
                .await
                .unwrap(),
            expected
        );
    }
    rpc.request("fixture/reset_outcome", json!({"outcome":"futureUnknown"}))
        .await
        .unwrap();
    assert!(
        consume_with_rpc(&mut rpc, &uuid::Uuid::new_v4().to_string(), &account)
            .await
            .is_err()
    );
    assert_eq!(
        rpc.request("fixture/redemption_stats", json!({}))
            .await
            .unwrap()["consumed"],
        0
    );
    rpc.close().await;
}
