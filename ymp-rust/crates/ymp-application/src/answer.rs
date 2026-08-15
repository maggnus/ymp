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
//! Fixing bytes is only worth anything where the file it fixes is what decides the run. A project
//! runs its tests through a script or through make, and both are proposed from; a project that runs
//! them through npm is recognised and refused by name, because npm takes the program that runs each
//! script from files the candidate carries. The refusal names the file and the reason: an operator
//! told their project runs no tests has no way to see that stating a verifier of their own is what
//! is left to them.
//!
//! The same holds for a script this host would not run. A test script carrying no execute
//! permission is the obstacle, and the search stops there rather than passing on to whatever file
//! comes next in the order: a refusal naming a file the operator did not write their tests into
//! sends them to fix something that was never wrong.
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
        "the verifier program {program} accepted {substitution_control}, which carries no result: \
         it is {stated}, so a candidate that can choose the condition it is judged by decides its \
         own run"
    )]
    VerifierAcceptsSubstitutedEntryPoint {
        program: PathBuf,
        substitution_control: PathBuf,
        /// What that candidate did, so the refusal names the way in rather than only the path.
        stated: String,
    },
    #[error(
        "no test entry point was found in {0}: ymp proposes a verifier from the way a project \
         already runs its tests, and this directory names none it can run"
    )]
    NoTestEntryPoint(PathBuf),
    #[error(
        "this project runs its tests through {path}, and ymp does not propose a verifier from it: \
         {reason}. A proposal it could not hold the candidate to would be one the candidate \
         decides, so nothing is proposed — state a verifier of your own instead"
    )]
    TestEntryPointNotSupported { path: PathBuf, reason: String },
    #[error(
        "this project runs its tests through {path}, and this host will not run it: the command \
         would be {command}, and the file carries no permission to be executed. The file is the \
         obstacle and nothing further was looked for — grant it execute permission, or state a \
         verifier of your own instead"
    )]
    TestEntryPointNotExecutable { path: PathBuf, command: String },
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
    /// The names that, standing beside the fixed file, this command reads in preference to it.
    /// `make` reads `GNUmakefile`, then `makefile`, then `Makefile`, so a candidate that leaves the
    /// fixed file untouched and adds one of these has still chosen what decides it. Fixing bytes
    /// says nothing about which file is read, which is why these are named rather than assumed
    /// away.
    shadowed_by: &'static [&'static str],
}

/// One way a project can be recognised as running its tests.
struct Candidate {
    relative: &'static str,
    /// What the product would propose from this, or why it will not. A way of running tests it
    /// will not propose from is still recognised, because a project that runs its tests that way
    /// is owed the name of what stands in the way rather than being told it runs no tests at all.
    proposal: Result<Proposal, &'static str>,
}

/// What a proposed verifier would run, and what it must refuse before running it.
struct Proposal {
    /// The command, written so it reads the fixed file and not whichever file the command would
    /// otherwise pick. `make -f Makefile test` is that: `make test` alone reads a name the
    /// candidate can add.
    command: &'static str,
    substitution: &'static str,
    executable: bool,
    shadowed_by: &'static [&'static str],
}

/// What this project runs its tests through, and whether the product will propose a verifier
/// from it.
#[derive(Clone, Debug)]
pub enum TestEntry {
    /// A file whose bytes can be fixed into a program, run by a command that reads that file and
    /// no other.
    Supported(TestEntryPoint),
    /// A way of running tests the product recognises and does not propose from. Naming it is the
    /// whole point: the operator learns which file stands in the way and can state a verifier of
    /// their own, rather than being told this project runs no tests.
    Unsupported {
        relative_path: PathBuf,
        reason: String,
    },
    /// A file the product would propose from, which this host would not run: the command is the
    /// file itself, and the file carries no permission to be executed. Looking past it would leave
    /// the operator a refusal naming some other file, while the one they wrote their tests into
    /// stands unmentioned and one `chmod` away from working.
    NotExecutable {
        relative_path: PathBuf,
        command: String,
    },
}

