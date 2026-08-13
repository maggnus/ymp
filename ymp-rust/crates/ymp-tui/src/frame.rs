//! The single component and layout layer — `AppFrame` of the design handoff.
//!
//! Every full-screen surface is described by a [`SurfaceSpec`] and every floating surface by a
//! [`ModalSpec`]; this module is the only place that writes into a ratatui frame. Composition
//! modules (`ui`, `overlay`) build specs and never draw, so a page cannot acquire a private
//! drawing path without the framework inventory check rejecting it.
//!
//! Deviation from `ymp-docs/design/ymp_chat_tui.dc.html`, recorded deliberately: the command
//! palette is drawn as one bordered box with an inner rule instead of two stacked boxes. The
//! content, keys and behaviour are the artifact's; one box lets the palette reuse the same
//! component as every other floating surface.

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::text;
use crate::theme;

/// The smallest terminal the interface supports.
pub const MIN_WIDTH: u16 = 80;
pub const MIN_HEIGHT: u16 = 24;

/// The narrowest title worth drawing on a border: two spaces and something between them.
const MIN_TITLE_ROOM: usize = 6;

/// A full-screen surface: one header row, the body, one input row, one status row, hairlines.
#[derive(Clone, Debug, Default)]
pub struct SurfaceSpec {
    pub header_left: Vec<Span<'static>>,
    pub header_right: Vec<Span<'static>>,
    pub body: Vec<Line<'static>>,
    pub input_left: Vec<Span<'static>>,
    pub input_right: Vec<Span<'static>>,
    pub status_left: Vec<Span<'static>>,
    pub status_right: Vec<Span<'static>>,
}

/// A floating surface. A decision carries the amber frame; reference material carries the rule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModalRole {
    /// A decision the operator must make: amber frame, explicit action, Esc.
    Decision,
    /// Reference or navigation: grey frame, never an irreversible action.
    Reference,
}

#[derive(Clone, Debug)]
pub struct ModalSpec {
    pub title: String,
    pub badge: String,
    pub role: ModalRole,
    /// Preferred width in cells; clamped to the area.
    pub width: u16,
    pub body: Vec<Line<'static>>,
}

/// The six rows every surface is laid out into.
pub struct Rows {
    pub header: Rect,
    pub rule_top: Rect,
    pub body: Rect,
    pub input: Rect,
    pub rule_bottom: Rect,
    pub status: Rect,
}

pub fn rows(area: Rect) -> Rows {
    let split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);
    Rows {
        header: split[0],
        rule_top: split[1],
        body: split[2],
        input: split[3],
        rule_bottom: split[4],
        status: split[5],
    }
}

/// Draw a full-screen surface. The only entry point for a page or the transcript.
pub fn render_surface(frame: &mut Frame, area: Rect, spec: &SurfaceSpec) {
    let rows = rows(area);
    split_row(frame, rows.header, &spec.header_left, &spec.header_right);
    hairline(frame, rows.rule_top);
    body(frame, rows.body, &spec.body);
    split_row(frame, rows.input, &spec.input_left, &spec.input_right);
    hairline(frame, rows.rule_bottom);
    split_row(frame, rows.status, &spec.status_left, &spec.status_right);
}

/// Draw a floating surface over a dimmed frame.
///
/// The badge carries the irreversibility marker, so the border reserves it before the title and
/// the title yields: a decision drawn with a long identifier reads as truncated, never as
/// reversible. When even the reserved badge would not fit on the border, it moves to the first
/// body line rather than disappearing.
pub fn render_modal(frame: &mut Frame, area: Rect, spec: &ModalSpec) {
    dim(frame, area);

    let badge = if spec.badge.is_empty() {
        String::new()
    } else {
        format!(" {} ", spec.badge)
    };
    let badge_width = text::width(&badge);
    // Reserve one row for the badge when it cannot share the border with the title.
    let probe_inner = spec
        .width
        .min(area.width.saturating_sub(2))
        .saturating_sub(2) as usize;
    let badge_on_border = badge.is_empty() || badge_width + MIN_TITLE_ROOM <= probe_inner;

    let mut body = spec.body.clone();
    if !badge.is_empty() && !badge_on_border {
        body.insert(
            0,
            Line::from(Span::styled(badge.trim().to_owned(), theme::amber())),
        );
    }

    let height = body.len() as u16 + 2;
    let rect = centered(area, spec.width, height);
    frame.render_widget(Clear, rect);

    let inner_width = rect.width.saturating_sub(2) as usize;
    let title_room = if badge_on_border {
        inner_width.saturating_sub(badge_width + 1)
    } else {
        inner_width
    };

    let border = match spec.role {
        ModalRole::Decision => theme::accent(),
        ModalRole::Reference => theme::rule(),
    };
    let mut block = Block::default().borders(Borders::ALL).border_style(border);
    if title_room >= MIN_TITLE_ROOM {
        block = block.title_top(Line::from(Span::styled(
            format!(" {} ", text::truncate(&spec.title, title_room - 2)),
            match spec.role {
                ModalRole::Decision => theme::accent_bold(),
                ModalRole::Reference => theme::bold(),
            },
        )));
    }
    if badge_on_border && !badge.is_empty() {
        block = block.title_top(Line::from(Span::styled(badge, theme::amber())).right_aligned());
    }
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    frame.render_widget(Paragraph::new(body), inner);
}

