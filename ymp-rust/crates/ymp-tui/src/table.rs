//! Tables in the style of k9s.
//!
//! A list the interface shows outside a popup is a table: a line of upper-case column titles,
//! then one row per record with its cells aligned under them. Widths come from the content. When
//! a table is too wide, its flexible column gives up width first, then columns are hidden by
//! rank, and a popup still shows one record in full.
//!
//! A page stays a list of items. A heading opens a table and carries its columns; the rows after
//! it, up to the next heading, belong to that table and carry one cell per column. This module
//! arranges a built page before anything reads it, so keys, the selection and Inspect act on the
//! rows on screen, and it lays tables out and paints their lines. It builds no rows itself: the
//! pages and the sidebar supply the cells.

use crate::text;
use crate::theme::Theme;
use crate::views::{Item, ItemKind, View};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::cmp::Ordering;

/// Spaces between two columns.
pub const GAP: usize = 2;
/// Cells in front of every page row for the selection marker.
pub const MARKER: usize = 2;
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
    /// The title, in upper case. It may be empty, as it is for a column of markers. A sort is
    /// remembered by this title.
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

    /// This column may be hidden on a narrow surface. Higher ranks are hidden first.
    pub const fn hide(self, rank: u8) -> Self {
        Self { hide: rank, ..self }
    }
}

/// What a cell is ordered by. An unknown value follows every known one in either direction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SortKey {
    Text(String),
    Number(i128),
    Unknown,
}

static UNKNOWN: SortKey = SortKey::Unknown;

/// One cell: styled text, and the value it is ordered by.
#[derive(Clone, Debug)]
pub struct Cell {
    pub spans: Vec<Span<'static>>,
    pub sort: SortKey,
}

impl Cell {
    /// Text, ordered as text without regard to case. An empty cell is ordered as unknown.
    pub fn text(value: impl Into<String>, style: Style) -> Self {
        Self::spans(vec![Span::styled(value.into(), style)])
    }

    /// A figure shown as `display` and ordered by `value`. A figure nobody reported has no value
    /// and is ordered as unknown, never as zero.
    pub fn number(display: impl Into<String>, value: Option<i128>, style: Style) -> Self {
        Self {
            spans: vec![Span::styled(display.into(), style)],
            sort: value.map_or(SortKey::Unknown, SortKey::Number),
        }
    }

    /// Several styled pieces, ordered by the text they read as together.
    pub fn spans(spans: Vec<Span<'static>>) -> Self {
        let plain: String = spans.iter().map(|span| span.content.as_ref()).collect();
        let plain = plain.trim();
        let sort = if plain.is_empty() {
            SortKey::Unknown
        } else {
            SortKey::Text(plain.to_lowercase())
        };
        Self { spans, sort }
    }

    /// The same cell, ordered by `sort` instead of by its text.
    pub fn sorted_by(self, sort: SortKey) -> Self {
        Self { sort, ..self }
    }

    /// A cell with nothing to show.
    pub fn empty() -> Self {
        Self {
            spans: Vec::new(),
            sort: SortKey::Unknown,
        }
    }

    /// The text the cell reads as.
    pub fn plain(&self) -> String {
        self.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    pub fn width(&self) -> usize {
        self.spans
            .iter()
            .map(|span| text::width(&span.content))
            .sum()
    }
}

/// Which way a sorted column runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Ascending,
    Descending,
}

/// The column a table is sorted by, named by its title so that it survives a rebuild.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sort {
    pub column: &'static str,
    pub direction: Direction,
}

impl Sort {
    /// What a column's sort key does when pressed: order by that column ascending, then
    /// descending, then return to the order the page gave.
    pub fn cycle(current: Option<Sort>, column: &'static str) -> Option<Sort> {
        match current {
            Some(sort) if sort.column == column && sort.direction == Direction::Ascending => {
                Some(Sort {
                    column,
                    direction: Direction::Descending,
                })
            }
            Some(sort) if sort.column == column => None,
            _ => Some(Sort {
                column,
                direction: Direction::Ascending,
            }),
        }
    }
}

