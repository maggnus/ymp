//! Application state, the event model and the keyboard contract.
//!
//! `App` owns everything the interface knows and is the only place that decides what a key
//! means. It reads from the store and the configuration directly, so navigating is a pure
//! read; anything that starts, stops or continues a run leaves here as an [`Action`] for the
//! event loop to execute. That split is what keeps read-only navigation provably read-only,
//! and what lets the keyboard contract be tested without a terminal or a provider.

use crate::commands;
use crate::exit;
use crate::frame;
use crate::label;
use crate::prefs::Prefs;
use crate::provenance::{Attribution, Pool, Records};
use crate::text;
use crate::theme::{self, Theme};
use crate::transcript::{self, Entry, Notice};
use crate::usage::Stats;
use crate::views::{self, Ctx, Page, View};
use anyhow::{bail, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::text::Line;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Instant;
use ymp_core::{AgentProfile, Config, Message, SessionUsage, Task, UiEvent};
use ymp_storage::Store;
use ymp_workspace::repository::{self, Repository};

/// The runtime reports tool use inside a turn as agent activity: this prefix, then the tool.
const TOOL_ACTIVITY: &str = "tool: ";

/// The region that owns the keyboard. Exactly one is active at any moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Composer,
    Main,
}

impl Focus {
    pub fn label(self) -> &'static str {
        match self {
            Focus::Composer => "composer",
            Focus::Main => "main",
        }
    }
}

/// What a floating surface is asking for.
#[derive(Clone, Debug)]
pub enum Overlay {
    /// Search and run any command.
    Palette { field: Field, selected: usize },
    /// Choose a colour theme, previewed live.
    Themes { selected: usize, original: String },
    /// The complete attributed text of one message.
    Inspect {
        title: String,
        body: Vec<Line<'static>>,
        scroll: usize,
    },
    /// Type a single value.
    Prompt {
        target: PromptTarget,
        label: String,
        help: String,
        field: Field,
    },
    /// Confirm something that cannot be undone.
    Confirm { question: String, target: Confirm },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromptTarget {
    AgentModel(String),
    AgentInstructions(String),
    Limit(&'static str),
    MemorySearch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Confirm {
    ForgetMemory(String),
}

/// Work the event loop must perform. Everything that can start or stop a provider turn
/// travels through this type; nothing in `App` does it directly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Quit,
    /// Begin an unrelated session with this prompt.
    StartRun {
        prompt: String,
    },
    /// Continue an existing conversation. The engine decides whether new work is needed.
    FollowUp {
        session: String,
        prompt: String,
    },
    /// Deliver a message to a run that is already active.
    QueueMessage {
        session: String,
        text: String,
    },
    /// Continue an interrupted run.
    Resume {
        session: String,
    },
    /// Stop active turns.
    Cancel,
    /// Ask the installations what they offer, which is the only scan the interface performs.
    RefreshCatalog {
        /// One provider, or every enabled provider when absent.
        provider: Option<String>,
    },
}

/// Where a submitted line of text belongs. Kept separate from the key handler so the
/// routing rule can be checked without a running engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    /// A slash command.
    Command,
    /// A run is active: the message joins its shared chat at the next turn boundary.
    Queue(String),
    /// Idle with a loaded conversation: answer in the same conversation.
    FollowUp(String),
    /// Idle with no conversation: open a new session.
    NewRun,
    /// A run is active but has not reported a session yet.
    Busy,
}

/// Decide where a submitted line goes.
///
/// An idle prompt continues the loaded conversation. Only `/new`, or a session that was
/// never opened, starts a fresh run, because a question about finished work must not
/// allocate a new task graph.
pub fn route(input: &str, active: bool, session: Option<&str>) -> Route {
    if input.starts_with('/') {
        return Route::Command;
    }
    match (active, session) {
        (true, Some(id)) => Route::Queue(id.to_owned()),
        (true, None) => Route::Busy,
        (false, Some(id)) => Route::FollowUp(id.to_owned()),
        (false, None) => Route::NewRun,
    }
}

/// The four limits a reader may change on the limits page. Everything else there is a
/// record of a run that already happened, and a record is not edited.
fn editable_limit(key: &str) -> bool {
    matches!(key, "parallel" | "turns" | "timeout" | "attempts")
}

/// A single-line-aware text field with a byte cursor kept on character boundaries.
#[derive(Clone, Debug, Default)]
pub struct Field {
    pub value: String,
    pub cursor: usize,
}

impl Field {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.len();
        Self { value, cursor }
    }
    pub fn insert(&mut self, ch: char) {
        self.value.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }
    pub fn insert_str(&mut self, text: &str) {
        self.value.insert_str(self.cursor, text);
        self.cursor += text.len();
    }
    pub fn backspace(&mut self) {
        if let Some((index, _)) = self.value[..self.cursor].char_indices().next_back() {
            self.value.drain(index..self.cursor);
            self.cursor = index;
        }
    }
    pub fn delete(&mut self) {
        if let Some(ch) = self.value[self.cursor..].chars().next() {
            let end = self.cursor + ch.len_utf8();
            self.value.drain(self.cursor..end);
        }
    }
    pub fn left(&mut self) {
        if let Some((index, _)) = self.value[..self.cursor].char_indices().next_back() {
            self.cursor = index;
        }
    }
    pub fn right(&mut self) {
        if let Some(ch) = self.value[self.cursor..].chars().next() {
            self.cursor += ch.len_utf8();
        }
    }
    pub fn home(&mut self) {
        self.cursor = 0;
    }
    pub fn end(&mut self) {
        self.cursor = self.value.len();
    }
    pub fn set(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor = self.value.len();
    }
    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }
    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
}

/// Measurements recorded while drawing, so the next key press can scroll accurately.
#[derive(Clone, Debug, Default)]
pub struct Viewport {
    pub height: usize,
    pub width: usize,
    pub total: usize,
    /// Start line and height of each transcript entry.
    pub entries: Vec<(usize, usize)>,
}

pub struct App {
    pub store: Store,
    pub config: Config,
    pub cwd: PathBuf,
    /// What version control, if anything, was found at or above the working directory.
    /// Discovered outside the drawing path: when the window opens, when the reader asks
    /// for the change page, and after a run, which is when the answer can have changed.
    pub repository: Repository,
    pub prefs: Prefs,
    pub theme: Theme,

    pub input: Field,
    pub history: Vec<String>,
    pub history_index: usize,

    pub focus: Focus,
    pub view: View,
    pub overlay: Option<Overlay>,
    pub completion: usize,

    pub messages: Vec<Message>,
    pub notices: Vec<Notice>,
    pub streams: BTreeMap<String, String>,
    pub statuses: BTreeMap<String, String>,
    pub tasks: Vec<Task>,
    pub session: Option<String>,
    pub session_status: String,
    /// Profiles the loaded session captured when it started. The configuration may have
    /// been edited since, so this is what the run is actually using.
    pub session_team: Vec<AgentProfile>,
    /// The loaded session's own records, as the controller last read them. Pages present
    /// this snapshot; none of them reads the store while it is being painted.
    pub records: Records,
    /// What the controller last found installed on this machine. Inspecting it looks for
    /// executables on `PATH`, so it happens here and never while a page is painted.
    pub pool: Pool,
    /// Which invocation wrote each message and which one each agent is running, as last read.
    /// The transcript, the sidebar and the status row name agents from this read.
    pub attribution: Attribution,
    /// Token accounting for the loaded session, per agent. Replaced as a whole whenever
    /// the run reports, and re-read from the store when a conversation is opened.
    pub stats: Stats,
    pub turns_used: usize,
    pub status: String,
    pub active: bool,
    pub started: Option<std::time::Instant>,
    /// When a first Ctrl+C asked for a second one. The status row asks while this is set.
    pub exit_requested: Option<Instant>,

    pub follow: bool,
    pub top: usize,
    pub selected_entry: usize,
    pub expanded: BTreeSet<i64>,
    pub viewport: Viewport,

    pub page_selected: usize,
    pub page_top: usize,
    /// The filter and the sorts the reader applied to the open page's tables.
    pub table: crate::table::Controls,
    /// The row the selection stays on while the page is filtered or sorted again.
    table_anchor: Option<String>,
    pub memory_query: String,

    pub tick: u64,
    pub dirty: bool,
    revision: u64,
    entries: Vec<Entry>,
    entries_revision: u64,
    page_cache: Option<(View, u16, u64, Page)>,
}

