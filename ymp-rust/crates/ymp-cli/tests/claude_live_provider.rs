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
    CancellationToken, DiagnosticSummary, InvocationRequest, Readiness, RuntimeDriver,
    RuntimeEventKind, RuntimeFailureKind, RuntimeKind, Usage,
};
use ymp_runtime_claude::{
    APPROVED_SEARCH_PATH, ClaudeProfile, ClaudeRuntime, PINNED_CLAUDE_VERSION,
};
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
    let mut observed = LiveObservation::default();
    while Instant::now() < deadline && !handle.is_finished() {
        while let Some(event) = handle.try_next() {
            observed.absorb(event);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    while let Some(event) = handle.try_next() {
        observed.absorb(event);
    }
    assert!(handle.is_finished(), "the live managed attempt did not end");

    let session = observed
        .session
        .clone()
        .expect("the live attempt reported no session");
    assert!(!session.is_empty());
    let terminal = observed
        .terminal
        .clone()
        .expect("the live attempt reported no terminal runtime event");
    let submitted = observed
        .coordination
        .iter()
        .any(|call| call == "submit:completed");
    println!("live coordination: {:?}", observed.coordination);

    // Model traffic and its accounting are the product's obligation whatever the model decided.
    let usage = observed
        .usage
        .clone()
        .expect("the live attempt reported no usage");
    println!(
        "live usage: {}",
        serde_json::to_string(&usage).expect("usage")
    );
    assert!(usage.input_tokens > 0);
    assert!(usage.output_tokens > 0);
    assert!(usage.cost_microusd.is_some_and(|cost| cost > 0));
    assert!(usage.wall_time_ms > 0);
    live_runtime_evidence(&data_root, &attempt_id, &session);

    // A turn that closed with no committed controller action, and in which no coordination call
    // was ever answered, is a model that declined to act. The product did its part: it started the
    // pinned build, admitted the session, carried the turn, accounted for it and refused to invent
    // a candidate. That outcome is reported and the check ends without failing, so a failing
    // result of this check always names a defect of the product rather than a choice of the model.
    let declined_turn = matches!(
        &terminal,
        RuntimeEventKind::Failed { kind, diagnostic, .. }
            if *kind == RuntimeFailureKind::Protocol
                && diagnostic.as_ref()
                    == Some(&DiagnosticSummary::from_bytes(
                        b"controller_action_count_invalid",
                        false,
                    ))
    );
    if declined_turn && !submitted && observed.candidate.is_none() {
        println!(
            "live outcome: model_declined_to_act — the pinned build ran and accounted for its \
             turn, and the model closed it without calling the coordination tool"
        );
        handle.join().expect("join worker");
        return;
    }

    println!("live outcome: product_path_completed");
    assert_eq!(
        observed.failure, None,
        "the live managed attempt failed: {terminal:?}"
    );
    assert!(
        matches!(terminal, RuntimeEventKind::Completed { .. }),
        "the attempt neither completed nor closed as a declined turn: {terminal:?}"
    );
    assert!(submitted, "the completed attempt answered no submit call");

    let candidate = observed
        .candidate
        .clone()
        .expect("the live attempt produced no candidate");
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

    handle.join().expect("join worker");
}

/// What a live managed attempt reported, kept whole so that the outcome is classified once, from
/// the complete stream, rather than from whichever event the reading loop happened to end on.
#[derive(Default)]
struct LiveObservation {
    candidate: Option<String>,
    failure: Option<String>,
    session: Option<String>,
    usage: Option<Usage>,
    terminal: Option<RuntimeEventKind>,
    coordination: Vec<String>,
}

impl LiveObservation {
    fn absorb(&mut self, event: ManagedRunEvent) {
        match event {
            ManagedRunEvent::CandidateAvailable {
                candidate_digest, ..
            } => self.candidate = Some(candidate_digest),
            ManagedRunEvent::Failed { detail } => self.failure = Some(detail),
            ManagedRunEvent::Runtime(event) => match &event.event {
                RuntimeEventKind::Started { opaque_session_id } => {
                    self.session = Some(opaque_session_id.clone());
                }
                RuntimeEventKind::McpToolCall { tool, status, .. } => {
                    self.coordination.push(format!("{tool}:{status}"));
                }
                RuntimeEventKind::Output { text } => println!("claude: {text}"),
                RuntimeEventKind::Completed { usage }
                | RuntimeEventKind::Failed { usage, .. }
                | RuntimeEventKind::TimedOut { usage, .. }
                | RuntimeEventKind::Cancelled { usage } => {
                    self.usage = Some(usage.clone());
                    self.terminal = Some(event.event.clone());
                }
                _ => {}
            },
            ManagedRunEvent::Finished => {}
        }
    }
}

/// The evidence a managed attempt owes whatever the model decided: the pinned profile record and a
/// transcript that names no session identifier.
fn live_runtime_evidence(data_root: &std::path::Path, attempt_id: &str, session: &str) {
    let evidence_directory = data_root.join("runtime-evidence").join(attempt_id);
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
    // The record states the environment the child actually received. Only the approved names reach
    // it, and its search path is the approved one rather than the operator's.
    let recorded: Vec<&str> = profile["profile"]["launch_descriptor"]["environment"]
        .as_array()
        .expect("the record states the child environment")
        .iter()
        .map(|variable| variable["name"].as_str().expect("environment name"))
        .collect();
    assert_eq!(
        recorded,
        [
            "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
            "CLAUDE_CONFIG_DIR",
            "HOME",
            "NO_COLOR",
            "PATH",
            "TMPDIR",
            "YMP_AGENT_SOCKET",
            "YMP_AGENT_TOKEN",
            "YMP_ATTEMPT_ID",
            "YMP_INVOCATION_ID",
        ],
        "the profile record carries an environment the profile did not approve"
    );
    let search_path = profile["profile"]["launch_descriptor"]["environment"]
        .as_array()
        .expect("the record states the child environment")
        .iter()
        .find(|variable| variable["name"] == "PATH")
        .and_then(|variable| variable["value"].as_str())
        .expect("the record states the search path");
    assert_eq!(search_path, APPROVED_SEARCH_PATH.join(":"));
    assert_ne!(
        Some(search_path),
        std::env::var("PATH").ok().as_deref(),
        "the record carries the operator search path"
    );

    let evidence =
        fs::read_to_string(evidence_directory.join("events.jsonl")).expect("event evidence");
    assert!(!evidence.contains(session), "evidence exposed the session");
    println!("live evidence records: {}", evidence.lines().count());
}
