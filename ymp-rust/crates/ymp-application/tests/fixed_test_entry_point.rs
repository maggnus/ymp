#![forbid(unsafe_code)]

//! Acceptance: a verifier cannot be rewritten by the candidate it judges.
//!
//! The verifier this product proposes runs the project's own test entry point, which is a file
//! inside the directory being judged. Nothing else in a contract is under the candidate's hand, so
//! that one file is where a candidate can decide its own run: replace `scripts/test.sh` with
//! `exit 0` and every candidate is accepted, including one that holds no work at all.
//!
//! The negative half is kept here as a program: `delegating_without_fixed_bytes` is the shape this
//! product wrote before, and it accepts exactly that candidate. The proposed verifier carries the
//! bytes of the entry point instead, so the same candidate is rejected before its tests are run,
//! while a candidate that left the entry point alone and did the work is still accepted.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tempfile::TempDir;
use ymp_application::answer::{
    AnswerError, Assembled, assemble, discriminates_within,
    refuses_a_substituted_entry_point_within,
};
use ymp_domain::contract::default_wall_time_ms;

struct Project {
    _root: TempDir,
    directory: PathBuf,
    workspace: PathBuf,
    root: PathBuf,
}

/// A project as an operator has one: it runs its tests through a script of its own, and that
/// script fails while the work is not done.
fn project() -> Project {
    let root = TempDir::new().expect("temporary root");
    let directory = root.path().canonicalize().expect("resolve root");
    let source = directory.join("project");
    fs::create_dir_all(source.join("scripts")).expect("project directory");
    fs::write(source.join("README.md"), b"a project\n").expect("project file");
    executable(
        &source.join("scripts/test.sh"),
        "#!/bin/sh\ntest -f result.txt\n",
    );
    Project {
        _root: root,
        workspace: directory.join("draft"),
        root: directory,
        directory: source,
    }
}

impl Project {
    fn assembled(&self) -> Assembled {
        assemble(&self.directory, &self.workspace).expect("the project names a test entry point")
    }

    /// A candidate that holds no work and carries a test entry point accepting everything.
    fn substituted(&self, name: &str) -> PathBuf {
        let candidate = self.root.join(name);
        fs::create_dir_all(candidate.join("scripts")).expect("candidate directory");
        executable(&candidate.join("scripts/test.sh"), "#!/bin/sh\nexit 0\n");
        candidate
    }

    /// A candidate that did the work and left the entry point as the contract found it.
    fn satisfying(&self, name: &str) -> PathBuf {
        let candidate = self.root.join(name);
        fs::create_dir_all(candidate.join("scripts")).expect("candidate directory");
        fs::copy(
            self.directory.join("scripts/test.sh"),
            candidate.join("scripts/test.sh"),
        )
        .expect("copy the entry point");
        executable(
            &candidate.join("scripts/test.sh"),
            "#!/bin/sh\ntest -f result.txt\n",
        );
        fs::write(candidate.join("result.txt"), b"done\n").expect("the work");
        candidate
    }

