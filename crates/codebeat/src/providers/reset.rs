use super::rpc::RpcClient;
use crate::{ResetCredit, ResetCredits, ResetOutcome};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

pub fn parse_credits(value: &Value) -> Option<ResetCredits> {
    // 明细列表可能被截断或为 null，只有官方计数才是权威值。
    Some(ResetCredits {
        available_count: value["rateLimitResetCredits"]["availableCount"].as_u64()?,
        account_key: None,
        credits: value["rateLimitResetCredits"]["credits"]
            .as_array()
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| {
                        Some(ResetCredit {
                            id: row["id"].as_str()?.to_owned(),
                            expires_at: row["expiresAt"]
                                .as_i64()
                                .and_then(|at| chrono::DateTime::from_timestamp(at, 0)),
                            status: row["status"].as_str().unwrap_or("unknown").to_owned(),
                            expiry_known: row.get("expiresAt").is_some_and(|v| {
                                v.is_null()
                                    || v.as_i64()
                                        .and_then(|at| chrono::DateTime::from_timestamp(at, 0))
                                        .is_some()
                            }),
                        })
                    })
                    .collect()
            }),
    })
}

pub fn account_key(value: &Value) -> Option<String> {
    let account = value.get("account")?;
    if account["type"] != "chatgpt" {
        return None;
    }
    // 绝不向重置界面暴露邮箱或认证数据。这个指纹只用于防止在展示计数
    // 与点击使用之间切换了账号。
    let email = account["email"].as_str().filter(|s| !s.is_empty())?;
    // 该指纹在进程和编译器版本之间保持稳定，使应用更新后持久化的重试仍绑定
    // 到同一账号。注意：这不是授权哈希。
    let identity = json!([email, value.get("workspaceRouting")]).to_string();
    let hash = identity.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    Some(format!("{hash:016x}"))
}

pub async fn consume_with_rpc(
    rpc: &mut RpcClient,
    idempotency_key: &str,
    expected_account: &str,
) -> Result<ResetOutcome> {
    uuid::Uuid::parse_str(idempotency_key).context("RESET_INVALID_ATTEMPT")?;
    if expected_account.is_empty() {
        bail!("RESET_ACCOUNT_CHANGED");
    }
    let account = rpc
        .request("account/read", json!({"refreshToken":false}))
        .await?;
    if account_key(&account).as_deref() != Some(expected_account) {
        bail!("RESET_ACCOUNT_CHANGED");
    }
    // 只有这个显式的、由用户触发的函数才会消费重置次数。额度刷新、
    // 监控、计划任务和预启动都不会兑换重置次数。
    let response = rpc
        .request(
            "account/rateLimitResetCredit/consume",
            json!({"idempotencyKey":idempotency_key}),
        )
        .await?;
    serde_json::from_value(response["outcome"].clone()).context("RESET_UNKNOWN_OUTCOME")
}

pub async fn consume(idempotency_key: &str, expected_account: &str) -> Result<ResetOutcome> {
    // 首页和用量窗口共用同一把锁，同一时间只允许一次兑换。重试沿用同一个幂等键。
    static LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _guard = LOCK.lock().await;
    let path = super::executable(crate::ProviderId::Codex)?;
    let mut rpc = RpcClient::connect(&path).await?;
    let result = consume_with_rpc(&mut rpc, idempotency_key, expected_account).await;
    rpc.close().await;
    result
}
