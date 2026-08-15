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
    APPROVED_SEARCH_PATH, ClaudeProfile, ClaudeRuntime, MINIMUM_CLAUDE_VERSION, PINNED_CLAUDE_MODEL,
};
use ymp_runtime_supervisor::{
    ManagedCandidateRequest, ManagedContract, ManagedRunEvent, start_managed_candidate,
};

#[test]
#[ignore = "starts the real pinned Claude Code build and spends provider budget"]
fn the_installed_build_reports_its_own_version_and_authentication_readiness() {
    let runtime = ClaudeRuntime::default();
    let probe = runtime.probe().expect("probe the installed build");
    println!("probe: {}", serde_json::to_string(&probe).expect("probe"));
    assert_eq!(probe.kind, RuntimeKind::ClaudeCode);
    // The probe names the build this host has installed, whatever it is, and admits it because it
    // is at or above the floor rather than because it repeats one frozen string.
    let version = probe
        .version
        .clone()
        .expect("the probe measured no version");
    assert!(version.ends_with("(Claude Code)"), "{version}");
    assert!(
        at_least_the_floor(&version),
        "{version} is below {MINIMUM_CLAUDE_VERSION}"
    );
    assert_eq!(probe.readiness, Readiness::Ready, "{}", probe.detail);
    assert!(probe.detail.contains("api_provider=firstParty"));
    assert!(probe.detail.contains("native_subagents=disabled"));
    assert!(
        probe.detail.contains("executable_digest="),
        "the readiness report does not name the executable it measured: {}",
        probe.detail
    );

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

/// Admission now follows the installed release, so every build this host has at or above the floor
/// is admitted rather than refused for not being one frozen string. What keeps that honest is the
/// agreement between the two measurements: the build the probe reads from an executable is the
/// build the session started from that same executable announces, and a session that named any
/// other build would have been refused before model traffic.
#[test]
#[ignore = "starts the real installed Claude Code build and spends provider budget"]
fn every_installed_build_is_admitted_and_the_session_names_the_build_that_was_probed() {
    let installed = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .expect("home directory")
        .join(".local/share/claude/versions");
    let builds: Vec<std::path::PathBuf> = fs::read_dir(&installed)
        .expect("installed Claude Code builds")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    assert!(!builds.is_empty(), "no Claude Code build is installed");
    for build in &builds {
        let probe = ClaudeRuntime::new(build)
            .probe()
            .expect("probe an installed build");
        println!(
            "installed probe: {}",
            serde_json::to_string(&probe).expect("probe")
        );
        let version = probe
            .version
            .clone()
            .expect("the probe measured no version");
        let named = build
            .file_name()
            .and_then(|name| name.to_str())
            .expect("build file name");
        assert!(
            version.starts_with(named),
            "the probe of {named} measured {version}"
        );
        assert_eq!(
            probe.readiness,
            Readiness::Ready,
            "{named}: {}",
            probe.detail
        );
    }

    // The live half: a real session against the installed build starts, which it can only do after
    // its `init` reported the same build the probe measured from that executable.
    let attempt = live_managed_attempt("Reply with the single word done. Change no file.");
    assert!(
        attempt.observed.session.is_some(),
        "the live attempt reported no session"
    );
}

/// The model list the registry records for the Claude engine is measured from the installed build,
/// and the cheaper routes the owner permits for experiments are in it.
///
/// The measurement starts the real build once per candidate and reaches no provider: the build
/// names an unrecognised model and then refuses the missing input, so nothing is spent. Both
/// halves are checked here — the list names models the build serves, and it does not name a model
/// no build serves, which is what would happen if the measurement had stopped discriminating and
/// were reporting every candidate as served.
#[test]
#[ignore = "starts the real installed Claude Code build once per model candidate"]
fn the_measured_model_catalog_names_the_routes_the_installed_build_serves() {
    let runtime = ClaudeRuntime::default();
    let catalog = runtime
        .measure_model_catalog()
        .expect("measure the model catalog of the installed build");
    println!("catalog: {catalog:?}");
    assert!(
        !catalog.is_empty(),
        "the installed build served no model at all"
    );
    for permitted in ["haiku", "sonnet"] {
        assert!(
            catalog.iter().any(|model| model.contains(permitted)),
            "the measured catalog names no {permitted} route: {catalog:?}"
        );
    }
    assert!(
        catalog.iter().any(|model| model == PINNED_CLAUDE_MODEL),
        "the measured catalog does not name the pinned route {PINNED_CLAUDE_MODEL}: {catalog:?}"
    );
    // Negative half: a name shaped like a model but served by no build never enters the list. A
    // measurement that had lost its negative answer would carry this candidate through.
    assert!(
        !catalog
            .iter()
            .any(|model| model.starts_with("claude-instant")),
        "the measured catalog names a route no installed build serves: {catalog:?}"
    );
}

/// Reads a `--version` line against the profile floor, comparing the build ordinals as numbers.
fn at_least_the_floor(reported: &str) -> bool {
    let ordinal = |version: &str| -> Vec<u64> {
        version
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .split('.')
            .map(|component| component.parse::<u64>().unwrap_or_default())
            .collect()
    };
    let (found, floor) = (ordinal(reported), ordinal(MINIMUM_CLAUDE_VERSION));
    let read = |values: &[u64], index: usize| values.get(index).copied().unwrap_or(0);
    (0..found.len().max(floor.len()))
        .find_map(
            |index| match read(&found, index).cmp(&read(&floor, index)) {
                std::cmp::Ordering::Equal => None,
                ordering => Some(ordering == std::cmp::Ordering::Greater),
            },
        )
        .unwrap_or(true)
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
    let runtime =
        ClaudeRuntime::with_profile(ClaudeRuntime::default().executable().to_owned(), profile);
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
    let attempt = live_managed_attempt(concat!(
        "Replace the whole content of input.txt in the current directory with the ",
        "single line: after. Then call the ymp submit tool once with command_id ",
        "agent.submit.live. Then stop."
    ));
    let observed = &attempt.observed;
    let terminal = observed
        .terminal
        .clone()
        .expect("the live attempt reported no terminal runtime event");
    let outcome = classify(observed);
    println!("live outcome: {outcome:?}");
    if outcome == LiveOutcome::ModelDeclinedToAct {
        // The product did its part: it started the pinned build, admitted the session, carried the
        // turn, accounted for it and refused to invent a candidate the controller never received.
        // The turn ended because the model called nothing, so the check ends without failing.
        return;
    }
    assert_eq!(
        outcome,
        LiveOutcome::ProductPathCompleted,
        "the managed attempt is a product defect: terminal={terminal:?}, supervision={:?}, \
         coordination={:?}, candidate={:?}",
        observed.failure,
        observed.coordination,
        observed.candidate
    );

    let candidate = observed
        .candidate
        .clone()
        .expect("the live attempt produced no candidate");
    let committed = attempt
        .application
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
        attempt
            .application
            .lock()
            .expect("application lock")
            .state()
            .candidate_digest
            .as_deref(),
        Some(candidate.as_str())
    );
}

