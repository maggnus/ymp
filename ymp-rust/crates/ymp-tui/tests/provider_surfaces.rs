//! Acceptance: the operator reaches the provider level as tables, takes both decisions on a
//! selected row, and nothing about a provider is measured before it is enabled.
//!
//! Everything here goes through the production session and the production key handling, at both
//! declared terminal sizes, so what is asserted is what an operator at a terminal reaches.
//!
//! * `/providers` is the **whole supported list**, whatever this root has touched, and reading it
//!   starts nothing: the negative half asserts that no engine record and no provider record is
//!   written by looking at the table, and that the enable transition is what writes both.
//! * A provider is opened, not decided on: `Enter` opens its properties, and the consequence of
//!   the key that enables it stands **above** that key on the same screen — which is decision D4's
//!   disclosure notice and what replaces the confirmation dialogue.
//! * `/models` is derived when it is read, states the age of every observation, and draws a route
//!   that serves nothing as a route that serves nothing rather than as a shorter catalog.
//! * Every key returns the action a command of the same executable performs.

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossterm::event::KeyCode;
use support::{SIZES, buffer, open_command, press, screen};
use tempfile::TempDir;
use ymp_runtime_registry::{
    Engine, ModelCatalog, ModelSource, ProviderFamily, Registry, RegistryAddress,
};
use ymp_tui::Session;
use ymp_tui::app::Action;
use ymp_tui::state::{App, PageKind, Surface};

/// A product root of this check's own. Nothing here reads or writes the owner's own root.
struct Root {
    _directory: TempDir,
    root: PathBuf,
}

