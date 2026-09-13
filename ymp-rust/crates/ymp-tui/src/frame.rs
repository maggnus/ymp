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

/// The inset page content keeps on each side of the main column, in cells.
pub const PAGE_INSET: u16 = 2;

/// The widest the read-only inspect surface grows, and the narrowest any floating surface
/// is drawn at. [`render_modal`] holds to the same two numbers.
const INSPECT_MAX: u16 = 86;
pub const MODAL_MIN_WIDTH: u16 = 12;

/// Columns a page wraps its own lines for inside a main column `total` cells wide.
///
/// One width, for every surface that paints page content: the row list, the detail pane
/// and the empty state. A page is wrapped once, so a surface that painted it into a
/// narrower rect than this would cut the right edge off every line there, which is how
/// prose loses a word in the middle of a sentence.
pub fn page_content_width(total: u16) -> u16 {
    total.saturating_sub(PAGE_INSET * 2)
}

/// Columns the main column keeps beside a sidebar `sidebar` cells wide: the arithmetic
/// [`split_body`] performs, stated once so the controller can ask for the width the frame
/// will paint instead of the width of the whole terminal.
pub fn main_width(total: u16, sidebar: u16) -> u16 {
    if sidebar == 0 || total <= sidebar + 8 {
        total
    } else {
        total.saturating_sub(sidebar + 1)
    }
}

/// Width of the read-only inspect surface over a terminal `total` cells wide.
pub fn inspect_width(total: u16) -> u16 {
    INSPECT_MAX
        .min(total.saturating_sub(4))
        .max(MODAL_MIN_WIDTH)
}

/// Columns that surface has for its body: its border and a cell of padding take two cells on
/// each side. Lines built for anything wider are cut when they are painted.
pub fn inspect_content_width(total: u16) -> u16 {
    modal_content_width(inspect_width(total))
}

/// The widest a file preview grows: most source lines fit beside their numbers.
const PREVIEW_MAX: u16 = 132;

/// Width of the file preview surface over a terminal `total` cells wide.
pub fn preview_width(total: u16) -> u16 {
    PREVIEW_MAX
        .min(total.saturating_sub(4))
        .max(MODAL_MIN_WIDTH)
}

/// Columns the file preview has for its body, as [`inspect_content_width`] counts them.
pub fn preview_content_width(total: u16) -> u16 {
    modal_content_width(preview_width(total))
}

/// Cells a floating surface keeps across its width for its border and for a cell of padding
/// inside the border on each side.
pub const MODAL_CHROME: u16 = 4;

/// Columns a floating surface `width` cells wide has for its body.
pub fn modal_content_width(width: u16) -> u16 {
    width.saturating_sub(MODAL_CHROME)
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
    if main_width(body.width, sidebar) == body.width {
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

/// The rule between the main column and the sidebar.
pub fn vertical_rule(frame: &mut Frame, area: Rect, theme: &Theme) {
    let style = theme.border(false);
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

/// A page header, painted so that the page's own name survives a long summary.
pub fn header(frame: &mut Frame, area: Rect, left: Vec<Span<'static>>, right: Vec<Span<'static>>) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    frame.render_widget(
        Paragraph::new(text::header_row(area.width as usize, left, right)),
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

/// The inside of a bordered surface less a cell of padding on each side, so text never
/// touches the border.
pub fn padded(inner: Rect) -> Rect {
    if inner.width <= 2 {
        return inner;
    }
    Rect {
        x: inner.x + 1,
        width: inner.width - 2,
        ..inner
    }
}

/// Blank a margin one cell wide around a floating surface, so the text underneath never runs up
/// against its border.
///
/// The margin is blanked only where it falls inside `regions`, the areas that carry content: the
/// main column and the sidebar. The rules around and between them keep every cell the surface
/// itself does not cover, because a blanked cell on a rule reads as a gap cut into it.
pub fn clear_around(frame: &mut Frame, rect: Rect, regions: &[Rect], theme: &Theme) {
    let left = rect.x.saturating_sub(1);
    let top = rect.y.saturating_sub(1);
    let margin = Rect {
        x: left,
        y: top,
        width: rect.right().saturating_add(1) - left,
        height: rect.bottom().saturating_add(1) - top,
    };
    for region in regions {
        let blank = margin.intersection(*region);
        if blank.is_empty() {
            continue;
        }
        frame.render_widget(Clear, blank);
        frame.render_widget(Block::default().style(theme.base()), blank);
    }
}

/// Rows a floating surface's keys take. They sit a blank row below the body, so they never read
/// as its last line.
const MODAL_KEY_ROWS: u16 = 2;

/// Rows of its body a floating surface shows over an area `height` rows tall. Its border and a
/// row of the area above and below it take four, and its keys, when it names any, take
/// [`MODAL_KEY_ROWS`] more. A list longer than that has to keep its selection within these rows.
pub fn modal_body_rows(height: u16, keys: bool) -> usize {
    let keys = if keys { MODAL_KEY_ROWS } else { 0 };
    height.saturating_sub(4 + keys).max(1) as usize
}

/// The furthest a body `lines` long scrolls on a surface that shows `rows` of it: the point where
/// its last line reaches the last row. Scrolling further would only leave blank rows under it.
pub fn modal_last_scroll(lines: usize, rows: usize) -> usize {
    lines.saturating_sub(rows)
}

/// Draw a floating surface. Returns the rect its body is painted in, inside the border and the
/// padding, so a caller can place a cursor in it.
///
/// The surface is placed in `area`, but the margin around it is cleared only inside `regions`,
/// the main column and the sidebar: a margin that reached a rule would cut a gap into it.
pub fn render_modal(
    frame: &mut Frame,
    area: Rect,
    regions: &[Rect],
    spec: &ModalSpec,
    theme: &Theme,
) -> Rect {
    dim(frame, area, theme);
    let width = spec
        .width
        .min(area.width.saturating_sub(2))
        .max(MODAL_MIN_WIDTH);
    let keys = !spec.footer.is_empty();
    let footer_height = if keys { MODAL_KEY_ROWS } else { 0 };
    let max_body = modal_body_rows(area.height, keys);
    let total = spec.body.len();
    // The surface is as tall as the most of its body it can show, wherever the body is scrolled
    // to, so scrolling never resizes it.
    let rows = total.min(max_body);
    let scroll = spec.scroll.min(modal_last_scroll(total, rows));
    let visible: Vec<Line<'static>> = spec.body.iter().skip(scroll).take(rows).cloned().collect();
    let height = rows as u16 + 2 + footer_height;
    let rect = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 3,
        width,
        height: height.min(area.height),
    };
    clear_around(frame, rect, regions, theme);
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
    let inner = padded(block.inner(rect));
    frame.render_widget(block, rect);
    if footer_height > 0 && inner.height > footer_height {
        let split = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
            .split(inner);
        paint(frame, split[0], visible);
        row(frame, split[2], key_hints(&spec.footer, theme), Vec::new());
        return split[0];
    }
    paint(frame, inner, visible);
    inner
}
