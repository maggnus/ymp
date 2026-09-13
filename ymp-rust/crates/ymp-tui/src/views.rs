//! Page construction.
//!
//! Every full-screen surface other than the conversation is a [`Page`]: a title, a list of
//! rows, and the detail of whichever row is selected. Building them here means each page is
//! data rather than a private drawing path, so they share one selection model, one set of
//! keyboard rules, and one empty state.
//!
//! Pages are read-only projections. Building one never starts an agent and never writes to
//! the user's working directory.

use crate::commands::{self, Group};
use crate::files;
use crate::git_view::{self, GitView};
use crate::highlight;
use crate::label;
use crate::provenance::{Acceptance, Pool, Records};
use crate::table::{Cell, Column, Sort, SortKey};
use crate::text;
use crate::theme::Theme;
use crate::usage::{self, Stats};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use std::path::Path;
use ymp_core::{
    AgentIdentity, AgentIdentityStatus, AgentProfile, AllocationBoundary, AllocationDecision,
    AssignmentRecord, BoardChange, BoardCommitment, BoardProposal, BoardProposalStatus,
    CapabilitySource, CheckOutcome, CheckRun, Config, ConfirmationStatus, DecisionRecord,
    ExecutionSettings, GrantRecord, InvocationRecord, InvocationState, KnowledgeRetrievalMode,
    Limits, MemoryEntry, ModelCapabilities, ModelEffort, NativeControl, NativeControlValue,
    NativeControlValues, PoolAgent, PoolExclusion, PoolModelStatus, ProviderCapabilities, Session,
    SessionBudget, Task, TaskAccess, TaskState, UsageTotals, WorkspaceAccess, WorkspaceWait,
};
use ymp_storage::Store;
use ymp_workspace::git::{self, Comparison, GitError};
use ymp_workspace::repository::Repository;

/// Every page a command can open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Chat,
    Tasks,
    Usage,
    Sessions,
    Files,
    Changes,
    Git,
    Checks,
    Assignments,
    Decisions,
    Team,
    Agents,
    Providers,
    Memory,
    Reputation,
    Limits,
    Help,
}

impl View {
    pub fn title(self) -> &'static str {
        match self {
            View::Chat => "Conversation",
            View::Tasks => "Tasks",
            View::Usage => "Token usage",
            View::Sessions => "Sessions",
            View::Files => "Files",
            View::Changes => "Changed files",
            View::Git => "Git",
            View::Checks => "Recorded checks",
            View::Assignments => "Assignments",
            View::Decisions => "Decisions",
            View::Team => "Team",
            View::Agents => "Agent profiles",
            View::Providers => "Providers",
            View::Memory => "Memory",
            View::Reputation => "Reputation",
            View::Limits => "Limits",
            View::Help => "Help",
        }
    }
    pub fn command(self) -> &'static str {
        match self {
            View::Chat => "/chat",
            View::Tasks => "/tasks",
            View::Usage => "/usage",
            View::Sessions => "/sessions",
            View::Files => "/files",
            View::Changes => "/diff",
            View::Git => "/git",
            View::Checks => "/checks",
            View::Assignments => "/assignments",
            View::Decisions => "/decisions",
            View::Team => "/team",
            View::Agents => "/agents",
            View::Providers => "/providers",
            View::Memory => "/memory",
            View::Reputation => "/reputation",
            View::Limits => "/limits",
            View::Help => "/help",
        }
    }

    /// Pages built from the records a session wrote. The controller refreshes its snapshot
    /// when one of them is opened, and again while a run is reporting into one, so a page
    /// never reads the store itself.
    pub fn reads_records(self) -> bool {
        matches!(
            self,
            View::Tasks
                | View::Assignments
                | View::Decisions
                | View::Team
                | View::Limits
                | View::Reputation
        )
    }

    /// Pages that describe what is installed on this machine. Inspecting that looks for
    /// executables on `PATH`, which is a read the controller performs, never a page.
    pub fn reads_pool(self) -> bool {
        matches!(self, View::Team | View::Agents | View::Providers)
    }

    /// Upper-case letters this page already answers, which no column may take as its sort key.
    pub fn sort_reserved(self) -> &'static [char] {
        match self {
            View::Agents | View::Providers => &['R'],
            _ => &[],
        }
    }
}

/// A row on a page, or a heading that opens a table. Headings are shown but never selected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemKind {
    Row,
    Heading,
}

#[derive(Clone)]
pub struct Item {
    pub kind: ItemKind,
    /// Identifier the page actions use: an agent id, a task id, a command name.
    pub key: String,
    /// A heading's title, as the page names its table. It may be empty.
    pub title: String,
    /// A heading's columns. The rows under it carry one cell for each.
    pub columns: Vec<Column>,
    /// A row's cells. A single cell under several columns is a note that spans its table.
    pub cells: Vec<Cell>,
    /// Everything the row stands for, read in full with Inspect.
    pub detail: Vec<Line<'static>>,
    /// Set on a heading when the page is arranged: the order its table is in.
    pub sort: Option<Sort>,
    /// Set on a heading when the page is arranged: the letter that sorts by each column.
    pub sort_keys: Vec<Option<char>>,
}

impl Item {
    pub fn row(key: impl Into<String>, cells: Vec<Cell>) -> Self {
        Self {
            kind: ItemKind::Row,
            key: key.into(),
            title: String::new(),
            columns: Vec::new(),
            cells,
            detail: Vec::new(),
            sort: None,
            sort_keys: Vec::new(),
        }
    }

    /// A row that is a sentence rather than a record. It spans the table it is in.
    pub fn note(key: impl Into<String>, spans: Vec<Span<'static>>) -> Self {
        Self::row(key, vec![Cell::spans(spans)])
    }

    pub fn with_detail(mut self, detail: Vec<Line<'static>>) -> Self {
        self.detail = detail;
        self
    }

    /// A heading that opens a table with these columns.
    pub fn table(title: impl Into<String>, columns: &[Column]) -> Self {
        Self {
            kind: ItemKind::Heading,
            key: String::new(),
            title: title.into(),
            columns: columns.to_vec(),
            cells: Vec::new(),
            detail: Vec::new(),
            sort: None,
            sort_keys: Vec::new(),
        }
    }

    /// A row that is a record, as opposed to a note that explains its page.
    pub fn is_record(&self) -> bool {
        self.kind == ItemKind::Row && self.cells.len() > 1
    }

    /// What a row reads as: its cells in order, empty ones left out.
    #[cfg(test)]
    pub fn text(&self) -> String {
        self.cells
            .iter()
            .map(Cell::plain)
            .filter(|text| !text.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

pub struct Page {
    pub view: View,
    pub title: String,
    pub subtitle: String,
    pub items: Vec<Item>,
    /// Shown instead of the list when there is nothing to show.
    pub empty: Vec<Line<'static>>,
    pub hints: Vec<(&'static str, &'static str)>,
}

impl Page {
    pub fn selectable(&self) -> bool {
        self.items.iter().any(|i| i.kind == ItemKind::Row)
    }
}

/// Everything a page may read. No field permits a write.
pub struct Ctx<'a> {
    pub store: &'a Store,
    pub config: &'a Config,
    pub cwd: &'a Path,
    /// What the controller discovered about version control for `cwd`. Pages present it;
    /// they never look at the filesystem for it themselves.
    pub repository: &'a Repository,
    /// Where the files page is in the working directory, and what the controller read there.
    pub files: &'a files::Files,
    /// What the controller last read from Git for the Git page. Pages never read a repository.
    pub git: &'a GitView,
    pub theme: &'a Theme,
    pub session: Option<&'a str>,
    pub tasks: &'a [Task],
    /// The profiles the loaded session captured, or the ones the next run would use.
    pub team: &'a [AgentProfile],
    /// True when `team` is what a session captured rather than what a next run would use.
    pub team_captured: bool,
    pub stats: &'a Stats,
    /// A run is active in this window. Only then is an open invocation one in flight.
    pub live: bool,
    /// What the controller last read of the loaded session's own records.
    pub records: &'a Records,
    /// What the controller last found installed on this machine.
    pub pool: &'a Pool,
    pub memory_query: &'a str,
    pub width: usize,
}

/// Build the page for `view`. Failures become a visible error state rather than a panic.
pub fn build(view: View, ctx: &Ctx) -> Page {
    match view {
        View::Chat => page(view, "The conversation", Vec::new(), ctx),
        View::Help => help(ctx),
        View::Tasks => tasks(ctx),
        View::Usage => tokens(ctx),
        View::Sessions => guard(view, sessions(ctx), ctx),
        View::Files => guard(view, files(ctx), ctx),
        View::Changes => guard(view, changes(ctx), ctx),
        View::Git => git_page(ctx),
        View::Checks => guard(view, checks(ctx), ctx),
        View::Assignments => assignments(ctx),
        View::Decisions => decisions(ctx),
        View::Team => team(ctx),
        View::Agents => agents(ctx),
        View::Providers => providers(ctx),
        View::Memory => guard(view, memory(ctx), ctx),
        View::Reputation => guard(view, reputation(ctx), ctx),
        View::Limits => limits(ctx),
    }
}

fn guard(view: View, result: anyhow::Result<Page>, ctx: &Ctx) -> Page {
    match result {
        Ok(page) => page,
        Err(error) => {
            let mut page = page(view, "Could not be read", Vec::new(), ctx);
            page.empty = vec![
                Line::from(Span::styled(
                    format!("{} {}", ctx.theme.markers.fail, view.title()),
                    ctx.theme.bad(),
                )),
                Line::default(),
                Line::from(Span::styled(
                    text::one_line(&format!("{error:#}")),
                    ctx.theme.body(),
                )),
            ];
            page
        }
    }
}

fn page(view: View, subtitle: &str, items: Vec<Item>, _ctx: &Ctx) -> Page {
    Page {
        view,
        title: view.title().to_owned(),
        subtitle: subtitle.to_owned(),
        items,
        empty: Vec::new(),
        hints: Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Shared building blocks
// ---------------------------------------------------------------------------

/// The widest label that shares a line with its value, and the gap between the two.
const LABEL_COLUMN: usize = 16;
const LABEL_GAP: usize = 2;

pub(crate) fn field(theme: &Theme, label: &str, value: &str, width: usize) -> Vec<Line<'static>> {
    // Every value of a record starts in one column, so the record reads as two aligned
    // columns. A label too long for its column takes a line of its own above the value
    // rather than pushing that value out of line. The column narrows on a narrow surface.
    let column = LABEL_COLUMN.min(width / 3);
    let indent = column + LABEL_GAP;
    let room = width.saturating_sub(indent).max(1);
    let pieces = text::wrap(&text::sanitize(value), room);
    let mut lines = Vec::new();
    let own_line = text::width(label) > column;
    if own_line {
        lines.push(Line::from(Span::styled(
            text::truncate(label, width),
            theme.muted(),
        )));
        if pieces.iter().all(|piece| piece.is_empty()) {
            return lines;
        }
    }
    for (index, piece) in pieces.into_iter().enumerate() {
        let head = if index == 0 && !own_line {
            format!("{label:<indent$}")
        } else {
            " ".repeat(indent)
        };
        lines.push(Line::from(vec![
            Span::styled(head, theme.muted()),
            Span::styled(piece, theme.body()),
        ]));
    }
    lines
}

pub(crate) fn paragraph(theme: &Theme, body: &str, width: usize) -> Vec<Line<'static>> {
    text::wrap(&text::sanitize(body), width.max(8))
        .into_iter()
        .map(|piece| Line::from(Span::styled(piece, theme.body())))
        .collect()
}

fn nothing(theme: &Theme, headline: &str, hint: &str, width: usize) -> Vec<Line<'static>> {
    let mut lines = vec![
        Line::from(Span::styled(headline.to_owned(), theme.bold())),
        Line::default(),
    ];
    lines.extend(
        text::wrap(hint, width.max(8))
            .into_iter()
            .map(|piece| Line::from(Span::styled(piece, theme.muted()))),
    );
    lines
}

/// Marker, word and colour for a task state. The word is the authority; the colour repeats it.
pub fn task_state(state: TaskState, theme: &Theme) -> (String, &'static str, Style) {
    let m = theme.markers;
    match state {
        TaskState::Ready => (m.idle.into(), "ready", theme.muted()),
        TaskState::Running => (m.busy.into(), "running", theme.warn()),
        TaskState::Review => (m.notice.into(), "in review", theme.info()),
        TaskState::Accepted => (m.ok.into(), "accepted", theme.good()),
        TaskState::Blocked => (m.fail.into(), "blocked", theme.bad()),
    }
}

/// Marker, word and colour for a session status.
pub fn session_status(status: &str, theme: &Theme) -> (String, Style) {
    let m = theme.markers;
    match status {
        "completed" => (format!("{} completed", m.ok), theme.good()),
        "running" => (format!("{} running", m.busy), theme.warn()),
        "paused" => (format!("{} paused", m.paused), theme.info()),
        "blocked" => (format!("{} blocked", m.fail), theme.bad()),
        other => (format!("{} {other}", m.idle), theme.muted()),
    }
}

fn yes_no(value: bool, theme: &Theme) -> Span<'static> {
    if value {
        Span::styled(format!("{} on", theme.markers.ok), theme.good())
    } else {
        Span::styled(format!("{} off", theme.markers.idle), theme.faint())
    }
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

const COMMAND_COLUMNS: [Column; 2] = [Column::left("COMMAND"), Column::left("SUMMARY").flex()];
const KEY_COLUMNS: [Column; 2] = [Column::left("KEY"), Column::left("ACTION").flex()];
const NOTICE_COLUMNS: [Column; 1] = [Column::left("NOTICE").flex()];

fn help(ctx: &Ctx) -> Page {
    let theme = ctx.theme;
    let mut items = Vec::new();
    for group in Group::all() {
        items.push(Item::table(group.title(), &COMMAND_COLUMNS));
        for command in commands::COMMANDS.iter().filter(|c| c.group == *group) {
            let mut detail = field(theme, "usage", command.usage, ctx.width);
            detail.push(Line::default());
            detail.extend(paragraph(theme, command.summary, ctx.width));
            items.push(
                Item::row(
                    command.name,
                    vec![
                        Cell::text(command.name, theme.accent()),
                        Cell::text(command.summary, theme.muted()),
                    ],
                )
                .with_detail(detail),
            );
        }
    }
    items.push(Item::table("Keyboard", &KEY_COLUMNS));
    for (key, description) in commands::KEYS {
        items.push(
            Item::row(
                *key,
                vec![
                    Cell::text(*key, theme.accent()),
                    Cell::text(*description, theme.muted()),
                ],
            )
            .with_detail(paragraph(theme, description, ctx.width)),
        );
    }
    let notice = format!(
        "Source previews are highlighted with syntax definitions the bat project collects, bundled by two-face. Their licences and notices are listed at {}",
        highlight::NOTICES
    );
    items.push(Item::table("Included syntax definitions", &NOTICE_COLUMNS));
    items.push(
        Item::note("notices", vec![Span::styled(notice.clone(), theme.muted())])
            .with_detail(paragraph(theme, &notice, ctx.width)),
    );
    Page {
        view: View::Help,
        title: View::Help.title().into(),
        subtitle: "Every command and key, in one place".into(),
        items,
        empty: Vec::new(),
        hints: vec![
            ("Enter", "run the command"),
            ("Esc", "back to the conversation"),
        ],
    }
}

const TASK_COLUMNS: [Column; 5] = [
    Column::left(""),
    Column::left("TITLE").flex(),
    Column::left("STATE"),
    Column::left("GRADE").hide(2),
    Column::left("RESPONSIBLE").hide(1),
];

fn tasks(ctx: &Ctx) -> Page {
    let theme = ctx.theme;
    let rows = ctx
        .tasks
        .iter()
        .map(|task| {
            let (marker, word, style) = task_state(task.state, theme);
            let assignee = task
                .assignee
                .as_deref()
                .map(|id| presented_name(ctx, id))
                .unwrap_or_else(|| "unassigned".into());
            let mut detail = Vec::new();
            detail.extend(field(theme, "state", word, ctx.width));
            detail.extend(field(theme, "assignee", &assignee, ctx.width));
            if let Some(reviewer) = &task.reviewer {
                detail.extend(field(
                    theme,
                    "reviewer",
                    &presented_name(ctx, reviewer),
                    ctx.width,
                ));
            }
            detail.extend(field(
                theme,
                "competence",
                &format!("{} · {}", task.competence, task.difficulty),
                ctx.width,
            ));
            detail.extend(field(
                theme,
                "declared access",
                declared_access_words(task.access),
                ctx.width,
            ));
            detail.extend(field(
                theme,
                "attempts",
                &task.attempts.to_string(),
                ctx.width,
            ));
            if !task.dependencies.is_empty() {
                detail.extend(field(
                    theme,
                    "depends on",
                    &task
                        .dependencies
                        .iter()
                        .map(|id| text::short_id(id))
                        .collect::<Vec<_>>()
                        .join(", "),
                    ctx.width,
                ));
            }
            if !task.checks.is_empty() {
                detail.extend(field(
                    theme,
                    "planned checks",
                    &task.checks.join(" ; "),
                    ctx.width,
                ));
            }
            if let Some(workspace) = &task.workspace {
                detail.extend(field(
                    theme,
                    "directory",
                    &workspace.display().to_string(),
                    ctx.width,
                ));
            }
            detail.extend(board_task_lines(ctx, &task.id));
            let deferred = ctx
                .records
                .deferrals_for_task(&task.id)
                .into_iter()
                .map(|(decision, _)| decision.id.clone())
                .collect::<Vec<_>>();
            for (decision, wait) in ctx.records.waits_for_task(&task.id) {
                // A deferral is a wait too, and it is already named above as what was put off.
                if deferred.contains(&decision.id) {
                    continue;
                }
                detail.extend(field(theme, "waited", &wait_words(wait), ctx.width));
            }
            if task.interrupted {
                detail.extend(field(theme, "interrupted", "yes", ctx.width));
            }
            detail.push(Line::default());
            detail.extend(paragraph(theme, &task.description, ctx.width));
            if let Some(result) = &task.result {
                detail.push(Line::default());
                detail.push(Line::from(Span::styled(
                    "Reported result".to_owned(),
                    theme.muted(),
                )));
                detail.extend(text::markdown(result, ctx.width, theme, theme.body()));
            }
            detail.push(Line::default());
            detail.push(Line::from(Span::styled(
                "Acceptance".to_owned(),
                theme.muted(),
            )));
            detail.extend(acceptance_lines(ctx, task));
            let grade = match ctx.records.acceptance(&task.id) {
                Some(acceptance) => Cell::text(
                    grade_word(&acceptance),
                    if acceptance.confirmed() {
                        theme.good()
                    } else {
                        theme.info()
                    },
                ),
                None => Cell::empty(),
            };
            Item::row(
                task.id.clone(),
                vec![
                    Cell::text(marker, style),
                    Cell::text(text::one_line(&task.title), theme.text()),
                    Cell::text(word, style),
                    grade,
                    Cell::text(
                        responsibility_or_assignee(ctx, task, &assignee),
                        theme.faint(),
                    ),
                ],
            )
            .with_detail(detail)
        })
        .collect::<Vec<_>>();
    let mut items = Vec::new();
    if !rows.is_empty() {
        items.push(Item::table("", &TASK_COLUMNS));
    }
    items.extend(rows);
    let items = board_items(ctx, items);
    let accepted = ctx
        .tasks
        .iter()
        .filter(|t| t.state == TaskState::Accepted)
        .count();
    Page {
        view: View::Tasks,
        title: View::Tasks.title().into(),
        subtitle: if ctx.tasks.is_empty() {
            "No task graph for this session".into()
        } else {
            let pending = ctx.records.pending_proposals();
            match pending {
                0 => format!("{accepted} of {} accepted", ctx.tasks.len()),
                1 => format!("{accepted} of {} accepted · 1 proposal waiting", ctx.tasks.len()),
                many => format!(
                    "{accepted} of {} accepted · {many} proposals waiting",
                    ctx.tasks.len()
                ),
            }
        },
        items,
        empty: nothing(
            theme,
            "No tasks yet",
            "A task graph appears once the team has agreed on a plan. Simple requests are answered without one.",
            ctx.width,
        ),
        hints: vec![("Enter", "inspect"), ("Esc", "back")],
    }
}

/// Token statistics for the loaded session: the whole session, then every agent in it.
///
/// The page is a projection of what the store recorded. It starts nothing, and it never
/// fills a gap with a guess: a count no provider reported stays a dash, and a figure that
/// an unfinished invocation can still add to is marked as a lower bound.
/// The turn bound these figures should be read against, named as what it is.
fn turn_bound_words(ctx: &Ctx) -> String {
    match captured_limits(ctx) {
        Some((limits, _)) => format!(
            "{} of {} captured by this session",
            ctx.records
                .trace
                .as_ref()
                .map(|trace| trace.session.turns_used)
                .unwrap_or(0),
            limits.turns
        ),
        None => format!(
            "this session captured no bound of its own; {} is what a new run would start with",
            ctx.config.limits.turns
        ),
    }
}

fn tokens(ctx: &Ctx) -> Page {
    let theme = ctx.theme;
    // One detail block is read in two places: under the list, and in the overlay Enter
    // opens, which is narrower. Prose wraps to fit both, and the figures sit in a column
    // of their own rather than at the far right of whichever surface is showing them.
    let prose = ctx.width.saturating_sub(6).clamp(16, PROSE);
    let ledger = prose.min(LEDGER);
    let total = ctx.stats.total();
    let mut items = Vec::new();
    if usage::present(ctx.session, ctx.stats) {
        let unattributed = ctx.stats.unattributed();
        // The session total already counts every invocation, including any the session
        // cannot attribute, so its own counters say all there is to say. What it does not
        // say on its own is which session it belongs to and which bound it was spent
        // against, so both are named first.
        let mut detail = Vec::new();
        if let Some(session) = ctx.session {
            detail.extend(field(theme, "session", session, prose));
        }
        detail.extend(field(theme, "turns", &turn_bound_words(ctx), prose));
        detail.push(Line::default());
        detail.extend(usage::breakdown(total, false, theme, ledger));
        detail.push(Line::default());
        detail.extend(paragraph(
            theme,
            &usage::coverage(total, unattributed),
            prose,
        ));
        detail.extend(open_note(ctx, total));
        if let Some(note) = usage::unattributed_note(unattributed) {
            detail.push(Line::default());
            detail.extend(hint(theme, &note, prose));
        }
        for note in usage::NOTES {
            detail.push(Line::default());
            detail.extend(hint(theme, note, prose));
        }
        items.push(Item::table("Session", &SESSION_TOKEN_COLUMNS));
        items.push(
            Item::row(
                "session",
                vec![
                    Cell::text("session total", theme.text()),
                    token_cell(
                        usage::headline(total, theme),
                        total,
                        usage::headline_style(total, theme),
                    ),
                ],
            )
            .with_detail(detail),
        );

        let rows = usage::agent_rows(ctx.stats, ctx.team, ctx.config);
        let mut heading = false;
        for row in rows.iter().filter(|row| !row.outside_team) {
            if !std::mem::replace(&mut heading, true) {
                items.push(Item::table("Agents", &AGENT_TOKEN_COLUMNS));
            }
            items.push(agent_usage(ctx, row));
        }
        let mut heading = false;
        for row in rows.iter().filter(|row| row.outside_team) {
            if !std::mem::replace(&mut heading, true) {
                items.push(Item::table(
                    "Recorded outside the captured team",
                    &AGENT_TOKEN_COLUMNS,
                ));
            }
            items.push(agent_usage(ctx, row));
        }
    }
    Page {
        view: View::Usage,
        title: View::Usage.title().into(),
        subtitle: match usage::present(ctx.session, ctx.stats) {
            false => "No session is loaded".to_owned(),
            // A narrow column keeps the figure; the coverage behind it is one row down,
            // in the detail of the session row, rather than pushing the title off screen.
            true if ctx.width < 60 => format!("{} tokens", usage::headline(total, theme)),
            true => format!(
                "{} tokens · {}",
                usage::headline(total, theme),
                usage::coverage(total, ctx.stats.unattributed())
            ),
        },
        items,
        empty: nothing(
            theme,
            "No session is loaded",
            "Token statistics belong to a session. Open one from /sessions, or describe a task to start one. Each figure appears as its provider reports it, which for some providers is only once a turn has completed.",
            ctx.width,
        ),
        hints: vec![("Enter", "inspect"), ("Esc", "back")],
    }
}

/// One agent's row. The provider is named as metadata and never merges two agents: each
/// agent in the session keeps its own counters, however many share a provider.
fn agent_usage(ctx: &Ctx, row: &usage::AgentUsage) -> Item {
    let theme = ctx.theme;
    let prose = ctx.width.saturating_sub(6).clamp(16, PROSE);
    let ledger = prose.min(LEDGER);
    let totals = row.totals.as_ref();
    let mut detail = field(theme, "agent", &row.id, prose);
    detail.extend(field(theme, "ran as", &models_used(ctx, &row.id), prose));
    detail.extend(field(
        theme,
        "provider",
        if row.provider.is_empty() {
            "unknown"
        } else {
            &row.provider
        },
        prose,
    ));
    if row.outside_team {
        detail.extend(field(
            theme,
            "membership",
            "spent tokens in this session but is not in the team it captured",
            prose,
        ));
    }
    detail.push(Line::default());
    detail.extend(usage::breakdown(totals, row.incomplete, theme, ledger));
    detail.push(Line::default());
    detail.extend(paragraph(theme, &usage::coverage(totals, 0), prose));
    detail.extend(open_note(ctx, totals));
    if row.incomplete {
        detail.push(Line::default());
        detail.extend(hint(
            theme,
            &format!(
                "The session recorded {} invocation(s) without an agent. Any of them could be this one's, so what is shown here is a lower bound rather than this agent's whole spending.",
                ctx.stats.unattributed()
            ),
            prose,
        ));
    }
    detail.push(Line::default());
    detail.extend(hint(
        theme,
        "Counters belong to the agent, not to the provider it runs on. Two agents that share a provider are counted apart.",
        prose,
    ));
    Item::row(
        row.id.clone(),
        vec![
            Cell::text(presented_name(ctx, &row.id), theme.text()),
            Cell::text(row.provider.clone(), theme.faint()),
            token_cell(
                usage::agent_headline(row, theme),
                totals,
                usage::agent_headline_style(row, theme),
            ),
        ],
    )
    .with_detail(detail)
}

const SESSION_TOKEN_COLUMNS: [Column; 2] = [Column::left("SCOPE").flex(), Column::right("TOKENS")];
const AGENT_TOKEN_COLUMNS: [Column; 3] = [
    Column::left("AGENT").flex(),
    Column::left("PROVIDER").hide(1),
    Column::right("TOKENS"),
];

/// A token figure as its headline reads, ordered by the tokens reported. A figure nobody
/// reported is ordered as unknown rather than as zero.
fn token_cell(display: String, totals: Option<&UsageTotals>, style: Style) -> Cell {
    Cell::number(
        display,
        totals
            .and_then(|totals| totals.known_total())
            .map(i128::from),
        style,
    )
}

/// Say what the open invocations mean, when there are any.
fn open_note(ctx: &Ctx, totals: Option<&UsageTotals>) -> Vec<Line<'static>> {
    let open = totals.map_or(0, |totals| totals.open_calls);
    usage::open_note(open, ctx.live)
        .map(|note| {
            hint(
                ctx.theme,
                &note,
                ctx.width.saturating_sub(6).clamp(16, PROSE),
            )
        })
        .unwrap_or_default()
}