    /// The verifier this product wrote before the entry point's bytes were fixed into it: it runs
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

fn limit() -> Duration {
    Duration::from_millis(default_wall_time_ms())
}

/// Whether the program accepted this directory, decided by the executor a run uses. The check that
/// resolves a negative control requires a rejection, so its refusal is where an acceptance is
/// reported.
fn accepts(program: &Path, candidate: &Path) -> bool {
    match discriminates_within(program, candidate, limit()) {
        Ok(()) => false,
        Err(AnswerError::VerifierAcceptsNegativeControl { .. }) => true,
        Err(refusal) => panic!("the program decided neither way: {refusal}"),
    }
}

/// The measurement this card was opened on, kept as the half that fails without the fix.
#[test]
fn a_verifier_that_runs_whatever_the_candidate_keeps_there_accepts_a_candidate_holding_no_work() {
    let project = project();
    let delegating = project.delegating_without_fixed_bytes();
    let substituted = project.substituted("substituted");

    assert!(
        accepts(&delegating, &substituted),
        "the shape this product wrote before no longer accepts the candidate it was measured \
         accepting, so the half this card must fix is not being measured"
    );

    // Which is what the draft's own check now refuses to carry into a contract.
    let assembled = project.assembled();
    let refusal = refuses_a_substituted_entry_point_within(
        &delegating,
        &assembled.negative_control,
        &assembled.substitution_control,
        limit(),
    )
    .expect_err("a verifier the candidate can rewrite was carried into a contract");
    assert!(
        matches!(
            refusal,
            AnswerError::VerifierAcceptsSubstitutedEntryPoint { .. }
        ),
        "{refusal}"
    );
    assert!(
        refusal.to_string().contains("decides its own run"),
        "the refusal does not name the reason: {refusal}"
    );
}

/// The outcome: the same candidate is rejected by the verifier the product proposes now.
#[test]
fn a_candidate_that_replaced_its_own_test_entry_point_is_rejected() {
    let project = project();
    let assembled = project.assembled();

    assert!(
        !accepts(&assembled.program, &project.substituted("substituted")),
        "a candidate that holds no work and rewrote the test entry point was accepted"
    );

    // The bytes decide it, not the exit status of the candidate's own script: a candidate whose
    // entry point differs at all is rejected, and one that removed it is rejected too.
    let edited = project.substituted("edited");
    executable(
        &edited.join("scripts/test.sh"),
        "#!/bin/sh\ntest -f result.txt\n#\n",
    );
    fs::write(edited.join("result.txt"), b"done\n").expect("the work");
    assert!(
        !accepts(&assembled.program, &edited),
        "a candidate that did the work but edited the test entry point was accepted"
    );

    let removed = project.substituted("removed");
    fs::remove_file(removed.join("scripts/test.sh")).expect("remove the entry point");
    fs::write(removed.join("result.txt"), b"done\n").expect("the work");
    assert!(
        !accepts(&assembled.program, &removed),
        "a candidate that removed the test entry point was accepted"
    );
}

/// The consequence that keeps the product honest: fixing the bytes must not stop the project's own
/// tests from accepting a candidate that did the work.
#[test]
fn a_candidate_that_left_the_entry_point_alone_and_did_the_work_is_accepted() {
    let project = project();
    let assembled = project.assembled();

    assert!(
        accepts(&assembled.program, &project.satisfying("satisfying")),
        "a candidate carrying the project's own test entry point and the work it asks for was \
         rejected, so the verifier can never accept anything"
    );

    // The copy of the project as it stands holds no result, so the same program rejects it.
    assert!(
        !accepts(&assembled.program, &assembled.negative_control),
        "the copy of the project that holds no result was accepted"
    );
}

/// Both halves the draft demonstrates before it is shown, in one call through the run's executor.
#[test]
fn the_assembled_draft_demonstrates_both_rejections() {
    let project = project();
    let assembled = project.assembled();
    refuses_a_substituted_entry_point_within(
        &assembled.program,
        &assembled.negative_control,
        &assembled.substitution_control,
        limit(),
    )
    .expect("the proposed verifier rejects the copy holding no result and the rewritten candidate");
}

/// An entry point is carried as data. A script that spells shell out of its own body, or names the
/// block that carries it, decides candidates exactly as it did in the project.
#[test]
fn an_entry_point_is_carried_as_bytes_rather_than_as_shell() {
    let project = project();
    executable(
        &project.directory.join("scripts/test.sh"),
        "#!/bin/sh\n# $(rm -rf /) `false` 'quoted' \"double\" \\\\ YMP_APPROVED_ENTRY_0\ntest -f result.txt\n",
    );
    let assembled = project.assembled();
    let satisfying = project.satisfying("satisfying");
    fs::copy(
        project.directory.join("scripts/test.sh"),
        satisfying.join("scripts/test.sh"),
    )
    .expect("carry the same entry point into the candidate");

    assert!(
        accepts(&assembled.program, &satisfying),
        "an entry point holding shell characters no longer decides the candidate it decided in \
         the project"
    );
    assert!(
        !accepts(&assembled.program, &project.substituted("substituted")),
        "an entry point holding shell characters stopped being fixed"
    );
}

/// A verifier is proposed only from bytes this host can carry inside a program. Anything else is
/// said plainly, because a verifier written around bytes it could not carry would be one the
/// candidate rewrites.
#[test]
fn an_entry_point_that_cannot_be_carried_is_refused_rather_than_delegated_to() {
    let project = project();
    fs::write(project.directory.join("scripts/test.sh"), [0x00, 0xff])
        .expect("write an entry point that is not text");
    let refusal = assemble(&project.directory, &project.workspace)
        .expect_err("a verifier was proposed from bytes it could not carry");
    assert!(
        matches!(refusal, AnswerError::TestEntryPointNotFixable { .. }),
        "{refusal}"
    );
    assert!(
        refusal.to_string().contains("decides its own run"),
        "the refusal does not name the reason: {refusal}"
    );
}
