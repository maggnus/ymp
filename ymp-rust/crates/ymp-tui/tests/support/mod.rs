//! Shared harness: build an interface over a domain run and render deterministic buffers.
//!
//! Each test binary links the whole module and uses part of it, so unused helpers are expected
//! here rather than a sign of dead code.

#![allow(dead_code)]
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ymp_runtime_api::Readiness;
use ymp_runtime_registry::{Engine, ModelCatalog, ModelSource};
use ymp_tui::app::{self, Action};
use ymp_tui::journal::Model;
use ymp_tui::projection::{ContractFacts, Environment, Projection, VerifierFacts};
use ymp_tui::runtimes::{EngineFacts, ProfileFacts, Report};
use ymp_tui::scenario::{self, Run};
use ymp_tui::state::{App, PageKind};
use ymp_tui::{theme, ui};

/// The two sizes the contract names.
pub const SIZES: [(u16, u16); 2] = [(80, 24), (120, 40)];

pub fn environment() -> Environment {
    Environment {
        version: "0.1.0".into(),
        project: "checkout".into(),
        project_path: PathBuf::from("/tmp/checkout"),
        data_root: PathBuf::from("/tmp/checkout/.ymp-data"),
        assurance_profile: ymp_tui::projection::ASSURANCE_PROFILE.to_owned(),
        assurance_limit: ymp_tui::projection::ASSURANCE_LIMIT.to_owned(),
    }
}

pub fn contract(verified: bool) -> ContractFacts {
    if !verified {
        return ContractFacts::refused(
            "contract-1".into(),
            PathBuf::from("/tmp/checkout/source"),
            "keep the replay path idempotent".into(),
            "no run started — the request states no acceptance condition — a verifier that \
             decides whether a candidate is accepted"
                .into(),
        );
    }
    ContractFacts {
        contract_id: "contract-1".into(),
        contract_digest: scenario::digest(0xc0),
        source: PathBuf::from("/tmp/checkout/source"),
        prompt: "keep the replay path idempotent".into(),
        verifier: Some(VerifierFacts {
            program: PathBuf::from("/tmp/checkout/verify.sh"),
            oracle_digest: scenario::digest(0x0a),
            negative_control: PathBuf::from("/tmp/checkout/negative-control"),
            wall_time_ms: 60_000,
        }),
        budget: Some(ymp_domain::Budget::new(1, 1)),
        run_id: Some("run-000000000000".into()),
        blocked: None,
        previously_authorized: false,
    }
}

/// An engine the registry admits, with the model list a measurement of it would have recorded.
fn engine(engine: Engine, names: Vec<String>) -> EngineFacts {
    EngineFacts {
        engine,
        enabled: true,
        disabled_reason: None,
        models: ModelCatalog {
            source: match names.is_empty() {
                true => ModelSource::Unmeasured,
                false => ModelSource::Measured,
            },
            measured_for_version: (!names.is_empty()).then(|| "0.0.0".to_owned()),
            note: None,
            names,
        },
    }
}

pub fn report() -> Report {
    Report {
        profiles: vec![
            ProfileFacts {
                name: "fake".into(),
                runtime: "in-process".into(),
                model_route: None,
                executable: "ymp-internal-fake".into(),
                version: Some("0.1.0".into()),
                readiness: Readiness::Ready,
                detail: "deterministic in-process runtime".into(),
                // The fixture runtime is not an engine: it has no executable, no credential and
                // no models, so the registry holds no row for it.
                registry: None,
            },
            ProfileFacts {
                name: "codex".into(),
                runtime: "codex".into(),
                model_route: Some("test/model-route".into()),
                executable: "/nonexistent/codex".into(),
                version: None,
                readiness: Readiness::NotInstalled,
                detail: "executable not found".into(),
                registry: Some(engine(Engine::Codex, Vec::new())),
            },
            // Exactly one managed profile is ready, which is the host on which a run has one
            // route and no choice to make. The other is unusable, so the same report is also the
            // mixed-readiness state the contract names.
            ProfileFacts {
                name: "claude-code".into(),
                runtime: "claude-code".into(),
                model_route: Some("test/claude-route".into()),
                executable: "/nonexistent/claude".into(),
                version: Some("0.0.0".into()),
                readiness: Readiness::Ready,
                detail: "fixture profile".into(),
                registry: Some(engine(
                    Engine::ClaudeCode,
                    vec!["test/claude-route".to_owned()],
                )),
            },
        ],
    }
}

/// The projection the interface would build over this run, with the routing the session settles.
fn projection_of(run: Option<&Run>, contracts: Vec<ContractFacts>) -> (Model, Projection) {
    let mut model = Model::cold(environment(), contracts);
    if let Some(run) = run {
        model.absorb(&run.state, &run.events);
    }
    let mut projection = model.projection(Some(&report()));
    let (route, note) = ymp_tui::attempt::routing_facts(None, Some(&report()));
    projection.route = route;
    projection.route_note = note;
    (model, projection)
}

/// Build the interface over a run, with the runtime report already delivered.
pub fn app_for(run: Option<&Run>, contracts: Vec<ContractFacts>) -> App {
    App::new(projection_of(run, contracts).1)
}

/// Rebuild the projection the way the event loop does, so a describe selection survives.
pub fn rebuild(app: &mut App, run: Option<&Run>, contracts: Vec<ContractFacts>) {
    let (model, mut projection) = projection_of(run, contracts);
    if let Some(index) = app.describe_index
        && let Some(page) = model.describe_candidate(index)
    {
        projection.pages.push((PageKind::Describe, page));
    }
    app.adopt(projection);
}

/// The rendered buffer as plain text, one string per row: colour carries no information here.
pub fn buffer(app: &App, width: u16, height: u16) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal
        .draw(|frame| ui::render(frame, app, &theme::UNICODE))
        .expect("draw");
    let buffer = terminal.backend().buffer().clone();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect()
}

pub fn screen(app: &App, width: u16, height: u16) -> String {
    buffer(app, width, height).join("\n")
}

/// Press one key and return the action the loop would have executed.
pub fn press(app: &mut App, code: KeyCode, height: u16) -> Option<Action> {
    app::handle_key(app, KeyEvent::new(code, KeyModifiers::NONE), height)
}

/// Type a run of characters.
pub fn type_text(app: &mut App, text: &str, height: u16) {
    for character in text.chars() {
        press(app, KeyCode::Char(character), height);
    }
}

/// Open a page through the `/` palette, exactly as an operator would.
pub fn open_command(app: &mut App, name: &str, height: u16) -> Option<Action> {
    press(app, KeyCode::Char('/'), height);
    type_text(app, name, height);
    press(app, KeyCode::Enter, height)
}

/// Every Rust source file of this crate, at any depth under `src`, keyed by its path relative
/// to `src`. The walk is recursive: a screen placed in a subdirectory is not out of reach of the
/// inventory checks.
pub fn crate_sources() -> Vec<(String, String)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    collect_sources(&root, &mut files);
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).expect("readable source");
            let name = path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .to_string_lossy()
                .into_owned();
            (name, text)
        })
        .collect()
}

fn collect_sources(directory: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).expect("crate sources") {
        let path = entry.expect("readable directory entry").path();
        if path.is_dir() {
            collect_sources(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}
