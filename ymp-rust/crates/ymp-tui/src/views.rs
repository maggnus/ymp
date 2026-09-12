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
use crate::provenance::{Acceptance, Pool, Records};
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
use ymp_workspace::repository::Repository;

/// Every destination the sidebar and the commands can reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Chat,
    Tasks,
    Usage,
    Sessions,
    Files,
    Changes,
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

/// Navigation order, used by the sidebar and by the help page.
pub const NAV: &[View] = &[
    View::Chat,
    View::Tasks,
    View::Usage,
    View::Sessions,
    View::Files,
    View::Changes,
    View::Checks,
    View::Assignments,
    View::Decisions,
    View::Team,
    View::Agents,
    View::Providers,
    View::Memory,
    View::Reputation,
    View::Limits,
    View::Help,
];

impl View {
    pub fn title(self) -> &'static str {
        match self {
            View::Chat => "Conversation",
            View::Tasks => "Tasks",
            View::Usage => "Token usage",
            View::Sessions => "Sessions",
            View::Files => "Files",
            View::Changes => "Changed files",
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
}

/// A row on a page. Headings are shown but never selected.
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
    pub left: Vec<Span<'static>>,
    pub right: Vec<Span<'static>>,
    pub detail: Vec<Line<'static>>,
}

impl Item {
    pub fn row(key: impl Into<String>, left: Vec<Span<'static>>) -> Self {
        Self {
            kind: ItemKind::Row,
            key: key.into(),
            left,
            right: Vec::new(),
            detail: Vec::new(),
        }
    }
    pub fn with_right(mut self, right: Vec<Span<'static>>) -> Self {
        self.right = right;
        self
    }
    pub fn with_detail(mut self, detail: Vec<Line<'static>>) -> Self {
        self.detail = detail;
        self
    }
    pub fn heading(text: impl Into<String>, theme: &Theme) -> Self {
        Self {
            kind: ItemKind::Heading,
            key: String::new(),
            left: vec![Span::styled(text.into().to_uppercase(), theme.muted())],
            right: Vec::new(),
            detail: Vec::new(),
        }
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

fn field(theme: &Theme, label: &str, value: &str, width: usize) -> Vec<Line<'static>> {
    // A label longer than the usual column pushes its value right, so the value is wrapped to
    // the room actually left beside it rather than running past the width.
    let label_width = 14usize.max(text::width(label)).min(width.saturating_sub(4));
    let room = width.saturating_sub(label_width + 1).max(8);
    let mut lines = Vec::new();
    for (index, piece) in text::wrap(&text::sanitize(value), room)
        .into_iter()
        .enumerate()
    {
        let head = if index == 0 {
            format!("{label:<label_width$} ")
        } else {
            " ".repeat(label_width + 1)
        };
        lines.push(Line::from(vec![
            Span::styled(head, theme.muted()),
            Span::styled(piece, theme.body()),
        ]));
    }
    lines
}

fn paragraph(theme: &Theme, body: &str, width: usize) -> Vec<Line<'static>> {
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

fn help(ctx: &Ctx) -> Page {
    let theme = ctx.theme;
    let mut items = Vec::new();
    for group in Group::all() {
        items.push(Item::heading(group.title(), theme));
        for command in commands::COMMANDS.iter().filter(|c| c.group == *group) {
            let mut detail = field(theme, "usage", command.usage, ctx.width);
            detail.push(Line::default());
            detail.extend(paragraph(theme, command.summary, ctx.width));
            items.push(
                Item::row(
                    command.name,
                    vec![Span::styled(command.name.to_owned(), theme.accent())],
                )
                .with_right(vec![Span::styled(
                    text::truncate(command.summary, ctx.width / 2),
                    theme.muted(),
                )])
                .with_detail(detail),
            );
        }
    }
    items.push(Item::heading("Keyboard", theme));
    for (key, description) in commands::KEYS {
        items.push(
            Item::row(*key, vec![Span::styled((*key).to_owned(), theme.accent())])
                .with_right(vec![Span::styled((*description).to_owned(), theme.muted())])
                .with_detail(paragraph(theme, description, ctx.width)),
        );
    }
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

fn tasks(ctx: &Ctx) -> Page {
    let theme = ctx.theme;
    let items = ctx
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
            let grade = ctx.records.acceptance(&task.id);
            Item::row(
                task.id.clone(),
                vec![
                    Span::styled(format!("{marker} "), style),
                    Span::styled(task.title.clone(), theme.text()),
                ],
            )
            .with_right(vec![
                Span::styled(format!("{word}  "), style),
                Span::styled(
                    match &grade {
                        Some(acceptance) => format!("{}  ", grade_word(acceptance)),
                        None => String::new(),
                    },
                    match &grade {
                        Some(acceptance) if acceptance.confirmed() => theme.good(),
                        Some(_) => theme.info(),
                        None => theme.muted(),
                    },
                ),
                Span::styled(
                    responsibility_or_assignee(ctx, task, &assignee),
                    theme.faint(),
                ),
            ])
            .with_detail(detail)
        })
        .collect::<Vec<_>>();
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
        items.push(Item::heading("Session", theme));
        items.push(
            Item::row(
                "session",
                vec![Span::styled("session total".to_owned(), theme.text())],
            )
            .with_right(vec![Span::styled(
                usage::headline(total, theme),
                usage::headline_style(total, theme),
            )])
            .with_detail(detail),
        );