/// The ways of running tests ymp recognises, in the order it looks for them.
///
/// A proposed one is a file whose content decides what the command does, which is why the proposed
/// verifier fixes those bytes and why the product can build the candidate that replaces them. The
/// rest are recognised so they can be refused by name.
fn candidates() -> Vec<Candidate> {
    const SCRIPT_SUBSTITUTION: &str = "#!/bin/sh\nexit 0\n";
    let script = |relative, command| Candidate {
        relative,
        proposal: Ok(Proposal {
            command,
            substitution: SCRIPT_SUBSTITUTION,
            executable: true,
            shadowed_by: &[],
        }),
    };
    vec![
        script("scripts/test.sh", "./scripts/test.sh"),
        script("test.sh", "./test.sh"),
        script("scripts/verify.sh", "./scripts/verify.sh"),
        script("verify.sh", "./verify.sh"),
        Candidate {
            relative: "Makefile",
            proposal: Ok(Proposal {
                command: "make -f Makefile test",
                substitution: "test:\n\ttrue\n",
                executable: false,
                shadowed_by: &["GNUmakefile", "makefile"],
            }),
        },
        Candidate {
            relative: "package.json",
            // Fixing these bytes would fix the text of a script and not the program that runs it:
            // npm takes the shell each script runs in from `.npmrc`, a file of the candidate's, so
            // a candidate carrying `script-shell` chooses what decides it while package.json
            // stands untouched. Beyond that, a verifier here runs with a path holding no npm, so
            // the program would reject every candidate including one that did the work — and a
            // demonstration that asks only for rejections would carry it into a contract anyway.
            // Holding npm to a contract is its own problem and is not solved by writing a command
            // here.
            proposal: Err(
                "npm takes the program that runs each script from files the candidate carries, so \
                 fixing this file does not fix what decides a candidate",
            ),
        },
    ]
}

/// The way this project runs its tests, when ymp recognises one.
pub fn detect_test_entry_point(source: &Path) -> Option<TestEntry> {
    for candidate in candidates() {
        let path = source.join(candidate.relative);
        if !path.is_file() {
            continue;
        }
        if candidate.relative == "Makefile" && !names_a_target(&path, "test") {
            continue;
        }
        if candidate.relative == "package.json" && !names_a_script(&path, "test") {
            continue;
        }
        let unsupported = |reason: String| {
            Some(TestEntry::Unsupported {
                relative_path: PathBuf::from(candidate.relative),
                reason,
            })
        };
        let proposal = match candidate.proposal {
            Ok(proposal) => proposal,
            Err(reason) => return unsupported(reason.to_owned()),
        };
        // A script this host would refuse to run is where the search stops. The project runs its
        // tests through this file, and the thing standing in the way is one permission bit on it;
        // carrying on to the next recognised file would answer a question nobody asked and name a
        // file that is not the obstacle.
        if proposal.executable && !is_executable(&path).unwrap_or(false) {
            return Some(TestEntry::NotExecutable {
                relative_path: PathBuf::from(candidate.relative),
                command: proposal.command.to_owned(),
            });
        }
        // A project that already carries a name this command reads first is not proposed from this
        // file either. A verifier fixed to bytes the command never reads would reject every
        // candidate, including one that did the work, and would say nothing about why.
        if let Some(read_first) = proposal
            .shadowed_by
            .iter()
            .find(|name| names_an_entry(path.parent().unwrap_or(source), name))
        {
            return unsupported(format!(
                "this project also carries {read_first}, which the command reads in preference to \
                 it, so fixing this file would fix one the tests never run"
            ));
        }
        return Some(TestEntry::Supported(TestEntryPoint {
            relative_path: PathBuf::from(candidate.relative),
            command: proposal.command.to_owned(),
            substitution: proposal.substitution.to_owned(),
            substitution_is_executable: proposal.executable,
            shadowed_by: proposal.shadowed_by,
        }));
    }
    None
}

/// Whether this directory holds an entry under exactly this name.
///
/// The name is compared against what the directory reports, not asked of the file system: a host
/// that folds case answers `Makefile` to a question about `makefile`, and a project would then be
/// refused a verifier over a file it does not have.
fn names_an_entry(directory: &Path, name: &str) -> bool {
    let Ok(entries) = fs::read_dir(directory) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        entry
            .file_name()
            .to_str()
            .is_some_and(|present| present == name)
    })
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

