//! Composition: state becomes the lines and rectangles the frame layer draws.
//!
//! The layout is chat-first with a right sidebar. From the top: a context header, the body
//! split into the main column and the sidebar, the composer, and a status row. Nothing here
//! invents a value; every span carries text that came from the state or names a key.

use crate::exit;
use crate::frame::{self, ModalRole, ModalSpec};
use crate::highlight;
use crate::sidebar;
use crate::state::{App, Confirm, Focus, Overlay, PromptTarget};
use crate::table::{self, Cell, Column};
use crate::text;
use crate::theme::{self, Theme};
use crate::transcript;
use crate::usage;
use crate::views::{self, ItemKind, Page, View};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::FrameExt as _;
use ratatui::Frame;

/// The tallest the composer may grow before it starts scrolling.
const COMPOSER_MAX: u16 = 5;

/// Keys every page table answers, named after a page's own keys.
const TABLE_HINTS: [(&str, &str); 3] =
    [("/", "filter"), ("Shift+letter", "sort"), ("d", "inspect")];

/// Draw the whole interface. The single entry point the event loop calls.
pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    if area.width < 8 || area.height < 2 {
        return;
    }
    let theme = app.theme;
    frame::fill(frame, area, &theme);
    app.viewport.width = area.width as usize;

    let composer_width = area.width.saturating_sub(3).max(4) as usize;
    let (composer_lines, cursor_position) =
        text::compose(&app.input.value, composer_width, app.input.cursor);
    let composer_height = (composer_lines.len() as u16).clamp(1, COMPOSER_MAX);
    let rows = frame::chrome(area, composer_height);

    let sidebar_width = if app.prefs.sidebar {
        frame::sidebar_width(area.width)
    } else {
        0
    };
    let (main, side) = frame::split_body(rows.body, sidebar_width);

    if rows.header.height > 0 {
        header(frame, rows.header, app);
        frame::hairline(frame, rows.top_rule, &theme);
    }

    match app.view {
        View::Chat => transcript_view(frame, main, app),
        _ => page_view(frame, main, app),
    }

    if let Some((rule, panel)) = side {
        frame::vertical_rule(frame, rule, &theme);
        frame::fill_surface(frame, panel, &theme);
        let inner = Rect {
            x: panel.x + 1,
            y: panel.y,
            width: panel.width.saturating_sub(2),
            height: panel.height,
        };
        let lines = sidebar::lines(app, inner.width as usize, inner.height as usize);
        frame::paint(frame, inner, lines);
    }

    frame::hairline_with(
        frame,
        rows.bottom_rule,
        &theme,
        theme.border(app.focus == Focus::Composer),
    );
    composer(frame, rows.composer, app, &composer_lines, cursor_position);
    if rows.status.height > 0 {
        // The page is already built and cached by the body above, so this is a lookup.
        let page_hints = match app.view {
            View::Chat => Vec::new(),
            _ => app.page(main.width.saturating_sub(2)).hints.clone(),
        };
        status(frame, rows.status, app, &page_hints);
    }

    let completions = completion_popup(frame, main, rows.composer, app);
    let mut cursor =
        (!completions && app.focus == Focus::Composer && app.overlay.is_none()).then(|| {
            composer_cursor(
                rows.composer,
                &composer_lines,
                cursor_position,
                composer_height,
            )
        });
    if app.overlay.is_some() {
        // A surface may blank the main column and the sidebar around itself, but never the rules
        // around and between them.
        let regions: Vec<Rect> = std::iter::once(main)
            .chain(side.map(|(_, panel)| panel))
            .collect();
        // The popups that scroll name their keys, so they show the rows of body this leaves.
        app.viewport.modal_rows = frame::modal_body_rows(area.height, true);
        cursor = overlay(frame, area, &regions, app);
    }
    if let Some((x, y)) = cursor {
        if x < area.right() && y < area.bottom() {
            frame.set_cursor_position((x, y));
        }
    }
}

// ---------------------------------------------------------------------------
// Chrome
// ---------------------------------------------------------------------------

fn header(frame: &mut Frame, area: Rect, app: &App) {
    let theme = &app.theme;
    let wide = area.width >= 100;
    let project = app
        .cwd
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| app.cwd.display().to_string());
    let mut left = vec![
        Span::styled("ymp".to_owned(), theme.accent_bold()),
        Span::styled("  ".to_owned(), theme.faint()),
        Span::styled(project, theme.bold()),
    ];
    if wide {
        left.push(Span::styled(
            format!("  {}", app.cwd.display()),
            theme.faint(),
        ));
    }
    let (status, status_style) = if app.active || app.session.is_some() {
        views::session_status(app.live_status(), theme)
    } else {
        ("no session".to_owned(), theme.faint())
    };
    let total = app.stats.total();
    let mut segments: Vec<Segment> = Vec::new();
    if let Some(id) = &app.session {
        segments.push(Segment::new(3, text::short_id(id), theme.muted()));
    }
    segments.push(Segment::new(2, status, status_style));
    if usage::present(app.session.as_deref(), &app.stats) {
        // The session total belongs here as well as in the sidebar: it stays readable when
        // the sidebar is hidden, and when a short terminal has shortened its section away.
        segments.push(Segment::new(
            1,
            format!(
                "{} {}",
                usage::headline(total, theme),
                if wide { "tokens" } else { "tok" }
            ),
            usage::headline_style(total, theme),
        ));
    }
    // The denominator is the bound the loaded session captured, not the one a later edit
    // left in the configuration: a finished run is measured against what it ran under.
    let (limit, captured) = app.turn_limit();
    segments.push(Segment::new(
        4,
        if wide {
            format!(
                "{} / {limit} turns{}",
                app.turns_used,
                if captured { " captured" } else { "" }
            )
        } else {
            format!("{}/{limit}", app.turns_used)
        },
        theme.faint(),
    ));
    if app.prefs.details {
        segments.push(Segment::new(5, "details".to_owned(), theme.accent()));
    }
    frame::row(frame, area, left, fit_segments(segments, area.width));
}