        let rows = usage::agent_rows(ctx.stats, ctx.team, ctx.config);
        let mut heading = false;
        for row in rows.iter().filter(|row| !row.outside_team) {
            if !std::mem::replace(&mut heading, true) {
                items.push(Item::heading("Agents", theme));
            }
            items.push(agent_usage(ctx, row));
        }
        let mut heading = false;
        for row in rows.iter().filter(|row| row.outside_team) {
            if !std::mem::replace(&mut heading, true) {
                items.push(Item::heading("Recorded outside the captured team", theme));
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
    let mut left = vec![Span::styled(row.name.clone(), theme.text())];
    if !row.provider.is_empty() {
        left.push(Span::styled(format!(" · {}", row.provider), theme.faint()));
    }
    Item::row(row.id.clone(), left)
        .with_right(vec![Span::styled(
            usage::agent_headline(row, theme),
            usage::agent_headline_style(row, theme),
        )])
        .with_detail(detail)
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

fn sessions(ctx: &Ctx) -> anyhow::Result<Page> {
    let theme = ctx.theme;
    let project = ctx.store.project(ctx.cwd)?;
    let sessions: Vec<Session> = ctx.store.sessions(Some(&project.id))?;
    let items = sessions
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
            detail.extend(field(
                theme,
                "team",
                &session
                    .team
                    .iter()
                    .map(|a| a.name.clone())
                    .collect::<Vec<_>>()
                    .join(", "),
                ctx.width,
            ));
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
            let mut left = vec![Span::styled(
                format!("{} ", text::short_id(&session.id)),
                theme.faint(),
            )];
            left.push(Span::styled(
                text::one_line(&session.title),
                if current { theme.bold() } else { theme.text() },
            ));
            if current {
                left.push(Span::styled(" · loaded".to_owned(), theme.accent()));
            }
            Item::row(session.id.clone(), left)
                .with_right(vec![Span::styled(status, style)])
                .with_detail(detail)
        })
        .collect::<Vec<_>>();
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

fn files(ctx: &Ctx) -> anyhow::Result<Page> {
    let theme = ctx.theme;
    const SKIP: &[&str] = &[".git", "node_modules", "target", "__pycache__", ".ymp2"];
    let mut entries: Vec<(bool, String, std::fs::Metadata)> = Vec::new();
    for entry in std::fs::read_dir(ctx.cwd)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if SKIP.contains(&name.as_str()) {
            continue;
        }
        let metadata = entry.metadata()?;
        entries.push((metadata.is_dir(), name, metadata));
    }
    entries.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    let items = entries
        .iter()
        .map(|(is_dir, name, metadata)| {
            let path = ctx.cwd.join(name);
            let mut detail = field(theme, "path", &path.display().to_string(), ctx.width);
            detail.extend(field(
                theme,
                "kind",
                if *is_dir { "directory" } else { "file" },
                ctx.width,
            ));
            if !*is_dir {
                detail.extend(field(theme, "size", &size(metadata.len()), ctx.width));
            }
            Item::row(
                path.display().to_string(),
                vec![Span::styled(
                    if *is_dir {
                        format!("{name}/")
                    } else {
                        name.clone()
                    },
                    if *is_dir {
                        theme.accent()
                    } else {
                        theme.text()
                    },
                )],
            )
            .with_right(vec![Span::styled(
                if *is_dir {
                    "directory".to_owned()
                } else {
                    size(metadata.len())
                },
                theme.faint(),
            )])
            .with_detail(detail)
        })
        .collect::<Vec<_>>();
    Ok(Page {
        view: View::Files,
        title: View::Files.title().into(),
        subtitle: ctx.cwd.display().to_string(),
        items,
        empty: nothing(
            theme,
            "The working directory is empty",
            "Agents write here directly. Nothing is copied into the ymp home directory.",
            ctx.width,
        ),
        hints: vec![("Enter", "show the full path"), ("Esc", "back")],
    })
}

fn size(bytes: u64) -> String {
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
                    Item::row(path.clone(), vec![Span::styled(path.clone(), theme.text())])
                        .with_right(vec![Span::styled(status, style)])
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
        items.push(Item::heading("Recorded changes", theme));
        items.append(&mut rows);
    }
    if !outcomes.is_empty() {
        items.push(Item::heading("Where accepted work was recorded", theme));
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
        hints: vec![("Enter", "show the full path"), ("Esc", "back")],
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
    Item::row(
        "recovery",
        vec![
            Span::styled(format!("{} ", theme.markers.notice), theme.info()),
            Span::styled(
                "How this directory is used, and what cannot be put back".to_owned(),
                theme.text(),
            ),
        ],
    )
    .with_right(vec![Span::styled(
        "direct · metadata only".to_owned(),
        theme.muted(),
    )])
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
            Span::styled(
                format!(
                    "{} ",
                    if outcome.current {
                        theme.markers.ok
                    } else {
                        theme.markers.warn
                    }
                ),
                style,
            ),
            Span::styled(
                text::truncate(&outcome.summary, ctx.width.saturating_sub(28)),
                theme.text(),
            ),
        ],
    )
    .with_right(vec![Span::styled(word, style)])
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
            Span::styled(format!("{} ", theme.markers.warn), theme.warn()),
            Span::styled("recorded locations".to_owned(), theme.text()),
        ],
    )
    .with_right(vec![Span::styled(
        "could not be read".to_owned(),
        theme.warn(),
    )])
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
        items.push(Item::heading("Recorded runs", theme));
        items.extend(recorded.iter().map(|run| recorded_check(ctx, run)));
    }
    if !planned.is_empty() {
        items.push(Item::heading("Declared, without a recorded run", theme));
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
        hints: vec![("Enter", "show the recorded run"), ("Esc", "back")],
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
    Item::row(
        "how checks run",
        vec![
            Span::styled(format!("{} ", theme.markers.notice), theme.info()),
            Span::styled("How checks run".to_owned(), theme.text()),
        ],
    )
    .with_right(vec![Span::styled(
        "read from the session log".to_owned(),
        theme.muted(),
    )])
    .with_detail(detail)
}

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
            Span::styled(format!("{marker} "), style),
            Span::styled(
                text::truncate(&command, ctx.width.saturating_sub(24)),
                theme.text(),
            ),
        ],
    )
    .with_right(vec![Span::styled(word.to_owned(), style)])
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
            Span::styled(format!("{} ", theme.markers.idle), theme.faint()),
            Span::styled(
                text::truncate(command, ctx.width.saturating_sub(24)),
                theme.muted(),
            ),
        ],
    )
    .with_right(vec![Span::styled(
        "no recorded run".to_owned(),
        theme.faint(),
    )])
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
    items.push(Item::heading(
        if ctx.team_captured {
            "Members of this session"
        } else {
            "Members of the next run"
        },
        theme,
    ));
    for profile in &current {
        items.push(member_row(ctx, profile));
    }
    if !replaced.is_empty() {
        items.push(Item::heading("Captured here, no longer a member", theme));
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
        items.push(Item::heading("Worked here, not in this list", theme));
        for id in &recorded_only {
            items.push(aside_row(ctx, id, Aside::RecordsOnly));
        }
    }
    items.push(Item::heading("How the roster is bounded", theme));
    items.push(roster_row(ctx));
    items.push(roster_rules_row(ctx));
    items.push(Item::heading("Available on this machine", theme));
    if ctx.pool.agents().is_empty() {
        items.push(pool_unavailable_row(ctx));
    } else {
        for agent in ctx.pool.agents() {
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
            ("Space", "add or remove"),
            ("Enter", "open the record"),
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
            match invocation
                .reported
                .model
                .as_deref()
                .or(invocation.sent.model.as_deref())
            {
                Some(model) => {
                    if !names.iter().any(|seen| seen == model) {
                        names.push(model.to_owned());
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
/// What is stored for a provider: a reading from the installation, a claim the configuration
/// wrote, or nothing, with the last attempt and any bounded failure the scan recorded.
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
            None => format!("read from the installation by {method} at {observed_at}"),
            Some(failure) => format!(
                "the attempt at {} ended as {failure}; the reading by {method} at {observed_at} is kept",
                snapshot.last_attempt
            ),
        },
        (Some(snapshot), _) => match snapshot.failure.as_deref() {
            Some(failure) => format!(
                "not read · the attempt at {} ended as {failure}",
                snapshot.last_attempt
            ),
            None => format!("attempted at {}, with nothing stored", snapshot.last_attempt),
        },
        (None, Some(CapabilitySource::Configured)) => {
            "written by configuration; no reading from the installation stands behind it".to_owned()
        }
        (None, Some(CapabilitySource::NativeMetadata { .. })) => {
            "a claim of a native reading that no stored scan matches".to_owned()
        }
        (None, None) => "nothing stored; this provider's own offerings have not been read".to_owned(),
    }
}

/// What the catalog lists, in the installation's own names, and whether it claims to be whole.
fn catalog_models_words(catalog: Option<&ProviderCapabilities>) -> String {
    match catalog {
        None => "none listed".to_owned(),
        Some(catalog) if catalog.models.is_empty() && catalog.models_complete => {
            "none, and the list is complete: this installation offers nothing".to_owned()
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
                "the whole list"
            } else {
                "not known to be the whole list"
            }
        ),
    }
}

/// The label a captured member carries: the native name the record captured at admission, or
/// the name the session captured with the profile.
fn captured_label(ctx: &Ctx, profile: &AgentProfile) -> String {
    captured_identity(ctx, &profile.id)
        .map(|identity| identity.name.clone())
        .unwrap_or_else(|| profile.name.clone())
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
        lines.extend(field(theme, "name", &identity.name, ctx.width));
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
                identity
                    .model
                    .as_deref()
                    .unwrap_or("no model was recorded with it")
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
    if let Some(id) = captured_identity(ctx, &profile.id).and_then(|i| i.model.as_deref()) {
        return id.to_owned();
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

/// The label a row carries for an agent, and the short words beside it.
///
/// A provider id is a transport label: `codex` is how ymp reaches an installation, not the name
/// of a model it offers. So a native name, where the installation gave one, is the label; where
/// it did not, the label is the configured one and the words beside it say exactly what is
/// missing. Nothing here derives a name from an identifier.
fn identity_row_words(
    identity: Option<&AgentIdentity>,
    profile: &AgentProfile,
) -> (String, String) {
    let Some(identity) = identity else {
        return (
            profile.name.clone(),
            "what is installed could not be read".to_owned(),
        );
    };
    match identity.status {
        AgentIdentityStatus::Native => (
            identity.name.clone(),
            identity
                .model
                .clone()
                .unwrap_or_else(|| "no identifier recorded".to_owned()),
        ),
        AgentIdentityStatus::Stale => (identity.name.clone(), "not read recently".to_owned()),
        AgentIdentityStatus::Unknown => (
            identity.configured_name.clone(),
            "not in the catalog".to_owned(),
        ),
        AgentIdentityStatus::Unresolved => (
            identity.configured_name.clone(),
            "no native model".to_owned(),
        ),
        AgentIdentityStatus::Local => (identity.name.clone(), "a local provider".to_owned()),
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
    let mut lines = field(theme, "name", &identity.name, ctx.width);
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
        "model",
        &match identity.model.as_deref() {
            Some(id) => id.to_owned(),
            None => "none is set, so the installation would choose".to_owned(),
        },
        ctx.width,
    ));
    if let Some(resolved) = identity.resolved_model.as_deref() {
        lines.extend(field(theme, "resolved to", resolved, ctx.width));
    }
    lines.extend(field(theme, "name from", &name_source(identity), ctx.width));
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
        ) => format!("the installation, read by {method} at {observed_at}"),
        (
            AgentIdentityStatus::Stale,
            Some(CapabilitySource::NativeMetadata {
                method,
                observed_at,
            }),
        ) => format!(
            "the installation, read by {method} at {observed_at}, which is no longer current"
        ),
        (AgentIdentityStatus::Native | AgentIdentityStatus::Stale, _) => {
            "a stored reading whose source was not recorded".to_owned()
        }
        (AgentIdentityStatus::Unknown, _) => {
            "the configuration · no stored catalog lists this model".to_owned()
        }
        (AgentIdentityStatus::Unresolved, _) => {
            "nowhere yet · no model is set and no stored catalog names a default".to_owned()
        }
        (AgentIdentityStatus::Local, _) => {
            "the configuration · a local provider has no native identity".to_owned()
        }
    }
}

