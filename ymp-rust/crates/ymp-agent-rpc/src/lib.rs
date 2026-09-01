#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use thiserror::Error;
use ymp_agent_api::{AgentToolCall, AgentToolError, AgentToolHandler};
use ymp_application::{Application, WorkspaceSubmission};
use ymp_domain::digest_bytes;

const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_COMMAND_ID_CHARS: usize = 128;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct YieldConfirmation {
    pub sequence: u64,
    pub command_id: String,
    pub attempt_id: String,
    pub invocation_id: String,
    pub digest: String,
}

#[derive(Clone, Debug, Default)]
pub struct InvocationControl {
    state: Arc<Mutex<InvocationControlState>>,
}

#[derive(Debug, Default)]
struct InvocationControlState {
    confirmations: Vec<YieldConfirmation>,
}

impl InvocationControl {
    pub fn confirmations_after(
        &self,
        cursor: u64,
    ) -> Result<Vec<YieldConfirmation>, AgentToolError> {
        let state = self
            .state
            .lock()
            .map_err(|_| AgentToolError::internal("invocation control lock is unavailable"))?;
        Ok(state
            .confirmations
            .iter()
            .filter(|confirmation| confirmation.sequence > cursor)
            .cloned()
            .collect())
    }

    fn authorize_yield(
        &self,
        command_id: String,
        attempt_id: &str,
        invocation_id: &str,
    ) -> Result<Value, AgentToolError> {
        let command_chars = command_id.chars().count();
        if !(1..=MAX_COMMAND_ID_CHARS).contains(&command_chars) {
            return Err(AgentToolError::invalid(
                "yield command_id must contain between 1 and 128 characters",
            ));
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| AgentToolError::internal("invocation control lock is unavailable"))?;
        if let Some(confirmation) = state
            .confirmations
            .iter()
            .find(|confirmation| confirmation.command_id == command_id)
        {
            return serde_json::to_value(confirmation)
                .map_err(|_| AgentToolError::internal("yield confirmation serialization failed"));
        }
        let sequence = u64::try_from(state.confirmations.len())
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| AgentToolError::internal("yield confirmation sequence exhausted"))?;
        let digest = digest_bytes(
            &serde_json::to_vec(&(sequence, &command_id, attempt_id, invocation_id))
                .map_err(|_| AgentToolError::internal("yield confirmation serialization failed"))?,
        );
        let confirmation = YieldConfirmation {
            sequence,
            command_id,
            attempt_id: attempt_id.to_owned(),
            invocation_id: invocation_id.to_owned(),
            digest,
        };
        let value = serde_json::to_value(&confirmation)
            .map_err(|_| AgentToolError::internal("yield confirmation serialization failed"))?;
        state.confirmations.push(confirmation);
        Ok(value)
    }
}

#[derive(Debug, Error)]
pub enum AgentRpcError {
    #[error("agent RPC I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("agent RPC JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("agent RPC payload exceeded its {0}-byte limit")]
    PayloadTooLarge(usize),
    #[error("agent RPC is unavailable on this platform")]
    UnsupportedPlatform,
}

#[derive(Debug, Deserialize, Serialize)]
struct RpcRequest {
    token: String,
    attempt_id: String,
    #[serde(default)]
    invocation_id: Option<String>,
    call: AgentToolCall,
}

#[derive(Debug, Deserialize, Serialize)]
struct RpcResponse {
    result: Option<Value>,
    error: Option<AgentToolError>,
}

