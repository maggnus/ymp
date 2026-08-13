//! Acceptance: the negative control a draft is demonstrated against is the project as it stands.
//!
//! The product states its copy as "a copy of the project as it stands", and a run started from
//! that draft is judged against that copy for as long as it lasts. A copy kept from an earlier
//! assembly would make both statements false: the draft would be demonstrated against a state
//! the project has left, and the operator would authorize a control that no longer represents
//! anything.
//!
//! The scenario is the reviewer's: assemble a draft, change the project so that its own test
//! entry point now succeeds, and assemble again. The verifier proposed from that entry point now
//! accepts the project, so the second assembly must refuse it — a program that accepts the
//! negative control decides nothing. Against a kept copy it would pass instead, and a contract
//! would be drafted over a control that holds a result.

mod support;

use std::fs;
use std::path::PathBuf;

use support::screen;
use tempfile::TempDir;
use ymp_tui::Session;
use ymp_tui::state::App;

struct Project {
    _root: TempDir,
    source: PathBuf,
    data_root: PathBuf,
}

/// A project whose own test entry point fails while the work is not done.
fn project() -> Project {
    let root = TempDir::new().expect("temporary root");
    let directory = root.path().canonicalize().expect("resolve root");
    let source = directory.join("source");
    let data_root = directory.join("data");
    fs::create_dir_all(source.join("scripts")).expect("source directory");
    fs::create_dir_all(&data_root).expect("data root");
    let entry_point = source.join("scripts/test.sh");
    fs::write(&entry_point, b"#!/bin/sh\ntest -f result.txt\n").expect("test entry point");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&entry_point).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&entry_point, permissions).expect("make executable");
    }
    Project {
        _root: root,
        source,
        data_root,
    }
}

fn flattened(app: &App) -> String {
    screen(app, 120, 40)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn a_second_draft_is_demonstrated_against_the_project_as_it_stands() {
    let project = project();
    let mut session = Session::open(&project.data_root, &[]);
    let mut app = App::new(session.projection(None));

    session.local_turn("keep the replay path idempotent".to_owned());
    session.local_turn(format!("source {}", project.source.display()));
    app.adopt(session.projection(None));

    let first = app
        .data
        .contracts
        .first()
        .cloned()
        .expect("the project reached a drafted contract");
    let control = first
        .verifier
        .as_ref()
        .expect("the drafted contract carries a verifier")
        .negative_control
        .clone();
    assert!(
        !control.join("result.txt").exists(),
        "the copy taken before the work already held a result"
    );

    // The work happens: the project now satisfies its own test entry point.
    fs::write(project.source.join("result.txt"), b"result\n").expect("the work");

    // The draft is stated again. What it is demonstrated against is the project as it now
    // stands, so the verifier proposed from that entry point accepts it and is refused.
    session.local_turn("keep the replay path idempotent, once more".to_owned());
    app.adopt(session.projection(None));

    let stated = flattened(&app);
    assert!(
        stated.contains("accepted the negative control"),
        "the second draft was demonstrated against a copy the project has left:\n{stated}"
    );
    assert!(
        app.data.contracts.is_empty(),
        "a contract was drafted over a negative control that holds a result"
    );
    assert!(
        !project.data_root.join("events.jsonl").exists(),
        "a refused draft wrote a journal"
    );

    // The copy the withdrawn draft was assembled against is gone with it, so nothing offers a
    // control that no longer exists.
    assert!(
        !control.is_dir(),
        "the copy of a withdrawn draft was left behind at {}",
        control.display()
    );
}
