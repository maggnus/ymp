#![forbid(unsafe_code)]

//! Acceptance: the shipped product carries one participant attempt from an authorized contract to
//! a terminal outcome and writes the evidence out, through its own surface and nothing else.
//!
//! Everything here drives the built `ymp` executable. The runtime profile is a fixture placed on
//! the child's search path, so the path under test is the product's — its own probe, its own
//! admission of the programs a launch enters, its own controller, its own verifier and its own
//! export — while the agent itself is deterministic and spends nothing.
//!
//! Each half fails on its own.
//!
//! * A profile that cannot do the work stops the attempt. The command names the profile it was
//!   given, records nothing, and never routes the work to the profile that is ready instead.
//! * The attempt that does run reaches a terminal outcome the journal carries: the candidate is
//!   submitted by the agent through the product's own bridge, judged by the verifier the contract
//!   names, and recorded as accepted. The export then carries the exact candidate, the verifier
//!   evidence, the environment each piece of evidence was bound to, and the runtime evidence.
//! * A verifier that answers nothing ends the run as an infrastructure condition, with the
//!   candidate committed and no verdict recorded against it.
//! * A candidate the verifier rejects, with nothing left in the budget to try again with, ends the
//!   run as exhausted.
//!
//! What this does not cover: the two real profiles, whose live runs are recorded separately;
//! cancelling a working attempt, which `cancelling_a_live_attempt.rs` drives in process because
//! the store's writer belongs to whichever process holds the attempt; and failures of the runtime
//! itself, which `codex_product_path.rs` covers at the controller.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;
use ymp_domain::RunStatus;
use ymp_tui::Session;

/// The version the shipped Codex profile is pinned to. A fixture that answered anything else
/// would be refused by the product's own probe, which is the point of stating it here.
const PINNED: &str = "codex-cli 0.147.0";

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
        Self::with_verifier("#!/bin/sh\ntest -f \"$1/result.txt\"\n")
    }

    /// The same project under a stated acceptance condition.
    fn with_verifier(program: &str) -> Self {
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

        // The acceptance condition. It must reject the negative control — a copy of the project
        // as it stands — or the product refuses to draft a contract from it at all.
        let verifier = base.join("verify.sh");
        executable(&verifier, program);
        executable(&search_path.join("codex"), CODEX_FIXTURE);

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

    /// The request, as the lines an operator types.
    fn lines(&self) -> [String; 4] {
        [
            "write the result of the work".to_owned(),
            format!("source {}", self.source.display()),
            format!("verifier {}", self.verifier.display()),
            format!("negative control {}", self.negative_control.display()),
        ]
    }

    fn request_arguments(&self) -> Vec<String> {
        vec![
            "--prompt=write the result of the work".to_owned(),
            format!("--source={}", self.source.display()),
            format!("--verifier={}", self.verifier.display()),
            format!("--negative-control={}", self.negative_control.display()),
        ]
    }

    /// Run the built product against one store.
    ///
    /// The child sees only the fixture profile: its search path holds the fixture and the system
    /// directories the run's own lifecycle utilities live in, and its home directory is empty, so
    /// no profile installed on this host can be reached and the outcome is the same wherever this
    /// runs.
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

    /// The contract identifier the same request derives, read from the interface's own session
    /// over a store of its own. The derivation is content-addressed, so the store the commands act
    /// on carries the same contract.
    ///
    /// The run identifier is not derived here. A run is identified by its store as well as by its
    /// contract, so the run of this session is not the run the commands act on; that one is read
    /// from the store it was started in.
    fn contract_id(&self) -> String {
        let mut session = Session::open(&self.store("identifiers"), &[]);
        for line in self.lines() {
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

/// Every file under a directory, as paths relative to it and in a stated order.
fn tree(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory).expect("read directory").flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            found.push(
                path.strip_prefix(root)
                    .expect("path under the root")
                    .display()
                    .to_string(),
            );
        }
    }
    found.sort();
    found
}

fn journal(store: &Path) -> String {
    fs::read_to_string(store.join("events.jsonl")).expect("committed journal")
}

fn status(store: &Path) -> RunStatus {
    Session::open(store, &[])
        .projection(None)
        .run
        .expect("the store holds a run")
        .status
}

