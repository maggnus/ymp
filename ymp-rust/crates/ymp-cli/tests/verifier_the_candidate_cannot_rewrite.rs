#![forbid(unsafe_code)]

//! Acceptance, driven on the built product: the acceptance condition of a contract is not the
//! candidate's to rewrite.
//!
//! The verifier `ymp` proposes runs the project's own test entry point, and that entry point is a
//! file inside the directory being judged. A candidate that writes `exit 0` over it would decide
//! its own run, so the proposed program carries the bytes that file had when the contract was
//! drafted and rejects a candidate holding anything else.
//!
//! Every decision here is read through the product itself. `ymp request --verifier … --negative
//! control …` asks the named program to reject the named directory through the executor a run
//! uses, so the command succeeding is that directory being rejected, and the command refusing with
//! `accepted the negative control` is that directory being accepted.
//!
//! The negative half is `a_verifier_that_runs_whatever_the_candidate_keeps_there`: the shape the
//! product wrote before, handed to the same command, which reports it accepting the candidate that
//! holds no work at all.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

const PROMPT: &str = "--prompt=keep the replay path idempotent";

/// What the entry point of a project holds while the work is not done.
const ENTRY_POINT: &str = "#!/bin/sh\ntest -f result.txt\n";

struct Project {
    _root: TempDir,
    root: PathBuf,
    directory: PathBuf,
    data_root: PathBuf,
}

/// A project as an operator has one: it runs its tests through a script of its own, and that
/// script fails while the work is not done.
fn project() -> Project {
    let holder = TempDir::new().expect("temporary root");
    let root = holder.path().canonicalize().expect("resolve root");
    let directory = root.join("work");
    fs::create_dir_all(directory.join("scripts")).expect("project directory");
    fs::write(directory.join("README.md"), b"a project\n").expect("project file");
    executable(&directory.join("scripts/test.sh"), ENTRY_POINT);
    Project {
        _root: holder,
        data_root: root.join("data"),
        directory,
        root,
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

impl Project {
    /// Run one command of the product from inside the project, which is what decides the source.
    fn run(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_ymp"))
            .current_dir(&self.directory)
            .arg("--data-root")
            .arg(&self.data_root)
            .args(arguments)
            .output()
            .expect("run the ymp executable")
    }

    /// The verifier the product proposes for this project, kept where the next assembly cannot
    /// discard it: a draft's workspace belongs to that assembly alone.
    fn proposed_verifier(&self) -> PathBuf {
        let drafted = self.run(&["request", PROMPT]);
        assert!(
            drafted.status.success(),
            "the project was not drafted a contract:\n{}",
            String::from_utf8_lossy(&drafted.stderr)
        );
        let workspace = fs::read_dir(self.data_root.join("draft"))
            .expect("the product wrote a draft workspace")
            .map(|entry| entry.expect("read the draft workspace").path())
            .find(|path| path.join("verify.sh").is_file())
            .expect("the product proposed a verifier");
        let kept = self.root.join("proposed-verify.sh");
        fs::copy(workspace.join("verify.sh"), &kept).expect("keep the proposed verifier");
        executable(
            &kept,
            &fs::read_to_string(&kept).expect("read the proposed verifier"),
        );
        kept
    }

    /// A candidate that holds no work and carries a test entry point accepting everything.
    fn substituted(&self) -> PathBuf {
        let candidate = self.root.join("substituted");
        fs::create_dir_all(candidate.join("scripts")).expect("candidate directory");
        executable(&candidate.join("scripts/test.sh"), "#!/bin/sh\nexit 0\n");
        candidate
    }

    /// A candidate that did the work and left the entry point as the contract found it.
    fn satisfying(&self) -> PathBuf {
        let candidate = self.root.join("satisfying");
        fs::create_dir_all(candidate.join("scripts")).expect("candidate directory");
        executable(&candidate.join("scripts/test.sh"), ENTRY_POINT);
        fs::write(candidate.join("result.txt"), b"done\n").expect("the work");
        candidate
    }

    /// The verifier this product wrote before the entry point's bytes were carried in it: it runs
    /// whatever the candidate keeps at that path.
    fn delegating_without_fixed_bytes(&self) -> PathBuf {
        let program = self.root.join("delegating.sh");
        executable(
            &program,
            "#!/bin/sh\ncd \"$1\" || exit 1\n./scripts/test.sh\nif [ $? -eq 0 ]; then exit 0; fi\n\
             exit 1\n",
        );
        program
    }

    /// Whether the named program accepted the named directory, as the product reports it.
    fn accepts(&self, program: &Path, candidate: &Path) -> bool {
        let output = self.run(&[
            "request",
            PROMPT,
            &format!("--verifier={}", program.display()),
            &format!("--negative-control={}", candidate.display()),
        ]);
        let transcript = String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let refusal = String::from_utf8_lossy(&output.stderr).into_owned();
        if output.status.success() {
            assert!(
                transcript.contains("drafted · digest"),
                "the command succeeded without drafting anything:\n{transcript}"
            );
            return false;
        }
        assert!(
            refusal.contains("accepted the negative control"),
            "the program decided neither way:\n{refusal}"
        );
        true
    }
}

/// The measurement this card was opened on, kept as the half that fails without the fix.
#[test]
fn a_verifier_that_runs_whatever_the_candidate_keeps_there_accepts_a_candidate_holding_no_work() {
    let project = project();
    assert!(
        project.accepts(
            &project.delegating_without_fixed_bytes(),
            &project.substituted()
        ),
        "the shape this product wrote before no longer accepts the candidate it was measured \
         accepting, so the half this card must fix is not being measured"
    );
}

/// The outcome: the verifier the product proposes now rejects that same candidate, and still
/// accepts one that did the work without touching the entry point.
#[test]
fn the_proposed_verifier_rejects_a_candidate_that_rewrote_its_own_test_entry_point() {
    let project = project();
    let proposed = project.proposed_verifier();

    assert!(
        !project.accepts(&proposed, &project.substituted()),
        "a candidate that holds no work and rewrote the test entry point was accepted"
    );
    assert!(
        project.accepts(&proposed, &project.satisfying()),
        "a candidate carrying the project's own test entry point and the work it asks for was \
         rejected, so the verifier can never accept anything"
    );
}
