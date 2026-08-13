//! Taking one typed answer where it is typed.
//!
//! A request is assembled from answers, and each answer names something on this host. Resolving
//! them only at assembly means the operator answers every remaining question before learning that
//! the first answer was unusable, and the refusal then names a path typed several questions ago.
//! The functions here resolve one answer at a time, so a refusal arrives at the answer that caused
//! it.
//!
//! [`discriminates`] does more than resolve a path. It asks the verifier to decide the negative
//! control, through the same executor a run uses, and requires it to reject. A program that
//! accepts a deliberately wrong candidate decides nothing, so it cannot enter a contract however
//! well its path resolves. Refusing such an answer is not the same as supplying a usable one: the
//! acceptance condition still comes from the operator and is never invented here.
//!
//! Assembly keeps its own validation in [`crate::contract::prepare_contract`]: a contract also
//! arrives from a package that nobody typed, and what is stored is judged there.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use thiserror::Error;
use ymp_domain::VerificationDecision;
use ymp_domain::contract::{default_output_limit_bytes, default_wall_time_ms};
use ymp_domain::digest_bytes;
use ymp_verifier::{CommandVerifier, VerifierError};

/// Why an answer was not taken. Each case names the answer and what this host said about it.
#[derive(Debug, Error)]
pub enum AnswerError {
    #[error("the source directory {path} could not be read: {reason}")]
    Source { path: PathBuf, reason: String },
    #[error("the source path is not a directory: {0}")]
    SourceNotADirectory(PathBuf),
    #[error(
        "the verifier program {path} could not be read: {reason}. An answer is one path and not a \
         command line, so arguments belong inside the program"
    )]
    VerifierProgram { path: PathBuf, reason: String },
    #[error("the verifier program is not an executable file: {0}")]
    VerifierNotExecutable(PathBuf),
    #[error("the negative control {path} could not be read: {reason}")]
    NegativeControl { path: PathBuf, reason: String },
    #[error("the negative control is not a directory: {0}")]
    NegativeControlNotADirectory(PathBuf),
    #[error(
        "the verifier program {program} accepted the negative control {negative_control}, so it \
         decides nothing: a program that accepts a deliberately wrong candidate cannot judge a run"
    )]
    VerifierAcceptsNegativeControl {
        program: PathBuf,
        negative_control: PathBuf,
    },
    #[error(
        "the verifier program {program} could not be run against the negative control \
         {negative_control}: {reason}"
    )]
    VerifierNotRun {
        program: PathBuf,
        negative_control: PathBuf,
        reason: String,
    },
    #[error(
        "the verifier program {program} rejected {positive_sample}, a sample built to satisfy it, \
         so it decides nothing: a program that rejects every candidate can never accept the work \
         either"
    )]
    VerifierRejectsPositiveSample {
        program: PathBuf,
        positive_sample: PathBuf,
    },
    #[error(
        "no test entry point was found in {0}: ymp proposes a verifier from the way a project \
         already runs its tests, and this directory names none it can run"
    )]
    NoTestEntryPoint(PathBuf),
    #[error(
        "the source directory {path} was not copied: it holds more than {limit} — state a \
         negative control of your own instead"
    )]
    SourceTooLarge { path: PathBuf, limit: String },
    #[error("the draft could not be assembled under {path}: {reason}")]
    Workspace { path: PathBuf, reason: String },
}

/// The directory the work is done in, resolved on this host.
pub fn source_directory(answer: &Path) -> Result<PathBuf, AnswerError> {
    let path = answer.canonicalize().map_err(|error| AnswerError::Source {
        path: answer.to_path_buf(),
        reason: error.to_string(),
    })?;
    if !path.is_dir() {
        return Err(AnswerError::SourceNotADirectory(path));
    }
    Ok(path)
}

