//! Acceptance: every action the terminal interface offers is a command of the same executable,
//! with the same authority checks, the same confirmations and the same journal.
//!
//! * The inventory is compared in both directions. The interface's side is read from the
//!   interface itself — its page list, the palette a live projection offers, the two typed
//!   confirmations, and everything its event loop can be asked to perform — never from a copy of
//!   it kept here. The command's side is read from the parser. Neither side may hold a surplus,
//!   and the namespace excluded from the comparison is pinned to what it held.
//! * The comparison is widened past those enumerations to the keyboard: every key, on every
//!   surface and every modal, either changes what is on screen or returns an action a command
//!   performs.
//! * An answer stays an answer. A stated value that begins with the command prefix, or carries an escape or
//!   a control character, must not reach a surface the command did not open — the reproduction
//!   that led to this check started an irreversible run from an `authorize` invocation.
//! * The journals are compared record by record. The same request is carried to a run and then
//!   cancelled twice: once through the interface's own session, once through the commands of the
//!   built executable. The two stores must hold the same events, in the same order, under the
//!   same command identifiers. What is set aside is the run identifier and the digest chain that
//!   follows it: a run is identified by its store as well as by its contract, so two stores hold
//!   two runs and are meant to say so. Each store is required to name its own run and no other.
//! * The approved-contract check is exercised from the command side: a request that states no
//!   acceptance condition starts nothing and exits non-zero.
//! * The typed confirmation is exercised from the command side: absent and wrong confirmations
//!   commit nothing and exit non-zero.
//!
//! What none of this covers: an interface action reachable other than by a key or the palette —
//! a mouse binding, or a future surface driven by something else — and the contents of the
//! internal namespace, which is pinned by name rather than compared with the interface.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as Process, Output};
use std::time::SystemTime;

use clap::{CommandFactory, Parser};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use tempfile::TempDir;
use ymp_cli::surface::{PageName, PoolCommand, ProviderCommand, PublicCommand, RuntimeCommand};
use ymp_cli::{Cli, Command};
use ymp_domain::RunStatus;
use ymp_runtime_registry::{Engine, PoolEntry, PoolName, ProviderFamily, RegistryAddress};
use ymp_tui::app::Action;
use ymp_tui::journal::Model;
use ymp_tui::projection::{ContractFacts, Environment};
use ymp_tui::state::{App, Command as InterfaceCommand, ConfirmAction, Modal, PageKind, Surface};
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
    ("attempt", &["ymp", "attempt", "--confirm", "run-1"]),
    ("cancel-run", &["ymp", "cancel", "--confirm", "run-1"]),
    ("export", &["ymp", "export"]),
    ("export-apply", &["ymp", "export", "--apply"]),
    (
        "runtime-enable",
        &["ymp", "runtime", "enable", "claude-code"],
    ),
    (
        "runtime-disable",
        &[
            "ymp",
            "runtime",
            "disable",
            "codex",
            "--reason",
            "usage limit until 2026-09-12",
        ],
    ),
    (
        "provider-enable",
        &["ymp", "provider", "enable", "anthropic"],
    ),
    (
        "provider-disable",
        &[
            "ymp",
            "provider",
            "disable",
            "openai",
            "--reason",
            "kept out of this experiment",
        ],
    ),
    (
        "provider-refresh",
        &["ymp", "provider", "refresh", "anthropic"],
    ),
    (
        "pool-permit",
        &["ymp", "pool", "permit", "default", "claude-opus-5"],
    ),
    (
        "pool-exclude",
        &["ymp", "pool", "exclude", "default", "claude-opus-5"],
    ),
    (
        "pool-set-capacity",
        &[
            "ymp",
            "pool",
            "set-capacity",
            "default",
            "--participants",
            "6",
        ],
    ),
    ("page:providers", &["ymp", "show", "providers"]),
    (
        "page:provider",
        &["ymp", "show", "provider", "--provider", "anthropic"],
    ),
    ("page:models", &["ymp", "show", "models"]),
    ("page:pools", &["ymp", "show", "pools"]),
    ("page:pool", &["ymp", "show", "pool", "--pool", "default"]),
    ("page:runtimes", &["ymp", "show", "runtimes"]),
    ("page:candidates", &["ymp", "show", "candidates"]),
    ("page:events", &["ymp", "show", "events"]),
    ("page:budgets", &["ymp", "show", "budgets"]),
    ("page:attempts", &["ymp", "show", "attempts"]),
    ("page:commitments", &["ymp", "show", "commitments"]),
    (
        "page:describe",
        &["ymp", "show", "describe", "--candidate", "0"],
    ),
];

