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
//! A verifier this module proposes runs the project's own test entry point, which is a file inside
//! the directory being judged. The bytes of that file are therefore fixed into the proposed program
//! itself, and a candidate whose copy of it differs is rejected before the tests are run. Without
//! that, a candidate would decide its own run by writing `exit 0` over the script the verifier
//! delegates to. The fixed bytes are part of the program, so the oracle digest a contract records
//! already covers them, and a proposal whose bytes are edited afterwards no longer matches the
//! contract that approved it.
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
        "the verifier program {program} accepted {substitution_control}, a candidate that carries \
         no result and differs from the project only in having replaced its test entry point with \
         a program that accepts everything: a candidate that can rewrite the condition it is \
         judged by decides its own run"
    )]
    VerifierAcceptsSubstitutedEntryPoint {
        program: PathBuf,
        substitution_control: PathBuf,
    },
    #[error(
        "no test entry point was found in {0}: ymp proposes a verifier from the way a project \
         already runs its tests, and this directory names none it can run"
    )]
    NoTestEntryPoint(PathBuf),
    #[error(
        "the test entry point {path} could not be fixed into a proposed verifier: {reason}. A \
         verifier that runs a file inside the candidate is proposed only when the bytes of that \
         file can be fixed into it, because a candidate that may rewrite them decides its own run"
    )]
    TestEntryPointNotFixable { path: PathBuf, reason: String },
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
    /// A version of that file under which the command succeeds whatever the work is. It is what a
    /// candidate would put in place of the entry point to be accepted without doing the work, so
    /// the proposed verifier is required to reject a candidate carrying it.
    substitution: String,
    substitution_is_executable: bool,
}

