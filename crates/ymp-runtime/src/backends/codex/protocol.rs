//! Bounded local JSON-RPC transport. Protocol data never grants task authority.
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};
use ymp_domain::{Denial, Result};
pub const VERSION: &str = "0.156.1";
pub const DISABLED: &[&str] = &[
    "shell_tool",
    "code_mode",
    "apps",
    "plugins",
    "hooks",
    "multi_agent",
    "multi_agent_v2",
    "browser_use",
    "browser_use_external",
    "computer_use",
    "view_image",
    "image_generation",
    "goals",
    "memories",
    "shell_snapshot",
    "skill_mcp_dependency_install",
    "skill_search",
    "workspace_dependencies",
    "realtime_conversation",
    "in_app_browser",
    "remote_plugin",
    "tool_suggest",
    "unbounded_connection_retries",
];
pub(crate) fn denied(code: &str) -> Denial {
    Denial::new(
        code,
        "Codex protocol boundary was not established; no credentials or native payload are included",
    )
}

/// Resolve the pinned release's packaged helper before admitting native work.
/// Ambiguous legacy resource layouts are unavailable rather than inferred from
/// ambient CODEX_HOME/package-manager settings.
pub(crate) fn code_mode_host(executable: &Path) -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let executable = executable
        .canonicalize()
        .map_err(|_| denied("codex_installation"))?;
    let parent = executable
        .parent()
        .ok_or_else(|| denied("codex_installation"))?;
    let bin = match parent.file_name().and_then(|s| s.to_str()) {
        Some("bin") => Some(parent.to_path_buf()),
        Some("codex-resources") => parent.parent().map(|p| p.join("bin")),
        Some("MacOS")
            if parent
                .parent()
                .is_some_and(|p| p.ends_with("CodexCLI.app/Contents")) =>
        {
            parent
                .parent()
                .and_then(Path::parent)
                .and_then(Path::parent)
                .map(|p| p.join("bin"))
        }
        _ => None,
    }
    .filter(|bin| {
        bin.is_dir()
            && bin
                .parent()
                .is_some_and(|p| p.join("codex-package.json").is_file())
    });
    let fallback = parent.join("codex-code-mode-host");
    let helper = if let Some(bin) = bin {
        let bundled = bin
            .parent()
            .unwrap()
            .join("codex-resources/codex-code-mode-host");
        if bundled.is_file() {
            bundled
        } else if bin.join("codex-code-mode-host").is_file() {
            bin.join("codex-code-mode-host")
        } else {
            fallback
        }
    } else {
        if parent
            .join("codex-resources/codex-code-mode-host")
            .is_file()
        {
            return Err(denied("codex_installation_layout"));
        }
        fallback
    };
    let metadata = helper
        .metadata()
        .map_err(|_| denied("codex_code_mode_host_unavailable"))?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return Err(denied("codex_code_mode_host_unavailable"));
    }
    Ok(helper)
}
struct Startup {
    child: Option<Child>,
    cwd: PathBuf,
}
fn terminate(child: &mut Child) {
    if let Some(group) = rustix::process::Pid::from_raw(child.id() as i32) {
        let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
    }
    let _ = child.kill();
    let until = Instant::now() + Duration::from_millis(250);
    while child.try_wait().is_ok_and(|s| s.is_none()) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(2));
    }
}
impl Drop for Startup {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            terminate(child);
            let _ = std::fs::remove_dir(&self.cwd);
        }
    }
}
pub(crate) struct Transport {
    child: Child,
    stop: Arc<AtomicBool>,
    readers: Vec<std::thread::JoinHandle<()>>,
    pub writer: Arc<Mutex<ChildStdin>>,
    reader: Receiver<Result<Value>>,
    seq: u64,
    pub pending: VecDeque<Value>,
    pub cwd: PathBuf,
    pub frame: usize,
}
impl Drop for Transport {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        terminate(&mut self.child);
        for reader in self.readers.drain(..) {
            let _ = reader.join();
        }
        let _ = std::fs::remove_dir(&self.cwd);
    }
}
pub(crate) fn send(writer: &Arc<Mutex<ChildStdin>>, value: &Value, frame: usize) -> Result<()> {
    let mut bytes = serde_json::to_vec(value).map_err(|_| denied("codex_encode"))?;
    if bytes.len() > frame {
        return Err(denied("codex_frame"));
    }
    bytes.push(b'\n');
    let deadline = Instant::now() + Duration::from_millis(250);
    let mut out = loop {
        match writer.try_lock() {
            Ok(out) => break out,
            Err(std::sync::TryLockError::Poisoned(_)) => return Err(denied("codex_transport")),
            Err(_) if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(2)),
            Err(_) => return Err(denied("codex_write_timeout")),
        }
    };
    let mut offset = 0;
    while offset < bytes.len() {
        if Instant::now() >= deadline {
            return Err(denied("codex_write_timeout"));
        }
        match out.write(&bytes[offset..]) {
            Ok(0) => return Err(denied("codex_disconnect")),
            Ok(n) => offset += n,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(2))
            }
            Err(_) => return Err(denied("codex_disconnect")),
        }
    }
    Ok(())
}
impl Transport {
    pub fn spawn(executable: &Path, servers: &[String], frame: usize) -> Result<Self> {
        if servers.iter().any(|server| {
            server.is_empty()
                || !server
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        }) {
            return Err(denied("codex_mcp_name"));
        }
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|_| denied("codex_entropy"))?;
        let cwd =
            std::env::temp_dir().join(format!("ymp-codex-{}", ymp_domain::Digest::of(random)));
        std::fs::create_dir(&cwd).map_err(|_| denied("codex_directory"))?;
        let cwd = std::fs::canonicalize(&cwd).map_err(|_| {
            let _ = std::fs::remove_dir(&cwd);
            denied("codex_directory")
        })?;
        use std::os::unix::process::CommandExt;
        let mut command = Command::new(executable);
        command.process_group(0);
        command.args(["app-server", "--listen", "stdio://"]);
        for feature in DISABLED {
            command.args(["-c", &format!("features.{feature}=false")]);
        }
        for value in [
            "features.code_mode_host.enabled=true",
            "features.code_mode_host.disable_in_process_fallback=true",
            "features.skip_host_skill_discovery=true",
            "notify=[]",
            "web_search=\"disabled\"",
            "approval_policy=\"never\"",
            "approvals_reviewer=\"user\"",
            "sandbox_mode=\"read-only\"",
        ] {
            command.args(["-c", value]);
        }
        for server in servers {
            if server.is_empty()
                || !server
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            {
                return Err(denied("codex_mcp_name"));
            }
            command.args(["-c", &format!("mcp_servers.{server}.enabled=false")]);
        }
        let child = command
            .current_dir(&cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| {
                let _ = std::fs::remove_dir(&cwd);
                denied("codex_spawn")
            })?;
        let mut startup = Startup {
            child: Some(child),
            cwd: cwd.clone(),
        };
        let child = startup.child.as_mut().unwrap();
        let stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        for descriptor in [
            &stdin as &dyn std::os::fd::AsFd,
            &stdout as &dyn std::os::fd::AsFd,
            &stderr as &dyn std::os::fd::AsFd,
        ] {
            rustix::fs::fcntl_setfl(
                descriptor,
                rustix::fs::fcntl_getfl(descriptor).map_err(|_| denied("codex_pipe"))?
                    | rustix::fs::OFlags::NONBLOCK,
            )
            .map_err(|_| denied("codex_pipe"))?;
        }
        let writer = Arc::new(Mutex::new(stdin));
        let stop = Arc::new(AtomicBool::new(false));
        let (sender, reader) = mpsc::sync_channel(64);
        let stopped = stop.clone();
        let output = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let mut buffer = [0; 4096];
            let deliver = |mut message: Result<Value>| {
                while !stopped.load(Ordering::SeqCst) {
                    match sender.try_send(message) {
                        Ok(()) => return true,
                        Err(mpsc::TrySendError::Full(value)) => {
                            message = value;
                            std::thread::sleep(Duration::from_millis(2));
                        }
                        Err(_) => return false,
                    }
                }
                false
            };
            while !stopped.load(Ordering::SeqCst) {
                match stdout.read(&mut buffer) {
                    Ok(0) => {
                        deliver(Err(denied("codex_disconnect")));
                        break;
                    }
                    Ok(n) => {
                        for byte in &buffer[..n] {
                            bytes.push(*byte);
                            if bytes.len() > frame {
                                deliver(Err(denied("codex_frame")));
                                return;
                            }
                            if *byte == b'\n' {
                                let message = ymp_domain::journal::decode::<Value>(&bytes);
                                let failed = message.is_err();
                                if !deliver(message) || failed {
                                    return;
                                }
                                bytes.clear();
                            }
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => {
                        deliver(Err(denied("codex_disconnect")));
                        break;
                    }
                }
            }
        });
        // Drain without retaining native diagnostics or inherited-pipe blocking.
        let stopped = stop.clone();
        let errors = std::thread::spawn(move || {
            let mut buffer = [0; 4096];
            while !stopped.load(Ordering::SeqCst) {
                match stderr.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(_) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => break,
                }
            }
        });
        let child = startup.child.take().unwrap();
        Ok(Self {
            child,
            stop,
            readers: vec![output, errors],
            writer,
            reader,
            seq: 0,
            pending: VecDeque::new(),
            cwd,
            frame,
        })
    }
    pub fn receive(&self, deadline: Instant) -> Result<Value> {
        let timeout = deadline
            .checked_duration_since(Instant::now())
            .ok_or_else(|| denied("codex_timeout"))?;
        self.reader
            .recv_timeout(timeout)
            .map_err(|_| denied("codex_timeout"))?
    }
    pub fn request(&mut self, method: &str, params: Value, deadline: Instant) -> Result<Value> {
        self.seq += 1;
        let id = self.seq;
        send(
            &self.writer,
            &json!({"id":id,"method":method,"params":params}),
            self.frame,
        )?;
        loop {
            let message = self.receive(deadline)?;
            if message.get("id") == Some(&json!(id)) && message.get("method").is_none() {
                if message.get("error").is_some() {
                    return Err(denied("codex_rpc"));
                }
                return message
                    .get("result")
                    .cloned()
                    .ok_or_else(|| denied("codex_response"));
            }
            if message.get("method").is_some() && message.get("id").is_some() {
                reject(&self.writer, &message, self.frame)?;
            } else if message.get("method").is_some() {
                if self.pending.len() >= 64 {
                    return Err(denied("codex_notifications"));
                }
                self.pending.push_back(message);
            } else {
                return Err(denied("codex_foreign_response"));
            }
        }
    }
    pub fn initialize(&mut self, deadline: Instant) -> Result<()> {
        self.request("initialize",json!({"clientInfo":{"name":"ymp","version":"0.1.0"},"capabilities":{"experimentalApi":true}}),deadline)?;
        send(&self.writer, &json!({"method":"initialized"}), self.frame)
    }
}
pub(crate) fn reject(writer: &Arc<Mutex<ChildStdin>>, message: &Value, frame: usize) -> Result<()> {
    let method = message["method"]
        .as_str()
        .ok_or_else(|| denied("codex_request"))?;
    let result = match method {
        "item/tool/call" => {
            json!({"contentItems":[{"type":"inputText","text":"Denied by the host authority boundary"}],"success":false})
        }
        "item/commandExecution/requestApproval" | "item/fileChange/requestApproval" => {
            json!({"decision":"decline"})
        }
        "item/permissions/requestApproval" => json!({"permissions":{},"scope":"turn"}),
        "mcpServer/elicitation/request" => json!({"action":"decline","content":null}),
        _ => {
            return send(
                writer,
                &json!({"id":message["id"],"error":{"code":-32601,"message":"Unsupported host request"}}),
                frame,
            );
        }
    };
    send(writer, &json!({"id":message["id"],"result":result}), frame)
}
pub(crate) fn servers(config: &Value) -> Result<Vec<String>> {
    let entries = config
        .get("mcp_servers")
        .and_then(Value::as_object)
        .ok_or_else(|| denied("codex_config"))?;
    if entries.len() > 128 {
        return Err(denied("codex_config"));
    }
    Ok(entries.keys().cloned().collect())
}
pub(crate) fn verify(config: &Value, features: &Value, requirements: &Value) -> Result<()> {
    let requirement = requirements
        .get("requirements")
        .ok_or_else(|| denied("codex_requirements"))?;
    if !requirement.is_null() && !requirement.is_object() {
        return Err(denied("codex_requirements"));
    }
    let entries = config
        .get("mcp_servers")
        .and_then(Value::as_object)
        .ok_or_else(|| denied("codex_config"))?;
    if entries
        .values()
        .any(|s| s.get("enabled") != Some(&Value::Bool(false)))
        || config["notify"] != json!([])
        || config["web_search"] != "disabled"
        || config["approval_policy"] != "never"
        || requirements
            .get("requirements")
            .is_some_and(|r| !r.is_null() && r.get("hooks").is_some_and(|h| !h.is_null()))
    {
        return Err(denied("codex_external_surfaces"));
    }
    let list = features["data"]
        .as_array()
        .ok_or_else(|| denied("codex_features"))?;
    let mut actual = std::collections::BTreeMap::new();
    for feature in list {
        let name = feature["name"]
            .as_str()
            .ok_or_else(|| denied("codex_features"))?;
        let enabled = feature["enabled"]
            .as_bool()
            .ok_or_else(|| denied("codex_features"))?;
        if actual.insert(name, enabled).is_some() {
            return Err(denied("codex_features"));
        }
    }
    if features.get("nextCursor").is_some_and(|c| !c.is_null())
        || DISABLED.iter().any(|name| actual.get(name) != Some(&false))
        || actual.get("code_mode_host") != Some(&true)
        || config["features"]["code_mode_host"]["enabled"] != true
        || config["features"]["code_mode_host"]["disable_in_process_fallback"] != true
        || actual.get("skip_host_skill_discovery") != Some(&true)
    {
        return Err(denied("codex_external_surfaces"));
    }
    Ok(())
}
pub(crate) fn guarded(
    executable: &Path,
    frame: usize,
    timeout: Duration,
) -> Result<(Transport, Value)> {
    let end = Instant::now() + timeout;
    let mut initial = Transport::spawn(executable, &[], frame)?;
    initial.initialize(end)?;
    let config = initial.request("config/read", json!({"includeLayers":false}), end)?;
    let names = servers(&config["config"])?;
    drop(initial);
    let mut transport = Transport::spawn(executable, &names, frame)?;
    transport.initialize(end)?;
    let config = verify_current(&mut transport, end)?;
    let account = transport.request("account/read", json!({"refreshToken":false}), end)?;
    let requires_auth = account["requiresOpenaiAuth"]
        .as_bool()
        .ok_or_else(|| denied("codex_authentication"))?;
    if requires_auth && account.get("account").is_none_or(Value::is_null) {
        return Err(denied("codex_authentication"));
    }
    Ok((transport, config))
}