impl Root {
    fn new() -> Self {
        let directory = TempDir::new().expect("temporary directory");
        let root = directory.path().join("root");
        fs::create_dir_all(&root).expect("root directory");
        Self {
            _directory: directory,
            root,
        }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn address(&self) -> RegistryAddress {
        RegistryAddress::Root(self.root.clone())
    }

    /// The interface over this root, with the store of a project that holds no run.
    fn interface(&self) -> App {
        App::new(self.session().projection(None))
    }

    fn session(&self) -> Session {
        Session::open_under_root(&self.root, &self.root.join("runs/0001"), &[])
    }

    /// An engine measured on this host, as a measurement of it would have recorded it.
    fn measured(&self, engine: Engine, names: &[&str]) {
        Registry::under(&self.root)
            .update(engine, |record| {
                record.enabled = true;
                record.disabled_reason = None;
                record.properties.executable = Some(format!("/usr/local/bin/{}", engine.program()));
                record.properties.version = Some("2.1.227".to_owned());
                record.properties.executable_digest = Some("engine-digest".to_owned());
                record.properties.credential_origin =
                    Some("delegated_host_keychain_credential".to_owned());
                record.models = ModelCatalog {
                    source: ModelSource::Measured,
                    measured_for_version: Some("2.1.227".to_owned()),
                    measured_for_digest: Some("engine-digest".to_owned()),
                    note: None,
                    names: names.iter().map(|name| (*name).to_owned()).collect(),
                };
            })
            .expect("record the measurement");
    }

    /// A provider enabled and observed at a stated moment, so the age a surface states is fixed.
    fn enabled_and_observed(&self, family: ProviderFamily, at: SystemTime) {
        let providers = self.address().providers();
        providers
            .set_enabled(family, true, None)
            .expect("enable the provider");
        providers
            .observe_family(family, &Registry::under(&self.root), at)
            .expect("observe the provider");
    }
}

/// The row of the rendered screen a phrase stands on, so "above" can be asserted rather than
/// assumed.
fn row_of(rendered: &[String], phrase: &str) -> usize {
    rendered
        .iter()
        .position(|row| row.contains(phrase))
        .unwrap_or_else(|| panic!("no row carries {phrase:?}:\n{}", rendered.join("\n")))
}

/// The row the footer states a key on. It is the last row naming the key, because the sentence
/// above it names the same key and the point of the assertion is that the two are in that order.
fn key_row(rendered: &[String], key: &str) -> usize {
    let row = rendered
        .iter()
        .rposition(|row| row.contains(key))
        .unwrap_or_else(|| panic!("no row offers {key:?}:\n{}", rendered.join("\n")));
    assert!(
        rendered[row].contains("Esc"),
        "the row taken for the footer is not the footer:\n{}",
        rendered.join("\n")
    );
    row
}

/// Open a page and leave the operator standing on it.
fn open(app: &mut App, name: &str, height: u16) {
    let action = open_command(app, name, height);
    assert_eq!(action, None, "opening {name} asked for an action");
}

/// The supported list is shown in full on a root nothing has been enabled on, and looking at it
/// measures nothing.
///
/// The negative half is the second half of this test: a probe writes the engine record it measured
/// and the observation writes the provider record, so a root that holds neither after the table has
/// been drawn is a root on which nothing was started. The check that must fail: measure the
/// providers when the page is composed, and the records appear before any Enable was pressed.
#[test]
fn the_supported_list_is_drawn_in_full_and_nothing_is_measured_by_looking_at_it() {
    for (width, height) in SIZES {
        let root = Root::new();
        let mut app = root.interface();
        open(&mut app, "providers", height);
        assert_eq!(app.surface, Surface::Page(PageKind::Providers));

        let rendered = screen(&app, width, height);
        assert!(
            rendered.contains("providers(all)[2]"),
            "the table did not draw the supported list at {width}x{height}:\n{rendered}"
        );
        for family in ProviderFamily::ALL {
            assert!(
                rendered.contains(family.name()),
                "{} is missing from the supported list at {width}x{height}:\n{rendered}",
                family.name()
            );
        }
        // The columns of surface S03, in its order.
        for column in ["PROVIDER", "STATE", "REACHED BY", "MODELS", "OBSERVED"] {
            assert!(
                rendered.contains(column),
                "the table dropped the {column} column at {width}x{height}:\n{rendered}"
            );
        }
        assert!(rendered.contains("disabled"), "{rendered}");

        assert!(
            !root.path().join("runtimes").exists(),
            "reading the supported list measured an engine at {width}x{height}"
        );
        assert!(
            !root.path().join("providers").exists(),
            "reading the supported list wrote a provider record at {width}x{height}"
        );

        press(&mut app, KeyCode::Esc, height);
        assert_eq!(app.surface, Surface::Transcript, "Esc did not return");
    }
}

/// A row is opened rather than decided on, and what enabling permits is stated above the key that
/// enables — which is decision D4's disclosure notice and the whole reason this view exists.
#[test]
fn enter_opens_a_provider_and_the_disclosure_stands_above_the_key_that_enables() {
    for (width, height) in SIZES {
        let root = Root::new();
        let session = root.session();
        let mut app = App::new(session.projection(None));
        open(&mut app, "providers", height);

        let action = press(&mut app, KeyCode::Enter, height);
        assert_eq!(action, Some(Action::Rebuild), "Enter opened nothing");
        assert_eq!(app.provider_index, Some(0));
        app.adopt(session.projection_for(None, app.provider_index));
        assert_eq!(app.surface, Surface::Page(PageKind::Provider));

        let rendered = buffer(&app, width, height);
        let shown = rendered.join("\n");
        assert!(shown.contains("providers › anthropic"), "{shown}");
        for field in ["status", "authentication", "models", "last refresh"] {
            assert!(
                shown.contains(field),
                "the properties view drops {field:?} at {width}x{height}:\n{shown}"
            );
        }
        assert!(shown.contains("never"), "{shown}");

        // The consequence is on the screen, above the key, and it is the sentence D4 places here.
        let consequence = row_of(&rendered, "repository");
        let key = key_row(&rendered, "e enable");
        assert!(
            consequence < key,
            "the disclosure was drawn below the key it belongs above at {width}x{height}:\n{shown}"
        );
        // It is a disclosure notice, so it has to be readable in full and not cut off the right
        // of a narrow screen. The rows are joined and their padding collapsed, because the layout
        // wraps it.
        let flattened = shown.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            flattened.contains(
                "e enables anthropic — enabling sends repository content from any workspace to \
                 anthropic, including the bounded excerpts ymp reads to work out what \"done\" \
                 means before a run starts"
            ),
            "the disclosure is not readable in full at {width}x{height}:\n{shown}"
        );

        // Nothing was confirmed and nothing was typed to get here.
        assert!(matches!(app.modal, ymp_tui::state::Modal::None));

        press(&mut app, KeyCode::Esc, height);
        assert_eq!(
            app.surface,
            Surface::Page(PageKind::Providers),
            "Esc left the list instead of returning to it"
        );
    }
}

