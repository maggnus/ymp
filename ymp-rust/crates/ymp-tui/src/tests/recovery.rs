//! Owner control of a loaded session from the team page, against the real owner API.
//!
//! Each test builds temporary application homes and projects. Native agents are the runtime's
//! offline protocol fixture and the other agents are compiled scripted backends; no model is
//! contacted. Owner work leaves the state layer as a request, is performed by the function the
//! event loop uses, and its reply is taken back by the state layer.

use super::*;
use crate::control::{self, Read, Request};
use crate::control_page;
use crate::state::Confirm;
use crate::views::ItemKind;
use anyhow::bail;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, Semaphore};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use ymp_core::{
    ExecutionBackendIdentity, InvocationState, Limits, LocalEffectScope, OwnerTeamAction,
    ProviderConfig, ProviderKind, RecoveryInspectionCommand, RecoveryStage, RecoveryWaitReason,
    ResourceLimits, SessionTrace, WorkspaceAccess,
};
use ymp_providers::{ExecutionBackend, ExecutionFuture, ProviderEvent, TurnRequest, TurnResult};
use ymp_runtime::Engine;

static CLOCK: AtomicU64 = AtomicU64::new(1);

/// A moment after every earlier one, so each read the state layer wants is admitted at once.
fn later() -> Instant {
    Instant::now() + Duration::from_secs(CLOCK.fetch_add(1, Ordering::SeqCst))
}

/// Perform what the state layer asked of the owner API, as the event loop does, until it asks for
/// nothing more. Returns the actions the replies produced.
async fn settle(app: &mut App) -> Vec<Action> {
    let mut actions = Vec::new();
    for _ in 0..16 {
        let Some(request) = app
            .control
            .take_request()
            .or_else(|| app.control.take_read(later()))
        else {
            break;
        };
        let reply = perform(request, &app.store, &app.config).await;
        actions.extend(app.control_reply(reply));
    }
    actions
}

/// Perform one request on a task of its own, as the event loop does.
async fn perform(request: Request, store: &Store, config: &Config) -> control::Reply {
    tokio::spawn(control::perform(
        request,
        store.clone(),
        config.clone(),
        CancellationToken::new(),
    ))
    .await
    .unwrap()
}

/// Run a session on a task of its own, as the event loop does.
async fn run_on_task(
    engine: Engine,
    path: &Path,
    prompt: &str,
    resume: Option<&str>,
) -> ymp_runtime::RunOutcome {
    let (path, prompt, resume) = (
        path.to_path_buf(),
        prompt.to_owned(),
        resume.map(str::to_owned),
    );
    tokio::spawn(async move { engine.run(&path, &prompt, resume.as_deref()).await })
        .await
        .unwrap()
        .unwrap()
}

/// [`settle`] with engines that execute through a compiled scripted backend.
async fn settle_with(app: &mut App, engine: &impl Fn() -> Engine) -> Vec<Action> {
    let mut actions = Vec::new();
    for _ in 0..16 {
        let Some(request) = app
            .control
            .take_request()
            .or_else(|| app.control.take_read(later()))
        else {
            break;
        };
        let reply = tokio::spawn(control::perform_with(request, app.store.clone(), engine()))
            .await
            .unwrap();
        actions.extend(app.control_reply(reply));
    }
    actions
}

async fn open_team(app: &mut App, session: &str) {
    app.load_session(session).unwrap();
    app.command("/team", 100);
    settle(app).await;
    assert!(
        app.control.ready().is_some(),
        "the session team was not read: {:?}",
        app.control.read
    );
}

fn select(app: &mut App, key: &str) {
    let width = app.page_width(100);
    let found = app
        .page(width)
        .items
        .iter()
        .position(|item| item.kind == ItemKind::Row && item.key == key);
    match found {
        Some(index) => app.page_selected = index,
        None => panic!("no row {key}: {:?}", row_keys(app, width)),
    }
}

fn press(app: &mut App, code: KeyCode) -> Vec<Action> {
    app.on_key(key(code), 100)
}

fn offered(app: &App) -> Vec<(String, String, Option<String>)> {
    match &app.overlay {
        Some(Overlay::Choose { options, .. }) => options
            .iter()
            .map(|o| (o.label.clone(), o.detail.clone(), o.reason.clone()))
            .collect(),
        other => panic!("no chooser is open: {other:?}"),
    }
}

fn offer(app: &App, matches: impl Fn(&str) -> bool) -> (usize, String, Option<String>) {
    offered(app)
        .into_iter()
        .enumerate()
        .find(|(_, (label, _, _))| matches(label))
        .map(|(index, (_, detail, reason))| (index, detail, reason))
        .unwrap_or_else(|| panic!("not offered: {:?}", offered(app)))
}

/// Choose the option labelled exactly `label`.
fn choose(app: &mut App, label: &str) -> Vec<Action> {
    let (index, _, _) = offer(app, |name| name == label);
    pick_index(app, index)
}

/// Choose the agent whose label ends with its ID.
fn choose_agent(app: &mut App, id: &str) -> Vec<Action> {
    let (index, _, _) = offer(app, |name| name == id || name.ends_with(&format!("  {id}")));
    pick_index(app, index)
}

fn pick_index(app: &mut App, index: usize) -> Vec<Action> {
    if let Some(Overlay::Choose { selected, .. }) = &mut app.overlay {
        *selected = index;
    }
    press(app, KeyCode::Enter)
}

fn reason_of(app: &App, label: &str) -> Option<String> {
    offer(app, |name| name == label).2
}

fn question(app: &App) -> String {
    match &app.overlay {
        Some(Overlay::Confirm { question, .. }) => question.clone(),
        other => panic!("no confirmation is open: {other:?}"),
    }
}

fn last_notice(app: &App) -> String {
    app.notices
        .last()
        .map(|notice| notice.text.clone())
        .unwrap_or_default()
}

fn last_failure(app: &App) -> String {
    let notice = app.notices.last().expect("nothing was reported");
    assert!(notice.failure, "not reported as a failure: {}", notice.text);
    notice.text.clone()
}

