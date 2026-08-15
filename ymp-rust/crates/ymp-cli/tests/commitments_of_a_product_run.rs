#![forbid(unsafe_code)]

//! Acceptance: a run carried out through the product records the facts of its commitment kernel in
//! its own journal, and the operator reads them back from that store afterwards.
//!
//! Everything here drives the built `ymp` executable against a store of its own, with a home
//! directory of its own and a runtime fixture on its search path, so the path under test is the
//! product's: its own request, its own authorization, its own controller and its own pages. The
//! operator's store is never touched.
//!
//! Two things are required of the run that finishes. Its journal carries both record types the
//! commitment kernel writes — the genesis a ledger is built from, and the facts each commitment
//! command committed — and those facts name the work this run formed rather than any state a
//! process happened to hold. And `ymp show commitments`, reading that store as an operator reads
//! it, draws the task contract and the obligations under it instead of refusing.
//!
//! The negative half is the tree before the kernel of a run reached the journal: the same run left
//! no such record, and `ymp show commitments` over its store exited non-zero saying the store has
//! no commitments page. Running this check against that tree is what shows it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;
use ymp_tui::Session;

/// A project, an acceptance condition, and a Codex stand-in the product's own probe admits.
struct Fixture {
    root: TempDir,
    source: PathBuf,
    verifier: PathBuf,
    negative_control: PathBuf,
    search_path: PathBuf,
    home: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = TempDir::new().expect("temporary root");
        let base = root.path();
        let source = base.join("source");
        let negative_control = base.join("negative-control");
        let search_path = base.join("bin");
        let home = base.join("home");
        for directory in [&source, &negative_control, &search_path, &home] {
            fs::create_dir_all(directory).expect("fixture directory");
        }
        fs::write(source.join("input.txt"), b"before\n").expect("source file");
        let verifier = base.join("verify.sh");
        executable(&verifier, "#!/bin/sh\ntest -f \"$1/result.txt\"\n");
        executable(&search_path.join("codex"), &codex_fixture());

        Self {
            root,
            source,
            verifier,
            negative_control,
            search_path,
            home,
        }
    }

    fn store(&self, name: &str) -> PathBuf {
        self.root.path().join(name)
    }

    fn request_arguments(&self) -> Vec<String> {
        vec![
            "--prompt=write the result of the work".to_owned(),
            format!("--source={}", self.source.display()),
            format!("--verifier={}", self.verifier.display()),
            format!("--negative-control={}", self.negative_control.display()),
        ]
    }

    /// Run the built product against one store, with a home and a search path of its own so that
    /// no profile installed on this host is reachable and the operator's own state is untouched.
    fn command(&self, store: &Path, arguments: &[String]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ymp"))
            .env(
                "PATH",
                format!(
                    "{}:/usr/bin:/bin:/usr/sbin:/sbin",
                    self.search_path.display()
                ),
            )
            .env("HOME", &self.home)
            .env_remove("CODEX_HOME")
            .env_remove("CLAUDE_CONFIG_DIR")
            .arg("--data-root")
            .arg(store)
            .args(arguments)
            .output()
            .expect("run the ymp executable")
    }

    /// The contract identifier the same request derives, read from the interface's own session over
    /// a store of its own.
    fn contract_id(&self) -> String {
        let mut session = Session::open(&self.store("identifiers"), &[]);
        for line in [
            "write the result of the work".to_owned(),
            format!("source {}", self.source.display()),
            format!("verifier {}", self.verifier.display()),
            format!("negative control {}", self.negative_control.display()),
        ] {
            session.local_turn(line);
        }
        session
            .projection(None)
            .contracts
            .first()
            .expect("the request produced a contract")
            .contract_id
            .clone()
    }
}

fn executable(path: &Path, contents: &str) {
    fs::write(path, contents).expect("write program");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions).expect("make executable");
    }
}

fn reported(outcome: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&outcome.stdout),
        String::from_utf8_lossy(&outcome.stderr)
    )
}

fn journal(store: &Path) -> String {
    fs::read_to_string(store.join("events.jsonl")).expect("committed journal")
}

fn run_id(store: &Path) -> String {
    Session::open(store, &[])
        .projection(None)
        .run
        .expect("the store holds a run")
        .run_id
}

/// Carry the request to a run and let one attempt of it finish, the way an operator does.
fn attempt_through_the_product(fixture: &Fixture, store: &Path) {
    let admitted = fixture.command(
        store,
        &[
            "runtime".to_owned(),
            "enable".to_owned(),
            "codex".to_owned(),
        ],
    );
    assert!(
        admitted.status.success(),
        "the codex engine was not admitted: {}",
        reported(&admitted)
    );
    let contract_id = fixture.contract_id();
    let mut arguments = fixture.request_arguments();
    arguments.insert(0, "start".to_owned());
    arguments.push(format!("--confirm={contract_id}"));
    let started = fixture.command(store, &arguments);
    assert!(
        started.status.success(),
        "the run was not started: {}",
        reported(&started)
    );
    let run_id = run_id(store);
    let attempted = fixture.command(
        store,
        &[
            "attempt".to_owned(),
            "--runtime=codex".to_owned(),
            format!("--confirm={run_id}"),
        ],
    );
    assert!(
        attempted.status.success(),
        "the attempt did not complete: {}",
        reported(&attempted)
    );
}

