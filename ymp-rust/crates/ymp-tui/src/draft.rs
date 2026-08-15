//! The typed request as it is assembled into a run request.
//!
//! The operator states the work in one line. Everything else that a contract needs, the product
//! supplies from the project itself: the directory the work is done in, a negative control that
//! is a copy of that project as it stands, and a verifier proposed from the way the project
//! already runs its tests. None of it is an acceptance condition until the operator approves it,
//! and none of it starts anything.
//!
//! A project that runs no tests is drafted for as well. There the check is generated from the
//! request itself: a self-contained program that decides the one mechanical claim the request can
//! be read as making — that a named file was produced, that a picture decodes — and decides
//! nothing about what that file means. The draft states what that program leaves to the operator,
//! gives the digest the contract pins it by, and shows its text — whole where it fits on a screen,
//! and to a stated bound with the path to the rest where it does not. Approval is what turns the
//! program into an acceptance condition, and approval of an unread program would be approval of
//! nothing.
//!
//! What the product supplies, it demonstrates. The proposed verifier has to reject the copy that
//! holds no result, and reject every candidate the product can build that holds no result either
//! and reached acceptance through the file the verifier delegates to — one that rewrote it, one
//! per name the command would read in preference to it. A generated check is shown both halves
//! instead: it has to reject the project as it stands and accept that project carrying the
//! artifact, because a check that rejects everything decides as little as one that accepts
//! everything. A program that fails any of those decides nothing and never reaches a draft.
//! Assembling and demonstrating starts programs and waits for them, so this module hands that work
//! back to the caller as a [`DraftJob`] rather than doing it where it was asked for.
//!
//! What the draft states is bounded by what was demonstrated. One file is fixed; what that file
//! runs in turn stays the candidate's, and the statement says so rather than leaving the operator
//! to read more into it than the program does.
//!
//! While the draft is unauthorized, the next typed line amends it — a different verifier, source,
//! negative control or budget — and the amended draft is assembled again and restated. A line
//! that amends nothing restates the work itself.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ymp_application::answer::{Generated, SubstitutionControl};
use ymp_application::{AcceptanceCondition, AnswerError, RunRequest, answer};
use ymp_domain::Budget;
use ymp_domain::contract::default_wall_time_ms;

/// One request under assembly, and whatever later lines amended.
///
/// Every field beyond the prompt is an amendment: absent means the product supplies it.
#[derive(Clone, Debug, Default)]
pub struct Draft {
    pub prompt: String,
    pub source: Option<PathBuf>,
    pub program: Option<PathBuf>,
    pub negative_control: Option<PathBuf>,
    pub budget: Option<Budget>,
    /// Whether work this draft asked for is still deciding it.
    assembling: bool,
}

/// What a typed line did to the draft.
#[derive(Clone, Debug)]
pub enum Amendment {
    /// The draft changed and has to be assembled again. The text states what changed.
    Took(String),
    /// The line named something this host could not take. The draft is unchanged.
    Refused(String),
}

