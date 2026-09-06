use crate::forms::{Action, Value};
use crate::{
    app::{App, COMMANDS, Layer, Speaker},
    resources::{Kind, ResourceView},
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Paragraph, Row, Table, TableState, Wrap},
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub const BASE: Color = Color::Rgb(12, 18, 27);
const PANEL: Color = Color::Rgb(17, 26, 38);
const CARD: Color = Color::Rgb(23, 35, 49);
const SELECTED: Color = Color::Rgb(27, 49, 68);
const LINE: Color = Color::Rgb(45, 62, 80);
const TEXT: Color = Color::Rgb(224, 233, 241);
const MUTED: Color = Color::Rgb(154, 173, 189);
const DIM: Color = Color::Rgb(124, 145, 164);
const BLUE: Color = Color::Rgb(115, 201, 255);
const MINT: Color = Color::Rgb(132, 218, 180);
const VIOLET: Color = Color::Rgb(189, 169, 255);
const PEACH: Color = Color::Rgb(255, 182, 151);
fn style(color: Color) -> Style {
    Style::default().fg(color)
}
fn bold(color: Color) -> Style {
    style(color).add_modifier(Modifier::BOLD)
}
fn area(x: u16, y: u16, width: u16, height: u16) -> Rect {
    Rect::new(x, y, width, height)
}
fn inset(r: Rect, x: u16, y: u16) -> Rect {
    area(
        r.x + x.min(r.width),
        r.y + y.min(r.height),
        r.width.saturating_sub(x * 2),
        r.height.saturating_sub(y * 2),
    )
}
fn row(r: Rect, y: u16, height: u16) -> Rect {
    area(
        r.x,
        r.y + y.min(r.height),
        r.width,
        height.min(r.height.saturating_sub(y)),
    )
}
fn fill(f: &mut Frame, r: Rect, color: Color) {
    f.render_widget(Block::default().style(Style::default().bg(color)), r);
}
fn label(f: &mut Frame, r: Rect, text: impl Into<String>, color: Color) {
    f.render_widget(Paragraph::new(text.into()).style(style(color)), r);
}
fn right(f: &mut Frame, r: Rect, text: impl Into<String>, color: Color) {
    f.render_widget(
        Paragraph::new(text.into())
            .style(style(color))
            .alignment(Alignment::Right),
        r,
    );
}
fn field(f: &mut Frame, r: Rect, name: &str, value: impl ToString, color: Color) {
    let value = value.to_string();
    let width = (UnicodeWidthStr::width(value.as_str()) as u16).min(r.width);
    label(
        f,
        area(r.x, r.y, r.width.saturating_sub(width + 1), r.height),
        name,
        MUTED,
    );
    right(
        f,
        area(r.right() - width, r.y, width, r.height),
        value,
        color,
    );
}
fn state_color(state: &str) -> Color {
    match state {
        "Running" | "Active" | "Starting" | "Developing" | "Verifying" => BLUE,
        "Failed" | "Attention" | "Interrupted" | "Stopped" => PEACH,
        "Completed" | "Returned" | "Passed" | "Demo ready" | "Delivered" => MINT,
        "Candidate" | "Knowledge" => VIOLET,
        _ => MUTED,
    }
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    let screen = frame.area();
    app.width = screen.width;
    app.action_hits.clear();
    app.buttons.clear();
    app.editor_hits.clear();
    app.menu_hits.clear();
    fill(frame, screen, BASE);
    if screen.width < 80 || screen.height < 24 {
        label(
            frame,
            inset(screen, 2, 2),
            "Please enlarge the terminal to at least 80 × 24.\nCtrl Q to quit.",
            TEXT,
        );
        return;
    }
    let canvas = inset(screen, 2, 0);
    header(frame, row(canvas, 0, 3), app);
    let body = area(canvas.x, canvas.y + 4, canvas.width, canvas.height - 6);
    let active = app.layers.is_empty();
    if let Some(table) = &mut app.table {
        resource_table(frame, body, table, active);
        let has_query = !table.query.is_empty();
        if has_query {
            button(
                frame,
                area(body.right() - 34, body.y, 12, 1),
                "Clear filter",
                Action::ClearFilter,
                app,
            );
        }
    } else {
        let side = if app.sidebar && screen.width >= 110 {
            if screen.width >= 140 { 34 } else { 30 }
        } else {
            0
        };
        let gap = if side > 0 { 2 } else { 0 };
        conversation(
            frame,
            area(body.x, body.y, body.width - side - gap, body.height),
            app,
        );
        if side > 0 {
            services(
                frame,
                area(body.right() - side, body.y, side, body.height),
                app,
            );
        }
    }
    footer(
        frame,
        area(canvas.x, screen.bottom() - 1, canvas.width, 1),
        app,
    );
    if let Some(layer) = app.layers.last().cloned() {
        match layer {
            Layer::Commands => palette(frame, screen, app),
            Layer::Detail(id) => {
                app.buttons.clear();
                detail(frame, screen, app, &id);
            }
            Layer::Actions => action_menu(frame, screen, app),
            Layer::Editor => editor(frame, screen, app),
            Layer::Confirm => confirmation(frame, screen, app),
            Layer::Help => help(frame, screen),
            Layer::Services => {
                let r = popup(frame, screen, 36, screen.height - 4, " Workspace ");
                app.action_hits.clear();
                services(frame, inset(r, 1, 1), app);
            }
        }
    }
}

