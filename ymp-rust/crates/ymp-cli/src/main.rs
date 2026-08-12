#![forbid(unsafe_code)]

use anyhow::{Context, bail};
use clap::{Parser, Subcommand, ValueEnum};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use uuid::Uuid;
use ymp_application::Application;
use ymp_domain::{Budget, Command as DomainCommand};
use ymp_runtime_api::{CancellationToken, InvocationRequest, McpBinding, Readiness, RuntimeDriver};
use ymp_runtime_claude::ClaudeRuntime;
use ymp_runtime_codex::CodexRuntime;
use ymp_runtime_fake::FakeRuntime;

#[derive(Debug, Parser)]
#[command(
    name = "ymp",
    version,
    about = "Bounded local coordination for coding agents"
)]
struct Cli {
    #[arg(long, global = true, default_value = ".ymp-data")]
    data_root: PathBuf,
    #[arg(
        long,
        global = true,
        value_name = "FILE",
        help = "Load a managed TUI contract; may be repeated"
    )]
    contract: Vec<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    Internal {
        #[command(subcommand)]
        command: InternalCommand,
    },
}

#[derive(Debug, Subcommand)]
enum InternalCommand {
    AgentMcp,
    RuntimeSmoke {
        #[arg(long)]
        runtime: RuntimeChoice,
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        prompt: String,
    },
    ManagedRuntimeSmoke {
        #[arg(long)]
        runtime: RuntimeChoice,
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long)]
        prompt: String,
    },
    ManagedCandidateSmoke {
        #[arg(long)]
        runtime: RuntimeChoice,
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        prompt: String,
    },
    Verifier {
        #[arg(long)]
        candidate: PathBuf,
        #[arg(long)]
        expected_sha256: String,
    },
    VerifyManagedCandidate {
        #[arg(long)]
        program: PathBuf,
        #[arg(long = "arg")]
        arguments: Vec<String>,
        #[arg(long)]
        negative_control: PathBuf,
        #[arg(long)]
        contract_digest: String,
        #[arg(long)]
        oracle_digest: String,
        #[arg(long, default_value_t = 60_000)]
        wall_time_ms: u64,
        #[arg(long, default_value_t = 1_048_576)]
        output_limit_bytes: usize,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum RuntimeChoice {
    Fake,
    Codex,
    Claude,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        None => {
            let contracts = cli
                .contract
                .iter()
                .map(ymp_runtime_supervisor::ManagedContract::load)
                .collect::<anyhow::Result<Vec<_>>>()?;
            ymp_tui::run_with_contracts(cli.data_root, contracts)
        }
        Some(Command::Internal { command }) => match command {
            InternalCommand::AgentMcp => run_agent_mcp(),
            InternalCommand::RuntimeSmoke {
                runtime,
                workspace,
                prompt,
            } => run_runtime_smoke(runtime, workspace, prompt),
            InternalCommand::ManagedRuntimeSmoke {
                runtime,
                workspace,
                prompt,
            } => run_managed_runtime_smoke(cli.data_root, runtime, workspace, prompt),
            InternalCommand::ManagedCandidateSmoke {
                runtime,
                source,
                prompt,
            } => run_managed_candidate_smoke(cli.data_root, runtime, source, prompt),
            InternalCommand::Verifier {
                candidate,
                expected_sha256,
            } => {
                let outcome = ymp_verifier::check_exact_digest(candidate, expected_sha256)?;
                println!("{}", serde_json::to_string_pretty(&outcome)?);
                if !outcome.matched {
                    bail!("candidate digest does not match");
                }
                Ok(())
            }
            InternalCommand::VerifyManagedCandidate {
                program,
                arguments,
                negative_control,
                contract_digest,
                oracle_digest,
                wall_time_ms,
                output_limit_bytes,
            } => run_managed_verification(ManagedVerificationRequest {
                data_root: cli.data_root,
                program,
                arguments,
                negative_control,
                contract_digest,
                oracle_digest,
                wall_time_ms,
                output_limit_bytes,
            }),
        },
    }
}

