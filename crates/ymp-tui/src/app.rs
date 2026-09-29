//! Application state of the interface. Everything shown about a session is read
//! from its recorded projection. The interface itself keeps only what the user is
//! typing, where they are looking and what the runtime answered to their commands.
use crate::screens::pages::{self, Page, PageKind, UNKNOWN};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;
use std::sync::Arc;
use ymp_runtime::{
    application::{RecoveryIntent, SessionView},
    domain::{Denial, Id, Result, workspace::WorkspacePath},
    live_session::{CommandOutcome, LiveSession, OwnerCommand, Pulse},
};

/// An agent the composition offers. Agent, provider and model stay distinct.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub agent: String,
    pub provider: String,
    pub model: Option<String>,
    pub effort: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionRow {
    pub session: String,
    pub status: String,
    pub revision: u64,
    pub goal: String,
}
/// A file whose exact bytes the user requires, checked by the runtime.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expectation {
    pub path: String,
    pub bytes: Vec<u8>,
    /// The file must keep the bytes it had when the expectation was stated.
    pub preserve: bool,
}
/// What the user has stated before a session exists. Nothing here is recorded.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    /// The user's words, verbatim.
    pub goal: String,
    pub expectations: Vec<Expectation>,
    pub budget: f64,
}
impl Draft {
    /// A refused start must not leave a partly recorded session, so the draft is
    /// checked before the runtime is asked.
    pub fn check(&self) -> Result<()> {
        if self.goal.trim().is_empty() {
            return Err(Denial::new("draft_goal", "State the goal first"));
        }
        if self.expectations.is_empty() {
            return Err(Denial::new(
                "draft_checks",
                "State at least one checked expectation with /expect or /preserve",
            ));
        }
        if !(self.budget.is_finite() && self.budget > 0.0) {
            return Err(Denial::new("draft_budget", "The budget must be positive"));
        }
        Ok(())
    }
}
/// The composition behind the interface. The interface names what the user asked
/// for; the host composes the runtime and the kernel decides.
pub trait Host {
    /// One line naming the backend, store and workspace of this composition.
    fn summary(&self) -> String;
    fn members(&self) -> Vec<Member>;
    fn budget(&self) -> f64;
    fn now(&self) -> u64;
    fn sessions(&self) -> Result<Vec<SessionRow>>;
    /// A read of the recorded projection. It grants nothing and drives nothing.
    fn view(&self, session: &Id) -> Result<SessionView>;
    /// Current bytes of a workspace file, for a preserved expectation.
    fn file(&self, path: &str) -> Result<Vec<u8>>;
    fn start(&self, draft: &Draft) -> Result<(Id, LiveSession)>;
    fn recover(&self, session: &Id, intent: RecoveryIntent) -> Result<LiveSession>;
}
pub struct Projection {
    pub view: Arc<SessionView>,
    pub json: Value,
    /// This interface drives the session now.
    pub driving: bool,
    /// When this interface began to drive the session.
    pub since: u64,
}
impl Projection {
    pub fn new(view: Arc<SessionView>) -> Result<Self> {
        Ok(Self {
            json: serde_json::to_value(&*view)
                .map_err(|_| Denial::new("projection", "Cannot present the projection"))?,
            view,
            driving: false,
            since: 0,
        })
    }
    /// A call is said to work only while this interface drives the session,
    /// the provider has not ended and the call was dispatched by this drive.
    /// A call left by an earlier drive is not resumed and has no recorded end.
    pub fn runs(&self, call: &Value) -> bool {
        self.driving
            && call["terminal"].is_null()
            && call["backend_terminal"].is_null()
            && call["dispatch"]["at"]
                .as_u64()
                .is_some_and(|at| at >= self.since)
    }
    pub fn revision(&self) -> u64 {
        self.view.revision()
    }
    /// Questions the recorded intake decision asks that no clarification answers.
    pub fn pending_questions(&self) -> Vec<&Value> {
        let answered: Vec<&Value> = self.json["task"]["goal"]["clarifications"]
            .as_array()
            .map(|all| all.iter().map(|c| &c["question"]).collect())
            .unwrap_or_default();
        let mut pending = vec![];
        for record in self.json["planning"]["history"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let questions = &record[1]["Intake"]["decision"]["outcome"]["questions"];
            for decision in questions.as_array().into_iter().flatten() {
                let asked = &decision["Ask"];
                if !asked.is_null() && !answered.contains(&&asked["question"]) {
                    pending.push(asked);
                }
            }
        }
        pending
    }
    pub fn status(&self) -> String {
        pages::cell(&serde_json::to_value(self.view.status()).unwrap_or(Value::Null))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    User,
    Agent,
    Runtime,
    Good,
    Bad,
    Report,
    Note,
}
/// One attributed item of the conversation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Stable across projections, so a reading position can refer to it.
    pub key: String,
    pub at: u64,
    pub order: u32,
    pub tone: Tone,
    pub heading: String,
    pub body: Vec<String>,
    /// User requests, questions and the report weigh more than activity.
    pub major: bool,
}
pub struct CommandSpec {
    pub name: &'static str,
    pub argument: &'static str,
    pub help: &'static str,
}
pub const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        name: "expect",
        argument: "PATH TEXT",
        help: "Require a file to contain exactly TEXT (\\n, \\t and \\\\ are escapes)",
    },
    CommandSpec {
        name: "preserve",
        argument: "PATH",
        help: "Require a file to keep its current bytes",
    },
    CommandSpec {
        name: "budget",
        argument: "AMOUNT",
        help: "Set the budget limit of the next session",
    },
    CommandSpec {
        name: "start",
        argument: "",
        help: "Start a session from the stated goal and expectations",
    },
    CommandSpec {
        name: "interrupt",
        argument: "",
        help: "Stop the running session; the runtime reports without new model calls",
    },
    CommandSpec {
        name: "recover",
        argument: "continue|report",
        help: "Drive the opened session again, or only deliver its report",
    },
    CommandSpec {
        name: "open",
        argument: "SESSION",
        help: "Open a recorded session read-only",
    },
    CommandSpec {
        name: "new",
        argument: "",
        help: "Leave the session as recorded and state a new goal",
    },
    CommandSpec {
        name: "sessions",
        argument: "",
        help: "Recorded sessions of this store",
    },
    CommandSpec {
        name: "team",
        argument: "",
        help: "Agents, providers, models and pool eligibility",
    },
    CommandSpec {
        name: "criteria",
        argument: "",
        help: "Criteria with their recorded status and checks",
    },
    CommandSpec {
        name: "plan",
        argument: "",
        help: "Work items of the recorded plan",
    },
    CommandSpec {
        name: "assignments",
        argument: "",
        help: "Admitted assignments with role, profile and lease",
    },
    CommandSpec {
        name: "commitments",
        argument: "",
        help: "Commitments with debtor, creditor and state",
    },
    CommandSpec {
        name: "resources",
        argument: "",
        help: "Budget, reserves, holds and usage coverage",
    },
    CommandSpec {
        name: "activity",
        argument: "",
        help: "Invocations with settings, usage and full reported output",
    },
    CommandSpec {
        name: "results",
        argument: "",
        help: "Submitted results and their attempts",
    },
    CommandSpec {
        name: "checks",
        argument: "",
        help: "Checks and their recorded runs",
    },
    CommandSpec {
        name: "acceptance",
        argument: "",
        help: "Reviews and acceptance decisions",
    },
    CommandSpec {
        name: "report",
        argument: "",
        help: "The delivered report",
    },
    CommandSpec {
        name: "policies",
        argument: "",
        help: "Recorded strategy and adapter selections",
    },
    CommandSpec {
        name: "help",
        argument: "",
        help: "List the commands and keys",
    },
    CommandSpec {
        name: "quit",
        argument: "",
        help: "Leave; a running session stays recoverable",
    },
];
pub const PAGES: &[(&str, PageKind)] = &[
    ("sessions", PageKind::Sessions),
    ("team", PageKind::Team),
    ("criteria", PageKind::Criteria),
    ("plan", PageKind::Plan),
    ("assignments", PageKind::Assignments),
    ("commitments", PageKind::Commitments),
    ("resources", PageKind::Resources),
    ("activity", PageKind::Activity),
    ("results", PageKind::Results),
    ("checks", PageKind::Checks),
    ("acceptance", PageKind::Acceptance),
    ("report", PageKind::Report),
    ("policies", PageKind::Policies),
];
pub struct PageState {
    pub page: Page,
    /// Rows after filtering and sorting, as indexes into `page.records`.
    pub visible: Vec<usize>,
    pub selected: usize,
    pub filter: String,
    pub filtering: bool,
    /// Column and whether the order is descending.
    pub sort: Option<(usize, bool)>,
    /// Revision and driving state the rows were read at.
    read: Option<(u64, bool, u64)>,
}
impl PageState {
    pub fn record(&self) -> Option<&pages::Record> {
        self.visible
            .get(self.selected)
            .and_then(|index| self.page.records.get(*index))
    }
    fn arrange(&mut self) {
        let key = self.record().map(|record| record.key.clone());
        let needle = self.filter.to_lowercase();
        self.visible = (0..self.page.records.len())
            .filter(|index| {
                needle.is_empty()
                    || self.page.records[*index]
                        .cells
                        .iter()
                        .any(|cell| cell.to_lowercase().contains(&needle))
            })
            .collect();
        if let Some((column, descending)) = self.sort {
            let numeric = self.page.columns.get(column).is_some_and(|c| c.numeric);
            let records = &self.page.records;
            self.visible.sort_by(|a, b| {
                let (a, b) = (&records[*a].cells[column], &records[*b].cells[column]);
                // An unrecorded value has no place in the order; it stays last.
                let order = match (a == UNKNOWN, b == UNKNOWN) {
                    (true, true) => return std::cmp::Ordering::Equal,
                    (true, false) => return std::cmp::Ordering::Greater,
                    (false, true) => return std::cmp::Ordering::Less,
                    _ => match (numeric, a.parse::<f64>(), b.parse::<f64>()) {
                        (true, Ok(a), Ok(b)) => a.total_cmp(&b),
                        _ => a.cmp(b),
                    },
                };
                if descending { order.reverse() } else { order }
            });
        }
        self.selected = key
            .and_then(|key| {
                self.visible
                    .iter()
                    .position(|index| self.page.records[*index].key == key)
            })
            .unwrap_or(0);
    }
}
pub struct Detail {
    pub title: String,
    pub text: String,
    pub top: usize,
}
pub struct Palette {
    pub query: String,
    pub selected: usize,
}
impl Palette {
    pub fn matches(&self) -> Vec<&'static CommandSpec> {
        let needle = self.query.to_lowercase();
        COMMANDS
            .iter()
            .filter(|c| c.name.contains(&needle) || c.help.to_lowercase().contains(&needle))
            .collect()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Lines(isize),
    Pages(isize),
    Start,
    End,
}
/// The first visible line of the conversation while the user reads older activity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reading {
    pub key: String,
    pub line: usize,
    /// Distance from the beginning when the position was taken.
    pub index: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Composer,
    Sidebar,
    Page,
    Detail,
    Palette,
}
/// What the executable says after the terminal is restored.
pub struct Leaving {
    pub session: Option<Id>,
    pub revision: Option<u64>,
    pub status: Option<String>,
    /// The session still had work to do when the interface stopped driving it.
    pub driving: bool,
    /// Why the record could not be read again; the revision and status are then
    /// the last ones shown.
    pub unread: Option<Denial>,
}
pub struct App<H: Host> {
    pub host: H,
    pub draft: Draft,
    pub session: Option<Id>,
    live: Option<LiveSession>,
    pub pulse: Option<Pulse>,
    pub projection: Option<Projection>,
    pub members: Vec<Member>,
    pub notices: Vec<Entry>,
    pub input: String,
    /// Byte offset of the cursor in `input`.
    pub cursor: usize,
    pub page: Option<PageState>,
    pub detail: Option<Detail>,
    pub palette: Option<Palette>,
    pub sidebar_focus: bool,
    pub navigation: usize,
    /// The user's choice; without one the width of the terminal decides.
    pub sidebar: Option<bool>,
    /// Whether the last frame had room for the sidebar beside the conversation.
    pub wide: bool,
    pub reading: Option<Reading>,
    pub motions: Vec<Motion>,
    /// Changes whenever the conversation may read differently.
    pub generation: u64,
    pub quit: bool,
    /// When this interface began to drive the session.
    since: u64,
    /// The draft of a start until the runtime has recorded the session.
    started: Option<Draft>,
    armed: bool,
}
fn unescape(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}
impl<H: Host> App<H> {
    pub fn new(host: H) -> Self {
        let draft = Draft {
            goal: String::new(),
            expectations: vec![],
            budget: host.budget(),
        };
        let members = host.members();
        let mut app = Self {
            host,
            draft,
            session: None,
            live: None,
            pulse: None,
            projection: None,
            members,
            notices: vec![],
            input: String::new(),
            cursor: 0,
            page: None,
            detail: None,
            palette: None,
            sidebar_focus: false,
            navigation: 0,
            sidebar: None,
            wide: true,
            reading: None,
            motions: vec![],
            generation: 0,
            quit: false,
            since: 0,
            started: None,
            armed: false,
        };
        app.note(
            Tone::Note,
            "ymp",
            vec![
                "State the goal in your own words and press Enter.".into(),
                "Name what must be true with /expect PATH TEXT or /preserve PATH; /start begins."
                    .into(),
                "Ctrl+P lists every command.".into(),
            ],
        );
        app
    }
    pub fn focus(&self) -> Focus {
        if self.palette.is_some() {
            Focus::Palette
        } else if self.detail.is_some() {
            Focus::Detail
        } else if self.page.is_some() {
            Focus::Page
        } else if self.sidebar_focus {
            Focus::Sidebar
        } else {
            Focus::Composer
        }
    }
    pub fn driving(&self) -> bool {
        self.live.is_some()
    }
    /// Stop driving and say what is recorded now. A command sent just before
    /// leaving may still be recorded, so the record is read after the session
    /// thread has returned.
    pub fn leaving(&mut self) -> Leaving {
        self.refresh();
        let mut driving = self.working();
        if let Some(live) = self.live.take() {
            live.close();
        }
        let mut unread = None;
        match self.session.as_ref().map(|session| self.host.view(session)) {
            Some(Ok(view)) if view.opened() => {
                // The last tick may have delivered the report.
                driving &= view.finalization().delivered.is_none();
                self.adopt(view);
            }
            // Nothing of this session was recorded.
            Some(Ok(_)) => self.session = None,
            Some(Err(denial)) => unread = Some(denial),
            None => {}
        }
        Leaving {
            session: self.session.clone(),
            revision: self.projection.as_ref().map(Projection::revision),
            status: self.projection.as_ref().map(Projection::status),
            driving,
            unread,
        }
    }
    /// The session thread still has recorded work to do.
    pub fn working(&self) -> bool {
        self.live.is_some()
            && matches!(
                self.pulse,
                Some(Pulse::Opening | Pulse::Working | Pulse::NeedsUser)
            )
    }
    fn note(&mut self, tone: Tone, heading: &str, body: Vec<String>) {
        let at = self
            .host
            .now()
            .max(self.projection.as_ref().map_or(0, |p| p.view.latest_at()));
        self.notices.push(Entry {
            key: format!("notice:{}", self.notices.len()),
            at,
            order: 900 + self.notices.len() as u32,
            tone,
            heading: heading.into(),
            body,
            major: false,
        });
        self.generation += 1;
    }
    /// A refusal names its code and what is known about the recorded state.
    fn denied(&mut self, action: &str, denial: &Denial, revisions: Option<(u64, u64)>) {
        let state = match revisions {
            Some((before, after)) if before == after => {
                format!("This recorded nothing; the revision shown is {after}.")
            }
            Some((before, after)) => {
                format!("The recorded revision moved from {before} to {after}.")
            }
            None => "Nothing was recorded.".into(),
        };
        self.note(
            Tone::Bad,
            &format!("Denied: {action}"),
            vec![format!("{}: {}", denial.code, denial.message), state],
        );
    }
    fn adopt(&mut self, view: SessionView) {
        match Projection::new(Arc::new(view)) {
            Ok(projection) => self.projection = Some(projection),
            Err(denial) => self.denied("present the session", &denial, None),
        }
        self.generation += 1;
    }
    /// Take what the session thread published since the last frame.
    pub fn refresh(&mut self) {
        let Some(live) = &self.live else { return };
        // Asked first: what an ended thread published is its last word.
        let ended = live.ended();
        let published = live.published();
        let outcomes = live.outcomes().unwrap_or_default();
        match published {
            Ok(published) => {
                if let Some(view) = published.view
                    && self
                        .projection
                        .as_ref()
                        .is_none_or(|current| !Arc::ptr_eq(&current.view, &view))
                {
                    match Projection::new(view) {
                        Ok(projection) => self.projection = Some(projection),
                        Err(denial) => self.denied("present the session", &denial, None),
                    }
                    self.generation += 1;
                }
                if self.pulse.as_ref() != Some(&published.pulse) {
                    if let Pulse::Failed(denial) = &published.pulse
                        && !ended
                    {
                        // The step may have recorded something before it failed.
                        let revision = self.projection.as_ref().map(Projection::revision);
                        self.note(
                            Tone::Bad,
                            "Failed: advance the session",
                            vec![
                                format!("{}: {}", denial.code, denial.message),
                                match revision {
                                    Some(revision) => {
                                        format!("The recorded revision is {revision}.")
                                    }
                                    None => "No revision is shown yet.".into(),
                                },
                            ],
                        );
                    }
                    self.pulse = Some(published.pulse);
                    self.generation += 1;
                }
            }
            Err(denial) if ended => self.pulse = Some(Pulse::Failed(denial)),
            Err(denial) => self.denied("read the session", &denial, None),
        }
        for outcome in outcomes {
            self.outcome(outcome);
        }
        if ended {
            self.released();
        }
        let driving = self.working();
        let since = self.since;
        if let Some(projection) = &mut self.projection
            && (projection.driving, projection.since) != (driving, since)
        {
            projection.driving = driving;
            projection.since = since;
            self.generation += 1;
        }
    }
    fn outcome(&mut self, outcome: CommandOutcome) {
        let action = match &outcome.command {
            OwnerCommand::Interrupt => "interrupt",
            OwnerCommand::Answer { .. } => "answer",
        };
        match outcome.denial {
            Some(denial) => self.denied(action, &denial, Some((outcome.before, outcome.after))),
            None => self.note(
                Tone::Runtime,
                &format!("Recorded: {action}"),
                vec![format!(
                    "Revision {} follows {}.",
                    outcome.after, outcome.before
                )],
            ),
        }
    }
    /// The session thread returned: the runtime refused to drive, or it was closed.
    fn released(&mut self) {
        let before = self.projection.as_ref().map(Projection::revision);
        let pulse = self.pulse.take();
        self.live = None;
        let recorded = self.session.clone().map(|session| self.host.view(&session));
        let mut after = None;
        if let Some(Ok(view)) = recorded {
            after = Some(view.revision());
            if view.opened() {
                self.adopt(view);
            } else if let Some(draft) = self.started.take() {
                // The start recorded nothing, so no session exists and the
                // stated goal and expectations are the draft again.
                self.session = None;
                self.projection = None;
                self.draft = draft;
            }
        }
        self.started = None;
        let denial = match pulse {
            Some(Pulse::Failed(denial)) => Some(denial),
            Some(Pulse::Closed) => None,
            // The thread returned without saying why: it did not end by itself.
            _ => Some(Denial::new(
                "live_session",
                match crate::events::fault() {
                    Some(fault) => format!("The session thread ended abnormally: {fault}"),
                    None => "The session thread ended abnormally".into(),
                },
            )),
        };
        if let Some(denial) = denial {
            let revisions = match (before, after) {
                (Some(before), Some(after)) => Some((before, after)),
                (None, Some(after)) if after > 0 => Some((0, after)),
                _ => None,
            };
            self.denied("drive the session", &denial, revisions);
        }
        if let Some(projection) = &mut self.projection {
            projection.driving = false;
        }
        self.generation += 1;
    }
    /// A provider call may be in flight. Closing the session thread would leave
    /// that call without an owner, so the session is interrupted or left first.
    fn engaged(&mut self, action: &str) -> bool {
        if !matches!(self.pulse, Some(Pulse::Opening | Pulse::Working)) || self.live.is_none() {
            return false;
        }
        let denial = Denial::new(
            "session_working",
            "This interface drives a working session; /interrupt stops it and /quit leaves it recoverable",
        );
        let revision = self.projection.as_ref().map(Projection::revision);
        self.denied(action, &denial, revision.map(|r| (r, r)));
        true
    }
    pub fn paste(&mut self, text: &str) {
        if matches!(self.focus(), Focus::Composer) {
            let text = text.replace("\r\n", "\n").replace('\r', "\n");
            self.input.insert_str(self.cursor, &text);
            self.cursor += text.len();
        }
    }
    pub fn key(&mut self, key: KeyEvent) {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        if control && key.code == KeyCode::Char('c') {
            self.interrupt_key();
            return;
        }
        self.armed = false;
        if control && key.code == KeyCode::Char('p') {
            self.palette = match self.palette {
                Some(_) => None,
                None => Some(Palette {
                    query: String::new(),
                    selected: 0,
                }),
            };
            return;
        }
        if control && key.code == KeyCode::Char('b') {
            self.toggle_sidebar(self.wide);
            return;
        }
        match self.focus() {
            Focus::Palette => self.palette_key(key),
            Focus::Detail => self.detail_key(key),
            Focus::Page => self.page_key(key),
            Focus::Sidebar => self.sidebar_key(key),
            Focus::Composer => self.composer_key(key),
        }
    }
    /// The width decides until the user has chosen; the renderer tells the width.
    pub fn sidebar_shown(&self, wide: bool) -> bool {
        self.sidebar.unwrap_or(wide)
    }
    pub fn toggle_sidebar(&mut self, wide: bool) {
        self.sidebar = Some(!self.sidebar_shown(wide));
        if self.sidebar == Some(false) {
            self.sidebar_focus = false;
        }
    }
    fn interrupt_key(&mut self) {
        if self.armed {
            self.quit = true;
            return;
        }
        self.armed = true;
        if self.working() {
            self.interrupt();
            self.note(
                Tone::Note,
                "Interrupt requested",
                vec!["Press Ctrl+C again to leave.".into()],
            );
        } else {
            self.note(
                Tone::Note,
                "Nothing is running",
                vec!["Press Ctrl+C again to leave.".into()],
            );
        }
    }
    fn interrupt(&mut self) {
        let sent = match &self.live {
            Some(live) if self.working() => live.send(OwnerCommand::Interrupt),
            // A stop recorded after the work ended would say nothing.
            Some(_) => Err(Denial::new(
                "not_working",
                "No work is running in this session; there is nothing to interrupt",
            )),
            None => Err(Denial::new(
                "not_driving",
                "This interface is not driving a session; /recover report delivers the report of an opened one",
            )),
        };
        if let Err(denial) = sent {
            let revision = self.projection.as_ref().map(Projection::revision);
            self.denied("interrupt", &denial, revision.map(|r| (r, r)));
        }
    }
    fn composer_key(&mut self, key: KeyEvent) {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('j') if control => self.insert('\n'),
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => self.insert('\n'),
            KeyCode::Char('u') if control => {
                self.input.clear();
                self.cursor = 0;
            }
            KeyCode::Char('a') if control => self.cursor = 0,
            KeyCode::Char('e') if control => self.cursor = self.input.len(),
            KeyCode::Char(c) if !control => self.insert(c),
            KeyCode::Enter => self.submit(),
            KeyCode::Backspace => {
                if let Some((index, _)) = self.input[..self.cursor].char_indices().next_back() {
                    self.input.remove(index);
                    self.cursor = index;
                }
            }
            KeyCode::Delete => {
                if self.cursor < self.input.len() {
                    self.input.remove(self.cursor);
                }
            }
            KeyCode::Left => {
                self.cursor = self.input[..self.cursor]
                    .char_indices()
                    .next_back()
                    .map_or(0, |(index, _)| index);
            }
            KeyCode::Right => {
                if let Some(c) = self.input[self.cursor..].chars().next() {
                    self.cursor += c.len_utf8();
                }
            }
            KeyCode::Tab => self.complete(),
            KeyCode::Esc => {
                if self.input.is_empty() {
                    self.motions.push(Motion::End);
                } else {
                    self.input.clear();
                    self.cursor = 0;
                }
            }
            KeyCode::PageUp => self.motions.push(Motion::Pages(-1)),
            KeyCode::PageDown => self.motions.push(Motion::Pages(1)),
            KeyCode::Up => self.motions.push(Motion::Lines(-1)),
            KeyCode::Down => self.motions.push(Motion::Lines(1)),
            KeyCode::Home if self.input.is_empty() => self.motions.push(Motion::Start),
            KeyCode::End if self.input.is_empty() => self.motions.push(Motion::End),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.input.len(),
            _ => {}
        }
    }
    fn insert(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }
    /// Commands the typed prefix can still become.
    pub fn completions(&self) -> Vec<&'static CommandSpec> {
        match self.input.strip_prefix('/') {
            Some(typed) if !typed.contains(char::is_whitespace) => COMMANDS
                .iter()
                .filter(|command| command.name.starts_with(typed))
                .collect(),
            _ => vec![],
        }
    }
    fn complete(&mut self) {
        let candidates = self.completions();
        if self.input.is_empty() || candidates.is_empty() {
            if self.input.is_empty() {
                self.sidebar_focus = true;
                if self.sidebar == Some(false) {
                    self.sidebar = Some(true);
                }
            }
            return;
        }
        let first = candidates[0].name;
        let shared = candidates.iter().fold(first.len(), |length, command| {
            first
                .bytes()
                .zip(command.name.bytes())
                .take(length)
                .take_while(|(a, b)| a == b)
                .count()
        });
        self.input = if candidates.len() == 1 {
            format!("/{first} ")
        } else {
            format!("/{}", &first[..shared])
        };
        self.cursor = self.input.len();
    }
    fn submit(&mut self) {
        let text = std::mem::take(&mut self.input);
        self.cursor = 0;
        if text.trim().is_empty() {
            return;
        }
        self.motions.push(Motion::End);
        match text.strip_prefix('/') {
            Some(command) => {
                let (name, argument) = match command.split_once(char::is_whitespace) {
                    Some((name, argument)) => (name, argument.trim()),
                    None => (command.trim(), ""),
                };
                self.command(name, argument);
            }
            None => self.text(text),
        }
    }
    /// Plain text is the goal before a session exists and an answer while the
    /// recorded intake waits for one. It never starts or continues work.
    fn text(&mut self, text: String) {
        if self.session.is_none() {
            self.draft.goal = text;
            self.generation += 1;
            let next = if self.draft.expectations.is_empty() {
                vec![
                    "A session needs at least one checked expectation.".into(),
                    "Add /expect PATH TEXT or /preserve PATH, then /start.".into(),
                ]
            } else {
                vec!["/start begins the session; text alone starts nothing.".into()]
            };
            self.note(Tone::Note, "Goal stated; not started", next);
            return;
        }
        let pending = self.projection.as_ref().and_then(|projection| {
            projection
                .pending_questions()
                .first()
                .and_then(|question| question["question"].as_str())
                .map(|question| (question.to_string(), projection.revision()))
        });
        match (&self.live, &self.pulse, pending) {
            (Some(live), Some(Pulse::NeedsUser), Some((question, expected_revision))) => {
                if let Err(denial) = live.send(OwnerCommand::Answer {
                    question,
                    answer: text,
                    expected_revision,
                }) {
                    self.denied(
                        "answer",
                        &denial,
                        Some((expected_revision, expected_revision)),
                    );
                }
            }
            _ => self.note(
                Tone::Note,
                "Text was not sent",
                vec![
                    "This session accepts commands and answers to recorded questions only.".into(),
                    "Nothing was recorded and no work was started.".into(),
                ],
            ),
        }
    }
    fn start(&mut self) {
        if let Some(session) = &self.session {
            let denial = Denial::new(
                "session_open",
                format!(
                    "Session {} is open; /new leaves it as recorded",
                    session.as_str()
                ),
            );
            let revision = self.projection.as_ref().map(Projection::revision);
            self.denied("start", &denial, revision.map(|r| (r, r)));
            return;
        }
        self.since = self.host.now();
        let started = self
            .draft
            .check()
            .and_then(|()| self.host.start(&self.draft));
        match started {
            Ok((session, live)) => {
                self.session = Some(session);
                self.live = Some(live);
                self.pulse = Some(Pulse::Opening);
                self.projection = None;
                self.started = Some(self.draft.clone());
                self.draft.goal.clear();
                self.draft.expectations.clear();
                self.generation += 1;
            }
            Err(denial) => self.denied("start", &denial, None),
        }
    }
    fn open(&mut self, session: &str) {
        if self.engaged("open") {
            return;
        }
        let opened = Id::new(session).and_then(|session| {
            let view = self.host.view(&session)?;
            if !view.opened() {
                return Err(Denial::new("session_missing", "No such recorded session"));
            }
            Ok((session, view))
        });
        match opened {
            Ok((session, view)) => {
                self.leave();
                self.session = Some(session);
                self.adopt(view);
                self.page = None;
                self.detail = None;
                self.note(
                    Tone::Note,
                    "Opened read-only",
                    vec![
                        "Reading a session drives nothing.".into(),
                        "/recover continue drives it again when the runtime permits; /recover report only delivers its report.".into(),
                    ],
                );
            }
            Err(denial) => self.denied("open", &denial, None),
        }
    }
    /// Stop driving. Nothing is recorded by leaving.
    fn leave(&mut self) {
        if let Some(live) = self.live.take() {
            live.close();
        }
        self.session = None;
        self.started = None;
        self.pulse = None;
        self.projection = None;
        self.reading = None;
        self.notices.clear();
        self.generation += 1;
    }
    fn recover(&mut self, argument: &str) {
        let intent = match argument {
            "continue" => RecoveryIntent::Continue,
            "report" => RecoveryIntent::Report,
            _ => {
                let denial = Denial::new("usage", "Use /recover continue or /recover report");
                self.denied("recover", &denial, None);
                return;
            }
        };
        let revision = self.projection.as_ref().map(Projection::revision);
        let unchanged = revision.map(|r| (r, r));
        let Some(session) = self.session.clone() else {
            let denial = Denial::new("session_missing", "Open a session first with /open");
            self.denied("recover", &denial, None);
            return;
        };
        if self.working() {
            let denial = Denial::new("already_driving", "This interface already drives it");
            self.denied("recover", &denial, unchanged);
            return;
        }
        // A thread that has nothing left to do is released; the runtime decides
        // from the record whether anything may be driven again.
        if let Some(live) = self.live.take() {
            live.close();
            self.pulse = None;
        }
        self.since = self.host.now();
        match self.host.recover(&session, intent) {
            Ok(live) => {
                self.live = Some(live);
                self.pulse = Some(Pulse::Opening);
                self.note(
                    Tone::Runtime,
                    &format!("Recovery requested: {argument}"),
                    vec![
                        "The runtime decides from the recorded state what may continue.".into(),
                        "Work that was in flight is not resumed; its holds and unknown usage stay as recorded.".into(),
                    ],
                );
            }
            Err(denial) => self.denied("recover", &denial, unchanged),
        }
    }
    pub fn command(&mut self, name: &str, argument: &str) {
        if let Some((_, kind)) = PAGES.iter().find(|(page, _)| *page == name) {
            self.show(*kind);
            return;
        }
        match name {
            "expect" | "preserve" => self.expect(name == "preserve", argument),
            "budget" => match argument.parse::<f64>() {
                Ok(budget) if budget.is_finite() && budget > 0.0 && self.session.is_none() => {
                    self.draft.budget = budget;
                    self.generation += 1;
                }
                _ => {
                    let denial = Denial::new(
                        "usage",
                        "Use /budget AMOUNT with a positive amount before the session starts",
                    );
                    let revision = self.projection.as_ref().map(Projection::revision);
                    self.denied("budget", &denial, revision.map(|r| (r, r)));
                }
            },
            "start" => self.start(),
            "interrupt" => self.interrupt(),
            "recover" => self.recover(argument),
            "open" => self.open(argument),
            "new" => {
                if self.engaged("new") {
                    return;
                }
                if self.session.is_none() {
                    self.draft.goal.clear();
                    self.draft.expectations.clear();
                    self.generation += 1;
                    self.note(
                        Tone::Note,
                        "Draft cleared",
                        vec![
                            "The goal and the expectations were dropped; nothing was recorded."
                                .into(),
                        ],
                    );
                    return;
                }
                self.leave();
                self.note(
                    Tone::Note,
                    "New goal",
                    vec!["The previous session stays as recorded; /sessions lists it.".into()],
                );
            }
            "help" => {
                let mut lines: Vec<String> = COMMANDS
                    .iter()
                    .map(|c| format!("/{} {} — {}", c.name, c.argument, c.help))
                    .collect();
                lines.push(
                    "Keys: Enter send · Ctrl+J newline · Tab complete or move focus · Ctrl+P commands · Ctrl+B sidebar · PageUp/PageDown scroll · End follow · Ctrl+C interrupt, again to leave".into(),
                );
                self.note(Tone::Note, "Commands", lines);
            }
            "quit" => self.quit = true,
            _ => {
                let denial = Denial::new("unknown_command", format!("No command /{name}"));
                let revision = self.projection.as_ref().map(Projection::revision);
                self.denied("command", &denial, revision.map(|r| (r, r)));
            }
        }
    }
    fn expect(&mut self, preserve: bool, argument: &str) {
        let stated = if self.session.is_some() {
            Err(Denial::new(
                "session_open",
                "Expectations belong to a new session; /new begins one",
            ))
        } else if preserve {
            if argument.is_empty() || argument.contains(char::is_whitespace) {
                Err(Denial::new("usage", "Use /preserve PATH"))
            } else {
                self.host.file(argument).map(|bytes| Expectation {
                    path: argument.into(),
                    bytes,
                    preserve,
                })
            }
        } else {
            match argument.split_once(char::is_whitespace) {
                Some((path, text)) if !text.is_empty() => {
                    WorkspacePath::new(path).map(|_| Expectation {
                        path: path.into(),
                        bytes: unescape(text).into_bytes(),
                        preserve,
                    })
                }
                _ => Err(Denial::new("usage", "Use /expect PATH TEXT")),
            }
        };
        match stated {
            Ok(expectation) => {
                self.draft
                    .expectations
                    .retain(|known| known.path != expectation.path);
                self.draft.expectations.push(expectation);
                self.generation += 1;
            }
            Err(denial) => {
                let revision = self.projection.as_ref().map(Projection::revision);
                self.denied("expectation", &denial, revision.map(|r| (r, r)));
            }
        }
    }
    /// Opening a page reads the projection; it starts no work.
    pub fn show(&mut self, kind: PageKind) {
        let (filter, sort) = match self.page.take() {
            Some(state) if state.page.kind == kind => (state.filter, state.sort),
            _ => (String::new(), None),
        };
        let sessions = if kind == PageKind::Sessions {
            match self.host.sessions() {
                Ok(sessions) => sessions,
                Err(denial) => {
                    self.denied("list sessions", &denial, None);
                    vec![]
                }
            }
        } else {
            vec![]
        };
        let mut state = PageState {
            page: pages::build(kind, self.projection.as_ref(), &self.members, &sessions),
            visible: vec![],
            selected: 0,
            filter,
            filtering: false,
            sort,
            read: self
                .projection
                .as_ref()
                .map(|p| (p.revision(), p.driving, p.since)),
        };
        state.arrange();
        self.page = Some(state);
        self.detail = None;
        self.palette = None;
        self.sidebar_focus = false;
    }
    /// Keep an open page on the current projection without moving the selection.
    pub fn follow_page(&mut self) {
        let read = self
            .projection
            .as_ref()
            .map(|p| (p.revision(), p.driving, p.since));
        let Some(state) = &mut self.page else { return };
        if state.read == read || state.page.kind == PageKind::Sessions {
            return;
        }
        state.page = pages::build(
            state.page.kind,
            self.projection.as_ref(),
            &self.members,
            &[],
        );
        state.read = read;
        state.arrange();
    }
    fn page_key(&mut self, key: KeyEvent) {
        let Some(state) = &mut self.page else { return };
        if state.filtering {
            match key.code {
                KeyCode::Esc => {
                    state.filtering = false;
                    state.filter.clear();
                }
                KeyCode::Enter => state.filtering = false,
                KeyCode::Backspace => {
                    state.filter.pop();
                }
                KeyCode::Char(c) => state.filter.push(c),
                _ => {}
            }
            state.arrange();
            return;
        }
        let last = state.visible.len().saturating_sub(1);
        match key.code {
            KeyCode::Esc if !state.filter.is_empty() => {
                state.filter.clear();
                state.arrange();
            }
            KeyCode::Esc | KeyCode::Char('q') => self.page = None,
            KeyCode::Up | KeyCode::Char('k') => state.selected = state.selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => state.selected = (state.selected + 1).min(last),
            KeyCode::PageUp => state.selected = state.selected.saturating_sub(10),
            KeyCode::PageDown => state.selected = (state.selected + 10).min(last),
            KeyCode::Home | KeyCode::Char('g') => state.selected = 0,
            KeyCode::End | KeyCode::Char('G') => state.selected = last,
            KeyCode::Char('/') => state.filtering = true,
            KeyCode::Char('s') => {
                let columns = state.page.columns.len();
                state.sort = match state.sort {
                    None => Some((0, false)),
                    Some((column, _)) if column + 1 >= columns => None,
                    Some((column, descending)) => Some((column + 1, descending)),
                };
                state.arrange();
            }
            KeyCode::Char('S') => {
                state.sort = Some(match state.sort {
                    None => (0, true),
                    Some((column, descending)) => (column, !descending),
                });
                state.arrange();
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let current = PAGES
                    .iter()
                    .position(|(_, kind)| *kind == state.page.kind)
                    .unwrap_or(0);
                let next = if key.code == KeyCode::Tab {
                    (current + 1) % PAGES.len()
                } else {
                    (current + PAGES.len() - 1) % PAGES.len()
                };
                self.show(PAGES[next].1);
            }
            KeyCode::Char('r') => {
                let kind = state.page.kind;
                self.show(kind);
            }
            KeyCode::Enter | KeyCode::Char('d') => {
                let sessions = state.page.kind == PageKind::Sessions;
                let Some(record) = state.record() else { return };
                if sessions && key.code == KeyCode::Enter {
                    let session = record.key.clone();
                    self.open(&session);
                } else {
                    self.detail = Some(Detail {
                        title: format!("{} · {}", state.page.kind.title(), record.key),
                        text: record.detail.clone(),
                        top: 0,
                    });
                }
            }
            _ => {}
        }
    }
    fn detail_key(&mut self, key: KeyEvent) {
        let Some(detail) = &mut self.detail else {
            return;
        };
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.detail = None,
            KeyCode::Up | KeyCode::Char('k') => detail.top = detail.top.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => detail.top += 1,
            KeyCode::PageUp => detail.top = detail.top.saturating_sub(10),
            KeyCode::PageDown => detail.top += 10,
            KeyCode::Home => detail.top = 0,
            KeyCode::End => detail.top = usize::MAX,
            _ => {}
        }
    }
    fn palette_key(&mut self, key: KeyEvent) {
        let Some(palette) = &mut self.palette else {
            return;
        };
        let matches = palette.matches();
        match key.code {
            KeyCode::Esc => self.palette = None,
            KeyCode::Up => palette.selected = palette.selected.saturating_sub(1),
            KeyCode::Down => {
                palette.selected = (palette.selected + 1).min(matches.len().saturating_sub(1));
            }
            KeyCode::Backspace => {
                palette.query.pop();
                palette.selected = 0;
            }
            KeyCode::Char(c) => {
                palette.query.push(c);
                palette.selected = 0;
            }
            KeyCode::Enter => {
                let chosen = matches.get(palette.selected).copied();
                self.palette = None;
                if let Some(command) = chosen {
                    if command.argument.is_empty() {
                        self.command(command.name, "");
                    } else {
                        // A command with an argument is completed by the user.
                        self.page = None;
                        self.detail = None;
                        self.input = format!("/{} ", command.name);
                        self.cursor = self.input.len();
                    }
                }
            }
            _ => {}
        }
    }
    fn sidebar_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Tab | KeyCode::BackTab => self.sidebar_focus = false,
            KeyCode::Up | KeyCode::Char('k') => {
                self.navigation = self.navigation.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.navigation = (self.navigation + 1).min(PAGES.len() - 1);
            }
            KeyCode::Enter => self.show(PAGES[self.navigation].1),
            _ => {}
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::cell::Cell;

    /// A composition that counts what it was asked to drive and refuses it.
    #[derive(Default)]
    struct Counting {
        started: Cell<u32>,
        recovered: Cell<u32>,
    }
    impl Host for &Counting {
        fn summary(&self) -> String {
            "counting".into()
        }
        fn members(&self) -> Vec<Member> {
            vec![]
        }
        fn budget(&self) -> f64 {
            100.0
        }
        fn now(&self) -> u64 {
            1
        }
        fn sessions(&self) -> Result<Vec<SessionRow>> {
            Ok(vec![])
        }
        fn view(&self, _: &Id) -> Result<SessionView> {
            Err(Denial::new("view", "No view in this test"))
        }
        fn file(&self, _: &str) -> Result<Vec<u8>> {
            Ok(b"kept".to_vec())
        }
        fn start(&self, _: &Draft) -> Result<(Id, LiveSession)> {
            self.started.set(self.started.get() + 1);
            Err(Denial::new("refused", "This test starts nothing"))
        }
        fn recover(&self, _: &Id, _: RecoveryIntent) -> Result<LiveSession> {
            self.recovered.set(self.recovered.get() + 1);
            Err(Denial::new("refused", "This test recovers nothing"))
        }
    }
    fn say<H: Host>(app: &mut App<H>, line: &str) {
        for c in line.chars() {
            app.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    }
    #[test]
    fn text_alone_starts_nothing() {
        let host = Counting::default();
        let mut app = App::new(&host);
        say(&mut app, "/expect out.txt done\\n");
        say(&mut app, "/preserve in.txt");
        say(&mut app, "Write the file");
        say(&mut app, "start");
        say(&mut app, "/recover continue");
        assert_eq!(app.draft.expectations.len(), 2);
        assert_eq!(app.draft.expectations[0].bytes, b"done\n");
        assert_eq!(app.draft.goal, "start");
        assert_eq!((host.started.get(), host.recovered.get()), (0, 0));
        say(&mut app, "/start");
        assert_eq!(host.started.get(), 1);
        assert!(app.session.is_none());
    }
    #[test]
    fn text_in_an_opened_session_continues_nothing() {
        let host = Counting::default();
        let mut app = App::new(&host);
        app.session = Some(Id::new("session-1").unwrap());
        let before = app.notices.len();
        say(&mut app, "continue");
        say(&mut app, "/recover continue please");
        assert_eq!((host.started.get(), host.recovered.get()), (0, 0));
        assert_eq!(app.notices.len(), before + 2);
        assert_eq!(app.notices[before].heading, "Text was not sent");
        assert_eq!(app.notices[before + 1].heading, "Denied: recover");
        // Only the exact user command asks the runtime.
        say(&mut app, "/recover continue");
        assert_eq!(host.recovered.get(), 1);
    }
}
