use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
use ymp_core::{new_id, now, MemoryEntry, UiEvent};
use ymp_storage::Store;

#[derive(Clone)]
pub struct Caller {
    pub agent: String,
    pub session: String,
    pub project: String,
}
pub struct TeamServer {
    pub socket: PathBuf,
    pub tokens: HashMap<String, String>,
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
        let mut callers = HashMap::new();
        let mut tokens = HashMap::new();
        for agent in &session.team {
            let token = new_id();
            tokens.insert(agent.id.clone(), token.clone());
            callers.insert(
                token,
                Caller {
                    agent: agent.id.clone(),
                    session: session.id.clone(),
                    project: session.project_id.clone(),
                },
            );
        }
        let callers = Arc::new(callers);
        let cancel = CancellationToken::new();
        let shutdown = cancel.clone();
        tokio::spawn(async move {
            loop {
                let accepted =
                    tokio::select! {_=shutdown.cancelled()=>break,r=listener.accept()=>r};
                let Ok((stream, _)) = accepted else { break };
                let db = store.clone();
                let callers = callers.clone();
                let tx = events.clone();
                tokio::spawn(async move {
                    let _ = serve(stream, db, callers, tx).await;
                });
            }
        });
        Ok(Self {
            socket,
            tokens,
            cancel,
        })
    }
}
impl Drop for TeamServer {
    fn drop(&mut self) {
        self.cancel.cancel();
        let _ = std::fs::remove_file(&self.socket);
    }
}

async fn serve(
    stream: UnixStream,
    store: Store,
    callers: Arc<HashMap<String, Caller>>,
    events: mpsc::UnboundedSender<UiEvent>,
) -> Result<()> {
    let (read, mut write) = stream.into_split();
    let mut input = BufReader::new(read);
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
        let caller = callers
            .get(req["token"].as_str().unwrap_or(""))
            .context("Invalid team credential")?;
        let a = &req["arguments"];
        match req["name"].as_str().unwrap_or("") {
            "team_post" => {
                let text = a["text"].as_str().context("text is required")?;
                if text.len() > 32_000 {
                    bail!("Message exceeds 32000 bytes");
                }
                let recipient = a["recipient"].as_str();
                if recipient.is_some_and(|id| !callers.values().any(|c| c.agent == id)) {
                    bail!("Unknown recipient");
                }
                let msg = store.message(&caller.session, &caller.agent, recipient, "chat", text)?;
                let _ = events.send(UiEvent::Message(msg.clone()));
                Ok(json!({"seq":msg.seq}))
            }
            "team_read" => Ok(serde_json::to_value(store.messages(
                &caller.session,
                a["after"].as_i64().unwrap_or(0),
                a["limit"].as_u64().unwrap_or(30).min(100) as usize,
            )?)?),
            "tasks_list" => Ok(serde_json::to_value(store.tasks(&caller.session)?)?),
            "memory_search" => Ok(serde_json::to_value(
                store.memory(Some(&caller.project), a["query"].as_str().unwrap_or(""))?,
            )?),
            "memory_propose" => {
                let m = MemoryEntry {
                    id: new_id(),
                    project_id: Some(caller.project.clone()),
                    kind: "procedure".into(),
                    title: a["title"].as_str().context("title required")?.into(),
                    content: a["content"].as_str().context("content required")?.into(),
                    source_session: caller.session.clone(),
                    author: caller.agent.clone(),
                    reviewer: None,
                    status: "proposed".into(),
                    created_at: now(),
                    supersedes: None,
                };
                store.save_memory(&m)?;
                Ok(json!({"id":m.id,"status":"proposed"}))
            }
            "task_propose" => {
                let text = serde_json::to_string(a)?;
                let msg = store.message(&caller.session, &caller.agent, None, "proposal", &text)?;
                let _ = events.send(UiEvent::Message(msg));
                Ok(
                    json!({"status":"proposed","note":"Proposal recorded for the next planning boundary"}),
                )
            }
            _ => bail!("Unknown team tool"),
        }
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
        {"name":"team_post","description":"Post a concise finding or question to the shared team chat. All teammates can read it. Posting does not block for an answer.","inputSchema":{"type":"object","properties":{"text":{"type":"string"},"recipient":{"type":"string"}},"required":["text"]}},
        {"name":"team_read","description":"Read the shared team chat, including peer findings. Use after to read newer messages.","inputSchema":{"type":"object","properties":{"after":{"type":"integer"},"limit":{"type":"integer"}}}},
        {"name":"tasks_list","description":"Inspect tasks, assignments, dependencies and outcomes.","inputSchema":{"type":"object","properties":{}}},
        {"name":"task_propose","description":"Suggest a new task or change in approach for team consideration.","inputSchema":{"type":"object","properties":{"title":{"type":"string"},"description":{"type":"string"}},"required":["title","description"]}},
        {"name":"memory_search","description":"Find verified project knowledge and shared procedures.","inputSchema":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"]}},
        {"name":"memory_propose","description":"Propose a reusable lesson; it becomes active only after independent review.","inputSchema":{"type":"object","properties":{"title":{"type":"string"},"content":{"type":"string"}},"required":["title","content"]}}
    ])
}

/// A small stdio MCP server, launched by providers. Team identity comes from the
/// environment and is checked by the live engine, never from tool arguments.
pub async fn stdio_bridge(socket: &Path) -> Result<()> {
    let token = std::env::var("YMP_MCP_TOKEN").context("Missing team credential")?;
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
                    stream.write_all(format!("{}\n",json!({"token":token,"name":req["params"]["name"],"arguments":req["params"]["arguments"]})).as_bytes()).await?;
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
mod tests {
    use super::*;
    use ymp_core::{Config, Session};

    #[tokio::test]
    async fn caller_identity_cannot_be_forged_and_invalid_tokens_cannot_post() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(&dir.path().join("state")).unwrap();
        let project = store.project(dir.path()).unwrap();
        let profile = Config::default().agents.remove(0);
        let session = Session {
            id: new_id(),
            project_id: project.id,
            title: "test".into(),
            status: "running".into(),
            created_at: now(),
            team: vec![profile.clone()],
            turns_used: 0,
        };
        store.save_session(&session).unwrap();
        let (tx, _rx) = mpsc::unbounded_channel();
        let server = TeamServer::start(store.clone(), &session, tx)
            .await
            .unwrap();
        for token in ["invalid", server.tokens[&profile.id].as_str()] {
            let mut stream = UnixStream::connect(&server.socket).await.unwrap();
            let request = json!({"token":token,"name":"team_post","arguments":{"text":"test","author":"forged","session":"other"}});
            stream
                .write_all(format!("{request}\n").as_bytes())
                .await
                .unwrap();
            let mut line = String::new();
            BufReader::new(stream).read_line(&mut line).await.unwrap();
            let result: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(result["ok"], token != "invalid");
        }
        let messages = store.messages(&session.id, 0, 10).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].author, profile.id);
        assert_eq!(messages[0].session_id, session.id);
    }
}
