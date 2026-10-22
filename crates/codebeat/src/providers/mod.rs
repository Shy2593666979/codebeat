pub mod discovery;
pub mod reset;
pub mod rpc;
pub mod session_lock;
pub mod stream;
use crate::*;
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::process::Command;

pub fn command(path: &Path) -> Command {
    let mut cmd = Command::new(path);
    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000);
    }
    cmd
}
pub fn executable(id: ProviderId) -> Result<PathBuf> {
    let name = match id {
        ProviderId::Codex => "codex",
        ProviderId::ClaudeCode => "claude",
    };
    if let Ok(p) = which::which(format!("{name}.exe")) {
        if id != ProviderId::Codex {
            return Ok(p);
        }
        if let Some(native) = discovery::native_codex_path(p) {
            return Ok(native);
        }
    }
    // npm 的 .cmd 启动器绝不通过 cmd.exe 传递提示词，改为解析它真正的可执行文件。
    if id == ProviderId::Codex {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let root = PathBuf::from(appdata).join("npm/node_modules/@openai/codex");
            for p in [
                "node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/bin/codex.exe",
                "vendor/x86_64-pc-windows-msvc/codex/codex.exe",
            ] {
                let exe = root.join(p);
                if exe.is_file() {
                    return Ok(exe);
                }
            }
        }
    }
    if id == ProviderId::Codex {
        if let Some(path) = discovery::desktop_executable() {
            return Ok(path);
        }
    }
    let path =
        which::which(name).context("CLI_NOT_FOUND: 请安装官方工具（Codex CLI 或桌面客户端）")?;
    let path = if id == ProviderId::Codex {
        discovery::native_codex_path(path)
            .context("CLI_NATIVE_REQUIRED: 请安装官方 Codex CLI 或桌面客户端")?
    } else {
        path
    };
    if path
        .extension()
        .is_some_and(|s| s == "cmd" || s == "bat" || s == "ps1")
    {
        bail!("CLI_NATIVE_REQUIRED: 请安装原生可执行 CLI");
    }
    Ok(path)
}
async fn output(path: &Path, args: &[&str]) -> Result<std::process::Output> {
    tokio::time::timeout(
        Duration::from_secs(15),
        command(path).args(args).kill_on_drop(true).output(),
    )
    .await
    .context("CLI_TIMEOUT")?
    .context("CLI_FAILED")
}
pub async fn detect(id: ProviderId) -> ProviderHealth {
    let mut h = ProviderHealth {
        id,
        executable_path: None,
        version: None,
        status: "missing".into(),
        auth_status: "unknown".into(),
        message: "请安装并登录官方工具（Codex 支持 CLI 或桌面客户端）".into(),
        capabilities: Capabilities::default(),
    };
    let path = match executable(id) {
        Ok(p) => p,
        Err(e) => {
            h.message = e.to_string();
            return h;
        }
    };
    h.executable_path = Some(path.clone());
    match output(&path, &["--version"]).await {
        Ok(o) if o.status.success() => {
            h.version = Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
        }
        _ => {
            h.status = "unavailable".into();
            h.message = "无法读取 CLI 版本".into();
            return h;
        }
    }
    h.status = "installed".into();
    h.capabilities = Capabilities {
        usage: id == ProviderId::Codex,
        sessions: true,
        resume: true,
        unattended: true,
        structured_events: true,
    };
    let auth = match id {
        ProviderId::Codex => match rpc::RpcClient::connect(&path).await {
            Ok(mut r) => {
                let result = r
                    .request("account/read", json!({"refreshToken":false}))
                    .await;
                r.close().await;
                result.map(|v| v.get("account").is_some_and(|a| !a.is_null()))
            }
            Err(e) => Err(e),
        },
        ProviderId::ClaudeCode => match output(&path, &["auth", "status", "--json"]).await {
            Ok(o) => serde_json::from_slice::<Value>(&o.stdout)
                .map(|v| v["loggedIn"].as_bool().unwrap_or(false))
                .map_err(Into::into),
            Err(e) => Err(e),
        },
    };
    match auth {
        Ok(true) => {
            h.status = "ready".into();
            h.auth_status = "authenticated".into();
            h.message = "已检测到登录状态，执行前将重新验证会话".into();
        }
        Ok(false) => {
            h.auth_status = "not_authenticated".into();
            h.message = "CLI 已安装，请在官方工具中登录".into();
        }
        Err(e) => h.message = format!("登录状态未知：{}", redact(&e.to_string())),
    }
    h
}
pub fn timestamp(v: &Value) -> Option<DateTime<Utc>> {
    let n = v.as_i64()?;
    if n.abs() > 100_000_000_000 {
        DateTime::from_timestamp_millis(n)
    } else {
        DateTime::from_timestamp(n, 0)
    }
}
pub fn parse_usage(value: &Value) -> Vec<UsageWindow> {
    let mut windows = Vec::new();
    let buckets: Vec<(String, &Value)> = if let Some(map) = value["rateLimitsByLimitId"]
        .as_object()
        .filter(|m| !m.is_empty())
    {
        map.iter().map(|(k, v)| (k.clone(), v)).collect()
    } else {
        vec![("codex".into(), &value["rateLimits"])]
    };
    for (bucket, data) in buckets {
        for slot in ["primary", "secondary"] {
            let w = &data[slot];
            if !w.is_object() {
                continue;
            }
            let used = w["usedPercent"]
                .as_f64()
                .filter(|n| n.is_finite() && (0.0..=100.0).contains(n));
            windows.push(UsageWindow {
                label: format!("{bucket} · {slot}"),
                used_percent: used,
                resets_at_utc: timestamp(&w["resetsAt"]),
                duration_minutes: w["windowDurationMins"].as_i64().filter(|n| *n > 0),
                observed_at_utc: Utc::now(),
                source: "codex app-server account/rateLimits/read".into(),
                reliability: if used.is_some() {
                    "official"
                } else {
                    "unknown"
                }
                .into(),
            });
        }
    }
    windows
}
pub async fn usage(id: ProviderId) -> UsageReport {
    let mut report = UsageReport {
        provider: id,
        windows: vec![],
        status: "unavailable".into(),
        message: "尚无公开稳定的 Claude Code 额度读取接口；请查看官方用量页面".into(),
        reset_credits: None,
    };
    if id == ProviderId::ClaudeCode {
        return report;
    }
    let result: Result<(Vec<UsageWindow>, Option<ResetCredits>)> = async {
        let path = executable(id)?;
        let mut r = rpc::RpcClient::connect(&path).await?;
        let result = read_usage_with_rpc(&mut r).await;
        r.close().await;
        result
    }
    .await;
    match result {
        Ok((w, credits)) => {
            report.windows = w;
            report.reset_credits = credits;
            report.status = if report.windows.is_empty() {
                "unknown"
            } else {
                "available"
            }
            .into();
            report.message = "仅展示官方返回的窗口；不推算固定恢复周期".into();
        }
        Err(e) => report.message = redact(&e.to_string()),
    }
    report
}
pub async fn read_usage_with_rpc(
    r: &mut rpc::RpcClient,
) -> Result<(Vec<UsageWindow>, Option<ResetCredits>)> {
    let value = r.request("account/rateLimits/read", json!({})).await?;
    let mut credits = reset::parse_credits(&value);
    if let Some(credits) = &mut credits {
        // 身份查询失败不能让原本有效的额度数据被隐藏。
        if let Ok(account) = r
            .request("account/read", json!({"refreshToken":false}))
            .await
        {
            credits.account_key = reset::account_key(&account);
        }
    }
    Ok((parse_usage(&value), credits))
}
fn codex_conversation(v: &Value) -> Option<Conversation> {
    let id = v["id"].as_str()?;
    let cwd = PathBuf::from(v["cwd"].as_str()?);
    Some(Conversation {
        lock_state: session_lock::state(ProviderId::Codex, id),
        provider: ProviderId::Codex,
        session_id: id.into(),
        title: v["name"]
            .as_str()
            .or_else(|| v["preview"].as_str())
            .unwrap_or("未命名会话")
            .chars()
            .take(120)
            .collect(),
        resumable: uuid::Uuid::parse_str(id).is_ok() && cwd.is_dir() && v["ephemeral"] != true,
        cwd,
        last_updated_utc: timestamp(&v["updatedAt"]),
        source: "codex app-server thread/list".into(),
    })
}
pub async fn sessions(
    id: ProviderId,
    search: &str,
    cursor: Option<String>,
    limit: usize,
) -> Result<SessionPage> {
    let limit = limit.clamp(1, 100);
    if id == ProviderId::ClaudeCode {
        let root = PathBuf::from(std::env::var_os("USERPROFILE").context("USERPROFILE missing")?)
            .join(".claude/projects");
        let search = search.to_string();
        return tokio::task::spawn_blocking(move || scan_claude(&root, &search, cursor, limit))
            .await?;
    }
    let mut r = rpc::RpcClient::connect(&executable(id)?).await?;
    let result=r.request("thread/list",json!({"cursor":cursor,"limit":limit,"sortKey":"updated_at","searchTerm":if search.is_empty(){None}else{Some(search)},"sourceKinds":[]})).await;
    r.close().await;
    let v = result?;
    Ok(SessionPage {
        data: v["data"]
            .as_array()
            .context("THREAD_LIST_INVALID")?
            .iter()
            .filter_map(codex_conversation)
            .collect(),
        next_cursor: v["nextCursor"].as_str().map(Into::into),
        warnings: vec![],
    })
}
pub fn parse_claude_file(path: &Path) -> Result<Option<Conversation>> {
    let file = std::fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut scanned = 0;
    let mut result: Option<Conversation> = None;
    // 只读取有限的前缀，绝不加载巨大的会话记录，也不跟随符号链接。
    for _ in 0..500 {
        line.clear();
        let count = reader.by_ref().take(1024 * 1024).read_line(&mut line)?;
        if count == 0 {
            break;
        }
        scanned += count;
        if scanned > 2 * 1024 * 1024 {
            break;
        }
        let Ok(v) = serde_json::from_str::<Value>(line.trim_start_matches('\u{feff}')) else {
            continue;
        };
        if let (Some(id), Some(cwd)) = (v["sessionId"].as_str(), v["cwd"].as_str()) {
            if uuid::Uuid::parse_str(id).is_err() {
                continue;
            }
            if path.file_stem().and_then(|s| s.to_str()) != Some(id) {
                continue;
            }
            let title = v["message"]["content"]
                .as_str()
                .or_else(|| v["summary"].as_str())
                .unwrap_or("未命名会话");
            let c = result.get_or_insert_with(|| Conversation {
                lock_state: SessionLockState::Unsupported,
                provider: ProviderId::ClaudeCode,
                session_id: id.into(),
                title: title.chars().take(120).collect(),
                cwd: cwd.into(),
                last_updated_utc: None,
                resumable: Path::new(cwd).is_dir(),
                source: "local Claude Code metadata (best-effort)".into(),
            });
            if let Some(t) = v["timestamp"].as_str().and_then(|s| s.parse().ok()) {
                c.last_updated_utc = Some(t);
            }
            if v["type"] == "user" && !title.starts_with('<') {
                c.title = title.chars().take(120).collect();
                break;
            }
        }
    }
    Ok(result)
}
pub fn scan_claude(
    root: &Path,
    search: &str,
    cursor: Option<String>,
    limit: usize,
) -> Result<SessionPage> {
    let mut data = Vec::new();
    let mut warnings = Vec::new();
    if !root.is_dir() {
        return Ok(SessionPage {
            data,
            next_cursor: None,
            warnings,
        });
    }
    for entry in walkdir::WalkDir::new(root).max_depth(2).follow_links(false) {
        let Ok(entry) = entry else {
            warnings.push("部分会话目录不可读".into());
            continue;
        };
        if entry.file_type().is_file() && entry.path().extension().is_some_and(|s| s == "jsonl") {
            match parse_claude_file(entry.path()) {
                Ok(Some(c)) if c.title.to_lowercase().contains(&search.to_lowercase()) => {
                    data.push(c)
                }
                Err(_) => warnings.push("部分会话元数据损坏或不可读".into()),
                _ => (),
            }
        }
    }
    data.sort_by(|a, b| {
        b.last_updated_utc
            .cmp(&a.last_updated_utc)
            .then(a.session_id.cmp(&b.session_id))
    });
    let offset = cursor
        .map(|s| s.parse::<usize>())
        .transpose()
        .context("INVALID_CURSOR")?
        .unwrap_or(0);
    let next = (data.len() > offset.saturating_add(limit))
        .then(|| offset.saturating_add(limit).to_string());
    Ok(SessionPage {
        data: data.into_iter().skip(offset).take(limit).collect(),
        next_cursor: next,
        warnings,
    })
}
pub async fn verify(c: &Conversation) -> Result<()> {
    if !c.resumable {
        bail!("SESSION_NOT_RESUMABLE");
    }
    local_directory(&c.cwd)?;
    uuid::Uuid::parse_str(&c.session_id)?;
    if c.provider == ProviderId::Codex {
        let mut r = rpc::RpcClient::connect(&executable(c.provider)?).await?;
        let v = r
            .request(
                "thread/read",
                json!({"threadId":c.session_id,"includeTurns":false}),
            )
            .await;
        r.close().await;
        let v = v?;
        let current = codex_conversation(&v["thread"]).context("SESSION_INVALID_METADATA")?;
        if !current.resumable || local_directory(&current.cwd)? != local_directory(&c.cwd)? {
            bail!("SESSION_CWD_MISMATCH");
        }
    } else {
        let root = PathBuf::from(std::env::var_os("USERPROFILE").context("USERPROFILE missing")?)
            .join(".claude/projects");
        let found = walkdir::WalkDir::new(root)
            .max_depth(2)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .find(|e| {
                e.file_type().is_file()
                    && e.file_name().to_str() == Some(&format!("{}.jsonl", c.session_id))
            });
        let found = found.context("SESSION_MISSING")?;
        let actual = parse_claude_file(found.path())?.context("SESSION_CORRUPT")?;
        if local_directory(&actual.cwd)? != local_directory(&c.cwd)? {
            bail!("SESSION_CWD_MISMATCH");
        }
    }
    Ok(())
}
pub async fn conversation_detail(c: &Conversation) -> Result<Value> {
    verify(c).await?;
    let mut messages = Vec::new();
    let mut created_at = None;
    let mut truncated = false;
    if c.provider == ProviderId::Codex {
        let mut rpc = rpc::RpcClient::connect(&executable(c.provider)?).await?;
        let result = rpc
            .request(
                "thread/read",
                json!({"threadId":c.session_id,"includeTurns":true}),
            )
            .await;
        rpc.close().await;
        let data = result?;
        created_at = timestamp(&data["thread"]["createdAt"]);
        if let Some(turns) = data["thread"]["turns"].as_array() {
            for turn in turns {
                if let Some(items) = turn["items"].as_array() {
                    for item in items {
                        let kind = item["type"].as_str().unwrap_or("");
                        if kind == "agentMessage" {
                            if let Some(text) = item["text"].as_str() {
                                messages.push(json!({"role":"assistant","text":text.chars().take(12000).collect::<String>()}));
                            }
                        }
                        if kind == "userMessage" {
                            if let Some(content) = item["content"].as_array() {
                                let text = content
                                    .iter()
                                    .filter_map(|v| v["text"].as_str())
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                messages.push(json!({"role":"user","text":text.chars().take(12000).collect::<String>()}));
                            }
                        }
                    }
                }
            }
        }
    } else {
        let root = PathBuf::from(std::env::var_os("USERPROFILE").context("USERPROFILE missing")?)
            .join(".claude/projects");
        let file = walkdir::WalkDir::new(root)
            .max_depth(2)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .find(|e| {
                e.file_type().is_file()
                    && e.file_name().to_str() == Some(&format!("{}.jsonl", c.session_id))
            })
            .context("SESSION_MISSING")?;
        let file = std::fs::File::open(file.path())?;
        truncated = file.metadata()?.len() > 4 * 1024 * 1024;
        let mut bytes = Vec::new();
        file.take(4 * 1024 * 1024).read_to_end(&mut bytes)?;
        for line in String::from_utf8_lossy(&bytes).lines() {
            let Ok(v) = serde_json::from_str::<Value>(line.trim_start_matches('\u{feff}')) else {
                continue;
            };
            if created_at.is_none() {
                created_at = v["timestamp"].as_str().and_then(|s| s.parse().ok());
            }
            let role = v["type"].as_str().unwrap_or("");
            if role != "user" && role != "assistant" {
                continue;
            }
            let content = &v["message"]["content"];
            let text = content.as_str().map(str::to_owned).unwrap_or_else(|| {
                content
                    .as_array()
                    .map(|parts| {
                        parts
                            .iter()
                            .filter_map(|p| p["text"].as_str())
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default()
            });
            if !text.is_empty() {
                messages
                    .push(json!({"role":role,"text":text.chars().take(12000).collect::<String>()}));
            }
        }
    }
    let count = messages.len();
    if messages.len() > 200 {
        messages.drain(..messages.len() - 200);
        truncated = true;
    }
    Ok(
        json!({"conversation":c,"created_at":created_at,"messages":messages,"message_count":if truncated{None}else{Some(count)},"file_count":null,"truncated":truncated}),
    )
}
pub fn resume_args(r: &CreateTaskRequest) -> Vec<String> {
    execution_args(r, false)
}
pub fn execution_args(r: &CreateTaskRequest, fork: bool) -> Vec<String> {
    match r.conversation.provider {
        ProviderId::Codex => vec![
            "exec".into(),
            "--ignore-user-config".into(),
            "-c".into(),
            "approval_policy=\"never\"".into(),
            "--sandbox".into(),
            match r.permission_profile {
                PermissionProfile::ReadOnly => "read-only",
                PermissionProfile::WorkspaceWrite => "workspace-write",
                PermissionProfile::FullAccess => "danger-full-access",
            }
            .into(),
            if fork { "fork" } else { "resume" }.into(),
            "--json".into(),
            r.conversation.session_id.clone(),
            "-".into(),
        ],
        ProviderId::ClaudeCode => vec![
            "--resume".into(),
            r.conversation.session_id.clone(),
            "-p".into(),
            "--verbose".into(),
            "--output-format".into(),
            "stream-json".into(),
            "--permission-mode".into(),
            if r.permission_profile == PermissionProfile::FullAccess {
                "bypassPermissions"
            } else {
                "dontAsk"
            }
            .into(),
            "--tools".into(),
            match r.permission_profile {
                PermissionProfile::ReadOnly => "Read,Glob,Grep",
                PermissionProfile::WorkspaceWrite => "Read,Glob,Grep,Edit,Write",
                PermissionProfile::FullAccess => "default",
            }
            .into(),
            "--allowedTools".into(),
            match r.permission_profile {
                PermissionProfile::ReadOnly => "Read,Glob,Grep",
                PermissionProfile::WorkspaceWrite => "Read,Glob,Grep,Edit,Write",
                PermissionProfile::FullAccess => "*",
            }
            .into(),
        ],
    }
}
pub fn pretrigger_args(id: ProviderId) -> Vec<String> {
    // 轻量触发独立使用官方 HTTPS 连接，不读取或改写用户的提供方配置。
    // 使用独立 ID，避免覆盖 Codex 保留的内置 openai 提供方。
    let args: Vec<&str> = match id {
        ProviderId::Codex => vec![
            "exec",
            "--ignore-user-config",
            "--ignore-rules",
            "--ephemeral",
            "--skip-git-repo-check",
            "--sandbox",
            "read-only",
            "-c",
            "model_provider=\"codebeat_http\"",
            "-c",
            "model_providers.codebeat_http={name=\"CodeBeat HTTPS\",wire_api=\"responses\",requires_openai_auth=true,supports_websockets=false}",
            "-c",
            "approval_policy=\"never\"",
            "-c",
            "model_reasoning_effort=\"low\"",
            "--json",
            "-",
        ],
        ProviderId::ClaudeCode => vec![
            "-p",
            "--model",
            "haiku",
            "--max-turns",
            "1",
            "--verbose",
            "--output-format",
            "stream-json",
            "--no-session-persistence",
            "--permission-mode",
            "dontAsk",
            "--tools",
            "",
            "--disable-slash-commands",
            "--strict-mcp-config",
            "--mcp-config",
            "{\"mcpServers\":{}}",
            "--setting-sources",
            "",
            "--settings",
            "{\"disableAllHooks\":true}",
            "--system-prompt",
            "Reply briefly. No tools or project work.",
        ],
    };
    args.into_iter().map(str::to_owned).collect()
}
pub fn classify_event(v: &Value) -> Option<&'static str> {
    if v["permission_denials"]
        .as_array()
        .is_some_and(|a| !a.is_empty())
        || v["type"] == "approval_needed"
    {
        return Some("blocked_approval");
    }
    if v["type"] == "turn.failed"
        || v["type"] == "error"
        || (v["type"] == "result" && v["is_error"] == true)
    {
        return Some("failed");
    }
    if v["type"] == "turn.completed"
        || (v["type"] == "result" && v["subtype"] == "success" && v["is_error"] != true)
    {
        return Some("succeeded");
    }
    None
}
pub fn redact(input: &str) -> String {
    let mut text = input.to_owned();
    for pattern in [
        r"(?i)Bearer\s+[A-Za-z0-9._~+/=-]+",
        r"\bsk-[A-Za-z0-9_-]{8,}",
        r#"(?i)"?(api[_-]?key|access[_-]?token|authorization)"?\s*[=:]\s*"?[^\s,\"}]+"#,
    ] {
        if let Ok(re) = regex::Regex::new(pattern) {
            text = re.replace_all(&text, "[REDACTED]").into_owned();
        }
    }
    if let Some(home) = std::env::var_os("USERPROFILE") {
        text = text.replace(&home.to_string_lossy().to_string(), "%USERPROFILE%");
        text = text.replace(
            &home.to_string_lossy().replace('\\', "\\\\"),
            "%USERPROFILE%",
        );
    }
    text
}

#[async_trait::async_trait]
pub trait CodingProvider: Send + Sync {
    fn id(&self) -> ProviderId;
    async fn detect(&self) -> ProviderHealth {
        detect(self.id()).await
    }
    async fn usage(&self) -> UsageReport {
        usage(self.id()).await
    }
    async fn list_sessions(
        &self,
        search: &str,
        cursor: Option<String>,
        limit: usize,
    ) -> Result<SessionPage> {
        sessions(self.id(), search, cursor, limit).await
    }
    async fn verify_session(&self, c: &Conversation) -> Result<()> {
        verify(c).await
    }
}
pub struct Provider(pub ProviderId);
#[async_trait::async_trait]
impl CodingProvider for Provider {
    fn id(&self) -> ProviderId {
        self.0
    }
}
