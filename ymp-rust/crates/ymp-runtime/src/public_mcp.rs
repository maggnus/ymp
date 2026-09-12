//! Public stdio facade. External scope and request identity never become team grants.
use crate::Engine;
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::mpsc,
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;
use ymp_core::*;
use ymp_storage::{projection, Store};

const PROTOCOL: &str = "2025-06-18";
const MAX_FRAME: u64 = 64 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Inspect {
    kind: String,
    session_id: Option<String>,
    cursor: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}
fn default_limit() -> usize {
    10
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Knowledge {
    #[serde(default)]
    query: String,
    id: Option<String>,
    version: Option<String>,
    #[serde(default)]
    include_unconfirmed: bool,
    cursor: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Run {
    request_id: String,
    action: String,
    prompt: Option<String>,
    session_id: Option<String>,
    max_turns: Option<usize>,
    turn_timeout_secs: Option<u64>,
    max_seconds: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestRef {
    request_id: String,
}
#[derive(Clone, Deserialize, Serialize)]
struct Operation {
    request_id: String,
    client_id: String,
    project_id: String,
    session_id: String,
    action: String,
    fingerprint: String,
    status: String,
    created_at: String,
    ended_at: Option<String>,
    error: Option<String>,
}
struct Active {
    cancel: CancellationToken,
    task: JoinHandle<()>,
}
struct Facade {
    store: Store,
    config: Config,
    project: Project,
    client: String,
    execute: bool,
    active: HashMap<String, Active>,
}

fn cursor(value: Option<&str>) -> Result<usize> {
    value
        .map(str::parse)
        .transpose()
        .context("Invalid cursor")
        .map(|v| v.unwrap_or(0))
}
fn identity(value: &str) -> Result<()> {
    ensure!(
        !value.is_empty()
            && value.len() <= 128
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.:".contains(c)),
        "Identity must contain 1–128 ASCII letters, digits, -, _, ., or :"
    );
    Ok(())
}
impl Facade {
    fn key(&self, request: &str) -> Result<String> {
        identity(request)?;
        Ok(format!(
            "public_mcp:v1:{}",
            content_digest(&serde_json::to_string(&(
                &self.project.id,
                &self.client,
                request
            ))?)
        ))
    }
    fn scoped_session(&self, id: &str) -> Result<Session> {
        let session = self
            .store
            .session(id)
            .context("scope_denied: Session is unavailable in this project")?;
        ensure!(
            session.project_id == self.project.id,
            "scope_denied: Session is unavailable in this project"
        );
        Ok(session)
    }
    fn operation(&self, request: &str) -> Result<Operation> {
        let raw = self
            .store
            .value(&self.key(request)?)?
            .context("scope_denied: Request is unavailable for this client and project")?;
        Ok(serde_json::from_value(raw)?)
    }
    fn progress(&self, request: &str) -> Result<Value> {
        let op = self.operation(request)?;
        let live = self
            .active
            .get(&self.key(request)?)
            .is_some_and(|a| !a.task.is_finished());
        let observation = if op.ended_at.is_some() {
            "terminal"
        } else if live {
            "running_here"
        } else {
            "interrupted_or_running_elsewhere"
        };
        let session = self.scoped_session(&op.session_id).ok();
        let budget = session
            .as_ref()
            .map(|s| self.store.session_budget(&s.id))
            .transpose()?;
        let usage = session
            .as_ref()
            .map(|s| self.store.session_usage(&s.id))
            .transpose()?;
        Ok(
            json!({"schema_version":1,"operation":op,"observation":observation,"session":projection::metadata(&session.as_ref().map(session_metadata))?,"budget":projection::metadata(&budget)?,"usage":projection::metadata(&usage)?}),
        )
    }
    fn inspect(&self, args: Inspect) -> Result<Value> {
        ensure!(
            (1..=25).contains(&args.limit),
            "limit must be between 1 and 25"
        );
        let offset = cursor(args.cursor.as_deref())?;
        if args.kind == "scope" {
            ensure!(
                args.session_id.is_none() && offset == 0,
                "scope does not take a session or cursor"
            );
            return Ok(
                json!({"schema_version":1,"project":self.project,"client_id":self.client,"actions":if self.execute { vec!["read","execute","cancel"] } else { vec!["read"] },"workspace_policy":"direct_mvp","protocol_version":PROTOCOL}),
            );
        }
        if args.kind == "pool" {
            ensure!(args.session_id.is_none(), "pool does not take a session");
            let pool = ymp_providers::discovery::inspect_pool(&self.config)?;
            let agents: Vec<_> = pool.agents.into_iter().map(|a| json!({"id":a.profile.id,"name":a.profile.name,"provider":a.profile.provider,"model":a.profile.model,"profile_version":a.profile_version,"exclusions":a.exclusions,"model_status":a.model_status})).collect();
            return projection::page(&agents, offset, args.limit);
        }
        if args.kind == "sessions" {
            ensure!(
                args.session_id.is_none(),
                "sessions does not take a session"
            );
            let mut sessions = self.store.sessions(Some(&self.project.id))?;
            sessions.sort_by(|a, b| a.id.cmp(&b.id));
            return projection::page(
                &sessions.iter().map(session_metadata).collect::<Vec<_>>(),
                offset,
                args.limit,
            );
        }
        let session = self.scoped_session(
            args.session_id
                .as_deref()
                .context("session_id is required")?,
        )?;
        match args.kind.as_str() {
            "session" => {
                ensure!(offset == 0, "session does not take a cursor");
                Ok(
                    json!({"schema_version":1,"session":projection::metadata(&session_metadata(&session))?,"budget":projection::metadata(&self.store.session_budget(&session.id)?)?,"usage":projection::metadata(&self.store.session_usage(&session.id)?)?,"team":projection::metadata(&self.store.team_state(&session.id)?)?}),
                )
            }
            "tasks" => projection::page(&self.store.tasks(&session.id)?, offset, args.limit),
            "results" => projection::page(&self.store.outcomes(&session.id)?, offset, args.limit),
            "evidence" => {
                let records: Vec<_> = self
                    .store
                    .decisions(&session.id)?
                    .into_iter()
                    .filter(|d| {
                        matches!(
                            d.kind.as_str(),
                            "task_accepted"
                                | "task_rejected"
                                | "confirmation_checked"
                                | "acceptance_contract_captured"
                        ) || d.links.check.is_some()
                    })
                    .collect();
                projection::page(&records, offset, args.limit)
            }
            "history" => {
                ensure!(
                    (1..=25).contains(&args.limit),
                    "limit must be between 1 and 25"
                );
                let after = i64::try_from(offset).context("Invalid history cursor")?;
                let mut messages = self.store.messages(&session.id, after, args.limit + 1)?;
                let more = messages.len() > args.limit;
                messages.truncate(args.limit);
                let mut result = projection::page(&messages, 0, args.limit)?;
                let returned = result["items"].as_array().map_or(0, Vec::len);
                result["next_cursor"] = if more || returned < messages.len() {
                    json!(messages[returned - 1].seq.to_string())
                } else {
                    Value::Null
                };
                result.as_object_mut().unwrap().remove("total");
                Ok(result)
            }
            _ => bail!("Unknown inspection kind"),
        }
    }
    fn knowledge(&self, args: Knowledge) -> Result<Value> {
        ensure!(args.query.len() <= 4096, "Query exceeds 4096 bytes");
        let mode = if args.include_unconfirmed {
            KnowledgeRetrievalMode::IncludeUnconfirmed
        } else {
            KnowledgeRetrievalMode::Supported
        };
        let entries = if let Some(id) = args.id {
            ensure!(args.query.is_empty(), "Use query or id, not both");
            vec![self
                .store
                .resolve_memory(
                    Some(&self.project.id),
                    &id,
                    args.version.as_deref(),
                    &Default::default(),
                    mode,
                )?
                .context("Knowledge is unavailable at this scope/version/confirmation mode")?]
        } else {
            ensure!(args.version.is_none(), "version requires id");
            self.store.search_memory(
                Some(&self.project.id),
                &args.query,
                &Default::default(),
                mode,
            )?
        };
        let rows = entries.iter().map(|entry| Ok(json!({"id":entry.id,"version":content_digest(&serde_json::to_string(entry)?),"entry":entry}))).collect::<Result<Vec<_>>>()?;
        projection::page(&rows, cursor(args.cursor.as_deref())?, args.limit)
    }
    fn start(&mut self, args: Run) -> Result<Value> {
        ensure!(
            self.execute,
            "execution_denied: launch with --allow-execution to enable bounded runs"
        );
        identity(&args.request_id)?;
        ensure!(
            (1..=3600).contains(&args.max_seconds),
            "max_seconds must be between 1 and 3600"
        );
        let key = self.key(&args.request_id)?;
        let fingerprint = content_digest(&serde_json::to_string(&args)?);
        if let Some(raw) = self.store.value(&key)? {
            let previous: Operation = serde_json::from_value(raw)?;
            ensure!(
                previous.fingerprint == fingerprint,
                "request_conflict: request_id already identifies different arguments"
            );
            return self.progress(&args.request_id);
        }
        self.active.retain(|_, a| !a.task.is_finished());
        ensure!(
            self.active.is_empty(),
            "execution_busy: this facade already owns a run; inspect or cancel it first"
        );
        // The project binding comes from the launcher; a later relocation cannot retarget it.
        ensure!(
            self.store.get_project(&self.project.id)?.path == self.project.path,
            "project_relocated: reopen the facade at the current project path"
        );
        let mut config = self.config.clone();
        let (session_id, prompt, resume) = match args.action.as_str() {
            "start" => {
                ensure!(
                    args.session_id.is_none(),
                    "start does not accept session_id"
                );
                let prompt = args
                    .prompt
                    .as_deref()
                    .filter(|p| !p.trim().is_empty() && p.len() <= 16_384)
                    .context("start requires a nonempty prompt of at most 16384 bytes")?;
                let turns = args.max_turns.context("start requires max_turns")?;
                let timeout = args
                    .turn_timeout_secs
                    .context("start requires turn_timeout_secs")?;
                ensure!(
                    turns > 0 && turns <= config.limits.turns,
                    "max_turns must be positive and cannot exceed configured turns"
                );
                ensure!(
                    timeout > 0 && timeout <= config.limits.turn_timeout_secs,
                    "turn_timeout_secs must be positive and cannot exceed configured timeout"
                );
                config.limits.turns = turns;
                config.limits.turn_timeout_secs = timeout;
                config.validate()?;
                (new_id(), prompt.to_owned(), false)
            }
            "resume" => {
                ensure!(args.prompt.is_none() && args.max_turns.is_none() && args.turn_timeout_secs.is_none(), "resume uses the captured prompt and remaining budget; omit prompt and budget overrides");
                let session = self.scoped_session(
                    args.session_id
                        .as_deref()
                        .context("resume requires session_id")?,
                )?;
                ensure!(
                    session.status != "completed",
                    "Completed sessions cannot be resumed"
                );
                let policy = self
                    .store
                    .session_policy(&session.id)?
                    .context("Resume requires captured runtime policy")?;
                ensure!(
                    policy.cwd == self.project.path,
                    "Captured workspace differs from this facade; inspect before relocating"
                );
                (session.id, String::new(), true)
            }
            _ => bail!("action must be start or resume"),
        };
        let operation = Operation {
            request_id: args.request_id.clone(),
            client_id: self.client.clone(),
            project_id: self.project.id.clone(),
            session_id: session_id.clone(),
            action: args.action,
            fingerprint,
            status: "pending".into(),
            created_at: now(),
            ended_at: None,
            error: None,
        };
        ensure!(
            self.store
                .put_value_if_absent(&key, &serde_json::to_value(&operation)?)?,
            "request_conflict: concurrent request claim; retry the same arguments"
        );
        let cancel = CancellationToken::new();
        let (events, receiver) = mpsc::unbounded_channel();
        drop(receiver); // Progress comes from persisted domain state, never an unbounded UI queue.
        let engine = Engine::new(self.store.clone(), config, events, cancel.clone())?;
        let db = self.store.clone();
        let path = self.project.path.clone();
        let run_cancel = cancel.clone();
        let run_key = key.clone();
        let task = tokio::spawn(async move {
            let run = async {
                if resume {
                    engine.run(&path, &prompt, Some(&session_id)).await
                } else {
                    engine.run_identified(&path, &prompt, &session_id).await
                }
            };
            tokio::pin!(run);
            let result = tokio::select! {
                result = &mut run => result,
                _ = tokio::time::sleep(Duration::from_secs(args.max_seconds)) => {
                    run_cancel.cancel();
                    tokio::time::timeout(Duration::from_secs(5), &mut run).await.unwrap_or_else(|_| Err(anyhow::anyhow!("Run cancellation grace expired; inspect interrupted work before resuming")))
                }
            };
            let mut op = operation;
            match result {
                Ok(outcome) => op.status = outcome.session.status,
                Err(e) => {
                    op.status = "interrupted".into();
                    op.error = Some(e.to_string().chars().take(1024).collect());
                }
            }
            op.ended_at = Some(now());
            let _ = db.put_value(&run_key, &json!(op));
        });
        self.active.insert(key, Active { cancel, task });
        self.progress(&args.request_id)
    }
    fn call(&mut self, name: &str, arguments: Value) -> Result<Value> {
        match name {
            "ymp_inspect_v1" => self.inspect(serde_json::from_value(arguments)?),
            "ymp_knowledge_v1" => self.knowledge(serde_json::from_value(arguments)?),
            "ymp_run_v1" => self.start(serde_json::from_value(arguments)?),
            "ymp_request_v1" => {
                self.progress(&serde_json::from_value::<RequestRef>(arguments)?.request_id)
            }
            "ymp_cancel_v1" => {
                ensure!(
                    self.execute,
                    "execution_denied: cancellation requires --allow-execution"
                );
                let request = serde_json::from_value::<RequestRef>(arguments)?.request_id;
                let operation = self.operation(&request)?;
                if let Some(active) = self.active.get(&self.key(&request)?) {
                    active.cancel.cancel();
                } else {
                    ensure!(
                        operation.ended_at.is_some(),
                        "not_owned: cancellation can only signal a run owned by this process"
                    );
                }
                self.progress(&request)
            }
            _ => bail!("Unknown public tool"),
        }
    }
    async fn shutdown(self) {
        for active in self.active.values() {
            active.cancel.cancel();
        }
        for (_, mut active) in self.active {
            if tokio::time::timeout(Duration::from_secs(5), &mut active.task)
                .await
                .is_err()
            {
                active.task.abort();
                let _ = active.task.await;
            }
        }
    }
}

fn session_metadata(s: &Session) -> Value {
    json!({"id":s.id,"project_id":s.project_id,"title":s.title,"status":s.status,"created_at":s.created_at,"agent_ids":s.team.iter().map(|a| &a.id).collect::<Vec<_>>(),"turns_used":s.turns_used})
}

pub fn tools() -> Value {
    let page = json!({"type":"string","description":"Opaque continuation returned as next_cursor. Stable row ordering; concurrent changes can require restarting."});
    let limit = json!({"type":"integer","minimum":1,"maximum":25,"default":10});
    let tools = vec![
        ("ymp_inspect_v1", "Inspect project scope, pool and saved runtime state without invoking agents. Responses omit snapshot bytes and mark truncation.", json!({"kind":{"type":"string","enum":["scope","pool","sessions","session","tasks","results","evidence","history"]},"session_id":{"type":"string"},"cursor":page,"limit":limit}), vec!["kind"], true),
        ("ymp_knowledge_v1", "Search or resolve project/shared knowledge without inference. Unconfirmed inspection is explicit. Entry versions identify the complete source even when its projection is truncated.", json!({"query":{"type":"string","maxLength":4096},"id":{"type":"string"},"version":{"type":"string"},"include_unconfirmed":{"type":"boolean","default":false},"cursor":page,"limit":limit}), vec![], true),
        ("ymp_run_v1", "Explicitly start or resume bounded work in the launcher's project using the common runtime. Returns a durable request handle immediately. Start requires prompt/max_turns/turn_timeout_secs; resume requires session_id and retains captured limits. max_seconds bounds this process's execution lease. Retry identical request_id/arguments to inspect, never repeat effects.", json!({"request_id":{"type":"string","minLength":1,"maxLength":128},"action":{"type":"string","enum":["start","resume"]},"prompt":{"type":"string","maxLength":16384},"session_id":{"type":"string"},"max_turns":{"type":"integer","minimum":1},"turn_timeout_secs":{"type":"integer","minimum":1},"max_seconds":{"type":"integer","minimum":1,"maximum":3600}}), vec!["request_id","action","max_seconds"], false),
        ("ymp_request_v1", "Inspect durable operation and saved progress/usage. A foreign process's liveness stays unknown; no agents are started.", json!({"request_id":{"type":"string"}}), vec!["request_id"], true),
        ("ymp_cancel_v1", "Request cancellation of an execution owned by this process. Poll request until terminal. Saved work and observed usage remain; cancellation does not roll back workspace writes.", json!({"request_id":{"type":"string"}}), vec!["request_id"], false),
    ];
    Value::Array(tools.into_iter().map(|(name, description, properties, required, read)| json!({"name":name,"description":description,"inputSchema":{"type":"object","additionalProperties":false,"properties":properties,"required":required},"annotations":{"readOnlyHint":read,"destructiveHint":!read,"openWorldHint":!read}})).collect())
}
fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
}
fn tool_result(value: Result<Value>) -> Value {
    let (value, failed) = match value {
        Ok(v) => (v, false),
        Err(e) => (
            json!({"schema_version":1,"code":"tool_error","message":e.to_string().chars().take(2048).collect::<String>()}),
            true,
        ),
    };
    json!({"content":[{"type":"text","text":value.to_string()}],"structuredContent":value,"isError":failed})
}

/// The stdio process has a fixed project and externally granted action scope.
/// Native provider configuration/authentication stays owned by Engine.
pub async fn serve(
    store: Store,
    config: Config,
    path: PathBuf,
    client: String,
    execute: bool,
) -> Result<()> {
    identity(&client)?;
    let project = store.project(&path)?;
    let mut facade = Facade {
        store,
        config,
        project,
        client,
        execute,
        active: HashMap::new(),
    };
    let result = exchange(&mut facade).await;
    facade.shutdown().await;
    result
}

async fn exchange(facade: &mut Facade) -> Result<()> {
    let mut input = BufReader::new(tokio::io::stdin());
    let mut output = tokio::io::stdout();
    let mut initialized = false;
    let mut ready = false;
    let mut ids = HashSet::new();
    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);
    loop {
        let mut line = String::new();
        let read = async { (&mut input).take(MAX_FRAME + 1).read_line(&mut line).await };
        let count = tokio::select! { _ = &mut shutdown => break, count = read => count? };
        if count == 0 {
            break;
        }
        if count as u64 > MAX_FRAME {
            output
                .write_all(
                    format!(
                        "{}\n",
                        rpc_error(
                            Value::Null,
                            -32600,
                            "Request exceeds 65536 bytes; connection closed"
                        )
                    )
                    .as_bytes(),
                )
                .await?;
            break;
        }
        let req: Value = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(_) => {
                output
                    .write_all(
                        format!("{}\n", rpc_error(Value::Null, -32700, "Parse error")).as_bytes(),
                    )
                    .await?;
                output.flush().await?;
                continue;
            }
        };
        let id = req.get("id").cloned();
        if req["jsonrpc"] != "2.0"
            || !req["method"].is_string()
            || id
                .as_ref()
                .is_some_and(|id| !id.is_string() && !id.is_i64() && !id.is_u64())
        {
            output
                .write_all(
                    format!("{}\n", rpc_error(Value::Null, -32600, "Invalid request")).as_bytes(),
                )
                .await?;
            output.flush().await?;
            continue;
        }
        let method = req["method"].as_str().unwrap();
        if req.get("params").is_some_and(|p| !p.is_object()) {
            if let Some(id) = id {
                output
                    .write_all(
                        format!(
                            "{}\n",
                            rpc_error(id, -32602, "Parameters must be an object")
                        )
                        .as_bytes(),
                    )
                    .await?;
                output.flush().await?;
            }
            continue;
        }
        let Some(id) = id else {
            if method == "notifications/initialized" && initialized {
                ready = true;
            }
            // Calls return handles immediately. Cancellation notifications for completed
            // RPCs are ignored; ymp_cancel_v1 cancels the separate durable operation.
            continue;
        };
        let response = if ids.len() >= 10_000 {
            rpc_error(
                id,
                -32000,
                "Connection request limit reached; reopen the process",
            )
        } else if !ids.insert(id.to_string()) {
            rpc_error(
                id,
                -32600,
                "Request IDs must be unique within this connection",
            )
        } else if method == "initialize" && !initialized {
            if !req["params"]["protocolVersion"].is_string()
                || !req["params"]["capabilities"].is_object()
                || !req["params"]["clientInfo"].is_object()
            {
                rpc_error(id, -32602, "Invalid initialize parameters")
            } else {
                initialized = true;
                json!({"jsonrpc":"2.0","id":id,"result":{"protocolVersion":PROTOCOL,"capabilities":{"tools":{}},"serverInfo":{"name":"ymp","version":env!("CARGO_PKG_VERSION")},"instructions":"Project scope is fixed by the launcher. Reads never invoke agents. Execution needs --allow-execution and explicit bounded ymp_run_v1. Poll ymp_request_v1; use ymp_cancel_v1 for durable execution cancellation."}})
            }
        } else if method == "ping" {
            json!({"jsonrpc":"2.0","id":id,"result":{}})
        } else if !ready {
            rpc_error(
                id,
                -32002,
                "Initialize and send notifications/initialized first",
            )
        } else {
            match method {
                "tools/list" if req["params"].get("cursor").is_none() => {
                    json!({"jsonrpc":"2.0","id":id,"result":{"tools":tools()}})
                }
                "tools/list" => rpc_error(id, -32602, "Tool list has no continuation cursor"),
                "tools/call" => {
                    let name = req["params"]["name"].as_str().unwrap_or("");
                    if !tools()
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|t| t["name"] == name)
                    {
                        rpc_error(id, -32602, "Unknown tool")
                    } else {
                        let args = req["params"].get("arguments").cloned().unwrap_or(json!({}));
                        json!({"jsonrpc":"2.0","id":id,"result":tool_result(facade.call(name,args))})
                    }
                }
                _ => rpc_error(id, -32601, "Method not found"),
            }
        };
        let mut encoded = serde_json::to_vec(&response)?;
        if encoded.len() > 128 * 1024 {
            encoded = serde_json::to_vec(&rpc_error(
                response["id"].clone(),
                -32603,
                "Response exceeds transport limit; request a smaller page",
            ))?;
        }
        encoded.push(b'\n');
        output.write_all(&encoded).await?;
        output.flush().await?;
    }
    Ok(())
}
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        if let Ok(mut signal) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! { _ = signal.recv() => {}, _ = tokio::signal::ctrl_c() => {} }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_tool_schemas_match_discovery() {
        let published: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../ymp-docs/architecture/public-mcp-tools-v1.json"
        )))
        .unwrap();
        assert_eq!(published["tools"], tools());
    }

    #[tokio::test]
    async fn identified_start_is_insert_only_and_retains_spend() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("metadata");
        let project = root.path().join("project");
        std::fs::create_dir(&project).unwrap();
        let store = Store::open(&home).unwrap();
        let config: Config = serde_json::from_value(json!({
            "version":1,"limits":{"turns":80,"turn_timeout_secs":10},
            "providers":[{"id":"mock","kind":"mock","command":"internal"}],
            "agents":[
                {"id":"one","name":"One","provider":"mock","model":"mock"},
                {"id":"two","name":"Two","provider":"mock","model":"mock"}],
            "execution":{"one":{"fixed":{"model":"mock","effort":"low"}},"two":{"fixed":{"model":"mock","effort":"low"}}},
            "team":["one","two"]
        })).unwrap();
        let (events, _) = mpsc::unbounded_channel();
        let engine = Engine::new(store.clone(), config, events, CancellationToken::new()).unwrap();
        let id = new_id();
        let first = engine
            .run_identified(&project, "Create greeting.txt", &id)
            .await
            .unwrap();
        assert_eq!(first.session.id, id);
        assert_eq!(first.session.status, "completed");
        let before = store.trace(&id).unwrap();
        assert!(engine
            .run_identified(&project, "Overwrite original goal", &id)
            .await
            .is_err());
        assert!(engine
            .run_identified(&project, "Invalid", "../unsafe")
            .await
            .is_err());
        let after = store.trace(&id).unwrap();
        assert_eq!(before.invocations.len(), after.invocations.len());
        assert_eq!(before.policy.unwrap().goal, after.policy.unwrap().goal);
        assert_eq!(before.session.turns_used, after.session.turns_used);
    }
}