fn stage_now(store: &Store, stage: &RecoveryStage) -> RecoveryStage {
    store
        .recovery_stages(&stage.session_id)
        .unwrap()
        .into_iter()
        .find(|s| s.id == stage.id)
        .unwrap()
}

fn stage_key(stage: &RecoveryStage) -> String {
    format!("{}{}", control_page::STAGE_PREFIX, stage.id)
}

fn decisions(trace: &SessionTrace, kind: &str) -> usize {
    trace.decisions.iter().filter(|d| d.kind == kind).count()
}

/// Replay a session's public historical records into `store` without its recovery stages, as an
/// old session holds them. No safety, resolution or scope record is written.
fn replay_history(
    store: &Store,
    original: &SessionTrace,
    path: &Path,
    prompt: &str,
    unknown: bool,
) {
    let mut session = original.session.clone();
    session.project_id = store.project(path).unwrap().id;
    session.turns_used = 0;
    store
        .create_session(&session, original.policy.as_ref().unwrap())
        .unwrap();
    store
        .put_value(
            &format!("team_state:v1:{}", session.id),
            &serde_json::to_value(&original.team_state).unwrap(),
        )
        .unwrap();
    store
        .put_value(&format!("prompt:{}", session.id), &json!(prompt))
        .unwrap();
    for invocation in &original.invocations {
        let mut assignment = original
            .assignments
            .iter()
            .find(|a| a.id == invocation.assignment_id)
            .unwrap()
            .clone();
        assignment.state = InvocationState::Running;
        assignment.ended_at = None;
        assignment.grant_ids.clear();
        let mut opened = invocation.clone();
        opened.state = InvocationState::Running;
        opened.ended_at = None;
        opened.usage = None;
        opened.terminal_reason = None;
        store.begin_invocation(&assignment, &opened).unwrap();
        let terminal = if unknown && assignment.purpose == "review_plan" {
            InvocationState::Interrupted
        } else {
            invocation.state
        };
        store
            .finish_invocation(
                &session.id,
                &opened.id,
                terminal,
                Some(invocation.state.as_str()),
            )
            .unwrap();
    }
    for record in original.decisions.iter().filter(|d| {
        matches!(
            d.kind.as_str(),
            "plan_proposed"
                | "workspace_access_acquired"
                | "workspace_access_admitted"
                | "workspace_access_released"
        )
    }) {
        store.record_decision(record).unwrap();
    }
    let plan = original
        .decisions
        .iter()
        .find_map(|d| {
            (d.kind == "plan_proposed")
                .then_some(d.links.plan_proposal.as_ref())
                .flatten()
        })
        .unwrap();
    store
        .invocation_message(
            &session.id,
            &plan.producer_invocation_id,
            "plan",
            &serde_json::to_string(&plan.plan).unwrap(),
        )
        .unwrap();
}

/// An old session whose independent plan review failed on a native ACP agent that could write,
/// with a Codex-protocol agent available that took no part in it.
struct Native {
    _temp: TempDir,
    path: PathBuf,
    log: PathBuf,
    store: Store,
    config: Config,
    session: String,
    stage: RecoveryStage,
}

impl Native {
    async fn new(turns: usize, unknown: bool) -> Self {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("work");
        std::fs::create_dir(&path).unwrap();
        let log = temp.path().join("protocol.jsonl");
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../ymp-runtime/tests/fixtures/fresh_plan_review.py");
        let mut config: Config = serde_json::from_value(json!({
            "version":1,
            "providers":[
                {"id":"author-native","kind":"codex","command":"python3","args":[script,"codex","author",log,"native-v1"]},
                {"id":"failed-native","kind":"acp","command":"python3","args":[script,"acp","failed",log,"native-v1"]},
                {"id":"fresh-native","kind":"codex","command":"python3","args":[script,"codex","fresh",log,"native-v1"]}
            ],
            "agents":[{"id":"author","name":"Author","provider":"author-native"},{"id":"failed","name":"Failed","provider":"failed-native"},{"id":"fresh","name":"Fresh","provider":"fresh-native"}],
            "team":["author","failed"],"team_constraints":{"fixed_roster":["author","failed"]},"limits":{"parallel":1,"turns":turns,"turn_timeout_secs":5,"attempts":3,"resources":{"startup_invocations":10}}
        }))
        .unwrap();
        let engine = |store: Store, config: &Config| {
            let (events, _) = mpsc::unbounded_channel();
            let mut engine =
                Engine::new(store, config.clone(), events, CancellationToken::new()).unwrap();
            engine.use_memory = false;
            engine
        };
        let source = Store::open(&temp.path().join("source")).unwrap();
        let first = run_on_task(
            engine(source.clone(), &config),
            &path,
            "Inspect the directory",
            None,
        )
        .await;
        assert_ne!(first.session.status, "completed", "{}", first.summary);
        let original = source.trace(&first.session.id).unwrap();
        let store = Store::open(&temp.path().join("legacy")).unwrap();
        replay_history(&store, &original, &path, "Inspect the directory", unknown);
        ymp_providers::discovery::refresh_catalog(
            &mut config,
            &temp.path().join("native-catalog"),
            &path,
            &PathBuf::new(),
            ymp_providers::discovery::ScanOptions {
                provider: Some("fresh-native".into()),
                timeout_secs: 5,
            },
            CancellationToken::new(),
        )
        .await
        .unwrap();
        let model = config
            .native_provider_snapshot("fresh-native")
            .unwrap()
            .catalog
            .as_ref()
            .unwrap()
            .default_model
            .clone()
            .unwrap();
        config
            .agents
            .iter_mut()
            .find(|a| a.id == "fresh")
            .unwrap()
            .model = Some(model);
        let before = std::fs::read(&log).unwrap();
        let session = original.session.id.clone();
        let resumed = run_on_task(engine(store.clone(), &config), &path, "", Some(&session)).await;
        assert_ne!(resumed.session.status, "completed");
        assert_eq!(std::fs::read(&log).unwrap(), before);
        let stage = store
            .recovery_stages(&session)
            .unwrap()
            .into_iter()
            .find(|s| s.purpose == "review_plan")
            .unwrap();
        Self {
            _temp: temp,
            path,
            log,
            store,
            config,
            session,
            stage,
        }
    }

