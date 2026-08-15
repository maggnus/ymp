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
//! A project that runs no tests at all is not out of reach either. [`generate`] derives the check
//! from the request instead: it reads out of the request's own words the one artifact the work is
//! asked to leave behind, and writes a self-contained program that decides that and nothing more.
//! A derived check is weaker than a project's own tests by construction, so what it decides and
//! what it leaves to the operator are both stated, and it takes effect only through the same
//! approval every other acceptance condition needs.
//!
//! The derivation here is a fixed rule over the words of the request — a production verb and the
//! artifact it names — and nothing else. No model is asked to write the program, so no part of the
//! request and no part of the project leaves this host to produce it, and a request the rule reads
//! no artifact out of is refused rather than guessed at. A richer derivation, which would send the
//! request to a supplier under a disclosure budget, is a product decision that has not been taken;
//! until it is, this rule is the whole of what generation does, and the draft says so.
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
        "nothing in the request \"{prompt}\" names a result this host could check for: it asks for \
         no artifact ymp knows how to look for, and this project runs no tests to propose a \
         verifier from. A check invented from words that state no observable result would decide \
         nothing, so none is offered — state a verifier of your own, or say what the work must \
         leave behind"
    )]
    NoCheckDerivable { prompt: String },
    #[error(
        "the check generated from this request — {claim} — is already satisfied by \
         {negative_control}, which is this project as it stands, so it says nothing about the work \
         being asked for: state a verifier of your own instead"
    )]
    GeneratedCheckAlreadySatisfied {
        claim: String,
        negative_control: PathBuf,
    },
    #[error(
        "the check generated from this request — {claim} — rejected {positive_control}, which \
         carries exactly the artifact the request asks for, so it would reject the finished work \
         as well and nothing is proposed from it"
    )]
    GeneratedCheckRejectsTheArtifact {
        claim: String,
        positive_control: PathBuf,
    },
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

// ---------------------------------------------------------------------------
// What the product generates where a project runs no tests of its own
// ---------------------------------------------------------------------------

/// The artifact a request asks the work to leave behind, as the rule reads it.
///
/// It is the whole of what a generated check decides. The names a candidate may satisfy it with
/// are fixed here, and so is whether the bytes have to decode as anything: a check that read more
/// out of a request than these two facts would be judging what the request means, which no rule
/// over words can do.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Artifact {
    /// The file names that satisfy the request, as the program's own patterns.
    patterns: &'static [&'static str],
    /// The names as one English list, for the sentence the draft states.
    stated_patterns: &'static str,
    /// Whether the bytes must also decode as an image with non-zero width and height.
    decodes_as_image: bool,
    /// The file the product plants to show the check accepts something, and its bytes.
    sample: (&'static str, &'static [u8]),
    /// How the draft names what that planted file is.
    stated_sample: &'static str,
}

/// A 1×1 image, which is the smallest thing that decodes with non-zero width and height. It is
/// planted so the check can be shown accepting a candidate, and it is deliberately the least an
/// accepted candidate can carry: what the demonstration shows is exactly how little that is.
const SAMPLE_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xfc, 0xcf, 0xc0, 0x50,
    0x0f, 0x00, 0x04, 0x85, 0x01, 0x80, 0x84, 0xa9, 0x8c, 0x21, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// Any picture the generated program can read the size of. A request that names one format is
/// checked against all of them: the program decides that an image was produced, and the format it
/// was produced in is left to the operator with everything else it does not decide.
const IMAGE: Artifact = Artifact {
    patterns: &["*.png", "*.jpg", "*.jpeg", "*.gif", "*.bmp"],
    stated_patterns: "*.png, *.jpg, *.jpeg, *.gif or *.bmp",
    decodes_as_image: true,
    sample: ("sample.png", SAMPLE_PNG),
    stated_sample: "a 1×1 picture named sample.png",
};