/// One field of the header, and how readily it gives up its cells.
///
/// A narrow terminal cannot show everything. Rather than truncating the project name away,
/// the header drops whole fields, least important first, and keeps their reading order.
struct Segment {
    priority: u8,
    text: String,
    style: Style,
}

impl Segment {
    fn new(priority: u8, text: String, style: Style) -> Self {
        Self {
            priority,
            text,
            style,
        }
    }
}

/// Room the left of the header keeps for the application name and part of the project.
const HEADER_LEFT: usize = 12;

fn fit_segments(segments: Vec<Segment>, width: u16) -> Vec<Span<'static>> {
    let budget = (width as usize).saturating_sub(HEADER_LEFT);
    let mut order: Vec<usize> = (0..segments.len()).collect();
    order.sort_by_key(|index| segments[*index].priority);
    let mut used = 0usize;
    let mut kept: Vec<bool> = vec![false; segments.len()];
    for index in order {
        let cells = text::width(&segments[index].text) + 2;
        if used + cells <= budget {
            used += cells;
            kept[index] = true;
        }
    }
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (index, segment) in segments.into_iter().enumerate() {
        if !kept[index] {
            continue;
        }
        if !spans.is_empty() {
            spans.push(Span::raw("  ".to_owned()));
        }
        spans.push(Span::styled(segment.text, segment.style));
    }
    spans
}

fn status(frame: &mut Frame, area: Rect, app: &App, page_hints: &[(&'static str, &'static str)]) {
    let theme = &app.theme;
    let mut left: Vec<Span<'static>> = Vec::new();
    if app.active {
        let spinner = theme.markers.spinner[(app.tick % 4) as usize];
        left.push(Span::styled(format!("{spinner} "), theme.accent()));
    }
    left.push(if app.exit_requested.is_some() {
        Span::styled(exit::CONFIRM_PROMPT.to_owned(), theme.warn())
    } else {
        Span::styled(
            text::one_line(&app.status),
            if app.active {
                theme.body()
            } else {
                theme.faint()
            },
        )
    });
    let mut right: Vec<Span<'static>> = Vec::new();
    if app.view == View::Chat && !app.follow {
        right.push(Span::styled(
            format!("{} paused  ", theme.markers.paused),
            theme.warn(),
        ));
        right.extend(frame::key_hints(&[("End", "latest")], theme));
        right.push(Span::styled("  ".to_owned(), theme.faint()));
    }
    right.push(Span::styled(
        format!("{} ", app.focus.label()),
        theme.muted(),
    ));
    right.extend(frame::key_hints(&hints(app, page_hints), theme));
    // What the window is doing outranks the list of keys: a long hint list must not take the
    // status text away, which is how a reader learns that a reading is running.
    frame::header(frame, area, left, right);
}

/// The keys worth naming right now. A page states its own; the conversation states the
/// ones that belong to whichever region owns the keyboard.
fn hints(
    app: &App,
    page_hints: &[(&'static str, &'static str)],
) -> Vec<(&'static str, &'static str)> {
    match (app.view, app.focus) {
        (View::Chat, Focus::Composer) => {
            vec![("Enter", "send"), ("Ctrl+P", "commands"), ("Tab", "focus")]
        }
        // Detailed mode draws every entry in full, so there is nothing left to expand.
        (View::Chat, Focus::Main) if app.prefs.details => {
            vec![("Enter", "inspect"), ("Esc", "composer")]
        }
        (View::Chat, Focus::Main) => vec![
            ("Enter", "inspect"),
            ("Space", "expand"),
            ("Esc", "composer"),
        ],
        (_, Focus::Composer) => vec![("Enter", "send"), ("Tab", "focus"), ("Esc", "back")],
        (_, Focus::Main) if app.table.typing => {
            vec![("type", "filter"), ("Enter", "keep"), ("Esc", "clear")]
        }
        (_, Focus::Main) => {
            let mut hints = if page_hints.is_empty() {
                vec![("Enter", "open"), ("Esc", "back"), ("Tab", "focus")]
            } else {
                page_hints.to_vec()
            };
            // The files page is a file explorer rather than a table, and names its own keys.
            if app.view != View::Files {
                hints.extend(TABLE_HINTS);
            }
            hints
        }
    }
}

fn composer(frame: &mut Frame, area: Rect, app: &App, lines: &[String], cursor: (usize, usize)) {
    if area.height == 0 {
        return;
    }
    let theme = &app.theme;
    frame::fill_surface(frame, area, theme);
    let height = area.height as usize;
    let first = cursor.0.saturating_sub(height.saturating_sub(1));
    let marker = if app.active {
        theme.markers.spinner[(app.tick % 4) as usize]
    } else {
        theme.markers.prompt
    };
    let mut rendered: Vec<Line<'static>> = Vec::new();
    if app.input.is_empty() {
        rendered.push(Line::from(vec![
            Span::styled(format!("{marker} "), theme.accent_bold()),
            Span::styled(
                if app.active {
                    "A run is active. A message here joins its shared chat.".to_owned()
                } else {
                    "Describe a task, or type / for commands.".to_owned()
                },
                theme.faint(),
            ),
        ]));
    } else {
        for (index, line) in lines.iter().skip(first).take(height).enumerate() {
            let prefix = if index == 0 && first == 0 {
                format!("{marker} ")
            } else {
                "  ".to_owned()
            };
            rendered.push(Line::from(vec![
                Span::styled(prefix, theme.accent_bold()),
                Span::styled(line.clone(), theme.text()),
            ]));
        }
    }
    frame::paint(frame, area, rendered);
}

