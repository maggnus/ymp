//! Acceptance: every action the terminal interface offers is a command of the same executable,
//! with the same authority checks, the same confirmations and the same journal.
//!
//! Four halves, one per acceptance clause.
//!
//! * The inventory is compared in both directions. The interface's side is read from the
//!   interface itself — its page list, the palette a live projection offers, the two typed
//!   confirmations, and everything its event loop can be asked to perform — never from a copy of
//!   it kept here. The command's side is read from the parser. Neither side may hold a surplus.
//! * The journal is compared byte for byte. The same request is carried to a run and then
//!   cancelled twice: once through the interface's own session, once through the commands of the
//!   built executable. The two stores must hold the same events, in the same order, under the
//!   same command identifiers.
//! * The approved-contract check is exercised from the command side: a request that states no
//!   acceptance condition starts nothing and exits non-zero.
//! * The typed confirmation is exercised from the command side: absent and wrong confirmations
//!   commit nothing and exit non-zero.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Output};

use clap::{CommandFactory, Parser};
use tempfile::TempDir;
use ymp_cli::surface::{PageName, PublicCommand};
use ymp_cli::{Cli, Command};
use ymp_domain::RunStatus;
use ymp_tui::app::Action;
use ymp_tui::journal::Model;
use ymp_tui::projection::{ContractFacts, Environment};
use ymp_tui::state::{App, Command as InterfaceCommand, ConfirmAction, PageKind, Surface};
use ymp_tui::theme::Markers;
use ymp_tui::{Session, decisions, scenario, ui};

/// The correspondence this card asserts: one action of the interface, one command that performs
/// it, stated as the exact invocation the parser must accept.
const CORRESPONDENCE: &[(&str, &[&str])] = &[
    (
        "request",
        &[
            "ymp",
            "request",
            "--prompt",
            "keep the replay path idempotent",
        ],
    ),
    ("authorize", &["ymp", "authorize", "contract-1"]),
    (
        "start-run",
        &["ymp", "start", "contract-1", "--confirm", "contract-1"],
    ),
    ("cancel-run", &["ymp", "cancel", "--confirm", "run-1"]),
    ("page:runtimes", &["ymp", "show", "runtimes"]),
    ("page:candidates", &["ymp", "show", "candidates"]),
    ("page:events", &["ymp", "show", "events"]),
    ("page:budgets", &["ymp", "show", "budgets"]),
    ("page:attempts", &["ymp", "show", "attempts"]),
    (
        "page:describe",
        &["ymp", "show", "describe", "--candidate", "0"],
    ),
];

/// The one action of the interface that acts on the interface rather than on the run: leaving
/// it. A command process leaves when it has finished its work, so no command mirrors it, and the
/// exclusion is held to exactly this one action.
const LIFECYCLE: [&str; 1] = ["quit"];

/// The namespace excluded from the comparison: the product's own machinery, which predates this
/// card and is not an operator capability. The exclusion is held to exactly this one namespace.
const NOT_AN_OPERATOR_SURFACE: [&str; 1] = ["internal"];

// ---------------------------------------------------------------------------
// The interface's own action vocabulary
// ---------------------------------------------------------------------------

/// The name of the action a palette entry performs.
///
/// The match is exhaustive: an entry added to the interface's palette fails to compile here
/// until this inventory names it, and it then has to be matched by a command below.
fn palette_action(command: InterfaceCommand) -> String {
    match command {
        InterfaceCommand::OpenPage(kind) => page_action(kind),
        InterfaceCommand::Authorize(_) => "authorize".to_owned(),
        InterfaceCommand::CancelRun => "cancel-run".to_owned(),
        InterfaceCommand::Quit => "quit".to_owned(),
    }
}

/// The name of the action a typed confirmation commits. Exhaustive for the same reason.
fn confirmed_action(action: &ConfirmAction) -> String {
    match action {
        ConfirmAction::StartRun { .. } => "start-run".to_owned(),
        ConfirmAction::CancelRun { .. } => "cancel-run".to_owned(),
    }
}

/// The name of the action the interface's event loop is asked to perform. Exhaustive for the
/// same reason.
fn performed_action(action: &Action) -> String {
    match action {
        Action::CancelRun => "cancel-run".to_owned(),
        Action::StartRun(_) => "start-run".to_owned(),
        Action::LocalTurn(_) => "request".to_owned(),
        // The interface rebuilds its projection when a candidate is opened; the surface that
        // opens is the describe page.
        Action::Rebuild => page_action(PageKind::Describe),
    }
}

fn page_action(kind: PageKind) -> String {
    format!("page:{}", kind.command_name())
}

