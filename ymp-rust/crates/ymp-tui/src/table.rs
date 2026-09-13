//! Tables in the style of k9s.
//!
//! A list the interface shows outside a popup is a table: a line of upper-case column titles,
//! then one row per record with its cells aligned under them. Widths come from the content. When
//! a table is too wide, its flexible column gives up width first, then columns are hidden by
//! rank, and a popup still shows one record in full. This module lays tables out and paints their
//! lines; the pages and the sidebar supply the cells.

use crate::text;
use crate::theme::Theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

/// Spaces between two columns.
pub const GAP: usize = 2;
/// The narrowest a flexible column is squeezed before another column is hidden.
const FLEX_MIN: usize = 8;

/// Which edge of its column a cell is aligned to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    /// Figures, so that their digits line up.
    Right,
}

/// One column of a table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Column {
    /// The title, in upper case. It may be empty, as it is for a column of markers.
    pub title: &'static str,
    pub align: Align,
    /// The column that gives up width first when the table is too wide.
    pub flex: bool,
    /// Columns are hidden in descending order of this rank when a squeezed table is still too
    /// wide. Rank zero is never hidden.
    pub hide: u8,
}

impl Column {
    pub const fn left(title: &'static str) -> Self {
        Self {
            title,
            align: Align::Left,
            flex: false,
            hide: 0,
        }
    }

    pub const fn right(title: &'static str) -> Self {
        Self {
            title,
            align: Align::Right,
            flex: false,
            hide: 0,
        }
    }

    /// This column gives up width first when the table is too wide.
    pub const fn flex(self) -> Self {
        Self { flex: true, ..self }
    }
}

/// One cell: styled text.
#[derive(Clone, Debug)]
pub struct Cell {
    pub spans: Vec<Span<'static>>,
}

impl Cell {
    pub fn text(value: impl Into<String>, style: Style) -> Self {
        Self {
            spans: vec![Span::styled(value.into(), style)],
        }
    }

    pub fn width(&self) -> usize {
        self.spans
            .iter()
            .map(|span| text::width(&span.content))
            .sum()
    }
}

/// The width of each column, or `None` for a hidden column.
///
/// A column is as wide as its title or its widest cell. When the table is wider than `width`,
/// columns are hidden by rank, highest first, until the table fits with its flexible column
/// squeezed to a few cells; the flexible column then gives up what is still too much. If only
/// columns of rank zero remain and the table is still too wide, the last visible column is cut.
pub fn widths(columns: &[Column], rows: &[&[Cell]], width: usize) -> Vec<Option<usize>> {
    let mut natural: Vec<usize> = columns
        .iter()
        .map(|column| text::width(column.title))
        .collect();
    for cells in rows {
        for (index, cell) in cells.iter().enumerate().take(columns.len()) {
            natural[index] = natural[index].max(cell.width());
        }
    }
    let flex = columns.iter().position(|column| column.flex);
    let floor = |index: usize| natural[index].min(FLEX_MIN.max(text::width(columns[index].title)));
    let total = |shown: &[Option<usize>]| {
        let visible = shown.iter().flatten().count();
        shown.iter().flatten().sum::<usize>() + GAP * visible.saturating_sub(1)
    };
    let mut shown: Vec<Option<usize>> = natural.iter().copied().map(Some).collect();
    loop {
        let mut squeezed = shown.clone();
        if let Some(index) = flex.filter(|index| shown[*index].is_some()) {
            squeezed[index] = Some(floor(index));
        }
        if total(&squeezed) <= width {
            break;
        }
        let hidden = columns
            .iter()
            .enumerate()
            .filter(|(index, column)| column.hide > 0 && shown[*index].is_some())
            .max_by_key(|(index, column)| (column.hide, *index))
            .map(|(index, _)| index);
        match hidden {
            Some(index) => shown[index] = None,
            None => break,
        }
    }
    let excess = total(&shown).saturating_sub(width);
    if let Some(index) = flex.filter(|index| shown[*index].is_some() && excess > 0) {
        shown[index] = Some(natural[index].saturating_sub(excess).max(floor(index)));
    }
    let excess = total(&shown).saturating_sub(width);
    if excess > 0 {
        if let Some(last) = shown.iter().rposition(Option::is_some) {
            shown[last] = shown[last].map(|cells| cells.saturating_sub(excess).max(1));
        }
    }
    shown
}