pub(crate) fn version(executable: &Path, timeout: Duration) -> Result<()> {
    use std::os::unix::process::CommandExt;
    let child = Command::new(executable)
        .arg("--version")
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| denied("codex_version"))?;
    let mut guard = Startup {
        child: Some(child),
        cwd: PathBuf::new(),
    };
    let child = guard.child.as_mut().unwrap();
    let mut out = child.stdout.take().unwrap();
    rustix::fs::fcntl_setfl(&out, rustix::fs::OFlags::NONBLOCK)
        .map_err(|_| denied("codex_pipe"))?;
    let until = Instant::now() + timeout;
    let mut bytes = vec![];
    let mut buffer = [0; 129];
    let mut eof = false;
    loop {
        if Instant::now() >= until {
            return Err(denied("codex_timeout"));
        }
        match out.read(&mut buffer) {
            Ok(0) => eof = true,
            Ok(n) => {
                bytes.extend_from_slice(&buffer[..n]);
                if bytes.len() > 128 {
                    return Err(denied("codex_version"));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => return Err(denied("codex_version")),
        }
        if eof && let Some(status) = child.try_wait().map_err(|_| denied("codex_version"))? {
            if status.success()
                && String::from_utf8_lossy(&bytes).trim() == format!("codex-cli {VERSION}")
            {
                return Ok(());
            }
            return Err(denied("codex_version"));
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

pub(crate) fn verify_current(transport: &mut Transport, deadline: Instant) -> Result<Value> {
    let config = transport.request("config/read", json!({"includeLayers":false}), deadline)?;
    let flags = transport.request("experimentalFeature/list", json!({"limit":200}), deadline)?;
    let requirements = transport.request("configRequirements/read", json!({}), deadline)?;
    verify(&config["config"], &flags, &requirements)?;
    Ok(config["config"].clone())
}

#[test]
fn packaged_host_prefers_resources_and_never_falls_back_from_a_denied_file() {
    use std::os::unix::fs::PermissionsExt;
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).unwrap();
    let root =
        std::env::temp_dir().join(format!("ymp-host-layout-{}", ymp_domain::Digest::of(nonce)));
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(root.join("bin")).unwrap();
    std::fs::create_dir(root.join("codex-resources")).unwrap();
    std::fs::write(root.join("codex-package.json"), b"{}").unwrap();
    let executable = root.join("bin/codex");
    let sibling = root.join("bin/codex-code-mode-host");
    let bundled = root.join("codex-resources/codex-code-mode-host");
    for path in [&executable, &sibling, &bundled] {
        std::fs::write(path, b"not executed by this metadata test").unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    assert_eq!(
        code_mode_host(&executable).unwrap(),
        bundled.canonicalize().unwrap()
    );
    std::fs::set_permissions(&bundled, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        code_mode_host(&executable).unwrap_err().code,
        "codex_code_mode_host_unavailable"
    );
    std::fs::remove_file(&bundled).unwrap();
    assert_eq!(
        code_mode_host(&executable).unwrap(),
        sibling.canonicalize().unwrap()
    );
    std::fs::remove_file(&sibling).unwrap();
    assert_eq!(
        code_mode_host(&executable).unwrap_err().code,
        "codex_code_mode_host_unavailable"
    );
    std::fs::remove_dir_all(root).unwrap();
}
