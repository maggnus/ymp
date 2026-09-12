//! Interface tests.
//!
//! Every test builds its own temporary ymp home and working directory, and none of them
//! reach a provider: the state layer reads the store directly, and anything that could
//! start a turn leaves as an `Action` the test inspects instead of executing.

use crate::prefs::Prefs;
use crate::state::{route, Action, App, Field, Focus, Overlay, Route};
use crate::text;
use crate::theme;
use crate::ui;
use crate::views::View;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::path::PathBuf;
use tempfile::TempDir;
use ymp_core::{new_id, now, Config, Message, Session, UiEvent};
use ymp_storage::Store;

struct Fixture {
    _home: TempDir,
    project: TempDir,
    store: Store,
}

fn fixture() -> Fixture {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    let store = Store::open(home.path()).unwrap();
    Fixture {
        _home: home,
        project,
        store,
    }
}

impl Fixture {
    fn app(&self) -> App {
        App::new(
            self.store.clone(),
            Config::default(),
            PathBuf::from(self.project.path()),
        )
    }
    /// A saved session with two stored messages, as a finished run would leave it.
    fn seed_session(&self, title: &str) -> String {
        let project = self.store.project(self.project.path()).unwrap();
        let session = Session {
            id: new_id(),
            project_id: project.id,
            title: title.to_owned(),
            status: "completed".into(),
            created_at: now(),
            team: Config::default().members(),
            turns_used: 4,
        };
        self.store.save_session(&session).unwrap();
        self.store
            .message(&session.id, "you", None, "user", title)
            .unwrap();
        self.store
            .message(
                &session.id,
                "codex",
                None,
                "summary",
                "Created index.html and verified it with a check.",
            )
            .unwrap();
        session.id
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn control(ch: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL)
}

/// Submit a line as though it had been typed and sent.
fn submit(app: &mut App, text: &str) -> Vec<Action> {
    app.input.set(text);
    app.submit(100)
}

fn typed(app: &mut App, text: &str) {
    for ch in text.chars() {
        app.on_key(key(KeyCode::Char(ch)), 100);
    }
}

fn draw(app: &mut App, width: u16, height: u16) -> String {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

// ---------------------------------------------------------------------------
// Themes
// ---------------------------------------------------------------------------

#[test]
fn a_chosen_theme_is_applied_and_restored_at_the_next_start() {
    let fixture = fixture();
    let mut app = fixture.app();
    assert_eq!(app.theme.id, theme::DEFAULT_THEME);

    app.command("/theme paper", 100);
    assert_eq!(app.theme.id, "paper");
    assert_eq!(Prefs::load(&fixture.store).theme, "paper");

    let restarted = fixture.app();
    assert_eq!(restarted.theme.id, "paper");
}

#[test]
fn every_theme_is_distinct_and_at_least_one_is_light() {
    let ids: Vec<&str> = theme::THEMES.iter().map(|t| t.id).collect();
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(ids.len(), unique.len(), "theme identifiers must be unique");
    assert!(theme::THEMES.len() >= 3);
    assert!(theme::THEMES
        .iter()
        .any(|t| t.kind == theme::ThemeKind::Light));
    for palette in theme::THEMES {
        assert_ne!(palette.bg, palette.text, "{} is unreadable", palette.id);
    }
}

#[test]
fn the_theme_chooser_previews_and_esc_restores_the_stored_choice() {
    let fixture = fixture();
    let mut app = fixture.app();
    let original = app.theme.id;

    app.on_key(control('t'), 100);
    assert!(matches!(app.overlay, Some(Overlay::Themes { .. })));
    app.on_key(key(KeyCode::Down), 100);
    assert_ne!(app.theme.id, original, "moving should preview the palette");

    app.on_key(key(KeyCode::Esc), 100);
    assert!(app.overlay.is_none());
    assert_eq!(app.theme.id, original, "cancelling restores the palette");
    assert!(
        fixture.store.value(crate::prefs::KEY).unwrap().is_none(),
        "a preview must not write a preference"
    );

    app.on_key(control('t'), 100);
    app.on_key(key(KeyCode::Down), 100);
    let previewed = app.theme.id;
    app.on_key(key(KeyCode::Enter), 100);
    assert!(app.overlay.is_none());
    assert_eq!(Prefs::load(&fixture.store).theme, previewed);
}

#[test]
fn an_unknown_theme_is_reported_and_changes_nothing() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.command("/theme nonexistent", 100);
    assert_eq!(app.theme.id, theme::DEFAULT_THEME);
    assert!(app.notices.last().unwrap().failure);
}

// ---------------------------------------------------------------------------
// Command palette, focus and Esc
// ---------------------------------------------------------------------------

#[test]
fn the_command_palette_filters_and_closes_without_side_effects() {
    let fixture = fixture();
    let mut app = fixture.app();

    app.on_key(control('p'), 100);
    let Some(Overlay::Palette { .. }) = &app.overlay else {
        panic!("Ctrl+P must open the palette");
    };
    typed(&mut app, "them");
    let Some(Overlay::Palette { field, .. }) = &app.overlay else {
        panic!("the palette must stay open while typing");
    };
    assert_eq!(field.value, "them");
    assert_eq!(
        crate::commands::search("them").first().unwrap().name,
        "/theme"
    );

    app.on_key(key(KeyCode::Esc), 100);
    assert!(app.overlay.is_none());
    assert_eq!(app.view, View::Chat);
    assert_eq!(app.theme.id, theme::DEFAULT_THEME);
}

#[test]
fn typing_a_command_offers_completions_and_tab_accepts_one() {
    let fixture = fixture();
    let mut app = fixture.app();
    typed(&mut app, "/ses");
    assert_eq!(app.completions().len(), 1);
    app.on_key(key(KeyCode::Tab), 100);
    assert_eq!(app.input.value, "/sessions");
    assert_eq!(app.focus, Focus::Composer, "completion must not move focus");
}

#[test]
fn tab_cycles_one_focus_owner_and_esc_walks_back_to_the_composer() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.viewport.width = 120;
    assert_eq!(app.focus, Focus::Composer);

