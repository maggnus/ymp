//! Composition: state becomes the lines and rectangles the frame layer draws.
//!
//! The layout is chat-first with a right sidebar. From the top: a context header, the body
//! split into the main column and the sidebar, the composer, and a status row. Nothing here
//! invents a value; every span carries text that came from the state or names a key.

use crate::frame::{self, ModalRole, ModalSpec};
use crate::sidebar;
use crate::state::{App, Focus, Overlay, PromptTarget};
use crate::text;
use crate::theme::{self, Theme};
use crate::transcript;
use crate::usage;
use crate::views::{self, ItemKind, View};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::Frame;

/// The tallest the composer may grow before it starts scrolling.
const COMPOSER_MAX: u16 = 5;

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
        frame::vertical_rule(frame, rule, &theme, app.focus == Focus::Sidebar);
        frame::fill_surface(frame, panel, &theme);
        let inner = Rect {
            x: panel.x + 1,
            y: panel.y,
            width: panel.width.saturating_sub(2),
            height: panel.height,
        };
        let lines = sidebar::lines(
            app,
            inner.width as usize,
            inner.height as usize,
            app.focus == Focus::Sidebar,
        );
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
        cursor = overlay(frame, area, app);
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
    left.push(Span::styled(
        text::one_line(&app.status),
        if app.active {
            theme.body()
        } else {
            theme.faint()
        },
    ));
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
        (_, Focus::Sidebar) => vec![("Enter", "open"), ("Tab", "focus"), ("Esc", "composer")],
        (View::Chat, Focus::Composer) => {
            vec![("Enter", "send"), ("Ctrl+P", "commands"), ("Tab", "focus")]
        }
        (View::Chat, Focus::Main) => vec![
            ("Enter", "inspect"),
            ("Space", "expand"),
            ("Esc", "composer"),
        ],
        (_, Focus::Composer) => vec![("Enter", "send"), ("Tab", "focus"), ("Esc", "back")],
        (_, Focus::Main) if !page_hints.is_empty() => page_hints.to_vec(),
        (_, _) => vec![("Enter", "open"), ("Esc", "back"), ("Tab", "focus")],
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
    let welcome = welcome_lines(app, width);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut offsets: Vec<(usize, usize)> = Vec::new();
    {
        let entries = app.entries();
        if entries.is_empty() {
            lines = welcome;
        } else {
            for (index, entry) in entries.iter().enumerate() {
                let start = lines.len();
                let is_selected = focus_main && index == selected;
                let is_expanded = entry.seq.is_some_and(|seq| expanded.contains(&seq));
                let rendered = transcript::render(entry, width, &theme, is_selected, is_expanded);
                offsets.push((start, rendered.len()));
                lines.extend(rendered);
            }
        }
    }
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
        "none configured · /team add ID".to_owned()
    } else {
        members
            .iter()
            .map(|m| views::actor_name(&app.config, &app.pool, &app.records, &m.id))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let facts = [
        ("working directory", app.cwd.display().to_string()),
        ("how it is used", "directly, no copy kept".to_owned()),
        ("team", team),
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
        "Every page reachable from the sidebar is read-only; none of them start an agent.",
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
    // One width for the whole page: the rows, the detail pane and the empty state all land
    // in rects of exactly this width, so nothing is wrapped wider than the rect it reaches.
    let inner_width = frame::page_content_width(area.width);
    // Building the page can move the selection onto the first selectable row, so the frame
    // reads the selection afterwards. Otherwise the first frame would paint a selection the
    // keyboard has already left, and the detail of the selected row would be missing from it.
    app.page(inner_width);
    let selected = app.page_selected;

    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(area);

    let (title, command, subtitle, rows, detail, selected_row, empty, selectable) = {
        let page = app.page(inner_width);
        let mut rows: Vec<Line<'static>> = Vec::new();
        let mut selected_row = 0usize;
        for (index, item) in page.items.iter().enumerate() {
            let is_selected = index == selected && item.kind == ItemKind::Row;
            if is_selected {
                selected_row = rows.len();
            }
            rows.push(item_line(
                item,
                inner_width as usize,
                &theme,
                is_selected,
                focused,
            ));
        }
        let detail = page
            .items
            .get(selected)
            .filter(|item| item.kind == ItemKind::Row)
            .map(|item| item.detail.clone())
            .unwrap_or_default();
        (
            page.title.clone(),
            page.view.command(),
            page.subtitle.clone(),
            rows,
            detail,
            selected_row,
            page.empty.clone(),
            page.selectable(),
        )
    };

    frame::header(
        frame,
        split[0],
        vec![
            Span::styled(" ".to_owned(), theme.faint()),
            Span::styled(title, theme.accent_bold()),
            Span::styled(format!("  {command}"), theme.faint()),
        ],
        vec![Span::styled(format!("{subtitle} "), theme.muted())],
    );
    frame::hairline_with(frame, split[1], &theme, theme.border(focused));
    let body = split[2];

    if !selectable {
        let inset = Rect {
            x: body.x + frame::PAGE_INSET,
            y: body.y + 1,
            width: inner_width,
            height: body.height.saturating_sub(1),
        };
        frame::paint(frame, inset, if empty.is_empty() { rows } else { empty });
        app.viewport.height = body.height as usize;
        return;
    }

    let detail_height = if detail.is_empty() || body.height < 12 {
        0
    } else {
        (detail.len() as u16 + 1).min(body.height / 2).max(2)
    };
    let list_height = body.height.saturating_sub(detail_height);
    let list_area = Rect {
        x: body.x + 1,
        y: body.y,
        // The marker column is the row's own indent. The width is the page width, so a row
        // keeps the same right margin the detail pane below it has.
        width: inner_width,
        height: list_height,
    };

    let visible = list_height as usize;
    let mut top = app.page_top.min(rows.len().saturating_sub(1));
    if selected_row < top {
        top = selected_row;
    } else if visible > 0 && selected_row >= top + visible {
        top = selected_row + 1 - visible;
    }
    app.page_top = top;
    app.viewport.height = visible;
    frame::paint(
        frame,
        list_area,
        rows.into_iter().skip(top).take(visible).collect(),
    );

    if detail_height > 0 {
        let rule = Rect {
            x: body.x,
            y: body.y + list_height,
            width: body.width,
            height: 1,
        };
        frame::hairline(frame, rule, &theme);
        // Two columns of indent on each side: exactly the width the page wrapped for.
        let detail_area = Rect {
            x: body.x + frame::PAGE_INSET,
            y: rule.y + 1,
            width: inner_width,
            height: detail_height.saturating_sub(1),
        };
        frame::paint(frame, detail_area, detail);
    }
}

fn item_line(
    item: &views::Item,
    width: usize,
    theme: &Theme,
    selected: bool,
    focused: bool,
) -> Line<'static> {
    if item.kind == ItemKind::Heading {
        let mut left = vec![Span::raw("  ".to_owned())];
        left.extend(item.left.iter().cloned());
        return text::row(width, left, Vec::new());
    }
    let marker = if selected && focused {
        theme.markers.selection
    } else if selected {
        theme.markers.activity
    } else {
        " "
    };
    let style: Option<Style> = (selected && focused).then(|| theme.selected());
    let restyle = |spans: &Vec<Span<'static>>| -> Vec<Span<'static>> {
        spans
            .iter()
            .map(|span| match style {
                Some(selected) => Span::styled(span.content.clone(), selected),
                None => span.clone(),
            })
            .collect()
    };
    let mut left = vec![Span::styled(
        format!("{marker} "),
        style.unwrap_or_else(|| theme.accent()),
    )];
    left.extend(restyle(&item.left));
    text::row(width, left, restyle(&item.right))
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
    let visible = matches.len().min(6);
    let height = visible as u16 + 2;
    if composer.y <= main.y + height || main.width < 30 {
        return false;
    }
    let width = main.width.min(64);
    let area = Rect {
        x: main.x + 1,
        y: composer.y.saturating_sub(height + 1),
        width,
        height,
    };
    let selected = app.completion.min(matches.len() - 1);
    let first = selected.saturating_sub(visible - 1);
    let body = matches
        .iter()
        .enumerate()
        .skip(first)
        .take(visible)
        .map(|(index, command)| {
            let chosen = index == selected;
            let style = if chosen {
                theme.selected()
            } else {
                theme.body()
            };
            text::row(
                width.saturating_sub(2) as usize,
                vec![Span::styled(
                    format!(
                        "{} {}",
                        if chosen { theme.markers.selection } else { " " },
                        command.name
                    ),
                    style,
                )],
                vec![Span::styled(
                    text::truncate(command.summary, width as usize / 2),
                    if chosen { style } else { theme.faint() },
                )],
            )
        })
        .collect::<Vec<_>>();
    frame.render_widget(ratatui::widgets::Clear, area);
    let block = ratatui::widgets::Block::default()
        .borders(ratatui::widgets::Borders::ALL)
        .border_style(theme.rule())
        .style(theme.surface())
        .title_top(Line::from(Span::styled(
            format!(" {} commands ", matches.len()),
            theme.muted(),
        )));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame::paint(frame, inner, body);
    false
}