/// Every action the interface offers, read from the interface.
fn interface_actions(root: &Path) -> BTreeSet<String> {
    let mut actions = BTreeSet::new();
    for kind in PageKind::ALL {
        actions.insert(page_action(kind));
    }

    // The palette over a projection that carries a contract and a live run, which is the state
    // in which the interface offers the most.
    let run = scenario::running();
    let mut model = Model::cold(Environment::detect(root), vec![drafted_facts()]);
    model.absorb(&run.state, &run.events);
    for item in model.projection(None).commands {
        actions.insert(palette_action(item.command));
    }

    // The two typed confirmations, built by the interface's own decision surfaces.
    let authorize = decisions::authorize(&drafted_facts(), model.environment(), None, None);
    let start = decisions::start_run(&authorize.action.expect("a startable contract"));
    let cancel = decisions::cancel_run(
        &model
            .projection(None)
            .run
            .expect("the scenario carries a run"),
    );
    for confirm in [start, cancel] {
        actions.insert(confirmed_action(&confirm.action));
    }

    // Everything the event loop can be asked to perform.
    for action in [
        Action::CancelRun,
        Action::StartRun("contract-1".to_owned()),
        Action::LocalTurn("keep the replay path idempotent".to_owned()),
        Action::Rebuild,
    ] {
        actions.insert(performed_action(&action));
    }

    actions
}

/// A drafted contract, as the projection carries one before anything is spent.
fn drafted_facts() -> ContractFacts {
    ContractFacts {
        contract_id: "contract-1".into(),
        contract_digest: "a".repeat(64),
        source: PathBuf::from("/tmp/source"),
        prompt: "keep the replay path idempotent".into(),
        verifier: Some(ymp_tui::projection::VerifierFacts {
            program: PathBuf::from("/tmp/verify.sh"),
            oracle_digest: "b".repeat(64),
            negative_control: PathBuf::from("/tmp/negative"),
            wall_time_ms: 60_000,
        }),
        budget: Some(ymp_domain::Budget::new(1, 1)),
        run_id: Some("run-aaaaaaaaaaaa".into()),
        blocked: None,
    }
}

// ---------------------------------------------------------------------------
// The command surface's own action vocabulary
// ---------------------------------------------------------------------------

/// The name of the action a command performs. Exhaustive: a command added to the public surface
/// fails to compile here until it is named, and it then has to match an interface action.
fn command_action(command: &PublicCommand) -> String {
    match command {
        PublicCommand::Request { .. } => "request".to_owned(),
        PublicCommand::Authorize { .. } => "authorize".to_owned(),
        PublicCommand::Start { .. } => "start-run".to_owned(),
        PublicCommand::Cancel { .. } => "cancel-run".to_owned(),
        PublicCommand::Show { page, .. } => page_action(page.kind()),
    }
}

#[test]
fn every_interface_action_is_a_command_and_neither_side_holds_a_surplus() {
    let root = TempDir::new().expect("temporary root");

    // Every stated invocation parses, and reaches the command it claims to reach.
    let mut commanded = BTreeSet::new();
    let mut named = BTreeSet::new();
    for (action, invocation) in CORRESPONDENCE {
        let cli = Cli::try_parse_from(*invocation)
            .unwrap_or_else(|error| panic!("{invocation:?} is not accepted: {error}"));
        let Some(Command::Public(command)) = cli.command else {
            panic!("{invocation:?} does not reach the public command surface");
        };
        assert_eq!(
            &command_action(&command),
            action,
            "{invocation:?} performs a different action"
        );
        commanded.insert((*action).to_owned());
        named.insert(invocation[1].to_owned());
    }

    let offered = interface_actions(root.path());
    let excluded: BTreeSet<String> = LIFECYCLE.iter().map(|name| (*name).to_owned()).collect();

    // Direction one: the only action of the interface without a command is the one that acts on
    // the interface itself. The comparison is an equality, so the exclusion cannot quietly grow.
    let uncovered: BTreeSet<String> = offered.difference(&commanded).cloned().collect();
    assert_eq!(
        uncovered, excluded,
        "these interface actions have no command"
    );

    // Direction two: the command surface adds nothing the interface lacks.
    let surplus: Vec<&String> = commanded.difference(&offered).collect();
    assert!(
        surplus.is_empty(),
        "these commands perform something the interface does not offer: {surplus:?}"
    );

    // Every public command of the parser is in the correspondence, so a command cannot be added
    // outside it. The internal namespace is the one deliberate exclusion.
    let parser = Cli::command();
    let public: Vec<String> = parser
        .get_subcommands()
        .map(|subcommand| subcommand.get_name().to_owned())
        .filter(|name| !NOT_AN_OPERATOR_SURFACE.contains(&name.as_str()))
        .collect();
    assert_eq!(
        public.iter().cloned().collect::<BTreeSet<_>>(),
        named,
        "a public command is outside the correspondence"
    );
    assert!(
        parser.find_subcommand("internal").is_some(),
        "the excluded namespace must exist for the exclusion to mean anything"
    );

    // Every page the command surface can name is in the correspondence too, so a page cannot be
    // added to the command surface without an interface page behind it.
    for page in [
        PageName::Runtimes,
        PageName::Candidates,
        PageName::Events,
        PageName::Budgets,
        PageName::Attempts,
        PageName::Describe,
    ] {
        assert!(
            commanded.contains(&page_action(page.kind())),
            "the command surface shows {:?} outside the correspondence",
            page.kind()
        );
    }
}