    app.on_key(key(KeyCode::Tab), 120);
    assert_eq!(app.focus, Focus::Main);
    app.on_key(key(KeyCode::Tab), 120);
    assert_eq!(app.focus, Focus::Sidebar);
    app.on_key(key(KeyCode::Tab), 120);
    assert_eq!(app.focus, Focus::Composer);

    app.on_key(key(KeyCode::Tab), 120);
    app.on_key(key(KeyCode::Esc), 120);
    assert_eq!(app.focus, Focus::Composer);
}

#[test]
fn esc_closes_the_topmost_surface_in_order() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.command("/tasks", 100);
    assert_eq!(app.view, View::Tasks);
    assert_eq!(app.focus, Focus::Main);

    app.on_key(key(KeyCode::Enter), 100);
    app.on_key(control('p'), 100);
    assert!(app.overlay.is_some());

    app.on_key(key(KeyCode::Esc), 100);
    assert!(app.overlay.is_none(), "the overlay closes first");
    assert_eq!(app.view, View::Tasks);

    app.on_key(key(KeyCode::Esc), 100);
    assert_eq!(app.view, View::Chat, "the page closes next");

    typed(&mut app, "draft");
    app.on_key(key(KeyCode::Esc), 100);
    assert!(app.input.is_empty(), "the draft is cleared last");
}

#[test]
fn the_sidebar_opens_a_destination_without_leaving_the_keyboard() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.viewport.width = 120;
    app.on_key(key(KeyCode::Tab), 120);
    app.on_key(key(KeyCode::Tab), 120);
    assert_eq!(app.focus, Focus::Sidebar);
    app.on_key(key(KeyCode::Down), 120);
    let actions = app.on_key(key(KeyCode::Enter), 120);
    assert!(actions.is_empty());
    assert_eq!(app.view, View::Tasks);
    assert_eq!(app.focus, Focus::Main);
}

// ---------------------------------------------------------------------------
// Sessions
// ---------------------------------------------------------------------------

#[test]
fn opening_a_saved_session_reads_it_and_starts_nothing() {
    let fixture = fixture();
    let id = fixture.seed_session("Build a landing page");
    let mut app = fixture.app();

    app.command("/sessions", 100);
    assert_eq!(app.view, View::Sessions);
    assert!(app.page(100).selectable());

    let actions = app.on_key(key(KeyCode::Enter), 100);
    assert!(
        actions.is_empty(),
        "opening a session must not produce a run action"
    );
    assert_eq!(app.session.as_deref(), Some(id.as_str()));
    assert_eq!(app.messages.len(), 2);
    assert_eq!(app.view, View::Chat);
}

#[test]
fn resuming_a_session_is_a_separate_explicit_key() {
    let fixture = fixture();
    let id = fixture.seed_session("Build a landing page");
    let mut app = fixture.app();
    app.command("/sessions", 100);
    let actions = app.on_key(key(KeyCode::Char('r')), 100);
    assert_eq!(actions, vec![Action::Resume { session: id }]);
}

#[test]
fn read_only_pages_never_produce_a_run_action() {
    let fixture = fixture();
    fixture.seed_session("Build a landing page");
    let mut app = fixture.app();
    for view in crate::views::NAV {
        app.set_view(*view);
        for code in [
            KeyCode::Down,
            KeyCode::Up,
            KeyCode::Enter,
            KeyCode::Char(' '),
            KeyCode::End,
        ] {
            for action in app.on_key(key(code), 100) {
                assert!(
                    !matches!(
                        action,
                        Action::StartRun { .. } | Action::FollowUp { .. } | Action::Resume { .. }
                    ),
                    "{view:?} produced {action:?}"
                );
            }
            app.overlay = None;
        }
    }
}