fn composer_cursor(
    area: Rect,
    lines: &[String],
    cursor: (usize, usize),
    height: u16,
) -> (u16, u16) {
    let height = height.max(1) as usize;
    let first = cursor.0.saturating_sub(height.saturating_sub(1));
    let row = cursor.0.saturating_sub(first) as u16;
    let column = if lines.is_empty() { 0 } else { cursor.1 as u16 };
    (
        area.x + 2 + column.min(area.width.saturating_sub(3)),
        area.y + row.min(area.height.saturating_sub(1)),
    )
}

// ---------------------------------------------------------------------------
// Conversation
// ---------------------------------------------------------------------------

fn transcript_view(frame: &mut Frame, area: Rect, app: &mut App) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let theme = app.theme;
    let width = area.width as usize;
    let selected = app.selected_entry;
    let focus_main = app.focus == Focus::Main;
    let expanded = app.expanded.clone();
    // Detailed mode collapses nothing, so every entry is drawn as if it had been expanded.
    let details = app.prefs.details;
    let welcome = welcome_lines(app, width);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut offsets: Vec<(usize, usize)> = Vec::new();
    // Code no earlier frame highlighted takes a bounded share of each frame, so a long
    // conversation opens at once and the frames after it complete its colours.
    let work = highlight::Frame::begin(highlight::FRAME_WORK);
    {
        let entries = app.entries();
        if entries.is_empty() {
            lines = welcome;
        } else {
            for (index, entry) in entries.iter().enumerate() {
                let start = lines.len();
                let is_selected = focus_main && index == selected;
                let is_expanded = details || entry.seq.is_some_and(|seq| expanded.contains(&seq));
                let rendered = transcript::render(entry, width, &theme, is_selected, is_expanded);
                offsets.push((start, rendered.len()));
                lines.extend(rendered);
            }
        }
    }
    app.redraw |= work.deferred();
    drop(work);
    let height = area.height as usize;
    // A short conversation sits at the bottom, next to the composer, the way a chat reads.
    // The welcome text stays at the top, because it is a page rather than a conversation.
    let pad = if offsets.is_empty() {
        0
    } else {
        height.saturating_sub(lines.len())
    };
    if pad > 0 {
        let mut padded = vec![Line::default(); pad];
        padded.extend(lines);
        lines = padded;
        for entry in &mut offsets {
            entry.0 += pad;
        }
    }
    app.viewport.height = height;
    app.viewport.total = lines.len();
    app.viewport.entries = offsets;
    let max_top = lines.len().saturating_sub(height);
    let top = if app.follow {
        max_top
    } else {
        app.top.min(max_top)
    };
    app.top = top;
    let visible: Vec<Line<'static>> = lines.into_iter().skip(top).take(height).collect();
    frame::paint(frame, area, visible);
}

fn welcome_lines(app: &App, width: usize) -> Vec<Line<'static>> {
    let theme = &app.theme;
    let members = app.config.members();
    let team = if members.is_empty() {
        "none enabled · /team add ID".to_owned()
    } else {
        members
            .iter()
            .map(|m| app.agent_label(&m.id))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let facts = [
        ("working directory", app.cwd.display().to_string()),
        ("how it is used", "directly, no copy kept".to_owned()),
        ("preferred agents", team),
        ("theme", theme.name.to_owned()),
    ];
    let mut lines = vec![
        Line::default(),
        Line::from(Span::styled(
            "  A team of local agents, working in this directory.".to_owned(),
            theme.bold(),
        )),
        Line::default(),
    ];
    for (label, value) in facts {
        lines.push(text::row(
            width.min(78),
            vec![Span::styled(format!("  {label:<20}"), theme.muted())],
            vec![Span::styled(
                text::truncate(&value, width / 2),
                theme.body(),
            )],
        ));
    }
    lines.push(Line::default());
    for hint in [
        "Describe a task and press Enter. Agents create and change files in this directory itself.",
        "ymp records a path and a hash for each change, never earlier content, so it cannot put a file back. /diff states what a run recorded and where.",
        "Ctrl+P opens the command palette. Ctrl+T changes the colour theme.",
        "Every page opens with its command, such as /tasks, or from the palette. Pages are read-only; none of them start an agent.",
    ] {
        for piece in text::wrap(hint, width.saturating_sub(4).max(8)) {
            lines.push(Line::from(vec![
                Span::raw("  ".to_owned()),
                Span::styled(piece, theme.faint()),
            ]));
        }
    }
    lines
}

// ---------------------------------------------------------------------------
// Pages
// ---------------------------------------------------------------------------