/// The program that decides a candidate, resolved on this host.
///
/// The answer is a path. A command line with arguments, a directory and a name that resolves to
/// nothing all fail here, because none of them is a file this host can execute.
pub fn verifier_program(answer: &Path) -> Result<PathBuf, AnswerError> {
    let path = answer
        .canonicalize()
        .map_err(|error| AnswerError::VerifierProgram {
            path: answer.to_path_buf(),
            reason: error.to_string(),
        })?;
    let executable = path.is_file()
        && is_executable(&path).map_err(|error| AnswerError::VerifierProgram {
            path: path.clone(),
            reason: error.to_string(),
        })?;
    if !executable {
        return Err(AnswerError::VerifierNotExecutable(path));
    }
    Ok(path)
}

/// The directory holding the deliberately wrong candidate, resolved on this host.
pub fn negative_control_directory(answer: &Path) -> Result<PathBuf, AnswerError> {
    let path = answer
        .canonicalize()
        .map_err(|error| AnswerError::NegativeControl {
            path: answer.to_path_buf(),
            reason: error.to_string(),
        })?;
    if !path.is_dir() {
        return Err(AnswerError::NegativeControlNotADirectory(path));
    }
    Ok(path)
}

/// Run the named verifier against the negative control and require it to reject.
///
/// The executor is the one a run uses, so what is demonstrated here is what would decide the run
/// rather than a lighter imitation of it. That executor judges the negative control before the
/// candidate, so passing the negative control as both asks the same directory twice: once as the
/// control the program must reject, and once as the subject whose decision must also be rejection.
/// A program that accepts it either time cannot decide a run.
///
/// Both paths must already be resolved by [`verifier_program`] and [`negative_control_directory`].
/// Nothing is stored: the evidence this produces names no contract and is discarded.
///
/// The check is bounded: it applies the limit the contract would apply to the same program, so a
/// program that never returns ends in a refusal that names the limit instead of holding whoever
/// asked for the check. Running it away from the thread that draws is the caller's decision, and
/// the interface makes it in `ymp-tui`.
pub fn discriminates(program: &Path, negative_control: &Path) -> Result<(), AnswerError> {
    discriminates_within(
        program,
        negative_control,
        Duration::from_millis(default_wall_time_ms()),
    )
}

/// [`discriminates`] under a stated limit, which is the limit the contract carries.
pub fn discriminates_within(
    program: &Path,
    negative_control: &Path,
    wall_limit: Duration,
) -> Result<(), AnswerError> {
    let refused = |reason: String| AnswerError::VerifierNotRun {
        program: program.to_path_buf(),
        negative_control: negative_control.to_path_buf(),
        reason,
    };
    let accepted = || AnswerError::VerifierAcceptsNegativeControl {
        program: program.to_path_buf(),
        negative_control: negative_control.to_path_buf(),
    };

    let verifier = demonstration_verifier(program, negative_control, wall_limit)?;
    let subject_digest = demonstration_digest(program, negative_control);

    match verifier.verify_candidate(negative_control, negative_control, subject_digest) {
        Ok(evidence) => match evidence.decision() {
            VerificationDecision::Reject => Ok(()),
            VerificationDecision::Accept => Err(accepted()),
        },
        Err(VerifierError::NegativeControlPassed) => Err(accepted()),
        Err(error) => Err(refused(error.to_string())),
    }
}

/// The executor a demonstration runs under: the one a run uses, bound to the program itself.
fn demonstration_verifier(
    program: &Path,
    negative_control: &Path,
    wall_limit: Duration,
) -> Result<CommandVerifier, AnswerError> {
    let refused = |reason: String| AnswerError::VerifierNotRun {
        program: program.to_path_buf(),
        negative_control: negative_control.to_path_buf(),
        reason,
    };
    let oracle_digest =
        digest_bytes(&fs::read(program).map_err(|error| refused(error.to_string()))?);
    CommandVerifier::new(
        demonstration_digest(program, negative_control),
        oracle_digest,
        program,
        Vec::new(),
        wall_limit,
        default_output_limit_bytes(),
    )
    .map_err(|error| refused(error.to_string()))
}