fn header(f: &mut Frame, r: Rect, app: &App) {
    let session = app.title();
    let mut spans = vec![
        Span::styled(
            " ymp ",
            Style::default()
                .fg(BASE)
                .bg(BLUE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("   /   {session}"), style(TEXT)),
    ];
    if let Some(table) = &app.table {
        spans.push(Span::styled(
            format!("   /   {}", table.data.kind.name()),
            style(BLUE),
        ));
    }
    if app.table.is_none() && !app.chat_filter.is_empty() && !app.filtering_chat {
        let used: usize = spans.iter().map(Span::width).sum();
        let width = (r.width as usize).saturating_sub(used + 7);
        spans.push(Span::styled(
            clip(&format!("  /{}", app.chat_filter), width),
            style(BLUE),
        ));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)),
        area(r.x, r.y + 1, r.width - 14, 1),
    );
    f.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            " SIMULATION ",
            Style::default().fg(VIOLET).bg(CARD),
        )]))
        .alignment(Alignment::Right),
        area(r.right() - 12, r.y + 1, 12, 1),
    );
    label(f, row(r, 2, 1), "─".repeat(r.width as usize), LINE);
}

fn conversation(f: &mut Frame, r: Rect, app: &mut App) {
    let composer = area(r.x, r.bottom() - 2, r.width, 2);
    let filter_visible = app.filtering_chat;
    let top = r.y + u16::from(filter_visible);
    let question = app
        .simulation
        .as_ref()
        .and_then(|s| s.task())
        .is_some_and(|t| t.stage == crate::entities::Stage::Clarifying && !t.archived);
    let thread = area(
        r.x,
        top,
        r.width,
        composer
            .y
            .saturating_sub(top + if question { 3 } else { 0 }),
    );
    if app.messages.is_empty() {
        label(f, row(thread, 1, 1), "What are we building?", TEXT);
        label(f, row(thread, 3, 2), "Battleship scenario", MUTED);
    } else {
        messages(f, thread, app);
    }
    if app.paused {
        label(
            f,
            area(r.x + 1, composer.y - 1, r.width - 1, 1),
            "Paused",
            DIM,
        );
    }
    if question {
        let q = area(r.x, composer.y.saturating_sub(3), r.width, 2);
        fill(f, q, PANEL);
        button(
            f,
            area(q.x + 1, q.y + 1, 22, 1),
            "Use proposed rules",
            Action::Answer,
            app,
        );
        button(
            f,
            area(q.x + 26, q.y + 1, 22, 1),
            "Edit requirements",
            Action::EditAnswer,
            app,
        );
    }
    composer_view(f, composer, app);
    if filter_visible {
        filter_line(
            f,
            row(r, 0, 1),
            &app.chat_filter,
            app.filtering_chat,
            app.layers.is_empty(),
        );
    }
}

