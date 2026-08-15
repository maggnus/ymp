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
use ymp_domain::commitment::{AccountRef, BudgetVector, CommitmentEvent, Dimension};
use ymp_domain::{Budget, EventEnvelope, EventKind, RunState};
use ymp_tui::scenario::{self, Run};
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

/// A journal built record by record: the run, the kernel it opens, and one commitment record for
/// each group of facts given. Nothing here is decided by a ledger, because what is under test is a
/// record a ledger would refuse.
fn journal_of(records: Vec<Vec<CommitmentEvent>>) -> Run {
    let budget = BudgetVector::ZERO.with(Dimension::MoneyMicros, 1_000);
    let mut events = Vec::new();
    let mut sequence = 0;
    let mut predecessor = None;
    let mut push = |event: EventKind, events: &mut Vec<EventEnvelope>| {
        sequence += 1;
        let envelope = EventEnvelope::new(
            "demo-run",
            sequence,
            format!("cmd-{sequence}"),
            scenario::digest(sequence as u8),
            predecessor.clone(),
            event,
        )
        .expect("event envelope");
        predecessor = Some(envelope.digest.clone());
        events.push(envelope);
    };
    push(
        EventKind::RunStarted {
            budget: Budget::new(1, 1),
        },
        &mut events,
    );
    push(
        EventKind::CommitmentKernelOpened {
            root_participant: "sponsor-root".to_owned(),
            root_principal: "principal-root".to_owned(),
            root_obligation: "obligation-root".to_owned(),
            budget,
        },
        &mut events,
    );
    for facts in records {
        push(EventKind::CommitmentFactsRecorded { facts }, &mut events);
    }
    let mut state = RunState::from_start(&events[0]).expect("run_started is the first event");
    for envelope in events.iter().skip(1) {
        state.apply(envelope);
    }
    Run { state, events }
}

/// The reason the fold stopped is the one that stopped it, whatever records follow.
///
/// A fact that cannot be replayed ends the rebuilding of the ledger, and the page states why. The
/// records after it are read against a ledger that is no longer being built, so each of them finds
/// no kernel — and stating that instead told the operator the run had opened none, which is both
/// untrue and not what happened. What the page must carry is the first refusal.
///
/// The check that must fail: let a later commitment record write its own reason over the recorded
/// one, and the page names the missing kernel rather than the fact the ledger refused.
#[test]
fn the_page_keeps_the_first_refusal_when_further_records_follow_it() {
    // Capacity moving out of an account the ledger has never heard of. No ledger can replay it.
    let unreplayable = CommitmentEvent::BudgetTransferred {
        from: AccountRef::Participant {
            participant_id: "participant-nobody".to_owned(),
        },
        to: AccountRef::Participant {
            participant_id: "sponsor-root".to_owned(),
        },
        amount: BudgetVector::ZERO.with(Dimension::MoneyMicros, 1),
    };
    let after = CommitmentEvent::ClockAdvanced { to: 1 };
    let run = journal_of(vec![vec![unreplayable], vec![after]]);
    let app = app_for(Some(&run), vec![contract(true)]);

    let page = app
        .page(PageKind::Commitments)
        .expect("a run whose commitment record was refused still has the page that states it");
    let notes = page.notes.join(" ");
    assert!(
        notes.contains("unknown account: participant participant-nobody"),
        "the page does not state the fact the ledger refused: {notes}"
    );
    assert!(
        !notes.contains("before any kernel was opened"),
        "a later record replaced the refusal with a consequence of it: {notes}"
    );
}

#[test]
#[ignore = "renders the page for a human reader rather than asserting on it"]
fn render_the_page() {
    println!("{}", page(AWARDED));
}