    fn requests(&self) -> Vec<Value> {
        std::fs::read_to_string(&self.log)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    fn app(&self) -> App {
        App::new(self.store.clone(), self.config.clone(), self.path.clone())
    }
}

#[tokio::test]
async fn a_saved_plan_is_reviewed_again_and_continued_with_current_files_from_the_team_page() {
    let f = Native::new(40, false).await;
    let mut app = f.app();
    let preferred = app.config.team.clone();
    open_team(&mut app, &f.session).await;
    let stage = stage_key(&f.stage);
    assert_eq!(
        cell_of_key(&mut app, 100, &stage, "REASON").trim(),
        control_page::reason(&f.stage)
    );

    // The failed agent is replaced through the owner API; the starting preferences stay.
    select(&mut app, &format!("{}failed", control_page::MEMBER_PREFIX));
    press(&mut app, KeyCode::Enter);
    choose(&mut app, "Replace");
    choose_agent(&mut app, "fresh");
    let asked = question(&app);
    assert!(
        asked.contains("Replace failed with fresh") && asked.contains("fixed roster"),
        "{asked}"
    );
    press(&mut app, KeyCode::Char('y'));
    settle(&mut app).await;
    assert!(
        last_notice(&app).contains("Replaced failed with fresh"),
        "{}",
        last_notice(&app)
    );
    let members = app
        .control
        .ready()
        .unwrap()
        .view
        .effective
        .current_members
        .clone();
    assert!(members.contains(&"fresh".to_owned()) && !members.contains(&"failed".to_owned()));
    assert_eq!(app.config.team, preferred);

    // A new independent review of the exact saved plan. Neither its author nor the failed agent
    // can be chosen, and choosing starts no production.
    let before = f.requests().len();
    select(&mut app, &stage);
    press(&mut app, KeyCode::Enter);
    choose(&mut app, "Review saved plan again");
    assert!(
        offered(&app)
            .iter()
            .all(|(label, _, reason)| reason.is_some()
                || !(label.ends_with("author") || label.ends_with("failed"))),
        "the plan author or the failed agent can be chosen: {:?}",
        offered(&app)
    );
    choose_agent(&mut app, "fresh");
    assert!(settle(&mut app).await.is_empty());
    let reviewed = last_notice(&app);
    assert!(
        reviewed.contains("approved by fresh") && reviewed.contains("No task was started"),
        "{reviewed}"
    );
    let requests = f.requests();
    let review = &requests[before..];
    assert_eq!(
        review
            .iter()
            .filter(|r| r["purpose"] == "review_plan")
            .count(),
        1
    );
    assert!(review
        .iter()
        .any(|r| r["method"] == "thread/start" && r["sandbox"] == "read-only"));
    assert!(!review.iter().any(|r| r["purpose"] == "plan"
        || r["purpose"] == "execute"
        || r["method"] == "thread/resume"));
    assert!(f.store.tasks(&f.session).unwrap().is_empty());

    // Inspection is an offered action. This legacy call has no enforced effect scope, so the
    // runtime refuses with its cause; nothing is resolved and the exact command is kept.
    select(&mut app, &stage);
    press(&mut app, KeyCode::Enter);
    assert_eq!(reason_of(&app, "Inspect effects"), None);
    choose(&mut app, "Inspect effects");
    settle(&mut app).await;
    assert!(last_failure(&app).contains("The effects were not inspected"));
    assert!(stage_now(&f.store, &f.stage).effect_resolution.is_none());
    assert_eq!(f.requests(), requests);
    let width = app.page_width(100);
    assert!(row_keys(&mut app, width).contains(&control_page::RETRY_KEY.to_owned()));

    // The files change between the presented context and the confirmation: nothing is recorded
    // and no run is asked for.
    select(&mut app, &stage);
    press(&mut app, KeyCode::Enter);
    choose(&mut app, "Continue with current files");
    assert!(settle(&mut app).await.is_empty());
    let presented = question(&app);
    assert!(
        presented.contains("Directory:")
            && presented.contains("failed (")
            && presented.contains("stay unverified"),
        "{presented}"
    );
    for markers in [theme::UNICODE, theme::ASCII] {
        app.theme = theme::resolved(theme::DEFAULT_THEME, markers);
        for (width, height) in [(100, 32), (60, 16), (48, 12)] {
            let screen = draw(&mut app, width, height);
            assert!(
                screen.contains("Directory:")
                    && screen.contains("yes")
                    && screen.contains("starts the run"),
                "the confirmation lost its subject or keys at {width}x{height}:\n{screen}"
            );
        }
    }
    std::fs::write(
        f.path.join("new-file.txt"),
        "Changed while the owner was reading",
    )
    .unwrap();
    press(&mut app, KeyCode::Char('y'));
    assert!(
        settle(&mut app).await.is_empty(),
        "a refused authorization asked for a run"
    );
    assert!(last_failure(&app).contains("Continuation was not authorized"));
    assert!(f
        .store
        .current_files_authorizations(&f.session)
        .unwrap()
        .is_empty());

    // A new explicit choice presents the new context. One confirmation authorizes, then asks for
    // the ordinary run of the same session in its recorded directory.
    select(&mut app, &stage);
    press(&mut app, KeyCode::Enter);
    choose(&mut app, "Continue with current files");
    settle(&mut app).await;
    press(&mut app, KeyCode::Char('y'));
    let exact = app.control.pending().cloned().unwrap();
    let actions = settle(&mut app).await;
    let directory = f.path.canonicalize().unwrap();
    assert_eq!(
        actions,
        vec![Action::ContinueRun {
            session: f.session.clone(),
            directory: directory.clone(),
        }]
    );
    let receipt = app.control.continuation.clone().unwrap();
    let again = perform(exact, &f.store, &app.config).await;
    match again.outcome {
        control::Outcome::Authorize(Ok(repeated)) => assert_eq!(repeated, receipt),
        other => panic!("the same command did not return its recorded outcome: {other:?}"),
    }
    assert_eq!(
        f.store
            .current_files_authorizations(&f.session)
            .unwrap()
            .len(),
        1
    );

    // A run that does not start keeps the authorization, and starting it asks nothing again.
    app.continuation_not_started(&f.session, "a run is already active");
    assert!(last_failure(&app).contains("the authorization is not requested again"));
    select(&mut app, control_page::CONTINUE_KEY);
    assert_eq!(press(&mut app, KeyCode::Enter), actions);
    assert!(app.control.pending().is_none());
    select(&mut app, &stage);
    press(&mut app, KeyCode::Enter);
    assert!(offer(&app, |name| name == "Continue with current files")
        .1
        .contains("Already authorized"));
    assert_eq!(choose(&mut app, "Continue with current files"), actions);
    assert!(app.control.pending().is_none() && app.overlay.is_none());

    // The ordinary run, as the event loop starts it.
    app.continuation_started(&f.session);
    assert!(app.control.continuation.is_none());
    let (events, _received) = mpsc::unbounded_channel();
    let engine = Engine::new(
        f.store.clone(),
        app.config.clone(),
        events,
        CancellationToken::new(),
    )
    .unwrap();
    let outcome = run_on_task(engine, &directory, "", Some(&f.session)).await;
    assert_eq!(outcome.session.id, f.session);
    assert_eq!(outcome.session.status, "completed", "{}", outcome.summary);
    assert_eq!(
        std::fs::read_to_string(f.path.join("current-result.txt")).unwrap(),
        "New work from current files; historical effects remain unknown.\n"
    );
    let trace = f.store.trace(&f.session).unwrap();
    assert!(!trace.tasks.is_empty());
    assert!(trace.tasks.iter().all(|t| t.state == TaskState::Accepted));
    assert_eq!(decisions(&trace, "plan_proposed"), 1);
    assert_eq!(decisions(&trace, "plan_review"), 1);
    assert_eq!(decisions(&trace, "owner_current_files_authorized"), 1);
    let after = stage_now(&f.store, &f.stage);
    assert_eq!(after.failures, f.stage.failures);
    assert!(after.effect_resolution.is_none() && !after.manual_permit);
    let continued = &f.requests()[requests.len()..];
    assert!(continued
        .iter()
        .all(|r| r["actor"] != "failed" && r["purpose"] != "plan"));
    assert_eq!(
        continued
            .iter()
            .filter(|r| r["purpose"] == "execute")
            .count(),
        1
    );
    assert_eq!(app.config.team, preferred);
    assert!(!f.store.home.join("config.toml").exists());
}

#[tokio::test]
async fn stage_holds_outlast_team_changes_and_unavailable_actions_name_their_cause() {
    let f = Native::new(40, false).await;
    let stage = stage_key(&f.stage);

    // Without an agent independent of the plan, no reviewer can be chosen.
    let mut narrow = f.config.clone();
    narrow
        .agents
        .retain(|agent| agent.provider != "fresh-native");
    let mut app = App::new(f.store.clone(), narrow, f.path.clone());
    open_team(&mut app, &f.session).await;
    select(&mut app, &stage);
    press(&mut app, KeyCode::Enter);
    choose(&mut app, "Review saved plan again");
    assert!(last_failure(&app).contains("No eligible independent reviewer"));
    assert!(app.overlay.is_none() && app.control.pending().is_none());

    // A stage the owner holds stays held through a membership change.
    let mut app = f.app();
    open_team(&mut app, &f.session).await;
    let calls = f.requests();
    select(&mut app, &stage);
    press(&mut app, KeyCode::Enter);
    choose(&mut app, "Wait");
    typed(&mut app, "Owner checks the directory");
    press(&mut app, KeyCode::Enter);
    settle(&mut app).await;
    let held = stage_now(&f.store, &f.stage);
    assert_eq!(held.wait_reason, Some(RecoveryWaitReason::OwnerWait));
    assert_eq!(
        cell_of_key(&mut app, 100, &stage, "REASON").trim(),
        "held by the owner"
    );
    select(&mut app, "fresh");
    press(&mut app, KeyCode::Char(' '));
    assert!(question(&app).contains("Add fresh"));
    press(&mut app, KeyCode::Char('y'));
    settle(&mut app).await;
    assert!(
        last_notice(&app).contains("Added fresh"),
        "{}",
        last_notice(&app)
    );
    assert_eq!(stage_now(&f.store, &f.stage).wait_reason, held.wait_reason);

    let hold = "This stage is held. Release the hold first.";
    select(&mut app, &stage);
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        reason_of(&app, "Review saved plan again").as_deref(),
        Some(hold)
    );
    assert_eq!(
        reason_of(&app, "Continue with current files").as_deref(),
        Some(hold)
    );
    choose(&mut app, "Review saved plan again");
    assert!(matches!(app.overlay, Some(Overlay::Choose { .. })));
    assert_eq!(last_failure(&app), hold);

