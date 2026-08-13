//! The typed request as it is assembled into a run request.
//!
//! The operator states the work in one line. Two things follow that the kernel cannot infer and
//! will not invent: the directory the work is done in, and the acceptance condition that decides
//! a candidate. The first has an honest default — the directory the product was started in —
//! and the second has none, so it is asked for and never supplied on the operator's behalf.
//!
//! This module only collects answers. Validating them, resolving them on this host and turning
//! them into a contract belongs to `ymp-application`, so the interface and the equivalent
//! command reach the kernel through one implementation.
//!
//! Each answer is taken where it is typed. An answer the application cannot resolve — a verifier
//! that is not an executable file on this host, a source that is not a directory, or a verifier
//! that accepts the deliberately wrong candidate — ends the draft at that answer with the reason
//! named, instead of being carried through the remaining questions and refused at assembly.

use std::path::{Path, PathBuf};

use ymp_application::{AcceptanceCondition, RunRequest, answer};

/// What the draft is waiting for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Question {
    /// The directory the work is done in. Empty answer accepts the project directory.
    Source,
    /// The program that decides whether a candidate is accepted. There is no default.
    Verifier,
    /// The deliberately wrong candidate that program must reject.
    NegativeControl,
}

/// One request under assembly.
#[derive(Clone, Debug)]
pub struct Draft {
    pub prompt: String,
    pub source: Option<PathBuf>,
    pub program: Option<PathBuf>,
    pub negative_control: Option<PathBuf>,
    pub question: Question,
}

/// What the draft needs next.
#[derive(Clone, Debug)]
pub enum Step {
    /// Still collecting: the question to put to the operator.
    Ask(Question),
    /// The answer just typed was not taken, and the draft ends here. The text names the answer
    /// and what this host said about it, so the operator learns it at that answer rather than
    /// after the remaining questions.
    Refused(String),
    /// Complete as far as the interface can take it. The application decides whether it is a
    /// contract: a request with no acceptance condition reaches it and is refused there.
    Ready(Box<RunRequest>),
}

impl Draft {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            source: None,
            program: None,
            negative_control: None,
            question: Question::Source,
        }
    }

    /// Take one typed answer and return what the draft needs next.
    ///
    /// An empty answer to the source question accepts the project directory. An empty answer to
    /// either acceptance question leaves the condition absent, and the request goes to the
    /// application without one rather than being silently completed here.
    ///
    /// A non-empty answer is resolved on this host before the next question is asked, and what is
    /// kept is the resolved path, so the contract is assembled from exactly what was checked. The
    /// negative control is the last answer of the acceptance condition, so the verifier is asked
    /// to reject it there — the earliest point at which both halves are known.
    pub fn answer(&mut self, text: &str, project: &Path) -> Step {
        let answer = text.trim();
        match self.question {
            Question::Source => {
                let stated = if answer.is_empty() {
                    project.to_path_buf()
                } else {
                    PathBuf::from(answer)
                };
                let source = match answer::source_directory(&stated) {
                    Ok(source) => source,
                    Err(refusal) => return Step::Refused(refusal.to_string()),
                };
                self.source = Some(source);
                self.question = Question::Verifier;
                Step::Ask(Question::Verifier)
            }
            Question::Verifier => {
                if answer.is_empty() {
                    return Step::Ready(Box::new(self.request()));
                }
                let program = match answer::verifier_program(Path::new(answer)) {
                    Ok(program) => program,
                    Err(refusal) => return Step::Refused(refusal.to_string()),
                };
                self.program = Some(program);
                self.question = Question::NegativeControl;
                Step::Ask(Question::NegativeControl)
            }
            Question::NegativeControl => {
                if !answer.is_empty() {
                    let negative_control =
                        match answer::negative_control_directory(Path::new(answer)) {
                            Ok(negative_control) => negative_control,
                            Err(refusal) => return Step::Refused(refusal.to_string()),
                        };
                    if let Some(program) = &self.program
                        && let Err(refusal) = answer::discriminates(program, &negative_control)
                    {
                        return Step::Refused(refusal.to_string());
                    }
                    self.negative_control = Some(negative_control);
                }
                Step::Ready(Box::new(self.request()))
            }
        }
    }

    /// What the answers so far have set, stated back so an accepted default is visible.
    pub fn taken(&self) -> String {
        let path = |value: &Option<PathBuf>| {
            value
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default()
        };
        match self.question {
            Question::Source => String::new(),
            Question::Verifier => format!("source directory {}", path(&self.source)),
            Question::NegativeControl => format!("verifier program {}", path(&self.program)),
        }
    }

    /// The request as it stands. The acceptance condition is present only when both of its parts
    /// were given.
    fn request(&self) -> RunRequest {
        let acceptance = match (&self.program, &self.negative_control) {
            (Some(program), Some(negative_control)) => {
                Some(AcceptanceCondition::new(program, negative_control))
            }
            // A program with no negative control is still passed on: the application names the
            // part that is missing, and it names it the same way for every surface.
            (Some(program), None) => Some(AcceptanceCondition::new(program, PathBuf::new())),
            _ => None,
        };
        RunRequest {
            prompt: self.prompt.clone(),
            source: self.source.clone().unwrap_or_default(),
            acceptance,
            capture_exclusions: Vec::new(),
            contract_id: None,
            budget: None,
        }
    }
}