// ---------------------------------------------------------------------------
// Routing
// ---------------------------------------------------------------------------

#[test]
fn an_idle_prompt_continues_the_loaded_conversation() {
    assert_eq!(route("/help", false, None), Route::Command);
    assert_eq!(route("Where is the file?", false, None), Route::NewRun);
    assert_eq!(
        route("Where is the file?", false, Some("s1")),
        Route::FollowUp("s1".into())
    );
    assert_eq!(
        route("Also add tests", true, Some("s1")),
        Route::Queue("s1".into())
    );
    assert_eq!(route("Anything", true, None), Route::Busy);
}

#[test]
fn submitting_follows_up_after_a_session_is_loaded_and_new_starts_a_fresh_run() {
    let fixture = fixture();
    let id = fixture.seed_session("Build a landing page");
    let mut app = fixture.app();

    let actions = submit(&mut app, "Where did you put the file?");
    assert_eq!(
        actions,
        vec![Action::StartRun {
            prompt: "Where did you put the file?".into()
        }]
    );

    app.load_session(&id).unwrap();
    let actions = submit(&mut app, "Where did you put the file?");
    assert_eq!(
        actions,
        vec![Action::FollowUp {
            session: id.clone(),
            prompt: "Where did you put the file?".into()
        }],
        "an idle question must continue the same conversation"
    );

    app.command("/new", 100);
    assert!(app.session.is_none());
    let actions = submit(&mut app, "Unrelated task");
    assert_eq!(
        actions,
        vec![Action::StartRun {
            prompt: "Unrelated task".into()
        }]
    );
}

#[test]
fn a_message_sent_during_a_run_is_queued_for_the_shared_chat() {
    let fixture = fixture();
    let id = fixture.seed_session("Build a landing page");
    let mut app = fixture.app();
    app.load_session(&id).unwrap();
    app.active = true;
    let actions = submit(&mut app, "Please also add a footer");
    assert_eq!(
        actions,
        vec![Action::QueueMessage {
            session: id,
            text: "Please also add a footer".into()
        }]
    );
}

// ---------------------------------------------------------------------------
// Editing
// ---------------------------------------------------------------------------

#[test]
fn editing_never_splits_a_character() {
    let mut field = Field::default();
    for ch in "🦀界a".chars() {
        field.insert(ch);
    }
    assert_eq!(field.value, "🦀界a");
    field.backspace();
    assert_eq!(field.value, "🦀界");
    field.backspace();
    assert_eq!(field.value, "🦀");
    field.left();
    assert_eq!(field.cursor, 0);
    field.right();
    assert_eq!(field.cursor, "🦀".len());
    field.home();
    field.delete();
    assert!(field.value.is_empty());
    assert_eq!(field.cursor, 0);
}

#[test]
fn the_composer_places_the_cursor_on_the_cell_the_next_character_fills() {
    let (lines, position) = text::compose("héllo 界", 20, "héllo ".len());
    assert_eq!(lines, vec!["héllo 界".to_owned()]);
    assert_eq!(position, (0, 6));

    // A double-width character occupies two cells, so the cursor after it is at column two.
    let (_, position) = text::compose("界x", 20, "界".len());
    assert_eq!(position, (0, 2));

    // Hard wrapping keeps every character, including runs of spaces.
    let (lines, position) = text::compose("ab  cd", 3, 6);
    assert_eq!(lines, vec!["ab ".to_owned(), " cd".to_owned()]);
    assert_eq!(position, (1, 3));

    let (lines, _) = text::compose("one\ntwo", 20, 0);
    assert_eq!(lines, vec!["one".to_owned(), "two".to_owned()]);
}

#[test]
fn unicode_survives_a_round_trip_through_the_transcript() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.event(UiEvent::Message(Message {
        seq: 1,
        session_id: "s1".into(),
        author: "you".into(),
        recipient: None,
        kind: "user".into(),
        text: "Проверь файл 界面.txt 🦀".into(),
        created_at: now(),
    }));
    // A double-width character fills one cell and blanks the next, so compare without
    // the padding the test backend inserts.
    let rendered: String = draw(&mut app, 100, 24).replace(' ', "");
    assert!(rendered.contains("Проверьфайл"), "{rendered}");
    assert!(rendered.contains("界面.txt"), "{rendered}");
    assert!(rendered.contains('🦀'));
}

// ---------------------------------------------------------------------------
// Transcript, scrolling and collapsing
// ---------------------------------------------------------------------------

fn conversation(app: &mut App, count: i64) {
    for seq in 1..=count {
        app.event(UiEvent::Message(Message {
            seq,
            session_id: "s1".into(),
            author: if seq % 2 == 0 { "codex" } else { "you" }.into(),
            recipient: None,
            kind: if seq % 2 == 0 { "summary" } else { "user" }.into(),
            text: format!("Message number {seq} with enough text to occupy a row."),
            created_at: now(),
        }));
    }
}