/// The controls an offering advertises, in the installation's own words.
fn control_lines(theme: &Theme, offering: &ModelCapabilities, width: usize) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    match offering.controls.as_deref() {
        None => lines.extend(field(
            theme,
            "controls",
            "unknown · the catalog does not say which it offers",
            width,
        )),
        Some([]) => lines.extend(field(
            theme,
            "controls",
            "none · the catalog says it offers none",
            width,
        )),
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
        None => format!("{values} · no default is named"),
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
        (format!("{turns} turn(s) here"), theme.good())
    } else if ctx.team_captured {
        ("no turn recorded".to_owned(), theme.muted())
    } else {
        match eligible {
            Some(agent) if agent.exclusions.is_empty() => ("eligible".to_owned(), theme.good()),
            Some(agent) => (exclusion_word(agent.exclusions[0]).to_owned(), theme.warn()),
            None => ("not in the pool".to_owned(), theme.warn()),
        }
    };
    let identity = identity_of(ctx, profile);
    let (label, words) = identity_row_words(identity, profile);
    let mut detail = field(theme, "profile", &profile.id, ctx.width);
    // A catalog read today says nothing about a turn that ran yesterday, so a captured member
    // is read from what its own session recorded and never from the catalog as it stands now.
    let (label, words) = if ctx.team_captured {
        detail.extend(captured_identity_lines(ctx, profile));
        (
            captured_label(ctx, profile),
            captured_model_row_words(ctx, profile),
        )
    } else {
        detail.extend(identity_lines(ctx, profile));
        (label, words)
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
            Span::styled(
                format!(
                    "{} ",
                    if open > 0 {
                        theme.markers.busy
                    } else if turns > 0 {
                        theme.markers.ok
                    } else {
                        theme.markers.idle
                    }
                ),
                style,
            ),
            Span::styled(label, theme.text()),
            Span::styled(format!(" · {words}"), theme.muted()),
            Span::styled(format!(" · {}", profile.provider), theme.faint()),
        ],
    )
    .with_right(vec![Span::styled(right, style)])
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
        Aside::RecordsOnly => (format!("{turns} turn(s) recorded here"), theme.info()),
    };
    Item::row(
        id.to_owned(),
        vec![
            Span::styled(
                format!(
                    "{} ",
                    match aside {
                        Aside::Replaced => theme.markers.notice,
                        Aside::RecordsOnly => theme.markers.activity,
                    }
                ),
                style,
            ),
            Span::styled(presented_name(ctx, id), theme.text()),
        ],
    )
    .with_right(vec![Span::styled(right, style)])
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
                Span::styled(format!("{} ", theme.markers.idle), theme.faint()),
                Span::styled("roster".to_owned(), theme.muted()),
            ],
        )
        .with_right(vec![Span::styled(
            "none recorded".to_owned(),
            theme.faint(),
        )])
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
            Span::styled(format!("{} ", theme.markers.ok), theme.good()),
            Span::styled("roster".to_owned(), theme.text()),
        ],
    )
    .with_right(vec![Span::styled(
        format!(
            "{} member(s) · revision {}",
            state.current_members.len(),
            state.revision
        ),
        theme.muted(),
    )])
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
            Span::styled(format!("{} ", theme.markers.bullet), theme.faint()),
            Span::styled("roster rules".to_owned(), theme.text()),
        ],
    )
    .with_right(vec![Span::styled(size, theme.muted())])
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
    Item::row(
        agent.profile.id.clone(),
        vec![
            Span::styled(
                format!(
                    "{} ",
                    if eligible {
                        theme.markers.ok
                    } else {
                        theme.markers.warn
                    }
                ),
                if eligible { theme.good() } else { theme.warn() },
            ),
            Span::styled(agent.profile.name.clone(), theme.muted()),
            Span::styled(format!(" · {}", agent.profile.provider), theme.faint()),
        ],
    )
    .with_right(vec![Span::styled(
        if eligible {
            "eligible".to_owned()
        } else {
            exclusion_word(agent.exclusions[0]).to_owned()
        },
        if eligible { theme.good() } else { theme.warn() },
    )])
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
            Span::styled(format!("{} ", theme.markers.warn), theme.warn()),
            Span::styled("Nothing was inspected".to_owned(), theme.text()),
        ],
    )
    .with_right(vec![Span::styled("no pool read".to_owned(), theme.muted())])
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
        PoolExclusion::AgentDisabled => "the profile is disabled",
        PoolExclusion::ProviderDisabled => "its provider is disabled",
        PoolExclusion::ExecutableMissing => "its program was not found",
        PoolExclusion::ModelUnlisted => "the model is not in the catalog",
        PoolExclusion::NoModelsAvailable => "the catalog lists no model",
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