/// The words that say a request asks for something to be produced.
///
/// A request that names an artifact without asking for one to be made is not a request for that
/// artifact: "fix the html escaping" names html and asks for no file. Matching is by stem, so one
/// entry covers "create", "creates" and "creating" without a list of inflections to maintain.
const PRODUCTION_STEMS: [&str; 11] = [
    "creat", "generat", "writ", "mak", "produc", "render", "draw", "emit", "export", "build", "sav",
];

/// One file the check knows only by name, with the sample the product plants for it.
///
/// The sample is empty, because emptiness is what such a check permits: a program that decides a
/// name accepts a file with nothing in it, and the demonstration shows that rather than describing
/// it.
macro_rules! named_file {
    ($extension:literal) => {
        Artifact {
            patterns: &[concat!("*.", $extension)],
            stated_patterns: concat!("*.", $extension),
            decodes_as_image: false,
            sample: (concat!("sample.", $extension), b""),
            stated_sample: concat!("an empty file named sample.", $extension),
        }
    };
}

/// The words a request names an artifact with, and what satisfies each.
///
/// A word outside this table derives nothing. That is the point: the rule reads the artifact out
/// of the request or refuses, and it never falls back on a check it made up. A format that names a
/// picture is read as a picture before it is read as a file, so a request for a png is checked by
/// decoding, and one for an svg — which this host cannot decode — by name.
const ARTIFACT_WORDS: &[(&str, Artifact)] = &[
    ("png", IMAGE),
    ("jpg", IMAGE),
    ("jpeg", IMAGE),
    ("gif", IMAGE),
    ("bmp", IMAGE),
    ("image", IMAGE),
    ("images", IMAGE),
    ("picture", IMAGE),
    ("pictures", IMAGE),
    ("photo", IMAGE),
    ("photograph", IMAGE),
    ("screenshot", IMAGE),
    ("html", named_file!("html")),
    ("htm", named_file!("htm")),
    ("css", named_file!("css")),
    ("js", named_file!("js")),
    ("javascript", named_file!("js")),
    ("json", named_file!("json")),
    ("yaml", named_file!("yaml")),
    ("yml", named_file!("yml")),
    ("toml", named_file!("toml")),
    ("xml", named_file!("xml")),
    ("svg", named_file!("svg")),
    ("csv", named_file!("csv")),
    ("tsv", named_file!("tsv")),
    ("md", named_file!("md")),
    ("markdown", named_file!("md")),
    ("txt", named_file!("txt")),
    ("pdf", named_file!("pdf")),
    ("sql", named_file!("sql")),
    ("rs", named_file!("rs")),
    ("py", named_file!("py")),
    ("python", named_file!("py")),
    ("sh", named_file!("sh")),
];

/// The check a request states, when the rule can read one out of it.
#[derive(Clone, Debug)]
pub struct DerivedCheck {
    /// What the program decides, in the words the draft states it in.
    pub claim: String,
    /// What the program does not decide and the operator keeps, in the same words.
    pub remainder: String,
    /// The program, as the text an operator reads before approving it.
    pub program: String,
    artifact: Artifact,
}

/// The one machine-checkable claim this request makes, read by a fixed rule from its words.
///
/// The rule wants two things: a word saying something is to be produced, and a word naming what.
/// A request carrying only one of them states no observable result this host could look for, and
/// nothing is derived from it.
pub fn derive_check(prompt: &str) -> Option<DerivedCheck> {
    let words: Vec<String> = prompt
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| word.to_ascii_lowercase())
        .collect();
    if !words
        .iter()
        .any(|word| PRODUCTION_STEMS.iter().any(|stem| word.starts_with(stem)))
    {
        return None;
    }
    let artifact = ARTIFACT_WORDS
        .iter()
        .find(|(named, _)| words.iter().any(|word| word == named))
        .map(|(_, artifact)| *artifact)?;

    let (claim, remainder) = match artifact.decodes_as_image {
        true => (
            format!(
                "a file named {} was produced, and its bytes decode as an image with non-zero \
                 width and height",
                artifact.stated_patterns
            ),
            "whether the picture shows what you meant, and whether it is in the format you named, \
             are not decided here and stay yours to judge"
                .to_owned(),
        ),
        false => (
            format!("a file named {} was produced", artifact.stated_patterns),
            "whether that file holds what you meant — its content, and whether it holds anything \
             at all — is not decided here and stays yours to judge"
                .to_owned(),
        ),
    };
    let program = generated_program(&artifact, &claim, &remainder);
    Some(DerivedCheck {
        claim,
        remainder,
        program,
        artifact,
    })
}

