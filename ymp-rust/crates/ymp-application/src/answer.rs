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
pub fn discriminates(program: &Path, negative_control: &Path) -> Result<(), AnswerError> {
    let refused = |reason: String| AnswerError::VerifierNotRun {
        program: program.to_path_buf(),
        negative_control: negative_control.to_path_buf(),
        reason,
    };
    let accepted = || AnswerError::VerifierAcceptsNegativeControl {
        program: program.to_path_buf(),
        negative_control: negative_control.to_path_buf(),
    };

    let oracle_digest =
        digest_bytes(&fs::read(program).map_err(|error| refused(error.to_string()))?);
    // The check precedes every contract, so there is no contract digest to stamp its evidence
    // with. The digest below names this check on this host, and the evidence never leaves.
    let subject_digest = digest_bytes(
        format!(
            "negative control demonstration\u{0}{}\u{0}{}",
            program.display(),
            negative_control.display()
        )
        .as_bytes(),
    );
    let verifier = CommandVerifier::new(
        subject_digest.clone(),
        oracle_digest,
        program,
        Vec::new(),
        Duration::from_millis(default_wall_time_ms()),
        default_output_limit_bytes(),
    )
    .map_err(|error| refused(error.to_string()))?;

    match verifier.verify_candidate(negative_control, negative_control, subject_digest) {
        Ok(evidence) => match evidence.decision() {
            VerificationDecision::Reject => Ok(()),
            VerificationDecision::Accept => Err(accepted()),
        },
        Err(VerifierError::NegativeControlPassed) => Err(accepted()),
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