fn agents(ctx: &Ctx) -> Page {
    let theme = ctx.theme;
    let items = ctx
        .config
        .agents
        .iter()
        .map(|profile| {
            let in_team = ctx.config.team.contains(&profile.id);
            let (label, words) = identity_row_words(identity_of(ctx, profile), profile);
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
                if profile.enabled { "yes" } else { "no" },
                ctx.width,
            ));
            detail.extend(field(
                theme,
                "in team",
                if in_team { "yes" } else { "no" },
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
                    "None. Press i to write them."
                } else {
                    &profile.instructions
                },
                ctx.width,
            ));
            detail.push(Line::default());
            detail.push(Line::from(Span::styled(
                "Changing the model or the instructions starts a new experience identity. Sessions already running keep the profile they captured.".to_owned(),
                theme.faint(),
            )));
            Item::row(
                profile.id.clone(),
                vec![
                    Span::styled(label, theme.text()),
                    Span::styled(format!(" · {words}"), theme.muted()),
                    Span::styled(format!(" · {}", profile.provider), theme.faint()),
                ],
            )
            .with_right(vec![
                yes_no(profile.enabled, theme),
                Span::styled(
                    if in_team { "  in team".to_owned() } else { String::new() },
                    theme.accent(),
                ),
            ])
            .with_detail(detail)
        })
        .collect::<Vec<_>>();
    Page {
        view: View::Agents,
        title: View::Agents.title().into(),
        subtitle: format!("{} profiles", ctx.config.agents.len()),
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
            ("Space", "enable"),
            ("t", "team"),
            ("r", "re-read"),
            ("R", "ask the installations"),
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
                        None => "none was reported; an agent that pins no model has no name here"
                            .to_owned(),
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
                        Span::styled(provider.id.clone(), theme.text()),
                        Span::styled(format!("  {}", provider.command), theme.faint()),
                    ],
                )
                .with_right(vec![
                    Span::styled(
                        match (found, health.is_some_and(|health| health.available)) {
                            (true, _) => format!("{} on PATH  ", theme.markers.ok),
                            (false, true) => format!("{} in process  ", theme.markers.ok),
                            (false, false) => format!("{} not found  ", theme.markers.warn),
                        },
                        if found || health.is_some_and(|health| health.available) {
                            theme.good()
                        } else {
                            theme.warn()
                        },
                    ),
                    yes_no(provider.enabled, theme),
                ])
                .with_detail(detail)
            })
            .collect::<Vec<_>>();
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
            ("r", "re-read what is stored"),
            ("R", "ask this installation"),
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
                    vec![Span::styled(entry.title.clone(), theme.text())],
                )
                .with_right(vec![
                    Span::styled(format!("{state}  "), style),
                    Span::styled(
                        scope_name.to_owned(),
                        if scope_name == "global" {
                            theme.accent()
                        } else {
                            theme.faint()
                        },
                    ),
                ])
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
            ("/", "search"),
            ("f", "retire the entry"),
            ("Enter", "read"),
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
                        Span::styled(observation.agent_name.clone(), theme.text()),
                        Span::styled(
                            format!(" · {} / {}", observation.competence, observation.difficulty),
                            theme.faint(),
                        ),
                    ],
                )
                .with_right(vec![
                    Span::styled(format!("{verdict}  "), style),
                    Span::styled(
                        confirmation_tag(observation.confirmation).to_owned(),
                        if observation.confirmation == ConfirmationStatus::Confirmed {
                            theme.good()
                        } else {
                            theme.muted()
                        },
                    ),
                ])
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
        hints: vec![("Enter", "read the evidence"), ("Esc", "back")],
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