/// The run this store holds. It is read from the store, because the store is part of what
/// identifies the run it holds.
fn run_id(store: &Path) -> String {
    Session::open(store, &[])
        .projection(None)
        .run
        .expect("the store holds a run")
        .run_id
}

/// Start the run this fixture describes, without launching anything.
/// State the decision the engine registry holds, the way an operator states it.
///
/// The Codex engine is held back where no operator says otherwise, and every check here drives a
/// Codex fixture of its own rather than the owner's account. The decision is therefore stated
/// through the same command an operator uses, so what these checks exercise is the product with an
/// admitted engine and not a product that ignores the registry.
fn admit_codex(fixture: &Fixture, store: &Path) {
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
}

/// Start the run and return the identifier the store gives it.
fn start(fixture: &Fixture, store: &Path, contract_id: &str) -> String {
    admit_codex(fixture, store);
    let mut arguments = fixture.request_arguments();
    arguments.insert(0, "start".to_owned());
    arguments.push(format!("--confirm={contract_id}"));
    let started = fixture.command(store, &arguments);
    assert!(
        started.status.success(),
        "the run was not started: {}",
        reported(&started)
    );
    assert!(
        !journal(store).contains("\"type\":\"attempt_started\""),
        "starting the run started an agent, which is a separate authorization"
    );
    run_id(store)
}

#[test]
fn a_profile_that_cannot_do_the_work_stops_the_attempt_and_is_not_replaced() {
    let fixture = Fixture::new();
    let store = fixture.store("named-profile");
    let contract_id = fixture.contract_id();
    let run_id = start(&fixture, &store, &contract_id);

    let refused = fixture.command(
        &store,
        &[
            "attempt".to_owned(),
            "--runtime=claude-code".to_owned(),
            format!("--confirm={run_id}"),
        ],
    );
    let reported = reported(&refused);
    assert!(
        !refused.status.success(),
        "an attempt was launched on a profile this host cannot start: {reported}"
    );
    assert!(
        reported.contains("claude-code"),
        "the refusal does not name the profile it was given: {reported}"
    );
    assert!(
        reported.contains("nothing else is used in its place"),
        "the refusal does not state that no other profile is used: {reported}"
    );
    let journal = journal(&store);
    assert!(
        !journal.contains("\"type\":\"attempt_started\"")
            && !journal.contains("\"type\":\"candidate_submitted\""),
        "a refused attempt reached the journal: {journal}"
    );
    assert_eq!(status(&store), RunStatus::Running);
}