/// Width of the column the token figures are set in. Wide enough for a grouped figure
/// beside its label, narrow enough to survive the overlay the detail is also read in.
const LEDGER: usize = 44;

/// Widest the statistics prose is wrapped, whatever the terminal offers. The detail of a
/// row is also read inside the inspect overlay, which is never wider than 86 cells.
const PROSE: usize = 78;

fn hint(theme: &Theme, body: &str, width: usize) -> Vec<Line<'static>> {
    text::wrap(&text::sanitize(body), width.max(8))
        .into_iter()
        .map(|piece| Line::from(Span::styled(piece, theme.faint())))
        .collect()
}

const SESSION_COLUMNS: [Column; 5] = [
    Column::left("SESSION"),
    Column::left("TITLE").flex(),
    Column::left("STATUS"),
    Column::right("TURNS").hide(2),
    Column::left("CREATED").hide(1),
];

/// A recorded time to the minute, which is what tells two sessions apart in a list.
fn stamp(timestamp: &str) -> String {
    timestamp
        .get(..16)
        .map_or_else(|| timestamp.to_owned(), |minute| minute.replace('T', " "))
}

fn sessions(ctx: &Ctx) -> anyhow::Result<Page> {
    let theme = ctx.theme;
    let project = ctx.store.project(ctx.cwd)?;
    let sessions: Vec<Session> = ctx.store.sessions(Some(&project.id))?;
    let rows = sessions
        .iter()
        .map(|session| {
            let (status, style) = session_status(&session.status, theme);
            let current = ctx.session == Some(session.id.as_str());
            let mut detail = Vec::new();
            detail.extend(field(theme, "session", &session.id, ctx.width));
            detail.extend(field(theme, "status", &session.status, ctx.width));
            detail.extend(field(theme, "created", &session.created_at, ctx.width));
            detail.extend(field(
                theme,
                "turns used",
                &session.turns_used.to_string(),
                ctx.width,
            ));
            // A session's team is named by what its own turns recorded. The profiles it captured
            // carry configured names and aliases, and the providers as configured now say nothing
            // about it, so only the loaded session, whose records were read, gives models here.
            detail.extend(field(
                theme,
                "team",
                &session
                    .team
                    .iter()
                    .map(|profile| profile.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
                ctx.width,
            ));
            if current && ctx.records.trace.is_some() {
                for profile in &session.team {
                    detail.extend(field(
                        theme,
                        &profile.id,
                        &models_used(ctx, &profile.id),
                        ctx.width,
                    ));
                }
            } else if !session.team.is_empty() {
                detail.extend(field(
                    theme,
                    "models",
                    "read from this session's own records, which are read when it is loaded",
                    ctx.width,
                ));
            }
            detail.push(Line::default());
            detail.extend(paragraph(theme, &session.title, ctx.width));
            detail.push(Line::default());
            detail.push(Line::from(Span::styled(
                "Enter loads this conversation for reading. It does not start agents.".to_owned(),
                theme.muted(),
            )));
            detail.push(Line::from(Span::styled(
                "r continues the run, which does start agents.".to_owned(),
                theme.muted(),
            )));
            let mut title = vec![Span::styled(
                text::one_line(&session.title),
                if current { theme.bold() } else { theme.text() },
            )];
            if current {
                title.push(Span::styled(" · loaded".to_owned(), theme.accent()));
            }
            Item::row(
                session.id.clone(),
                vec![
                    Cell::text(text::short_id(&session.id), theme.faint()),
                    Cell::spans(title),
                    Cell::text(status, style),
                    Cell::number(
                        session.turns_used.to_string(),
                        Some(session.turns_used as i128),
                        theme.muted(),
                    ),
                    Cell::text(stamp(&session.created_at), theme.faint())
                        .sorted_by(SortKey::Text(session.created_at.clone())),
                ],
            )
            .with_detail(detail)
        })
        .collect::<Vec<_>>();
    let mut items = Vec::new();
    if !rows.is_empty() {
        items.push(Item::table("", &SESSION_COLUMNS));
    }
    items.extend(rows);
    Ok(Page {
        view: View::Sessions,
        title: View::Sessions.title().into(),
        subtitle: format!("{} saved in {}", sessions.len(), project.name),
        items,
        empty: nothing(
            theme,
            "No saved sessions for this project",
            "Send a prompt to start one. Sessions are stored as metadata under the ymp home directory; your files stay where they are.",
            ctx.width,
        ),
        hints: vec![
            ("Enter", "open read-only"),
            ("r", "resume the run"),
            ("Esc", "back"),
        ],
    })
}

/// Where the files page is. The explorer lists the directory and draws its rows, so the page
/// itself is the header, the keys, and what is shown when there is no list at all.
fn files(ctx: &Ctx) -> anyhow::Result<Page> {
    let theme = ctx.theme;
    let mut page = Page {
        view: View::Files,
        title: View::Files.title().into(),
        subtitle: files::display_path(ctx.files.directory()),
        items: Vec::new(),
        empty: Vec::new(),
        hints: vec![
            ("Enter", "open"),
            ("Backspace", "parent"),
            ("/", "filter"),
            ("d", "details"),
            ("r", "read again"),
            ("Esc", "close"),
        ],
    };
    if !ctx.files.is_open() {
        page.empty = vec![
            Line::from(Span::styled(
                format!("{} This directory cannot be listed", theme.markers.fail),
                theme.bad(),
            )),
            Line::default(),
        ];
        page.empty.extend(paragraph(
            theme,
            ctx.files.notice().unwrap_or_default(),
            ctx.width,
        ));
        page.empty.push(Line::default());
        page.empty.extend(hint(
            theme,
            "r tries again, and Esc closes the page.",
            ctx.width,
        ));
    }
    Ok(page)
}

const GIT_COLUMNS: [Column; 5] = [
    Column::left("ST"),
    Column::left("PATH").flex(),
    Column::left("STATE").hide(2),
    Column::right("ADDED").hide(1),
    Column::right("REMOVED").hide(1),
];

/// The Git page: the changed files of one comparison in one worktree, as the backend last read
/// them. A failed reading is said, and a list kept from before it is marked as out of date.
fn git_page(ctx: &Ctx) -> Page {
    let theme = ctx.theme;
    let view = ctx.git;
    let root = view.root(ctx.cwd);
    let mut page = Page {
        view: View::Git,
        title: View::Git.title().into(),
        subtitle: files::display_path(root),
        items: Vec::new(),
        empty: Vec::new(),
        hints: vec![
            ("Enter", "diff"),
            ("m", "compare"),
            ("w", "worktree"),
            ("b", "branch"),
            ("r", "refresh"),
            ("Esc", "close"),
        ],
    };
    let Some(snapshot) = view.snapshot() else {
        page.empty = match view.error() {
            Some(GitError::NotRepository) => nothing(
                theme,
                "Not a Git working tree",
                &format!(
                    "{} is not inside a Git working tree. r reads it again.",
                    files::display_path(root)
                ),
                ctx.width,
            ),
            Some(error) => {
                let mut lines = vec![
                    Line::from(Span::styled(
                        format!("{} Git could not be read", theme.markers.fail),
                        theme.bad(),
                    )),
                    Line::default(),
                ];
                lines.extend(paragraph(theme, &error.to_string(), ctx.width));
                lines.push(Line::default());
                lines.extend(hint(
                    theme,
                    "r reads again, and Esc closes the page.",
                    ctx.width,
                ));
                lines
            }
            None => nothing(
                theme,
                "Reading Git",
                "The changes appear when the reading answers.",
                ctx.width,
            ),
        };
        return page;
    };
    // Branch names are case sensitive, so they are shown where they keep their case: a table
    // title is drawn in upper case.
    let since = match (snapshot.comparison, &snapshot.base) {
        (Comparison::Committed, Some(base)) => format!(" since {}", text::sanitize(base)),
        (Comparison::Committed, None) => ", no base branch identified".to_owned(),
        (Comparison::Uncommitted, _) => String::new(),
    };
    page.subtitle = format!(
        "{}{since} · {}",
        branch_label(snapshot),
        files::display_path(&snapshot.root)
    );
    let heading = match snapshot.comparison {
        Comparison::Uncommitted => "Uncommitted changes against HEAD",
        Comparison::Committed => "Committed changes since the base branch",
    }
    .to_owned();
    let mut notes = Vec::new();
    if let Some(error) = view.error() {
        notes.push((
            theme.bad(),
            format!(
                "{} The last reading failed: {error}. What is shown is from the reading before it.",
                theme.markers.fail
            ),
        ));
    }
    if let Some(notice) = &snapshot.notice {
        notes.push((theme.warn(), text::sanitize(notice)));
    }
    if snapshot.files.is_empty() {
        let advice = match snapshot.comparison {
            Comparison::Uncommitted => {
                "Nothing in the working tree differs from HEAD. m shows what the branch has committed since its base."
            }
            Comparison::Committed if snapshot.dirty => {
                "The branch has no commits beyond its base. The working tree has uncommitted changes: m shows them."
            }
            Comparison::Committed => "The branch has no commits beyond its base.",
        };
        let mut lines = vec![
            Line::from(Span::styled(heading, theme.muted())),
            Line::default(),
        ];
        lines.extend(nothing(theme, "No changes to display", advice, ctx.width));
        for (style, note) in notes {
            lines.push(Line::default());
            lines.extend(
                text::wrap(&note, ctx.width.max(8))
                    .into_iter()
                    .map(|piece| Line::from(Span::styled(piece, style))),
            );
        }
        page.empty = lines;
        return page;
    }
    for (index, (style, note)) in notes.into_iter().enumerate() {
        page.items.push(Item::note(
            format!("note:{index}"),
            vec![Span::styled(note, style)],
        ));
    }
    page.items.push(Item::table(heading, &GIT_COLUMNS));
    page.items
        .extend(snapshot.files.iter().map(|change| git_row(ctx, change)));
    page
}

fn branch_label(snapshot: &git::Snapshot) -> String {
    match (&snapshot.branch, &snapshot.head) {
        (Some(branch), _) => text::sanitize(branch),
        (None, Some(head)) => format!("detached at {}", git_view::short_head(head)),
        (None, None) => "no commits yet".into(),
    }
}

fn git_row(ctx: &Ctx, change: &git::Change) -> Item {
    let theme = ctx.theme;
    let path = files::display_path(&change.path);
    let shown = match &change.old_path {
        Some(old) => format!("{path} (from {})", files::display_path(old)),
        None => path.clone(),
    };
    let status = text::sanitize(&change.status);
    let status_style = match status.chars().next() {
        Some('A' | '?') => theme.good(),
        Some('D' | 'U') => theme.bad(),
        _ => theme.warn(),
    };
    let state = [
        (change.staged, "staged"),
        (change.unstaged, "unstaged"),
        (change.untracked, "untracked"),
    ]
    .iter()
    .filter(|(set, _)| *set)
    .map(|(_, word)| *word)
    .collect::<Vec<_>>()
    .join(", ");
    let count = |value: Option<u64>, sign: &str, style: Style| match value {
        _ if change.binary => Cell::text("binary", theme.muted()),
        Some(lines) => Cell::number(format!("{sign}{lines}"), Some(i128::from(lines)), style),
        None => Cell::number(theme.markers.unknown, None, theme.faint()),
    };
    let mut detail = field(theme, "path", &path, ctx.width);
    if let Some(old) = &change.old_path {
        detail.extend(field(
            theme,
            "previous path",
            &files::display_path(old),
            ctx.width,
        ));
    }
    detail.extend(field(theme, "status", &status, ctx.width));
    if !state.is_empty() {
        detail.extend(field(theme, "state", &state, ctx.width));
    }
    let lines =
        |value: Option<u64>| value.map_or_else(|| "not reported".to_owned(), |n| n.to_string());
    if change.binary {
        detail.extend(field(theme, "content", "binary", ctx.width));
    } else {
        detail.extend(field(theme, "added", &lines(change.added), ctx.width));
        detail.extend(field(theme, "removed", &lines(change.removed), ctx.width));
    }
    Item::row(
        git_view::row_key(&change.path),
        vec![
            Cell::text(status, status_style),
            Cell::text(shown, theme.text()),
            Cell::text(state, theme.muted()),
            count(change.added, "+", theme.good()),
            count(change.removed, "-", theme.bad()),
        ],
    )
    .with_detail(detail)
}

pub(crate) fn size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "kB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// The statement the interface makes wherever a change is shown, because a list of paths
/// otherwise reads like a list of backups. The two parts are shown together, except where
/// the first is already the heading of what the reader is looking at.
const NO_EARLIER_CONTENT: &str = "ymp cannot restore a previous version of a file.";
/// How a run uses the working directory in this release. Every sentence is about what the
/// code does: the directory is the one that was selected, the only exclusion is the lock a
/// run holds, and nothing is staged, copied or published anywhere else.
const DIRECT_WORKSPACE: &[&str] = &[
    "Agents work in this directory itself. Files they create, change or delete are the real ones, there is no staging copy and no review step between a turn and the directory.",
    "What a turn may do here is the access its execution backend actually enforces, and the run records that access per turn. The native adapter enforces read-only access for a turn it asks read-only, with one exception it records rather than hides: over the ACP protocol a mode name is not a filesystem guarantee, so such a turn is recorded as writing the whole directory. Any other execution backend is taken to write the whole directory unless it states otherwise.",
    "A task declared read-only in the plan is asked read-only, and a task that declares nothing may write. The declaration is explicit and is never concluded from what the work looks like.",
    "Turns overlap only where their recorded access does not conflict. A turn that writes the whole directory excludes every other turn here, two readers do not exclude each other, and declared disjoint paths do not. Every turn that waits records why, and the checks and the acceptance that judge a candidate hold the whole directory while they run.",
    "One ymp run uses a project's directory at a time: a run holds an exclusive lock in ymp's own metadata home and another run refuses to start while it is held. That lock says nothing about other programs, or about a command run by hand, which can write here at any time.",
    "ymp keeps its own metadata and evidence in its home directory and the deliverables here. It does not create a hidden copy of this tree, and it does not move or rename the directory you selected.",
    "Isolated execution with a reviewed publication step is not part of this release. What a run writes here is applied as it works, so interrupting one leaves whatever it had already written.",
];
const WHAT_WAS_RECORDED: &str =
    "It recorded a path, a status and a content hash for each file, never a copy, so \
     earlier content exists only where the working directory's own version control or a \
     backup already kept it.";

const CHANGE_COLUMNS: [Column; 2] = [Column::left("PATH").flex(), Column::left("STATUS")];
const OUTCOME_COLUMNS: [Column; 3] = [
    Column::left(""),
    Column::left("RESULT").flex(),
    Column::left("STATE"),
];

fn changes(ctx: &Ctx) -> anyhow::Result<Page> {
    let theme = ctx.theme;
    let mut rows = Vec::new();
    let mut subtitle = "No session is loaded".to_owned();
    if let Some(id) = ctx.session {
        let session = ctx.store.session(id)?;
        let file = ctx
            .store
            .session_dir(&session)
            .join("workspace/changes.json");
        subtitle = ctx.cwd.display().to_string();
        if let Ok(body) = std::fs::read_to_string(&file) {
            let parsed: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
            for change in parsed.as_array().cloned().unwrap_or_default() {
                let path = change["path"].as_str().unwrap_or_default().to_owned();
                let status = change["status"].as_str().unwrap_or("changed").to_owned();
                let style = match status.as_str() {
                    "created" => theme.good(),
                    "deleted" => theme.bad(),
                    _ => theme.warn(),
                };
                let mut detail = field(theme, "path", &path, ctx.width)
                    .into_iter()
                    .chain(field(theme, "status", &status, ctx.width))
                    .collect::<Vec<_>>();
                detail.push(Line::default());
                detail.extend(paragraph(
                    theme,
                    &format!("{NO_EARLIER_CONTENT} {WHAT_WAS_RECORDED}"),
                    ctx.width,
                ));
                rows.push(
                    Item::row(
                        path.clone(),
                        vec![Cell::text(path, theme.text()), Cell::text(status, style)],
                    )
                    .with_detail(detail),
                );
            }
        }
    }
    // With nothing to list, the page is prose and the empty state carries the statement.
    // With rows, it leads them, and opens selected, so the first frame already shows it.
    let mut outcomes = Vec::new();
    if let Some(reason) = &ctx.records.outcomes_unreadable {
        outcomes.push(unreadable_outcomes_row(ctx, reason));
    }
    outcomes.extend(
        ctx.records
            .outcomes
            .iter()
            .map(|outcome| outcome_row(ctx, outcome)),
    );
    let mut items = Vec::new();
    if !rows.is_empty() || !outcomes.is_empty() {
        items.push(recovery_row(ctx));
    }
    if !rows.is_empty() {
        items.push(Item::table("Recorded changes", &CHANGE_COLUMNS));
        items.append(&mut rows);
    }
    if !outcomes.is_empty() {
        items.push(Item::table(
            "Where accepted work was recorded",
            &OUTCOME_COLUMNS,
        ));
        items.append(&mut outcomes);
    }
    Ok(Page {
        view: View::Changes,
        title: View::Changes.title().into(),
        subtitle,
        items,
        empty: {
            let mut lines = nothing(
                theme,
                "No recorded file changes",
                "Change metadata is written when a run finishes or stops. Files themselves are created directly in the working directory.",
                ctx.width,
            );
            lines.push(Line::default());
            lines.extend(paragraph(
                theme,
                &format!("{NO_EARLIER_CONTENT} {WHAT_WAS_RECORDED}"),
                ctx.width,
            ));
            lines.push(Line::default());
            for sentence in DIRECT_WORKSPACE {
                lines.extend(paragraph(theme, sentence, ctx.width));
            }
            lines.push(Line::default());
            lines.extend(recovery_lines(ctx));
            lines
        },
        hints: vec![("Enter", "inspect"), ("Esc", "back")],
    })
}

/// The row that leads the change list: how the directory is used, what was recorded, and
/// what no longer exists. It is the first row, so its detail is what the first frame shows.
fn recovery_row(ctx: &Ctx) -> Item {
    let theme = ctx.theme;
    let mut detail = vec![
        Line::from(Span::styled(NO_EARLIER_CONTENT.to_owned(), theme.bold())),
        Line::default(),
    ];
    detail.extend(paragraph(theme, WHAT_WAS_RECORDED, ctx.width));
    detail.push(Line::default());
    detail.push(Line::from(Span::styled(
        "How this directory is used".to_owned(),
        theme.muted(),
    )));
    for sentence in DIRECT_WORKSPACE {
        detail.extend(paragraph(theme, sentence, ctx.width));
    }
    detail.push(Line::default());
    detail.extend(recovery_lines(ctx));
    Item::note(
        "recovery",
        vec![
            Span::styled(format!("{} ", theme.markers.notice), theme.info()),
            Span::styled(
                "How this directory is used, and what cannot be put back".to_owned(),
                theme.text(),
            ),
            Span::styled("  direct · metadata only".to_owned(), theme.muted()),
        ],
    )
    .with_detail(detail)
}

/// One accepted result and the directory the run recorded it in.
///
/// The location is historical: it is the directory the result was accepted in, and moving
/// the project afterwards does not move it. Nothing here re-reads the files.
fn outcome_row(ctx: &Ctx, outcome: &ymp_core::StoredOutcome) -> Item {
    let theme = ctx.theme;
    let (word, style) = match (outcome.confirmation, outcome.current) {
        (ConfirmationStatus::Confirmed, true) => ("confirmed".to_owned(), theme.good()),
        (_, true) => ("accepted, unconfirmed".to_owned(), theme.info()),
        (_, false) => ("superseded by later changes".to_owned(), theme.warn()),
    };
    let mut detail = field(
        theme,
        "recorded in",
        &outcome.directory.display().to_string(),
        ctx.width,
    );
    detail.extend(field(
        theme,
        "result",
        &format!(
            "{} · version {}",
            text::short_id(&outcome.result_id),
            outcome.result_version
        ),
        ctx.width,
    ));
    detail.extend(field(
        theme,
        "acceptance",
        &text::short_id(&outcome.acceptance_id),
        ctx.width,
    ));
    detail.extend(field(theme, "state", &word, ctx.width));
    if outcome.artifacts.is_empty() {
        detail.extend(field(theme, "artifacts", "none named", ctx.width));
    }
    for artifact in &outcome.artifacts {
        detail.extend(field(
            theme,
            "artifact",
            &format!(
                "{} · {}",
                artifact.path.display(),
                match &artifact.sha256 {
                    Some(digest) => format!("sha256 {}", text::short_id(digest)),
                    None => "no digest recorded".to_owned(),
                }
            ),
            ctx.width,
        ));
    }
    detail.push(Line::default());
    detail.extend(paragraph(
        theme,
        "This is the directory the run recorded when the result was accepted. It stays that directory: changing the project's path later does not move the record, and ymp keeps no copy of the artifact anywhere else. If a file was moved or deleted afterwards, the path is still the one that was recorded, and the digest is how that can be told.",
        ctx.width,
    ));
    if !outcome.current {
        detail.push(Line::default());
        detail.extend(paragraph(
            theme,
            "The task or the files this result was accepted against have changed since, so the acceptance is reported as superseded rather than confirmed. The record is not rewritten and the evidence it names is still the evidence it named.",
            ctx.width,
        ));
    }
    Item::row(
        outcome.result_id.clone(),
        vec![
            Cell::text(
                if outcome.current {
                    theme.markers.ok
                } else {
                    theme.markers.warn
                },
                style,
            ),
            Cell::text(text::one_line(&outcome.summary), theme.text()),
            Cell::text(word, style),
        ],
    )
    .with_detail(detail)
}

/// Recorded locations could not be read. An absent answer, stated as one.
fn unreadable_outcomes_row(ctx: &Ctx, reason: &str) -> Item {
    let theme = ctx.theme;
    let mut detail = field(theme, "reported", reason, ctx.width);
    detail.push(Line::default());
    detail.extend(paragraph(
        theme,
        "The accepted results of this session could not be listed with their locations, so this page shows none of them. That is a failed read and not a statement that the session accepted nothing: the records are in ymp's own storage either way, and nothing here was changed by the attempt.",
        ctx.width,
    ));
    Item::row(
        "outcome locations",
        vec![
            Cell::text(theme.markers.warn, theme.warn()),
            Cell::text("recorded locations", theme.text()),
            Cell::text("could not be read", theme.warn()),
        ],
    )
    .with_detail(detail)
}

/// Version control as the controller found it, then the limits of the record itself.
fn recovery_lines(ctx: &Ctx) -> Vec<Line<'static>> {
    let theme = ctx.theme;
    let mut lines = vec![Line::from(Span::styled(
        "Version control".to_owned(),
        theme.muted(),
    ))];
    for sentence in ctx.repository.describe() {
        lines.extend(paragraph(theme, &sentence, ctx.width));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "Limits of the record".to_owned(),
        theme.muted(),
    )));
    for sentence in [
        "The walk skips .git, node_modules, target, __pycache__, .DS_Store, .ymp2 and ymp's own metadata directory, so a change inside one of those was not recorded.",
        "A listed change means the file differed from the fingerprint taken when the session started. It does not say which agent or which other process wrote it.",
        "The list is written once, when a run finishes or stops. A run still working, or one whose process ended before it could write, leaves nothing here to read.",
    ] {
        lines.extend(paragraph(theme, sentence, ctx.width));
    }
    lines
}

/// Commands ymp ran itself, read from the session log, with what the log recorded.
///
/// Two kinds of row, kept apart: a run the log recorded, and a command the accepted plan
/// declared that the log has no run for. The second is not a result.
fn checks(ctx: &Ctx) -> anyhow::Result<Page> {
    let theme = ctx.theme;
    let recorded = match ctx.session {
        Some(id) => ctx.store.checks(id)?,
        None => Vec::new(),
    };
    let mut planned: Vec<(String, Vec<String>)> = Vec::new();
    for task in ctx.tasks {
        for command in &task.checks {
            // A run covers this command when the record names this task, and also when the
            // record names no task at all: the final pass runs the union of every declared
            // command, and a record written before runs carried a task names none either.
            // Both are matched by command text, which is all such a record offers.
            let covered = recorded.iter().any(|run| {
                run.command.as_deref() == Some(command.as_str())
                    && run
                        .task
                        .as_ref()
                        .is_none_or(|attempt| attempt.task_id == task.id)
            });
            if covered {
                continue;
            }
            match planned.iter_mut().find(|(known, _)| known == command) {
                Some((_, tasks)) => tasks.push(task.title.clone()),
                None => planned.push((command.clone(), vec![task.title.clone()])),
            }
        }
    }
    let mut items = Vec::new();
    if !recorded.is_empty() || !planned.is_empty() {
        items.push(how_checks_run(ctx));
    }
    if !recorded.is_empty() {
        items.push(Item::table("Recorded runs", &RECORDED_CHECK_COLUMNS));
        items.extend(recorded.iter().map(|run| recorded_check(ctx, run)));
    }
    if !planned.is_empty() {
        items.push(Item::table(
            "Declared, without a recorded run",
            &DECLARED_CHECK_COLUMNS,
        ));
        items.extend(
            planned
                .iter()
                .map(|(command, tasks)| declared_check(ctx, command, tasks)),
        );
    }
    Ok(Page {
        view: View::Checks,
        title: View::Checks.title().into(),
        subtitle: match ctx.session {
            None => "No session is loaded".to_owned(),
            Some(_) => format!(
                "{} recorded · {} declared without a run",
                recorded.len(),
                planned.len()
            ),
        },
        items,
        empty: {
            // With no session open nothing was read, which is not a statement about a
            // session's checks. This page of all pages must keep the two apart.
            let mut lines = match ctx.session {
                None => nothing(
                    theme,
                    "Nothing was read",
                    "No session is loaded, so no session log was read. Open one from /sessions to see the commands ymp ran for it.",
                    ctx.width,
                ),
                Some(_) => nothing(
                    theme,
                    "No checks were recorded",
                    "ymp records a check when the accepted plan supplies an acceptance command and the run reaches it. A session without one was judged by inspection alone.",
                    ctx.width,
                ),
            };
            lines.push(Line::default());
            lines.extend(paragraph(theme, HOW_CHECKS_RUN[0], ctx.width));
            lines
        },
        hints: vec![("Enter", "inspect"), ("Esc", "back")],
    })
}