fn limits(ctx: &Ctx) -> Page {
    let theme = ctx.theme;
    let mut items = vec![about_row(
        ctx,
        "what a limit is",
        "What a limit is, and which run it applies to",
        WHAT_LIMITS_ARE,
    )];
    items.push(Item::heading("This session, as captured", theme));
    items.extend(captured_limit_rows(ctx));
    items.push(Item::heading("The next run", theme));
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
            Item::row(key, vec![Span::styled(key.to_owned(), theme.text())])
                .with_right(vec![Span::styled(value, theme.accent())])
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
            ("Enter", "type a value, or read the record"),
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
        return vec![Item::row(
            "captured:none",
            vec![
                Span::styled(format!("{} ", theme.markers.idle), theme.muted()),
                Span::styled(headline.to_owned(), theme.text()),
            ],
        )
        .with_right(vec![Span::styled(
            "nothing captured".to_owned(),
            theme.muted(),
        )])
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
        vec![Span::styled(label.to_owned(), theme.text())],
    )
    .with_right(vec![Span::styled(value.to_owned(), theme.body())])
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
    let theme = ctx.theme;
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
        items.push(Item::heading(
            if ctx.live { "Running now" } else { "Left open" },
            theme,
        ));
        for assignment in &running {
            items.push(assignment_row(ctx, assignment, ctx.live));
        }
    }
    if !earlier.is_empty() {
        items.push(Item::heading("Recorded earlier", theme));
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
        hints: vec![("Enter", "show the record"), ("Esc", "back")],
    }
}

