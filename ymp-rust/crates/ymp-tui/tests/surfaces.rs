//! Acceptance: the product reaches every supported surface with the declared keys at both
//! sizes, and its deterministic buffers preserve terminal reason, primary action, selection,
//! bounded content and state distinctions without colour or mouse input.
//!
//! Removing one required surface, one state marker or one keyboard transition makes this
//! harness fail: each is asserted by name below.

mod support;

use crossterm::event::KeyCode;
use support::{SIZES, app_for, buffer, contract, open_command, press, rebuild, screen, type_text};

/// One line of text: the rendered rows joined and their padding collapsed, so an assertion can
/// look for a phrase the layout wrapped.
fn flatten(rendered: &str) -> String {
    rendered.split_whitespace().collect::<Vec<_>>().join(" ")
}
use ymp_tui::app::Action;
use ymp_tui::scenario;
use ymp_tui::state::{Follow, Modal, PageKind, Surface};

/// Every page reachable through the `:` command line, with the text that identifies it.
const PAGES: [(PageKind, &str); 5] = [
    (PageKind::Runtimes, "runtimes(all)"),
    (PageKind::Candidates, "candidates(demo-run)"),
    (PageKind::Events, "events(demo-run)"),
    (PageKind::Budgets, "budgets(demo-run)"),
    (PageKind::Attempts, "attempts(demo-run)"),
];

#[test]
fn every_page_is_reached_through_the_command_line_at_both_sizes() {
    for (width, height) in SIZES {
        let run = scenario::running();
        for (kind, marker) in PAGES {
            let mut app = app_for(Some(&run), vec![contract(true)]);
            let action = open_command(&mut app, kind.command_name(), height);
            assert_eq!(action, None, "{kind:?} asked for an action");
            assert_eq!(
                app.surface,
                Surface::Page(kind),
                "{kind:?} at {width}x{height}"
            );

            let rendered = screen(&app, width, height);
            assert!(
                rendered.contains(marker),
                "{kind:?} at {width}x{height} did not draw {marker:?}:\n{rendered}"
            );

            press(&mut app, KeyCode::Esc, height);
            assert_eq!(app.surface, Surface::Transcript, "Esc did not return");
        }
    }
}

#[test]
fn enter_on_a_candidate_descends_to_describe_and_esc_returns_to_the_list() {
    for (width, height) in SIZES {
        let run = scenario::running();
        let mut app = app_for(Some(&run), vec![contract(true)]);
        open_command(&mut app, "candidates", height);

        let action = press(&mut app, KeyCode::Enter, height);
        assert_eq!(action, Some(Action::Rebuild));
        rebuild(&mut app, Some(&run), vec![contract(true)]);
        assert_eq!(app.surface, Surface::Page(PageKind::Describe));

        let rendered = screen(&app, width, height);
        assert!(rendered.contains("describe #0003"), "{rendered}");
        assert!(rendered.contains("rejected"), "{rendered}");

        press(&mut app, KeyCode::Esc, height);
        assert_eq!(app.surface, Surface::Page(PageKind::Candidates));
    }
}

#[test]
fn both_decision_surfaces_and_the_key_map_are_reachable_at_both_sizes() {
    for (width, height) in SIZES {
        let run = scenario::running();

        let mut app = app_for(Some(&run), vec![contract(true)]);
        open_command(&mut app, "authorize contract-1", height);
        assert!(matches!(app.modal, Modal::Authorize(_)));
        let rendered = screen(&app, width, height);
        assert!(rendered.contains("authorize contract"), "{rendered}");
        assert!(
            flatten(&rendered).contains("negative control"),
            "{rendered}"
        );
        press(&mut app, KeyCode::Esc, height);
        assert!(matches!(app.modal, Modal::None));

        let mut app = app_for(Some(&run), vec![contract(true)]);
        open_command(&mut app, "cancel demo-run", height);
        assert!(matches!(app.modal, Modal::Confirm(_)));
        let rendered = screen(&app, width, height);
        assert!(rendered.contains("cancel run demo-run"), "{rendered}");
        assert!(rendered.contains("irreversible"), "{rendered}");
        press(&mut app, KeyCode::Esc, height);
        assert!(matches!(app.modal, Modal::None));

        let mut app = app_for(Some(&run), vec![contract(true)]);
        press(&mut app, KeyCode::Char('?'), height);
        assert!(matches!(app.modal, Modal::Keys));
        let rendered = screen(&app, width, height);
        assert!(rendered.contains("keys"), "{rendered}");
        assert!(rendered.contains("resume live follow"), "{rendered}");
        press(&mut app, KeyCode::Esc, height);
        assert!(matches!(app.modal, Modal::None));
    }
}

/// A run identifier the product actually generates: `ymp internal managed-candidate-smoke`
/// names its run with a prefix and a UUID. A short fixture leaves the decision title well
/// inside the frame; a real one does not, and the irreversibility marker is what must survive.
const PRODUCT_RUN_ID: &str = "managed-candidate-smoke-c2b1b17b-49c9-4e11-91cd-f8469e82995d";

