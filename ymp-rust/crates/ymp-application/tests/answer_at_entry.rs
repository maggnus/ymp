#![forbid(unsafe_code)]

//! Acceptance: an answer is judged where it is typed, and a verifier answer is judged by what the
//! program does rather than by what its path looks like.
//!
//! The negative half of the discrimination check is the program that succeeds on anything. It
//! resolves to an existing executable file, so every check that reads only the file system accepts
//! it — `prepare_contract` does, and that is measured here. Only running it against the negative
//! control separates it from a verifier, and `discriminates` must be what does so.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tempfile::TempDir;
use ymp_application::answer::{
    AnswerError, SubstitutionControl, discriminates, discriminates_within,
    negative_control_directory, refuses_a_substituted_entry_point_within, source_directory,
    verifier_program,
};
use ymp_application::{AcceptanceCondition, RunRequest, prepare_contract};
use ymp_domain::contract::default_wall_time_ms;

struct Workspace {
    _root: TempDir,
    root: PathBuf,
    source: PathBuf,
    negative_control: PathBuf,
}

fn workspace() -> Workspace {
    let root = TempDir::new().expect("temporary root");
    let directory = root.path().canonicalize().expect("resolve root");
    let source = directory.join("source");
    let negative_control = directory.join("negative-control");
    fs::create_dir_all(&source).expect("source directory");
    fs::create_dir_all(&negative_control).expect("negative control directory");
    fs::write(source.join("result.txt"), b"result\n").expect("source file");
    Workspace {
        _root: root,
        root: directory,
        source,
        negative_control,
    }
}

impl Workspace {
    fn program(&self, name: &str, body: &str) -> PathBuf {
        let path = self.root.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write program");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(&path).expect("metadata").permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(&path, permissions).expect("make executable");
        }
        path
    }

    /// The request the interface's draft would hand to the application for this verifier.
    fn assembles(&self, program: &Path) -> bool {
        prepare_contract(&RunRequest {
            prompt: "keep the replay path idempotent".to_owned(),
            source: self.source.clone(),
            acceptance: Some(AcceptanceCondition::new(program, &self.negative_control)),
            capture_exclusions: Vec::new(),
            contract_id: None,
            budget: None,
        })
        .is_ok()
    }
}

/// The subject is the only argument the executor passes, so the whole argument list names it.
const DISCRIMINATES: &str = "test -f \"$*/result.txt\"";

#[test]
fn a_program_that_succeeds_on_anything_is_refused_although_assembly_accepts_it() {
    let workspace = workspace();
    let accepts_anything = workspace.program("accept.sh", "exit 0");

    // Assembly reads the file system and finds nothing wrong: the path is an executable file.
    assert!(
        workspace.assembles(&accepts_anything),
        "assembly already refused the program the point of entry has to refuse"
    );

    let refusal = discriminates(&accepts_anything, &workspace.negative_control)
        .expect_err("a program that exits zero on the negative control decides nothing");
    assert!(
        matches!(refusal, AnswerError::VerifierAcceptsNegativeControl { .. }),
        "{refusal}"
    );
    assert!(
        refusal
            .to_string()
            .contains("accepted the negative control"),
        "the refusal does not name the reason: {refusal}"
    );
}

#[test]
fn a_program_that_rejects_the_negative_control_is_taken() {
    let workspace = workspace();
    let verifier = workspace.program("verify.sh", DISCRIMINATES);
    discriminates(&verifier, &workspace.negative_control)
        .expect("a program that rejects the deliberately wrong candidate is a verifier");

    // The same program accepts the source, so the rejection above is a decision and not a program
    // that fails on everything.
    discriminates(&verifier, &workspace.source)
        .expect_err("a directory the program accepts cannot serve as a negative control");
}

/// A program that only ever rejects passes every check the product can put to it, and this is
/// where that is measured rather than assumed.
///
/// An earlier card closed this by asking the program to accept a sample the product built. That
/// sample could only ever be built by replacing the file a proposed verifier delegates to, which is
/// the substitution the product now refuses, so the sample was a forgery and the half that used it
/// is gone. What replaced it asks for more rejections, and a program that rejects everything
/// answers all of them.
///
/// Nothing in the product claims otherwise: a draft states what the verifier accepts as
/// undemonstrated, and the first candidate that satisfies the project's own tests is where
/// acceptance is first shown. This test fails if that boundary moves, which is when the statement
/// has to move with it.
#[test]
fn a_program_that_rejects_every_candidate_answers_every_rejection_the_product_can_ask_for() {
    let workspace = workspace();
    let rejects_everything = workspace.program("reject.sh", "exit 1");
    let limit = Duration::from_millis(default_wall_time_ms());
    let controls = [SubstitutionControl {
        path: workspace.source.clone(),
        stated: "a candidate that had replaced its test entry point".to_owned(),
    }];

    discriminates(&rejects_everything, &workspace.negative_control)
        .expect("a program that rejects everything rejects the negative control too");
    refuses_a_substituted_entry_point_within(
        &rejects_everything,
        &workspace.negative_control,
        &controls,
        limit,
    )
    .expect("a program that rejects everything rejects the substitution controls too");

    // What the check does separate is a program that accepts one of those candidates: the same
    // call refuses it and names what that candidate did.
    let accepts_the_control = workspace.program("verify.sh", DISCRIMINATES);
    let refusal = refuses_a_substituted_entry_point_within(
        &accepts_the_control,
        &workspace.negative_control,
        &controls,
        limit,
    )
    .expect_err("a program that accepts a candidate holding no work was carried into a contract");
    assert!(
        matches!(
            refusal,
            AnswerError::VerifierAcceptsSubstitutedEntryPoint { .. }
        ),
        "{refusal}"
    );
    assert!(
        refusal
            .to_string()
            .contains("had replaced its test entry point"),
        "the refusal does not name what the candidate did: {refusal}"
    );
}