/// What the reader asked of the open page's tables: a filter, and a sort for each table.
#[derive(Clone, Debug, Default)]
pub struct Controls {
    /// Text every row on the open page must contain. Opening another page clears it.
    pub filter: String,
    /// Keys are being typed into the filter.
    pub typing: bool,
    /// Rows the open page had before the filter removed any.
    pub unfiltered: usize,
    /// The sort of each table, by page and table title, for as long as the window is open.
    sorts: Vec<(View, String, Sort)>,
}

impl Controls {
    pub fn sort(&self, view: View, table: &str) -> Option<Sort> {
        self.sorts
            .iter()
            .find(|(sorted, title, _)| *sorted == view && title == table)
            .map(|(_, _, sort)| *sort)
    }

    pub fn set_sort(&mut self, view: View, table: &str, sort: Option<Sort>) {
        self.sorts
            .retain(|(sorted, title, _)| !(*sorted == view && title == table));
        if let Some(sort) = sort {
            self.sorts.push((view, table.to_owned(), sort));
        }
    }

    /// Stop typing and forget the filter.
    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.typing = false;
    }
}

/// The sort key of every column: the first letter of its title that no earlier column took and
/// the page does not already answer. The key is pressed with Shift, so it is kept in upper case.
pub fn sort_keys(columns: &[Column], reserved: &[char]) -> Vec<Option<char>> {
    let mut taken: Vec<char> = reserved.iter().map(char::to_ascii_uppercase).collect();
    columns
        .iter()
        .map(|column| {
            let key = column
                .title
                .chars()
                .filter(char::is_ascii_alphabetic)
                .map(|letter| letter.to_ascii_uppercase())
                .find(|letter| !taken.contains(letter));
            taken.extend(key);
            key
        })
        .collect()
}

/// Whether a row matches a filter: the text is looked for in every cell, without regard to case.
/// A filter that starts with `!` keeps the rows that do not match.
pub fn matches(filter: &str, cells: &[Cell]) -> bool {
    let filter = filter.trim();
    let (inverse, needle) = match filter.strip_prefix('!') {
        Some(rest) => (true, rest.trim()),
        None => (false, filter),
    };
    if needle.is_empty() {
        return true;
    }
    let needle = needle.to_lowercase();
    let found = cells
        .iter()
        .any(|cell| cell.plain().to_lowercase().contains(&needle));
    found != inverse
}

/// Order two values of one column.
fn compare(a: &SortKey, b: &SortKey, direction: Direction) -> Ordering {
    let order = match (a, b) {
        (SortKey::Unknown, SortKey::Unknown) => return Ordering::Equal,
        // Unknown values follow known ones whichever way the column runs.
        (SortKey::Unknown, _) => return Ordering::Greater,
        (_, SortKey::Unknown) => return Ordering::Less,
        (SortKey::Number(a), SortKey::Number(b)) => a.cmp(b),
        (SortKey::Text(a), SortKey::Text(b)) => a.cmp(b),
        (SortKey::Number(_), SortKey::Text(_)) => Ordering::Less,
        (SortKey::Text(_), SortKey::Number(_)) => Ordering::Greater,
    };
    match direction {
        Direction::Ascending => order,
        Direction::Descending => order.reverse(),
    }
}

/// A row with one cell under several columns is a note that spans its table: a sentence rather
/// than a record. It is not sized with the columns, and a sorted table keeps it above its rows.
pub fn spans_table(columns: &[Column], cells: &[Cell]) -> bool {
    cells.len() == 1 && columns.len() > 1
}