/// The entry points ymp proposes a verifier from, in the order it looks for them.
///
/// Each one is a file whose content decides what the command does, which is why the proposed
/// verifier fixes those bytes and why the product can build the candidate that replaces them.
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
    for (relative, command, substitution, executable) in candidates() {
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
            substitution: substitution.to_owned(),
            substitution_is_executable: executable,
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
    /// point inside whatever directory it is given to judge, and which carries the bytes that
    /// entry point had here, so the directory being judged cannot supply different ones.
    pub program: PathBuf,
    /// A copy of the source as it stands, which holds no result and must therefore be rejected.
    pub negative_control: PathBuf,
    /// A candidate that holds no result either and differs from the project only in having
    /// replaced its test entry point with a program that accepts everything. A verifier that
    /// accepts it is judged by the candidate rather than the other way round, and is refused
    /// rather than carried into a contract.
    pub substitution_control: PathBuf,
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
/// copy that serves as the negative control, and the candidate that replaced its own test entry
/// point. Nothing is run here and nothing is stored durably;
/// [`refuses_a_substituted_entry_point_within`] decides whether what was assembled can judge
/// anything, and the operator decides whether it may.
pub fn assemble(source: &Path, workspace: &Path) -> Result<Assembled, AnswerError> {
    let entry_point = detect_test_entry_point(source)
        .ok_or_else(|| AnswerError::NoTestEntryPoint(source.into()))?;
    let failed = |reason: String| AnswerError::Workspace {
        path: workspace.to_path_buf(),
        reason,
    };
    let entry_path = source.join(&entry_point.relative_path);
    let not_fixable = |reason: String| AnswerError::TestEntryPointNotFixable {
        path: entry_path.clone(),
        reason,
    };
    let approved = fs::read(&entry_path).map_err(|error| not_fixable(error.to_string()))?;

    fs::create_dir_all(workspace).map_err(|error| failed(error.to_string()))?;

    let program = workspace.join("verify.sh");
    write_program(
        &program,
        &proposed_verifier(&entry_point, &approved).map_err(not_fixable)?,
    )
    .map_err(|error| failed(error.to_string()))?;

    let negative_control = copy_negative_control(source, workspace)?;

    let substitution_control = workspace.join("substitution-control");
    let substituted = substitution_control.join(&entry_point.relative_path);
    if let Some(parent) = substituted.parent() {
        fs::create_dir_all(parent).map_err(|error| failed(error.to_string()))?;
    }
    if entry_point.substitution_is_executable {
        write_program(&substituted, &entry_point.substitution)
            .map_err(|error| failed(error.to_string()))?;
    } else {
        fs::write(&substituted, &entry_point.substitution)
            .map_err(|error| failed(error.to_string()))?;
    }

    Ok(Assembled {
        program,
        negative_control,
        substitution_control,
        entry_point,
    })
}

/// How many characters of the entry point's digest name the quoted block that carries its bytes.
const FIXED_BYTES_MARKER_CHARS: usize = 16;

/// The proposed verifier, as the program text it is written as.
///
/// The subject is the only argument the executor passes. The program judges that directory and
/// nothing else, and reports the one refusal the executor understands whatever the project's own
/// command reports, so a project that exits 2 is a rejection rather than a verifier that could not
/// be run.
///
/// Between those two, it compares the entry point it is about to run against the bytes that file
/// had in the project the contract was drafted from, and rejects a candidate that carries anything
/// else. That comparison is what stops the candidate from deciding its own run: without it, the
/// program the verifier delegates to is one the candidate writes. Trailing newlines are not part of
/// the comparison, because both sides are read the same way and a shell drops them from both.
///
/// The bytes are quoted, not interpreted, so a `$` or a backquote in the entry point is data. The
/// block that carries them is named after their own digest, so no line of an entry point can end
/// the block early — and an entry point that carries such a line anyway is refused rather than
/// quoted wrongly.
fn proposed_verifier(entry_point: &TestEntryPoint, approved: &[u8]) -> Result<String, String> {
    let text = std::str::from_utf8(approved).map_err(|_| {
        "it is not text, and only text can be carried inside a shell program".to_owned()
    })?;
    if text.contains('\0') {
        return Err("it holds a zero byte, which no shell program can carry".to_owned());
    }
    let marker = format!(
        "YMP_APPROVED_ENTRY_{}",
        &digest_bytes(approved)[..FIXED_BYTES_MARKER_CHARS]
    );
    if text.lines().any(|line| line == marker) {
        return Err(format!(
            "it holds the line {marker}, which is what ends the bytes carried in the program"
        ));
    }
    /// The program an operator reads when they open the proposed verifier. Only the entry point's
    /// path, its bytes, the name of the block carrying them, and the project's own command differ
    /// between one proposal and the next.
    const TEMPLATE: &str = r#"#!/bin/sh
# This program decides one contract. It runs the project's own test entry point inside the
# directory it is given, and the bytes that entry point had when the contract was drafted are
# carried below. A candidate holding anything else there is rejected before its tests are run:
# a candidate that could rewrite this file would be deciding its own run.
cd "$1" || exit 1
entry={quoted_path}
approved=$(cat <<'{marker}'
{body}
{marker}
)
present=$(cat -- "$entry" 2>/dev/null) || present=
if [ "$approved" != "$present" ]; then
  printf '%s is not the test entry point this contract was drafted against\n' "$entry" >&2
  exit 1
fi
{command}
if [ $? -eq 0 ]; then exit 0; fi
exit 1
"#;
    // The entry point's own bytes go in last. Everything else is filled from this host, and a file
    // that happened to spell one of these names would otherwise be read as one of them.
    Ok(TEMPLATE
        .replace(
            "{quoted_path}",
            &shell_word(&entry_point.relative_path.to_string_lossy()),
        )
        .replace("{marker}", &marker)
        .replace("{command}", &entry_point.command)
        .replace("{body}", text.strip_suffix('\n').unwrap_or(text)))
}

/// One shell word carrying exactly the given text, whatever it holds.
fn shell_word(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
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

/// Both halves of what a verifier has to show when there is a sample it must accept.
///
/// The executor judges the negative control before the subject, so one call demonstrates both:
/// the program must reject the copy that holds no result, and accept a sample built to satisfy
/// it. A program that fails the first accepts a wrong candidate; a program that fails the second
/// rejects every candidate, and neither decides anything.
///
/// A verifier proposed from a project's own tests is demonstrated by
/// [`refuses_a_substituted_entry_point_within`] instead, because the product has no sample it can
/// build for one: the only candidate it could build that such a program accepts is a candidate
/// whose test entry point it replaced, and the whole point of that program is to reject exactly
/// that. What it accepts is stated as undemonstrated rather than shown by a forgery.
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

/// Both halves of what a verifier proposed from a project's own tests has to show.
///
/// The executor judges the negative control before the subject, so one call demonstrates both: the
/// program must reject the copy of the project that holds no result, and it must reject a candidate
/// that holds no result either and differs only in having replaced the test entry point with a
/// program that accepts everything. The first half is the same one every verifier answers. The
/// second is what a verifier delegating to a file inside the candidate has to answer as well,
/// because a candidate it accepts on the strength of that file has written its own acceptance
/// condition.
///
/// What such a verifier accepts stays undemonstrated here, and the draft says so. The product
/// cannot build a candidate the project's own tests pass on without replacing those tests, which is
/// the very substitution this check refuses; the first candidate that satisfies them is where
/// acceptance is first shown.
pub fn refuses_a_substituted_entry_point_within(
    program: &Path,
    negative_control: &Path,
    substitution_control: &Path,
    wall_limit: Duration,
) -> Result<(), AnswerError> {
    let refused = |reason: String| AnswerError::VerifierNotRun {
        program: program.to_path_buf(),
        negative_control: negative_control.to_path_buf(),
        reason,
    };
    let substituted = || AnswerError::VerifierAcceptsSubstitutedEntryPoint {
        program: program.to_path_buf(),
        substitution_control: substitution_control.to_path_buf(),
    };
    let verifier = demonstration_verifier(program, negative_control, wall_limit)?;
    let subject_digest = demonstration_digest(program, negative_control);

    match verifier.verify_candidate(substitution_control, negative_control, subject_digest) {
        Ok(evidence) => match evidence.decision() {
            VerificationDecision::Reject => Ok(()),
            VerificationDecision::Accept => Err(substituted()),
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