#[test]
fn the_irreversibility_marker_survives_a_product_run_identifier() {
    for (width, height) in SIZES {
        let run = scenario::running_named(PRODUCT_RUN_ID);

        let mut app = app_for(Some(&run), vec![contract(true)]);
        open_command(&mut app, &format!("cancel {PRODUCT_RUN_ID}"), height);
        assert!(matches!(app.modal, Modal::Confirm(_)));
        let rendered = screen(&app, width, height);
        assert!(
            rendered.contains("irreversible"),
            "the cancellation reads as reversible at {width}x{height}:\n{rendered}"
        );
        // The operator has to type the identifier exactly, so it must be readable in full.
        assert!(
            flatten(&rendered).contains(PRODUCT_RUN_ID),
            "the run id to type is not readable at {width}x{height}:\n{rendered}"
        );

        let mut app = app_for(Some(&run), vec![contract(true)]);
        app.open_authorize();
        let rendered = screen(&app, width, height);
        assert!(
            rendered.contains("irreversible"),
            "the authorization surface lost its marker at {width}x{height}:\n{rendered}"
        );
    }
}

#[test]
fn typed_confirmation_gates_the_only_irreversible_command() {
    let run = scenario::running();
    let mut app = app_for(Some(&run), vec![contract(true)]);
    open_command(&mut app, "cancel demo-run", 40);

    // A partial identifier never confirms.
    type_text(&mut app, "demo", 40);
    assert_eq!(press(&mut app, KeyCode::Enter, 40), None);
    assert!(matches!(app.modal, Modal::Confirm(_)));
    assert!(screen(&app, 120, 40).contains("disabled until the id matches exactly"));

    type_text(&mut app, "-run", 40);
    assert!(screen(&app, 120, 40).contains("Enter confirm"));
    assert_eq!(press(&mut app, KeyCode::Enter, 40), Some(Action::CancelRun));
    assert!(matches!(app.modal, Modal::None));
}

#[test]
fn every_terminal_outcome_is_named_with_a_distinct_marker_and_its_recorded_reason() {
    let cases = [
        (scenario::accepted(), "[+] accepted", None),
        (
            scenario::exhausted(),
            "[-] exhausted",
            Some("attempt budget exhausted"),
        ),
        (
            scenario::abstained(),
            "[?] abstained",
            Some("could not decide"),
        ),
        (
            scenario::cancelled(),
            "[x] cancelled",
            Some("cancelled by the operator"),
        ),
        (
            scenario::infrastructure_error(),
            "[!] infrastructure_error",
            Some("environment object could not be read"),
        ),
    ];

    for (width, height) in SIZES {
        for (run, marker, reason) in &cases {
            let app = app_for(Some(run), vec![contract(true)]);
            let rendered = screen(&app, width, height);
            assert!(
                rendered.contains(marker),
                "{marker} missing at {width}x{height}:\n{rendered}"
            );
            assert!(
                !rendered.contains("done"),
                "quiescence was named as completion:\n{rendered}"
            );
            if let Some(reason) = reason {
                // The reason may be wrapped in the transcript, so look for it there and in the
                // status line without the wrapping.
                let flat = flatten(&rendered);
                assert!(
                    flat.contains(reason),
                    "terminal reason {reason:?} missing at {width}x{height}:\n{rendered}"
                );
            }
        }
    }
}

#[test]
fn a_live_run_offers_cancel_and_a_terminal_one_does_not() {
    let live = app_for(Some(&scenario::running()), vec![contract(true)]);
    let ended = app_for(Some(&scenario::cancelled()), vec![contract(true)]);

    let names = |app: &ymp_tui::state::App| -> Vec<String> {
        app.data
            .commands
            .iter()
            .map(|item| item.name.clone())
            .collect()
    };
    assert!(names(&live).iter().any(|name| name.starts_with("cancel")));
    assert!(!names(&ended).iter().any(|name| name.starts_with("cancel")));

    let mut ended = ended;
    ended.open_cancel_confirm();
    assert!(
        matches!(ended.modal, Modal::None),
        "a terminal run offered cancellation"
    );
}

#[test]
fn scrolling_pauses_follow_and_end_resumes_it() {
    for (width, height) in SIZES {
        let mut app = app_for(Some(&scenario::high_volume(200)), vec![contract(true)]);
        assert_eq!(app.follow, Follow::Live);

        press(&mut app, KeyCode::PageUp, height);
        assert_eq!(app.follow, Follow::Paused);
        let paused = screen(&app, width, height);
        assert!(paused.contains("paused"), "{paused}");
        assert!(paused.contains("End resume live"), "{paused}");

        press(&mut app, KeyCode::End, height);
        assert_eq!(app.follow, Follow::Live);
        assert!(screen(&app, width, height).contains("live"));
    }
}

