use anyhow::{bail, ensure, Context, Result};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
use ymp_core::{
    new_id, AssignmentRecord, GrantRecord, InvocationRecord, InvocationState, TeamOperation,
    UiEvent,
};
use ymp_storage::Store;

struct ActiveGrant {
    record: GrantRecord,
    requests: HashSet<String>,
}
type ActiveGrants = Arc<Mutex<HashMap<String, ActiveGrant>>>;

pub struct TeamServer {
    pub socket: PathBuf,
    store: Store,
    session: String,
    active: ActiveGrants,
    cancel: CancellationToken,
}
impl TeamServer {
    pub async fn start(
        store: Store,
        session: &ymp_core::Session,
        events: mpsc::UnboundedSender<UiEvent>,
    ) -> Result<Self> {
        let dir = store.home.join("run");
        std::fs::create_dir_all(&dir)?;
        let socket = dir.join(format!("{}.sock", &new_id()[..8]));
        let listener = UnixListener::bind(&socket)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
        }
        // Restored conversation IDs and durable grant records are audit context;
        // every new server starts without active capabilities.
        let active = ActiveGrants::default();
        let callers = active.clone();
        let cancel = CancellationToken::new();
        let shutdown = cancel.clone();
        let db = store.clone();
        tokio::spawn(async move {
            loop {
                let accepted =
                    tokio::select! { _=shutdown.cancelled()=>break, r=listener.accept()=>r };
                let Ok((stream, _)) = accepted else { break };
                let db = db.clone();
                let callers = callers.clone();
                let tx = events.clone();
                tokio::spawn(async move {
                    let _ = serve(stream, db, callers, tx).await;
                });
            }
        });
        Ok(Self {
            socket,
            store,
            session: session.id.clone(),
            active,
            cancel,
        })
    }

    /// Admit the next unspent explicit ordinal and return its secret only to the
    /// native adapter. Use `admit_reserved` when admission order is uncertain.
    /// Never serialize, log, or persist this return value.
    pub fn admit(
        &self,
        assignment: &mut AssignmentRecord,
        invocation: &InvocationRecord,
        operations: Vec<TeamOperation>,
    ) -> Result<String> {
        self.admit_inner(assignment, &mut invocation.clone(), operations, false)
    }

    /// Publish a live capability only after budget, ordinal and grant commit.
    pub fn admit_reserved(
        &self,
        assignment: &mut AssignmentRecord,
        invocation: &mut InvocationRecord,
        operations: Vec<TeamOperation>,
    ) -> Result<String> {
        self.admit_inner(assignment, invocation, operations, true)
    }

    fn admit_inner(
        &self,
        assignment: &mut AssignmentRecord,
        invocation: &mut InvocationRecord,
        operations: Vec<TeamOperation>,
        allocate_turn: bool,
    ) -> Result<String> {
        ensure!(
            assignment.session_id == self.session,
            "Assignment belongs to another team server"
        );
        ensure!(
            assignment.grant_ids.is_empty(),
            "Cannot restore or self-grant an existing capability"
        );
        let mut active = self
            .active
            .lock()
            .map_err(|_| anyhow::anyhow!("Grant lock poisoned"))?;
        let grant = GrantRecord::for_assignment(assignment, invocation, operations);
        let mut admitted = assignment.clone();
        admitted.grant_ids.push(grant.id.clone());
        if allocate_turn {
            *invocation = self.store.admit_invocation_with_grants(
                &admitted,
                invocation.clone(),
                std::slice::from_ref(&grant),
            )?;
        } else {
            self.store.begin_invocation_with_grants(
                &admitted,
                invocation,
                std::slice::from_ref(&grant),
            )?;
        }
        let token = new_id();
        active.insert(
            token.clone(),
            ActiveGrant {
                record: grant,
                requests: HashSet::new(),
            },
        );
        *assignment = admitted;
        Ok(token)
    }

    /// End process authority first, even if the terminal database write fails.
    pub fn finish(
        &self,
        invocation_id: &str,
        state: InvocationState,
        reason: Option<&str>,
    ) -> Result<()> {
        ensure!(
            state != InvocationState::Running,
            "Cannot finish a running invocation"
        );
        let mut active = self
            .active
            .lock()
            .map_err(|_| anyhow::anyhow!("Grant lock poisoned"))?;
        active.retain(|_, g| g.record.invocation_id != invocation_id);
        self.store
            .finish_invocation(&self.session, invocation_id, state, reason)
    }
}
impl Drop for TeamServer {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Ok(mut active) = self.active.lock() {
            for grant in active.values() {
                let _ = self.store.finish_invocation(
                    &self.session,
                    &grant.record.invocation_id,
                    InvocationState::Interrupted,
                    Some("Team server stopped"),
                );
            }
            active.clear();
        }
        let _ = std::fs::remove_file(&self.socket);
    }
}