fn messages(f: &mut Frame, r: Rect, app: &mut App) {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut tools: Vec<(usize, String)> = Vec::new();
    let query = app.chat_filter.to_lowercase();
    for message in &app.messages {
        if !message.text.to_lowercase().contains(&query) {
            continue;
        }
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        if let Some(id) = &message.tool {
            if let Some(tool) = app
                .simulation
                .as_ref()
                .and_then(|s| s.tools.iter().find(|t| &t.id == id))
            {
                let expanded = app.expanded_tools.contains(id);
                let focused = app.tool_focus.as_ref() == Some(id);
                let caption = format!(
                    "{} {}  {} · {}",
                    if expanded { "▾" } else { "▸" },
                    tool.command,
                    tool.status.label(),
                    crate::simulation::age(
                        tool.ended.unwrap_or(app.simulation.as_ref().unwrap().now),
                        tool.started
                    )
                );
                tools.push((lines.len(), id.clone()));
                lines.push(Line::from(Span::styled(
                    clip(&caption, r.width.saturating_sub(2) as usize),
                    if focused {
                        bold(BLUE).bg(SELECTED)
                    } else {
                        style(MUTED)
                    },
                )));
                if expanded {
                    for original in tool.output.lines() {
                        for text in wrap(original, r.width.saturating_sub(4) as usize) {
                            lines.push(Line::from(Span::styled(format!("  {text}"), style(DIM))));
                        }
                    }
                }
            }
            continue;
        }
        let (name, role, color) = if matches!(message.speaker, Speaker::You) {
            ("YOU", "", TEXT)
        } else {
            (
                message.author.as_deref().unwrap_or("YMP"),
                if message.author.is_some() {
                    "agent"
                } else {
                    "system"
                },
                BLUE,
            )
        };
        lines.push(Line::from(vec![
            Span::styled(format!("  {name}"), bold(color)),
            Span::styled(
                if role.is_empty() {
                    String::new()
                } else {
                    format!(" · {role}")
                },
                style(DIM),
            ),
        ]));
        for text in wrap(&message.text, r.width.saturating_sub(4) as usize) {
            lines.push(Line::from(Span::styled(
                format!("  {text}"),
                style(if matches!(message.speaker, Speaker::You) {
                    TEXT
                } else {
                    MUTED
                }),
            )));
        }
    }
    if lines.is_empty() && !query.is_empty() {
        lines.push(Line::from(Span::styled("No matching messages", style(DIM))));
    }
    let start = lines
        .len()
        .saturating_sub(r.height as usize)
        .saturating_sub(app.scroll as usize);
    f.render_widget(
        Paragraph::new(lines).scroll((start.min(u16::MAX as usize) as u16, 0)),
        r,
    );
    app.visible_tools.clear();
    for (index, id) in tools {
        if index >= start && index < start + r.height as usize {
            let row = area(r.x, r.y + (index - start) as u16, r.width, 1);
            app.buttons.push((row, Action::ToggleTool(id.clone())));
            app.visible_tools.push(id);
        }
    }
}

fn composer_view(f: &mut Frame, r: Rect, app: &App) {
    let active = app.layers.is_empty() && !app.filtering_chat && app.tool_focus.is_none();
    label(
        f,
        row(r, 0, 1),
        "─".repeat(r.width as usize),
        if active { LINE } else { PANEL },
    );
    label(f, area(r.x, r.y + 1, 2, 1), "›", BLUE);
    let input = area(r.x + 2, r.y + 1, r.width - 2, 1);
    let (visible, cursor) = input_window(
        &app.draft,
        app.cursor,
        input.width.saturating_sub(1) as usize,
    );
    label(
        f,
        input,
        if app.draft.is_empty() {
            "Message"
        } else {
            &visible
        },
        if app.draft.is_empty() { DIM } else { TEXT },
    );
    if active {
        f.set_cursor_position((input.x + cursor as u16, input.y));
    }
}