/// A demonstration precedes every contract, so there is no contract digest to stamp its evidence
/// with. This names the demonstration on this host, and the evidence never leaves.
fn demonstration_digest(program: &Path, negative_control: &Path) -> String {
    digest_bytes(
        format!(
            "negative control demonstration\u{0}{}\u{0}{}",
            program.display(),
            negative_control.display()
        )
        .as_bytes(),
    )
}

// ---------------------------------------------------------------------------
// What the product supplies so nobody has to answer for it
// ---------------------------------------------------------------------------

/// How a project already runs its tests.
///
/// The operator knows this and should not have to state it. What ymp can find, it proposes; what
/// it cannot find, it says it cannot find, and it never invents an acceptance condition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestEntryPoint {
    /// The file that names how tests are run, relative to the source.
    pub relative_path: PathBuf,
    /// The command that runs them, from inside the directory being judged.
    pub command: String,
    /// A version of that file under which the command succeeds. It is what makes a program that
    /// rejects everything visible: such a program rejects this too.
    stub: String,
    stub_is_executable: bool,
}

/// The entry points ymp proposes a verifier from, in the order it looks for them.
///
/// Each one is a file whose content decides what the command does, so the product can build a
/// sample the command must succeed on. An entry point ymp cannot build that sample for is not
/// proposed at all: a proposal whose acceptance nobody demonstrated is what this list exists to
/// avoid.
fn candidates() -> Vec<(&'static str, &'static str, &'static str, bool)> {
    vec![
        (
            "scripts/test.sh",
            "./scripts/test.sh",
            "#!/bin/sh\nexit 0\n",
            true,
        ),
        ("test.sh", "./test.sh", "#!/bin/sh\nexit 0\n", true),
        (
            "scripts/verify.sh",
            "./scripts/verify.sh",
            "#!/bin/sh\nexit 0\n",
            true,
        ),
        ("verify.sh", "./verify.sh", "#!/bin/sh\nexit 0\n", true),
        ("Makefile", "make test", "test:\n\ttrue\n", false),
        (
            "package.json",
            "npm test --silent",
            "{\"scripts\":{\"test\":\"exit 0\"}}\n",
            false,
        ),
    ]
}

/// The way this project runs its tests, when ymp recognises one.
pub fn detect_test_entry_point(source: &Path) -> Option<TestEntryPoint> {
    for (relative, command, stub, executable) in candidates() {
        let path = source.join(relative);
        if !path.is_file() {
            continue;
        }
        if relative == "Makefile" && !names_a_target(&path, "test") {
            continue;
        }
        if relative == "package.json" && !names_a_script(&path, "test") {
            continue;
        }
        if executable && !is_executable(&path).unwrap_or(false) {
            continue;
        }
        return Some(TestEntryPoint {
            relative_path: PathBuf::from(relative),
            command: command.to_owned(),
            stub: stub.to_owned(),
            stub_is_executable: executable,
        });
    }
    None
}

fn names_a_target(makefile: &Path, target: &str) -> bool {
    fs::read_to_string(makefile).is_ok_and(|text| {
        text.lines()
            .any(|line| line.starts_with(&format!("{target}:")))
    })
}

fn names_a_script(package: &Path, script: &str) -> bool {
    let Ok(text) = fs::read_to_string(package) else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return false;
    };
    value
        .get("scripts")
        .and_then(|scripts| scripts.get(script))
        .is_some()
}

/// Everything a drafted contract needs that the operator would otherwise have been asked for.
#[derive(Clone, Debug)]
pub struct Assembled {
    /// The proposed verifier: a program ymp writes, which runs this project's own test entry
    /// point inside whatever directory it is given to judge.
    pub program: PathBuf,
    /// A copy of the source as it stands, which holds no result and must therefore be rejected.
    pub negative_control: PathBuf,
    /// A directory built so that the project's test entry point succeeds in it. A program that
    /// rejects it rejects everything, and is refused rather than carried into a contract.
    pub positive_sample: PathBuf,
    pub entry_point: TestEntryPoint,
}