/// A candidate the product builds to be accepted without doing the work.
///
/// Each one holds no result, so a verifier that decides the work rejects it. Each one also does
/// one thing a real candidate could do to the file the verifier delegates to. A verifier that
/// accepts any of them is judged by the candidate rather than the other way round.
#[derive(Clone, Debug)]
pub struct SubstitutionControl {
    pub path: PathBuf,
    /// What this candidate did, in the words the draft states it in.
    pub stated: String,
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
    /// Every candidate the product can build that a verifier delegating to a file inside the
    /// candidate would accept if it were not fixed to that file: one that rewrote it, and one per
    /// name the command would read in preference to it. All of them must be rejected before the
    /// proposal is carried into a contract.
    pub substitution_controls: Vec<SubstitutionControl>,
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
/// Everything is written under `workspace`, which the caller owns: the proposed verifier, the copy
/// that serves as the negative control, and one candidate per way the product knows of reaching
/// acceptance through the file the verifier delegates to. Nothing is run here and nothing is stored
/// durably; [`refuses_a_substituted_entry_point_within`] decides whether what was assembled can
/// judge anything, and the operator decides whether it may.
pub fn assemble(source: &Path, workspace: &Path) -> Result<Assembled, AnswerError> {
    let entry_point = match detect_test_entry_point(source) {
        Some(TestEntry::Supported(entry_point)) => entry_point,
        Some(TestEntry::Unsupported {
            relative_path,
            reason,
        }) => {
            return Err(AnswerError::TestEntryPointNotSupported {
                path: source.join(relative_path),
                reason,
            });
        }
        Some(TestEntry::NotExecutable {
            relative_path,
            command,
        }) => {
            return Err(AnswerError::TestEntryPointNotExecutable {
                path: source.join(relative_path),
                command,
            });
        }
        None => return Err(AnswerError::NoTestEntryPoint(source.into())),
    };
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

    // The candidate that rewrites the file the verifier runs.
    let rewritten = workspace.join("substitution-control");
    plant(
        &rewritten.join(&entry_point.relative_path),
        entry_point.substitution.as_bytes(),
        entry_point.substitution_is_executable,
    )
    .map_err(|error| failed(error.to_string()))?;
    let mut substitution_controls = vec![SubstitutionControl {
        path: rewritten,
        stated: format!(
            "a candidate that had replaced {} with a program accepting everything",
            entry_point.relative_path.display()
        ),
    }];

    // And one per name the command reads first: the file stays as the contract found it, byte for
    // byte, and the candidate chooses what runs by the name it gives a second one.
    for shadow in entry_point.shadowed_by {
        let shadowed = workspace.join(format!("shadow-control-{shadow}"));
        let entry = shadowed.join(&entry_point.relative_path);
        plant(&entry, &approved, entry_point.substitution_is_executable)
            .map_err(|error| failed(error.to_string()))?;
        let beside = entry.parent().unwrap_or(&shadowed).join(shadow);
        plant(
            &beside,
            entry_point.substitution.as_bytes(),
            entry_point.substitution_is_executable,
        )
        .map_err(|error| failed(error.to_string()))?;
        substitution_controls.push(SubstitutionControl {
            path: shadowed,
            stated: format!(
                "a candidate that had left {} byte for byte and added {shadow}, which the command \
                 reads in preference to it",
                entry_point.relative_path.display()
            ),
        });
    }

    Ok(Assembled {
        program,
        negative_control,
        substitution_controls,
        entry_point,
    })
}

/// Write one file of a control, creating what it sits in.
fn plant(path: &Path, contents: &[u8], executable: bool) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    if executable {
        make_executable(path)?;
    }
    Ok(())
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
/// Between those two, it settles two separate questions, because fixing what a file holds says
/// nothing about which file is read. It compares the entry point against the bytes that file had in
/// the project the contract was drafted from, and it refuses a candidate carrying a name the
/// command reads in preference to it. Without the first, the program the verifier delegates to is
/// one the candidate writes; without the second, `make test` in a candidate carrying a `GNUmakefile`
/// runs that instead, whatever the fixed `Makefile` holds. Trailing newlines are not part of the
/// comparison, because both sides are read the same way and a shell drops them from both.
///
/// What is fixed is exactly one file. Whatever that file runs in turn — a script it sources, a
/// program a recipe calls — is a file of the candidate's, and the draft says so rather than leaving
/// the operator to assume otherwise.
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
    /// path, its bytes, the name of the block carrying them, the names it must not find beside it,
    /// and the project's own command differ between one proposal and the next.
    const TEMPLATE: &str = r#"#!/bin/sh
# This program decides one contract. It runs the project's own test entry point inside the
# directory it is given, and the bytes that entry point had when the contract was drafted are
# carried below. A candidate holding anything else there, or holding a file the command would
# read in preference to it, is rejected before its tests are run: a candidate that could choose
# either would be deciding its own run.
#
# One file is fixed. Whatever that file runs in turn is the candidate's own.
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
{shadow_check}{command}
if [ $? -eq 0 ]; then exit 0; fi
exit 1
"#;
    /// The names checked one by one against what the directory reports, rather than asked of the
    /// file system: a host that folds case answers `Makefile` to a question about `makefile`, and
    /// every candidate would then be rejected, including one that did the work.
    const SHADOW_CHECK: &str = r#"for shadow in {shadow_names}; do
  if [ -n "$(find {shadow_directory} -maxdepth 1 -name "$shadow" -print 2>/dev/null)" ]; then
    printf '%s would be read in preference to %s\n' "$shadow" "$entry" >&2
    exit 1
  fi
done
"#;
    let shadow_check = if entry_point.shadowed_by.is_empty() {
        String::new()
    } else {
        let directory = entry_point.relative_path.parent().unwrap_or(Path::new(""));
        let directory = match directory.as_os_str().is_empty() {
            true => ".".to_owned(),
            false => directory.to_string_lossy().into_owned(),
        };
        SHADOW_CHECK
            .replace(
                "{shadow_names}",
                &entry_point
                    .shadowed_by
                    .iter()
                    .map(|name| shell_word(name))
                    .collect::<Vec<_>>()
                    .join(" "),
            )
            .replace("{shadow_directory}", &shell_word(&directory))
    };
    // The entry point's own bytes go in last. Everything else is filled from this host, and a file
    // that happened to spell one of these names would otherwise be read as one of them.
    Ok(TEMPLATE
        .replace(
            "{quoted_path}",
            &shell_word(&entry_point.relative_path.to_string_lossy()),
        )
        .replace("{marker}", &marker)
        .replace("{shadow_check}", &shadow_check)
        .replace("{command}", &entry_point.command)
        .replace("{body}", text.strip_suffix('\n').unwrap_or(text)))
}

