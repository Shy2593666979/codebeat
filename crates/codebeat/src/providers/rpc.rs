use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{path::Path, time::Duration};
use tokio::{
    io::{AsyncWriteExt, BufReader},
    process::{Child, ChildStdin},
    sync::mpsc,
};

pub struct RpcClient {
    child: Child,
    input: ChildStdin,
    rx: mpsc::Receiver<Result<Value>>,
    next: u64,
}
impl RpcClient {
    pub async fn connect(path: &Path) -> Result<Self> {
        let mut command = super::command(path);
        command
            .arg("app-server")
            .kill_on_drop(true)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = command.spawn().context("APP_SERVER_START_FAILED")?;
        let input = child.stdin.take().context("stdin unavailable")?;
        let mut lines = BufReader::new(child.stdout.take().context("stdout unavailable")?);
        let stderr = child.stderr.take().context("stderr unavailable")?;
        tokio::spawn(async move {
            let mut r = BufReader::new(stderr);
            while let Ok(Some(_)) = super::stream::next_line(&mut r, 1024 * 1024).await {}
        });
        let (tx, rx) = mpsc::channel(256);
        tokio::spawn(async move {
            loop {
                match super::stream::next_line(&mut lines, 8 * 1024 * 1024).await {
                    Ok(Some(line)) => {
                        if line.len() > 8 * 1024 * 1024 {
                            let _ = tx.send(Err(anyhow::anyhow!("RPC_FRAME_TOO_LARGE"))).await;
                            break;
                        }
                        match serde_json::from_str::<Value>(&line) {
                            Ok(v) => {
                                if tx.send(Ok(v)).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(Err(e.into())).await;
                                break;
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                        break;
                    }
                }
            }
        });
        let mut rpc = Self {
            child,
            input,
            rx,
            next: 0,
        };
        rpc.request(
            "initialize",
            json!({"clientInfo":{"name":"codebeat","title":"codebeat","version":"0.1.0"}}),
        )
        .await?;
        rpc.send(json!({"method":"initialized","params":{}}))
            .await?;
        Ok(rpc)
    }
    async fn send(&mut self, v: Value) -> Result<()> {
        let mut bytes = serde_json::to_vec(&v)?;
        bytes.push(b'\n');
        self.input.write_all(&bytes).await?;
        self.input.flush().await?;
        Ok(())
    }
    pub async fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        self.next += 1;
        let id = self.next;
        self.send(json!({"id":id,"method":method,"params":params}))
            .await?;
        tokio::time::timeout(Duration::from_secs(25),async {
            while let Some(value)=self.rx.recv().await {
                let value=value?;
                if value.get("id")==Some(&json!(id)) {
                    if let Some(e)=value.get("error") { bail!("RPC_ERROR: {}",super::redact(&e.to_string())); }
                    return value.get("result").cloned().context("RPC_INVALID_RESPONSE");
                }
                if value.get("id").is_some() && value.get("method").is_some() {
                    self.send(json!({"id":value["id"],"error":{"code":-32601,"message":"Interactive requests unsupported"}})).await?;
                }
            }
            bail!("APP_SERVER_EXITED")
        }).await.context("APP_SERVER_TIMEOUT")?
    }
    pub async fn close(mut self) {
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}