/// The guard below the minimum size. It replaces every other surface.
pub fn render_size_guard(frame: &mut Frame, area: Rect, lines: &[Line<'static>]) {
    let width = lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| text::width(&span.content))
                .sum::<usize>()
        })
        .max()
        .unwrap_or(0) as u16;
    let top = area.height.saturating_sub(lines.len() as u16) / 2;
    let rect = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + top,
        width: width.min(area.width),
        height: (lines.len() as u16).min(area.height),
    };
    frame.render_widget(Paragraph::new(lines.to_vec()), rect);
}

/// Push every already-drawn cell into the background so a floating surface reads alone.
fn dim(frame: &mut Frame, area: Rect) {
    let buffer = frame.buffer_mut();
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            buffer[(x, y)].set_fg(theme::RULE).set_bg(theme::BG);
        }
    }
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + area.height.saturating_sub(height) / 3,
        width,
        height,
    }
}

fn hairline(frame: &mut Frame, area: Rect) {
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "─".repeat(area.width as usize),
            theme::rule(),
        ))),
        area,
    );
}

fn body(frame: &mut Frame, area: Rect, lines: &[Line<'static>]) {
    let mut visible = lines.to_vec();
    visible.truncate(area.height as usize);
    while visible.len() < area.height as usize {
        visible.push(Line::default());
    }
    frame.render_widget(Paragraph::new(visible), area);
}

/// Left and right halves on one row. The left side is truncated rather than clipping the keys.
fn split_row(frame: &mut Frame, area: Rect, left: &[Span<'static>], right: &[Span<'static>]) {
    frame.render_widget(Paragraph::new(row_line(area.width, left, right)), area);
}

/// The composed row, exposed so line-level assertions do not need a rendered buffer.
pub fn row_line(width: u16, left: &[Span<'static>], right: &[Span<'static>]) -> Line<'static> {
    let width = width as usize;
    let right_width: usize = right.iter().map(|span| text::width(&span.content)).sum();
    let left_width: usize = left.iter().map(|span| text::width(&span.content)).sum();

    let mut spans: Vec<Span<'static>> = Vec::new();
    if left_width + right_width + 2 > width {
        let budget = width.saturating_sub(right_width + 3);
        let mut used = 0usize;
        for span in left {
            let piece = text::width(&span.content);
            if used + piece <= budget {
                used += piece;
                spans.push(span.clone());
            } else {
                let room = budget.saturating_sub(used);
                if room > 1 {
                    spans.push(Span::styled(
                        text::truncate(&span.content, room),
                        span.style,
                    ));
                    used += room;
                }
                break;
            }
        }
        spans.push(Span::raw(
            " ".repeat(width.saturating_sub(used + right_width)),
        ));
    } else {
        spans.extend(left.iter().cloned());
        spans.push(Span::raw(
            " ".repeat(width.saturating_sub(left_width + right_width)),
        ));
    }
    spans.extend(right.iter().cloned());
    Line::from(spans)
}

/// A `key label  key label` run for a header or footer.
pub fn key_hints(hints: &[(&str, &str)]) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    for (index, (key, label)) in hints.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("  ".to_owned(), theme::faint()));
        }
        spans.push(Span::styled((*key).to_owned(), theme::accent()));
        spans.push(Span::styled(format!(" {label}"), theme::muted()));
    }
    spans
}

/// A full-width rule inside a floating surface.
pub fn inner_rule(width: u16) -> Line<'static> {
    Line::from(Span::styled("─".repeat(width as usize), theme::rule()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_keeps_the_right_half_when_the_left_overflows() {
        let left = vec![Span::raw("l".repeat(200))];
        let right = vec![Span::raw("Esc back".to_owned())];
        let line = row_line(80, &left, &right);
        let text: String = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert!(text.ends_with("Esc back"), "{text:?}");
        assert_eq!(text::width(&text), 80);
    }

    #[test]
    fn a_row_pads_to_the_full_width() {
        let line = row_line(
            40,
            &[Span::raw("left".to_owned())],
            &[Span::raw("right".to_owned())],
        );
        let text: String = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect();
        assert_eq!(text::width(&text), 40);
    }
}