impl Draft {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            ..Self::default()
        }
    }

    /// Read one typed line as an amendment of this draft.
    ///
    /// A line that opens with what it changes changes it; anything else restates the work, which
    /// leaves what the product supplied in place. Nothing here is a question the operator was
    /// asked: the dialogue never demands a path, it accepts one when it is offered.
    pub fn amend(&mut self, text: &str) -> Amendment {
        let line = text.trim();
        if line.is_empty() {
            return Amendment::Refused("an empty line amends nothing".to_owned());
        }
        for (keyword, field) in [
            ("verifier", Field::Verifier),
            ("verify with", Field::Verifier),
            ("source", Field::Source),
            ("negative control", Field::NegativeControl),
            ("attempts", Field::Attempts),
            ("verification queries", Field::Queries),
            ("queries", Field::Queries),
        ] {
            let Some(value) = strip_keyword(line, keyword) else {
                continue;
            };
            return self.set(field, value);
        }
        self.prompt = line.to_owned();
        Amendment::Took(format!("the work is now: {line}"))
    }

    fn set(&mut self, field: Field, value: &str) -> Amendment {
        match field {
            Field::Verifier => match answer::verifier_program(Path::new(value)) {
                Ok(program) => {
                    let stated = format!("the verifier is now {}", program.display());
                    self.program = Some(program);
                    Amendment::Took(stated)
                }
                Err(refusal) => Amendment::Refused(refusal.to_string()),
            },
            Field::Source => match answer::source_directory(Path::new(value)) {
                Ok(source) => {
                    let stated = format!("the work is now done in {}", source.display());
                    self.source = Some(source);
                    Amendment::Took(stated)
                }
                Err(refusal) => Amendment::Refused(refusal.to_string()),
            },
            Field::NegativeControl => match answer::negative_control_directory(Path::new(value)) {
                Ok(control) => {
                    let stated = format!("the negative control is now {}", control.display());
                    self.negative_control = Some(control);
                    Amendment::Took(stated)
                }
                Err(refusal) => Amendment::Refused(refusal.to_string()),
            },
            Field::Attempts | Field::Queries => match value.trim().parse::<u32>() {
                Ok(amount) => {
                    let budget = self.budget.clone().unwrap_or(DEFAULT_BUDGET);
                    let (attempts, queries) = match field {
                        Field::Attempts => (amount, budget.verification_queries_remaining),
                        _ => (budget.attempts_remaining, amount),
                    };
                    self.budget = Some(Budget::new(attempts, queries));
                    Amendment::Took(format!(
                        "the run would start with {attempts} attempts and {queries} verification \
                         queries"
                    ))
                }
                Err(_) => Amendment::Refused(format!(
                    "a budget dimension is a whole number, and {value} is not one"
                )),
            },
        }
    }

    /// The work this draft needs before it can be shown as a contract.
    ///
    /// `project` is the directory the product was started in, which is the source unless a line
    /// amended it. `drafts` is where the product may write what it supplies, and `attempt` is the
    /// number this assembly is asked under.
    ///
    /// Every assembly gets a workspace of its own, so the copy it takes of the project is the
    /// project as it stands at that moment. Reusing an earlier copy would let a draft assembled
    /// after the project changed be demonstrated against a state that no longer exists, and be
    /// stated as a copy of the project as it stands, which it would not be.
    pub fn job(&mut self, project: &Path, drafts: &Path, attempt: u64) -> DraftJob {
        self.assembling = true;
        let source = self.source.clone().unwrap_or_else(|| project.to_path_buf());
        let workspace = drafts.join(workspace_name(&source, attempt));
        DraftJob {
            prompt: self.prompt.clone(),
            source,
            workspace,
            program: self.program.clone(),
            negative_control: self.negative_control.clone(),
            budget: self.budget.clone(),
            wall_limit: Duration::from_millis(default_wall_time_ms()),
        }
    }

    /// Whether work this draft asked for is still deciding it.
    pub fn is_assembling(&self) -> bool {
        self.assembling
    }

    /// The work has decided, whichever way.
    pub fn settled(&mut self) {
        self.assembling = false;
    }
}

/// The budget a drafted run starts with when nothing amended it. It is the application's own
/// default, restated here only so an amendment has something to change.
const DEFAULT_BUDGET: Budget = Budget::new(1, 1);

#[derive(Clone, Copy, Debug)]
enum Field {
    Verifier,
    Source,
    NegativeControl,
    Attempts,
    Queries,
}

/// How many characters of the source digest open the name of a workspace. The digest names the
/// source, and the number after it names the assembly, so no two assemblies share a copy.
const WORKSPACE_NAME_CHARS: usize = 12;

fn workspace_name(source: &Path, attempt: u64) -> String {
    format!("{}-{attempt}", source_name(source))
}

/// The part of a workspace name that names the source it was taken from.
pub fn source_name(source: &Path) -> String {
    ymp_domain::digest_bytes(source.display().to_string().as_bytes())[..WORKSPACE_NAME_CHARS]
        .to_owned()
}

/// A line amends what it opens by naming, and the rest of the line is the value.
fn strip_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(keyword)?;
    let value = rest.trim_start_matches([':', '=', ' ']).trim();
    (!value.is_empty() && rest.starts_with([':', '=', ' '])).then_some(value)
}

/// Assembling one draft: everything the work needs and nothing that reads the interface, so it
/// can be moved to another thread and its outcome handed back as a value.
#[derive(Clone, Debug)]
pub struct DraftJob {
    prompt: String,
    source: PathBuf,
    workspace: PathBuf,
    program: Option<PathBuf>,
    negative_control: Option<PathBuf>,
    budget: Option<Budget>,
    /// The limit the contract would apply to the same program. A program that never returns ends
    /// in a refusal that names this limit rather than in a wait without end.
    wall_limit: Duration,
}

