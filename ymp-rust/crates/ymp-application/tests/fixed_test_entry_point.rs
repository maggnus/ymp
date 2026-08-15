#![forbid(unsafe_code)]

//! Acceptance: a verifier cannot be rewritten by the candidate it judges.
//!
//! The verifier this product proposes runs the project's own test entry point, which is a file
//! inside the directory being judged. Nothing else in a contract is under the candidate's hand, so
//! that one file is where a candidate can decide its own run: replace `scripts/test.sh` with
//! `exit 0` and every candidate is accepted, including one that holds no work at all.
//!
//! Choosing that file has two ways in, and each has its negative half kept here as a program.
//! `delegating_without_fixed_bytes` is the shape this product wrote first: it runs whatever the
//! candidate keeps at that path, and accepts a candidate that wrote `exit 0` there.
//! `fixing_bytes_but_not_the_name` is the shape that followed: it fixes what the file holds but
//! lets `make test` pick which file to read, and accepts a candidate that kept the fixed `Makefile`
//! byte for byte and added a `GNUmakefile`. The proposed verifier settles both, so both candidates
//! are rejected before any tests run, while a candidate that left the file alone and did the work
//! is still accepted.
//!
//! One file is fixed and no more.
//! `what_the_fixed_entry_point_runs_in_turn_stays_the_candidates_own` measures where that ends: a
//! candidate that keeps the entry point byte for byte and rewrites the script it sources is
//! accepted. The draft states that boundary rather than claiming the acceptance condition is
//! altogether beyond the candidate's reach.
//!
//! Two ways of running tests are recognised and not proposed from: a makefile the command would
//! not read, and npm, whose scripts are run by a program the candidate chooses. Both are refused by
//! name and reason, because an operator told their project runs no tests has no way to see that
//! stating a verifier of their own is what is left to them. A test script this host would not run
//! is refused the same way, by its own name and the permission it lacks, rather than by the name
//! of whatever recognised file happens to come next.

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

/// A project that runs its tests through make, which is where the name a command reads first is a
/// second way into acceptance.
fn makefile_project() -> Project {
    let project = bare_project();
    fs::write(
        project.directory.join("Makefile"),
        b"test:\n\ttest -f result.txt\n",
    )
    .expect("project file");
    project
}

