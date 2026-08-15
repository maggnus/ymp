//! Acceptance: the operator reaches the pool level as a table, edits a pool on the row the edit
//! acts on, and nothing on either surface creates a participant.
//!
//! Everything here goes through the production session and the production key handling, at both
//! declared terminal sizes, so what is asserted is what an operator at a terminal reaches.
//!
//! * `/pools` states the pools this root holds and, where it holds none, what creates one — the
//!   operator is never asked to create a pool.
//! * A pool is opened, not decided on: `Enter` opens its properties, and what each key would do is
//!   stated **above** the keys on the same screen. The first edit of a pool that follows the
//!   catalog replaces the whole-catalog form with the list the operator leaves, and the surface
//!   says so before the key is pressed rather than in a dialogue after it.
//! * Permitting an entry and taking one out are keys on the entry's own row, so no model name is
//!   typed; raising and lowering a ceiling are keys on the ceiling's own row.
//! * Holding an account back leaves the pool standing and empty, never deleted, and takes back the
//!   readiness this session measured for the engines that account is reached through.
//!
//! **What is not driven here is a measurement.** Enabling an account starts the engines that reach
//! it, and this check runs in the test process, whose search path is the operator's own: a check
//! that enabled an account here would start the operator's installed engine and put their own
//! account to work. The observation paths that start nothing are driven in full, and the enable
//! transition is driven against the built executable with a search path of its own, in
//! `crates/ymp-cli/tests/pool_surfaces.rs`.

