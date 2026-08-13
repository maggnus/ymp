//! Acceptance: every value on screen derives from an application projection.
//!
//! Two halves. The structural half rejects a rendered field whose value is fixed in the source
//! instead of read from state: no module may carry a domain-shaped identifier, a POC-2 concept
//! or a currency amount as a literal. The differential half changes the state and requires the
//! screen to change with it, so a field cannot be bound to a constant that merely looks right.
//!
//! The check that must fail: put a domain value back into the drawing layer — for example
//! restore the hint `" fix t-13 · "` in `src/overlay.rs`, or a fabricated home path such as
//! `format!(" (~/code/{})", project)` in `src/ui.rs` — and
//! `no_screen_value_is_fixed_in_the_source` reports it with a non-zero exit.

mod support;

use support::{app_for, contract, crate_sources, open_command, screen};
use ymp_tui::journal;
use ymp_tui::projection::{BudgetClass, BudgetDimension};
use ymp_tui::scenario;
use ymp_tui::state::PageKind;

/// The shape of an identifier in the design illustration: a short lowercase tag, a dash and a
/// number — `rn-2209`, `tc-07`, `cd-32`, `pt-01`, `at-12`, `ev-0041`, `vr-21`, `t-13`, `m-12`.
/// A real identifier reaches the screen from the journal or from a probe, never from the source.
fn illustration_shape(token: &str) -> bool {
    let Some((tag, number)) = token.split_once('-') else {
        return false;
    };
    (1..=3).contains(&tag.len())
        && tag.chars().all(|c| c.is_ascii_lowercase())
        && !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
}

/// Concepts the current domain cannot produce. Naming one as a value would claim it exists.
const UNSUPPORTED_CONCEPTS: [&str; 9] = [
    "usd_llm",
    "wall_clock",
    "participant_starts",
    "tokens_est",
    "disk_mb",
    "board.message",
    "acme-payments",
    "poc_disposable",
    "~/code",
];