impl App {
    pub fn new(store: Store, config: Config, cwd: PathBuf) -> Self {
        let prefs = Prefs::load(&store);
        let theme = theme::resolved(&prefs.theme, theme::detect_markers());
        let pool = Pool::read(&config);
        Self {
            store,
            config,
            repository: repository::discover(&cwd),
            cwd,
            prefs,
            theme,
            input: Field::default(),
            history: Vec::new(),
            history_index: 0,
            focus: Focus::Composer,
            view: View::Chat,
            overlay: None,
            completion: 0,
            messages: Vec::new(),
            notices: Vec::new(),
            streams: BTreeMap::new(),
            statuses: BTreeMap::new(),
            tasks: Vec::new(),
            session: None,
            session_status: "no session".into(),
            session_team: Vec::new(),
            records: Records::unopened(),
            pool,
            attribution: Attribution::default(),
            stats: Stats::default(),
            turns_used: 0,
            status: "Ready".into(),
            active: false,
            started: None,
            exit_requested: None,
            follow: true,
            top: 0,
            selected_entry: 0,
            expanded: BTreeSet::new(),
            viewport: Viewport::default(),
            page_selected: 0,
            page_top: 0,
            table: Default::default(),
            table_anchor: None,
            memory_query: String::new(),
            tick: 0,
            dirty: true,
            revision: 1,
            entries: Vec::new(),
            entries_revision: 0,
            page_cache: None,
        }
    }

