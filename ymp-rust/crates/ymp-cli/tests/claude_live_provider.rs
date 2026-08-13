#![forbid(unsafe_code)]

//! Provider-path evidence for the managed Claude Code profile.
//!
//! These checks start the real pinned Claude Code build, so they spend provider budget and depend
//! on an authenticated operator credential. They are therefore ignored by default and are run
//! explicitly with `cargo test -p ymp-cli --test claude_live_provider -- --ignored`.

use std::fs;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use ymp_application::Application;
use ymp_domain::{Budget, EventKind};
use ymp_runtime_api::{
    CancellationToken, InvocationRequest, Readiness, RuntimeDriver, RuntimeEventKind,
    RuntimeFailureKind, RuntimeKind,
};
use ymp_runtime_claude::{ClaudeProfile, ClaudeRuntime, PINNED_CLAUDE_VERSION};
use ymp_runtime_supervisor::{
    ManagedCandidateRequest, ManagedContract, ManagedRunEvent, start_managed_candidate,
};

#[test]
#[ignore = "starts the real pinned Claude Code build and spends provider budget"]
fn the_pinned_build_reports_its_exact_version_and_authentication_readiness() {
    let runtime = ClaudeRuntime::default();
    let probe = runtime.probe().expect("probe the pinned build");
    println!("probe: {}", serde_json::to_string(&probe).expect("probe"));
    assert_eq!(probe.kind, RuntimeKind::ClaudeCode);
    assert_eq!(probe.version.as_deref(), Some(PINNED_CLAUDE_VERSION));
    assert_eq!(probe.readiness, Readiness::Ready);
    assert!(probe.detail.contains("api_provider=firstParty"));
    assert!(probe.detail.contains("native_subagents=disabled"));

    // Negative half: the same build in the same generated home, with no credential delegated, is
    // unauthenticated. The generated home therefore isolates authentication rather than inheriting
    // it, and the readiness above is produced by the delegated credential and nothing else.
    let isolated = ClaudeRuntime::default().without_delegated_credential();
    let isolated = isolated.probe().expect("probe without delegation");
    println!(
        "isolated probe: {}",
        serde_json::to_string(&isolated).expect("probe")
    );
    assert_eq!(isolated.readiness, Readiness::Unauthenticated);
}

#[test]
#[ignore = "starts the real pinned Claude Code build and spends provider budget"]
fn an_unpinned_build_is_detected_and_refused() {
    let installed = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .expect("home directory")
        .join(".local/share/claude/versions");
    let unpinned = fs::read_dir(&installed)
        .expect("installed Claude Code builds")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.is_file()
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| !PINNED_CLAUDE_VERSION.starts_with(name))
        })
        .expect("an installed build other than the pinned one");
    let probe = ClaudeRuntime::new(&unpinned)
        .probe()
        .expect("probe the unpinned build");
    println!(
        "unpinned probe: {}",
        serde_json::to_string(&probe).expect("probe")
    );
    assert_eq!(probe.readiness, Readiness::Incompatible);
    assert!(probe.detail.contains(PINNED_CLAUDE_VERSION));
}

#[test]
#[ignore = "starts the real pinned Claude Code build and spends provider budget"]
fn a_live_budget_stop_ends_the_attempt_and_keeps_its_reported_usage() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let workspace = temporary.path().join("workspace");
    fs::create_dir(&workspace).expect("workspace directory");
    fs::write(workspace.join("input.txt"), b"before\n").expect("workspace file");

    let profile = ClaudeProfile {
        // The smallest bound the profile admits. The harness enforces it, so the attempt stops on
        // its first accounted request instead of running to completion.
        max_budget_microusd: 1,
        ..ClaudeProfile::default()
    };
    let runtime = ClaudeRuntime::with_profile(
        std::env::var_os("HOME")
            .map(std::path::PathBuf::from)
            .expect("home directory")
            .join(".local/share/claude/versions")
            .join(
                PINNED_CLAUDE_VERSION
                    .split_whitespace()
                    .next()
                    .expect("pinned build"),
            ),
        profile,
    );
    let mut session = runtime
        .start(InvocationRequest {
            invocation_id: "invocation-live-budget".to_owned(),
            attempt_id: "attempt-live-budget".to_owned(),
            workspace,
            mcp: None,
            prompt: "Reply with exactly: ok".to_owned(),
            cancellation: CancellationToken::default(),
        })
        .expect("start the bounded live attempt");

    let mut terminal = None;
    while let Ok(Some(event)) = session.next_event() {
        println!(
            "live budget event: {}",
            serde_json::to_string(&event).expect("event")
        );
        if let RuntimeEventKind::Failed { kind, usage, .. } = event.event {
            terminal = Some((kind, usage));
            break;
        }
    }
    let (kind, usage) = terminal.expect("the bounded attempt produced no terminal failure");
    assert_eq!(kind, RuntimeFailureKind::RuntimeReported);
    assert!(
        usage.cost_microusd.is_some_and(|cost| cost > 0),
        "the budget stop lost its reported cost"
    );
    assert!(usage.wall_time_ms > 0);
    assert!(session.next_event().expect("terminal session").is_none());
}