/// How a check is executed and what is kept about it. Stated plainly, because the one
/// thing the interface must not imply is that running it was constrained or agreed to.
const HOW_CHECKS_RUN: &[&str] = &[
    "ymp runs these commands itself, with /bin/sh in the working directory, after the \
     agent's own turn in the same directory. It places no limit on what a check may do and \
     asks nothing before running one.",
    "Each run is recorded with its command, the directory, whether the command exited \
     zero, and the first 20000 characters of its combined output. A check that timed out, \
     or that was stopped with the run, leaves no record at all.",
    "A command that exits non-zero keeps the task unaccepted whatever an agent reported \
     about it. The reverse does not hold: a command that exits zero is evidence about that \
     command, not about the task.",
    "The commands come from the plan the session accepted. This page reads the log and \
     runs nothing.",
    "A run for one task records that task and its attempt, so it says nothing about the \
     same command declared by another task. The final pass over every declared command is \
     recorded without a task, as are runs from before records carried one, and such a run \
     is matched by its command text alone and counts for every task that declared that \
     command.",
];

fn how_checks_run(ctx: &Ctx) -> Item {
    let theme = ctx.theme;
    let mut detail = Vec::new();
    for (index, sentence) in HOW_CHECKS_RUN.iter().enumerate() {
        if index > 0 {
            detail.push(Line::default());
        }
        detail.extend(paragraph(theme, sentence, ctx.width));
    }
    Item::note(
        "how checks run",
        vec![
            Span::styled(format!("{} ", theme.markers.notice), theme.info()),
            Span::styled("How checks run".to_owned(), theme.text()),
            Span::styled("  read from the session log".to_owned(), theme.muted()),
        ],
    )
    .with_detail(detail)
}

const RECORDED_CHECK_COLUMNS: [Column; 3] = [
    Column::left(""),
    Column::left("COMMAND").flex(),
    Column::left("OUTCOME"),
];
const DECLARED_CHECK_COLUMNS: [Column; 3] = [
    Column::left(""),
    Column::left("COMMAND").flex(),
    Column::left("STATUS"),
];

/// Lines of recorded output shown before the rest is left to the full record.
const OUTPUT_LINES: usize = 80;

fn recorded_check(ctx: &Ctx, run: &CheckRun) -> Item {
    let theme = ctx.theme;
    let (marker, word, style) = match run.outcome {
        CheckOutcome::Passed => (theme.markers.ok, "passed", theme.good()),
        CheckOutcome::Failed => (theme.markers.fail, "failed", theme.bad()),
        CheckOutcome::Unrecorded => (theme.markers.unknown, "no recorded outcome", theme.muted()),
    };
    let command = run
        .command
        .clone()
        .unwrap_or_else(|| "command not recorded".to_owned());
    let mut detail = field(theme, "command", &command, ctx.width);
    detail.extend(field(theme, "outcome", word, ctx.width));
    detail.extend(field(
        theme,
        "task",
        &match &run.task {
            Some(attempt) => format!(
                "{} · attempt {}",
                ctx.tasks
                    .iter()
                    .find(|task| task.id == attempt.task_id)
                    .map(|task| task.title.clone())
                    .unwrap_or_else(|| text::short_id(&attempt.task_id)),
                attempt.attempt
            ),
            None => "no task recorded".to_owned(),
        },
        ctx.width,
    ));
    detail.extend(field(
        theme,
        "directory",
        run.directory.as_deref().unwrap_or("not recorded"),
        ctx.width,
    ));
    detail.extend(field(theme, "recorded", &run.recorded_at, ctx.width));
    detail.push(Line::default());
    match &run.output {
        Some(output) => {
            detail.push(Line::from(Span::styled(
                "Recorded output".to_owned(),
                theme.muted(),
            )));
            let wrapped = text::wrap(&text::sanitize(output), ctx.width.max(8));
            let shown = wrapped.len().min(OUTPUT_LINES);
            detail.extend(
                wrapped[..shown]
                    .iter()
                    .map(|piece| Line::from(Span::styled(piece.clone(), theme.body()))),
            );
            if wrapped.len() > shown {
                detail.extend(paragraph(
                    theme,
                    &format!(
                        "{} further lines were recorded and no page shows them; they stay in the session log.",
                        wrapped.len() - shown
                    ),
                    ctx.width,
                ));
            }
        }
        None => detail.extend(paragraph(
            theme,
            "No output was recorded for this run.",
            ctx.width,
        )),
    }
    Item::row(
        command.clone(),
        vec![
            Cell::text(marker, style),
            Cell::text(text::one_line(&command), theme.text()),
            Cell::text(word, style),
        ],
    )
    .with_detail(detail)
}

fn declared_check(ctx: &Ctx, command: &str, tasks: &[String]) -> Item {
    let theme = ctx.theme;
    let mut detail = field(theme, "command", command, ctx.width);
    detail.extend(field(theme, "status", "no recorded run", ctx.width));
    detail.extend(field(theme, "declared by", &tasks.join(" ; "), ctx.width));
    detail.push(Line::default());
    detail.extend(paragraph(
        theme,
        "The accepted plan declares this command for each task listed above, and the session log holds no run of it for those tasks and none recorded without a task. It may not have been reached, or a run may have ended before it could be recorded. Either way it is not a result.",
        ctx.width,
    ));
    Item::row(
        command.to_owned(),
        vec![
            Cell::text(theme.markers.idle, theme.faint()),
            Cell::text(text::one_line(command), theme.muted()),
            Cell::text("no recorded run", theme.faint()),
        ],
    )
    .with_detail(detail)
}

const WHAT_MEMBERSHIP_MEANS: &[&str] = &[
    "The pool is who may be drawn on; a session's team is who its run actually formed. Neither is assembled turn by turn by hand, and editing one does not rewrite the other.",
    "A session holds a roster of the members a turn may be given to now, and keeps every identity it ever admitted. A run may replace a member, so those are two lists: both are shown, and an identity that left the roster keeps its records.",
    "A reserved final reviewer is availability, not authority. The roster keeps one eligible agent out of production so that something other than the producer can review the result; it is not a rank and it grants nothing.",
    "What a session captured stays as captured. An agent that worked in it keeps its place in that record after it leaves the pool, and a profile edited afterwards does not change how the finished session reads.",
    "A role lasts as long as the assignment that created it. Planning, executing and reviewing are what an agent is doing in a turn, never a rank and never a standing permission.",
    "Eligibility below is about this machine: the profile is enabled, its provider is enabled, and the provider's program was found on PATH. Model lists come from the configuration, not from asking a provider. Whether an account may run a model is the installation's own business, and ymp reads no credential to build this page.",
];

const MEMBER_COLUMNS: [Column; 4] = [
    Column::left(""),
    Column::left("AGENT").flex(),
    Column::left("PROVIDER").hide(1),
    Column::left("STATE"),
];
const ASIDE_COLUMNS: [Column; 3] = [
    Column::left(""),
    Column::left("AGENT").flex(),
    Column::left("STATE"),
];
const ROSTER_COLUMNS: [Column; 3] = [
    Column::left(""),
    Column::left("RULE").flex(),
    Column::left("VALUE"),
];
const POOL_COLUMNS: [Column; 4] = [
    Column::left(""),
    Column::left("MODEL").flex(),
    Column::left("PROVIDER"),
    Column::left("STATE"),
];

fn team(ctx: &Ctx) -> Page {
    let theme = ctx.theme;
    let mut items = vec![about_row(
        ctx,
        "what membership means",
        "What membership means here",
        WHAT_MEMBERSHIP_MEANS,
    )];
    let members = ctx.team;
    // The captured identities split into the roster a turn may be given to now and the ones
    // a run replaced. Without a recorded roster there is nothing to split by, and every
    // captured identity is presented as a member, which is what such a record says.
    let (current, replaced): (Vec<&AgentProfile>, Vec<&AgentProfile>) = members
        .iter()
        .partition(|profile| ctx.records.in_roster(&profile.id) != Some(false));
    items.push(Item::table(
        if ctx.team_captured {
            "Members of this session"
        } else {
            "Members of the next run"
        },
        &MEMBER_COLUMNS,
    ));
    for profile in &current {
        items.push(member_row(ctx, profile));
    }
    if !replaced.is_empty() {
        items.push(Item::table(
            "Captured here, no longer a member",
            &ASIDE_COLUMNS,
        ));
        for profile in &replaced {
            items.push(aside_row(ctx, &profile.id, Aside::Replaced));
        }
    }
    let recorded_only: Vec<String> = ctx
        .records
        .agents()
        .into_iter()
        .filter(|id| !members.iter().any(|member| &member.id == id))
        .collect();
    if !recorded_only.is_empty() {
        items.push(Item::table("Worked here, not in this list", &ASIDE_COLUMNS));
        for id in &recorded_only {
            items.push(aside_row(ctx, id, Aside::RecordsOnly));
        }
    }
    items.push(Item::table("How the roster is bounded", &ROSTER_COLUMNS));
    items.push(roster_row(ctx));
    items.push(roster_rules_row(ctx));
    items.push(Item::table("Available on this machine", &POOL_COLUMNS));
    if ctx.pool.agents().is_empty() {
        items.push(pool_unavailable_row(ctx));
    } else {
        for agent in ctx.pool.agents() {
            // An alias that resolves to no concrete model is no choice of model. A member stays
            // listed with the members above, where its model reads as unknown.
            let member = members.iter().any(|member| member.id == agent.profile.id);
            if label::unresolved_alias(&agent.identity) && !member {
                continue;
            }
            items.push(pool_row(ctx, agent));
        }
    }
    let eligible = ctx.pool.pool.as_ref().map(|pool| pool.eligible().count());
    Page {
        view: View::Team,
        title: View::Team.title().into(),
        subtitle: match (ctx.team_captured, replaced.is_empty(), eligible) {
            (true, false, _) => format!(
                "{} in the roster · {} captured by this session",
                current.len(),
                members.len()
            ),
            (true, true, Some(eligible)) => format!(
                "{} captured by this session · {eligible} eligible on this machine",
                members.len()
            ),
            (true, true, None) => format!("{} captured by this session", members.len()),
            (false, _, Some(eligible)) => format!(
                "{} for the next run · {eligible} eligible on this machine",
                members.len()
            ),
            (false, _, None) => format!("{} for the next run", members.len()),
        },
        items,
        empty: nothing(
            theme,
            "No profile is eligible",
            "A run needs at least two profiles, so that no agent accepts its own work. Add one with /agent add ID PROVIDER, then /team add ID.",
            ctx.width,
        ),
        hints: vec![
            ("Space", "next-session team"),
            ("Enter", "inspect"),
            ("Esc", "back"),
        ],
    }
}

/// The models this agent's recorded turns actually ran with, exactly as recorded.
///
/// This is not the catalog and not the configuration: it is what each invocation reported, or
/// failing that what was sent to the installation. A turn whose model nothing recorded is
/// counted as such rather than filled in from the profile, because a spent turn may have run
/// under a different model than the one configured now.
fn recorded_models(ctx: &Ctx, agent: &str) -> (Vec<String>, usize) {
    let records = ctx.records;
    let mut names: Vec<String> = Vec::new();
    let mut unrecorded = 0usize;
    for assignment in records
        .assignments()
        .iter()
        .filter(|assignment| assignment.agent_id == agent)
    {
        for invocation in records.invocations_of(&assignment.id) {
            let named = invocation.reported.model.is_some() || invocation.sent.model.is_some();
            // The internal default alias is resolved as the turn's own identity resolves it, or
            // it names no model.
            match named
                .then(|| label::assignment_model(assignment, Some(invocation)))
                .flatten()
            {
                Some(model) => {
                    if !names.contains(&model) {
                        names.push(model);
                    }
                }
                None => unrecorded += 1,
            }
        }
    }
    (names, unrecorded)
}

/// The same read, in words.
fn models_used(ctx: &Ctx, agent: &str) -> String {
    let (names, unrecorded) = recorded_models(ctx, agent);
    match (names.is_empty(), unrecorded) {
        (true, 0) => "no turn of this agent was recorded".to_owned(),
        (true, turns) => format!("unknown · {turns} recorded turn(s) named no model"),
        (false, 0) => names.join(", "),
        (false, turns) => format!(
            "{} · {turns} further recorded turn(s) named no model",
            names.join(", ")
        ),
    }
}

/// What a member of a session that is already over ran as.
///
/// A catalog read today says nothing about a turn that ran yesterday, so a captured member is
/// never resolved against the present catalog: the answer comes from the profile the session
/// captured, or from the turns the session itself recorded, or it stays missing.
/// What is stored for a provider: a native scan, a catalog the configuration wrote, or nothing,
/// with the last attempt and any bounded failure the scan recorded.
fn catalog_words(ctx: &Ctx, provider: &str) -> String {
    let snapshot = ctx.config.native_provider_snapshot(provider);
    let source = ctx
        .config
        .provider_capabilities(provider)
        .map(|catalog| catalog.source.clone());
    match (snapshot, source) {
        (
            Some(snapshot),
            Some(CapabilitySource::NativeMetadata {
                method,
                observed_at,
            }),
        ) => match snapshot.failure.as_deref() {
            None => format!("native scan · {method} · {observed_at}"),
            Some(failure) => format!(
                "last attempt {} failed: {failure} · keeping native scan · {method} · {observed_at}",
                snapshot.last_attempt
            ),
        },
        (Some(snapshot), _) => match snapshot.failure.as_deref() {
            Some(failure) => format!(
                "not scanned · last attempt {} failed: {failure}",
                snapshot.last_attempt
            ),
            None => format!("not scanned · attempt {} stored nothing", snapshot.last_attempt),
        },
        (None, Some(CapabilitySource::Configured)) => "configured · not scanned".to_owned(),
        (None, Some(CapabilitySource::NativeMetadata { .. })) => {
            "native claim · no stored scan matches".to_owned()
        }
        (None, None) => "not scanned".to_owned(),
    }
}

/// What the catalog lists, in the installation's own names, and whether it claims to be whole.
fn catalog_models_words(catalog: Option<&ProviderCapabilities>) -> String {
    match catalog {
        None => "none listed".to_owned(),
        Some(catalog) if catalog.models.is_empty() && catalog.models_complete => {
            "none · complete list".to_owned()
        }
        Some(catalog) if catalog.models.is_empty() => "none listed".to_owned(),
        Some(catalog) => format!(
            "{} · {}",
            catalog
                .models
                .iter()
                .map(|model| match model.display_name.as_deref() {
                    Some(name) if name != model.id => format!("{name} ({})", model.id),
                    _ => model.id.clone(),
                })
                .collect::<Vec<_>>()
                .join(", "),
            if catalog.models_complete {
                "complete list"
            } else {
                "completeness not reported"
            }
        ),
    }
}

/// The identity recorded with this agent's last assignment, if one was recorded.
fn captured_identity<'a>(ctx: &'a Ctx, agent: &str) -> Option<&'a AgentIdentity> {
    captured_identity_of(ctx.records, agent)
}

fn captured_identity_of<'a>(records: &'a Records, agent: &str) -> Option<&'a AgentIdentity> {
    records
        .assignments()
        .iter()
        .filter(|assignment| assignment.agent_id == agent)
        .next_back()
        .and_then(|assignment| assignment.agent_identity.as_ref())
}

/// What a captured member was, from the records alone.
fn captured_identity_lines(ctx: &Ctx, profile: &AgentProfile) -> Vec<Line<'static>> {
    let theme = ctx.theme;
    let mut lines = Vec::new();
    if let Some(identity) = captured_identity(ctx, &profile.id) {
        lines.extend(field(theme, "native label", &identity.name, ctx.width));
        if identity.configured_name != identity.name {
            lines.extend(field(
                theme,
                "configured as",
                &identity.configured_name,
                ctx.width,
            ));
        }
        lines.extend(field(
            theme,
            "captured as",
            &format!(
                "{} · the identity recorded when the turn was admitted",
                asked_words(identity.model.as_deref())
            ),
            ctx.width,
        ));
    }
    lines.extend(field(
        theme,
        "model",
        &captured_model_detail_words(ctx, profile),
        ctx.width,
    ));
    lines
}

fn captured_model_row_words(ctx: &Ctx, profile: &AgentProfile) -> String {
    if let Some(id) = captured_identity(ctx, &profile.id).and_then(label::resolved_model) {
        return label::clean(id);
    }
    // What the turns recorded comes before the profile's own field: a model can reach a turn
    // from an execution policy without ever being written on the profile, so an empty field is
    // not a statement that no model was used.
    if let Some(id) = recorded_models(ctx, &profile.id).0.first() {
        return id.clone();
    }
    profile
        .model
        .clone()
        .unwrap_or_else(|| "model not recorded".to_owned())
}

fn captured_model_detail_words(ctx: &Ctx, profile: &AgentProfile) -> String {
    match profile.model.as_deref() {
        Some(id) => format!("{id} · the profile this session captured pins it"),
        None => format!(
            "{} · read from the turns this session recorded, not from the catalog as it stands now",
            models_used(ctx, &profile.id)
        ),
    }
}

/// The native identity of a profile, as the pool snapshot holds it.
///
/// Painting reads the snapshot and never scans: `Pool::read` rebuilt it when the window opened,
/// a session was loaded or the pages were opened, and an explicit scan is its own action.
fn identity_of<'a>(ctx: &'a Ctx, profile: &AgentProfile) -> Option<&'a AgentIdentity> {
    ctx.pool.agent(&profile.id).map(|agent| &agent.identity)
}

/// The cell that names an agent in a row.
///
/// A provider id is a transport label: `codex` is how ymp reaches an installation, not a model it
/// offers. A caption such as `Default (recommended)` describes an offering without naming its
/// model. So the label is the concrete model identifier native metadata resolves the agent to,
/// and where nothing resolves one the label says the model is unknown. A local fixture has no
/// native model and keeps its configured name. A short qualifier follows only where the identity
/// needs attention; where a name came from, and when, is read with Inspect.
fn identity_cell(
    theme: &Theme,
    identity: Option<&AgentIdentity>,
    profile: &AgentProfile,
    config: &Config,
) -> Cell {
    let (label, qualifier) = match identity {
        None => (label::profile(profile, config), Some("unavailable")),
        Some(identity) => (
            label::offering(identity).unwrap_or_else(|| label::UNKNOWN_MODEL.to_owned()),
            match identity.status {
                AgentIdentityStatus::Stale => Some("stale"),
                AgentIdentityStatus::Unknown => Some("not in catalog"),
                AgentIdentityStatus::Native
                | AgentIdentityStatus::Unresolved
                | AgentIdentityStatus::Local => None,
            },
        ),
    };
    let mut spans = vec![Span::styled(label, theme.text())];
    if let Some(qualifier) = qualifier {
        spans.push(Span::styled(format!(" · {qualifier}"), theme.warn()));
    }
    Cell::spans(spans)
}

/// The model an identity stands for, in words, where the row gives it as a label.
fn identity_model_words(identity: &AgentIdentity) -> String {
    match label::offering(identity) {
        Some(model) => model,
        None if identity.model.is_some() => {
            "unknown · native metadata resolves what it asks for to no concrete model".to_owned()
        }
        None => "unknown · none is set, so the installation would choose".to_owned(),
    }
}

/// What the settings ask the installation for, with the internal default alias said as such.
fn asked_words(model: Option<&str>) -> String {
    match model {
        Some(id) if label::is_default_alias(id) => "the installation's default alias".to_owned(),
        Some(id) => id.to_owned(),
        None => "no model; the installation would choose".to_owned(),
    }
}

/// The same identity in full: what it is, where the name came from, and what it does not say.
fn identity_lines(ctx: &Ctx, profile: &AgentProfile) -> Vec<Line<'static>> {
    let theme = ctx.theme;
    let identity = identity_of(ctx, profile);
    let Some(identity) = identity else {
        return field(
            theme,
            "model",
            "unknown · what is installed on this machine could not be read, so this says nothing about the model",
            ctx.width,
        );
    };
    let mut lines = field(theme, "model", &identity_model_words(identity), ctx.width);
    // A caption stays source metadata beside the model, never the name.
    if identity.status != AgentIdentityStatus::Local {
        lines.extend(field(theme, "native label", &identity.name, ctx.width));
    }
    if identity.configured_name != identity.name {
        lines.extend(field(
            theme,
            "configured as",
            &identity.configured_name,
            ctx.width,
        ));
    }
    lines.extend(field(
        theme,
        "asks for",
        &asked_words(identity.model.as_deref()),
        ctx.width,
    ));
    if let Some(resolved) = identity.resolved_model.as_deref() {
        lines.extend(field(theme, "resolved to", resolved, ctx.width));
    }
    lines.extend(field(theme, "source", &name_source(identity), ctx.width));
    let catalog = ctx.config.provider_capabilities(&profile.provider);
    if let Some(offering) = catalog
        .zip(identity.model.as_deref())
        .and_then(|(catalog, model)| catalog.model(model))
    {
        if !offering.aliases.is_empty() {
            lines.extend(field(
                theme,
                "also known as",
                &offering.aliases.join(", "),
                ctx.width,
            ));
        }
        if let Some(picker) = offering.picker_id.as_deref() {
            lines.extend(field(
                theme,
                "chosen in the picker as",
                &format!("{picker} · a selector, not a model to send"),
                ctx.width,
            ));
        }
        lines.extend(control_lines(theme, offering, ctx.width));
    }
    lines
}

/// Where a name came from, which is the only thing that makes it a native name.
fn name_source(identity: &AgentIdentity) -> String {
    match (&identity.status, &identity.source) {
        (
            AgentIdentityStatus::Native,
            Some(CapabilitySource::NativeMetadata {
                method,
                observed_at,
            }),
        ) => format!("native scan · {method} · {observed_at}"),
        (
            AgentIdentityStatus::Stale,
            Some(CapabilitySource::NativeMetadata {
                method,
                observed_at,
            }),
        ) => format!("native scan · {method} · {observed_at} · stale"),
        (AgentIdentityStatus::Native | AgentIdentityStatus::Stale, _) => {
            "stored scan · method and time not recorded".to_owned()
        }
        (AgentIdentityStatus::Unknown, _) => {
            "configuration · no stored catalog lists this model".to_owned()
        }
        (AgentIdentityStatus::Unresolved, _) => {
            "not resolved · no model is set and no stored catalog reports a default".to_owned()
        }
        (AgentIdentityStatus::Local, _) => {
            "configuration · local provider, no native identity".to_owned()
        }
    }
}

/// The controls an offering advertises, in the installation's own words.
fn control_lines(theme: &Theme, offering: &ModelCapabilities, width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    match offering.controls.as_deref() {
        None => lines.extend(field(theme, "controls", "not reported", width)),
        Some([]) => lines.extend(field(theme, "controls", "none", width)),
        Some(controls) => {
            for control in controls {
                let label = control.display_name.as_deref().unwrap_or(&control.id);
                lines.extend(field(theme, label, &control_values(control), width));
            }
        }
    }
    lines
}

/// The values one control takes, named as the installation names them.
fn control_values(control: &NativeControl) -> String {
    let values = match &control.values {
        NativeControlValues::Choices { options } => options
            .iter()
            .map(|option| match control.value_names.get(option) {
                Some(name) if name != option => format!("{name} ({option})"),
                _ => option.clone(),
            })
            .collect::<Vec<_>>()
            .join(", "),
        NativeControlValues::Boolean => "on or off".to_owned(),
        NativeControlValues::Integer { min, max } => format!("{min} to {max}"),
    };
    match &control.default {
        Some(NativeControlValue::Choice(value)) => format!("{values} · {value} by default"),
        Some(NativeControlValue::Boolean(value)) => format!(
            "{values} · {} by default",
            if *value { "on" } else { "off" }
        ),
        Some(NativeControlValue::Integer(value)) => format!("{values} · {value} by default"),
        None => format!("{values} · default not reported"),
    }
}