/// A directory with nothing in it yet, so a test can put in exactly what it is about.
fn bare_project() -> Project {
    let root = TempDir::new().expect("temporary root");
    let directory = root.path().canonicalize().expect("resolve root");
    let source = directory.join("project");
    fs::create_dir_all(&source).expect("project directory");
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

    /// The verifier this product wrote when it fixed the bytes of a makefile but still let `make`
    /// choose which file to read.
    fn fixing_bytes_but_not_the_name(&self) -> PathBuf {
        let program = self.root.join("named-by-make.sh");
        executable(
            &program,
            "#!/bin/sh\ncd \"$1\" || exit 1\nentry='Makefile'\napproved=$(cat <<'FIXED'\n\
             test:\n\ttest -f result.txt\nFIXED\n)\npresent=$(cat -- \"$entry\" 2>/dev/null) || \
             present=\nif [ \"$approved\" != \"$present\" ]; then exit 1; fi\nmake test\n\
             if [ $? -eq 0 ]; then exit 0; fi\nexit 1\n",
        );
        program
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
        &assembled.substitution_controls,
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

/// Every rejection the draft demonstrates before it is shown, through the run's own executor.
#[test]
fn the_assembled_draft_demonstrates_every_rejection_it_states() {
    let project = project();
    let assembled = project.assembled();
    refuses_a_substituted_entry_point_within(
        &assembled.program,
        &assembled.negative_control,
        &assembled.substitution_controls,
        limit(),
    )
    .expect("the proposed verifier rejects the copy holding no result and the rewritten candidate");
}

/// Fixing what a file holds says nothing about which file is read. `make` reads `GNUmakefile`
/// before `makefile` before `Makefile`, so a candidate that keeps the fixed `Makefile` byte for
/// byte and adds a `GNUmakefile` whose `test` target succeeds has still chosen what decides it.
#[test]
fn a_candidate_that_added_a_file_the_command_reads_first_is_rejected() {
    let project = makefile_project();
    let assembled = project.assembled();

    // The candidate the review measured accepted: the fixed file untouched, the work not done, and
    // a name make reads in preference to it.
    let shadowed = project.root.join("shadowed");
    fs::create_dir_all(&shadowed).expect("candidate directory");
    fs::copy(
        project.directory.join("Makefile"),
        shadowed.join("Makefile"),
    )
    .expect("the fixed file");
    fs::write(shadowed.join("GNUmakefile"), b"test:\n\ttrue\n").expect("the file make reads first");

    // The half that fails without the fix, kept as a program: bytes fixed, `make test` left to
    // choose its own file, and the candidate above accepted on the strength of that choice.
    assert!(
        accepts(&project.fixing_bytes_but_not_the_name(), &shadowed),
        "the shape this product wrote before no longer accepts the candidate the review measured \
         it accepting, so the half this rework must fix is not being measured"
    );

    assert!(
        !accepts(&assembled.program, &shadowed),
        "a candidate that added GNUmakefile beside the fixed Makefile was accepted"
    );

    // Both controls of the same measurement: without that file the same candidate is rejected for
    // holding no work, and with the work done it is accepted.
    let plain = project.root.join("plain");
    fs::create_dir_all(&plain).expect("candidate directory");
    fs::copy(project.directory.join("Makefile"), plain.join("Makefile")).expect("the fixed file");
    assert!(
        !accepts(&assembled.program, &plain),
        "a candidate holding no work was accepted"
    );
    fs::write(plain.join("result.txt"), b"done\n").expect("the work");
    assert!(
        accepts(&assembled.program, &plain),
        "a candidate that did the work and touched nothing else was rejected"
    );

    // And the draft demonstrates it rather than only claiming it.
    refuses_a_substituted_entry_point_within(
        &assembled.program,
        &assembled.negative_control,
        &assembled.substitution_controls,
        limit(),
    )
    .expect("the proposed verifier rejects every candidate the product builds for it");
    assert_eq!(
        assembled.substitution_controls.len(),
        3,
        "the demonstration does not cover both the rewrite and each name read in preference"
    );
}

/// A project that runs its tests in a way the product does not fix is told which file stands in
/// the way, not told it runs no tests.
///
/// The operator can then state a verifier of their own, which they cannot do if the product
/// reports the project as running nothing. Two ways of running tests reach this: a makefile the
/// command would not read, and npm, whose scripts are run by a program the candidate chooses.
#[test]
fn a_way_of_running_tests_the_product_does_not_fix_is_refused_by_name_and_reason() {
    let makefile = makefile_project();
    fs::write(makefile.directory.join("GNUmakefile"), b"test:\n\ttrue\n").expect("a second file");
    let refusal = assemble(&makefile.directory, &makefile.workspace)
        .expect_err("a verifier was proposed from a file the command would not read");
    assert!(
        matches!(refusal, AnswerError::TestEntryPointNotSupported { .. }),
        "{refusal}"
    );
    let refusal = refusal.to_string();
    for named in [
        "Makefile",
        "GNUmakefile",
        "reads in preference",
        "state a verifier of your own",
    ] {
        assert!(
            refusal.contains(named),
            "the refusal does not name {named}: {refusal}"
        );
    }

    let npm = bare_project();
    fs::write(
        npm.directory.join("package.json"),
        b"{\"scripts\":{\"test\":\"test -f result.txt\"}}\n",
    )
    .expect("a project that runs its tests through npm");
    let refusal = assemble(&npm.directory, &npm.workspace)
        .expect_err("a verifier was proposed from a file that does not decide what runs");
    assert!(
        matches!(refusal, AnswerError::TestEntryPointNotSupported { .. }),
        "{refusal}"
    );
    let refusal = refusal.to_string();
    for named in [
        "package.json",
        "files the candidate carries",
        "state a verifier of your own",
    ] {
        assert!(
            refusal.contains(named),
            "the refusal does not name {named}: {refusal}"
        );
    }
    assert!(
        !refusal.contains("no test entry point"),
        "a project that plainly runs tests is reported as running none: {refusal}"
    );
}

/// A test script this host would not run is itself named as the obstacle, with the permission it
/// lacks.
///
/// This is the measurement the card was opened on, kept as the half that fails without the fix.
/// Before it, a `scripts/test.sh` without execute permission was passed over in silence and the
/// refusal named the next recognised file — a `package.json` the operator did not write their
/// tests into — which sends them to fix a file that was never the obstacle.
#[cfg(unix)]
#[test]
fn a_test_script_that_cannot_be_executed_is_named_with_the_permission_it_lacks() {
    let project = bare_project();
    fs::create_dir_all(project.directory.join("scripts")).expect("project directory");
    fs::write(
        project.directory.join("scripts/test.sh"),
        b"#!/bin/sh\ntest -f result.txt\n",
    )
    .expect("a test script nobody made executable");
    fs::write(
        project.directory.join("package.json"),
        b"{\"scripts\":{\"test\":\"test -f result.txt\"}}\n",
    )
    .expect("a second recognised file, further down the order");

    let refusal = assemble(&project.directory, &project.workspace)
        .expect_err("a verifier was proposed from a file this host will not run");
    assert!(
        matches!(refusal, AnswerError::TestEntryPointNotExecutable { .. }),
        "{refusal}"
    );
    let refusal = refusal.to_string();
    for named in [
        "scripts/test.sh",
        "./scripts/test.sh",
        "permission to be executed",
        "execute permission",
    ] {
        assert!(
            refusal.contains(named),
            "the refusal does not name {named}: {refusal}"
        );
    }
    assert!(
        !refusal.contains("package.json"),
        "the refusal names a file the operator did not write their tests into: {refusal}"
    );
}

/// A project that runs no tests this product recognises is still told exactly that, so the refusal
/// above is about the way tests are run and not about every project.
#[test]
fn a_project_that_runs_no_tests_this_product_recognises_is_told_so() {
    let project = bare_project();
    fs::write(project.directory.join("README.md"), b"a project\n").expect("project file");
    let refusal = assemble(&project.directory, &project.workspace)
        .expect_err("nothing can be proposed from a project that runs no tests");
    assert!(
        matches!(refusal, AnswerError::NoTestEntryPoint(_)),
        "{refusal}"
    );
}

/// The boundary the draft states, measured: one file is fixed, and what that file runs in turn is
/// the candidate's own. A candidate that keeps the entry point byte for byte, rewrites the script
/// it sources and does no work is accepted — which is why the draft says so instead of claiming
/// the acceptance condition is beyond the candidate's reach.
#[test]
fn what_the_fixed_entry_point_runs_in_turn_stays_the_candidates_own() {
    let project = project();
    executable(
        &project.directory.join("scripts/test.sh"),
        "#!/bin/sh\n. ./scripts/lib.sh\ncheck\n",
    );
    fs::write(
        project.directory.join("scripts/lib.sh"),
        b"check() { test -f result.txt; }\n",
    )
    .expect("the file the entry point sources");
    let assembled = project.assembled();

    let second = project.root.join("second-file");
    fs::create_dir_all(second.join("scripts")).expect("candidate directory");
    fs::copy(
        project.directory.join("scripts/test.sh"),
        second.join("scripts/test.sh"),
    )
    .expect("the entry point, byte for byte");
    executable(
        &second.join("scripts/test.sh"),
        "#!/bin/sh\n. ./scripts/lib.sh\ncheck\n",
    );
    fs::write(second.join("scripts/lib.sh"), b"check() { true; }\n").expect("the second file");

    assert!(
        accepts(&assembled.program, &second),
        "a candidate that rewrote the file its test entry point sources was rejected, so the \
         boundary the draft states is narrower than the program's and the statement is wrong"
    );
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