fn page_view(frame: &mut Frame, area: Rect, app: &mut App) {
    if area.width < 4 || area.height < 3 {
        return;
    }
    let theme = app.theme;
    let focused = app.focus == Focus::Main;
    // One width for the whole page: the tables and the empty state land in rects of exactly
    // this width, so nothing is laid out wider than the rect it reaches.
    let inner_width = frame::page_content_width(area.width);
    // Building the page can move the selection onto the first selectable row, so the frame
    // reads the selection afterwards. Otherwise the first frame would paint a selection the
    // keyboard has already left.
    app.page(inner_width);
    let selected = app.page_selected;
    let filter = app.table.filter.clone();
    let typing = app.table.typing;
    let unfiltered = app.table.unfiltered;

    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(area);

    let (title, command, subtitle, lines, headers, selected_line, empty, selectable, shown) = {
        let page = app.page(inner_width);
        let (lines, headers, selected_line) =
            table_lines(page, inner_width as usize, &theme, selected, focused);
        (
            page.title.clone(),
            page.view.command(),
            page.subtitle.clone(),
            lines,
            headers,
            selected_line,
            page.empty.clone(),
            page.selectable(),
            page.items.iter().filter(|item| item.is_record()).count(),
        )
    };

    // The page's name, then the filter and how many rows are shown, as k9s writes them:
    // `Tasks</word>[3]`.
    let mut left = vec![
        Span::styled(" ".to_owned(), theme.faint()),
        Span::styled(title, theme.accent_bold()),
    ];
    if !filter.is_empty() {
        left.push(Span::styled(format!("</{filter}>"), theme.accent()));
    }
    let count = if app.view == View::Files {
        app.files.is_open().then(|| app.files.shown())
    } else {
        (unfiltered > 0).then_some(shown)
    };
    if let Some(count) = count {
        left.push(Span::styled(format!("[{count}]"), theme.muted()));
    }
    left.push(Span::styled(format!("  {command}"), theme.faint()));
    frame::header(
        frame,
        split[0],
        left,
        vec![Span::styled(format!("{subtitle} "), theme.muted())],
    );
    if typing {
        frame::row(
            frame,
            split[1],
            vec![
                Span::styled(" / ".to_owned(), theme.accent_bold()),
                Span::styled(filter.clone(), theme.text()),
                Span::styled(" ".to_owned(), theme.selected()),
            ],
            frame::key_hints(&[("Enter", "keep"), ("Esc", "clear")], &theme),
        );
    } else {
        frame::hairline_with(frame, split[1], &theme, theme.border(focused));
    }
    let body = split[2];

    if app.view == View::Files {
        files_body(frame, body, app, inner_width, focused, empty, &filter);
        return;
    }

    if !selectable {
        let inset = Rect {
            x: body.x + frame::PAGE_INSET,
            y: body.y + 1,
            width: inner_width,
            height: body.height.saturating_sub(1),
        };
        let shown = if !filter.trim().is_empty() && unfiltered > 0 {
            // The filter removed every row. The page's own empty state would say there is
            // nothing here at all, which is not what happened.
            vec![
                Line::from(Span::styled(
                    format!("Nothing on this page matches /{filter}"),
                    theme.text(),
                )),
                Line::from(Span::styled(
                    "Esc clears the filter.".to_owned(),
                    theme.faint(),
                )),
            ]
        } else if empty.is_empty() {
            lines
        } else {
            empty
        };
        frame::paint(frame, inset, shown);
        app.viewport.height = body.height as usize;
        return;
    }

    // The tables take the whole page. A row's full record opens in a popup.
    let list_area = Rect {
        x: body.x + 1,
        y: body.y,
        width: inner_width,
        height: body.height,
    };
    let visible = body.height as usize;
    let mut top = app.page_top.min(lines.len().saturating_sub(1));
    if selected_line < top {
        top = selected_line;
    } else if visible > 0 && selected_line >= top + visible {
        top = selected_line + 1 - visible;
    }
    // Column titles stay on screen, as in k9s. When the titles of the table under the top line
    // have scrolled away they are painted over that line, so the view keeps the selected row
    // below them.
    let covered = |top: usize| {
        headers
            .get(top)
            .copied()
            .flatten()
            .filter(|header| *header < top)
    };
    if top > 0 && selected_line == top && covered(top).is_some() {
        top -= 1;
    }
    app.page_top = top;
    app.viewport.height = visible;
    let mut shown: Vec<Line<'static>> = lines.iter().skip(top).take(visible).cloned().collect();
    if let (Some(header), Some(first)) = (covered(top), shown.first_mut()) {
        *first = lines[header].clone();
    }
    frame::paint(frame, list_area, shown);
}

/// The files page below its header: why the last move failed, when it did, then the explorer's
/// own list, and in place of rows that are not there, what is true instead.
fn files_body(
    frame: &mut Frame,
    body: Rect,
    app: &mut App,
    inner_width: u16,
    focused: bool,
    empty: Vec<Line<'static>>,
    filter: &str,
) {
    let theme = app.theme;
    app.viewport.height = body.height as usize;
    if !app.files.is_open() {
        let inset = Rect {
            x: body.x + frame::PAGE_INSET,
            y: body.y + 1,
            width: inner_width,
            height: body.height.saturating_sub(1),
        };
        frame::paint(frame, inset, empty);
        return;
    }
    let notes: Vec<Line<'static>> = app
        .files
        .notice()
        .map(|notice| {
            text::wrap(
                &text::sanitize(&format!("{} {notice}", theme.markers.warn)),
                inner_width.max(1) as usize,
            )
            .into_iter()
            .map(|piece| Line::from(Span::styled(piece, theme.warn())))
            .collect()
        })
        .unwrap_or_default();
    let noted = (notes.len() as u16).min(body.height / 2);
    frame::paint(
        frame,
        Rect {
            x: body.x + frame::PAGE_INSET,
            y: body.y,
            width: inner_width,
            height: noted,
        },
        notes,
    );
    let list = Rect {
        x: body.x + 1,
        y: body.y + noted,
        width: inner_width,
        height: body.height - noted,
    };
    let instead = if app.files.shown() > 0 {
        Vec::new()
    } else if !filter.trim().is_empty() {
        vec![
            Line::from(Span::styled(
                format!("Nothing in this directory matches /{filter}"),
                theme.text(),
            )),
            Line::from(Span::styled(
                "Esc clears the filter.".to_owned(),
                theme.faint(),
            )),
        ]
    } else {
        vec![
            Line::from(Span::styled(
                "This directory is empty".to_owned(),
                theme.bold(),
            )),
            Line::from(Span::styled(
                "Backspace goes to the parent directory.".to_owned(),
                theme.faint(),
            )),
        ]
    };
    // Only the rows there are take room when something is said beneath them.
    let rows = if instead.is_empty() {
        list.height
    } else {
        (app.files.rows() as u16).min(list.height)
    };
    app.files.style(&theme, focused);
    if let Some(widget) = app.files.widget() {
        frame.render_widget_ref(
            widget,
            Rect {
                height: rows,
                ..list
            },
        );
    }
    if !instead.is_empty() {
        let gap = rows.saturating_add(1).min(list.height);
        frame::paint(
            frame,
            Rect {
                x: body.x + frame::PAGE_INSET,
                y: list.y + gap,
                width: inner_width,
                height: list.height - gap,
            },
            instead,
        );
    }
    app.viewport.height = list.height as usize;
}

