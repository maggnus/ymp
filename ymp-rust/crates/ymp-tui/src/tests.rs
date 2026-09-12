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
use ymp_core::{
    new_id, now, AgentProfile, Config, Message, Session, SessionUsage, Task, TaskAttemptRef,
    TaskState, TokenCounts, UiEvent, UsageSnapshot,
};
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
    /// A saved session with a chosen team and no turns of its own, so its statistics are
    /// exactly the invocations a test records.
    fn seed_with_team(&self, title: &str, team: Vec<AgentProfile>) -> String {
        self.seed_counted(title, team, 0)
    }
    /// A saved session that claims `turns_used` turns. An older trace can count its turns
    /// without recording which agent took each one.
    fn seed_counted(&self, title: &str, team: Vec<AgentProfile>, turns_used: usize) -> String {
        let project = self.store.project(self.project.path()).unwrap();
        let session = Session {
            id: new_id(),
            project_id: project.id,
            title: title.to_owned(),
            status: "completed".into(),
            created_at: now(),
            team,
            turns_used,
        };
        self.store.save_session(&session).unwrap();
        session.id
    }
    /// Record one invocation the way a run does. `counts` of `None` reports nothing, and
    /// `status` of `None` leaves the invocation without a final status.
    fn record(
        &self,
        session: &str,
        turn: u64,
        agent: &str,
        counts: Option<(u64, u64)>,
        status: Option<&str>,
    ) {
        self.store.begin_usage(session, turn, agent).unwrap();
        if let Some((input, output)) = counts {
            self.store
                .update_usage(
                    session,
                    turn,
                    &UsageSnapshot {
                        counts: TokenCounts {
                            input: Some(input),
                            output: Some(output),
                            cache_read: Some(0),
                            cache_write: Some(0),
                            reasoning: None,
                        },
                        finalized: true,
                        partial: false,
                        note: None,
                        native_total: None,
                    },
                )
                .unwrap();
        }
        if let Some(status) = status {
            self.store.finish_usage(session, turn, status).unwrap();
        }
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

/// The plain text of a block of rendered lines, as one line: prose is wrapped to the
/// width it was built for, so an assertion about wording must not depend on where it broke.
fn lines_prose(lines: &[ratatui::text::Line<'static>]) -> String {
    text::one_line(&lines_text(lines))
}

/// The plain text of a block of rendered lines.
fn lines_text(lines: &[ratatui::text::Line<'static>]) -> String {
    lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
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

// ---------------------------------------------------------------------------
// Token statistics
// ---------------------------------------------------------------------------

/// One recorded invocation: the agent that ran, the input and output counts it reported,
/// and whether it was left without a final status.
type Invocation<'a> = (&'a str, Option<(u64, u64)>, bool);

/// A snapshot as the runtime publishes one: one entry per invocation, summed into the
/// session total the same way the store sums it.
///
/// `counts` of `None` is an invocation that reported nothing, which is unknown rather than
/// zero. `open` marks an invocation with no final status recorded.
fn snapshot(invocations: &[Invocation]) -> SessionUsage {
    let mut usage = SessionUsage::default();
    for (agent, counts, open) in invocations {
        let reported = counts.map(|(input, output)| UsageSnapshot {
            counts: TokenCounts {
                input: Some(input),
                output: Some(output),
                cache_read: Some(input / 2),
                cache_write: Some(0),
                reasoning: Some(output / 4),
            },
            finalized: !*open,
            partial: false,
            note: None,
            native_total: None,
        });
        usage.total.include(reported.as_ref(), *open);
        usage
            .agents
            .entry((*agent).to_owned())
            .or_default()
            .include(reported.as_ref(), *open);
    }
    usage
}

fn usage_event(session: &str, invocations: &[Invocation]) -> UiEvent {
    UiEvent::Usage {
        session_id: session.to_owned(),
        usage: snapshot(invocations),
    }
}

/// The keys of the selectable rows on the open page.
fn row_keys(app: &mut App, width: u16) -> Vec<String> {
    app.page(width)
        .items
        .iter()
        .filter(|item| item.kind == crate::views::ItemKind::Row)
        .map(|item| item.key.clone())
        .collect()
}

#[test]
fn a_snapshot_replaces_the_previous_one_instead_of_adding_to_it() {
    let fixture = fixture();
    let mut app = fixture.app();
    conversation(&mut app, 2);

    app.event(usage_event("s1", &[("codex", Some((1_000, 200)), false)]));
    assert_eq!(app.stats.total().unwrap().known_total(), Some(1_200));

    // The same invocation, reported again with a larger figure.
    app.event(usage_event("s1", &[("codex", Some((1_500, 300)), false)]));
    let total = app.stats.total().unwrap();
    assert_eq!(
        total.known_total(),
        Some(1_800),
        "snapshots must not accumulate"
    );
    assert_eq!(total.calls, 1);
    assert_eq!(app.stats.agent("codex").unwrap().known_total(), Some(1_800));
}

#[test]
fn a_live_snapshot_reaches_the_header_before_the_run_finishes() {
    let fixture = fixture();
    let mut app = fixture.app();
    conversation(&mut app, 2);
    app.active = true;

    app.event(usage_event("s1", &[("codex", Some((12_000, 345)), true)]));
    let rendered = draw(&mut app, 120, 30);
    assert!(
        rendered.contains("12.3k+ tokens"),
        "the running total is missing from the header:\n{rendered}"
    );
    assert!(app.active, "showing statistics must not end the run");

    // A reader who scrolled back stays where they were while the figures move.
    conversation(&mut app, 40);
    draw(&mut app, 120, 30);
    app.on_key(key(KeyCode::PageUp), 120);
    draw(&mut app, 120, 30);
    let anchored = app.top;
    app.event(usage_event("s1", &[("codex", Some((14_000, 400)), true)]));
    draw(&mut app, 120, 30);
    assert_eq!(app.top, anchored, "new statistics moved the transcript");
    assert!(!app.follow);
}

#[test]
fn opening_another_session_shows_the_statistics_that_session_recorded() {
    let fixture = fixture();
    let first = fixture.seed_with_team("First", Config::default().members());
    let second = fixture.seed_with_team("Second", Config::default().members());
    fixture.record(&first, 1, "codex", Some((900, 100)), Some("completed"));
    fixture.record(
        &second,
        1,
        "claude",
        Some((40_000, 2_000)),
        Some("completed"),
    );
    let mut app = fixture.app();

    app.load_session(&first).unwrap();
    assert_eq!(app.stats.total().unwrap().known_total(), Some(1_000));

    app.load_session(&second).unwrap();
    assert_eq!(app.stats.total().unwrap().known_total(), Some(42_000));
    assert_eq!(
        app.stats.agent("codex").unwrap().known_total(),
        Some(0),
        "an agent this session never invoked has spent nothing"
    );

    // A snapshot for the conversation that is no longer open belongs to another run.
    app.event(usage_event(
        &first,
        &[("codex", Some((5_000, 5_000)), false)],
    ));
    assert_eq!(app.stats.total().unwrap().known_total(), Some(42_000));
}

#[test]
fn statistics_are_forgotten_when_the_conversation_is() {
    let fixture = fixture();
    let id = fixture.seed_with_team("First", Config::default().members());
    fixture.record(&id, 1, "codex", Some((900, 100)), Some("completed"));
    let mut app = fixture.app();
    app.load_session(&id).unwrap();
    assert!(app.stats.total().is_some());

    app.command("/new", 100);
    assert!(app.stats.total().is_none());
    let rendered = draw(&mut app, 120, 30);
    assert!(rendered.contains("no session"), "{rendered}");
}

#[test]
fn two_agents_on_one_provider_are_counted_apart() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.command("/agent add reviewer codex", 100);
    app.command("/team add reviewer", 100);
    conversation(&mut app, 2);

    app.event(usage_event(
        "s1",
        &[
            ("codex", Some((10_000, 1_000)), false),
            ("reviewer", Some((2_000, 500)), false),
        ],
    ));
    assert_eq!(
        app.stats.agent("codex").unwrap().known_total(),
        Some(11_000)
    );
    assert_eq!(
        app.stats.agent("reviewer").unwrap().known_total(),
        Some(2_500)
    );

    app.command("/usage", 120);
    let keys = row_keys(&mut app, 118);
    assert!(
        keys.contains(&"codex".to_owned()) && keys.contains(&"reviewer".to_owned()),
        "agents sharing a provider were folded together: {keys:?}"
    );
    let rendered = draw(&mut app, 120, 30);
    assert!(rendered.contains("11.0k"), "{rendered}");
    assert!(rendered.contains("2500"), "{rendered}");
}

#[test]
fn unknown_is_not_zero_and_a_growing_total_says_so() {
    let fixture = fixture();
    let mut app = fixture.app();
    conversation(&mut app, 2);

    // Nothing has been invoked, so zero is a figure the store knows.
    app.event(UiEvent::Usage {
        session_id: "s1".into(),
        usage: SessionUsage::default(),
    });
    assert_eq!(app.stats.total().unwrap().known_total(), Some(0));
    let rendered = draw(&mut app, 120, 30);
    assert!(rendered.contains("0 tokens"), "{rendered}");

    // An invocation that reported nothing leaves the amount unknown.
    app.event(usage_event("s1", &[("codex", None, false)]));
    assert_eq!(app.stats.total().unwrap().known_total(), None);
    let rendered = draw(&mut app, 120, 30);
    assert!(
        rendered.contains("— tokens"),
        "unknown was shown as a number:\n{rendered}"
    );

    // A reported figure with an invocation still open is a lower bound.
    app.event(usage_event("s1", &[("codex", Some((12_000, 345)), true)]));
    let rendered = draw(&mut app, 120, 30);
    assert!(rendered.contains("12.3k+ tokens"), "{rendered}");
}

#[test]
fn a_provider_that_reports_part_of_a_turn_is_marked_partial() {
    let fixture = fixture();
    let mut app = fixture.app();
    conversation(&mut app, 2);

    // One installed provider reports only the last request of a turn; the rest of the
    // turn is unaccounted for, and the interface must not present it as complete.
    let reported = UsageSnapshot {
        counts: TokenCounts {
            input: Some(800),
            output: Some(200),
            cache_read: Some(700),
            cache_write: Some(0),
            reasoning: None,
        },
        finalized: true,
        partial: true,
        note: Some("only the last request of the turn".into()),
        native_total: None,
    };
    let mut usage = SessionUsage::default();
    usage.total.include(Some(&reported), false);
    usage
        .agents
        .entry("glm".into())
        .or_default()
        .include(Some(&reported), false);
    app.event(UiEvent::Usage {
        session_id: "s1".into(),
        usage,
    });

    app.command("/usage", 120);
    let rendered = draw(&mut app, 120, 30);
    assert!(
        rendered.contains("1000+"),
        "a partial figure was shown as exact:\n{rendered}"
    );
    assert!(rendered.contains("1 partial"), "{rendered}");
    assert!(app.stats.total().unwrap().is_partial());
}

#[test]
fn an_open_invocation_in_a_stored_session_is_not_called_running() {
    let fixture = fixture();
    let id = fixture.seed_with_team("Interrupted", Config::default().members());
    // An invocation with no final status: what an interrupted run leaves behind.
    fixture.record(&id, 1, "codex", Some((5_000, 500)), None);
    let mut app = fixture.app();
    app.load_session(&id).unwrap();
    assert!(!app.active);

    app.command("/usage", 120);
    let detail = app
        .page(118)
        .items
        .iter()
        .find(|item| item.key == "session")
        .map(|item| lines_text(&item.detail))
        .unwrap();
    assert!(detail.contains("open"), "{detail}");
    let prose = text::one_line(&detail);
    assert!(
        prose.contains("no final status was recorded"),
        "an unfinalised invocation was presented as work in progress:\n{detail}"
    );
    assert!(!prose.contains("active run has not finished"), "{detail}");
}

#[test]
fn the_statistics_page_reads_and_starts_nothing() {
    let fixture = fixture();
    let id = fixture.seed_with_team("Landing page", Config::default().members());
    fixture.record(&id, 1, "codex", Some((900, 100)), Some("completed"));
    let mut app = fixture.app();
    app.load_session(&id).unwrap();

    let actions = type_and_enter(&mut app, "/usage");
    assert!(actions.is_empty(), "opening the page produced {actions:?}");
    assert_eq!(app.view, View::Usage);
    for code in [
        KeyCode::Down,
        KeyCode::Up,
        KeyCode::Enter,
        KeyCode::Char(' '),
        KeyCode::Char('r'),
        KeyCode::End,
    ] {
        let actions = app.on_key(key(code), 100);
        assert!(actions.is_empty(), "{code:?} produced {actions:?}");
        app.overlay = None;
    }
    assert_eq!(app.session.as_deref(), Some(id.as_str()));
}

#[test]
fn a_new_snapshot_keeps_the_row_and_the_reading_position() {
    let fixture = fixture();
    let mut app = fixture.app();
    // With no team of its own, the page lists exactly the agents the session recorded,
    // in the order the store keeps them.
    app.command("/team remove codex", 100);
    app.command("/team remove claude", 100);
    conversation(&mut app, 2);
    app.event(usage_event(
        "s1",
        &[("reviewer", Some((1_000, 100)), false)],
    ));

    app.command("/usage", 100);
    draw(&mut app, 100, 30);
    app.on_key(key(KeyCode::Down), 100);
    assert_eq!(app.selected_item(98).unwrap().key, "reviewer");
    let focus = app.focus;
    let top = app.page_top;

    // A later snapshot records an agent that sorts before the selected one.
    app.event(usage_event(
        "s1",
        &[
            ("reviewer", Some((1_200, 150)), false),
            ("auditor", Some((300, 30)), false),
        ],
    ));
    assert_eq!(
        app.selected_item(98).unwrap().key,
        "reviewer",
        "the selection moved to another agent"
    );
    assert_eq!(app.focus, focus);
    assert_eq!(app.page_top, top);
}

#[test]
fn the_session_total_and_the_agents_survive_a_small_terminal() {
    let fixture = fixture();
    let mut app = fixture.app();
    conversation(&mut app, 2);
    app.event(usage_event(
        "s1",
        &[
            ("codex", Some((12_000, 345)), false),
            ("claude", Some((2_000, 100)), false),
        ],
    ));

    let rendered = draw(&mut app, 80, 24);
    assert!(
        rendered.contains("14.4k"),
        "the session total left the 80x24 screen:\n{rendered}"
    );
    assert!(rendered.contains("TOKENS"), "{rendered}");
    assert!(rendered.contains("Codex"), "{rendered}");

    // The project is still identifiable, and the composer still works.
    typed(&mut app, "hello");
    let rendered = draw(&mut app, 80, 24);
    assert!(rendered.contains("ymp"), "{rendered}");
    assert!(rendered.contains("hello"), "{rendered}");
}

#[test]
fn invocations_with_no_agent_leave_the_agent_figures_incomplete() {
    let fixture = fixture();
    // An older session that counted three turns but recorded who took only one of them.
    let id = fixture.seed_counted("Historic", Config::default().members(), 3);
    fixture.record(&id, 1, "codex", Some((4_000, 200)), Some("completed"));
    let mut app = fixture.app();
    app.load_session(&id).unwrap();

    assert_eq!(app.stats.unattributed(), 2);
    assert_eq!(
        app.stats.agent("claude"),
        None,
        "an agent with no row cannot be called zero while invocations are unattributed"
    );

    app.command("/usage", 120);
    let rows: Vec<(String, String)> = app
        .page(118)
        .items
        .iter()
        .filter(|item| item.kind == crate::views::ItemKind::Row)
        .map(|item| {
            (
                item.key.clone(),
                item.right
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>(),
            )
        })
        .collect();
    let figure = |key: &str| {
        rows.iter()
            .find(|(row, _)| row == key)
            .map(|(_, figure)| figure.clone())
            .unwrap_or_default()
    };
    assert_eq!(
        figure("codex"),
        "4200+",
        "a figure that may be missing invocations must say so"
    );
    assert_eq!(
        figure("claude"),
        "—",
        "an unattributable agent must not read as zero"
    );
    assert_eq!(figure("session"), "4200+");

    let detail = app
        .page(118)
        .items
        .iter()
        .find(|item| item.key == "session")
        .map(|item| lines_text(&item.detail))
        .unwrap();
    assert!(detail.contains("2 without an agent"), "{detail}");
    assert!(
        text::one_line(&detail).contains("recorded without the agent that made them"),
        "the unattributed invocations were not explained:\n{detail}"
    );

    // The detail of an agent row marks its total incomplete for the same reason.
    let agent = app
        .page(118)
        .items
        .iter()
        .find(|item| item.key == "codex")
        .map(|item| lines_prose(&item.detail))
        .unwrap();
    assert!(
        agent.contains("4 200+"),
        "the agent detail total was presented as settled:\n{agent}"
    );
    assert!(agent.contains("could be this one's"), "{agent}");

    // The sidebar keeps the same distinction.
    let rendered = draw(&mut app, 120, 30);
    assert!(rendered.contains("4200+"), "{rendered}");
}

#[test]
fn opening_the_statistics_page_shows_the_session_breakdown_at_once() {
    let fixture = fixture();
    let id = fixture.seed_with_team("Landing page", Config::default().members());
    fixture.record(&id, 1, "codex", Some((9_000, 900)), Some("completed"));
    let mut app = fixture.app();
    app.load_session(&id).unwrap();
    app.command("/usage", 120);

    // The first frame, before any key is pressed, already carries the detail.
    let rendered = draw(&mut app, 120, 34);
    assert!(
        rendered.contains("cache read"),
        "the breakdown was missing from the first frame:\n{rendered}"
    );
    assert_eq!(app.selected_item(118).unwrap().key, "session");

    // The first press of Down moves on to the first agent, not onto the session row.
    app.on_key(key(KeyCode::Down), 120);
    assert_eq!(app.selected_item(118).unwrap().key, "codex");
}

#[test]
fn a_running_session_shows_live_tokens_turns_and_status_together() {
    let fixture = fixture();
    let id = fixture.seed_with_team("Release check", Config::default().members());
    let mut app = fixture.app();
    app.load_session(&id).unwrap();
    // The event loop marks the window active while a run it started is working.
    app.active = true;

    app.event(usage_event(
        &id,
        &[
            ("codex", Some((500, 100)), false),
            ("claude", Some((90, 10)), true),
        ],
    ));
    let rendered = draw(&mut app, 120, 36);
    assert!(rendered.contains("700+ tokens"), "{rendered}");
    assert!(
        rendered.contains("2 / 200 turns"),
        "the turn counter did not follow the invocations:\n{rendered}"
    );
    assert!(
        rendered.matches("running").count() >= 2,
        "the header and the sidebar must both report the active run:\n{rendered}"
    );
    assert!(
        !rendered.contains("completed"),
        "the stored status outranked the active run:\n{rendered}"
    );

    // When the run ends, the persisted status is shown again and the counted turns stay.
    app.active = false;
    app.event(UiEvent::Finished {
        session_id: id.clone(),
        status: "completed".into(),
    });
    let rendered = draw(&mut app, 120, 36);
    assert!(rendered.contains("completed"), "{rendered}");
    assert!(!rendered.contains("running"), "{rendered}");
    assert!(rendered.contains("700+ tokens"), "{rendered}");
    assert!(
        rendered.contains("2 / 200 turns"),
        "the counted turns were lost when the run finished:\n{rendered}"
    );
}

#[test]
fn browsing_a_saved_session_reports_what_was_stored() {
    let fixture = fixture();
    let id = fixture.seed_with_team("Finished work", Config::default().members());
    fixture.record(&id, 1, "codex", Some((300, 40)), Some("completed"));
    let mut app = fixture.app();
    app.load_session(&id).unwrap();

    assert!(!app.active);
    assert_eq!(app.live_status(), "completed");
    let rendered = draw(&mut app, 120, 36);
    assert!(rendered.contains("completed"), "{rendered}");
    assert!(!rendered.contains("running"), "{rendered}");
    assert!(rendered.contains("1 / 200 turns"), "{rendered}");
}

// ---------------------------------------------------------------------------
// Recorded checks and recovery limits
// ---------------------------------------------------------------------------

/// Wording that would promise something ymp cannot do. No previous file content is kept
/// anywhere, so no surface may sound like an offer to put a file back, and no surface may
/// describe a check as contained or agreed to.
const FORBIDDEN: &[&str] = &[
    "rollback",
    "roll back",
    "revert",
    "undo",
    "can restore",
    "will restore",
    "sandbox",
    "approval",
    "approved",
    "permitted",
    "allowlist",
];

fn forbidden(text: &str) -> Option<&'static str> {
    let lowered = text.to_lowercase();
    FORBIDDEN
        .iter()
        .copied()
        .find(|word| lowered.contains(word))
}

/// The prose of the page's empty state.
fn empty_prose(app: &mut App, width: u16) -> String {
    lines_prose(&app.page(width).empty)
}

/// What the change page says about recovery, from whichever surface is carrying it: the
/// leading row when the session recorded changes, the empty state when it did not.
fn recovery_prose(app: &mut App, width: u16) -> String {
    let page = app.page(width);
    match page.items.iter().find(|item| item.key == "recovery") {
        Some(item) => lines_prose(&item.detail),
        None => lines_prose(&page.empty),
    }
}

/// The detail prose of every row with this key, in page order. One command can be both a
/// recorded run and a command another task is still waiting for.
fn details_for(app: &mut App, width: u16, key: &str) -> Vec<String> {
    app.page(width)
        .items
        .iter()
        .filter(|item| item.key == key && item.kind == crate::views::ItemKind::Row)
        .map(|item| lines_prose(&item.detail))
        .collect()
}

/// The prose of the detail of the row with this key.
fn detail_prose(app: &mut App, width: u16, key: &str) -> String {
    app.page(width)
        .items
        .iter()
        .find(|item| item.key == key)
        .map(|item| lines_prose(&item.detail))
        .unwrap_or_else(|| panic!("no row keyed {key} on {:?}", app.view))
}

impl Fixture {
    /// Record a check the way `Engine::checks` records one.
    fn seed_check(&self, session: &str, command: &str, success: Option<bool>, output: &str) {
        let mut data = serde_json::json!({
            "cwd": self.project.path(),
            "command": command,
            "output": output,
        });
        if let Some(success) = success {
            data["success"] = serde_json::json!(success);
        }
        self.store.event(session, "check", &data).unwrap();
    }
    /// Record a check the way `Engine::checks` records one for a task attempt, or for the
    /// final pass over every declared command, which is recorded without a task.
    fn seed_check_for(
        &self,
        session: &str,
        task: Option<TaskAttemptRef>,
        command: &str,
        success: Option<bool>,
        output: &str,
    ) {
        let mut data = serde_json::json!({
            "id": new_id(),
            "task": task,
            "cwd": self.project.path(),
            "command": command,
            "output": output,
        });
        if let Some(success) = success {
            data["success"] = serde_json::json!(success);
        }
        self.store.event(session, "check", &data).unwrap();
    }
    /// A stored task that declares acceptance commands, left where its checks have run and
    /// no independent review has decided yet.
    ///
    /// These scenarios are about what the session log recorded and what the page reads back
    /// from it; none of them asserts anything about acceptance. Acceptance is a decision the
    /// store takes only through `save_task_with_decision`, bound to the reviewed attempt and
    /// the evidence behind it, so a display fixture must not claim it. A task submitted and
    /// awaiting review is also the state a recorded check actually belongs to: the runtime
    /// runs a task's checks after its execution turn and before the review decision.
    fn seed_task(&self, session: &str, title: &str, checks: &[&str]) -> String {
        let task = Task {
            id: new_id(),
            session_id: session.to_owned(),
            title: title.to_owned(),
            description: "Fixture task".into(),
            competence: "implementation".into(),
            difficulty: "standard".into(),
            dependencies: Vec::new(),
            checks: checks.iter().map(|c| (*c).to_owned()).collect(),
            state: TaskState::Review,
            assignee: Some("codex".into()),
            // The reviewer is recorded by the review decision, which has not been taken.
            reviewer: None,
            attempts: 1,
            result: Some("Submitted for independent review.".into()),
            workspace: Some(self.project.path().to_path_buf()),
            base_commit: None,
            interrupted: false,
        };
        self.store.save_task(&task).unwrap();
        task.id
    }
}

/// The changed-files page must say what was recorded and what was never recorded, in both
/// its empty and its populated state. A reader cannot be left to infer from a list of
/// paths that ymp is holding the previous versions of them.
#[test]
fn the_change_view_states_that_previous_content_was_never_recorded() {
    let fixture = fixture();
    let id = fixture.seed_session("Build a landing page");
    let mut app = fixture.app();
    app.load_session(&id).unwrap();
    app.command("/diff", 100);

    let empty = empty_prose(&mut app, 100);
    assert!(
        empty.contains("cannot restore"),
        "the empty change view promised nothing about recovery:\n{empty}"
    );
    assert!(
        empty.contains("hash"),
        "the empty change view did not say what is recorded instead:\n{empty}"
    );

    // The statement is on the frame at the smallest supported size, not only in the data.
    let rendered = draw(&mut app, 80, 24);
    assert!(
        rendered.contains("cannot restore"),
        "the empty change view did not show the statement at 80x24:\n{rendered}"
    );

    // The same statement has to survive the page having rows to show.
    let workspace = fixture
        .store
        .session_dir(&fixture.store.session(&id).unwrap())
        .join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::write(
        workspace.join("changes.json"),
        serde_json::json!([{"path": fixture.project.path().join("index.html"), "status": "created"}])
            .to_string(),
    )
    .unwrap();
    app.set_view(View::Changes);
    let keys = row_keys(&mut app, 100);
    assert!(
        keys.iter().any(|key| key.ends_with("index.html")),
        "the recorded change is missing: {keys:?}"
    );
    let recovery = recovery_prose(&mut app, 100);
    assert!(
        recovery.contains("cannot restore"),
        "the populated change view dropped the recovery statement:\n{recovery}"
    );
    let file = detail_prose(&mut app, 100, &keys[1]);
    assert!(
        file.contains("cannot restore"),
        "a selected change did not state what ymp kept:\n{file}"
    );
    for text in [&empty, &recovery, &file] {
        assert_eq!(forbidden(text), None, "in:\n{text}");
    }

    // At the smallest supported size the statement is on the first frame, unselected.
    let rendered = draw(&mut app, 80, 24);
    assert!(
        rendered.contains("cannot restore"),
        "the statement was not visible at 80x24:\n{rendered}"
    );
}

/// Acceptance commands that ymp ran itself are evidence, and the only honest place to read
/// them is the session that recorded them. A declared command that never ran must not be
/// presented as one that did.
#[test]
fn recorded_checks_show_their_command_and_outcome_for_the_selected_session() {
    let fixture = fixture();
    let id = fixture.seed_session("Build a landing page");
    fixture.seed_check(&id, "grep -q doctype index.html", Some(true), "exit: 0\n");
    fixture.seed_check(
        &id,
        "cargo test --workspace",
        Some(false),
        "exit: 101\nfailed",
    );
    // A record written without an outcome is not a pass.
    fixture.seed_check(&id, "npm run build", None, "");
    fixture.seed_task(&id, "Write the page", &["grep -q doctype index.html"]);
    fixture.seed_task(&id, "Check the layout", &["cargo fmt --check"]);

    let mut app = fixture.app();
    app.load_session(&id).unwrap();
    app.command("/checks", 100);
    assert_eq!(app.view, View::Checks, "/checks did not open the page");

    let keys = row_keys(&mut app, 100);
    for command in [
        "grep -q doctype index.html",
        "cargo test --workspace",
        "npm run build",
        "cargo fmt --check",
    ] {
        assert!(
            keys.iter().any(|key| key.contains(command)),
            "{command} is missing from {keys:?}"
        );
    }

    let passed = detail_prose(&mut app, 100, "grep -q doctype index.html");
    assert!(passed.contains("passed"), "{passed}");
    let failed = detail_prose(&mut app, 100, "cargo test --workspace");
    assert!(failed.contains("failed"), "{failed}");
    assert!(
        failed.contains("exit: 101"),
        "the output is missing:\n{failed}"
    );
    let unknown = detail_prose(&mut app, 100, "npm run build");
    assert!(
        unknown.contains("no recorded outcome"),
        "a record without an outcome was given one:\n{unknown}"
    );
    let planned = detail_prose(&mut app, 100, "cargo fmt --check");
    assert!(
        planned.contains("no recorded run"),
        "a command that never ran looks like it ran:\n{planned}"
    );

    // The page explains how checks run, and claims nothing about containment.
    let about = detail_prose(&mut app, 100, "how checks run");
    assert!(about.contains("working directory"), "{about}");
    for text in [&passed, &failed, &unknown, &planned, &about] {
        assert_eq!(forbidden(text), None, "in:\n{text}");
    }

    let rendered = draw(&mut app, 80, 24);
    assert!(rendered.contains("cargo test"), "{rendered}");
    assert!(rendered.contains("failed"), "{rendered}");
}

/// A working directory nested below a repository root is still inside that repository, and
/// a directory with no `.git` entry of its own proves nothing. The change view must report
/// what was found, name what it inspected, and never conclude absence.
#[test]
fn repository_discovery_looks_above_the_working_directory_and_never_concludes_absence() {
    let outer = TempDir::new().unwrap();
    std::fs::create_dir_all(outer.path().join(".git/objects")).unwrap();
    let nested = outer.path().join("service/web");
    std::fs::create_dir_all(&nested).unwrap();
    let home = TempDir::new().unwrap();
    let store = Store::open(home.path()).unwrap();
    let mut app = App::new(store, Config::default(), nested.clone());
    app.command("/diff", 100);
    let found = recovery_prose(&mut app, 100);
    // The marker itself, not the working directory, which merely starts with the same path.
    let marker = outer.path().canonicalize().unwrap().join(".git");
    assert!(
        found.contains("A Git repository directory is at"),
        "the repository above the working directory was not reported:\n{found}"
    );
    assert!(
        found.contains(&marker.display().to_string()),
        "the marker {} was not named:\n{found}",
        marker.display()
    );

    // Without a marker anywhere the wording stays a statement about what was inspected.
    let bare = fixture();
    let mut app = bare.app();
    app.command("/diff", 100);
    let unknown = recovery_prose(&mut app, 100);
    assert!(
        unknown.contains("inspected"),
        "the bounds of discovery were not stated:\n{unknown}"
    );
    for claim in [
        "not under version control",
        "no repository",
        "is not a repository",
    ] {
        assert!(
            !unknown.to_lowercase().contains(claim),
            "discovery concluded absence with {claim:?}:\n{unknown}"
        );
    }
}

/// A record longer than the page shows must say where the rest is, and the page must not
/// offer a keystroke that reaches it. `Enter` clones the lines the row was built with, so
/// the cut is already in them: there is no second, fuller view to promise.
#[test]
fn a_truncated_record_says_the_rest_is_not_shown_anywhere() {
    let fixture = fixture();
    let id = fixture.seed_session("Build a landing page");
    let long = (0..400)
        .map(|line| format!("recorded line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    fixture.seed_check(&id, "cargo test --workspace", Some(true), &long);
    let mut app = fixture.app();
    app.load_session(&id).unwrap();
    app.command("/checks", 100);

    let detail = detail_prose(&mut app, 100, "cargo test --workspace");
    assert!(
        detail.contains("session log"),
        "the remainder was not placed where it actually is:\n{detail}"
    );
    assert!(
        detail.contains("no page shows"),
        "the detail did not say the rest is unreachable:\n{detail}"
    );

    // The hint must describe what Enter does, which is to open these same lines.
    let hints = app.page(100).hints.clone();
    assert!(
        hints.iter().any(|(key, text)| *key == "Enter"
            && *text == "show the recorded run"
            && !text.contains("whole")),
        "the hint promises more than Enter opens: {hints:?}"
    );

    // What Enter opens is the selected row's detail, cut lines and all.
    app.on_key(key(KeyCode::Down), 100);
    app.on_key(key(KeyCode::Enter), 100);
    let Some(Overlay::Inspect { title, body, .. }) = &app.overlay else {
        panic!("Enter did not open the record");
    };
    assert_eq!(title, "cargo test --workspace");
    let opened = lines_prose(body);
    assert!(opened.contains("no page shows"), "{opened}");
    assert_eq!(
        opened.matches("recorded line").count(),
        detail.matches("recorded line").count(),
        "the overlay showed a different amount of output than the row"
    );
}

/// With no session open the page read nothing. Saying that nothing was recorded would be a
/// statement about a session the interface never looked at, on the one page whose purpose is
/// to keep an absent record apart from a result.
#[test]
fn the_checks_page_does_not_report_an_absent_session_as_an_absent_record() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.command("/checks", 100);
    assert!(app.session.is_none());

    let empty = empty_prose(&mut app, 100);
    assert!(
        !empty.contains("No checks were recorded"),
        "a session that was never opened was reported as having no checks:\n{empty}"
    );
    assert!(
        empty.contains("No session is loaded"),
        "the reason nothing was read is missing:\n{empty}"
    );
    let rendered = draw(&mut app, 80, 24);
    assert!(rendered.contains("Nothing was read"), "{rendered}");

    // With a session that recorded nothing, the page does report the absent record.
    let id = fixture.seed_session("Build a landing page");
    app.load_session(&id).unwrap();
    app.command("/checks", 100);
    let empty = empty_prose(&mut app, 100);
    assert!(empty.contains("No checks were recorded"), "{empty}");
}

/// A record names the task attempt it was run for, so a run that belongs to one task says
/// nothing about the same command declared by another. The second task's command is still
/// waiting, and the page has to show it as waiting.
#[test]
fn a_run_scoped_to_one_task_does_not_cover_the_same_command_in_another() {
    let fixture = fixture();
    let id = fixture.seed_session("Build a landing page");
    let first = fixture.seed_task(&id, "Write the page", &["cargo fmt --check"]);
    fixture.seed_task(&id, "Check the layout", &["cargo fmt --check"]);
    fixture.seed_check_for(
        &id,
        Some(TaskAttemptRef {
            task_id: first.clone(),
            attempt: 1,
        }),
        "cargo fmt --check",
        Some(true),
        "exit: 0",
    );
    let mut app = fixture.app();
    app.load_session(&id).unwrap();
    app.command("/checks", 100);

    let details = details_for(&mut app, 100, "cargo fmt --check");
    assert_eq!(
        details.len(),
        2,
        "the command is one recorded run and one task still waiting:\n{details:#?}"
    );
    let run = &details[0];
    assert!(run.contains("passed"), "{run}");
    assert!(
        run.contains("Write the page") && run.contains("attempt 1"),
        "the run did not name the task attempt it belongs to:\n{run}"
    );
    let waiting = &details[1];
    assert!(waiting.contains("no recorded run"), "{waiting}");
    assert!(
        waiting.contains("Check the layout") && !waiting.contains("Write the page"),
        "the waiting command named the wrong task:\n{waiting}"
    );
    assert_eq!(
        app.page(100).subtitle,
        "1 recorded · 1 declared without a run"
    );
}

/// The final pass runs the union of every declared command and is recorded without a task.
/// Such a run does cover each task that declared the command, and the page says on what
/// basis, because the record itself cannot say which task it was for.
#[test]
fn a_run_recorded_without_a_task_covers_every_task_that_declared_the_command() {
    let fixture = fixture();
    let id = fixture.seed_session("Build a landing page");
    fixture.seed_task(&id, "Write the page", &["cargo fmt --check"]);
    fixture.seed_task(&id, "Check the layout", &["cargo fmt --check"]);
    fixture.seed_check_for(&id, None, "cargo fmt --check", Some(true), "exit: 0");
    let mut app = fixture.app();
    app.load_session(&id).unwrap();
    app.command("/checks", 100);

    let details = details_for(&mut app, 100, "cargo fmt --check");
    assert_eq!(details.len(), 1, "a covered command was listed as waiting");
    assert!(
        details[0].contains("no task recorded"),
        "the run claimed a task the record does not name:\n{}",
        details[0]
    );
    let about = detail_prose(&mut app, 100, "how checks run");
    assert!(
        about.contains("matched by its command text alone"),
        "the page does not say how an unscoped run is matched:\n{about}"
    );
    assert!(
        about.contains("counts for every task that declared"),
        "the page does not say what an unscoped run covers:\n{about}"
    );
    assert_eq!(forbidden(&about), None, "in:\n{about}");
}

/// Records written before runs carried a task have no task field at all. They must read as
/// unscoped rather than as belonging to nothing, and they must still cover the commands they
/// name, or reopening an old session would invent work that was already done.
#[test]
fn a_record_written_before_tasks_were_recorded_is_read_as_unscoped() {
    let fixture = fixture();
    let id = fixture.seed_session("Build a landing page");
    fixture.seed_task(&id, "Write the page", &["cargo fmt --check"]);
    // The older shape: no task key in the event at all.
    fixture.seed_check(&id, "cargo fmt --check", Some(true), "exit: 0");
    let mut app = fixture.app();
    app.load_session(&id).unwrap();
    app.command("/checks", 100);

    let details = details_for(&mut app, 100, "cargo fmt --check");
    assert_eq!(
        details.len(),
        1,
        "an older record stopped covering its command"
    );
    assert!(
        details[0].contains("no task recorded"),
        "an older record was given a task:\n{}",
        details[0]
    );
    assert!(
        detail_prose(&mut app, 100, "how checks run").contains("before records carried one"),
        "the page does not account for records written before tasks were recorded"
    );
}

// ---------------------------------------------------------------------------
// Frame width: prose is painted at the width it was wrapped for
// ---------------------------------------------------------------------------

/// Sizes the interface claims to support: the documented minimum and three larger ones.
const SUPPORTED_SIZES: &[(u16, u16)] = &[(80, 24), (100, 30), (120, 40), (160, 48)];

/// One sentence that is longer than any supported main column, so it must wrap and a lost
/// column shows up as a missing word rather than as a missing space.
const RECOVERY_SENTENCE: &str = "It recorded a path, a status and a content hash for each \
     file, never a copy, so earlier content exists only where the working directory's own \
     version control or a backup already kept it.";

/// The screen as rows, so a test can see where a painted line ended.
fn screen_rows(app: &mut App, width: u16, height: u16) -> Vec<String> {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal.draw(|frame| ui::render(frame, app)).unwrap();
    let buffer = terminal.backend().buffer().clone();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect()
}

/// The columns `first .. first + room` of every row, as one line.
fn column_prose(rows: &[String], first: usize, room: usize) -> String {
    let block = rows
        .iter()
        .map(|row| row.chars().skip(first).take(room).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    text::one_line(&block)
}

/// What the main column says, as one line. The sidebar is cut away first, because it sits
/// to the right of every row and would otherwise interrupt each sentence. A sentence
/// survives the frame only if it is still here unbroken: a clipped right edge cuts a word
/// in half instead.
fn main_prose(app: &mut App, width: u16, height: u16) -> String {
    let sidebar = crate::frame::sidebar_width(width);
    let main = if sidebar == 0 {
        width
    } else {
        width - sidebar - 1
    };
    column_prose(&screen_rows(app, width, height), 0, main as usize)
}

/// What the floating read-only surface says, as one line. It is centred and 86 columns
/// wide, or the terminal less four, and its border takes the first and the last column.
fn modal_prose(app: &mut App, width: u16, height: u16) -> String {
    let surface = 86u16.min(width.saturating_sub(4)).max(12);
    let left = (width.saturating_sub(surface) / 2 + 1) as usize;
    column_prose(&screen_rows(app, width, height), left, inspect_room(width))
}

/// Columns the read-only inspect surface has for its body: it is 86 columns wide, or the
/// terminal less four, and its own frame takes one column on each side.
fn inspect_room(total: u16) -> usize {
    86u16
        .min(total.saturating_sub(4))
        .saturating_sub(2)
        .max(8)
        .into()
}

/// Change metadata as a finished run writes it, so the change page has a row to select.
fn seed_change(fixture: &Fixture, session: &str, name: &str, status: &str) {
    let workspace = fixture
        .store
        .session_dir(&fixture.store.session(session).unwrap())
        .join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::write(
        workspace.join("changes.json"),
        serde_json::json!([{"path": fixture.project.path().join(name), "status": status}])
            .to_string(),
    )
    .unwrap();
}

#[test]
fn the_detail_pane_paints_prose_whole_at_every_supported_size() {
    let fixture = fixture();
    let id = fixture.seed_session("Build a landing page");
    seed_change(&fixture, &id, "index.html", "created");
    for (width, height) in SUPPORTED_SIZES.iter().copied() {
        let mut app = fixture.app();
        app.load_session(&id).unwrap();
        app.set_view(View::Changes);
        let prose = main_prose(&mut app, width, height);
        assert!(
            prose.contains(RECOVERY_SENTENCE),
            "at {width}x{height} the detail pane lost words from its statement:\n{prose}"
        );
    }
}

#[test]
fn an_empty_page_paints_its_prose_whole_at_every_supported_size() {
    let fixture = fixture();
    let id = fixture.seed_session("Build a landing page");
    for (width, height) in SUPPORTED_SIZES.iter().copied() {
        let mut app = fixture.app();
        app.load_session(&id).unwrap();
        app.set_view(View::Changes);
        let prose = main_prose(&mut app, width, height);
        assert!(
            prose.contains(RECOVERY_SENTENCE),
            "at {width}x{height} the empty state lost words from its statement:\n{prose}"
        );
    }
}

#[test]
fn the_inspect_surface_is_built_for_its_own_width_and_not_the_terminal() {
    let fixture = fixture();
    let id = fixture.seed_session("Build a landing page");
    seed_change(&fixture, &id, "index.html", "created");
    for (width, height) in SUPPORTED_SIZES.iter().copied() {
        let mut app = fixture.app();
        app.load_session(&id).unwrap();
        app.set_view(View::Changes);
        app.on_key(key(KeyCode::Enter), width);
        let Some(Overlay::Inspect { body, .. }) = &app.overlay else {
            panic!("Enter must open the selected record at {width}x{height}");
        };
        let room = inspect_room(width);
        for line in body {
            let painted = line
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>();
            assert!(
                text::width(&painted) <= room,
                "at {width}x{height} a line of {} cells was built for a surface {room} wide:\n{painted}",
                text::width(&painted)
            );
        }
        let prose = modal_prose(&mut app, width, height);
        assert!(
            prose.contains(RECOVERY_SENTENCE),
            "at {width}x{height} the inspect surface lost words from its statement:\n{prose}"
        );
    }
}

// ---------------------------------------------------------------------------
// Records a real mock run writes
// ---------------------------------------------------------------------------

/// A configuration whose only provider answers inside ymp.
///
/// Nothing in these tests reaches a provider process, reads a credential or leaves the
/// temporary directories the fixture owns. The records the pages are asserted against are
/// written by the runtime itself, through the same calls a real run makes.
fn mock_config() -> Config {
    use ymp_core::{Limits, ProviderConfig, ProviderKind, ResourceLimits};
    // Only the fields this fixture needs are named; everything else keeps the shipped
    // default, so a field added to the configuration does not break these tests.
    Config {
        limits: Limits {
            parallel: 2,
            turns: 80,
            turn_timeout_secs: 10,
            attempts: 2,
            resources: Some(ResourceLimits::default()),
        },
        providers: vec![ProviderConfig {
            id: "mock".into(),
            kind: ProviderKind::Mock,
            command: "internal".into(),
            args: vec![],
            env_refs: Default::default(),
            enabled: true,
        }],
        agents: ["one", "two"]
            .into_iter()
            .map(|id| AgentProfile {
                id: id.into(),
                name: id.into(),
                provider: "mock".into(),
                model: None,
                instructions: id.into(),
                enabled: true,
            })
            .collect(),
        team: vec!["one".into(), "two".into()],
        ..Config::default()
    }
}

struct Run {
    _home: TempDir,
    project: TempDir,
    store: Store,
    config: Config,
    session: String,
    status: String,
}

impl Run {
    fn app(&self) -> App {
        App::new(
            self.store.clone(),
            self.config.clone(),
            PathBuf::from(self.project.path()),
        )
    }
}

/// Run the real engine once against the mock provider and keep what it wrote.
///
/// `prepare` receives the engine before the run, which is how a test installs an acceptance
/// contract or a resource limit. No model is contacted at any effort: the mock answers in
/// process, and the only settings that travel are the ones the runtime itself sets.
async fn mock_run(prompt: &str, prepare: impl FnOnce(&mut ymp_runtime::Engine)) -> Run {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    let store = Store::open(home.path()).unwrap();
    let config = mock_config();
    let (tx, _events) = tokio::sync::mpsc::unbounded_channel();
    let mut engine = ymp_runtime::Engine::new(
        store.clone(),
        config.clone(),
        tx,
        tokio_util::sync::CancellationToken::new(),
    )
    .unwrap();
    prepare(&mut engine);
    let outcome = engine.run(project.path(), prompt, None).await.unwrap();
    Run {
        _home: home,
        project,
        store,
        config: engine.config.clone(),
        session: outcome.session.id,
        status: outcome.session.status,
    }
}

/// The contract a run is given so its acceptance can be confirmed: one criterion, one
/// trusted check over the artifact the mock executor writes.
fn exact_greeting_contract() -> ymp_core::AcceptanceContract {
    use ymp_core::{AcceptanceContract, AcceptanceCriterion, CheckAssertion, TrustedCheck};
    AcceptanceContract {
        task_title: "Create a greeting".into(),
        criteria: vec![AcceptanceCriterion {
            id: "greeting-content".into(),
            description: "The greeting file contains exactly the requested greeting".into(),
        }],
        artifacts: vec!["greeting.txt".into()],
        inputs: vec![],
        checks: vec![TrustedCheck {
            id: "exact-greeting-v1".into(),
            criterion_ids: vec!["greeting-content".into()],
            assertion: CheckAssertion::ExactBytes {
                artifact: "greeting.txt".into(),
                expected: b"Hello from ymp\n".to_vec(),
            },
        }],
    }
}

/// The rows of a page, by key, so a test can name the one it means.
fn keys_of(app: &mut App, width: u16) -> Vec<String> {
    app.page(width)
        .items
        .iter()
        .map(|item| item.key.clone())
        .collect()
}

/// The detail of the first row whose left text contains `needle`, as one line.
fn row_prose(app: &mut App, width: u16, needle: &str) -> String {
    let page = app.page(width);
    let item = page
        .items
        .iter()
        .find(|item| {
            item.left
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
                .contains(needle)
        })
        .unwrap_or_else(|| {
            panic!(
                "no row mentions {needle}: {:?}",
                page.items.iter().map(|i| i.key.clone()).collect::<Vec<_>>()
            )
        });
    text::one_line(&lines_text(&item.detail))
}

/// The right-hand text of the row with this key, which names one record exactly.
fn right_of_key(app: &mut App, width: u16, key: &str) -> String {
    let page = app.page(width);
    page.items
        .iter()
        .find(|item| item.key == key)
        .map(|item| {
            item.right
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .unwrap_or_else(|| panic!("no row has the key {key}"))
}

/// The detail of the row with this key, as one line.
fn detail_of_key(app: &mut App, width: u16, key: &str) -> String {
    let page = app.page(width);
    let item = page
        .items
        .iter()
        .find(|item| item.key == key)
        .unwrap_or_else(|| panic!("no row has the key {key}"));
    text::one_line(&lines_text(&item.detail))
}

/// The right-hand text of the first row whose left text contains `needle`.
fn row_right(app: &mut App, width: u16, needle: &str) -> String {
    let page = app.page(width);
    page.items
        .iter()
        .find(|item| {
            item.left
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
                .contains(needle)
        })
        .map(|item| {
            item.right
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .unwrap_or_else(|| panic!("no row mentions {needle}"))
}

#[tokio::test]
async fn an_assignment_shows_what_was_sent_and_never_claims_an_unreported_setting_applied() {
    let run = mock_run("Create a greeting", |_| {}).await;
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/assignments", 100);

    let subtitle = app.page(65).subtitle.clone();
    assert!(
        subtitle.contains("assigned") && subtitle.contains("none open"),
        "a finished run was not described as finished: {subtitle}"
    );
    let execute = row_prose(&mut app, 65, "execute");
    assert!(
        execute.contains("write sent") && execute.contains("unconfirmed"),
        "a permission mode nobody reported was not marked unconfirmed:\n{execute}"
    );
    assert!(
        !execute.contains("the installation reported the same"),
        "a setting nothing reported was presented as confirmed:\n{execute}"
    );
    assert!(
        execute.contains("nothing requested; the installation used its own default"),
        "an unrequested model was not stated as the installation's own default:\n{execute}"
    );
    assert!(
        execute.contains("nothing is fixed"),
        "the page did not say whether the settings were constrained:\n{execute}"
    );
    assert!(
        execute.contains("ymp.native"),
        "the record's execution backend is missing:\n{execute}"
    );
    assert!(
        execute.contains("Create a greeting") && execute.contains("attempt 1"),
        "the turn did not name the task attempt it belongs to:\n{execute}"
    );
    assert!(
        execute.contains("post to the board"),
        "the coordination permission the record grants is not named:\n{execute}"
    );
    assert!(
        execute.contains("completed"),
        "the turn's recorded state is missing:\n{execute}"
    );
    // A record is not a claim about containment.
    for forbidden in ["sandbox", "isolated", "approved by you"] {
        assert!(
            !execute.contains(forbidden),
            "the page claims {forbidden}:\n{execute}"
        );
    }
}

#[tokio::test]
async fn an_acceptance_on_review_alone_is_not_presented_as_confirmed() {
    let run = mock_run("Create a greeting", |_| {}).await;
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/decisions", 100);

    let trace = run.store.trace(&run.session).unwrap();
    let acceptance = trace
        .decisions
        .iter()
        .find(|decision| decision.kind == "task_accepted")
        .unwrap();
    let reviewer = trace
        .decisions
        .iter()
        .find(|decision| acceptance.links.review_ids.contains(&decision.id))
        .and_then(|review| review.actor.clone())
        .expect("the runtime links the review its acceptance rests on");
    let accepted = row_prose(&mut app, 65, "task accepted");
    assert!(
        accepted.contains("accepted, unconfirmed"),
        "an acceptance with no evidence was graded as something else:\n{accepted}"
    );
    assert!(
        accepted.contains("an independent review, with no applicable check evidence"),
        "the basis of the acceptance is not stated:\n{accepted}"
    );
    assert!(
        accepted.contains(&format!("reviewed by {reviewer}")),
        "the reviewer the acceptance links is not named:\n{accepted}"
    );
    assert!(
        !accepted.contains("accepted, confirmed"),
        "an unconfirmed acceptance reads as confirmed:\n{accepted}"
    );

    app.command("/tasks", 100);
    assert!(
        row_right(&mut app, 65, "Create a greeting").contains("unconfirmed"),
        "the task row does not carry the grade of its acceptance"
    );
    let task = row_prose(&mut app, 65, "Create a greeting");
    assert!(
        task.contains("no competence credit was recorded"),
        "an unconfirmed acceptance was credited:\n{task}"
    );
    assert!(
        !task.contains("competence credited to"),
        "credit appeared without a credit record:\n{task}"
    );
}

#[tokio::test]
async fn a_confirmed_acceptance_names_its_evidence_and_its_credit() {
    let run = mock_run("Create a greeting", |engine| {
        engine.acceptance_contracts.push(exact_greeting_contract())
    })
    .await;
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/decisions", 100);

    let accepted = row_prose(&mut app, 65, "task accepted");
    assert!(
        accepted.contains("accepted, confirmed"),
        "a confirmed acceptance was not distinguished:\n{accepted}"
    );
    assert!(
        accepted.contains("covering every applicable criterion"),
        "the evidence behind a confirmed acceptance is not stated:\n{accepted}"
    );
    assert!(
        accepted.contains("still current"),
        "the page does not say whether the accepted files are still the ones on disk:\n{accepted}"
    );
    let credited = row_prose(&mut app, 65, "competence credited");
    assert!(
        credited.contains("competence observation"),
        "the credit record does not name its observation:\n{credited}"
    );

    app.command("/tasks", 100);
    assert!(
        row_right(&mut app, 65, "Create a greeting").contains("confirmed"),
        "the task row does not show that its acceptance was confirmed"
    );
    let task = row_prose(&mut app, 65, "Create a greeting");
    assert!(
        task.contains("competence credited to"),
        "a credited task does not say so:\n{task}"
    );

    app.command("/reputation", 100);
    let producer = run
        .store
        .observations()
        .unwrap()
        .first()
        .expect("a confirmed acceptance credits its producer")
        .agent_name
        .clone();
    let observation = row_prose(&mut app, 65, &producer);
    assert!(
        observation.contains("confirmed: evidence passed for every criterion"),
        "the observation's evidence status is missing:\n{observation}"
    );
    assert!(
        observation.contains("this session recorded the credit for it"),
        "the credit this session recorded is not shown:\n{observation}"
    );
    assert!(
        app.page(65).subtitle.contains("on confirmed evidence"),
        "the page does not count confirmed observations apart: {}",
        app.page(65).subtitle
    );
}

#[tokio::test]
async fn an_accepted_result_whose_files_changed_stays_accepted_and_says_what_changed() {
    let run = mock_run("Create a greeting", |engine| {
        engine.acceptance_contracts.push(exact_greeting_contract())
    })
    .await;
    // The working directory moves on after acceptance, as a later run would move it.
    std::fs::write(run.project.path().join("greeting.txt"), "Changed by hand\n").unwrap();
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/decisions", 100);

    let accepted = row_prose(&mut app, 65, "task accepted");
    assert!(
        accepted.contains("accepted, confirmed"),
        "a later edit retracted a recorded acceptance:\n{accepted}"
    );
    assert!(
        accepted.contains("superseded by later changes"),
        "a stale result was presented as current:\n{accepted}"
    );
    assert!(
        !accepted.contains("still current"),
        "the page claims the accepted files are unchanged:\n{accepted}"
    );
}

#[tokio::test]
async fn an_unreadable_grade_is_never_read_as_a_pass() {
    let run = mock_run("Create a greeting", |_| {}).await;
    // What an older installation left behind: an outcome recorded before grading existed.
    run.store
        .observe(&ymp_core::Observation {
            confirmation: ymp_core::ConfirmationStatus::Unknown,
            id: ymp_core::new_id(),
            agent_version: "legacy-version".into(),
            agent_name: "two".into(),
            competence: "implementation".into(),
            difficulty: "simple".into(),
            success: true,
            evidence: "Recorded before evidence was graded.".into(),
            created_at: ymp_core::now(),
        })
        .unwrap();
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/reputation", 100);

    let legacy = row_prose(&mut app, 65, "two");
    assert!(
        legacy.contains("unknown: the record was written before grading"),
        "a legacy observation was given a grade it does not carry:\n{legacy}"
    );
    assert!(
        legacy.contains("not eligible: only a confirmed outcome can be credited"),
        "an ungraded observation was treated as eligible for selection:\n{legacy}"
    );
    assert!(
        row_right(&mut app, 65, "two").contains("grade unknown"),
        "the row does not show that the grade is unknown"
    );
    assert!(
        app.page(65).subtitle.contains("0 on confirmed evidence"),
        "an ungraded observation was counted as confirmed: {}",
        app.page(65).subtitle
    );
}

#[tokio::test]
async fn a_reopened_session_is_measured_against_the_limits_it_captured() {
    let run = mock_run("Create a greeting", |_| {}).await;
    // The configuration moves on after the run, which must not rewrite what it ran under.
    let mut config = run.config.clone();
    config.limits.turns = 7;
    let mut app = App::new(run.store.clone(), config, PathBuf::from(run.project.path()));
    app.load_session(&run.session).unwrap();

    let (limit, captured) = app.turn_limit();
    assert!(
        captured,
        "a session with a captured budget was read as if it had none"
    );
    assert_eq!(
        limit, 80,
        "the session was measured against an edited limit"
    );

    app.command("/limits", 100);
    let captured_turns = row_prose(&mut app, 65, "turns");
    assert!(
        captured_turns.contains("cannot be edited here"),
        "a captured limit was offered for editing:\n{captured_turns}"
    );
    let keys = keys_of(&mut app, 65);
    assert!(
        keys.contains(&"captured:turns".to_owned()) && keys.contains(&"turns".to_owned()),
        "the captured limits and the next run's are not kept apart: {keys:?}"
    );
    assert!(
        app.page(65).subtitle.contains("this session: 80 turns")
            && app.page(65).subtitle.contains("the next run: 7 turns"),
        "the page does not separate the two: {}",
        app.page(65).subtitle
    );
    let tokens = row_prose(&mut app, 65, "tokens observed");
    assert!(
        tokens.contains("not as zero tokens"),
        "unreported spend was not distinguished from nothing spent:\n{tokens}"
    );

    // Pressing the editing keys on a record must not change the configuration.
    app.page_selected = app
        .page(65)
        .items
        .iter()
        .position(|item| item.key == "captured:turns")
        .unwrap();
    app.on_key(key(KeyCode::Char('+')), 100);
    app.on_key(key(KeyCode::Char('-')), 100);
    assert_eq!(
        app.config.limits.turns, 7,
        "a keystroke on a captured record edited the configuration"
    );
}

#[tokio::test]
async fn a_session_that_captured_no_limits_says_so_instead_of_showing_todays() {
    let fixture = fixture();
    let id = fixture.seed_session("An older run");
    let mut app = fixture.app();
    app.load_session(&id).unwrap();

    let (limit, captured) = app.turn_limit();
    assert!(!captured, "a session with no captured budget claimed one");
    assert_eq!(limit, Config::default().limits.turns);

    app.command("/limits", 100);
    let none = row_prose(&mut app, 65, "captured no limits");
    assert!(
        none.contains("ran before limits were captured")
            && none.contains("the values below are not it"),
        "an absent capture was not distinguished from a captured value:\n{none}"
    );

    app.command("/assignments", 100);
    let empty = empty_prose(&mut app, 65);
    assert!(
        empty.contains("No assignments were recorded"),
        "a session that recorded nothing was described as unread:\n{empty}"
    );
    assert!(
        !empty.contains("Nothing was read"),
        "an opened session was reported as unopened:\n{empty}"
    );

    app.command("/new", 100);
    app.command("/decisions", 100);
    let unopened = empty_prose(&mut app, 65);
    assert!(
        unopened.contains("Nothing was read") && unopened.contains("No session is loaded"),
        "an unopened session was reported as one that recorded nothing:\n{unopened}"
    );
}

#[tokio::test]
async fn membership_keeps_an_agent_that_worked_here_after_it_leaves_the_pool() {
    let run = mock_run("Create a greeting", |_| {}).await;
    // The profile is deleted from the configuration after the run, as editing the file does.
    let mut config = run.config.clone();
    config.agents.retain(|agent| agent.id != "two");
    config.team.retain(|id| id != "two");
    let mut app = App::new(run.store.clone(), config, PathBuf::from(run.project.path()));
    app.load_session(&run.session).unwrap();
    app.command("/team", 100);

    let keys = keys_of(&mut app, 65);
    assert!(
        keys.contains(&"two".to_owned()),
        "an agent that worked in this session disappeared with its profile: {keys:?}"
    );
    let member = row_prose(&mut app, 65, "two");
    assert!(
        member.contains("captured by this session when it started"),
        "the session's own membership is not named as captured:\n{member}"
    );
    assert!(
        member.contains("not in the pool on this machine"),
        "an agent missing from the pool was presented as available:\n{member}"
    );
    assert!(
        member.contains("turns recorded here"),
        "the work the agent did here is not counted:\n{member}"
    );
    assert!(
        app.page(65).subtitle.contains("captured by this session"),
        "the page does not say which team it shows: {}",
        app.page(65).subtitle
    );
}

#[tokio::test]
async fn a_turn_left_open_is_not_called_running_until_a_run_is_active() {
    let run = mock_run("Create a greeting", |_| {}).await;
    let trace = run.store.trace(&run.session).unwrap();
    let previous = trace.assignments.last().unwrap().clone();
    // One more turn, admitted and not finished: what a turn in flight looks like in the
    // store, written with the same calls the runtime uses.
    let mut assignment = ymp_core::AssignmentRecord {
        id: ymp_core::new_id(),
        grant_ids: Vec::new(),
        state: ymp_core::InvocationState::Running,
        started_at: ymp_core::now(),
        ended_at: None,
        ..previous.clone()
    };
    let open = ymp_core::InvocationRecord {
        id: ymp_core::new_id(),
        session_id: run.session.clone(),
        assignment_id: assignment.id.clone(),
        execution_backend: None,
        turn: trace.invocations.len() as u64 + 1,
        requested: assignment.requested.clone(),
        sent: Default::default(),
        reported: Default::default(),
        resumed_from: None,
        native_session_id: None,
        native_turn_id: None,
        native_version: None,
        state: ymp_core::InvocationState::Running,
        started_at: ymp_core::now(),
        ended_at: None,
        usage: None,
        terminal_reason: None,
    };
    let grants = vec![ymp_core::GrantRecord::for_assignment(
        &assignment,
        &open,
        ymp_core::TeamOperation::coordination(),
    )];
    assignment.grant_ids = grants.iter().map(|grant| grant.id.clone()).collect();
    run.store
        .begin_invocation_with_grants(&assignment, &open, &grants)
        .unwrap();

    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/team", 100);
    assert!(!app.active);
    let idle = row_right(&mut app, 65, &assignment.agent_id);
    assert!(
        idle.contains("a turn was left open"),
        "an open record in a stored session was described as live: {idle}"
    );
    app.command("/assignments", 100);
    let row_key = text::short_id(&assignment.id);
    let idle_row = right_of_key(&mut app, 65, &row_key);
    assert!(
        idle_row.contains("left open") && !idle_row.contains("running"),
        "an open record in a stored session was described as running: {idle_row}"
    );
    assert!(
        app.page(65).subtitle.contains("1 left open"),
        "the page described a stored open turn as one in flight: {}",
        app.page(65).subtitle
    );

    app.active = true;
    app.command("/team", 100);
    let live = row_right(&mut app, 65, &assignment.agent_id);
    assert!(
        live.contains("running now"),
        "a turn open during an active run was not described as running: {live}"
    );
    app.command("/assignments", 100);
    let live_row = right_of_key(&mut app, 65, &row_key);
    assert!(
        live_row.contains("running"),
        "a turn in flight was not described as running: {live_row}"
    );
    assert!(
        app.page(65).subtitle.contains("1 running"),
        "the page does not count the turn that is in flight: {}",
        app.page(65).subtitle
    );

    // Stopping a run recovers the turn it left open, which is the call the runtime makes
    // after a cancellation. The record then says interrupted, and nothing says completed.
    assert_eq!(
        run.store.interrupt_open_invocations(&run.session).unwrap(),
        1
    );
    app.active = false;
    app.command("/assignments", 100);
    let stopped = right_of_key(&mut app, 65, &row_key);
    assert!(
        stopped.contains("interrupted"),
        "a recovered turn was not described as interrupted: {stopped}"
    );
    assert!(
        app.page(65).subtitle.contains("none open"),
        "a recovered turn is still counted as open: {}",
        app.page(65).subtitle
    );
    let detail = detail_of_key(&mut app, 65, &row_key);
    assert!(
        detail.contains("Runtime recovered an invocation without an observed terminal result"),
        "the reason the turn ended is not shown:\n{detail}"
    );
}

#[tokio::test]
async fn a_budget_stop_is_named_on_the_limits_page() {
    let run = mock_run("Create a greeting", |engine| {
        engine
            .config
            .limits
            .resources
            .as_mut()
            .unwrap()
            .startup_context_chars = 100;
    })
    .await;
    assert_eq!(run.status, "paused", "the budget did not stop the run");
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/limits", 100);

    let stop = row_prose(&mut app, 65, "last stop");
    assert!(
        stop.contains("the budget refused") && stop.contains("allowance"),
        "the recorded stop is not explained:\n{stop}"
    );
    assert!(
        row_right(&mut app, 65, "last stop").contains("context_limit"),
        "the stop does not carry the code the budget recorded"
    );
    let admitted = row_prose(&mut app, 65, "turns admitted");
    assert!(
        admitted.contains("in flight only while a run is active"),
        "an open turn count was not qualified:\n{admitted}"
    );
}