/// A program that never decides is a refusal, not a wait without end.
///
/// The check applies the limit the contract would apply to the same program, so what happens at
/// entry is what would happen to the run: the program is ended and the refusal names the limit.
/// The interface runs this check away from the thread that draws, so the wait it bounds is a
/// wait on that thread and never on the screen.
#[test]
fn a_program_that_never_returns_is_refused_at_the_limit_the_check_carries() {
    let workspace = workspace();
    let never = workspace.program("never.sh", "sleep 30");

    let limit = Duration::from_millis(300);
    let started = Instant::now();
    let refusal = discriminates_within(&never, &workspace.negative_control, limit)
        .expect_err("a program that never decides cannot judge a run");
    let waited = started.elapsed();

    assert!(
        matches!(refusal, AnswerError::VerifierNotRun { .. }),
        "{refusal}"
    );
    assert!(
        refusal.to_string().contains("300"),
        "the refusal does not name the limit it applied: {refusal}"
    );
    assert!(
        waited < Duration::from_secs(5),
        "the check waited {waited:?} for a program it was told to bound at {limit:?}"
    );
}

#[test]
fn a_program_that_decides_nothing_is_refused_with_what_this_host_reported() {
    let workspace = workspace();
    for (name, body) in [
        ("unclassified.sh", "exit 2"),
        ("crashes.sh", "kill -TERM $$"),
    ] {
        let program = workspace.program(name, body);
        let refusal = discriminates(&program, &workspace.negative_control)
            .expect_err("a program with no classified exit cannot decide a run");
        assert!(
            matches!(refusal, AnswerError::VerifierNotRun { .. }),
            "{name}: {refusal}"
        );
    }
}

#[test]
fn a_verifier_answer_that_is_not_an_executable_file_is_refused() {
    let workspace = workspace();
    let verifier = workspace.program("verify.sh", DISCRIMINATES);
    let unreadable = workspace.root.join("plain.sh");
    fs::write(&unreadable, "#!/bin/sh\nexit 1\n").expect("write file");

    // A command line is not a path, and neither is a directory or a name that resolves to nothing.
    for stated in [
        PathBuf::from(format!("{} --strict", verifier.display())),
        workspace.root.join("no-such-program"),
        workspace.source.clone(),
    ] {
        let refusal = verifier_program(&stated).expect_err("this is not an executable file");
        assert!(
            matches!(
                refusal,
                AnswerError::VerifierProgram { .. } | AnswerError::VerifierNotExecutable(_)
            ),
            "{}: {refusal}",
            stated.display()
        );
    }
    #[cfg(unix)]
    assert!(matches!(
        verifier_program(&unreadable).expect_err("a file without execute permission"),
        AnswerError::VerifierNotExecutable(_)
    ));

    assert_eq!(
        verifier_program(&verifier).expect("an executable file resolves"),
        verifier
    );
}

#[test]
fn a_directory_answer_is_refused_unless_it_is_a_directory_on_this_host() {
    let workspace = workspace();
    let verifier = workspace.program("verify.sh", DISCRIMINATES);

    assert!(matches!(
        source_directory(&workspace.root.join("no-such-directory"))
            .expect_err("a directory that does not exist"),
        AnswerError::Source { .. }
    ));
    assert!(matches!(
        source_directory(&verifier).expect_err("a file is not a directory"),
        AnswerError::SourceNotADirectory(_)
    ));
    assert!(matches!(
        negative_control_directory(&verifier).expect_err("a file is not a directory"),
        AnswerError::NegativeControlNotADirectory(_)
    ));
    assert_eq!(
        source_directory(&workspace.source).expect("an existing directory resolves"),
        workspace.source
    );
    assert_eq!(
        negative_control_directory(&workspace.negative_control).expect("an existing directory"),
        workspace.negative_control
    );
}
