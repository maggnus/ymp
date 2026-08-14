//! Acceptance: cancelling from the interface ends a working attempt in both records that state
//! how a run ended, and leaves nothing of it running.
//!
//! The reproduction this answers is a cancellation written into the journal alone. The kernel that
//! holds the run's process slice lives in the process that started the attempt, so a cancellation
//! that went past it would move the journal while the kernel still held the slice open on an
//! unjudged candidate — and a verdict arriving afterwards could drive the same run to acceptance.
//! The interface therefore cancels through the attempt it holds, and this reads both records back.
//!
//! The check that must fail: make `Session::cancel_run` reach the journal directly while an
//! attempt is working, and the kernel terminal asserted below is absent.
//!
//! This binary holds one test and sets its own search path, so the profile the product probes is
//! the fixture beside it rather than whatever this host has installed. The agent is deterministic
//! and spends nothing; what is real is the product's own probe, admission, controller, kernel and
//! process supervision.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use ymp_domain::RunStatus;
use ymp_tui::runtimes::probe_all;
use ymp_tui::{Session, app::AttemptProgress};

/// A Codex build that launches, says it started and then waits to be stopped. It never submits,
/// so the only thing that can end this run is the cancellation.
const WAITING_FIXTURE: &str = r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
elif [ "$1" = "login" ]; then
  exit 0
else
  cat >/dev/null &
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-waiting"}'
  printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"working"}}'
  sleep 120
fi
"##;

fn executable(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write program");
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path).expect("metadata").permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions).expect("make executable");
}

#[test]
fn cancelling_a_working_attempt_ends_it_in_both_records_that_state_how_a_run_ended() {
    let temporary = tempfile::tempdir().expect("temporary root");
    let base = temporary.path();
    let source = base.join("source");
    let negative_control = base.join("negative-control");
    let search_path = base.join("bin");
    for directory in [&source, &negative_control, &search_path] {
        fs::create_dir_all(directory).expect("fixture directory");
    }
    fs::write(source.join("input.txt"), b"before\n").expect("source file");
    let verifier = base.join("verify.sh");
    executable(&verifier, "#!/bin/sh\ntest -f \"$1/result.txt\"\n");
    executable(&search_path.join("codex"), WAITING_FIXTURE);

    // This binary holds one test, so the search path it sets is read by nothing else.
    let path = format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", search_path.display());
    unsafe {
        std::env::set_var("PATH", &path);
    }

    let store = base.join("store");
    let mut session = Session::open(&store, &[]);
    for line in [
        "write the result of the work".to_owned(),
        format!("source {}", source.display()),
        format!("verifier {}", verifier.display()),
        format!("negative control {}", negative_control.display()),
    ] {
        session.local_turn(line);
    }
    let contract_id = session
        .projection(None)
        .contracts
        .first()
        .expect("the request produced a contract")
        .contract_id
        .clone();
    session.set_runtimes(probe_all());
    assert_eq!(
        session.projection(None).route.as_deref(),
        Some("codex"),
        "the fixture profile is not the one this host would use: {}",
        session.projection(None).route_note
    );
    session.start_run(&contract_id);
    session.start_attempt();
    assert!(
        session.attempt_is_live(),
        "no attempt was launched, so there is nothing to cancel"
    );

    // Let the agent reach the point where it is working and waiting.
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut working = false;
    while Instant::now() < deadline && !working {
        if matches!(session.poll_attempt(), AttemptProgress::Advanced) {
            working = session
                .projection(None)
                .entries
                .iter()
                .any(|entry| matches!(entry, ymp_tui::Entry::RuntimeNote { text, .. } if text == "working"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(working, "the fixture agent never reported that it started");

    session.cancel_run();

    // The journal: the run ended as cancelled and never as anything about a candidate.
    let projection = session.projection(None);
    let run = projection.run.as_ref().expect("the run is still readable");
    assert_eq!(run.status, RunStatus::Cancelled);
    assert!(run.candidate_digest.is_none());

    // The kernel record of the process slice, read back from the attempt the interface held.
    let transcript: String = projection
        .entries
        .iter()
        .filter_map(|entry| match entry {
            ymp_tui::Entry::AppReply { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        transcript.contains("the kernel record of the attempt reads Cancelled"),
        "the cancellation did not reach the kernel record of the attempt:\n{transcript}"
    );

    // Nothing of the attempt is left running or left able to commit a later command.
    assert!(!session.attempt_is_live());
    assert!(
        !fs::read_to_string(store.join("events.jsonl"))
            .expect("committed journal")
            .contains("candidate_submitted"),
        "a cancelled attempt committed a candidate"
    );
}
