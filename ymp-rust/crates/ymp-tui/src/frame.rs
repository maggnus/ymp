//! Layout and the drawing primitives every surface shares.
//!
//! This is the only module that writes into a ratatui frame. Views and the sidebar build
//! lines; the frame decides where they land. Keeping the two apart is what lets the layout
//! stay responsive without every page repeating its own size arithmetic.

use crate::text;
use crate::theme::Theme;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

/// The shortest terminal that still gets the header and the status row.
pub const CHROME_MIN_HEIGHT: u16 = 10;

/// The horizontal rows of the application chrome. A zero-height rect means the row was
/// dropped because the terminal is too short for it.
pub struct Rows {
    pub header: Rect,
    pub top_rule: Rect,
    pub body: Rect,
    pub bottom_rule: Rect,
    pub composer: Rect,
    pub status: Rect,
}

/// Split the screen. The composer is never sacrificed: it keeps its rows even when that
/// leaves the body with a single line, and the header and status row are dropped first.
pub fn chrome(area: Rect, composer_height: u16) -> Rows {
    let composer_height = composer_height.clamp(1, area.height.max(1));
    if area.height < CHROME_MIN_HEIGHT {
        let composer_height = composer_height.min(area.height.saturating_sub(1)).max(1);
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(composer_height),
                Constraint::Length(if area.height > composer_height { 1 } else { 0 }),
            ])
            .split(area);
        return Rows {
            header: Rect::new(area.x, area.y, area.width, 0),
            top_rule: Rect::new(area.x, area.y, area.width, 0),
            body: split[0],
            bottom_rule: Rect::new(area.x, area.y, area.width, 0),
            composer: split[1],
            status: split[2],
        };
    }
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(composer_height),
            Constraint::Length(1),
        ])
        .split(area);
    Rows {
        header: split[0],
        top_rule: split[1],
        body: split[2],
        bottom_rule: split[3],
        composer: split[4],
        status: split[5],
    }
}

/// Sidebar width for a terminal `total` cells wide. Zero means no sidebar fits.
pub fn sidebar_width(total: u16) -> u16 {
    match total {
        0..=71 => 0,
        72..=99 => 26,
        100..=139 => 30,
        _ => 34,
    }
}

/// Split the body into the main column, a one-cell rule, and the sidebar.
pub fn split_body(body: Rect, sidebar: u16) -> (Rect, Option<(Rect, Rect)>) {
    if sidebar == 0 || body.width <= sidebar + 8 {
        return (body, None);
    }
    let split = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(8),
            Constraint::Length(1),
            Constraint::Length(sidebar),
        ])
        .split(body);
    (split[0], Some((split[1], split[2])))
}

/// Paint the background of an area with a theme token.
pub fn fill(frame: &mut Frame, area: Rect, theme: &Theme) {
    frame.render_widget(Block::default().style(theme.base()), area);
}

pub fn fill_surface(frame: &mut Frame, area: Rect, theme: &Theme) {
    frame.render_widget(Block::default().style(theme.surface()), area);
}

pub fn hairline(frame: &mut Frame, area: Rect, theme: &Theme) {
    hairline_with(frame, area, theme, theme.rule());
}

/// A hairline drawn in a chosen style, so a focused region can carry a brighter edge.
pub fn hairline_with(frame: &mut Frame, area: Rect, theme: &Theme, style: ratatui::style::Style) {
    if area.height == 0 {
        return;
    }
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            theme.markers.hline.repeat(area.width as usize),
            style,
        ))),
        area,
    );
}

/// The rule between the main column and the sidebar. It brightens when the sidebar owns
/// the keyboard, which is one of the two signals that the focus is there.
pub fn vertical_rule(frame: &mut Frame, area: Rect, theme: &Theme, focused: bool) {
    let style = theme.border(focused);
    let lines = (0..area.height)
        .map(|_| Line::from(Span::styled(theme.markers.vline.to_owned(), style)))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines), area);
}