#[test]
fn streaming_text_does_not_move_the_reading_position() {
    let fixture = fixture();
    let mut app = fixture.app();
    conversation(&mut app, 40);
    draw(&mut app, 100, 24);
    assert!(app.follow);

    app.on_key(key(KeyCode::PageUp), 100);
    draw(&mut app, 100, 24);
    assert!(!app.follow, "scrolling up pauses auto-follow");
    let anchored = app.top;

    for chunk in ["Streaming ", "more ", "text "] {
        app.event(UiEvent::Delta {
            agent: "codex".into(),
            text: chunk.into(),
        });
        draw(&mut app, 100, 24);
    }
    assert_eq!(app.top, anchored, "appended text must not scroll the view");

    app.on_key(key(KeyCode::End), 100);
    draw(&mut app, 100, 24);
    assert!(app.follow, "End returns to the latest message");
    assert_eq!(
        app.top,
        app.viewport.total.saturating_sub(app.viewport.height),
        "the newest message is at the bottom of the viewport"
    );
}

#[test]
fn the_paused_state_and_its_remedy_are_shown() {
    let fixture = fixture();
    let mut app = fixture.app();
    conversation(&mut app, 40);
    draw(&mut app, 100, 24);
    app.on_key(key(KeyCode::PageUp), 100);
    let rendered = draw(&mut app, 100, 24);
    assert!(rendered.contains("paused"), "{rendered}");
    assert!(rendered.contains("End"));
}

#[test]
fn routine_coordination_is_collapsed_and_routing_is_hidden() {
    let fixture = fixture();
    let mut app = fixture.app();
    let messages = [
        ("you", "user", "Build a landing page".to_owned()),
        (
            "codex",
            "plan",
            serde_json::json!({"summary":"Create one page","tasks":[{"title":"a"}]}).to_string(),
        ),
        (
            "claude",
            "review",
            serde_json::json!({"approved":true,"reason":"The page renders"}).to_string(),
        ),
        (
            "codex",
            "conversation",
            serde_json::json!({"action":"answer","answer":"Internal routing"}).to_string(),
        ),
    ];
    for (index, (author, kind, body)) in messages.iter().enumerate() {
        app.event(UiEvent::Message(Message {
            seq: index as i64 + 1,
            session_id: "s1".into(),
            author: (*author).into(),
            recipient: None,
            kind: (*kind).into(),
            text: body.clone(),
            created_at: now(),
        }));
    }
    let rendered = draw(&mut app, 110, 30);
    assert!(rendered.contains("Build a landing page"));
    assert!(rendered.contains("proposed a plan"), "{rendered}");
    assert!(rendered.contains("accepted"));
    assert!(
        !rendered.contains("\"approved\""),
        "raw JSON must not reach the transcript"
    );
    assert!(
        !rendered.contains("Internal routing"),
        "conversation routing must be hidden"
    );

    // The full attributed text stays reachable.
    app.prefs.details = true;
    let entries =
        crate::transcript::build(&app.messages, &[], &Default::default(), &app.config, true);
    assert_eq!(entries.len(), 4, "detailed mode shows every message");
    assert!(entries.iter().any(|e| e.raw.contains("Internal routing")));
}

#[test]
fn an_entry_can_be_inspected_in_full() {
    let fixture = fixture();
    let mut app = fixture.app();
    conversation(&mut app, 4);
    draw(&mut app, 100, 24);
    app.on_key(key(KeyCode::Tab), 100);
    assert_eq!(app.focus, Focus::Main);
    app.on_key(key(KeyCode::Up), 100);
    app.on_key(key(KeyCode::Enter), 100);
    assert!(matches!(app.overlay, Some(Overlay::Inspect { .. })));
    let rendered = draw(&mut app, 100, 24);
    assert!(rendered.contains("read only"), "{rendered}");
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

#[test]
fn representative_viewports_render_without_clipping_the_composer() {
    let fixture = fixture();
    fixture.seed_session("Build a landing page");
    for (width, height) in [(40, 12), (60, 18), (80, 24), (100, 30), (120, 40)] {
        let mut app = fixture.app();
        conversation(&mut app, 12);
        typed(&mut app, "hello");
        let rendered = draw(&mut app, width, height);
        assert!(
            rendered.contains("hello"),
            "the composer was lost at {width}x{height}"
        );
        if height >= 10 {
            assert!(
                rendered.contains("ymp"),
                "the header was lost at {width}x{height}"
            );
        }
    }
}

#[test]
fn the_sidebar_is_present_from_eighty_columns_and_can_be_hidden() {
    let fixture = fixture();
    let mut app = fixture.app();
    for width in [80u16, 100, 120, 180] {
        let rendered = draw(&mut app, width, 24);
        assert!(
            rendered.contains("NAVIGATE"),
            "no sidebar at {width} columns"
        );
        assert!(rendered.contains("TEAM"), "no team activity at {width}");
    }
    app.command("/sidebar", 100);
    let rendered = draw(&mut app, 120, 30);
    assert!(!rendered.contains("NAVIGATE"));
    assert!(!Prefs::load(&fixture.store).sidebar);
}

#[test]
fn every_page_renders_at_the_smallest_supported_size() {
    let fixture = fixture();
    fixture.seed_session("Build a landing page");
    let mut app = fixture.app();
    for view in crate::views::NAV {
        app.set_view(*view);
        for (width, height) in [(80u16, 24u16), (40, 12), (120, 40)] {
            let rendered = draw(&mut app, width, height);
            assert!(
                !rendered.is_empty(),
                "{view:?} produced nothing at {width}x{height}"
            );
        }
    }
}

#[test]
fn the_working_directory_is_always_visible() {
    let fixture = fixture();
    let mut app = fixture.app();
    let rendered = draw(&mut app, 120, 30);
    let name = fixture
        .project
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(rendered.contains(&name), "{rendered}");
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[test]
fn team_membership_and_limits_are_saved_and_reloaded() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.command("/team remove claude", 100);
    assert!(!app.config.team.contains(&"claude".to_owned()));
    let reloaded = Config::load(&fixture.store.home).unwrap();
    assert!(!reloaded.team.contains(&"claude".to_owned()));

    app.command("/limits turns 42", 100);
    assert_eq!(app.config.limits.turns, 42);
    assert_eq!(Config::load(&fixture.store.home).unwrap().limits.turns, 42);
}

#[test]
fn a_rejected_configuration_change_is_reverted_and_reported() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.command("/limits turns 0", 100);
    assert_eq!(app.config.limits.turns, Config::default().limits.turns);
    assert!(app.notices.last().unwrap().failure);
}

