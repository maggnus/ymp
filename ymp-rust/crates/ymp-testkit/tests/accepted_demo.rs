//! Acceptance: the accepted demo verifies its submitted candidate through the typed application
//! operation and remains recoverable without extracting an object path.

use tempfile::tempdir;
use ymp_application::Application;
use ymp_domain::RunStatus;

#[test]
fn accepted_demo_verifies_and_recovers_through_application() {
    let temporary = tempdir().expect("temporary directory");
    let report = ymp_testkit::run_accepted_demo(temporary.path()).expect("run accepted demo");

    assert_eq!(report.state.status, RunStatus::Accepted);
    assert!(report.duplicate_command_replayed);
    assert_eq!(
        report.state.candidate_digest.as_deref(),
        Some(report.candidate.snapshot_digest.as_str())
    );

    let reopened = Application::open(temporary.path()).expect("reopen accepted demo");
    assert_eq!(reopened.state(), &report.state);
}
