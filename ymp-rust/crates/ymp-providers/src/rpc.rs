use crate::{ProviderEvent, TurnRequest};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{collections::VecDeque, process::Stdio};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStdin, ChildStdout, Command},
    sync::mpsc,
};
use ymp_core::ProviderKind;

pub(crate) struct RpcProcess {
    child: Child,
    input: ChildStdin,
    output: Lines<BufReader<ChildStdout>>,
    next_id: u64,
    pending: VecDeque<Value>,
    pub acp_text: String,
    pub acp_config_options: Option<Value>,
    read_only: bool,
}
impl RpcProcess {
    pub async fn spawn(req: &TurnRequest) -> Result<Self> {
        let mut cmd = if req.provider.kind == ProviderKind::Claude {
            if !req.bridge.is_file() {
                bail!(
                    "Claude bridge is missing. Run npm ci && npm run build in ymp-bridges/claude"
                );
            }
            let mut c = Command::new("node");
            c.arg(&req.bridge);
            c
        } else {
            let mut c = Command::new(&req.provider.command);
            c.args(&req.provider.args);
            c
        };
        if req.provider.kind == ProviderKind::Codex {
            cmd.arg("app-server").arg("--stdio");
            if let Some(mcp) = &req.mcp {
                cmd.arg("-c")
                    .arg(format!("mcp_servers.ymp.command={}", json!(mcp.command)));
                cmd.arg("-c")
                    .arg(format!("mcp_servers.ymp.args={}", json!(mcp.args)));
                cmd.arg("-c")
                    .arg("mcp_servers.ymp.env_vars=[\"YMP_MCP_TOKEN\"]");
                cmd.arg("-c").arg("mcp_servers.ymp.required=true");
                cmd.arg("-c")
                    .arg("mcp_servers.ymp.default_tools_approval_mode=\"approve\"");
            }
        }
        for (target, source) in &req.provider.env_refs {
            cmd.env(
                target,
                std::env::var(source).with_context(|| {
                    format!("Required environment variable {source} is not set")
                })?,
            );
        }
        if let Some(mcp) = &req.mcp {
            cmd.env("YMP_MCP_TOKEN", &mcp.token);
        }
        // Nested-session guards belong to the invoking host, not the new session.
        cmd.env_remove("CLAUDECODE")
            .env_remove("CLAUDE_CODE_ENTRYPOINT");
        let host = std::env::current_exe()?;
        if host.file_name().is_some_and(|name| name == "ymp") {
            let original = cmd.as_std();
            let mut supervised = Command::new(host);
            supervised
                .arg("_supervise")
                .arg(original.get_program())
                .args(original.get_args());
            for (name, value) in original.get_envs() {
                if let Some(value) = value {
                    supervised.env(name, value);
                } else {
                    supervised.env_remove(name);
                }
            }
            cmd = supervised;
        }
        cmd.current_dir(&req.cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        cmd.process_group(0);
        let mut child = cmd
            .spawn()
            .with_context(|| format!("Cannot start {}", req.provider.id))?;
        let input = child.stdin.take().context("Missing stdin")?;
        let output = BufReader::new(child.stdout.take().context("Missing stdout")?).lines();
        // Drain stderr without logging it: SDK diagnostics may contain credentials.
        if let Some(mut err) = child.stderr.take() {
            tokio::spawn(async move {
                let _ = tokio::io::copy(&mut err, &mut tokio::io::sink()).await;
            });
        }
        Ok(Self {
            child,
            input,
            output,
            next_id: 1,
            pending: VecDeque::new(),
            acp_text: String::new(),
            acp_config_options: None,
            read_only: req.read_only,
        })
    }
    async fn send(&mut self, v: &Value) -> Result<()> {
        let mut b = serde_json::to_vec(v)?;
        b.push(b'\n');
        self.input.write_all(&b).await?;
        self.input.flush().await?;
        Ok(())
    }
    pub async fn notify(&mut self, method: &str, params: Value) -> Result<()> {
        self.send(&json!({"jsonrpc":"2.0","method":method,"params":params}))
            .await
    }
    async fn read(&mut self) -> Result<Value> {
        loop {
            let line = self
                .output
                .next_line()
                .await?
                .context("Agent process exited before completing the request")?;
            if line.trim().is_empty() {
                continue;
            }
            if line.len() > 16 * 1024 * 1024 {
                bail!("Provider event exceeds 16 MiB");
            }
            return serde_json::from_str(&line).context("Invalid JSON from agent protocol");
        }
    }
    pub async fn next(&mut self) -> Result<Value> {
        if let Some(v) = self.pending.pop_front() {
            Ok(v)
        } else {
            self.read().await
        }
    }
    pub async fn request(
        &mut self,
        method: &str,
        params: Value,
        events: &mpsc::UnboundedSender<ProviderEvent>,
    ) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await?;
        // Record only settings actually written to the native transport. A
        // request to the Claude bridge is not itself a native SDK setting.
        if let Some(sent) = sent_settings(method, &params) {
            let _ = events.send(ProviderEvent::Execution(Box::new(
                ymp_core::InvocationObservation {
                    sent: Some(sent),
                    ..Default::default()
                },
            )));
        }
        loop {
            let v = self.read().await?;
            if self.respond_server(&v).await? {
                continue;
            }
            if v.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(err) = v.get("error") {
                    bail!("{method}: {err}");
                }
                return Ok(v.get("result").cloned().unwrap_or(Value::Null));
            }
            let method = v["method"].as_str().unwrap_or("");
            if method == "capabilities" {
                if let Ok(catalog) =
                    serde_json::from_value::<ymp_core::ProviderCapabilities>(v["params"].clone())
                {
                    let _ = events.send(ProviderEvent::Capabilities(catalog));
                }
            } else if method == "execution" {
                if let Ok(observation) =
                    serde_json::from_value::<ymp_core::InvocationObservation>(v["params"].clone())
                {
                    let _ = events.send(ProviderEvent::Execution(Box::new(observation)));
                }
            } else if method == "usage" {
                if let Ok(snapshot) =
                    serde_json::from_value::<ymp_core::UsageSnapshot>(v["params"].clone())
                {
                    let _ = events.send(ProviderEvent::Usage(snapshot));
                }
            } else if method == "delta" {
                if let Some(t) = v.pointer("/params/text").and_then(Value::as_str) {
                    let _ = events.send(ProviderEvent::Delta(t.into()));
                }
            } else if method == "session" {
                if let Some(t) = v.pointer("/params/id").and_then(Value::as_str) {
                    let _ = events.send(ProviderEvent::Session(t.into()));
                }
            } else if method == "session/update" {
                if params
                    .get("sessionId")
                    .is_some_and(|id| v.pointer("/params/sessionId") != Some(id))
                {
                    continue;
                }
                let u = &v["params"]["update"];
                if u["sessionUpdate"].as_str() == Some("config_option_update") {
                    self.acp_config_options = Some(u["configOptions"].clone());
                }
                if u["sessionUpdate"].as_str() == Some("agent_message_chunk") {
                    if let Some(t) = u.pointer("/content/text").and_then(Value::as_str) {
                        self.acp_text.push_str(t);
                        let _ = events.send(ProviderEvent::Delta(t.into()));
                    }
                }
            } else {
                self.pending.push_back(v);
            }
        }
    }
    pub async fn respond_server(&mut self, v: &Value) -> Result<bool> {
        if v.get("id").is_none() || v.get("method").is_none() {
            return Ok(false);
        }
        let method = v["method"].as_str().unwrap_or("");
        let response = match method {
            "session/request_permission" => {
                let kind = if self.read_only {
                    "reject_once"
                } else {
                    "allow_once"
                };
                let opt = v
                    .pointer("/params/options")
                    .and_then(Value::as_array)
                    .and_then(|a| a.iter().find(|o| o["kind"].as_str() == Some(kind)))
                    .and_then(|o| o["optionId"].as_str());
                if let Some(id) = opt {
                    json!({"outcome":{"outcome":"selected","optionId":id}})
                } else {
                    json!({"outcome":{"outcome":"cancelled"}})
                }
            }
            "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
                json!({"decision":if self.read_only{"decline"}else{"accept"}})
            }
            _ => {
                self.send(&json!({"jsonrpc":"2.0","id":v["id"],"error":{"code":-32601,"message":"Unsupported client request"}})).await?;
                return Ok(true);
            }
        };
        self.send(&json!({"jsonrpc":"2.0","id":v["id"],"result":response}))
            .await?;
        Ok(true)
    }
    pub async fn close(&mut self) {
        let _ = self.input.shutdown().await;
        if tokio::time::timeout(std::time::Duration::from_secs(2), self.child.wait())
            .await
            .is_err()
        {
            self.kill_group();
            let _ = self.child.kill().await;
            let _ = self.child.wait().await;
        }
    }
    fn kill_group(&mut self) {
        if let Some(pid) = self.child.id() {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
    }
}

fn sent_settings(method: &str, params: &Value) -> Option<ymp_core::ExecutionSettings> {
    let string = |key: &str| params[key].as_str().map(str::to_owned);
    match method {
        "thread/start" | "thread/resume" => Some(ymp_core::ExecutionSettings {
            model: string("model"),
            effort: params
                .pointer("/config/model_reasoning_effort")
                .and_then(Value::as_str)
                .map(str::to_owned),
            permission_mode: string("sandbox"),
        }),
        "turn/start" => Some(ymp_core::ExecutionSettings {
            model: string("model"),
            effort: string("effort"),
            permission_mode: None,
        }),
        "session/set_config_option" if params["configId"] == "thought_level" => {
            Some(ymp_core::ExecutionSettings {
                model: None,
                effort: string("value"),
                permission_mode: None,
            })
        }
        "session/set_model" => Some(ymp_core::ExecutionSettings {
            model: string("modelId"),
            ..Default::default()
        }),
        "session/set_mode" => Some(ymp_core::ExecutionSettings {
            permission_mode: string("modeId"),
            ..Default::default()
        }),
        _ => None,
    }
}
impl Drop for RpcProcess {
    fn drop(&mut self) {
        self.kill_group();
    }
}