/// How much of a source directory is copied before the copy is refused instead.
const COPY_ENTRY_LIMIT: usize = 20_000;
const COPY_BYTE_LIMIT: u64 = 256 * 1024 * 1024;

/// Directories a clean copy of a project never needs: history, build output and fetched
/// dependencies are rebuilt, not judged.
const NOT_COPIED: [&str; 5] = [".git", "target", "node_modules", ".ymp-data", ".venv"];

/// Assemble what the product can supply for a request against this source.
///
/// Everything is written under `workspace`, which the caller owns: the proposed verifier, the
/// copy that serves as the negative control, and the sample the verifier must accept. Nothing is
/// run here and nothing is stored durably; [`demonstrates_within`] decides whether what was
/// assembled can judge anything, and the operator decides whether it may.
pub fn assemble(source: &Path, workspace: &Path) -> Result<Assembled, AnswerError> {
    let entry_point = detect_test_entry_point(source)
        .ok_or_else(|| AnswerError::NoTestEntryPoint(source.into()))?;
    let failed = |reason: String| AnswerError::Workspace {
        path: workspace.to_path_buf(),
        reason,
    };

    fs::create_dir_all(workspace).map_err(|error| failed(error.to_string()))?;

    let program = workspace.join("verify.sh");
    // The subject is the only argument the executor passes. The wrapper judges that directory
    // and nothing else, and reports the one refusal the executor understands whatever the
    // project's own command reports, so a project that exits 2 is a rejection rather than a
    // verifier that could not be run.
    write_program(
        &program,
        &format!(
            "#!/bin/sh\ncd \"$1\" || exit 1\n{}\nif [ $? -eq 0 ]; then exit 0; fi\nexit 1\n",
            entry_point.command
        ),
    )
    .map_err(|error| failed(error.to_string()))?;

    let negative_control = copy_negative_control(source, workspace)?;

    let positive_sample = workspace.join("positive-sample");
    let stub = positive_sample.join(&entry_point.relative_path);
    if let Some(parent) = stub.parent() {
        fs::create_dir_all(parent).map_err(|error| failed(error.to_string()))?;
    }
    if entry_point.stub_is_executable {
        write_program(&stub, &entry_point.stub).map_err(|error| failed(error.to_string()))?;
    } else {
        fs::write(&stub, &entry_point.stub).map_err(|error| failed(error.to_string()))?;
    }

    Ok(Assembled {
        program,
        negative_control,
        positive_sample,
        entry_point,
    })
}