#[test]
fn an_agent_model_and_instructions_can_be_edited_from_the_page() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.command("/agents", 100);
    app.on_key(key(KeyCode::Char('m')), 100);
    let Some(Overlay::Prompt { .. }) = &app.overlay else {
        panic!("m must open the model editor");
    };
    for ch in "gpt-5".chars() {
        app.on_key(key(KeyCode::Char(ch)), 100);
    }
    app.on_key(key(KeyCode::Enter), 100);
    assert_eq!(
        app.config.agent("codex").unwrap().model.as_deref(),
        Some("gpt-5")
    );
    assert_eq!(
        Config::load(&fixture.store.home)
            .unwrap()
            .agent("codex")
            .unwrap()
            .model
            .as_deref(),
        Some("gpt-5")
    );

    app.command("/agent instructions codex Verify every claim", 100);
    assert_eq!(
        app.config.agent("codex").unwrap().instructions,
        "Verify every claim"
    );
}

#[test]
fn an_unknown_command_is_reported_without_changing_the_view() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.command("/nope", 100);
    assert_eq!(app.view, View::Chat);
    let notice = app.notices.last().unwrap();
    assert!(notice.failure);
    assert!(notice.text.contains("/nope"));
}

#[test]
fn help_lists_every_command_in_the_registry() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.set_view(View::Help);
    let keys: Vec<String> = app.page(100).items.iter().map(|i| i.key.clone()).collect();
    for command in crate::commands::COMMANDS {
        assert!(
            keys.iter().any(|k| k == command.name),
            "{} is missing from help",
            command.name
        );
    }
}