    // The chooser keeps its subject, the reason and its keys in normal and small terminals.
    let (review, _, _) = offer(&app, |name| name == "Review saved plan again");
    if let Some(Overlay::Choose { selected, .. }) = &mut app.overlay {
        *selected = review;
    }
    for markers in [theme::UNICODE, theme::ASCII] {
        app.theme = theme::resolved(theme::DEFAULT_THEME, markers);
        for (width, height) in [(120, 32), (80, 24), (60, 16), (48, 12)] {
            let screen = draw(&mut app, width, height);
            assert!(
                screen.contains(&format!("{} Review saved plan again", markers.selection))
                    && screen.contains("This stage is held.")
                    && screen.contains("choose"),
                "the chooser lost its selection, reason or keys at {width}x{height}:\n{screen}"
            );
        }
    }

    choose(&mut app, "Release hold");
    settle(&mut app).await;
    let released = stage_now(&f.store, &f.stage);
    assert_eq!(released.wait_reason, f.stage.wait_reason);
    assert_eq!(released.status, f.stage.status);
    assert_eq!(released.condition, f.stage.condition);
    assert_eq!(released.failures, f.stage.failures);
    assert!(!released.manual_permit && released.effect_resolution.is_none());
    assert_eq!(f.requests(), calls, "holding or releasing called a model");