fn assignment_row(ctx: &Ctx, assignment: &AssignmentRecord, live: bool) -> Item {
    let theme = ctx.theme;
    let invocation = ctx.records.last_invocation(&assignment.id);
    let state = invocation.map(|i| i.state).unwrap_or(assignment.state);
    let (marker, word, style) = invocation_state(state, live, theme);
    let name = presented_name(ctx, &assignment.agent_id);
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
            Span::styled(format!("{marker} "), style),
            Span::styled(name, theme.text()),
            Span::styled(format!(" · {}", assignment.purpose), theme.faint()),
        ],
    )
    .with_right(vec![
        Span::styled(format!("{word}  "), style),
        Span::styled(model_word(assignment, invocation).to_owned(), theme.muted()),
    ])
    .with_detail(detail)
}

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

/// The model on the row: what the installation reported, or what was sent, said as such.
fn model_word<'a>(
    assignment: &'a AssignmentRecord,
    invocation: Option<&'a InvocationRecord>,
) -> &'a str {
    invocation
        .and_then(|i| i.reported.model.as_deref())
        .or_else(|| invocation.and_then(|i| i.sent.model.as_deref()))
        .or(assignment.requested.model.as_deref())
        .unwrap_or("no model recorded")
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
    let author = presented_name(ctx, &proposal.agent_id);
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
            let holder = presented_name(ctx, agent_id);
            (
                format!("give {title} to {holder}"),
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
                .map(|id| presented_name(ctx, id))
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
    detail.extend(field(
        theme,
        "asked by",
        &presented_name(ctx, &proposal.agent_id),
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
            Span::styled(format!("{} ", theme.markers.activity), style),
            Span::styled(short, theme.text()),
        ],
    )
    .with_right(vec![
        Span::styled(format!("{outcome}  "), style),
        Span::styled(presented_name(ctx, &proposal.agent_id), theme.faint()),
    ])
    .with_detail(detail)
}

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
            &commitment_words(ctx, commitment),
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