#[test]
fn no_screen_value_is_fixed_in_the_source() {
    let mut offenders = Vec::new();
    for (name, source) in crate_sources() {
        for (number, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            // Documentation may name the superseded illustration; only code may not.
            if trimmed.starts_with("//") {
                continue;
            }
            let report = |offenders: &mut Vec<String>, what: &str| {
                offenders.push(format!("{name}:{}: {what} in {}", number + 1, line.trim()));
            };
            for concept in UNSUPPORTED_CONCEPTS {
                if line.contains(concept) {
                    report(&mut offenders, concept);
                }
            }
            for token in line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-')) {
                if illustration_shape(token) {
                    report(&mut offenders, token);
                }
            }
            // A currency amount cannot come from a domain this budget does not carry.
            if line.contains('$') && line.contains(char::is_numeric) {
                report(&mut offenders, "a currency amount");
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these values are fixed in the source instead of read from state:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn the_budget_class_on_screen_follows_the_projection() {
    // The page is asked to draw a class the current domain never produces. A page that spelled
    // the class out instead of reading it would print `enforced` here.
    let dimensions = vec![
        BudgetDimension {
            name: "gated",
            class: BudgetClass::Enforced,
            total: Some(3),
            remaining: 1,
        },
        BudgetDimension {
            name: "recorded_only",
            class: BudgetClass::Observed,
            total: None,
            remaining: 7,
        },
    ];
    let page = journal::budgets_page("demo-run", &dimensions, "status".to_owned());
    let rendered: String = page
        .layout(120, 24)
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    for dimension in &dimensions {
        assert!(
            rendered.contains(dimension.class.label()),
            "{} lost its class:\n{rendered}",
            dimension.name
        );
    }
    assert_eq!(page.breadcrumb[1], "budgets(demo-run)[2]");

    // And the live page shows only the classes the run's own dimensions carry.
    let run = scenario::running();
    let mut app = app_for(Some(&run), vec![contract(true)]);
    let facts = app.data.run.clone().expect("the run was read");
    let initial = run
        .initial_budget()
        .expect("the run recorded its starting budget");
    let live = facts.budget_dimensions(Some(&initial));
    open_command(&mut app, "budgets", 40);
    let screen = screen(&app, 120, 40);
    for class in [BudgetClass::Observed, BudgetClass::Estimated] {
        if !live.iter().any(|dimension| dimension.class == class) {
            assert!(
                !screen.contains(class.label()),
                "the live page shows a class no dimension carries: {}\n{screen}",
                class.label()
            );
        }
    }
}

#[test]
fn the_transcript_and_status_change_when_the_run_state_changes() {
    let running = app_for(Some(&scenario::running()), vec![contract(true)]);
    let accepted = app_for(Some(&scenario::accepted()), vec![contract(true)]);

    let live = screen(&running, 120, 40);
    let ended = screen(&accepted, 120, 40);
    assert_ne!(live, ended, "two different runs drew the same screen");
    assert!(live.contains("[*] running"), "{live}");
    assert!(!live.contains("[+] accepted"), "{live}");
    assert!(ended.contains("[+] accepted"), "{ended}");
    assert!(!ended.contains("[*] running"), "{ended}");
}

#[test]
fn the_budget_page_shows_the_dimensions_the_state_carries_and_no_others() {
    let run = scenario::running();
    let mut app = app_for(Some(&run), vec![contract(true)]);
    open_command(&mut app, "budgets", 40);
    let rendered = screen(&app, 120, 40);

    assert!(rendered.contains("attempts"), "{rendered}");
    assert!(rendered.contains("verification_queries"), "{rendered}");
    // The totals and remainders come from the run, not from a constant.
    assert!(rendered.contains("budgets(demo-run)[2]"), "{rendered}");
    assert!(
        rendered.contains(&run.state.budget.attempts_remaining.to_string()),
        "{rendered}"
    );
    assert!(
        rendered.contains("cost, wall time and participant starts are unavailable"),
        "the page did not state what the domain cannot produce:\n{rendered}"
    );
}

#[test]
fn a_verdict_on_screen_follows_the_verdict_in_the_journal() {
    for (run, expected, forbidden) in [
        (scenario::running(), "rejected", "accepted"),
        (scenario::accepted(), "accepted", "rejected"),
    ] {
        let mut app = app_for(Some(&run), vec![contract(true)]);
        open_command(&mut app, "candidates", 40);
        let rendered = screen(&app, 120, 40);
        assert!(rendered.contains(expected), "{rendered}");
        assert!(!rendered.contains(forbidden), "{rendered}");
    }
}

#[test]
fn the_authorization_map_reports_the_contract_it_was_given() {
    let run = scenario::running();

    let mut covered = app_for(Some(&run), vec![contract(true)]);
    covered.open_authorize();
    let rendered = screen(&covered, 120, 40);
    assert!(rendered.contains("verify.sh"), "{rendered}");
    assert!(rendered.contains("negative-control"), "{rendered}");
    assert!(!rendered.contains("BLOCKING"), "{rendered}");

    let mut blocked = app_for(Some(&run), vec![contract(false)]);
    blocked.open_authorize();
    let rendered = screen(&blocked, 120, 40);
    assert!(rendered.contains("BLOCKING"), "{rendered}");
    assert!(rendered.contains("blocking item"), "{rendered}");
}

#[test]
fn a_page_with_no_state_behind_it_is_unavailable_rather_than_populated() {
    // A cold store carries no run, so every run-scoped page is absent from the palette and
    // from the projection; asking for one lands on the unavailable surface, not on numbers.
    let app = app_for(None, vec![]);
    for kind in [
        PageKind::Candidates,
        PageKind::Events,
        PageKind::Budgets,
        PageKind::Attempts,
    ] {
        assert!(
            app.page(kind).is_none(),
            "{kind:?} was populated without a run"
        );
        assert!(
            !app.data
                .commands
                .iter()
                .any(|item| item.name == kind.command_name()),
            "{kind:?} was offered without a run"
        );
    }

    let mut app = app;
    app.surface = ymp_tui::state::Surface::Page(PageKind::Events);
    let rendered = screen(&app, 120, 40);
    assert!(rendered.contains("unavailable"), "{rendered}");
    assert!(!rendered.contains("demo-run"), "{rendered}");
}

#[test]
fn the_assurance_profile_on_screen_is_the_one_the_project_contract_names() {
    let app = app_for(Some(&scenario::running()), vec![contract(true)]);
    let rendered = screen(&app, 120, 40);
    assert!(
        rendered.contains(ymp_tui::projection::ASSURANCE_PROFILE),
        "{rendered}"
    );
    assert!(
        rendered.contains("no hostile-code containment"),
        "the profile was shown without its limit:\n{rendered}"
    );
}