/// Draw `lines` into `area`, clipped rather than wrapped.
pub fn paint(frame: &mut Frame, area: Rect, lines: Vec<Line<'static>>) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let mut visible = lines;
    visible.truncate(area.height as usize);
    frame.render_widget(Paragraph::new(visible), area);
}

/// A single row with a left and a right half.
pub fn row(frame: &mut Frame, area: Rect, left: Vec<Span<'static>>, right: Vec<Span<'static>>) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    frame.render_widget(
        Paragraph::new(text::row(area.width as usize, left, right)),
        area,
    );
}

/// A `key label` run for a header or a status row.
pub fn key_hints(hints: &[(&str, &str)], theme: &Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (index, (key, label)) in hints.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("  ".to_owned(), theme.faint()));
        }
        spans.push(Span::styled((*key).to_owned(), theme.accent()));
        spans.push(Span::styled(format!(" {label}"), theme.muted()));
    }
    spans
}

/// How a floating surface reads. A choice carries the accent border and names its keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalRole {
    Choice,
    Reference,
}

pub struct ModalSpec {
    pub title: String,
    pub badge: String,
    pub role: ModalRole,
    pub width: u16,
    pub body: Vec<Line<'static>>,
    pub footer: Vec<(&'static str, &'static str)>,
    pub scroll: usize,
}

/// Push everything already drawn into the background so a floating surface reads alone.
pub fn dim(frame: &mut Frame, area: Rect, theme: &Theme) {
    let buffer = frame.buffer_mut();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            buffer[(x, y)].set_fg(theme.rule).set_bg(theme.bg);
        }
    }
}

/// Draw a floating surface. Returns the inner rect so a caller can place a cursor in it.
pub fn render_modal(frame: &mut Frame, area: Rect, spec: &ModalSpec, theme: &Theme) -> Rect {
    dim(frame, area, theme);
    let width = spec.width.min(area.width.saturating_sub(2)).max(12);
    let footer_height = u16::from(!spec.footer.is_empty());
    let max_body = area.height.saturating_sub(4 + footer_height).max(1) as usize;
    let total = spec.body.len();
    let scroll = spec.scroll.min(total.saturating_sub(1));
    let visible: Vec<Line<'static>> = spec
        .body
        .iter()
        .skip(scroll)
        .take(max_body)
        .cloned()
        .collect();
    let height = visible.len() as u16 + 2 + footer_height;
    let rect = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 3,
        width,
        height: height.min(area.height),
    };
    frame.render_widget(Clear, rect);
    let border = match spec.role {
        ModalRole::Choice => theme.accent(),
        ModalRole::Reference => theme.rule(),
    };
    let mut block = Block::default()
        .borders(Borders::ALL)
        .border_style(border)
        .style(theme.surface());
    let inner_width = rect.width.saturating_sub(2) as usize;
    let badge_width = if spec.badge.is_empty() {
        0
    } else {
        text::width(&spec.badge) + 2
    };
    if inner_width > badge_width + 4 {
        block = block.title_top(Line::from(Span::styled(
            format!(
                " {} ",
                text::truncate(&spec.title, inner_width - badge_width - 4)
            ),
            theme.accent_bold(),
        )));
        if !spec.badge.is_empty() {
            block = block.title_top(
                Line::from(Span::styled(format!(" {} ", spec.badge), theme.muted()))
                    .right_aligned(),
            );
        }
    }
    if total > visible.len() {
        block = block.title_bottom(
            Line::from(Span::styled(
                format!(" {} more ", total - visible.len() - scroll),
                theme.faint(),
            ))
            .right_aligned(),
        );
    }
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    if footer_height == 1 && inner.height > 0 {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(inner);
        paint(frame, split[0], visible);
        row(frame, split[1], key_hints(&spec.footer, theme), Vec::new());
        return split[0];
    }
    paint(frame, inner, visible);
    inner
}