/// Apply a filter and each table's sort to a built page.
///
/// Rows the filter does not keep are removed, and so is a table a filter left with no row. Each
/// heading learns the sort keys of its columns, avoiding the letters in `reserved`, and the sort
/// `sort` names for its title; its rows are then put in that order, and ties keep the order the
/// page gave.
pub fn arrange(
    items: Vec<Item>,
    filter: &str,
    reserved: &[char],
    sort: impl Fn(&str) -> Option<Sort>,
) -> Vec<Item> {
    let filtering = !filter.trim().is_empty();
    let keep = |cells: &[Cell]| !filtering || matches(filter, cells);
    let mut arranged = Vec::with_capacity(items.len());
    let mut items = items.into_iter().peekable();
    while let Some(mut heading) = items.next() {
        if heading.kind != ItemKind::Heading {
            // A row before any table has no columns to be ordered by; only the filter applies.
            if keep(&heading.cells) {
                arranged.push(heading);
            }
            continue;
        }
        let mut rows = Vec::new();
        while let Some(row) = items.next_if(|next| next.kind != ItemKind::Heading) {
            if keep(&row.cells) {
                rows.push(row);
            }
        }
        heading.sort_keys = sort_keys(&heading.columns, reserved);
        let applied = sort(&heading.title).and_then(|sort| {
            heading
                .columns
                .iter()
                .position(|column| column.title == sort.column)
                .map(|column| (sort, column))
        });
        heading.sort = applied.map(|(sort, _)| sort);
        if let Some((sort, column)) = applied {
            let columns = heading.columns.clone();
            fn key(row: &Item, column: usize) -> &SortKey {
                row.cells.get(column).map_or(&UNKNOWN, |cell| &cell.sort)
            }
            rows.sort_by(|a, b| {
                let record = |row: &Item| !spans_table(&columns, &row.cells);
                record(a)
                    .cmp(&record(b))
                    .then_with(|| compare(key(a, column), key(b, column), sort.direction))
            });
        }
        if filtering && rows.is_empty() {
            continue;
        }
        arranged.push(heading);
        arranged.extend(rows);
    }
    arranged
}

/// The width of each column, or `None` for a hidden column. The columns and the gaps between them
/// never take more than `width` cells.
///
/// A column is as wide as its title or its widest cell, and the sorted column keeps room for its
/// direction. A column with neither a title nor a cell takes no room. When the table is wider than
/// `width`, columns are hidden by rank, highest first, until it fits with its flexible column
/// squeezed to a few cells. What is still too much is then given up in this order: the flexible
/// column, down to one cell; the widest other text column, down to one cell; the widest figure,
/// down to one cell; and last, whole columns from the right.
pub fn widths(
    columns: &[Column],
    rows: &[&[Cell]],
    sort: Option<Sort>,
    width: usize,
) -> Vec<Option<usize>> {
    let mut natural: Vec<usize> = columns
        .iter()
        .map(|column| {
            let indicator = if sort.is_some_and(|sort| sort.column == column.title) {
                2
            } else {
                0
            };
            text::width(column.title) + indicator
        })
        .collect();
    for cells in rows {
        if spans_table(columns, cells) {
            continue;
        }
        for (index, cell) in cells.iter().enumerate().take(columns.len()) {
            natural[index] = natural[index].max(cell.width());
        }
    }
    let flex = columns.iter().position(|column| column.flex);
    let floor = |index: usize| natural[index].min(FLEX_MIN.max(text::width(columns[index].title)));
    let mut shown: Vec<Option<usize>> = natural
        .iter()
        .map(|cells| (*cells > 0).then_some(*cells))
        .collect();
    loop {
        let mut squeezed = shown.clone();
        if let Some(index) = flex.filter(|index| shown[*index].is_some()) {
            squeezed[index] = Some(floor(index));
        }
        if span(&squeezed) <= width {
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
    let text = |index: usize| columns[index].align == Align::Left;
    give_way(&mut shown, width, |index| Some(index) == flex, |_| 1);
    give_way(&mut shown, width, text, |_| 1);
    give_way(&mut shown, width, |_| true, |_| 1);
    while span(&shown) > width {
        let Some(last) = shown.iter().rposition(Option::is_some) else {
            break;
        };
        shown[last] = None;
    }
    shown
}

/// The cells a table takes: its visible columns and the gaps between them.
fn span(shown: &[Option<usize>]) -> usize {
    let visible = shown.iter().flatten().count();
    shown.iter().flatten().sum::<usize>() + GAP * visible.saturating_sub(1)
}

/// Take one cell at a time from the widest column `eligible` allows, the rightmost of equals,
/// until the table fits in `width` or every such column is down to `floor`.
fn give_way(
    shown: &mut [Option<usize>],
    width: usize,
    eligible: impl Fn(usize) -> bool,
    floor: impl Fn(usize) -> usize,
) {
    while span(shown) > width {
        let widest = (0..shown.len())
            .filter(|index| eligible(*index))
            .filter_map(|index| shown[index].map(|cells| (cells, index)))
            .filter(|(cells, index)| *cells > floor(*index))
            .max();
        let Some((cells, index)) = widest else {
            return;
        };
        shown[index] = Some(cells - 1);
    }
}

/// The column titles, aligned as the cells under them. The sorted column carries its direction
/// beside its title, and the letter that sorts by a column is underlined in its title.
pub fn titles(
    columns: &[Column],
    widths: &[Option<usize>],
    keys: &[Option<char>],
    sort: Option<Sort>,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let style = theme.muted().add_modifier(Modifier::BOLD);
    let cells: Vec<Cell> = columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            let mut title = column.title.to_owned();
            if let Some(sort) = sort.filter(|sort| sort.column == column.title) {
                title.push(' ');
                title.push_str(match sort.direction {
                    Direction::Ascending => theme.markers.ascending,
                    Direction::Descending => theme.markers.descending,
                });
            }
            Cell::spans(underline(&title, keys.get(index).copied().flatten(), style))
        })
        .collect();
    aligned(columns, widths, &cells)
}