#[test]
fn the_attempt_is_done_judged_and_exported_through_the_product_alone() {
    let fixture = Fixture::new();
    let store = fixture.store("complete");
    let contract_id = fixture.contract_id();
    let run_id = start(&fixture, &store, &contract_id);

    let attempted = fixture.command(
        &store,
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

    // The journal is what the run is: the attempt, the candidate the agent submitted through the
    // product's own bridge, and the verdict of the verifier the contract names.
    //
    // Each is named in the form the run's own events take. The same journal also carries the facts
    // of the run's commitment kernel, and two of those facts — the attempt the kernel starts and
    // the verdict it records — carry the same names under a tag of their own, so a check that read
    // the name alone would no longer say which of the two records it found.
    let journal = journal(&store);
    for fact in [
        "\"type\":\"attempt_started\"",
        "\"type\":\"candidate_submitted\"",
        "\"type\":\"verification_recorded\"",
    ] {
        assert!(
            journal.contains(fact),
            "the journal carries no {fact}: {journal}"
        );
    }
    assert!(
        journal.contains("\"accepted\":true"),
        "the candidate was not accepted: {journal}"
    );
    assert_eq!(status(&store), RunStatus::Accepted);

    // The attempt's own record of what it started, kept beside the journal.
    let evidence = store.join("runtime-evidence");
    let attempt = fs::read_dir(&evidence)
        .expect("runtime evidence directory")
        .next()
        .expect("one attempt of runtime evidence")
        .expect("readable attempt directory")
        .path();
    assert!(attempt.join("profile.json").is_file());
    assert!(attempt.join("events.jsonl").is_file());

    // An export exists to leave the store, so it is written where the operator names it.
    let export = fixture.store("evidence");
    let exported = fixture.command(
        &store,
        &["export".to_owned(), format!("--to={}", export.display())],
    );
    assert!(
        exported.status.success(),
        "nothing was exported: {}",
        reported(&exported)
    );
    for part in [
        "manifest.json",
        "events.jsonl",
        "state.json",
        "candidate-manifest.json",
    ] {
        assert!(export.join(part).is_file(), "the export carries no {part}");
    }
    // The exported candidate is the tree the agent produced, not a description of it.
    assert_eq!(
        fs::read_to_string(export.join("candidate/result.txt")).expect("exported candidate"),
        "done\n"
    );
    let evidence: Vec<_> = fs::read_dir(export.join("evidence"))
        .expect("exported evidence")
        .flatten()
        .collect();
    assert_eq!(
        evidence.len(),
        1,
        "the export carries no verifier evidence for the verdict the journal records"
    );
    let environments: Vec<_> = fs::read_dir(export.join("environments"))
        .expect("exported environments")
        .flatten()
        .collect();
    assert_eq!(
        environments.len(),
        1,
        "the export carries evidence bound to an environment it does not carry"
    );
    assert!(
        export.join("runtime-evidence").is_dir(),
        "the export carries no record of the runtime that produced the candidate"
    );

    // The other form the same export takes: the accepted candidate applied where the operator
    // works. What lands there is the candidate's files and nothing about the run, which is what
    // separates this from the bundle written just above.
    let project = fixture.store("project");
    let applied = fixture.command(
        &store,
        &[
            "export".to_owned(),
            "--apply".to_owned(),
            format!("--to={}", project.display()),
        ],
    );
    assert!(
        applied.status.success(),
        "the candidate was not applied: {}",
        reported(&applied)
    );
    assert_eq!(
        fs::read_to_string(project.join("result.txt")).expect("applied candidate"),
        "done\n"
    );
    assert_eq!(
        tree(&project),
        tree(&export.join("candidate")),
        "the project directory holds something other than the candidate's files"
    );

    // A file the project already holds is a refusal that names it, and the file is untouched.
    fs::write(project.join("result.txt"), b"the operator's own work\n").expect("project file");
    let refused = fixture.command(
        &store,
        &[
            "export".to_owned(),
            "--apply".to_owned(),
            format!("--to={}", project.display()),
        ],
    );
    assert!(
        !refused.status.success(),
        "applying over a project file was not refused: {}",
        reported(&refused)
    );
    assert!(
        reported(&refused).contains("result.txt"),
        "the refusal does not name the file that stopped it: {}",
        reported(&refused)
    );
    assert_eq!(
        fs::read_to_string(project.join("result.txt")).expect("project file"),
        "the operator's own work\n",
        "a refused application changed a project file"
    );

    // Stated, the replacement is made.
    let replaced = fixture.command(
        &store,
        &[
            "export".to_owned(),
            "--apply".to_owned(),
            "--overwrite".to_owned(),
            format!("--to={}", project.display()),
        ],
    );
    assert!(
        replaced.status.success(),
        "the stated replacement did not happen: {}",
        reported(&replaced)
    );
    assert_eq!(
        fs::read_to_string(project.join("result.txt")).expect("applied candidate"),
        "done\n"
    );
}

/// A verifier that cannot decide leaves the run undecided, and the run says so in its own
/// vocabulary. Nothing about the candidate was established, so the candidate is neither accepted
/// nor rejected — the condition is the machinery's.
#[test]
fn a_verifier_that_cannot_decide_is_an_infrastructure_condition_and_not_a_rejection() {
    let fixture = Fixture::new();
    let store = fixture.store("verifier-gone");
    let contract_id = fixture.contract_id();
    let run_id = start(&fixture, &store, &contract_id);
    // The contract names this exact program. Removing it after the authorization is the plainest
    // way to reach a verifier that answers nothing at all.
    fs::remove_file(&fixture.verifier).expect("remove the verifier the contract names");

    let attempted = fixture.command(
        &store,
        &[
            "attempt".to_owned(),
            "--runtime=codex".to_owned(),
            format!("--confirm={run_id}"),
        ],
    );
    let reported = reported(&attempted);
    assert!(
        !attempted.status.success(),
        "a run nothing could judge reported success: {reported}"
    );
    assert!(
        reported.contains("neither accepted nor rejected"),
        "the condition is not stated as its own kind: {reported}"
    );
    let journal = journal(&store);
    assert!(
        journal.contains("\"type\":\"candidate_submitted\""),
        "the agent's candidate was not committed: {journal}"
    );
    // The run's own verdict, in the form the run's events take. What the kernel records under the
    // same name is a fact of the commitment ledger, and it states the condition rather than a
    // decision about the candidate: the verifier answered nothing, and the ledger says so.
    assert!(
        !journal.contains("\"type\":\"verification_recorded\""),
        "a verdict was recorded by a verifier that never answered: {journal}"
    );
    assert_eq!(status(&store), RunStatus::InfrastructureError);
}

/// A candidate the verifier rejects, with nothing left in the budget to try again with, ends the
/// run as exhausted — the terminal a run with nothing left to try actually reaches.
#[test]
fn a_rejected_candidate_with_no_attempt_left_exhausts_the_run() {
    // The acceptance condition asks for a result this agent does not write, and rejects the
    // negative control for the same reason, so it discriminates and is taken.
    let fixture = Fixture::with_verifier("#!/bin/sh\ntest -f \"$1/never-written.txt\"\n");
    let store = fixture.store("rejected");
    let contract_id = fixture.contract_id();
    let run_id = start(&fixture, &store, &contract_id);

    let attempted = fixture.command(
        &store,
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
    let journal = journal(&store);
    assert!(
        journal.contains("\"accepted\":false"),
        "the candidate was not rejected: {journal}"
    );
    assert!(
        journal.contains("attempt budget exhausted"),
        "a run with nothing left to try stayed live: {journal}"
    );
    assert_eq!(status(&store), RunStatus::Exhausted);
}

/// A Codex build that does the work of this fixture: it writes the result into its workspace and
/// submits it through the product's own bridge, which is the only way a candidate reaches the
/// journal.
const CODEX_FIXTURE: &str = r##"#!/bin/sh
if [ "$1" = "--version" ]; then
  printf '%s\n' 'codex-cli 0.147.0'
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
        bridge=${argument#mcp_servers.ymp.command=}
        bridge=${bridge#\"}
        bridge=${bridge%\"}
        ;;
    esac
    previous=$argument
  done
  test -n "$bridge" || exit 31
  test -n "$workspace" || exit 32
  cat >/dev/null
  printf '%s\n' 'done' > "$workspace/result.txt"
  {
    printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}'
    printf '%s\n' '{"jsonrpc":"2.0","method":"notifications/initialized"}'
    printf '%s\n' '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"submit","arguments":{"command_id":"fixture.submit"}}}'
  } | "$bridge" internal agent-mcp > "$workspace/mcp.responses"
  grep -q '"snapshot_digest"' "$workspace/mcp.responses" || exit 39
  printf '%s\n' '{"type":"thread.started","thread_id":"thread-fixture"}'
  printf '%s\n' '{"type":"item.completed","item":{"type":"agent_message","text":"the result is written"}}'
  printf '%s\n' '{"type":"item.completed","item":{"type":"mcp_tool_call","server":"ymp","tool":"submit","status":"completed","arguments":{"command_id":"fixture.submit"},"result":{"committed":true},"error":null}}'
  printf '%s\n' '{"type":"turn.completed","usage":{"input_tokens":11,"cached_input_tokens":4,"output_tokens":5,"reasoning_output_tokens":1}}'
fi
"##;

/// The fixture answers the version the shipped profile is pinned to. Stating it twice — once in
/// the program, once here — is what makes a change to the pin fail this check rather than silently
/// turn the run below into a refusal.
#[test]
fn the_fixture_answers_the_version_the_shipped_profile_requires() {
    assert!(CODEX_FIXTURE.contains(PINNED));
    assert_eq!(PINNED, ymp_runtime_codex::PINNED_CODEX_VERSION);
}