#[test]
#[ignore = "starts the real pinned Claude Code build and spends provider budget"]
fn the_pinned_build_completes_one_managed_candidate_through_the_product_path() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    fs::create_dir(&source).expect("source directory");
    fs::write(source.join("input.txt"), b"before\n").expect("source file");

    let data_root = temporary.path().join("data");
    let application = Arc::new(Mutex::new(
        Application::create(&data_root, "run-claude-live", Budget::new(1, 1))
            .expect("create application"),
    ));
    let handle = start_managed_candidate(
        Arc::clone(&application),
        Box::new(ClaudeRuntime::default()),
        ManagedCandidateRequest {
            contract: ManagedContract {
                contract_id: "claude-live-contract".to_owned(),
                contract_digest: "d".repeat(64),
                source,
                prompt: concat!(
                    "Replace the whole content of input.txt in the current directory with the ",
                    "single line: after. Then call the ymp submit tool once with command_id ",
                    "agent.submit.live. Then stop."
                )
                .to_owned(),
                capture_exclusions: Vec::new(),
                verifier: None,
            },
            bridge_executable: env!("CARGO_BIN_EXE_ymp").into(),
        },
    )
    .expect("start the managed live candidate");
    let attempt_id = handle.attempt_id().to_owned();

    let deadline = Instant::now() + Duration::from_secs(600);
    let mut candidate = None;
    let mut failure = None;
    let mut session = None;
    let mut usage = None;
    let mut coordination = Vec::new();
    while Instant::now() < deadline && !handle.is_finished() {
        while let Some(event) = handle.try_next() {
            match event {
                ManagedRunEvent::CandidateAvailable {
                    candidate_digest, ..
                } => candidate = Some(candidate_digest),
                ManagedRunEvent::Failed { detail } => failure = Some(detail),
                ManagedRunEvent::Runtime(event) => match event.event {
                    RuntimeEventKind::Started { opaque_session_id } => {
                        session = Some(opaque_session_id)
                    }
                    RuntimeEventKind::McpToolCall { tool, status, .. } => {
                        coordination.push(format!("{tool}:{status}"))
                    }
                    RuntimeEventKind::Completed { usage: reported } => usage = Some(reported),
                    RuntimeEventKind::Output { text } => println!("claude: {text}"),
                    _ => {}
                },
                ManagedRunEvent::Finished => {}
            }
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    while let Some(event) = handle.try_next() {
        match event {
            ManagedRunEvent::CandidateAvailable {
                candidate_digest, ..
            } => candidate = Some(candidate_digest),
            ManagedRunEvent::Failed { detail } => failure = Some(detail),
            ManagedRunEvent::Runtime(event) => match event.event {
                RuntimeEventKind::Started { opaque_session_id } => {
                    session = Some(opaque_session_id)
                }
                RuntimeEventKind::McpToolCall { tool, status, .. } => {
                    coordination.push(format!("{tool}:{status}"))
                }
                RuntimeEventKind::Completed { usage: reported } => usage = Some(reported),
                _ => {}
            },
            ManagedRunEvent::Finished => {}
        }
    }
    assert!(handle.is_finished(), "the live managed attempt did not end");
    assert_eq!(failure, None, "the live managed attempt failed");

    let session = session.expect("the live attempt reported no session");
    assert!(!session.is_empty());
    let usage = usage.expect("the live attempt reported no usage");
    println!(
        "live usage: {}",
        serde_json::to_string(&usage).expect("usage")
    );
    println!("live coordination: {coordination:?}");
    assert!(usage.input_tokens > 0);
    assert!(usage.output_tokens > 0);
    assert!(usage.cost_microusd.is_some_and(|cost| cost > 0));
    assert!(usage.wall_time_ms > 0);
    assert!(coordination.iter().any(|call| call == "submit:completed"));

    let candidate = candidate.expect("the live attempt produced no candidate");
    let committed = application
        .lock()
        .expect("application lock")
        .events_after(0)
        .expect("controller events");
    let submissions: Vec<_> = committed
        .iter()
        .filter(|event| matches!(&event.event, EventKind::CandidateSubmitted { .. }))
        .collect();
    assert_eq!(submissions.len(), 1);
    assert_eq!(submissions[0].command_id, "agent.submit.live");
    assert_eq!(
        application
            .lock()
            .expect("application lock")
            .state()
            .candidate_digest
            .as_deref(),
        Some(candidate.as_str())
    );

    let evidence_directory = data_root.join("runtime-evidence").join(&attempt_id);
    let profile: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(evidence_directory.join("profile.json")).expect("profile evidence"),
    )
    .expect("parse profile evidence");
    assert_eq!(
        profile["profile"]["probe"]["version"],
        PINNED_CLAUDE_VERSION
    );
    assert_eq!(
        profile["profile"]["environment_policy"],
        "synthetic_allowlist_with_delegated_credential_v1"
    );
    let evidence =
        fs::read_to_string(evidence_directory.join("events.jsonl")).expect("event evidence");
    assert!(!evidence.contains(&session), "evidence exposed the session");
    println!("live evidence records: {}", evidence.lines().count());
    handle.join().expect("join worker");
}