#[test]
fn a_run_carried_out_through_the_product_journals_its_commitments_and_shows_them() {
    let fixture = Fixture::new();
    let store = fixture.store("commitments");
    attempt_through_the_product(&fixture, &store);

    // Both record types of the kernel are in the run's own journal: the state the ledger was built
    // from, and the facts of the commands committed against it.
    let journal = journal(&store);
    assert!(
        journal.contains("\"type\":\"commitment_kernel_opened\""),
        "the run opened no commitment kernel in its journal: {journal}"
    );
    assert!(
        journal.contains("\"type\":\"commitment_facts_recorded\""),
        "the run committed no commitment fact to its journal: {journal}"
    );
    // The facts are this run's own work: the contract it formed, the obligation created under it,
    // and the process slice the runtime ran in.
    for fact in [
        "\"event\":\"task_contract_formed\"",
        "\"event\":\"obligation_created\"",
        "\"event\":\"invocation_started\"",
        "\"event\":\"invocation_closed\"",
        "\"event\":\"submission_recorded\"",
    ] {
        assert!(
            journal.contains(fact),
            "the journal carries no {fact}: {journal}"
        );
    }

    // And the operator reads them off that store, through the command that shows the page.
    let shown = fixture.command(&store, &["show".to_owned(), "commitments".to_owned()]);
    let page = reported(&shown);
    assert!(
        shown.status.success(),
        "the commitments of a finished run could not be shown: {page}"
    );
    assert!(
        page.contains(&format!("commitments({})", run_id(&store))),
        "the page shown is not the commitments of this run: {page}"
    );
    assert!(
        page.contains("1 task contract"),
        "the page states no task contract for a run that formed one: {page}"
    );
    assert!(
        !page.contains("0 obligation"),
        "the page states no obligation for a run that created one: {page}"
    );
    assert!(
        !page.contains("the kernel is open and holds nothing yet"),
        "the page of a finished run reads as empty: {page}"
    );
    assert!(
        !page.contains("could not be replayed"),
        "the page refused the record the run wrote: {page}"
    );
}

/// A Codex build that does the work of this fixture: it writes the result into its workspace and
/// submits it through the product's own bridge, which is the only way a candidate reaches the
/// journal. It answers the version the shipped profile is pinned to, read from the profile itself
/// so that a change to the pin cannot leave this fixture silently refused.
fn codex_fixture() -> String {
    format!(
        r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' '{pinned}'
elif [ "$1" = "login" ]; then
  exit 0
else
  bridge=''
  workspace=''
  previous=''
  for argument in "$@"; do
    if [ "$previous" = '-C' ]; then
      workspace=$argument
    fi
    case "$argument" in
      mcp_servers.ymp.command=*)
        bridge=${{argument#mcp_servers.ymp.command=}}
        bridge=${{bridge#\"}}
        bridge=${{bridge%\"}}
        ;;
    esac
    previous=$argument
  done
  test -n "$bridge" || exit 31
  test -n "$workspace" || exit 32
  cat >/dev/null
  printf '%s\n' 'done' > "$workspace/result.txt"
  {{
    printf '%s\n' '{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-11-25"}}}}'
    printf '%s\n' '{{"jsonrpc":"2.0","method":"notifications/initialized"}}'
    printf '%s\n' '{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"submit","arguments":{{"command_id":"fixture.submit"}}}}}}'
  }} | "$bridge" internal agent-mcp > "$workspace/mcp.responses"
  grep -q '"snapshot_digest"' "$workspace/mcp.responses" || exit 39
  printf '%s\n' '{{"type":"thread.started","thread_id":"thread-fixture"}}'
  printf '%s\n' '{{"type":"item.completed","item":{{"type":"agent_message","text":"the result is written"}}}}'
  printf '%s\n' '{{"type":"item.completed","item":{{"type":"mcp_tool_call","server":"ymp","tool":"submit","status":"completed","arguments":{{"command_id":"fixture.submit"}},"result":{{"committed":true}},"error":null}}}}'
  printf '%s\n' '{{"type":"turn.completed","usage":{{"input_tokens":11,"cached_input_tokens":4,"output_tokens":5,"reasoning_output_tokens":1}}}}'
fi
"##,
        pinned = ymp_runtime_codex::PINNED_CODEX_VERSION
    )
}
