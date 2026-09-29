//! Screens: the conversation workspace with its sidebar and composer, the
//! inspection pages, the description overlay and the command palette. Every
//! state is written in words; color only repeats what the words say.
pub mod conversation;
pub mod pages;

use crate::app::{App, COMMANDS, Focus, Host, PAGES, Tone};
use conversation::{RowKind, compose, printable, profile, wrap};
use pages::{UNKNOWN, cell};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, TableState},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};
use ymp_runtime::live_session::Pulse;

/// The sidebar stays beside the conversation from this width on.
pub const WIDE: u16 = 100;
const SIDEBAR: u16 = 38;

fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}
fn accent() -> Style {
    Style::default().fg(Color::Cyan)
}
fn strong() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}
fn tone(tone: Tone) -> Style {
    match tone {
        Tone::User => Style::default().fg(Color::Cyan),
        Tone::Agent => Style::default().fg(Color::Blue),
        Tone::Runtime => Style::default().fg(Color::Magenta),
        Tone::Good => Style::default().fg(Color::Green),
        Tone::Bad => Style::default().fg(Color::Red),
        Tone::Report => Style::default().fg(Color::Yellow),
        Tone::Note => dim(),
    }
}
/// Cut to `width` columns; a cut is marked so that clipping is visible.
pub fn clip(text: &str, width: usize) -> String {
    // One line of data: what would move the cursor or reorder text is replaced.
    let text = printable(&text.replace(['\n', '\t'], " "));
    if text.width() <= width {
        return text;
    }
    if width == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = c.width().unwrap_or(0);
        if used + w + 1 > width {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('…');
    out
}
/// What the interface is doing about the session, in words.
pub fn activity<H: Host>(app: &App<H>) -> &'static str {
    match (&app.session, app.driving(), &app.pulse) {
        (None, ..) => "no session",
        (Some(_), false, _) => "read-only",
        (_, _, Some(Pulse::Opening)) => "opening",
        (_, _, Some(Pulse::Working)) => "working",
        (_, _, Some(Pulse::NeedsUser)) => "waiting for your answer",
        (_, _, Some(Pulse::Observed)) => "observing",
        (_, _, Some(Pulse::Delivered)) => "report delivered",
        (_, _, Some(Pulse::Failed(_))) => "refused; see the denial",
        (_, _, Some(Pulse::Closed) | None) => "closed",
    }
}
pub fn draw<H: Host>(frame: &mut Frame, app: &mut App<H>) {
    let area = frame.area();
    app.wide = area.width >= WIDE;
    app.follow_page();
    let composer_lines = compose(
        &app.input,
        app.cursor,
        area.width.saturating_sub(3) as usize,
    )
    .0
    .len() as u16;
    let composer_height = (composer_lines.clamp(1, 5) + 2).min(area.height.saturating_sub(3));
    let [header, body, composer, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(composer_height.max(1)),
        Constraint::Length(1),
    ])
    .areas(area);
    draw_header(frame, app, header);
    let shown = app.sidebar_shown(app.wide);
    let beside = shown && app.wide && body.width > SIDEBAR + 30;
    let (main, side) = if beside {
        let [main, side] =
            Layout::horizontal([Constraint::Min(30), Constraint::Length(SIDEBAR)]).areas(body);
        (main, Some(side))
    } else {
        (body, None)
    };
    if app.page.is_some() {
        draw_page(frame, app, main);
    } else {
        draw_conversation(frame, app, main);
    }
    if let Some(side) = side {
        draw_sidebar(frame, app, side, false);
    } else if shown {
        // A narrow terminal shows the sidebar over the conversation on request.
        let width = SIDEBAR.min(body.width);
        let over = Rect::new(body.x + body.width - width, body.y, width, body.height);
        frame.render_widget(Clear, over);
        draw_sidebar(frame, app, over, true);
    }
    draw_composer(frame, app, composer);
    draw_status(frame, app, status);
    if app.detail.is_some() {
        draw_detail(frame, app, body);
    }
    if app.palette.is_some() {
        draw_palette(frame, app, area);
    }
}
fn draw_header<H: Host>(frame: &mut Frame, app: &App<H>, area: Rect) {
    let mut spans = vec![Span::styled(" ymp ", strong().fg(Color::Cyan))];
    match (&app.session, &app.projection) {
        (Some(session), Some(projection)) => {
            spans.push(Span::raw(format!("session {} ", session.as_str())));
            spans.push(Span::styled(
                format!("· recorded status {} ", projection.status()),
                strong(),
            ));
            spans.push(Span::raw(format!(
                "· revision {} · {} ",
                projection.revision(),
                activity(app)
            )));
        }
        (Some(session), None) => {
            spans.push(Span::raw(format!(
                "session {} · nothing recorded yet · {} ",
                session.as_str(),
                activity(app)
            )));
        }
        _ => spans.push(Span::raw("no session ")),
    }
    spans.push(Span::styled(
        format!("· {}", printable(&app.host.summary())),
        dim(),
    ));
    let line = clip(
        &spans.iter().map(|s| s.content.as_ref()).collect::<String>(),
        area.width as usize,
    );
    // Clipping works on the whole line; styles are kept only when all of it fits.
    if line.ends_with('…') {
        frame.render_widget(Paragraph::new(line), area);
    } else {
        frame.render_widget(Paragraph::new(Line::from(spans)), area);
    }
}
fn draw_conversation<H: Host>(frame: &mut Frame, app: &mut App<H>, area: Rect) {
    let width = area.width.saturating_sub(2) as usize;
    let height = area.height as usize;
    let entries = conversation::entries(app.projection.as_ref(), &app.draft, &app.notices);
    let rows = conversation::rows(&entries, width);
    let top = conversation::position(&entries, &rows, height, &mut app.reading, &mut app.motions);
    let lines: Vec<Line> = rows
        .iter()
        .skip(top)
        .take(height)
        .map(|row| {
            let entry = &entries[row.entry];
            let bar = if entry.major { "▌ " } else { "  " };
            let style = match row.kind {
                RowKind::Heading if entry.major => tone(entry.tone).add_modifier(Modifier::BOLD),
                RowKind::Heading => tone(entry.tone),
                RowKind::Body if entry.major => Style::default(),
                RowKind::Body => dim(),
                RowKind::Gap => Style::default(),
            };
            Line::from(vec![
                Span::styled(bar, tone(entry.tone)),
                Span::styled(row.text.clone(), style),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
    if app.reading.is_some() && area.height > 0 {
        let below = rows.len().saturating_sub(top + height);
        let note = format!(" reading older activity · {below} lines below · End follows ");
        let note = clip(&note, area.width as usize);
        let width = (note.width() as u16).min(area.width);
        let at = Rect::new(
            area.x + area.width.saturating_sub(width),
            area.y + area.height - 1,
            width,
            1,
        );
        frame.render_widget(Clear, at);
        frame.render_widget(
            Paragraph::new(note).style(Style::default().add_modifier(Modifier::REVERSED)),
            at,
        );
    }
}
fn section(lines: &mut Vec<Line<'static>>, title: &str) {
    if !lines.is_empty() {
        lines.push(Line::raw(""));
    }
    lines.push(Line::styled(
        title.to_uppercase(),
        accent().add_modifier(Modifier::BOLD),
    ));
}
fn pair(lines: &mut Vec<Line<'static>>, name: &str, value: String, width: usize) {
    let room = width.saturating_sub(name.width() + 1);
    lines.push(Line::from(vec![
        Span::styled(format!("{name} "), dim()),
        Span::raw(clip(&value, room)),
    ]));
}
fn draw_sidebar<H: Host>(frame: &mut Frame, app: &App<H>, area: Rect, over: bool) {
    let block = Block::default()
        .borders(if over { Borders::ALL } else { Borders::LEFT })
        .border_style(if app.sidebar_focus { accent() } else { dim() });
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let width = inner.width.saturating_sub(1) as usize;
    let null = serde_json::Value::Null;
    let json = app.projection.as_ref().map_or(&null, |p| &p.json);
    let mut lines: Vec<Line> = vec![];
    section(&mut lines, "Session");
    match &app.projection {
        Some(projection) => {
            pair(&mut lines, "status", projection.status(), width);
            pair(
                &mut lines,
                "revision",
                projection.revision().to_string(),
                width,
            );
            pair(&mut lines, "interface", activity(app).into(), width);
        }
        None => pair(&mut lines, "status", "not started".into(), width),
    }
    section(&mut lines, "Team");
    let agents = json["registry"]["input"]["facts"]["agents"].as_array();
    match agents {
        Some(agents) if !agents.is_empty() => {
            for agent in agents {
                let id = cell(&agent["id"]);
                let recorded = &json["progress"]["profiles"][&id];
                let settings = if recorded.is_null() {
                    &agent["defaults"]
                } else {
                    recorded
                };
                pair(&mut lines, &id, profile(settings), width);
            }
        }
        _ => {
            for member in &app.members {
                let model = member.model.clone().unwrap_or_else(|| UNKNOWN.into());
                pair(&mut lines, &member.agent, model, width);
            }
        }
    }
    section(&mut lines, "Criteria");
    let criteria = json["criteria"].as_array().cloned().unwrap_or_default();
    if criteria.is_empty() {
        for expectation in &app.draft.expectations {
            pair(&mut lines, "draft", expectation.path.clone(), width);
        }
        if app.draft.expectations.is_empty() {
            lines.push(Line::styled("none stated", dim()));
        }
    }
    for criterion in &criteria {
        let status = &json["ledger"]["entries"][cell(&criterion["id"])]["status"];
        let (words, style) = match status.as_str() {
            None => ("not assessed".to_string(), dim()),
            Some("Satisfied") => ("Satisfied".to_string(), tone(Tone::Good)),
            Some(other) => (other.to_string(), tone(Tone::Bad)),
        };
        let room = width.saturating_sub(words.width() + 1);
        lines.push(Line::from(vec![
            Span::styled(format!("{words} "), style),
            Span::raw(clip(&cell(&criterion["id"]), room)),
        ]));
    }
    section(&mut lines, "Work");
    let invocations = json["execution"]["invocations"].as_object();
    let open: Vec<_> = invocations
        .into_iter()
        .flatten()
        .filter(|(_, record)| record["terminal"].is_null())
        .collect();
    let runs = |record: &serde_json::Value| app.projection.as_ref().is_some_and(|p| p.runs(record));
    let working = open.iter().filter(|(_, record)| runs(record)).count();
    pair(
        &mut lines,
        "calls",
        format!(
            "{} recorded · {} working · {} without end",
            invocations.map_or(0, |all| all.len()),
            working,
            open.len() - working
        ),
        width,
    );
    for (_, record) in open {
        let assignment = &record["dispatch"]["assignment"];
        pair(
            &mut lines,
            match (record["backend_terminal"].is_null(), runs(record)) {
                (false, _) => "unsettled",
                (true, true) => "now",
                (true, false) => "no end",
            },
            format!(
                "{} · {}",
                cell(&assignment["agent"]),
                cell(&assignment["role"])
            ),
            width,
        );
    }
    for (name, item) in json["results"]["items"].as_object().into_iter().flatten() {
        pair(&mut lines, name, cell(&item["state"]), width);
    }
    section(&mut lines, "Resources");
    let budget = &json["treasury"]["budget"];
    if budget.is_null() {
        pair(
            &mut lines,
            "budget",
            format!("{} (draft)", app.draft.budget),
            width,
        );
    } else {
        pair(&mut lines, "limit", cell(&budget["limit"]), width);
        pair(&mut lines, "spent", cell(&budget["spent"]), width);
        pair(&mut lines, "held", cell(&budget["held"]), width);
        pair(
            &mut lines,
            "reserves",
            format!(
                "verification {} · reporting {}",
                cell(&budget["verification_reserve"]),
                cell(&budget["reporting_reserve"])
            ),
            width,
        );
        let unknown = pages::unknown_usage(json);
        let room = width.saturating_sub(14);
        lines.push(Line::from(vec![
            Span::styled("unknown usage ", dim()),
            Span::styled(
                clip(unknown, room),
                if unknown != "none recorded" {
                    tone(Tone::Bad).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            ),
        ]));
    }
    section(&mut lines, "Pending");
    let pending = app
        .projection
        .as_ref()
        .map_or(0, |p| p.pending_questions().len());
    if pending > 0 {
        lines.push(Line::styled(
            clip(&format!("{pending} question(s) for you"), width),
            tone(Tone::Report).add_modifier(Modifier::BOLD),
        ));
    } else {
        lines.push(Line::styled("no decision is waiting", dim()));
    }
    section(&mut lines, "Pages");
    for (index, (name, _)) in PAGES.iter().enumerate() {
        let chosen = app.sidebar_focus && index == app.navigation;
        lines.push(Line::styled(
            format!("{} {name}", if chosen { ">" } else { " " }),
            if chosen {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            },
        ));
    }
    // A short terminal keeps the focused navigation in sight.
    let height = inner.height as usize;
    let top = if app.sidebar_focus {
        lines.len().saturating_sub(height)
    } else {
        0
    };
    let lines: Vec<Line> = lines.into_iter().skip(top).take(height).collect();
    frame.render_widget(
        Paragraph::new(lines),
        Rect::new(
            inner.x + 1,
            inner.y,
            inner.width.saturating_sub(1),
            inner.height,
        ),
    );
}
fn draw_composer<H: Host>(frame: &mut Frame, app: &App<H>, area: Rect) {
    let focused = app.focus() == Focus::Composer;
    let title = match (&app.session, &app.pulse, app.driving()) {
        (None, ..) => " goal or /command ",
        (_, Some(Pulse::NeedsUser), true) => " your answer, or /command ",
        _ => " /command ",
    };
    let block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(if focused { accent() } else { dim() })
        .title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 || inner.width < 3 {
        return;
    }
    // One column stays free so that the cursor after a full line is visible.
    let width = inner.width.saturating_sub(3) as usize;
    let (lines, (cursor_line, cursor_column)) = compose(&app.input, app.cursor, width);
    let top = (cursor_line + 1).saturating_sub(inner.height as usize);
    let shown: Vec<Line> = lines
        .iter()
        .enumerate()
        .skip(top)
        .take(inner.height as usize)
        .map(|(index, line)| {
            Line::from(vec![
                Span::styled(if index == 0 { "> " } else { "  " }, accent()),
                Span::raw(line.clone()),
            ])
        })
        .collect();
    frame.render_widget(Paragraph::new(shown), inner);
    if focused {
        frame.set_cursor_position(Position::new(
            inner.x + 2 + (cursor_column as u16).min(inner.width.saturating_sub(3)),
            inner.y + cursor_line.saturating_sub(top) as u16,
        ));
    }
}
fn draw_status<H: Host>(frame: &mut Frame, app: &App<H>, area: Rect) {
    let completions = app.completions();
    let text = if !completions.is_empty() && app.focus() == Focus::Composer {
        format!(
            " Tab completes: {}",
            completions
                .iter()
                .map(|c| format!("/{}", c.name))
                .collect::<Vec<_>>()
                .join(" ")
        )
    } else {
        match app.focus() {
            Focus::Composer => format!(
                " composer · {} · Enter send · Ctrl+J newline · Ctrl+P commands · Ctrl+B sidebar · PgUp/PgDn read · Ctrl+C interrupt",
                activity(app)
            ),
            Focus::Sidebar => " sidebar · ↑↓ choose page · Enter open · Esc composer".into(),
            Focus::Page => match &app.page {
                Some(state) if state.filtering => {
                    " page filter · type to filter · Enter keep · Esc clear".into()
                }
                _ => " page · ↑↓ select · Enter describe · / filter · s sort · S reverse · Tab next page · Esc close".into(),
            },
            Focus::Detail => " description · ↑↓ PgUp PgDn scroll · Esc close".into(),
            Focus::Palette => " commands · type to filter · ↑↓ choose · Enter run · Esc close".into(),
        }
    };
    frame.render_widget(
        Paragraph::new(clip(&text, area.width as usize))
            .style(Style::default().add_modifier(Modifier::REVERSED)),
        area,
    );
}
fn draw_page<H: Host>(frame: &mut Frame, app: &mut App<H>, area: Rect) {
    let Some(state) = &app.page else { return };
    let page = &state.page;
    let order = match state.sort {
        Some((column, descending)) => format!(
            " · sorted by {} {}",
            page.columns[column].title,
            if descending {
                "descending"
            } else {
                "ascending"
            }
        ),
        None => String::new(),
    };
    let filter = if state.filtering || !state.filter.is_empty() {
        format!(" · filter /{}", state.filter)
    } else {
        String::new()
    };
    let title = format!(
        " {} · {} of {}{filter}{order} ",
        page.kind.title(),
        state.visible.len(),
        page.records.len()
    );
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(accent())
        .title(clip(&title, area.width.saturating_sub(2) as usize));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let (note, table) = if page.note.is_empty() || inner.height < 3 {
        (None, inner)
    } else {
        let lines = wrap(&page.note, inner.width.saturating_sub(1) as usize);
        let height = (lines.len() as u16).min(inner.height / 3).max(1);
        let [note, table] =
            Layout::vertical([Constraint::Length(height), Constraint::Min(1)]).areas(inner);
        (Some((note, lines)), table)
    };
    if let Some((area, lines)) = note {
        frame.render_widget(
            Paragraph::new(
                lines
                    .into_iter()
                    .map(|line| Line::styled(format!(" {line}"), dim()))
                    .collect::<Vec<_>>(),
            ),
            area,
        );
    }
    // Columns take what their content needs; the widest give way first.
    let mut widths: Vec<usize> = page
        .columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            state
                .visible
                .iter()
                .map(|row| page.records[*row].cells[index].width())
                .max()
                .unwrap_or(0)
                .max(column.title.width())
                .min(48)
        })
        .collect();
    let room = (table.width as usize).saturating_sub(widths.len() + 2);
    while widths.iter().sum::<usize>() > room {
        let Some(widest) = widths.iter_mut().max() else {
            break;
        };
        if *widest <= 6 {
            break;
        }
        *widest -= 1;
    }
    let fit = |text: &str, index: usize| {
        let text = clip(text, widths[index]);
        if page.columns[index].numeric {
            format!("{text:>width$}", width = widths[index])
        } else {
            text
        }
    };
    let header = Row::new(
        page.columns
            .iter()
            .enumerate()
            .map(|(index, column)| Cell::from(fit(&column.title.to_uppercase(), index))),
    )
    .style(strong());
    let rows = state.visible.iter().map(|row| {
        Row::new(
            page.records[*row]
                .cells
                .iter()
                .enumerate()
                .map(|(index, text)| Cell::from(fit(text, index))),
        )
    });
    let widget = Table::new(
        rows,
        widths.iter().map(|width| Constraint::Length(*width as u16)),
    )
    .header(header)
    .column_spacing(1)
    .highlight_symbol("> ")
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    let mut selection = TableState::default();
    if !state.visible.is_empty() {
        selection.select(Some(state.selected));
    }
    frame.render_stateful_widget(widget, table, &mut selection);
}
fn draw_detail<H: Host>(frame: &mut Frame, app: &mut App<H>, area: Rect) {
    let Some(detail) = &mut app.detail else {
        return;
    };
    let margin = if area.width > 60 { 3 } else { 0 };
    let over = Rect::new(
        area.x + margin,
        area.y,
        area.width.saturating_sub(2 * margin),
        area.height,
    );
    frame.render_widget(Clear, over);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(accent())
        .title(clip(
            &format!(" {} ", detail.title),
            over.width.saturating_sub(2) as usize,
        ));
    let inner = block.inner(over);
    frame.render_widget(block, over);
    let lines = wrap(&detail.text, inner.width.saturating_sub(2) as usize);
    detail.top = detail
        .top
        .min(lines.len().saturating_sub(inner.height as usize));
    let shown: Vec<Line> = lines
        .into_iter()
        .skip(detail.top)
        .take(inner.height as usize)
        .map(|line| Line::raw(format!(" {line}")))
        .collect();
    frame.render_widget(Paragraph::new(shown), inner);
}
fn draw_palette<H: Host>(frame: &mut Frame, app: &mut App<H>, area: Rect) {
    let Some(palette) = &mut app.palette else {
        return;
    };
    let matches = palette.matches();
    palette.selected = palette.selected.min(matches.len().saturating_sub(1));
    let width = area.width.saturating_sub(4).min(86);
    let height = (COMMANDS.len() as u16 + 3).min(area.height.saturating_sub(2));
    if width < 10 || height < 3 {
        return;
    }
    let over = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + 1,
        width,
        height,
    );
    frame.render_widget(Clear, over);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(accent())
        .title(" commands ");
    let inner = block.inner(over);
    frame.render_widget(block, over);
    let room = inner.width as usize;
    let mut lines = vec![Line::from(vec![
        Span::styled(" > ", accent()),
        Span::raw(clip(&palette.query, room.saturating_sub(4))),
    ])];
    let visible = (inner.height as usize).saturating_sub(1);
    let top = (palette.selected + 1).saturating_sub(visible);
    for (index, command) in matches.iter().enumerate().skip(top).take(visible) {
        let name = format!("/{} {}", command.name, command.argument);
        let text = clip(
            &format!(" {name:<26} {}", command.help),
            room.saturating_sub(1),
        );
        lines.push(Line::styled(
            text,
            if index == palette.selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            },
        ));
    }
    if matches.is_empty() {
        lines.push(Line::styled(" no command matches", dim()));
    }
    frame.render_widget(Paragraph::new(lines), inner);
    frame.set_cursor_position(Position::new(
        inner.x + 3 + (palette.query.width() as u16).min(inner.width.saturating_sub(4)),
        inner.y,
    ));
}
