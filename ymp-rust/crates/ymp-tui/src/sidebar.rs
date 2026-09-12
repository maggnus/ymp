//! The right sidebar: where you are, what the team is doing, and what the session is.
//!
//! The sidebar is built as a list of sections with a minimum height each. When the terminal
//! is short the sections shrink in a defined order instead of being clipped, so the context
//! that matters most stays on screen at 80x24.

use crate::state::App;
use crate::text;
use crate::theme::Theme;
use crate::usage;
use crate::views::{self, View};
use ratatui::text::{Line, Span};
use ymp_core::TaskState;

struct Section {
    lines: Vec<Line<'static>>,
    /// Lines that must survive when space runs out, including the title.
    keep: usize,
    /// A row that must stay visible when the section is shortened. The section scrolls
    /// to it instead of losing it off the bottom.
    anchor: Option<usize>,
}

/// Compose the sidebar. `width` is the inner width available for text.
pub fn lines(app: &App, width: usize, height: usize, focused: bool) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let sections = vec![
        navigation(app, width, focused),
        session(app, width),
        tokens(app, width),
        team(app, width),
        tasks(app, width),
    ];
    // When the sidebar owns the keyboard the navigation list must stay whole; otherwise the
    // live context is worth more than the full list of destinations.
    let shrink_order: [usize; 5] = if focused {
        [4, 3, 2, 1, 0]
    } else {
        [0, 4, 3, 2, 1]
    };
    fit(sections, height, &shrink_order, theme)
}

/// Fit the sections into `height` rows.
///
/// Heights are decided first and rendered afterwards, so the row that reports hidden
/// content is counted inside the budget rather than added to it. Sections shrink to their
/// `keep` height in the given order and are only then dropped whole; a section title with
/// nothing underneath it is noise, so a section that cannot keep a row of content does
/// not appear at all.
fn fit(
    sections: Vec<Section>,
    height: usize,
    shrink_order: &[usize],
    theme: &Theme,
) -> Vec<Line<'static>> {
    let mut heights: Vec<usize> = sections.iter().map(|s| s.lines.len()).collect();
    let total = |heights: &[usize]| -> usize {
        let present = heights.iter().filter(|rows| **rows > 0).count();
        heights.iter().sum::<usize>() + present.saturating_sub(1)
    };
    for &index in shrink_order {
        if total(&heights) <= height {
            break;
        }
        let excess = total(&heights) - height;
        let reducible = heights[index].saturating_sub(sections[index].keep);
        heights[index] -= reducible.min(excess);
    }
    for &index in shrink_order {
        if total(&heights) <= height {
            break;
        }
        heights[index] = 0;
    }
    let mut out: Vec<Line<'static>> = Vec::new();
    for (section, rows) in sections.into_iter().zip(heights) {
        let rendered = shorten(section, rows, theme);
        if rendered.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(Line::default());
        }
        out.extend(rendered);
    }
    out.truncate(height);
    out
}

/// Render one section into at most `rows` rows, scrolling to keep its anchor visible.
fn shorten(section: Section, rows: usize, theme: &Theme) -> Vec<Line<'static>> {
    if rows == 0 {
        return Vec::new();
    }
    if rows >= section.lines.len() {
        return section.lines;
    }
    let mut body = section.lines;
    let title = body.remove(0);
    let mut out = vec![title];
    // One row reports what is not shown, so the body gets what is left.
    let capacity = rows.saturating_sub(2);
    if capacity == 0 {
        // A section never draws more rows than it was given, so a single row is the
        // title alone rather than a title and a counter.
        if rows > 1 {
            out.push(hidden_row(body.len(), theme));
        }
        return out;
    }
    // The anchor indexes the whole section, and the title was row zero.
    let anchor = section.anchor.map(|row| row.saturating_sub(1)).unwrap_or(0);
    let start = anchor
        .saturating_sub(capacity - 1)
        .min(body.len().saturating_sub(capacity));
    out.extend(body.iter().skip(start).take(capacity).cloned());
    out.push(hidden_row(body.len() - capacity, theme));
    out
}

fn hidden_row(hidden: usize, theme: &Theme) -> Line<'static> {
    Line::from(Span::styled(
        format!("{} {hidden} more", theme.markers.more),
        theme.faint(),
    ))
}

fn title(theme: &Theme, text: &str, focused: bool) -> Line<'static> {
    let mut spans = vec![Span::styled(text.to_uppercase(), theme.muted())];
    if focused {
        spans.push(Span::styled("  focus".to_owned(), theme.accent()));
    }
    Line::from(spans)
}