/// A model that closes its turn without calling anything is reported as a declined turn rather than
/// as a failure of the product. This measures the outcome the classification treats as non-failing,
/// so the two branches of the live check are both evidence rather than one branch and one reading.
#[test]
#[ignore = "starts the real pinned Claude Code build and spends provider budget"]
fn a_model_that_declines_to_act_is_reported_without_failing() {
    let attempt = live_managed_attempt(concat!(
        "Do not change any file and do not call any tool, including the ymp tools. ",
        "Reply with exactly: ok. Then stop."
    ));
    assert_eq!(
        classify(&attempt.observed),
        LiveOutcome::ModelDeclinedToAct,
        "a model that called nothing was not reported as a declined turn: terminal={:?}, \
         supervision={:?}, coordination={:?}",
        attempt.observed.terminal,
        attempt.observed.failure,
        attempt.observed.coordination
    );
    assert_eq!(
        attempt
            .application
            .lock()
            .expect("application lock")
            .state()
            .candidate_digest,
        None,
        "a declined turn produced a candidate"
    );
}

/// One finished live managed attempt, together with what the product owes whatever the model did:
/// a session, an accounted turn, and the evidence records of the pinned profile.
struct LiveAttempt {
    observed: LiveObservation,
    application: Arc<Mutex<Application>>,
    _temporary: tempfile::TempDir,
}

fn live_managed_attempt(prompt: &str) -> LiveAttempt {
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
                prompt: prompt.to_owned(),
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
    handle.join().expect("join worker");

    let session = observed
        .session
        .clone()
        .expect("the live attempt reported no session");
    assert!(!session.is_empty());
    println!("live coordination: {:?}", observed.coordination);
    println!("live supervision detail: {:?}", observed.failure);

    // Model traffic, its accounting and the evidence records are the product's obligation whatever
    // the model decided, so they are measured before the outcome is classified.
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

    LiveAttempt {
        observed,
        application,
        _temporary: temporary,
    }
}