/// The actions of the interface that act on the interface rather than on the run. A command
/// process leaves when it has finished its work, and it runs the check a draft needs inside its
/// own process rather than scheduling it, so neither action has a command to mirror it. Neither
/// writes anything durable and neither spends anything; the exclusion is held to exactly these
/// two and the comparison below is an equality, so it cannot grow unnoticed.
const LIFECYCLE: [&str; 2] = ["quit", "cancel-check"];

/// The namespace excluded from the comparison: the product's own machinery, which predates this
/// card and is not an operator capability. The exclusion is held to exactly this one namespace.
const NOT_AN_OPERATOR_SURFACE: [&str; 1] = ["internal"];

/// What that namespace held when this card was written. Pinning it keeps the exclusion from
/// becoming a place to add an operator capability without comparing it to the interface.
const INTERNAL_MACHINERY: [&str; 6] = [
    "agent-mcp",
    "runtime-smoke",
    "managed-runtime-smoke",
    "managed-candidate-smoke",
    "verifier",
    "verify-managed-candidate",
];

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
        InterfaceCommand::StartAttempt => "attempt".to_owned(),
        InterfaceCommand::CancelRun => "cancel-run".to_owned(),
        InterfaceCommand::Export => "export".to_owned(),
        InterfaceCommand::Quit => "quit".to_owned(),
    }
}

/// The name of the action a typed confirmation commits. Exhaustive for the same reason.
fn confirmed_action(action: &ConfirmAction) -> String {
    match action {
        ConfirmAction::StartRun { .. } => "start-run".to_owned(),
        ConfirmAction::StartAttempt { .. } => "attempt".to_owned(),
        ConfirmAction::CancelRun { .. } => "cancel-run".to_owned(),
    }
}