/// Both keys of the properties view return the action a command of the same executable performs.
#[test]
fn the_keys_of_a_provider_are_the_actions_a_command_performs() {
    let root = Root::new();
    let session = root.session();
    let mut app = App::new(session.projection(None));
    open(&mut app, "providers", 40);
    press(&mut app, KeyCode::Enter, 40);
    app.adopt(session.projection_for(None, app.provider_index));

    assert_eq!(
        press(&mut app, KeyCode::Char('e'), 40),
        Some(Action::SetProviderEnabled {
            family: ProviderFamily::Anthropic,
            enabled: true,
            reason: None,
        }),
        "`e` on a provider that is not enabled did not enable it"
    );
    assert_eq!(
        press(&mut app, KeyCode::Char('r'), 40),
        Some(Action::RefreshProviderModels {
            family: ProviderFamily::Anthropic,
        })
    );

    // The second row is the second provider: the act reaches the account the row states.
    press(&mut app, KeyCode::Esc, 40);
    press(&mut app, KeyCode::Down, 40);
    press(&mut app, KeyCode::Enter, 40);
    app.adopt(session.projection_for(None, app.provider_index));
    assert_eq!(
        press(&mut app, KeyCode::Char('e'), 40),
        Some(Action::SetProviderEnabled {
            family: ProviderFamily::OpenAi,
            enabled: true,
            reason: None,
        })
    );
}

/// An enabled provider states what it measured and how old that measurement is, and its keys
/// change to the acts that are then available.
#[test]
fn an_enabled_provider_states_its_measurement_and_its_age_at_both_sizes() {
    let observed = UNIX_EPOCH + Duration::from_secs(1_770_000_000);
    for (width, height) in SIZES {
        let root = Root::new();
        root.measured(Engine::ClaudeCode, &["claude-opus-5", "claude-sonnet-5"]);
        root.enabled_and_observed(ProviderFamily::Anthropic, observed);

        // The age is stated against the moment the surface is read, which is now: the observation
        // is dated in the past, so whatever the clock says the row states an age and not "just
        // now". The exact number is asserted in the unit tests, where the clock is stated.
        let session = root.session();
        let mut app = App::new(session.projection(None));
        open(&mut app, "providers", height);
        let shown = screen(&app, width, height);
        assert!(shown.contains("ready"), "{shown}");
        assert!(shown.contains('2'), "the model count is missing:\n{shown}");

        press(&mut app, KeyCode::Enter, height);
        app.adopt(session.projection_for(None, app.provider_index));
        let rendered = buffer(&app, width, height);
        let shown = rendered.join("\n");
        assert!(shown.contains("ago"), "no age is stated:\n{shown}");
        assert!(
            shown.contains("delegated_host_keychain_credential"),
            "the credential origin is not named:\n{shown}"
        );
        assert!(
            shown.contains("claude-code"),
            "the engine that reaches the account is not named:\n{shown}"
        );
        // The consequence of each key is still above the keys, now for the acts that are offered.
        let consequence = row_of(&rendered, "disables anthropic");
        let key = key_row(&rendered, "e disable");
        assert!(consequence < key, "{shown}");
        assert!(shown.contains("r measures anthropic again"), "{shown}");
    }
}