fn runtime_driver(runtime: RuntimeChoice) -> Box<dyn RuntimeDriver> {
    match runtime {
        RuntimeChoice::Fake => Box::new(FakeRuntime::default()),
        RuntimeChoice::Codex => Box::new(CodexRuntime::default()),
        RuntimeChoice::Claude => Box::new(ClaudeRuntime::default()),
    }
}

fn run_runtime_smoke(
    runtime: RuntimeChoice,
    workspace: PathBuf,
    prompt: String,
) -> anyhow::Result<()> {
    let driver = runtime_driver(runtime);
    let probe = driver.probe()?;
    println!("{}", serde_json::to_string(&probe)?);
    if probe.readiness != Readiness::Ready {
        bail!("runtime profile is not ready: {}", probe.detail);
    }
    let mut session = driver.start(InvocationRequest {
        invocation_id: format!("smoke-{}", Uuid::new_v4()),
        attempt_id: format!("attempt-{}", Uuid::new_v4()),
        workspace,
        mcp: None,
        prompt,
        cancellation: CancellationToken::default(),
    })?;
    while let Some(event) = session.next_event()? {
        println!("{}", serde_json::to_string(&event)?);
    }
    Ok(())
}

fn run_managed_runtime_smoke(
    data_root: PathBuf,
    runtime: RuntimeChoice,
    workspace: PathBuf,
    prompt: String,
) -> anyhow::Result<()> {
    let run_id = format!("managed-smoke-{}", Uuid::new_v4());
    let attempt_id = format!("attempt-{}", Uuid::new_v4());
    let mut application = Application::create(&data_root, &run_id, Budget::new(1, 1))?;
    application.execute(
        format!("{attempt_id}.start"),
        DomainCommand::StartAttempt {
            attempt_id: attempt_id.clone(),
        },
    )?;
    let application = Arc::new(Mutex::new(application));
    let token = Uuid::new_v4().simple().to_string();
    let socket_path = data_root.join("runtime").join("agent.sock");
    let _rpc_server = ymp_agent_rpc::AgentRpcServer::start(
        &socket_path,
        &token,
        &attempt_id,
        Arc::clone(&application),
    )?;
    let executable = std::env::current_exe()
        .context("resolve current ymp executable")?
        .canonicalize()
        .context("canonicalize current ymp executable")?;
    let driver = runtime_driver(runtime);
    let probe = driver.probe()?;
    println!("{}", serde_json::to_string(&probe)?);
    if probe.readiness != Readiness::Ready {
        bail!("runtime profile is not ready: {}", probe.detail);
    }
    let mut session = driver.start(InvocationRequest {
        invocation_id: format!("invocation-{}", Uuid::new_v4()),
        attempt_id,
        workspace,
        mcp: Some(McpBinding {
            executable,
            socket_path,
            token,
        }),
        prompt,
        cancellation: CancellationToken::default(),
    })?;
    while let Some(event) = session.next_event()? {
        println!("{}", serde_json::to_string(&event)?);
    }
    let application = application
        .lock()
        .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?;
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "type": "managed_smoke_state",
            "state": application.state()
        }))?
    );
    Ok(())
}