/// The column titles, aligned as the cells under them.
pub fn titles(columns: &[Column], widths: &[Option<usize>], theme: &Theme) -> Vec<Span<'static>> {
    let style = theme.muted().add_modifier(Modifier::BOLD);
    let cells: Vec<Cell> = columns
        .iter()
        .map(|column| Cell::text(column.title, style))
        .collect();
    aligned(columns, widths, &cells)
}

/// Cells aligned to `widths`, with the gap between columns and hidden columns left out.
pub fn aligned(columns: &[Column], widths: &[Option<usize>], cells: &[Cell]) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut first = true;
    for (index, column) in columns.iter().enumerate() {
        let Some(cells_wide) = widths.get(index).copied().flatten() else {
            continue;
        };
        if !first {
            spans.push(Span::raw(" ".repeat(GAP)));
        }
        first = false;
        let pieces = cells
            .get(index)
            .map(|cell| cell.spans.as_slice())
            .unwrap_or(&[]);
        spans.extend(fit(pieces, cells_wide, column.align));
    }
    spans
}

/// Spans cut to `width` cells and padded to exactly `width` on the side `align` leaves free.
fn fit(spans: &[Span<'static>], width: usize, align: Align) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    let mut used = 0usize;
    for span in spans {
        let piece = text::width(&span.content);
        if used + piece <= width {
            out.push(span.clone());
            used += piece;
            continue;
        }
        let room = width - used;
        if room > 0 {
            let cut = text::truncate(&span.content, room);
            used += text::width(&cut);
            out.push(Span::styled(cut, span.style));
        }
        break;
    }
    let padding = Span::raw(" ".repeat(width.saturating_sub(used)));
    match align {
        Align::Left => out.push(padding),
        Align::Right => out.insert(0, padding),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLUMNS: [Column; 3] = [
        Column::left("NAME").flex(),
        Column::left("STATE"),
        Column {
            hide: 1,
            ..Column::right("TOKENS")
        },
    ];

    fn cells(name: &str, state: &str, tokens: &str) -> Vec<Cell> {
        vec![
            Cell::text(name, Style::default()),
            Cell::text(state, Style::default()),
            Cell::text(tokens, Style::default()),
        ]
    }

    fn plain(spans: &[Span<'static>]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }

    #[test]
    fn columns_are_as_wide_as_their_widest_cell_and_figures_align_right() {
        let rows = [
            cells("glm-5.2 max", "working", "120"),
            cells("claude-opus-5", "idle", "7"),
        ];
        let borrowed: Vec<&[Cell]> = rows.iter().map(Vec::as_slice).collect();
        let widths = widths(&COLUMNS, &borrowed, 80);
        assert_eq!(widths, vec![Some(13), Some(7), Some(6)]);
        let theme = crate::theme::theme("ember");
        assert_eq!(
            plain(&titles(&COLUMNS, &widths, theme)),
            format!("{:<13}  {:<7}  {:>6}", "NAME", "STATE", "TOKENS")
        );
        assert_eq!(
            plain(&aligned(&COLUMNS, &widths, &rows[0])),
            format!("{:<13}  {:<7}  {:>6}", "glm-5.2 max", "working", "120")
        );
        assert_eq!(
            plain(&aligned(&COLUMNS, &widths, &rows[1])),
            format!("{:<13}  {:<7}  {:>6}", "claude-opus-5", "idle", "7")
        );
    }

    #[test]
    fn a_narrow_table_squeezes_its_flexible_column_then_hides_by_rank_then_cuts() {
        let rows = [cells("a-very-long-model-identifier", "working", "5")];
        let borrowed: Vec<&[Cell]> = rows.iter().map(Vec::as_slice).collect();
        // Everything fits once the name is cut.
        assert_eq!(
            widths(&COLUMNS, &borrowed, 30),
            vec![Some(13), Some(7), Some(6)]
        );
        // Too narrow even with the name at its floor: the ranked column goes first.
        assert_eq!(
            widths(&COLUMNS, &borrowed, 17),
            vec![Some(8), Some(7), None]
        );
        // Nothing left to hide: the last visible column is cut to what remains.
        assert_eq!(
            widths(&COLUMNS, &borrowed, 14),
            vec![Some(8), Some(4), None]
        );
        let widths = widths(&COLUMNS, &borrowed, 14);
        assert_eq!(
            plain(&aligned(&COLUMNS, &widths, &rows[0])),
            "a-very-…  wor…"
        );
    }
}
