//! Acceptance: a measurement that lands after the operator held an account back does not put that
//! account's readiness back on this session.
//!
//! The measurement runs on a worker, so the operator can act while the engines are answering.
//! Holding an account back is one of the acts they can take, and it withdraws the readiness this
//! session measured — a profile left `ready` under a withdrawn account is what the routing reads,
//! and the routing decides which account a run is sent to. The report the engines produced was
//! read *before* that decision, so applying it whole would restore exactly what the decision took
//! away, and the run would reach the account whose permission to receive repository content the
//! operator has just withdrawn.
//!
//! The check that must fail: apply the outcome's report unconditionally. `/runtimes` then states
//! the withdrawn account's engine as ready, and the routing offers it as the profile that would do
//! the work — on a root whose provider record reads `enabled: false`.

mod support;

use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use ymp_runtime_registry::{ProviderFamily, RegistryAddress};
use ymp_tui::Session;
use ymp_tui::app::MeasurementOutcome;

/// How long the planted engine takes to answer `--version`, which is the window the operator's
/// decision arrives in.
const ENGINE_SECONDS: u64 = 2;

/// How long the outcome is waited for. It bounds a check that hangs, not the measurement.
const OUTCOME_WAIT: Duration = Duration::from_secs(60);

/// An engine this host would route to: it reports the release the pinned profile admits and an
/// authenticated first-party account, which is what makes its probe report `ready`.
fn plant_a_ready_engine(path: &Path) {
    fs::write(
        path,
        format!(
            r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  sleep {ENGINE_SECONDS}
  printf '%s\n' '2.1.227 (Claude Code)'
  exit 0
elif [ "$1" = "auth" ]; then
  printf '%s\n' '{{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"firstParty"}}'
  exit 0
fi
exit 1
"##
        ),
    )
    .expect("write the planted engine");
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).expect("make the planted engine executable");
}

#[test]
fn a_measurement_that_lands_after_the_account_was_held_back_restores_no_readiness() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let base = temporary.path();
    let root = base.join("root");
    let home = base.join("home");
    let search_path = base.join("bin");
    for directory in [&root, &home, &search_path] {
        fs::create_dir_all(directory).expect("fixture directory");
    }
    plant_a_ready_engine(&search_path.join("claude"));

    // This binary holds one test, so the search path and home it sets are read by nothing else.
    let path = format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", search_path.display());
    unsafe {
        std::env::set_var("PATH", &path);
        std::env::set_var("HOME", &home);
        std::env::remove_var("CLAUDE_CONFIG_DIR");
    }

    let address = RegistryAddress::Root(root.clone());
    let mut session = Session::open_under_root(&root, &root.join("runs/0001"), &[]);
    session.set_runtimes(ymp_tui::runtimes::read_all(&address));

    // The operator enables the account, and its engines start answering on a worker.
    let pending = session
        .set_provider_enabled(ProviderFamily::Anthropic, true, None)
        .expect("enabling an account asks for a measurement of it");
    let (deliver, outcomes) = mpsc::channel::<MeasurementOutcome>();
    session.start_measurement(pending, move |outcome| {
        let _ = deliver.send(outcome);
    });

    // While the engines are still answering, they change their mind and hold the account back.
    // The decision is durable and takes effect on this session at once.
    assert!(
        outcomes.try_recv().is_err(),
        "the engine answered before the decision was taken; raise ENGINE_SECONDS"
    );
    let held_back = Instant::now();
    assert!(
        session
            .set_provider_enabled(
                ProviderFamily::Anthropic,
                false,
                Some("held back while it was being measured".to_owned()),
            )
            .is_none(),
        "holding an account back asked for a measurement"
    );
    assert!(
        held_back.elapsed() < Duration::from_secs(ENGINE_SECONDS),
        "the decision waited for the engines it was taken during"
    );
    assert_eq!(
        session
            .runtimes()
            .expect("this session holds a reading")
            .ready_count(),
        0,
        "holding the account back left a profile of it ready before the measurement even landed"
    );

    // The measurement lands afterwards, carrying what the engines reported before the decision.
    let outcome = outcomes
        .recv_timeout(OUTCOME_WAIT)
        .expect("the measurement returned its outcome through the channel");
    session.finish_measurement(outcome);

    // The records hold the operator's decision.
    let record = fs::read_to_string(root.join("providers").join("anthropic.json"))
        .expect("the provider record");
    assert!(
        record.contains("\"enabled\": false"),
        "the account is not held back in the records: {record}"
    );

    // And so does this session: nothing of the withdrawn account is ready, and no run would be
    // routed to it.
    let report = session.runtimes().expect("this session holds a reading");
    assert_eq!(
        report.ready_count(),
        0,
        "the measurement put back the readiness of an account the operator held back: {:?}",
        report
            .profiles
            .iter()
            .map(|profile| (profile.name.clone(), profile.readiness_text()))
            .collect::<Vec<_>>()
    );
    let claude = report
        .profile("claude-code")
        .expect("the engine that reaches anthropic");
    assert!(
        !claude.ready(),
        "the engine of a withdrawn account is ready: {}",
        claude.detail
    );
    assert!(
        claude.detail.contains("nothing was measured here"),
        "the engine of a withdrawn account states a measurement taken before the withdrawal: {}",
        claude.detail
    );

    let projection = session.projection(None);
    assert_eq!(
        projection.route, None,
        "a run would be routed to an account the operator held back: {:?} · {}",
        projection.route, projection.route_note
    );
}
