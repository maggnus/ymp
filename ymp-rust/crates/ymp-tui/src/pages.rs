//! Full-screen data pages — the `DataPage` structure of the design handoff.
//!
//! One dense table per page, in the k9s tradition: breadcrumb with count and state marker,
//! column headers, rows with a stable selection, `↳` fix rows under unusable entries, note
//! lines, and a footer naming the next keys. Pages reuse the application frame; only the body
//! differs from the transcript.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::text;
use crate::theme;
use crate::transcript::pad;

/// One table cell: text plus its style. Fixtures build these; live views map domain data.
#[derive(Clone, Debug)]
pub struct Cell {
    pub text: String,
    pub style: Style,
}

impl Cell {
    pub fn new(text: impl Into<String>, style: Style) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }
}

/// One table row, optionally followed by a `↳ fix:` line, optionally dimmed.
#[derive(Clone, Debug)]
pub struct Row {
    pub cells: Vec<Cell>,
    pub fix: Option<Vec<Span<'static>>>,
    pub dim: bool,
}

/// Column with a fixed width in cells; `0` means "take the rest".
#[derive(Clone, Debug)]
pub struct Column {
    pub title: &'static str,
    pub width: u16,
}

/// A describe surface: named groups of `key: value` lines instead of a table.
#[derive(Clone, Debug)]
pub struct DescribeGroup {
    pub title: String,
    pub fields: Vec<(String, Vec<Span<'static>>)>,
}

#[derive(Clone, Debug)]
pub enum Body {
    Table {
        columns: Vec<Column>,
        rows: Vec<Row>,
    },
    Describe {
        groups: Vec<DescribeGroup>,
    },
}

/// A full-screen data page.
#[derive(Clone, Debug)]
pub struct Page {
    /// Breadcrumb segments; the last one is the page identity and is drawn in amber.
    pub breadcrumb: Vec<String>,
    /// Facts after the breadcrumb: "· 2 ready · 3 unusable" or cursor/head/lag for events.
    pub summary: Vec<Span<'static>>,
    pub body: Body,
    /// Muted explanatory lines under the table.
    pub notes: Vec<String>,
    /// Left half of the footer: what is true right now.
    pub footer: Vec<Span<'static>>,
    /// Right-side footer keys, e.g. [("Enter", "describe"), ("Esc", "back")].
    pub keys: Vec<(&'static str, &'static str)>,
    pub selected: usize,
}

impl Page {
    pub fn rows_len(&self) -> usize {
        match &self.body {
            Body::Table { rows, .. } => rows.len(),
            Body::Describe { .. } => 0,
        }
    }

    pub fn select_next(&mut self) {
        let len = self.rows_len();
        if len > 0 {
            self.selected = (self.selected + 1).min(len - 1);
        }
    }

    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    /// Lay the page body out for a given viewport. The frame draws the surrounding chrome.
    ///
    /// Rows are windowed around the selection, so a table longer than the terminal keeps the
    /// selected row visible and states how many rows are outside the window instead of
    /// silently dropping them.
    pub fn layout(&self, width: u16, height: u16) -> Vec<Line<'static>> {
        let width = width as usize;
        let mut lines = Vec::new();

        let note_lines = self.note_lines(width);
        let capacity = (height as usize)
            .saturating_sub(note_lines.len() + usize::from(!note_lines.is_empty()));

        match &self.body {
            Body::Table { columns, rows } => {
                let widths = resolve_widths(columns, width);

                // Column headers.
                let mut header: Vec<Span<'static>> = vec![Span::raw(" ".to_owned())];
                for (column, w) in columns.iter().zip(&widths) {
                    header.push(Span::styled(pad(column.title, *w), theme::muted()));
                }
                lines.push(Line::from(header));

                let (start, end, hidden) = self.window(rows, capacity.saturating_sub(2));
                for (index, row) in rows.iter().enumerate().take(end).skip(start) {
                    let selected = index == self.selected;
                    let lead = if selected {
                        theme::selected()
                    } else {
                        theme::text()
                    };
                    let mut spans: Vec<Span<'static>> = vec![Span::styled(" ".to_owned(), lead)];

                    let mut used = 1usize;
                    for (cell, w) in row.cells.iter().zip(&widths) {
                        let style = if selected {
                            theme::selected()
                        } else if row.dim {
                            theme::faint()
                        } else {
                            cell.style
                        };
                        spans.push(Span::styled(
                            pad(&text::truncate(&cell.text, *w), *w),
                            style,
                        ));
                        used += *w;
                    }
                    if selected && used < width {
                        spans.push(Span::styled(" ".repeat(width - used), theme::selected()));
                    }
                    lines.push(Line::from(spans));

                    if let Some(fix) = &row.fix {
                        let mut fix_spans: Vec<Span<'static>> =
                            vec![Span::styled("   ".to_owned(), theme::faint())];
                        fix_spans.extend(fix.iter().cloned());
                        lines.push(Line::from(fix_spans));
                    }
                }

                if hidden > 0 {
                    lines.push(Line::from(Span::styled(
                        format!(" … {hidden} more rows outside the window · ↑↓ select"),
                        theme::faint(),
                    )));
                }
            }

            Body::Describe { groups } => {
                let key_width = groups
                    .iter()
                    .flat_map(|group| group.fields.iter())
                    .map(|(key, _)| text::width(key))
                    .max()
                    .unwrap_or(0)
                    .max(12);

                for (index, group) in groups.iter().enumerate() {
                    if index > 0 {
                        lines.push(Line::default());
                    }
                    lines.push(Line::from(Span::styled(group.title.clone(), theme::text())));
                    for (key, value) in &group.fields {
                        let mut spans: Vec<Span<'static>> = vec![
                            Span::raw("  ".to_owned()),
                            Span::styled(pad(&format!("{key}:"), key_width + 2), theme::amber()),
                        ];
                        spans.extend(value.iter().cloned());
                        lines.push(Line::from(spans));
                    }
                }
            }
        }