fn services(f: &mut Frame, r: Rect, app: &mut App) {
    fill(f, r, PANEL);
    let inner = inset(r, 1, 1);
    let mut lines: Vec<(String, String, Color, String)> = Vec::new();
    let c = &app.world.counts;
    lines.push(("TASK".into(), String::new(), DIM, "/tasks".into()));
    if let Some(sim) = &app.simulation {
        if let Some(t) = sim.task() {
            lines.push((
                clip(&t.goal, inner.width.saturating_sub(10) as usize),
                t.id.clone(),
                TEXT,
                String::new(),
            ));
            lines.push((
                if t.archived {
                    "Archived"
                } else {
                    t.stage.label()
                }
                .into(),
                crate::simulation::age(sim.now, t.created),
                BLUE,
                String::new(),
            ));
            if let Some(at) = t.run_started {
                lines.push((
                    format!("Run R-{:03}", t.run),
                    crate::simulation::age(t.run_ended.unwrap_or(sim.now), at),
                    MUTED,
                    String::new(),
                ));
            }
        } else {
            lines.push(("No task".into(), String::new(), MUTED, String::new()));
        }
        lines.push((String::new(), String::new(), DIM, String::new()));
        lines.push(("NEXT".into(), String::new(), DIM, String::new()));
        let (label, command) = sim.next_action();
        lines.push((label.into(), String::new(), MINT, command.into()));
        if !command.is_empty() {
            lines.push((command.into(), String::new(), BLUE, command.into()));
        }
        if sim.busy() {
            lines.push((
                "Stop development".into(),
                "/stop".into(),
                PEACH,
                "/stop".into(),
            ));
        } else if sim
            .task()
            .is_some_and(|t| t.stage == crate::simulation::Stage::Review)
        {
            lines.push(("Checks".into(), "/checks".into(), BLUE, "/checks".into()));
            lines.push((
                "Accept result".into(),
                "/accept".into(),
                MINT,
                "/accept".into(),
            ));
        }
    } else {
        lines.push(("Stress fixture".into(), String::new(), MUTED, String::new()));
    }
    lines.push((String::new(), String::new(), DIM, String::new()));
    lines.push((
        "AGENTS".into(),
        app.world
            .agents
            .records
            .iter()
            .filter(|r| !r.archived)
            .count()
            .to_string(),
        DIM,
        "/agents".into(),
    ));
    lines.push((
        format!("{} active · {} waiting", c.running, c.yielded),
        String::new(),
        BLUE,
        String::new(),
    ));
    lines.push((
        format!("{} finished · {} stopped", c.completed, c.failed),
        String::new(),
        MUTED,
        String::new(),
    ));
    lines.push((String::new(), String::new(), DIM, String::new()));
    lines.push((
        "BOARD".into(),
        format!("{} messages", app.world.knowledge.records.len()),
        DIM,
        "/board".into(),
    ));
    if let Some(sim) = &app.simulation {
        let tools: Vec<_> = sim
            .tools
            .iter()
            .filter(|t| {
                sim.task()
                    .is_some_and(|task| t.task == task.id && t.run == task.run)
            })
            .collect();
        lines.push((
            "TOOLS".into(),
            format!(
                "{} active",
                tools.iter().filter(|t| t.ended.is_none()).count()
            ),
            DIM,
            "/tools".into(),
        ));
        if let Some(tool) = tools.last() {
            lines.push((tool.command.clone(), String::new(), MUTED, "/tools".into()));
        }
        lines.push((String::new(), String::new(), DIM, String::new()));
        let checks: Vec<_> = sim
            .checks
            .iter()
            .filter(|c| {
                sim.task()
                    .is_some_and(|t| t.id == c.task && t.run == c.run && t.candidate == c.candidate)
            })
            .collect();
        let status = if checks.is_empty() {
            "Not checked".into()
        } else {
            format!(
                "{} passed · {} failed",
                checks.iter().filter(|c| c.passed).count(),
                checks.iter().filter(|c| !c.passed).count()
            )
        };
        lines.push(("LATEST CHECKS".into(), String::new(), DIM, "/checks".into()));
        lines.push((status, String::new(), MUTED, "/checks".into()));
        lines.push((String::new(), String::new(), DIM, String::new()));
        lines.push((
            "PROVIDERS".into(),
            format!(
                "{} enabled",
                sim.providers.iter().filter(|p| p.enabled).count()
            ),
            DIM,
            "/providers".into(),
        ));
    }
    if let Some(sim) = &app.simulation {
        let background = sim
            .tasks
            .iter()
            .filter(|t| t.stage.busy() && sim.task().is_none_or(|current| current.id != t.id))
            .count();
        if background > 0 {
            lines.push((
                "OTHER TASKS".into(),
                format!("{background} running"),
                BLUE,
                "/tasks".into(),
            ));
        }
    }
    for (model, count) in &app.world.models {
        lines.push((model.clone(), count.to_string(), MUTED, "/agents".into()));
    }
    for (y, (left, value, color, command)) in
        lines.into_iter().enumerate().take(inner.height as usize)
    {
        let r = row(inner, y as u16, 1);
        if value.is_empty() {
            label(f, r, clip(&left, r.width as usize), color);
        } else {
            field(f, r, &left, value, color);
        }
        if !command.is_empty() {
            app.action_hits.push((r, command));
        }
    }
}

fn filter_line(f: &mut Frame, r: Rect, query: &str, editing: bool, active: bool) {
    fill(f, r, PANEL);
    let (text, cursor) = input_window(query, query.len(), r.width.saturating_sub(3) as usize);
    label(f, r, format!("/ {text}"), if editing { BLUE } else { DIM });
    if editing && active {
        f.set_cursor_position((r.x + 2 + cursor as u16, r.y));
    }
}

