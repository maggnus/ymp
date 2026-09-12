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
use crate::text;
use crate::theme::Theme;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use std::path::Path;
use ymp_core::{Config, MemoryEntry, Session, Task, TaskState};
use ymp_storage::Store;

/// Every destination the sidebar and the commands can reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Chat,
    Tasks,
    Sessions,
    Files,
    Changes,
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
    View::Sessions,
    View::Files,
    View::Changes,
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
            View::Sessions => "Sessions",
            View::Files => "Files",
            View::Changes => "Changed files",
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
            View::Sessions => "/sessions",
            View::Files => "/files",
            View::Changes => "/diff",
            View::Team => "/team",
            View::Agents => "/agents",
            View::Providers => "/providers",
            View::Memory => "/memory",
            View::Reputation => "/reputation",
            View::Limits => "/limits",
            View::Help => "/help",
        }
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
    pub theme: &'a Theme,
    pub session: Option<&'a str>,
    pub tasks: &'a [Task],
    pub memory_query: &'a str,
    pub width: usize,
}

/// Build the page for `view`. Failures become a visible error state rather than a panic.
pub fn build(view: View, ctx: &Ctx) -> Page {
    match view {
        View::Chat => page(view, "The conversation", Vec::new(), ctx),
        View::Help => help(ctx),
        View::Tasks => tasks(ctx),
        View::Sessions => guard(view, sessions(ctx), ctx),
        View::Files => guard(view, files(ctx), ctx),
        View::Changes => guard(view, changes(ctx), ctx),
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
    let label_width = 14usize.min(width.saturating_sub(4));
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

/// Is `command` reachable? A relative name is looked up on PATH; nothing is executed.
fn on_path(command: &str) -> bool {
    if command.contains('/') {
        return Path::new(command).is_file();
    }
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(command).is_file()))
        .unwrap_or(false)
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
                .map(|id| display_name(ctx.config, id))
                .unwrap_or_else(|| "unassigned".into());
            let mut detail = Vec::new();
            detail.extend(field(theme, "state", word, ctx.width));
            detail.extend(field(theme, "assignee", &assignee, ctx.width));
            if let Some(reviewer) = &task.reviewer {
                detail.extend(field(
                    theme,
                    "reviewer",
                    &display_name(ctx.config, reviewer),
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
                detail.extend(field(theme, "checks", &task.checks.join(" ; "), ctx.width));
            }
            if let Some(workspace) = &task.workspace {
                detail.extend(field(
                    theme,
                    "directory",
                    &workspace.display().to_string(),
                    ctx.width,
                ));
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
            Item::row(
                task.id.clone(),
                vec![
                    Span::styled(format!("{marker} "), style),
                    Span::styled(task.title.clone(), theme.text()),
                ],
            )
            .with_right(vec![
                Span::styled(format!("{word}  "), style),
                Span::styled(assignee, theme.faint()),
            ])
            .with_detail(detail)
        })
        .collect::<Vec<_>>();
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
            format!("{accepted} of {} accepted", ctx.tasks.len())
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

fn changes(ctx: &Ctx) -> anyhow::Result<Page> {
    let theme = ctx.theme;
    let mut items = Vec::new();
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
                let detail = field(theme, "path", &path, ctx.width)
                    .into_iter()
                    .chain(field(theme, "status", &status, ctx.width))
                    .collect();
                items.push(
                    Item::row(path.clone(), vec![Span::styled(path.clone(), theme.text())])
                        .with_right(vec![Span::styled(status, style)])
                        .with_detail(detail),
                );
            }
        }
    }
    Ok(Page {
        view: View::Changes,
        title: View::Changes.title().into(),
        subtitle,
        items,
        empty: nothing(
            theme,
            "No recorded file changes",
            "Change metadata is written when a run finishes or stops. Files themselves are created directly in the working directory.",
            ctx.width,
        ),
        hints: vec![("Enter", "show the full path"), ("Esc", "back")],
    })
}