        if !note_lines.is_empty() {
            lines.push(Line::default());
            lines.extend(note_lines);
        }

        lines
    }

    fn note_lines(&self, width: usize) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for note in &self.notes {
            for piece in text::wrap(note, width.saturating_sub(2).max(1)) {
                lines.push(Line::from(vec![
                    Span::raw("  ".to_owned()),
                    Span::styled(piece, theme::muted()),
                ]));
            }
        }
        lines
    }

    /// The window of rows to draw: keeps the selected row visible and counts what is outside.
    fn window(&self, rows: &[Row], capacity: usize) -> (usize, usize, usize) {
        let cost = |row: &Row| 1 + usize::from(row.fix.is_some());
        let total: usize = rows.iter().map(cost).sum();
        if capacity == 0 || total <= capacity {
            return (0, rows.len(), 0);
        }

        // Start at the selection and grow backwards, then forwards, until the window is full.
        let selected = self.selected.min(rows.len().saturating_sub(1));
        let mut start = selected;
        let mut end = selected + 1;
        let mut used = cost(&rows[selected]);
        while used < capacity && (start > 0 || end < rows.len()) {
            if end < rows.len() && used + cost(&rows[end]) <= capacity {
                used += cost(&rows[end]);
                end += 1;
            } else if start > 0 && used + cost(&rows[start - 1]) <= capacity {
                start -= 1;
                used += cost(&rows[start]);
            } else {
                break;
            }
        }
        (start, end, rows.len() - (end - start))
    }
}

/// Fixed widths stay; the single flexible column absorbs the remainder.
fn resolve_widths(columns: &[Column], total: usize) -> Vec<usize> {
    let fixed: usize = columns
        .iter()
        .filter(|column| column.width > 0)
        .map(|column| column.width as usize)
        .sum();
    let flexible = columns.iter().filter(|column| column.width == 0).count();
    let remainder = total.saturating_sub(fixed + 1);
    let share = if flexible > 0 {
        remainder / flexible.max(1)
    } else {
        0
    };
    columns
        .iter()
        .map(|column| {
            if column.width > 0 {
                column.width as usize
            } else {
                share
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(rows: Vec<Row>, selected: usize) -> Page {
        Page {
            breadcrumb: vec!["transcript".into(), format!("sample[{}]", rows.len())],
            summary: vec![],
            body: Body::Table {
                columns: vec![
                    Column {
                        title: "PROFILE",
                        width: 18,
                    },
                    Column {
                        title: "STATE",
                        width: 0,
                    },
                ],
                rows,
            },
            notes: vec!["one explanatory note".into()],
            footer: vec![],
            keys: vec![("Enter", "describe"), ("Esc", "back")],
            selected,
        }
    }

    fn row(name: &str, state: &str, fix: Option<&str>) -> Row {
        Row {
            cells: vec![
                Cell::new(name.to_owned(), theme::text()),
                Cell::new(state.to_owned(), theme::green()),
            ],
            fix: fix.map(|text| vec![Span::styled(text.to_owned(), theme::muted())]),
            dim: false,
        }
    }

    fn rendered(page: &Page, width: u16, height: u16) -> Vec<String> {
        page.layout(width, height)
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    fn sample() -> Page {
        table(
            vec![
                row("profile-a", "ready", None),
                row(
                    "profile-b",
                    "blocked",
                    Some("↳ probe reported no executable"),
                ),
            ],
            1,
        )
    }

    #[test]
    fn selection_moves_within_bounds() {
        let mut page = sample();
        page.selected = 0;
        page.select_prev();
        assert_eq!(page.selected, 0);
        page.select_next();
        assert_eq!(page.selected, 1);
        page.select_next();
        assert_eq!(page.selected, 1);
    }

    #[test]
    fn layout_contains_headers_rows_and_fix_lines() {
        let page = sample();
        let lines = rendered(&page, 80, 20);
        let joined = lines.join("\n");
        assert!(joined.contains("PROFILE"), "{joined}");
        assert!(joined.contains("profile-a"), "{joined}");
        assert!(joined.contains("no executable"), "{joined}");
        for line in &lines {
            assert!(text::width(line) <= 80, "line {line:?} too wide");
        }
    }

    #[test]
    fn a_table_longer_than_the_viewport_keeps_the_selection_and_counts_the_rest() {
        let name = |index: usize| format!("row-{index:04}");
        let rows: Vec<Row> = (0..4096)
            .map(|index| row(&name(index), "ready", None))
            .collect();
        let page = table(rows, 318);
        let lines = rendered(&page, 80, 12);
        let joined = lines.join("\n");
        assert!(joined.contains(&name(318)), "{joined}");
        assert!(!joined.contains(&name(0)), "{joined}");
        assert!(joined.contains("more rows outside the window"), "{joined}");
        assert!(lines.len() <= 12, "{} lines drawn", lines.len());
    }
}
