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

const MAX_REQUEST_BYTES: usize = 64 * 1024;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

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
        Ok(Self::new(socket_path, token, attempt_id))
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
}

impl AgentRpcServer {
    #[cfg(unix)]
    pub fn start(
        socket_path: impl Into<PathBuf>,
        token: impl Into<String>,
        attempt_id: impl Into<String>,
        application: Arc<Mutex<Application>>,
    ) -> Result<Self, AgentRpcError> {
        Self::start_inner(socket_path, token, attempt_id, application, None)
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
            application,
            Some(submission),
        )
    }

    #[cfg(unix)]
    fn start_inner(
        socket_path: impl Into<PathBuf>,
        token: impl Into<String>,
        attempt_id: impl Into<String>,
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
    let mut application = application
        .lock()
        .map_err(|_| AgentToolError::internal("controller state lock is unavailable"))?;
    match submission {
        Some(submission) => application
            .workspace_agent_session(expected_attempt, submission)
            .call(request.call),
        None => application
            .agent_session(expected_attempt)
            .call(request.call),
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
    use std::fs;
    use std::sync::{Arc, Mutex};
    use ymp_agent_api::{AgentToolCall, AgentToolErrorCode, AgentToolHandler, SubmitArguments};
    use ymp_application::{Application, WorkspaceSubmission};
    use ymp_domain::{Budget, Command};

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
}
