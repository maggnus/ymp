//! Explicit, bounded native pilot. Normal builds and tests never run this example.
//! Usage: homogeneous_claude discover CLAUDE | run CLAUDE NEW_DIRECTORY MODEL [EFFORT]
//! A model without native effort support runs with no effort; none is translated.
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use ymp_domain::{
    Digest, Id, Prob,
    coordination::CommitmentTerms,
    identity::*,
    journal::{Capability, MethodKind},
    resources::*,
    task::*,
    verification::*,
    workspace::*,
};
use ymp_kernel::{
    intake::IntakeRequest,
    journal::{Journal, ParameterSchemas},
    ports::{checks::NativeDiscovery, execution::ClaudeParameters, planning::*, progress::*},
    session::{SessionDefinition, VisibleCheck},
};
use ymp_runtime::{
    backends::claude::ClaudeStreamJson,
    checks::retained::RetainedBytes,
    clock::{Clock, SystemClock},
    dispatcher::{Dispatcher, SessionPolicies, SessionStart, Tick},
    policies::*,
    workspace::direct::Direct,
};
use ymp_storage::journal::SqliteJournal;
const GOAL: &str = "Read input.json and write sorted.json containing the numbers sorted ascending, preserving duplicates. The output must be exactly [1,2,2,3] followed by one newline. Only sorted.json may be changed. The supplied owner criteria are sufficient; do not add criteria. Use one WorkItem. Follow each role's structured response schema without Markdown fences.";
const SESSION: &str = "homogeneous-claude-elementary";
fn id<T>(s: &str) -> Id<T> {
    Id::new(s).unwrap()
}
fn real(n: f64) -> Real {
    Real::new(n).unwrap()
}
fn policies() -> SessionPolicies {
    SessionPolicies {
        readiness: Box::new(ymp_runtime::readiness::StaticDependencyProbe::new().unwrap()),
        intake: Box::new(intake::NoQuestions::new().unwrap()),
        method: Box::new(method::FixedMethod::new(MethodKind::SoloWithVerifier).unwrap()),
        planner: Box::new(planner::AsNeededDecomposition::new().unwrap()),
        contributions: Box::new(
            contribution::FixedWorkflow::new(ContributionParameters {
                expected: real(16000.0),
                p90: real(16000.0),
                p_success: Prob::new(0.5).unwrap(),
                delta_belief: real(0.1),
            })
            .unwrap(),
        ),
        belief: Box::new(belief::LikelihoodRatioTable::standard().unwrap()),
        credit: Box::new(credit::ConfirmedOnly::new().unwrap()),
        monitor: Box::new(
            progress::EvidenceDelta::new(MonitorParameters {
                epsilon: real(0.001),
                stall_limit: 3,
            })
            .unwrap(),
        ),
        diagnosis: Box::new(
            diagnosis::RuleBasedDiagnoser::new(DiagnosisParameters {
                p_min: Prob::new(0.1).unwrap(),
                mutation_threshold: Prob::new(0.8).unwrap(),
            })
            .unwrap(),
        ),
        escalation: Box::new(
            escalation::DiagnosisFirstLadder::new(EscalationParameters {
                max_uses: 1,
                max_cost: real(16000.0),
                timeout_ms: 60000,
            })
            .unwrap(),
        ),
        award: Arc::new(
            award::FirstOffer::with_commitment_terms(CommitmentTerms {
                lease_duration: 480000,
                renewal_duration: 480000,
                renew_on: BTreeSet::new(),
                renewals: 0,
                release_delta: real(1.0),
            })
            .unwrap(),
        ),
        cost: Arc::new(
            resources::PriceWeighted::new(PriceWeightedParameters {
                // Calibrated from run 02: native thinking makes output vary widely.
                expected_input: 10000,
                expected_output: 1000,
                p90_factor: real(2.0),
            })
            .unwrap(),
        ),
        resources: Box::new(
            resources::PurposeBounded::new(PurposeBoundedParameters {
                max_cost: real(30000.0),
                // A producer's lease is bounded by this timeout and is discharged
                // only by acceptance; with 60000 it expired while verification ran.
                timeout: 240000,
                native_turns: 1,
                output_chars: 8000,
                report_call_cost: real(15000.0),
            })
            .unwrap(),
        ),
        context: Box::new(context::CriteriaProjection::new(4).unwrap()),
        reviewer: Box::new(reviewer::AnyNonProducer::new().unwrap()),
        narrative: Box::new(narrative::DeterministicReport::new().unwrap()),
        audit: Box::new(claim_audit::EvidenceClassRules::new().unwrap()),
    }
}