/// One member of the team a session captured, or of the team the next run would use.
fn member_row(ctx: &Ctx, profile: &AgentProfile) -> Item {
    let theme = ctx.theme;
    let records = ctx.records;
    let turns = records
        .assignments()
        .iter()
        .filter(|assignment| assignment.agent_id == profile.id)
        .count();
    let open = records
        .open()
        .into_iter()
        .filter(|assignment| assignment.agent_id == profile.id)
        .count();
    let eligible = ctx.pool.agent(&profile.id);
    let (right, style) = if open > 0 && ctx.live {
        ("running now".to_owned(), theme.warn())
    } else if open > 0 {
        ("a turn was left open".to_owned(), theme.info())
    } else if turns > 0 {
        (
            format!("{turns} {} here", if turns == 1 { "turn" } else { "turns" }),
            theme.good(),
        )
    } else if ctx.team_captured {
        ("no turn recorded".to_owned(), theme.muted())
    } else {
        match eligible {
            Some(agent) if agent.exclusions.is_empty() => ("eligible".to_owned(), theme.good()),
            Some(agent) => (exclusion_word(agent.exclusions[0]).to_owned(), theme.warn()),
            None => ("not in the pool".to_owned(), theme.warn()),
        }
    };
    let mut detail = field(theme, "profile", &profile.id, ctx.width);
    // A catalog read today says nothing about a turn that ran yesterday, so a captured member
    // is named from what its own session recorded and never from the catalog as it stands now.
    let name = if ctx.team_captured {
        detail.extend(captured_identity_lines(ctx, profile));
        // The model the session recorded follows the name only where the name does not already
        // carry it, so an unknown label still shows what its turns ran with, or that none said.
        let name = presented_name(ctx, &profile.id);
        let model = captured_model_row_words(ctx, profile);
        let mut spans = vec![Span::styled(name.clone(), theme.text())];
        if !name.contains(&model) {
            spans.push(Span::styled(format!(" · {model}"), theme.muted()));
        }
        Cell::spans(spans)
    } else {
        detail.extend(identity_lines(ctx, profile));
        identity_cell(theme, identity_of(ctx, profile), profile, ctx.config)
    };
    detail.extend(field(theme, "provider", &profile.provider, ctx.width));
    detail.extend(field(
        theme,
        "membership",
        &membership_words(ctx),
        ctx.width,
    ));
    detail.extend(field(
        theme,
        "turns recorded here",
        &turns.to_string(),
        ctx.width,
    ));
    detail.extend(field(
        theme,
        "responsible for",
        &match ctx.records.commitment_of(&profile.id) {
            Some((entry, commitment)) => format!(
                "{} · {} · committed by proposal {}",
                entry.task.title,
                settings_words(&commitment.settings),
                text::short_id(&commitment.proposal_id)
            ),
            None => "no task on the plan is committed to this member".to_owned(),
        },
        ctx.width,
    ));
    let (source, fixed) = pinned(ctx, &profile.id);
    detail.extend(field(theme, "fixed", &pin_words(&fixed, source), ctx.width));
    detail.extend(field(
        theme,
        "on this machine",
        &eligibility_words(eligible),
        ctx.width,
    ));
    detail.push(Line::default());
    detail.push(Line::from(Span::styled(
        "Permissions in force now".to_owned(),
        theme.muted(),
    )));
    let live = records.live_grants(&profile.id);
    if live.is_empty() {
        detail.extend(paragraph(
            theme,
            "None. A coordination permission exists only while the assignment that holds it is running.",
            ctx.width,
        ));
    } else {
        for grant in &live {
            detail.extend(field(theme, "granted", &grant_detail(grant), ctx.width));
        }
    }
    detail.push(Line::default());
    detail.extend(paragraph(
        theme,
        if profile.instructions.is_empty() {
            "No additional instructions."
        } else {
            &profile.instructions
        },
        ctx.width,
    ));
    Item::row(
        profile.id.clone(),
        vec![
            Cell::text(
                if open > 0 {
                    theme.markers.busy
                } else if turns > 0 {
                    theme.markers.ok
                } else {
                    theme.markers.idle
                },
                style,
            ),
            name,
            Cell::text(profile.provider.clone(), theme.faint()),
            Cell::text(right, style),
        ],
    )
    .with_detail(detail)
}

/// How a member came to be in the list above, as the records have it.
fn membership_words(ctx: &Ctx) -> String {
    match (ctx.team_captured, ctx.records.roster()) {
        (true, Some(state)) => format!(
            "captured by this session and in the roster it holds now, revision {}",
            state.revision
        ),
        (true, None) => "captured by this session, which recorded no roster of its own".into(),
        (false, _) => "the configuration as it stands now".into(),
    }
}

/// Why an agent is listed apart from the members.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Aside {
    /// This session captured it as a member and the roster it holds now does not list it.
    Replaced,
    /// Assignment records name it and no captured membership does.
    RecordsOnly,
}

/// An agent with records here that the list above does not present as a member.
///
/// Both cases state the same thing about the work: a membership that ended does not remove
/// the turns that were recorded under it, and nothing here re-reads or re-grades them.
fn aside_row(ctx: &Ctx, id: &str, aside: Aside) -> Item {
    let theme = ctx.theme;
    let turns = ctx
        .records
        .assignments()
        .iter()
        .filter(|assignment| assignment.agent_id == id)
        .count();
    let mut detail = field(theme, "profile", id, ctx.width);
    detail.extend(field(
        theme,
        "turns recorded here",
        &turns.to_string(),
        ctx.width,
    ));
    detail.extend(field(
        theme,
        "on this machine",
        &eligibility_words(ctx.pool.agent(id)),
        ctx.width,
    ));
    detail.push(Line::default());
    detail.extend(paragraph(
        theme,
        &match aside {
            Aside::Replaced => format!(
                "This session captured this agent as a member, and the roster it holds now, revision {}, does not list it, so no further turn would be given to it here. The records stay as written, because membership that ended does not remove the work it did.",
                ctx.records.roster().map_or(0, |state| state.revision)
            ),
            Aside::RecordsOnly => "This agent has assignment records in this session and is not in the list above: either it is no longer a member, or the list does not name it. The records stay as written either way, because membership that ended does not remove the work it did.".to_owned(),
        },
        ctx.width,
    ));
    let (right, style) = match aside {
        Aside::Replaced => ("no longer a member".to_owned(), theme.muted()),
        Aside::RecordsOnly => (
            format!(
                "{turns} {} recorded here",
                if turns == 1 { "turn" } else { "turns" }
            ),
            theme.info(),
        ),
    };
    Item::row(
        id.to_owned(),
        vec![
            Cell::text(
                match aside {
                    Aside::Replaced => theme.markers.notice,
                    Aside::RecordsOnly => theme.markers.activity,
                },
                style,
            ),
            Cell::text(presented_name(ctx, id), theme.text()),
            Cell::text(right, style),
        ],
    )
    .with_detail(detail)
}

/// The roster record itself: which revision the session holds, and what it reserves.
fn roster_row(ctx: &Ctx) -> Item {
    let theme = ctx.theme;
    let Some(state) = ctx.records.roster() else {
        let mut detail = paragraph(
            theme,
            match ctx.session {
                None => "No session is loaded, so there is no roster to read. The list above is the team the next run would form from the configuration as it stands now.",
                Some(_) => "This session recorded no roster of its own, so every identity it captured is presented as a member. A run that resumes it would form one and record it.",
            },
            ctx.width,
        );
        detail.push(Line::default());
        detail.extend(paragraph(
            theme,
            "A roster is a record written when a run admits or replaces a member. Its absence is the absence of that record, not an empty team.",
            ctx.width,
        ));
        return Item::row(
            "roster".to_owned(),
            vec![
                Cell::text(theme.markers.idle, theme.faint()),
                Cell::text("roster", theme.muted()),
                Cell::text("none recorded", theme.faint()),
            ],
        )
        .with_detail(detail);
    };
    let mut detail = field(
        theme,
        "members now",
        &if state.current_members.is_empty() {
            "none".to_owned()
        } else {
            state
                .current_members
                .iter()
                .map(|id| presented_name(ctx, id))
                .collect::<Vec<_>>()
                .join(", ")
        },
        ctx.width,
    );
    detail.extend(field(
        theme,
        "revision",
        &format!("{} · last changed {}", state.revision, state.updated_at),
        ctx.width,
    ));
    detail.extend(field(
        theme,
        "final reviewer kept free",
        &match &state.reserved_final_reviewer {
            Some(id) => presented_name(ctx, id),
            None => "none reserved".to_owned(),
        },
        ctx.width,
    ));
    detail.extend(field(
        theme,
        "eligible when written",
        &format!("{} profile(s)", state.eligible_agents.len()),
        ctx.width,
    ));
    detail.extend(field(theme, "method", &state.method, ctx.width));
    detail.push(Line::default());
    detail.extend(paragraph(
        theme,
        "The roster is who a turn may be given to now. The reserved reviewer is held out of production so the result can be reviewed by something other than its producer; it is availability and not authority, and it carries no permission of its own.",
        ctx.width,
    ));
    detail.push(Line::default());
    detail.extend(paragraph(
        theme,
        "Eligibility was observed when this revision was written. What is installed on this machine now is listed further down, read at a different moment.",
        ctx.width,
    ));
    Item::row(
        "roster".to_owned(),
        vec![
            Cell::text(theme.markers.ok, theme.good()),
            Cell::text("roster", theme.text()),
            value_cell(
                format!(
                    "{} {} · revision {}",
                    state.current_members.len(),
                    if state.current_members.len() == 1 {
                        "member"
                    } else {
                        "members"
                    },
                    state.revision
                ),
                theme.muted(),
            ),
        ],
    )
    .with_detail(detail)
}

/// The bounds a roster is formed under: captured by the session, or configured for the next.
fn roster_rules_row(ctx: &Ctx) -> Item {
    let theme = ctx.theme;
    let captured = ctx.records.captured_constraints();
    let rules = captured.unwrap_or(&ctx.config.team_constraints);
    let size = match (rules.fixed_size, &rules.fixed_roster) {
        (Some(size), _) => format!("exactly {size}"),
        (None, Some(roster)) => format!("exactly the {} named", roster.len()),
        (None, None) => format!("up to {}", rules.max_members),
    };
    let mut detail = field(theme, "members allowed", &size, ctx.width);
    detail.extend(field(
        theme,
        "named roster",
        &match &rules.fixed_roster {
            Some(roster) => roster.join(", "),
            None => "none; the run forms one from the eligible profiles".to_owned(),
        },
        ctx.width,
    ));
    detail.extend(field(
        theme,
        "restricted to",
        &match &rules.eligible_agents {
            Some(ids) => ids.join(", "),
            None => "no restriction beyond what is eligible here".to_owned(),
        },
        ctx.width,
    ));
    detail.extend(field(
        theme,
        "ceiling",
        &format!("{} member(s)", rules.max_members),
        ctx.width,
    ));
    detail.extend(field(
        theme,
        "read from",
        match (ctx.session, captured) {
            (Some(_), Some(_)) => "the bounds this session captured when it started",
            (Some(_), None) => "the configuration now; this session captured no bounds",
            (None, _) => "the configuration as it stands now",
        },
        ctx.width,
    ));
    detail.push(Line::default());
    detail.extend(paragraph(
        theme,
        match captured {
            Some(_) => "A session keeps the bounds it captured. Editing the configuration changes what the next run may form, and not the roster recorded here.",
            None => "These bounds come from the configuration, which can still be edited. A session captures them when it starts, and a session that captured none would be resumed under whatever they are then.",
        },
        ctx.width,
    ));
    Item::row(
        "roster rules".to_owned(),
        vec![
            Cell::text(theme.markers.bullet, theme.faint()),
            Cell::text("roster rules", theme.text()),
            value_cell(size, theme.muted()),
        ],
    )
    .with_detail(detail)
}

/// One profile in the pool this machine could draw on.
fn pool_row(ctx: &Ctx, agent: &PoolAgent) -> Item {
    let theme = ctx.theme;
    let eligible = agent.exclusions.is_empty();
    let mut detail = field(theme, "profile", &agent.profile.id, ctx.width);
    detail.extend(field(theme, "provider", &agent.profile.provider, ctx.width));
    detail.extend(field(theme, "version", &agent.profile_version, ctx.width));
    detail.extend(field(theme, "model", model_status_words(agent), ctx.width));
    if eligible {
        detail.extend(field(theme, "eligible", "yes, on this machine", ctx.width));
    } else {
        for exclusion in &agent.exclusions {
            detail.extend(field(
                theme,
                "excluded",
                exclusion_word(*exclusion),
                ctx.width,
            ));
        }
    }
    detail.push(Line::default());
    detail.extend(paragraph(
        theme,
        "Eligibility says the program is installed and enabled here. It says nothing about authentication, quota or whether a model will accept the turn: only the installation can answer that, and it is asked when a turn runs.",
        ctx.width,
    ));
    let style = if eligible { theme.good() } else { theme.warn() };
    Item::row(
        agent.profile.id.clone(),
        vec![
            Cell::text(
                if eligible {
                    theme.markers.ok
                } else {
                    theme.markers.warn
                },
                style,
            ),
            Cell::text(
                label::offering(&agent.identity).unwrap_or_else(|| label::UNKNOWN_MODEL.to_owned()),
                theme.muted(),
            ),
            Cell::text(agent.profile.provider.clone(), theme.faint()),
            Cell::text(
                if eligible {
                    "eligible"
                } else {
                    exclusion_word(agent.exclusions[0])
                },
                style,
            ),
        ],
    )
    .with_detail(detail)
}

fn pool_unavailable_row(ctx: &Ctx) -> Item {
    let theme = ctx.theme;
    let mut detail = Vec::new();
    match &ctx.pool.unreadable {
        Some(error) => {
            detail.extend(paragraph(
                theme,
                "The pool could not be inspected, which is not the same as a machine with nothing installed.",
                ctx.width,
            ));
            detail.push(Line::default());
            detail.extend(paragraph(theme, error, ctx.width));
        }
        None => detail.extend(paragraph(
            theme,
            "No profile is configured on this machine. Add one with /agent add ID PROVIDER.",
            ctx.width,
        )),
    }
    Item::row(
        "pool",
        vec![
            Cell::text(theme.markers.warn, theme.warn()),
            Cell::text("Nothing was inspected", theme.text()),
            Cell::empty(),
            Cell::text("no pool read", theme.muted()),
        ],
    )
    .with_detail(detail)
}

fn eligibility_words(agent: Option<&PoolAgent>) -> String {
    match agent {
        None => "this profile is not in the pool on this machine".into(),
        Some(agent) if agent.exclusions.is_empty() => "eligible".into(),
        Some(agent) => agent
            .exclusions
            .iter()
            .map(|exclusion| exclusion_sentence(*exclusion))
            .collect::<Vec<_>>()
            .join("; "),
    }
}

/// The same reason with room to state what it means for a turn.
fn exclusion_sentence(exclusion: PoolExclusion) -> &'static str {
    match exclusion {
        PoolExclusion::AgentDisabled => "the profile is disabled, so no turn is given to it",
        PoolExclusion::ProviderDisabled => "its provider is disabled, so it cannot be reached",
        PoolExclusion::ExecutableMissing => "the provider's program was not found on PATH",
        PoolExclusion::ModelUnlisted => {
            "the catalog read for its provider does not list the model it asks for"
        }
        PoolExclusion::NoModelsAvailable => "the catalog for its provider lists no model at all",
        PoolExclusion::NativeModelUnresolved => {
            "no native model is resolved for it, so nothing would be sent"
        }
    }
}

/// Why the pool refuses a profile, short enough for the right-hand side of a row at the
/// narrowest supported width. The sentence behind it belongs in the record.
fn exclusion_word(exclusion: PoolExclusion) -> &'static str {
    match exclusion {
        PoolExclusion::AgentDisabled => "profile disabled",
        PoolExclusion::ProviderDisabled => "provider disabled",
        PoolExclusion::ExecutableMissing => "executable not found",
        PoolExclusion::ModelUnlisted => "model not in catalog",
        PoolExclusion::NoModelsAvailable => "no models in catalog",
        PoolExclusion::NativeModelUnresolved => "no native model",
    }
}

fn model_status_words(agent: &PoolAgent) -> &'static str {
    match agent.model_status {
        PoolModelStatus::InheritedDefault => "none set; the installation chooses",
        PoolModelStatus::Listed => "listed in the configured catalog",
        PoolModelStatus::Unlisted => "not in the configured catalog",
        PoolModelStatus::Unknown => "the configured catalog does not say",
    }
}

/// Rows under one untitled table, or nothing when there are no rows, so an empty page shows its
/// empty state rather than column titles over nothing.
fn with_table(columns: &[Column], rows: Vec<Item>) -> Vec<Item> {
    if rows.is_empty() {
        return rows;
    }
    let mut items = vec![Item::table("", columns)];
    items.extend(rows);
    items
}

/// TEAM here is the next-session preference in `Config.team`, not the membership of a loaded
/// session, so the column names its scope.
const AGENT_COLUMNS: [Column; 4] = [
    Column::left("AGENT").flex(),
    Column::left("PROVIDER"),
    Column::left("ENABLED"),
    Column::left("NEXT TEAM"),
];
const PROVIDER_COLUMNS: [Column; 4] = [
    Column::left("PROVIDER"),
    Column::left("COMMAND"),
    Column::left("EXECUTABLE").flex(),
    Column::left("ENABLED"),
];
const MEMORY_COLUMNS: [Column; 3] = [
    Column::left("TITLE").flex(),
    Column::left("STATE"),
    Column::left("SCOPE"),
];
const REPUTATION_COLUMNS: [Column; 4] = [
    Column::left("AGENT"),
    Column::left("COMPETENCE").flex(),
    Column::left("OUTCOME"),
    Column::left("EVIDENCE"),
];

fn agents(ctx: &Ctx) -> Page {
    let theme = ctx.theme;
    let items = ctx
        .config
        .agents
        .iter()
        .map(|profile| {
            let in_team = ctx.config.team.contains(&profile.id);
            let mut detail = field(theme, "profile", &profile.id, ctx.width);
            detail.extend(identity_lines(ctx, profile));
            detail.extend(field(theme, "provider", &profile.provider, ctx.width));
            detail.extend(field(
                theme,
                "catalog",
                &catalog_words(ctx, &profile.provider),
                ctx.width,
            ));
            detail.extend(field(
                theme,
                "enabled",
                if profile.enabled { "on" } else { "off" },
                ctx.width,
            ));
            detail.extend(field(
                theme,
                "next team",
                if in_team {
                    "true · next-session preference"
                } else {
                    "false · next-session preference"
                },
                ctx.width,
            ));
            detail.push(Line::default());
            detail.push(Line::from(Span::styled(
                "Instructions".to_owned(),
                theme.muted(),
            )));
            detail.extend(paragraph(
                theme,
                if profile.instructions.is_empty() {
                    "None. Press i to edit."
                } else {
                    &profile.instructions
                },
                ctx.width,
            ));
            detail.push(Line::default());
            detail.push(Line::from(Span::styled(
                "Experience is recorded with the profile configuration in use, including its model and instructions. Sessions already running keep the profile they captured.".to_owned(),
                theme.faint(),
            )));
            Item::row(
                profile.id.clone(),
                vec![
                    identity_cell(theme, identity_of(ctx, profile), profile, ctx.config),
                    Cell::text(profile.provider.clone(), theme.faint()),
                    Cell::spans(vec![yes_no(profile.enabled, theme)]),
                    if in_team {
                        Cell::text("true", theme.accent())
                    } else {
                        Cell::text("false", theme.faint())
                    },
                ],
            )
            .with_detail(detail)
        })
        .collect::<Vec<_>>();
    let items = with_table(&AGENT_COLUMNS, items);
    Page {
        view: View::Agents,
        title: View::Agents.title().into(),
        subtitle: format!(
            "{} {}",
            ctx.config.agents.len(),
            if ctx.config.agents.len() == 1 {
                "profile"
            } else {
                "profiles"
            }
        ),
        items,
        empty: nothing(
            theme,
            "No agent profiles",
            "Create one with /agent add ID PROVIDER.",
            ctx.width,
        ),
        hints: vec![
            ("m", "model"),
            ("i", "instructions"),
            ("Space", "enable or disable"),
            ("t", "next-session team"),
            ("r", "reload"),
            ("R", "scan catalogs"),
            ("Esc", "back"),
        ],
    }
}

fn providers(ctx: &Ctx) -> Page {
    let theme = ctx.theme;
    let items =
        ctx.config
            .providers
            .iter()
            .map(|provider| {
                let health = ctx.pool.provider(&provider.id);
                let found = health.is_some_and(|health| health.executable.is_some());
                let mut detail = field(theme, "provider", &provider.id, ctx.width);
                detail.extend(field(
                    theme,
                    "kind",
                    &format!("{:?}", provider.kind),
                    ctx.width,
                ));
                detail.extend(field(
                    theme,
                    "command",
                    &format!("{} {}", provider.command, provider.args.join(" ")),
                    ctx.width,
                ));
                detail.extend(field(
                    theme,
                    "executable",
                    &match health {
                        Some(health) => match &health.executable {
                            Some(path) => path.display().to_string(),
                            None if health.available => {
                                "none; this provider runs inside ymp".to_owned()
                            }
                            None => "not found on PATH".to_owned(),
                        },
                        None => "not inspected".to_owned(),
                    },
                    ctx.width,
                ));
                detail.extend(field(
                    theme,
                    "inspected",
                    if ctx.pool.read_at.is_empty() {
                        "not yet"
                    } else {
                        &ctx.pool.read_at
                    },
                    ctx.width,
                ));
                let catalog = ctx.config.provider_capabilities(&provider.id);
                detail.extend(field(
                theme,
                "catalog",
                &catalog_words(ctx, &provider.id),
                ctx.width,
            ));
                detail.extend(field(
                    theme,
                    "offers",
                    &catalog_models_words(catalog),
                    ctx.width,
                ));
                detail.extend(field(
                    theme,
                    "default",
                    &match catalog.and_then(|catalog| catalog.default_model.as_deref()) {
                        Some(id) => id.to_owned(),
                        None => "not reported".to_owned(),
                    },
                    ctx.width,
                ));
                if !provider.env_refs.is_empty() {
                    detail.extend(field(
                        theme,
                        "environment",
                        &provider
                            .env_refs
                            .iter()
                            .map(|(k, v)| format!("{k}={v}"))
                            .collect::<Vec<_>>()
                            .join(", "),
                        ctx.width,
                    ));
                }
                detail.push(Line::default());
                detail.push(Line::from(Span::styled(
                    "The provider manages its own credentials. ymp never reads or stores a token."
                        .to_owned(),
                    theme.faint(),
                )));
                detail.extend(paragraph(
                    theme,
                    "Opening this page reads the catalog that is already stored. It never asks the installation what it offers, so a name that is not here has not been read yet rather than being unavailable.",
                    ctx.width,
                ));
                Item::row(
                    provider.id.clone(),
                    vec![
                        Cell::text(provider.id.clone(), theme.text()),
                        Cell::text(provider.command.clone(), theme.faint()),
                        Cell::text(
                            match (found, health.is_some_and(|health| health.available)) {
                                (true, _) => format!("{} on PATH", theme.markers.ok),
                                (false, true) => format!("{} in process", theme.markers.ok),
                                (false, false) => format!("{} not found", theme.markers.warn),
                            },
                            if found || health.is_some_and(|health| health.available) {
                                theme.good()
                            } else {
                                theme.warn()
                            },
                        ),
                        Cell::spans(vec![yes_no(provider.enabled, theme)]),
                    ],
                )
                .with_detail(detail)
            })
            .collect::<Vec<_>>();
    let items = with_table(&PROVIDER_COLUMNS, items);
    Page {
        view: View::Providers,
        title: View::Providers.title().into(),
        subtitle: "Local programs that run turns".into(),
        items,
        empty: nothing(
            theme,
            "No providers configured",
            "Edit the file printed by `ymp config --path` to add one.",
            ctx.width,
        ),
        hints: vec![
            ("Space", "enable or disable"),
            ("r", "reload"),
            ("R", "scan catalog"),
            ("Esc", "back"),
        ],
    }
}

/// What an entry here does and does not establish. Knowledge is not one status: an entry a
/// reviewer accepted and an entry nobody reviewed are both kept, and the page says which is
/// which rather than calling all of it verified.
const MEMORY_BASIS: &str = "An entry is either a projection of a result this project \
     accepted, or a candidate a run proposed. Confirmed means the acceptance it names carried \
     passing checks; unconfirmed means nobody's evidence is attached to it, and unknown means \
     the entry was written before provenance was recorded at all. The text of an entry is not \
     evidence for itself, whatever it claims.";

const WHAT_RETAINED_KNOWLEDGE_IS: &[&str] = &[
    "Every entry this project retained is listed, with the standing the store gives it now. Current means the default retrieval of a run accepts it under the conditions in force here. Everything else is readable and is not offered to a run.",
    "A correction does not delete what it corrects. An accepted correction marks the old entry superseded, names the replacement on it, and both stay here with their own evidence, so a reader can see what changed and on what basis.",
    "A correction is only accepted where a trusted contract named the criteria for it and the evidence for those criteria passed. The contract, the acceptance and the policy that applied it are all named on the entry.",
];

const MEMORY_SUPPORT: &str = "Only supported entries are given to a run as context. That \
     question is decided when a run assembles a prompt, by re-reading the source record, so an \
     entry recorded as confirmed stops being offered once the task, the files or the criteria \
     behind it change. This page lists every entry either way, because leaving one out would \
     let a candidate read like a fact.";