fn navigation(app: &App, width: usize, focused: bool) -> Section {
    let theme = &app.theme;
    let mut lines = vec![title(theme, "navigate", focused)];
    for (index, view) in views::NAV.iter().enumerate() {
        let current = *view == app.view;
        let selected = focused && index == app.nav;
        let marker = if selected {
            theme.markers.selection
        } else if current {
            theme.markers.busy
        } else {
            " "
        };
        let style = if selected {
            theme.selected()
        } else if current {
            theme.accent()
        } else {
            theme.body()
        };
        let badge = match view {
            View::Tasks if !app.tasks.is_empty() => app.tasks.len().to_string(),
            // The session total stays reachable from the list even when the statistics
            // section below has been shortened away.
            View::Usage if usage::present(app.session.as_deref(), &app.stats) => {
                usage::headline(app.stats.total(), theme)
            }
            View::Team => app.config.members().len().to_string(),
            _ => String::new(),
        };
        lines.push(text::row(
            width,
            vec![Span::styled(format!("{marker} {}", view.title()), style)],
            if badge.is_empty() {
                Vec::new()
            } else {
                vec![Span::styled(badge, theme.faint())]
            },
        ));
    }
    Section {
        // The title is row zero, so the selected destination sits one row below its index.
        anchor: focused.then(|| app.nav.min(views::NAV.len() - 1) + 1),
        // While the list is being driven it keeps room for a few neighbours, so the
        // selection has context instead of sitting alone above a counter.
        keep: if focused { 5 } else { 2 },
        lines,
    }
}

fn session(app: &App, width: usize) -> Section {
    let theme = &app.theme;
    let mut lines = vec![title(theme, "session", false)];
    match &app.session {
        Some(id) => {
            let (status, style) = views::session_status(app.live_status(), theme);
            lines.push(text::row(
                width,
                vec![Span::styled(text::short_id(id), theme.text())],
                vec![Span::styled(status, style)],
            ));
        }
        None => lines.push(Line::from(Span::styled(
            "none · the next message opens one".to_owned(),
            theme.faint(),
        ))),
    }
    // Against the bound the session captured where it has one, so a stored run is not
    // measured against a limit edited after it finished.
    let (limit, captured) = app.turn_limit();
    lines.push(text::row(
        width,
        vec![Span::styled(
            if captured {
                "turns this session".to_owned()
            } else {
                "turns next run".to_owned()
            },
            theme.muted(),
        )],
        vec![Span::styled(
            format!("{} / {limit}", app.turns_used),
            theme.body(),
        )],
    ));
    if let Some(started) = app.started {
        let seconds = started.elapsed().as_secs();
        lines.push(text::row(
            width,
            vec![Span::styled("elapsed".to_owned(), theme.muted())],
            vec![Span::styled(
                format!("{:02}:{:02}", seconds / 60, seconds % 60),
                theme.body(),
            )],
        ));
    }
    // The directory agents actually write in. It is labelled, because a bare path in a
    // narrow column is not self-explanatory.
    lines.push(text::row(
        width,
        vec![Span::styled("dir".to_owned(), theme.muted())],
        vec![Span::styled(
            tail_path(&app.cwd.display().to_string(), width.saturating_sub(4)),
            theme.body(),
        )],
    ));
    Section {
        lines,
        keep: 4,
        anchor: None,
    }
}

/// Token statistics: the session total on the section title, then one row per agent.
///
/// The title carries the total because a shortened section keeps its title, so the figure
/// a reader checks most often survives a short terminal. Every row is an agent: two agents
/// that share a provider are two rows and two counters, never one.
fn tokens(app: &App, width: usize) -> Section {
    let theme = &app.theme;
    let total = app.stats.total();
    let mut lines = vec![text::row(
        width,
        vec![Span::styled("TOKENS".to_owned(), theme.muted())],
        vec![Span::styled(
            usage::headline(total, theme),
            usage::headline_style(total, theme),
        )],
    )];
    if !usage::present(app.session.as_deref(), &app.stats) {
        lines.push(Line::from(Span::styled(
            "none · nothing spent yet".to_owned(),
            theme.faint(),
        )));
        return Section {
            lines,
            keep: 2,
            anchor: None,
        };
    }
    let (team, _) = app.active_team();
    let rows = usage::agent_rows(&app.stats, &team, &app.config);
    if rows.is_empty() {
        lines.push(Line::from(Span::styled(
            "no agent recorded".to_owned(),
            theme.faint(),
        )));
    }
    for row in rows {
        let figure = usage::agent_headline(&row, theme);
        lines.push(text::row(
            width,
            vec![Span::styled(
                text::truncate(&row.name, width.saturating_sub(text::width(&figure) + 2)),
                theme.body(),
            )],
            vec![Span::styled(
                figure,
                usage::agent_headline_style(&row, theme),
            )],
        ));
    }
    Section {
        lines,
        keep: 3,
        anchor: None,
    }
}