fn run_managed_candidate_smoke(
    data_root: PathBuf,
    runtime: RuntimeChoice,
    source: PathBuf,
    prompt: String,
) -> anyhow::Result<()> {
    let source = source
        .canonicalize()
        .with_context(|| format!("canonicalize source directory {}", source.display()))?;
    if !source.is_dir() {
        bail!("source is not a directory: {}", source.display());
    }
    let run_id = format!("managed-candidate-smoke-{}", Uuid::new_v4());
    let attempt_id = format!("attempt-{}", Uuid::new_v4());
    let mut application = Application::create(&data_root, &run_id, Budget::new(1, 1))?;
    let artifacts = application.artifact_store();
    let base = artifacts.capture_source(&source)?;
    let workspace = data_root.join("workspaces").join(&attempt_id);
    artifacts.materialize(&base.manifest_digest, &workspace)?;
    initialize_private_git(&workspace)?;
    application.execute(
        format!("{attempt_id}.start"),
        DomainCommand::StartAttempt {
            attempt_id: attempt_id.clone(),
        },
    )?;

    let application = Arc::new(Mutex::new(application));
    let token = Uuid::new_v4().simple().to_string();
    let socket_path = data_root.join("runtime").join("agent.sock");
    let _rpc_server = ymp_agent_rpc::AgentRpcServer::start(
        &socket_path,
        &token,
        &attempt_id,
        Arc::clone(&application),
    )?;
    let executable = std::env::current_exe()
        .context("resolve current ymp executable")?
        .canonicalize()
        .context("canonicalize current ymp executable")?;
    let driver = runtime_driver(runtime);
    let probe = driver.probe()?;
    println!("{}", serde_json::to_string(&probe)?);
    if probe.readiness != Readiness::Ready {
        bail!("runtime profile is not ready: {}", probe.detail);
    }
    let mut session = driver.start(InvocationRequest {
        invocation_id: format!("invocation-{}", Uuid::new_v4()),
        attempt_id: attempt_id.clone(),
        workspace: workspace.clone(),
        mcp: Some(McpBinding {
            executable,
            socket_path,
            token,
        }),
        prompt,
        cancellation: CancellationToken::default(),
    })?;
    while let Some(event) = session.next_event()? {
        println!("{}", serde_json::to_string(&event)?);
    }

    let mut application = application
        .lock()
        .map_err(|_| anyhow::anyhow!("application lock was poisoned"))?;
    let submitted = application.submit_workspace_candidate_excluding(
        format!("{attempt_id}.submit-workspace"),
        &attempt_id,
        &base.manifest_digest,
        &workspace,
        &["target"],
    )?;
    let candidate_directory = data_root
        .join("candidates")
        .join(&submitted.candidate.snapshot_digest);
    application
        .artifact_store()
        .materialize(&submitted.candidate.snapshot_digest, &candidate_directory)?;
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "type": "managed_candidate_smoke_result",
            "base": base,
            "submission": submitted.submission,
            "candidate": submitted.candidate,
            "candidate_directory": candidate_directory,
            "state": application.state()
        }))?
    );
    Ok(())
}

fn initialize_private_git(workspace: &std::path::Path) -> anyhow::Result<()> {
    for (action, arguments) in [
        ("initialize private Git repository", vec!["init", "--quiet"]),
        ("stage private workspace", vec!["add", "--all"]),
        (
            "commit private baseline",
            vec![
                "-c",
                "user.name=ymp",
                "-c",
                "user.email=ymp@invalid",
                "-c",
                "core.hooksPath=/dev/null",
                "commit",
                "--quiet",
                "--allow-empty",
                "-m",
                "ymp private baseline",
            ],
        ),
    ] {
        let status = ProcessCommand::new("git")
            .args(arguments)
            .current_dir(workspace)
            .status()
            .with_context(|| format!("{action} in {}", workspace.display()))?;
        if !status.success() {
            bail!("{action} failed with {status}");
        }
    }
    Ok(())
}

struct ManagedVerificationRequest {
    data_root: PathBuf,
    program: PathBuf,
    arguments: Vec<String>,
    negative_control: PathBuf,
    contract_digest: String,
    oracle_digest: String,
    wall_time_ms: u64,
    output_limit_bytes: usize,
}