/// A draft the product assembled and demonstrated, ready to be shown as one contract.
#[derive(Clone, Debug)]
pub struct Assembly {
    pub request: RunRequest,
    /// What was supplied and what was demonstrated, in one statement rather than a sequence of
    /// questions.
    pub stated: Vec<String>,
}

impl DraftJob {
    /// Where this assembly writes what the product supplies. It belongs to this assembly alone.
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    /// The source this assembly copies, so the interface can tell one project's workspaces from
    /// another's when it removes the ones it no longer needs.
    pub fn source(&self) -> &Path {
        &self.source
    }

    /// What the interface says it is waiting for while this runs.
    pub fn waiting_for(&self) -> String {
        match &self.program {
            Some(program) => format!(
                "checking {} against the negative control · limit {} s",
                file_name(program),
                self.wall_limit.as_secs()
            ),
            None => format!(
                "assembling a contract from {} · copying it as the negative control and asking \
                 the proposed verifier to decide it · limit {} s",
                self.source.display(),
                self.wall_limit.as_secs()
            ),
        }
    }

    /// Assemble what the product supplies, demonstrate it, and produce the request a contract
    /// would be prepared from. The refusal is kept as text, because what the interface does with
    /// it is state it.
    pub fn run(self) -> Result<Box<Assembly>, String> {
        self.assemble().map(Box::new)
    }

    fn assemble(self) -> Result<Assembly, String> {
        let mut stated = Vec::new();
        let (program, negative_control, supplied) = match &self.program {
            // A verifier the operator named is theirs; the product supplies the control it must
            // reject and states that it has no sample of its own that this program must accept.
            Some(program) => {
                let control = match &self.negative_control {
                    Some(control) => control.clone(),
                    None => answer::copy_negative_control(&self.source, &self.workspace)
                        .map_err(|refusal| refusal.to_string())?,
                };
                stated.push(format!("verifier {} — stated by you", program.display()));
                (program.clone(), control, Supplied::Stated)
            }
            None => match answer::assemble(&self.source, &self.workspace) {
                Ok(assembled) => {
                    stated.push(format!(
                        "verifier {} — proposed from {}, which this project runs as `{}`. It \
                         carries the bytes of that one file and runs no other by that name, so a \
                         candidate cannot supply its own · what that file runs in turn — a script \
                         it sources, a program a recipe calls — is the candidate's own, and \
                         changing one of those changes what these tests do",
                        assembled.program.display(),
                        assembled.entry_point.relative_path.display(),
                        assembled.entry_point.command
                    ));
                    let control = self
                        .negative_control
                        .clone()
                        .unwrap_or_else(|| assembled.negative_control.clone());
                    (
                        assembled.program,
                        control,
                        Supplied::Proposed(assembled.substitution_controls),
                    )
                }
                // A project that runs no tests is not out of reach: there is nothing to propose a
                // verifier from, so the check is generated from the request instead. It is weaker
                // than a project's own tests by construction, which is why the draft shows it in
                // full and names what it does not decide.
                Err(AnswerError::NoTestEntryPoint(_)) => {
                    let generated = answer::generate(&self.prompt, &self.source, &self.workspace)
                        .map_err(|refusal| refusal.to_string())?;
                    stated.push(format!(
                        "verifier {} — generated by ymp from your request, because this project \
                         names no way of running its tests to propose one from. It decides one \
                         mechanical claim: {} · {}",
                        generated.program.display(),
                        generated.claim,
                        generated.remainder
                    ));
                    stated.push(
                        "derived by a fixed rule over the words of your request — the word that \
                         asks for something to be produced, and the artifact it names. No model \
                         was asked to write it, so neither your request nor this project left \
                         this host, and a request naming no artifact is refused rather than \
                         guessed at"
                            .to_owned(),
                    );
                    stated.push(stated_program(&generated));
                    let control = self
                        .negative_control
                        .clone()
                        .unwrap_or_else(|| generated.negative_control.clone());
                    (
                        generated.program.clone(),
                        control,
                        Supplied::Generated(Box::new(generated)),
                    )
                }
                Err(refusal) => return Err(refusal.to_string()),
            },
        };

        stated.push(match &self.negative_control {
            Some(_) => format!(
                "negative control {} — stated by you",
                negative_control.display()
            ),
            None => format!(
                "negative control {} — a copy of the project as it stands, which holds no result",
                negative_control.display()
            ),
        });

        match &supplied {
            Supplied::Proposed(controls) => {
                answer::refuses_a_substituted_entry_point_within(
                    &program,
                    &negative_control,
                    controls,
                    self.wall_limit,
                )
                .map_err(|refusal| refusal.to_string())?;
                stated.push(format!(
                    "demonstrated: the verifier rejected the negative control, and rejected {} · \
                     what it accepts is undemonstrated until a candidate satisfies the project's \
                     own tests",
                    joined(controls.iter().map(|control| control.stated.as_str()))
                ));
            }
            // Here both halves can be shown, because the product can build what a generated check
            // looks for. Showing the acceptance is the point: it is how little a candidate has to
            // do to satisfy this check, and the operator authorizes knowing it.
            Supplied::Generated(generated) => {
                answer::decides_the_generated_check_within(
                    generated,
                    &negative_control,
                    self.wall_limit,
                )
                .map_err(|refusal| refusal.to_string())?;
                stated.push(format!(
                    "demonstrated: the generated verifier rejected the negative control, and \
                     accepted a copy of the project carrying {} · that is the whole of what it \
                     decides, and the rest of the work is judged by you",
                    generated.stated_positive_control
                ));
            }
            Supplied::Stated => {
                answer::discriminates_within(&program, &negative_control, self.wall_limit)
                    .map_err(|refusal| refusal.to_string())?;
                stated.push(
                    "demonstrated: the verifier rejected the negative control · what it accepts \
                     is undemonstrated, because a verifier you named has no sample the product \
                     can build for it"
                        .to_owned(),
                );
            }
        }

        Ok(Assembly {
            request: RunRequest {
                prompt: self.prompt,
                source: self.source,
                acceptance: Some(AcceptanceCondition::new(program, negative_control)),
                capture_exclusions: Vec::new(),
                contract_id: None,
                budget: self.budget,
            },
            stated,
        })
    }
}

