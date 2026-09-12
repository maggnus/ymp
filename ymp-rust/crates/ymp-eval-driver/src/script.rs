use crate::{file_digest, write_json};
use anyhow::{bail, ensure, Result};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    sync::mpsc,
};
use ymp_core::*;
use ymp_providers::{ExecutionBackend, ExecutionFuture, ProviderEvent, TurnRequest, TurnResult};

#[derive(Default)]
pub struct NativeJournal {
    rows: Mutex<Vec<Value>>,
    active: AtomicUsize,
    peak: AtomicUsize,
}
impl NativeJournal {
    pub fn push(&self, mut value: Value) {
        let mut rows = self.rows.lock().unwrap();
        value["seq"] = json!(rows.len() + 1);
        value["observed_at"] = json!(now());
        rows.push(value);
    }
    pub fn save(&self, directory: &Path) -> Result<()> {
        write_json(&directory.join("native.json"), &*self.rows.lock().unwrap())
    }
    pub fn snapshot(&self) -> Vec<Value> {
        self.rows.lock().unwrap().clone()
    }
    pub fn peak(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }
}
pub struct ScriptedBackend {
    pub case: String,
    pub task_title: String,
    pub task_prompt: String,
    pub output: String,
    pub writer: PathBuf,
    pub journal: Arc<NativeJournal>,
}
struct InvocationGuard<'a> {
    journal: &'a NativeJournal,
    id: String,
    finished: bool,
}
impl Drop for InvocationGuard<'_> {
    fn drop(&mut self) {
        self.journal.active.fetch_sub(1, Ordering::SeqCst);
        self.journal.push(json!({"type":"native_closed","native_id":self.id,"outcome":if self.finished {"completed"} else {"interrupted"}}));
    }
}
pub async fn team_call(request: &TurnRequest, name: &str, arguments: Value) -> Result<Value> {
    let endpoint = request
        .mcp
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Missing assignment team capability"))?;
    let socket = endpoint
        .args
        .last()
        .ok_or_else(|| anyhow::anyhow!("Missing team socket"))?;
    let mut stream = UnixStream::connect(socket).await?;
    let message =
        json!({"token":endpoint.token,"request_id":new_id(),"name":name,"arguments":arguments});
    stream.write_all(format!("{message}\n").as_bytes()).await?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).await?;
    Ok(serde_json::from_str(&line)?)
}
impl ExecutionBackend for ScriptedBackend {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "ymp.evals.scripted".into(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }
    fn workspace_access(&self, request: &TurnRequest) -> WorkspaceAccess {
        if request.read_only {
            WorkspaceAccess::ReadAll
        } else {
            WorkspaceAccess::WriteAll
        }
    }
    fn execute(
        &self,
        request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            let id = new_id();
            let active = self.journal.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.journal.peak.fetch_max(active, Ordering::SeqCst);
            let mut guard = InvocationGuard {
                journal: &self.journal,
                id: id.clone(),
                finished: false,
            };
            self.journal.push(json!({"type":"native_started","native_id":id,"agent_id":request.profile.id,"purpose":request.purpose,"settings":request.settings,"working_directory":request.cwd,"prompt_sha256":content_digest(&request.prompt),"prompt":request.prompt,"controls":request.resource_controls}));
            let observation = InvocationObservation {
                sent: Some(request.settings.clone()),
                reported: Some(request.settings.clone()),
                native_session_id: Some(id.clone()),
                native_turn_id: Some(id.clone()),
                native_version: Some("scripted-runtime-v1".into()),
                ..Default::default()
            };
            events.send(ProviderEvent::Execution(Box::new(observation.clone())))?;
            self.journal.push(
                json!({"type":"native_observation","native_id":id,"observation":observation}),
            );
            let text = match request.purpose.as_str() {
                "plan" => json!({"summary":"One explicit artifact and independent review","tasks":[{"title":self.task_title,"description":self.task_prompt,"competence":"implementation","difficulty":"simple","access":"write","dependencies":[],"checks":[]}]}).to_string(),
                "review_plan" => json!({"approved":true,"reason":"The plan preserves the requested artifact and requires independent acceptance"}).to_string(),
                "execute" => {
                    let reply = team_call(&request,"team_post",json!({"text":"Producing the assigned artifact from the selected input files."})).await?;
                    self.journal.push(json!({"type":"team_response","native_id":id,"operation":"team_post","response":reply}));
                    ensure!(reply["ok"]==true, "Production team post was denied: {reply}");
                    let output = tokio::process::Command::new("python3").arg(&self.writer).arg(&self.case).arg(&request.cwd).output().await?;
                    self.journal.push(json!({"type":"script_process","native_id":id,"script_sha256":file_digest(&self.writer)?,"exit_code":output.status.code(),"stdout":String::from_utf8_lossy(&output.stdout),"stderr":String::from_utf8_lossy(&output.stderr)}));
                    ensure!(output.status.success(), "Scripted production failed: {}", String::from_utf8_lossy(&output.stderr));
                    format!("Created {} from the supplied task inputs; ready for independent review.",self.output)
                }
                "review" | "final_review" => {
                    let bytes = std::fs::read(request.cwd.join(&self.output))?;
                    let acceptable = !bytes.is_empty();
                    self.journal.push(json!({"type":"scripted_review_inspection","native_id":id,"path":self.output,"sha256":bytes_digest(&bytes),"bytes":bytes,"independent_agent_id":request.profile.id}));
                    json!({"approved":acceptable,"reason":if self.case=="qualitative" {"The two proposed activities give attendees a concrete, voluntary way to participate and are welcoming and usable; this is an independent scripted qualitative assessment, not external confirmation"} else {"Inspected the actual artifact and the runtime's observed input-scoped check; the requested result is present"}}).to_string()
                }
                "synthesis" => format!("The accepted result is available at {}",request.cwd.join(&self.output).display()),
                "learn" => json!({"useful":false}).to_string(),
                purpose => bail!("No scripted adapter for native purpose {purpose}"),
            };
            let usage = UsageSnapshot {
                counts: TokenCounts {
                    input: Some(3),
                    output: Some(4),
                    ..TokenCounts::zero()
                },
                finalized: true,
                partial: false,
                note: Some("Controlled synthetic request accounting".into()),
                native_total: None,
            };
            self.journal
                .push(json!({"type":"native_usage","native_id":id,"usage":usage}));
            events.send(ProviderEvent::Usage(usage))?;
            guard.finished = true;
            Ok(TurnResult {
                text,
                session_id: id,
                usage: None,
            })
        })
    }
}
