use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ymp_application::Application;
use ymp_domain::{Budget, RunStatus, digest_bytes};
use ymp_runtime_api::{
    InvocationRequest, LaunchChain, LaunchDescriptor, ProbeReport, Readiness, RuntimeDriver,
    RuntimeError, RuntimeEvent, RuntimeEventKind, RuntimeKind, RuntimeSession, Usage,
    configure_process_group, create_launch_marker, end_process_tree_or_keep,
    managed_launch_command, register_launch_marker, terminate_process_tree,
};
use ymp_runtime_supervisor::{ManagedCandidateRequest, ManagedContract, start_managed_candidate};

struct ProcessCodexRuntime {
    executable: PathBuf,
    prepared: Mutex<Option<LaunchDescriptor>>,
}

impl RuntimeDriver for ProcessCodexRuntime {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Codex
    }

    fn executable(&self) -> &Path {
        &self.executable
    }

    fn probe(&self) -> Result<ProbeReport, RuntimeError> {
        Ok(ProbeReport {
            kind: RuntimeKind::Codex,
            executable: self.executable.display().to_string(),
            version: Some("codex-cli 0.151.0".to_owned()),
            readiness: Readiness::Ready,
            detail: "exact process-boundary fixture".to_owned(),
        })
    }

    fn start(&self, _request: InvocationRequest) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
        Err(RuntimeError::Unsupported(
            "the managed fixture requires its launch descriptor",
        ))
    }

    fn prepare_launch(
        &self,
        request: &InvocationRequest,
    ) -> Result<Option<LaunchDescriptor>, RuntimeError> {
        let descriptor = LaunchDescriptor {
            schema_version: 1,
            invocation_id: request.invocation_id.clone(),
            attempt_id: request.attempt_id.clone(),
            executable: self.executable.clone(),
            executable_digest: digest_bytes(&std::fs::read(&self.executable)?),
            coordination_executable: None,
            coordination_executable_digest: None,
            launch_chain: LaunchChain::default().admit()?,
            arguments: Vec::new(),
            environment: Vec::new(),
            working_directory: request.workspace.clone(),
        };
        *self
            .prepared
            .lock()
            .map_err(|_| RuntimeError::InvalidProfile("fixture lock failed".to_owned()))? =
            Some(descriptor.clone());
        Ok(Some(descriptor))
    }

    fn start_prepared(
        &self,
        request: InvocationRequest,
        descriptor: Option<&LaunchDescriptor>,
    ) -> Result<Box<dyn RuntimeSession>, RuntimeError> {
        let descriptor = descriptor.ok_or_else(|| {
            RuntimeError::InvalidProfile("fixture launch descriptor is missing".to_owned())
        })?;
        let prepared = self
            .prepared
            .lock()
            .map_err(|_| RuntimeError::InvalidProfile("fixture lock failed".to_owned()))?
            .take()
            .ok_or_else(|| {
                RuntimeError::InvalidProfile("fixture launch was not prepared".to_owned())
            })?;
        if &prepared != descriptor {
            return Err(RuntimeError::InvalidProfile(
                "fixture launch descriptor changed".to_owned(),
            ));
        }
        let marker = create_launch_marker()?;
        let mut command = managed_launch_command(
            &descriptor.executable,
            &descriptor.arguments,
            &marker,
            &descriptor.launch_chain,
        )?;
        command
            .current_dir(&request.workspace)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        configure_process_group(&mut command);
        let child = command.spawn()?;
        register_launch_marker(&child, marker);
        Ok(Box::new(ProcessCodexSession {
            child,
            invocation_id: request.invocation_id,
            sequence: 0,
            pending: VecDeque::from([
                RuntimeEventKind::Launch {
                    descriptor: Box::new(descriptor.clone()),
                },
                RuntimeEventKind::Started {
                    opaque_session_id: "thread-process-0151".to_owned(),
                },
            ]),
            interrupted: false,
            interruption_emitted: false,
            ended: false,
        }))
    }
}

struct ProcessCodexSession {
    child: Child,
    invocation_id: String,
    sequence: u64,
    pending: VecDeque<RuntimeEventKind>,
    interrupted: bool,
    interruption_emitted: bool,
    ended: bool,
}

impl ProcessCodexSession {
    fn event(&mut self, event: RuntimeEventKind) -> RuntimeEvent {
        self.sequence += 1;
        RuntimeEvent {
            sequence: self.sequence,
            event_id: format!("{}.event-{}", self.invocation_id, self.sequence),
            invocation_id: self.invocation_id.clone(),
            event,
        }
    }
}