async fn serve(
    stream: UnixStream,
    store: Store,
    callers: ActiveGrants,
    events: mpsc::UnboundedSender<UiEvent>,
) -> Result<()> {
    let (read, mut write) = stream.into_split();
    let mut input = BufReader::new(read.take(1024 * 1024 + 1));
    let mut line = String::new();
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        input.read_line(&mut line),
    )
    .await??;
    if line.len() > 1024 * 1024 {
        bail!("Request too large");
    }
    let req: Value = serde_json::from_str(&line)?;
    let response = (|| -> Result<Value> {
        let mut active = callers
            .lock()
            .map_err(|_| anyhow::anyhow!("Grant lock poisoned"))?;
        let caller = active
            .get_mut(req["token"].as_str().unwrap_or(""))
            .context("Invalid or expired team credential")?;
        let request_id = req["request_id"]
            .as_str()
            .context("Request identity required")?;
        ensure!(
            !request_id.is_empty() && request_id.len() <= 256,
            "Invalid request identity"
        );
        // An invocation has a bounded request ledger, including failed calls.
        // Replay rejection and the action share this lock with revocation.
        ensure!(
            caller.requests.len() < 10_000,
            "Assignment team request limit reached"
        );
        ensure!(
            caller.requests.insert(request_id.into()),
            "Replayed team request"
        );
        ensure!(
            req.as_object().is_some_and(|v| v.keys().all(|k| [
                "token",
                "request_id",
                "name",
                "arguments"
            ]
            .contains(&k.as_str()))),
            "Request cannot override runtime authority"
        );
        let operation: TeamOperation = serde_json::from_value(req["name"].clone())
            .context("Unknown or runtime-only team operation")?;
        let (result, message) =
            store.team_call(&caller.record, operation, &req["arguments"], request_id)?;
        if let Some(message) = message {
            let _ = events.send(UiEvent::Message(message));
        }
        Ok(result)
    })();
    let data = match response {
        Ok(v) => json!({"ok":true,"value":v}),
        Err(e) => json!({"ok":false,"error":e.to_string()}),
    };
    write.write_all(format!("{data}\n").as_bytes()).await?;
    Ok(())
}

pub fn tools() -> Value {
    json!([
        {"name":"team_post","description":"Post a concise finding or question to the shared team chat. All teammates can read it. Posting does not block for an answer.","inputSchema":{"type":"object","additionalProperties":false,"properties":{"text":{"type":"string"},"recipient":{"type":"string"}},"required":["text"]}},
        {"name":"team_read","description":"Read the shared team chat, including peer findings. Use after to read newer messages.","inputSchema":{"type":"object","additionalProperties":false,"properties":{"after":{"type":"integer"},"limit":{"type":"integer"}}}},
        {"name":"tasks_list","description":"Inspect tasks, assignments, dependencies and outcomes.","inputSchema":{"type":"object","additionalProperties":false,"properties":{}}},
        {"name":"task_propose","description":"Suggest a new task or change in approach for team consideration.","inputSchema":{"type":"object","additionalProperties":false,"properties":{"title":{"type":"string"},"description":{"type":"string"}},"required":["title","description"]}},
        {"name":"memory_search","description":"Find verified project knowledge and shared procedures.","inputSchema":{"type":"object","additionalProperties":false,"properties":{"query":{"type":"string"}},"required":["query"]}},
        {"name":"memory_propose","description":"Propose a reusable lesson; it becomes active only after independent review.","inputSchema":{"type":"object","additionalProperties":false,"properties":{"title":{"type":"string"},"content":{"type":"string"}},"required":["title","content"]}}
    ])
}

/// A small stdio MCP server, launched by providers. Team identity comes from the
/// environment and is checked by the live engine, never from tool arguments.
pub async fn stdio_bridge(socket: &Path) -> Result<()> {
    let token = std::env::var("YMP_MCP_TOKEN").context("Missing team credential")?;
    let bridge_id = new_id();
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines.next_line().await? {
        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Some(id) = req.get("id") else { continue };
        let result = match req["method"].as_str().unwrap_or("") {
            "initialize" => {
                json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"ymp-team","version":"0.1.0"}})
            }
            "ping" => json!({}),
            "tools/list" => json!({"tools":tools()}),
            "tools/call" => {
                let result=async{
                    let mut stream=UnixStream::connect(socket).await?;
                    stream.write_all(format!("{}\n",json!({"token":token,"request_id":format!("{bridge_id}:{id}"),"name":req["params"]["name"],"arguments":req["params"].get("arguments").cloned().unwrap_or_else(||json!({}))})).as_bytes()).await?;
                    let mut line=String::new();BufReader::new(stream).read_line(&mut line).await?;
                    anyhow::Ok(serde_json::from_str::<Value>(&line)?)
                }.await;
                match result {
                    Ok(v) if v["ok"] == true => {
                        json!({"content":[{"type":"text","text":v["value"].to_string()}]})
                    }
                    Ok(v) => {
                        json!({"isError":true,"content":[{"type":"text","text":v["error"].as_str().unwrap_or("Team tool failed")}]})
                    }
                    Err(e) => {
                        json!({"isError":true,"content":[{"type":"text","text":e.to_string()}]})
                    }
                }
            }
            _ => {
                stdout.write_all(format!("{}\n",json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Method not found"}})).as_bytes()).await?;
                continue;
            }
        };
        stdout
            .write_all(format!("{}\n", json!({"jsonrpc":"2.0","id":id,"result":result})).as_bytes())
            .await?;
        stdout.flush().await?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