fn memory(ctx: &Ctx) -> anyhow::Result<Page> {
    let theme = ctx.theme;
    let project = ctx.store.project(ctx.cwd)?;
    // The scope a lookup is answered under. It is configuration, so an entry that applies only
    // elsewhere is not current here, and the page says which scope it read under.
    let scope = &ctx.config.knowledge_scope;
    // Every entry's current standing, the correction that touched it and what replaced it, as
    // the store answers it. The page presents that answer and adds no judgement of its own.
    let inspected = ctx.store.inspect_knowledge(Some(&project.id), scope)?;
    // Browsing lists every entry with its own label. Whether an entry would be given to a
    // run is a separate question, and the store answers it here rather than the page: the
    // supported set is exactly what the default retrieval of a run would accept.
    let entries: Vec<MemoryEntry> = if ctx.memory_query.is_empty() {
        ctx.store.memory_inventory(Some(&project.id))?
    } else {
        ctx.store.search_memory(
            Some(&project.id),
            ctx.memory_query,
            scope,
            KnowledgeRetrievalMode::IncludeUnconfirmed,
        )?
    };
    let supported: Vec<String> = ctx
        .store
        .search_memory(
            Some(&project.id),
            ctx.memory_query,
            scope,
            KnowledgeRetrievalMode::Supported,
        )?
        .into_iter()
        .map(|entry| entry.id)
        .collect();
    let mut items = vec![about_row(
        ctx,
        ABOUT_KEY,
        "What current, superseded and corrected mean here",
        WHAT_RETAINED_KNOWLEDGE_IS,
    )];
    if !entries.is_empty() {
        items.push(Item::table("", &MEMORY_COLUMNS));
    }
    items.extend(
        entries
            .iter()
            .map(|entry| {
                let scope_name = if entry.project_id.is_some() {
                    "project"
                } else {
                    "global"
                };
                let offered = supported.contains(&entry.id);
                let standing = inspected.iter().find(|item| item.id == entry.id);
                // The standing and the basis come first: at the smallest size only a few lines of
                // a record are on the page at once, and what a reader needs there is whether this
                // entry still stands and on what evidence. Enter opens the whole of it.
                let mut detail = knowledge_standing_lines(ctx, standing, &inspected);
                detail.extend(knowledge_basis(ctx, entry, offered));
                detail.extend(field(theme, "entry", &entry.id, ctx.width));
                detail.extend(field(theme, "kind", &entry.kind, ctx.width));
                detail.extend(field(theme, "status", &entry.status, ctx.width));
                detail.extend(field(theme, "scope", scope_name, ctx.width));
                detail.extend(field(
                    theme,
                    "read under",
                    &knowledge_scope_words(ctx),
                    ctx.width,
                ));
                detail.extend(field(theme, "author", &entry.author, ctx.width));
                detail.extend(field(
                    theme,
                    "reviewer",
                    entry
                        .reviewer
                        .as_deref()
                        .unwrap_or("none recorded; this entry is a candidate"),
                    ctx.width,
                ));
                detail.extend(field(
                    theme,
                    "from session",
                    if entry.source_session.is_empty() {
                        "not recorded"
                    } else {
                        &entry.source_session
                    },
                    ctx.width,
                ));
                detail.extend(field(theme, "recorded", &entry.created_at, ctx.width));
                detail.push(Line::default());
                detail.extend(paragraph(theme, MEMORY_BASIS, ctx.width));
                detail.push(Line::default());
                detail.extend(paragraph(theme, MEMORY_SUPPORT, ctx.width));
                detail.push(Line::default());
                detail.extend(text::markdown(
                    &entry.content,
                    ctx.width,
                    theme,
                    theme.body(),
                ));
                let (state, style) = entry_state(ctx, entry, standing, offered);
                Item::row(
                    entry.id.clone(),
                    vec![
                        Cell::text(text::one_line(&entry.title), theme.text()),
                        Cell::text(state, style),
                        Cell::text(
                            scope_name,
                            if scope_name == "global" {
                                theme.accent()
                            } else {
                                theme.faint()
                            },
                        ),
                    ],
                )
                .with_detail(detail)
            })
            .collect::<Vec<_>>(),
    );
    if entries.is_empty() {
        items.clear();
    }
    Ok(Page {
        view: View::Memory,
        title: View::Memory.title().into(),
        subtitle: {
            let current = inspected
                .iter()
                .filter(|item| item.availability == ymp_core::KnowledgeAvailability::Available)
                .count();
            if ctx.memory_query.is_empty() {
                format!(
                    "{} recorded · {current} current · {} supported as context",
                    entries.len(),
                    supported.len()
                )
            } else {
                format!(
                    "Matching \"{}\" · {current} current of {} recorded",
                    ctx.memory_query,
                    inspected.len()
                )
            }
        },
        items,
        empty: {
            let mut lines = nothing(
                theme,
                "No recorded memory matches",
                "An entry is recorded when a run retains one against an accepted result. Search with /memory QUERY.",
                ctx.width,
            );
            lines.push(Line::default());
            lines.extend(paragraph(theme, MEMORY_BASIS, ctx.width));
            lines.push(Line::default());
            lines.extend(paragraph(theme, MEMORY_SUPPORT, ctx.width));
            lines
        },
        hints: vec![
            ("s", "search"),
            ("f", "retire the entry"),
            ("Enter", "inspect"),
            ("Esc", "back"),
        ],
    })
}

/// The word for one entry's current standing, and the colour that repeats it.
///
/// The store answers this question; the page only puts it into words. Nothing here upgrades an
/// entry: an entry that is not current says which of the reasons applies to it.
fn availability_words(availability: &ymp_core::KnowledgeAvailability) -> &'static str {
    use ymp_core::KnowledgeAvailability::*;
    match availability {
        Available => "current",
        Superseded => "superseded",
        Retired => "retired",
        Rejected => "rejected",
        PendingCorrection => "correction proposed",
        ScopeMismatch => "other scope",
        Unconfirmed => "unconfirmed",
        SourceVersionChanged => "source changed",
        SourceUnavailable => "source unavailable",
    }
}

/// The same standing as a sentence: what it means for a run that looks this up.
fn availability_sentence(availability: &ymp_core::KnowledgeAvailability) -> &'static str {
    use ymp_core::KnowledgeAvailability::*;
    match availability {
        Available => "current: the default retrieval of a run accepts this entry in this scope.",
        Superseded => "superseded: an accepted correction replaced this entry. It is kept so the correction has a predecessor to point at, and it is not offered to a run.",
        Retired => "retired: this entry was withdrawn. It is kept and never offered.",
        Rejected => "rejected: this entry was refused when it was reviewed, so it was never support for anything.",
        PendingCorrection => "a correction to this entry was proposed and has not been committed. Until the runtime commits it, this entry is still the proposed one and not support.",
        ScopeMismatch => "recorded for other conditions than the ones in force here. Applicability is matched exactly, and a condition this scope does not name does not match.",
        Unconfirmed => "no passing evidence is attached to the acceptance it names, so it is context and not a confirmed finding.",
        SourceVersionChanged => "the result it was retained against has moved on, so what it says can no longer be re-read from that result.",
        SourceUnavailable => "the default retrieval does not accept it, and the reason is not one of the ones above. What it records is readable here and is not support.",
    }
}

/// What the store says about one entry now: its standing, what replaced it, what it replaced,
/// and the correction that did it with the evidence that correction was bound to.
fn knowledge_standing_lines(
    ctx: &Ctx,
    standing: Option<&ymp_core::KnowledgeInspection>,
    all: &[ymp_core::KnowledgeInspection],
) -> Vec<Line<'static>> {
    let theme = ctx.theme;
    let Some(standing) = standing else {
        return field(
            theme,
            "standing",
            "not read for this project, so its current standing is unknown here",
            ctx.width,
        );
    };
    let title_of = |id: &str| {
        all.iter()
            .find(|item| item.id == id)
            .map(|item| format!("{} ({})", item.entry.title, text::short_id(id)))
            .unwrap_or_else(|| text::short_id(id))
    };
    let mut lines = field(
        theme,
        "standing",
        availability_sentence(&standing.availability),
        ctx.width,
    );
    lines.extend(field(
        theme,
        "version",
        &format!(
            "{} · the stored lifecycle and content version of this entry",
            text::short_id(&standing.version)
        ),
        ctx.width,
    ));
    lines.extend(field(
        theme,
        "replaced by",
        &match &standing.replaced_by {
            Some(id) => title_of(id),
            None => "nothing; no accepted correction replaced this entry".to_owned(),
        },
        ctx.width,
    ));
    lines.extend(field(
        theme,
        "replaces",
        &match &standing.entry.supersedes {
            Some(id) => title_of(id),
            None => "nothing; this entry is not a replacement".to_owned(),
        },
        ctx.width,
    ));
    if let Some(correction) = &standing.correction {
        let side = if correction.target.id == standing.id {
            "this entry is the one that was corrected"
        } else {
            "this entry is the correction"
        };
        lines.extend(field(
            theme,
            "correction",
            &format!(
                "{side} · {} replaced {} at version {}",
                text::short_id(&correction.replacement_id),
                text::short_id(&correction.target.id),
                text::short_id(&correction.target.version)
            ),
            ctx.width,
        ));
        lines.extend(field(
            theme,
            "authorised by",
            &format!(
                "acceptance {} under trusted contract {}",
                text::short_id(&correction.acceptance_id),
                text::short_id(&correction.contract_id)
            ),
            ctx.width,
        ));
        lines.extend(field(
            theme,
            "corrected by",
            &format!(
                "policy {} version {}",
                correction.policy.id, correction.policy.version
            ),
            ctx.width,
        ));
    }
    if standing.availability == ymp_core::KnowledgeAvailability::Superseded
        && standing.replaced_by.is_some()
    {
        lines.extend(paragraph(
            theme,
            "Both are kept. A correction does not delete what it corrects, so the predecessor stays readable here with its own evidence and the replacement names it.",
            ctx.width,
        ));
    }
    lines.push(Line::default());
    lines
}