// ---------------------------------------------------------------------------
// The same journal
// ---------------------------------------------------------------------------

struct Fixture {
    _root: TempDir,
    source: PathBuf,
    verifier: PathBuf,
    negative_control: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().expect("temporary root");
        let source = root.path().join("source");
        let negative_control = root.path().join("negative-control");
        let verifier = root.path().join("verify.sh");
        fs::create_dir_all(&source).expect("source directory");
        fs::create_dir_all(&negative_control).expect("negative control directory");
        fs::write(source.join("input.txt"), b"before\n").expect("source file");
        fs::write(&verifier, b"#!/bin/sh\nexit 0\n").expect("verifier program");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&verifier).expect("metadata").permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(&verifier, permissions).expect("make executable");
        }
        Self {
            _root: root,
            source,
            verifier,
            negative_control,
        }
    }

    fn data_root(&self, name: &str) -> PathBuf {
        self._root.path().join(name)
    }

    /// The request, as the four answers the interface's draft asks for.
    fn answers(&self) -> [String; 4] {
        [
            "keep the replay path idempotent".to_owned(),
            self.source.display().to_string(),
            self.verifier.display().to_string(),
            self.negative_control.display().to_string(),
        ]
    }

    fn request_arguments(&self) -> Vec<String> {
        let answers = self.answers();
        vec![
            format!("--prompt={}", answers[0]),
            format!("--source={}", answers[1]),
            format!("--verifier={}", answers[2]),
            format!("--negative-control={}", answers[3]),
        ]
    }

    fn command(&self, data_root: &Path, arguments: &[String]) -> Output {
        Process::new(env!("CARGO_BIN_EXE_ymp"))
            .arg("--data-root")
            .arg(data_root)
            .args(arguments)
            .output()
            .expect("run the ymp executable")
    }
}

/// Carry the request to a started run and then cancel it, through the interface's own session.
fn through_the_interface(fixture: &Fixture, data_root: &Path) -> (String, String) {
    let mut session = Session::open(data_root, &[]);
    for answer in fixture.answers() {
        session.local_turn(answer);
    }
    let contract_id = session
        .projection(None)
        .contracts
        .first()
        .expect("the request produced a contract")
        .contract_id
        .clone();
    session.start_run(&contract_id);
    let run_id = session
        .projection(None)
        .run
        .expect("the interface started a run")
        .run_id
        .clone();
    session.cancel_run();
    (contract_id, run_id)
}

fn journal(data_root: &Path) -> String {
    fs::read_to_string(data_root.join("events.jsonl")).expect("committed journal")
}

#[test]
fn the_command_and_the_interface_commit_the_same_journal() {
    let fixture = Fixture::new();
    let interface_root = fixture.data_root("interface");
    let command_root = fixture.data_root("command");

    let (contract_id, run_id) = through_the_interface(&fixture, &interface_root);

    let mut start = fixture.request_arguments();
    start.insert(0, "start".to_owned());
    start.push(format!("--confirm={contract_id}"));
    let started = fixture.command(&command_root, &start);
    assert!(
        started.status.success(),
        "the command did not start the run: {}",
        String::from_utf8_lossy(&started.stderr)
    );

    let cancelled = fixture.command(
        &command_root,
        &["cancel".to_owned(), format!("--confirm={run_id}")],
    );
    assert!(
        cancelled.status.success(),
        "the command did not cancel the run: {}",
        String::from_utf8_lossy(&cancelled.stderr)
    );

    assert_eq!(
        journal(&command_root),
        journal(&interface_root),
        "the command and the interface committed different journals"
    );
}

// ---------------------------------------------------------------------------
// The approved-contract check
// ---------------------------------------------------------------------------