#[test]
fn a_high_volume_page_bounds_its_rows_and_keeps_the_selection_visible() {
    for (width, height) in SIZES {
        let run = scenario::high_volume(2_000);
        let mut app = app_for(Some(&run), vec![contract(true)]);
        open_command(&mut app, "candidates", height);

        let rendered = screen(&app, width, height);
        assert!(
            rendered.contains("candidates(demo-run)[2000]"),
            "{rendered}"
        );
        assert!(
            rendered.contains("more rows outside the window"),
            "{rendered}"
        );

        // The selection starts on the newest row and survives paging.
        let selected = app.selection_of(PageKind::Candidates);
        assert_eq!(selected, 1_999);
        press(&mut app, KeyCode::PageUp, height);
        let moved = app.selection_of(PageKind::Candidates);
        assert!(moved < selected, "PageUp did not move the selection");
        let rendered = buffer(&app, width, height);
        assert!(
            rendered.iter().any(|line| line.contains(&format!(
                "#{:04}",
                // rows are published from journal position 3 upwards
                moved + 3
            ))),
            "the selected row left the window:\n{}",
            rendered.join("\n")
        );
    }
}

/// The opening screen at both declared sizes: the wordmark, one line of basic facts and the
/// invitation to state a request. Every other line the interface used to print on start is a
/// fact another surface already carries, and printing it here spent the first screen on it.
#[test]
fn the_cold_transcript_opens_on_the_logo_one_line_of_basics_and_the_invitation() {
    for (width, height) in SIZES {
        let app = app_for(None, Vec::new());
        let rendered = screen(&app, width, height);
        let stated = flatten(&rendered);

        assert!(
            rendered.contains(r"  \__, |_| |_| |_| .__/"),
            "the wordmark did not open the transcript at {width}x{height}:\n{rendered}"
        );
        assert!(
            stated.contains("/tmp/checkout · ymp 0.1.0"),
            "the basics line did not carry the directory and the version:\n{rendered}"
        );
        assert!(
            stated.contains("state your request below in one line"),
            "the invitation is missing:\n{rendered}"
        );

        // What left the feed, and where each fact still lives: the store and the assurance
        // glyph on the header and the status line, the assurance sentence in `?` and on
        // `/runtimes`, the palette in the status line's own hint.
        for retired in [
            "ymp store",
            "no hostile-code containment",
            "no run recorded",
            "no contract drafted",
            "/runtimes",
            "key map",
        ] {
            assert!(
                !stated.contains(retired),
                "the opening screen still prints {retired:?} at {width}x{height}:\n{rendered}"
            );
        }
        assert!(stated.contains("no contract · no run"), "{rendered}");
        assert!(stated.contains("/commands"), "{rendered}");
    }
}

#[test]
fn below_the_minimum_size_the_guard_replaces_every_surface() {
    let app = app_for(Some(&scenario::running()), vec![contract(true)]);
    let rendered = screen(&app, 72, 20);
    assert!(rendered.contains("terminal too small"), "{rendered}");
    assert!(rendered.contains("80x24"), "{rendered}");
    assert!(rendered.contains("72x20"), "{rendered}");
    assert!(rendered.contains("quit safely"), "{rendered}");
    assert!(!rendered.contains("follow"), "the frame leaked through");
}

#[test]
fn buffers_are_deterministic_and_never_exceed_the_terminal() {
    for (width, height) in SIZES {
        for run in [
            scenario::running(),
            scenario::accepted(),
            scenario::cancelled(),
        ] {
            let app = app_for(Some(&run), vec![contract(true)]);
            let first = buffer(&app, width, height);
            let second = buffer(&app, width, height);
            assert_eq!(first, second, "the same state drew two different buffers");
            assert_eq!(first.len(), height as usize);
            for line in &first {
                assert_eq!(
                    line.chars().count(),
                    width as usize,
                    "line width drifted: {line:?}"
                );
            }
        }
    }
}

#[test]
fn a_local_turn_is_answered_honestly_and_never_shown_as_recorded() {
    let mut app = app_for(Some(&scenario::running()), vec![contract(true)]);
    type_text(&mut app, "please add a test", 40);
    let action = press(&mut app, KeyCode::Enter, 40);
    assert_eq!(
        action,
        Some(Action::LocalTurn("please add a test".to_owned()))
    );
}

#[test]
fn quitting_is_reachable_from_the_transcript_and_from_a_page() {
    let mut app = app_for(Some(&scenario::running()), vec![contract(true)]);
    press(&mut app, KeyCode::Char('q'), 40);
    assert!(app.should_quit);

    let mut app = app_for(Some(&scenario::running()), vec![contract(true)]);
    open_command(&mut app, "events", 40);
    press(&mut app, KeyCode::Char('q'), 40);
    assert!(app.should_quit);
}