/// The scope the page answered under, named so a reader can tell an absent entry from one that
/// simply applies elsewhere.
fn knowledge_scope_words(ctx: &Ctx) -> String {
    if ctx.config.knowledge_scope.is_empty() {
        return "no conditions are configured, so only entries recorded without conditions of their own can be current".to_owned();
    }
    ctx.config
        .knowledge_scope
        .iter()
        .map(|(key, value)| format!("{key} is {value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The word for one entry's standing, which never upgrades a candidate into a fact.
///
/// Being offered as context is the store's answer, not a reading of the text: an entry whose
/// source moved on is recorded as confirmed and is no longer supported, and both are said.
fn entry_state(
    ctx: &Ctx,
    entry: &MemoryEntry,
    standing: Option<&ymp_core::KnowledgeInspection>,
    offered: bool,
) -> (String, Style) {
    let theme = ctx.theme;
    if let Some(standing) = standing {
        use ymp_core::KnowledgeAvailability::*;
        let style = match standing.availability {
            Available => theme.good(),
            Superseded | Retired => theme.faint(),
            Rejected => theme.bad(),
            _ => theme.warn(),
        };
        return (availability_words(&standing.availability).to_owned(), style);
    }
    if entry.status == "retired" {
        return ("retired".to_owned(), theme.faint());
    }
    match (&entry.provenance, offered) {
        (_, true) => ("supported".to_owned(), theme.good()),
        (Some(provenance), false) => match provenance.confirmation {
            ConfirmationStatus::Confirmed => ("confirmed, not offered".to_owned(), theme.warn()),
            ConfirmationStatus::Unconfirmed => ("unconfirmed".to_owned(), theme.warn()),
            ConfirmationStatus::Unknown => ("confirmation unknown".to_owned(), theme.warn()),
        },
        (None, false) => ("provenance unknown".to_owned(), theme.warn()),
    }
}

/// What one entry records about where it came from, field by field.
fn knowledge_basis(ctx: &Ctx, entry: &MemoryEntry, offered: bool) -> Vec<Line<'static>> {
    let theme = ctx.theme;
    let mut lines = field(
        theme,
        "confirmation",
        match &entry.provenance {
            Some(provenance) => match provenance.confirmation {
                ConfirmationStatus::Confirmed => {
                    "confirmed: the acceptance it names carried passing checks"
                }
                ConfirmationStatus::Unconfirmed => "unconfirmed: no passing evidence is attached",
                ConfirmationStatus::Unknown => "recorded without a grade",
            },
            None => "no provenance was recorded; this entry is context with unknown confirmation",
        },
        ctx.width,
    );
    lines.extend(field(
        theme,
        "given to a run",
        if offered {
            "yes, as support under the default retrieval"
        } else if entry.status == "retired" {
            "no; the entry is retired"
        } else {
            "no; it can be read here and is not offered as support"
        },
        ctx.width,
    ));
    let Some(provenance) = &entry.provenance else {
        return lines;
    };
    lines.extend(field(
        theme,
        "source",
        &match &provenance.source {
            Some(source) => format!(
                "acceptance {} · result {} version {} · criteria {} · {} confirmation(s)",
                text::short_id(&source.acceptance_id),
                text::short_id(&source.result_id),
                source.result_version,
                text::short_id(&source.criteria_version),
                source.confirmation_ids.len()
            ),
            None => "none recorded, so nothing can be re-read to support it".to_owned(),
        },
        ctx.width,
    ));
    lines.extend(field(
        theme,
        "applies only where",
        &if provenance.applicability.is_empty() {
            "no further condition is recorded".to_owned()
        } else {
            provenance
                .applicability
                .iter()
                .map(|(key, value)| format!("{key} is {value}"))
                .collect::<Vec<_>>()
                .join(", ")
        },
        ctx.width,
    ));
    lines.extend(field(
        theme,
        "retained by",
        &format!(
            "policy {} version {}",
            provenance.policy.id, provenance.policy.version
        ),
        ctx.width,
    ));
    if let Some(assignment) = &provenance.assignment_id {
        lines.extend(field(
            theme,
            "from assignment",
            &text::short_id(assignment),
            ctx.width,
        ));
    }
    lines
}

const WHAT_AN_OBSERVATION_IS: &[&str] = &[
    "An observation records one outcome for one agent at one kind of work. It is written when a result is accepted or rejected, never from an agent's own report.",
    "Only a confirmed observation that a session also credited counts toward who a later run may pick. A confirmed outcome is one whose evidence passed for every criterion it applies to; unconfirmed means the result was accepted on an independent review alone; unknown means the record predates grading.",
    "A high rate from few observations is not evidence of reliability, and an agent without observations is not thereby unreliable. Absence of a record is absence of a record.",
];

fn reputation(ctx: &Ctx) -> anyhow::Result<Page> {
    let theme = ctx.theme;
    let observations = ctx.store.observations()?;
    let confirmed = observations
        .iter()
        .filter(|observation| observation.confirmation == ConfirmationStatus::Confirmed)
        .count();
    let mut items = vec![about_row(
        ctx,
        "what an observation is",
        "What an observation is, and what counts toward selection",
        WHAT_AN_OBSERVATION_IS,
    )];
    if !observations.is_empty() {
        items.push(Item::table("", &REPUTATION_COLUMNS));
    }
    items.extend(
        observations
            .iter()
            .take(200)
            .map(|observation| {
                let style = if observation.success {
                    theme.good()
                } else {
                    theme.bad()
                };
                let verdict = if observation.success {
                    format!("{} accepted", theme.markers.ok)
                } else {
                    format!("{} rejected", theme.markers.fail)
                };
                let mut detail = field(theme, "agent", &observation.agent_name, ctx.width);
                detail.extend(field(
                    theme,
                    "version",
                    &observation.agent_version,
                    ctx.width,
                ));
                detail.extend(field(
                    theme,
                    "competence",
                    &format!("{} · {}", observation.competence, observation.difficulty),
                    ctx.width,
                ));
                detail.extend(field(
                    theme,
                    "outcome",
                    if observation.success {
                        "accepted"
                    } else {
                        "rejected"
                    },
                    ctx.width,
                ));
                detail.extend(field(
                    theme,
                    "evidence status",
                    confirmation_word(observation.confirmation),
                    ctx.width,
                ));
                detail.extend(field(
                theme,
                "credit",
                if ctx.records.credited.contains(&observation.id) {
                    "this session recorded the credit for it"
                } else if observation.confirmation == ConfirmationStatus::Confirmed {
                    "whether it was credited is recorded by the session that accepted the work, which this page does not read"
                } else {
                    "not eligible: only a confirmed outcome can be credited"
                },
                ctx.width,
            ));
                detail.extend(field(theme, "recorded", &observation.created_at, ctx.width));
                detail.push(Line::default());
                detail.extend(paragraph(theme, &observation.evidence, ctx.width));
                Item::row(
                    observation.id.clone(),
                    vec![
                        Cell::text(observation.agent_name.clone(), theme.text()),
                        Cell::text(
                            format!("{} / {}", observation.competence, observation.difficulty),
                            theme.faint(),
                        ),
                        Cell::text(verdict, style),
                        Cell::text(
                            confirmation_tag(observation.confirmation),
                            if observation.confirmation == ConfirmationStatus::Confirmed {
                                theme.good()
                            } else {
                                theme.muted()
                            },
                        ),
                    ],
                )
                .with_detail(detail)
            })
            .collect::<Vec<Item>>(),
    );
    Ok(Page {
        view: View::Reputation,
        title: View::Reputation.title().into(),
        subtitle: format!(
            "{} recorded · {confirmed} on confirmed evidence",
            observations.len()
        ),
        items,
        empty: nothing(
            theme,
            "No observations yet",
            "An observation is recorded when a reviewer accepts or rejects a candidate. A high rate from few observations is not evidence of reliability.",
            ctx.width,
        ),
        hints: vec![("Enter", "inspect"), ("Esc", "back")],
    })
}

const WHAT_LIMITS_ARE: &[&str] = &[
    "A limit is a bound on what a session may admit, not a target. When the next turn would not fit, it is refused, the session stops and stays resumable, and no turn is admitted around the refusal.",
    "A session captures the limits it started with. The values under the next run are editable and apply to a session started later: they do not change what a session already ran under, and they are not what a finished session was measured against.",
    "A session that ran before its limits were captured has nothing recorded here. That is a missing record, not a run without bounds.",
    "Token figures are what installations reported. A count nobody reported stays unknown rather than zero, and a figure an open turn can still add to is shown as a lower bound.",
    "A token ceiling is checked when a turn is admitted, against the tokens reported so far and the allowances of turns still open. A turn already running can report more than its allowance, so a ceiling bounds what is admitted, not everything a run spends.",
    "Where a count is incomplete, a session follows the policy it captured: stop admitting under its ceiling, or go on admitting against what was reported. Going on does not complete the count, so what is left under the ceiling is then a reported remainder and not a known one.",
];

/// A value that opens with a figure, such as `300 s` or `4 member(s)`, ordered by that figure so
/// that 4 comes before 12. A value recorded as unknown is ordered as unknown, after every known
/// value in either direction. Any other value is ordered as text, including `none`, which may
/// mean that no bound was set rather than that one is not known.
fn value_cell(value: impl Into<String>, style: Style) -> Cell {
    let value = value.into();
    let unknown = value == "unknown" || value.starts_with("unknown ");
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    let figure = digits.parse::<i128>().ok();
    let cell = Cell::text(value, style);
    match figure {
        Some(figure) => cell.sorted_by(SortKey::Number(figure)),
        None if unknown => cell.sorted_by(SortKey::Unknown),
        None => cell,
    }
}

const LIMIT_COLUMNS: [Column; 2] = [Column::left("LIMIT").flex(), Column::left("VALUE")];

fn limits(ctx: &Ctx) -> Page {
    let theme = ctx.theme;
    let mut items = vec![about_row(
        ctx,
        "what a limit is",
        "What a limit is, and which run it applies to",
        WHAT_LIMITS_ARE,
    )];
    items.push(Item::table("This session, as captured", &LIMIT_COLUMNS));
    items.extend(captured_limit_rows(ctx));
    items.push(Item::table("The next run", &LIMIT_COLUMNS));
    let limits = &ctx.config.limits;
    for (key, value, description) in [
        (
            "parallel",
            limits.parallel.to_string(),
            "Turns that may be admitted at the same time. Whether two of them overlap is decided by their recorded access, not by this number.",
        ),
        (
            "turns",
            limits.turns.to_string(),
            "Total provider turns a session may spend before it pauses.",
        ),
        (
            "timeout",
            format!("{} s", limits.turn_timeout_secs),
            "How long a single turn may run before it is abandoned.",
        ),
        (
            "attempts",
            limits.attempts.to_string(),
            "Rejected attempts a task may make before it is blocked.",
        ),
    ] {
        let mut detail = paragraph(theme, description, ctx.width);
        detail.push(Line::default());
        detail.extend(paragraph(
            theme,
            "Editing this changes what a later session starts with. A session already running keeps the value it captured.",
            ctx.width,
        ));
        items.push(
            Item::row(
                key,
                vec![
                    Cell::text(key, theme.text()),
                    value_cell(value, theme.accent()),
                ],
            )
            .with_detail(detail),
        );
    }
    Page {
        view: View::Limits,
        title: View::Limits.title().into(),
        // Both variants are short enough to leave the title and the command their cells at
        // the smallest supported width. The two sections below carry the numbers in full.
        subtitle: match captured_limits(ctx) {
            Some((limits, _)) => format!(
                "captured {} turns, {} at a time",
                limits.turns, limits.parallel
            ),
            None => "No captured limits were read".into(),
        },
        items,
        empty: Vec::new(),
        hints: vec![
            ("+ / -", "adjust the next run"),
            ("Enter", "edit or inspect"),
            ("Esc", "back"),
        ],
    }
}

/// The limits a session captured, and the budget it accounted against them.
fn captured_limits<'a>(ctx: &'a Ctx) -> Option<(&'a Limits, Option<&'a SessionBudget>)> {
    let trace = ctx.records.trace.as_ref()?;
    let budget = trace.budget.as_ref();
    let limits = budget
        .map(|budget| &budget.limits)
        .or(trace.policy.as_ref().map(|policy| &policy.limits))?;
    Some((limits, budget))
}

fn captured_limit_rows(ctx: &Ctx) -> Vec<Item> {
    let theme = ctx.theme;
    let Some((limits, budget)) = captured_limits(ctx) else {
        let headline = match (ctx.session, ctx.records.unreadable.as_ref()) {
            (None, _) => "No session is loaded, so nothing was read",
            (Some(_), Some(_)) => "This session's records could not be read",
            (Some(_), None) => "This session captured no limits",
        };
        let mut detail = paragraph(
            theme,
            match (ctx.session, ctx.records.unreadable.as_ref()) {
                (None, _) => "Open a session with /sessions to see the limits it ran under.",
                (Some(_), Some(_)) => "Reading failed, which is not the same as a session that captured nothing.",
                (Some(_), None) => "It ran before limits were captured with a session. What it was actually bounded by is not recorded, and the values below are not it.",
            },
            ctx.width,
        );
        if let Some(error) = &ctx.records.unreadable {
            detail.push(Line::default());
            detail.extend(paragraph(theme, error, ctx.width));
        }
        return vec![Item::note(
            "captured:none",
            vec![
                Span::styled(format!("{} ", theme.markers.idle), theme.muted()),
                Span::styled(headline.to_owned(), theme.text()),
                Span::styled("  nothing captured".to_owned(), theme.muted()),
            ],
        )
        .with_detail(detail)];
    };
    let used = ctx
        .records
        .trace
        .as_ref()
        .map(|trace| trace.session.turns_used)
        .unwrap_or(0);
    let mut rows = vec![
        captured_row(
            ctx,
            "captured:turns",
            "turns",
            &format!("{used} of {}", limits.turns),
            "The turn bound this session started with. Used counts the turns its record names.",
        ),
        captured_row(
            ctx,
            "captured:parallel",
            "parallel",
            &limits.parallel.to_string(),
            "Turns this session was allowed to run at the same time.",
        ),
        captured_row(
            ctx,
            "captured:timeout",
            "timeout",
            &format!("{} s", limits.turn_timeout_secs),
            "How long one of this session's turns could run before it was abandoned.",
        ),
        captured_row(
            ctx,
            "captured:attempts",
            "attempts",
            &limits.attempts.to_string(),
            "Rejected attempts a task in this session could make before it was blocked.",
        ),
    ];
    let Some(budget) = budget else {
        rows.push(captured_row(
            ctx,
            "captured:budget",
            "accounting",
            "not recorded",
            "This session captured its limits without the accounting that later sessions keep. What it admitted and what it spent against the bound is not in the record.",
        ));
        return rows;
    };
    rows.push(captured_row(
        ctx,
        "captured:admitted",
        "turns admitted",
        &budget.admitted_invocations.to_string(),
        &format!(
            "This session was admitted to run {} turn(s), and its record still shows {} open. An open turn is in flight only while a run is active.",
            budget.admitted_invocations, budget.in_flight_invocations
        ),
    ));
    rows.push(captured_row(
        ctx,
        "captured:reserved",
        "reserved turns",
        &format!(
            "{} + {}",
            budget.startup_invocations, budget.protected_review_invocations
        ),
        &format!(
            "{} turn(s) were set aside so the session could begin, and {} kept so a result could still be reviewed independently after the rest of the budget was gone.",
            budget.startup_invocations, budget.protected_review_invocations
        ),
    ));
    rows.extend(token_rows(ctx, limits.resources.as_ref(), budget));
    if let Some(denial) = &budget.last_denial {
        rows.push(captured_row(
            ctx,
            "captured:denial",
            "last stop",
            &denial.code,
            &format!(
                "At {} the budget refused {} work: {}",
                denial.at, denial.purpose, denial.message
            ),
        ));
    }
    rows
}

/// A captured value is a record, so its row is read-only: the keys that edit a limit are
/// the four under the next run, and nothing here shares one.
fn captured_row(ctx: &Ctx, key: &str, label: &str, value: &str, description: &str) -> Item {
    let theme = ctx.theme;
    let mut detail = paragraph(theme, description, ctx.width);
    detail.push(Line::default());
    detail.extend(paragraph(
        theme,
        "This is what the session recorded. It cannot be edited here, and editing the next run does not change it.",
        ctx.width,
    ));
    Item::row(
        key.to_owned(),
        vec![
            Cell::text(label, theme.text()),
            value_cell(value, theme.body()),
        ],
    )
    .with_detail(detail)
}

/// What the session captured about tokens, and where its accounting stands now.
///
/// Every figure is read from the limits the session captured or from the budget the store
/// computed for it; none is taken from the configuration a later run would use. A remainder
/// under the ceiling is given as what reported counts leave, because where a count is
/// incomplete that is all that is known.
fn token_rows(
    ctx: &Ctx,
    resources: Option<&ymp_core::ResourceLimits>,
    budget: &SessionBudget,
) -> Vec<Item> {
    let Some(resources) = resources else {
        return vec![
            observed_row(ctx, budget),
            captured_row(
                ctx,
                "captured:token-policy",
                "token policy",
                "not captured",
                "This session captured no resource limits, so no token ceiling, turn allowance, review protection or policy for incomplete counts applied to it. None of today's values is shown in their place.",
            ),
            strict_row(ctx, budget, None),
        ];
    };
    let totals = &budget.observed_usage;
    let partial = totals.is_partial();
    let held = budget.reserved_tokens.unwrap_or(0);
    let ceiling = match resources.observed_tokens {
        None => "This session captured no token ceiling, so no turn was refused for tokens. What installations reported is still recorded under tokens observed.".to_owned(),
        Some(total) => {
            let standing = match totals.known_total() {
                Some(spent) if partial => {
                    let left = total.saturating_sub(spent).saturating_sub(held);
                    format!("By reported counts, {spent} were spent and {held} are held for turns still open, which leaves {left}. Not every count is complete, so what is truly left is not known: it is at most {left}.")
                }
                Some(spent) => {
                    let left = total.saturating_sub(spent).saturating_sub(held);
                    format!("{spent} were reported spent and {held} are held for turns still open, which leaves {left}.")
                }
                None => format!("No turn reported a count, so no spending is known, {held} are held for turns still open, and what is truly left is not known."),
            };
            format!("A turn was admitted only if the tokens reported so far, the allowances still held by open turns and its own allowance fitted under {total}. {standing}")
        }
    };
    let allowance = match resources.invocation_tokens {
        Some(each) => format!("A turn whose assignment requested no allowance was admitted with {each}, and no assignment could request more. An allowance is what admission set aside while the turn ran. It is not a limit the installation enforced, and a turn can report more than its allowance."),
        None => "No allowance was captured, so turns were admitted without setting tokens aside for them.".to_owned(),
    };
    let review = match (
        budget.protected_review_tokens,
        resources.review_reserve_tokens,
        resources.invocation_tokens,
    ) {
        (Some(protected), Some(reserve), _) => format!("The session captured {reserve} tokens for the review it owes, and {protected} of them are protected now. What review turns have reported, and the allowances of review turns still open, come off that, and nothing is protected once the review the session owes is complete. Other work is refused where it would reach into them."),
        (Some(protected), None, Some(each)) => format!(
            "No review reserve was captured, so the protection is one turn allowance for each review turn still owed: {each} × {} = {protected}. Other work is refused where it would reach into them.",
            budget.protected_review_invocations
        ),
        _ => "No token ceiling was captured, so no tokens were protected for review. The review turns kept aside are counted under reserved turns.".to_owned(),
    };
    let (policy, policy_words) = match resources.unknown_usage {
        ymp_core::UnknownUsagePolicy::Stop => (
            "stop admitting",
            "Where an installation left a turn's count incomplete, the session admits no further turn under its token ceiling, and nothing is inferred to fill the gap.",
        ),
        ymp_core::UnknownUsagePolicy::BoundedNative => (
            "admit on reported",
            "Where an installation left a turn's count incomplete, the session goes on admitting turns against what was reported. An incomplete count stays incomplete: the remainder under the ceiling is what reported counts leave, not what is truly left, and no strict token bound follows. The ceiling, the review protection and the turn, parallel, context, output and timeout limits still apply.",
        ),
    };
    let policy_detail = match (resources.observed_tokens, partial) {
        (None, _) => format!("{policy_words} The session captured no token ceiling, so this policy had nothing to act on."),
        (Some(_), true) => format!("{policy_words} Not every count in this session is complete."),
        (Some(_), false) => policy_words.to_owned(),
    };
    let figure = |value: Option<u64>| value.map_or_else(|| "none".to_owned(), |v| v.to_string());
    vec![
        observed_row(ctx, budget),
        captured_row(
            ctx,
            "captured:token-ceiling",
            "token ceiling",
            &figure(resources.observed_tokens),
            &ceiling,
        ),
        captured_row(
            ctx,
            "captured:turn-allowance",
            "turn allowance",
            &figure(resources.invocation_tokens),
            &allowance,
        ),
        captured_row(
            ctx,
            "captured:review-tokens",
            "review tokens",
            &figure(budget.protected_review_tokens),
            &review,
        ),
        captured_row(
            ctx,
            "captured:unknown-usage",
            "incomplete counts",
            policy,
            &policy_detail,
        ),
        strict_row(ctx, budget, Some(resources)),
    ]
}

/// What installations reported for the session, with a count nobody reported kept unknown.
fn observed_row(ctx: &Ctx, budget: &SessionBudget) -> Item {
    captured_row(
        ctx,
        "captured:tokens",
        "tokens observed",
        &short_observed(budget),
        &format!(
            "{}. This is what installations reported for this session. A turn that reported nothing is counted as a turn, and its tokens stay unknown rather than being counted as zero.",
            observed_words(budget)
        ),
    )
}

/// Whether a strict token bound holds for what the session spent.
fn strict_row(
    ctx: &Ctx,
    budget: &SessionBudget,
    resources: Option<&ymp_core::ResourceLimits>,
) -> Item {
    let words = if budget.strict_token_bound {
        "The record states that what this session spent was held to a proved token bound."
            .to_owned()
    } else {
        let mut words = "No proved bound holds for what this session spent. A turn already running can report more than its allowance, and no installation this session used proves a hard cap on a whole run, so a ceiling bounds what is admitted and not everything that is spent.".to_owned();
        if budget.observed_usage.is_partial() {
            words.push_str(
                " Its counts are also incomplete, which on its own rules a strict bound out.",
            );
        }
        if resources.is_some_and(|r| r.unknown_usage == ymp_core::UnknownUsagePolicy::BoundedNative)
        {
            words.push_str(" Admitting on reported counts does not change that.");
        }
        words
    };
    captured_row(
        ctx,
        "captured:bound",
        "strict bound",
        if budget.strict_token_bound {
            "proved"
        } else {
            "not proved"
        },
        &words,
    )
}

/// The short form for the row, which must leave room for its own label.
fn short_observed(budget: &SessionBudget) -> String {
    let totals = &budget.observed_usage;
    match totals.known_total() {
        Some(total) if totals.open_calls > 0 || totals.partial_calls > 0 => format!("{total}+"),
        Some(total) => total.to_string(),
        None => "unknown".to_owned(),
    }
}

fn observed_words(budget: &SessionBudget) -> String {
    let totals = &budget.observed_usage;
    let mut text = match totals.known_total() {
        Some(total) if totals.open_calls > 0 || totals.partial_calls > 0 => {
            format!("at least {total} over {} turn(s)", totals.calls)
        }
        Some(total) => format!("{total} over {} turn(s)", totals.calls),
        None => format!("unknown over {} turn(s)", totals.calls),
    };
    if totals.reported < totals.calls {
        text.push_str(&format!(
            " · {} reported nothing",
            totals.calls - totals.reported
        ));
    }
    if let Some(overshoot) = budget.observed_token_overshoot {
        text.push_str(&format!(" · {overshoot} past the bound"));
    }
    text
}

// ---------------------------------------------------------------------------
// Assignments, settings and temporary authority
// ---------------------------------------------------------------------------

/// What the assignment page says about itself, before it lists a single record.
///
/// Every sentence is about what the runtime writes, and each one was checked against the
/// record it describes. None of them promises that a setting took effect, because only the
/// installation can report that, and none of them describes a grant as a sandbox.
const HOW_WORK_IS_ASSIGNED: &[&str] = &[
    "A run forms its own team and assigns each piece of work itself. Nothing on this page was chosen by hand, and opening it starts nothing.",
    "An assignment names the agent, the purpose of the turn, the task attempt it belongs to and the directory the turn ran in. It is written when the turn is admitted and is never rewritten afterwards.",
    "Requested is what the run asked for. Sent is what the adapter passed to the installation. Reported is what the installation said it used. A column the record leaves empty stays empty here: no value is copied from one column into another, and a requested value nobody confirmed is never shown as applied.",
    "A fixed model or effort is a constraint the configuration states, and it is the only value allowed for that agent. Where nothing is fixed, the run chooses for itself and may choose differently on the next turn.",
    "A grant is permission to use one coordination call, issued for one assignment and recorded with it. It stops working when that turn ends or when the record says it was revoked. It is not a restriction on what the turn can do in the working directory: by the time a turn runs, it already has the same access to that directory as the user who started ymp.",
    "Where a session captured a token ceiling, a turn is admitted with a token allowance: the one its assignment requested, or the session's per-turn default where it requested none. An allowance is what admission set aside while the turn ran, not a limit the installation enforced.",
    "Tokens counted against a turn stay with it whether the turn completed, failed or was cancelled, and a count no installation reported stays unknown rather than zero.",
];

fn assignments(ctx: &Ctx) -> Page {
    let records = ctx.records;
    let mut items = vec![about_row(
        ctx,
        "how work is assigned",
        "How work is assigned, and what these records are",
        HOW_WORK_IS_ASSIGNED,
    )];
    let open = records.open();
    let running: Vec<&AssignmentRecord> = open.clone();
    let earlier: Vec<&AssignmentRecord> = records
        .assignments()
        .iter()
        .filter(|assignment| !open.iter().any(|live| live.id == assignment.id))
        .collect();
    if !running.is_empty() {
        // A record with no end is a turn in flight only while a run is active in this
        // window. Otherwise it is a turn that was left open, which is not the same thing.
        items.push(Item::table(
            if ctx.live { "Running now" } else { "Left open" },
            &ASSIGNMENT_COLUMNS,
        ));
        for assignment in &running {
            items.push(assignment_row(ctx, assignment, ctx.live));
        }
    }
    if !earlier.is_empty() {
        items.push(Item::table("Recorded earlier", &ASSIGNMENT_COLUMNS));
        for assignment in &earlier {
            items.push(assignment_row(ctx, assignment, false));
        }
    }
    if records.assignments().is_empty() {
        items.clear();
    }
    Page {
        view: View::Assignments,
        title: View::Assignments.title().into(),
        subtitle: match (ctx.session, records.assignments().len()) {
            (None, _) => "No session is loaded".into(),
            (Some(_), 0) => "Nothing was assigned in this session".into(),
            (Some(_), total) => format!(
                "{total} assigned · {} · read {}",
                match (running.len(), ctx.live) {
                    (0, _) => "none open".to_owned(),
                    (open, true) => format!("{open} running"),
                    (open, false) => format!("{open} left open"),
                },
                text::clock(&records.read_at)
            ),
        },
        items,
        empty: empty_records(
            ctx,
            "No assignments were recorded",
            "A session records an assignment for every turn it admits. A session that recorded none either ran before assignments were written or never reached a turn.",
        ),
        hints: vec![("Enter", "inspect"), ("Esc", "back")],
    }
}

fn assignment_row(ctx: &Ctx, assignment: &AssignmentRecord, live: bool) -> Item {
    let theme = ctx.theme;
    let invocation = ctx.records.last_invocation(&assignment.id);
    let state = invocation.map(|i| i.state).unwrap_or(assignment.state);
    let (marker, word, style) = invocation_state(state, live, theme);
    // The row is one turn, so it is named by that turn's own model and reported effort.
    let name = label::assignment(assignment, invocation);
    let mut detail = field(theme, "assignment", &assignment.id, ctx.width);
    detail.extend(field(
        theme,
        "agent",
        &format!("{name} · {}", assignment.agent_id),
        ctx.width,
    ));
    detail.extend(field(
        theme,
        "profile version",
        &assignment.agent_config_version,
        ctx.width,
    ));
    detail.extend(field(theme, "provider", &assignment.provider_id, ctx.width));
    detail.extend(field(theme, "purpose", &assignment.purpose, ctx.width));
    detail.extend(field(
        theme,
        "task",
        &task_attempt(ctx, assignment),
        ctx.width,
    ));
    detail.extend(field(theme, "state", word, ctx.width));
    detail.extend(field(theme, "started", &assignment.started_at, ctx.width));
    detail.extend(field(
        theme,
        "ended",
        assignment.ended_at.as_deref().unwrap_or("still open"),
        ctx.width,
    ));
    detail.extend(field(
        theme,
        "directory",
        &assignment.cwd.display().to_string(),
        ctx.width,
    ));
    detail.extend(field(
        theme,
        "turn timeout",
        &format!("{} s", assignment.timeout_secs),
        ctx.width,
    ));
    detail.extend(field(
        theme,
        "token allowance",
        &allowance_words(ctx, assignment),
        ctx.width,
    ));
    detail.push(Line::default());
    detail.push(Line::from(Span::styled(
        "Access to the directory".to_owned(),
        theme.muted(),
    )));
    detail.extend(access_lines(ctx, assignment));
    detail.push(Line::default());
    detail.push(Line::from(Span::styled(
        "Model and effort".to_owned(),
        theme.muted(),
    )));
    detail.extend(settings_lines(ctx, assignment, invocation));
    detail.push(Line::default());
    detail.push(Line::from(Span::styled(
        "Where the turn ran".to_owned(),
        theme.muted(),
    )));
    match invocation {
        Some(invocation) => {
            detail.extend(field(
                theme,
                "backend",
                &invocation
                    .execution_backend
                    .as_ref()
                    .map(|backend| format!("{} {}", backend.id, backend.version))
                    .unwrap_or_else(|| "not recorded".into()),
                ctx.width,
            ));
            detail.extend(field(
                theme,
                "native session",
                invocation
                    .native_session_id
                    .as_deref()
                    .unwrap_or("not reported"),
                ctx.width,
            ));
            detail.extend(field(
                theme,
                "native turn",
                invocation.native_turn_id.as_deref().unwrap_or("not reported"),
                ctx.width,
            ));
            detail.extend(field(
                theme,
                "native version",
                invocation.native_version.as_deref().unwrap_or("not reported"),
                ctx.width,
            ));
            detail.extend(field(
                theme,
                "resumed from",
                invocation
                    .resumed_from
                    .as_deref()
                    .unwrap_or("nothing; this turn started fresh"),
                ctx.width,
            ));
            detail.extend(field(theme, "tokens", &invocation_usage(invocation), ctx.width));
            if let Some(reason) = &invocation.terminal_reason {
                detail.extend(field(theme, "ended because", reason, ctx.width));
            }
        }
        None => detail.extend(paragraph(
            theme,
            "No invocation was recorded for this assignment, so nothing is known about the turn itself.",
            ctx.width,
        )),
    }
    detail.push(Line::default());
    detail.push(Line::from(Span::styled(
        "Context the turn was given".to_owned(),
        theme.muted(),
    )));
    if assignment.context.is_empty() {
        detail.extend(paragraph(
            theme,
            "The record names no context references.",
            ctx.width,
        ));
    } else {
        for reference in &assignment.context {
            detail.extend(field(
                theme,
                context_kind(&reference.kind),
                &context_detail(reference),
                ctx.width,
            ));
        }
    }
    detail.push(Line::default());
    detail.push(Line::from(Span::styled(
        "Coordination permissions".to_owned(),
        theme.muted(),
    )));
    let grants = ctx.records.grants_of(&assignment.id);
    if grants.is_empty() {
        detail.extend(paragraph(
            theme,
            "No grant was recorded for this assignment, so its turn could not use the coordination calls at all.",
            ctx.width,
        ));
    } else {
        for grant in &grants {
            detail.extend(field(theme, "granted", &grant_detail(grant), ctx.width));
        }
    }
    detail.push(Line::default());
    detail.push(Line::from(Span::styled(
        "Recorded reason".to_owned(),
        theme.muted(),
    )));
    detail.extend(paragraph(theme, &assignment.reason, ctx.width));
    Item::row(
        text::short_id(&assignment.id),
        vec![
            Cell::text(marker, style),
            Cell::text(name, theme.text()),
            Cell::text(assignment.purpose.to_string(), theme.faint()),
            Cell::text(word, style),
            Cell::text(stamp(&assignment.started_at), theme.faint())
                .sorted_by(SortKey::Text(assignment.started_at.clone())),
        ],
    )
    .with_detail(detail)
}

const ASSIGNMENT_COLUMNS: [Column; 5] = [
    Column::left(""),
    Column::left("AGENT").flex(),
    Column::left("PURPOSE"),
    Column::left("STATE"),
    Column::left("STARTED").hide(1),
];

/// The token allowance a turn was admitted with, and where it came from.
///
/// An assignment that requested an allowance names it, and one that did not took the per-turn
/// default the session captured. Both are read from the record and from the captured limits,
/// never from the configuration a later run would use.
fn allowance_words(ctx: &Ctx, assignment: &AssignmentRecord) -> String {
    let requested = assignment.token_reservation;
    let Some((limits, _)) = captured_limits(ctx) else {
        return match requested {
            Some(tokens) => format!(
                "{tokens} · requested by this assignment; the session captured no limits to read it against"
            ),
            None => {
                "not known · the session captured no limits, so no allowance can be named".to_owned()
            }
        };
    };
    let ceiling = limits
        .resources
        .as_ref()
        .and_then(|resources| resources.invocation_tokens);
    match (requested, ceiling) {
        (Some(tokens), Some(ceiling)) if tokens <= ceiling => format!(
            "{tokens} · requested by this assignment, within the per-turn ceiling of {ceiling} the session captured"
        ),
        (Some(tokens), Some(ceiling)) => format!(
            "{tokens} · requested by this assignment, above the per-turn ceiling of {ceiling} the session captured"
        ),
        (Some(tokens), None) => format!(
            "{tokens} · requested by this assignment; the session captured no per-turn ceiling"
        ),
        (None, Some(each)) => format!(
            "{each} · inherited: this assignment requested none, so it took the session's per-turn default"
        ),
        (None, None) => {
            "none · the session captured no token ceiling, so nothing was set aside for this turn"
                .to_owned()
        }
    }
}

/// Marker, word and colour for the state of a turn. A record that says `running` while no
/// run is active in this window describes a turn that was left open, not one in flight.
fn invocation_state(
    state: InvocationState,
    live: bool,
    theme: &Theme,
) -> (String, &'static str, Style) {
    let m = theme.markers;
    match state {
        InvocationState::Running if live => (m.busy.into(), "running", theme.warn()),
        InvocationState::Running => (m.paused.into(), "left open", theme.info()),
        InvocationState::Completed => (m.ok.into(), "completed", theme.good()),
        InvocationState::Failed => (m.fail.into(), "failed", theme.bad()),
        InvocationState::Cancelled => (m.warn.into(), "cancelled", theme.warn()),
        InvocationState::Interrupted => (m.warn.into(), "interrupted", theme.warn()),
    }
}

/// The three columns of one execution setting, then what they do and do not establish.
fn settings_lines(
    ctx: &Ctx,
    assignment: &AssignmentRecord,
    invocation: Option<&InvocationRecord>,
) -> Vec<Line<'static>> {
    let theme = ctx.theme;
    let mut lines = Vec::new();
    let empty = ExecutionSettings::default();
    let sent = invocation.map(|i| &i.sent).unwrap_or(&empty);
    let reported = invocation.map(|i| &i.reported).unwrap_or(&empty);
    for (label, requested, sent, reported) in [
        (
            "model",
            assignment.requested.model.as_deref(),
            sent.model.as_deref(),
            reported.model.as_deref(),
        ),
        (
            "effort",
            assignment.requested.effort.as_deref(),
            sent.effort.as_deref(),
            reported.effort.as_deref(),
        ),
        (
            "permissions",
            assignment.requested.permission_mode.as_deref(),
            sent.permission_mode.as_deref(),
            reported.permission_mode.as_deref(),
        ),
    ] {
        lines.extend(field(
            theme,
            label,
            &setting_state(requested, sent, reported),
            ctx.width,
        ));
    }
    let (source, fixed) = pinned(ctx, &assignment.agent_id);
    lines.extend(field(theme, "fixed", &pin_words(&fixed, source), ctx.width));
    lines
}

/// What an acceptance contract binds, as it was captured before the work ran.
///
/// The contract is what a result is judged against: its criteria, the artifacts it names,
/// the inputs whose bytes were recorded, and the checker that will run its checks. Nothing
/// here says a check passed; a passing check is a record of its own.
fn contract_detail(
    ctx: &Ctx,
    captured: &ymp_core::CapturedAcceptanceContract,
) -> Vec<Line<'static>> {
    let theme = ctx.theme;
    let contract = &captured.contract;
    let mut lines = field(theme, "for the task", &contract.task_title, ctx.width);
    lines.extend(field(
        theme,
        "criteria",
        &if contract.criteria.is_empty() {
            "none".to_owned()
        } else {
            contract
                .criteria
                .iter()
                .map(|criterion| criterion.id.clone())
                .collect::<Vec<_>>()
                .join(", ")
        },
        ctx.width,
    ));
    for criterion in &contract.criteria {
        lines.extend(field(
            theme,
            &criterion.id,
            &criterion.description,
            ctx.width,
        ));
    }
    lines.extend(field(
        theme,
        "checks bound",
        &format!(
            "{} · {}",
            contract.checks.len(),
            contract
                .checks
                .iter()
                .map(|check| check.id.clone())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ctx.width,
    ));
    lines.extend(field(
        theme,
        "artifacts named",
        &if contract.artifacts.is_empty() {
            "none".to_owned()
        } else {
            contract
                .artifacts
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        },
        ctx.width,
    ));
    // Each declared input is named with the digest it was captured at. A criterion that says a
    // claim matches the declared source says nothing to a reader who cannot see which file
    // that was.
    if contract.inputs.is_empty() {
        lines.extend(field(theme, "declared input", "none", ctx.width));
    }
    for input in &contract.inputs {
        let digest = captured
            .inputs
            .iter()
            .find(|snapshot| &snapshot.path == input)
            .and_then(|snapshot| snapshot.sha256.as_deref())
            .map(|digest| format!("captured at sha256 {}", text::short_id(digest)))
            .unwrap_or_else(|| "no digest was captured".to_owned());
        lines.extend(field(
            theme,
            "declared input",
            &format!("{} · {digest}", input.display()),
            ctx.width,
        ));
    }
    if let Some(binding) = &contract.knowledge_correction {
        lines.extend(field(
            theme,
            "corrects",
            &format!(
                "retained entry {} at version {}",
                text::short_id(&binding.target.id),
                text::short_id(&binding.target.version)
            ),
            ctx.width,
        ));
        lines.extend(field(
            theme,
            "source change",
            &match &binding.source_replacement {
                Some(source) => format!(
                    "{} replaced by {}",
                    source.previous_input.display(),
                    source.replacement_input.display()
                ),
                None => "none declared".to_owned(),
            },
            ctx.width,
        ));
        lines.extend(field(
            theme,
            "established by",
            &binding.criterion_ids.join(", "),
            ctx.width,
        ));
    }
    lines.extend(field(
        theme,
        "checker",
        &format!("{} {}", captured.checker.id, captured.checker.version),
        ctx.width,
    ));
    lines.extend(field(
        theme,
        "contract version",
        &text::short_id(&captured.version),
        ctx.width,
    ));
    lines.extend(field(
        theme,
        "checker code recorded",
        &format!("{} path(s) by digest", captured.verifier_digests.len()),
        ctx.width,
    ));
    lines.push(Line::default());
    lines.extend(paragraph(
        theme,
        "This is what a result will be judged against, captured before the work ran. It is a binding and not a result: whether each check then passed is a record of its own, and an acceptance states which criteria the evidence covered.",
        ctx.width,
    ));
    if contract.knowledge_correction.is_some() {
        lines.push(Line::default());
        lines.extend(paragraph(
            theme,
            "It also binds a correction. The entry it names is replaced only if a result is accepted with passing evidence for the criteria it is established by, and that replacement is a record of its own.",
            ctx.width,
        ));
    }
    lines
}

/// The access a task declares, which is a declaration and not a measurement.
///
/// Write is the default, so a task that declares nothing is a writing task. A read-only
/// task is one the plan said so about, and the runtime then asks its backend for read-only
/// access; it is never concluded from what the work appears to need.
fn declared_access_words(access: TaskAccess) -> &'static str {
    match access {
        TaskAccess::ReadOnly => "read-only, declared in the plan; the turn is asked read-only",
        TaskAccess::Write => "may write; this is the default where a plan declares nothing",
    }
}

/// What one access actually permits, in words, without naming a protection.
fn access_words(access: &WorkspaceAccess) -> String {
    match access {
        WorkspaceAccess::WriteAll => "may write anywhere in the directory".to_owned(),
        WorkspaceAccess::ReadAll => "reads the directory and writes nothing".to_owned(),
        WorkspaceAccess::Scoped { reads, writes } if writes.is_empty() => {
            format!("reads {} declared path(s) and writes nothing", reads.len())
        }
        WorkspaceAccess::Scoped { reads, writes } => format!(
            "writes {} declared path(s), reads {}",
            writes.len(),
            reads.len()
        ),
    }
}

/// The paths a scoped access names, so a reader can see what was declared.
fn access_paths(theme: &Theme, access: &WorkspaceAccess, width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if let WorkspaceAccess::Scoped { reads, writes } = access {
        for (label, paths) in [("writes", writes), ("reads", reads)] {
            if !paths.is_empty() {
                lines.extend(field(
                    theme,
                    label,
                    &paths
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                    width,
                ));
            }
        }
    }
    lines
}

// ---------------------------------------------------------------------------
// The shared plan
// ---------------------------------------------------------------------------

/// The title a proposal's target task carries, from the plan the snapshot holds.
///
/// A proposal names a task by id and by the version it was made against, so the title is read
/// from the plan and never from the proposal: a title the proposal carried could have changed.
fn board_task_title(ctx: &Ctx, task_id: &str) -> String {
    ctx.records
        .board_task(task_id)
        .map(|entry| entry.task.title.clone())
        .or_else(|| {
            ctx.tasks
                .iter()
                .find(|task| task.id == task_id)
                .map(|task| task.title.clone())
        })
        .unwrap_or_else(|| text::short_id(task_id))
}

/// The settings a proposal asks a turn to run with, as the proposal carries them.
///
/// An absent value is absent: the runtime resolves it at admission from the actor's own policy,
/// and naming a default here would claim the proposal said something it did not.
fn settings_words(settings: &ymp_core::ModelEffort) -> String {
    match (&settings.model, &settings.effort) {
        (Some(model), Some(effort)) => format!("{model} at {effort}"),
        (Some(model), None) => format!("{model}, effort not named"),
        (None, Some(effort)) => format!("effort {effort}, model not named"),
        (None, None) => "neither model nor effort was named".to_owned(),
    }
}