/// Who holds a task and what their turns were committed to run with.
fn commitment_words(ctx: &Ctx, commitment: &BoardCommitment) -> String {
    format!(
        "{} · {} · against task version {}",
        presented_name(ctx, &commitment.agent_id),
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
            Some(commitment) => commitment_words(ctx, commitment),
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
                    presented_name(ctx, &proposal.agent_id)
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
        items.push(Item::heading("the plan", theme));
        items.push(
            Item::row(
                "board-unreadable",
                vec![Span::styled("the plan could not be read".to_owned(), theme.bad())],
            )
            .with_right(vec![Span::styled("unavailable".to_owned(), theme.bad())])
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
    items.push(Item::heading("proposals to change this plan", theme));
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
        hints: vec![("Enter", "show the record"), ("Esc", "back")],
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
    let actor = decision
        .actor
        .as_deref()
        .map(|id| presented_name(ctx, id))
        .unwrap_or_else(|| "the runtime".into());
    let mut detail = field(theme, "decision", &decision.id, ctx.width);
    detail.extend(field(
        theme,
        "kind",
        &decision_kind(&decision.kind),
        ctx.width,
    ));
    detail.extend(field(theme, "actor", &actor, ctx.width));
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
            &presented_name(ctx, &board.proposal.agent_id),
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
                &commitment_words(ctx, commitment),
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
        detail.extend(bound_detail(ctx, resource));
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
            Span::styled(format!("{marker} "), style),
            Span::styled(decision_kind(&decision.kind), theme.text()),
            Span::styled(format!(" · {actor}"), theme.faint()),
        ],
    )
    .with_right(vec![Span::styled(outcome_word, right_style)])
    .with_detail(detail)
}