mod support;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use crossterm::event::KeyCode;
use support::{SIZES, buffer, open_command, press, screen};
use tempfile::TempDir;
use ymp_runtime_registry::{
    Engine, ModelCatalog, ModelSource, PoolName, PoolStateKind, ProviderFamily, Registry,
    RegistryAddress,
};
use ymp_tui::Session;
use ymp_tui::app::{Action, Selected};
use ymp_tui::pools::{self, Ceiling, PoolRow};
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

    fn session(&self) -> Session {
        Session::open_under_root(&self.root, &self.root.join("runs/0001"), &[])
    }

    /// The records one measurement of an account would have left: the engine it was reached
    /// through, the models it served, and the observation that attributed them to the account.
    ///
    /// It is written rather than measured for the reason stated at the top of this file, and it is
    /// exactly what the enable transition writes: the surfaces below read records and cannot tell
    /// which call wrote them.
    fn measured(&self, names: &[&str]) {
        Registry::under(&self.root)
            .update(Engine::ClaudeCode, |record| {
                record.enabled = true;
                record.disabled_reason = None;
                record.properties.executable = Some("/usr/local/bin/claude".to_owned());
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
        let providers = self.address().providers();
        providers
            .set_enabled(ProviderFamily::Anthropic, true, None)
            .expect("enable the account");
        providers
            .observe_family(
                ProviderFamily::Anthropic,
                &Registry::under(&self.root),
                UNIX_EPOCH + Duration::from_secs(1_770_000_000),
            )
            .expect("observe the account");
    }

    /// The pools resolved against what the records now hold, which is what an observation leaves
    /// behind on the product path.
    fn resolved(&self) {
        let address = self.address();
        address
            .pools()
            .reconcile(&address.providers(), &address.registry())
            .expect("resolve the pools");
    }

    fn pool_record(&self) -> ymp_runtime_registry::PoolRecord {
        self.address()
            .pools()
            .read(&PoolName::default_pool())
            .expect("the pool record is readable")
            .expect("this root holds the default pool")
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

/// Where the operator is standing, as the event loop reports it to the session.
fn standing_on(app: &App) -> Selected {
    Selected {
        pool: app.pool_index,
        ..Selected::none()
    }
}

/// Open a page and leave the operator standing on it.
fn open(app: &mut App, name: &str, height: u16) {
    let action = open_command(app, name, height);
    assert_eq!(action, None, "opening {name} asked for an action");
}

/// Open the properties of the first pool, which is where every act on a pool is taken.
fn open_the_pool(session: &Session, app: &mut App, height: u16) {
    open(app, "pools", height);
    let action = press(app, KeyCode::Enter, height);
    assert_eq!(action, Some(Action::Rebuild), "Enter opened nothing");
    app.adopt(session.projection_for(standing_on(app)));
    assert_eq!(app.surface, Surface::Page(PageKind::Pool));
}

/// Move the cursor onto one row of the properties view, by pressing the key an operator presses.
///
/// Which row that is comes from the same call the page builds its rows from, so a check stands on
/// what an operator would be standing on rather than on a position guessed from the drawing.
fn select_row(app: &mut App, wanted: &PoolRow, height: u16) {
    let report = app.data.pools.clone().expect("the pool level was read");
    let pool = report.pool_at(0).expect("the pool");
    let index = pools::rows(pool, &report.entries)
        .iter()
        .position(|row| row == wanted)
        .unwrap_or_else(|| panic!("the properties view holds no {wanted:?} row"));
    while app.selection_of(PageKind::Pool) > 0 {
        press(app, KeyCode::Up, height);
    }
    for _ in 0..index {
        press(app, KeyCode::Down, height);
    }
    assert_eq!(app.selection_of(PageKind::Pool), index);
}

/// A root that has measured nothing holds no pool, and the table says what creates one rather than
/// asking the operator to create it.
#[test]
fn a_root_that_measured_nothing_holds_no_pool_and_never_asks_for_one() {
    for (width, height) in SIZES {
        let root = Root::new();
        let session = root.session();
        let mut app = App::new(session.projection(None));
        open(&mut app, "pools", height);
        assert_eq!(app.surface, Surface::Page(PageKind::Pools));

        let shown = screen(&app, width, height);
        assert!(
            shown.contains("pools(all)[0]"),
            "the table did not state that this root holds no pool at {width}x{height}:\n{shown}"
        );
        assert!(shown.contains("no pool stands under this root"), "{shown}");
        assert!(
            shown.contains("/providers"),
            "the page does not name what creates a pool at {width}x{height}:\n{shown}"
        );
        assert!(
            !shown.contains("No AgentPool"),
            "the first-run state was drawn as a missing configuration:\n{shown}"
        );
        assert!(
            !root.path().join("pools").exists(),
            "reading the table created a pool"
        );

        press(&mut app, KeyCode::Esc, height);
        assert_eq!(app.surface, Surface::Transcript, "Esc did not return");
    }
}

/// A measured root holds the pool the controller created, and the table states what it permits,
/// what it is held to and whether it still follows the catalog.
#[test]
fn the_table_states_what_each_pool_permits_at_both_sizes() {
    for (width, height) in SIZES {
        let root = Root::new();
        root.measured(&["claude-opus-5", "claude-sonnet-5"]);
        root.resolved();

        let session = root.session();
        let mut app = App::new(session.projection(None));
        open(&mut app, "pools", height);
        let shown = screen(&app, width, height);
        for column in ["NAME", "MODELS", "CAPACITY", "TRACKING", "STATE"] {
            assert!(
                shown.contains(column),
                "the table dropped the {column} column at {width}x{height}:\n{shown}"
            );
        }
        assert!(shown.contains("default"), "{shown}");
        assert!(shown.contains("all admissible"), "{shown}");
        assert!(shown.contains("ready"), "{shown}");
        assert!(
            shown.contains("not who works"),
            "the page does not say what a pool is at {width}x{height}:\n{shown}"
        );
    }
}

/// The properties view holds one row per entry and states, above the keys, what the first edit of
/// a pool that follows the catalog would replace.
#[test]
fn the_properties_view_states_the_consequence_above_the_keys_at_both_sizes() {
    for (width, height) in SIZES {
        let root = Root::new();
        root.measured(&["claude-opus-5", "claude-sonnet-5"]);
        root.resolved();

        let session = root.session();
        let mut app = App::new(session.projection(None));
        open_the_pool(&session, &mut app, height);

        let rendered = buffer(&app, width, height);
        let shown = rendered.join("\n");
        assert!(shown.contains("claude-opus-5"), "{shown}");
        assert!(shown.contains("up to 6 participants"), "{shown}");

        // The consequence is on the screen and above the keys the footer offers.
        let consequence = row_of(&rendered, "stops following it");
        let keys = rendered
            .iter()
            .rposition(|row| row.contains("Esc"))
            .expect("the footer names its keys");
        assert!(
            consequence < keys,
            "the consequence was drawn below the keys it belongs above at \
             {width}x{height}:\n{shown}"
        );
        // The `?` map is full at the narrowest size the product supports, so the keys of this
        // surface are named in its own footer, exactly as the provider surfaces name theirs.
        for key in ["Enter", "+", "-", "Esc"] {
            assert!(
                rendered[keys].contains(key),
                "the footer does not name {key:?} at {width}x{height}:\n{}",
                rendered[keys]
            );
        }
        assert!(matches!(app.modal, ymp_tui::state::Modal::None));

        press(&mut app, KeyCode::Esc, height);
        assert_eq!(
            app.surface,
            Surface::Page(PageKind::Pools),
            "Esc left the list instead of returning to it"
        );
    }
}

/// Taking an entry out is a key on the entry's own row, and it turns a pool that followed the
/// catalog into the list the operator left — which the record states.
///
/// The check that must fail: leave the pool tracking after an edit, and a model measured later
/// would join a pool the operator narrowed on purpose.
#[test]
fn taking_an_entry_out_turns_tracking_into_the_list_the_operator_left() {
    let root = Root::new();
    root.measured(&["claude-opus-5", "claude-sonnet-5"]);
    root.resolved();
    assert!(root.pool_record().resolved.tracking);

    let mut session = root.session();
    let mut app = App::new(session.projection(None));
    open_the_pool(&session, &mut app, 40);

    // The first entry row is the second row of the view: the declaration stands above it.
    press(&mut app, KeyCode::Down, 40);
    let action = press(&mut app, KeyCode::Enter, 40).expect("the entry row carries an act");
    let Action::SetPoolEntryPermitted {
        pool,
        entry,
        permitted,
    } = action
    else {
        panic!("Enter on an entry row did not reach the pool: {action:?}");
    };
    assert_eq!(pool.as_str(), "default");
    assert_eq!(entry.model, "claude-opus-5");
    assert!(!permitted, "Enter on a permitted entry did not take it out");
    assert!(
        session.set_pool_entry_permitted(pool, entry, permitted),
        "the edit was refused"
    );

    let record = root.pool_record();
    assert!(
        !record.resolved.tracking,
        "the pool still follows the catalog after an edit"
    );
    assert!(record.resolved.holds(PoolStateKind::Explicit));
    assert_eq!(record.resolved.entries.len(), 1);
    assert_eq!(record.resolved.entries[0].model, "claude-sonnet-5");

    // The surface says so where the operator is standing, and the entry that left is still a row
    // — it is what the operator would press to permit it again.
    app.adopt(session.projection_for(standing_on(&app)));
    let shown = screen(&app, 120, 40);
    assert!(shown.contains("explicit"), "{shown}");
    // The notes are wrapped to the terminal, so the sentence is read from the flattened screen.
    let flattened = shown.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        flattened.contains("does not follow the catalog"),
        "the surface does not state that the pool stopped following the catalog:\n{shown}"
    );
    assert!(shown.contains("claude-opus-5"), "{shown}");
    assert!(shown.contains("not permitted"), "{shown}");

    // Nothing was confirmed, nothing was typed, and no participant was created.
    assert!(matches!(app.modal, ymp_tui::state::Modal::None));
    let stated = session
        .projection(None)
        .entries
        .iter()
        .map(|entry| format!("{entry:?}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(stated.contains("no longer permits"), "{stated}");
    assert!(
        !stated.contains("participant"),
        "editing a pool spoke of a participant:\n{stated}"
    );
}

/// A ceiling is raised and lowered on the row that states it, and moving one leaves the entries —
/// and therefore the digest a run freezes — exactly as they were.
#[test]
fn a_ceiling_is_raised_on_the_row_that_states_it_and_moves_no_entry() {
    let root = Root::new();
    root.measured(&["claude-opus-5"]);
    root.resolved();
    let before = root.pool_record();

    let mut session = root.session();
    let mut app = App::new(session.projection(None));
    open_the_pool(&session, &mut app, 40);
    select_row(&mut app, &PoolRow::Capacity(Ceiling::Participants), 40);

    let action = press(&mut app, KeyCode::Char('+'), 40).expect("the ceiling row carries an act");
    let Action::SetPoolCapacity {
        pool,
        max_agents,
        max_concurrent_attempts,
    } = action
    else {
        panic!("+ on a ceiling row did not reach the pool: {action:?}");
    };
    assert_eq!(max_agents, Some(7));
    assert_eq!(max_concurrent_attempts, Some(3));
    assert!(
        session.set_pool_capacity(pool, max_agents, max_concurrent_attempts),
        "the edit was refused"
    );

    let after = root.pool_record();
    assert_eq!(after.declared.capacity.max_agents, 7);
    assert_eq!(after.declared.capacity.max_concurrent_attempts, 3);
    assert_eq!(
        after.resolved.digest, before.resolved.digest,
        "a ceiling moved the digest a run freezes, which follows the entries and their order alone"
    );
    assert!(
        after.resolved.tracking,
        "raising a ceiling stopped the pool following the catalog"
    );

    // Lowering states the same thing in the other direction.
    app.adopt(session.projection_for(standing_on(&app)));
    select_row(&mut app, &PoolRow::Capacity(Ceiling::Participants), 40);
    let action = press(&mut app, KeyCode::Char('-'), 40).expect("the ceiling row carries an act");
    let Action::SetPoolCapacity { max_agents, .. } = action else {
        panic!("- on a ceiling row did not reach the pool: {action:?}");
    };
    assert_eq!(max_agents, Some(6));
}

/// Holding the only account back leaves the pool standing and empty — never deleted — and the
/// surfaces state which account would restore it.
#[test]
fn holding_the_only_account_back_leaves_the_pool_standing_and_empty() {
    let root = Root::new();
    root.measured(&["claude-opus-5", "claude-sonnet-5"]);
    root.resolved();
    assert_eq!(root.pool_record().resolved.admissible, 2);

    let mut session = root.session();
    session.set_provider_enabled(
        ProviderFamily::Anthropic,
        false,
        Some("kept out for this check".to_owned()),
    );

    let record = root.pool_record();
    assert!(
        record.resolved.holds(PoolStateKind::Empty),
        "the pool was not left empty: {:?}",
        record.resolved.states
    );
    assert_eq!(record.resolved.admissible, 0);
    assert!(
        root.path().join("pools").join("default.json").is_file(),
        "holding an account back deleted the pool"
    );

    let mut app = App::new(session.projection(None));
    open(&mut app, "pools", 40);
    let shown = screen(&app, 120, 40);
    assert!(shown.contains("empty"), "{shown}");
    assert!(
        shown.contains("no entry this pool permits is offered"),
        "the pool does not state why it is empty:\n{shown}"
    );
    // The entries it permits are still there, each carrying the measured reason: a pool that grew
    // shorter would answer "why is this model not offered" with silence.
    assert!(shown.contains("anthropic is not enabled"), "{shown}");
    let stated = session
        .projection(None)
        .entries
        .iter()
        .map(|entry| format!("{entry:?}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        stated.contains("default empty"),
        "the reply does not state what the pools now hold:\n{stated}"
    );
}

/// Holding an account back takes back the readiness this session measured for the engines that
/// account is reached through, and starts nothing to do it.
///
/// The check that must fail: leave the held reading alone on the disable path, and a profile the
/// operator has just held back is still offered as the route of the next run for the rest of the
/// session — the decision reaches the records and not the session it was taken in.
///
/// The reading the session holds is a fixture rather than a measurement, for the reason stated at
/// the top of this file: what is under check is what a disable does with a reading, not how one is
/// produced.
#[test]
fn holding_an_account_back_takes_back_the_readiness_this_session_measured() {
    let root = Root::new();
    root.measured(&["claude-opus-5"]);
    root.resolved();

    let mut session = root.session();
    session.set_runtimes(support::report());
    let measured = session.projection(None);
    assert_eq!(
        measured.route.as_deref(),
        Some("claude-code"),
        "the fixture does not stand for an account this session measured ready"
    );

    session.set_provider_enabled(
        ProviderFamily::Anthropic,
        false,
        Some("kept out for this check".to_owned()),
    );

    let withdrawn = session.projection(None);
    assert_eq!(
        withdrawn.route, None,
        "a profile whose account was held back is still the route of the next run"
    );
    let shown = screen(&App::new(withdrawn), 120, 40);
    assert!(
        !shown.contains("ready"),
        "the interface still reads the account as ready after it was held back:\n{shown}"
    );
    assert!(
        !root.path().join("runtimes").join("codex.json").exists()
            || fs::read_to_string(root.path().join("runtimes").join("codex.json"))
                .is_ok_and(|record| !record.contains("2.1.227")),
        "holding an account back measured an engine"
    );
}

/// A goal stated where this host offers models and holds no pool is held, and the reply names that
/// state plainly.
///
/// It should never occur — the pools are resolved wherever a provider is observed — so the second
/// half of this check is the one that matters: with the pool the controller creates in place, the
/// sentence is not stated at all.
#[test]
fn a_goal_typed_with_a_catalog_and_no_pool_is_held_and_names_the_state() {
    let root = Root::new();
    root.measured(&["claude-opus-5"]);

    let mut session = root.session();
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
        stated.contains("holds no pool to draw them from"),
        "the state was not said in plain words:\n{stated}"
    );
    assert!(
        !stated.contains("No AgentPool"),
        "the technical sentence the design forbids was shown:\n{stated}"
    );

    // With the pool the controller creates, the state is gone: the goal is answered without it.
    root.resolved();
    let mut session = root.session();
    session.begin_turn("Создай браузерную игру сапер".to_owned());
    let stated = session
        .projection(None)
        .entries
        .iter()
        .map(|entry| format!("{entry:?}"))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !stated.contains("holds no pool to draw them from"),
        "a root holding the pool still answered a goal with the state that has none:\n{stated}"
    );
}

/// Both surfaces draw the same buffer twice and never exceed the terminal they are given.
#[test]
fn the_pool_surfaces_are_deterministic_and_never_exceed_the_terminal() {
    for (width, height) in SIZES {
        let root = Root::new();
        root.measured(&["claude-opus-5", "claude-sonnet-5"]);
        root.resolved();
        let session = root.session();

        let mut app = App::new(session.projection(None));
        open(&mut app, "pools", height);
        let first = buffer(&app, width, height);
        assert_eq!(first, buffer(&app, width, height), "pools drew two buffers");
        assert_eq!(first.len(), height as usize);
        for line in &first {
            assert_eq!(line.chars().count(), width as usize, "{line:?}");
        }

        open_the_pool(&session, &mut app, height);
        let first = buffer(&app, width, height);
        assert_eq!(first, buffer(&app, width, height), "pool drew two buffers");
        assert_eq!(first.len(), height as usize);
        for line in &first {
            assert_eq!(line.chars().count(), width as usize, "{line:?}");
        }
    }
}