/// What one proposal asks for, in a few words for a row and a sentence for a record.
fn board_change_words(ctx: &Ctx, proposal: &BoardProposal) -> (String, String) {
    let author = record_words(&proposer_name(ctx, proposal), &proposal.agent_id);
    match &proposal.change {
        BoardChange::AcceptResponsibility { task, settings } => {
            let title = board_task_title(ctx, &task.task_id);
            (
                format!("take on {title}"),
                format!(
                    "{author} asks to be the agent responsible for {title}, running {}",
                    settings_words(settings)
                ),
            )
        }
        BoardChange::Assign {
            task,
            agent_id,
            settings,
        } => {
            let title = board_task_title(ctx, &task.task_id);
            let holder = proposed_actor_words(ctx, proposal, agent_id);
            let holder_name = if *agent_id == proposal.agent_id {
                proposer_name(ctx, proposal)
            } else {
                record_name(ctx, agent_id, None, None)
            };
            (
                format!("give {title} to {holder_name}"),
                format!(
                    "{author} asks that {holder} be responsible for {title}, running {}",
                    settings_words(settings)
                ),
            )
        }
        BoardChange::Revise {
            task,
            approach,
            dependencies,
            checks,
        } => {
            let title = board_task_title(ctx, &task.task_id);
            let mut adds = Vec::new();
            if !dependencies.is_empty() {
                adds.push(format!("{} dependency(ies)", dependencies.len()));
            }
            if !checks.is_empty() {
                adds.push(format!("{} check(s)", checks.len()));
            }
            let additions = if adds.is_empty() {
                "no dependency or check".to_owned()
            } else {
                adds.join(" and ")
            };
            (
                format!("revise {title}"),
                format!(
                    "{author} asks to add an approach to {title}, with {additions}. A revision may only add: the objectives, checks, dependencies and authority the task already carries cannot be removed. The approach reads: {}",
                    text::one_line(approach)
                ),
            )
        }
        BoardChange::AddTask {
            title,
            competence,
            difficulty,
            access,
            dependencies,
            checks,
            ..
        } => (
            format!("add {title}"),
            format!(
                "{author} asks to add a task called {title}: {competence} work of {difficulty} difficulty, declaring {}, with {} dependency(ies) and {} check(s)",
                declared_access_words(*access),
                dependencies.len(),
                checks.len()
            ),
        ),
        BoardChange::Membership { members } => {
            let names = members
                .iter()
                .map(|id| proposed_actor_words(ctx, proposal, id))
                .collect::<Vec<_>>();
            (
                "change who is in the team".to_owned(),
                format!(
                    "{author} asks that the session's members be {}",
                    if names.is_empty() {
                        "nobody".to_owned()
                    } else {
                        names.join(", ")
                    }
                ),
            )
        }
    }
}

/// The outcome of one proposal, from the decision the runtime wrote for it.
///
/// The decision is the authority. A proposal's own stored status is written in the same
/// transaction, so the two agree, but only the decision carries why, and a proposal no decision
/// answers is still waiting rather than refused.
fn proposal_outcome(ctx: &Ctx, proposal: &BoardProposal) -> (String, Style) {
    let theme = ctx.theme;
    match ctx.records.board_decision(&proposal.id) {
        Some((_, board)) if board.accepted => ("committed".to_owned(), theme.good()),
        Some((_, _)) => ("rejected".to_owned(), theme.bad()),
        None => match proposal.status {
            BoardProposalStatus::Pending => ("proposed".to_owned(), theme.info()),
            // The status moved without a decision this session can read, which is a statement
            // about what was read and not about what the runtime decided.
            BoardProposalStatus::Committed => {
                ("committed, reason not read".to_owned(), theme.warn())
            }
            BoardProposalStatus::Rejected => ("rejected, reason not read".to_owned(), theme.warn()),
        },
    }
}

/// One proposal as a row: what it asks, what became of it, and everything behind both.
fn proposal_row(ctx: &Ctx, proposal: &BoardProposal) -> Item {
    let theme = ctx.theme;
    let (short, sentence) = board_change_words(ctx, proposal);
    let (outcome, style) = proposal_outcome(ctx, proposal);
    let mut detail = field(theme, "proposal", &proposal.id, ctx.width);
    let author = proposer_name(ctx, proposal);
    detail.extend(field(
        theme,
        "asked by",
        &record_words(&author, &proposal.agent_id),
        ctx.width,
    ));
    detail.extend(field(theme, "outcome", &outcome, ctx.width));
    detail.extend(field(
        theme,
        "from turn",
        &format!(
            "assignment {} · invocation {} · under grant {}",
            text::short_id(&proposal.assignment_id),
            text::short_id(&proposal.invocation_id),
            text::short_id(&proposal.grant_id)
        ),
        ctx.width,
    ));
    detail.extend(field(
        theme,
        "against plan",
        &format!(
            "{} · {}",
            text::short_id(&proposal.plan_version),
            match ctx.records.plan_version() {
                Some(current) if current == proposal.plan_version =>
                    "the plan as it stands now".to_owned(),
                Some(current) => format!("the plan is now {}", text::short_id(current)),
                None => "the plan could not be read here".to_owned(),
            }
        ),
        ctx.width,
    ));
    detail.extend(field(
        theme,
        "membership",
        &format!(
            "{} · a proposal made against other membership cannot be committed",
            text::short_id(&proposal.team_version)
        ),
        ctx.width,
    ));
    detail.extend(field(theme, "recorded", &proposal.created_at, ctx.width));
    detail.push(Line::default());
    detail.extend(paragraph(theme, &sentence, ctx.width));
    detail.push(Line::default());
    detail.push(Line::from(Span::styled(
        "Why it was asked for".to_owned(),
        theme.muted(),
    )));
    detail.extend(paragraph(theme, &proposal.rationale, ctx.width));
    detail.push(Line::default());
    detail.push(Line::from(Span::styled(
        "What the runtime decided".to_owned(),
        theme.muted(),
    )));
    detail.extend(board_decision_lines(ctx, proposal));
    Item::row(
        proposal.id.clone(),
        vec![
            Cell::text(theme.markers.activity, style),
            Cell::text(short, theme.text()),
            Cell::text(outcome, style),
            Cell::text(author, theme.faint()),
        ],
    )
    .with_detail(detail)
}

const PLAN_COLUMNS: [Column; 2] = [Column::left("PLAN").flex(), Column::left("STATE")];
const PROPOSAL_COLUMNS: [Column; 4] = [
    Column::left(""),
    Column::left("CHANGE").flex(),
    Column::left("OUTCOME"),
    Column::left("ASKED BY").hide(1),
];

/// The decision a proposal received, or the fact that it has not received one.
fn board_decision_lines(ctx: &Ctx, proposal: &BoardProposal) -> Vec<Line<'static>> {
    let theme = ctx.theme;
    let Some((decision, board)) = ctx.records.board_decision(&proposal.id) else {
        return paragraph(
            theme,
            match proposal.status {
                BoardProposalStatus::Pending => "Nothing yet. The runtime answers a proposal at a work boundary, when no turn of this session is running, and writes a decision with its reason either way.",
                _ => "This session's records carry no decision for this proposal, although its stored status has moved. What the runtime decided, and why, is not readable here.",
            },
            ctx.width,
        );
    };
    let mut lines = field(
        theme,
        "decided",
        if board.accepted {
            "committed · the plan now carries it"
        } else {
            "rejected · the plan was not changed"
        },
        ctx.width,
    );
    lines.extend(field(theme, "recorded", &decision.created_at, ctx.width));
    lines.extend(field(
        theme,
        "decided by",
        &format!(
            "{} {} · the runtime, not an agent",
            board.implementation.id, board.implementation.version
        ),
        ctx.width,
    ));
    lines.extend(field(
        theme,
        "resulting plan",
        &format!(
            "{} · {}",
            text::short_id(&board.resulting_plan_version),
            if board.accepted {
                "the version the plan took on"
            } else {
                "unchanged by this proposal"
            }
        ),
        ctx.width,
    ));
    if let Some(commitment) = &board.commitment {
        lines.extend(field(
            theme,
            "responsibility",
            &commitment_words(
                &proposed_actor_words(ctx, proposal, &commitment.agent_id),
                commitment,
            ),
            ctx.width,
        ));
    }
    lines.push(Line::default());
    if board.reason == proposal.rationale {
        lines.extend(paragraph(
            theme,
            "The reason it recorded is the one the proposal gave, above.",
            ctx.width,
        ));
    } else {
        lines.extend(paragraph(theme, &board.reason, ctx.width));
    }
    lines
}

/// Who holds a task and what their turns were committed to run with. The caller names the holder:
/// the plan as it stands is a roster, and a recorded decision is history.
fn commitment_words(holder: &str, commitment: &BoardCommitment) -> String {
    format!(
        "{holder} · {} · against task version {}",
        settings_words(&commitment.settings),
        text::short_id(&commitment.task_version)
    )
}

/// What the plan records about one task: its version, who holds it, what was put off.
fn board_task_lines(ctx: &Ctx, task_id: &str) -> Vec<Line<'static>> {
    let theme = ctx.theme;
    let Some(entry) = ctx.records.board_task(task_id) else {
        if let Some(error) = &ctx.records.board_unreadable {
            return field(
                theme,
                "the plan",
                &format!("could not be read: {}", text::one_line(error)),
                ctx.width,
            );
        }
        return Vec::new();
    };
    let mut lines = field(
        theme,
        "task version",
        &format!(
            "{} · changes when the task or its commitment changes",
            text::short_id(&entry.version)
        ),
        ctx.width,
    );
    lines.extend(field(
        theme,
        "responsibility",
        &match &entry.commitment {
            Some(commitment) => {
                commitment_words(&presented_name(ctx, &commitment.agent_id), commitment)
            }
            None => {
                "nobody holds this task; the runtime picks an executor at the next work boundary"
                    .to_owned()
            }
        },
        ctx.width,
    ));
    if let Some(commitment) = &entry.commitment {
        if let Some(proposal) = ctx
            .records
            .proposals()
            .iter()
            .find(|proposal| proposal.id == commitment.proposal_id)
        {
            lines.extend(field(
                theme,
                "committed by",
                &format!(
                    "proposal {} from {}",
                    text::short_id(&proposal.id),
                    record_words(&proposer_name(ctx, proposal), &proposal.agent_id)
                ),
                ctx.width,
            ));
        }
    }
    for (_, wait) in ctx.records.deferrals_for_task(task_id) {
        lines.extend(field(theme, "put off", &wait_words(wait), ctx.width));
    }
    lines
}

/// Who the plan holds responsible for a task, or who its latest attempt was given to.
///
/// These are two different facts and the row says which it has. A commitment is the plan's own
/// statement about the next turn; an assignee is what an attempt already carried.
fn responsibility_or_assignee(ctx: &Ctx, task: &Task, assignee: &str) -> String {
    match ctx
        .records
        .board_task(&task.id)
        .and_then(|entry| entry.commitment.as_ref())
    {
        Some(commitment) => presented_name(ctx, &commitment.agent_id),
        None => assignee.to_owned(),
    }
}

/// The plan's own rows, appended under the tasks they are about.
///
/// Proposals live on this page because they are changes to this plan and nothing else. A session
/// whose agents proposed nothing gains no heading, so the page does not grow a section that
/// explains an absence.
fn board_items(ctx: &Ctx, mut items: Vec<Item>) -> Vec<Item> {
    let theme = ctx.theme;
    let proposals = ctx.records.proposals();
    if let Some(error) = &ctx.records.board_unreadable {
        items.push(Item::table("the plan", &PLAN_COLUMNS));
        items.push(
            Item::row(
                "board-unreadable",
                vec![
                    Cell::text("the plan could not be read", theme.bad()),
                    Cell::text("unavailable", theme.bad()),
                ],
            )
            .with_detail(paragraph(
                theme,
                &format!(
                    "Reading the shared plan failed with: {}. Without it this page cannot say which task is committed to whom, which proposals exist, or what version the plan is at. The tasks above are what the session's own task records say.",
                    text::one_line(error)
                ),
                ctx.width,
            )),
        );
        return items;
    }
    if proposals.is_empty() {
        return items;
    }
    items.push(Item::table(
        "proposals to change this plan",
        &PROPOSAL_COLUMNS,
    ));
    for proposal in proposals {
        items.push(proposal_row(ctx, proposal));
    }
    items
}

/// A wait the runtime recorded, in its own words, with the code it recorded it under.
fn wait_words(wait: &WorkspaceWait) -> String {
    let reason = match wait.code.as_str() {
        "resource_conflict" => "another turn held access that conflicts with this one",
        "agent_busy" => "the agent already had an active assignment",
        "commitment_busy" => {
            "the agent responsible for it was already working, so it keeps the commitment"
        }
        "concurrency_limit" => "the run's own ceiling on active turns was occupied",
        "dependencies" => "a task it depends on was not accepted yet",
        _ => "the runtime recorded this code",
    };
    format!("{} · {reason}", wait.code)
}

/// What the records say about the access one turn held, and what it waited for.
///
/// Access here is what the execution backend actually enforces for the turn. A policy may
/// describe it as broader, never as narrower: the runtime refuses a policy that claims to
/// narrow what the backend can do. Nothing is inferred from the purpose of the turn.
fn access_lines(ctx: &Ctx, assignment: &AssignmentRecord) -> Vec<Line<'static>> {
    let theme = ctx.theme;
    let mut lines = Vec::new();
    match ctx.records.reservation(&assignment.id) {
        Some(reservation) => {
            let access = reservation.access;
            lines.extend(field(
                theme,
                "effective",
                &access_words(&access.effective_access),
                ctx.width,
            ));
            lines.extend(access_paths(theme, &access.effective_access, ctx.width));
            if access.backend_access != access.effective_access {
                lines.extend(field(
                    theme,
                    "backend enforces",
                    &access_words(&access.backend_access),
                    ctx.width,
                ));
                lines.extend(access_paths(theme, &access.backend_access, ctx.width));
            }
            lines.extend(field(
                theme,
                "enforced by",
                &format!("{} {}", access.backend.id, access.backend.version),
                ctx.width,
            ));
            lines.extend(field(
                theme,
                "coordinated by",
                &format!("{} {}", access.policy.id, access.policy.version),
                ctx.width,
            ));
            lines.extend(field(
                theme,
                "turn admitted",
                &match reservation.admitted {
                    Some(admitted) => text::clock(&admitted.created_at),
                    None => "no admission was recorded under this reservation".to_owned(),
                },
                ctx.width,
            ));
            lines.extend(field(
                theme,
                "held",
                &match reservation.released {
                    Some(released) => format!(
                        "from {} until {}",
                        text::clock(&reservation.acquired.created_at),
                        text::clock(&released.created_at)
                    ),
                    None => format!(
                        "from {}; no release was recorded",
                        text::clock(&reservation.acquired.created_at)
                    ),
                },
                ctx.width,
            ));
        }
        None => {
            lines.extend(field(
                theme,
                "effective",
                "no access record was read for this turn",
                ctx.width,
            ));
        }
    }
    let waits = ctx.records.waits_of(&assignment.agent_id);
    if waits.is_empty() {
        lines.extend(field(theme, "waited", "no wait was recorded", ctx.width));
    } else {
        for (_, wait) in &waits {
            lines.extend(field(theme, "waited", &wait_words(wait), ctx.width));
        }
    }
    lines.push(Line::default());
    lines.extend(paragraph(
        theme,
        "This is the access the execution backend enforces for the turn, as the run recorded it. A coordination policy may describe it as broader and never as narrower, so a turn recorded as writing the whole directory could write any file in it whatever its purpose was. Waits are recorded against the agent, so a wait listed here belongs to this agent and not necessarily to this turn.",
        ctx.width,
    ));
    lines
}

/// One setting, in the state the records actually leave it in.
///
/// The distinction this keeps is the whole point of the column: a value that was asked for
/// and never confirmed must not read like a value the installation used.
fn setting_state(requested: Option<&str>, sent: Option<&str>, reported: Option<&str>) -> String {
    match (requested, sent, reported) {
        (None, None, None) => "nothing requested; the installation used its own default".into(),
        // A value can be rewritten on its way to the backend: a permission mode becomes the
        // name that transport uses. Naming only what was sent would hide the request that
        // produced it, so both ends are named, and what was reported stays separate.
        (Some(requested), Some(sent), reported) if requested != sent => match reported {
            Some(reported) if reported == sent => {
                format!("{requested} requested · {sent} sent · the installation reported the same")
            }
            Some(reported) => {
                format!("{requested} requested · {sent} sent · the installation reported {reported}")
            }
            None => format!(
                "{requested} requested · {sent} sent · unconfirmed, the installation reported nothing"
            ),
        },
        (_, Some(sent), Some(reported)) if sent == reported => {
            format!("{sent} · the installation reported the same")
        }
        (_, Some(sent), Some(reported)) => {
            format!("{sent} sent · the installation reported {reported}")
        }
        (_, Some(sent), None) => {
            format!("{sent} sent · unconfirmed, the installation reported nothing")
        }
        (Some(requested), None, Some(reported)) => {
            format!("{requested} requested · the installation reported {reported}")
        }
        (Some(requested), None, None) => {
            format!("{requested} requested · nothing was sent or reported")
        }
        (None, None, Some(reported)) => {
            format!("nothing requested · the installation reported {reported}")
        }
    }
}

/// The values the configuration fixes for an agent, and where that statement came from.
fn pinned(ctx: &Ctx, agent: &str) -> (&'static str, ModelEffort) {
    match ctx.records.trace.as_ref().and_then(|t| t.policy.as_ref()) {
        Some(policy) => (
            "as this session captured it",
            policy
                .execution
                .get(agent)
                .cloned()
                .unwrap_or_default()
                .fixed,
        ),
        None => (
            "as the configuration stands now",
            ctx.config
                .execution
                .get(agent)
                .cloned()
                .unwrap_or_default()
                .fixed,
        ),
    }
}

fn pin_words(fixed: &ModelEffort, source: &str) -> String {
    match (fixed.model.as_deref(), fixed.effort.as_deref()) {
        (None, None) => format!("nothing is fixed {source}; the run may choose and change both"),
        (Some(model), None) => format!("model {model} only, {source}; effort is the run's choice"),
        (None, Some(effort)) => {
            format!("effort {effort} only, {source}; the model is the run's choice")
        }
        (Some(model), Some(effort)) => format!("model {model} and effort {effort} only, {source}"),
    }
}

fn task_attempt(ctx: &Ctx, assignment: &AssignmentRecord) -> String {
    match &assignment.task {
        Some(task) => {
            let title = ctx
                .tasks
                .iter()
                .find(|candidate| candidate.id == task.task_id)
                .map(|candidate| candidate.title.clone())
                .unwrap_or_else(|| text::short_id(&task.task_id));
            if task.attempt == 0 {
                format!("{title} · before the first attempt")
            } else {
                format!("{title} · attempt {}", task.attempt)
            }
        }
        None => "no task; this turn belongs to the session as a whole".into(),
    }
}

fn context_kind(kind: &ymp_core::ContextKind) -> &'static str {
    match kind {
        ymp_core::ContextKind::KnowledgeCorrection => "a correction to what was retained",
        ymp_core::ContextKind::Message => "message",
        ymp_core::ContextKind::Memory => "memory",
        ymp_core::ContextKind::Task => "task",
        ymp_core::ContextKind::Result => "result",
        ymp_core::ContextKind::Session => "session",
        ymp_core::ContextKind::Prompt => "prompt",
        ymp_core::ContextKind::ProfileInstructions => "instructions",
        ymp_core::ContextKind::NativeContinuation => "continued",
    }
}

fn context_detail(reference: &ymp_core::ContextReference) -> String {
    let mut text = text::short_id(&reference.id);
    if let Some(session) = &reference.session_id {
        text.push_str(&format!(" from session {}", text::short_id(session)));
    }
    match reference.included_chars {
        Some(chars) => text.push_str(&format!(" · {chars} characters included")),
        None => text.push_str(" · length not recorded"),
    }
    if reference.digest.is_some() {
        text.push_str(" · content hashed");
    }
    text
}

fn grant_detail(grant: &GrantRecord) -> String {
    let operations = grant
        .operations
        .iter()
        .map(|operation| operation_word(*operation))
        .collect::<Vec<_>>()
        .join(", ");
    match (&grant.revoked_at, &grant.revocation_reason) {
        (Some(at), Some(reason)) => format!("{operations} · revoked {at}: {reason}"),
        (Some(at), None) => format!("{operations} · revoked {at}"),
        (None, _) => format!("{operations} · issued {}", grant.issued_at),
    }
}

fn operation_word(operation: ymp_core::TeamOperation) -> &'static str {
    match operation {
        ymp_core::TeamOperation::TeamPost => "post to the team chat",
        ymp_core::TeamOperation::TeamRead => "read the team chat",
        ymp_core::TeamOperation::TasksList => "list tasks",
        ymp_core::TeamOperation::TaskPropose => "propose a task",
        ymp_core::TeamOperation::BoardRead => "read the shared board",
        ymp_core::TeamOperation::MemorySearch => "search memory",
        ymp_core::TeamOperation::MemoryPropose => "propose memory",
    }
}

fn invocation_usage(invocation: &InvocationRecord) -> String {
    let Some(usage) = &invocation.usage else {
        return "no count was reported for this turn".into();
    };
    let counts = &usage.counts;
    let known = [counts.input, counts.output]
        .into_iter()
        .flatten()
        .sum::<u64>();
    let mut text = match (counts.input, counts.output) {
        (None, None) => "reported without counts".to_owned(),
        _ => format!("{known} in and out"),
    };
    if usage.partial {
        text.push_str(" · partial, the turn could still add to it");
    }
    if !usage.finalized {
        text.push_str(" · not final");
    }
    if let Some(note) = &usage.note {
        text.push_str(&format!(" · {note}"));
    }
    text
}

// ---------------------------------------------------------------------------
// Decisions, confirmation and what credit requires
// ---------------------------------------------------------------------------

const WHAT_A_DECISION_IS: &[&str] = &[
    "Every choice that changed the session's state is appended here: a plan accepted, a candidate reviewed, a task accepted or rejected, competence credited. Records are appended, never edited and never removed, so a later change does not restate an earlier one.",
    "A record carries the reason its actor stated and the records it links. ymp stores no deliberation beyond that, so there is no hidden reasoning trace behind these rows and none is reconstructed here.",
    "An acceptance also carries a grade. Confirmed means the evidence the acceptance bound passed for every criterion it applies to. Unconfirmed means the result was accepted on an independent review alone. Unknown means the record predates grading, or the runtime did not classify it; it is not a quiet pass.",
    "Accepted work stays accepted. Where a file the result named has changed since, the acceptance is still shown, and so is the fact that what it was accepted against is no longer what is on disk.",
    "Competence credit is a record of its own, written only where confirmed evidence supported a single producer. Only a credited confirmed acceptance counts toward who a later run may pick, and an uncredited acceptance is not evidence of unreliability.",
];

fn decisions(ctx: &Ctx) -> Page {
    let records = ctx.records;
    let mut items = vec![about_row(
        ctx,
        "what a decision record is",
        "What a decision record is, and what a grade means",
        WHAT_A_DECISION_IS,
    )];
    let all = records.decisions();
    let confirmed = all
        .iter()
        .filter(|decision| records.describe(decision).confirmed())
        .count();
    if !all.is_empty() {
        items.push(Item::table("", &DECISION_COLUMNS));
    }
    for decision in all {
        items.push(decision_row(ctx, decision));
    }
    if all.is_empty() {
        items.clear();
    }
    Page {
        view: View::Decisions,
        title: View::Decisions.title().into(),
        subtitle: match (ctx.session, all.len()) {
            (None, _) => "No session is loaded".into(),
            (Some(_), 0) => "This session recorded no decisions".into(),
            (Some(_), total) => format!("{total} recorded · {confirmed} confirmed"),
        },
        items,
        empty: empty_records(
            ctx,
            "No decisions were recorded",
            "A session appends a decision whenever it accepts a plan, reviews a candidate, accepts or rejects a task, or credits competence. A session that recorded none made no such choice, or ran before decisions were written.",
        ),
        hints: vec![("Enter", "inspect"), ("Esc", "back")],
    }
}