    // Unverified termination: continuing names the missing evidence, and inspection is not offered.
    let unknown = Native::new(40, true).await;
    let mut app = unknown.app();
    open_team(&mut app, &unknown.session).await;
    select(&mut app, &stage_key(&unknown.stage));
    press(&mut app, KeyCode::Enter);
    assert_eq!(
        reason_of(&app, "Continue with current files").as_deref(),
        Some("Earlier execution has not verifiably ended.")
    );
    assert!(!offered(&app)
        .iter()
        .any(|(label, _, _)| label == "Inspect effects"));

    // An exhausted budget refuses the review with its cause; the same command sent again issues
    // no call.
    let poor = Native::new(4, false).await;
    let mut app = poor.app();
    open_team(&mut app, &poor.session).await;
    app.command("/team add fresh", 100);
    press(&mut app, KeyCode::Char('y'));
    settle(&mut app).await;
    let calls = poor.requests();
    select(&mut app, &stage_key(&poor.stage));
    press(&mut app, KeyCode::Enter);
    choose(&mut app, "Review saved plan again");
    choose_agent(&mut app, "fresh");
    settle(&mut app).await;
    assert!(last_failure(&app).contains("The saved plan was not reviewed"));
    assert!(poor.store.tasks(&poor.session).unwrap().is_empty());
    assert_eq!(poor.requests(), calls);
    select(&mut app, control_page::RETRY_KEY);
    press(&mut app, KeyCode::Enter);
    settle(&mut app).await;
    assert!(last_failure(&app).contains("The saved plan was not reviewed"));
    assert_eq!(poor.requests(), calls);
}