/// The line of column titles over a page table, indented past the selection marker.
pub fn header(
    columns: &[Column],
    widths: &[Option<usize>],
    keys: &[Option<char>],
    sort: Option<Sort>,
    theme: &Theme,
) -> Line<'static> {
    let mut spans = vec![Span::raw(" ".repeat(MARKER))];
    spans.extend(titles(columns, widths, keys, sort, theme));
    Line::from(spans)
}

/// A title split so that the first occurrence of its sort key is underlined.
fn underline(title: &str, key: Option<char>, style: Style) -> Vec<Span<'static>> {
    let Some((at, letter)) = key.and_then(|key| {
        title
            .char_indices()
            .find(|(_, letter)| letter.eq_ignore_ascii_case(&key))
    }) else {
        return vec![Span::styled(title.to_owned(), style)];
    };
    let after = at + letter.len_utf8();
    vec![
        Span::styled(title[..at].to_owned(), style),
        Span::styled(
            title[at..after].to_owned(),
            style.add_modifier(Modifier::UNDERLINED),
        ),
        Span::styled(title[after..].to_owned(), style),
    ]
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

/// One page row: the selection marker, then its cells aligned to `widths`, filled to `width`
/// cells so that a selected row is highlighted across the whole table. A note is one piece of
/// text across the line.
pub fn row(
    columns: &[Column],
    widths: &[Option<usize>],
    cells: &[Cell],
    width: usize,
    theme: &Theme,
    selected: bool,
    focused: bool,
) -> Line<'static> {
    let marker = if selected && focused {
        theme.markers.selection
    } else if selected {
        theme.markers.activity
    } else {
        " "
    };
    let highlight = (selected && focused).then(|| theme.selected());
    let mut spans = vec![Span::styled(
        format!("{marker} "),
        highlight.unwrap_or_else(|| theme.accent()),
    )];
    let body = width.saturating_sub(MARKER);
    let content = if spans_table(columns, cells) || columns.is_empty() {
        let pieces: Vec<Span<'static>> = cells
            .iter()
            .flat_map(|cell| cell.spans.iter().cloned())
            .collect();
        fit(&pieces, body, Align::Left)
    } else {
        let mut content = aligned(columns, widths, cells);
        let used: usize = content.iter().map(|span| text::width(&span.content)).sum();
        if used < body {
            content.push(Span::raw(" ".repeat(body - used)));
        }
        content
    };
    spans.extend(content);
    if let Some(style) = highlight {
        spans = spans
            .into_iter()
            .map(|span| Span::styled(span.content, style))
            .collect();
    }
    Line::from(spans)
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
        Column::right("TOKENS").hide(1),
    ];

    fn cells(name: &str, state: &str, tokens: Option<i128>) -> Vec<Cell> {
        vec![
            Cell::text(name, Style::default()),
            Cell::text(state, Style::default()),
            Cell::number(
                tokens.map_or_else(|| "—".to_owned(), |value| value.to_string()),
                tokens,
                Style::default(),
            ),
        ]
    }

    fn plain(spans: &[Span<'static>]) -> String {
        spans.iter().map(|span| span.content.as_ref()).collect()
    }

    fn keys(items: &[Item]) -> Vec<String> {
        items
            .iter()
            .filter(|item| item.kind == ItemKind::Row)
            .map(|item| item.key.clone())
            .collect()
    }

    #[test]
    fn columns_are_as_wide_as_their_widest_cell_and_figures_align_right() {
        let rows = [
            cells("glm-5.2 max", "working", Some(120)),
            cells("claude-opus-5", "idle", Some(7)),
        ];
        let borrowed: Vec<&[Cell]> = rows.iter().map(Vec::as_slice).collect();
        let widths = widths(&COLUMNS, &borrowed, None, 80);
        assert_eq!(widths, vec![Some(13), Some(7), Some(6)]);
        let theme = crate::theme::theme("ember");
        assert_eq!(
            plain(&header(&COLUMNS, &widths, &[None; 3], None, theme).spans),
            format!("  {:<13}  {:<7}  {:>6}", "NAME", "STATE", "TOKENS")
        );
        let line =
            |cells: &[Cell]| plain(&row(&COLUMNS, &widths, cells, 40, theme, false, false).spans);
        assert_eq!(
            line(&rows[0]),
            format!(
                "  {:<13}  {:<7}  {:>6}{:8}",
                "glm-5.2 max", "working", "120", ""
            )
        );
        assert_eq!(
            line(&rows[1]),
            format!(
                "  {:<13}  {:<7}  {:>6}{:8}",
                "claude-opus-5", "idle", "7", ""
            )
        );
    }

    #[test]
    fn a_narrow_table_squeezes_its_flexible_column_then_hides_by_rank_then_cuts() {
        let rows = [cells("a-very-long-model-identifier", "working", Some(5))];
        let borrowed: Vec<&[Cell]> = rows.iter().map(Vec::as_slice).collect();
        // Everything fits once the name is cut.
        assert_eq!(
            widths(&COLUMNS, &borrowed, None, 30),
            vec![Some(13), Some(7), Some(6)]
        );
        // Too narrow even with the name at its floor: the ranked column goes first.
        assert_eq!(
            widths(&COLUMNS, &borrowed, None, 17),
            vec![Some(8), Some(7), None]
        );
        // Nothing left to hide: the name gives way before the state beside it.
        let narrow = widths(&COLUMNS, &borrowed, None, 14);
        assert_eq!(narrow, vec![Some(5), Some(7), None]);
        assert_eq!(
            plain(&aligned(&COLUMNS, &narrow, &rows[0])),
            "a-ve…  working"
        );
        // With the name down to one cell, the state is cut to what remains.
        let narrower = widths(&COLUMNS, &borrowed, None, 7);
        assert_eq!(narrower, vec![Some(1), Some(4), None]);
        assert_eq!(plain(&aligned(&COLUMNS, &narrower, &rows[0])), "…  wor…");
    }

    #[test]
    fn a_table_never_takes_more_than_its_width() {
        // A Providers page: nothing may be hidden, and a long path is not the flexible column.
        let columns = [
            Column::left("PROVIDER"),
            Column::left("COMMAND").flex(),
            Column::left("EXECUTABLE"),
            Column::right("TURNS"),
            Column::left(""),
        ];
        let rows = [vec![
            Cell::text("claude", Style::default()),
            Cell::text("claude-agent-acp", Style::default()),
            Cell::text(
                "/Users/someone/.local/share/fixture/bin/agent-acp",
                Style::default(),
            ),
            Cell::number("12345", Some(12345), Style::default()),
            Cell::empty(),
        ]];
        let borrowed: Vec<&[Cell]> = rows.iter().map(Vec::as_slice).collect();
        let sort = Sort::cycle(None, "TURNS");
        let natural = span(&widths(&columns, &borrowed, sort, usize::MAX));
        for width in 0..=natural + 2 {
            let shown = widths(&columns, &borrowed, sort, width);
            assert!(
                span(&shown) <= width,
                "{shown:?} takes {} cells of {width}",
                span(&shown)
            );
            let line = plain(&aligned(&columns, &shown, &rows[0]));
            assert_eq!(text::width(&line), span(&shown), "{line:?} at {width}");
        }
        // An empty column takes neither room nor a gap.
        assert_eq!(widths(&columns, &borrowed, None, 200)[4], None);
        // At a 60-column terminal the long path gives way, and the figure stays whole while text
        // can still be cut.
        let page = widths(&columns, &borrowed, None, 56);
        assert_eq!(span(&page), 56, "{page:?}");
        assert_eq!(
            page[3],
            Some(5),
            "the figure was cut before the text: {page:?}"
        );
        assert!(page[2].is_some_and(|cells| cells < 49), "{page:?}");
        let line = plain(&aligned(&columns, &page, &rows[0]));
        assert!(line.ends_with("12345") && line.contains('…'), "{line:?}");
        // Only when every text column is down to one cell is the figure cut.
        let tight = widths(&columns, &borrowed, None, 13);
        assert_eq!(tight, vec![Some(1), Some(1), Some(1), Some(4), None]);
    }

    #[test]
    fn sorting_cycles_and_unknown_values_follow_known_ones_either_way() {
        let mut items = vec![Item::table("", &COLUMNS)];
        items.extend(
            [("b", Some(20)), ("a", None), ("c", Some(3))]
                .map(|(name, tokens)| Item::row(name, cells(name, "idle", tokens))),
        );
        let order = |sort: Option<Sort>| keys(&arrange(items.clone(), "", &[], |_| sort));
        let ascending = Sort::cycle(None, "TOKENS");
        // Numbers, not the text they are shown as: 3 comes before 20.
        assert_eq!(order(ascending), ["c", "b", "a"]);
        let descending = Sort::cycle(ascending, "TOKENS");
        assert_eq!(order(descending), ["b", "c", "a"]);
        assert_eq!(Sort::cycle(descending, "TOKENS"), None);
        assert_eq!(order(None), ["b", "a", "c"]);
        let arranged = arrange(items.clone(), "", &[], |_| descending);
        assert_eq!(
            arranged[0].sort, descending,
            "the heading does not know its sort"
        );
        let theme = crate::theme::theme("ember");
        let borrowed: Vec<&[Cell]> = arranged[1..]
            .iter()
            .map(|item| item.cells.as_slice())
            .collect();
        let widths = widths(&COLUMNS, &borrowed, descending, 80);
        let titles = titles(&COLUMNS, &widths, &arranged[0].sort_keys, descending, theme);
        assert!(
            plain(&titles).contains(&format!("TOKENS {}", theme.markers.descending)),
            "the sorted column does not show its direction: {:?}",
            plain(&titles)
        );
        assert!(
            titles.iter().any(|span| span.content == "T"
                && span.style.add_modifier.contains(Modifier::UNDERLINED)),
            "the sort key is not underlined in its title"
        );
    }

    #[test]
    fn a_note_stays_above_the_rows_of_a_sorted_table() {
        let items = vec![
            Item::table("", &COLUMNS),
            Item::note("about", vec![Span::raw("What this page is")]),
            Item::row("z", cells("z", "idle", Some(1))),
            Item::row("a", cells("a", "idle", Some(2))),
        ];
        let sort = Sort::cycle(None, "NAME");
        assert_eq!(
            keys(&arrange(items, "", &[], |_| sort)),
            ["about", "a", "z"]
        );
    }

    #[test]
    fn a_filter_keeps_matching_rows_and_a_leading_bang_inverts_it() {
        let mut items = vec![Item::table("", &COLUMNS)];
        items.push(Item::row("one", cells("glm-5.2 max", "working", Some(1))));
        items.push(Item::row("two", cells("claude-opus-5", "idle", Some(2))));
        let filtered = |filter: &str| keys(&arrange(items.clone(), filter, &[], |_| None));
        assert_eq!(filtered("GLM"), ["one"]);
        assert_eq!(filtered("!glm"), ["two"]);
        assert_eq!(filtered("! glm"), ["two"]);
        assert_eq!(filtered(""), ["one", "two"]);
        assert_eq!(filtered("!"), ["one", "two"]);
        // A table left with nothing under a filter is not shown at all.
        assert!(arrange(items.clone(), "no such row", &[], |_| None).is_empty());
    }

    #[test]
    fn sort_keys_take_the_first_free_letter_of_each_title() {
        let columns = [
            Column::left(""),
            Column::left("STATE"),
            Column::left("SESSION"),
            Column::left("RESULT"),
        ];
        assert_eq!(
            sort_keys(&columns, &['R']),
            vec![None, Some('S'), Some('E'), Some('U')]
        );
    }
}