/// The name of the action the interface's event loop is asked to perform. Exhaustive for the
/// same reason.
fn performed_action(action: &Action) -> String {
    match action {
        Action::CancelRun => "cancel-run".to_owned(),
        Action::StartRun(_) => "start-run".to_owned(),
        Action::StartAttempt => "attempt".to_owned(),
        Action::ExportEvidence(_) => "export".to_owned(),
        Action::ApplyCandidate { .. } => "export-apply".to_owned(),
        Action::LocalTurn(_) => "request".to_owned(),
        Action::SetEngineEnabled { enabled: true, .. } => "runtime-enable".to_owned(),
        Action::SetEngineEnabled { enabled: false, .. } => "runtime-disable".to_owned(),
        Action::SetProviderEnabled { enabled: true, .. } => "provider-enable".to_owned(),
        Action::SetProviderEnabled { enabled: false, .. } => "provider-disable".to_owned(),
        Action::RefreshProviderModels { .. } => "provider-refresh".to_owned(),
        Action::SetPoolEntryPermitted {
            permitted: true, ..
        } => "pool-permit".to_owned(),
        Action::SetPoolEntryPermitted {
            permitted: false, ..
        } => "pool-exclude".to_owned(),
        Action::SetPoolCapacity { .. } => "pool-set-capacity".to_owned(),
        Action::CancelCheck => "cancel-check".to_owned(),
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
    let authorize = decisions::authorize(
        &drafted_facts(),
        model.environment(),
        None,
        None,
        decisions::StartFacts::unknown(),
    );
    let start = decisions::start_run(&authorize.action.expect("a startable contract"));
    let cancel = decisions::cancel_run(
        &model
            .projection(None)
            .run
            .expect("the scenario carries a run"),
    );
    let attempt = decisions::start_attempt(
        &model
            .projection(None)
            .run
            .expect("the scenario carries a run"),
        "codex",
    );
    for confirm in [start, cancel, attempt] {
        actions.insert(confirmed_action(&confirm.action));
    }

    // Everything the event loop can be asked to perform.
    for action in [
        Action::CancelRun,
        Action::StartRun("contract-1".to_owned()),
        Action::LocalTurn("keep the replay path idempotent".to_owned()),
        Action::StartAttempt,
        Action::ExportEvidence(None),
        Action::ApplyCandidate {
            destination: None,
            overwrite: false,
        },
        Action::CancelCheck,
        Action::SetEngineEnabled {
            engine: Engine::ClaudeCode,
            enabled: true,
            reason: None,
        },
        Action::SetEngineEnabled {
            engine: Engine::Codex,
            enabled: false,
            reason: Some("usage limit until 2026-09-12".to_owned()),
        },
        Action::SetProviderEnabled {
            family: ProviderFamily::Anthropic,
            enabled: true,
            reason: None,
        },
        Action::SetProviderEnabled {
            family: ProviderFamily::OpenAi,
            enabled: false,
            reason: Some("kept out of this experiment".to_owned()),
        },
        Action::RefreshProviderModels {
            family: ProviderFamily::Anthropic,
        },
        Action::SetPoolEntryPermitted {
            pool: PoolName::default_pool(),
            entry: pool_entry(),
            permitted: true,
        },
        Action::SetPoolEntryPermitted {
            pool: PoolName::default_pool(),
            entry: pool_entry(),
            permitted: false,
        },
        Action::SetPoolCapacity {
            pool: PoolName::default_pool(),
            max_agents: Some(6),
            max_concurrent_attempts: None,
        },
        Action::Rebuild,
    ] {
        actions.insert(performed_action(&action));
    }

    actions
}

/// A root the controller has resolved a pool under: one engine measured, one account enabled and
/// observed, and the pools resolved against what that leaves in the catalog.
///
/// It is the state the first observation of a provider leaves behind, built here from the records
/// rather than by starting anything: what this check drives is the keyboard, and an engine started
/// to produce a pool would make it a measurement test instead.
fn measured_root(address: &RegistryAddress) {
    address
        .registry()
        .update(Engine::ClaudeCode, |record| {
            record.enabled = true;
            record.disabled_reason = None;
            record.properties.executable = Some("/usr/local/bin/claude".to_owned());
            record.properties.version = Some("2.1.227".to_owned());
            record.properties.executable_digest = Some("engine-digest".to_owned());
            record.properties.credential_origin =
                Some("delegated_host_keychain_credential".to_owned());
            record.models = ymp_runtime_registry::ModelCatalog {
                source: ymp_runtime_registry::ModelSource::Measured,
                measured_for_version: Some("2.1.227".to_owned()),
                measured_for_digest: Some("engine-digest".to_owned()),
                note: None,
                names: vec!["claude-opus-5".to_owned()],
            };
        })
        .expect("record the measurement");
    let providers = address.providers();
    providers
        .set_enabled(ProviderFamily::Anthropic, true, None)
        .expect("enable the account");
    providers
        .observe_family(
            ProviderFamily::Anthropic,
            &address.registry(),
            SystemTime::now(),
        )
        .expect("observe the account");
    address
        .pools()
        .reconcile(&providers, &address.registry())
        .expect("resolve the pools");
}

/// One catalog entry, as a pool names it.
fn pool_entry() -> PoolEntry {
    PoolEntry {
        provider: "anthropic".to_owned(),
        engine: "claude-code".to_owned(),
        model: "claude-opus-5".to_owned(),
    }
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
        previously_authorized: false,
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
        PublicCommand::Attempt { .. } => "attempt".to_owned(),
        PublicCommand::Cancel { .. } => "cancel-run".to_owned(),
        // The same command reaches both forms an export takes, so the form is what names the
        // action: the bundle unless the operator states that the candidate is applied in place.
        PublicCommand::Export { apply: true, .. } => "export-apply".to_owned(),
        PublicCommand::Export { .. } => "export".to_owned(),
        PublicCommand::Runtime { command } => match command {
            RuntimeCommand::Enable { .. } => "runtime-enable".to_owned(),
            RuntimeCommand::Disable { .. } => "runtime-disable".to_owned(),
        },
        PublicCommand::Provider { command } => match command {
            ProviderCommand::Enable { .. } => "provider-enable".to_owned(),
            ProviderCommand::Disable { .. } => "provider-disable".to_owned(),
            ProviderCommand::Refresh { .. } => "provider-refresh".to_owned(),
        },
        PublicCommand::Pool { command } => match command {
            PoolCommand::Permit { .. } => "pool-permit".to_owned(),
            PoolCommand::Exclude { .. } => "pool-exclude".to_owned(),
            PoolCommand::SetCapacity { .. } => "pool-set-capacity".to_owned(),
        },
        PublicCommand::Show { page, .. } => page_action(page.kind()),
    }
}