/// Where the verifier in a draft came from, and therefore what has to be demonstrated about it.
enum Supplied {
    /// A verifier the operator named. The product has no candidate of its own to offer it.
    Stated,
    /// A verifier proposed from the project's own tests, with every candidate that would have
    /// reached acceptance through the file it delegates to.
    Proposed(Vec<SubstitutionControl>),
    /// A check generated from the request, with the copy carrying the artifact it must accept.
    Generated(Box<Generated>),
}

/// How many lines of a generated program the draft states in the transcript.
///
/// A longer program is stated to this bound and the rest addressed by the path holding it. A
/// statement that fills the screen pushes the claim, the digest and the line that authorizes off
/// the top of it, and an operator who can no longer see what they are approving is not better
/// informed for having been shown more of it. The digest covers the whole program either way.
const PROGRAM_LINES_STATED: usize = 24;

/// The generated program as the draft states it: its text, with each line break marked rather
/// than dropped, and the digest the contract pins it by.
///
/// The transcript keeps no control characters, so a program written into it would otherwise
/// arrive with its lines run together. The operator is being asked to approve this text, so the
/// breaks are shown as a mark they can read.
fn stated_program(generated: &Generated) -> String {
    let lines: Vec<&str> = generated.program_text.lines().map(str::trim_end).collect();
    let digest = crate::projection::short_digest(&generated.oracle_digest);
    if lines.len() <= PROGRAM_LINES_STATED {
        return format!(
            "the program in full, which the contract pins by digest {digest} — ⏎ marks each line \
             break: {}",
            lines.join(" ⏎ ")
        );
    }
    format!(
        "the program's first {PROGRAM_LINES_STATED} lines of {}, all of which the contract pins by \
         digest {digest} and {} holds — ⏎ marks each line break: {}",
        lines.len(),
        generated.program.display(),
        lines[..PROGRAM_LINES_STATED].join(" ⏎ ")
    )
}