fn request(discovery: Discovery, model: String, effort: Option<String>, now: u64) -> SessionStart {
    let criterion = Criterion {
        id: id("sorted-json"),
        text: "sorted.json is exactly [1,2,2,3] followed by one newline".into(),
        kind: CriterionKind::NewBehavior,
        weight: real(1.0),
        required: true,
        origin: CriterionOrigin::User,
        needs_class: BTreeSet::new(),
    };
    let preservation = Criterion {
        id: id("input-preserved"),
        text: "input.json remains exactly the original input".into(),
        kind: CriterionKind::Preserve,
        weight: real(1.0),
        required: true,
        origin: CriterionOrigin::User,
        needs_class: BTreeSet::new(),
    };
    SessionStart {
        session: id(SESSION),
        task: IntakeRequest { task: Task { id: id("sort-json"), goal: Goal { request: GOAL.into(), assumptions: vec![], clarifications: vec![] }, contract: id("sorting-contract"), constraints: Constraints { budget: real(250000.0), verification_reserve: real(60000.0), deadline: Some(now + 480000), pins: Pins { team_size: Some(2), roster: Some(BTreeSet::from([id("claude-a"), id("claude-b")])), models: Some(BTreeSet::from([model.clone()])), efforts: effort.clone().map(|effort| BTreeSet::from([effort])) }, allowed: BTreeSet::from([Capability::ReadFiles, Capability::WriteFiles]), parallel_limit: 1, attempt_limit: 2, max_members: 2 } }, criteria: vec![criterion.clone(), preservation.clone()] },
        facts: RegistryFacts { agents: ["claude-a", "claude-b"].into_iter().map(|name| Agent { id: id(name), name: name.into(), provider: discovery.provider.id.clone(), defaults: ProfileSettings { model: Some(model.clone()), effort: effort.clone() }, instructions: "Use the host-provided role context and exact structured response contract. Be concise. Use only the mediated file tools. Preserve attribution; do not claim checks you did not observe.".into(), enabled: true }).collect(), discoveries: vec![discovery], dependencies: vec![] },
        definition: SessionDefinition { workspace: id("pilot-workspace"), offer_window_ms: 5000, checks: vec![VisibleCheck { criterion: criterion.reference().unwrap(), spec: CheckSpec::ExactBytes { path: WorkspacePath::new("sorted.json").unwrap(), digest: Digest::of(b"[1,2,2,3]\n") } }, VisibleCheck { criterion: preservation.reference().unwrap(), spec: CheckSpec::ExactBytes { path: WorkspacePath::new("input.json").unwrap(), digest: Digest::of(b"[3,2,1,2]\n") } }], rules: AssessmentRules { mutation_threshold: Prob::new(0.8).unwrap() } },
        pricebook: PriceBook { version: "pilot-relative-token-weights-not-currency-v1".into(), rates: vec![], fallback: Rates::fallback() }, unknown_usage: UnknownUsage::Stop,
    }
}
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(args.len() == 2 && args[0] == "discover"
        || matches!(args.len(), 4 | 5) && args[0] == "run")
    {
        return Err(
            "Usage: homogeneous_claude discover CLAUDE | run CLAUDE NEW_DIRECTORY MODEL [EFFORT]"
                .into(),
        );
    }
    let clock = Arc::new(SystemClock);
    let backend = Arc::new(ClaudeStreamJson::new(
        id("claude"),
        ClaudeParameters {
            // The native tool is installed as a link to its current release.
            executable: PathBuf::from(&args[1]).canonicalize()?,
            source: DiscoverySource::Native,
            connect_timeout_ms: 30000,
            frame_bytes: 1_048_576,
            round_trips: 8,
        },
        clock.now()?,
    )?);
    let executable = backend.configuration().executable.clone();
    let discovery = backend.discover()?;
    if args[0] == "discover" {
        println!("{}", serde_json::to_string_pretty(&discovery)?);
        return Ok(());
    }
    let effort = args.get(4).cloned();
    let offered = discovery.offerings.iter().any(|o| {
        o.model.as_deref() == Some(args[3].as_str())
            && effort
                .as_ref()
                .is_none_or(|effort| o.efforts.as_ref().is_some_and(|e| e.contains(effort)))
    });
    if !offered {
        return Err("Model and effort must appear together in current native discovery".into());
    }
    let root = PathBuf::from(&args[2]);
    // Never adopt or erase an existing directory, database or user workspace.
    std::fs::create_dir(&root)?;
    let work = root.join("workspace");
    std::fs::create_dir(&work)?;
    std::fs::write(work.join("input.json"), b"[3,2,1,2]\n")?;
    std::fs::write(work.join("sorted.json"), b"[]\n")?;
    std::fs::write(
        root.join("discovery.json"),
        serde_json::to_vec_pretty(&discovery)?,
    )?;
    let now = clock.now()?;
    let mut start = request(discovery, args[3].clone(), effort.clone(), now);
    start
        .facts
        .dependencies
        .push(ymp_runtime::readiness::StaticDependencyProbe::capture(
            id("claude"),
            None,
            &executable,
            DependencyKind::Executable,
            now,
        )?);
    let selected = policies();
    std::fs::write(
        root.join("experiment.json"),
        serde_json::to_vec_pretty(
            &serde_json::json!({"purpose":"Homogeneous light Anthropic team elementary fixed-workflow pilot", "driver_digest":Digest::of(include_bytes!("homogeneous_claude.rs")), "definition":start.definition, "input":"[3,2,1,2]\n", "expected":"[1,2,2,3]\n", "goal":GOAL, "requested_model":args[3], "requested_effort":effort, "comparative_token_efficiency":"unknown; no comparative usage evidence", "constraints":start.task.task.constraints, "policies":selected.selections(), "pricebook":start.pricebook, "started_at_ms":now, "watchdog_ms":480000, "cleanup_ms":30000, "unknown_usage":"Stop"}),
        )?,
    )?;
    let journal = Arc::new(SqliteJournal::open(
        root.join("journal.sqlite"),
        ParameterSchemas::default(),
    )?);
    let content = Arc::new(journal.content_store());
    let mut session = Dispatcher::start(
        journal.clone(),
        content,
        start,
        selected,
        backend,
        Arc::new(Direct::open(&work, CaptureLimits::default())?),
        Arc::new(RetainedBytes),
        clock.clone(),
    )?;
    let begun = Instant::now();
    let mut stopped = None;
    let mut failure = None;
    let mut last = 0;
    loop {
        if begun.elapsed() >= Duration::from_secs(480) && stopped.is_none() {
            session
                .application()
                .interrupt(&session.owner(), clock.now()?)?;
            stopped = Some(Instant::now());
            failure = Some("Pilot wall-clock limit reached".to_string());
        }
        if stopped.is_some_and(|at: Instant| at.elapsed() >= Duration::from_secs(30)) {
            break;
        }
        match session.tick() {
            Ok(Tick::Delivered(_)) => break,
            Ok(Tick::NeedsUser | Tick::Observed) => {
                failure = Some(
                    "Pilot reached a user decision; no answer or continuation is fabricated".into(),
                );
                session
                    .application()
                    .interrupt(&session.owner(), clock.now()?)?;
                stopped.get_or_insert_with(Instant::now);
            }
            Ok(Tick::Waiting { until }) => std::thread::sleep(Duration::from_millis(
                until.saturating_sub(clock.now()?).clamp(1, 50),
            )),
            Ok(Tick::Advanced) => {}
            Err(error) => {
                failure = Some(format!("{}: {}", error.code, error.message));
                session
                    .application()
                    .interrupt(&session.owner(), clock.now()?)?;
                stopped.get_or_insert_with(Instant::now);
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        let view = session.view()?;
        if view.revision() != last {
            last = view.revision();
            eprintln!(
                "revision={last} status={:?} invocations={}",
                view.status(),
                view.execution().invocations().len()
            );
        }
    }
    let view = session.view()?;
    std::fs::write(root.join("view.json"), serde_json::to_vec_pretty(&view)?)?;
    let bytes = std::fs::read(work.join("sorted.json"))?;
    let input_unchanged = std::fs::read(work.join("input.json"))? == b"[3,2,1,2]\n";
    let summary = serde_json::json!({"status":view.status(), "outcome":view.finalization().delivered.as_ref().map(|r| &r.outcome), "elapsed_ms":begun.elapsed().as_millis(), "calls":view.execution().invocations().len(), "sorted_bytes_match":bytes == b"[1,2,2,3]\n", "input_unchanged":input_unchanged, "failure":failure, "revision":view.revision()});
    std::fs::write(
        root.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    // The durable journal is primary evidence, including unknown effects/usage.
    let _ = journal.read(&id(SESSION))?;
    Ok(())
}