#[test]
fn every_theme_renders_a_complete_screen() {
    let fixture = fixture();
    for palette in theme::THEMES {
        let mut app = fixture.app();
        app.command(&format!("/theme {}", palette.id), 120);
        assert_eq!(app.theme.id, palette.id);
        conversation(&mut app, 6);
        for view in crate::views::NAV {
            app.set_view(*view);
            let rendered = draw(&mut app, 100, 30);
            assert!(
                rendered.contains("NAVIGATE"),
                "{} lost the sidebar on {view:?}",
                palette.id
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Enter and command arguments
// ---------------------------------------------------------------------------

/// Type a command and press Enter the way a person does, through the key handler.
fn type_and_enter(app: &mut App, command: &str) -> Vec<Action> {
    typed(app, command);
    app.on_key(key(KeyCode::Enter), 100)
}

#[test]
fn enter_runs_a_command_whose_arguments_are_optional() {
    let fixture = fixture();
    let mut app = fixture.app();

    type_and_enter(&mut app, "/theme");
    assert!(
        matches!(app.overlay, Some(Overlay::Themes { .. })),
        "one Enter on /theme must open the chooser, not leave a draft"
    );
    assert!(app.input.is_empty());
    app.on_key(key(KeyCode::Esc), 100);

    for (command, view) in [
        ("/team", View::Team),
        ("/memory", View::Memory),
        ("/limits", View::Limits),
        ("/tasks", View::Tasks),
        ("/sessions", View::Sessions),
        ("/help", View::Help),
    ] {
        app.set_view(View::Chat);
        app.focus = Focus::Composer;
        type_and_enter(&mut app, command);
        assert_eq!(app.view, view, "{command} did not run on one Enter");
        assert!(app.input.is_empty(), "{command} left a draft behind");
    }
}

#[test]
fn enter_on_resume_without_an_argument_reports_instead_of_drafting() {
    let fixture = fixture();
    let mut app = fixture.app();
    let actions = type_and_enter(&mut app, "/resume");
    assert!(actions.is_empty());
    assert!(app.input.is_empty(), "/resume must not leave a draft");
    assert!(
        app.notices.last().unwrap().failure,
        "with no session to continue, /resume must say so"
    );
}

#[test]
fn enter_drafts_only_a_command_that_cannot_act_without_an_argument() {
    let fixture = fixture();
    let mut app = fixture.app();
    let actions = type_and_enter(&mut app, "/agent");
    assert!(actions.is_empty());
    assert_eq!(app.input.value, "/agent ", "/agent needs an argument typed");
    assert_eq!(app.view, View::Chat);
    assert!(app.notices.is_empty(), "drafting is not an error");
}

#[test]
fn enter_completes_a_partial_name_and_runs_it() {
    let fixture = fixture();
    let mut app = fixture.app();
    type_and_enter(&mut app, "/thm");
    assert_eq!(
        app.view,
        View::Chat,
        "a name that matches nothing is not a command"
    );
    app.input.clear();

    type_and_enter(&mut app, "/the");
    assert!(
        matches!(app.overlay, Some(Overlay::Themes { .. })),
        "a unique prefix of an optional-argument command still runs on one Enter"
    );
}

#[test]
fn the_palette_runs_optional_argument_commands_and_drafts_required_ones() {
    let fixture = fixture();
    let mut app = fixture.app();

    app.on_key(control('p'), 100);
    typed(&mut app, "theme");
    app.on_key(key(KeyCode::Enter), 100);
    assert!(
        matches!(app.overlay, Some(Overlay::Themes { .. })),
        "the palette must run /theme, not hand back a draft"
    );
    app.on_key(key(KeyCode::Esc), 100);

    app.on_key(control('p'), 100);
    typed(&mut app, "agent");
    // "agent" also prefixes /agents, so the required-argument command is one row down.
    app.on_key(key(KeyCode::Down), 100);
    app.on_key(key(KeyCode::Enter), 100);
    assert!(app.overlay.is_none());
    assert_eq!(app.input.value, "/agent ");
    assert_eq!(app.focus, Focus::Composer);
}

// ---------------------------------------------------------------------------
// Paste
// ---------------------------------------------------------------------------

#[test]
fn a_paste_reaches_the_field_that_owns_the_keyboard() {
    let fixture = fixture();
    let mut app = fixture.app();

    // The agent instructions editor, with Unicode and more than one line.
    app.command("/agents", 100);
    app.on_key(key(KeyCode::Char('i')), 100);
    app.on_key(control('u'), 100);
    crate::handle(
        &mut app,
        Event::Paste("Проверяй 界面\nи ссылки 🦀".to_owned()),
    );
    let Some(Overlay::Prompt { field, .. }) = &app.overlay else {
        panic!("the editor must still own the keyboard");
    };
    assert_eq!(field.value, "Проверяй 界面\nи ссылки 🦀");
    assert!(
        app.input.is_empty(),
        "pasted text must not reach the conversation composer"
    );
    app.on_key(key(KeyCode::Enter), 100);
    assert_eq!(
        app.config.agent("codex").unwrap().instructions,
        "Проверяй 界面\nи ссылки 🦀"
    );

    // The command palette.
    app.on_key(control('p'), 100);
    crate::handle(&mut app, Event::Paste("theme".to_owned()));
    let Some(Overlay::Palette { field, .. }) = &app.overlay else {
        panic!("the palette must still own the keyboard");
    };
    assert_eq!(field.value, "theme");
    assert!(app.input.is_empty());
    app.on_key(key(KeyCode::Esc), 100);

    // Nothing open: the composer takes it and the focus follows.
    app.set_view(View::Tasks);
    assert_eq!(app.focus, Focus::Main);
    crate::handle(&mut app, Event::Paste("a task\r\nsecond line".to_owned()));
    assert_eq!(app.input.value, "a task\nsecond line");
    assert_eq!(app.focus, Focus::Composer);
}

#[test]
fn cancelling_an_editor_discards_the_text_pasted_into_it() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.command("/agents", 100);
    app.on_key(key(KeyCode::Char('m')), 100);
    crate::handle(&mut app, Event::Paste("secret-internal-model".to_owned()));

    app.on_key(key(KeyCode::Esc), 100);
    assert!(app.overlay.is_none());
    assert!(
        app.input.is_empty(),
        "a cancelled editor must not spill its draft into the composer"
    );
    assert_eq!(
        app.config.agent("codex").unwrap().model,
        None,
        "cancelling must not save"
    );
    let rendered = draw(&mut app, 100, 30);
    assert!(!rendered.contains("secret-internal-model"));
}

#[test]
fn a_paste_into_a_confirmation_is_ignored() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.overlay = Some(Overlay::Confirm {
        question: "Retire it?".into(),
        target: crate::state::Confirm::ForgetMemory("id".into()),
    });
    crate::handle(&mut app, Event::Paste("stray text".to_owned()));
    assert!(app.input.is_empty());
    assert!(matches!(app.overlay, Some(Overlay::Confirm { .. })));
}

// ---------------------------------------------------------------------------
// Session identity during a run
// ---------------------------------------------------------------------------

#[test]
fn a_message_cannot_be_queued_to_a_session_the_run_is_not_working_on() {
    let fixture = fixture();
    let running = fixture.seed_session("The run that is active");
    let other = fixture.seed_session("A different saved session");
    let mut app = fixture.app();
    app.load_session(&running).unwrap();
    app.active = true;

    app.command("/sessions", 100);
    let selected = app.selected_item(100).unwrap().key;
    assert_eq!(selected, other, "the newest session is selected first");

    let actions = app.on_key(key(KeyCode::Enter), 100);
    assert!(actions.is_empty());
    assert_eq!(
        app.session.as_deref(),
        Some(running.as_str()),
        "a run in progress keeps its conversation"
    );
    assert!(app.notices.last().unwrap().failure);

    let actions = submit(&mut app, "Also add a footer");
    assert_eq!(
        actions,
        vec![Action::QueueMessage {
            session: running.clone(),
            text: "Also add a footer".into()
        }],
        "the message must reach the run that is active"
    );

    // Resuming another session is refused for the same reason.
    app.command("/sessions", 100);
    let actions = app.on_key(key(KeyCode::Char('r')), 100);
    assert!(actions.is_empty());
    assert!(app.notices.last().unwrap().failure);
}

#[test]
fn browsing_sessions_during_a_run_stays_available() {
    let fixture = fixture();
    let running = fixture.seed_session("The run that is active");
    fixture.seed_session("A different saved session");
    let mut app = fixture.app();
    app.load_session(&running).unwrap();
    app.active = true;
    app.command("/sessions", 100);
    let rendered = draw(&mut app, 110, 30);
    assert!(rendered.contains("A different saved session"), "{rendered}");
    assert!(rendered.contains("The run that is active"));
}

// ---------------------------------------------------------------------------
// Sidebar selection
// ---------------------------------------------------------------------------

#[test]
fn the_focused_sidebar_keeps_its_selection_visible_at_every_size() {
    let fixture = fixture();
    for (width, height) in [(80u16, 24u16), (100, 24), (120, 30), (80, 14)] {
        let mut app = fixture.app();
        app.viewport.width = width as usize;
        draw(&mut app, width, height);
        app.on_key(key(KeyCode::Tab), width);
        app.on_key(key(KeyCode::Tab), width);
        assert_eq!(app.focus, Focus::Sidebar);

        app.on_key(key(KeyCode::End), width);
        assert_eq!(app.nav, crate::views::NAV.len() - 1);
        let rendered = draw(&mut app, width, height);
        assert!(
            rendered.contains("› Help"),
            "the selected destination vanished at {width}x{height}:\n{rendered}"
        );

        app.on_key(key(KeyCode::Home), width);
        let rendered = draw(&mut app, width, height);
        assert!(
            rendered.contains("› Conversation"),
            "the selection vanished at the top at {width}x{height}"
        );
    }
}

// ---------------------------------------------------------------------------
// Whitespace fidelity
// ---------------------------------------------------------------------------

#[test]
fn wrapping_keeps_whitespace_that_carries_meaning() {
    assert_eq!(
        text::wrap(r#"print("a  b")"#, 40),
        vec![r#"print("a  b")"#.to_owned()],
        "repeated spaces inside a line are content"
    );
    assert_eq!(
        text::wrap("    indented  body", 40),
        vec!["    indented  body".to_owned()],
        "leading indentation is content"
    );
    assert_eq!(
        text::wrap("界面  测试", 40),
        vec!["界面  测试".to_owned()],
        "double-width text keeps its spacing"
    );
    // Exact wrapping loses nothing at all, which is what code needs.
    let exact = text::wrap_exact("a  b   c", 4);
    assert_eq!(exact.concat(), "a  b   c");
    assert!(exact.iter().all(|line| text::width(line) <= 4));
    let wide = text::wrap_exact("界面测试", 4);
    assert_eq!(wide, vec!["界面".to_owned(), "测试".to_owned()]);
    assert_eq!(
        text::wrap_exact("\nb", 4),
        vec![String::new(), "b".to_owned()],
        "a leading blank line is a line"
    );
}

#[test]
fn a_fenced_code_block_is_rendered_character_for_character() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.event(UiEvent::Message(Message {
        seq: 1,
        session_id: "s1".into(),
        author: "codex".into(),
        recipient: None,
        kind: "answer".into(),
        text: "Added a check:\n\n```python\nprint(\"a  b\")\n    if x:  pass\n```".into(),
        created_at: now(),
    }));
    let rendered = draw(&mut app, 110, 30);
    assert!(
        rendered.contains(r#"print("a  b")"#),
        "the repeated space was lost:\n{rendered}"
    );
    assert!(
        rendered.contains("if x:  pass"),
        "the indented line lost its spacing:\n{rendered}"
    );
}

// ---------------------------------------------------------------------------
// Transcript presentation
// ---------------------------------------------------------------------------

fn post(app: &mut App, seq: i64, author: &str, kind: &str, text: &str) {
    app.event(UiEvent::Message(Message {
        seq,
        session_id: "s1".into(),
        author: author.into(),
        recipient: None,
        kind: kind.into(),
        text: text.to_owned(),
        created_at: now(),
    }));
}

#[test]
fn a_plan_review_is_summarised_like_every_other_review() {
    let fixture = fixture();
    let mut app = fixture.app();
    post(
        &mut app,
        1,
        "claude",
        "review_plan",
        &serde_json::json!({"approved": false, "reason": "The plan skips the parser"}).to_string(),
    );
    let rendered = draw(&mut app, 110, 30);
    assert!(
        rendered.contains("review · rejected · The plan skips the parser"),
        "{rendered}"
    );
    assert!(
        !rendered.contains("\"approved\""),
        "raw JSON reached the transcript:\n{rendered}"
    );
}

#[test]
fn an_execution_turn_is_labelled_as_a_report_not_as_a_change() {
    let fixture = fixture();
    let mut app = fixture.app();
    post(
        &mut app,
        1,
        "codex",
        "execute",
        "Inspected the parser. No change was necessary; the behaviour is already correct.",
    );
    let rendered = draw(&mut app, 110, 30);
    assert!(rendered.contains("execution report"), "{rendered}");
    assert!(
        !rendered.contains("changed the working directory"),
        "a read-only analysis must not be reported as a change"
    );
}

#[test]
fn a_single_line_stream_cannot_fill_the_transcript() {
    let fixture = fixture();
    let mut app = fixture.app();
    conversation(&mut app, 2);
    let huge = format!(
        "{{\"tasks\":[{}]}}",
        (0..400)
            .map(|i| format!("{{\"title\":\"task number {i}\"}}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    app.event(UiEvent::Delta {
        agent: "codex".into(),
        text: huge,
    });
    draw(&mut app, 100, 30);
    let stream = app
        .viewport
        .entries
        .last()
        .copied()
        .expect("the stream entry is rendered");
    assert!(
        stream.1 <= 5,
        "one line of JSON occupied {} rows of the transcript",
        stream.1
    );
}

// ---------------------------------------------------------------------------
// Team identity
// ---------------------------------------------------------------------------

#[test]
fn the_sidebar_shows_the_team_the_session_captured_not_the_edited_configuration() {
    let fixture = fixture();
    let id = fixture.seed_session("Build a landing page");
    let mut app = fixture.app();
    app.load_session(&id).unwrap();

    let (members, captured) = app.active_team();
    assert!(captured);
    assert_eq!(members.len(), 2);

    // Editing the configuration describes the next run, not this one.
    app.command("/team remove claude", 100);
    assert_eq!(app.config.members().len(), 1);
    let (members, captured) = app.active_team();
    assert!(captured, "a loaded session keeps the team it captured");
    assert_eq!(members.len(), 2);

    let rendered = draw(&mut app, 120, 30);
    assert!(rendered.contains("this session"), "{rendered}");
    assert!(
        rendered.contains("Claude"),
        "the team actually in use disappeared from the sidebar"
    );

    // With no session loaded, the sidebar describes the next run instead.
    app.command("/new", 100);
    let (members, captured) = app.active_team();
    assert!(!captured);
    assert_eq!(members.len(), 1);
    let rendered = draw(&mut app, 120, 30);
    assert!(rendered.contains("next run"), "{rendered}");
}

#[test]
fn instructions_editor_preserves_multiline_content_and_keeps_the_cursor_visible() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.command("/agents", 80);
    app.on_key(key(KeyCode::Char('i')), 80);
    app.on_key(control('u'), 80);
    let instructions = "Keep exact whitespace.\n\n    print(\"a  b\")\nUnicode café 界面";
    crate::handle(&mut app, Event::Paste(instructions.into()));
    let rendered = draw(&mut app, 80, 24);
    assert!(rendered.contains("print(\"a  b\")"));
    app.on_key(control('j'), 80);
    app.on_key(key(KeyCode::Char('x')), 80);
    app.on_key(key(KeyCode::Enter), 80);
    let expected = format!("{instructions}\nx");
    assert_eq!(app.config.agent("codex").unwrap().instructions, expected);
    assert_eq!(
        Config::load(&app.store.home)
            .unwrap()
            .agent("codex")
            .unwrap()
            .instructions,
        expected
    );
}