/// Lay out a page's tables: for each its title, its column titles and its rows.
///
/// Also returns, for every line, the line holding the column titles of the table it belongs to,
/// and the line of the selected row.
fn table_lines(
    page: &Page,
    width: usize,
    theme: &Theme,
    selected: usize,
    focused: bool,
) -> (Vec<Line<'static>>, Vec<Option<usize>>, usize) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut headers: Vec<Option<usize>> = Vec::new();
    let mut selected_line = 0usize;
    let mut table: Option<(usize, Vec<Option<usize>>)> = None;
    let mut header: Option<usize> = None;
    // Cells after the selection marker. The widths leave that marker its room, so a row is
    // never wider than the page and nothing is cut at its edge.
    let body = width.saturating_sub(table::MARKER);
    for (index, item) in page.items.iter().enumerate() {
        match item.kind {
            ItemKind::Heading => {
                if !lines.is_empty() {
                    lines.push(Line::default());
                    headers.push(None);
                }
                if !item.title.is_empty() {
                    lines.push(Line::from(vec![
                        Span::raw("  ".to_owned()),
                        Span::styled(
                            text::truncate(&item.title.to_uppercase(), body),
                            theme.muted(),
                        ),
                    ]));
                    headers.push(None);
                }
                let cells: Vec<&[Cell]> = page.items[index + 1..]
                    .iter()
                    .take_while(|row| row.kind == ItemKind::Row)
                    .map(|row| row.cells.as_slice())
                    .collect();
                let widths = table::widths(&item.columns, &cells, item.sort, body);
                header = (!item.columns.is_empty()).then_some(lines.len());
                if header.is_some() {
                    lines.push(table::header(
                        &item.columns,
                        &widths,
                        &item.sort_keys,
                        item.sort,
                        theme,
                    ));
                    headers.push(header);
                }
                table = Some((index, widths));
            }
            ItemKind::Row => {
                let is_selected = index == selected;
                if is_selected {
                    selected_line = lines.len();
                }
                let (columns, widths): (&[Column], &[Option<usize>]) = match &table {
                    Some((heading, widths)) => (&page.items[*heading].columns, widths),
                    None => (&[], &[]),
                };
                lines.push(table::row(
                    columns,
                    widths,
                    &item.cells,
                    width,
                    theme,
                    is_selected,
                    focused,
                ));
                headers.push(header);
            }
        }
    }
    (lines, headers, selected_line)
}

// ---------------------------------------------------------------------------
// Floating surfaces
// ---------------------------------------------------------------------------

/// The inline completion list shown while a command name is being typed.
fn completion_popup(frame: &mut Frame, main: Rect, composer: Rect, app: &App) -> bool {
    let matches = app.completions();
    if matches.is_empty() || app.overlay.is_some() || app.focus != Focus::Composer {
        return false;
    }
    let theme = &app.theme;
    let visible = matches.len().min(COMPLETION_ROWS);
    let height = visible as u16 + 2;
    if composer.y <= main.y + height || main.width < 30 {
        return false;
    }
    let width = main.width.saturating_sub(2).min(COMPLETION_MAX_WIDTH);
    let area = Rect {
        x: main.x + 1,
        y: composer.y.saturating_sub(height + 1),
        width,
        height,
    };
    let selected = app.completion.min(matches.len() - 1);
    let first = selected.saturating_sub(visible - 1);
    let room = frame::modal_content_width(width) as usize;
    let name_width = command_name_width();
    let body = matches
        .iter()
        .enumerate()
        .skip(first)
        .take(visible)
        .map(|(index, command)| command_line(command, name_width, room, index == selected, theme))
        .collect::<Vec<_>>();
    frame::clear_around(frame, area, &[main], theme);
    frame.render_widget(ratatui::widgets::Clear, area);
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(theme.rule())
        .style(theme.surface())
        .title_top(Line::from(Span::styled(
            " Commands ".to_owned(),
            theme.muted(),
        )))
        .title_top(
            Line::from(Span::styled(
                format!(" {} of {} ", selected + 1, matches.len()),
                theme.faint(),
            ))
            .right_aligned(),
        );
    let inner = frame::padded(block.inner(area));
    frame.render_widget(block, area);
    frame::paint_list(frame, inner, body, Some(selected - first), theme);
    false
}

/// Rows of commands the inline completion list shows at once.
const COMPLETION_ROWS: usize = 6;
/// The widest the inline completion list grows, border included.
const COMPLETION_MAX_WIDTH: u16 = 88;
/// The most commands the palette lists at once, when the terminal has the rows for them.
const PALETTE_ROWS: usize = 10;
/// The most rows an owner chooser gives the explanation of its chosen option.
const CHOICE_EXPLANATION_ROWS: usize = 3;
/// The most themes the chooser lists at once. With the chosen theme's summary, the count below
/// and the hint, the chooser then fits inside the body of a 24-row terminal.
const THEME_ROWS: usize = 9;

/// Cells the longest command name takes, so every summary in a list starts in one column
/// however the list is filtered.
fn command_name_width() -> usize {
    crate::commands::COMMANDS
        .iter()
        .map(|command| text::width(command.name))
        .max()
        .unwrap_or(0)
}

