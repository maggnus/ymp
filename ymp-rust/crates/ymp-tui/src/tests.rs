//! Interface tests.
//!
//! Every test builds its own temporary ymp home and working directory, and none of them
//! reach a provider: the state layer reads the store directly, and anything that could
//! start a turn leaves as an `Action` the test inspects instead of executing.

use crate::exit::{self, Departure};
use crate::prefs::Prefs;
use crate::state::{route, Action, App, Field, Focus, Overlay, Route};
use crate::text;
use crate::theme;
use crate::ui;
use crate::views::View;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, ModifierKeyCode};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::path::PathBuf;
use std::time::{Duration, Instant};
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
fn command_lists_align_their_summaries_and_keep_text_off_the_border() {
    let fixture = fixture();
    // The column each of the first commands' summaries starts in, on the row that lists it.
    let columns = |rows: &[String]| -> Vec<usize> {
        crate::commands::COMMANDS
            .iter()
            .take(4)
            .map(|command| {
                let head: String = command.summary.chars().take(12).collect();
                let row = rows
                    .iter()
                    .find(|row| row.contains('│') && row.contains(&format!(" {} ", command.name)))
                    .unwrap_or_else(|| {
                        panic!("{} is not listed:\n{}", command.name, rows.join("\n"))
                    });
                let at = row
                    .find(&head)
                    .unwrap_or_else(|| panic!("{} lost its summary:\n{row}", command.name));
                row[..at].chars().count()
            })
            .collect()
    };

    let mut app = fixture.app();
    typed(&mut app, "/");
    let rows = screen_rows(&mut app, 120, 40);
    let inline = columns(&rows);
    assert!(
        inline.windows(2).all(|pair| pair[0] == pair[1]),
        "completion summaries start in different columns: {inline:?}"
    );
    assert!(
        rows.iter().any(|row| row.contains("│ › /chat")),
        "the completion list runs into its border:\n{}",
        rows.join("\n")
    );

    let mut app = fixture.app();
    app.on_key(control('p'), 120);
    let rows = screen_rows(&mut app, 120, 40);
    let palette = columns(&rows);
    assert!(
        palette.windows(2).all(|pair| pair[0] == pair[1]),
        "palette summaries start in different columns: {palette:?}"
    );
    assert!(
        rows.iter().any(|row| row.contains("│ › /chat")),
        "the palette runs into its border:\n{}",
        rows.join("\n")
    );
}

#[test]
fn a_floating_surface_leaves_the_rules_around_the_body_intact() {
    let fixture = fixture();
    // At 80x24 the palette starts on the first row of the body, so the margin cleared around
    // it falls on the rule under the header unless the margin is kept inside the body.
    let (width, height) = (80u16, 24u16);
    let rules = [1, height as usize - 3];
    type Open = fn(&mut App);
    let surfaces: [(&str, Open); 2] = [
        ("palette", |app| {
            app.on_key(control('p'), 80);
        }),
        ("theme chooser", |app| {
            app.on_key(control('t'), 80);
        }),
    ];
    for (name, open) in surfaces {
        let mut app = fixture.app();
        open(&mut app);
        assert!(app.overlay.is_some(), "the {name} did not open");
        let rows = screen_rows(&mut app, width, height);
        for rule in rules {
            assert!(
                rows[rule].chars().all(|ch| ch == '─'),
                "the {name} cut into the rule on row {rule}:\n{}",
                rows.join("\n")
            );
        }
    }
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
    assert_eq!(app.focus, Focus::Composer, "the sidebar takes no focus");
    app.on_key(key(KeyCode::BackTab), 120);
    assert_eq!(app.focus, Focus::Main);

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

/// Every page, in the order the commands list them.
const PAGES: &[View] = &[
    View::Chat,
    View::Tasks,
    View::Usage,
    View::Sessions,
    View::Files,
    View::Changes,
    View::Checks,
    View::Assignments,
    View::Decisions,
    View::Team,
    View::Agents,
    View::Providers,
    View::Memory,
    View::Reputation,
    View::Limits,
    View::Help,
];

#[test]
fn the_sidebar_lists_no_destinations() {
    let fixture = fixture();
    let mut app = fixture.app();
    let width = 120u16;
    let side = crate::frame::sidebar_width(width) as usize;
    let sidebar = screen_rows(&mut app, width, 40)
        .iter()
        .map(|row| row.chars().skip(width as usize - side).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(sidebar.contains("SESSION"), "{sidebar}");
    for view in PAGES {
        assert!(
            !sidebar.contains(view.title()),
            "the sidebar still lists {view:?}:\n{sidebar}"
        );
    }
}

// ---------------------------------------------------------------------------
// Leaving
// ---------------------------------------------------------------------------

#[test]
fn a_first_ctrl_c_leaves_active_work_and_the_draft_untouched() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.active = true;
    app.status = "Working in the project".into();
    typed(&mut app, "a draft");

    assert_eq!(app.on_key(control('c'), 100), Vec::<Action>::new());
    assert!(app.active, "the run is not stopped");
    assert_eq!(app.status, "Working in the project");
    assert_eq!(app.input.value, "a draft");
    assert_eq!(app.on_key(control('c'), 100), vec![Action::Quit]);
}

#[test]
fn a_first_ctrl_c_when_idle_does_not_leave() {
    let fixture = fixture();
    let mut app = fixture.app();
    assert_eq!(app.on_key(control('c'), 100), Vec::<Action>::new());
    assert_eq!(app.on_key(control('c'), 100), vec![Action::Quit]);
}

#[test]
fn ctrl_c_twice_leaves_from_an_open_overlay_without_closing_it_first() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.on_key(control('p'), 100);
    typed(&mut app, "the");

    assert_eq!(app.on_key(control('c'), 100), Vec::<Action>::new());
    assert!(matches!(&app.overlay, Some(Overlay::Palette { field, .. }) if field.value == "the"));
    assert_eq!(app.on_key(control('c'), 100), vec![Action::Quit]);
}

#[test]
fn a_first_ctrl_c_during_a_catalog_reading_stops_nothing() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.status = "Asking the installations what they offer".into();
    assert!(app.on_key(control('c'), 100).is_empty());
    assert_eq!(app.status, "Asking the installations what they offer");
}

#[test]
fn the_question_is_painted_and_withdrawn_when_its_window_passes() {
    let fixture = fixture();
    let mut app = fixture.app();
    let start = Instant::now();

    assert!(app.on_key_at(control('c'), 100, start).is_empty());
    assert!(draw(&mut app, 100, 30).contains(exit::CONFIRM_PROMPT));
    app.expire_exit_request(start + Duration::from_millis(1_999));
    assert!(draw(&mut app, 100, 30).contains(exit::CONFIRM_PROMPT));

    app.expire_exit_request(start + exit::CONFIRM_WINDOW);
    assert!(!draw(&mut app, 100, 30).contains(exit::CONFIRM_PROMPT));
    let later = start + Duration::from_millis(2_100);
    assert!(app.on_key_at(control('c'), 100, later).is_empty());
}

#[test]
fn a_second_ctrl_c_after_the_window_only_asks_again() {
    let fixture = fixture();
    let mut app = fixture.app();
    let start = Instant::now();
    let late = start + exit::CONFIRM_WINDOW;

    assert!(app.on_key_at(control('c'), 100, start).is_empty());
    assert!(app.on_key_at(control('c'), 100, late).is_empty());
    assert_eq!(
        app.on_key_at(control('c'), 100, late + Duration::from_millis(1_999)),
        vec![Action::Quit]
    );
}

#[test]
fn other_input_withdraws_the_question() {
    let fixture = fixture();
    let mut app = fixture.app();
    let start = Instant::now();
    let at = |ms: u64| start + Duration::from_millis(ms);

    assert!(app.on_key_at(control('c'), 100, at(0)).is_empty());
    assert!(app
        .on_key_at(key(KeyCode::Char('x')), 100, at(100))
        .is_empty());
    assert_eq!(app.input.value, "x", "the key still does its own work");
    assert!(app.on_key_at(control('c'), 100, at(200)).is_empty());

    app.paste(" pasted");
    assert!(app.on_key_at(control('c'), 100, at(300)).is_empty());

    // Ctrl held on the way to the second press is part of that press.
    let held = KeyEvent::new(
        KeyCode::Modifier(ModifierKeyCode::LeftControl),
        KeyModifiers::CONTROL,
    );
    assert!(app.on_key_at(held, 100, at(400)).is_empty());
    assert_eq!(
        app.on_key_at(control('c'), 100, at(500)),
        vec![Action::Quit]
    );
    assert_eq!(app.input.value, "x pasted");
}

/// Only repeats and releases a terminal reports as such can be told apart. ymp does not ask for
/// event types, so a terminal that repeats a held Ctrl+C as plain presses is not covered here.
#[test]
fn a_repeated_or_released_key_is_not_a_second_press() {
    let fixture = fixture();
    let mut app = fixture.app();
    let start = Instant::now();
    let later = start + Duration::from_millis(100);

    assert!(app.on_key_at(control('c'), 100, start).is_empty());
    for (code, modifiers, kind) in [
        (
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
            KeyEventKind::Repeat,
        ),
        (
            KeyCode::Char('c'),
            KeyModifiers::CONTROL,
            KeyEventKind::Release,
        ),
        (
            KeyCode::Char('x'),
            KeyModifiers::NONE,
            KeyEventKind::Release,
        ),
    ] {
        let event = KeyEvent::new_with_kind(code, modifiers, kind);
        assert!(app.on_key_at(event, 100, later).is_empty(), "{event:?}");
        assert!(
            app.exit_requested.is_some(),
            "{event:?} withdrew the question"
        );
    }
    assert_eq!(app.on_key_at(control('c'), 100, later), vec![Action::Quit]);
}

#[test]
fn ctrl_c_asks_the_same_question_from_every_region() {
    let fixture = fixture();
    let start = Instant::now();
    let later = start + Duration::from_millis(300);
    type Open = fn(&mut App);
    let regions: [(&str, Open); 4] = [
        ("composer", |_| {}),
        ("transcript", |app| app.focus = Focus::Main),
        ("page", |app| app.set_view(View::Tasks)),
        ("theme chooser", |app| {
            app.on_key(control('t'), 120);
        }),
    ];
    for (region, open) in regions {
        let mut app = fixture.app();
        app.viewport.width = 120;
        open(&mut app);
        let before = (app.focus, app.view, app.overlay.is_some(), app.theme.id);

        assert!(
            app.on_key_at(control('c'), 120, start).is_empty(),
            "{region}"
        );
        assert_eq!(
            (app.focus, app.view, app.overlay.is_some(), app.theme.id),
            before,
            "{region}"
        );
        assert!(
            draw(&mut app, 120, 30).contains(exit::CONFIRM_PROMPT),
            "{region}"
        );
        assert_eq!(
            app.on_key_at(control('c'), 120, later),
            vec![Action::Quit],
            "{region}"
        );
    }
}

#[test]
fn quit_and_ctrl_d_still_leave_on_one_explicit_request() {
    let fixture = fixture();
    let mut app = fixture.app();
    assert_eq!(submit(&mut app, "/quit"), vec![Action::Quit]);
    assert_eq!(app.on_key(control('d'), 100), vec![Action::Quit]);
    typed(&mut app, "draft");
    assert!(
        app.on_key(control('d'), 100).is_empty(),
        "Ctrl+D keeps a draft"
    );

    app.active = true;
    assert_eq!(submit(&mut app, "/stop"), vec![Action::Cancel]);
}

#[test]
fn the_closing_status_names_the_work_it_waits_on() {
    assert_eq!(exit::closing_status(false, false), "Leaving ymp");
    assert!(exit::closing_status(true, false).contains("stopping the active run and waiting"));
    assert!(exit::closing_status(false, true).contains("stopping the catalog reading"));
    assert!(exit::closing_status(true, true).contains("the active run and the catalog reading"));
}

#[tokio::test]
async fn leaving_stops_the_work_and_names_a_session_opened_while_it_stopped() {
    let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
    let cancel = tokio_util::sync::CancellationToken::new();
    let scan_cancel = tokio_util::sync::CancellationToken::new();
    let run = tokio::spawn({
        let cancel = cancel.clone();
        async move {
            cancel.cancelled().await;
            let message = Message {
                seq: 1,
                session_id: "opened-while-stopping".into(),
                author: "you".into(),
                recipient: None,
                kind: "user".into(),
                text: "A prompt".into(),
                created_at: now(),
            };
            events.send(UiEvent::Message(message)).unwrap();
        }
    });
    let scan = tokio::spawn({
        let scan_cancel = scan_cancel.clone();
        async move { scan_cancel.cancelled().await }
    });

    let departure = exit::stop_and_wait(
        &cancel,
        &scan_cancel,
        Some(run),
        Some(scan),
        &mut received,
        None,
        Duration::from_secs(30),
    )
    .await;
    assert_eq!(
        departure,
        Departure {
            session: Some("opened-while-stopping".into()),
            unfinished: false,
        }
    );
}

#[tokio::test]
async fn leaving_waits_only_so_long_for_work_that_does_not_stop() {
    let (_events, mut received) = tokio::sync::mpsc::unbounded_channel();
    let cancel = tokio_util::sync::CancellationToken::new();
    let stuck = tokio::spawn(std::future::pending::<()>());

    let departure = exit::stop_and_wait(
        &cancel,
        &tokio_util::sync::CancellationToken::new(),
        Some(stuck),
        None::<tokio::task::JoinHandle<()>>,
        &mut received,
        Some("loaded".into()),
        Duration::from_millis(50),
    )
    .await;
    assert!(cancel.is_cancelled());
    assert_eq!(
        departure,
        Departure {
            session: Some("loaded".into()),
            unfinished: true,
        }
    );
}

/// The arguments a shell reads from a printed `ymp` command, without starting ymp.
fn shell_words(shell: &str, flags: &[&str], command: &str) -> Vec<String> {
    let arguments = command.strip_prefix("ymp ").expect("a ymp command");
    let scratch = TempDir::new().unwrap();
    let output = std::process::Command::new(shell)
        .args(flags)
        .arg("-c")
        .arg(format!("printf '%s\\0' {arguments}"))
        .current_dir(scratch.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{shell}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .split_terminator('\0')
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_resume_command_names_the_saved_project_and_reads_back_through_a_shell() {
    let fixture = fixture();
    let elsewhere = TempDir::new().unwrap();
    let directory = elsewhere
        .path()
        .join("project ' $(printf leaked) \"literal\"");
    std::fs::create_dir(&directory).unwrap();
    let project = fixture.store.project(&directory).unwrap();
    let session = Session {
        id: new_id(),
        project_id: project.id.clone(),
        title: "Stopped".into(),
        status: "paused".into(),
        created_at: now(),
        team: Config::default().members(),
        turns_used: 0,
    };
    fixture.store.save_session(&session).unwrap();

    // The window started in the fixture project and then opened a session saved elsewhere.
    let mut app = fixture.app();
    app.load_session(&session.id).unwrap();
    let departure = Departure {
        session: app.session.clone(),
        unfinished: false,
    };
    let text = exit::farewell(&fixture.store, &departure);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[0], "Resume this session with:", "{text}");
    assert!(lines[2].contains("starts no agents") && lines[2].contains("/resume"));

    let home = std::path::absolute(&fixture.store.home).unwrap();
    assert_eq!(
        shell_words("/bin/sh", &[], lines[1]),
        [
            "--home",
            home.to_str().unwrap(),
            "-C",
            project.path.to_str().unwrap(),
            "resume",
            session.id.as_str(),
        ]
    );
}

#[test]
fn only_a_session_that_exists_gets_a_resume_command() {
    let fixture = fixture();
    let id = fixture.seed_session("Finished");
    let home = std::path::absolute(&fixture.store.home).unwrap();
    let command = exit::resume_command(&fixture.store, &id).unwrap();
    assert!(
        command.starts_with(&format!(
            "ymp --home {} -C ",
            exit::shell_word(home.to_str().unwrap())
        )),
        "the metadata directory is named even when it is the standard one: {command}"
    );
    assert!(command.ends_with(&format!(" resume {id}")), "{command}");

    assert!(fixture.app().session.is_none());
    assert_eq!(exit::farewell(&fixture.store, &Departure::default()), "");
    let stopped = Departure {
        session: None,
        unfinished: true,
    };
    let text = exit::farewell(&fixture.store, &stopped);
    assert!(text.contains("stopped waiting") && !text.contains("Resume this session"));

    let missing = Departure {
        session: Some(new_id()),
        unfinished: false,
    };
    let text = exit::farewell(&fixture.store, &missing);
    assert!(text.contains("could not be read again") && !text.contains("Resume this session"));
}

#[test]
fn a_word_with_quotes_or_control_characters_reads_back_exactly() {
    let words = [
        "plain-1.2_x/y:z@a+b,c",
        "two words",
        "it's",
        "",
        "tab\there",
        "\u{1b}]0;title\u{7}",
        "back\\slash 'single' \"double\" $HOME `id` $(printf leaked)",
    ];
    let quoted: Vec<String> = words.iter().map(|word| exit::shell_word(word)).collect();
    assert_eq!(quoted[0], words[0], "a plain word stays as it is");
    assert!(
        quoted
            .iter()
            .all(|word| !word.chars().any(char::is_control)),
        "{quoted:?}"
    );
    let command = format!("ymp {}", quoted.join(" "));
    for (shell, flags) in [
        ("/bin/bash", &["--noprofile", "--norc"][..]),
        ("/bin/zsh", &["-f"][..]),
    ] {
        if std::path::Path::new(shell).exists() {
            assert_eq!(shell_words(shell, flags, &command), words, "{shell}");
        }
    }
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
    for view in PAGES {
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
    let entries = crate::transcript::build(
        &app.messages,
        &[],
        &Default::default(),
        &app.attribution,
        true,
    );
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
            rendered.contains("SESSION"),
            "no sidebar at {width} columns"
        );
        assert!(rendered.contains("TEAM"), "no team activity at {width}");
    }
    app.command("/sidebar", 100);
    let rendered = draw(&mut app, 120, 30);
    assert!(!rendered.contains("SESSION"));
    assert!(!Prefs::load(&fixture.store).sidebar);
}

/// The sidebar as plain lines, `width` cells wide and at most `height` rows tall.
fn sidebar_text(app: &App, width: usize, height: usize) -> Vec<String> {
    crate::sidebar::lines(app, width, height)
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect()
        })
        .collect()
}

/// The display column at which `needle` starts in `line`.
fn column_of(line: &str, needle: &str) -> Option<usize> {
    line.find(needle).map(|at| text::width(&line[..at]))
}

#[tokio::test]
async fn the_sidebar_lists_tokens_team_and_open_tasks_as_aligned_tables() {
    let run = mock_run("Create a greeting", |_| {}).await;
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.event(usage_event(
        &run.session,
        &[
            ("one", Some((12_000, 345)), false),
            ("two", Some((80, 9)), false),
        ],
    ));
    assert!(!app.tasks.is_empty(), "the fixture planned no task");
    app.tasks[0].state = ymp_core::TaskState::Running;
    let accepted = app
        .tasks
        .iter()
        .filter(|task| task.state == ymp_core::TaskState::Accepted)
        .count();

    let lines = sidebar_text(&app, 30, 80);
    let section = |title: &str| -> Vec<String> {
        lines
            .iter()
            .skip_while(|line| !line.starts_with(title))
            .take_while(|line| !line.is_empty())
            .cloned()
            .collect()
    };
    assert!(
        !lines.iter().any(|line| line.contains("NAVIGATE")),
        "navigation came back to the sidebar:\n{}",
        lines.join("\n")
    );

    // Figures end under the end of their column title.
    let tokens = section("TOKENS");
    assert_eq!(tokens.len(), 4, "{}", tokens.join("\n"));
    let header = tokens[1].trim_end();
    assert!(
        header.starts_with("AGENT") && header.ends_with("TOKENS"),
        "the token table has no column titles:\n{}",
        tokens.join("\n")
    );
    for row in &tokens[2..] {
        assert_eq!(
            text::width(row.trim_end()),
            text::width(header),
            "a token figure does not end under its title:\n{}",
            tokens.join("\n")
        );
    }

    // Activity words start under their title.
    let team = section("TEAM");
    let activity = column_of(&team[1], "ACTIVITY").expect("the team table has no activity column");
    assert!(column_of(&team[1], "AGENT").is_some());
    assert_eq!(team.len(), 4, "{}", team.join("\n"));
    for row in &team[2..] {
        assert_eq!(
            column_of(row, "idle"),
            Some(activity),
            "an activity word is not under its title:\n{}",
            team.join("\n")
        );
    }

    // The accepted count stays beside the title, and open tasks are a table.
    let tasks = section("TASKS");
    assert!(
        tasks[0].contains(&format!("accepted {accepted} / {}", app.tasks.len())),
        "{}",
        tasks.join("\n")
    );
    let state = column_of(&tasks[1], "STATE").expect("the task table has no state column");
    assert!(
        tasks
            .iter()
            .skip(2)
            .any(|row| column_of(row, "running") == Some(state)),
        "an open task's state is not under its title:\n{}",
        tasks.join("\n")
    );

    // The narrowest sidebar cuts a long title in its cell and keeps the state beside it.
    app.tasks[0].title = "A task title far longer than the narrowest sidebar".into();
    let narrowest = (crate::frame::sidebar_width(80) - 2) as usize;
    let lines = sidebar_text(&app, narrowest, 80);
    let open = lines
        .iter()
        .find(|line| line.contains("A task"))
        .expect("the open task is not listed");
    assert!(
        text::width(open) <= narrowest && open.contains('…') && open.contains("running"),
        "a long task title was not cut inside its cell: {open:?}"
    );
    for line in lines.iter().skip_while(|line| !line.starts_with("TOKENS")) {
        assert!(
            text::width(line) <= narrowest,
            "a sidebar table line overflows {narrowest} cells: {line:?}"
        );
    }

    // A shortened section never leaves column titles over no rows.
    let more = app.theme.markers.more;
    for height in 4..30 {
        let lines = sidebar_text(&app, 30, height);
        assert!(lines.len() <= height);
        for (index, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            let header = trimmed.starts_with("AGENT") || trimmed.starts_with("TASK ");
            if !header {
                continue;
            }
            let titled = index > 0
                && ["TOKENS", "TEAM", "TASKS"]
                    .iter()
                    .any(|title| lines[index - 1].starts_with(title));
            let followed = lines
                .get(index + 1)
                .is_some_and(|next| !next.is_empty() && !next.starts_with(more));
            assert!(
                titled && followed,
                "column titles were split from their section or their rows at height {height}:\n{}",
                lines.join("\n")
            );
        }
    }
}