impl RpcResponse {
    fn from_result(result: Result<Value, AgentToolError>) -> Self {
        match result {
            Ok(result) => Self {
                result: Some(result),
                error: None,
            },
            Err(error) => Self {
                result: None,
                error: Some(error),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct SocketToolHandler {
    socket_path: PathBuf,
    token: String,
    attempt_id: String,
    invocation_id: Option<String>,
}

impl SocketToolHandler {
    pub fn new(
        socket_path: impl Into<PathBuf>,
        token: impl Into<String>,
        attempt_id: impl Into<String>,
    ) -> Self {
        Self {
            socket_path: socket_path.into(),
            token: token.into(),
            attempt_id: attempt_id.into(),
            invocation_id: None,
        }
    }

    pub fn for_invocation(
        socket_path: impl Into<PathBuf>,
        token: impl Into<String>,
        attempt_id: impl Into<String>,
        invocation_id: impl Into<String>,
    ) -> Self {
        Self {
            socket_path: socket_path.into(),
            token: token.into(),
            attempt_id: attempt_id.into(),
            invocation_id: Some(invocation_id.into()),
        }
    }

    pub fn from_env() -> Result<Self, AgentRpcError> {
        let socket_path = std::env::var_os("YMP_AGENT_SOCKET")
            .map(PathBuf::from)
            .ok_or(AgentRpcError::UnsupportedPlatform)?;
        let token =
            std::env::var("YMP_AGENT_TOKEN").map_err(|_| AgentRpcError::UnsupportedPlatform)?;
        let attempt_id =
            std::env::var("YMP_ATTEMPT_ID").map_err(|_| AgentRpcError::UnsupportedPlatform)?;
        let invocation_id =
            std::env::var("YMP_INVOCATION_ID").map_err(|_| AgentRpcError::UnsupportedPlatform)?;
        Ok(Self::for_invocation(
            socket_path,
            token,
            attempt_id,
            invocation_id,
        ))
    }
}

#[cfg(unix)]
impl AgentToolHandler for SocketToolHandler {
    fn call(&mut self, call: AgentToolCall) -> Result<Value, AgentToolError> {
        use std::net::Shutdown;
        use std::os::unix::net::UnixStream;

        let request = RpcRequest {
            token: self.token.clone(),
            attempt_id: self.attempt_id.clone(),
            invocation_id: self.invocation_id.clone(),
            call,
        };
        let encoded = serde_json::to_vec(&request)
            .map_err(|_| AgentToolError::internal("agent RPC request serialization failed"))?;
        if encoded.len() > MAX_REQUEST_BYTES {
            return Err(AgentToolError::invalid("agent RPC request is too large"));
        }
        let mut stream = UnixStream::connect(&self.socket_path)
            .map_err(|_| AgentToolError::internal("agent RPC controller is unavailable"))?;
        stream
            .write_all(&encoded)
            .and_then(|_| stream.shutdown(Shutdown::Write))
            .map_err(|_| AgentToolError::internal("agent RPC request write failed"))?;
        let bytes = read_bounded(&mut stream, MAX_RESPONSE_BYTES)
            .map_err(|_| AgentToolError::internal("agent RPC response read failed"))?;
        let response: RpcResponse = serde_json::from_slice(&bytes)
            .map_err(|_| AgentToolError::internal("agent RPC response is malformed"))?;
        match (response.result, response.error) {
            (Some(result), None) => Ok(result),
            (None, Some(error)) => Err(error),
            _ => Err(AgentToolError::internal(
                "agent RPC response has an invalid result shape",
            )),
        }
    }
}

#[cfg(not(unix))]
impl AgentToolHandler for SocketToolHandler {
    fn call(&mut self, _call: AgentToolCall) -> Result<Value, AgentToolError> {
        Err(AgentToolError::internal(
            "agent RPC is unavailable on this platform",
        ))
    }
}

pub struct AgentRpcServer {
    socket_path: PathBuf,
    shutdown: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    invocation_control: Option<InvocationControl>,
}

impl AgentRpcServer {
    #[cfg(unix)]
    pub fn start(
        socket_path: impl Into<PathBuf>,
        token: impl Into<String>,
        attempt_id: impl Into<String>,
        application: Arc<Mutex<Application>>,
    ) -> Result<Self, AgentRpcError> {
        Self::start_inner(socket_path, token, attempt_id, None, application, None)
    }

    #[cfg(unix)]
    pub fn start_with_submission(
        socket_path: impl Into<PathBuf>,
        token: impl Into<String>,
        attempt_id: impl Into<String>,
        application: Arc<Mutex<Application>>,
        submission: WorkspaceSubmission,
    ) -> Result<Self, AgentRpcError> {
        Self::start_inner(
            socket_path,
            token,
            attempt_id,
            None,
            application,
            Some(submission),
        )
    }

    #[cfg(unix)]
    pub fn start_with_submission_for_invocation(
        socket_path: impl Into<PathBuf>,
        token: impl Into<String>,
        attempt_id: impl Into<String>,
        invocation_id: impl Into<String>,
        application: Arc<Mutex<Application>>,
        submission: WorkspaceSubmission,
    ) -> Result<Self, AgentRpcError> {
        Self::start_inner(
            socket_path,
            token,
            attempt_id,
            Some(invocation_id.into()),
            application,
            Some(submission),
        )
    }

    #[cfg(unix)]
    fn start_inner(
        socket_path: impl Into<PathBuf>,
        token: impl Into<String>,
        attempt_id: impl Into<String>,
        invocation_id: Option<String>,
        application: Arc<Mutex<Application>>,
        submission: Option<WorkspaceSubmission>,
    ) -> Result<Self, AgentRpcError> {
        use std::os::unix::fs::PermissionsExt;
        use std::os::unix::net::UnixListener;

        let socket_path = socket_path.into();
        if socket_path.exists() {
            return Err(AgentRpcError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "refusing to replace an existing agent RPC socket",
            )));
        }
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent)?;
            let mut permissions = std::fs::metadata(parent)?.permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(parent, permissions)?;
        }
        let listener = UnixListener::bind(&socket_path)?;
        let mut permissions = std::fs::metadata(&socket_path)?.permissions();
        permissions.set_mode(0o600);
        std::fs::set_permissions(&socket_path, permissions)?;
        listener.set_nonblocking(true)?;
        let token = token.into();
        let attempt_id = attempt_id.into();
        let invocation_control = invocation_id.as_ref().map(|_| InvocationControl::default());
        let thread_control = invocation_control.clone();
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_shutdown = Arc::clone(&shutdown);
        let thread = thread::spawn(move || {
            while !thread_shutdown.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let response = handle_connection(
                            &mut stream,
                            &token,
                            &attempt_id,
                            invocation_id.as_deref(),
                            thread_control.as_ref(),
                            &application,
                            submission.clone(),
                        );
                        let response = RpcResponse::from_result(response);
                        if let Ok(encoded) = serde_json::to_vec(&response) {
                            let _ = stream.write_all(&encoded);
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => return,
                }
            }
        });
        Ok(Self {
            socket_path,
            shutdown,
            thread: Some(thread),
            invocation_control,
        })
    }

    #[cfg(not(unix))]
    pub fn start(
        _socket_path: impl Into<PathBuf>,
        _token: impl Into<String>,
        _attempt_id: impl Into<String>,
        _application: Arc<Mutex<Application>>,
    ) -> Result<Self, AgentRpcError> {
        Err(AgentRpcError::UnsupportedPlatform)
    }

    #[cfg(not(unix))]
    pub fn start_with_submission(
        _socket_path: impl Into<PathBuf>,
        _token: impl Into<String>,
        _attempt_id: impl Into<String>,
        _application: Arc<Mutex<Application>>,
        _submission: WorkspaceSubmission,
    ) -> Result<Self, AgentRpcError> {
        Err(AgentRpcError::UnsupportedPlatform)
    }

    #[cfg(not(unix))]
    pub fn start_with_submission_for_invocation(
        _socket_path: impl Into<PathBuf>,
        _token: impl Into<String>,
        _attempt_id: impl Into<String>,
        _invocation_id: impl Into<String>,
        _application: Arc<Mutex<Application>>,
        _submission: WorkspaceSubmission,
    ) -> Result<Self, AgentRpcError> {
        Err(AgentRpcError::UnsupportedPlatform)
    }

    pub fn invocation_control(&self) -> Option<InvocationControl> {
        self.invocation_control.clone()
    }
}