/// Directories a produced artifact is never looked for in: history and fetched dependencies are
/// not what the work left behind, and a candidate that happens to carry one of them would
/// otherwise satisfy the check with a file it never wrote.
const NOT_SEARCHED: [&str; 4] = [".git", "node_modules", ".ymp-data", "target"];

/// The generated program, as the text an operator reads before approving it.
fn generated_program(artifact: &Artifact, claim: &str, remainder: &str) -> String {
    /// What every generated program opens with: where it came from, what it decides, and what it
    /// leaves to the operator. The operator approves this text, so the text says what it is.
    const PREAMBLE: &str = r#"#!/bin/sh
# ymp generated this program from the request itself, because this project names no way of
# running its tests that a verifier could be proposed from. A fixed rule read out of the words
# of the request the one artifact the work is asked to leave behind; no model wrote this, so
# neither the request nor this project left the host to produce it.
#
# It decides one mechanical claim and nothing beyond it:
#   {claim}
# What it does not decide stays with the operator:
#   {remainder}
#
# It reads the directory it is given and nothing else, and it is pinned by digest in the
# contract, so a program edited after the draft was approved no longer decides that contract.
cd "$1" || exit 1
"#;
    /// Finding the artifact by name, which is all a check of a named file does.
    const BY_NAME: &str = r#"
found=$(find . {pruned} -type f {matched} -print 2>/dev/null | head -n 1)
if [ -z "$found" ]; then
  printf 'no file named {stated_patterns} was produced\n' >&2
  exit 1
fi
printf '%s was produced\n' "$found"
exit 0
"#;
    /// Reading the size out of a picture's own header, so "an image was produced" is decided by
    /// the bytes rather than by the name they were given. Each format is read where it states its
    /// own width and height; a file that states neither is not an image this program can accept.
    const BY_DECODING: &str = r#"
read_bytes() {
  od -An -v -tu1 -j "$2" -N "$3" -- "$1" 2>/dev/null | tr '\n' ' '
}

png_dimensions() {
  set -- $(read_bytes "$1" 16 8)
  [ $# -eq 8 ] || return 1
  printf '%s %s\n' "$(( $1 * 16777216 + $2 * 65536 + $3 * 256 + $4 ))" \
                   "$(( $5 * 16777216 + $6 * 65536 + $7 * 256 + $8 ))"
}

gif_dimensions() {
  set -- $(read_bytes "$1" 6 4)
  [ $# -eq 4 ] || return 1
  printf '%s %s\n' "$(( $2 * 256 + $1 ))" "$(( $4 * 256 + $3 ))"
}

bmp_dimensions() {
  set -- $(read_bytes "$1" 18 8)
  [ $# -eq 8 ] || return 1
  width=$(( $4 * 16777216 + $3 * 65536 + $2 * 256 + $1 ))
  height=$(( $8 * 16777216 + $7 * 65536 + $6 * 256 + $5 ))
  # Both are signed, and a bitmap written from the top down states a negative height.
  [ "$width" -gt 2147483647 ] && width=$(( 4294967296 - width ))
  [ "$height" -gt 2147483647 ] && height=$(( 4294967296 - height ))
  printf '%s %s\n' "$width" "$height"
}

jpeg_dimensions() {
  file=$1
  offset=2
  steps=0
  # The size is in a frame header, which is reached by walking the segments before it. The walk
  # is bounded: a file that never states a frame ends the walk rather than the program.
  while [ "$steps" -lt 512 ]; do
    steps=$(( steps + 1 ))
    set -- $(read_bytes "$file" "$offset" 4)
    [ $# -ge 2 ] || return 1
    [ "$1" -eq 255 ] || return 1
    case "$2" in
      255) offset=$(( offset + 1 )); continue ;;
      1|208|209|210|211|212|213|214|215) offset=$(( offset + 2 )); continue ;;
      217|218) return 1 ;;
      192|193|194|195|197|198|199|201|202|203|205|206|207)
        set -- $(read_bytes "$file" $(( offset + 5 )) 4)
        [ $# -eq 4 ] || return 1
        printf '%s %s\n' "$(( $3 * 256 + $4 ))" "$(( $1 * 256 + $2 ))"
        return 0 ;;
    esac
    [ $# -eq 4 ] || return 1
    length=$(( $3 * 256 + $4 ))
    [ "$length" -ge 2 ] || return 1
    offset=$(( offset + 2 + length ))
  done
  return 1
}

decodes() {
  file=$1
  set -- $(read_bytes "$file" 0 4)
  [ $# -eq 4 ] || return 1
  if [ "$1" -eq 137 ] && [ "$2" -eq 80 ] && [ "$3" -eq 78 ] && [ "$4" -eq 71 ]; then
    size=$(png_dimensions "$file")
  elif [ "$1" -eq 71 ] && [ "$2" -eq 73 ] && [ "$3" -eq 70 ]; then
    size=$(gif_dimensions "$file")
  elif [ "$1" -eq 255 ] && [ "$2" -eq 216 ]; then
    size=$(jpeg_dimensions "$file")
  elif [ "$1" -eq 66 ] && [ "$2" -eq 77 ]; then
    size=$(bmp_dimensions "$file")
  else
    return 1
  fi
  set -- $size
  [ $# -eq 2 ] || return 1
  [ "$1" -gt 0 ] && [ "$2" -gt 0 ]
}

separator=$(printf '\nx')
separator=${separator%x}
saved=$IFS
IFS=$separator
set -- $(find . {pruned} -type f {matched} -print 2>/dev/null)
IFS=$saved
for candidate in "$@"; do
  if decodes "$candidate"; then
    printf '%s was produced and decodes with non-zero width and height\n' "$candidate"
    exit 0
  fi
done
printf 'no file named {stated_patterns} was produced that decodes as an image with non-zero width and height\n' >&2
exit 1
"#;

    let pruned = format!(
        "\\( {} \\) -prune -o",
        NOT_SEARCHED
            .iter()
            .map(|name| format!("-name {}", shell_word(name)))
            .collect::<Vec<_>>()
            .join(" -o ")
    );
    let matched = format!(
        "\\( {} \\)",
        artifact
            .patterns
            .iter()
            .map(|pattern| format!("-name {}", shell_word(pattern)))
            .collect::<Vec<_>>()
            .join(" -o ")
    );
    let body = match artifact.decodes_as_image {
        true => BY_DECODING,
        false => BY_NAME,
    };
    format!("{PREAMBLE}{body}")
        .replace("{claim}", claim)
        .replace("{remainder}", remainder)
        .replace("{stated_patterns}", artifact.stated_patterns)
        .replace("{pruned}", &pruned)
        .replace("{matched}", &matched)
}

/// A check generated for one request, written out and ready to be demonstrated.
#[derive(Clone, Debug)]
pub struct Generated {
    /// The program on this host. The contract pins it by the digest of these bytes.
    pub program: PathBuf,
    /// The same program as text, so the draft can show what is being approved.
    pub program_text: String,
    /// The digest the contract will record for it, computed here so the draft can state it before
    /// the contract exists.
    pub oracle_digest: String,
    /// A copy of the project as it stands, which the check must reject.
    pub negative_control: PathBuf,
    /// The same copy carrying the artifact the request asks for, which the check must accept. A
    /// check that rejects everything decides as little as one that accepts everything.
    pub positive_control: PathBuf,
    /// What that copy carries, in the words the draft states it in.
    pub stated_positive_control: String,
    /// What the program decides, and what it leaves to the operator.
    pub claim: String,
    pub remainder: String,
}

/// Generate a check for this request, write it under `workspace`, and build both controls.
///
/// Nothing is run here and nothing is stored durably: [`decides_the_generated_check_within`]
/// decides whether what was generated decides anything, and the operator decides whether it may.
pub fn generate(prompt: &str, source: &Path, workspace: &Path) -> Result<Generated, AnswerError> {
    let derived = derive_check(prompt).ok_or_else(|| AnswerError::NoCheckDerivable {
        prompt: stated_request(prompt),
    })?;
    let failed = |reason: String| AnswerError::Workspace {
        path: workspace.to_path_buf(),
        reason,
    };
    fs::create_dir_all(workspace).map_err(|error| failed(error.to_string()))?;

    let program = workspace.join("generated-verify.sh");
    write_program(&program, &derived.program).map_err(|error| failed(error.to_string()))?;
    let negative_control = copy_negative_control(source, workspace)?;

    let positive_control = workspace.join("artifact-control");
    if positive_control.exists() {
        fs::remove_dir_all(&positive_control).map_err(|error| failed(error.to_string()))?;
    }
    copy_tree(source, &positive_control)?;
    let (name, bytes) = derived.artifact.sample;
    plant(&positive_control.join(name), bytes, false).map_err(|error| failed(error.to_string()))?;

    Ok(Generated {
        oracle_digest: digest_bytes(derived.program.as_bytes()),
        program,
        program_text: derived.program,
        negative_control,
        positive_control,
        stated_positive_control: derived.artifact.stated_sample.to_owned(),
        claim: derived.claim,
        remainder: derived.remainder,
    })
}

/// The request as a refusal quotes it back: one line, bounded, so a long paste does not become the
/// whole message.
fn stated_request(prompt: &str) -> String {
    const QUOTED_CHARS: usize = 120;
    let line = prompt.lines().next().unwrap_or_default().trim();
    match line.char_indices().nth(QUOTED_CHARS) {
        None => line.to_owned(),
        Some((index, _)) => format!("{}…", &line[..index]),
    }
}

/// Both decisions a generated check has to make before it is shown.
///
/// It must reject the project as it stands, which is the decision every verifier answers, and it
/// must accept that same project carrying the artifact the request asks for. Unlike a verifier
/// proposed from a project's own tests, this one can be shown both halves: the product can build
/// the artifact a derived check looks for, because looking for it is all the check does. Showing
/// the acceptance is also the only way an operator can see how little it takes to satisfy the
/// check, which is what the draft asks them to judge.
pub fn decides_the_generated_check_within(
    generated: &Generated,
    negative_control: &Path,
    wall_limit: Duration,
) -> Result<(), AnswerError> {
    let program = generated.program.as_path();
    let refused = |reason: String| AnswerError::VerifierNotRun {
        program: program.to_path_buf(),
        negative_control: negative_control.to_path_buf(),
        reason,
    };
    let accepted_the_project = || AnswerError::GeneratedCheckAlreadySatisfied {
        claim: generated.claim.clone(),
        negative_control: negative_control.to_path_buf(),
    };
    let rejected_the_artifact = || AnswerError::GeneratedCheckRejectsTheArtifact {
        claim: generated.claim.clone(),
        positive_control: generated.positive_control.clone(),
    };

    let verifier = demonstration_verifier(program, negative_control, wall_limit)?;
    let subject_digest = || demonstration_digest(program, negative_control);

    // The executor judges the negative control before the subject, so the copy carrying the
    // artifact is judged in the same call that requires the plain copy to be rejected.
    match verifier.verify_candidate(
        &generated.positive_control,
        negative_control,
        subject_digest(),
    ) {
        Ok(evidence) => match evidence.decision() {
            VerificationDecision::Accept => Ok(()),
            VerificationDecision::Reject => Err(rejected_the_artifact()),
        },
        Err(VerifierError::NegativeControlPassed) => Err(accepted_the_project()),
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
