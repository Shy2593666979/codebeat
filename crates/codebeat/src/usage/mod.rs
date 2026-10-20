//! 只读、增量的本地用量统计，绝不保留对话内容。
use crate::ProviderId;
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::File,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Instant, SystemTime},
};

const LINE_LIMIT: u64 = 8 * 1024 * 1024;
const SCAN_BUDGET: u64 = 1024 * 1024 * 1024;
const FILE_LIMIT: usize = 20_000;

#[derive(Debug, Clone, Copy, Default, Serialize, PartialEq, Eq)]
pub struct Counts {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}
impl Counts {
    fn add(&mut self, other: Self) {
        self.input = self.input.saturating_add(other.input);
        self.output = self.output.saturating_add(other.output);
        self.cache_read = self.cache_read.saturating_add(other.cache_read);
        self.cache_write = self.cache_write.saturating_add(other.cache_write);
    }
    fn delta(self, previous: Self) -> Self {
        Self {
            input: self.input.saturating_sub(previous.input),
            output: self.output.saturating_sub(previous.output),
            cache_read: self.cache_read.saturating_sub(previous.cache_read),
            cache_write: self.cache_write.saturating_sub(previous.cache_write),
        }
    }
    fn total(self) -> u64 {
        self.input.saturating_add(self.output)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DailyTokens {
    pub date: String,
    pub provider: ProviderId,
    pub counts: Counts,
    pub requests: u64,
}
#[derive(Debug, Serialize)]
pub struct TokenSource {
    pub provider: ProviderId,
    pub status: String,
    pub files: usize,
    pub records: usize,
    pub warnings: Vec<String>,
}
#[derive(Debug, Serialize)]
pub struct TokenReport {
    pub days: Vec<DailyTokens>,
    pub sources: Vec<TokenSource>,
    pub observed_at: DateTime<Utc>,
    pub timezone: String,
}
#[derive(Debug, Clone)]
struct Record {
    at: DateTime<Utc>,
    identity: String,
    counts: Counts,
    // Codex 上报会话累计计数；Claude 上报消息计数。
    last: Option<Counts>,
}
#[derive(Default)]
struct CachedFile {
    length: u64,
    modified: Option<SystemTime>,
    offset: u64,
    session: String,
    session_start: Option<DateTime<Utc>>,
    records: Vec<Record>,
    damaged: bool,
}
#[derive(Default)]
pub struct TokenScanner {
    cache: HashMap<PathBuf, CachedFile>,
}

fn number(v: &Value, key: &str) -> u64 {
    v[key].as_u64().unwrap_or(0)
}
fn codex_counts(v: &Value) -> Option<Counts> {
    let input = v["input_tokens"].as_u64()?;
    let output = v["output_tokens"].as_u64()?;
    Some(Counts {
        input,
        output,
        cache_read: number(v, "cached_input_tokens").min(input),
        cache_write: number(v, "cache_write_input_tokens").min(input),
    })
}
fn parse_line(line: &[u8], file: &mut CachedFile, provider: ProviderId) {
    // 只有包含用量元数据的行才会被反序列化，避免加载巨大的消息体。
    let relevant = match provider {
        ProviderId::Codex => {
            line.windows(11).any(|w| w == b"token_count") || file.session.is_empty()
        }
        ProviderId::ClaudeCode => line.windows(7).any(|w| w == b"\"usage\""),
    };
    if !relevant {
        return;
    }
    let Ok(v) =
        serde_json::from_slice::<Value>(line.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(line))
    else {
        file.damaged = true;
        return;
    };
    if provider == ProviderId::Codex && v["type"] == "session_meta" {
        if let Some(id) = v["payload"]["id"]
            .as_str()
            .or(v["payload"]["session_id"].as_str())
        {
            file.session = id.to_string();
        }
        file.session_start = v["payload"]["timestamp"]
            .as_str()
            .and_then(|s| s.parse().ok());
        return;
    }
    let Some(at) = v["timestamp"].as_str().and_then(|s| s.parse().ok()) else {
        return;
    };
    match provider {
        ProviderId::Codex if v["type"] == "event_msg" && v["payload"]["type"] == "token_count" => {
            if file.session_start.is_some_and(|start| at < start) {
                return;
            }
            let info = &v["payload"]["info"];
            if info.is_null() {
                return;
            }
            let Some(counts) = codex_counts(&info["total_token_usage"]) else {
                file.damaged = true;
                return;
            };
            file.records.push(Record {
                at,
                identity: file.session.clone(),
                counts,
                last: codex_counts(&info["last_token_usage"]),
            });
        }
        ProviderId::ClaudeCode
            if v["type"] == "assistant" && v["message"]["model"] != "<synthetic>" =>
        {
            let usage = &v["message"]["usage"];
            if usage.is_null() {
                return;
            }
            let Some(id) = v["message"]["id"].as_str().filter(|id| !id.is_empty()) else {
                file.damaged = true;
                return;
            };
            let (Some(input), Some(output)) = (
                usage["input_tokens"].as_u64(),
                usage["output_tokens"].as_u64(),
            ) else {
                file.damaged = true;
                return;
            };
            let read = number(usage, "cache_read_input_tokens");
            let write = number(usage, "cache_creation_input_tokens");
            file.records.push(Record {
                at,
                identity: id.into(),
                counts: Counts {
                    input: input.saturating_add(read).saturating_add(write),
                    output,
                    cache_read: read,
                    cache_write: write,
                },
                last: None,
            });
        }
        _ => {}
    }
}

fn scan_file(
    path: &Path,
    cached: &mut CachedFile,
    provider: ProviderId,
    budget: &mut u64,
    start: Instant,
) -> std::io::Result<bool> {
    let metadata = path.metadata()?;
    let modified = metadata.modified().ok();
    if metadata.len() < cached.offset
        || metadata.len() < cached.length
        || metadata.len() == cached.length && modified != cached.modified
    {
        *cached = CachedFile::default();
    }
    cached.length = metadata.len();
    cached.modified = modified;
    if cached.offset == cached.length {
        return Ok(true);
    }
    let mut input = File::open(path)?;
    input.seek(SeekFrom::Start(cached.offset))?;
    let mut reader = BufReader::new(input);
    let mut line = Vec::new();
    loop {
        if *budget == 0 || start.elapsed().as_secs() >= 5 {
            return Ok(false);
        }
        line.clear();
        let mut count = reader
            .by_ref()
            .take(LINE_LIMIT + 1)
            .read_until(b'\n', &mut line)? as u64;
        if count == 0 {
            return Ok(true);
        }
        // 写入方可能还在追加最后一行，留到下次刷新时再处理。
        if count <= LINE_LIMIT && line.last() != Some(&b'\n') {
            return Ok(true);
        }
        if count > LINE_LIMIT {
            cached.damaged = true;
            while line.last() != Some(&b'\n') {
                let buf = reader.fill_buf()?;
                if buf.is_empty() {
                    return Ok(true);
                }
                let end = buf.iter().position(|b| *b == b'\n').map(|p| p + 1);
                let consumed = end.unwrap_or(buf.len());
                reader.consume(consumed);
                count += consumed as u64;
                if end.is_some() {
                    break;
                }
            }
        } else {
            parse_line(&line, cached, provider);
        }
        cached.offset += count;
        *budget = budget.saturating_sub(count);
    }
}

impl TokenScanner {
    pub fn report(
        &mut self,
        roots: &[(ProviderId, PathBuf)],
        days: i64,
        timezone: &str,
        now: DateTime<Utc>,
    ) -> anyhow::Result<TokenReport> {
        let zone: chrono_tz::Tz = timezone
            .parse()
            .map_err(|_| anyhow::anyhow!("TOKEN_TIMEZONE_INVALID"))?;
        let first = now.with_timezone(&zone).date_naive() - Duration::days(days.clamp(1, 30) - 1);
        let today = now.with_timezone(&zone).date_naive();
        let mut rows = BTreeMap::<(String, String), DailyTokens>::new();
        for n in 0..days.clamp(1, 30) {
            for provider in [ProviderId::Codex, ProviderId::ClaudeCode] {
                let date = (first + Duration::days(n)).to_string();
                rows.insert(
                    (date.clone(), provider.as_str().into()),
                    DailyTokens {
                        date,
                        provider,
                        counts: Counts::default(),
                        requests: 0,
                    },
                );
            }
        }
        let mut sources = Vec::new();
        let mut present = HashSet::new();
        let start = Instant::now();
        let mut budget = SCAN_BUDGET;
        for provider in [ProviderId::Codex, ProviderId::ClaudeCode] {
            let mut warnings = Vec::new();
            let mut paths = Vec::new();
            let mut exists = false;
            for (_, root) in roots.iter().filter(|(p, _)| *p == provider) {
                if !root.exists() {
                    continue;
                }
                exists = true;
                for entry in walkdir::WalkDir::new(root)
                    .follow_links(false)
                    .max_depth(20)
                {
                    match entry {
                        Ok(e)
                            if e.file_type().is_file()
                                && e.path().extension().is_some_and(|x| x == "jsonl") =>
                        {
                            if paths.len() >= FILE_LIMIT {
                                warnings.push("文件数量超出扫描上限".into());
                                break;
                            }
                            paths.push(e.into_path());
                        }
                        Err(_) => warnings.push("部分目录无法读取".into()),
                        _ => {}
                    }
                }
            }
            paths.sort_by_cached_key(|p| {
                std::cmp::Reverse(p.metadata().and_then(|m| m.modified()).ok())
            });
            let mut records = Vec::new();
            for path in &paths {
                present.insert(path.clone());
                let file = self.cache.entry(path.clone()).or_default();
                match scan_file(path, file, provider, &mut budget, start) {
                    Ok(false) => warnings.push("正在增量扫描历史记录，当前统计尚不完整".into()),
                    Err(_) => warnings.push("部分用量文件无法读取".into()),
                    _ => {}
                }
                if file.damaged {
                    warnings.push("部分记录格式不兼容或过大，已跳过".into());
                }
                records.extend(file.records.iter().cloned());
            }
            let records_count = records.len();
            let recognized = records_count > 0;
            records.sort_by_key(|r| r.at);
            let mut events = Vec::new();
            if provider == ProviderId::Codex {
                let mut previous = HashMap::<String, Counts>::new();
                let mut seen = HashSet::new();
                for record in records {
                    // 复制来的 rollout 文件可能重复同一个线程事件。
                    let key = (
                        record.identity.clone(),
                        record.at,
                        record.counts.input,
                        record.counts.output,
                        record.counts.cache_read,
                        record.counts.cache_write,
                    );
                    if !seen.insert(key) {
                        continue;
                    }
                    if record.identity.is_empty() {
                        warnings.push("部分 Codex 记录缺少会话标识".into());
                        continue;
                    }
                    let delta = if let Some(old) = previous.get(&record.identity) {
                        if record.counts.input < old.input || record.counts.output < old.output {
                            warnings.push("部分 Codex 累计计数回退，已避免重复计入".into());
                            continue;
                        }
                        record.counts.delta(*old)
                    } else if let Some(last) = record.last {
                        if record.counts.total() > last.total() {
                            warnings.push("部分 Codex 会话缺少早期计数，仅计入可确认的增量".into());
                        }
                        last
                    } else {
                        warnings.push("部分 Codex 会话缺少初始增量，已跳过不明确的基数".into());
                        Counts::default()
                    };
                    previous.insert(record.identity, record.counts);
                    if delta.total() > 0 {
                        events.push((record.at, delta));
                    }
                }
            } else {
                let mut unique = BTreeMap::<String, (DateTime<Utc>, Counts)>::new();
                for record in records {
                    let entry = unique
                        .entry(record.identity)
                        .or_insert((record.at, record.counts));
                    entry.0 = entry.0.min(record.at);
                    if (record.counts.output, record.counts.total())
                        > (entry.1.output, entry.1.total())
                    {
                        entry.1 = record.counts;
                    }
                }
                events.extend(unique.into_values());
            }
            for (at, counts) in events {
                let date = at.with_timezone(&zone).date_naive();
                if at > now || date < first || date > today {
                    continue;
                }
                if let Some(row) = rows.get_mut(&(date.to_string(), provider.as_str().into())) {
                    row.counts.add(counts);
                    row.requests += 1;
                }
            }
            warnings.sort();
            warnings.dedup();
            let status = if !exists {
                "missing"
            } else if !warnings.is_empty() {
                "partial"
            } else if !recognized {
                "empty"
            } else {
                "available"
            };
            sources.push(TokenSource {
                provider,
                status: status.into(),
                files: paths.len(),
                records: records_count,
                warnings,
            });
        }
        self.cache.retain(|path, _| present.contains(path));
        Ok(TokenReport {
            days: rows.into_values().collect(),
            sources,
            observed_at: now,
            timezone: timezone.into(),
        })
    }
}

pub async fn local_usage(days: i64, timezone: String) -> anyhow::Result<TokenReport> {
    tokio::task::spawn_blocking(move || {
        static SCANNER: OnceLock<Mutex<TokenScanner>> = OnceLock::new();
        let home = PathBuf::from(
            std::env::var_os("USERPROFILE")
                .or_else(|| std::env::var_os("HOME"))
                .ok_or_else(|| anyhow::anyhow!("TOKEN_HOME_MISSING"))?,
        );
        let codex = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".codex"));
        let claude = std::env::var_os("CLAUDE_CONFIG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".claude"));
        SCANNER
            .get_or_init(Mutex::default)
            .lock()
            .map_err(|_| anyhow::anyhow!("TOKEN_SCAN_UNAVAILABLE"))?
            .report(
                &[
                    (ProviderId::Codex, codex.join("sessions")),
                    (ProviderId::Codex, codex.join("archived_sessions")),
                    (ProviderId::ClaudeCode, claude.join("projects")),
                ],
                days,
                &timezone,
                Utc::now(),
            )
    })
    .await?
}