fn run_managed_verification(request: ManagedVerificationRequest) -> anyhow::Result<()> {
    let ManagedVerificationRequest {
        data_root,
        program,
        arguments,
        negative_control,
        contract_digest,
        oracle_digest,
        wall_time_ms,
        output_limit_bytes,
    } = request;
    let program = program
        .canonicalize()
        .with_context(|| format!("canonicalize verifier program {}", program.display()))?;
    let negative_control = negative_control.canonicalize().with_context(|| {
        format!(
            "canonicalize negative control {}",
            negative_control.display()
        )
    })?;
    let mut application = Application::open(&data_root)?;
    let candidate_digest = application
        .state()
        .candidate_digest
        .clone()
        .context("run has no submitted candidate")?;
    let verifier = ymp_verifier::CommandVerifier::new(
        contract_digest,
        oracle_digest,
        program,
        arguments,
        Duration::from_millis(wall_time_ms),
        output_limit_bytes,
    )?;
    let environment_digest = verifier.environment_digest()?;
    if let Some(outcome) = application.stored_verification(
        &candidate_digest,
        verifier.contract_digest(),
        verifier.oracle_digest(),
        &environment_digest,
    ) {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "type": "managed_verification_result",
                "outcome": outcome,
                "state": application.state()
            }))?
        );
        return Ok(());
    }
    let (_, candidate) = materialize_controller_candidate(&application, &data_root)?;
    let command_id = format!("managed.verify.{}", Uuid::new_v4());
    let evidence = match verifier.verify_candidate(&candidate, &negative_control, &candidate_digest)
    {
        Ok(evidence) => evidence,
        Err(error) => {
            let outcome = application.execute(
                command_id,
                DomainCommand::FailInfrastructure {
                    reason: format!("verifier failed: {error}")
                        .chars()
                        .take(ymp_domain::MAX_REASON_BYTES)
                        .collect(),
                },
            )?;
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "type": "managed_verification_result",
                    "error": error.to_string(),
                    "outcome": outcome,
                    "state": application.state()
                }))?
            );
            return Ok(());
        }
    };
    let outcome = application.record_verification(command_id, &evidence)?;
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "type": "managed_verification_result",
            "evidence": evidence,
            "outcome": outcome,
            "state": application.state()
        }))?
    );
    Ok(())
}

fn materialize_controller_candidate(
    application: &Application,
    data_root: &std::path::Path,
) -> anyhow::Result<(String, PathBuf)> {
    let candidate_digest = application
        .state()
        .candidate_digest
        .clone()
        .context("run has no submitted candidate")?;
    let candidate = data_root.join("verification-inputs").join(format!(
        "{}-{}",
        candidate_digest,
        Uuid::new_v4()
    ));
    application
        .artifact_store()
        .materialize(&candidate_digest, &candidate)
        .context("materialize the controller-bound candidate for verification")?;
    Ok((candidate_digest, candidate))
}