impl RuntimeSession for ProcessCodexSession {
    fn next_event(&mut self) -> Result<Option<RuntimeEvent>, RuntimeError> {
        if let Some(event) = self.pending.pop_front() {
            return Ok(Some(self.event(event)));
        }
        if self.interrupted {
            if self.interruption_emitted {
                return Ok(None);
            }
            self.interruption_emitted = true;
            return Ok(Some(self.event(RuntimeEventKind::Cancelled {
                usage: Usage::default(),
            })));
        }
        std::thread::sleep(Duration::from_millis(5));
        Ok(Some(self.event(RuntimeEventKind::Output {
            text: "working".to_owned(),
        })))
    }

    fn resume(&mut self, _input: String) -> Result<(), RuntimeError> {
        Err(RuntimeError::NotYielded)
    }

    fn interrupt(&mut self) -> Result<(), RuntimeError> {
        terminate_process_tree(&mut self.child)?;
        self.ended = true;
        self.interrupted = true;
        Ok(())
    }

    fn usage(&self) -> Usage {
        Usage::default()
    }
}

impl Drop for ProcessCodexSession {
    fn drop(&mut self) {
        if !self.ended {
            end_process_tree_or_keep(&mut self.child);
        }
    }
}

/// The exact Codex projection crosses the production managed-supervisor gate with a real process
/// tree. The descendant ignores the graceful signal, so only bounded tree escalation can close it.
#[test]
fn exact_0151_projection_cancels_the_managed_fake_executable_and_its_descendant() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    std::fs::create_dir(&source).expect("source directory");
    std::fs::write(source.join("input.txt"), b"base\n").expect("source file");
    let executable = temporary.path().join("codex-0151-process-fixture");
    std::fs::write(
        &executable,
        b"#!/bin/sh\ntrap '' TERM\n( trap '' TERM; while :; do sleep 1; done ) &\nprintf '%s\\n' \"$!\" > descendant.pid\nwait\n",
    )
    .expect("write process fixture");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&executable)
            .expect("fixture metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&executable, permissions).expect("make fixture executable");
    }
    let application = Arc::new(Mutex::new(
        Application::create(temporary.path().join("data"), "run-0151", Budget::new(1, 1))
            .expect("create application"),
    ));
    let handle = start_managed_candidate(
        Arc::clone(&application),
        Box::new(ProcessCodexRuntime {
            executable,
            prepared: Mutex::new(None),
        }),
        ManagedCandidateRequest {
            contract: ManagedContract {
                contract_id: "contract-0151-process".to_owned(),
                contract_digest: "c".repeat(64),
                source,
                prompt: "wait for cancellation".to_owned(),
                capture_exclusions: Vec::new(),
                verifier: None,
            },
            bridge_executable: std::env::current_exe().expect("current executable"),
        },
    )
    .expect("start exact 0.151 managed fixture");
    let workspace = application
        .lock()
        .expect("application")
        .private_workspace_path(handle.attempt_id())
        .expect("private workspace");
    let pid_path = workspace.join("descendant.pid");
    let deadline = Instant::now() + Duration::from_secs(5);
    while !pid_path.is_file() && Instant::now() < deadline {
        let _ = handle.try_next();
        std::thread::sleep(Duration::from_millis(5));
    }
    let descendant = std::fs::read_to_string(&pid_path).expect("descendant pid");
    handle
        .cancel("process-boundary cancellation")
        .expect("cancel");
    while !handle.is_finished() && Instant::now() < deadline {
        let _ = handle.try_next();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(handle.is_finished(), "managed process did not finish");
    assert_eq!(
        application.lock().expect("application").state().status,
        RunStatus::Cancelled
    );
    let profile: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            temporary
                .path()
                .join("data/runtime-evidence")
                .join(handle.attempt_id())
                .join("profile.json"),
        )
        .expect("runtime profile"),
    )
    .expect("runtime profile JSON");
    assert_eq!(profile["profile"]["probe"]["version"], "codex-cli 0.151.0");
    let alive = std::process::Command::new("/bin/kill")
        .args(["-0", descendant.trim()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("probe descendant")
        .success();
    assert!(!alive, "managed descendant survived cancellation");
    handle.join().expect("join managed fixture");
}