/// Several statements read as one sentence, so a draft states what it demonstrated rather than
/// listing it.
fn joined<'a>(parts: impl Iterator<Item = &'a str>) -> String {
    let parts = parts.collect::<Vec<_>>();
    match parts.split_last() {
        None => String::new(),
        Some((last, [])) => (*last).to_owned(),
        Some((last, rest)) => format!("{}, and {last}", rest.join(", ")),
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::{Amendment, Draft};
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    /// A project on disk with the entry point ymp proposes a verifier from: a test script that
    /// judges the directory it is given, failing while the work is not done.
    struct Project {
        _root: TempDir,
        directory: PathBuf,
        data_root: PathBuf,
        program: PathBuf,
    }

    fn project() -> Project {
        let root = TempDir::new().expect("temporary root");
        let directory = root
            .path()
            .canonicalize()
            .expect("resolve path")
            .join("work");
        let data_root = directory.join(".ymp-data");
        fs::create_dir_all(directory.join("scripts")).expect("scripts directory");
        fs::create_dir_all(&data_root).expect("data root");
        executable(
            &directory.join("scripts/test.sh"),
            "#!/bin/sh\ntest -f result.txt\n",
        );
        let program = directory.join("stated-verifier.sh");
        executable(&program, "#!/bin/sh\ntest -f \"$*/result.txt\"\n");
        Project {
            _root: root,
            directory,
            data_root,
            program,
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

    fn took(amendment: Amendment) -> String {
        match amendment {
            Amendment::Took(stated) => stated,
            Amendment::Refused(reason) => panic!("the line was refused: {reason}"),
        }
    }

    #[test]
    fn a_request_alone_reaches_a_contract_the_product_assembled() {
        let project = project();
        let mut draft = Draft::new("keep the replay path idempotent");
        let assembly = draft
            .job(&project.directory, &project.data_root.join("draft"), 1)
            .run()
            .expect("the project names a test entry point");

        let acceptance = assembly
            .request
            .acceptance
            .expect("the assembled draft carries an acceptance condition");
        assert!(acceptance.program.starts_with(&project.data_root));
        assert!(acceptance.negative_control.starts_with(&project.data_root));
        assert_eq!(assembly.request.source, project.directory);
        assert!(
            assembly
                .stated
                .iter()
                .any(|line| line.contains("scripts/test.sh")),
            "{:?}",
            assembly.stated
        );
        assert!(
            assembly
                .stated
                .iter()
                .any(|line| line.contains("replaced scripts/test.sh with a program accepting")),
            "the draft does not state what it demonstrated about the entry point: {:?}",
            assembly.stated
        );
    }

    /// What the draft promises is what the program does, no more.
    ///
    /// The operator authorizes on this statement, so it names the one file that is fixed and says
    /// plainly that whatever that file runs in turn is still the candidate's. A candidate that
    /// keeps the entry point byte for byte and rewrites the script it sources is accepted, and
    /// `what_the_fixed_entry_point_runs_in_turn_stays_the_candidates_own` in `ymp-application`
    /// measures that. A statement that claimed more would be read as covering it.
    #[test]
    fn the_draft_states_the_boundary_the_program_actually_holds() {
        let project = project();
        let mut draft = Draft::new("keep the replay path idempotent");
        let assembly = draft
            .job(&project.directory, &project.data_root.join("draft"), 1)
            .run()
            .expect("the project names a test entry point");
        let stated = assembly.stated.join(" · ");

        for promise in [
            "carries the bytes of that one file",
            "runs no other by that name",
            "what that file runs in turn",
            "is the candidate's own",
            "what it accepts is undemonstrated",
        ] {
            assert!(
                stated.contains(promise),
                "the draft does not state `{promise}`: {stated}"
            );
        }
    }

    /// A project whose test entry point is not text this host can carry inside a shell program is
    /// told so, rather than being given a verifier the candidate could rewrite.
    #[test]
    fn a_test_entry_point_that_cannot_be_carried_is_refused_rather_than_delegated_to() {
        let project = project();
        fs::write(project.directory.join("scripts/test.sh"), [0x00, 0xff])
            .expect("write an entry point that is not text");
        let mut draft = Draft::new("keep the replay path idempotent");
        let refusal = draft
            .job(&project.directory, &project.data_root.join("draft"), 1)
            .run()
            .expect_err("a verifier was proposed from bytes it could not carry");
        assert!(refusal.contains("could not be fixed into"), "{refusal}");
    }

    /// A project that runs no tests still reaches a draft, and what the operator is asked to
    /// approve is a program they can read: its text, what it decides, what it does not, and the
    /// digest the contract pins it by.
    #[test]
    fn a_project_with_no_test_entry_point_draws_the_check_from_the_request_itself() {
        let project = project();
        fs::remove_file(project.directory.join("scripts/test.sh")).expect("remove entry point");
        let mut draft = Draft::new("create an empty html file");
        let assembly = draft
            .job(&project.directory, &project.data_root.join("draft"), 1)
            .run()
            .expect("a request naming an artifact reaches a generated check");
        let stated = assembly.stated.join(" · ");

        for promise in [
            "generated by ymp from your request",
            "a file named *.html was produced",
            "stays yours to judge",
            "No model was asked to write it",
            "the program in full",
            "#!/bin/sh",
            "rejected the negative control, and accepted a copy of the project carrying an empty \
             file named sample.html",
        ] {
            assert!(
                stated.contains(promise),
                "the draft does not state `{promise}`: {stated}"
            );
        }

        // The digest shown is the digest of the program on this host, which is the digest the
        // contract records for the oracle. An operator comparing the two compares the same thing.
        let program = assembly
            .request
            .acceptance
            .as_ref()
            .expect("the assembled draft carries an acceptance condition")
            .program
            .clone();
        let digest = ymp_domain::digest_bytes(&fs::read(&program).expect("read the program"));
        assert!(
            stated.contains(&crate::projection::short_digest(&digest)),
            "the draft states no digest for the program it shows: {stated}"
        );
    }

    /// Generation stops where honesty does. A request that states no result this host can look
    /// for is refused, and the refusal names what is left to the operator instead of inventing a
    /// condition for them.
    #[test]
    fn a_request_stating_no_artifact_is_refused_in_a_project_that_runs_no_tests() {
        let project = project();
        fs::remove_file(project.directory.join("scripts/test.sh")).expect("remove entry point");
        let mut draft = Draft::new("keep the replay path idempotent");
        let refusal = draft
            .job(&project.directory, &project.data_root.join("draft"), 1)
            .run()
            .expect_err("a check was invented for a request that states no result");
        assert!(
            refusal.contains("state a verifier of your own"),
            "{refusal}"
        );
    }

    #[test]
    fn a_line_amends_the_draft_and_the_amended_draft_is_assembled_again() {
        let project = project();
        let mut draft = Draft::new("keep the replay path idempotent");
        assert!(
            took(draft.amend(&format!("verifier {}", project.program.display())))
                .contains("stated-verifier.sh")
        );
        assert!(took(draft.amend("attempts 3")).contains("3 attempts"));
        assert_eq!(draft.prompt, "keep the replay path idempotent");

        let assembly = draft
            .job(&project.directory, &project.data_root.join("draft"), 1)
            .run()
            .expect("a stated verifier that rejects the copy is taken");
        let acceptance = assembly.request.acceptance.expect("acceptance condition");
        assert_eq!(acceptance.program, project.program);
        assert_eq!(
            assembly
                .request
                .budget
                .expect("the amended budget")
                .attempts_remaining,
            3
        );
        assert!(
            assembly
                .stated
                .iter()
                .any(|line| line.contains("undemonstrated")),
            "a verifier nobody demonstrated acceptance for was stated as demonstrated: {:?}",
            assembly.stated
        );
    }

    #[test]
    fn a_line_that_amends_nothing_restates_the_work() {
        let project = project();
        let mut draft = Draft::new("keep the replay path idempotent");
        draft.amend(&format!("source {}", project.directory.display()));
        assert!(
            took(draft.amend("make the replay path idempotent under load")).contains("the work")
        );
        assert_eq!(draft.prompt, "make the replay path idempotent under load");
        assert_eq!(draft.source.as_deref(), Some(project.directory.as_path()));
    }

    #[test]
    fn a_line_naming_something_this_host_cannot_take_leaves_the_draft_unchanged() {
        let project = project();
        let mut draft = Draft::new("keep the replay path idempotent");
        let Amendment::Refused(reason) = draft.amend("verifier /no/such/program") else {
            panic!("a verifier that is not on this host was taken");
        };
        assert!(reason.contains("could not be read"), "{reason}");
        assert!(draft.program.is_none());
        let _ = project;
    }
}
