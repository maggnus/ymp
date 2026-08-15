#![forbid(unsafe_code)]

//! Acceptance: a run carried out through the product records the facts of its commitment kernel in
//! its own journal, and the operator reads them back from that store afterwards.
//!
//! Everything here drives the built `ymp` executable against a store of its own, with a home
//! directory of its own and a runtime fixture on its search path, so the path under test is the
//! product's: its own request, its own authorization, its own controller and its own pages. The
//! operator's store is never touched.
//!
//! Three things are required of the run that finishes. Its journal carries both record types the
//! commitment kernel writes — the genesis a ledger is built from, and the facts each commitment
//! command committed — and those facts name the work this run formed rather than any state a
//! process happened to hold. The result it produced is in that journal as a construction and not
//! only as a seal: the objects it stored whole, the bundle it published them as, and the result
//! those form, each carrying the digests the run's own record states. And `ymp show commitments`,
//! reading that store as an operator reads it, draws the task contract and the obligations under it
//! instead of refusing.
//!
//! The negative half is the tree before the kernel of a run reached the journal: the same run left
//! no such record, and `ymp show commitments` over its store exited non-zero saying the store has
//! no commitments page. Before the live path submitted bundles, the same run left `submission_recorded`
//! and nothing else, so the construction this check reads back was not there to read. Running this
//! check against that tree is what shows it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;
use ymp_domain::commitment::{BundleChange, CommitmentEvent, MAX_BUNDLE_CHANGES, PathChange};
use ymp_domain::{EventEnvelope, EventKind, ProvenanceLimit};
use ymp_tui::Session;