/// One shell word carrying exactly the given text, whatever it holds.
fn shell_word(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

fn write_program(path: &Path, body: &str) -> std::io::Result<()> {
    fs::write(path, body)?;
    make_executable(path)
}

#[cfg(unix)]
fn make_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> std::io::Result<()> {
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

/// Every decision a verifier proposed from a project's own tests has to make before it is shown.
///
/// The executor judges the negative control before the subject, so each call demonstrates two
/// things: the program rejects the copy of the project that holds no result, and it rejects one
/// candidate that would have reached acceptance through the file the program delegates to. The
/// first is the decision every verifier answers. The rest are what a verifier delegating to a file
/// inside the candidate has to answer as well, one for each way the product knows of choosing that
/// file's content or its name — because a candidate accepted on the strength of either has written
/// its own acceptance condition.
///
/// There is no half here that asks the program to accept something. The product cannot build a
/// candidate the project's own tests pass on without replacing those tests, which is the very
/// substitution these controls refuse, so a program that rejects every candidate is not separated
/// from one whose tests need the work — the draft states what the verifier accepts as
/// undemonstrated instead of showing it with a candidate the product wrote for the purpose. The
/// first candidate that satisfies the project's tests is where acceptance is first shown.
pub fn refuses_a_substituted_entry_point_within(
    program: &Path,
    negative_control: &Path,
    substitution_controls: &[SubstitutionControl],
    wall_limit: Duration,
) -> Result<(), AnswerError> {
    let refused = |reason: String| AnswerError::VerifierNotRun {
        program: program.to_path_buf(),
        negative_control: negative_control.to_path_buf(),
        reason,
    };
    let verifier = demonstration_verifier(program, negative_control, wall_limit)?;

    for control in substitution_controls {
        let subject_digest = demonstration_digest(program, negative_control);
        match verifier.verify_candidate(&control.path, negative_control, subject_digest) {
            Ok(evidence) => match evidence.decision() {
                VerificationDecision::Reject => {}
                VerificationDecision::Accept => {
                    return Err(AnswerError::VerifierAcceptsSubstitutedEntryPoint {
                        program: program.to_path_buf(),
                        substitution_control: control.path.clone(),
                        stated: control.stated.clone(),
                    });
                }
            },
            Err(VerifierError::NegativeControlPassed) => {
                return Err(AnswerError::VerifierAcceptsNegativeControl {
                    program: program.to_path_buf(),
                    negative_control: negative_control.to_path_buf(),
                });
            }
            Err(error) => return Err(refused(error.to_string())),
        }
    }
    Ok(())
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
