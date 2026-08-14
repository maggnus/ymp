//! Acceptance: the interface states who would do the work before anything is spent, refuses a
//! profile that cannot do it without reaching for another, and shows what the agent reports as the
//! agent's own words rather than as a fact the journal carries.
//!
//! Every surface here is rendered at both accepted sizes. The runtime report is a value, so the
//! same host state renders identically wherever these run; the live halves of the same claims are
//! `ymp-cli/tests/the_product_completes_and_exports_a_run.rs`, which drives the built product.

mod support;

use std::path::PathBuf;

use support::{SIZES, environment, screen};
use ymp_application::Application;
use ymp_domain::Budget;
use ymp_runtime_api::Readiness;
use ymp_tui::journal::Model;
use ymp_tui::runtimes::{ProfileFacts, Report};
use ymp_tui::state::{App, Modal};
use ymp_tui::{Session, scenario};

fn profile(name: &str, readiness: Readiness, detail: &str) -> ProfileFacts {
    ProfileFacts {
        name: name.to_owned(),
        runtime: name.to_owned(),
        model_route: None,
        executable: format!("/nonexistent/{name}"),
        version: None,
        readiness,
        detail: detail.to_owned(),
    }
}

/// A host on which exactly one managed profile can do the work.
fn one_ready() -> Report {
    Report {
        profiles: vec![
            profile("fake", Readiness::Ready, "deterministic in-process runtime"),
            profile("codex", Readiness::Ready, "authenticated"),
            profile(
                "claude-code",
                Readiness::Unauthenticated,
                "no operator credential",
            ),
        ],
    }
}

/// A host on which none can.
fn none_ready() -> Report {
    Report {
        profiles: vec![
            profile("fake", Readiness::Ready, "deterministic in-process runtime"),
            profile("codex", Readiness::NotInstalled, "executable not found"),
            profile(
                "claude-code",
                Readiness::Incompatible,
                "profile requires 2.1.227, found 2.1.232",
            ),
        ],
    }
}

/// A store holding one live run, opened by a session that carries the given host report.
fn live_run(report: Report) -> (tempfile::TempDir, Session) {
    let directory = tempfile::tempdir().expect("temporary store");
    let store = directory.path().join("data");
    std::fs::create_dir_all(&store).expect("data root");
    drop(Application::create(&store, "live-run", Budget::new(1, 1)).expect("committed run"));
    let mut session = Session::open(&store, &[]);
    session.set_runtimes(report);
    (directory, session)
}

#[test]
fn the_only_ready_profile_is_the_one_the_attempt_would_use() {
    let (_directory, session) = live_run(one_ready());
    let projection = session.projection(None);
    assert_eq!(projection.route.as_deref(), Some("codex"));
    assert!(
        projection
            .commands
            .iter()
            .any(|item| item.name == "attempt live-run"),
        "the interface offers no way to start the work"
    );

    let mut app = App::new(projection);
    app.open_attempt_confirm();
    let Modal::Confirm(confirm) = &app.modal else {
        panic!("the interface offers no confirmation for starting the agent");
    };
    assert_eq!(confirm.required, "live-run");
    for (width, height) in SIZES {
        let rendered = screen(&app, width, height);
        assert!(rendered.contains("codex"), "{rendered}");
        assert!(rendered.contains("irreversible"), "{rendered}");
        assert!(
            rendered.contains("your own account"),
            "the confirmation does not say what is spent:\n{rendered}"
        );
    }
}

#[test]
fn a_host_that_can_do_no_work_offers_no_attempt_and_names_every_profile() {
    let (_directory, mut session) = live_run(none_ready());
    let projection = session.projection(None);
    assert_eq!(projection.route, None);
    assert!(
        !projection
            .commands
            .iter()
            .any(|item| item.name == "attempt live-run"),
        "an attempt was offered on a host where nothing can do the work"
    );
    // The note names each profile and what this host reported about it, so the operator knows
    // which one to make ready rather than being told only that nothing is.
    for expected in ["codex", "claude-code", "executable not found", "2.1.227"] {
        assert!(
            projection.route_note.contains(expected),
            "the routing does not name {expected}: {}",
            projection.route_note
        );
    }

    session.start_attempt();
    let mut app = App::new(session.projection(None));
    app.resume_live();
    for (width, height) in SIZES {
        let rendered = screen(&app, width, height);
        assert!(
            rendered.contains("no attempt was launched"),
            "the refusal is not on screen:\n{rendered}"
        );
    }
}

