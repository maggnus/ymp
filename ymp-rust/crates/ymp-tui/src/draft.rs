//! The typed request as it is assembled into a run request.
//!
//! The operator states the work in one line. Everything else that a contract needs, the product
//! supplies from the project itself: the directory the work is done in, a negative control that
//! is a copy of that project as it stands, and a verifier proposed from the way the project
//! already runs its tests. None of it is an acceptance condition until the operator approves it,
//! and none of it starts anything.
//!
//! What the product supplies, it demonstrates. The proposed verifier has to reject the copy that
//! holds no result, and reject a candidate that holds no result either and differs only in having
//! replaced the project's test entry point with a program that accepts everything; a program that
//! fails either half decides nothing and never reaches a draft. Assembling and demonstrating
//! starts programs and waits for them, so this module hands that work back to the caller as a
//! [`DraftJob`] rather than doing it where it was asked for.
//!
//! While the draft is unauthorized, the next typed line amends it — a different verifier, source,
//! negative control or budget — and the amended draft is assembled again and restated. A line
//! that amends nothing restates the work itself.

use std::path::{Path, PathBuf};
use std::time::Duration;

use ymp_application::answer::Assembled;
use ymp_application::{AcceptanceCondition, RunRequest, answer};
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
        let (program, negative_control, substitution_control) = match &self.program {
            // A verifier the operator named is theirs; the product supplies the control it must
            // reject and states that it has no sample of its own that this program must accept.
            Some(program) => {
                let control = match &self.negative_control {
                    Some(control) => control.clone(),
                    None => answer::copy_negative_control(&self.source, &self.workspace)
                        .map_err(|refusal| refusal.to_string())?,
                };
                stated.push(format!("verifier {} — stated by you", program.display()));
                (program.clone(), control, None)
            }
            None => {
                let assembled: Assembled = answer::assemble(&self.source, &self.workspace)
                    .map_err(|refusal| refusal.to_string())?;
                stated.push(format!(
                    "verifier {} — proposed from {}, which this project runs as `{}`, and it \
                     carries the bytes that file has here, so a candidate cannot supply its own",
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
                    Some(assembled.substitution_control),
                )
            }
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

        match &substitution_control {
            Some(substituted) => {
                answer::refuses_a_substituted_entry_point_within(
                    &program,
                    &negative_control,
                    substituted,
                    self.wall_limit,
                )
                .map_err(|refusal| refusal.to_string())?;
                stated.push(
                    "demonstrated: the verifier rejected the negative control, and rejected a \
                     candidate that had replaced this project's test entry point with a program \
                     accepting everything, so the condition is not the candidate's to rewrite · \
                     what it accepts is undemonstrated until a candidate satisfies the project's \
                     own tests"
                        .to_owned(),
                );
            }
            None => {
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
                .any(|line| line.contains("replaced this project's test entry point")),
            "the draft does not state what it demonstrated about the entry point: {:?}",
            assembly.stated
        );
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

    #[test]
    fn a_project_with_no_test_entry_point_is_told_so_rather_than_asked_for_a_path() {
        let project = project();
        fs::remove_file(project.directory.join("scripts/test.sh")).expect("remove entry point");
        let mut draft = Draft::new("keep the replay path idempotent");
        let refusal = draft
            .job(&project.directory, &project.data_root.join("draft"), 1)
            .run()
            .expect_err("nothing can be proposed from a project that runs no tests");
        assert!(refusal.contains("no test entry point"), "{refusal}");
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