/// The catalog is a table, states the age of the observation it was joined from, and draws a route
/// that serves nothing as a route that serves nothing.
#[test]
fn the_model_catalog_states_its_ages_and_a_route_that_serves_nothing() {
    let observed = UNIX_EPOCH + Duration::from_secs(1_770_000_000);
    for (width, height) in SIZES {
        let root = Root::new();
        root.measured(Engine::ClaudeCode, &["claude-opus-5", "claude-sonnet-5"]);
        root.enabled_and_observed(ProviderFamily::Anthropic, observed);

        let session = root.session();
        let mut app = App::new(session.projection(None));
        open(&mut app, "models", height);
        assert_eq!(app.surface, Surface::Page(PageKind::Models));
        let shown = screen(&app, width, height);
        assert!(shown.contains("MODEL"), "{shown}");
        assert!(shown.contains("PROVIDER"), "{shown}");
        assert!(shown.contains("STATE"), "{shown}");
        assert!(shown.contains("claude-opus-5"), "{shown}");
        assert!(shown.contains("offered"), "{shown}");
        assert!(
            shown.contains("anthropic · observed"),
            "the age of the observation is not stated at {width}x{height}:\n{shown}"
        );
        // The account nobody enabled is on the page as that, so a catalog holding one account's
        // models is never read as everything this build reaches.
        assert!(shown.contains("openai"), "{shown}");
        assert!(shown.contains("not enabled"), "{shown}");

        // The engine record leaves the root. Its models leave the catalog with it, and the route
        // it served them through stays and says so.
        fs::remove_file(Registry::under(root.path()).path_of(Engine::ClaudeCode))
            .expect("remove the engine record");
        let session = root.session();
        let mut app = App::new(session.projection(None));
        open(&mut app, "models", height);
        let shown = screen(&app, width, height);
        assert!(
            !shown.contains("claude-opus-5"),
            "a removed record still served its models:\n{shown}"
        );
        assert!(
            shown.contains("no models"),
            "a route that serves nothing was answered with a shorter catalog at \
             {width}x{height}:\n{shown}"
        );
        assert!(shown.contains("claude-code"), "{shown}");
    }
}

/// The three surfaces draw the same buffer twice and never exceed the terminal they are given.
#[test]
fn the_provider_surfaces_are_deterministic_and_never_exceed_the_terminal() {
    let observed = UNIX_EPOCH + Duration::from_secs(1_770_000_000);
    for (width, height) in SIZES {
        let root = Root::new();
        root.measured(Engine::ClaudeCode, &["claude-opus-5"]);
        root.enabled_and_observed(ProviderFamily::Anthropic, observed);
        let session = root.session();

        for page in ["providers", "models"] {
            let mut app = App::new(session.projection(None));
            open(&mut app, page, height);
            let first = buffer(&app, width, height);
            let second = buffer(&app, width, height);
            assert_eq!(first, second, "{page} drew two different buffers");
            assert_eq!(first.len(), height as usize);
            for line in &first {
                assert_eq!(
                    line.chars().count(),
                    width as usize,
                    "{page} drifted in width: {line:?}"
                );
            }
        }

        let mut app = App::new(session.projection(None));
        open(&mut app, "providers", height);
        press(&mut app, KeyCode::Enter, height);
        app.adopt(session.projection_for(None, app.provider_index));
        let first = buffer(&app, width, height);
        assert_eq!(first, buffer(&app, width, height));
        assert_eq!(first.len(), height as usize);
        for line in &first {
            assert_eq!(line.chars().count(), width as usize, "{line:?}");
        }
    }
}

/// A goal stated while nothing is enabled is held, and the reply is the state in plain words with
/// the command that supplies what is missing.
///
/// The check that must fail: answer it with the technical sentence the brief names — `no providers
/// configured` — or with a refusal that loses the goal.
#[test]
fn a_goal_typed_with_no_provider_enabled_is_held_and_names_the_command() {
    let root = Root::new();
    let mut session = root.session();
    // The turn is taken, and the work it would schedule is not run: what is asserted is the reply,
    // and running the assembly would copy this test process's own directory.
    let pending = session.begin_turn("Создай браузерную игру сапер".to_owned());
    assert!(
        pending.is_some(),
        "the goal was refused rather than taken and held"
    );

    let stated = session
        .projection(None)
        .entries
        .iter()
        .map(|entry| format!("{entry:?}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        stated.contains("at least one enabled AI provider"),
        "the state was not said in plain words:\n{stated}"
    );
    assert!(stated.contains("/providers"), "{stated}");
    assert!(
        !stated.contains("no providers configured"),
        "the technical sentence the brief forbids was shown:\n{stated}"
    );
    assert!(
        !root.path().join("runtimes").exists(),
        "stating a goal measured an engine"
    );
}