/// A profile the operator named and this host cannot start stops the work there. The ready
/// profile beside it is not used instead: a run done by another agent is another run.
#[test]
fn a_named_profile_that_cannot_do_the_work_is_never_replaced_by_the_one_that_can() {
    let (_directory, mut session) = live_run(one_ready());
    session.local_turn("runtime claude-code".to_owned());
    assert_eq!(session.projection(None).route, None);

    session.start_attempt();
    let app = App::new(session.projection(None));
    let rendered = screen(&app, 120, 40);
    assert!(rendered.contains("claude-code"), "{rendered}");
    assert!(
        rendered.contains("nothing else is used in its place"),
        "{rendered}"
    );
    assert!(
        !rendered.contains("attempt launched"),
        "the work was routed to a profile the operator did not name:\n{rendered}"
    );
}

/// A name that selects no profile selects nothing at all.
#[test]
fn a_profile_name_this_product_does_not_ship_is_refused_rather_than_resolved() {
    let (_directory, mut session) = live_run(one_ready());
    session.local_turn("runtime claude".to_owned());
    let rendered = screen(&App::new(session.projection(None)), 120, 40);
    assert!(
        rendered.contains("no runtime profile is named claude"),
        "{rendered}"
    );
    // The route the host settled on its own is untouched: a line that named nothing changed
    // nothing.
    assert_eq!(session.projection(None).route.as_deref(), Some("codex"));
}

/// What the agent says is the agent's, and it is never dressed as a durable fact.
#[test]
fn what_the_runtime_reports_is_attributed_to_it_and_is_not_a_journal_event() {
    let run = scenario::running();
    let mut model = Model::cold(environment(), Vec::new());
    model.absorb(&run.state, &run.events);
    model.runtime("codex", "the result is written");
    let app = App::new(model.projection(None));

    for (width, height) in SIZES {
        let rendered = screen(&app, width, height);
        assert!(
            rendered.contains("codex") && rendered.contains("the result is written"),
            "the agent's own output is not attributed on screen:\n{rendered}"
        );
    }
    // The events page reads the journal, and the journal carries no such event.
    let events = model
        .projection(None)
        .page(ymp_tui::state::PageKind::Events)
        .expect("the run has an events page")
        .clone();
    let ymp_tui::pages::Body::Table { rows, .. } = &events.body else {
        panic!("the events page is a table");
    };
    assert!(
        !rows.iter().any(|row| row
            .cells
            .iter()
            .any(|cell| cell.text.contains("the result is written"))),
        "what the runtime said was recorded as a journal event"
    );
}

/// There is something to take out of a store only once a candidate exists.
#[test]
fn the_export_is_offered_once_a_candidate_exists_and_not_before() {
    let started = scenario::exhausted();
    let mut model = Model::cold(environment(), Vec::new());
    model.absorb(&started.state, &started.events);
    assert!(
        !model
            .projection(None)
            .commands
            .iter()
            .any(|item| item.name == "export"),
        "an export was offered for a run that published no candidate"
    );

    let run = scenario::accepted();
    let mut model = Model::cold(environment(), Vec::new());
    model.absorb(&run.state, &run.events);
    assert!(
        model
            .projection(None)
            .commands
            .iter()
            .any(|item| item.name == "export"),
        "the accepted candidate cannot be taken out of the store"
    );
}

/// A store with no run has nothing to export, and says so instead of writing an empty directory.
#[test]
fn exporting_a_store_with_no_run_writes_nothing() {
    let directory = tempfile::tempdir().expect("temporary store");
    let store = directory.path().join("empty");
    std::fs::create_dir_all(&store).expect("data root");
    let mut session = Session::open(&store, &[]);
    session.export_evidence(None);
    let rendered = screen(&App::new(session.projection(None)), 120, 40);
    assert!(rendered.contains("nothing to export"), "{rendered}");
    assert!(!store.join("exports").exists());
    let _: PathBuf = session.export_destination();
}
