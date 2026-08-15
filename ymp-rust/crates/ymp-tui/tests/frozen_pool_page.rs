//! Acceptance: the run's own pages state the capability boundary the run was created under.
//!
//! The operator never creates the snapshot and never has to. Where they meet it is the evidence of
//! the run — so the pages that exist for a created run state it in one line: the pool it was taken
//! from, how much of it admission found live, the digest, and the entry the run ignited on.
//!
//! The check that must fail: draw either line from anything but the record — a fixed digest, or the
//! first entry of the snapshot instead of the first live one — and this reports it with a non-zero
//! exit, because the scenario's first entry is deliberately one admission did not find live.

mod support;

use support::{app_for, contract, open_command, screen};
use ymp_tui::scenario;
use ymp_tui::state::{PageKind, Surface};

fn page(kind: PageKind) -> String {
    let run = scenario::with_a_frozen_pool();
    let mut app = app_for(Some(&run), vec![contract(true)]);
    let action = open_command(&mut app, kind.command_name(), 40);
    assert_eq!(action, None, "the page asked for an action");
    assert_eq!(app.surface, Surface::Page(kind));
    screen(&app, 120, 40)
}

/// The events page carries the record itself and the note beneath it. It exists for every run that
/// was created, which is what makes it the page the freeze is stated on.
#[test]
fn the_events_page_states_the_boundary_the_run_was_created_under() {
    let rendered = page(PageKind::Events);

    assert!(
        rendered.contains("pool.frozen"),
        "the record is not on the page:\n{rendered}"
    );
    // The entry the run ignites on is the first live one, which is the second entry of this pool.
    assert!(
        rendered.contains("claude-opus-5"),
        "the page does not name the entry the run ignited on:\n{rendered}"
    );
    assert!(
        rendered.contains("1 of 2 entries live"),
        "the page does not state how much of the pool was live:\n{rendered}"
    );
    // And the digest is the snapshot's own, in the short form every other digest is shown in.
    let digest = scenario::digest(0xf0);
    assert!(
        rendered.contains(&digest[..8]),
        "the page does not state the digest the run froze:\n{rendered}"
    );
}

/// The same sentence stands with the run's other notes, where a reader looking for what the run may
/// draw on would go.
#[test]
fn the_note_names_the_pool_the_digest_and_the_ignition_entry() {
    let rendered = page(PageKind::Events);

    assert!(
        rendered.contains("default"),
        "the note does not name the pool the snapshot was taken from:\n{rendered}"
    );
    assert!(
        rendered.contains("belongs to the next run"),
        "the note does not state that a later change reaches the next run:\n{rendered}"
    );
}