fn run_agent_mcp() -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    let handler = ymp_agent_rpc::SocketToolHandler::from_env()
        .context("load controller-bound agent RPC capability")?;
    let mut server = ymp_agent_mcp::McpServer::new(handler);
    for line in stdin.lock().lines() {
        let line = line?;
        if let Some(response) = server.handle_line(&line) {
            writeln!(stdout, "{response}")?;
            stdout.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        Cli, ManagedVerificationRequest, materialize_controller_candidate, run_managed_verification,
    };
    use clap::{CommandFactory, Parser};
    use std::fs;
    use std::time::Duration;
    use tempfile::tempdir;
    use ymp_application::Application;
    use ymp_domain::{Budget, Command, RunStatus};

    #[test]
    fn cli_public_command_surface_exposes_only_the_internal_namespace() {
        let mut command = Cli::command();
        let public_commands = command
            .get_subcommands()
            .map(|subcommand| subcommand.get_name())
            .collect::<Vec<_>>();
        assert_eq!(public_commands, ["internal"]);

        let help = command.render_long_help().to_string().to_lowercase();
        for forbidden in ["demo", "inspect", "probe", "daemon", "socket", "headless"] {
            assert!(
                !help
                    .split(|character: char| !character.is_alphanumeric() && character != '-')
                    .any(|word| word == forbidden),
                "public help exposes forbidden command {forbidden}:\n{help}"
            );
        }
    }

    #[test]
    fn cli_default_invocation_selects_the_foreground_mode() {
        let cli = Cli::try_parse_from(["ymp"]).expect("default invocation");
        assert!(cli.command.is_none());
    }

    #[test]
    fn cli_internal_children_are_nested_below_the_internal_namespace() {
        let command = Cli::command();
        let internal = command
            .find_subcommand("internal")
            .expect("internal namespace");
        for child in [
            "agent-mcp",
            "runtime-smoke",
            "managed-runtime-smoke",
            "managed-candidate-smoke",
            "verifier",
            "verify-managed-candidate",
        ] {
            assert!(command.find_subcommand(child).is_none());
            assert!(
                internal.find_subcommand(child).is_some(),
                "missing internal child {child}"
            );
        }
    }

    #[test]
    fn managed_verification_materializes_the_controller_bound_candidate() {
        let root = tempdir().expect("temporary root");
        let data_root = root.path().join("data");
        let source = root.path().join("source");
        let workspace = root.path().join("workspace");
        fs::create_dir_all(&source).expect("source directory");
        fs::write(source.join("result.txt"), b"before\n").expect("base file");

        let mut application =
            Application::create(&data_root, "run-1", Budget::new(1, 1)).expect("application");
        let base = application
            .artifact_store()
            .capture_source(&source)
            .expect("base capture");
        application
            .artifact_store()
            .materialize(&base.manifest_digest, &workspace)
            .expect("workspace");
        fs::write(workspace.join("result.txt"), b"after\n").expect("candidate edit");
        application
            .execute(
                "attempt.start",
                Command::StartAttempt {
                    attempt_id: "attempt-1".to_owned(),
                },
            )
            .expect("attempt start");
        let submitted = application
            .submit_workspace_candidate(
                "attempt.submit",
                "attempt-1",
                &base.manifest_digest,
                &workspace,
            )
            .expect("candidate submission");

        let (digest, materialized) =
            materialize_controller_candidate(&application, &data_root).expect("materialization");
        assert_eq!(digest, submitted.candidate.snapshot_digest);
        assert_eq!(
            fs::read(materialized.join("result.txt")).expect("materialized file"),
            b"after\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn passing_managed_negative_control_records_infrastructure_error() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempdir().expect("temporary root");
        let data_root = root.path().join("data");
        let source = root.path().join("source");
        let workspace = root.path().join("workspace");
        let negative = root.path().join("negative");
        let program = root.path().join("oracle");
        fs::create_dir_all(&source).expect("source directory");
        fs::create_dir(&negative).expect("negative directory");
        fs::write(source.join("result.txt"), b"candidate\n").expect("source file");
        fs::write(&program, b"#!/bin/sh\nexit 0\n").expect("oracle program");
        let mut permissions = fs::metadata(&program).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&program, permissions).expect("make executable");

        let mut application =
            Application::create(&data_root, "run-1", Budget::new(1, 1)).expect("application");
        let base = application
            .artifact_store()
            .capture_source(&source)
            .expect("base capture");
        application
            .artifact_store()
            .materialize(&base.manifest_digest, &workspace)
            .expect("workspace");
        application
            .execute(
                "attempt.start",
                Command::StartAttempt {
                    attempt_id: "attempt-1".to_owned(),
                },
            )
            .expect("attempt start");
        application
            .submit_workspace_candidate(
                "attempt.submit",
                "attempt-1",
                &base.manifest_digest,
                &workspace,
            )
            .expect("candidate submission");
        drop(application);

        let result = run_managed_verification(ManagedVerificationRequest {
            data_root: data_root.clone(),
            program,
            arguments: Vec::new(),
            negative_control: negative,
            contract_digest: "1".repeat(64),
            oracle_digest: "2".repeat(64),
            wall_time_ms: Duration::from_secs(5).as_millis() as u64,
            output_limit_bytes: 1024,
        });
        assert!(result.is_ok(), "infrastructure result must be committed");

        let recovered = Application::open(&data_root).expect("reopen application");
        assert_eq!(recovered.state().status, RunStatus::InfrastructureError);
        assert!(recovered.state().candidate_digest.is_some());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unavailable_managed_launcher_records_infrastructure_error() {
        use std::os::unix::fs::PermissionsExt;

        let launcher_available = std::process::Command::new("/usr/bin/unshare")
            .args([
                "--user",
                "--map-root-user",
                "--mount",
                "--pid",
                "--net",
                "--fork",
                "/bin/true",
            ])
            .status()
            .is_ok_and(|status| status.success());
        if launcher_available {
            return;
        }

        let root = tempdir().expect("temporary root");
        let data_root = root.path().join("data");
        let source = root.path().join("source");
        let workspace = root.path().join("workspace");
        let negative = root.path().join("negative");
        let program = root.path().join("oracle");
        fs::create_dir_all(&source).expect("source directory");
        fs::create_dir(&negative).expect("negative directory");
        fs::write(source.join("result.txt"), b"candidate\n").expect("source file");
        fs::write(&program, b"#!/bin/sh\nexit 1\n").expect("oracle program");
        let mut permissions = fs::metadata(&program).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&program, permissions).expect("make executable");

        let mut application =
            Application::create(&data_root, "run-1", Budget::new(1, 1)).expect("application");
        let base = application
            .artifact_store()
            .capture_source(&source)
            .expect("base capture");
        application
            .artifact_store()
            .materialize(&base.manifest_digest, &workspace)
            .expect("workspace");
        application
            .execute(
                "attempt.start",
                Command::StartAttempt {
                    attempt_id: "attempt-1".to_owned(),
                },
            )
            .expect("attempt start");
        let candidate_digest = application
            .submit_workspace_candidate(
                "attempt.submit",
                "attempt-1",
                &base.manifest_digest,
                &workspace,
            )
            .expect("candidate submission")
            .candidate
            .snapshot_digest;
        drop(application);

        run_managed_verification(ManagedVerificationRequest {
            data_root: data_root.clone(),
            program,
            arguments: Vec::new(),
            negative_control: negative,
            contract_digest: "1".repeat(64),
            oracle_digest: "2".repeat(64),
            wall_time_ms: Duration::from_secs(5).as_millis() as u64,
            output_limit_bytes: 1024,
        })
        .expect("infrastructure result must be committed");

        let recovered = Application::open(&data_root).expect("reopen application");
        assert_eq!(recovered.state().status, RunStatus::InfrastructureError);
        assert_eq!(
            recovered.state().candidate_digest.as_deref(),
            Some(candidate_digest.as_str())
        );
    }

    #[cfg(unix)]
    #[test]
    fn repeated_managed_verification_returns_the_stored_result() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempdir().expect("temporary root");
        let data_root = root.path().join("data");
        let source = root.path().join("source");
        let workspace = root.path().join("workspace");
        let negative = root.path().join("negative");
        let program = root.path().join("oracle");
        fs::create_dir_all(&source).expect("source directory");
        fs::create_dir(&negative).expect("negative directory");
        fs::write(source.join("pass"), b"candidate\n").expect("candidate marker");
        fs::write(
            &program,
            b"#!/bin/sh\nif [ -f \"$1/pass\" ]; then exit 0; else exit 1; fi\n",
        )
        .expect("oracle program");
        let mut permissions = fs::metadata(&program).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&program, permissions).expect("make executable");

        let mut application =
            Application::create(&data_root, "run-1", Budget::new(1, 1)).expect("application");
        let base = application
            .artifact_store()
            .capture_source(&source)
            .expect("base capture");
        application
            .artifact_store()
            .materialize(&base.manifest_digest, &workspace)
            .expect("workspace");
        application
            .execute(
                "attempt.start",
                Command::StartAttempt {
                    attempt_id: "attempt-1".to_owned(),
                },
            )
            .expect("attempt start");
        application
            .submit_workspace_candidate(
                "attempt.submit",
                "attempt-1",
                &base.manifest_digest,
                &workspace,
            )
            .expect("candidate submission");
        drop(application);

        for _ in 0..2 {
            run_managed_verification(ManagedVerificationRequest {
                data_root: data_root.clone(),
                program: program.clone(),
                arguments: Vec::new(),
                negative_control: negative.clone(),
                contract_digest: "1".repeat(64),
                oracle_digest: "2".repeat(64),
                wall_time_ms: Duration::from_secs(5).as_millis() as u64,
                output_limit_bytes: 1024,
            })
            .expect("managed verification");
        }

        let recovered = Application::open(&data_root).expect("reopen application");
        assert_eq!(recovered.state().status, RunStatus::Accepted);
        assert_eq!(recovered.state().budget.verification_queries_remaining, 0);
        assert_eq!(recovered.events_after(0).expect("events").len(), 4);
    }
}