/// What one membership decision proposed, and what it was decided at.
fn membership_detail(ctx: &Ctx, allocation: &AllocationDecision) -> Vec<Line<'static>> {
    let theme = ctx.theme;
    let names = |ids: &[String]| {
        if ids.is_empty() {
            "none".to_owned()
        } else {
            ids.iter()
                .map(|id| presented_name(ctx, id))
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
            Some(id) => presented_name(ctx, id),
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
fn bound_detail(ctx: &Ctx, resource: &ymp_core::ResourceAllocationDecision) -> Vec<Line<'static>> {
    let theme = ctx.theme;
    let mut lines = field(
        theme,
        "for",
        &presented_name(ctx, &resource.input.agent_id),
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
        &match ctx.records.credit_for(&task.id) {
            Some(agent) => format!("competence credited to {}", presented_name(ctx, agent)),
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

fn reviewer_words(ctx: &Ctx, acceptance: &Acceptance) -> String {
    if acceptance.reviewers.is_empty() {
        return "no review decision is linked to this acceptance".into();
    }
    acceptance
        .reviewers
        .iter()
        .map(|id| presented_name(ctx, id))
        .collect::<Vec<_>>()
        .join(", ")
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
    Item::row(
        key.to_owned(),
        vec![
            Span::styled(format!("{} ", theme.markers.notice), theme.info()),
            Span::styled(title.to_owned(), theme.text()),
        ],
    )
    .with_right(vec![Span::styled(
        "what this page is".to_owned(),
        theme.muted(),
    )])
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

pub fn display_name(config: &Config, id: &str) -> String {
    config
        .agents
        .iter()
        .find(|a| a.id == id)
        .map(|a| a.name.clone())
        .unwrap_or_else(|| id.to_owned())
}

/// The name to present for one actor, from what was actually recorded or read.
///
/// The order is a statement about sources and not a preference for longer names. An identity a
/// record captured names the actor as its turns ran, so it comes first and a later reading of
/// the installations never renames finished work. A session that recorded turns without
/// capturing an identity keeps its configured name for the same reason: the present catalog is
/// not evidence about a past turn. Only where no record speaks does the reading name the actor,
/// and where nothing was read the configured name is all that is known. A provider id is never
/// used as a name.
fn presented_name(ctx: &Ctx, id: &str) -> String {
    actor_name(ctx.config, ctx.pool, ctx.records, id)
}

pub fn actor_name(config: &Config, pool: &Pool, records: &Records, id: &str) -> String {
    if let Some(identity) = captured_identity_of(records, id) {
        return identity.name.clone();
    }
    if records
        .assignments()
        .iter()
        .any(|assignment| assignment.agent_id == id)
    {
        return display_name(config, id);
    }
    match pool.agent(id).map(|agent| &agent.identity) {
        Some(identity) => match identity.status {
            AgentIdentityStatus::Native
            | AgentIdentityStatus::Stale
            | AgentIdentityStatus::Local => identity.name.clone(),
            // The installation named no model for this actor, so it has no native name to
            // present. The pages that discuss models say that in their own words.
            AgentIdentityStatus::Unknown | AgentIdentityStatus::Unresolved => {
                identity.configured_name.clone()
            }
        },
        None => display_name(config, id),
    }
}