impl Drop for AgentRpcServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        #[cfg(unix)]
        {
            let _ = std::os::unix::net::UnixStream::connect(&self.socket_path);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

#[cfg(unix)]
fn handle_connection(
    stream: &mut std::os::unix::net::UnixStream,
    expected_token: &str,
    expected_attempt: &str,
    expected_invocation: Option<&str>,
    invocation_control: Option<&InvocationControl>,
    application: &Arc<Mutex<Application>>,
    submission: Option<WorkspaceSubmission>,
) -> Result<Value, AgentToolError> {
    let bytes = read_bounded(stream, MAX_REQUEST_BYTES)
        .map_err(|_| AgentToolError::invalid("agent RPC request is malformed or too large"))?;
    let request: RpcRequest = serde_json::from_slice(&bytes)
        .map_err(|_| AgentToolError::invalid("agent RPC request is malformed"))?;
    if request.token != expected_token || request.attempt_id != expected_attempt {
        return Err(AgentToolError::rejected("agent RPC capability is invalid"));
    }
    if let Some(expected_invocation) = expected_invocation
        && request.invocation_id.as_deref() != Some(expected_invocation)
    {
        return Err(AgentToolError::rejected(
            "agent RPC invocation capability is invalid",
        ));
    }
    let call = match request.call {
        AgentToolCall::Yield(arguments) => {
            let invocation_id = expected_invocation.ok_or_else(|| {
                AgentToolError::rejected("this endpoint is not bound to a managed invocation")
            })?;
            let control = invocation_control.ok_or_else(|| {
                AgentToolError::internal("managed invocation control is unavailable")
            })?;
            return control.authorize_yield(arguments.command_id, expected_attempt, invocation_id);
        }
        call => call,
    };
    let mut application = application
        .lock()
        .map_err(|_| AgentToolError::internal("controller state lock is unavailable"))?;
    match submission {
        Some(submission) => application
            .workspace_agent_session(expected_attempt, submission)
            .call(call),
        None => application.agent_session(expected_attempt).call(call),
    }
}

fn read_bounded(reader: &mut impl Read, limit: usize) -> Result<Vec<u8>, AgentRpcError> {
    let mut bytes = Vec::new();
    reader
        .take((limit.saturating_add(1)) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err(AgentRpcError::PayloadTooLarge(limit));
    }
    Ok(bytes)
}

#[cfg(unix)]
pub fn socket_path_is_private(path: &Path) -> Result<bool, AgentRpcError> {
    use std::os::unix::fs::PermissionsExt;
    let metadata = std::fs::metadata(path)?;
    Ok(metadata.permissions().mode() & 0o077 == 0)
}

#[cfg(not(unix))]
pub fn socket_path_is_private(_path: &Path) -> Result<bool, AgentRpcError> {
    Err(AgentRpcError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::{AgentRpcServer, SocketToolHandler, socket_path_is_private};
    use serde_json::{Value, json};
    use std::fs;
    use std::sync::{Arc, Mutex};
    use ymp_agent_api::{
        AgentToolCall, AgentToolErrorCode, AgentToolHandler, SubmitArguments, YieldArguments,
    };
    use ymp_application::{Application, WorkspaceSubmission};
    use ymp_domain::{Budget, Command};

    fn mcp_request(
        server: &mut ymp_agent_mcp::McpServer<SocketToolHandler>,
        request: Value,
    ) -> Value {
        serde_json::from_str(
            &server
                .handle_line(&request.to_string())
                .expect("MCP request has a response"),
        )
        .expect("parse MCP response")
    }

    #[test]
    fn mcp_rpc_application_chain_derives_collaboration_metadata() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let app = Arc::new(Mutex::new(
            Application::create(
                temporary.path().join("data"),
                "run-collaboration",
                Budget::new(1, 1),
            )
            .expect("create application"),
        ));
        app.lock()
            .expect("application lock")
            .execute(
                "start-root",
                Command::StartAttempt {
                    attempt_id: "participant-root".to_owned(),
                },
            )
            .expect("start controller-bound participant");
        let socket = temporary.path().join("runtime").join("collaboration.sock");
        let _server = AgentRpcServer::start(
            &socket,
            "secret-token",
            "participant-root",
            Arc::clone(&app),
        )
        .expect("start RPC server");
        let mut mcp = ymp_agent_mcp::McpServer::new(SocketToolHandler::new(
            &socket,
            "secret-token",
            "participant-root",
        ));
        mcp_request(
            &mut mcp,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }),
        );
        let content = "published through MCP and RPC";
        let unknown_field = mcp_request(
            &mut mcp,
            json!({
                "jsonrpc": "2.0",
                "id": "unknown-field",
                "method": "tools/call",
                "params": {
                    "name": "publish",
                    "arguments": {
                        "command_id": "forged-message",
                        "author": "model-selected-author",
                        "audience": { "audience": "project_discovery" },
                        "kind": "observation",
                        "content": "must not publish",
                        "salience_ms": 5_000,
                        "references": [],
                        "relation": { "relation": "standalone" },
                        "claimed_decision_basis": []
                    }
                }
            }),
        );
        assert_eq!(unknown_field["error"]["code"], -32602);
        assert_eq!(
            app.lock()
                .expect("application lock")
                .board_observation()
                .audit_messages,
            0
        );
        let published = mcp_request(
            &mut mcp,
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {
                    "name": "publish",
                    "arguments": {
                        "command_id": "message-through-production-chain",
                        "audience": { "audience": "project_discovery" },
                        "kind": "observation",
                        "content": content,
                        "salience_ms": 5_000,
                        "references": [],
                        "relation": { "relation": "standalone" },
                        "claimed_decision_basis": []
                    }
                }
            }),
        );
        assert_eq!(published["result"]["isError"], false);

        let read = mcp_request(
            &mut mcp,
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {
                    "name": "read_board",
                    "arguments": { "limit_bytes": 32_768 }
                }
            }),
        );
        let result = &read["result"]["structuredContent"];
        assert_eq!(result["reader"], "participant-root");
        assert_eq!(result["messages"][0]["content"], content);
        assert_eq!(
            result["messages"][0]["message"]["author"],
            "participant-root"
        );
        assert_eq!(
            result["messages"][0]["message"]["payload_bytes"],
            content.len() as u64
        );
        assert_eq!(
            result["messages"][0]["message"]["payload_digest"],
            ymp_domain::digest_bytes(content.as_bytes())
        );
        let projection = app
            .lock()
            .expect("application lock")
            .operator_board_projection()
            .expect("resolve application board");
        assert_eq!(projection.messages.len(), 1);
        assert_eq!(projection.messages[0].message.author, "participant-root");
        assert_eq!(projection.messages[0].payload, content.as_bytes());
    }

    #[test]
    fn socket_capability_binds_calls_to_one_attempt() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let app = Arc::new(Mutex::new(
            Application::create(temporary.path().join("data"), "run-1", Budget::new(1, 1))
                .expect("create application"),
        ));
        let source = temporary.path().join("source");
        let workspace = temporary.path().join("workspace");
        fs::create_dir(&source).expect("source directory");
        fs::write(source.join("result.txt"), b"before\n").expect("source file");
        let base = {
            let mut application = app.lock().expect("application lock");
            let base = application
                .artifact_store()
                .capture_source(&source)
                .expect("capture base");
            application
                .artifact_store()
                .materialize(&base.manifest_digest, &workspace)
                .expect("materialize workspace");
            application
                .execute(
                    "start",
                    Command::StartAttempt {
                        attempt_id: "attempt-1".to_owned(),
                    },
                )
                .expect("start attempt");
            base
        };
        fs::write(workspace.join("result.txt"), b"after\n").expect("candidate edit");
        let socket = temporary.path().join("runtime").join("agent.sock");
        let _server = AgentRpcServer::start_with_submission(
            &socket,
            "secret-token",
            "attempt-1",
            Arc::clone(&app),
            WorkspaceSubmission::new(&base.manifest_digest, &workspace, Vec::new()),
        )
        .expect("start RPC server");
        assert!(socket_path_is_private(&socket).expect("socket permissions"));

        let mut client = SocketToolHandler::new(&socket, "secret-token", "attempt-1");
        let state = client.call(AgentToolCall::ReadControl).expect("read state");
        assert_eq!(state["run_id"], "run-1");
        let submitted = client
            .call(AgentToolCall::Submit(SubmitArguments {
                command_id: "agent.submit".to_owned(),
            }))
            .expect("submit bound workspace");
        let submitted_digest = submitted["candidate"]["snapshot_digest"]
            .as_str()
            .expect("submitted candidate digest");
        assert_eq!(
            Some(submitted_digest),
            app.lock()
                .expect("application lock")
                .state()
                .candidate_digest
                .as_deref()
        );

        let mut wrong = SocketToolHandler::new(&socket, "wrong-token", "attempt-1");
        let error = wrong
            .call(AgentToolCall::ReadControl)
            .expect_err("reject wrong token");
        assert_eq!(error.code, AgentToolErrorCode::Rejected);
    }

    #[test]
    fn invocation_bound_yield_replays_one_authoritative_confirmation() {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let app = Arc::new(Mutex::new(
            Application::create(
                temporary.path().join("data"),
                "run-yield",
                Budget::new(1, 1),
            )
            .expect("create application"),
        ));
        let source = temporary.path().join("source");
        let workspace = temporary.path().join("workspace");
        fs::create_dir(&source).expect("source directory");
        fs::write(source.join("result.txt"), b"before\n").expect("source file");
        let base = {
            let mut application = app.lock().expect("application lock");
            let base = application
                .artifact_store()
                .capture_source(&source)
                .expect("capture base");
            application
                .artifact_store()
                .materialize(&base.manifest_digest, &workspace)
                .expect("materialize workspace");
            application
                .execute(
                    "start",
                    Command::StartAttempt {
                        attempt_id: "attempt-yield".to_owned(),
                    },
                )
                .expect("start attempt");
            base
        };
        let socket = temporary.path().join("runtime").join("yield.sock");
        let server = AgentRpcServer::start_with_submission_for_invocation(
            &socket,
            "secret-token",
            "attempt-yield",
            "invocation-yield",
            Arc::clone(&app),
            WorkspaceSubmission::new(&base.manifest_digest, &workspace, Vec::new()),
        )
        .expect("start invocation-bound RPC server");
        let mut client = SocketToolHandler::for_invocation(
            &socket,
            "secret-token",
            "attempt-yield",
            "invocation-yield",
        );
        let first = client
            .call(AgentToolCall::Yield(YieldArguments {
                command_id: "yield-1".to_owned(),
            }))
            .expect("authorize yield");
        let replay = client
            .call(AgentToolCall::Yield(YieldArguments {
                command_id: "yield-1".to_owned(),
            }))
            .expect("replay lost yield reply");
        assert_eq!(first, replay);
        let confirmations = server
            .invocation_control()
            .expect("invocation control")
            .confirmations_after(0)
            .expect("yield confirmations");
        assert_eq!(confirmations.len(), 1);
        assert_eq!(confirmations[0].command_id, "yield-1");
        assert_eq!(confirmations[0].invocation_id, "invocation-yield");

        let mut wrong = SocketToolHandler::for_invocation(
            &socket,
            "secret-token",
            "attempt-yield",
            "substituted-invocation",
        );
        let error = wrong
            .call(AgentToolCall::Yield(YieldArguments {
                command_id: "yield-wrong".to_owned(),
            }))
            .expect_err("reject substituted invocation");
        assert_eq!(error.code, AgentToolErrorCode::Rejected);
    }
}