    // -----------------------------------------------------------------------
    // Bookkeeping
    // -----------------------------------------------------------------------

    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.dirty = true;
    }

    pub fn notice(&mut self, text: impl Into<String>) {
        self.notices.push(Notice {
            failure: false,
            text: text.into(),
            time: text::clock(&ymp_core::now()),
        });
        self.changed();
    }

    pub fn fail(&mut self, text: impl Into<String>) {
        self.notices.push(Notice {
            failure: true,
            text: text.into(),
            time: text::clock(&ymp_core::now()),
        });
        self.follow = true;
        self.changed();
    }

    /// Deliver pasted text to whichever field currently owns the keyboard.
    ///
    /// A paste must never land in a field the reader cannot see. When an editor or the
    /// command palette is open it receives the text; otherwise it goes to the composer,
    /// and the focus follows it so the result is visible. Overlays that have no field,
    /// such as a confirmation, ignore the paste rather than leaking it underneath.
    pub fn paste(&mut self, text: &str) {
        self.dirty = true;
        self.exit_requested = None;
        let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
        match &mut self.overlay {
            Some(Overlay::Prompt {
                target: PromptTarget::AgentInstructions(_),
                field,
                ..
            }) => {
                field.insert_str(&normalized);
            }
            // A single-line editor takes the text as one line; the newlines would
            // otherwise be invisible in a field that shows one row.
            Some(Overlay::Prompt { field, .. }) | Some(Overlay::Palette { field, .. }) => {
                let flattened = normalized.replace('\n', " ");
                field.insert_str(&flattened);
            }
            Some(_) => {}
            None => {
                self.focus = Focus::Composer;
                self.input.insert_str(&normalized);
            }
        }
    }

    /// The conversation as presentable entries, rebuilt only when something changed.
    pub fn entries(&mut self) -> &[Entry] {
        if self.entries_revision != self.revision {
            self.entries = transcript::build(
                &self.messages,
                &self.notices,
                &self.streams,
                &self.attribution,
                self.prefs.details,
            );
            self.entries_revision = self.revision;
            if self.selected_entry >= self.entries.len() {
                self.selected_entry = self.entries.len().saturating_sub(1);
            }
        }
        &self.entries
    }

    /// The current page, rebuilt when the view, the width or the data changes.
    pub fn page(&mut self, width: u16) -> &Page {
        let stale = match &self.page_cache {
            Some((view, cached, revision, _)) => {
                *view != self.view || *cached != width || *revision != self.revision
            }
            None => true,
        };
        if stale {
            let theme = self.theme;
            let (team, team_captured) = self.active_team();
            let mut page = views::build(
                self.view,
                &Ctx {
                    store: &self.store,
                    config: &self.config,
                    cwd: &self.cwd,
                    repository: &self.repository,
                    theme: &theme,
                    session: self.session.as_deref(),
                    tasks: &self.tasks,
                    team: &team,
                    team_captured,
                    stats: &self.stats,
                    live: self.active,
                    records: &self.records,
                    pool: &self.pool,
                    memory_query: &self.memory_query,
                    width: width as usize,
                },
            );
            // Keys, the selection and Inspect act on the rows on screen, so the filter and the
            // sorts are applied to the page itself rather than only when it is painted.
            let is_row = |item: &views::Item| item.kind == views::ItemKind::Row;
            self.table.unfiltered = page.items.iter().filter(|item| is_row(item)).count();
            let view = self.view;
            let controls = &self.table;
            page.items = crate::table::arrange(
                page.items,
                &controls.filter,
                view.sort_reserved(),
                |title| controls.sort(view, title),
            );
            if let Some(key) = self.table_anchor.take() {
                // The row the reader was on, or the first row when the filter removed it.
                self.page_selected = page
                    .items
                    .iter()
                    .position(|item| is_row(item) && item.key == key)
                    .unwrap_or(0);
            }
            self.page_cache = Some((self.view, width, self.revision, page));
            self.select_a_row();
        }
        &self.page_cache.as_ref().expect("page was just built").3
    }

    /// Build the open page again under a changed filter or sort, keeping the selected row.
    fn rearrange(&mut self) {
        self.table_anchor = self.page_anchor();
        self.page_cache = None;
        self.page_top = 0;
        self.dirty = true;
    }

    /// Shift and a letter: sort the table holding the selection by the column that letter
    /// names, cycling ascending, descending and the page's own order. Returns false when no
    /// column of that table answers the letter, so a page key can still take it.
    fn sort_by_key(&mut self, letter: char, width: u16) -> bool {
        let width = self.page_width(width);
        let view = self.view;
        let selected = self.page_selected;
        let page = self.page(width);
        let is_heading = |item: &views::Item| item.kind == views::ItemKind::Heading;
        let end = selected.min(page.items.len());
        let heading = page.items[..end]
            .iter()
            .rposition(is_heading)
            .or_else(|| page.items.iter().position(is_heading));
        let Some(heading) = heading.map(|index| &page.items[index]) else {
            return false;
        };
        let Some(column) = heading
            .sort_keys
            .iter()
            .position(|key| *key == Some(letter))
        else {
            return false;
        };
        let table = heading.title.clone();
        let column = heading.columns[column].title;
        let sort = crate::table::Sort::cycle(self.table.sort(view, &table), column);
        self.table.set_sort(view, &table, sort);
        self.rearrange();
        true
    }

    /// Point the selection at a row rather than at a heading.
    ///
    /// A page opens with its first selectable row already selected, so the detail of that
    /// row is on screen before the reader presses anything. A selection that is already on
    /// a row is left exactly where it is.
    fn select_a_row(&mut self) {
        let Some((_, _, _, page)) = self.page_cache.as_ref() else {
            return;
        };
        let is_row = |item: &&views::Item| item.kind == views::ItemKind::Row;
        if page.items.get(self.page_selected).iter().any(is_row) {
            return;
        }
        if let Some(index) = page.items.iter().position(|item| is_row(&item)) {
            self.page_selected = index;
        }
    }

    /// Apply a runtime event.
    pub fn event(&mut self, event: UiEvent) {
        match event {
            UiEvent::Usage { session_id, usage } => return self.usage(&session_id, usage),
            UiEvent::Message(message) => {
                if self
                    .session
                    .as_deref()
                    .is_some_and(|id| id != message.session_id)
                {
                    self.tasks.clear();
                }
                let opened = self.session.as_deref() != Some(message.session_id.as_str());
                self.session = Some(message.session_id.clone());
                let agent = !matches!(message.author.as_str(), "you" | "ymp");
                self.streams.remove(&message.author);
                if !self.messages.iter().any(|old| old.seq == message.seq) {
                    self.messages.push(message);
                }
                if opened {
                    // A new session has just been recorded; read the team it captured.
                    self.refresh_session_facts();
                } else if agent {
                    // The invocation an agent message is linked to was committed with it.
                    self.refresh_attribution();
                }
            }
            UiEvent::Delta { agent, text } => {
                let started = !self.streams.contains_key(&agent);
                let buffer = self.streams.entry(agent).or_default();
                buffer.push_str(&text);
                if buffer.len() > 100_000 {
                    let cut = buffer
                        .char_indices()
                        .map(|(index, _)| index)
                        .find(|&index| index >= buffer.len() - 80_000)
                        .unwrap_or(0);
                    buffer.drain(..cut);
                }
                if started {
                    // A stream is named by the invocation running it, which this reads.
                    self.refresh_attribution();
                }
            }
            UiEvent::AgentStatus { agent, status } => {
                // A turn starting or ending is when its invocation record appears or closes.
                self.refresh_attribution();
                if let Some(tool) = status.strip_prefix(TOOL_ACTIVITY) {
                    // Tool use is activity inside a turn, not a change in what the turn is for:
                    // the member keeps its purpose, and the status row names the running
                    // invocation and the tool.
                    let working = label::streaming(&agent, self.attribution.trace.as_ref());
                    self.status = format!("{working} · {}", label::clean(tool));
                } else {
                    self.statuses.insert(agent, status);
                    if self.view.reads_records() {
                        self.refresh_records();
                    }
                }
            }
            UiEvent::Task(task) => {
                match self.tasks.iter_mut().find(|t| t.id == task.id) {
                    Some(existing) => *existing = task,
                    None => self.tasks.push(task),
                }
                if self.view.reads_records() {
                    self.refresh_records();
                }
            }
            UiEvent::Status(status) => self.status = status,
            UiEvent::Finished { session_id, status } => {
                self.session = Some(session_id);
                self.session_status = status;
                self.streams.clear();
                self.refresh_session_facts();
                // A run may have created or removed a repository in the directory it
                // worked in, so what was discovered at startup is re-read here.
                self.repository = repository::discover(&self.cwd);
            }
        }
        self.changed();
    }

    /// Apply a token statistics snapshot.
    ///
    /// A snapshot describes an entire session and replaces the one before it; adding them
    /// together would count every turn again. A snapshot that names another conversation
    /// belongs to a run this window is not showing, so it is discarded rather than mixed
    /// into the open one.
    fn usage(&mut self, session: &str, usage: SessionUsage) {
        if self.session.as_deref().is_some_and(|open| open != session) {
            return;
        }
        // The statistics page is a live list. Remember the row the reader selected, so a
        // snapshot that records a new agent above it does not move them onto another one.
        // No other page is built from statistics, so none of them is rebuilt here.
        let anchor = (self.view == View::Usage)
            .then(|| self.page_anchor())
            .flatten();
        self.stats.replace(session, usage);
        self.count_turns();
        self.changed();
        self.restore_page_anchor(anchor.as_deref());
    }

    /// Advance the turn counter to the invocations the statistics account for.
    ///
    /// A run writes its own counter at its own pace, so during a turn the stored figure can
    /// lag behind the work already done. Invocations are what the turn budget spends, so the
    /// larger of the two is the one shown, and it never moves backwards inside one session:
    /// a snapshot with fewer invocations is not a retraction of the ones already counted.
    fn count_turns(&mut self) {
        let Some(id) = self.session.clone() else {
            return;
        };
        if !self.stats.describes(&id) {
            return;
        }
        if let Some(total) = self.stats.total() {
            self.turns_used = self.turns_used.max(total.calls as usize);
        }
    }

    /// The session status to present.
    ///
    /// A run active in this window is running, whatever the stored record still says: the
    /// record is written when the run ends. Browsing a saved session shows what was stored,
    /// and so does this window once its own run has finished.
    pub fn live_status(&self) -> &str {
        if self.active {
            "running"
        } else {
            &self.session_status
        }
    }

    /// The key of the row the open page is pointing at, if it has one.
    fn page_anchor(&self) -> Option<String> {
        let (_, _, _, page) = self.page_cache.as_ref()?;
        page.items
            .get(self.page_selected)
            .filter(|item| item.kind == views::ItemKind::Row)
            .map(|item| item.key.clone())
    }

    /// Put the selection back on the row it was on after the data underneath it changed.
    fn restore_page_anchor(&mut self, key: Option<&str>) {
        let (Some(key), Some((_, width, _, _))) = (key, self.page_cache.as_ref()) else {
            return;
        };
        let width = *width;
        let found = self
            .page(width)
            .items
            .iter()
            .position(|item| item.kind == views::ItemKind::Row && item.key == key);
        if let Some(index) = found {
            self.page_selected = index;
        }
    }

    /// Read the token statistics the store recorded for the loaded session.
    ///
    /// Reopening a conversation must show what it actually spent, so the store is read
    /// rather than assumed empty. A read that fails leaves the statistics unavailable,
    /// which is not the same as a session that spent nothing.
    fn reload_usage(&mut self) {
        let Some(id) = self.session.clone() else {
            self.stats.clear();
            return;
        };
        match self.store.session_usage(&id) {
            Ok(usage) => self.stats.replace(&id, usage),
            Err(_) => self.stats.unavailable(&id),
        }
    }

    /// Re-read the turn counter recorded for the loaded session.
    fn refresh_session_facts(&mut self) {
        let Some(id) = self.session.clone() else {
            return;
        };
        if let Ok(session) = self.store.session(&id) {
            self.session_status = session.status;
            self.turns_used = session.turns_used;
            self.session_team = session.team;
        }
        if let Ok(Some(value)) = self.store.value(&format!("turns:{id}")) {
            if let Some(turns) = value.as_u64() {
                self.turns_used = turns as usize;
            }
        }
        self.count_turns();
        // Statistics already being reported for this session are the live ones; only a
        // conversation the interface was not following needs a read.
        if !self.stats.describes(&id) {
            self.reload_usage();
        }
        self.refresh_records();
        self.refresh_attribution();
    }

    /// Read which invocation wrote each message and which one each agent is running now.
    ///
    /// Only the session's own records answer that, so this reads them rather than inferring an
    /// invocation from an author's latest turn or from the configuration.
    fn refresh_attribution(&mut self) {
        self.attribution
            .refresh(&self.store, self.session.as_deref(), &self.messages);
    }

    /// Read the loaded session's own records again.
    ///
    /// Every page built from them presents this one snapshot, so the moment of the read is
    /// the moment the page describes, and it is stated on the page. A page never reads the
    /// store itself, which is what keeps opening one a pure read.
    fn refresh_records(&mut self) {
        self.records = Records::read(&self.store, self.session.as_deref());
        self.page_cache = None;
    }

    /// Inspect what is installed on this machine again.
    ///
    /// This looks for the provider programs on `PATH`. It starts nothing, asks no provider
    /// anything and reads no credential: whether an account may run a model is the
    /// installation's own business and is never decided here.
    fn refresh_pool(&mut self) {
        self.pool = Pool::read(&self.config);
        self.page_cache = None;
    }

    /// Load a stored conversation for reading. This never starts an agent and never
    /// touches the user's working directory.
    ///
    /// Switching is refused while a run is active. The loaded conversation is also the
    /// one a message is delivered to, so replacing it mid-run would route the next
    /// message into a session the running engine is not working on. Browsing the list
    /// stays available; only the switch is blocked.
    pub fn load_session(&mut self, id: &str) -> Result<()> {
        if self.active && self.session.as_deref() != Some(id) {
            bail!(
                "A run is active in this conversation. Stop it with /stop before opening another session."
            );
        }
        let session = self.store.session(id)?;
        self.messages = self.store.messages(id, 0, 10_000)?;
        self.tasks = self.store.tasks(id)?;
        self.session = Some(id.to_owned());
        self.session_status = session.status;
        self.turns_used = session.turns_used;
        self.session_team = session.team;
        self.streams.clear();
        self.notices.clear();
        self.expanded.clear();
        self.follow = true;
        self.top = 0;
        self.selected_entry = usize::MAX;
        self.reload_usage();
        self.refresh_session_facts();
        self.changed();
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Navigation
    // -----------------------------------------------------------------------

    pub fn set_view(&mut self, view: View) {
        if view == View::Changes {
            self.repository = repository::discover(&self.cwd);
        }
        if view.reads_records() {
            self.refresh_records();
        }
        if view.reads_pool() {
            self.refresh_pool();
        }
        self.view = view;
        self.page_selected = 0;
        self.page_top = 0;
        // A filter belongs to the page it was typed on. Sorts are kept per page and table.
        self.table.clear_filter();
        self.table_anchor = None;
        self.focus = if view == View::Chat {
            Focus::Composer
        } else {
            Focus::Main
        };
        self.dirty = true;
        self.page_cache = None;
    }

    /// The team to present, and whether it is the one a run captured.
    ///
    /// A loaded or running session keeps the profiles it started with. The configuration
    /// describes the next run instead, and may be mid-edit, so the two are never shown as
    /// if they were the same thing.
    pub fn active_team(&self) -> (Vec<AgentProfile>, bool) {
        if self.session.is_some() && !self.session_team.is_empty() {
            (self.session_team.clone(), true)
        } else {
            (self.config.members(), false)
        }
    }

    /// How the window names an agent outside the transcript.
    ///
    /// While a run in this window has the agent working, it is named by the invocation the
    /// records show it running. Otherwise it is named by what the session recorded for it or,
    /// where nothing was recorded, by the pool as it was last read.
    pub fn agent_label(&self, id: &str) -> String {
        let trace = self.attribution.trace.as_ref();
        let working = self.active
            && self.statuses.get(id).is_some_and(|status| {
                !matches!(status.as_str(), "idle" | "error") && !status.starts_with("waiting")
            });
        if let Some(attribution) = working
            .then(|| trace.and_then(|trace| trace.active_agent_attribution(id)))
            .flatten()
        {
            return label::invocation(&attribution);
        }
        let pooled = self.pool.agent(id).map(|agent| &agent.identity);
        label::agent(id, trace, pooled, &self.config)
    }

    /// The width a page is wrapped for in a terminal `total` cells wide.
    ///
    /// Key handling reads the page the frame painted, so it has to ask for the same width.
    /// Asking for the width of the whole terminal would build a second page, wrapped for a
    /// rect that does not exist, and Enter would then open lines too wide for the surface
    /// that shows them.
    pub fn page_width(&self, total: u16) -> u16 {
        let sidebar = if self.prefs.sidebar {
            frame::sidebar_width(total)
        } else {
            0
        };
        frame::page_content_width(frame::main_width(total, sidebar))
    }

    /// Tab and Shift+Tab. The composer and the transcript or page are the only regions that take
    /// the keyboard, so either key moves the focus to the other one.
    fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Composer => Focus::Main,
            Focus::Main => Focus::Composer,
        };
        self.dirty = true;
    }

    /// Inline command completion candidates, or nothing when the composer is not typing a
    /// bare command name.
    pub fn completions(&self) -> Vec<&'static commands::Command> {
        let value = &self.input.value;
        if !value.starts_with('/') || value.contains(char::is_whitespace) {
            return Vec::new();
        }
        commands::matching(value)
    }

    // -----------------------------------------------------------------------
    // Scrolling
    // -----------------------------------------------------------------------

    fn scroll_by(&mut self, delta: isize) {
        let height = self.viewport.height.max(1);
        let max_top = self.viewport.total.saturating_sub(height);
        let current = if self.follow { max_top } else { self.top };
        let next = current.saturating_add_signed(delta).min(max_top);
        self.top = next;
        self.follow = next >= max_top;
        self.dirty = true;
    }

    pub fn jump_to_latest(&mut self) {
        self.follow = true;
        self.top = self
            .viewport
            .total
            .saturating_sub(self.viewport.height.max(1));
        self.selected_entry = usize::MAX;
        self.dirty = true;
    }

    /// Keep the selected entry inside the viewport without losing the reading position.
    fn reveal_entry(&mut self) {
        let Some(&(start, height)) = self.viewport.entries.get(self.selected_entry) else {
            return;
        };
        let view_height = self.viewport.height.max(1);
        let max_top = self.viewport.total.saturating_sub(view_height);
        let mut top = if self.follow { max_top } else { self.top };
        if start < top {
            top = start;
        } else if start + height > top + view_height {
            top = (start + height).saturating_sub(view_height);
        }
        self.top = top.min(max_top);
        self.follow = self.top >= max_top;
        self.dirty = true;
    }

    fn move_entry(&mut self, delta: isize) {
        let count = self.entries().len();
        if count == 0 {
            return;
        }
        // Nothing is selected until the reader asks for it; the first move picks the
        // newest entry, whichever direction it was.
        if self.selected_entry >= count {
            self.selected_entry = count - 1;
            self.reveal_entry();
            return;
        }
        let current = self.selected_entry;
        let next = current
            .saturating_add_signed(delta)
            .min(count.saturating_sub(1));
        self.selected_entry = next;
        self.reveal_entry();
    }

    fn move_page_selection(&mut self, delta: isize, width: u16) {
        let width = self.page_width(width);
        let rows: Vec<usize> = self
            .page(width)
            .items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.kind == views::ItemKind::Row)
            .map(|(index, _)| index)
            .collect();
        if rows.is_empty() {
            return;
        }
        let position = rows
            .iter()
            .position(|index| *index >= self.page_selected)
            .unwrap_or(0);
        let next = position
            .saturating_add_signed(delta)
            .min(rows.len().saturating_sub(1));
        self.page_selected = rows[next];
        self.dirty = true;
    }

    /// The selected page row, if the page has one.
    pub fn selected_item(&mut self, width: u16) -> Option<views::Item> {
        let width = self.page_width(width);
        self.selected_item_at(width)
    }

    /// The selected row of the page built for exactly `width` columns.
    fn selected_item_at(&mut self, width: u16) -> Option<views::Item> {
        // Building the page may move the selection onto the first row, so read it after.
        self.page(width);
        let selected = self.page_selected;
        self.page(width)
            .items
            .get(selected)
            .filter(|item| item.kind == views::ItemKind::Row)
            .cloned()
    }

    // -----------------------------------------------------------------------
    // Themes and preferences
    // -----------------------------------------------------------------------

    pub fn apply_theme(&mut self, id: &str) {
        self.theme = theme::resolved(id, self.theme.markers);
        self.page_cache = None;
        self.dirty = true;
    }

    fn persist_theme(&mut self, id: &str) {
        self.prefs.theme = theme::theme(id).id.to_owned();
        self.apply_theme(id);
        if let Err(error) = self.prefs.save(&self.store) {
            self.fail(format!("The theme could not be saved: {error:#}"));
        }
    }

    fn save_prefs(&mut self) {
        if let Err(error) = self.prefs.save(&self.store) {
            self.fail(format!("Preferences could not be saved: {error:#}"));
        }
    }

    // -----------------------------------------------------------------------
    // Keyboard
    // -----------------------------------------------------------------------

    pub fn on_key(&mut self, key: KeyEvent, width: u16) -> Vec<Action> {
        self.on_key_at(key, width, Instant::now())
    }

    /// A key pressed at `now`. Only the Ctrl+C confirmation depends on the time.
    pub(crate) fn on_key_at(&mut self, key: KeyEvent, width: u16, now: Instant) -> Vec<Action> {
        if key.kind != KeyEventKind::Press {
            return Vec::new();
        }
        self.dirty = true;
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        // Ctrl+C belongs to no overlay and no region, so it is handled before either. The
        // first press stops nothing, clears nothing and closes nothing.
        if control && key.code == KeyCode::Char('c') {
            return self.interrupt(now);
        }
        // Anything else the reader does withdraws the question. A bare modifier is reported
        // on the way to a chord, possibly the second Ctrl+C itself, so it does not.
        if !matches!(key.code, KeyCode::Modifier(_)) {
            self.exit_requested = None;
        }
        if self.overlay.is_some() {
            return self.overlay_key(key);
        }
        match key.code {
            KeyCode::Char('d') if control && self.input.is_empty() => {
                return vec![Action::Quit];
            }
            KeyCode::Char('p') if control => {
                self.overlay = Some(Overlay::Palette {
                    field: Field::default(),
                    selected: 0,
                });
                return Vec::new();
            }
            KeyCode::Char('t') if control => {
                self.open_theme_chooser();
                return Vec::new();
            }
            KeyCode::Char('b') if control => {
                self.prefs.sidebar = !self.prefs.sidebar;
                self.save_prefs();
                return Vec::new();
            }
            KeyCode::Char('l') if control => {
                self.prefs.details = !self.prefs.details;
                self.save_prefs();
                self.changed();
                return Vec::new();
            }
            KeyCode::Char('j') if control => {
                self.input.insert('\n');
                return Vec::new();
            }
            KeyCode::Esc => {
                self.escape();
                return Vec::new();
            }
            KeyCode::Tab => {
                if self.completions().is_empty() {
                    self.cycle_focus();
                } else {
                    self.accept_completion();
                }
                return Vec::new();
            }
            KeyCode::BackTab => {
                self.cycle_focus();
                return Vec::new();
            }
            KeyCode::PageUp => {
                self.scroll_main(-1, width);
                return Vec::new();
            }
            KeyCode::PageDown => {
                self.scroll_main(1, width);
                return Vec::new();
            }
            _ => {}
        }
        match self.focus {
            Focus::Composer => self.composer_key(key, width),
            Focus::Main => self.main_key(key, width),
        }
    }

    /// Ctrl+C. The first press asks for a second; a second within the window leaves.
    fn interrupt(&mut self, now: Instant) -> Vec<Action> {
        match self.exit_requested.take() {
            Some(asked) if now.saturating_duration_since(asked) < exit::CONFIRM_WINDOW => {
                vec![Action::Quit]
            }
            _ => {
                self.exit_requested = Some(now);
                Vec::new()
            }
        }
    }

    /// Withdraw a first Ctrl+C whose window passed without a second one.
    pub fn expire_exit_request(&mut self, now: Instant) {
        if self
            .exit_requested
            .is_some_and(|asked| now.saturating_duration_since(asked) >= exit::CONFIRM_WINDOW)
        {
            self.exit_requested = None;
            self.dirty = true;
        }
    }

    fn scroll_main(&mut self, direction: isize, width: u16) {
        let step = (self.viewport.height.max(3) - 2) as isize;
        if self.view == View::Chat {
            self.scroll_by(direction * step);
        } else {
            self.move_page_selection(direction * step, width);
        }
    }

    /// Esc always removes the topmost thing: the overlay, then the completion list, then the
    /// page's filter, then the open page, then a focus that is not the composer, and finally
    /// the draft.
    fn escape(&mut self) {
        if self.overlay.take().is_some() {
            return;
        }
        if !self.completions().is_empty() {
            self.input.clear();
            self.completion = 0;
            return;
        }
        if self.view != View::Chat && (self.table.typing || !self.table.filter.is_empty()) {
            self.table.clear_filter();
            self.rearrange();
            return;
        }
        if self.view != View::Chat {
            self.set_view(View::Chat);
            return;
        }
        if self.focus != Focus::Composer {
            self.focus = Focus::Composer;
            self.selected_entry = usize::MAX;
            return;
        }
        if !self.input.is_empty() {
            self.input.clear();
        }
    }

    /// The completion the list is currently pointing at.
    fn highlighted_completion(&self) -> Option<&'static commands::Command> {
        let matches = self.completions();
        matches
            .get(self.completion.min(matches.len().saturating_sub(1)))
            .copied()
    }

    /// Tab: write the highlighted name into the composer, leaving room for an argument
    /// when the command accepts one.
    fn accept_completion(&mut self) {
        let Some(command) = self.highlighted_completion() else {
            return;
        };
        self.input.set(if command.wants_argument() {
            format!("{} ", command.name)
        } else {
            command.name.to_owned()
        });
        self.completion = 0;
    }

    /// The command a press of Enter would act on: the name already typed in full, or
    /// otherwise the highlighted completion.
    fn command_for_enter(&self) -> (Option<&'static commands::Command>, bool) {
        let trimmed = self.input.value.trim();
        let exact = commands::find(trimmed).filter(|c| c.name == trimmed);
        match exact {
            Some(command) => (Some(command), true),
            None => (self.highlighted_completion(), false),
        }
    }

    fn composer_key(&mut self, key: KeyEvent, width: u16) -> Vec<Action> {
        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Enter if shift || alt => {
                self.input.insert('\n');
            }
            KeyCode::Enter => {
                // A command that needs an argument leaves a draft to finish; anything
                // else runs on this one press, including commands whose arguments are
                // optional.
                let (command, exact) = self.command_for_enter();
                if let Some(command) = command {
                    if !command.complete_alone() {
                        self.input.set(format!("{} ", command.name));
                        self.completion = 0;
                        return Vec::new();
                    }
                    if !exact {
                        self.input.set(command.name.to_owned());
                    }
                }
                return self.submit(width);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input.clear()
            }
            KeyCode::Backspace => self.input.backspace(),
            KeyCode::Delete => self.input.delete(),
            KeyCode::Left => self.input.left(),
            KeyCode::Right => self.input.right(),
            KeyCode::Home => {
                if self.input.is_empty() {
                    self.top = 0;
                    self.follow = false;
                } else {
                    self.input.home();
                }
            }
            KeyCode::End => {
                if self.input.is_empty() {
                    self.jump_to_latest();
                } else {
                    self.input.end();
                }
            }
            KeyCode::Up => {
                if !self.completions().is_empty() {
                    self.completion = self.completion.saturating_sub(1);
                } else if self.history_index > 0 {
                    self.history_index -= 1;
                    self.input.set(self.history[self.history_index].clone());
                }
            }
            KeyCode::Down => {
                if !self.completions().is_empty() {
                    let last = self.completions().len().saturating_sub(1);
                    self.completion = (self.completion + 1).min(last);
                } else {
                    self.history_index = (self.history_index + 1).min(self.history.len());
                    let value = self
                        .history
                        .get(self.history_index)
                        .cloned()
                        .unwrap_or_default();
                    self.input.set(value);
                }
            }
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.input.insert(ch);
                self.completion = 0;
            }
            _ => {}
        }
        Vec::new()
    }

    fn main_key(&mut self, key: KeyEvent, width: u16) -> Vec<Action> {
        if self.view == View::Chat {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_entry(-1),
                KeyCode::Down | KeyCode::Char('j') => self.move_entry(1),
                KeyCode::Home => {
                    self.selected_entry = 0;
                    self.reveal_entry();
                }
                KeyCode::End => self.jump_to_latest(),
                // Detailed mode already draws every entry in full, so Space has nothing to expand.
                KeyCode::Char(' ') if !self.prefs.details => {
                    let index = self.selected_entry;
                    if let Some(seq) = self.entries().get(index).and_then(|entry| entry.seq) {
                        if !self.expanded.remove(&seq) {
                            self.expanded.insert(seq);
                        }
                        self.dirty = true;
                    }
                }
                KeyCode::Enter => self.inspect_selected_entry(),
                _ => {}
            }
            return Vec::new();
        }
        let plain = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        if self.table.typing {
            // Typing a filter: letters go into it, and only the arrows still move the selection.
            match key.code {
                KeyCode::Enter => self.table.typing = false,
                KeyCode::Up => self.move_page_selection(-1, width),
                KeyCode::Down => self.move_page_selection(1, width),
                KeyCode::Backspace => {
                    self.table.filter.pop();
                    self.rearrange();
                }
                KeyCode::Char(ch) if plain => {
                    self.table.filter.push(ch);
                    self.rearrange();
                }
                _ => {}
            }
            return Vec::new();
        }
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_page_selection(-1, width),
            KeyCode::Down | KeyCode::Char('j') => self.move_page_selection(1, width),
            KeyCode::Home => {
                self.page_selected = 0;
                self.move_page_selection(0, width);
            }
            KeyCode::End => self.move_page_selection(isize::MAX / 2, width),
            KeyCode::Char('/') if plain => self.table.typing = true,
            KeyCode::Char('d') if plain => self.inspect_selected_row(width),
            KeyCode::Char(letter)
                if plain && letter.is_ascii_uppercase() && self.sort_by_key(letter, width) => {}
            _ => return self.page_action(key, width),
        }
        Vec::new()
    }

    fn inspect_selected_entry(&mut self) {
        let theme = self.theme;
        // The message is wrapped for the surface that will show it, not for the terminal.
        let width = frame::inspect_content_width(self.viewport.width as u16) as usize;
        let index = self.selected_entry;
        let Some(entry) = self.entries().get(index).cloned() else {
            return;
        };
        let mut body = Vec::new();
        body.push(Line::from(ratatui::text::Span::styled(
            text::truncate(
                &format!(
                    "{}{}",
                    entry.title(),
                    if entry.time.is_empty() {
                        String::new()
                    } else {
                        format!(" · {}", entry.time)
                    }
                ),
                width,
            ),
            theme.muted(),
        )));
        if let Some(origin) = &entry.origin {
            body.push(Line::from(ratatui::text::Span::styled(
                text::truncate(origin, width),
                theme.faint(),
            )));
        }
        body.push(Line::default());
        body.extend(text::markdown(&entry.raw, width, &theme, theme.body()));
        self.overlay = Some(Overlay::Inspect {
            title: entry.author.clone(),
            body,
            scroll: 0,
        });
    }

    /// Keys that belong to one page. Every one of these is either a read or an explicit
    /// configuration change the user asked for.
    fn page_action(&mut self, key: KeyEvent, width: u16) -> Vec<Action> {
        let Some(item) = self.selected_item(width) else {
            return Vec::new();
        };
        let view = self.view;
        match (view, key.code) {
            (View::Help, KeyCode::Enter) => {
                if item.key.starts_with('/') {
                    let command = item.key.clone();
                    return self.command(&command, width);
                }
            }
            (View::Sessions, KeyCode::Enter) => {
                let id = item.key.clone();
                match self.load_session(&id) {
                    Ok(()) => {
                        self.set_view(View::Chat);
                        self.notice(format!(
                            "Loaded session {} for reading. Send a message to continue it, or press r on the list to resume the run.",
                            text::short_id(&id)
                        ));
                    }
                    Err(error) => self.fail(format!("{error:#}")),
                }
            }
            (View::Sessions, KeyCode::Char('r')) => {
                if self.active {
                    self.fail("A run is already active. Stop it with /stop first.");
                    return Vec::new();
                }
                return vec![Action::Resume {
                    session: item.key.clone(),
                }];
            }
            (View::Providers | View::Agents, KeyCode::Char('r')) => {
                self.refresh_pool();
                self.notice(
                    "Re-read what is installed on this machine and the catalog already stored. This asks no provider anything: reading a provider's own offerings is R.",
                );
            }
            (View::Providers | View::Agents, KeyCode::Char('R')) => {
                // One provider when the row is a provider; every enabled one from the pool page,
                // because a profile's row names an agent and not what would be asked.
                let provider = match view {
                    View::Providers => Some(item.key.clone()),
                    _ => None,
                };
                if let Some(id) = provider.as_deref() {
                    if !self.config.provider(id).is_ok_and(|p| p.enabled) {
                        self.fail(format!(
                            "{id} is disabled, so nothing would be asked of it. Enable it with Space first."
                        ));
                        return Vec::new();
                    }
                }
                return vec![Action::RefreshCatalog { provider }];
            }
            (View::Agents, KeyCode::Char('m')) => {
                let id = item.key.clone();
                let current = self
                    .config
                    .agent(&id)
                    .ok()
                    .and_then(|a| a.model.clone())
                    .unwrap_or_default();
                self.overlay = Some(Overlay::Prompt {
                    target: PromptTarget::AgentModel(id.clone()),
                    label: format!("Model for {id}"),
                    help: "Leave empty, or type default, to inherit the provider's model.".into(),
                    field: Field::new(current),
                });
            }
            (View::Agents, KeyCode::Char('i')) => {
                let id = item.key.clone();
                let current = self
                    .config
                    .agent(&id)
                    .map(|a| a.instructions.clone())
                    .unwrap_or_default();
                self.overlay = Some(Overlay::Prompt {
                    target: PromptTarget::AgentInstructions(id.clone()),
                    label: format!("Instructions for {id}"),
                    help: "Guidance added to every turn. Never put credentials here.".into(),
                    field: Field::new(current),
                });
            }
            (View::Agents, KeyCode::Char(' ')) => {
                let id = item.key.clone();
                self.edit_config(|config| {
                    let profile = config
                        .agents
                        .iter_mut()
                        .find(|a| a.id == id)
                        .ok_or_else(|| anyhow::anyhow!("Unknown profile: {id}"))?;
                    profile.enabled = !profile.enabled;
                    Ok(format!(
                        "{id} is now {}.",
                        if profile.enabled {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    ))
                });
            }
            (View::Agents, KeyCode::Char('t')) | (View::Team, KeyCode::Char(' ')) => {
                let id = item.key.clone();
                self.toggle_membership(&id);
            }
            (View::Providers, KeyCode::Char(' ')) => {
                let id = item.key.clone();
                self.edit_config(|config| {
                    let provider = config
                        .providers
                        .iter_mut()
                        .find(|p| p.id == id)
                        .ok_or_else(|| anyhow::anyhow!("Unknown provider: {id}"))?;
                    provider.enabled = !provider.enabled;
                    Ok(format!(
                        "Provider {id} is now {}.",
                        if provider.enabled {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    ))
                });
            }
            (View::Memory, KeyCode::Char('s')) => {
                self.overlay = Some(Overlay::Prompt {
                    target: PromptTarget::MemorySearch,
                    label: "Search memory".into(),
                    help: "Words are matched against titles and content. Leave empty to list everything.".into(),
                    field: Field::new(self.memory_query.clone()),
                });
            }
            (View::Memory, KeyCode::Char('f')) if item.key != crate::views::ABOUT_KEY => {
                self.overlay = Some(Overlay::Confirm {
                    question: format!(
                        "Retire memory entry {}? It stops informing future runs.",
                        text::short_id(&item.key)
                    ),
                    target: Confirm::ForgetMemory(item.key.clone()),
                });
            }
            (View::Limits, KeyCode::Char('+')) | (View::Limits, KeyCode::Char('='))
                if editable_limit(&item.key) =>
            {
                self.nudge_limit(&item.key, 1)
            }
            (View::Limits, KeyCode::Char('-')) if editable_limit(&item.key) => {
                self.nudge_limit(&item.key, -1)
            }
            (View::Limits, KeyCode::Enter) if editable_limit(&item.key) => {
                let key = match item.key.as_str() {
                    "parallel" => "parallel",
                    "turns" => "turns",
                    "timeout" => "timeout",
                    _ => "attempts",
                };
                let current = self.limit_value(key).to_string();
                self.overlay = Some(Overlay::Prompt {
                    target: PromptTarget::Limit(key),
                    label: format!("Limit: {key}"),
                    help: "Applied to the next run. Sessions already running keep their budget."
                        .into(),
                    field: Field::new(current),
                });
            }
            (_, KeyCode::Enter) => {
                self.inspect_selected_row(width);
            }
            _ => {}
        }
        Vec::new()
    }

    /// Open the selected row as a read-only record.
    ///
    /// The record is built again for the width of the surface that shows it, which is
    /// narrower than the terminal. Reusing lines wrapped for the page would wrap them for the
    /// wrong rect, and the right edge of each one would be cut off when it is painted.
    fn inspect_selected_row(&mut self, total: u16) {
        let theme = self.theme;
        let room = frame::inspect_content_width(total);
        let Some(item) = self.selected_item_at(room) else {
            return;
        };
        // The popup is titled by the row's flexible cell, the text that names the record, and
        // by its key where that cell is empty or the row is a note.
        let title = {
            let selected = self.page_selected;
            let page = self.page(room);
            page.items[..selected.min(page.items.len())]
                .iter()
                .rfind(|heading| heading.kind == views::ItemKind::Heading)
                .and_then(|heading| heading.columns.iter().position(|column| column.flex))
                .filter(|_| item.cells.len() > 1)
                .and_then(|column| item.cells.get(column))
                .map(|cell| text::one_line(cell.plain().trim()))
                .filter(|title| !title.is_empty())
                .unwrap_or_else(|| item.key.clone())
        };
        let body = if item.detail.is_empty() {
            text::wrap(&item.key, room as usize)
                .into_iter()
                .map(|piece| Line::from(ratatui::text::Span::styled(piece, theme.body())))
                .collect()
        } else {
            item.detail
        };
        self.overlay = Some(Overlay::Inspect {
            title,
            body,
            scroll: 0,
        });
    }

    /// The turn bound this window measures against, and whether a session captured it.
    ///
    /// A finished session was bounded by what it started with. Measuring it against a limit
    /// edited afterwards would report a run against a constraint it never ran under.
    pub fn turn_limit(&self) -> (usize, bool) {
        let captured = self.records.trace.as_ref().and_then(|trace| {
            trace
                .budget
                .as_ref()
                .map(|budget| budget.limits.turns)
                .or_else(|| trace.policy.as_ref().map(|policy| policy.limits.turns))
        });
        match captured {
            Some(turns) => (turns, true),
            None => (self.config.limits.turns, false),
        }
    }

    fn limit_value(&self, key: &str) -> usize {
        match key {
            "parallel" => self.config.limits.parallel,
            "turns" => self.config.limits.turns,
            "timeout" => self.config.limits.turn_timeout_secs as usize,
            _ => self.config.limits.attempts,
        }
    }

    fn nudge_limit(&mut self, key: &str, delta: isize) {
        let step: isize = match key {
            "turns" => 10,
            "timeout" => 60,
            _ => 1,
        };
        let value = (self.limit_value(key) as isize + delta * step).max(1) as usize;
        let key = key.to_owned();
        self.edit_config(move |config| {
            match key.as_str() {
                "parallel" => config.limits.parallel = value,
                "turns" => config.limits.turns = value,
                "timeout" => config.limits.turn_timeout_secs = value as u64,
                _ => config.limits.attempts = value,
            }
            Ok(format!("{key} is now {value}. It applies to the next run."))
        });
    }

    fn toggle_membership(&mut self, id: &str) {
        let id = id.to_owned();
        self.edit_config(move |config| {
            if config.team.contains(&id) {
                config.team.retain(|member| member != &id);
                return Ok(format!("{id} left the team."));
            }
            let profile = config.agent(&id)?.clone();
            if let Some(provider) = config
                .providers
                .iter_mut()
                .find(|p| p.id == profile.provider)
            {
                provider.enabled = true;
            }
            if let Some(agent) = config.agents.iter_mut().find(|a| a.id == id) {
                agent.enabled = true;
            }
            config.team.push(id.clone());
            Ok(format!("{id} joined the team."))
        });
    }

    /// Take the configuration a catalog reading produced.
    ///
    /// The reading has already written `config.toml` and its own snapshot under the
    /// application home, so this does not save again: saving here would race with the check
    /// that refuses a reading when the file changed under it. The loaded session, its records
    /// and the conversation are untouched; only what the next run would use changes.
    pub fn adopt_config(&mut self, config: Config) {
        self.config = config;
        self.refresh_pool();
    }

    /// Apply a configuration change, validate it, and save it. A rejected change is
    /// reverted in memory so the interface never shows a state that was not written.
    fn edit_config<F>(&mut self, change: F)
    where
        F: FnOnce(&mut Config) -> Result<String>,
    {
        let previous = self.config.clone();
        let outcome = change(&mut self.config)
            .and_then(|message| self.config.save(&self.store.home).map(|()| message));
        match outcome {
            Ok(message) => {
                self.notice(message);
                // The change may have made an agent eligible or taken a provider out of
                // reach, so what is installed is inspected again rather than assumed.
                self.refresh_pool();
            }
            Err(error) => {
                self.config = previous;
                self.fail(format!("The change was not saved: {error:#}"));
            }
        }
    }

    // -----------------------------------------------------------------------
    // Overlays
    // -----------------------------------------------------------------------

    fn open_theme_chooser(&mut self) {
        self.overlay = Some(Overlay::Themes {
            selected: theme::index_of(&self.prefs.theme),
            original: self.prefs.theme.clone(),
        });
    }

    fn overlay_key(&mut self, key: KeyEvent) -> Vec<Action> {
        let Some(overlay) = self.overlay.take() else {
            return Vec::new();
        };
        match overlay {
            Overlay::Themes { selected, original } => self.theme_key(key, selected, original),
            Overlay::Palette { field, selected } => self.palette_key(key, field, selected),
            Overlay::Inspect {
                title,
                body,
                scroll,
            } => {
                let height = self.viewport.height.max(4);
                let next = match key.code {
                    KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => return Vec::new(),
                    KeyCode::Up | KeyCode::Char('k') => scroll.saturating_sub(1),
                    KeyCode::Down | KeyCode::Char('j') => {
                        (scroll + 1).min(body.len().saturating_sub(1))
                    }
                    KeyCode::PageUp => scroll.saturating_sub(height),
                    KeyCode::PageDown => (scroll + height).min(body.len().saturating_sub(1)),
                    KeyCode::Home => 0,
                    KeyCode::End => body.len().saturating_sub(1),
                    _ => scroll,
                };
                self.overlay = Some(Overlay::Inspect {
                    title,
                    body,
                    scroll: next,
                });
                Vec::new()
            }
            Overlay::Prompt {
                target,
                label,
                help,
                mut field,
            } => {
                match key.code {
                    KeyCode::Esc => return Vec::new(),
                    KeyCode::Enter
                        if matches!(target, PromptTarget::AgentInstructions(_))
                            && key
                                .modifiers
                                .intersects(KeyModifiers::ALT | KeyModifiers::SHIFT) =>
                    {
                        field.insert('\n');
                    }
                    KeyCode::Char('j')
                        if matches!(target, PromptTarget::AgentInstructions(_))
                            && key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        field.insert('\n');
                    }
                    KeyCode::Enter => {
                        let value = if matches!(target, PromptTarget::AgentInstructions(_)) {
                            field.value
                        } else {
                            field.value.trim().to_owned()
                        };
                        self.commit_prompt(target, value);
                        return Vec::new();
                    }
                    KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        field.clear()
                    }
                    KeyCode::Backspace => field.backspace(),
                    KeyCode::Delete => field.delete(),
                    KeyCode::Left => field.left(),
                    KeyCode::Right => field.right(),
                    KeyCode::Home => field.home(),
                    KeyCode::End => field.end(),
                    KeyCode::Char(ch)
                        if !key
                            .modifiers
                            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                    {
                        field.insert(ch)
                    }
                    _ => {}
                }
                self.overlay = Some(Overlay::Prompt {
                    target,
                    label,
                    help,
                    field,
                });
                Vec::new()
            }
            Overlay::Confirm { question, target } => {
                match key.code {
                    KeyCode::Char('y') | KeyCode::Enter => match target {
                        Confirm::ForgetMemory(id) => match self.store.forget_memory(&id) {
                            Ok(()) => {
                                self.page_cache = None;
                                self.notice("Memory entry retired.");
                            }
                            Err(error) => {
                                self.fail(format!("The entry was not retired: {error:#}"))
                            }
                        },
                    },
                    KeyCode::Esc | KeyCode::Char('n') => {}
                    _ => self.overlay = Some(Overlay::Confirm { question, target }),
                }
                Vec::new()
            }
        }
    }

    fn theme_key(&mut self, key: KeyEvent, selected: usize, original: String) -> Vec<Action> {
        let last = theme::THEMES.len() - 1;
        let next = match key.code {
            KeyCode::Esc => {
                // Preview only; leaving restores whatever was stored.
                self.apply_theme(&original);
                return Vec::new();
            }
            KeyCode::Enter => {
                let id = theme::THEMES[selected.min(last)].id;
                self.persist_theme(id);
                self.notice(format!("Theme set to {}.", theme::theme(id).name));
                return Vec::new();
            }
            KeyCode::Up | KeyCode::Char('k') => selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => (selected + 1).min(last),
            KeyCode::Home => 0,
            KeyCode::End => last,
            _ => selected,
        };
        // Live preview: the whole interface repaints in the highlighted palette.
        self.apply_theme(theme::THEMES[next].id);
        self.overlay = Some(Overlay::Themes {
            selected: next,
            original,
        });
        Vec::new()
    }

    fn palette_key(&mut self, key: KeyEvent, mut field: Field, selected: usize) -> Vec<Action> {
        let width = self.viewport.width as u16;
        match key.code {
            KeyCode::Esc => return Vec::new(),
            KeyCode::Enter => {
                let matches = commands::search(&field.value);
                if let Some(command) = matches.get(selected.min(matches.len().saturating_sub(1))) {
                    if !command.complete_alone() {
                        self.input.set(format!("{} ", command.name));
                        self.focus = Focus::Composer;
                        return Vec::new();
                    }
                    let name = command.name.to_owned();
                    return self.command(&name, width);
                }
                return Vec::new();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => field.clear(),
            KeyCode::Backspace => field.backspace(),
            KeyCode::Delete => field.delete(),
            KeyCode::Left => field.left(),
            KeyCode::Right => field.right(),
            KeyCode::Char(ch)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                field.insert(ch)
            }
            _ => {}
        }
        let count = commands::search(&field.value).len();
        let selected = match key.code {
            KeyCode::Up => selected.saturating_sub(1),
            KeyCode::Down => (selected + 1).min(count.saturating_sub(1)),
            _ => selected.min(count.saturating_sub(1)),
        };
        self.overlay = Some(Overlay::Palette { field, selected });
        Vec::new()
    }

    fn commit_prompt(&mut self, target: PromptTarget, value: String) {
        match target {
            PromptTarget::AgentModel(id) => {
                let model = (!value.is_empty() && value != "default").then_some(value);
                self.edit_config(move |config| {
                    let profile = config
                        .agents
                        .iter_mut()
                        .find(|a| a.id == id)
                        .ok_or_else(|| anyhow::anyhow!("Unknown profile: {id}"))?;
                    profile.model = model;
                    Ok(format!(
                        "{id} now uses {}. Sessions already running keep their captured profile.",
                        profile.model.as_deref().unwrap_or("the provider default")
                    ))
                });
            }
            PromptTarget::AgentInstructions(id) => {
                self.edit_config(move |config| {
                    let profile = config
                        .agents
                        .iter_mut()
                        .find(|a| a.id == id)
                        .ok_or_else(|| anyhow::anyhow!("Unknown profile: {id}"))?;
                    profile.instructions = value;
                    Ok(format!(
                        "Instructions for {id} saved. This starts a new experience identity."
                    ))
                });
            }
            PromptTarget::Limit(key) => match value.parse::<usize>() {
                Ok(number) if number > 0 => {
                    let key = key.to_owned();
                    self.edit_config(move |config| {
                        match key.as_str() {
                            "parallel" => config.limits.parallel = number,
                            "turns" => config.limits.turns = number,
                            "timeout" => config.limits.turn_timeout_secs = number as u64,
                            _ => config.limits.attempts = number,
                        }
                        Ok(format!("{key} is now {number}."))
                    });
                }
                _ => self.fail("A limit must be a positive whole number."),
            },
            PromptTarget::MemorySearch => {
                self.memory_query = value;
                self.page_cache = None;
                self.set_view(View::Memory);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Submitting
    // -----------------------------------------------------------------------

    pub fn submit(&mut self, width: u16) -> Vec<Action> {
        let input = self.input.value.trim().to_owned();
        self.input.clear();
        self.completion = 0;
        if input.is_empty() {
            return Vec::new();
        }
        if self.history.last().map(String::as_str) != Some(input.as_str()) {
            self.history.push(input.clone());
        }
        self.history_index = self.history.len();
        self.follow = true;
        self.selected_entry = usize::MAX;
        match route(&input, self.active, self.session.as_deref()) {
            Route::Command => self.command(&input, width),
            Route::Queue(session) => {
                self.status = "Message saved. Agents receive it at the next turn boundary.".into();
                vec![Action::QueueMessage {
                    session,
                    text: input,
                }]
            }
            Route::Busy => {
                self.notice("A run is starting. Send the message once the session opens.");
                Vec::new()
            }
            Route::FollowUp(session) => {
                self.view = View::Chat;
                vec![Action::FollowUp {
                    session,
                    prompt: input,
                }]
            }
            Route::NewRun => {
                self.view = View::Chat;
                self.messages.clear();
                self.tasks.clear();
                self.streams.clear();
                self.changed();
                vec![Action::StartRun { prompt: input }]
            }
        }
    }

    // -----------------------------------------------------------------------
    // Commands
    // -----------------------------------------------------------------------

    pub fn command(&mut self, input: &str, width: u16) -> Vec<Action> {
        match self.run_command(input, width) {
            Ok(actions) => actions,
            Err(error) => {
                self.fail(format!("{error:#}"));
                Vec::new()
            }
        }
    }

    fn run_command(&mut self, input: &str, _width: u16) -> Result<Vec<Action>> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        let Some(&name) = parts.first() else {
            return Ok(Vec::new());
        };
        if commands::find(name).is_none() {
            bail!("Unknown command {name}. Press Ctrl+P for the command palette.");
        }
        match name {
            "/quit" => return Ok(vec![Action::Quit]),
            "/chat" => self.set_view(View::Chat),
            "/help" => self.set_view(View::Help),
            "/tasks" => self.set_view(View::Tasks),
            "/usage" => self.set_view(View::Usage),
            "/sessions" => self.set_view(View::Sessions),
            "/files" => self.set_view(View::Files),
            "/diff" => self.set_view(View::Changes),
            "/checks" => self.set_view(View::Checks),
            "/assignments" => self.set_view(View::Assignments),
            "/decisions" => self.set_view(View::Decisions),
            "/providers" => self.set_view(View::Providers),
            "/agents" => self.set_view(View::Agents),
            "/reputation" => self.set_view(View::Reputation),
            "/limits" => {
                if parts.len() == 3 {
                    let number: usize = parts[2].parse()?;
                    if number == 0 {
                        bail!("A limit must be a positive whole number.");
                    }
                    let key = match parts[1] {
                        "turns" => "turns",
                        "parallel" => "parallel",
                        "attempts" => "attempts",
                        "timeout" => "timeout",
                        other => bail!(
                            "Unknown limit {other}. Use turns, parallel, attempts or timeout."
                        ),
                    };
                    let key_owned = key.to_owned();
                    self.edit_config(move |config| {
                        match key_owned.as_str() {
                            "turns" => config.limits.turns = number,
                            "parallel" => config.limits.parallel = number,
                            "attempts" => config.limits.attempts = number,
                            _ => config.limits.turn_timeout_secs = number as u64,
                        }
                        Ok(format!("{key_owned} is now {number}."))
                    });
                }
                self.set_view(View::Limits);
            }
            "/sidebar" => {
                self.prefs.sidebar = !self.prefs.sidebar;
                self.save_prefs();
                self.notice(if self.prefs.sidebar {
                    "Sidebar shown."
                } else {
                    "Sidebar hidden."
                });
            }
            "/details" => {
                self.prefs.details = !self.prefs.details;
                self.save_prefs();
                self.changed();
                self.set_view(View::Chat);
                self.notice(if self.prefs.details {
                    "Showing every attributed agent message in full."
                } else {
                    "Showing routine coordination as one line each."
                });
            }
            "/theme" => match parts.get(1) {
                Some(id) => {
                    if !theme::THEMES.iter().any(|t| t.id == *id) {
                        bail!(
                            "Unknown theme {id}. Available: {}.",
                            theme::THEMES
                                .iter()
                                .map(|t| t.id)
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                    }
                    self.persist_theme(id);
                    self.notice(format!("Theme set to {}.", theme::theme(id).name));
                }
                None => self.open_theme_chooser(),
            },
            "/memory" => {
                if parts.get(1) == Some(&"forget") {
                    let id = parts
                        .get(2)
                        .ok_or_else(|| anyhow::anyhow!("Use /memory forget ID."))?;
                    self.store.forget_memory(id)?;
                    self.page_cache = None;
                    self.notice("Memory entry retired.");
                } else {
                    self.memory_query = parts[1..].join(" ");
                }
                self.set_view(View::Memory);
            }
            "/new" => {
                if self.active {
                    bail!("Stop the active run first with /stop.");
                }
                self.messages.clear();
                self.tasks.clear();
                self.notices.clear();
                self.streams.clear();
                self.statuses.clear();
                self.expanded.clear();
                self.session = None;
                self.session_status = "no session".into();
                self.session_team.clear();
                self.attribution = Attribution::default();
                self.stats.clear();
                self.turns_used = 0;
                self.set_view(View::Chat);
                self.notice("Ready for an unrelated task. The next message opens a new session.");
            }
            "/pause" | "/stop" => {
                if !self.active {
                    bail!("No run is active.");
                }
                self.notice("Stopping active turns. Continue later with /resume.");
                return Ok(vec![Action::Cancel]);
            }
            "/resume" => {
                if self.active {
                    bail!("A run is already active.");
                }
                let id = parts
                    .get(1)
                    .map(|s| (*s).to_owned())
                    .or_else(|| self.session.clone())
                    .ok_or_else(|| {
                        anyhow::anyhow!("Choose a session first with /sessions, or pass its id.")
                    })?;
                return Ok(vec![Action::Resume { session: id }]);
            }
            "/team" => {
                match (parts.get(1).copied(), parts.len()) {
                    (Some("add"), 3) | (Some("remove"), 3) => {
                        let id = parts[2].to_owned();
                        let adding = parts[1] == "add";
                        let member = self.config.team.contains(&id);
                        if adding == member {
                            self.notice(format!(
                                "{id} is already {}.",
                                if member {
                                    "a member"
                                } else {
                                    "outside the team"
                                }
                            ));
                        } else {
                            self.toggle_membership(&id);
                        }
                    }
                    (Some(_), _) => bail!("Use /team add ID or /team remove ID."),
                    (None, _) => {}
                }
                self.set_view(View::Team);
            }
            "/agent" => {
                match parts.get(1).copied() {
                    Some("add") if parts.len() >= 4 => {
                        let id = parts[2].to_owned();
                        let provider = parts[3].to_owned();
                        let model = parts.get(4).map(|s| (*s).to_owned());
                        self.edit_config(move |config| {
                            config.provider(&provider)?;
                            if config.agents.iter().any(|a| a.id == id) {
                                bail!("Profile {id} already exists.");
                            }
                            config.agents.push(AgentProfile {
                                id: id.clone(),
                                name: id.clone(),
                                provider,
                                model,
                                instructions: String::new(),
                                enabled: true,
                            });
                            Ok(format!("Profile {id} created."))
                        });
                    }
                    Some("model") if parts.len() == 4 => {
                        let id = parts[2].to_owned();
                        let model = parts[3].to_owned();
                        self.commit_prompt(PromptTarget::AgentModel(id), model);
                    }
                    Some("instructions") if parts.len() >= 4 => {
                        let id = parts[2].to_owned();
                        let text = parts[3..].join(" ");
                        self.commit_prompt(PromptTarget::AgentInstructions(id), text);
                    }
                    Some(id) if parts.len() == 2 => {
                        let profile = self.config.agent(id)?.clone();
                        self.set_view(View::Agents);
                        let page_width = self.page_width(_width);
                        self.page_selected = self
                            .page(page_width)
                            .items
                            .iter()
                            .position(|item| item.key == profile.id)
                            .unwrap_or(0);
                        return Ok(Vec::new());
                    }
                    _ => bail!(
                        "Use /agent add ID PROVIDER [MODEL], /agent model ID MODEL, /agent instructions ID TEXT, or /agent ID."
                    ),
                }
                self.set_view(View::Agents);
            }
            other => bail!("The command {other} is not implemented."),
        }
        Ok(Vec::new())
    }
}