/// Every action a command performs, with the invocation that reaches it checked against the
/// parser: a stated correspondence that no longer parses, or that reaches a different command, is
/// not a correspondence.
fn commanded_actions() -> (BTreeSet<String>, BTreeSet<String>) {
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
    (commanded, named)
}

#[test]
fn every_interface_action_is_a_command_and_neither_side_holds_a_surplus() {
    let root = TempDir::new().expect("temporary root");
    let (commanded, named) = commanded_actions();
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
    // The excluded namespace is pinned rather than trusted: it is compared to the machinery it
    // held when this card was written, so a capability cannot be added to the executable by
    // putting it there instead.
    let internal = parser
        .find_subcommand("internal")
        .expect("the excluded namespace must exist for the exclusion to mean anything");
    assert_eq!(
        internal
            .get_subcommands()
            .map(|child| child.get_name().to_owned())
            .collect::<BTreeSet<_>>(),
        INTERNAL_MACHINERY
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<BTreeSet<_>>(),
        "the excluded namespace changed; a capability may have been added outside the comparison"
    );

    // Every page the command surface can name is in the correspondence too, so a page cannot be
    // added to the command surface without an interface page behind it.
    for page in [
        PageName::Providers,
        PageName::Provider,
        PageName::Models,
        PageName::Pools,
        PageName::Pool,
        PageName::Runtimes,
        PageName::Candidates,
        PageName::Events,
        PageName::Budgets,
        PageName::Attempts,
        PageName::Commitments,
        PageName::Describe,
    ] {
        assert!(
            commanded.contains(&page_action(page.kind())),
            "the command surface shows {:?} outside the correspondence",
            page.kind()
        );
    }
}

/// Every state the interface's key handling can be in, so no binding is read in one state only.
fn keyboard_states(root: &Path) -> Vec<App> {
    let run = scenario::running();
    let mut model = Model::cold(Environment::detect(root), vec![drafted_facts()]);
    model.absorb(&run.state, &run.events);
    let projection = model.projection(None);
    let facts = projection.run.clone().expect("the scenario carries a run");

    let mut states = Vec::new();
    let base = App::new(projection);

    // The conversation: nothing typed, something typed, and an answer awaited.
    states.push(base.clone());
    let mut typing = base.clone();
    typing.prompt.buffer = "keep the replay path idempotent".into();
    states.push(typing);
    let mut awaiting = base.clone();
    awaiting.data.awaiting = Some("answer: source directory".into());
    states.push(awaiting);
    let mut working = base.clone();
    working.data.working = Some("running verify.sh against the negative control".into());
    states.push(working);

    // Every page, including the one reached only by opening a candidate.
    for kind in PageKind::ALL {
        let mut page = base.clone();
        page.surface = Surface::Page(kind);
        states.push(page);
    }

    // The provider properties view with a provider selected, which is the state its keys act in.
    // Without it the keys of that surface would be driven over a view holding no provider, and
    // every one of them would return nothing whatever it was bound to.
    let mut provider = base.clone();
    provider.data.providers = Some(ymp_tui::providers::read(
        &RegistryAddress::Root(root.to_path_buf()),
        SystemTime::now(),
    ));
    provider.provider_index = Some(0);
    provider.surface = Surface::Page(PageKind::Provider);
    states.push(provider);

    // The pool properties view with a pool selected, and the cursor on each kind of row its keys
    // act on: an entry, a ceiling, and a row that carries no act. Without them every key of that
    // surface would be driven over a view holding no pool and would return nothing whatever it
    // was bound to.
    let address = RegistryAddress::Root(root.to_path_buf());
    measured_root(&address);
    let report = ymp_tui::pools::read(&address);
    let rows = ymp_tui::pools::rows(
        report.pool_at(0).expect("the pool the controller created"),
        &report.entries,
    );
    for row in 0..rows.len() {
        let mut pool = base.clone();
        pool.data.pools = Some(report.clone());
        pool.pool_index = Some(0);
        pool.surface = Surface::Page(PageKind::Pool);
        pool.data.pages.push((
            PageKind::Pool,
            ymp_tui::pools::pool_page(
                report.pool_at(0).expect("the pool"),
                &report.entries,
                "idle".to_owned(),
            ),
        ));
        for _ in 0..row {
            pool.select_next(PageKind::Pool);
        }
        states.push(pool);
    }

    // Everything that floats above a surface.
    let mut palette = base.clone();
    palette.modal = Modal::Palette(palette.open_palette());
    states.push(palette);
    let mut keys = base.clone();
    keys.modal = Modal::Keys;
    states.push(keys);
    let mut authorize = base.clone();
    authorize.open_authorize_at(0);
    states.push(authorize);
    for typed in ["", "wrong", &facts.run_id.clone()] {
        let mut confirm = decisions::cancel_run(&facts);
        confirm.typed = typed.to_owned();
        let mut state = base.clone();
        state.modal = Modal::Confirm(confirm);
        states.push(state);
    }

    states
}

