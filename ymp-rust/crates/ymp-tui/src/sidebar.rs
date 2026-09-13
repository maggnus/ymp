//! The right sidebar: what the session is, what it spent and what the team is doing.
//!
//! The sidebar is built as a list of sections with a minimum height each. When the terminal
//! is short the sections shrink in a defined order instead of being clipped, so the context
//! that matters most stays on screen at 80x24. Its lists are compact tables: column titles over
//! aligned rows, as on the pages, without a selection.

use crate::state::App;
use crate::table::{self, Cell, Column};
use crate::text;
use crate::theme::Theme;
use crate::usage;
use crate::views;
use ratatui::text::{Line, Span};
use ymp_core::TaskState;

struct Section {
    lines: Vec<Line<'static>>,
    /// Lines that must survive when space runs out, including the title.
    keep: usize,
    /// Lines at the top that stay together when the section is shortened: the title, and the
    /// column titles of the table under it.
    fixed: usize,
    /// A row that must stay visible when the section is shortened. The section scrolls
    /// to it instead of losing it off the bottom.
    anchor: Option<usize>,
}

const TOKEN_COLUMNS: [Column; 2] = [Column::left("AGENT").flex(), Column::right("TOKENS")];
// A member's model and effort matter more than what it is doing, so the activity word gives way
// first. The marker opens the agent cell rather than taking a column and a gap of its own.
const TEAM_COLUMNS: [Column; 2] = [Column::left("AGENT"), Column::left("STATE").flex()];
const TASK_COLUMNS: [Column; 2] = [Column::left("TASK").flex(), Column::left("STATE")];

/// Compose the sidebar. `width` is the inner width available for text.
///
/// The sidebar lists no destinations and never takes the keyboard: a page opens with its command
/// or from the palette. Sections give way from the bottom, so the session stays longest.
pub fn lines(app: &App, width: usize, height: usize) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let sections = vec![
        session(app, width),
        tokens(app, width),
        team(app, width),
        tasks(app, width),
    ];
    fit(sections, height, &[3, 2, 1, 0], theme)
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
    let fixed = section.fixed.clamp(1, body.len());
    let mut head: Vec<Line<'static>> = body.drain(..fixed).collect();
    if rows < fixed + 2 {
        // Column titles over no rows say nothing, so a section too short for titles, a row and
        // the counter gives the row of titles to a record.
        head.truncate(1);
    }
    // One row reports what is not shown, so the body gets what is left.
    let capacity = rows.saturating_sub(head.len() + 1);
    if capacity == 0 {
        // A section never draws more rows than it was given: this one is its title, with a
        // counter when there is room.
        let mut out = head;
        if rows > 1 {
            out.push(hidden_row(body.len(), theme));
        }
        return out;
    }
    let mut out = head;
    // The anchor indexes the whole section, whose fixed lines come first.
    let anchor = section
        .anchor
        .map(|row| row.saturating_sub(fixed))
        .unwrap_or(0);
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

fn title(theme: &Theme, text: &str) -> Line<'static> {
    Line::from(Span::styled(text.to_uppercase(), theme.muted()))
}

/// A list in the sidebar: its column titles, then its rows aligned under them.
fn compact_table(
    columns: &[Column],
    rows: &[Vec<Cell>],
    width: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let borrowed: Vec<&[Cell]> = rows.iter().map(Vec::as_slice).collect();
    let widths = table::widths(columns, &borrowed, None, width);
    let mut lines = vec![Line::from(table::titles(
        columns,
        &widths,
        &[],
        None,
        theme,
    ))];
    lines.extend(
        rows.iter()
            .map(|cells| Line::from(table::aligned(columns, &widths, cells))),
    );
    lines
}

fn session(app: &App, width: usize) -> Section {
    let theme = &app.theme;
    let mut lines = vec![title(theme, "session")];
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
        fixed: 1,
        anchor: None,
    }
}

/// Token statistics: the session total on the section title, then a table of agents.
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
            fixed: 1,
            anchor: None,
        };
    }
    let (team, _) = app.active_team();
    let rows: Vec<Vec<Cell>> = usage::agent_rows(&app.stats, &team, &app.config)
        .iter()
        .map(|row| {
            vec![
                Cell::text(app.agent_label(&row.id), theme.body()),
                Cell::text(
                    usage::agent_headline(row, theme),
                    usage::agent_headline_style(row, theme),
                ),
            ]
        })
        .collect();
    if rows.is_empty() {
        lines.push(Line::from(Span::styled(
            "no agent recorded".to_owned(),
            theme.faint(),
        )));
        return Section {
            lines,
            keep: 2,
            fixed: 1,
            anchor: None,
        };
    }
    lines.extend(compact_table(&TOKEN_COLUMNS, &rows, width, theme));
    Section {
        lines,
        keep: 4,
        fixed: 2,
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
            fixed: 1,
            anchor: None,
        };
    }
    let rows: Vec<Vec<Cell>> = members
        .iter()
        .map(|member| {
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
            vec![
                Cell::spans(vec![
                    Span::styled(format!("{marker} "), style),
                    Span::styled(app.agent_label(&member.id), theme.body()),
                ]),
                Cell::text(word, style),
            ]
        })
        .collect();
    lines.extend(compact_table(&TEAM_COLUMNS, &rows, width, theme));
    Section {
        lines,
        keep: 2,
        fixed: 2,
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

/// Tasks: how many were accepted beside the title, then a table of the ones still open.
fn tasks(app: &App, width: usize) -> Section {
    let theme = &app.theme;
    if app.tasks.is_empty() {
        return Section {
            lines: vec![
                title(theme, "tasks"),
                Line::from(Span::styled("no task graph yet".to_owned(), theme.faint())),
            ],
            keep: 2,
            fixed: 1,
            anchor: None,
        };
    }
    let accepted = app
        .tasks
        .iter()
        .filter(|task| task.state == TaskState::Accepted)
        .count();
    // The title carries the accepted count, so a shortened section keeps that figure.
    let mut lines = vec![text::row(
        width,
        vec![Span::styled("TASKS".to_owned(), theme.muted())],
        vec![Span::styled(
            format!("accepted {accepted} / {}", app.tasks.len()),
            theme.good(),
        )],
    )];
    let rows: Vec<Vec<Cell>> = app
        .tasks
        .iter()
        .filter(|task| task.state != TaskState::Accepted)
        .map(|task| {
            let (marker, word, style) = views::task_state(task.state, theme);
            vec![
                Cell::spans(vec![
                    Span::styled(format!("{marker} "), style),
                    Span::styled(text::one_line(&task.title), theme.body()),
                ]),
                Cell::text(word, style),
            ]
        })
        .collect();
    if rows.is_empty() {
        return Section {
            lines,
            keep: 1,
            fixed: 1,
            anchor: None,
        };
    }
    lines.extend(compact_table(&TASK_COLUMNS, &rows, width, theme));
    Section {
        lines,
        keep: 2,
        fixed: 2,
        anchor: None,
    }
}