/// The only supervision detail a declined turn produces. The supervisor reports a turn that closed
/// with no committed controller action as a failed supervision, so the declined outcome is
/// recognised by this exact detail; any other detail, including one carrying a termination the run
/// could not establish, is a defect of the product.
const DECLINED_SUPERVISION_DETAIL: &str = "managed_runtime_supervision_failed";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LiveOutcome {
    /// The product carried the attempt through to a committed candidate.
    ProductPathCompleted,
    /// The product carried the attempt and the model closed the turn without calling anything.
    ModelDeclinedToAct,
    /// Anything else, including a coordination call the product failed to answer.
    ProductDefect,
}

/// Decides what a finished live attempt measured. A declined turn is recognised only when the
/// model called nothing at all: a coordination call that was made and failed carries the same
/// terminal signature — a protocol failure with an invalid controller action count — while being a
/// defect of the product, so the emptiness of the call list, and not the absence of a completed
/// submission, separates the two.
fn classify(observed: &LiveObservation) -> LiveOutcome {
    let Some(terminal) = &observed.terminal else {
        return LiveOutcome::ProductDefect;
    };
    let submitted = observed
        .coordination
        .iter()
        .any(|call| call == "submit:completed");
    if matches!(terminal, RuntimeEventKind::Completed { .. })
        && submitted
        && observed.candidate.is_some()
        && observed.failure.is_none()
    {
        return LiveOutcome::ProductPathCompleted;
    }
    let closed_without_action = matches!(
        terminal,
        RuntimeEventKind::Failed { kind, diagnostic, .. }
            if *kind == RuntimeFailureKind::Protocol
                && diagnostic.as_ref()
                    == Some(&DiagnosticSummary::from_bytes(
                        b"controller_action_count_invalid",
                        false,
                    ))
    );
    if closed_without_action
        && observed.coordination.is_empty()
        && observed.candidate.is_none()
        && observed.failure.as_deref() == Some(DECLINED_SUPERVISION_DETAIL)
    {
        return LiveOutcome::ModelDeclinedToAct;
    }
    LiveOutcome::ProductDefect
}

/// Negative half of the classification: a coordination call the product failed to answer produces
/// the same terminal signature as a model that called nothing, and must still be a product defect.
/// This check needs no provider budget, so it runs with the ordinary suite.
#[test]
fn a_failed_coordination_call_is_never_read_as_a_declined_model() {
    let closed_without_action = RuntimeEventKind::Failed {
        kind: RuntimeFailureKind::Protocol,
        usage: Usage::default(),
        diagnostic: Some(DiagnosticSummary::from_bytes(
            b"controller_action_count_invalid",
            false,
        )),
    };
    let declined = LiveObservation {
        session: Some("session".to_owned()),
        usage: Some(Usage::default()),
        terminal: Some(closed_without_action.clone()),
        failure: Some(DECLINED_SUPERVISION_DETAIL.to_owned()),
        ..LiveObservation::default()
    };
    assert_eq!(classify(&declined), LiveOutcome::ModelDeclinedToAct);

    let failed_submission = LiveObservation {
        coordination: vec!["submit:failed".to_owned()],
        ..declined.clone()
    };
    assert_eq!(
        classify(&failed_submission),
        LiveOutcome::ProductDefect,
        "a submission the product failed to answer was read as a declined model"
    );

    let unestablished_termination = LiveObservation {
        failure: Some(format!(
            "{DECLINED_SUPERVISION_DETAIL}; managed_runtime_termination_unestablished: attempt"
        )),
        ..declined.clone()
    };
    assert_eq!(
        classify(&unestablished_termination),
        LiveOutcome::ProductDefect,
        "a run that could not establish its terminations was read as a declined model"
    );

    let submitted_but_uncommitted = LiveObservation {
        coordination: vec!["submit:completed".to_owned()],
        ..declined
    };
    assert_eq!(
        classify(&submitted_but_uncommitted),
        LiveOutcome::ProductDefect
    );

    let completed = LiveObservation {
        session: Some("session".to_owned()),
        usage: Some(Usage::default()),
        terminal: Some(RuntimeEventKind::Completed {
            usage: Usage::default(),
        }),
        coordination: vec!["submit:completed".to_owned()],
        candidate: Some("digest".to_owned()),
        failure: None,
    };
    assert_eq!(classify(&completed), LiveOutcome::ProductPathCompleted);
}

/// What a live managed attempt reported, kept whole so that the outcome is classified once, from
/// the complete stream, rather than from whichever event the reading loop happened to end on.
#[derive(Clone, Default)]
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
    // The record states the release that was measured for this attempt, not a constant, so the
    // evidence names the build that actually ran.
    let recorded_version = profile["profile"]["probe"]["version"]
        .as_str()
        .expect("the record names no measured version");
    assert!(
        recorded_version.ends_with("(Claude Code)") && at_least_the_floor(recorded_version),
        "the record names {recorded_version}, which this profile does not admit"
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