fn resource_table(f: &mut Frame, r: Rect, view: &mut ResourceView, active: bool) {
    let title = format!(
        "{} ({}){}",
        view.data.kind.name().to_uppercase(),
        view.matches.len(),
        if view.show_archived {
            " · including archived"
        } else {
            ""
        }
    );
    let title_area = area(
        r.x,
        r.y,
        r.width
            .saturating_sub(if view.query.is_empty() { 20 } else { 35 }),
        1,
    );
    let mut spans = vec![Span::styled(title, bold(TEXT))];
    if !view.query.is_empty() && !view.editing {
        let remaining = (title_area.width as usize).saturating_sub(spans[0].width());
        spans.push(Span::styled(
            clip(&format!("  /{}", view.query), remaining),
            style(BLUE),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), title_area);
    let filter_rows = u16::from(view.editing);
    if view.editing {
        filter_line(f, row(r, 1, 1), &view.query, true, active);
    }
    let has_states = view.data.filters.len() > 1;
    if has_states {
        let total_width: usize = view.data.filters.iter().map(|name| name.len() + 3).sum();
        let filters = if total_width > r.width as usize {
            vec![Span::styled(
                format!(" {} ", view.data.filters[view.filter]),
                Style::default().fg(BASE).bg(BLUE),
            )]
        } else {
            view.data
                .filters
                .iter()
                .enumerate()
                .flat_map(|(i, name)| {
                    [
                        Span::styled(
                            format!(" {name} "),
                            if view.filter == i {
                                Style::default().fg(BASE).bg(BLUE)
                            } else {
                                style(DIM)
                            },
                        ),
                        Span::raw(" "),
                    ]
                })
                .collect()
        };
        f.render_widget(
            Paragraph::new(Line::from(filters)),
            row(r, 1 + filter_rows, 1),
        );
    }
    let table_y = 2 + filter_rows + u16::from(has_states);
    let table_area = area(
        r.x,
        r.y + table_y,
        r.width,
        r.height.saturating_sub(table_y),
    );
    view.resize(table_area.height.saturating_sub(1) as usize);
    let header = Row::new(view.data.headers.iter().enumerate().map(|(i, text)| {
        Cell::from(if view.sort_column == Some(i) {
            format!("{text} {}", if view.descending { "▼" } else { "▲" })
        } else {
            (*text).into()
        })
        .style(if view.sort_column == Some(i) {
            bold(BLUE)
        } else {
            bold(MUTED)
        })
    }))
    .style(Style::default().bg(PANEL));
    let mut widths = view.data.widths.clone();
    let fixed: u16 = widths.iter().sum();
    widths.push(
        r.width
            .saturating_sub(fixed + widths.len() as u16 + 2)
            .max(1),
    );
    let visible = view.visible();
    let rows = view.matches[visible.clone()].iter().map(|&index| {
        let record = &view.data.records[index];
        Row::new(record.cells.iter().enumerate().map(|(column, text)| {
            Cell::from(clip(text, widths[column] as usize)).style(style(
                if matches!(view.data.headers[column], "STATE" | "STATUS" | "RESULT") {
                    state_color(text)
                } else {
                    TEXT
                },
            ))
        }))
    });
    let widget = Table::new(rows, widths.iter().copied().map(Constraint::Length))
        .header(header)
        .column_spacing(1)
        .row_highlight_style(Style::default().bg(SELECTED))
        .highlight_symbol("› ");
    let mut state = TableState::default();
    if !view.matches.is_empty() {
        state.select(Some(view.selected - visible.start));
    }
    f.render_stateful_widget(widget, table_area, &mut state);
    if view.matches.is_empty() {
        let text = if !view.query.is_empty() {
            "No matches"
        } else {
            match view.data.kind {
                Kind::Providers => "No connections",
                Kind::Tasks => "No tasks",
                Kind::Agents => "No agents",
                Kind::Board => "No messages",
                Kind::Tools => "No tool activity",
                Kind::Checks => "No checks",
                Kind::Files => "No candidates or exports",
                _ => "No records",
            }
        };
        label(f, row(table_area, 2, 2), text, MUTED);
    }
}

fn footer(f: &mut Frame, r: Rect, app: &App) {
    if let Some(error) = &app.persistence_error {
        label(f, r, error, PEACH);
    } else if let Some((message, _)) = &app.toast {
        label(f, r, message, MUTED);
    }
}

fn button(f: &mut Frame, r: Rect, text: &str, action: Action, app: &mut App) {
    f.render_widget(Paragraph::new(text).style(style(BLUE)), r);
    app.buttons.push((r, action));
}

fn popup(f: &mut Frame, screen: Rect, width: u16, height: u16, title: &str) -> Rect {
    let width = width.min(screen.width.saturating_sub(6));
    let height = height.min(screen.height.saturating_sub(4));
    let r = area(
        screen.x + (screen.width - width) / 2,
        screen.y + (screen.height - height) / 2,
        width,
        height,
    );
    fill(f, area(r.x + 1, r.y + 1, r.width, r.height), Color::Black);
    f.render_widget(Clear, r);
    f.render_widget(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(style(BLUE))
            .style(Style::default().bg(PANEL))
            .title(Span::styled(title.to_owned(), bold(BLUE))),
        r,
    );
    r
}
fn palette(f: &mut Frame, screen: Rect, app: &App) {
    let r = popup(f, screen, 66, COMMANDS.len() as u16 + 6, " Commands ");
    let inner = inset(r, 2, 1);
    label(f, row(inner, 0, 1), format!("{}▏", app.command_query), TEXT);
    label(f, row(inner, 1, 1), "─".repeat(inner.width as usize), LINE);
    let commands = app.commands();
    let capacity = inner.height.saturating_sub(4) as usize;
    let offset = app
        .command_selection
        .saturating_sub(capacity.saturating_sub(1));
    for (position, &index) in commands.iter().enumerate().skip(offset).take(capacity) {
        let item = row(inner, 2 + (position - offset) as u16, 1);
        let selected = position == app.command_selection;
        if selected {
            fill(f, item, SELECTED);
        }
        label(
            f,
            area(item.x, item.y, 15, 1),
            format!(
                "{} {}",
                if selected { "›" } else { " " },
                COMMANDS[index].name
            ),
            if selected { BLUE } else { TEXT },
        );
        label(
            f,
            area(item.x + 16, item.y, item.width.saturating_sub(16), 1),
            COMMANDS[index].description,
            DIM,
        );
    }
    if commands.is_empty() {
        label(f, row(inner, 2, 1), "No matching commands", MUTED);
    }
}
fn detail(f: &mut Frame, screen: Rect, app: &mut App, id: &str) {
    let Some(table) = &app.table else {
        return;
    };
    let Some(record) = table.data.records.iter().find(|r| r.id == id).cloned() else {
        return;
    };
    let width = 80u16.min(screen.width.saturating_sub(6));
    let body_width = width.saturating_sub(4) as usize;
    let mut lines = Vec::new();
    for (name, value) in table.data.headers.iter().zip(&record.cells) {
        for line in wrap(&format!("{name}: {value}"), body_width) {
            lines.push(Line::from(Span::styled(line, style(TEXT))));
        }
    }
    lines.push(Line::default());
    for line in record.detail.lines() {
        if line.is_empty() {
            lines.push(Line::default());
        } else {
            for part in wrap(line, body_width) {
                lines.push(Line::from(Span::styled(part, style(MUTED))));
            }
        }
    }
    let has_links = false;
    let r = popup(
        f,
        screen,
        width,
        (lines.len() + 6 + usize::from(has_links)).min(u16::MAX as usize) as u16,
        &format!(" {} ", record.id),
    );
    let inner = inset(r, 2, 1);
    label(
        f,
        row(inner, 0, 1),
        format!("{} · {}", table.data.kind.name(), record.state),
        state_color(record.state),
    );
    let body = row(
        inner,
        2,
        inner.height.saturating_sub(3 + u16::from(has_links)),
    );
    let total = lines.len();
    app.detail_scroll = app.detail_scroll.min(
        total
            .saturating_sub(body.height as usize)
            .min(u16::MAX as usize) as u16,
    );
    f.render_widget(Paragraph::new(lines).scroll((app.detail_scroll, 0)), body);
}

fn help(f: &mut Frame, screen: Rect) {
    let r = popup(f, screen, 72, 20, " Help ");
    f.render_widget(
        Paragraph::new(
            "/  Ctrl K       Commands
.  a            Entity actions
n  e            Add / edit (tables)
Enter           Details
f  Ctrl F       Filter
Tab  [ ]        State / transcript focus
s  Shift S      Sort / reverse
Ctrl U          Clear filter or input
Esc             Back / cancel
?  F1           Help

FORMS
Tab  Shift Tab  Field
← →             Choice / cursor
Ctrl S          Save

Ctrl B          Task context
Ctrl P          Pause simulation
Ctrl Q          Exit",
        )
        .style(style(TEXT))
        .wrap(Wrap { trim: true }),
        inset(r, 2, 1),
    );
}

fn clip(text: &str, width: usize) -> String {
    if UnicodeWidthStr::width(text) <= width {
        return text.into();
    }
    let mut out = String::new();
    let mut used = 0;
    for g in text.graphemes(true) {
        let w = UnicodeWidthStr::width(g);
        if used + w + 1 > width {
            break;
        }
        out.push_str(g);
        used += w;
    }
    if width > 0 {
        out.push('…');
    }
    out
}

fn input_window(text: &str, cursor: usize, width: usize) -> (String, usize) {
    let mut start = cursor;
    let mut used = 0;
    for (index, grapheme) in text[..cursor].grapheme_indices(true).rev() {
        let size = UnicodeWidthStr::width(grapheme);
        if used + size > width {
            break;
        }
        used += size;
        start = index;
    }
    let mut visible = String::new();
    let mut length = 0;
    for grapheme in text[start..].graphemes(true) {
        let size = UnicodeWidthStr::width(grapheme);
        if length + size > width {
            break;
        }
        visible.push_str(grapheme);
        length += size;
    }
    (visible, used)
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut result = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty()
            && UnicodeWidthStr::width(line.as_str()) + 1 + UnicodeWidthStr::width(word) > width
        {
            result.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        for grapheme in word.graphemes(true) {
            if UnicodeWidthStr::width(line.as_str()) + UnicodeWidthStr::width(grapheme) > width
                && !line.is_empty()
            {
                result.push(std::mem::take(&mut line));
            }
            line.push_str(grapheme);
        }
    }
    if !line.is_empty() {
        result.push(line);
    }
    result
}

fn action_menu(f: &mut Frame, screen: Rect, app: &mut App) {
    let height = (app.menu.len() + 5).min(u16::MAX as usize) as u16;
    let target = app
        .selected_record()
        .map(|r| r.cells.first().cloned().unwrap_or_else(|| r.id.clone()))
        .or_else(|| {
            app.simulation
                .as_ref()
                .and_then(|s| s.task())
                .map(|t| t.id.clone())
        })
        .unwrap_or_default();
    let r = popup(
        f,
        screen,
        68,
        height,
        &format!(" Actions · {} ", clip(&target, 45)),
    );
    let inner = inset(r, 2, 1);
    let capacity = inner.height.saturating_sub(2) as usize;
    let offset = app
        .menu_selection
        .saturating_sub(capacity.saturating_sub(1));
    app.menu_hits.clear();
    for (index, item) in app.menu.iter().enumerate().skip(offset).take(capacity) {
        let area = row(inner, (index - offset) as u16, 1);
        let selected = index == app.menu_selection;
        if selected {
            fill(f, area, SELECTED);
        }
        label(
            f,
            area,
            format!("{} {}", if selected { "›" } else { " " }, item.label),
            if item.reason.is_some() {
                DIM
            } else if selected {
                BLUE
            } else {
                TEXT
            },
        );
        app.menu_hits.push((area, index));
    }
    if app.menu.is_empty() {
        label(f, row(inner, 0, 1), "No actions", DIM);
    }
    if let Some(reason) = app
        .menu
        .get(app.menu_selection)
        .and_then(|item| item.reason.as_ref())
    {
        label(
            f,
            row(inner, inner.height.saturating_sub(2), 2),
            reason,
            PEACH,
        );
    }
}
fn editor(f: &mut Frame, screen: Rect, app: &mut App) {
    let Some(e) = app.editor.as_ref() else {
        return;
    };
    let height = (e.fields.len() as u16 * 3 + 7).min(screen.height.saturating_sub(4));
    let width = 92u16.min(screen.width.saturating_sub(6));
    let r = popup(f, screen, width, height, &format!(" {} ", e.title));
    let inner = inset(r, 2, 1);

    label(
        f,
        row(inner, 1, 1),
        clip(&e.context, inner.width as usize),
        DIM,
    );
    app.editor_hits.clear();
    for (index, field) in e.fields.iter().enumerate() {
        let y = 3 + index as u16 * 3;
        label(
            f,
            row(inner, y, 1),
            &field.label,
            if e.selected == index { BLUE } else { DIM },
        );
        let input = row(inner, y + 1, 1);
        fill(f, input, if e.selected == index { SELECTED } else { CARD });
        match &field.value {
            Value::Text { value, cursor } => {
                let (visible, offset) =
                    input_window(value, *cursor, input.width.saturating_sub(1) as usize);
                label(f, input, visible, TEXT);
                if e.selected == index {
                    f.set_cursor_position((input.x + offset as u16, input.y));
                }
            }
            Value::Choice { .. } => label(f, input, format!("‹ {} ›", field.display()), TEXT),
        }
        app.editor_hits.push((input, index));
    }
    let buttons = row(inner, inner.height.saturating_sub(1), 1);
    let save = area(buttons.x, buttons.y, 10, 1);
    let cancel = area(buttons.x + 13, buttons.y, 10, 1);
    if e.selected == e.fields.len() {
        fill(f, save, SELECTED);
    }
    if e.selected == e.fields.len() + 1 {
        fill(f, cancel, SELECTED);
    }
    label(
        f,
        save,
        if matches!(e.kind, crate::forms::FormKind::Answer { .. }) {
            "[ Send ]"
        } else {
            "[ Save ]"
        },
        BLUE,
    );
    label(f, cancel, "Cancel", MUTED);
    app.editor_hits.push((save, e.fields.len()));
    app.editor_hits.push((cancel, e.fields.len() + 1));
    if let Some(error) = &e.error {
        label(
            f,
            row(inner, inner.height.saturating_sub(2), 1),
            clip(error, inner.width as usize),
            PEACH,
        );
    }
}
fn confirmation(f: &mut Frame, screen: Rect, app: &mut App) {
    let Some(c) = app.confirmation.as_ref() else {
        return;
    };
    let lines = wrap(&c.detail, 64);
    let r = popup(
        f,
        screen,
        72,
        (lines.len() + 6) as u16,
        &format!(" {} ", c.title),
    );
    let inner = inset(r, 2, 1);
    label(
        f,
        row(inner, 0, inner.height.saturating_sub(2)),
        lines.join("\n"),
        TEXT,
    );
    let cancel = area(inner.x, inner.bottom() - 1, 12, 1);
    let accept = area(
        inner.x + 16,
        inner.bottom() - 1,
        inner.width.saturating_sub(16),
        1,
    );
    if c.selected {
        fill(f, accept, SELECTED);
    } else {
        fill(f, cancel, SELECTED);
    }
    label(f, cancel, "Cancel", MUTED);
    label(f, accept, &c.accept, PEACH);
    app.editor_hits = vec![(cancel, 0), (accept, 1)];
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};
    fn screen(app: &mut App, width: u16, height: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
        let b = terminal.backend().buffer();
        (0..height)
            .map(|y| (0..width).map(|x| b[(x, y)].symbol()).collect())
            .collect()
    }
    #[test]
    fn cold_start_has_no_invented_agents_or_navigation_bar() {
        for (w, h) in [(80, 24), (120, 40), (180, 50)] {
            let mut app = App::new();
            let lines = screen(&mut app, w, h);
            let text = lines.join("\n");
            assert!(text.contains("What are we building?"));
            assert!(!text.contains("A-0001"));
            assert!(!text.contains("1 Chat"));
            assert!(text.contains("SIMULATION"));
            assert!(
                !text.contains("Esc back")
                    && !text.contains("Rows 1")
                    && !text.contains("Ctrl Q quit")
            );
        }
    }
    #[test]
    fn sidebar_tracks_the_lifecycle_and_exposes_working_actions() {
        for (scene, word, action) in [
            ("provider", "Provider required", "/providers"),
            ("ready", "Ready", "/start"),
            ("working", "Developing", "/stop"),
            ("review", "Review", "/accept"),
            ("done", "Delivered", "/new"),
        ] {
            let mut app = App::scenario_scene(scene).unwrap();
            let text = screen(&mut app, 144, 44).join("\n");
            assert!(text.contains(word), "{scene}");
            assert!(
                app.action_hits.iter().any(|(_, c)| c == action),
                "{scene}: missing action"
            );
        }
    }
    #[test]
    fn filter_is_hidden_until_requested_and_restores_table_space() {
        let mut app = App::demo(512);
        app.open_table(Kind::Agents, "");
        let baseline = screen(&mut app, 80, 24);
        assert!(
            !baseline[..baseline.len() - 1]
                .iter()
                .any(|l| l.trim_start().starts_with('/'))
        );
        let rows = app.table.as_ref().unwrap().page_rows;
        app.table.as_mut().unwrap().editing = true;
        let active = screen(&mut app, 80, 24);
        assert!(active.iter().any(|l| l.trim_start().starts_with('/')));
        assert_eq!(app.table.as_ref().unwrap().page_rows + 1, rows);
        let t = app.table.as_mut().unwrap();
        t.query = "failed".into();
        t.rebuild();
        t.editing = false;
        let applied = screen(&mut app, 80, 24);
        assert!(
            !applied[..applied.len() - 1]
                .iter()
                .any(|l| l.trim_start().starts_with('/'))
        );
        assert!(applied.join("\n").contains("/failed"));
        assert_eq!(app.table.as_ref().unwrap().matches.len(), 8);
    }
}