/// What the runtime fixture writes into its workspace, and where. The kernel's record of the
/// result is read back against these exact bytes.
const RESULT_PATH: &str = "result.txt";
const RESULT_BYTES: &[u8] = b"done\n";

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
        Self::writing(0)
    }

    /// The same fixture whose runtime writes `extra` further files into its workspace beside the
    /// result, so that what the run produces changes that many paths more.
    fn writing(extra: usize) -> Self {
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
        executable(&search_path.join("codex"), &codex_fixture(extra));

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

/// The records of the run's journal, as the store holds them.
fn records(store: &Path) -> Vec<EventEnvelope> {
    journal(store)
        .lines()
        .map(|line| serde_json::from_str(line).expect("a committed record"))
        .collect()
}

/// Every commitment fact of this run, in the order its commands committed them.
fn commitment_facts(store: &Path) -> Vec<CommitmentEvent> {
    records(store)
        .into_iter()
        .filter_map(|envelope| match envelope.event {
            EventKind::CommitmentFactsRecorded { facts } => Some(facts),
            _ => None,
        })
        .flatten()
        .collect()
}

/// What the run's own record says its attempt produced: the base it was started from and the
/// snapshot its submission committed.
fn submitted_candidate(store: &Path) -> (String, String) {
    records(store)
        .into_iter()
        .find_map(|envelope| match envelope.event {
            EventKind::CandidateSubmitted {
                base_digest,
                object_digest,
                ..
            } => Some((base_digest, object_digest)),
            _ => None,
        })
        .expect("the run committed a candidate")
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
    // A run is created against the pool it may draw its models from, so this store's root is put
    // into the state one measured account leaves behind — on the engine the attempt routes
    // through, so that resolving the pool enables no second profile beside the fixture.
    ymp_testkit::ready_root::measured_engine(
        store,
        ymp_runtime_registry::Engine::Codex,
        &["gpt-5-codex"],
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
        "\"event\":\"object_recorded\"",
        "\"event\":\"bundle_recorded\"",
        "\"event\":\"candidate_formed\"",
        "\"event\":\"submission_recorded\"",
    ] {
        assert!(
            journal.contains(fact),
            "the journal carries no {fact}: {journal}"
        );
    }

    // The result this run produced is in the journal as the construction that produced it. The
    // digests are the run's own: the base its attempt started from, and the exact bytes its
    // runtime wrote into the workspace.
    let facts = commitment_facts(&store);
    let (base_digest, _) = submitted_candidate(&store);
    let result = ymp_domain::digest_bytes(RESULT_BYTES);
    assert!(
        facts.iter().any(|fact| matches!(
            fact,
            CommitmentEvent::ObjectRecorded { object_digest } if *object_digest == result
        )),
        "the object the result puts at its path was never recorded whole: {facts:?}"
    );
    let (bundle_digest, bundle_base, parents, changes) = facts
        .iter()
        .find_map(|fact| match fact {
            CommitmentEvent::BundleRecorded {
                bundle_digest,
                base_digest,
                parents,
                changes,
            } => Some((
                bundle_digest.clone(),
                base_digest.clone(),
                parents.clone(),
                changes.clone(),
            )),
            _ => None,
        })
        .expect("the run published no bundle");
    assert_eq!(
        bundle_base, base_digest,
        "the bundle names a base the run was not started from"
    );
    assert!(
        parents.is_empty(),
        "a first attempt carried a result forward: {parents:?}"
    );
    assert!(
        changes.contains(&BundleChange {
            path: RESULT_PATH.to_owned(),
            change: PathChange::Upsert {
                object_digest: result.clone(),
                executable: false,
            },
        }),
        "the bundle does not put the bytes the runtime wrote at the path it wrote them: {changes:?}"
    );
    let candidate = facts
        .iter()
        .find_map(|fact| match fact {
            CommitmentEvent::CandidateFormed {
                candidate_digest,
                base_digest,
                bundle_digest,
                changes,
                ..
            } => Some((
                candidate_digest.clone(),
                base_digest.clone(),
                bundle_digest.clone(),
                changes.clone(),
            )),
            _ => None,
        })
        .expect("the run formed no result from its bundle");
    assert_eq!(candidate.1, base_digest, "the result names another base");
    assert_eq!(
        candidate.2, bundle_digest,
        "the result names a bundle this run never published"
    );
    assert!(
        candidate.3.iter().any(|change| change.path == RESULT_PATH),
        "the result does not carry the path its bundle changed: {:?}",
        candidate.3
    );
    assert!(
        facts.iter().any(|fact| matches!(
            fact,
            CommitmentEvent::SubmissionRecorded { candidate_digest, .. }
                if *candidate_digest == candidate.0
        )),
        "the work was sealed with something other than the result it formed"
    );
    // And the verdict a protected query produced reached that construction, which is what closes
    // the work this run is accountable for.
    assert!(
        facts.iter().any(|fact| matches!(
            fact,
            CommitmentEvent::VerificationRecorded { candidate_digest, .. }
                if *candidate_digest == candidate.0
        )),
        "the verdict was recorded against something other than the result the work sealed: {facts:?}"
    );
    assert!(
        facts
            .iter()
            .any(|fact| matches!(fact, CommitmentEvent::ObligationReturned { .. })),
        "the work of a judged run was left open: {facts:?}"
    );

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

/// A run through the product whose result the commitment kernel cannot state is judged like any
/// other, and the page an operator reads says what is missing from it.
///
/// The runtime writes more files than one bundle may carry, which is ordinary work: a bound on how
/// much one submission states at once belongs to the record and not to the task. So the run reaches
/// its verdict exactly as the run above does, its work is sealed with the result its journal names,
/// and no fact about a construction is in the kernel. What the record adds is the one thing a seal
/// on its own cannot say — that the construction is missing and which bound left it out — and
/// `ymp show commitments` states it where an operator reads the run's commitments.
#[test]
fn a_run_whose_construction_the_kernel_cannot_state_is_still_judged_and_says_so() {
    let fixture = Fixture::writing(MAX_BUNDLE_CHANGES);
    let store = fixture.store("bounded");
    attempt_through_the_product(&fixture, &store);

    let facts = commitment_facts(&store);
    let (_, candidate) = submitted_candidate(&store);
    assert!(
        !facts.iter().any(|fact| matches!(
            fact,
            CommitmentEvent::ObjectRecorded { .. }
                | CommitmentEvent::BundleRecorded { .. }
                | CommitmentEvent::CandidateFormed { .. }
        )),
        "a construction the kernel could not state left facts about itself behind: {facts:?}"
    );
    assert!(
        facts.iter().any(|fact| matches!(
            fact,
            CommitmentEvent::SubmissionRecorded { candidate_digest, .. }
                if *candidate_digest == candidate
        )),
        "the work carries no result, so a protected query had nothing to be spent on: {facts:?}"
    );
    assert!(
        facts.iter().any(|fact| matches!(
            fact,
            CommitmentEvent::VerificationRecorded { candidate_digest, .. }
                if *candidate_digest == candidate
        )),
        "the result was never judged: {facts:?}"
    );

    // The run says what its kernel could not state, once, naming the result and the bound.
    let notes: Vec<_> = records(&store)
        .into_iter()
        .filter_map(|envelope| match envelope.event {
            EventKind::CandidateProvenanceUnrecorded {
                candidate_digest,
                reason,
                protocol_rule,
                ..
            } => Some((candidate_digest, reason, protocol_rule)),
            _ => None,
        })
        .collect();
    assert_eq!(notes.len(), 1, "the run states it {} times", notes.len());
    assert_eq!(notes[0].0, candidate, "the note names another result");
    assert!(
        matches!(notes[0].1, ProvenanceLimit::TooManyChanges { changes } if changes > MAX_BUNDLE_CHANGES as u64),
        "the note states another bound: {:?}",
        notes[0].1
    );
    assert!(
        notes[0].2.contains(&MAX_BUNDLE_CHANGES.to_string()),
        "the note carries no rule for the bound it states: {}",
        notes[0].2
    );

    // And an operator reads it off that store, on the page the run's commitments are shown on.
    let shown = fixture.command(&store, &["show".to_owned(), "commitments".to_owned()]);
    let page = reported(&shown);
    assert!(
        shown.status.success(),
        "the commitments of a finished run could not be shown: {page}"
    );
    assert!(
        page.contains("1 task contract"),
        "the page states no task contract for a run that formed one: {page}"
    );
    assert!(
        page.contains("is not in this kernel") && page.contains("too_many_changes"),
        "the page of a run whose construction was not recorded does not say so: {page}"
    );
}

/// A Codex build that does the work of this fixture: it writes the result into its workspace and
/// submits it through the product's own bridge, which is the only way a candidate reaches the
/// journal. It answers the version the shipped profile is pinned to, read from the profile itself
/// so that a change to the pin cannot leave this fixture silently refused.
fn codex_fixture(extra: usize) -> String {
    let further = (0..extra)
        .map(|index| format!("  printf '%s\\n' 'done' > \"$workspace/file-{index}.txt\""))
        .collect::<Vec<_>>()
        .join("\n");
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
{further}
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
        pinned = ymp_runtime_codex::PINNED_CODEX_VERSION,
        further = further
    )
}