/// Every key an operator can press, over every modifier the interface distinguishes.
fn key_presses() -> Vec<KeyEvent> {
    let mut codes: Vec<KeyCode> = (0x20u8..=0x7e)
        .map(|byte| KeyCode::Char(byte as char))
        .collect();
    codes.extend([KeyCode::Char('\u{1b}'), KeyCode::Char('\u{7}')]);
    codes.extend([
        KeyCode::Enter,
        KeyCode::Esc,
        KeyCode::Backspace,
        KeyCode::Tab,
        KeyCode::BackTab,
        KeyCode::Delete,
        KeyCode::Insert,
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Left,
        KeyCode::Right,
        KeyCode::Home,
        KeyCode::End,
        KeyCode::PageUp,
        KeyCode::PageDown,
        KeyCode::F(1),
        KeyCode::F(12),
        KeyCode::Null,
    ]);
    let modifiers = [
        KeyModifiers::NONE,
        KeyModifiers::CONTROL,
        KeyModifiers::ALT,
        KeyModifiers::SHIFT,
    ];
    codes
        .into_iter()
        .flat_map(|code| {
            modifiers
                .iter()
                .map(move |modifier| KeyEvent::new(code, *modifier))
        })
        .collect()
}

/// The inventory compares four enumerations of the interface's vocabulary. This widens it to the
/// keyboard: whatever an operator presses, on whatever surface, the interface either changes what
/// is on screen or returns an action — and every action it can return is one a command performs.
///
/// It also states the shape of the boundary. `handle_key` is a pure function over view state: it
/// holds no session and no store, so a binding cannot commit anything without returning an action
/// through this path, and a binding that returns nothing changed the view and nothing else.
#[test]
fn every_action_a_key_can_reach_is_an_action_a_command_performs() {
    let root = TempDir::new().expect("temporary root");
    let (commanded, _) = commanded_actions();
    let covered: BTreeSet<String> = commanded
        .union(&LIFECYCLE.iter().map(|name| (*name).to_owned()).collect())
        .cloned()
        .collect();

    let mut reached = BTreeSet::new();
    for state in keyboard_states(root.path()) {
        for key in key_presses() {
            let mut app = state.clone();
            if let Some(action) = ymp_tui::app::handle_key(&mut app, key, 40) {
                let name = performed_action(&action);
                assert!(
                    covered.contains(&name),
                    "{key:?} reaches {name}, which no command performs"
                );
                reached.insert(name);
            }
            // Pasted text is the other way input arrives, and it must stay text.
            let mut app = state.clone();
            assert!(
                ymp_tui::app::handle_event(&mut app, Event::Paste(":authorize".into()), 40)
                    .is_none(),
                "pasted text reached an action"
            );
        }
    }

    // The keyboard really does reach the actions this asserts about, so the assertion above is
    // not vacuous.
    assert!(
        reached.contains("request") && reached.contains("cancel-run"),
        "the driven keys reached only {reached:?}"
    );
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
        // A verifier decides: it accepts the source and rejects the empty negative control. A
        // program that exited zero on anything would be refused where the answer is typed, so a
        // fixture built on one could no longer reach the surfaces these tests drive.
        fs::write(&verifier, b"#!/bin/sh\ntest -f \"$1/input.txt\"\n").expect("verifier program");
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

    /// The request, as the lines an operator types: the work, then the amendments that name what
    /// the product would otherwise supply.
    fn lines(&self) -> [String; 4] {
        [
            "keep the replay path idempotent".to_owned(),
            format!("source {}", self.source.display()),
            format!("verifier {}", self.verifier.display()),
            format!("negative control {}", self.negative_control.display()),
        ]
    }

    fn request_arguments(&self) -> Vec<String> {
        vec![
            "--prompt=keep the replay path idempotent".to_owned(),
            format!("--source={}", self.source.display()),
            format!("--verifier={}", self.verifier.display()),
            format!("--negative-control={}", self.negative_control.display()),
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

    /// A contract package under a known identifier, so an invocation carries a contract whose
    /// name an answer could be made to spell.
    fn package(&self, contract_id: &str) -> PathBuf {
        let path = self._root.path().join(format!("{contract_id}.json"));
        let oracle = ymp_domain::digest_bytes(&fs::read(&self.verifier).expect("verifier bytes"));
        fs::write(
            &path,
            serde_json::to_vec(&serde_json::json!({
                "schema_version": 2,
                "contract_id": contract_id,
                "source": self.source,
                "prompt": "keep the replay path idempotent",
                "verifier": {
                    "program": self.verifier,
                    "negative_control": self.negative_control,
                    "oracle_digest": oracle,
                    "wall_time_ms": 60_000,
                    "output_limit_bytes": 1024
                }
            }))
            .expect("package bytes"),
        )
        .expect("write package");
        path
    }
}

/// Values an operator may legitimately pass, each of which the interface's input row would read
/// as something other than text: a leading slash opens its command line, a control character or
/// an escape sequence is a key press of its own, and a path — which also begins with the command
/// prefix — is ordinary text that must keep working. The identifier a confirmation would require
/// is spelled by the answers around them, so an answer that escaped into a decision surface
/// would be able to complete it.
const ANSWERS_THAT_MUST_STAY_ANSWERS: [(&str, &str); 5] = [
    ("slash", "/authorize contract-package-1"),
    ("colon", ":authorize contract-package-1"),
    ("path", "/no/such/directory"),
    ("escape", "\u{1b}[2K/authorize contract-package-1"),
    ("control", "\u{7}/quit"),
];

#[test]
fn an_answer_never_reaches_a_surface_the_command_did_not_open() {
    let fixture = Fixture::new();
    let package = fixture.package("contract-package-1");

    for (name, value) in ANSWERS_THAT_MUST_STAY_ANSWERS {
        for verb in ["request", "authorize"] {
            let data_root = fixture.data_root(&format!("{verb}-{name}"));
            let refused = fixture.command(
                &data_root,
                &[
                    "--contract".to_owned(),
                    package.display().to_string(),
                    verb.to_owned(),
                    "--prompt=work".to_owned(),
                    format!("--source={value}"),
                    "--verifier=anything".to_owned(),
                    // The exact identifier a start confirmation requires. Nothing in this
                    // invocation authorizes a run, so nothing may consume it as one.
                    "--negative-control=contract-package-1".to_owned(),
                ],
            );

            assert!(
                !data_root.join("events.jsonl").exists(),
                "`ymp {verb}` with a {name} answer started a run: {}",
                String::from_utf8_lossy(&refused.stdout)
            );
            assert!(
                !refused.status.success(),
                "`ymp {verb}` with a {name} answer reported success for a request it could not \
                 prepare"
            );
        }
    }
}

/// Carry the request to a started run and then cancel it, through the interface's own session.
fn through_the_interface(fixture: &Fixture, data_root: &Path) -> (String, String) {
    let mut session = Session::open(data_root, &[]);
    for line in fixture.lines() {
        session.local_turn(line);
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

/// What a store committed, with the identity of the run set aside.
///
/// A run is identified by its contract and by the store that holds it, so two stores never commit
/// one run identifier and never commit one digest chain. What the two surfaces must agree on is
/// what was committed: the same events, in the same order, under the same command identifiers.
fn committed(data_root: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(data_root.join("events.jsonl"))
        .expect("committed journal")
        .lines()
        .map(|line| {
            let mut record: serde_json::Value =
                serde_json::from_str(line).expect("every record is an event envelope");
            let object = record.as_object_mut().expect("an envelope is an object");
            for followed in ["run_id", "digest", "predecessor_digest"] {
                object.remove(followed);
            }
            record
        })
        .collect()
}

/// The run this store holds, read from the store itself rather than derived a second time.
fn run_id(data_root: &Path) -> String {
    let session = Session::open(data_root, &[]);
    session
        .projection(None)
        .run
        .expect("the store holds a run")
        .run_id
}

#[test]
fn the_command_and_the_interface_commit_the_same_journal() {
    let fixture = Fixture::new();
    let interface_root = fixture.data_root("interface");
    let command_root = fixture.data_root("command");

    let (contract_id, interface_run) = through_the_interface(&fixture, &interface_root);

    let mut start = fixture.request_arguments();
    start.insert(0, "start".to_owned());
    start.push(format!("--confirm={contract_id}"));
    let started = fixture.command(&command_root, &start);
    assert!(
        started.status.success(),
        "the command did not start the run: {}",
        String::from_utf8_lossy(&started.stderr)
    );

    // The run this store holds is the one the command cancels: the store names its run, and the
    // identifier the other store's run carries is not it.
    let command_run = run_id(&command_root);
    assert_ne!(
        command_run, interface_run,
        "two stores hold two runs and name them the same"
    );
    let cancelled = fixture.command(
        &command_root,
        &["cancel".to_owned(), format!("--confirm={command_run}")],
    );
    assert!(
        cancelled.status.success(),
        "the command did not cancel the run: {}",
        String::from_utf8_lossy(&cancelled.stderr)
    );

    assert_eq!(
        committed(&command_root),
        committed(&interface_root),
        "the command and the interface committed different journals"
    );
}

// ---------------------------------------------------------------------------
// The approved-contract check
// ---------------------------------------------------------------------------

/// Nothing runs without something that would reject a wrong candidate. Where the product can
/// propose that something it does, where it can derive one from the request it generates it, and
/// where it can do neither it says so; either way a command that reaches nothing starts nothing
/// and exits non-zero. This request names no artifact in a project that runs no tests, which is
/// exactly the case where neither route is open.
#[test]
fn a_command_with_nothing_that_could_judge_a_candidate_starts_nothing() {
    let fixture = Fixture::new();
    let data_root = fixture.data_root("no-acceptance");
    let refused = fixture.command(
        &data_root,
        &[
            "start".to_owned(),
            "--prompt=keep the replay path idempotent".to_owned(),
            format!("--source={}", fixture.source.display()),
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
        reported.contains("state a verifier of your own"),
        "the refusal does not name what could not be proposed:\n{reported}"
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
    let (contract_id, _) = through_the_interface(&fixture, &fixture.data_root("cancel"));
    // The interface cancelled its own run, so a second store carries the live one. That store
    // holds its own run: the identifier is read from it rather than derived from the request,
    // which names the contract and not the run.
    let live = fixture.data_root("cancel-live");
    let mut arguments = fixture.request_arguments();
    arguments.insert(0, "start".to_owned());
    arguments.push(format!("--confirm={contract_id}"));
    assert!(fixture.command(&live, &arguments).status.success());
    let run_id = run_id(&live);

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