/// One command in a list `width` cells wide: the selection marker, the name in a column
/// `name_width` cells wide, then as much of the summary as fits. [`frame::paint_list`] highlights
/// the chosen row across its whole width.
fn command_line(
    command: &crate::commands::Command,
    name_width: usize,
    width: usize,
    chosen: bool,
    theme: &Theme,
) -> Line<'static> {
    let style = if chosen {
        theme.selected()
    } else {
        theme.body()
    };
    let marker = if chosen { theme.markers.selection } else { " " };
    let lead = text::truncate(&format!("{marker} {:<name_width$}  ", command.name), width);
    let summary = text::truncate(command.summary, width.saturating_sub(text::width(&lead)));
    Line::from(vec![
        Span::styled(lead, style),
        Span::styled(summary, if chosen { style } else { theme.faint() }),
    ])
}

/// The row under a list that does not fit its surface: how many entries follow the last one shown,
/// or a blank row once the last is shown, so moving the selection never changes the surface's
/// height.
fn more_line(below: usize, theme: &Theme) -> Line<'static> {
    if below == 0 {
        return Line::default();
    }
    Line::from(Span::styled(
        format!("{} {below} more", theme.markers.more),
        theme.faint(),
    ))
}

/// Draw whichever floating surface is open, and return where its cursor belongs.
fn overlay(frame: &mut Frame, area: Rect, regions: &[Rect], app: &App) -> Option<(u16, u16)> {
    let theme = &app.theme;
    let overlay = app.overlay.as_ref()?;
    match overlay {
        Overlay::Themes { selected, .. } => {
            let width = 58u16.min(area.width.saturating_sub(4));
            let inner = frame::modal_content_width(width) as usize;
            // Names are padded to the longest, so every kind starts in one column.
            let name_width = theme::catalog()
                .iter()
                .map(|palette| text::width(palette.name))
                .max()
                .unwrap_or(0);
            let kind_width = text::width(theme::ThemeKind::Light.label());
            let themes = theme::catalog();
            // The list takes no more rows than the surface shows, keeping one for the count of
            // the themes below when they do not all fit, so the selection never sits on a row the
            // surface cuts off. Only the chosen theme's summary follows its row, below it, and it
            // is given the rows of the longest summary, so the surface keeps one height whichever
            // theme is chosen.
            let rows = frame::modal_body_rows(area.height, true);
            let mut visible = THEME_ROWS.min(themes.len());
            if visible + usize::from(themes.len() > visible) > rows {
                visible = rows.saturating_sub(1).max(1);
            }
            let first = selected.saturating_sub(visible.saturating_sub(1));
            let summary_width = inner.saturating_sub(2);
            let summary_rows = themes
                .iter()
                .map(|palette| text::wrap(palette.summary, summary_width).len())
                .max()
                .unwrap_or(0);
            let mut summary_shown = 0;
            let mut chosen_line = None;
            let mut body = Vec::new();
            for (index, palette) in themes.iter().enumerate().skip(first).take(visible) {
                let chosen = index == *selected;
                let style = if chosen {
                    chosen_line = Some(body.len());
                    theme.selected()
                } else {
                    theme.body()
                };
                body.push(text::row(
                    inner,
                    vec![
                        Span::styled(
                            format!(
                                "{} {:<name_width$}",
                                if chosen { theme.markers.selection } else { " " },
                                palette.name
                            ),
                            style,
                        ),
                        Span::styled(
                            format!("  {:<kind_width$}", palette.kind.label()),
                            if chosen { style } else { theme.muted() },
                        ),
                    ],
                    swatches(palette, inner.saturating_sub(name_width + kind_width + 5)),
                ));
                if chosen {
                    let pieces = text::wrap(palette.summary, summary_width);
                    summary_shown = pieces.len();
                    for piece in pieces {
                        body.push(Line::from(vec![
                            Span::raw("  ".to_owned()),
                            Span::styled(piece, theme.faint()),
                        ]));
                    }
                }
            }
            body.extend(std::iter::repeat_n(
                Line::default(),
                summary_rows.saturating_sub(summary_shown),
            ));
            if themes.len() > visible {
                let below = themes.len().saturating_sub(first + visible);
                body.push(more_line(below, theme));
            }
            body.push(Line::default());
            body.push(Line::from(Span::styled(
                "The whole interface previews as you move.".to_owned(),
                theme.faint(),
            )));
            frame::render_modal(
                frame,
                area,
                regions,
                &ModalSpec {
                    title: "Colour theme".into(),
                    badge: "saved on Enter".into(),
                    role: ModalRole::Choice,
                    width,
                    body,
                    footer: vec![("Up/Down", "preview"), ("Enter", "keep"), ("Esc", "cancel")],
                    scroll: 0,
                    selected: chosen_line,
                },
                theme,
            );
            None
        }
        Overlay::Palette { field, selected } => {
            let width = 72u16.min(area.width.saturating_sub(4));
            let inner = frame::modal_content_width(width) as usize;
            let matches = crate::commands::search(&field.value);
            let mut body = vec![
                Line::from(vec![
                    Span::styled(format!("{} ", theme.markers.prompt), theme.accent_bold()),
                    if field.value.is_empty() {
                        Span::styled("Type to search commands".to_owned(), theme.faint())
                    } else {
                        Span::styled(field.value.clone(), theme.text())
                    },
                ]),
                Line::from(Span::styled(
                    theme.markers.hline.repeat(inner),
                    theme.rule(),
                )),
            ];
            if matches.is_empty() {
                body.push(Line::from(Span::styled(
                    "Nothing matches.".to_owned(),
                    theme.muted(),
                )));
            }
            let selected = (*selected).min(matches.len().saturating_sub(1));
            // The list takes no more rows than the surface shows under the search line, keeping
            // one for the count of the commands below when they do not all fit, so the selection
            // never sits on a row the surface cuts off. That row stays, blank, once the last
            // command is shown.
            let rows = frame::modal_body_rows(area.height, true).saturating_sub(body.len());
            let mut visible = PALETTE_ROWS.min(matches.len());
            if visible + usize::from(matches.len() > visible) > rows {
                visible = rows.saturating_sub(1).max(1);
            }
            let first = selected.saturating_sub(visible.saturating_sub(1));
            let name_width = command_name_width();
            let mut chosen_line = None;
            for (index, command) in matches.iter().enumerate().skip(first).take(visible) {
                if index == selected {
                    chosen_line = Some(body.len());
                }
                body.push(command_line(
                    command,
                    name_width,
                    inner,
                    index == selected,
                    theme,
                ));
            }
            if matches.len() > visible {
                let below = matches.len().saturating_sub(first + visible);
                body.push(more_line(below, theme));
            }
            let rect = frame::render_modal(
                frame,
                area,
                regions,
                &ModalSpec {
                    title: "Commands".into(),
                    badge: format!("{} of {}", matches.len(), crate::commands::COMMANDS.len()),
                    role: ModalRole::Choice,
                    width,
                    body,
                    footer: vec![("Enter", "run"), ("Up/Down", "select"), ("Esc", "close")],
                    scroll: 0,
                    selected: chosen_line,
                },
                theme,
            );
            Some((
                rect.x + 2 + text::width(&field.value[..field.cursor]) as u16,
                rect.y,
            ))
        }
        Overlay::Inspect {
            title,
            body,
            scroll,
        } => {
            frame::render_modal(
                frame,
                area,
                regions,
                &ModalSpec {
                    title: title.clone(),
                    badge: "read only".into(),
                    role: ModalRole::Reference,
                    width: frame::inspect_width(area.width),
                    body: body.clone(),
                    footer: vec![("Up/Down", "scroll"), ("Esc", "close")],
                    scroll: *scroll,
                    selected: None,
                },
                theme,
            );
            None
        }
        Overlay::Preview {
            title,
            body,
            scroll,
        } => {
            frame::render_modal(
                frame,
                area,
                regions,
                &ModalSpec {
                    title: title.clone(),
                    badge: "read only".into(),
                    role: ModalRole::Reference,
                    width: frame::preview_width(area.width),
                    body: body.clone(),
                    footer: vec![
                        ("Up/Down", "scroll"),
                        ("PageUp/PageDown", "page"),
                        ("Esc", "back to the list"),
                    ],
                    scroll: *scroll,
                    selected: None,
                },
                theme,
            );
            None
        }
        Overlay::Prompt {
            target,
            label,
            help,
            field,
        } => {
            let width = 76u16.min(area.width.saturating_sub(4));
            let inner = frame::modal_content_width(width) as usize;
            let (field_lines, (cursor_row, cursor_col)) =
                text::compose(&field.value, inner.saturating_sub(2).max(1), field.cursor);
            let visible_rows = usize::from(area.height.saturating_sub(8).clamp(1, 10));
            let first_row = cursor_row.saturating_sub(visible_rows.saturating_sub(1));
            let mut body = field_lines
                .iter()
                .skip(first_row)
                .take(visible_rows)
                .enumerate()
                .map(|(i, line)| {
                    Line::from(vec![
                        Span::styled(
                            if i == 0 {
                                format!("{} ", theme.markers.prompt)
                            } else {
                                "  ".into()
                            },
                            theme.accent_bold(),
                        ),
                        Span::styled(line.clone(), theme.text()),
                    ])
                })
                .collect::<Vec<_>>();
            body.push(Line::default());
            for piece in text::wrap(help, inner) {
                body.push(Line::from(Span::styled(piece, theme.faint())));
            }
            let rect = frame::render_modal(
                frame,
                area,
                regions,
                &ModalSpec {
                    title: label.clone(),
                    badge: "saved on Enter".into(),
                    role: ModalRole::Choice,
                    width,
                    body,
                    footer: if matches!(target, PromptTarget::AgentInstructions(_)) {
                        vec![
                            ("Enter", "save"),
                            ("Alt+Enter", "newline"),
                            ("Esc", "cancel"),
                        ]
                    } else {
                        vec![("Enter", "save"), ("Esc", "cancel")]
                    },
                    scroll: 0,
                    selected: None,
                },
                theme,
            );
            Some((
                rect.x + 2 + cursor_col as u16,
                rect.y + (cursor_row - first_row) as u16,
            ))
        }
        Overlay::GitChoice {
            purpose,
            options,
            selected,
        } => {
            use crate::git_view::Purpose;
            let width = 76u16.min(area.width.saturating_sub(4));
            let inner = frame::modal_content_width(width) as usize;
            // As in the palette: no more rows than the surface shows, keeping one for the count of
            // those below, blank once the last is shown, so the selection never sits on a row the
            // surface cuts off and the surface keeps its height.
            let rows = frame::modal_body_rows(area.height, true);
            let mut visible = options.len();
            if visible > rows {
                visible = rows.saturating_sub(1).max(1);
            }
            let first = selected.saturating_sub(visible.saturating_sub(1));
            let mut body = Vec::new();
            let mut chosen_line = None;
            if options.is_empty() {
                let none = match purpose {
                    Purpose::Worktree => "Git lists no worktree here.",
                    Purpose::Branch => "Git lists no local branch here.",
                };
                body.push(Line::from(Span::styled(none.to_owned(), theme.muted())));
            }
            // A detail never takes more than a third of the row, so a long branch name cannot push
            // the path out of sight, and a worktree path keeps its end, where worktrees that share a
            // long prefix differ.
            let detail_room = (inner / 3).max(8);
            for (index, option) in options.iter().enumerate().skip(first).take(visible) {
                let chosen = index == *selected;
                let style = if chosen {
                    chosen_line = Some(body.len());
                    theme.selected()
                } else if option.enabled || option.current {
                    theme.body()
                } else {
                    theme.faint()
                };
                let marker = if chosen { theme.markers.selection } else { " " };
                let detail = text::truncate(&option.detail, detail_room);
                let room = inner.saturating_sub(text::width(marker) + 1 + text::width(&detail) + 2);
                let label = match purpose {
                    Purpose::Worktree => text::last_cells(&option.label, room),
                    Purpose::Branch => text::truncate(&option.label, room),
                };
                body.push(text::row(
                    inner,
                    vec![Span::styled(format!("{marker} {label}"), style)],
                    vec![Span::styled(
                        detail,
                        if chosen { style } else { theme.muted() },
                    )],
                ));
            }
            if options.len() > visible {
                let below = options.len().saturating_sub(first + visible);
                body.push(more_line(below, theme));
            }
            let (title, badge, action) = match purpose {
                Purpose::Worktree => ("Inspect a worktree", "read only", "inspect"),
                Purpose::Branch => ("Switch branch", "checks out", "switch"),
            };
            frame::render_modal(
                frame,
                area,
                regions,
                &ModalSpec {
                    title: title.into(),
                    badge: badge.into(),
                    role: ModalRole::Choice,
                    width,
                    body,
                    footer: vec![("Up/Down", "select"), ("Enter", action), ("Esc", "cancel")],
                    scroll: 0,
                    selected: chosen_line,
                },
                theme,
            );
            None
        }
        Overlay::Choose {
            title,
            badge,
            options,
            selected,
        } => {
            let width = 76u16.min(area.width.saturating_sub(4));
            let inner = frame::modal_content_width(width) as usize;
            // The chosen option's explanation follows the list, given the rows of the longest so
            // the surface keeps one height whichever option is chosen.
            let explain = |option: &crate::state::ChoiceOption| {
                let mut pieces =
                    text::wrap(option.reason.as_deref().unwrap_or(&option.detail), inner);
                pieces.truncate(CHOICE_EXPLANATION_ROWS);
                pieces
            };
            let explanation_rows = options.iter().map(|o| explain(o).len()).max().unwrap_or(0);
            let rows = frame::modal_body_rows(area.height, true)
                .saturating_sub(explanation_rows + 1)
                .max(1);
            let mut visible = options.len();
            if visible > rows {
                visible = rows.saturating_sub(1).max(1);
            }
            let first = selected.saturating_sub(visible.saturating_sub(1));
            let mut body = Vec::new();
            let mut chosen_line = None;
            for (index, option) in options.iter().enumerate().skip(first).take(visible) {
                let chosen = index == *selected;
                let enabled = option.reason.is_none();
                let style = if chosen {
                    chosen_line = Some(body.len());
                    theme.selected()
                } else if enabled {
                    theme.body()
                } else {
                    theme.faint()
                };
                let marker = if chosen { theme.markers.selection } else { " " };
                let tag = if enabled { "" } else { "unavailable" };
                let room = inner.saturating_sub(text::width(marker) + 1 + text::width(tag) + 2);
                body.push(text::row(
                    inner,
                    vec![Span::styled(
                        format!("{marker} {}", text::truncate(&option.label, room)),
                        style,
                    )],
                    vec![Span::styled(
                        tag.to_owned(),
                        if chosen { style } else { theme.warn() },
                    )],
                ));
            }
            if options.len() > visible {
                let below = options.len().saturating_sub(first + visible);
                body.push(more_line(below, theme));
            }
            body.push(Line::default());
            let shown = options.get(*selected).map(explain).unwrap_or_default();
            let padding = explanation_rows.saturating_sub(shown.len());
            let reason = options.get(*selected).is_some_and(|o| o.reason.is_some());
            for piece in shown {
                body.push(Line::from(Span::styled(
                    piece,
                    if reason { theme.warn() } else { theme.faint() },
                )));
            }
            body.extend(std::iter::repeat_n(Line::default(), padding));
            frame::render_modal(
                frame,
                area,
                regions,
                &ModalSpec {
                    title: title.clone(),
                    badge: badge.clone(),
                    role: ModalRole::Choice,
                    width,
                    body,
                    footer: vec![
                        ("Up/Down", "select"),
                        ("Enter", "choose"),
                        ("Esc", "cancel"),
                    ],
                    scroll: 0,
                    selected: chosen_line,
                },
                theme,
            );
            None
        }
        Overlay::Confirm { question, target } => {
            let width = 68u16.min(area.width.saturating_sub(4));
            // A question may name several facts, one to a line.
            let body = question
                .lines()
                .flat_map(|line| text::wrap(line, frame::modal_content_width(width) as usize))
                .map(|piece| Line::from(Span::styled(piece, theme.body())))
                .collect();
            frame::render_modal(
                frame,
                area,
                regions,
                &ModalSpec {
                    title: "Confirm".into(),
                    badge: match target {
                        Confirm::ForgetMemory(_) => "cannot be undone",
                        Confirm::SwitchBranch { .. } => "checks out",
                        Confirm::Owner { badge, .. } => badge,
                    }
                    .into(),
                    role: ModalRole::Choice,
                    width,
                    body,
                    footer: vec![("y", "yes"), ("n", "no"), ("Esc", "cancel")],
                    scroll: 0,
                    selected: None,
                },
                theme,
            );
            None
        }
    }
}

/// As many two-cell colour chips of a palette as fit in `room` cells.
fn swatches(palette: &Theme, room: usize) -> Vec<Span<'static>> {
    [
        palette.accent,
        palette.good,
        palette.warn,
        palette.bad,
        palette.info,
        palette.surface,
    ]
    .into_iter()
    .take(room / 2)
    .map(|color| Span::styled("  ".to_owned(), Style::default().bg(color)))
    .collect()
}
