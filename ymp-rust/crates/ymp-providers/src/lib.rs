pub mod discovery;
mod redaction;
mod rpc;
mod settings;
pub mod supervisor;
mod usage;

use anyhow::{bail, Context, Result};
use rpc::RpcProcess;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use ymp_core::{
    AgentProfile, ExecutionSettings, InvocationObservation, ProviderConfig, ProviderKind,
    TokenCounts, UsageSnapshot,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct McpEndpoint {
    pub command: String,
    pub args: Vec<String>,
    pub token: String,
}
impl std::fmt::Debug for McpEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpEndpoint")
            .field("command", &self.command)
            .field("args", &self.args)
            .field("token", &"[redacted]")
            .finish()
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnRequest {
    pub profile: AgentProfile,
    #[serde(default)]
    pub settings: ExecutionSettings,
    pub provider: ProviderConfig,
    pub cwd: PathBuf,
    pub prompt: String,
    pub purpose: String,
    pub read_only: bool,
    pub resume: Option<String>,
    #[serde(default)]
    pub usage_baseline: Option<TokenCounts>,
    pub mcp: Option<McpEndpoint>,
    #[serde(default)]
    pub resource_controls: ymp_core::NativeResourceControls,
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
    Capabilities(ymp_core::ProviderCapabilities),
    Execution(Box<InvocationObservation>),
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
    let capability = req.mcp.as_ref().map(|mcp| mcp.token.clone());
    run_turn_inner(req, cancel, events)
        .await
        .map_err(|error| redaction::team_capability(error, capability.as_deref()))
}

async fn run_turn_inner(
    req: TurnRequest,
    cancel: CancellationToken,
    events: mpsc::UnboundedSender<ProviderEvent>,
) -> Result<TurnResult> {
    ymp_core::ModelEffort {
        model: req.settings.model.clone(),
        effort: req.settings.effort.clone(),
    }
    .validate()?;
    match req.settings.permission_mode.as_deref() {
        None => {}
        Some("read_only") if req.read_only => {}
        Some("write") if !req.read_only => {}
        _ => {
            bail!("Requested permission guarantee is unsupported or disagrees with the assignment")
        }
    }
    let _ = events.send(ProviderEvent::Execution(Box::new(InvocationObservation {
        permission_limitations: vec![match req.provider.kind {
            ProviderKind::Codex => "Codex receives the selected native sandbox and approval policy. Team API authorization does not contain host filesystem or process access.",
            ProviderKind::Claude => "Claude receives native tool and permission settings. The SDK tool allowlist is not OS filesystem or process isolation.",
            ProviderKind::Acp => "ACP receives an advertised permission mode and denies approval requests during read assignments. Approval callbacks and instructions are not OS filesystem or process isolation.",
            ProviderKind::Mock => "Offline fixture execution has no native sandbox; team API checks cover coordination authority only.",
        }.into()],
        ..Default::default()
    })));
    let timeout = req.timeout_secs;
    let max_output = req.resource_controls.max_output_chars;
    let (forward, mut rx) = mpsc::unbounded_channel();
    let mut output_chars = 0u64;
    let work = async {
        let native = run_native(req, forward);
        tokio::pin!(native);
        loop {
            tokio::select! {
                result = &mut native => break result,
                Some(event) = rx.recv() => {
                    if let ProviderEvent::Delta(text) = &event {
                        output_chars = output_chars.saturating_add(text.chars().count() as u64);
                    }
                    let _ = events.send(event);
                    if max_output.is_some_and(|max| output_chars > max) {
                        bail!("output_limit: Native stream exceeds the visible-output character limit");
                    }
                },
            }
        }
    };
    let result = tokio::select! {
        _=cancel.cancelled()=>Err(anyhow::anyhow!("Turn cancelled; process resources released")),
        result=tokio::time::timeout(std::time::Duration::from_secs(timeout),work)=>
            result.context("Provider turn timed out; outcome may be incomplete").and_then(|r| r),
    };
    // Drain on every terminal path, including timeout, cancellation and a stream
    // limit. Usage already received must not disappear with the forwarding queue.
    while let Ok(event) = rx.try_recv() {
        if let ProviderEvent::Delta(text) = &event {
            output_chars = output_chars.saturating_add(text.chars().count() as u64);
        }
        let _ = events.send(event);
    }
    if max_output.is_some_and(|max| output_chars > max) {
        bail!("output_limit: Native stream exceeds the visible-output character limit");
    }
    let result = result?;
    if max_output.is_some_and(|max| result.text.chars().count() as u64 > max) {
        bail!("output_limit: Native result exceeds the visible-output character limit");
    }
    Ok(result)
}

/// Query native control/metadata interfaces without sending a model prompt.
/// ACP controls describe the selected model; uninspected models remain unknown.
pub async fn inspect_capabilities(
    mut req: TurnRequest,
    cancel: CancellationToken,
) -> Result<ymp_core::ProviderCapabilities> {
    req.resume = None;
    req.mcp = None;
    req.read_only = true;
    req.prompt.clear();
    let (events, _rx) = mpsc::unbounded_channel();
    let timeout = req.timeout_secs;
    let work = async {
        if req.provider.kind == ProviderKind::Mock {
            return Ok(ymp_core::ProviderCapabilities::default());
        }
        let mut proc = RpcProcess::spawn(&req).await?;
        let result = async {
            match req.provider.kind {
                ProviderKind::Codex => {
                    proc.request("initialize", json!({"clientInfo":{"name":"ymp","version":env!("CARGO_PKG_VERSION")},"capabilities":{}}), &events).await?;
                    proc.notify("initialized", json!({})).await?;
                    settings::codex_catalog(&mut proc, &events).await
                }
                ProviderKind::Claude => Ok(serde_json::from_value(proc.request("capabilities", serde_json::to_value(&req)?, &events).await?)?),
                ProviderKind::Acp => {
                    proc.request("initialize", json!({"protocolVersion":1,"clientCapabilities":{},"clientInfo":{"name":"ymp","version":env!("CARGO_PKG_VERSION")}}), &events).await?;
                    let response = proc.request("session/new", json!({"cwd":req.cwd,"mcpServers":[]}), &events).await?;
                    let mut catalog = settings::acp_catalog(&response, "session/new")?;
                    if let Some(model) = &req.settings.model {
                        proc.acp_config_options = None;
                        proc.request("session/set_model", json!({"sessionId":response["sessionId"],"modelId":model}), &events).await?;
                        let options = proc.acp_config_options.clone().unwrap_or(Value::Null);
                        settings::refreshed_acp_model(&mut catalog, model, &options)?;
                    }
                    Ok(catalog)
                }
                ProviderKind::Mock => unreachable!(),
            }
        }.await;
        proc.close().await;
        result
    };
    tokio::select! {
        _ = cancel.cancelled() => bail!("Capability inspection cancelled"),
        result = tokio::time::timeout(std::time::Duration::from_secs(timeout), work) => result.context("Native capability inspection timed out")?,
    }
}

async fn run_native(
    req: TurnRequest,
    events: mpsc::UnboundedSender<ProviderEvent>,
) -> Result<TurnResult> {
    if req.provider.kind == ProviderKind::Mock {
        return mock_turn(&req, CancellationToken::new(), events).await;
    }
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
    let catalog = settings::codex_catalog(proc, events).await?;
    if let Some(model) = &req.settings.model {
        settings::validate_choice(&catalog, model, req.settings.effort.as_deref(), "effort")?;
    }
    let mut params = json!({"cwd":req.cwd,"approvalPolicy":"never","sandbox":if req.read_only{"read-only"}else{"danger-full-access"},"developerInstructions":format!("{}\nAll responses, documentation, comments, and artifacts in ymp must be in English.",req.profile.instructions)});
    if let Some(model) = &req.settings.model {
        params["model"] = json!(model);
    }
    if let Some(effort) = &req.settings.effort {
        params["config"] = json!({"model_reasoning_effort":effort});
    }
    if let Some(mcp) = &req.mcp {
        // Explicitly replace any saved thread configuration on both start and
        // resume. The secret travels only through the current process environment.
        params["config"]["mcp_servers"]["ymp"] = json!({
            "command":mcp.command, "args":mcp.args, "env_vars":["YMP_MCP_TOKEN"],
            "required":true, "default_tools_approval_mode":"approve"
        });
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
    let native_model = response.get("model").and_then(Value::as_str);
    let native_effort = response.get("reasoningEffort").and_then(Value::as_str);
    let _ = events.send(ProviderEvent::Execution(Box::new(InvocationObservation {
        reported: Some(ExecutionSettings {
            model: native_model.map(str::to_owned),
            effort: native_effort.map(str::to_owned),
            permission_mode: None,
        }),
        ..Default::default()
    })));
    if let Some(model) = native_model {
        settings::validate_choice(&catalog, model, req.settings.effort.as_deref(), "effort")?;
        if req
            .settings
            .model
            .as_deref()
            .is_some_and(|requested| requested != model)
        {
            bail!("Codex acknowledged a different model than requested");
        }
    } else if req.settings.effort.is_some() && req.settings.model.is_none() {
        bail!("Codex did not identify the native default model for effort validation");
    }
    if let (Some(requested), Some(actual)) = (req.settings.effort.as_deref(), native_effort) {
        if requested != actual {
            bail!("Codex acknowledged a different effort than requested");
        }
    }
    let mut turn_params = json!({"threadId":session,"input":[{"type":"text","text":req.prompt}]});
    if let Some(model) = &req.settings.model {
        turn_params["model"] = json!(model);
    }
    if let Some(effort) = &req.settings.effort {
        turn_params["effort"] = json!(effort);
    }
    let response = proc.request("turn/start", turn_params, events).await?;
    let turn = response
        .pointer("/turn/id")
        .and_then(Value::as_str)
        .context("Codex did not return a turn id")?
        .to_owned();
    let _ = events.send(ProviderEvent::Execution(Box::new(InvocationObservation {
        native_turn_id: Some(turn.clone()),
        ..Default::default()
    })));
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
    let _ = events.send(ProviderEvent::Execution(Box::new(InvocationObservation {
        native_version: init
            .pointer("/agentInfo/version")
            .and_then(Value::as_str)
            .map(str::to_owned),
        ..Default::default()
    })));
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
    let mut catalog = settings::acp_catalog(
        &response,
        if req.resume.is_some() {
            "session/load"
        } else {
            "session/new"
        },
    )?;
    let mut selected_model = response
        .pointer("/models/currentModelId")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let mut options = response["configOptions"].clone();
    if let Some(model) = &req.settings.model {
        proc.acp_config_options = None;
        proc.request(
            "session/set_model",
            json!({"sessionId":session,"modelId":model}),
            events,
        )
        .await?;
        selected_model = Some(model.clone());
        // GLM clamps thought_level on model change. Never validate against the
        // previous model's list if no refreshed options were observed.
        options = proc.acp_config_options.clone().unwrap_or(Value::Null);
        settings::refreshed_acp_model(&mut catalog, model, &options)?;
    }
    settings::publish(&catalog, events);
    let mut native_effort = settings::acp_effort(&options);
    let _ = events.send(ProviderEvent::Execution(Box::new(InvocationObservation {
        reported: Some(ExecutionSettings {
            model: if req.settings.model.is_none() {
                selected_model.clone()
            } else {
                None
            },
            effort: native_effort.clone(),
            permission_mode: None,
        }),
        ..Default::default()
    })));

    if let Some(effort) = &req.settings.effort {
        let model = selected_model
            .as_deref()
            .context("ACP default model was not identified")?;
        settings::validate_choice(&catalog, model, Some(effort), "thought_level")?;
        let ack = proc
            .request(
                "session/set_config_option",
                json!({"sessionId":session,"configId":"thought_level","value":effort}),
                events,
            )
            .await?;
        native_effort = settings::acp_effort(&ack["configOptions"]);
        let _ = events.send(ProviderEvent::Execution(Box::new(InvocationObservation {
            reported: Some(ExecutionSettings {
                model: None,
                effort: native_effort.clone(),
                permission_mode: None,
            }),
            ..Default::default()
        })));
        if native_effort.as_ref() != Some(effort) {
            bail!("ACP did not acknowledge the requested thought_level");
        }
    }
    let _ = events.send(ProviderEvent::Execution(Box::new(InvocationObservation {
        // set_model returns only {}, an acknowledgement rather than a model
        // report. Retain the reported model only when it was not changed.
        reported: Some(ExecutionSettings {
            model: if req.settings.model.is_none() {
                selected_model
            } else {
                None
            },
            effort: native_effort,
            permission_mode: None,
        }),
        ..Default::default()
    })));
    let advertised = response
        .pointer("/modes/availableModes")
        .and_then(Value::as_array)
        .context("ACP did not advertise permission modes; requested authority cannot be applied")?;
    let candidates: &[&str] = if req.read_only {
        &["read-only", "read_only", "plan", "default"]
    } else {
        &["bypass_permissions"]
    };
    let mode = candidates
        .iter()
        .find(|id| advertised.iter().any(|m| m["id"].as_str() == Some(**id)))
        .context("ACP has no compatible advertised permission mode")?;
    // Apply on every invocation, including restored native contexts that may
    // have previously used bypass_permissions.
    proc.request(
        "session/set_mode",
        json!({"sessionId":session,"modeId":mode}),
        events,
    )
    .await?;
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
    let _ = events.send(ProviderEvent::Execution(Box::new(InvocationObservation {
        sent: Some(req.settings.clone()),
        ..Default::default()
    })));
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
    // The revision fixture responds to the current assignment, independently of
    // earlier proposals retained in shared-message context.
    let marker = format!("Your current assignment ({}):\n", req.purpose);
    let assignment = req.prompt.rsplit(&marker).next().unwrap_or(&req.prompt);
    let revise_plan = req.profile.instructions.contains("[mock:revise-plan]");
    let acceptance_checks = if req.profile.instructions.contains("[mock:no-checks]") {
        vec![]
    } else if req.profile.instructions.contains("[mock:unrelated-check]") {
        vec!["true"]
    } else {
        vec!["test -f greeting.txt && grep -q 'Hello from ymp' greeting.txt"]
    };
    let split_writers = req.profile.instructions.contains("[mock:split-writers]");
    let mut plan_tasks = vec![
        json!({"title":"Create a greeting","description":"Write greeting.txt containing Hello from ymp","competence":"implementation","difficulty":"simple","dependencies":[],"checks":acceptance_checks}),
    ];
    if split_writers {
        plan_tasks.push(json!({"title":"Create another greeting","description":"Inspect and complete the second component","competence":"implementation","difficulty":"simple","dependencies":[0],"checks":[]}));
    }
    let text=match req.purpose.as_str(){
        "conversation" => {
            if !req.prompt.contains("Original request:") || !req.prompt.contains("Previous outcome:") {
                bail!("Follow-up context is missing");
            }
            let path = req.cwd.join("greeting.txt");
            json!({"action":"answer","answer":if path.exists() {format!("The file is located at {}",path.display())} else {"The earlier run did not create a file.".into()}}).to_string()
        },
        "plan"=>json!({"summary":if revise_plan && assignment.starts_with("Revise your plan") {"Revised mock plan"} else {"Create and verify a small deliverable"},"tasks":plan_tasks}).to_string(),
        "review_plan"|"review"|"final_review"|"review_memory"=>{
            if (req.profile.instructions.contains("[mock:dispute]") && req.purpose == "review" && assignment.starts_with("Independently inspect")) || req.profile.instructions.contains(&format!("[mock:reject:{}]",req.purpose)) || (revise_plan && req.purpose == "review_plan" && !assignment.contains("Revised mock plan")) {
                json!({"approved":false,"reason":"The requested result is incomplete."}).to_string()
            } else {
                json!({"approved":true,"reason":"The stated acceptance criteria are satisfied.","lesson":"Check the produced artifact against the requested content."}).to_string()
            }
        },
        "bid"=>json!({"willing":!split_writers || (assignment.contains("Create another greeting") == (req.profile.id == "two")),"approach":"Inspect the task, implement, and verify."}).to_string(),
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
