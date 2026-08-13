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

use tempfile::TempDir;
use ymp_application::answer::{
    AnswerError, discriminates, negative_control_directory, source_directory, verifier_program,
};
use ymp_application::{AcceptanceCondition, RunRequest, prepare_contract};

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