/// Keep the end of a path, which is the part that identifies it.
fn tail_path(path: &str, width: usize) -> String {
    if text::width(path) <= width {
        return path.to_owned();
    }
    let mut kept: Vec<char> = Vec::new();
    for ch in path.chars().rev() {
        if kept.len() + 1 >= width {
            break;
        }
        kept.push(ch);
    }
    kept.reverse();
    format!("…{}", kept.into_iter().collect::<String>())
}

fn team(app: &App, width: usize) -> Section {
    let theme = &app.theme;
    let (members, captured) = app.active_team();
    let mut lines = vec![text::row(
        width,
        vec![Span::styled("TEAM".to_owned(), theme.muted())],
        vec![Span::styled(
            if captured { "this session" } else { "next run" }.to_owned(),
            theme.faint(),
        )],
    )];
    if members.is_empty() {
        lines.push(Line::from(Span::styled(
            "empty · /team add ID".to_owned(),
            theme.faint(),
        )));
        return Section {
            lines,
            keep: 2,
            anchor: None,
        };
    }
    for member in members {
        let raw = app.statuses.get(&member.id).map(String::as_str);
        let (word, style) = activity(raw, theme);
        let marker = match raw {
            Some("idle") | None => theme.markers.idle,
            Some("error") => theme.markers.fail,
            // A turn held up by coordination is not work in flight, and the two must not
            // share a marker. Why it waits is on the task and the decision that recorded it.
            Some(status) if status.starts_with("waiting") => theme.markers.paused,
            Some(_) => theme.markers.busy,
        };
        lines.push(text::row(
            width,
            vec![
                Span::styled(format!("{marker} "), style),
                Span::styled(member.name.clone(), theme.body()),
            ],
            vec![Span::styled(word.to_owned(), style)],
        ));
    }
    Section {
        lines,
        keep: 2,
        anchor: None,
    }
}

/// Turn a runtime purpose into a word a reader can act on.
fn activity(status: Option<&str>, theme: &Theme) -> (&'static str, ratatui::style::Style) {
    match status {
        None | Some("idle") => ("idle", theme.faint()),
        Some("error") => ("failed", theme.bad()),
        Some("plan") => ("planning", theme.info()),
        Some("bid") => ("bidding", theme.info()),
        Some("execute") => ("working", theme.warn()),
        Some("review") | Some("final_review") => ("reviewing", theme.info()),
        Some("review_memory") => ("checking", theme.info()),
        Some("synthesis") => ("summarising", theme.accent()),
        Some("learn") => ("learning", theme.accent()),
        Some("conversation") => ("answering", theme.accent()),
        Some(status) if status.starts_with("waiting") => ("waiting", theme.muted()),
        Some(_) => ("busy", theme.warn()),
    }
}

fn tasks(app: &App, width: usize) -> Section {
    let theme = &app.theme;
    let mut lines = vec![title(theme, "tasks", false)];
    if app.tasks.is_empty() {
        lines.push(Line::from(Span::styled(
            "no task graph yet".to_owned(),
            theme.faint(),
        )));
        return Section {
            lines,
            keep: 2,
            anchor: None,
        };
    }
    let count = |state: TaskState| app.tasks.iter().filter(|t| t.state == state).count();
    let accepted = count(TaskState::Accepted);
    lines.push(text::row(
        width,
        vec![Span::styled("accepted".to_owned(), theme.muted())],
        vec![Span::styled(
            format!("{accepted} / {}", app.tasks.len()),
            theme.good(),
        )],
    ));
    for task in app.tasks.iter().filter(|t| t.state != TaskState::Accepted) {
        let (marker, word, style) = views::task_state(task.state, theme);
        lines.push(text::row(
            width,
            vec![
                Span::styled(format!("{marker} "), style),
                Span::styled(
                    text::truncate(&task.title, width.saturating_sub(word.len() + 4)),
                    theme.body(),
                ),
            ],
            vec![Span::styled(word.to_owned(), style)],
        ));
    }
    Section {
        lines,
        keep: 2,
        anchor: None,
    }
}
