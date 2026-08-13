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

use std::path::{Path, PathBuf};

use ymp_application::{AcceptanceCondition, RunRequest};

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
    pub fn answer(&mut self, text: &str, project: &Path) -> Step {
        let answer = text.trim();
        match self.question {
            Question::Source => {
                self.source = Some(if answer.is_empty() {
                    project.to_path_buf()
                } else {
                    PathBuf::from(answer)
                });
                self.question = Question::Verifier;
                Step::Ask(Question::Verifier)
            }
            Question::Verifier => {
                if answer.is_empty() {
                    return Step::Ready(Box::new(self.request()));
                }
                self.program = Some(PathBuf::from(answer));
                self.question = Question::NegativeControl;
                Step::Ask(Question::NegativeControl)
            }
            Question::NegativeControl => {
                if !answer.is_empty() {
                    self.negative_control = Some(PathBuf::from(answer));
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
    use std::path::{Path, PathBuf};

    #[test]
    fn an_empty_source_answer_accepts_the_project_directory() {
        let mut draft = Draft::new("keep the replay path idempotent");
        let project = Path::new("/tmp/checkout");
        assert!(matches!(
            draft.answer("", project),
            Step::Ask(Question::Verifier)
        ));
        assert_eq!(draft.source.as_deref(), Some(project));
    }

    #[test]
    fn declining_the_acceptance_question_produces_a_request_without_one() {
        let mut draft = Draft::new("keep the replay path idempotent");
        draft.answer("", Path::new("/tmp/checkout"));
        let Step::Ready(request) = draft.answer("", Path::new("/tmp/checkout")) else {
            panic!("the draft kept asking after the acceptance question was declined");
        };
        assert!(request.acceptance.is_none());
        assert_eq!(request.prompt, "keep the replay path idempotent");
    }

    #[test]
    fn both_acceptance_answers_reach_the_request_unchanged() {
        let mut draft = Draft::new("keep the replay path idempotent");
        draft.answer("/tmp/source", Path::new("/tmp/checkout"));
        assert!(matches!(
            draft.answer("/tmp/verify.sh", Path::new("/tmp/checkout")),
            Step::Ask(Question::NegativeControl)
        ));
        let Step::Ready(request) = draft.answer("/tmp/negative", Path::new("/tmp/checkout")) else {
            panic!("the draft did not complete");
        };
        let acceptance = request.acceptance.expect("acceptance condition");
        assert_eq!(acceptance.program, PathBuf::from("/tmp/verify.sh"));
        assert_eq!(acceptance.negative_control, PathBuf::from("/tmp/negative"));
        assert_eq!(request.source, PathBuf::from("/tmp/source"));
    }
}