#[test]
fn a_command_that_states_no_acceptance_condition_starts_nothing() {
    let fixture = Fixture::new();
    let data_root = fixture.data_root("no-acceptance");
    let answers = fixture.answers();
    let refused = fixture.command(
        &data_root,
        &[
            "start".to_owned(),
            format!("--prompt={}", answers[0]),
            format!("--source={}", answers[1]),
            "--confirm=anything".to_owned(),
        ],
    );

    assert!(
        !refused.status.success(),
        "a run was started against a contract nothing could judge"
    );
    let reported = format!(
        "{}{}",
        String::from_utf8_lossy(&refused.stdout),
        String::from_utf8_lossy(&refused.stderr)
    );
    assert!(
        reported.contains("no run started — the request states no acceptance condition"),
        "the refusal does not name the missing part:\n{reported}"
    );
    assert!(
        !data_root.join("events.jsonl").exists(),
        "a refused request wrote a journal"
    );
}

// ---------------------------------------------------------------------------
// The typed confirmation
// ---------------------------------------------------------------------------

#[test]
fn starting_a_run_requires_the_contract_id_to_be_typed() {
    let fixture = Fixture::new();
    let identity = fixture.data_root("identity");
    let (contract_id, _) = through_the_interface(&fixture, &identity);

    let attempts: [Option<String>; 3] = [
        None,
        Some("wrong".to_owned()),
        Some(contract_id[..8].into()),
    ];
    for (index, confirmation) in attempts.iter().enumerate() {
        let data_root = fixture.data_root(&format!("start-{index}"));
        let mut arguments = fixture.request_arguments();
        arguments.insert(0, "start".to_owned());
        if let Some(typed) = &confirmation {
            arguments.push(format!("--confirm={typed}"));
        }
        let refused = fixture.command(&data_root, &arguments);
        assert!(
            !refused.status.success(),
            "a run started on the confirmation {confirmation:?}"
        );
        assert!(
            !data_root.join("events.jsonl").exists(),
            "the confirmation {confirmation:?} committed a journal"
        );
    }

    // The exact identifier is what commits it, and nothing less than the exact identifier.
    let data_root = fixture.data_root("start-exact");
    let mut arguments = fixture.request_arguments();
    arguments.insert(0, "start".to_owned());
    arguments.push(format!("--confirm={contract_id}"));
    assert!(
        fixture.command(&data_root, &arguments).status.success(),
        "the exact contract id did not start the run"
    );
}

#[test]
fn cancelling_a_run_requires_the_run_id_to_be_typed() {
    let fixture = Fixture::new();
    let (contract_id, run_id) = through_the_interface(&fixture, &fixture.data_root("cancel"));
    // The interface cancelled its own run, so a second store carries the live one. The same
    // request derives the same identifiers, so the run id above is the one this store holds.
    let live = fixture.data_root("cancel-live");
    let mut arguments = fixture.request_arguments();
    arguments.insert(0, "start".to_owned());
    arguments.push(format!("--confirm={contract_id}"));
    assert!(fixture.command(&live, &arguments).status.success());

    let attempts: [Option<String>; 3] = [None, Some("wrong".to_owned()), Some(run_id[..6].into())];
    for confirmation in &attempts {
        let mut arguments = vec!["cancel".to_owned()];
        if let Some(typed) = &confirmation {
            arguments.push(format!("--confirm={typed}"));
        }
        let refused = fixture.command(&live, &arguments);
        assert!(
            !refused.status.success(),
            "the run was cancelled on the confirmation {confirmation:?}"
        );
        assert_eq!(
            status(&live),
            RunStatus::Running,
            "the confirmation {confirmation:?} ended the run"
        );
    }

    assert!(
        fixture
            .command(&live, &["cancel".to_owned(), format!("--confirm={run_id}")])
            .status
            .success(),
        "the exact run id did not cancel the run"
    );
    assert_eq!(status(&live), RunStatus::Cancelled);
}

fn status(data_root: &Path) -> RunStatus {
    let session = Session::open(data_root, &[]);
    session
        .projection(None)
        .run
        .expect("the store holds a run")
        .status
}

// ---------------------------------------------------------------------------
// The same page
// ---------------------------------------------------------------------------

#[test]
fn a_command_prints_the_page_the_interface_lays_out() {
    let fixture = Fixture::new();
    let data_root = fixture.data_root("pages");
    through_the_interface(&fixture, &data_root);

    let printed = fixture.command(&data_root, &["show".to_owned(), "events".to_owned()]);
    assert!(printed.status.success());
    let printed = String::from_utf8_lossy(&printed.stdout).into_owned();

    let session = Session::open(&data_root, &[]);
    let mut app = App::new(session.projection(None));
    app.surface = Surface::Page(PageKind::Events);
    let drawn = ui::surface_spec(
        &app,
        ratatui::layout::Rect::new(0, 0, ymp_cli::surface::WIDTH, ymp_cli::surface::PAGE_HEIGHT),
        &Markers::detect(),
    );
    for line in drawn.body {
        let line = line.to_string();
        assert!(
            printed.contains(line.trim_end()),
            "the command did not print the line the interface draws:\n{line}"
        );
    }
}