#[tokio::test]
async fn owner_work_stays_with_its_session_and_a_native_review_stops_with_the_window() {
    let f = Native::new(40, false).await;
    let other = Session {
        id: new_id(),
        project_id: f.store.project(&f.path).unwrap().id,
        title: "Unrelated".into(),
        status: "completed".into(),
        created_at: now(),
        team: Vec::new(),
        turns_used: 0,
    };
    f.store.save_session(&other).unwrap();
    let mut app = f.app();
    open_team(&mut app, &f.session).await;
    app.command("/team add fresh", 100);
    press(&mut app, KeyCode::Char('y'));
    settle(&mut app).await;
    assert!(
        last_notice(&app).contains("Added fresh"),
        "{}",
        last_notice(&app)
    );

    // A read of this session that arrives after another session is displayed is not shown there.
    app.control.want_read();
    let stale = app.control.take_read(later()).unwrap();
    app.load_session(&other.id).unwrap();
    app.command("/team", 100);
    let reply = perform(stale, &f.store, &app.config).await;
    assert!(app.control_reply(reply).is_empty());
    assert!(
        !matches!(app.control.read, Read::Ready(_)),
        "the team of session {} was shown for session {}",
        f.session,
        other.id
    );
    settle(&mut app).await;
    assert!(matches!(app.control.read, Read::NotInitialized));
    open_team(&mut app, &f.session).await;

    // A review for this session finishes while another session is displayed.
    select(&mut app, &stage_key(&f.stage));
    press(&mut app, KeyCode::Enter);
    choose(&mut app, "Review saved plan again");
    choose_agent(&mut app, "fresh");
    let review = app.control.take_request().unwrap();
    assert_eq!(review.session(), f.session);
    app.load_session(&other.id).unwrap();
    app.command("/team", 100);
    settle(&mut app).await;
    assert!(matches!(app.control.read, Read::NotInitialized));
    let reply = perform(review, &f.store, &app.config).await;
    assert!(app.control_reply(reply).is_empty());
    assert!(
        last_notice(&app).contains(&format!("for session {}", text::short_id(&f.session))),
        "{}",
        last_notice(&app)
    );
    assert_eq!(app.session.as_deref(), Some(other.id.as_str()));
    assert!(app.overlay.is_none() && matches!(app.control.read, Read::NotInitialized));
    assert!(stage_now(&f.store, &f.stage).fresh_plan_review.is_some());

    // A session with no initialized team is not edited through preferences.
    let preferred = app.config.team.clone();
    select(&mut app, "fresh");
    press(&mut app, KeyCode::Char(' '));
    assert!(last_failure(&app).contains("no initialized team"));
    assert_eq!(app.config.team, preferred);

    // Current files read for the first session are not presented on the second.
    open_team(&mut app, &f.session).await;
    select(&mut app, &stage_key(&f.stage));
    press(&mut app, KeyCode::Enter);
    choose(&mut app, "Continue with current files");
    let read = app.control.take_request().unwrap();
    app.load_session(&other.id).unwrap();
    let reply = perform(read, &f.store, &app.config).await;
    assert!(app.control_reply(reply).is_empty());
    assert!(app.overlay.is_none());
    assert!(last_notice(&app).contains("no longer on display"));
    assert!(f
        .store
        .current_files_authorizations(&f.session)
        .unwrap()
        .is_empty());

    // An authorization confirmed for the first session is recorded, but its run does not start
    // while another session is displayed; the receipt is kept to start it later.
    open_team(&mut app, &f.session).await;
    select(&mut app, &stage_key(&f.stage));
    press(&mut app, KeyCode::Enter);
    choose(&mut app, "Continue with current files");
    settle(&mut app).await;
    let Some(Overlay::Confirm {
        target: Confirm::Owner { request, .. },
        ..
    }) = app.overlay.take()
    else {
        panic!("the current files were not presented for confirmation");
    };
    app.load_session(&other.id).unwrap();
    let reply = perform(*request, &f.store, &app.config).await;
    let actions = app.control_reply(reply);
    assert!(
        actions.is_empty(),
        "a run started for a session that is not displayed: {actions:?}"
    );
    assert_eq!(
        f.store
            .current_files_authorizations(&f.session)
            .unwrap()
            .len(),
        1
    );
    assert!(app
        .control
        .continuation
        .as_ref()
        .is_some_and(|receipt| receipt.command.context.session_id == f.session));
    assert_eq!(app.session.as_deref(), Some(other.id.as_str()));

    // A native review in progress: runs are refused beside it, /stop asks to cancel it, and
    // leaving cancels it and waits for it within the bound.
    let g = Native::new(40, false).await;
    std::fs::write(
        g.log.with_file_name("control.json"),
        r#"{"gate_review": true}"#,
    )
    .unwrap();
    let mut app = g.app();
    open_team(&mut app, &g.session).await;
    app.command("/team add fresh", 100);
    press(&mut app, KeyCode::Char('y'));
    settle(&mut app).await;
    select(&mut app, &stage_key(&g.stage));
    press(&mut app, KeyCode::Enter);
    choose(&mut app, "Review saved plan again");
    choose_agent(&mut app, "fresh");
    let review = app.control.take_request().unwrap();
    let cancel = CancellationToken::new();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let owner = tokio::spawn({
        let (store, config, cancel) = (g.store.clone(), app.config.clone(), cancel.clone());
        async move {
            let _ = sender.send(control::perform(review, store, config, cancel).await);
        }
    });
    let started = g.log.with_file_name("review-started");
    tokio::time::timeout(Duration::from_secs(10), async {
        while !started.exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the native review did not start");
    assert!(app
        .run_refusal()
        .is_some_and(|refusal| refusal.contains("using its directory")));
    assert_eq!(submit(&mut app, "/stop"), vec![Action::Cancel]);
    let (_events, mut received) = mpsc::unbounded_channel();
    let departure = exit::stop_and_wait(
        exit::Stopping {
            cancel: &CancellationToken::new(),
            scan_cancel: &CancellationToken::new(),
            owner_cancel: &cancel,
            running: None::<JoinHandle<()>>,
            scanning: None::<JoinHandle<()>>,
            owner: vec![owner],
        },
        &mut received,
        app.session.clone(),
        Duration::from_secs(10),
    )
    .await;
    assert!(
        !departure.unfinished,
        "the native review did not stop within the bound"
    );
    app.control_reply(receiver.await.unwrap());
    assert!(last_failure(&app).contains("The saved plan was not reviewed"));
    assert!(stage_now(&g.store, &g.stage).fresh_plan_review.is_none());
    let reviews = |requests: Vec<Value>| {
        requests
            .iter()
            .filter(|r| r["purpose"] == "review_plan")
            .count()
    };
    let issued = reviews(g.requests());
    std::fs::write(g.log.with_file_name("review-release"), "").unwrap();
    select(&mut app, control_page::RETRY_KEY);
    press(&mut app, KeyCode::Enter);
    settle(&mut app).await;
    assert_eq!(
        reviews(g.requests()),
        issued,
        "the same command issued a second native review"
    );
    assert!(stage_now(&g.store, &g.stage).fresh_plan_review.is_none());
}

struct Gate {
    purpose: &'static str,
    started: Semaphore,
    release: Semaphore,
}

/// A compiled backend with scripted answers, an optional gate, and one kind of failure.
struct Script {
    failure_purpose: &'static str,
    gate: Option<Arc<Gate>>,
    writes: bool,
    failures: AtomicUsize,
    calls: Mutex<Vec<TurnRequest>>,
}

impl Script {
    fn new(failure_purpose: &'static str, failures: usize, gate: Option<Arc<Gate>>) -> Arc<Self> {
        Arc::new(Self {
            failure_purpose,
            gate,
            writes: false,
            failures: AtomicUsize::new(failures),
            calls: Mutex::new(Vec::new()),
        })
    }
}

impl ExecutionBackend for Script {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "fixture.tui-recovery".into(),
            version: "1".into(),
        }
    }
    fn workspace_access(&self, _: &TurnRequest) -> WorkspaceAccess {
        if self.writes {
            WorkspaceAccess::WriteAll
        } else {
            WorkspaceAccess::ReadAll
        }
    }
    fn execute(
        &self,
        request: TurnRequest,
        _: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(request.clone());
            if let Some(gate) = &self.gate {
                if request.purpose == gate.purpose {
                    gate.started.add_permits(1);
                    gate.release.acquire().await.unwrap().forget();
                }
            }
            if request.purpose == self.failure_purpose
                && self
                    .failures
                    .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                    .is_ok()
            {
                bail!("Unknown scripted failure");
            }
            let mut text = match request.purpose.as_str() {
                "plan" => {
                    json!({"summary":"Saved proposal", "tasks":[{"title":"Read", "description":"Return findings", "access":"read_only", "competence":"analysis", "difficulty":"simple", "dependencies":[], "checks":[]}]})
                }
                "review_plan" | "review" | "final_review" => {
                    json!({"approved":true,"reason":"Independent evidence inspected"})
                }
                "execute" => json!("Findings from this invocation"),
                "synthesis" => json!("Accepted findings"),
                purpose => bail!("Unexpected purpose {purpose}"),
            };
            if request.purpose == "plan"
                && self.gate.as_ref().is_some_and(|g| g.purpose == "execute")
            {
                text["tasks"].as_array_mut().unwrap().push(json!({"title":"Second read","description":"Use the first finding","access":"read_only","competence":"analysis","difficulty":"simple","dependencies":[0],"checks":[]}));
            }
            Ok(TurnResult {
                text: match text {
                    Value::String(text) => text,
                    other => other.to_string(),
                },
                session_id: new_id(),
                usage: None,
            })
        })
    }
}