#[test]
fn every_page_renders_at_the_smallest_supported_size() {
    let fixture = fixture();
    fixture.seed_session("Build a landing page");
    let mut app = fixture.app();
    for view in PAGES {
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
        for view in PAGES {
            app.set_view(*view);
            let rendered = draw(&mut app, 100, 30);
            assert!(
                rendered.contains("SESSION"),
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

#[test]
fn detailed_mode_collapses_no_agent_message() {
    let fixture = fixture();
    let mut app = fixture.app();
    post(
        &mut app,
        1,
        "codex",
        "plan",
        &serde_json::json!({"summary": "Split the parser", "tasks": [{"title": "Tokenise the input"}]})
            .to_string(),
    );
    post(
        &mut app,
        2,
        "claude",
        "review",
        &serde_json::json!({"approved": true, "reason": "The tokens match", "evidence": "tokenise passes"})
            .to_string(),
    );
    let report = (1..=12)
        .map(|step| format!("Step {step} of the report."))
        .collect::<Vec<_>>()
        .join("\n\n");
    post(&mut app, 3, "codex", "execute", &report);
    let hidden_by_default = [
        "Tokenise the input",
        "tokenise passes",
        "Step 12 of the report.",
    ];

    app.focus = Focus::Main;
    let collapsed = draw(&mut app, 160, 60);
    assert!(
        collapsed.contains("proposed a plan · 1 task · Split the parser"),
        "{collapsed}"
    );
    assert!(collapsed.contains("more lines · Enter to read the full report"));
    assert!(collapsed.contains("Space expand"), "{collapsed}");
    for text in hidden_by_default {
        assert!(
            !collapsed.contains(text),
            "{text:?} was not collapsed:\n{collapsed}"
        );
    }

    app.command("/details", 160);
    app.focus = Focus::Main;
    let detailed = draw(&mut app, 160, 60);
    assert!(
        detailed.contains("proposed a plan · 1 task · Split the parser"),
        "detailed mode lost the sentence that names the plan:\n{detailed}"
    );
    for text in hidden_by_default {
        assert!(
            detailed.contains(text),
            "detailed mode collapsed {text:?}:\n{detailed}"
        );
    }
    assert!(!detailed.contains("more lines"), "{detailed}");
    assert!(!detailed.contains("Space expand"), "{detailed}");
    app.on_key(key(KeyCode::Char(' ')), 160);
    assert!(
        app.expanded.is_empty(),
        "Space has nothing to expand in detailed mode"
    );

    // Text still arriving is not a message yet, so its preview stays bounded here as well.
    app.event(UiEvent::Delta {
        agent: "codex".into(),
        text: "word ".repeat(2000),
    });
    draw(&mut app, 160, 60);
    let stream = app
        .viewport
        .entries
        .last()
        .copied()
        .expect("the stream entry is rendered");
    assert!(
        stream.1 <= 5,
        "a stream occupied {} rows in detailed mode",
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
    let rows = screen_rows(&mut app, 120, 30);
    let shown = rows
        .iter()
        .skip_while(|row| !row.contains("TEAM"))
        // The section title, then the column titles of its table.
        .skip(2)
        .take_while(|row| row.contains(app.theme.markers.idle))
        .count();
    assert_eq!(
        shown,
        2,
        "the team actually in use disappeared from the sidebar:\n{}",
        rows.join("\n")
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
    // Each agent keeps its row and figure. With no model recorded or read, its label says so
    // rather than borrowing its provider's name.
    assert!(rendered.contains("12.3k"), "{rendered}");
    assert!(!rendered.contains("Codex"), "{rendered}");

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
            access: ymp_core::TaskAccess::default(),
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
/// wide, or the terminal less four, and its border and a cell of padding take the first two
/// and the last two columns.
fn modal_prose(app: &mut App, width: u16, height: u16) -> String {
    let surface = 86u16.min(width.saturating_sub(4)).max(12);
    let left = (width.saturating_sub(surface) / 2 + 2) as usize;
    column_prose(&screen_rows(app, width, height), left, inspect_room(width))
}

/// Columns the read-only inspect surface has for its body: it is 86 columns wide, or the
/// terminal less four, and its frame and padding take two columns on each side.
fn inspect_room(total: u16) -> usize {
    86u16
        .min(total.saturating_sub(4))
        .max(12)
        .saturating_sub(4)
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
    home: TempDir,
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
        home,
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
        knowledge_correction: None,
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

/// The left-hand text of the row with this key, the mark it carries included.
fn left_of_key(app: &mut App, width: u16, key: &str) -> String {
    let page = app.page(width);
    page.items
        .iter()
        .find(|item| item.key == key)
        .map(|item| {
            item.left
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
        execute.contains("post to the team chat") && execute.contains("read the shared board"),
        "the coordination permissions the record grants are not named:\n{execute}"
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
        app.page(65).subtitle.contains("captured 80 turns"),
        "the page does not name the bound this session captured: {}",
        app.page(65).subtitle
    );
    assert!(
        right_of_key(&mut app, 65, "turns").contains('7'),
        "the next run's own value is not shown beside the captured one: {}",
        right_of_key(&mut app, 65, "turns")
    );
    let tokens = row_prose(&mut app, 65, "tokens observed");
    assert!(
        tokens.contains("its tokens stay unknown rather than being counted as zero"),
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
        member.contains("captured by this session and in the roster it holds now"),
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

/// A run whose captured limits carry a token ceiling and a policy for incomplete counts.
///
/// The mock installation reports no token counts unless a member's instructions ask it to.
/// With `reporting`, member `one` reports what it used and member `two` still reports nothing,
/// so some tokens are known while the counts stay incomplete; without it no count is known at
/// all. Either way the captured policy is what decides whether admission goes on.
async fn token_run(
    policy: ymp_core::UnknownUsagePolicy,
    review_reserve: Option<u64>,
    reporting: bool,
) -> Run {
    mock_run("Create a greeting", |engine| {
        let resources = engine.config.limits.resources.as_mut().unwrap();
        resources.observed_tokens = Some(100_000);
        resources.invocation_tokens = Some(4_000);
        resources.review_reserve_tokens = review_reserve;
        resources.unknown_usage = policy;
        if reporting {
            if let Some(agent) = engine.config.agents.iter_mut().find(|a| a.id == "one") {
                agent.instructions = "[mock:usage]".into();
            }
        }
    })
    .await
}

/// The configuration a later run would use, with every token setting moved away from what the
/// session captured, so a page that read today's values would show other ones.
fn moved_token_config(run: &Run, policy: ymp_core::UnknownUsagePolicy) -> Config {
    let mut config = run.config.clone();
    let resources = config.limits.resources.as_mut().unwrap();
    resources.observed_tokens = Some(50_000);
    resources.invocation_tokens = Some(2_000);
    resources.review_reserve_tokens = Some(7_000);
    resources.unknown_usage = policy;
    config
}

/// Admit one more turn that requests its own token allowance, through the store's admission.
///
/// The runtime at this version never requests one, so this is the only way such a record is
/// written, and the store still checks the request against the ceiling the session captured.
/// The turn is left open, which is what makes its allowance held.
fn admit_requested_allowance(run: &Run, tokens: u64) -> String {
    let trace = run.store.trace(&run.session).unwrap();
    let first = trace.assignments.first().unwrap().clone();
    let assignment = ymp_core::AssignmentRecord {
        id: ymp_core::new_id(),
        token_reservation: Some(tokens),
        purpose: "consultation".into(),
        task: None,
        grant_ids: Vec::new(),
        state: ymp_core::InvocationState::Running,
        started_at: ymp_core::now(),
        ended_at: None,
        ..first
    };
    let invocation = ymp_core::InvocationRecord {
        id: ymp_core::new_id(),
        session_id: run.session.clone(),
        assignment_id: assignment.id.clone(),
        execution_backend: None,
        turn: 1,
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
    run.store.admit_invocation(&assignment, invocation).unwrap();
    assignment.id
}

#[tokio::test]
async fn a_reopened_session_shows_the_token_policy_it_captured_and_not_todays() {
    use ymp_core::UnknownUsagePolicy::{BoundedNative, Stop};

    // A captured stop. No count is ever reported, so the turn after the first is refused.
    let stopped = token_run(Stop, Some(10_000), false).await;
    assert_eq!(
        stopped.status, "paused",
        "a stop on incomplete counts did not pause the run"
    );
    let budget = stopped
        .store
        .session_budget(&stopped.session)
        .unwrap()
        .unwrap();
    assert_eq!(
        budget
            .last_denial
            .as_ref()
            .map(|denial| denial.code.as_str()),
        Some("unknown_usage"),
        "the run stopped for another reason"
    );
    let protected = budget
        .protected_review_tokens
        .expect("a captured reserve protects review tokens");
    let mut app = App::new(
        stopped.store.clone(),
        moved_token_config(&stopped, BoundedNative),
        PathBuf::from(stopped.project.path()),
    );
    app.load_session(&stopped.session).unwrap();
    app.command("/limits", 100);
    for (key, value) in [
        ("captured:unknown-usage", "stop admitting".to_owned()),
        ("captured:token-ceiling", "100000".to_owned()),
        ("captured:turn-allowance", "4000".to_owned()),
        ("captured:review-tokens", protected.to_string()),
        ("captured:bound", "not proved".to_owned()),
        ("captured:denial", "unknown_usage".to_owned()),
    ] {
        assert_eq!(
            right_of_key(&mut app, 65, key),
            value,
            "{key} does not show what the session captured"
        );
    }
    let policy = detail_of_key(&mut app, 65, "captured:unknown-usage");
    assert!(
        policy.contains("admits no further turn under its token ceiling")
            && !policy.contains("goes on admitting"),
        "a captured stop reads as the policy the configuration holds now:\n{policy}"
    );
    let review = detail_of_key(&mut app, 65, "captured:review-tokens");
    assert!(
        review.contains("captured 10000 tokens for the review it owes")
            && review.contains(&format!("{protected} of them are protected now")),
        "the captured review reserve is not the one shown:\n{review}"
    );
    let ceiling = detail_of_key(&mut app, 65, "captured:token-ceiling");
    assert!(
        ceiling.contains("No turn reported a count, so no spending is known")
            && ceiling.contains("what is truly left is not known"),
        "a session with no count at all reads as if its spending were known:\n{ceiling}"
    );

    // A captured go-on. The same counts are missing, and admission went on to the end.
    let bounded = token_run(BoundedNative, None, true).await;
    assert_eq!(
        bounded.status, "completed",
        "admission on reported counts did not go on"
    );
    let budget = bounded
        .store
        .session_budget(&bounded.session)
        .unwrap()
        .unwrap();
    assert!(
        budget.observed_usage.is_partial() && !budget.strict_token_bound,
        "the fixture no longer has incomplete counts"
    );
    let mut app = App::new(
        bounded.store.clone(),
        moved_token_config(&bounded, Stop),
        PathBuf::from(bounded.project.path()),
    );
    app.load_session(&bounded.session).unwrap();
    app.command("/limits", 100);
    assert_eq!(
        right_of_key(&mut app, 65, "captured:unknown-usage"),
        "admit on reported"
    );
    assert_eq!(
        right_of_key(&mut app, 65, "captured:token-ceiling"),
        "100000"
    );
    let policy = detail_of_key(&mut app, 65, "captured:unknown-usage");
    for expected in [
        "goes on admitting turns against what was reported",
        "An incomplete count stays incomplete",
        "not what is truly left",
        "no strict token bound follows",
        "Not every count in this session is complete",
    ] {
        assert!(
            policy.contains(expected),
            "the captured policy does not say {expected:?}:\n{policy}"
        );
    }
    let ceiling = detail_of_key(&mut app, 65, "captured:token-ceiling");
    assert!(
        ceiling.contains("By reported counts")
            && ceiling.contains("what is truly left is not known: it is at most"),
        "a remainder by reported counts reads as a known one:\n{ceiling}"
    );
    let strict = detail_of_key(&mut app, 65, "captured:bound");
    assert!(
        strict.contains("Its counts are also incomplete")
            && strict.contains("Admitting on reported counts does not change that"),
        "going on without counts reads as a bound:\n{strict}"
    );
    let review = detail_of_key(&mut app, 65, "captured:review-tokens");
    assert!(
        review.contains(&format!(
            "4000 × {} = {}",
            budget.protected_review_invocations,
            budget.protected_review_tokens.unwrap()
        )),
        "the protection without a captured reserve is not explained by its parts:\n{review}"
    );
    if keys_of(&mut app, 65).contains(&"captured:denial".to_owned()) {
        assert_ne!(
            right_of_key(&mut app, 65, "captured:denial"),
            "unknown_usage",
            "a session that went on admitting shows a stop on incomplete counts"
        );
    }
}

#[tokio::test]
async fn an_assignment_says_whether_its_token_allowance_was_requested_or_inherited() {
    let run = token_run(
        ymp_core::UnknownUsagePolicy::BoundedNative,
        Some(10_000),
        false,
    )
    .await;
    let inherited = run.store.trace(&run.session).unwrap().assignments[0].clone();
    assert_eq!(
        inherited.token_reservation, None,
        "the runtime requested an allowance of its own"
    );
    let requested = admit_requested_allowance(&run, 1_500);

    // A later configuration with another default must not change what either turn was given.
    let mut config = run.config.clone();
    let resources = config.limits.resources.as_mut().unwrap();
    resources.invocation_tokens = Some(2_000);
    resources.observed_tokens = Some(50_000);
    let mut app = App::new(run.store.clone(), config, PathBuf::from(run.project.path()));
    app.load_session(&run.session).unwrap();
    app.command("/assignments", 100);
    let detail = detail_of_key(&mut app, 65, &text::short_id(&requested));
    assert!(
        detail.contains(
            "token allowance 1500 · requested by this assignment, within the per-turn ceiling of 4000 the session captured"
        ),
        "an allowance the assignment requested is not named as its own:\n{detail}"
    );
    let detail = detail_of_key(&mut app, 65, &text::short_id(&inherited.id));
    assert!(
        detail.contains(
            "token allowance 4000 · inherited: this assignment requested none, so it took the session's per-turn default"
        ),
        "an inherited allowance is not named as the session's default:\n{detail}"
    );

    app.command("/limits", 100);
    let ceiling = detail_of_key(&mut app, 65, "captured:token-ceiling");
    assert!(
        ceiling.contains("1500 are held for turns still open"),
        "the allowance an open turn holds is not counted against the ceiling:\n{ceiling}"
    );
}

#[tokio::test]
async fn a_session_without_a_token_ceiling_names_no_allowance_and_protects_nothing() {
    let run = mock_run("Create a greeting", |_| {}).await;
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/limits", 100);
    for key in [
        "captured:token-ceiling",
        "captured:turn-allowance",
        "captured:review-tokens",
    ] {
        assert_eq!(
            right_of_key(&mut app, 65, key),
            "none",
            "{key} shows a figure the session never captured"
        );
    }
    assert_eq!(
        right_of_key(&mut app, 65, "captured:unknown-usage"),
        "stop admitting"
    );
    let policy = detail_of_key(&mut app, 65, "captured:unknown-usage");
    assert!(
        policy.contains("this policy had nothing to act on"),
        "a policy without a ceiling reads as if it had stopped something:\n{policy}"
    );

    app.command("/assignments", 100);
    let first = run.store.trace(&run.session).unwrap().assignments[0]
        .id
        .clone();
    let detail = detail_of_key(&mut app, 65, &text::short_id(&first));
    assert!(
        detail.contains("token allowance none · the session captured no token ceiling"),
        "a turn without a ceiling is shown with an allowance:\n{detail}"
    );
}

#[tokio::test]
async fn a_session_that_captured_no_resource_limits_shows_no_token_policy_in_their_place() {
    let fixture = fixture();
    let id = fixture.seed_session("An older run");
    fixture
        .store
        .capture_legacy_budget_limits(
            &id,
            &ymp_core::Limits {
                resources: None,
                ..Default::default()
            },
        )
        .unwrap();
    let mut app = fixture.app();
    app.load_session(&id).unwrap();
    app.command("/limits", 100);

    let keys = keys_of(&mut app, 65);
    assert!(
        keys.contains(&"captured:token-policy".to_owned())
            && !keys.contains(&"captured:token-ceiling".to_owned()),
        "a capture without resource limits was given today's token rows: {keys:?}"
    );
    assert_eq!(
        right_of_key(&mut app, 65, "captured:token-policy"),
        "not captured"
    );
    let detail = detail_of_key(&mut app, 65, "captured:token-policy");
    assert!(
        detail.contains("None of today's values is shown in their place"),
        "an absent token policy is not distinguished from a captured one:\n{detail}"
    );
}

/// The headings of the page, in order, as one line each.
fn headings_of(app: &mut App, width: u16) -> Vec<String> {
    app.page(width)
        .items
        .iter()
        .filter(|item| item.kind == crate::views::ItemKind::Heading)
        .map(|item| lines_prose(&[ratatui::text::Line::from(item.left.clone())]))
        .collect()
}

#[tokio::test]
async fn a_member_the_roster_replaced_is_not_presented_as_one_and_keeps_its_records() {
    // One member at a time is how the runtime reaches an independent reviewer: it replaces
    // the member instead of adding one, so the session captures two identities and holds a
    // roster of one. Nothing here is hand-written into the store.
    let run = mock_run("Create a greeting", |engine| {
        engine.config.team_constraints.fixed_size = Some(1);
    })
    .await;
    let roster = run.store.team_state(&run.session).unwrap().unwrap();
    let captured = run.store.session(&run.session).unwrap().team;
    assert_eq!(
        roster.current_members.len(),
        1,
        "the roster did not stay at one"
    );
    assert_eq!(captured.len(), 2, "the session captured only one identity");
    let replaced = captured
        .iter()
        .find(|profile| !roster.current_members.contains(&profile.id))
        .expect("one captured identity left the roster")
        .id
        .clone();

    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/team", 100);
    let headings = headings_of(&mut app, 65);
    assert!(
        headings
            .iter()
            .any(|heading| heading.contains("NO LONGER A MEMBER")),
        "a replaced identity is presented among the members: {headings:?}"
    );
    let right = right_of_key(&mut app, 65, &replaced);
    assert!(
        right.contains("no longer a member"),
        "a replaced identity is still described as a member: {right}"
    );
    let detail = detail_of_key(&mut app, 65, &replaced);
    assert!(
        detail.contains(&format!("revision {}", roster.revision)),
        "the row does not name the roster it is measured against:\n{detail}"
    );
    assert!(
        detail.contains("turns recorded here"),
        "the work a replaced identity did is not counted:\n{detail}"
    );
    let member = roster.current_members[0].clone();
    assert!(
        detail_of_key(&mut app, 65, &member).contains("in the roster it holds now"),
        "a current member is not distinguished from a replaced one:\n{}",
        detail_of_key(&mut app, 65, &member)
    );
    assert!(
        app.page(65).subtitle.contains("1 in the roster")
            && app.page(65).subtitle.contains("2 captured by this session"),
        "the page does not say which list is which: {}",
        app.page(65).subtitle
    );
    let roster_row = detail_of_key(&mut app, 65, "roster");
    assert!(
        roster_row.contains("availability and not authority"),
        "the reserved reviewer is presented as a rank:\n{roster_row}"
    );
    assert!(
        roster_row.contains(&roster.method),
        "the page does not name the method the runtime recorded:\n{roster_row}"
    );
}

#[tokio::test]
async fn a_session_that_recorded_no_roster_says_so_and_still_shows_its_records() {
    let run = mock_run("Create a greeting", |_| {}).await;
    // A record shape this version cannot write: the same assignment records, no captured
    // team and no roster, which is what a session stored before membership was recorded
    // reads like. The snapshot is replaced, not the store, because the page is a pure
    // function of the snapshot the controller took.
    let mut trace = run.store.trace(&run.session).unwrap();
    trace.session.team.clear();
    trace.team_state = None;
    let worked = trace.assignments[0].agent_id.clone();
    let mut config = run.config.clone();
    config.team.clear();
    let mut app = App::new(run.store.clone(), config, PathBuf::from(run.project.path()));
    app.load_session(&run.session).unwrap();
    app.command("/team", 100);
    app.session_team.clear();
    app.records = crate::provenance::Records {
        session: Some(run.session.clone()),
        read_at: ymp_core::now(),
        trace: Some(trace),
        ..Default::default()
    };

    // A width the page was not built at, so the injected snapshot is the one it reads.
    let headings = headings_of(&mut app, 64);
    assert!(
        headings
            .iter()
            .any(|heading| heading.contains("WORKED HERE")),
        "agents with records and no membership are not shown apart: {headings:?}"
    );
    let detail = detail_of_key(&mut app, 64, &worked);
    assert!(
        detail.contains("either it is no longer a member, or the list does not name it"),
        "the page does not say what this row means:\n{detail}"
    );
    assert!(
        detail.contains("turns recorded here"),
        "the work this agent did is not counted:\n{detail}"
    );
    let roster_row = detail_of_key(&mut app, 64, "roster");
    assert!(
        roster_row.contains("recorded no roster of its own"),
        "an absent roster is not stated as absent:\n{roster_row}"
    );
    assert!(
        right_of_key(&mut app, 64, "roster").contains("none recorded"),
        "an absent roster is reported as an empty one: {}",
        right_of_key(&mut app, 64, "roster")
    );
}

#[tokio::test]
async fn a_confirmed_projection_and_a_candidate_are_not_shown_as_the_same_thing() {
    let run = mock_run("Create a greeting", |engine| {
        engine.acceptance_contracts.push(exact_greeting_contract())
    })
    .await;
    let project = run.store.project(run.project.path()).unwrap();
    let inventory = run.store.memory_inventory(Some(&project.id)).unwrap();
    let supported = run
        .store
        .search_memory(
            Some(&project.id),
            "",
            &Default::default(),
            ymp_core::KnowledgeRetrievalMode::Supported,
        )
        .unwrap();
    assert!(
        !supported.is_empty() && inventory.len() > supported.len(),
        "the run did not record both a supported projection and a candidate: {} of {}",
        supported.len(),
        inventory.len()
    );
    let mut app = run.app();
    app.command("/memory", 100);

    let keys = keys_of(&mut app, 65);
    for entry in &inventory {
        assert!(
            keys.contains(&entry.id),
            "an entry the store recorded is not on the page: {:?}",
            entry.title
        );
    }
    let confirmed = supported[0].id.clone();
    assert!(
        right_of_key(&mut app, 65, &confirmed).contains("current"),
        "a confirmed projection is not named as current: {}",
        right_of_key(&mut app, 65, &confirmed)
    );
    let candidate = inventory
        .iter()
        .find(|entry| !supported.iter().any(|offered| offered.id == entry.id))
        .expect("the run recorded a candidate");
    assert_ne!(
        right_of_key(&mut app, 65, &candidate.id),
        right_of_key(&mut app, 65, &confirmed),
        "a candidate and a confirmed projection read the same"
    );
    assert!(
        detail_of_key(&mut app, 65, &confirmed)
            .contains("yes, as support under the default retrieval"),
        "the page no longer says whether a run would be given this entry"
    );
    let detail = detail_of_key(&mut app, 65, &confirmed);
    assert!(
        detail.contains("the acceptance it names carried passing checks"),
        "the basis of a confirmed entry is not stated:\n{detail}"
    );
    assert!(
        detail.contains("acceptance") && detail.contains("criteria"),
        "a confirmed entry does not name the record it projects:\n{detail}"
    );

    // An entry a reviewer agreed with, without evidence, was previously labelled reviewed.
    let candidate = inventory
        .iter()
        .find(|entry| {
            entry.reviewer.is_some()
                && entry.provenance.as_ref().is_some_and(|provenance| {
                    provenance.confirmation != ymp_core::ConfirmationStatus::Confirmed
                })
        })
        .expect("the run recorded a candidate a reviewer agreed with");
    let right = right_of_key(&mut app, 65, &candidate.id);
    assert!(
        right.contains("unconfirmed") && !right.contains("supported"),
        "a candidate with a reviewer is presented as checked: {right}"
    );
    let candidate_detail = detail_of_key(&mut app, 65, &candidate.id);
    assert!(
        candidate_detail.contains("no passing evidence is attached"),
        "a candidate does not say what it lacks:\n{candidate_detail}"
    );
    assert!(
        candidate_detail.contains("is not offered as support"),
        "a candidate is not distinguished from context a run would be given:\n{candidate_detail}"
    );
    assert!(
        app.page(65)
            .subtitle
            .contains(&format!("{} recorded", inventory.len()))
            && app
                .page(65)
                .subtitle
                .contains(&format!("{} supported", supported.len())),
        "the page does not separate what is recorded from what is offered: {}",
        app.page(65).subtitle
    );

    run.store.forget_memory(&confirmed).unwrap();
    // A width this page was not built at, so the row is read again after the retirement.
    assert!(
        right_of_key(&mut app, 64, &confirmed).contains("retired"),
        "a retired entry disappears instead of saying it was retired: {}",
        right_of_key(&mut app, 64, &confirmed)
    );
    assert!(
        detail_of_key(&mut app, 64, &confirmed).contains("the entry is retired"),
        "a retired entry does not say it is no longer given to a run"
    );
}

#[tokio::test]
async fn knowledge_a_run_recorded_without_evidence_is_listed_rather_than_hidden() {
    let run = mock_run("Create a greeting", |_| {}).await;
    let project = run.store.project(run.project.path()).unwrap();
    let inventory = run.store.memory_inventory(Some(&project.id)).unwrap();
    assert!(
        !inventory.is_empty(),
        "the run recorded no knowledge at all, so this case is untested"
    );
    assert!(
        run.store.memory(Some(&project.id), "").unwrap().is_empty(),
        "a candidate was returned as support, which this test depends on not happening"
    );
    let mut app = run.app();
    app.command("/memory", 100);

    let keys = keys_of(&mut app, 65)
        .into_iter()
        .filter(|key| key != crate::views::ABOUT_KEY)
        .collect::<Vec<_>>();
    assert_eq!(
        keys.len(),
        inventory.len(),
        "the page shows {} of the {} entries this project has recorded",
        keys.len(),
        inventory.len()
    );
    assert!(
        app.page(65).subtitle.contains("0 supported as context"),
        "the page does not say that none of it would be given to a run: {}",
        app.page(65).subtitle
    );
    for entry in &inventory {
        let detail = detail_of_key(&mut app, 65, &entry.id);
        assert!(
            detail.contains("is not offered as support"),
            "an entry that is not support does not say so: {:?}\n{detail}",
            entry.title
        );
    }
}

#[tokio::test]
async fn an_accepted_outcome_names_the_directory_it_was_recorded_in() {
    let run = mock_run("Create a greeting", |engine| {
        engine.acceptance_contracts.push(exact_greeting_contract())
    })
    .await;
    let recorded = run.store.outcomes(&run.session).unwrap();
    assert!(
        !recorded.is_empty(),
        "the run accepted nothing, so this case is untested"
    );
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/diff", 100);

    let key = recorded[0].result_id.clone();
    // A path longer than the detail is wrapped rather than cut, so the width used for the
    // location assertions is one the whole path fits on.
    let wide = detail_of_key(&mut app, 110, &key);
    assert!(
        wide.contains(&recorded[0].directory.display().to_string()),
        "the outcome does not name the directory it was recorded in:\n{wide}"
    );
    let detail = detail_of_key(&mut app, 65, &key);
    assert!(
        detail.contains("ymp keeps no copy of the artifact anywhere else"),
        "the page implies the artifact exists somewhere else as well:\n{detail}"
    );
    assert!(
        right_of_key(&mut app, 65, &key).contains("confirmed"),
        "a confirmed outcome is not named as one: {}",
        right_of_key(&mut app, 65, &key)
    );

    // The effective mode and the recovery limit are in the leading row, which is the row the
    // first frame opens, so neither is a keystroke away.
    let leading = detail_of_key(&mut app, 65, "recovery");
    for statement in [
        "Agents work in this directory itself",
        "the access its execution backend actually enforces",
        "a mode name is not a filesystem guarantee",
        "A task declared read-only in the plan is asked read-only",
        "Turns overlap only where their recorded access does not conflict",
        "exclusive lock",
        "cannot restore a previous version of a file",
        "Isolated execution with a reviewed publication step is not part of this release",
    ] {
        assert!(
            leading.contains(statement),
            "the page does not state {statement:?}:\n{leading}"
        );
    }
    assert_eq!(
        app.page(65).items[0].key,
        "recovery",
        "the statement is not the first row of the page"
    );
}

#[tokio::test]
async fn a_relocated_project_does_not_move_where_an_outcome_was_recorded() {
    let run = mock_run("Create a greeting", |engine| {
        engine.acceptance_contracts.push(exact_greeting_contract())
    })
    .await;
    let recorded = run.store.outcomes(&run.session).unwrap();
    let before = recorded[0].directory.clone();
    let moved = TempDir::new().unwrap();
    let project = run.store.project(run.project.path()).unwrap();
    run.store
        .relocate_project(&project.id, moved.path())
        .unwrap();
    let elsewhere = moved.path().canonicalize().unwrap();

    // A reader opened after the project's current path changed.
    let mut app = App::new(
        run.store.clone(),
        run.config.clone(),
        PathBuf::from(&elsewhere),
    );
    app.load_session(&run.session).unwrap();
    app.command("/diff", 100);

    let detail = detail_of_key(&mut app, 110, &recorded[0].result_id);
    assert!(
        detail.contains(&before.display().to_string()),
        "the recorded location moved with the project:\n{detail}"
    );
    assert!(
        !detail.contains(&elsewhere.display().to_string()),
        "the page reports the new path as where the result was recorded:\n{detail}"
    );
}

#[test]
fn the_first_screen_says_how_the_directory_is_used_and_what_cannot_be_undone() {
    let fixture = fixture();
    let mut app = fixture.app();
    // The opening screen, at the smallest supported size, with no session yet.
    let screen = main_prose(&mut app, 80, 24);
    assert!(
        screen.contains("create and change files in this directory itself"),
        "the first screen does not say where a run works:\n{screen}"
    );
    assert!(
        screen.contains("it cannot put a file back"),
        "the first screen does not state the limit of what was recorded:\n{screen}"
    );
    assert!(
        screen.contains("directly, no copy kept"),
        "the first screen does not name the effective mode:\n{screen}"
    );
}

#[tokio::test]
async fn a_membership_decision_says_what_it_changed_and_what_it_reserved() {
    let run = mock_run("Create a greeting", |_| {}).await;
    let trace = run.store.trace(&run.session).unwrap();
    let (record, allocation) = trace
        .decisions
        .iter()
        .filter_map(|decision| {
            decision
                .links
                .allocation
                .as_deref()
                .map(|allocation| (decision, allocation))
        })
        .next()
        .expect("the run recorded a membership decision");
    let key = text::short_id(&record.id);
    let reviewer = allocation
        .proposal
        .reserved_final_reviewer
        .clone()
        .expect("the committed roster reserved a final reviewer");

    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/decisions", 100);

    assert!(
        right_of_key(&mut app, 65, &key).contains("membership committed"),
        "a committed membership decision is reported as ungraded: {}",
        right_of_key(&mut app, 65, &key)
    );
    let detail = detail_of_key(&mut app, 65, &key);
    assert!(
        detail.contains("the membership was committed"),
        "the opened record contradicts the row it was opened from:\n{detail}"
    );
    assert!(
        !detail.contains("recorded without an outcome"),
        "a committed membership reads as ungraded when opened:\n{detail}"
    );
    assert!(
        detail.contains("members proposed"),
        "the decision does not say who it admitted:\n{detail}"
    );
    assert!(
        detail.contains(&reviewer),
        "the decision does not name the reviewer it kept free:\n{detail}"
    );
    assert!(
        detail.contains("availability rather than authority"),
        "the reserved reviewer reads like a rank:\n{detail}"
    );
    assert!(
        detail.contains("the session started"),
        "the decision does not say at which moment it was taken:\n{detail}"
    );

    // The per-turn resource bound is a record of the same shape: its outcome is inside it.
    let bound = trace
        .decisions
        .iter()
        .find(|decision| decision.links.resource_allocation.is_some())
        .expect("the run recorded a resource bound");
    let bound_key = text::short_id(&bound.id);
    assert!(
        right_of_key(&mut app, 65, &bound_key).contains("bound set"),
        "a recorded turn bound is reported as ungraded: {}",
        right_of_key(&mut app, 65, &bound_key)
    );
    let bound_detail = detail_of_key(&mut app, 65, &bound_key);
    assert!(
        bound_detail.contains("native turns allowed")
            && bound_detail.contains("output characters allowed"),
        "the bound does not say what it allowed:\n{bound_detail}"
    );
    assert!(
        bound_detail.contains("a bound is not a report that it was reached"),
        "an allowance reads like a measurement:\n{bound_detail}"
    );
}

#[tokio::test]
async fn locations_that_could_not_be_read_are_not_reported_as_none() {
    let run = mock_run("Create a greeting", |engine| {
        engine.acceptance_contracts.push(exact_greeting_contract())
    })
    .await;
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/diff", 100);
    // What the controller holds when the store could not answer where an accepted result
    // was recorded: a reason, and no locations. A session records this when its captured
    // directory is missing, which this version no longer writes.
    app.records.outcomes.clear();
    app.records.outcomes_unreadable =
        Some("outcome_location_unknown: captured directory is missing".to_owned());

    // A width the page was not built at, so the replaced snapshot is the one it reads.
    let right = right_of_key(&mut app, 64, "outcome locations");
    assert!(
        right.contains("could not be read"),
        "a failed read is presented as an answer: {right}"
    );
    let detail = detail_of_key(&mut app, 64, "outcome locations");
    assert!(
        detail.contains("outcome_location_unknown"),
        "the page does not report what the store said:\n{detail}"
    );
    assert!(
        detail.contains("not a statement that the session accepted nothing"),
        "a failed read is not distinguished from an absence of outcomes:\n{detail}"
    );
}

#[tokio::test]
async fn the_record_pages_paint_their_statements_whole_at_every_supported_size() {
    let run = mock_run("Create a greeting", |engine| {
        engine.config.team_constraints.fixed_size = Some(1);
        engine.acceptance_contracts.push(exact_greeting_contract());
    })
    .await;
    for (width, height) in SUPPORTED_SIZES.iter().copied() {
        let mut app = run.app();
        app.load_session(&run.session).unwrap();

        app.set_view(View::Team);
        let prose = main_prose(&mut app, width, height);
        assert!(
            prose.contains("A session holds a roster of the members a turn may be given to now"),
            "at {width}x{height} the team page lost words from what a roster is:\n{prose}"
        );

        app.set_view(View::Memory);
        app.on_key(key(KeyCode::Down), width);
        let prose = main_prose(&mut app, width, height);
        assert!(
            prose.contains("no passing evidence is attached")
                || prose.contains("carried passing checks")
                || prose.contains("recorded without a grade"),
            "at {width}x{height} the memory page lost words from an entry's basis:\n{prose}"
        );

        // The workspace statement is longer than the detail pane at the smallest size, and
        // the surface Enter opens is where the whole of it is read.
        app.set_view(View::Changes);
        app.on_key(key(KeyCode::Enter), width);
        let prose = modal_prose(&mut app, width, height);
        assert!(
            prose.contains("the access its execution backend actually enforces"),
            "at {width}x{height} the workspace statement lost words:\n{prose}"
        );
    }
}

#[tokio::test]
async fn the_access_a_turn_held_is_shown_as_the_backend_enforced_it() {
    let run = mock_run("Create a greeting", |_| {}).await;
    let trace = run.store.trace(&run.session).unwrap();
    let writing = trace
        .assignments
        .iter()
        .find(|assignment| assignment.purpose == "execute")
        .expect("the run executed a task");
    let reading = trace
        .assignments
        .iter()
        .find(|assignment| assignment.purpose == "final_review")
        .expect("the run reviewed the result");
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/assignments", 100);

    let write = detail_of_key(&mut app, 65, &text::short_id(&writing.id));
    assert!(
        write.contains("may write anywhere in the directory"),
        "the access a writing turn held is not stated:\n{write}"
    );
    assert!(
        write.contains("ymp.native"),
        "the page does not name what enforced the access:\n{write}"
    );
    assert!(
        write.contains("turn admitted") && write.contains("until"),
        "the reservation lifecycle is incomplete:\n{write}"
    );
    assert!(
        write.contains("A coordination policy may describe it as broader and never as narrower"),
        "the page does not say what the recorded access is:\n{write}"
    );

    let read = detail_of_key(&mut app, 65, &text::short_id(&reading.id));
    assert!(
        read.contains("reads the directory and writes nothing"),
        "a read-only turn is not distinguished from a writing one:\n{read}"
    );
}

#[tokio::test]
async fn a_task_that_waited_says_what_it_waited_for() {
    // Two tasks, the second depending on the first, which is the runtime's own reason for a
    // recorded wait: the wave cannot admit a task whose dependency is not accepted yet.
    let run = mock_run("Create a greeting", |engine| {
        for agent in &mut engine.config.agents {
            agent.instructions.push_str(" [mock:split-writers]");
        }
    })
    .await;
    let trace = run.store.trace(&run.session).unwrap();
    let waits = trace
        .decisions
        .iter()
        .filter_map(|decision| {
            decision
                .links
                .workspace_wait
                .as_ref()
                .map(|wait| (decision, wait))
        })
        .collect::<Vec<_>>();
    assert!(
        !waits.is_empty(),
        "the run recorded no wait, so this case is untested"
    );
    let (record, wait) = waits[0];
    let task_id = record
        .links
        .task
        .as_ref()
        .expect("the wait names the task that waited")
        .task_id
        .clone();

    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/tasks", 100);
    let task = run
        .store
        .tasks(&run.session)
        .unwrap()
        .into_iter()
        .find(|task| task.id == task_id)
        .expect("the task that waited is still recorded");
    let detail = row_prose(&mut app, 65, &task.title);
    assert!(
        detail.contains(&wait.code),
        "the task does not say it waited, or under which code:\n{detail}"
    );

    app.command("/decisions", 100);
    let key = text::short_id(&record.id);
    assert!(
        right_of_key(&mut app, 65, &key).contains(&format!("waited · {}", wait.code)),
        "a wait is not reported as one: {}",
        right_of_key(&mut app, 65, &key)
    );
    let recorded = detail_of_key(&mut app, 65, &key);
    assert!(
        recorded.contains("waited because") && recorded.contains("holder"),
        "the wait record does not say what held it up:\n{recorded}"
    );
}

#[test]
fn a_read_only_task_is_shown_as_declared_and_never_as_measured() {
    let fixture = fixture();
    let id = fixture.seed_session("Build a landing page");
    let writing = fixture.seed_task(&id, "Write the page", &[]);
    let reading = fixture.seed_task(&id, "Read the existing page", &[]);
    let mut task = fixture
        .store
        .tasks(&id)
        .unwrap()
        .into_iter()
        .find(|task| task.id == reading)
        .unwrap();
    // What the store holds for a task a plan declared read-only.
    task.access = ymp_core::TaskAccess::ReadOnly;
    fixture.store.save_task(&task).unwrap();

    let mut app = fixture.app();
    app.load_session(&id).unwrap();
    app.command("/tasks", 100);
    let read_only = detail_of_key(&mut app, 65, &reading);
    assert!(
        read_only.contains("read-only, declared in the plan"),
        "a declared read-only task does not say so:\n{read_only}"
    );
    assert!(
        read_only.contains("the turn is asked read-only"),
        "the page does not say what the declaration does:\n{read_only}"
    );
    let writes = detail_of_key(&mut app, 65, &writing);
    assert!(
        writes.contains("may write; this is the default where a plan declares nothing"),
        "a writing task is not distinguished from a declared one:\n{writes}"
    );
}

#[test]
fn an_agent_held_up_by_coordination_is_not_reported_as_working() {
    let fixture = fixture();
    let mut app = fixture.app();
    let member = app.config.members()[0].id.clone();
    // What the runtime sends while a turn cannot start: the wait and the code it recorded.
    app.event(ymp_core::UiEvent::AgentStatus {
        agent: member.clone(),
        status: "waiting: resource_conflict".into(),
    });
    // The sidebar, which is where a live state is read: the whole screen, not one column.
    let screen = draw(&mut app, 100, 30);
    assert!(
        screen.contains("waiting"),
        "a waiting agent is not reported as waiting:\n{screen}"
    );
    assert!(
        !screen.contains("busy"),
        "a waiting agent is reported as work in flight:\n{screen}"
    );
}

#[tokio::test]
async fn a_captured_acceptance_contract_is_shown_as_a_binding_and_not_as_a_result() {
    let run = mock_run("Create a greeting", |engine| {
        engine.acceptance_contracts.push(exact_greeting_contract())
    })
    .await;
    let trace = run.store.trace(&run.session).unwrap();
    let captured = trace
        .decisions
        .iter()
        .find(|decision| decision.links.acceptance_contract.is_some())
        .expect("the run captured the contract it was given");
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/decisions", 100);

    let key = text::short_id(&captured.id);
    assert!(
        right_of_key(&mut app, 65, &key).contains("criteria captured"),
        "a captured contract is reported as an ungraded decision: {}",
        right_of_key(&mut app, 65, &key)
    );
    let marked = left_of_key(&mut app, 65, &key);
    assert!(
        !marked.starts_with(crate::theme::UNICODE.idle),
        "the row is marked as a decision without an outcome: {marked}"
    );
    let detail = detail_of_key(&mut app, 65, &key);
    for expected in [
        "greeting-content",
        "exact-greeting-v1",
        "greeting.txt",
        "checker",
        "declared input none",
    ] {
        assert!(
            detail.contains(expected),
            "the contract does not name {expected:?}:\n{detail}"
        );
    }
    assert!(
        detail.contains("It is a binding and not a result"),
        "a captured contract could be read as evidence:\n{detail}"
    );
}

#[tokio::test]
async fn a_record_that_carries_its_own_outcome_never_reads_as_ungraded() {
    let run = mock_run("Create a greeting", |engine| {
        engine.acceptance_contracts.push(exact_greeting_contract())
    })
    .await;
    let trace = run.store.trace(&run.session).unwrap();
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/decisions", 100);

    // Every record whose outcome lives inside itself: the row and the detail read it from the
    // same field, so a reader cannot be told two different things about one record.
    let own_outcome = trace.decisions.iter().filter(|decision| {
        decision.links.allocation.is_some()
            || decision.links.resource_allocation.is_some()
            || decision.links.workspace_wait.is_some()
            || decision.links.workspace_access.is_some()
            || decision.links.acceptance_contract.is_some()
    });
    let mut checked = 0;
    for decision in own_outcome {
        let key = text::short_id(&decision.id);
        let detail = detail_of_key(&mut app, 65, &key);
        assert!(
            !detail.contains("recorded without an outcome"),
            "{} reads as ungraded while its row states an outcome:\n{detail}",
            decision.kind
        );
        let row = right_of_key(&mut app, 65, &key);
        assert!(
            !row.contains("recorded without an outcome"),
            "{} reads as ungraded in its row while its record states an outcome: {row}",
            decision.kind
        );
        checked += 1;
    }
    assert!(
        checked >= 4,
        "the run recorded only {checked} such records, so this case is barely tested"
    );
    let released = trace
        .decisions
        .iter()
        .find(|decision| decision.kind == "workspace_access_released")
        .expect("the run released a reservation");
    assert!(
        detail_of_key(&mut app, 65, &text::short_id(&released.id))
            .contains("the reservation ended"),
        "a released reservation does not say so in its own record"
    );
}

#[test]
fn a_header_gives_up_its_summary_before_its_own_name() {
    // A captured count has no width the page controls, so the header is built to lose the
    // summary rather than the name of the page a reader is standing on.
    use ratatui::text::Span;
    let line = text::header_row(
        53,
        vec![
            Span::raw(" ".to_owned()),
            Span::raw("Limits".to_owned()),
            Span::raw("  /limits".to_owned()),
        ],
        vec![Span::raw(
            "captured 4294967295 turns, 4294967295 at a time ".to_owned(),
        )],
    );
    let painted = line
        .spans
        .iter()
        .map(|span| span.content.as_ref())
        .collect::<String>();
    assert_eq!(
        text::width(&painted),
        53,
        "the header is not exactly one row wide: {painted:?}"
    );
    assert!(
        painted.contains("Limits") && painted.contains("/limits"),
        "the page lost its own name to a long summary: {painted:?}"
    );
    assert!(
        painted.contains('…'),
        "the summary was cut without saying so: {painted:?}"
    );
}

#[tokio::test]
async fn a_page_keeps_its_name_and_command_at_every_supported_size() {
    // Bounds no reader would type, because the page cannot choose how wide a captured
    // number is and the header must survive whatever it reads.
    let run = mock_run("Create a greeting", |engine| {
        engine.config.limits.turns = 987_654;
        engine.config.limits.parallel = 321;
    })
    .await;
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/limits", 100);
    for (width, height) in SUPPORTED_SIZES.iter().copied() {
        let rows = screen_rows(&mut app, width, height);
        let header = rows
            .iter()
            .find(|row| row.contains("/limits"))
            .unwrap_or_else(|| {
                panic!(
                    "the page header is gone at {width}x{height}:\n{}",
                    rows.join("\n")
                )
            })
            .clone();
        assert!(
            header.contains("Limits"),
            "the page lost its own name at {width}x{height}:\n{header}"
        );
        assert!(
            header.contains("captured 987654 turns") || header.contains('…'),
            "the captured bound was dropped without a mark at {width}x{height}:\n{header}"
        );
    }
}

#[tokio::test]
async fn a_setting_rewritten_on_its_way_out_names_both_values() {
    let run = mock_run("Create a greeting", |_| {}).await;
    let trace = run.store.trace(&run.session).unwrap();
    let previous = trace.assignments.last().unwrap().clone();
    let mut assignment = ymp_core::AssignmentRecord {
        id: ymp_core::new_id(),
        grant_ids: Vec::new(),
        state: ymp_core::InvocationState::Running,
        started_at: ymp_core::now(),
        ended_at: None,
        ..previous.clone()
    };
    assignment.requested.permission_mode = Some("write".into());
    // What a backend writes when it rewrites the request into its own vocabulary: the
    // permission mode a transport names is not the word the runtime asked with.
    let turn = ymp_core::InvocationRecord {
        id: ymp_core::new_id(),
        session_id: run.session.clone(),
        assignment_id: assignment.id.clone(),
        execution_backend: None,
        turn: trace.invocations.len() as u64 + 1,
        requested: assignment.requested.clone(),
        sent: ymp_core::ExecutionSettings {
            permission_mode: Some("danger-full-access".into()),
            ..Default::default()
        },
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
        &turn,
        ymp_core::TeamOperation::coordination(),
    )];
    assignment.grant_ids = grants.iter().map(|grant| grant.id.clone()).collect();
    run.store
        .begin_invocation_with_grants(&assignment, &turn, &grants)
        .unwrap();

    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/assignments", 110);
    let detail = detail_of_key(&mut app, 110, &text::short_id(&assignment.id));
    assert!(
        detail.contains("write requested · danger-full-access sent"),
        "the request disappeared behind the value that was sent:\n{detail}"
    );
    assert!(
        detail.contains("unconfirmed, the installation reported nothing"),
        "a value nothing reported was not kept unconfirmed:\n{detail}"
    );
}

#[tokio::test]
async fn credit_this_session_did_not_read_is_reported_and_not_asserted() {
    // A confirmed acceptance in one session credits its producer there. Read from a window
    // with that session not loaded, which is where a reader meets an earlier session's
    // observation, the credit record is outside what this page read.
    let run = mock_run("Create a greeting", |engine| {
        engine.acceptance_contracts.push(exact_greeting_contract())
    })
    .await;
    let producer = run
        .store
        .observations()
        .unwrap()
        .first()
        .expect("a confirmed acceptance credits its producer")
        .agent_name
        .clone();
    let mut app = run.app();
    app.command("/reputation", 100);

    let prose = row_prose(&mut app, 65, &producer);
    assert!(
        prose.contains("confirmed: evidence passed for every criterion"),
        "the observation lost the evidence status it carries:\n{prose}"
    );
    assert!(
        prose.contains("whether it was credited is recorded by"),
        "the page states a credit it never read:\n{prose}"
    );
    assert!(
        !prose.contains("this session recorded the credit for it"),
        "a credit outside what was read is presented as read here:\n{prose}"
    );
}

/// A catalog as a scan of an installation stores it: exact identifiers, the verbatim picker
/// label, the aliases advertised, the control the installation names with its own words, and
/// the time the reading was observed at.
fn scanned_catalog(
    default_model: Option<&str>,
    observed_at: &str,
) -> ymp_core::ProviderCapabilities {
    use ymp_core::{
        CapabilitySource, ModelCapabilities, NativeControl, NativeControlValue,
        NativeControlValues, ProviderCapabilities,
    };
    ProviderCapabilities {
        source: CapabilitySource::NativeMetadata {
            method: "models.list".into(),
            observed_at: observed_at.into(),
        },
        models_complete: true,
        models: vec![ModelCapabilities {
            id: "gpt-5.6-sol".into(),
            display_name: Some("GPT-5.6-Sol".into()),
            picker_id: Some("sol-row".into()),
            aliases: vec!["gpt-5.6".into()],
            resolved_model: Some("gpt-5.6-sol-0913".into()),
            controls: Some(vec![NativeControl {
                id: "thought_level".into(),
                display_name: Some("Thought level".into()),
                value_names: [("max".to_owned(), "Maximum".to_owned())]
                    .into_iter()
                    .collect(),
                values: NativeControlValues::Choices {
                    options: vec!["none".into(), "high".into(), "max".into()],
                },
                default: Some(NativeControlValue::Choice("high".into())),
            }]),
        }],
        default_model: default_model.map(|id| id.to_owned()),
    }
}

/// A configuration holding a stored reading for its first provider, the way the application
/// holds one after a scan: the snapshot is bound to the provider it was read from by the same
/// fingerprint the scan writes, so the supported readers accept it.
fn config_with_reading(model: Option<&str>, catalog: ymp_core::ProviderCapabilities) -> Config {
    let mut config = Config::default();
    let provider = config.providers[0].id.clone();
    if let Some(agent) = config.agents.iter_mut().find(|a| a.provider == provider) {
        agent.model = model.map(|id| id.to_owned());
    }
    config.native_catalog.providers.insert(
        provider.clone(),
        ymp_core::NativeProviderSnapshot {
            provider_fingerprint: ymp_core::provider_fingerprint(
                config.provider(&provider).unwrap(),
            ),
            last_attempt: ymp_core::now(),
            failure: None,
            catalog: Some(catalog),
        },
    );
    config
}

fn app_with(fixture: &Fixture, config: Config) -> App {
    App::new(
        fixture.store.clone(),
        config,
        PathBuf::from(fixture.project.path()),
    )
}

#[test]
fn a_profile_with_no_native_reading_is_not_presented_as_a_named_agent() {
    // The shipped default: three provider-labelled profiles with no model of their own.
    let fixture = fixture();
    let mut app = fixture.app();
    app.command("/agents", 100);

    for profile in ["codex", "claude", "glm"] {
        let row = left_of_key(&mut app, 65, profile);
        assert!(
            row.contains("no native model"),
            "an unresolved profile does not say so: {row}"
        );
        let detail = detail_of_key(&mut app, 100, profile);
        assert!(
            detail.contains("no model is set and no stored catalog names a default"),
            "the page does not say why no name is known:\n{detail}"
        );
        assert!(
            detail.contains("nothing stored; this provider's own offerings have not been read"),
            "the reading behind the profile is not reported:\n{detail}"
        );
    }
    // The pool itself refuses them for the same reason, and the team page says which.
    app.command("/team", 100);
    let member = left_of_key(&mut app, 100, "codex");
    assert!(
        member.contains("unknown model") && !member.contains("Codex"),
        "a profile with no model is named after its provider: {member}"
    );
    let detail = detail_of_key(&mut app, 100, "codex");
    assert!(
        detail.contains("no native model is resolved for it"),
        "the pool's own reason is not shown:\n{detail}"
    );
}

#[test]
fn the_whole_window_names_a_member_by_what_the_installation_returned() {
    // The window outside the pages names members too: the right panel and the opening summary.
    // A provider label or an installation's caption standing in for an actor there would be the
    // same claim the pages refuse.
    let fixture = fixture();
    let config = config_with_reading(
        Some("gpt-5.6-sol"),
        scanned_catalog(Some("gpt-5.6-sol"), &ymp_core::now()),
    );
    let mut app = app_with(&fixture, config);
    let rendered = draw(&mut app, 100, 30);
    assert!(
        rendered.contains("gpt-5.6-sol-0913") && !rendered.contains("GPT-5.6-Sol"),
        "the window does not name the member by the model the installation resolved:\n{rendered}"
    );
    let team_line = rendered
        .lines()
        .find(|line| line.contains("team "))
        .unwrap_or_default()
        .to_owned();
    assert!(
        team_line.contains("gpt-5.6-sol-0913") && !team_line.contains("Codex"),
        "the opening summary still presents the provider label as the actor: {team_line}"
    );
    let panel = rendered
        .lines()
        .skip_while(|line| !line.contains("TEAM"))
        .take(4)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        panel.contains("gpt-5.6-sol-0913"),
        "the right panel still presents the provider label as the actor:\n{panel}"
    );
}

#[tokio::test]
async fn a_finished_session_is_named_by_the_identity_its_turns_captured() {
    // The identity a turn captured and the profile's name now are deliberately different here,
    // which is what a session looks like after its actor was reconfigured or re-read. The whole
    // window must name that session's actor as its turns ran, everywhere it names it at all.
    let run = mock_run("Create a greeting", |_| {}).await;
    let trace = run.store.trace(&run.session).unwrap();
    let previous = trace.assignments.last().unwrap().clone();
    let agent = previous.agent_id.clone();
    let configured = mock_config().agent(&agent).unwrap().name.clone();
    let assignment = ymp_core::AssignmentRecord {
        id: ymp_core::new_id(),
        grant_ids: Vec::new(),
        agent_identity: Some(ymp_core::AgentIdentity {
            name: "GPT-5.6-Terra".into(),
            configured_name: configured.clone(),
            model: Some("gpt-5.6-terra".into()),
            effort: None,
            resolved_model: None,
            source: None,
            status: ymp_core::AgentIdentityStatus::Native,
        }),
        state: ymp_core::InvocationState::Running,
        started_at: ymp_core::now(),
        ended_at: None,
        ..previous.clone()
    };
    let turn = ymp_core::InvocationRecord {
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
    run.store
        .begin_invocation_with_grants(&assignment, &turn, &[])
        .unwrap();
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    assert_ne!(
        configured, "GPT-5.6-Terra",
        "the fixture must actually differ from the configured name"
    );
    assert_eq!(
        app.config.agent(&agent).unwrap().name,
        configured,
        "the configuration must still carry the old name for this to be a regression"
    );

    // The window at large: the right panel lists this session's members.
    let rendered = draw(&mut app, 100, 30);
    let panel = rendered
        .lines()
        .skip_while(|line| !line.contains("TEAM"))
        .take(4)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        panel.contains("gpt-5.6-terra") && !panel.contains("GPT-5.6-Terra"),
        "the window does not name a finished session's actor as its turns captured it:\n{panel}"
    );

    // And every page that names the same actor agrees with the record.
    app.command("/assignments", 100);
    let row = left_of_key(&mut app, 100, &text::short_id(&assignment.id));
    assert!(
        row.contains("gpt-5.6-terra") && !row.contains("GPT-5.6-Terra"),
        "the assignment row does not name the identity it captured: {row}"
    );
    app.command("/team", 100);
    let member = left_of_key(&mut app, 100, &agent);
    assert!(
        member.contains("gpt-5.6-terra") && !member.contains("GPT-5.6-Terra"),
        "the team page does not name the member as its turns captured it: {member}"
    );
}

#[tokio::test]
async fn turns_recorded_without_a_captured_identity_are_not_renamed_from_the_present_catalog() {
    // The negative control for the rule above. This turn was recorded without an identity,
    // which is what every session written before the catalog existed looks like. Naming that
    // actor from the catalog as it stands now would rename finished work, and nothing recorded
    // says which model the turn ran, so its row says that instead.
    let run = mock_run("Create a greeting", |_| {}).await;
    let trace = run.store.trace(&run.session).unwrap();
    let previous = trace.assignments.last().unwrap().clone();
    let assignment = ymp_core::AssignmentRecord {
        id: ymp_core::new_id(),
        grant_ids: Vec::new(),
        agent_identity: None,
        state: ymp_core::InvocationState::Running,
        started_at: ymp_core::now(),
        ended_at: None,
        ..previous.clone()
    };
    let turn = ymp_core::InvocationRecord {
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
    run.store
        .begin_invocation_with_grants(&assignment, &turn, &[])
        .unwrap();
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    let profile = app.config.agent(&assignment.agent_id).unwrap().clone();
    let configured = profile.name.clone();
    // What the pool would hold if this installation were read after the session ended.
    app.pool = crate::provenance::Pool {
        read_at: ymp_core::now(),
        pool: Some(ymp_core::AgentPool {
            agents: vec![ymp_core::PoolAgent {
                identity: ymp_core::AgentIdentity {
                    name: "GPT-6-Astra".into(),
                    configured_name: configured.clone(),
                    model: Some("gpt-6-astra".into()),
                    effort: None,
                    resolved_model: None,
                    source: None,
                    status: ymp_core::AgentIdentityStatus::Native,
                },
                profile,
                profile_version: String::new(),
                exclusions: Vec::new(),
                model_status: ymp_core::PoolModelStatus::Listed,
            }],
            capabilities: Default::default(),
        }),
        health: Vec::new(),
        unreadable: None,
    };
    app.command("/assignments", 100);

    let row = left_of_key(&mut app, 100, &text::short_id(&assignment.id));
    assert!(
        row.contains("unknown model"),
        "a turn that captured no identity was given a model nothing recorded: {row}"
    );
    assert!(
        !row.contains("GPT-6-Astra"),
        "the catalog as it stands now renamed a finished turn: {row}"
    );
    // The same actor in the window at large, which must not disagree with the record.
    let rendered = draw(&mut app, 100, 30);
    assert!(
        !rendered.contains("GPT-6-Astra"),
        "the window renamed a finished session's actor from the present catalog:\n{rendered}"
    );
    // Its provider is a local fixture as configured now, which says nothing about a turn that
    // captured no identity: the member is not given the fixture's configured name either.
    app.command("/team", 100);
    let member = left_of_key(&mut app, 100, &assignment.agent_id);
    assert!(
        member.contains(crate::label::UNKNOWN_MODEL),
        "the provider as configured now named a turn that captured no identity: {member}"
    );
}

/// A later turn of `agent` that captured a native identity and reported its model and effort:
/// evidence about that turn, which a record written before it must not borrow.
fn append_known_native_turn(run: &Run, agent: &str) {
    let trace = run.store.trace(&run.session).unwrap();
    let previous = trace.assignments.last().unwrap().clone();
    let assignment = ymp_core::AssignmentRecord {
        id: ymp_core::new_id(),
        agent_id: agent.to_owned(),
        grant_ids: Vec::new(),
        agent_identity: Some(ymp_core::AgentIdentity {
            name: "GPT-5.6-Terra".into(),
            configured_name: agent.to_owned(),
            model: Some("gpt-5.6-terra".into()),
            effort: None,
            resolved_model: None,
            source: None,
            status: ymp_core::AgentIdentityStatus::Native,
        }),
        state: ymp_core::InvocationState::Running,
        started_at: ymp_core::now(),
        ended_at: None,
        ..previous
    };
    let reported = ymp_core::ExecutionSettings {
        model: Some("gpt-5.6-terra".into()),
        effort: Some("high".into()),
        permission_mode: None,
    };
    let turn = ymp_core::InvocationRecord {
        id: ymp_core::new_id(),
        session_id: run.session.clone(),
        assignment_id: assignment.id.clone(),
        execution_backend: None,
        turn: trace.invocations.len() as u64 + 1,
        requested: assignment.requested.clone(),
        sent: reported.clone(),
        reported,
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
    run.store
        .begin_invocation_with_grants(&assignment, &turn, &[])
        .unwrap();
}

/// A decision with an actor, from the run's own records, and whether it links that actor's turn.
fn actor_decision(run: &Run, linked: bool) -> (ymp_core::DecisionRecord, String) {
    let trace = run.store.trace(&run.session).unwrap();
    trace
        .decisions
        .iter()
        .find_map(|decision| {
            let actor = decision.actor.clone()?;
            let own = decision.links.assignment_id.as_deref().is_some_and(|id| {
                trace
                    .assignments
                    .iter()
                    .any(|assignment| assignment.id == id && assignment.agent_id == actor)
            });
            (own == linked).then(|| (decision.clone(), actor))
        })
        .expect("the run recorded a decision of the kind this test reads")
}

#[tokio::test]
async fn a_decision_is_named_by_the_turn_it_links_and_not_by_a_later_turn_of_its_actor() {
    let run = mock_run("Create a greeting", |_| {}).await;
    let (decision, actor) = actor_decision(&run, true);
    append_known_native_turn(&run, &actor);
    let mut app = run.app();
    app.load_session(&run.session).unwrap();

    // The fixture is effective: where the actor as it stands is meant, the later turn names it.
    app.command("/team", 100);
    assert!(
        left_of_key(&mut app, 100, &actor).contains("gpt-5.6-terra"),
        "the later native turn did not reach the roster, so this test proves nothing"
    );

    app.command("/decisions", 100);
    let key = text::short_id(&decision.id);
    let row = left_of_key(&mut app, 100, &key);
    let detail = detail_of_key(&mut app, 100, &key);
    assert!(
        !row.contains("gpt-5.6-terra") && !detail.contains("gpt-5.6-terra"),
        "a later turn renamed the actor of an earlier decision:\n{row}\n{detail}"
    );
    assert!(
        row.ends_with(&format!(" · {actor}")),
        "the decision is not named by the fixture turn it links: {row}"
    );
}

#[tokio::test]
async fn an_old_unlinked_record_stays_unknown_after_a_known_native_turn_of_its_actor() {
    // A record written before decisions linked the turn behind them, followed by a turn whose
    // model is known. Only that later turn names a model, and it is not this record's turn.
    let run = mock_run("Create a greeting", |_| {}).await;
    let (decision, actor) = actor_decision(&run, true);
    append_known_native_turn(&run, &actor);
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/decisions", 100);
    let trace = app.records.trace.as_mut().expect("the records were read");
    for recorded in &mut trace.decisions {
        if recorded.id == decision.id {
            recorded.links.assignment_id = None;
            recorded.links.invocation_id = None;
        }
    }

    let key = text::short_id(&decision.id);
    let row = left_of_key(&mut app, 100, &key);
    let detail = detail_of_key(&mut app, 100, &key);
    assert!(
        !row.contains("gpt-5.6-terra") && !detail.contains("gpt-5.6-terra"),
        "an unlinked record borrowed the model of a later turn:\n{row}\n{detail}"
    );
    assert!(
        row.ends_with(&format!(" · {}", crate::label::UNKNOWN_MODEL))
            && detail.contains(&format!("{} · {actor}", crate::label::UNKNOWN_MODEL)),
        "an unlinked record does not say its model is unknown:\n{row}\n{detail}"
    );
}

#[test]
fn a_record_binds_only_to_the_exact_invocation_it_names() {
    let fixture = fixture();
    let config = native_label_config();
    let session = native_label_session(&fixture, &config);
    let first = native_turn(
        &fixture,
        &session,
        "transport-one",
        captured("Latest release", "glm-5.2", None),
        (Some("glm-5.2"), Some("low")),
    );
    let mut app = app_with(&fixture, config);
    app.load_session(&session).unwrap();
    app.command("/decisions", 100);
    let trace = app.records.trace.as_mut().expect("the records were read");
    let recorded = trace
        .invocations
        .iter()
        .find(|turn| turn.id == first)
        .expect("the fixture recorded its turn")
        .clone();
    let assignment = recorded.assignment_id.clone();
    // A later call of the same assignment that reported another effort.
    trace.invocations.push(ymp_core::InvocationRecord {
        id: "later-call".into(),
        reported: ymp_core::ExecutionSettings {
            effort: Some("max".into()),
            ..recorded.reported.clone()
        },
        ..recorded
    });
    let decision = |id: &str, invocation: Option<&str>| ymp_core::DecisionRecord {
        id: id.into(),
        session_id: session.clone(),
        kind: "review".into(),
        actor: Some("transport-one".into()),
        reason: "fixture".into(),
        outcome: None,
        links: ymp_core::RecordLinks {
            assignment_id: Some(assignment.clone()),
            invocation_id: invocation.map(str::to_owned),
            ..Default::default()
        },
        created_at: ymp_core::now(),
    };
    trace.decisions.push(decision("exact-call", Some(&first)));
    trace
        .decisions
        .push(decision("missing-call", Some("not-recorded")));
    trace.decisions.push(decision("no-call-named", None));

    let row = |app: &mut App, id: &str| left_of_key(app, 100, &text::short_id(id));
    let exact = row(&mut app, "exact-call");
    assert!(
        exact.ends_with(" · glm-5.2 low"),
        "a record is not named by the invocation it names exactly: {exact}"
    );
    // An exact reference that names no recorded call does not fall back to another call.
    let missing = row(&mut app, "missing-call");
    assert!(
        missing.ends_with(&format!(" · {}", crate::label::UNKNOWN_MODEL)),
        "an invalid invocation reference was bound to another call: {missing}"
    );
    // The assignment made two calls and the record names neither: the label rests on what the
    // assignment captured, with no effort borrowed from either call.
    let ambiguous = row(&mut app, "no-call-named");
    assert!(
        ambiguous.ends_with(" · glm-5.2"),
        "an assignment-only reference chose one of several calls: {ambiguous}"
    );
}

#[test]
fn a_saved_session_names_its_team_by_its_records_and_not_by_the_profiles_it_captured() {
    // A captured profile carries a configured caption and an alias. Neither is a model a turn
    // recorded, and what kind its provider is configured as now says nothing about the session.
    let fixture = fixture();
    let team = ["codex", "claude"]
        .into_iter()
        .map(|id| AgentProfile {
            id: id.into(),
            name: "Captured Caption".into(),
            provider: id.into(),
            model: Some("opus".into()),
            instructions: String::new(),
            enabled: true,
        })
        .collect::<Vec<_>>();
    let session = fixture.seed_with_team("A finished session", team.clone());

    let mut app = fixture.app();
    app.command("/sessions", 100);
    let unread = detail_of_key(&mut app, 100, &session);
    assert!(
        !unread.contains("Captured Caption") && !unread.contains("opus"),
        "a session whose records were not read was named from its captured profiles:\n{unread}"
    );
    assert!(
        unread.contains("read from this session's own records"),
        "the page does not say where the team's models come from:\n{unread}"
    );

    app.load_session(&session).unwrap();
    app.command("/sessions", 100);
    let loaded = detail_of_key(&mut app, 100, &session);
    assert!(
        !loaded.contains("Captured Caption") && !loaded.contains("opus"),
        "the loaded session was named from its captured profiles:\n{loaded}"
    );
    assert!(
        team.iter().all(|profile| loaded.contains(&profile.id))
            && loaded.contains("no turn of this agent was recorded"),
        "the loaded session does not list its members:\n{loaded}"
    );
}

#[test]
fn a_scanned_model_is_shown_exactly_as_the_installation_resolved_it() {
    let fixture = fixture();
    let config = config_with_reading(
        Some("gpt-5.6-sol"),
        scanned_catalog(Some("gpt-5.6-sol"), &ymp_core::now()),
    );
    let mut app = app_with(&fixture, config);
    app.command("/agents", 100);

    let row = left_of_key(&mut app, 65, "codex");
    assert!(
        row.starts_with("gpt-5.6-sol-0913") && !row.contains("GPT-5.6-Sol"),
        "the model the installation resolved is not the label: {row}"
    );
    assert!(
        row.contains("gpt-5.6-sol") && row.contains("codex"),
        "the identifier and the provider are not kept beside it: {row}"
    );
    let detail = detail_of_key(&mut app, 100, "codex");
    for expected in [
        "native label",
        "GPT-5.6-Sol",
        "configured as",
        "Codex",
        "resolved to",
        "gpt-5.6-sol-0913",
        "the installation, read by models.list",
        "also known as",
        "gpt-5.6",
        "a selector, not a model to send",
        "Thought level",
        "Maximum (max)",
        "high by default",
    ] {
        assert!(
            detail.contains(expected),
            "the record does not carry {expected:?}:\n{detail}"
        );
    }
    assert!(
        !detail.contains("sol-row ·") || detail.contains("a selector"),
        "the picker identity is offered as a model to send:\n{detail}"
    );
}

#[test]
fn an_actor_whose_model_comes_from_its_execution_policy_is_still_named_natively() {
    // A legacy actor: nothing on the profile itself, a concrete model in its execution policy.
    // The migration deliberately leaves that profile field empty so the actor keeps its
    // qualified experience, so an empty field cannot be read as an unresolved agent.
    let fixture = fixture();
    let mut config =
        config_with_reading(None, scanned_catalog(Some("gpt-5.6-sol"), &ymp_core::now()));
    config.execution.insert(
        "codex".into(),
        ymp_core::AgentExecutionPolicy {
            defaults: ymp_core::ModelEffort {
                model: Some("gpt-5.6-sol".into()),
                effort: None,
            },
            ..Default::default()
        },
    );
    assert!(
        config.agent("codex").unwrap().model.is_none(),
        "the fixture must leave the profile's own field empty"
    );
    let mut app = app_with(&fixture, config);
    app.command("/agents", 100);

    let row = left_of_key(&mut app, 65, "codex");
    assert!(
        row.starts_with("gpt-5.6-sol-0913"),
        "an actor whose model comes from its policy is not named natively: {row}"
    );
    assert!(
        !row.contains("no native model"),
        "an empty profile field was read as an unresolved agent: {row}"
    );
    let detail = detail_of_key(&mut app, 100, "codex");
    assert!(
        detail.contains("gpt-5.6-sol") && detail.contains("the installation, read by models.list"),
        "the record does not name the model and where its name came from:\n{detail}"
    );
}

#[test]
fn a_reading_that_is_no_longer_current_says_so_rather_than_reading_as_native() {
    let fixture = fixture();
    // A reading from before this work started, which is more than a day old however long this
    // test lives.
    let config = config_with_reading(
        Some("gpt-5.6-sol"),
        scanned_catalog(Some("gpt-5.6-sol"), "2026-09-01T00:00:00+00:00"),
    );
    let mut app = app_with(&fixture, config);
    app.command("/agents", 100);

    let row = left_of_key(&mut app, 65, "codex");
    assert!(
        row.starts_with("gpt-5.6-sol-0913") && row.contains("not read recently"),
        "a reading two days old is presented as current: {row}"
    );
    assert!(
        detail_of_key(&mut app, 100, "codex").contains("which is no longer current"),
        "the record does not say the reading is stale"
    );
}

#[test]
fn a_model_no_reading_lists_is_unknown_and_never_native() {
    let fixture = fixture();
    let config = config_with_reading(
        Some("gpt-9"),
        scanned_catalog(Some("gpt-5.6-sol"), &ymp_core::now()),
    );
    let mut app = app_with(&fixture, config);
    app.command("/agents", 100);

    let row = left_of_key(&mut app, 65, "codex");
    assert!(
        row.starts_with("gpt-9") && row.contains("not in the catalog"),
        "a model nothing read is presented as a native name: {row}"
    );
    let detail = detail_of_key(&mut app, 100, "codex");
    assert!(
        detail.contains("no stored catalog lists this model"),
        "the record does not say what is missing:\n{detail}"
    );
    assert!(
        !detail.contains("GPT-5.6-Sol"),
        "another offering's name was borrowed:\n{detail}"
    );
}

#[test]
fn a_failed_attempt_keeps_the_previous_reading_and_says_it_failed() {
    let fixture = fixture();
    let mut config = config_with_reading(
        Some("gpt-5.6-sol"),
        scanned_catalog(Some("gpt-5.6-sol"), &ymp_core::now()),
    );
    let provider = config.providers[0].id.clone();
    // A bounded failure code, which is what the scan records: never native diagnostic text.
    config
        .native_catalog
        .providers
        .get_mut(&provider)
        .unwrap()
        .failure = Some("timeout".into());
    let mut app = app_with(&fixture, config);
    app.command("/providers", 100);

    let detail = detail_of_key(&mut app, 65, &provider);
    assert!(
        detail.contains("ended as timeout") && detail.contains("is kept"),
        "a failed attempt is not distinguished from a current reading:\n{detail}"
    );
    app.command("/agents", 100);
    assert!(
        left_of_key(&mut app, 65, "codex").contains("not read recently"),
        "a retained reading after a failure is presented as current"
    );
}

#[tokio::test]
async fn a_usage_row_names_the_model_its_turns_actually_ran_with() {
    let run = mock_run("Create a greeting", |_| {}).await;
    let trace = run.store.trace(&run.session).unwrap();
    let previous = trace.assignments.last().unwrap().clone();
    let mut assignment = ymp_core::AssignmentRecord {
        id: ymp_core::new_id(),
        grant_ids: Vec::new(),
        state: ymp_core::InvocationState::Running,
        started_at: ymp_core::now(),
        ended_at: None,
        ..previous.clone()
    };
    // A turn that ran under an identifier the configuration does not name now.
    let turn = ymp_core::InvocationRecord {
        id: ymp_core::new_id(),
        session_id: run.session.clone(),
        assignment_id: assignment.id.clone(),
        execution_backend: None,
        turn: trace.invocations.len() as u64 + 1,
        requested: assignment.requested.clone(),
        sent: ymp_core::ExecutionSettings {
            model: Some("glm-5.2".into()),
            ..Default::default()
        },
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
        &turn,
        ymp_core::TeamOperation::coordination(),
    )];
    assignment.grant_ids = grants.iter().map(|grant| grant.id.clone()).collect();
    run.store
        .begin_invocation_with_grants(&assignment, &turn, &grants)
        .unwrap();

    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/usage", 100);
    let agent = row_prose(&mut app, 100, &previous.agent_id);
    assert!(
        agent.contains("ran as") && agent.contains("glm-5.2"),
        "the page does not say what this agent's turns actually ran as:\n{agent}"
    );
    assert!(
        agent.contains("named no model"),
        "turns whose model nothing recorded are not counted as such:\n{agent}"
    );
}

#[test]
fn re_reading_the_catalog_is_an_action_that_asks_no_provider_anything() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.command("/providers", 100);
    let before = app.pool.read_at.clone();
    let agents = app.config.agents.clone();

    app.on_key(key(KeyCode::Char('r')), 100);

    let notice = app.notices.last().expect("the action reports what it did");
    assert!(!notice.failure, "re-reading was reported as a failure");
    assert!(
        notice.text.contains("asks no provider anything")
            && notice
                .text
                .contains("reading a provider's own offerings is R"),
        "the action claims more than it did, or does not point at the one that reads: {}",
        notice.text
    );
    assert!(
        app.pool.read_at >= before,
        "the action did not re-read what is installed"
    );
    assert_eq!(
        app.config.agents, agents,
        "re-reading changed the configuration"
    );
    let hints = app
        .page(65)
        .hints
        .iter()
        .map(|(key, _)| *key)
        .collect::<Vec<_>>();
    assert!(
        hints.contains(&"r") && hints.contains(&"R"),
        "the two actions are not both offered on the page: {hints:?}"
    );

    // The reading itself is an action the window performs, not something painting does.
    let actions = app.on_key(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT), 100);
    assert!(
        matches!(
            actions.as_slice(),
            [crate::state::Action::RefreshCatalog { provider: Some(id) }] if id == "codex"
        ),
        "R on a provider row does not ask that installation: {actions:?}"
    );
}

#[tokio::test]
async fn a_captured_member_is_not_relabelled_by_the_catalog_as_it_stands_now() {
    let run = mock_run("Create a greeting", |_| {}).await;
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/team", 100);
    // What the pool would hold if this agent's installation were read after the session ended.
    // It says nothing about a turn that already ran, and a member of a finished session must
    // not be relabelled by it.
    let agent_id = run.store.trace(&run.session).unwrap().assignments[0]
        .agent_id
        .clone();
    let profile = app.config.agent(&agent_id).unwrap().clone();
    app.pool = crate::provenance::Pool {
        read_at: ymp_core::now(),
        pool: Some(ymp_core::AgentPool {
            agents: vec![ymp_core::PoolAgent {
                identity: ymp_core::AgentIdentity {
                    name: "GPT-6-Astra".into(),
                    configured_name: profile.name.clone(),
                    model: Some("gpt-6-astra".into()),
                    effort: None,
                    resolved_model: None,
                    source: None,
                    status: ymp_core::AgentIdentityStatus::Native,
                },
                profile,
                profile_version: String::new(),
                exclusions: Vec::new(),
                model_status: ymp_core::PoolModelStatus::Listed,
            }],
            capabilities: Default::default(),
        }),
        health: Vec::new(),
        unreadable: None,
    };

    let agent = agent_id;
    let row = left_of_key(&mut app, 65, &agent);
    assert!(
        !row.contains("GPT-6-Astra"),
        "a finished session's member was relabelled by a later catalog: {row}"
    );
    assert!(
        row.contains("model not recorded"),
        "the row does not say that the session recorded no model for it: {row}"
    );
    let detail = detail_of_key(&mut app, 65, &agent);
    assert!(
        detail.contains("read from the turns this session recorded")
            && detail.contains("not from the catalog as it stands now"),
        "the page does not say where its answer came from:\n{detail}"
    );
    assert!(
        !detail.contains("GPT-6-Astra"),
        "the later catalog reached a captured member's record:\n{detail}"
    );
}

#[tokio::test]
async fn a_captured_member_shows_what_its_turns_ran_with_before_what_its_profile_said() {
    let run = mock_run("Create a greeting", |_| {}).await;
    let trace = run.store.trace(&run.session).unwrap();
    let agent = trace.assignments[0].agent_id.clone();
    let previous = trace.assignments.last().unwrap().clone();
    let mut assignment = ymp_core::AssignmentRecord {
        id: ymp_core::new_id(),
        grant_ids: Vec::new(),
        agent_id: agent.clone(),
        state: ymp_core::InvocationState::Running,
        started_at: ymp_core::now(),
        ended_at: None,
        ..previous
    };
    let turn = ymp_core::InvocationRecord {
        id: ymp_core::new_id(),
        session_id: run.session.clone(),
        assignment_id: assignment.id.clone(),
        execution_backend: None,
        turn: trace.invocations.len() as u64 + 1,
        requested: assignment.requested.clone(),
        sent: ymp_core::ExecutionSettings {
            model: Some("gpt-5.6-sol".into()),
            ..Default::default()
        },
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
        &turn,
        ymp_core::TeamOperation::coordination(),
    )];
    assignment.grant_ids = grants.iter().map(|grant| grant.id.clone()).collect();
    run.store
        .begin_invocation_with_grants(&assignment, &turn, &grants)
        .unwrap();

    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    app.command("/team", 100);
    // The profile this session captured names a model its turns did not use, which is what a
    // profile edited after the run, or a policy that supplied another, leaves behind.
    let mut trace = run.store.trace(&run.session).unwrap();
    for profile in trace.session.team.iter_mut() {
        if profile.id == agent {
            profile.model = Some("a-model-the-profile-named".into());
        }
    }
    app.records = crate::provenance::Records {
        session: Some(run.session.clone()),
        read_at: ymp_core::now(),
        trace: Some(trace),
        ..Default::default()
    };
    app.session_team = app
        .records
        .trace
        .as_ref()
        .map(|trace| trace.session.team.clone())
        .unwrap_or_default();

    let row = left_of_key(&mut app, 64, &agent);
    assert!(
        row.contains("gpt-5.6-sol"),
        "the row does not show what the turns actually ran with: {row}"
    );
    assert!(
        !row.contains("a-model-the-profile-named"),
        "a profile field outranked the recorded turns: {row}"
    );
}

#[test]
fn the_status_line_keeps_what_the_window_is_doing_at_every_supported_size() {
    let fixture = fixture();
    let mut app = fixture.app();
    app.command("/providers", 100);
    app.status = "Asking glm what it offers".into();
    for (width, height) in SUPPORTED_SIZES.iter().copied() {
        let rows = screen_rows(&mut app, width, height);
        let line = rows.last().expect("a status line").clone();
        assert!(
            line.contains("Asking glm what it offers"),
            "the hints took the status away at {width}x{height}:\n{line}"
        );
    }
}

#[test]
fn asking_a_disabled_installation_is_refused_with_its_reason() {
    let fixture = fixture();
    let mut config = Config::default();
    let disabled = config.providers[2].id.clone();
    config.providers[2].enabled = false;
    let mut app = App::new(
        fixture.store.clone(),
        config,
        PathBuf::from(fixture.project.path()),
    );
    app.command("/providers", 100);
    app.page_selected = app
        .page(100)
        .items
        .iter()
        .position(|item| item.key == disabled)
        .expect("the disabled provider has a row");

    let actions = app.on_key(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::SHIFT), 100);
    assert!(
        actions.is_empty(),
        "a disabled installation was asked anyway: {actions:?}"
    );
    let notice = app.notices.last().expect("the refusal is reported");
    assert!(
        notice.failure
            && notice
                .text
                .contains("is disabled, so nothing would be asked"),
        "the refusal does not say why: {}",
        notice.text
    );
}

#[tokio::test]
async fn no_row_puts_more_in_its_right_column_than_the_narrowest_column_holds() {
    // The right-hand side of a row is never truncated: a row that cannot fit it wraps, which
    // breaks the list. So every state word has to fit the narrowest supported column with room
    // left for the row's own name.
    let room = crate::frame::page_content_width(crate::frame::main_width(
        80,
        crate::frame::sidebar_width(80),
    )) as usize;
    let budget = room - 12;
    let run = mock_run("Create a greeting", |_| {}).await;
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    let mut checked = 0;
    for view in PAGES.iter().copied() {
        // Help is the one page whose right side is prose rather than a state word: it sizes a
        // description to the column it was built for and truncates it itself.
        if view == crate::views::View::Help {
            continue;
        }
        app.set_view(view);
        for item in app.page(80).items.iter() {
            let right = item
                .right
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>();
            assert!(
                text::width(&right) <= budget,
                "{:?} has a row whose right side needs {} of {budget} cells: {right:?}",
                view,
                text::width(&right)
            );
            checked += 1;
        }
    }
    // And again with the shipped configuration and no session, which is where the pool states
    // its own reasons for refusing a profile.
    let mut fresh = App::new(
        run.store.clone(),
        Config::default(),
        PathBuf::from(run.project.path()),
    );
    let mut refusals = 0;
    for view in PAGES.iter().copied() {
        if view == crate::views::View::Help {
            continue;
        }
        fresh.set_view(view);
        for item in fresh.page(80).items.iter() {
            let right = item
                .right
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>();
            assert!(
                text::width(&right) <= budget,
                "{:?} has a row whose right side needs {} of {budget} cells: {right:?}",
                view,
                text::width(&right)
            );
            if right.contains("no native model") {
                refusals += 1;
            }
            checked += 1;
        }
    }
    assert!(
        refusals > 0,
        "no row carried the pool's own refusal, so the narrowest case went unchecked"
    );
    assert!(checked > 40, "only {checked} rows were checked");
}

// ---------------------------------------------------------------------------
// The shared plan, as the runtime itself writes it
// ---------------------------------------------------------------------------

/// A scripted backend that coordinates through the real team API while its turn runs.
///
/// Nothing here is asked of a model. The plan is a fixed list of independent notes, and the
/// proposals are the ones these tests mean to read back: one that the runtime commits, one
/// repeated against a version the commitment has already moved, and one more that commits so
/// that two tasks end up owed to the same agent. Every other purpose is left to the offline
/// fixture executor underneath.
struct Coordinating {
    proposed: std::sync::Mutex<Vec<serde_json::Value>>,
    asked_once: std::sync::atomic::AtomicBool,
    /// Stop the notes the commitments are for, so a reader can be shown a responsibility that
    /// is still owed rather than one a finished session has already discharged.
    stall: bool,
}

async fn team_call(
    request: &ymp_providers::TurnRequest,
    name: &str,
    arguments: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let endpoint = request
        .mcp
        .as_ref()
        .expect("the turn carries a team endpoint");
    let socket = endpoint.args.last().expect("the endpoint names its socket");
    let mut stream = tokio::net::UnixStream::connect(socket).await?;
    stream
        .write_all(
            format!(
                "{}\n",
                serde_json::json!({"token": endpoint.token, "request_id": new_id(), "name": name, "arguments": arguments})
            )
            .as_bytes(),
        )
        .await?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).await?;
    Ok(serde_json::from_str(&line)?)
}

const NOTES: [&str; 4] = ["Alpha note", "Beta note", "Gamma note", "Delta note"];

impl ymp_providers::ExecutionBackend for Coordinating {
    fn identity(&self) -> ymp_core::ExecutionBackendIdentity {
        ymp_core::ExecutionBackendIdentity {
            id: "test.board-views".into(),
            version: "1".into(),
        }
    }
    fn execute(
        &self,
        request: ymp_providers::TurnRequest,
        events: tokio::sync::mpsc::UnboundedSender<ymp_providers::ProviderEvent>,
    ) -> ymp_providers::ExecutionFuture<'_> {
        Box::pin(async move {
            assert_eq!(
                request.provider.kind,
                ymp_core::ProviderKind::Mock,
                "no native inference in an interface test"
            );
            let purpose = request.purpose.clone();
            // The prompt carries the whole plan, so the task this turn is for is the one named
            // under its own assignment heading and not merely one the prompt mentions.
            let marker = format!("Your current assignment ({purpose}):\n");
            let assignment = request
                .prompt
                .rsplit(&marker)
                .next()
                .unwrap_or("")
                .to_owned();
            // Only the turn that actually runs the first note asks, and only once: a bid or a
            // review for the same task names it too.
            let first = purpose == "execute"
                && assignment.contains(NOTES[0])
                && !self
                    .asked_once
                    .swap(true, std::sync::atomic::Ordering::SeqCst);
            if self.stall
                && purpose == "execute"
                && (assignment.contains(NOTES[2]) || assignment.contains(NOTES[3]))
            {
                anyhow::bail!("Scripted stop before the committed notes run");
            }
            if first {
                let read = team_call(&request, "board_read", serde_json::json!({})).await?;
                assert_eq!(read["ok"], true, "{read}");
                let board = &read["value"];
                let reference = |title: &str| {
                    let entry = board["tasks"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|entry| entry["task"]["title"] == title)
                        .unwrap_or_else(|| panic!("the plan has no task called {title}"));
                    serde_json::json!({"task_id": entry["task"]["id"], "version": entry["version"]})
                };
                for (target, rationale) in [
                    (
                        NOTES[2],
                        "Take the third note on, having produced the first",
                    ),
                    (
                        NOTES[2],
                        "Take the third note on again, against the version this turn read",
                    ),
                    (NOTES[3], "Take the fourth note on as well, to hold both"),
                ] {
                    let asked = team_call(
                        &request,
                        "task_propose",
                        serde_json::json!({
                            "plan_version": board["plan_version"],
                            "change": {"kind": "assign", "task": reference(target), "agent_id": "one", "settings": {"model": "small", "effort": "low"}},
                            "rationale": rationale,
                        }),
                    )
                    .await?;
                    self.proposed.lock().unwrap().push(asked);
                }
            }
            let mut result = ymp_providers::NativeExecutionBackend
                .execute(request, events)
                .await?;
            if purpose == "plan" {
                let mut plan: serde_json::Value = serde_json::from_str(&result.text)?;
                let checks = plan["tasks"][0]["checks"].clone();
                plan["tasks"] = serde_json::Value::Array(
                    NOTES
                        .iter()
                        .map(|title| {
                            serde_json::json!({
                                "title": title,
                                "description": "Write greeting.txt containing Hello from ymp",
                                "competence": "implementation",
                                "difficulty": "simple",
                                "dependencies": [],
                                "checks": checks,
                            })
                        })
                        .collect(),
                );
                result.text = plan.to_string();
            }
            Ok(result)
        })
    }
}

/// The configuration the coordination fixture runs under: two mock actors with fixed settings,
/// so the settings a proposal names are exactly the ones the runtime will validate.
fn board_config() -> Config {
    let mut config = mock_config();
    config.limits.turns = 120;
    config.limits.parallel = 2;
    for id in ["one", "two"] {
        config.execution.insert(
            id.into(),
            ymp_core::AgentExecutionPolicy {
                fixed: ymp_core::ModelEffort {
                    model: Some("small".into()),
                    effort: Some("low".into()),
                },
                ..Default::default()
            },
        );
    }
    config
}

/// One run of the real runtime whose agents coordinated through the team API.
async fn board_run(stall: bool) -> Run {
    let home = TempDir::new().unwrap();
    let project = TempDir::new().unwrap();
    let store = Store::open(home.path()).unwrap();
    let (tx, _events) = tokio::sync::mpsc::unbounded_channel();
    let backend = std::sync::Arc::new(Coordinating {
        proposed: std::sync::Mutex::new(Vec::new()),
        asked_once: std::sync::atomic::AtomicBool::new(false),
        stall,
    });
    let mut config = board_config();
    if stall {
        config.limits.attempts = 1;
    }
    let mut engine = ymp_runtime::Engine::new(
        store.clone(),
        config,
        tx,
        tokio_util::sync::CancellationToken::new(),
    )
    .unwrap()
    .with_execution_backend(backend.clone())
    .unwrap();
    engine.use_memory = false;
    let outcome = engine
        .run(project.path(), "Write the notes this plan names", None)
        .await
        .unwrap();
    assert_eq!(
        backend.proposed.lock().unwrap().len(),
        3,
        "the fixture did not ask for what these tests read back"
    );
    Run {
        home,
        project,
        store,
        config: engine.config.clone(),
        session: outcome.session.id,
        status: outcome.session.status,
    }
}

/// The run the board tests read: its plan, its proposals and the decisions on them.
struct Board {
    run: Run,
    committed: String,
    rejected: String,
    /// The task a commitment is still owed on when the run stopped.
    owed: String,
}

async fn board_views() -> Board {
    let run = board_run(true).await;
    let board = run.store.board(&run.session).unwrap();
    let decided = |accepted: bool| {
        run.store
            .trace(&run.session)
            .unwrap()
            .decisions
            .into_iter()
            .filter_map(|decision| decision.links.board)
            .find(|board| board.accepted == accepted)
            .map(|board| board.proposal.id.clone())
            .expect("the fixture recorded a decision of each kind")
    };
    let owed = board
        .tasks
        .iter()
        .find(|entry| entry.commitment.is_some())
        .expect("the fixture left a commitment owed")
        .task
        .id
        .clone();
    Board {
        committed: decided(true),
        rejected: decided(false),
        owed,
        run,
    }
}

#[tokio::test]
async fn a_plan_change_an_agent_asked_for_is_listed_with_its_outcome_and_reason() {
    let board = board_views().await;
    let mut app = board.run.app();
    app.load_session(&board.run.session).unwrap();
    app.command("/tasks", 100);

    let keys = keys_of(&mut app, 100);
    assert!(
        keys.contains(&board.committed) && keys.contains(&board.rejected),
        "the plan's own proposals are not on the page"
    );
    let row = right_of_key(&mut app, 100, &board.committed);
    assert!(
        row.contains("committed"),
        "a committed proposal does not say so: {row}"
    );
    let left = left_of_key(&mut app, 100, &board.committed);
    assert!(
        left.contains("give") && left.contains("note"),
        "the row does not say what the proposal asks for: {left}"
    );
    let detail = detail_of_key(&mut app, 100, &board.committed);
    for expected in [
        "asked by",
        "committed · the plan now carries it",
        "decided by",
        "ymp.ordered-board",
        "the runtime, not an agent",
        "resulting plan",
        "the version the plan took on",
        "responsibility",
        "small at low",
        "against plan",
        "membership",
        "Take the third note on",
    ] {
        assert!(
            detail.contains(expected),
            "the committed proposal's record does not carry {expected:?}:\n{detail}"
        );
    }

    let row = right_of_key(&mut app, 100, &board.rejected);
    assert!(
        row.contains("rejected"),
        "a rejected proposal does not say so: {row}"
    );
    let detail = detail_of_key(&mut app, 100, &board.rejected);
    assert!(
        detail.contains("rejected · the plan was not changed")
            && detail.contains("stale_task")
            && detail.contains("unchanged by this proposal"),
        "the rejected proposal does not carry the runtime's own reason:\n{detail}"
    );
    assert!(
        !detail.contains("responsibility"),
        "a rejected proposal was shown as creating a responsibility:\n{detail}"
    );
}

#[tokio::test]
async fn a_task_says_who_is_responsible_for_it_and_what_was_put_off() {
    let board = board_views().await;
    let mut app = board.run.app();
    app.load_session(&board.run.session).unwrap();
    app.command("/tasks", 100);

    let detail = detail_of_key(&mut app, 100, &board.owed);
    for expected in [
        "task version",
        "responsibility",
        "small at low",
        "against task version",
        "committed by",
        "put off",
        "commitment_busy",
        "the agent responsible for it was already working",
    ] {
        assert!(
            detail.contains(expected),
            "the task does not carry {expected:?}:\n{detail}"
        );
    }
    let right = right_of_key(&mut app, 100, &board.owed);
    assert!(
        right.contains("one"),
        "the row does not name who is responsible for the task: {right}"
    );

    // The same responsibility, from the member's side.
    app.command("/team", 100);
    let member = detail_of_key(&mut app, 100, "one");
    assert!(
        member.contains("responsible for")
            && member.contains("note")
            && member.contains("small at low"),
        "the team page does not say what this member is responsible for:\n{member}"
    );
    let other = detail_of_key(&mut app, 100, "two");
    assert!(
        other.contains("no task on the plan is committed to this member"),
        "a member with no commitment is not said to have none:\n{other}"
    );
}

#[tokio::test]
async fn the_decision_and_not_the_stored_status_says_what_became_of_a_proposal() {
    // The negative control for the board outcome. A proposal's stored status and its decision
    // are written in one transaction, so they agree; only the decision carries the reason, and
    // only the decision is the runtime's own statement. Here the status is put back to pending
    // while the rejection stands, and the page must still read the decision.
    let board = board_views().await;
    let mut app = board.run.app();
    app.load_session(&board.run.session).unwrap();
    app.command("/tasks", 100);
    let mut snapshot = app.records.board.take().expect("the plan was read");
    for proposal in &mut snapshot.proposals {
        proposal.status = ymp_core::BoardProposalStatus::Pending;
    }
    app.records.board = Some(snapshot);

    let row = right_of_key(&mut app, 100, &board.rejected);
    assert!(
        row.contains("rejected"),
        "the page read the stored status instead of the decision: {row}"
    );
    let row = right_of_key(&mut app, 100, &board.committed);
    assert!(
        row.contains("committed"),
        "the page read the stored status instead of the decision: {row}"
    );
    assert!(
        !app.page(100).subtitle.contains("waiting"),
        "decided proposals were counted as waiting: {}",
        app.page(100).subtitle
    );
}

#[tokio::test]
async fn a_proposal_no_decision_answered_is_not_shown_as_refused() {
    let board = board_views().await;
    let mut app = board.run.app();
    app.load_session(&board.run.session).unwrap();
    // What the page must show between a turn asking and the next work boundary: the decisions
    // this session read are dropped, and the proposals stand as asked.
    app.command("/tasks", 100);
    let mut snapshot = app.records.board.take().expect("the plan was read");
    for proposal in &mut snapshot.proposals {
        proposal.status = ymp_core::BoardProposalStatus::Pending;
    }
    app.records.board = Some(snapshot);
    if let Some(trace) = app.records.trace.as_mut() {
        trace
            .decisions
            .retain(|decision| decision.links.board.is_none());
    }

    let row = right_of_key(&mut app, 100, &board.rejected);
    assert!(
        row.contains("proposed") && !row.contains("rejected"),
        "a proposal nothing answered was shown as refused: {row}"
    );
    let detail = detail_of_key(&mut app, 100, &board.rejected);
    assert!(
        detail.contains("The runtime answers a proposal at a work boundary"),
        "the page does not say when a proposal is answered:\n{detail}"
    );
    assert!(
        app.page(100).subtitle.contains("3 proposals waiting"),
        "the page does not say how many proposals are still waiting: {}",
        app.page(100).subtitle
    );
}

#[tokio::test]
async fn a_plan_that_could_not_be_read_is_not_shown_as_a_plan_with_nothing_on_it() {
    let board = board_views().await;
    let mut app = board.run.app();
    app.load_session(&board.run.session).unwrap();
    app.command("/tasks", 100);
    app.records.board = None;
    app.records.board_unreadable = Some("board_unreadable: disk error".into());

    let right = right_of_key(&mut app, 100, "board-unreadable");
    assert!(
        right.contains("unavailable"),
        "a failed read of the plan is not reported: {right}"
    );
    let detail = detail_of_key(&mut app, 100, "board-unreadable");
    assert!(
        detail.contains("board_unreadable: disk error") && detail.contains("which proposals exist"),
        "the page does not say what a failed read of the plan costs:\n{detail}"
    );
    let task = detail_of_key(&mut app, 100, &board.owed);
    assert!(
        !task.contains("responsibility"),
        "a task claimed a responsibility with no plan read:\n{task}"
    );
}

#[tokio::test]
async fn a_proposal_is_named_by_the_turn_that_asked_and_never_by_a_later_one() {
    let board = board_views().await;
    let proposer = board
        .run
        .store
        .board(&board.run.session)
        .unwrap()
        .proposals
        .iter()
        .find(|proposal| proposal.id == board.committed)
        .expect("the committed proposal is on the plan")
        .agent_id
        .clone();
    append_known_native_turn(&board.run, &proposer);
    let mut app = board.run.app();
    app.load_session(&board.run.session).unwrap();
    app.command("/tasks", 100);

    let right = right_of_key(&mut app, 100, &board.committed);
    let detail = detail_of_key(&mut app, 100, &board.committed);
    assert!(
        !right.contains("gpt-5.6-terra") && !detail.contains("gpt-5.6-terra"),
        "a later turn renamed the agent that asked for a change:\n{right}\n{detail}"
    );
    assert!(
        right.ends_with(&proposer),
        "the proposal is not named by the fixture turn that asked: {right}"
    );

    // A proposal whose turn is not among the records read names no model, however well the
    // actor's later turn is known.
    let mut snapshot = app.records.board.take().expect("the plan was read");
    for proposal in &mut snapshot.proposals {
        if proposal.id == board.committed {
            proposal.assignment_id = "not-recorded".into();
            proposal.invocation_id = "not-recorded".into();
        }
    }
    app.records.board = Some(snapshot);
    let right = right_of_key(&mut app, 101, &board.committed);
    assert!(
        right.ends_with(crate::label::UNKNOWN_MODEL) && !right.contains("gpt-5.6-terra"),
        "a proposal without its turn borrowed a model: {right}"
    );
}

#[tokio::test]
async fn a_decision_that_changed_the_plan_reads_as_its_own_outcome() {
    let board = board_views().await;
    let mut app = board.run.app();
    app.load_session(&board.run.session).unwrap();
    app.command("/decisions", 100);

    let committed = app
        .records
        .decisions()
        .iter()
        .find(|decision| decision.kind == "board_committed")
        .map(|decision| decision.id.clone())
        .expect("the fixture recorded a committed plan change");
    let key = text::short_id(&committed);
    let detail = detail_of_key(&mut app, 100, &key);
    for expected in [
        "plan change committed",
        "the plan took the change on",
        "proposal",
        "asked by",
        "resulting",
        "responsibility",
    ] {
        assert!(
            detail.contains(expected),
            "the decision does not carry {expected:?}:\n{detail}"
        );
    }
    // The list row is what a reader sees first, and it must not say less than the record.
    let row = right_of_key(&mut app, 100, &key);
    assert!(
        row == "committed",
        "the row of a committed plan change contradicts the record it opens: {row:?}"
    );
    assert!(
        left_of_key(&mut app, 100, &key).starts_with(crate::theme::UNICODE.ok),
        "the row of a committed plan change is not marked as done: {}",
        left_of_key(&mut app, 100, &key)
    );
    let rejected = app
        .records
        .decisions()
        .iter()
        .find(|decision| decision.kind == "board_rejected")
        .map(|decision| decision.id.clone())
        .expect("the fixture recorded a rejected plan change");
    let key = text::short_id(&rejected);
    let detail = detail_of_key(&mut app, 100, &key);
    assert!(
        detail.contains("plan change rejected") && detail.contains("the plan was left unchanged"),
        "a rejected plan change does not read as one:\n{detail}"
    );
    let row = right_of_key(&mut app, 100, &key);
    assert!(
        row == "rejected",
        "the row of a rejected plan change contradicts the record it opens: {row:?}"
    );
    assert!(
        left_of_key(&mut app, 100, &key).starts_with(crate::theme::UNICODE.fail),
        "the row of a rejected plan change is not marked as refused: {}",
        left_of_key(&mut app, 100, &key)
    );
}

// ---------------------------------------------------------------------------
// A correction to what was retained
// ---------------------------------------------------------------------------

/// A scripted backend that records one observation from a declared input file.
///
/// The value it records is the one this fixture sets, and the file it writes is named after it,
/// so the correction this test reads back is a real change of claim with its own evidence. No
/// model is asked anything: every purpose below the rewriting is the offline fixture executor.
struct Observing {
    value: std::sync::atomic::AtomicUsize,
}

impl ymp_providers::ExecutionBackend for Observing {
    fn identity(&self) -> ymp_core::ExecutionBackendIdentity {
        ymp_core::ExecutionBackendIdentity {
            id: "test.knowledge-views".into(),
            version: "1".into(),
        }
    }
    fn execute(
        &self,
        request: ymp_providers::TurnRequest,
        events: tokio::sync::mpsc::UnboundedSender<ymp_providers::ProviderEvent>,
    ) -> ymp_providers::ExecutionFuture<'_> {
        Box::pin(async move {
            assert_eq!(
                request.provider.kind,
                ymp_core::ProviderKind::Mock,
                "no native inference in an interface test"
            );
            let purpose = request.purpose.clone();
            let directory = request.cwd.clone();
            let mut result = ymp_providers::NativeExecutionBackend
                .execute(request, events)
                .await?;
            if purpose == "plan" {
                let mut plan: serde_json::Value = serde_json::from_str(&result.text)?;
                plan["tasks"][0]["title"] = serde_json::json!("Record observation");
                plan["tasks"][0]["description"] = serde_json::json!(
                    "Record the O04 completion percentage for Hill in 2026-W36 from the declared input"
                );
                result.text = plan.to_string();
            }
            if purpose == "execute" {
                let value = self.value.load(std::sync::atomic::Ordering::SeqCst);
                std::fs::write(
                    directory.join(format!("claim-{value}.json")),
                    serde_json::to_vec(
                        &serde_json::json!({"row":"O04","site":"Hill","week":"2026-W36","value":value}),
                    )?,
                )?;
                result.text = format!("Recorded observation O04 at Hill: {value} percent");
            }
            Ok(result)
        })
    }
}

/// The conditions this project's knowledge is recorded and looked up against.
fn observation_scope() -> std::collections::BTreeMap<String, String> {
    [
        ("dataset", "observations"),
        ("site", "Hill"),
        ("week", "2026-W36"),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value.to_owned()))
    .collect()
}

struct Correction {
    home: TempDir,
    _temp: TempDir,
    directory: PathBuf,
    store: Store,
    config: Config,
    /// The entry the first run retained, and the one that replaced it.
    old: String,
    new: String,
}

impl Correction {
    fn app(&self) -> App {
        App::new(
            self.store.clone(),
            self.config.clone(),
            self.directory.clone(),
        )
    }
}

/// Two runs of the real runtime: one that records 95 percent, and one whose accepted correction
/// replaces it with 60 percent read from a different declared input.
async fn correction_run() -> Correction {
    let home = TempDir::new().unwrap();
    let temp = TempDir::new().unwrap();
    let directory = temp.path().join("project");
    std::fs::create_dir_all(directory.join("inputs")).unwrap();
    let directory = directory.canonicalize().unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_owned();
    for name in ["observations.csv", "observations-corrected.csv"] {
        std::fs::copy(
            root.join("ymp-evals/fixtures/universal/inputs").join(name),
            directory.join("inputs").join(name),
        )
        .unwrap();
    }
    let verifier = temp.path().join("check_observation.py");
    std::fs::write(&verifier, "import csv,json,sys\nfrom pathlib import Path\nroot=Path(sys.argv[1])\nrows=list(csv.DictReader((root/sys.argv[2]).open()))\nrow=next(r for r in rows if r['row_id']=='O04')\nassert row['site']=='Hill' and row['week']=='2026-W36'\nexpected={'row':'O04','site':'Hill','week':'2026-W36','value':100*int(row['completed'])/int(row['scheduled'])}\nassert json.loads((root/sys.argv[3]).read_text())==expected\n").unwrap();
    let store = Store::open(home.path()).unwrap();
    let mut config = mock_config();
    config.knowledge_scope = observation_scope();
    config.limits.attempts = 1;
    for id in ["one", "two"] {
        if let Some(agent) = config.agents.iter_mut().find(|agent| agent.id == id) {
            agent.instructions = "[mock:no-checks]".into();
        }
    }
    let backend = std::sync::Arc::new(Observing {
        value: std::sync::atomic::AtomicUsize::new(95),
    });
    let (tx, _events) = tokio::sync::mpsc::unbounded_channel();
    let mut engine = ymp_runtime::Engine::new(
        store.clone(),
        config.clone(),
        tx,
        tokio_util::sync::CancellationToken::new(),
    )
    .unwrap()
    .with_execution_backend(backend.clone())
    .unwrap();
    let contract = |value: usize, target: Option<&ymp_core::MemoryEntry>| {
        let input = if value == 95 {
            "inputs/observations.csv"
        } else {
            "inputs/observations-corrected.csv"
        };
        let artifact = format!("claim-{value}.json");
        ymp_core::AcceptanceContract {
            knowledge_correction: target.map(|old| ymp_core::KnowledgeCorrectionBinding {
                projection: ymp_core::KnowledgeProjection::ProjectOutcome,
                target: ymp_core::KnowledgeRef {
                    id: old.id.clone(),
                    version: ymp_core::content_digest(&serde_json::to_string(old).unwrap()),
                },
                applicability: observation_scope(),
                criterion_ids: vec!["observation-value".into()],
                source_replacement: Some(ymp_core::KnowledgeSourceReplacement {
                    previous_input: "inputs/observations.csv".into(),
                    replacement_input: input.into(),
                }),
            }),
            task_title: "Record observation".into(),
            criteria: vec![ymp_core::AcceptanceCriterion {
                id: "observation-value".into(),
                description: "The O04 claim for Hill in 2026-W36 matches the declared source"
                    .into(),
            }],
            artifacts: vec![artifact.clone().into()],
            inputs: vec![input.into()],
            checks: vec![ymp_core::TrustedCheck {
                id: "actual-observation".into(),
                criterion_ids: vec!["observation-value".into()],
                assertion: ymp_core::CheckAssertion::Command {
                    program: "/usr/bin/python3".into(),
                    args: vec![
                        verifier.to_string_lossy().into(),
                        "{workdir}".into(),
                        input.into(),
                        artifact,
                    ],
                    verifier_files: vec![verifier.clone()],
                },
            }],
        }
    };
    engine.config.acceptance_contracts = Some(vec![contract(95, None)]);
    let first = engine
        .run(&directory, "Record the observation", None)
        .await
        .unwrap();
    assert_eq!(first.session.status, "completed", "{}", first.summary);
    let project = store.project(&directory).unwrap().id;
    let entry_of = |session: &str| {
        store
            .memory_inventory(Some(&project))
            .unwrap()
            .into_iter()
            .find(|entry| entry.source_session == session && entry.kind == "outcome")
            .unwrap_or_else(|| panic!("the run retained no outcome of its own"))
    };
    let old = entry_of(&first.session.id);
    assert!(old.content.contains("95"), "{}", old.content);
    backend.value.store(60, std::sync::atomic::Ordering::SeqCst);
    engine.config.acceptance_contracts = Some(vec![contract(60, Some(&old))]);
    let second = engine
        .run(&directory, "Record the observation again", None)
        .await
        .unwrap();
    assert_eq!(second.session.status, "completed", "{}", second.summary);
    let new = entry_of(&second.session.id);
    assert!(new.content.contains("60"), "{}", new.content);
    Correction {
        home,
        _temp: temp,
        directory,
        store,
        config: engine.config.clone(),
        old: old.id,
        new: new.id,
    }
}

#[test]
fn a_short_id_names_the_record_and_not_the_kind_it_is() {
    // Retained entries carry their kind in the id. Two of them must not read as the same record.
    let old = "knowledge:90a7f01d692667e5f373357a967907677455261c6c22ea9b7dea00003ac430ff";
    let new = "knowledge:ef37f436789fe7909e46a0187260d9e1823b7cfb773887e48acabd41dd511fe6";
    assert_ne!(
        text::short_id(old),
        text::short_id(new),
        "two entries of the same kind read as one record"
    );
    // An id with no kind in it is unchanged.
    assert_eq!(
        text::short_id("7ea8f187-2f8d-4bf1-b0c1-49aa14151a73"),
        "7ea8f187"
    );
}

#[tokio::test]
async fn both_sides_of_a_correction_are_kept_and_each_says_which_it_is() {
    let run = correction_run().await;
    let mut app = run.app();
    app.command("/memory", 100);

    let keys = keys_of(&mut app, 100);
    assert!(
        keys.contains(&run.old) && keys.contains(&run.new),
        "a correction did not keep both sides on the page"
    );

    let right = right_of_key(&mut app, 100, &run.old);
    assert!(
        right.contains("superseded"),
        "the corrected entry does not say it was superseded: {right}"
    );
    let detail = detail_of_key(&mut app, 100, &run.old);
    for expected in [
        "superseded: an accepted correction replaced this entry",
        "replaced by",
        "Record observation",
        "this entry is the one that was corrected",
        "authorised by",
        "under trusted contract",
        "corrected by",
        "ymp.bound-correction",
        "Both are kept",
        "95",
    ] {
        assert!(
            detail.contains(expected),
            "the corrected entry does not carry {expected:?}:\n{detail}"
        );
    }
    assert!(
        !detail.contains("yes, as support under the default retrieval"),
        "a superseded entry is still offered to a run:\n{detail}"
    );

    let right = right_of_key(&mut app, 100, &run.new);
    assert!(
        right.contains("current"),
        "the replacement does not read as current: {right}"
    );
    let detail = detail_of_key(&mut app, 100, &run.new);
    for expected in [
        "current: the default retrieval of a run accepts this entry",
        "replaces",
        "this entry is the correction",
        "yes, as support under the default retrieval",
        "60",
    ] {
        assert!(
            detail.contains(expected),
            "the replacement does not carry {expected:?}:\n{detail}"
        );
    }

    // Each side keeps the evidence it was accepted on, and they are not the same acceptance. The
    // record names two: the one that authorised the correction, which both sides share, and the
    // entry's own source, which is the last one on the record and is its own.
    let acceptance = |app: &mut App, key: &str| {
        let detail = detail_of_key(app, 100, key);
        let (_, rest) = detail
            .split_once("source ")
            .expect("the record names where its entry came from");
        let (_, rest) = rest
            .split_once("acceptance ")
            .expect("the source names its acceptance");
        rest.split(' ').next().unwrap_or_default().to_owned()
    };
    let before = acceptance(&mut app, &run.old);
    let after = acceptance(&mut app, &run.new);
    assert!(
        !before.is_empty() && !after.is_empty() && before != after,
        "both sides of the correction point at the same evidence: {before} and {after}"
    );
}

/// The session a correction was applied in, and the record that applied it.
fn applied_correction(run: &Correction) -> (String, ymp_core::DecisionRecord) {
    run.store
        .sessions(None)
        .unwrap()
        .into_iter()
        .find_map(|session| {
            run.store
                .trace(&session.id)
                .unwrap()
                .decisions
                .into_iter()
                .find(|decision| decision.links.knowledge_correction.is_some())
                .map(|decision| (session.id, decision))
        })
        .expect("the second run recorded the correction it applied")
}

#[tokio::test]
async fn a_correction_reads_as_applied_in_its_row_and_in_its_record() {
    let run = correction_run().await;
    let (session, correction) = applied_correction(&run);
    let mut app = run.app();
    app.load_session(&session).unwrap();
    app.command("/decisions", 100);

    let key = text::short_id(&correction.id);
    let row = right_of_key(&mut app, 100, &key);
    assert!(
        row == "entry replaced",
        "the row of an applied correction contradicts the record it opens: {row:?}"
    );
    assert!(
        left_of_key(&mut app, 100, &key).starts_with(crate::theme::UNICODE.ok),
        "the row of an applied correction is marked as a decision without an outcome: {}",
        left_of_key(&mut app, 100, &key)
    );
    let detail = detail_of_key(&mut app, 100, &key);
    for expected in [
        "retained knowledge corrected".to_owned(),
        "outcome what was retained was replaced".to_owned(),
        format!("replaced retained entry {}", text::short_id(&run.old)),
        format!("replacement retained entry {}", text::short_id(&run.new)),
        "corrected by policy ymp.bound-correction".to_owned(),
    ] {
        assert!(
            detail.contains(&expected),
            "the correction's record does not carry {expected:?}:\n{detail}"
        );
    }
    assert!(
        !detail.contains("recorded without an outcome"),
        "an applied correction reads as ungraded when opened:\n{detail}"
    );
}

#[tokio::test]
async fn a_contract_names_the_inputs_it_declares_and_the_source_a_correction_replaced() {
    let run = correction_run().await;
    // Each run captured its own contract, and only the second one binds a correction.
    let mut contracts = Vec::new();
    for session in run.store.sessions(None).unwrap() {
        for decision in run.store.trace(&session.id).unwrap().decisions {
            if let Some(captured) = decision.links.acceptance_contract.as_ref() {
                let corrects = captured.contract.knowledge_correction.is_some();
                contracts.push((session.id.clone(), decision.id.clone(), corrects));
            }
        }
    }
    assert_eq!(
        contracts
            .iter()
            .filter(|(_, _, corrects)| *corrects)
            .count(),
        1,
        "the fixture should hold one correcting contract: {contracts:?}"
    );
    assert_eq!(contracts.len(), 2, "each run should capture one contract");

    for (session, id, corrects) in contracts {
        let mut app = run.app();
        app.load_session(&session).unwrap();
        app.command("/decisions", 100);
        let key = text::short_id(&id);
        let detail = detail_of_key(&mut app, 100, &key);
        if corrects {
            for expected in [
                "declared input inputs/observations-corrected.csv · captured at sha256".to_owned(),
                format!("corrects retained entry {}", text::short_id(&run.old)),
                "source change inputs/observations.csv replaced by inputs/observations-corrected.csv"
                    .to_owned(),
                "established by observation-value".to_owned(),
                "It also binds a correction".to_owned(),
            ] {
                assert!(
                    detail.contains(&expected),
                    "the correcting contract does not carry {expected:?}:\n{detail}"
                );
            }
        } else {
            assert!(
                detail.contains("declared input inputs/observations.csv · captured at sha256"),
                "the first contract does not name the file it declared:\n{detail}"
            );
            assert!(
                !detail.contains("source change") && !detail.contains("It also binds a correction"),
                "a contract that binds no correction reads as one:\n{detail}"
            );
        }

        // A narrow pane wraps a path under its label: no line runs past the width the detail
        // was built for, and no character of a path is lost to the wrapping.
        for width in [44u16, 30] {
            let page = app.page(width);
            let item = page
                .items
                .iter()
                .find(|item| item.key == key)
                .expect("the contract row is on the page");
            let text = lines_text(&item.detail);
            for line in text.lines() {
                assert!(
                    line.chars().count() <= width as usize,
                    "at {width} a line of the contract runs past its pane: {line:?}"
                );
            }
            let squeezed: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();
            let path = if corrects {
                "inputs/observations.csvreplacedbyinputs/observations-corrected.csv"
            } else {
                "declaredinputinputs/observations.csv·capturedatsha256"
            };
            assert!(
                squeezed.contains(path),
                "at {width} the declared paths lost characters:\n{text}"
            );
        }
    }
}

#[tokio::test]
async fn an_entry_recorded_for_other_conditions_is_not_current_here() {
    // The negative control for the scope. Both entries are recorded for Hill, and this reader is
    // configured for Harbor. Their stored status has not changed, so a page that read the status,
    // or that asked with no conditions at all, would call them current.
    let run = correction_run().await;
    let mut app = run.app();
    app.config
        .knowledge_scope
        .insert("site".into(), "Harbor".into());
    app.command("/memory", 100);

    let right = right_of_key(&mut app, 100, &run.new);
    assert!(
        right.contains("other scope"),
        "an entry recorded for other conditions reads as current here: {right}"
    );
    let detail = detail_of_key(&mut app, 100, &run.new);
    assert!(
        detail.contains("recorded for other conditions than the ones in force here"),
        "the page does not say why the entry is not current:\n{detail}"
    );
    assert!(
        detail.contains("site is Harbor"),
        "the page does not say what conditions it read under:\n{detail}"
    );
    assert!(
        detail.contains("applies only where") && detail.contains("site is Hill"),
        "the page does not say what the entry itself requires:\n{detail}"
    );
    assert!(
        !detail.contains("yes, as support under the default retrieval"),
        "an entry out of scope is reported as support:\n{detail}"
    );
    assert!(
        app.page(100).subtitle.contains("0 current"),
        "the page counts entries as current that are not: {}",
        app.page(100).subtitle
    );
}

#[tokio::test]
async fn a_retired_entry_is_kept_and_is_never_offered() {
    let run = correction_run().await;
    let mut app = run.app();
    app.command("/memory", 100);
    // The action the page offers on an entry, taken through the interface itself.
    let page_keys = keys_of(&mut app, 100);
    let index = page_keys
        .iter()
        .position(|key| key == &run.new)
        .expect("the replacement is on the page");
    for _ in 0..index {
        app.on_key(key(KeyCode::Down), 100);
    }
    app.on_key(key(KeyCode::Char('f')), 100);
    app.on_key(key(KeyCode::Enter), 100);
    app.command("/memory", 100);

    let right = right_of_key(&mut app, 100, &run.new);
    assert!(
        right.contains("retired"),
        "a retired entry does not say so: {right}"
    );
    let detail = detail_of_key(&mut app, 100, &run.new);
    assert!(
        detail.contains("retired: this entry was withdrawn")
            && detail.contains("no; the entry is retired"),
        "a retired entry is not reported as withdrawn and unoffered:\n{detail}"
    );
    assert!(
        detail.contains("60"),
        "a retired entry lost the text it recorded:\n{detail}"
    );
}

#[tokio::test]
async fn the_row_that_explains_the_page_is_not_something_an_action_applies_to() {
    let run = correction_run().await;
    let mut app = run.app();
    app.command("/memory", 100);
    assert_eq!(
        keys_of(&mut app, 100).first().map(String::as_str),
        Some(crate::views::ABOUT_KEY),
        "the page does not open with what its words mean"
    );
    app.on_key(key(KeyCode::Char('f')), 100);
    assert!(
        app.overlay.is_none(),
        "retiring was offered for a row that is not an entry"
    );
}

/// Keep both coordination fixtures where an interface walk can open them.
///
/// It exists so a walk in a real terminal reads the same records these tests read, written by the
/// same runtime rather than by a second fixture kept in step by hand. Nothing is copied or moved:
/// the temporary directories the run actually used are kept instead of removed, so the files a
/// result was accepted against are still where its record says they are. It is ignored by default
/// because leaving directories behind is not something a test suite should do.
#[tokio::test]
#[ignore]
async fn keep_the_fixtures_an_interface_walk_reads() {
    let Ok(target) = std::env::var("YMP_WALK_FIXTURE") else {
        println!("YMP_WALK_FIXTURE is not set, so nothing was kept");
        return;
    };
    let mut manifest = serde_json::Map::new();

    let board = board_views().await;
    let mut config = board.run.config.clone();
    config.acceptance_contracts = None;
    let home = board.run.home.keep();
    let project = board.run.project.keep();
    config.save(&home).unwrap();
    manifest.insert(
        "board".into(),
        serde_json::json!({
            "home": home, "project": project,
            "session": board.run.session, "status": board.run.status,
            "committed": board.committed, "rejected": board.rejected, "owed": board.owed,
        }),
    );

    let correction = correction_run().await;
    let mut config = correction.config.clone();
    // The contracts did their work and are recorded. Their verifier is kept with the rest, and a
    // walk starts no run, so the configuration it opens does not carry them.
    config.acceptance_contracts = None;
    let home = correction.home.keep();
    let root = correction._temp.keep();
    config.save(&home).unwrap();
    manifest.insert(
        "knowledge".into(),
        serde_json::json!({
            "home": home, "project": correction.directory, "root": root,
            "old": correction.old, "new": correction.new,
        }),
    );

    // Two sessions that captured a token ceiling, each kept with a configuration that says the
    // opposite of what it captured, so the walk reads a reopened session against a changed one.
    for (name, policy, reserve, today) in [
        (
            "stopped",
            ymp_core::UnknownUsagePolicy::Stop,
            Some(10_000),
            ymp_core::UnknownUsagePolicy::BoundedNative,
        ),
        (
            "bounded",
            ymp_core::UnknownUsagePolicy::BoundedNative,
            None,
            ymp_core::UnknownUsagePolicy::Stop,
        ),
    ] {
        let run = token_run(policy, reserve, name == "bounded").await;
        let requested = (policy == ymp_core::UnknownUsagePolicy::BoundedNative)
            .then(|| admit_requested_allowance(&run, 1_500));
        let config = moved_token_config(&run, today);
        let home = run.home.keep();
        let project = run.project.keep();
        config.save(&home).unwrap();
        manifest.insert(
            name.into(),
            serde_json::json!({
                "home": home, "project": project,
                "session": run.session, "status": run.status, "requested": requested,
            }),
        );
    }

    std::fs::write(
        PathBuf::from(&target).join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
    println!("manifest written under {target}");
}

// ---------------------------------------------------------------------------
// Agent labels
// ---------------------------------------------------------------------------

/// A configuration whose agents run on a native installation no test launches. The profile
/// names are captions an installation returns, so a label that falls back to a configured name
/// is caught, and so is one that falls back to the actor identifier.
fn native_label_config() -> Config {
    use ymp_core::{ProviderConfig, ProviderKind};
    Config {
        providers: vec![ProviderConfig {
            id: "fixture-provider".into(),
            kind: ProviderKind::Acp,
            command: "/ymp-test/never-launched".into(),
            args: vec![],
            env_refs: Default::default(),
            enabled: true,
        }],
        agents: [
            ("transport-one", "Default (recommended)"),
            ("transport-two", "Latest release"),
        ]
        .into_iter()
        .map(|(id, name)| AgentProfile {
            id: id.into(),
            name: name.into(),
            provider: "fixture-provider".into(),
            model: None,
            instructions: String::new(),
            enabled: true,
        })
        .collect(),
        team: vec!["transport-one".into(), "transport-two".into()],
        ..Config::default()
    }
}

/// A session that captured this configuration's team and has recorded nothing yet.
fn native_label_session(fixture: &Fixture, config: &Config) -> String {
    let project = fixture.store.project(fixture.project.path()).unwrap();
    let session = Session {
        id: new_id(),
        project_id: project.id,
        title: "Report the fixture fact".into(),
        status: "running".into(),
        created_at: now(),
        team: config.agents.clone(),
        turns_used: 0,
    };
    fixture.store.save_session(&session).unwrap();
    session.id
}

/// The identity a run captures when it admits a turn: the installation's caption, the model the
/// settings ask for, and the concrete model native metadata resolves that to, if any.
fn captured(caption: &str, model: &str, resolved: Option<&str>) -> ymp_core::AgentIdentity {
    ymp_core::AgentIdentity {
        name: caption.into(),
        configured_name: caption.into(),
        model: Some(model.into()),
        effort: None,
        resolved_model: resolved.map(str::to_owned),
        source: None,
        status: ymp_core::AgentIdentityStatus::Native,
    }
}

/// Admit one turn the way a run does, and record the model and effort the installation reported.
fn native_turn(
    fixture: &Fixture,
    session: &str,
    agent: &str,
    identity: ymp_core::AgentIdentity,
    reported: (Option<&str>, Option<&str>),
) -> String {
    let (assignment, invocation) = native_records(fixture, session, agent, identity);
    fixture
        .store
        .begin_invocation_with_grants(&assignment, &invocation, &[])
        .unwrap();
    report(fixture, session, &invocation.id, reported);
    invocation.id
}

/// Record the model and effort an installation reported for a running invocation.
fn report(
    fixture: &Fixture,
    session: &str,
    invocation: &str,
    reported: (Option<&str>, Option<&str>),
) {
    fixture
        .store
        .observe_invocation(
            session,
            invocation,
            &ymp_core::InvocationObservation {
                reported: Some(ymp_core::ExecutionSettings {
                    model: reported.0.map(str::to_owned),
                    effort: reported.1.map(str::to_owned),
                    permission_mode: None,
                }),
                ..Default::default()
            },
        )
        .unwrap();
}

/// The name the transcript gives the entry that carries exactly `text`.
fn author_of(app: &mut App, text: &str) -> String {
    let entries = app.entries();
    entries
        .iter()
        .find(|entry| entry.raw == text)
        .map(|entry| entry.author.clone())
        .unwrap_or_else(|| {
            let authors: Vec<(&String, &String)> = entries
                .iter()
                .map(|entry| (&entry.raw, &entry.author))
                .collect();
            panic!("no entry carries {text:?}: {authors:#?}")
        })
}

/// The records a run writes when it admits one turn: the assignment with the identity it
/// captured, and a running invocation that asks for the identity's model.
fn native_records(
    fixture: &Fixture,
    session: &str,
    agent: &str,
    identity: ymp_core::AgentIdentity,
) -> (ymp_core::AssignmentRecord, ymp_core::InvocationRecord) {
    let turn = fixture.store.trace(session).unwrap().invocations.len() as u64 + 1;
    let requested = ymp_core::ExecutionSettings {
        model: identity.model.clone(),
        ..Default::default()
    };
    let assignment = ymp_core::AssignmentRecord {
        token_reservation: None,
        agent_identity: Some(identity),
        id: new_id(),
        session_id: session.into(),
        task: None,
        agent_id: agent.into(),
        agent_config_version: "fixture".into(),
        provider_id: "fixture-provider".into(),
        purpose: "plan".into(),
        reason: "Fixture turn".into(),
        cwd: fixture.project.path().into(),
        requested: requested.clone(),
        timeout_secs: 10,
        grant_ids: Vec::new(),
        context: Vec::new(),
        state: ymp_core::InvocationState::Running,
        started_at: now(),
        ended_at: None,
    };
    let invocation = ymp_core::InvocationRecord {
        id: new_id(),
        session_id: session.into(),
        assignment_id: assignment.id.clone(),
        execution_backend: None,
        turn,
        requested: requested.clone(),
        sent: requested,
        reported: Default::default(),
        resumed_from: None,
        native_session_id: None,
        native_turn_id: None,
        native_version: None,
        state: ymp_core::InvocationState::Running,
        started_at: now(),
        ended_at: None,
        usage: None,
        terminal_reason: None,
    };
    (assignment, invocation)
}

#[test]
fn every_message_names_the_model_and_effort_of_the_invocation_that_wrote_it() {
    let fixture = fixture();
    let config = native_label_config();
    let session = native_label_session(&fixture, &config);
    let store = &fixture.store;
    store
        .message(&session, "you", None, "user", "Report the fixture fact")
        .unwrap();
    // Written before messages were linked to invocations: no record says which turn wrote it.
    store
        .message(
            &session,
            "transport-one",
            None,
            "chat",
            "An unlinked finding",
        )
        .unwrap();
    // The first turn asked for the internal default alias, which native metadata resolved, and
    // the installation reported the model it ran but no effort.
    let first = native_turn(
        &fixture,
        &session,
        "transport-one",
        captured(
            "Default (recommended)",
            "default",
            Some("claude-opus-5[1m]"),
        ),
        (Some("claude-opus-5"), None),
    );
    store
        .invocation_message(&session, &first, "plan", "The first turn's plan")
        .unwrap();
    store
        .finish_invocation(&session, &first, ymp_core::InvocationState::Completed, None)
        .unwrap();
    // The same actor's next turn ran another model and reported its effort.
    let second = native_turn(
        &fixture,
        &session,
        "transport-one",
        captured("Latest release", "glm-5.2", None),
        (Some("glm-5.2"), Some("max")),
    );
    store
        .invocation_message(&session, &second, "execute", "The second turn's report")
        .unwrap();
    store
        .finish_invocation(
            &session,
            &second,
            ymp_core::InvocationState::Completed,
            None,
        )
        .unwrap();
    store
        .message(&session, "ymp", None, "notice", "A runtime notice")
        .unwrap();

    let mut app = app_with(&fixture, config);
    app.load_session(&session).unwrap();
    let authors: Vec<(String, String)> = app
        .entries()
        .iter()
        .map(|entry| (entry.raw.clone(), entry.author.clone()))
        .collect();
    let author = |text: &str| {
        authors
            .iter()
            .find(|(raw, _)| raw == text)
            .map(|(_, author)| author.clone())
            .unwrap_or_else(|| panic!("no entry carries {text:?}: {authors:#?}"))
    };
    assert_eq!(author("Report the fixture fact"), "you");
    assert_eq!(author("A runtime notice"), "ymp");
    assert_eq!(
        author("The first turn's plan"),
        "claude-opus-5",
        "the first turn is not named by the model it reported alone, without an effort it did not report"
    );
    assert_eq!(
        author("The second turn's report"),
        "glm-5.2 max",
        "the second turn is not named by its own model and effort"
    );
    assert_eq!(
        author("An unlinked finding"),
        "unknown model",
        "a message no invocation is linked to was named from a turn that did not write it"
    );
    for (_, author) in &authors {
        for forbidden in [
            "Default (recommended)",
            "Latest release",
            "transport-",
            "default",
        ] {
            assert!(
                !author.contains(forbidden),
                "{author:?} carries {forbidden:?} as a name"
            );
        }
    }
    let rendered = draw(&mut app, 120, 30);
    assert!(
        rendered.contains("glm-5.2 max")
            && rendered.contains("claude-opus-5")
            && !rendered.contains("claude-opus-5 none"),
        "the transcript does not show the invocation labels:\n{rendered}"
    );
}

#[test]
fn a_live_stream_and_the_sidebar_name_the_invocation_that_is_running() {
    let fixture = fixture();
    let config = native_label_config();
    let session = native_label_session(&fixture, &config);
    let store = &fixture.store;
    store
        .message(&session, "you", None, "user", "Report the fixture fact")
        .unwrap();
    let earlier = native_turn(
        &fixture,
        &session,
        "transport-one",
        captured(
            "Default (recommended)",
            "default",
            Some("claude-opus-5[1m]"),
        ),
        (Some("claude-opus-5"), None),
    );
    store
        .invocation_message(&session, &earlier, "plan", "The earlier plan")
        .unwrap();
    store
        .finish_invocation(
            &session,
            &earlier,
            ymp_core::InvocationState::Completed,
            None,
        )
        .unwrap();
    let mut app = app_with(&fixture, config);
    app.load_session(&session).unwrap();
    app.active = true;

    // The runtime records the admission, and what the installation reported, before it streams.
    native_turn(
        &fixture,
        &session,
        "transport-one",
        captured("Latest release", "glm-5.2", None),
        (Some("glm-5.2"), Some("max")),
    );
    app.event(UiEvent::AgentStatus {
        agent: "transport-one".into(),
        status: "execute".into(),
    });
    app.event(UiEvent::Delta {
        agent: "transport-one".into(),
        text: "Working through the fixture".into(),
    });

    let stream = app
        .entries()
        .iter()
        .find(|entry| entry.kind == "streaming")
        .map(|entry| entry.author.clone())
        .expect("the stream is an entry");
    assert_eq!(
        stream, "glm-5.2 max",
        "the stream is not named by the running invocation"
    );
    let rows = screen_rows(&mut app, 120, 30);
    let team = rows
        .iter()
        .skip_while(|row| !row.contains("TEAM"))
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        team.contains("glm-5.2 max"),
        "the working member is not named by its running invocation:\n{team}"
    );
    let screen = rows.join("\n");
    for forbidden in ["Default (recommended)", "Latest release", "transport-"] {
        assert!(
            !screen.contains(forbidden),
            "the window names an agent {forbidden:?}:\n{screen}"
        );
    }
}

#[test]
fn tool_activity_names_the_running_invocation_and_keeps_the_members_purpose() {
    let fixture = fixture();
    let config = native_label_config();
    let session = native_label_session(&fixture, &config);
    fixture
        .store
        .message(&session, "you", None, "user", "Report the fixture fact")
        .unwrap();
    let mut app = app_with(&fixture, config);
    app.load_session(&session).unwrap();
    app.active = true;
    native_turn(
        &fixture,
        &session,
        "transport-one",
        captured("Default (recommended)", "default", Some("glm-5.2")),
        (Some("glm-5.2"), Some("max")),
    );
    app.event(UiEvent::AgentStatus {
        agent: "transport-one".into(),
        status: "execute".into(),
    });
    // What the runtime sends for one tool call: a plain status, then the typed activity.
    app.event(UiEvent::Status("Using Bash".into()));
    app.event(UiEvent::AgentStatus {
        agent: "transport-one".into(),
        status: "tool: Bash".into(),
    });
    assert_eq!(
        app.status, "glm-5.2 max · Bash",
        "tool activity is not named by the running invocation"
    );
    assert_eq!(
        app.statuses.get("transport-one").map(String::as_str),
        Some("execute"),
        "tool use replaced what the member's turn is for"
    );
    assert_eq!(app.agent_label("transport-one"), "glm-5.2 max");
    let screen = screen_rows(&mut app, 120, 30).join("\n");
    for forbidden in ["Default (recommended)", "transport-", "tool: "] {
        assert!(
            !screen.contains(forbidden),
            "the window shows {forbidden:?} for tool activity:\n{screen}"
        );
    }
}

/// Stored readings for two installations. One resolves its internal default alias to a concrete
/// model; the other lists the alias and resolves it to nothing.
fn scanned_selection_config() -> Config {
    use ymp_core::{
        CapabilitySource, ModelCapabilities, NativeProviderSnapshot, ProviderCapabilities,
    };
    let offering = |id: &str, caption: &str, resolved: Option<&str>| ModelCapabilities {
        id: id.into(),
        display_name: Some(caption.into()),
        picker_id: None,
        aliases: Vec::new(),
        resolved_model: resolved.map(str::to_owned),
        controls: None,
    };
    let mut config = Config::default();
    for agent in &mut config.agents {
        match agent.id.as_str() {
            "claude" => agent.model = Some("default".into()),
            "codex" => agent.model = Some("fixture-sol-7".into()),
            _ => {}
        }
    }
    for (id, provider, model, caption) in [
        (
            "native-swift",
            "claude",
            "swift",
            "Fastest for quick answers",
        ),
        (
            "native-unresolved",
            "codex",
            "default",
            "Default (recommended)",
        ),
    ] {
        config.agents.push(AgentProfile {
            id: id.into(),
            name: caption.into(),
            provider: provider.into(),
            model: Some(model.into()),
            instructions: String::new(),
            enabled: true,
        });
    }
    for (provider, models) in [
        (
            "claude",
            vec![
                offering(
                    "default",
                    "Default (recommended)",
                    Some("fixture-opus-9[1m]"),
                ),
                offering(
                    "swift",
                    "Fastest for quick answers",
                    Some("fixture-swift-3"),
                ),
            ],
        ),
        (
            "codex",
            vec![
                offering("fixture-sol-7", "Latest release", None),
                offering("default", "Default (recommended)", None),
            ],
        ),
    ] {
        let provider_fingerprint =
            ymp_core::provider_fingerprint(config.provider(provider).unwrap());
        config.native_catalog.providers.insert(
            provider.into(),
            NativeProviderSnapshot {
                provider_fingerprint,
                last_attempt: now(),
                failure: None,
                catalog: Some(ProviderCapabilities {
                    source: CapabilitySource::NativeMetadata {
                        method: "fixture.models".into(),
                        observed_at: now(),
                    },
                    models_complete: true,
                    models,
                    default_model: Some("default".into()),
                }),
            },
        );
    }
    config
}

/// The rows under the pool heading of the team page, by key and left-hand text.
fn pool_choices(app: &mut App, width: u16) -> Vec<(String, String)> {
    let left = |item: &crate::views::Item| {
        item.left
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>()
    };
    app.page(width)
        .items
        .iter()
        .skip_while(|item| {
            !(item.kind == crate::views::ItemKind::Heading
                && left(item) == "AVAILABLE ON THIS MACHINE")
        })
        .skip(1)
        .filter(|item| item.kind == crate::views::ItemKind::Row)
        .map(|item| (item.key.clone(), left(item)))
        .collect()
}

#[test]
fn a_choice_of_agent_is_its_concrete_model_and_never_a_caption_or_an_unresolved_alias() {
    let fixture = fixture();
    let mut app = app_with(&fixture, scanned_selection_config());
    app.command("/agents", 100);
    let captions = [
        "Default (recommended)",
        "Fastest for quick answers",
        "Latest release",
    ];
    for (profile, model) in [
        ("claude", "fixture-opus-9[1m]"),
        ("native-swift", "fixture-swift-3"),
        ("codex", "fixture-sol-7"),
    ] {
        let row = left_of_key(&mut app, 100, profile);
        assert!(
            row.starts_with(model),
            "{profile} is not named by its concrete model: {row}"
        );
        for caption in captions {
            assert!(
                !row.contains(caption),
                "{profile} carries the caption {caption:?}: {row}"
            );
        }
    }
    // An alias nothing resolves stays a configured profile, and it is not given a model name.
    let unresolved = left_of_key(&mut app, 100, "native-unresolved");
    assert!(
        unresolved.starts_with("unknown model") && !unresolved.contains("default"),
        "an unresolved alias is presented as a model: {unresolved}"
    );

    app.command("/team", 100);
    let choices = pool_choices(&mut app, 100);
    let keys: Vec<&str> = choices.iter().map(|(key, _)| key.as_str()).collect();
    assert!(
        !keys.contains(&"native-unresolved"),
        "an alias with no concrete model is offered as a choice: {choices:#?}"
    );
    assert!(
        keys.contains(&"claude") && keys.contains(&"native-swift") && keys.contains(&"codex"),
        "a concrete choice is missing: {choices:#?}"
    );
    for (key, left) in &choices {
        for caption in captions {
            assert!(
                !left.contains(caption),
                "the choice {key} carries the caption {caption:?}: {left}"
            );
        }
    }
}

#[tokio::test]
async fn local_fixture_agents_keep_their_names_and_the_user_and_ymp_keep_theirs() {
    let run = mock_run("Create a greeting", |_| {}).await;
    let mut app = run.app();
    app.load_session(&run.session).unwrap();
    let entries = app.entries().to_vec();
    assert!(
        entries.iter().any(|entry| entry.author == "you"),
        "the user's own entry lost its name"
    );
    let agents: Vec<_> = entries
        .iter()
        .filter(|entry| entry.author != "you" && entry.author != "ymp")
        .collect();
    assert!(!agents.is_empty(), "the mock run recorded no agent message");
    for entry in agents {
        assert!(
            ["one", "two"].contains(&entry.author.as_str()),
            "a local fixture agent lost its configured name: {:?} on {}",
            entry.author,
            entry.kind
        );
    }
}

#[test]
fn an_unlinked_native_message_stays_unknown_after_its_actor_runs_as_a_local_fixture() {
    let fixture = fixture();
    let native = native_label_config();
    let session = native_label_session(&fixture, &native);
    let store = &fixture.store;
    // Written by the native actor before messages were linked to invocations.
    store
        .message(
            &session,
            "transport-one",
            None,
            "chat",
            "An old native finding",
        )
        .unwrap();
    // The actor is later moved to a local fixture provider, and its next turn is bound.
    let mut local = native.clone();
    local.providers[0].kind = ymp_core::ProviderKind::Mock;
    local.providers[0].command = "internal".into();
    local.agents[0].name = "Local fixture".into();
    let identity = ymp_core::AgentIdentity {
        name: "Local fixture".into(),
        configured_name: "Local fixture".into(),
        model: None,
        effort: None,
        resolved_model: None,
        source: None,
        status: ymp_core::AgentIdentityStatus::Local,
    };
    let turn = native_turn(&fixture, &session, "transport-one", identity, (None, None));
    store
        .invocation_message(&session, &turn, "chat", "A bound local finding")
        .unwrap();
    store
        .finish_invocation(&session, &turn, ymp_core::InvocationState::Completed, None)
        .unwrap();

    let mut app = app_with(&fixture, local);
    app.load_session(&session).unwrap();
    let authors: Vec<(String, String)> = app
        .entries()
        .iter()
        .map(|entry| (entry.raw.clone(), entry.author.clone()))
        .collect();
    let author = |text: &str| {
        authors
            .iter()
            .find(|(raw, _)| raw == text)
            .map(|(_, author)| author.clone())
            .unwrap_or_else(|| panic!("no entry carries {text:?}: {authors:#?}"))
    };
    assert_eq!(
        author("An old native finding"),
        "unknown model",
        "an unlinked message was named from a later turn or the current provider"
    );
    assert_eq!(
        author("A bound local finding"),
        "Local fixture",
        "a bound local fixture turn lost the name it captured"
    );
}

#[test]
fn a_conversation_that_moves_to_a_new_session_keeps_naming_the_earlier_messages() {
    let fixture = fixture();
    let config = native_label_config();
    let store = &fixture.store;
    let earlier = native_label_session(&fixture, &config);
    let turn = native_turn(
        &fixture,
        &earlier,
        "transport-one",
        captured(
            "Default (recommended)",
            "default",
            Some("claude-opus-5[1m]"),
        ),
        (Some("claude-opus-5"), None),
    );
    store
        .invocation_message(&earlier, &turn, "chat", "The earlier answer")
        .unwrap();
    store
        .finish_invocation(&earlier, &turn, ymp_core::InvocationState::Completed, None)
        .unwrap();
    let mut app = app_with(&fixture, config.clone());
    app.load_session(&earlier).unwrap();
    assert_eq!(author_of(&mut app, "The earlier answer"), "claude-opus-5");

    // A follow-up that needs new work records a child session. Its first message opens that
    // session in the same window, below the conversation already shown.
    let child = native_label_session(&fixture, &config);
    let later = native_turn(
        &fixture,
        &child,
        "transport-one",
        captured("Latest release", "glm-5.2", None),
        (Some("glm-5.2"), Some("max")),
    );
    let message = store
        .invocation_message(&child, &later, "chat", "The follow-up answer")
        .unwrap();
    app.event(UiEvent::Message(message));
    assert_eq!(app.session.as_deref(), Some(child.as_str()));
    assert_eq!(
        author_of(&mut app, "The earlier answer"),
        "claude-opus-5",
        "the earlier session's message lost the invocation it is linked to"
    );
    assert_eq!(author_of(&mut app, "The follow-up answer"), "glm-5.2 max");

    // A message of the earlier session that was never read is named from that session's own
    // records, not from the session that is open now.
    app.attribution = crate::provenance::Attribution::default();
    app.event(UiEvent::AgentStatus {
        agent: "transport-one".into(),
        status: "idle".into(),
    });
    assert_eq!(
        author_of(&mut app, "The earlier answer"),
        "claude-opus-5",
        "an earlier session's message was not resolved from its own records"
    );
}

#[test]
fn a_model_and_effort_reported_after_a_message_was_linked_rename_that_message() {
    let fixture = fixture();
    let config = native_label_config();
    let session = native_label_session(&fixture, &config);
    let store = &fixture.store;
    // The turn posts its answer before the installation has reported what it ran.
    let turn = native_turn(
        &fixture,
        &session,
        "transport-one",
        captured("Latest release", "glm-5.2", None),
        (None, None),
    );
    store
        .invocation_message(&session, &turn, "chat", "An answer written mid-turn")
        .unwrap();
    let mut app = app_with(&fixture, config);
    app.load_session(&session).unwrap();
    app.active = true;
    assert_eq!(author_of(&mut app, "An answer written mid-turn"), "glm-5.2");

    report(&fixture, &session, &turn, (Some("glm-5.2"), Some("max")));
    app.event(UiEvent::AgentStatus {
        agent: "transport-one".into(),
        status: "execute".into(),
    });
    assert_eq!(
        author_of(&mut app, "An answer written mid-turn"),
        "glm-5.2 max",
        "a message linked before its invocation reported kept the settings read first"
    );

    // The turn ends, and its message keeps what the turn reported.
    store
        .finish_invocation(&session, &turn, ymp_core::InvocationState::Completed, None)
        .unwrap();
    app.event(UiEvent::AgentStatus {
        agent: "transport-one".into(),
        status: "idle".into(),
    });
    assert_eq!(
        author_of(&mut app, "An answer written mid-turn"),
        "glm-5.2 max"
    );
}

#[test]
fn a_thinking_switch_is_not_shown_as_an_effort_and_stays_in_the_details() {
    let fixture = fixture();
    let config = native_label_config();
    let session = native_label_session(&fixture, &config);
    let store = &fixture.store;
    // What GLM installations report for glm-4.7 and glm-4.5-air: no effort was requested or
    // sent, and the native binary thought control reads on. That is not a graded level.
    let toggled = native_turn(
        &fixture,
        &session,
        "transport-one",
        captured("GLM-4.7", "glm-4.7", None),
        (Some("glm-4.7"), Some("on")),
    );
    store
        .invocation_message(&session, &toggled, "chat", "A switched-on answer")
        .unwrap();
    let air = native_turn(
        &fixture,
        &session,
        "transport-two",
        captured("GLM-4.5-Air", "glm-4.5-air", None),
        (Some("glm-4.5-air"), Some("on")),
    );
    store
        .invocation_message(&session, &air, "chat", "An air answer")
        .unwrap();
    // A native level called none, reported as such, is an effort and not missing metadata.
    let none = native_turn(
        &fixture,
        &session,
        "transport-one",
        captured("Latest release", "glm-5.2", None),
        (Some("glm-5.2"), Some("none")),
    );
    store
        .invocation_message(&session, &none, "chat", "A reported none")
        .unwrap();

    let mut app = app_with(&fixture, config);
    app.load_session(&session).unwrap();
    assert_eq!(author_of(&mut app, "A switched-on answer"), "glm-4.7");
    assert_eq!(author_of(&mut app, "An air answer"), "glm-4.5-air");
    assert_eq!(author_of(&mut app, "A reported none"), "glm-5.2 none");

    // The switch is kept exactly as the installation reported it.
    app.command("/assignments", 100);
    let detail = row_prose(&mut app, 100, "glm-4.7");
    assert!(
        detail.contains("the installation reported on"),
        "the reported thought control is not kept in the record:\n{detail}"
    );
}

#[test]
fn a_shared_chat_post_is_named_by_the_invocation_its_team_operation_committed() {
    let fixture = fixture();
    let config = native_label_config();
    let session = native_label_session(&fixture, &config);
    let store = &fixture.store;
    let (mut assignment, invocation) = native_records(
        &fixture,
        &session,
        "transport-one",
        captured("Latest release", "glm-5.2", None),
    );
    let grant = ymp_core::GrantRecord::for_assignment(
        &assignment,
        &invocation,
        vec![ymp_core::TeamOperation::TeamPost],
    );
    assignment.grant_ids.push(grant.id.clone());
    store
        .begin_invocation_with_grants(&assignment, &invocation, std::slice::from_ref(&grant))
        .unwrap();
    report(
        &fixture,
        &session,
        &invocation.id,
        (Some("glm-5.2"), Some("max")),
    );
    let (_, posted) = store
        .team_call(
            &grant,
            ymp_core::TeamOperation::TeamPost,
            &serde_json::json!({"text": "A shared finding"}),
            "post-1",
        )
        .unwrap();
    let posted = posted.expect("a team post writes a chat message");
    store
        .finish_invocation(
            &session,
            &invocation.id,
            ymp_core::InvocationState::Completed,
            None,
        )
        .unwrap();
    // Only the committed team operation names the post's sequence.
    let naming: Vec<String> = store
        .trace(&session)
        .unwrap()
        .history
        .iter()
        .filter(|event| {
            event
                .data
                .get("message_seq")
                .and_then(serde_json::Value::as_i64)
                == Some(posted.seq)
        })
        .map(|event| event.kind.clone())
        .collect();
    assert_eq!(
        naming,
        ["provenance"],
        "the post is not linked through its team operation alone"
    );

    let mut app = app_with(&fixture, config);
    app.load_session(&session).unwrap();
    assert_eq!(author_of(&mut app, &posted.text), "glm-5.2 max");
}