/// Draw whichever floating surface is open, and return where its cursor belongs.
fn overlay(frame: &mut Frame, area: Rect, app: &App) -> Option<(u16, u16)> {
    let theme = &app.theme;
    let overlay = app.overlay.as_ref()?;
    match overlay {
        Overlay::Themes { selected, .. } => {
            let width = 58u16.min(area.width.saturating_sub(4));
            let inner = width.saturating_sub(2) as usize;
            let mut body = Vec::new();
            for (index, palette) in theme::THEMES.iter().enumerate() {
                let chosen = index == *selected;
                let style = if chosen {
                    theme.selected()
                } else {
                    theme.body()
                };
                body.push(text::row(
                    inner,
                    vec![
                        Span::styled(
                            format!(
                                "{} {}",
                                if chosen { theme.markers.selection } else { " " },
                                palette.name
                            ),
                            style,
                        ),
                        Span::styled(format!("  {}", palette.kind.label()), theme.muted()),
                    ],
                    swatches(palette),
                ));
                if chosen {
                    for piece in text::wrap(palette.summary, inner.saturating_sub(4)) {
                        body.push(Line::from(vec![
                            Span::raw("    ".to_owned()),
                            Span::styled(piece, theme.faint()),
                        ]));
                    }
                }
            }
            body.push(Line::default());
            body.push(Line::from(Span::styled(
                "The whole interface previews as you move.".to_owned(),
                theme.faint(),
            )));
            frame::render_modal(
                frame,
                area,
                &ModalSpec {
                    title: "Colour theme".into(),
                    badge: "saved on Enter".into(),
                    role: ModalRole::Choice,
                    width,
                    body,
                    footer: vec![("Up/Down", "preview"), ("Enter", "keep"), ("Esc", "cancel")],
                    scroll: 0,
                },
                theme,
            );
            None
        }
        Overlay::Palette { field, selected } => {
            let width = 72u16.min(area.width.saturating_sub(4));
            let inner = width.saturating_sub(2) as usize;
            let matches = crate::commands::search(&field.value);
            let mut body = vec![
                Line::from(vec![
                    Span::styled(format!("{} ", theme.markers.prompt), theme.accent_bold()),
                    Span::styled(field.value.clone(), theme.text()),
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
            let visible = 10usize;
            let first = selected.saturating_sub(visible.saturating_sub(1));
            for (index, command) in matches.iter().enumerate().skip(first).take(visible) {
                let chosen = index == selected;
                let style = if chosen {
                    theme.selected()
                } else {
                    theme.body()
                };
                body.push(text::row(
                    inner,
                    vec![Span::styled(
                        format!(
                            "{} {}",
                            if chosen { theme.markers.selection } else { " " },
                            command.name
                        ),
                        style,
                    )],
                    vec![Span::styled(
                        text::truncate(command.summary, inner / 2),
                        if chosen { style } else { theme.faint() },
                    )],
                ));
            }
            let rect = frame::render_modal(
                frame,
                area,
                &ModalSpec {
                    title: "Commands".into(),
                    badge: format!("{} of {}", matches.len(), crate::commands::COMMANDS.len()),
                    role: ModalRole::Choice,
                    width,
                    body,
                    footer: vec![("Enter", "run"), ("Up/Down", "select"), ("Esc", "close")],
                    scroll: 0,
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
                &ModalSpec {
                    title: title.clone(),
                    badge: "read only".into(),
                    role: ModalRole::Reference,
                    width: frame::inspect_width(area.width),
                    body: body.clone(),
                    footer: vec![("Up/Down", "scroll"), ("Esc", "close")],
                    scroll: *scroll,
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
            let inner = width.saturating_sub(2) as usize;
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
                },
                theme,
            );
            Some((
                rect.x + 2 + cursor_col as u16,
                rect.y + (cursor_row - first_row) as u16,
            ))
        }
        Overlay::Confirm { question, .. } => {
            let width = 68u16.min(area.width.saturating_sub(4));
            let body = text::wrap(question, width.saturating_sub(2) as usize)
                .into_iter()
                .map(|piece| Line::from(Span::styled(piece, theme.body())))
                .collect();
            frame::render_modal(
                frame,
                area,
                &ModalSpec {
                    title: "Confirm".into(),
                    badge: "cannot be undone".into(),
                    role: ModalRole::Choice,
                    width,
                    body,
                    footer: vec![("y", "yes"), ("n", "no"), ("Esc", "cancel")],
                    scroll: 0,
                },
                theme,
            );
            None
        }
    }
}

fn swatches(palette: &Theme) -> Vec<Span<'static>> {
    [
        palette.accent,
        palette.good,
        palette.warn,
        palette.bad,
        palette.info,
        palette.surface,
    ]
    .into_iter()
    .map(|color| Span::styled("  ".to_owned(), Style::default().bg(color)))
    .collect()
}
