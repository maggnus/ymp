//! Acceptance: the operator reaches the commitments of a run, and every value on that page is
//! the ledger's.
//!
//! Two halves. The first requires the page to state the task contract, the obligations and the
//! party that holds each of them. The second changes the escrow one award moves and requires the
//! screen to change with it, so a cell cannot be bound to a number that merely looks right.
//!
//! The check that must fail: draw the page from anything but the folded ledger — for example
//! give a row a fixed escrow string instead of the contract's own — and
//! `the_escrow_on_screen_follows_the_ledger` reports it with a non-zero exit.

mod support;

use support::{app_for, contract, open_command, screen};
use ymp_tui::scenario;
use ymp_tui::state::{PageKind, Surface};

/// What one award moves in the scenario, and a second amount no ledger in this file holds twice.
const AWARDED: u64 = 2_000;
const AWARDED_AGAIN: u64 = 7_000;

fn page(units: u64) -> String {
    let run = scenario::with_commitments_of(units);
    let mut app = app_for(Some(&run), vec![contract(true)]);
    let action = open_command(&mut app, PageKind::Commitments.command_name(), 40);
    assert_eq!(action, None, "the commitments page asked for an action");
    assert_eq!(app.surface, Surface::Page(PageKind::Commitments));
    screen(&app, 120, 40)
}

#[test]
fn the_page_states_the_task_contract_and_the_obligations_under_it() {
    let rendered = page(AWARDED);
    assert!(rendered.contains("commitments(demo-run)"), "{rendered}");
    // The contract, the party holding it, and both obligations — the root the run is accountable
    // for and the one the award created under it.
    for value in [
        "contract-one",
        "participant-one",
        "obligation-root",
        "obligation-one",
        "sponsor-root",
    ] {
        assert!(rendered.contains(value), "{value} is missing:\n{rendered}");
    }
    assert!(rendered.contains("active"), "{rendered}");
    assert!(rendered.contains("root of the run"), "{rendered}");
}

#[test]
fn the_escrow_on_screen_follows_the_ledger() {
    let awarded = page(AWARDED);
    let awarded_again = page(AWARDED_AGAIN);
    assert_ne!(
        awarded, awarded_again,
        "two ledgers holding different escrow drew the same page"
    );
    assert!(awarded.contains(&AWARDED.to_string()), "{awarded}");
    assert!(
        !awarded.contains(&AWARDED_AGAIN.to_string()),
        "the page shows escrow this ledger does not hold:\n{awarded}"
    );
    assert!(
        awarded_again.contains(&AWARDED_AGAIN.to_string()),
        "{awarded_again}"
    );
}

#[test]
fn a_run_whose_journal_opened_no_kernel_offers_no_commitments_page() {
    let app = app_for(Some(&scenario::running()), vec![contract(true)]);
    assert!(
        app.page(PageKind::Commitments).is_none(),
        "the page was populated for a run with no commitment kernel"
    );
    assert!(
        !app.data
            .commands
            .iter()
            .any(|item| item.name == PageKind::Commitments.command_name()),
        "the page was offered for a run with no commitment kernel"
    );
}

#[test]
#[ignore = "renders the page for a human reader rather than asserting on it"]
fn render_the_page() {
    println!("{}", page(AWARDED));
}