fn write_program(path: &Path, body: &str) -> std::io::Result<()> {
    fs::write(path, body)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

/// The negative control the product supplies: a copy of the project as it stands.
///
/// It holds no result, because the work has not been done, so a verifier that can decide the
/// work must reject it. The copy is taken here and now, every time: the caller owns the
/// workspace and gets one workspace per assembly, so what is demonstrated is the project at the
/// moment the draft was assembled. A copy kept from an earlier assembly would be demonstrated
/// against a state the project has left, and stated as its current one.
///
/// Once the run starts, that copy is what it is judged against for as long as it lasts — the
/// directory the operator keeps working in is not that state.
pub fn copy_negative_control(source: &Path, workspace: &Path) -> Result<PathBuf, AnswerError> {
    let failed = |reason: String| AnswerError::Workspace {
        path: workspace.to_path_buf(),
        reason,
    };
    let negative_control = workspace.join("negative-control");
    if negative_control.exists() {
        fs::remove_dir_all(&negative_control).map_err(|error| failed(error.to_string()))?;
    }
    fs::create_dir_all(workspace).map_err(|error| failed(error.to_string()))?;
    copy_tree(source, &negative_control)?;
    Ok(negative_control)
}

/// Copy a project as it stands, bounded and without following links.
fn copy_tree(source: &Path, destination: &Path) -> Result<(), AnswerError> {
    let failed = |reason: String| AnswerError::Workspace {
        path: destination.to_path_buf(),
        reason,
    };
    let mut entries = 0usize;
    let mut bytes = 0u64;
    let mut pending = vec![(source.to_path_buf(), destination.to_path_buf())];
    fs::create_dir_all(destination).map_err(|error| failed(error.to_string()))?;

    while let Some((from, into)) = pending.pop() {
        for entry in fs::read_dir(&from).map_err(|error| failed(error.to_string()))? {
            let entry = entry.map_err(|error| failed(error.to_string()))?;
            let name = entry.file_name();
            if NOT_COPIED
                .iter()
                .any(|skipped| name.to_string_lossy() == *skipped)
            {
                continue;
            }
            let kind = entry
                .file_type()
                .map_err(|error| failed(error.to_string()))?;
            if kind.is_symlink() {
                continue;
            }
            entries += 1;
            if entries > COPY_ENTRY_LIMIT {
                return Err(AnswerError::SourceTooLarge {
                    path: source.to_path_buf(),
                    limit: format!("{COPY_ENTRY_LIMIT} files"),
                });
            }
            let target = into.join(&name);
            if kind.is_dir() {
                fs::create_dir_all(&target).map_err(|error| failed(error.to_string()))?;
                pending.push((entry.path(), target));
            } else {
                bytes += entry.metadata().map(|data| data.len()).unwrap_or(0);
                if bytes > COPY_BYTE_LIMIT {
                    return Err(AnswerError::SourceTooLarge {
                        path: source.to_path_buf(),
                        limit: format!("{} MiB", COPY_BYTE_LIMIT / (1024 * 1024)),
                    });
                }
                fs::copy(entry.path(), &target).map_err(|error| failed(error.to_string()))?;
            }
        }
    }
    Ok(())
}

/// Both halves of what a verifier has to show before it can decide a run.
///
/// The executor judges the negative control before the subject, so one call demonstrates both:
/// the program must reject the copy that holds no result, and accept a sample built to satisfy
/// it. A program that fails the first accepts a wrong candidate; a program that fails the second
/// rejects every candidate, and neither decides anything.
pub fn demonstrates_within(
    program: &Path,
    negative_control: &Path,
    positive_sample: &Path,
    wall_limit: Duration,
) -> Result<(), AnswerError> {
    let refused = |reason: String| AnswerError::VerifierNotRun {
        program: program.to_path_buf(),
        negative_control: negative_control.to_path_buf(),
        reason,
    };
    let verifier = demonstration_verifier(program, negative_control, wall_limit)?;
    let subject_digest = demonstration_digest(program, negative_control);

    match verifier.verify_candidate(positive_sample, negative_control, subject_digest) {
        Ok(evidence) => match evidence.decision() {
            VerificationDecision::Accept => Ok(()),
            VerificationDecision::Reject => Err(AnswerError::VerifierRejectsPositiveSample {
                program: program.to_path_buf(),
                positive_sample: positive_sample.to_path_buf(),
            }),
        },
        Err(VerifierError::NegativeControlPassed) => {
            Err(AnswerError::VerifierAcceptsNegativeControl {
                program: program.to_path_buf(),
                negative_control: negative_control.to_path_buf(),
            })
        }
        Err(error) => Err(refused(error.to_string())),
    }
}

#[cfg(unix)]
pub(crate) fn is_executable(path: &Path) -> std::io::Result<bool> {
    use std::os::unix::fs::PermissionsExt;

    Ok(fs::metadata(path)?.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
pub(crate) fn is_executable(path: &Path) -> std::io::Result<bool> {
    Ok(path.is_file())
}