/// A compiled backend whose plan review makes exactly one local write, which it declares.
struct LocalEffect {
    script: Arc<Script>,
}

impl ExecutionBackend for LocalEffect {
    fn identity(&self) -> ExecutionBackendIdentity {
        ExecutionBackendIdentity {
            id: "fixture.tui-local-effect".into(),
            version: "1".into(),
        }
    }
    fn workspace_access(&self, request: &TurnRequest) -> WorkspaceAccess {
        if request.purpose == "review_plan" {
            WorkspaceAccess::WriteAll
        } else {
            WorkspaceAccess::ReadAll
        }
    }
    fn local_effect_scope(&self, request: &TurnRequest) -> Option<LocalEffectScope> {
        (request.purpose == "review_plan").then(|| LocalEffectScope {
            files: vec!["review-side-effect.txt".into()],
        })
    }
    fn execute(
        &self,
        request: TurnRequest,
        events: mpsc::UnboundedSender<ProviderEvent>,
    ) -> ExecutionFuture<'_> {
        Box::pin(async move {
            if request.purpose == "review_plan" {
                std::fs::write(
                    request.cwd.join("review-side-effect.txt"),
                    "known local effect",
                )?;
            }
            self.script.execute(request, events).await
        })
    }
}

fn scripted_config(agents: &[&str], team: &[&str]) -> Config {
    Config {
        providers: vec![ProviderConfig {
            id: "offline".into(),
            kind: ProviderKind::Mock,
            command: "internal".into(),
            args: vec![],
            env_refs: Default::default(),
            enabled: true,
        }],
        agents: agents
            .iter()
            .map(|id| AgentProfile {
                id: (*id).into(),
                name: (*id).into(),
                provider: "offline".into(),
                model: None,
                instructions: "Scripted fixture".into(),
                enabled: true,
            })
            .collect(),
        team: team.iter().map(|id| (*id).to_owned()).collect(),
        limits: Limits {
            parallel: 1,
            turns: 80,
            turn_timeout_secs: 10,
            attempts: 3,
            resources: Some(ResourceLimits {
                startup_invocations: 20,
                ..Default::default()
            }),
        },
        ..Default::default()
    }
}

fn scripted_engine(store: Store, config: &Config, backend: Arc<dyn ExecutionBackend>) -> Engine {
    let (events, _) = mpsc::unbounded_channel();
    let mut engine = Engine::new(store, config.clone(), events, CancellationToken::new())
        .unwrap()
        .with_execution_backend(backend)
        .unwrap();
    engine.use_memory = false;
    engine
}

#[tokio::test]
async fn a_busy_member_is_replaced_during_a_run_without_touching_starting_preferences() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("work");
    std::fs::create_dir(&path).unwrap();
    let store = Store::open(&temp.path().join("state")).unwrap();
    let gate = Arc::new(Gate {
        purpose: "execute",
        started: Semaphore::new(0),
        release: Semaphore::new(0),
    });
    let script = Arc::new(Script {
        failure_purpose: "never",
        gate: Some(gate.clone()),
        writes: true,
        failures: AtomicUsize::new(0),
        calls: Mutex::new(Vec::new()),
    });
    let mut config = scripted_config(
        &["author", "reviewer", "reserve", "newcomer"],
        &["author", "reviewer"],
    );
    config.limits.parallel = 2;
    config.team_constraints.fixed_roster = Some(vec!["author".into(), "reserve".into()]);
    let running = scripted_engine(store.clone(), &config, script.clone());
    let session = new_id();
    let background = tokio::spawn({
        let (path, session) = (path.clone(), session.clone());
        async move { running.run_identified(&path, "Inspect", &session).await }
    });
    tokio::time::timeout(Duration::from_secs(10), gate.started.acquire())
        .await
        .unwrap()
        .unwrap()
        .forget();
    let trace = store.trace(&session).unwrap();
    let producer = trace
        .assignments
        .iter()
        .find(|a| a.purpose == "execute" && a.state == InvocationState::Running)
        .unwrap()
        .clone();

    let mut app = App::new(store.clone(), config.clone(), path.clone());
    app.load_session(&session).unwrap();
    app.active = true;
    app.command("/team", 100);
    settle(&mut app).await;
    let member = format!("{}{}", control_page::MEMBER_PREFIX, producer.agent_id);
    assert_ne!(cell_of_key(&mut app, 100, &member, "WORK").trim(), "idle");

    // A typed membership change for the loaded session asks about the session, not preferences.
    app.command("/team add newcomer", 100);
    match &app.overlay {
        Some(Overlay::Confirm {
            question,
            target: Confirm::Owner { request, .. },
        }) => {
            assert!(question.contains("fixed roster"), "{question}");
            assert!(matches!(
                request.as_ref(),
                Request::Team(command) if command.session_id == session
                    && command.action == OwnerTeamAction::Add { agent_id: "newcomer".into() }
            ));
        }
        other => panic!("the typed change was not a session command: {other:?}"),
    }
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.config.team, config.team);

    select(&mut app, &member);
    press(&mut app, KeyCode::Enter);
    choose(&mut app, "Replace");
    choose_agent(&mut app, "newcomer");
    assert!(question(&app).contains("keeps its current work"));
    press(&mut app, KeyCode::Char('y'));
    settle(&mut app).await;
    assert!(
        last_notice(&app).contains(&format!("{} is leaving", producer.agent_id)),
        "{}",
        last_notice(&app)
    );
    assert_eq!(
        cell_of_key(&mut app, 100, &member, "STATE").trim(),
        "leaving"
    );
    assert_eq!(
        cell_of_key(&mut app, 100, "member:newcomer", "STATE").trim(),
        "joining"
    );
    let producing = trace
        .invocations
        .iter()
        .find(|i| i.assignment_id == producer.id)
        .unwrap();
    assert_eq!(
        store.invocation(&session, &producing.id).unwrap().state,
        InvocationState::Running,
        "the replacement interrupted the busy member's work"
    );
    assert_eq!(app.config.team, config.team);
    assert!(!store.home.join("config.toml").exists());

    // Owner work that holds the directory keeps runs and branch switches out.
    app.control
        .submit(Request::Inspect(RecoveryInspectionCommand {
            session_id: session.clone(),
            stage_id: "never-sent".into(),
            expected_revision: 0,
            command_id: new_id(),
        }))
        .unwrap();
    assert!(app
        .run_refusal()
        .is_some_and(|refusal| refusal.contains("using its directory")));
    app.active = false;
    app.overlay = Some(Overlay::Confirm {
        question: String::new(),
        target: Confirm::SwitchBranch {
            root: path.clone(),
            branch: "main".into(),
            expected_head: None,
        },
    });
    press(&mut app, KeyCode::Char('y'));
    assert!(last_failure(&app).contains("An owner action is using the session directory"));
    assert!(app.git.switching().is_none());
    assert!(app.control.withdraw().is_some());
    app.active = true;

    let calls = script.calls.lock().unwrap().len();
    gate.release.add_permits(2);
    let result = tokio::time::timeout(Duration::from_secs(10), background)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(result.session.status, "completed", "{}", result.summary);
    {
        let recorded = script.calls.lock().unwrap();
        assert!(recorded[calls..]
            .iter()
            .all(|r| r.profile.id != producer.agent_id));
        assert!(recorded
            .iter()
            .any(|r| r.purpose == "execute" && r.profile.id == "newcomer"));
    }
    app.active = false;
    app.event(UiEvent::Finished {
        session_id: session.clone(),
        status: result.session.status.clone(),
    });
    settle(&mut app).await;
    assert_eq!(
        cell_of_key(&mut app, 100, "member:newcomer", "STATE").trim(),
        "member"
    );
    assert_eq!(app.config.team, config.team);

    // With no session loaded, the same command edits the starting preferences, as before.
    let home = TempDir::new().unwrap();
    let mut plain = App::new(Store::open(home.path()).unwrap(), config.clone(), path);
    plain.command("/team add reserve", 100);
    assert!(plain.config.team.contains(&"reserve".to_owned()));
    assert!(plain.overlay.is_none() && plain.control.pending().is_none());
}