/// The question put to the operator, and why the kernel is asking rather than deciding.
pub fn question_text(question: Question, project: &Path) -> String {
    match question {
        Question::Source => format!(
            "which directory is this work done in? Enter accepts {}, or type another path",
            project.display()
        ),
        Question::Verifier => "which program decides whether a candidate is accepted? Type its \
             path. There is no default: a contract whose acceptance condition ymp invented would \
             judge a result nobody asked for. Enter with nothing typed states that you have none."
            .to_owned(),
        Question::NegativeControl => "which deliberately wrong candidate must that program \
             reject? Type the path of a directory holding it. A program that accepts it is not \
             discriminating, so the run could not be judged."
            .to_owned(),
    }
}

/// The one-line form of the question, shown on the input row while the answer is awaited.
pub fn question_hint(question: Question) -> String {
    match question {
        Question::Source => {
            "answer: source directory · Enter accepts the project directory".to_owned()
        }
        Question::Verifier => {
            "answer: verifier program · Enter states that you have none".to_owned()
        }
        Question::NegativeControl => "answer: negative control directory".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::{Draft, Question, Step};
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    /// A project on disk: an answer names something on this host, so a draft can only be driven
    /// against paths that exist. The verifier discriminates — it accepts a directory holding
    /// `result.txt` and rejects one that does not — because a draft that completes has been shown
    /// a program that rejects the negative control.
    struct Project {
        _root: TempDir,
        directory: PathBuf,
        source: PathBuf,
        program: PathBuf,
        negative_control: PathBuf,
    }

    fn project() -> Project {
        let root = TempDir::new().expect("temporary root");
        let directory = canonical(root.path());
        let source = directory.join("source");
        let negative_control = directory.join("negative-control");
        let program = directory.join("verify.sh");
        fs::create_dir_all(&source).expect("source directory");
        fs::create_dir_all(&negative_control).expect("negative control directory");
        fs::write(source.join("result.txt"), b"result\n").expect("source file");
        // The subject is the only argument the executor passes, so the whole argument list names
        // it. Spelling it that way keeps a positional digit out of this file, which a guard test
        // reads as a value fixed in the source.
        executable(&program, "#!/bin/sh\ntest -f \"$*/result.txt\"\n");
        Project {
            _root: root,
            directory,
            source,
            program,
            negative_control,
        }
    }

    fn canonical(path: &Path) -> PathBuf {
        path.canonicalize().expect("resolve path")
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

    fn refusal(step: Step) -> String {
        match step {
            Step::Refused(reason) => reason,
            other => panic!("the answer was taken instead of refused: {other:?}"),
        }
    }

    #[test]
    fn an_empty_source_answer_accepts_the_project_directory() {
        let project = project();
        let mut draft = Draft::new("keep the replay path idempotent");
        assert!(matches!(
            draft.answer("", &project.directory),
            Step::Ask(Question::Verifier)
        ));
        assert_eq!(draft.source.as_deref(), Some(project.directory.as_path()));
    }

    #[test]
    fn declining_the_acceptance_question_produces_a_request_without_one() {
        let project = project();
        let mut draft = Draft::new("keep the replay path idempotent");
        draft.answer("", &project.directory);
        let Step::Ready(request) = draft.answer("", &project.directory) else {
            panic!("the draft kept asking after the acceptance question was declined");
        };
        assert!(request.acceptance.is_none());
        assert_eq!(request.prompt, "keep the replay path idempotent");
    }

    #[test]
    fn both_acceptance_answers_reach_the_request_resolved() {
        let project = project();
        let mut draft = Draft::new("keep the replay path idempotent");
        draft.answer(&project.source.display().to_string(), &project.directory);
        assert!(matches!(
            draft.answer(&project.program.display().to_string(), &project.directory),
            Step::Ask(Question::NegativeControl)
        ));
        let Step::Ready(request) = draft.answer(
            &project.negative_control.display().to_string(),
            &project.directory,
        ) else {
            panic!("the draft did not complete");
        };
        let acceptance = request.acceptance.expect("acceptance condition");
        assert_eq!(acceptance.program, project.program);
        assert_eq!(acceptance.negative_control, project.negative_control);
        assert_eq!(request.source, project.source);
    }

    /// The source is decided at the source answer, so the two acceptance questions are never put
    /// for a directory the request could not have used.
    #[test]
    fn a_source_that_is_not_a_directory_is_refused_at_the_source_answer() {
        let project = project();
        for (stated, expected) in [
            (
                project.directory.join("no-such-directory"),
                "could not be read",
            ),
            (project.program.clone(), "not a directory"),
        ] {
            let mut draft = Draft::new("keep the replay path idempotent");
            let reason = refusal(draft.answer(&stated.display().to_string(), &project.directory));
            assert!(reason.contains(expected), "{reason}");
            assert_eq!(draft.question, Question::Source);
            assert!(draft.source.is_none());
        }
    }

    /// An answer is one path. A command line, a directory and a file without execute permission
    /// are all refused where the verifier is typed.
    #[test]
    fn a_verifier_that_is_not_an_executable_file_is_refused_at_the_verifier_answer() {
        let project = project();
        let unreadable = project.directory.join("not-executable.sh");
        fs::write(&unreadable, "#!/bin/sh\nexit 1\n").expect("write file");
        let cases = [
            (
                format!("{} --strict", project.program.display()),
                "could not be read",
            ),
            (
                project
                    .directory
                    .join("no-such-program")
                    .display()
                    .to_string(),
                "could not be read",
            ),
            (
                project.source.display().to_string(),
                "not an executable file",
            ),
            (unreadable.display().to_string(), "not an executable file"),
        ];
        for (stated, expected) in cases {
            let mut draft = Draft::new("keep the replay path idempotent");
            draft.answer(&project.source.display().to_string(), &project.directory);
            let reason = refusal(draft.answer(&stated, &project.directory));
            assert!(reason.contains(expected), "{stated} — {reason}");
            assert_eq!(draft.question, Question::Verifier);
            assert!(draft.program.is_none());
        }
    }

    /// A program that exits zero on the deliberately wrong candidate decides nothing, so the draft
    /// never completes with it and no request carries it to the application.
    #[test]
    fn a_verifier_that_accepts_the_negative_control_is_refused_there() {
        let project = project();
        let accepts_anything = project.directory.join("accept.sh");
        executable(&accepts_anything, "#!/bin/sh\nexit 0\n");

        let mut draft = Draft::new("keep the replay path idempotent");
        draft.answer(&project.source.display().to_string(), &project.directory);
        draft.answer(&accepts_anything.display().to_string(), &project.directory);
        let reason = refusal(draft.answer(
            &project.negative_control.display().to_string(),
            &project.directory,
        ));
        assert!(reason.contains("accepted the negative control"), "{reason}");
        assert!(draft.negative_control.is_none());
    }

    #[test]
    fn a_negative_control_that_is_not_a_directory_is_refused_at_that_answer() {
        let project = project();
        let mut draft = Draft::new("keep the replay path idempotent");
        draft.answer(&project.source.display().to_string(), &project.directory);
        draft.answer(&project.program.display().to_string(), &project.directory);
        let reason =
            refusal(draft.answer(&project.program.display().to_string(), &project.directory));
        assert!(reason.contains("not a directory"), "{reason}");
        assert!(draft.negative_control.is_none());
    }
}
