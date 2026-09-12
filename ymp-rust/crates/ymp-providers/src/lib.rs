pub mod discovery;
mod rpc;
pub mod supervisor;
mod usage;

use anyhow::{bail, Context, Result};
use rpc::RpcProcess;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_core::{AgentProfile, ProviderConfig, ProviderKind, TokenCounts, UsageSnapshot};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpEndpoint {
    pub command: String,
    pub args: Vec<String>,
    pub token: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnRequest {
    pub profile: AgentProfile,
    pub provider: ProviderConfig,
    pub cwd: PathBuf,
    pub prompt: String,
    pub purpose: String,
    pub read_only: bool,
    pub resume: Option<String>,
    #[serde(default)]
    pub usage_baseline: Option<TokenCounts>,
    pub mcp: Option<McpEndpoint>,
    pub timeout_secs: u64,
    pub bridge: PathBuf,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnResult {
    pub text: String,
    pub session_id: String,
    pub usage: Option<Value>,
}
#[derive(Debug, Clone)]
pub enum ProviderEvent {
    Usage(UsageSnapshot),
    Delta(String),
    Session(String),
    Tool(String),
    Retry {
        session_id: String,
        turn_id: String,
        error_code: Option<String>,
    },
}

pub async fn run_turn(
    req: TurnRequest,
    cancel: CancellationToken,
    events: mpsc::UnboundedSender<ProviderEvent>,
) -> Result<TurnResult> {
    if req.provider.kind == ProviderKind::Mock {
        return mock_turn(&req, cancel, events).await;
    }
    let timeout = req.timeout_secs;
    let work = run_native(req, events);
    tokio::select! {
        _=cancel.cancelled()=>bail!("Turn cancelled; process resources released"),
        result=tokio::time::timeout(std::time::Duration::from_secs(timeout),work)=>result.context("Provider turn timed out; outcome may be incomplete")?,
    }
}

async fn run_native(
    req: TurnRequest,
    events: mpsc::UnboundedSender<ProviderEvent>,
) -> Result<TurnResult> {
    let mut proc = RpcProcess::spawn(&req).await?;
    let result = match req.provider.kind {
        ProviderKind::Codex => codex(&mut proc, &req, &events).await,
        ProviderKind::Claude => claude(&mut proc, &req, &events).await,
        ProviderKind::Acp => acp(&mut proc, &req, &events).await,
        ProviderKind::Mock => unreachable!(),
    };
    proc.close().await;
    result
}

async fn codex(
    proc: &mut RpcProcess,
    req: &TurnRequest,
    events: &mpsc::UnboundedSender<ProviderEvent>,
) -> Result<TurnResult> {
    proc.request(
        "initialize",
        json!({"clientInfo":{"name":"ymp","version":env!("CARGO_PKG_VERSION")},"capabilities":{}}),
        events,
    )
    .await?;
    proc.notify("initialized", json!({})).await?;
    let mut params = json!({"cwd":req.cwd,"approvalPolicy":"never","sandbox":if req.read_only{"read-only"}else{"danger-full-access"},"developerInstructions":format!("{}\nAll responses, documentation, comments, and artifacts in ymp must be in English.",req.profile.instructions)});
    if let Some(model) = &req.profile.model {
        params["model"] = json!(model);
    }
    let response = if let Some(id) = &req.resume {
        params["threadId"] = json!(id);
        proc.request("thread/resume", params, events).await?
    } else {
        proc.request("thread/start", params, events).await?
    };
    let session = response
        .pointer("/thread/id")
        .and_then(Value::as_str)
        .context("Codex did not return a thread id")?
        .to_owned();
    let _ = events.send(ProviderEvent::Session(session.clone()));
    let response = proc
        .request(
            "turn/start",
            json!({"threadId":session,"input":[{"type":"text","text":req.prompt}]}),
            events,
        )
        .await?;
    let turn = response
        .pointer("/turn/id")
        .and_then(Value::as_str)
        .context("Codex did not return a turn id")?
        .to_owned();
    let mut text = String::new();
    let mut usage = None;
    let mut accounting = usage::CodexUsage::new(req.resume.is_some(), req.usage_baseline.clone());
    loop {
        let msg = proc.next().await?;
        if proc.respond_server(&msg).await? {
            continue;
        }
        let p = &msg["params"];
        if p.get("threadId")
            .and_then(Value::as_str)
            .is_some_and(|id| id != session)
        {
            continue;
        }
        if p.get("turnId")
            .and_then(Value::as_str)
            .is_some_and(|id| id != turn)
        {
            if msg["method"].as_str() == Some("thread/tokenUsage/updated") {
                accounting.restored(&p["tokenUsage"]);
            }
            continue;
        }
        match msg["method"].as_str().unwrap_or("") {
            "item/agentMessage/delta" => {
                if let Some(delta) = p["delta"].as_str() {
                    let _ = events.send(ProviderEvent::Delta(delta.into()));
                }
            }
            "item/completed" => {
                if p.pointer("/item/type").and_then(Value::as_str) == Some("agentMessage") {
                    if let Some(t) = p.pointer("/item/text").and_then(Value::as_str) {
                        text = t.into();
                    }
                }
            }
            "thread/tokenUsage/updated" => {
                usage = p.get("tokenUsage").cloned();
                if let Some(snapshot) = usage.as_ref().and_then(|v| accounting.update(v)) {
                    let _ = events.send(ProviderEvent::Usage(snapshot));
                }
            }
            "turn/completed" => {
                if p.pointer("/turn/id").and_then(Value::as_str) != Some(&turn) {
                    continue;
                }
                if p.pointer("/turn/status").and_then(Value::as_str) != Some("completed") {
                    bail!("Codex turn did not complete: {}", p["turn"]);
                }
                if let Some(snapshot) = accounting.finish() {
                    let _ = events.send(ProviderEvent::Usage(snapshot));
                }
                break;
            }
            "error" => {
                if p["willRetry"].as_bool() != Some(true) {
                    bail!("Codex error: {}", p["error"]);
                }
                // The native turn owns this retry. Keep its text and accounting
                // and let the existing invocation cancellation/deadline apply.
                let _ = events.send(ProviderEvent::Retry {
                    session_id: session.clone(),
                    turn_id: turn.clone(),
                    error_code: codex_error_code(&p["error"]["codexErrorInfo"]),
                });
            }
            _ => {}
        }
    }
    if text.is_empty() {
        bail!("Codex completed without a final response");
    }
    Ok(TurnResult {
        text,
        session_id: session,
        usage,
    })
}

fn codex_error_code(info: &Value) -> Option<String> {
    // Retain only protocol-defined codes, never arbitrary provider messages or
    // additional details that could contain credentials or request contents.
    match info.as_str() {
        Some(
            code @ ("contextWindowExceeded"
            | "sessionBudgetExceeded"
            | "usageLimitExceeded"
            | "rateLimitExceeded"
            | "serverOverloaded"
            | "cyberPolicy"
            | "misalignmentPolicyViolation"
            | "internalServerError"
            | "unauthorized"
            | "badRequest"
            | "threadRollbackFailed"
            | "sandboxError"
            | "other"),
        ) => Some(code.into()),
        _ => [
            "httpConnectionFailed",
            "responseStreamConnectionFailed",
            "responseStreamDisconnected",
            "responseTooManyFailedAttempts",
            "activeTurnNotSteerable",
        ]
        .into_iter()
        .find(|code| info.get(code).is_some())
        .map(str::to_owned),
    }
}

async fn claude(
    proc: &mut RpcProcess,
    req: &TurnRequest,
    events: &mpsc::UnboundedSender<ProviderEvent>,
) -> Result<TurnResult> {
    let response = proc
        .request("run", serde_json::to_value(req)?, events)
        .await?;
    Ok(serde_json::from_value(response)?)
}

async fn acp(
    proc: &mut RpcProcess,
    req: &TurnRequest,
    events: &mpsc::UnboundedSender<ProviderEvent>,
) -> Result<TurnResult> {
    let init=proc.request("initialize",json!({"protocolVersion":1,"clientCapabilities":{},"clientInfo":{"name":"ymp","version":env!("CARGO_PKG_VERSION")}}),events).await?;
    let mcp=req.mcp.as_ref().map(|m|vec![json!({"name":"ymp","command":m.command,"args":m.args,"env":[{"name":"YMP_MCP_TOKEN","value":m.token}]})]).unwrap_or_default();
    let params = json!({"cwd":req.cwd,"mcpServers":mcp});
    let response = if let Some(id) = &req.resume {
        if init
            .pointer("/agentCapabilities/loadSession")
            .and_then(Value::as_bool)
            != Some(true)
        {
            bail!("ACP agent cannot restore sessions");
        }
        let mut p = params;
        p["sessionId"] = json!(id);
        proc.request("session/load", p, events).await?
    } else {
        proc.request("session/new", params, events).await?
    };
    let session = response["sessionId"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| req.resume.clone())
        .context("ACP session id missing")?;
    let _ = events.send(ProviderEvent::Session(session.clone()));
    if let Some(model) = &req.profile.model {
        proc.request(
            "session/set_model",
            json!({"sessionId":session,"modelId":model}),
            events,
        )
        .await?;
    }
    if !req.read_only {
        proc.request(
            "session/set_mode",
            json!({"sessionId":session,"modeId":"bypass_permissions"}),
            events,
        )
        .await?;
    }
    let prompt = format!(
        "{}\n\n{}\n\n{}",
        req.profile.instructions,
        if req.read_only {
            "Read-only task. Do not modify files or run commands that change state."
        } else {
            ""
        },
        req.prompt
    );
    proc.acp_text.clear();
    let result = proc
        .request(
            "session/prompt",
            json!({"sessionId":session,"prompt":[{"type":"text","text":prompt}]}),
            events,
        )
        .await?;
    if let Some(snapshot) = usage::acp_usage(&result["usage"]) {
        let _ = events.send(ProviderEvent::Usage(snapshot));
    }
    if result["stopReason"].as_str() != Some("end_turn") {
        bail!("ACP turn stopped: {}", result["stopReason"]);
    }
    if proc.acp_text.is_empty() {
        bail!("ACP completed without a response");
    }
    Ok(TurnResult {
        text: proc.acp_text.clone(),
        session_id: session,
        usage: result.get("usage").cloned(),
    })
}

async fn mock_turn(
    req: &TurnRequest,
    cancel: CancellationToken,
    events: mpsc::UnboundedSender<ProviderEvent>,
) -> Result<TurnResult> {
    let report_usage = req.profile.instructions.contains("[mock:usage]");
    if report_usage {
        let _ = events.send(ProviderEvent::Usage(UsageSnapshot {
            counts: TokenCounts {
                input: Some(100),
                ..Default::default()
            },
            partial: true,
            ..Default::default()
        }));
    }
    tokio::select! {_=cancel.cancelled()=>bail!("Cancelled"),_=tokio::time::sleep(std::time::Duration::from_millis(30))=>{}}
    if req
        .profile
        .instructions
        .contains(&format!("[mock:fail:{}]", req.purpose))
    {
        bail!("Mock provider failure during {}", req.purpose);
    }
    let text=match req.purpose.as_str(){
        "conversation" => {
            if !req.prompt.contains("Original request:") || !req.prompt.contains("Previous outcome:") {
                bail!("Follow-up context is missing");
            }
            let path = req.cwd.join("greeting.txt");
            json!({"action":"answer","answer":if path.exists() {format!("The file is located at {}",path.display())} else {"The earlier run did not create a file.".into()}}).to_string()
        },
        "plan"=>json!({"summary":"Create and verify a small deliverable","tasks":[{"title":"Create a greeting","description":"Write greeting.txt containing Hello from ymp","competence":"implementation","difficulty":"simple","dependencies":[],"checks":["test -f greeting.txt && grep -q 'Hello from ymp' greeting.txt"]}]}).to_string(),
        "review_plan"|"review"|"final_review"|"review_memory"=>{
            if req.profile.instructions.contains(&format!("[mock:reject:{}]",req.purpose)) {
                json!({"approved":false,"reason":"The requested result is incomplete."}).to_string()
            } else {
                json!({"approved":true,"reason":"The stated acceptance criteria are satisfied.","lesson":"Check the produced artifact against the requested content."}).to_string()
            }
        },
        "bid"=>json!({"willing":true,"approach":"Inspect the task, implement, and verify."}).to_string(),
        "execute"=>{tokio::fs::write(req.cwd.join("greeting.txt"),if req.profile.instructions.contains("[mock:broken-output]"){ "wrong output\n" }else{"Hello from ymp\n"}).await?;"Created greeting.txt and verified its content.".into()},
        "learn"=>json!({"useful":true,"title":"Verify file-producing tasks","content":"For file-producing tasks, check both existence and requested content. Run the check on the final integrated artifact."}).to_string(),
        _=>"The requested artifact is complete and independently verified.".into(),
    };
    let _ = events.send(ProviderEvent::Delta(text.clone()));
    if report_usage {
        let _ = events.send(ProviderEvent::Usage(UsageSnapshot {
            counts: TokenCounts {
                input: Some(100),
                output: Some(20),
                ..Default::default()
            },
            finalized: true,
            ..Default::default()
        }));
    }
    Ok(TurnResult {
        text,
        session_id: req.resume.clone().unwrap_or_else(ymp_core::new_id),
        usage: None,
    })
}