fn team(ctx: &Ctx) -> Page {
    let theme = ctx.theme;
    let mut items = Vec::new();
    let members = ctx.config.members();
    for id in &ctx.config.team {
        let Ok(profile) = ctx.config.agent(id) else {
            continue;
        };
        let usable = members.iter().any(|m| &m.id == id);
        let provider = ctx.config.provider(&profile.provider).ok();
        let reason = if usable {
            "ready".to_owned()
        } else if !profile.enabled {
            "profile disabled".to_owned()
        } else if provider.is_none_or(|p| !p.enabled) {
            "provider disabled".to_owned()
        } else {
            "unavailable".to_owned()
        };
        let mut detail = field(theme, "profile", &profile.id, ctx.width);
        detail.extend(field(theme, "provider", &profile.provider, ctx.width));
        detail.extend(field(
            theme,
            "model",
            profile.model.as_deref().unwrap_or("provider default"),
            ctx.width,
        ));
        detail.extend(field(theme, "status", &reason, ctx.width));
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
        items.push(
            Item::row(
                profile.id.clone(),
                vec![
                    Span::styled(
                        format!(
                            "{} ",
                            if usable {
                                theme.markers.ok
                            } else {
                                theme.markers.warn
                            }
                        ),
                        if usable { theme.good() } else { theme.warn() },
                    ),
                    Span::styled(profile.name.clone(), theme.text()),
                    Span::styled(format!(" · {}", profile.provider), theme.faint()),
                ],
            )
            .with_right(vec![Span::styled(
                reason,
                if usable { theme.good() } else { theme.warn() },
            )])
            .with_detail(detail),
        );
    }
    let outside: Vec<_> = ctx
        .config
        .agents
        .iter()
        .filter(|a| !ctx.config.team.contains(&a.id))
        .collect();
    if !outside.is_empty() {
        items.push(Item::heading("Not in the team", theme));
        for profile in outside {
            items.push(
                Item::row(
                    profile.id.clone(),
                    vec![
                        Span::styled(format!("{} ", theme.markers.idle), theme.faint()),
                        Span::styled(profile.name.clone(), theme.muted()),
                        Span::styled(format!(" · {}", profile.provider), theme.faint()),
                    ],
                )
                .with_right(vec![Span::styled("not a member".to_owned(), theme.faint())])
                .with_detail(field(theme, "profile", &profile.id, ctx.width)),
            );
        }
    }
    Page {
        view: View::Team,
        title: View::Team.title().into(),
        subtitle: format!(
            "{} usable of {} listed · applies to the next run",
            members.len(),
            ctx.config.team.len()
        ),
        items,
        empty: nothing(
            theme,
            "The team is empty",
            "Add a profile with /team add ID. A team run needs at least two profiles so results are reviewed independently.",
            ctx.width,
        ),
        hints: vec![
            ("Space", "add or remove"),
            ("Enter", "open the profile"),
            ("Esc", "back"),
        ],
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
            let mut detail = field(theme, "profile", &profile.id, ctx.width);
            detail.extend(field(theme, "provider", &profile.provider, ctx.width));
            detail.extend(field(
                theme,
                "model",
                profile.model.as_deref().unwrap_or("provider default"),
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
                    Span::styled(profile.name.clone(), theme.text()),
                    Span::styled(format!(" · {}", profile.provider), theme.faint()),
                    Span::styled(
                        format!(" · {}", profile.model.as_deref().unwrap_or("default model")),
                        theme.muted(),
                    ),
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
            ("Esc", "back"),
        ],
    }
}

fn providers(ctx: &Ctx) -> Page {
    let theme = ctx.theme;
    let items = ctx
        .config
        .providers
        .iter()
        .map(|provider| {
            let found = on_path(&provider.command);
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
                if found {
                    "found on PATH"
                } else {
                    "not found on PATH"
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
            Item::row(
                provider.id.clone(),
                vec![
                    Span::styled(provider.id.clone(), theme.text()),
                    Span::styled(format!("  {}", provider.command), theme.faint()),
                ],
            )
            .with_right(vec![
                Span::styled(
                    if found {
                        format!("{} on PATH  ", theme.markers.ok)
                    } else {
                        format!("{} missing  ", theme.markers.warn)
                    },
                    if found { theme.good() } else { theme.warn() },
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
        hints: vec![("Space", "enable or disable"), ("Esc", "back")],
    }
}

fn memory(ctx: &Ctx) -> anyhow::Result<Page> {
    let theme = ctx.theme;
    let project = ctx.store.project(ctx.cwd)?;
    let entries: Vec<MemoryEntry> = ctx.store.memory(Some(&project.id), ctx.memory_query)?;
    let items = entries
        .iter()
        .map(|entry| {
            let scope = if entry.project_id.is_some() {
                "project"
            } else {
                "global"
            };
            let mut detail = field(theme, "entry", &entry.id, ctx.width);
            detail.extend(field(theme, "scope", scope, ctx.width));
            detail.extend(field(theme, "kind", &entry.kind, ctx.width));
            detail.extend(field(theme, "author", &entry.author, ctx.width));
            detail.extend(field(
                theme,
                "reviewer",
                entry.reviewer.as_deref().unwrap_or("none"),
                ctx.width,
            ));
            detail.extend(field(theme, "recorded", &entry.created_at, ctx.width));
            detail.push(Line::default());
            detail.extend(text::markdown(
                &entry.content,
                ctx.width,
                theme,
                theme.body(),
            ));
            Item::row(
                entry.id.clone(),
                vec![Span::styled(entry.title.clone(), theme.text())],
            )
            .with_right(vec![Span::styled(
                scope.to_owned(),
                if scope == "global" {
                    theme.accent()
                } else {
                    theme.faint()
                },
            )])
            .with_detail(detail)
        })
        .collect::<Vec<_>>();
    Ok(Page {
        view: View::Memory,
        title: View::Memory.title().into(),
        subtitle: if ctx.memory_query.is_empty() {
            "Verified knowledge for this project and shared procedures".into()
        } else {
            format!("Matching \"{}\"", ctx.memory_query)
        },
        items,
        empty: nothing(
            theme,
            "No verified memory matches",
            "Entries appear after an independent reviewer accepts them. Search with /memory QUERY.",
            ctx.width,
        ),
        hints: vec![
            ("/", "search"),
            ("f", "retire the entry"),
            ("Enter", "read"),
            ("Esc", "back"),
        ],
    })
}

fn reputation(ctx: &Ctx) -> anyhow::Result<Page> {
    let theme = ctx.theme;
    let observations = ctx.store.observations()?;
    let items = observations
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
            .with_right(vec![Span::styled(verdict, style)])
            .with_detail(detail)
        })
        .collect::<Vec<_>>();
    Ok(Page {
        view: View::Reputation,
        title: View::Reputation.title().into(),
        subtitle: format!("{} recorded observations", observations.len()),
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

fn limits(ctx: &Ctx) -> Page {
    let theme = ctx.theme;
    let limits = &ctx.config.limits;
    let rows: [(&str, String, &str); 4] = [
        (
            "parallel",
            limits.parallel.to_string(),
            "Turns that may run at the same time. One task writes at a time regardless.",
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
    ];
    let items = rows
        .iter()
        .map(|(key, value, description)| {
            Item::row(
                (*key).to_owned(),
                vec![Span::styled((*key).to_owned(), theme.text())],
            )
            .with_right(vec![Span::styled(value.clone(), theme.accent())])
            .with_detail(paragraph(theme, description, ctx.width))
        })
        .collect::<Vec<_>>();
    Page {
        view: View::Limits,
        title: View::Limits.title().into(),
        subtitle: "Applied to the next run".into(),
        items,
        empty: Vec::new(),
        hints: vec![
            ("+ / -", "adjust"),
            ("Enter", "type a value"),
            ("Esc", "back"),
        ],
    }
}

pub fn display_name(config: &Config, id: &str) -> String {
    config
        .agents
        .iter()
        .find(|a| a.id == id)
        .map(|a| a.name.clone())
        .unwrap_or_else(|| id.to_owned())
}