fn decision_row(ctx: &Ctx, decision: &DecisionRecord) -> Item {
    let theme = ctx.theme;
    let acceptance = ctx.records.describe(decision);
    let (marker, style) = match bounded_outcome(decision) {
        Some(true) => (theme.markers.ok.to_owned(), theme.good()),
        Some(false) => (theme.markers.fail.to_owned(), theme.bad()),
        None if decision.links.workspace_wait.is_some() => {
            (theme.markers.paused.to_owned(), theme.warn())
        }
        None if decision.links.workspace_access.is_some()
            || decision.links.acceptance_contract.is_some() =>
        {
            (theme.markers.activity.to_owned(), theme.info())
        }
        None => decision_marker(&acceptance, theme),
    };
    // The actor is named by the turn this record links for it, never by its other turns.
    let actor = decision
        .actor
        .as_deref()
        .map(|id| decision_actor(ctx, decision, id))
        .unwrap_or_else(|| "the runtime".into());
    let mut detail = field(theme, "decision", &decision.id, ctx.width);
    detail.extend(field(
        theme,
        "kind",
        &decision_kind(&decision.kind),
        ctx.width,
    ));
    detail.extend(field(
        theme,
        "actor",
        &match decision.actor.as_deref() {
            Some(id) => record_words(&actor, id),
            None => actor.clone(),
        },
        ctx.width,
    ));
    detail.extend(field(theme, "recorded", &decision.created_at, ctx.width));
    let (outcome_word, outcome) = recorded_outcome(decision, &acceptance);
    detail.extend(field(theme, "outcome", &outcome, ctx.width));
    if acceptance.accepted == Some(true) {
        detail.extend(field(
            theme,
            "basis",
            &acceptance_basis(&acceptance),
            ctx.width,
        ));
        detail.extend(field(
            theme,
            "reviewed by",
            &reviewer_words(ctx, &acceptance),
            ctx.width,
        ));
    }
    if let Some(result) = acceptance.result {
        detail.extend(field(
            theme,
            "result",
            &format!(
                "{} · version {} · criteria version {}",
                text::short_id(&result.id),
                result.version,
                text::short_id(&result.criteria_version)
            ),
            ctx.width,
        ));
        detail.extend(field(
            theme,
            "files",
            &format!(
                "{} ({} named by the result)",
                acceptance.state.word(),
                result.artifacts.len()
            ),
            ctx.width,
        ));
    }
    if let Some(task) = &decision.links.task {
        detail.extend(field(
            theme,
            "task",
            &format!(
                "{} · attempt {}",
                ctx.tasks
                    .iter()
                    .find(|candidate| candidate.id == task.task_id)
                    .map(|candidate| candidate.title.clone())
                    .unwrap_or_else(|| text::short_id(&task.task_id)),
                task.attempt
            ),
            ctx.width,
        ));
    }
    if let Some(assignment) = &decision.links.assignment_id {
        detail.extend(field(
            theme,
            "assignment",
            &text::short_id(assignment),
            ctx.width,
        ));
    }
    if let Some(observation) = &decision.links.observation_id {
        detail.extend(field(
            theme,
            "credit",
            &format!("competence observation {}", text::short_id(observation)),
            ctx.width,
        ));
    }
    if let Some(check) = &decision.links.check {
        detail.extend(field(
            theme,
            "evidence",
            &format!(
                "{} · {} · checker {} {}",
                check.check_id,
                check_outcome_word(&check.outcome),
                check.checker.id,
                check.checker.version
            ),
            ctx.width,
        ));
    }
    if let Some(board) = decision.links.board.as_deref() {
        detail.extend(field(
            theme,
            "proposal",
            &format!(
                "{} · {}",
                text::short_id(&board.proposal.id),
                board_change_words(ctx, &board.proposal).0
            ),
            ctx.width,
        ));
        detail.extend(field(
            theme,
            "asked by",
            &record_words(
                &proposer_name(ctx, &board.proposal),
                &board.proposal.agent_id,
            ),
            ctx.width,
        ));
        detail.extend(field(
            theme,
            "plan",
            &format!(
                "proposed against {} · resulting {}",
                text::short_id(&board.proposal.plan_version),
                text::short_id(&board.resulting_plan_version)
            ),
            ctx.width,
        ));
        if let Some(commitment) = &board.commitment {
            detail.extend(field(
                theme,
                "responsibility",
                &commitment_words(
                    &proposed_actor_words(ctx, &board.proposal, &commitment.agent_id),
                    commitment,
                ),
                ctx.width,
            ));
        }
    }
    // The chain from a correction to the two entries it concerns, so the record says what it
    // replaced without sending the reader to the memory page for it.
    if let Some(correction) = decision.links.knowledge_correction.as_ref() {
        detail.extend(field(
            theme,
            "replaced",
            &format!(
                "retained entry {} at version {}",
                text::short_id(&correction.target.id),
                text::short_id(&correction.target.version)
            ),
            ctx.width,
        ));
        detail.extend(field(
            theme,
            "replacement",
            &format!(
                "retained entry {}",
                text::short_id(&correction.replacement_id)
            ),
            ctx.width,
        ));
        detail.extend(field(
            theme,
            "authorised by",
            &format!(
                "acceptance {} under trusted contract {}",
                text::short_id(&correction.acceptance_id),
                text::short_id(&correction.contract_id)
            ),
            ctx.width,
        ));
        detail.extend(field(
            theme,
            "corrected by",
            &format!(
                "policy {} version {}",
                correction.policy.id, correction.policy.version
            ),
            ctx.width,
        ));
    }
    if let Some(allocation) = decision.links.allocation.as_deref() {
        detail.extend(membership_detail(ctx, allocation));
    }
    if let Some(resource) = decision.links.resource_allocation.as_deref() {
        detail.extend(bound_detail(ctx, decision, resource));
    }
    if let Some(captured) = decision.links.acceptance_contract.as_ref() {
        detail.extend(contract_detail(ctx, captured));
    }
    if let Some(wait) = decision.links.workspace_wait.as_ref() {
        detail.extend(field(theme, "waited because", &wait_words(wait), ctx.width));
        detail.extend(field(
            theme,
            "holder",
            wait.holder.as_deref().unwrap_or("none was named"),
            ctx.width,
        ));
    }
    if let Some(access) = decision.links.workspace_access.as_ref() {
        detail.extend(field(
            theme,
            "reservation",
            &text::short_id(&access.reservation_id),
            ctx.width,
        ));
        detail.extend(field(
            theme,
            "directory",
            &access.directory.display().to_string(),
            ctx.width,
        ));
        detail.extend(field(
            theme,
            "effective",
            &access_words(&access.effective_access),
            ctx.width,
        ));
        detail.extend(access_paths(theme, &access.effective_access, ctx.width));
        if access.backend_access != access.effective_access {
            detail.extend(field(
                theme,
                "backend enforces",
                &access_words(&access.backend_access),
                ctx.width,
            ));
        }
        detail.extend(field(
            theme,
            "enforced by",
            &format!("{} {}", access.backend.id, access.backend.version),
            ctx.width,
        ));
    }
    detail.push(Line::default());
    detail.push(Line::from(Span::styled(
        if decision.actor.is_some() {
            "Reason the actor stated".to_owned()
        } else {
            "Reason the runtime recorded".to_owned()
        },
        theme.muted(),
    )));
    detail.extend(paragraph(theme, &decision.reason, ctx.width));
    // The row's word is read where the record's outcome is read, so the list cannot say that a
    // record has no outcome while the record it opens states one. Its colour is the marker's,
    // except that a reservation which ended is not news.
    let right_style = if decision.kind == "workspace_access_released" {
        theme.muted()
    } else {
        style
    };
    Item::row(
        text::short_id(&decision.id),
        vec![
            Cell::text(marker, style),
            Cell::text(decision_kind(&decision.kind), theme.text()),
            Cell::text(actor, theme.faint()),
            Cell::text(outcome_word, right_style),
            Cell::text(stamp(&decision.created_at), theme.faint())
                .sorted_by(SortKey::Text(decision.created_at.clone())),
        ],
    )
    .with_detail(detail)
}

const DECISION_COLUMNS: [Column; 5] = [
    Column::left(""),
    Column::left("KIND").flex(),
    Column::left("ACTOR"),
    Column::left("OUTCOME"),
    Column::left("RECORDED").hide(1),
];

/// The name of a decision's actor, from the assignment and invocation the decision links.
fn decision_actor(ctx: &Ctx, decision: &DecisionRecord, agent: &str) -> String {
    record_name(
        ctx,
        agent,
        decision.links.assignment_id.as_deref(),
        decision.links.invocation_id.as_deref(),
    )
}

/// What one membership decision proposed, and what it was decided at.
///
/// A membership decision links no turn of the actors it names, so none of them has a recorded
/// model here; each is named by its identifier beside that fact.
fn membership_detail(ctx: &Ctx, allocation: &AllocationDecision) -> Vec<Line<'static>> {
    let theme = ctx.theme;
    let unlinked = |id: &str| record_words(&record_name(ctx, id, None, None), id);
    let names = |ids: &[String]| {
        if ids.is_empty() {
            "none".to_owned()
        } else {
            ids.iter()
                .map(|id| unlinked(id))
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    let mut lines = field(
        theme,
        "members proposed",
        &names(&allocation.proposal.members),
        ctx.width,
    );
    lines.extend(field(
        theme,
        "was",
        &match &allocation.input.current {
            Some(state) => format!(
                "revision {} · {}",
                state.revision,
                names(&state.current_members)
            ),
            None => "no roster yet".to_owned(),
        },
        ctx.width,
    ));
    lines.extend(field(
        theme,
        "final reviewer kept free",
        &match &allocation.proposal.reserved_final_reviewer {
            Some(id) => unlinked(id),
            None => "none reserved".to_owned(),
        },
        ctx.width,
    ));
    lines.extend(field(
        theme,
        "decided at",
        boundary_word(allocation.input.boundary),
        ctx.width,
    ));
    lines.extend(field(
        theme,
        "method",
        &allocation.proposal.method,
        ctx.width,
    ));
    lines.extend(field(
        theme,
        "decided by",
        &format!(
            "{} version {}",
            allocation.implementation.id, allocation.implementation.version
        ),
        ctx.width,
    ));
    lines.push(Line::default());
    lines.extend(paragraph(
        theme,
        "A committed membership decision changes who a turn may be given to from here on. It does not revisit the turns already recorded, and the reserved reviewer it names is availability rather than authority.",
        ctx.width,
    ));
    lines
}

/// The outcome a record carries, read from the field that actually holds it: the word its row
/// shows, and the sentence its record states.
///
/// A membership change, a per-turn bound, a wait, a change to the shared plan, a correction to
/// what was retained, a captured contract and the three reservation records decide inside the
/// record they carry, and no grade is ever written for them. Reading the grade field would
/// report them as decisions recorded without an outcome, so the row and the record both read
/// from here and cannot say different things.
fn recorded_outcome(decision: &DecisionRecord, acceptance: &Acceptance) -> (String, String) {
    let pair = |word: &str, sentence: &str| (word.to_owned(), sentence.to_owned());
    if let Some(allocation) = decision.links.allocation.as_deref() {
        return if allocation.accepted {
            pair("membership committed", "the membership was committed")
        } else {
            pair("membership refused", "the membership was refused")
        };
    }
    if let Some(resource) = decision.links.resource_allocation.as_deref() {
        return if resource.accepted {
            pair("bound set", "the bound was set")
        } else {
            pair("bound refused", "the bound was refused")
        };
    }
    if let Some(wait) = decision.links.workspace_wait.as_ref() {
        return (
            format!("waited · {}", wait.code),
            format!("the turn waited · {}", wait.code),
        );
    }
    if let Some(board) = decision.links.board.as_deref() {
        return if board.accepted {
            pair("committed", "the plan took the change on")
        } else {
            pair("rejected", "the plan was left unchanged")
        };
    }
    if decision.links.knowledge_correction.is_some() {
        return pair("entry replaced", "what was retained was replaced");
    }
    if decision.links.acceptance_contract.is_some() {
        return pair(
            "criteria captured",
            "criteria were captured before the work",
        );
    }
    match decision.kind.as_str() {
        "workspace_access_acquired" => pair("directory reserved", "the directory was reserved"),
        "workspace_access_admitted" => pair(
            "turn admitted",
            "the turn was admitted under that reservation",
        ),
        "workspace_access_released" => pair("reservation ended", "the reservation ended"),
        "acceptance_contract_captured" => pair(
            "criteria captured",
            "criteria were captured before the work",
        ),
        _ => pair(acceptance.word(), acceptance.word()),
    }
}

/// Whether a decision carries its own outcome, for records the grade field was never
/// written for: membership, per-turn resource bounds, changes to the shared plan and
/// corrections to what was retained all decide inside their own record. A correction is
/// written only once it has been applied, so there is no refused one to mark.
fn bounded_outcome(decision: &DecisionRecord) -> Option<bool> {
    decision
        .links
        .allocation
        .as_deref()
        .map(|allocation| allocation.accepted)
        .or_else(|| {
            decision
                .links
                .resource_allocation
                .as_deref()
                .map(|resource| resource.accepted)
        })
        .or_else(|| decision.links.board.as_deref().map(|board| board.accepted))
        .or_else(|| decision.links.knowledge_correction.as_ref().map(|_| true))
}

/// What one turn was actually allowed to consume, and who decided it.
fn bound_detail(
    ctx: &Ctx,
    decision: &DecisionRecord,
    resource: &ymp_core::ResourceAllocationDecision,
) -> Vec<Line<'static>> {
    let theme = ctx.theme;
    let agent = &resource.input.agent_id;
    let mut lines = field(
        theme,
        "for",
        &record_words(&decision_actor(ctx, decision, agent), agent),
        ctx.width,
    );
    lines.extend(field(
        theme,
        "time allowed",
        &format!("{} second(s)", resource.proposal.timeout_secs),
        ctx.width,
    ));
    lines.extend(field(
        theme,
        "native turns allowed",
        &resource.proposal.native_max_turns.to_string(),
        ctx.width,
    ));
    lines.extend(field(
        theme,
        "output characters allowed",
        &resource.proposal.max_output_chars.to_string(),
        ctx.width,
    ));
    lines.extend(field(
        theme,
        "for the turn",
        &format!(
            "{} · {} work",
            resource.input.demand.purpose, resource.input.demand.difficulty
        ),
        ctx.width,
    ));
    lines.extend(field(
        theme,
        "decided by",
        &format!(
            "{} version {}",
            resource.implementation.id, resource.implementation.version
        ),
        ctx.width,
    ));
    lines.push(Line::default());
    lines.extend(paragraph(
        theme,
        "This is the bound the runtime set for one turn, before it ran. What the turn then used is recorded with the turn itself, and a bound is not a report that it was reached.",
        ctx.width,
    ));
    lines
}

/// The moment a membership decision was taken, in words the records define.
fn boundary_word(boundary: AllocationBoundary) -> &'static str {
    match boundary {
        AllocationBoundary::Startup => "the session started",
        AllocationBoundary::WorkReady => "work became ready",
        AllocationBoundary::ResultAvailable => "a result arrived",
        AllocationBoundary::CheckFailed => "a check failed",
        AllocationBoundary::GoalChanged => "the goal changed",
        AllocationBoundary::ParticipantUnavailable => "a participant became unavailable",
        AllocationBoundary::ResourcesChanged => "the resources changed",
        AllocationBoundary::Conversation => "the conversation asked for it",
    }
}

fn decision_marker(acceptance: &Acceptance, theme: &Theme) -> (String, Style) {
    let m = theme.markers;
    match (acceptance.accepted, acceptance.confirmed()) {
        (Some(true), true) => (m.ok.into(), theme.good()),
        (Some(true), false) => (m.notice.into(), theme.info()),
        (Some(false), _) => (m.fail.into(), theme.bad()),
        (None, _) => (m.idle.into(), theme.muted()),
    }
}

/// The runtime's own kinds, in words. An unknown kind is shown as recorded.
fn decision_kind(kind: &str) -> String {
    match kind {
        "plan_accepted" => "plan accepted".into(),
        "task_accepted" => "task accepted".into(),
        "task_rejected" => "task rejected".into(),
        "review" => "review".into(),
        "reputation_observed" => "competence credited".into(),
        "final_review_pending" => "final review required".into(),
        "result_invalidated" => "result invalidated".into(),
        "acceptance_contract_captured" => "acceptance criteria captured".into(),
        "workspace_access_acquired" => "directory reserved".into(),
        "workspace_access_admitted" => "turn admitted".into(),
        "workspace_access_released" => "reservation ended".into(),
        "assignment_waiting" => "a turn waited".into(),
        "board_committed" => "plan change committed".into(),
        "board_rejected" => "plan change rejected".into(),
        "task_claimed" => "task taken up".into(),
        "knowledge_superseded" => "retained knowledge corrected".into(),
        "allocation_deferred" => "membership left for later".into(),
        other => other.replace('_', " "),
    }
}

/// What the records say about one task's acceptance, for the task's own page.
fn acceptance_lines(ctx: &Ctx, task: &Task) -> Vec<Line<'static>> {
    let theme = ctx.theme;
    let Some(acceptance) = ctx.records.acceptance(&task.id) else {
        return paragraph(
            theme,
            match task.state {
                TaskState::Accepted => "This task is marked accepted and no acceptance decision was read for it. The state alone does not say who accepted it or against what evidence.",
                _ => "No acceptance decision was recorded for this task.",
            },
            ctx.width,
        );
    };
    let mut lines = field(theme, "outcome", acceptance.word(), ctx.width);
    lines.extend(field(
        theme,
        "decision",
        &format!(
            "{} recorded {}",
            text::short_id(&acceptance.decision.id),
            acceptance.decision.created_at
        ),
        ctx.width,
    ));
    lines.extend(field(
        theme,
        "basis",
        &acceptance_basis(&acceptance),
        ctx.width,
    ));
    lines.extend(field(
        theme,
        "reviewed by",
        &reviewer_words(ctx, &acceptance),
        ctx.width,
    ));
    if let Some(result) = acceptance.result {
        lines.extend(field(
            theme,
            "result",
            &format!(
                "version {} · criteria version {}",
                result.version,
                text::short_id(&result.criteria_version)
            ),
            ctx.width,
        ));
        lines.extend(field(theme, "files", acceptance.state.word(), ctx.width));
    }
    lines.extend(field(
        theme,
        "credit",
        &match ctx
            .records
            .credit_for(&task.id)
            .and_then(|credit| Some((credit, credit.actor.as_deref()?)))
        {
            Some((credit, agent)) => format!(
                "competence credited to {}",
                record_words(&decision_actor(ctx, credit, agent), agent)
            ),
            None => "no competence credit was recorded, which is not a judgement about the work"
                .to_owned(),
        },
        ctx.width,
    ));
    lines
}

/// The short tag an accepted row carries, so the grade is visible without opening it.
fn grade_word(acceptance: &Acceptance) -> &'static str {
    match acceptance.accepted {
        Some(true) if acceptance.confirmed() => "confirmed",
        Some(true) => match acceptance.confirmation {
            Some(ConfirmationStatus::Unconfirmed) => "unconfirmed",
            _ => "grade unknown",
        },
        Some(false) => "rejected",
        None => "no outcome",
    }
}

fn confirmation_word(status: ConfirmationStatus) -> &'static str {
    match status {
        ConfirmationStatus::Confirmed => {
            "confirmed: evidence passed for every criterion it applies to"
        }
        ConfirmationStatus::Unconfirmed => "unconfirmed: accepted on an independent review alone",
        ConfirmationStatus::Unknown => {
            "unknown: the record was written before grading, or was never classified"
        }
    }
}

fn confirmation_tag(status: ConfirmationStatus) -> &'static str {
    match status {
        ConfirmationStatus::Confirmed => "confirmed",
        ConfirmationStatus::Unconfirmed => "unconfirmed",
        ConfirmationStatus::Unknown => "grade unknown",
    }
}

fn acceptance_basis(acceptance: &Acceptance) -> String {
    match (acceptance.confirmation, acceptance.evidence) {
        (Some(ConfirmationStatus::Confirmed), count) => format!(
            "{count} piece(s) of passing evidence covering every applicable criterion"
        ),
        (Some(ConfirmationStatus::Unconfirmed), 0) => {
            "an independent review, with no applicable check evidence".into()
        }
        (Some(ConfirmationStatus::Unconfirmed), count) => format!(
            "an independent review and {count} piece(s) of evidence that do not cover every criterion"
        ),
        (Some(ConfirmationStatus::Unknown), _) | (None, _) => {
            "not classified by the runtime that wrote this record".into()
        }
    }
}

/// Who reviewed an accepted result, each named by the turn its own review decision links.
fn reviewer_words(ctx: &Ctx, acceptance: &Acceptance) -> String {
    let reviewers = acceptance
        .decision
        .links
        .review_ids
        .iter()
        .filter_map(|id| ctx.records.decision(id))
        .filter_map(|review| {
            let actor = review.actor.as_deref()?;
            Some(record_words(&decision_actor(ctx, review, actor), actor))
        })
        .collect::<Vec<_>>();
    if reviewers.is_empty() {
        return "no review decision is linked to this acceptance".into();
    }
    reviewers.join(", ")
}

fn check_outcome_word(outcome: &ymp_core::ConfirmationCheckOutcome) -> &'static str {
    match outcome {
        ymp_core::ConfirmationCheckOutcome::Passed => "passed",
        ymp_core::ConfirmationCheckOutcome::Failed => "failed",
        ymp_core::ConfirmationCheckOutcome::Inconclusive => "inconclusive",
    }
}

// ---------------------------------------------------------------------------
// Shared rows for pages built from records
// ---------------------------------------------------------------------------

/// The leading row every record page opens with: what the page is, in its own words.
/// Key of a row that explains its page rather than naming a thing on it. An action that
/// changes something must not apply to it: there is nothing there to change.
pub const ABOUT_KEY: &str = "about this page";

fn about_row(ctx: &Ctx, key: &str, title: &str, sentences: &[&str]) -> Item {
    let theme = ctx.theme;
    let mut detail = Vec::new();
    for (index, sentence) in sentences.iter().enumerate() {
        if index > 0 {
            detail.push(Line::default());
        }
        detail.extend(paragraph(theme, sentence, ctx.width));
    }
    if !ctx.records.read_at.is_empty() {
        detail.push(Line::default());
        detail.extend(paragraph(
            theme,
            &match &ctx.records.session {
                Some(session) => format!(
                    "These records were read for session {} at {}. A run still working writes more of them; this page shows what that read found.",
                    text::short_id(session),
                    ctx.records.read_at
                ),
                None => format!(
                    "No session was open at {}, so no records were read. What is shown describes the next run instead.",
                    ctx.records.read_at
                ),
            },
            ctx.width,
        ));
    }
    Item::note(
        key.to_owned(),
        vec![
            Span::styled(format!("{} ", theme.markers.notice), theme.info()),
            Span::styled(title.to_owned(), theme.text()),
            Span::styled("  what this page is".to_owned(), theme.muted()),
        ],
    )
    .with_detail(detail)
}

/// The empty state of a record page, which must not report an unopened session as one that
/// recorded nothing.
fn empty_records(ctx: &Ctx, headline: &str, hint: &str) -> Vec<Line<'static>> {
    let theme = ctx.theme;
    if let Some(error) = &ctx.records.unreadable {
        let mut lines = nothing(
            theme,
            "These records could not be read",
            "The session exists; reading its records failed, which is not the same as a session without any.",
            ctx.width,
        );
        lines.push(Line::default());
        lines.extend(paragraph(theme, error, ctx.width));
        return lines;
    }
    if ctx.session.is_none() {
        return nothing(
            theme,
            "Nothing was read",
            "No session is loaded, so no records were read. Open one with /sessions.",
            ctx.width,
        );
    }
    nothing(theme, headline, hint, ctx.width)
}

/// The name to present for one actor where no single invocation is meant.
///
/// The shared rule in [`label::agent`] decides it. What this session recorded for the actor
/// speaks first, so a later reading of the installations never renames finished work, and only
/// an actor the records do not mention is named from the pool. A caption, a configured name and
/// a provider or actor identifier are never the name; a model nothing recorded stays unknown.
fn presented_name(ctx: &Ctx, id: &str) -> String {
    actor_name(ctx.config, ctx.pool, ctx.records, id)
}

fn actor_name(config: &Config, pool: &Pool, records: &Records, id: &str) -> String {
    let pooled = pool.agent(id).map(|agent| &agent.identity);
    label::agent(id, records.trace.as_ref(), pooled, config)
}

/// The name a record gives one of its actors: the model and reported effort of the turn that
/// record itself links for that actor, or `unknown model`.
///
/// A record is history. The actor's other turns, earlier or later, say nothing about which model
/// ran the turn behind this record, so neither its latest turn nor a model all its turns share is
/// used in its place, and nothing is matched by time. A linked assignment that belongs to another
/// actor is not this actor's turn and names nothing for it.
fn record_name(
    ctx: &Ctx,
    agent: &str,
    assignment: Option<&str>,
    invocation: Option<&str>,
) -> String {
    let records = ctx.records;
    let Some(assignment) = assignment.and_then(|id| {
        records
            .assignments()
            .iter()
            .find(|assignment| assignment.id == id && assignment.agent_id == agent)
    }) else {
        return label::UNKNOWN_MODEL.to_owned();
    };
    let turns = records.invocations_of(&assignment.id);
    let turn = match invocation {
        // An exact reference binds to that invocation or to nothing: one that names no call of
        // this assignment is not evidence about another call.
        Some(id) => match turns.iter().find(|turn| turn.id == id) {
            Some(turn) => Some(*turn),
            None => return label::UNKNOWN_MODEL.to_owned(),
        },
        // Without one, a call is used only where the assignment made exactly one. Otherwise the
        // label rests on what the assignment captured, and no call lends it a reported effort.
        None => match turns.as_slice() {
            [only] => Some(*only),
            _ => None,
        },
    };
    label::assignment(assignment, turn)
}

/// The same actor in a record's details, where its identifier stands beside the name.
fn record_words(name: &str, agent: &str) -> String {
    if name == agent {
        name.to_owned()
    } else {
        format!("{name} · {agent}")
    }
}

/// The name of the actor who asked for a change to the plan, from the turn that asked.
fn proposer_name(ctx: &Ctx, proposal: &BoardProposal) -> String {
    record_name(
        ctx,
        &proposal.agent_id,
        Some(&proposal.assignment_id),
        Some(&proposal.invocation_id),
    )
}

/// An actor a proposal names besides its author. Only the author's own turn is linked to the
/// proposal, so any other actor it names has no recorded model here.
fn proposed_actor_words(ctx: &Ctx, proposal: &BoardProposal, agent: &str) -> String {
    let name = if agent == proposal.agent_id {
        proposer_name(ctx, proposal)
    } else {
        record_name(ctx, agent, None, None)
    };
    record_words(&name, agent)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_recorded_as_unknown_sorts_as_unknown_and_none_stays_a_value() {
        let style = Style::default();
        assert_eq!(value_cell("300 s", style).sort, SortKey::Number(300));
        assert_eq!(value_cell("unknown", style).sort, SortKey::Unknown);
        assert_eq!(
            value_cell("unknown over 3 turn(s)", style).sort,
            SortKey::Unknown
        );
        assert_eq!(value_cell("none", style).sort, SortKey::Text("none".into()));
        assert_eq!(
            value_cell("unknown over 3 turn(s)", style).plain(),
            "unknown over 3 turn(s)"
        );
    }

    fn painted(lines: &[Line<'static>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn every_value_of_a_record_starts_in_one_column() {
        let theme = crate::theme::resolved(crate::theme::DEFAULT_THEME, crate::theme::UNICODE);
        let mut lines = field(&theme, "state", "accepted", 60);
        lines.extend(field(&theme, "declared access", "may write", 60));
        lines.extend(field(&theme, "final reviewer kept free", "yes", 60));
        lines.extend(field(&theme, "chosen in the picker as", "", 60));
        assert_eq!(
            painted(&lines),
            [
                format!("{:<18}accepted", "state"),
                format!("{:<18}may write", "declared access"),
                "final reviewer kept free".to_owned(),
                format!("{:<18}yes", ""),
                "chosen in the picker as".to_owned(),
            ]
        );
    }

    #[test]
    fn a_narrow_record_keeps_its_lines_inside_the_width() {
        let theme = crate::theme::resolved(crate::theme::DEFAULT_THEME, crate::theme::UNICODE);
        let rows = painted(&field(
            &theme,
            "responsibility",
            "nobody holds this task; the runtime picks an executor at the next work boundary",
            24,
        ));
        assert_eq!(rows[0], "responsibility");
        assert!(
            rows[1..].iter().all(|row| row.starts_with(&" ".repeat(10))),
            "{rows:#?}"
        );
        assert!(rows.iter().all(|row| text::width(row) <= 24), "{rows:#?}");
    }
}