#[tokio::test]
async fn recorded_effects_are_inspected_and_the_stage_continued_as_separate_owner_actions() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("work");
    std::fs::create_dir(&path).unwrap();
    let config = scripted_config(&["author", "reviewer", "reserve"], &["author", "reviewer"]);
    let source = Store::open(&temp.path().join("source")).unwrap();
    let failing = Script::new("review_plan", 1, None);
    let initial = run_on_task(
        scripted_engine(
            source.clone(),
            &config,
            Arc::new(LocalEffect {
                script: failing.clone(),
            }),
        ),
        &path,
        "Inspect this directory",
        None,
    )
    .await;
    assert_ne!(initial.session.status, "completed");
    let original = source.trace(&initial.session.id).unwrap();
    let store = Store::open(&temp.path().join("legacy")).unwrap();
    replay_history(&store, &original, &path, "Inspect this directory", false);
    let script = Script::new("never", 0, None);
    let engine = || scripted_engine(store.clone(), &config, script.clone());
    let session = original.session.id.clone();
    assert_ne!(
        run_on_task(engine(), &path, "", Some(&session))
            .await
            .session
            .status,
        "completed"
    );
    let stage = store
        .recovery_stages(&session)
        .unwrap()
        .into_iter()
        .find(|s| s.purpose == "review_plan")
        .unwrap();

    let mut app = App::new(store.clone(), config.clone(), path.clone());
    app.load_session(&session).unwrap();
    app.command("/team", 100);
    settle_with(&mut app, &engine).await;
    select(&mut app, &stage_key(&stage));
    press(&mut app, KeyCode::Enter);
    assert_eq!(reason_of(&app, "Inspect effects"), None);
    assert!(!offered(&app)
        .iter()
        .any(|(label, _, _)| label == "Continue"));
    choose(&mut app, "Inspect effects");
    settle_with(&mut app, &engine).await;
    assert!(
        last_notice(&app).contains("were inspected")
            && last_notice(&app).contains("nothing was started"),
        "{}",
        last_notice(&app)
    );
    let inspected = stage_now(&store, &stage);
    assert!(inspected.effect_resolution.is_some());
    assert_eq!(inspected.failures, stage.failures);
    assert!(!inspected.manual_permit);
    assert_eq!(
        cell_of_key(&mut app, 100, &stage_key(&stage), "REASON").trim(),
        control_page::reason(&inspected)
    );
    assert!(store.tasks(&session).unwrap().is_empty());
    assert_eq!(
        script
            .calls
            .lock()
            .unwrap()
            .iter()
            .map(|r| (r.purpose.clone(), r.read_only))
            .collect::<Vec<_>>(),
        vec![("review".to_owned(), true)]
    );

    // Continuing is a separate recorded decision, and the run a separate explicit action.
    select(&mut app, &stage_key(&stage));
    press(&mut app, KeyCode::Enter);
    choose(&mut app, "Continue");
    assert!(settle_with(&mut app, &engine).await.is_empty());
    assert!(last_notice(&app).contains("Continuation of the stage is recorded"));
    assert_eq!(
        submit(&mut app, "/resume"),
        vec![Action::Resume {
            session: session.clone()
        }]
    );
    let result = run_on_task(engine(), &path, "", Some(&session)).await;
    assert_eq!(result.session.status, "completed", "{}", result.summary);
    let calls = script.calls.lock().unwrap();
    assert!(!calls.iter().any(|r| r.purpose == "plan"));
    assert_eq!(
        calls.iter().filter(|r| r.purpose == "review_plan").count(),
        1
    );
    assert!(calls
        .iter()
        .filter(|r| r.purpose == "review_plan")
        .all(|r| r.read_only));
    assert_eq!(stage_now(&store, &stage).failures, stage.failures);
}
