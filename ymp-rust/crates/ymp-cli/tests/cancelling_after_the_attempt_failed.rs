//! Acceptance: a cancellation reports the terminal the record holds, not the one it asked for.
//!
//! There is a window between an attempt failing and its worker reporting itself finished. A
//! cancellation issued inside it finds the run already terminal for another reason, the domain
//! refuses to rename a terminal, and nothing about the run was cancelled. Saying `cancelled`
//! anyway would be the interface asserting an outcome the journal does not carry — and an
//! operator would read a run the machinery lost as a run they had ended.
//!
//! The check that must fail: state the outcome from the command instead of from the state after
//! the model has caught up, and the reply below reads `cancelled` over a run the journal records
//! as `infrastructure_error`.
//!
//! This binary holds one test and sets its own search path, so the profile the product probes is
//! the fixture beside it. The agent is deterministic and spends nothing.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use ymp_domain::RunStatus;
use ymp_runtime_registry::Engine;
use ymp_tui::runtimes::{Measure, probe_all};
use ymp_tui::{Session, projection};

/// A Codex build that launches and then dies without a terminal event of its own. The controller
/// records the run as an infrastructure failure; the worker is still on its way out.
const DYING_FIXTURE: &str = r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.151.0'
elif [ "$1" = "exec" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'resume --json --ignore-user-config --ignore-rules'
elif [ "$1" = "exec" ] && [ "$2" = "resume" ] && [ "$3" = "--help" ]; then
  printf '%s\n' 'SESSION_ID --json --ignore-user-config --ignore-rules'
elif [ "$1" = "features" ] && [ "$2" = "list" ]; then
  printf '%s\n' 'hooks stable true' 'multi_agent stable true' 'multi_agent_v2 stable false' 'plugins stable true' 'remote_plugin stable true' 'shell_snapshot stable true' 'enable_fanout removed false' 'remote_control removed false' 'remote_models removed false'
elif [ "$1" = "app-server" ] && [ "$2" = "--help" ]; then
  printf '%s\n' 'generate-json-schema --listen <URL> stdio://'
elif [ "$1" = "app-server" ] && [ "$2" = "generate-json-schema" ]; then
  mkdir -p "$4"
  printf '\173"definitions":\173"v2":\173"TokenUsageBreakdown":\173"required":["cachedInputTokens","inputTokens","outputTokens","reasoningOutputTokens","totalTokens"]\175\175\175,"items":[\173"title":"McpToolCallThreadItem","required":["arguments","id","server","status","tool","type"]\175],"methods":["thread/resume","turn/interrupt","thread/tokenUsage/updated","turn/completed"]\175\n' > "$4/codex_app_server_protocol.schemas.json"
elif [ "$1" = "login" ]; then
  exit 0
else
  cat >/dev/null
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-dying"}'
  exit 19
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
fn cancelling_a_run_the_machinery_already_ended_reports_what_the_record_holds() {
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
    executable(&search_path.join("codex"), DYING_FIXTURE);

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
    // The registry decides which engines this host admits, and Codex is held back where no
    // operator states otherwise. This check runs against a Codex fixture of its own, so it states
    // that decision the way an operator does — through the interface's own line.
    session.local_turn("runtime enable codex".to_owned());
    session.set_runtimes(probe_all(session.registry_address(), Measure::Recorded));
    // A run is created against the pool it may draw models from, so this root is put into the
    // state one measured account leaves behind — on the engine this check routes through.
    ymp_testkit::ready_root::measured_engine(&store, Engine::Codex, &["gpt-5-codex"]);
    session.start_run(&contract_id);
    session.start_attempt();
    assert!(session.attempt_is_live(), "no attempt was launched");

    // The attempt is never polled, so the session still holds it while the controller records the
    // failure. Waiting on the journal rather than on the attempt is what puts the cancellation
    // inside the window this check is about.
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        session.refresh();
        let run = session
            .projection(None)
            .run
            .expect("the run is readable")
            .status;
        if run == RunStatus::InfrastructureError {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the controller never recorded the failure of the attempt"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    session.cancel_run();

    let projection = session.projection(None);
    let status = projection.run.as_ref().expect("the run is readable").status;
    assert_eq!(
        status,
        RunStatus::InfrastructureError,
        "a cancellation renamed a terminal the machinery had already recorded"
    );
    let said: String = projection
        .entries
        .iter()
        .filter_map(|entry| match entry {
            ymp_tui::Entry::AppReply { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let outcome = projection::outcome(status);
    assert!(
        said.contains(&format!(
            "the run ended with the terminal outcome {outcome}"
        )),
        "the cancellation did not report the terminal the record holds:\n{said}"
    );
    assert!(
        !said.contains("the terminal outcome cancelled"),
        "the cancellation claimed an outcome the journal does not carry:\n{said}"
    );
}
