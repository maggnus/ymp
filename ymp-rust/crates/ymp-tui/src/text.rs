//! Text measurement, sanitising, wrapping, and a small Markdown renderer.
//!
//! Every string that reaches the screen from a provider, the file system, or a stored
//! message passes through [`sanitize`] first: payload is data, never instructions for the
//! terminal. Wrapping is computed here rather than delegated to `Paragraph`, because the
//! transcript needs to know how tall an entry is before it can be scrolled or selected.

use crate::theme::Theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Display width of a string in terminal cells.
pub fn width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn char_width(ch: char) -> usize {
    UnicodeWidthChar::width(ch).unwrap_or(0)
}

/// Shorten `text` to at most `max` cells, marking the cut with an ellipsis.
pub fn truncate(text: &str, max: usize) -> String {
    if width(text) <= max {
        return text.to_owned();
    }
    if max == 0 {
        return String::new();
    }
    if max == 1 {
        return "…".to_owned();
    }
    let mut out = String::new();
    let mut used = 0usize;
    for ch in text.chars() {
        let w = char_width(ch);
        if used + w > max - 1 {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('…');
    out
}

/// Remove escape sequences and control characters, and flatten tabs.
pub fn sanitize(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\u{1b}' => match chars.peek() {
                // An operating system command runs until BEL or ST.
                Some(']') => {
                    for next in chars.by_ref() {
                        if next == '\u{7}' {
                            break;
                        }
                        if next == '\u{1b}' {
                            if matches!(chars.peek(), Some('\\')) {
                                chars.next();
                            }
                            break;
                        }
                    }
                }
                // A control sequence runs until a byte in 0x40..=0x7e.
                Some('[') => {
                    chars.next();
                    for next in chars.by_ref() {
                        if ('\u{40}'..='\u{7e}').contains(&next) {
                            break;
                        }
                    }
                }
                _ => {
                    chars.next();
                }
            },
            '\n' => out.push('\n'),
            '\t' => out.push_str("    "),
            c if c.is_control() => {}
            c if ('\u{80}'..='\u{9f}').contains(&c) => {}
            c => out.push(c),
        }
    }
    out
}

/// Collapse a possibly multi-line value into a single line of readable text.
pub fn one_line(text: &str) -> String {
    let mut out = String::new();
    let mut space = false;
    for ch in sanitize(text).chars() {
        if ch.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(ch);
    }
    out
}

/// The first non-empty line of `text`.
pub fn first_line(text: &str) -> String {
    sanitize(text)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .to_owned()
}

/// The leading characters of an identifier, enough to recognise it without filling a row.
pub fn short_id(id: &str) -> String {
    // A stored id may carry what it is as a prefix, as a retained entry's does. Eight characters
    // of that prefix name the kind and not the record, so the prefix is dropped first and two
    // entries of the same kind stay told apart.
    let body = id.split_once(':').map(|(_, rest)| rest).unwrap_or(id);
    body.chars().take(8).collect()
}

/// Wrap `text` to `width` cells on word boundaries, splitting words too long to fit.
///
/// Interior whitespace is content, not separation: `print("a  b")` must keep both spaces,
/// because a rendered agent message is evidence a reader may compare against a file. Only
/// whitespace that falls on a wrap boundary is dropped, which is unavoidable. Leading
/// indentation is preserved for the same reason.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let indent = paragraph.len() - paragraph.trim_start_matches(' ').len();
        let indent = if indent < width { indent } else { 0 };
        let mut line = paragraph[..indent].to_owned();
        let mut used = indent;
        for chunk in paragraph[indent..].split_inclusive(' ') {
            let word = chunk.trim_end_matches(' ');
            let spaces = chunk.len() - word.len();
            let word_width = self::width(word);
            if used > indent && used + word_width > width {
                lines.push(std::mem::take(&mut line));
                used = 0;
            }
            if word_width > width {
                for piece in split_to_width(word, width) {
                    let piece_width = self::width(&piece);
                    if used > 0 && used + piece_width > width {
                        lines.push(std::mem::take(&mut line));
                        used = 0;
                    }
                    line.push_str(&piece);
                    used += piece_width;
                }
            } else {
                line.push_str(word);
                used += word_width;
            }
            if spaces > 0 && used > 0 {
                let room = width.saturating_sub(used).min(spaces);
                line.push_str(&" ".repeat(room));
                used += room;
            }
        }
        lines.push(std::mem::take(&mut line));
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Wrap `text` to `width` cells without losing a single character.
///
/// Code is wrapped this way. A word-wrapped line may drop the space it broke on, which is
/// harmless in prose and wrong in a command line or a literal string.
pub fn wrap_exact(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut lines = vec![String::new()];
    let mut used = 0usize;
    for (index, paragraph) in text.split('\n').enumerate() {
        if index > 0 {
            lines.push(String::new());
            used = 0;
        }
        for ch in paragraph.chars() {
            let cell = UnicodeWidthChar::width(ch).unwrap_or(0);
            if used + cell > width {
                lines.push(String::new());
                used = 0;
            }
            if let Some(line) = lines.last_mut() {
                line.push(ch);
            }
            used += cell;
        }
    }
    lines
}

fn split_to_width(word: &str, width: usize) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut piece = String::new();
    let mut used = 0usize;
    for ch in word.chars() {
        let w = char_width(ch);
        if used + w > width && !piece.is_empty() {
            pieces.push(std::mem::take(&mut piece));
            used = 0;
        }
        piece.push(ch);
        used += w;
    }
    if !piece.is_empty() {
        pieces.push(piece);
    }
    pieces
}

/// Render a small, well-behaved subset of Markdown: headings, bullets, ordered items,
/// fenced code, inline code, and bold runs. Anything else is shown verbatim.
pub fn markdown(text: &str, width: usize, theme: &Theme, base: Style) -> Vec<Line<'static>> {
    let markers = theme.markers;
    let mut lines = Vec::new();
    let mut fenced = false;
    for raw in sanitize(text).split('\n') {
        let trimmed = raw.trim_end();
        if trimmed.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            let gutter = format!("{} ", markers.gutter);
            // Code is wrapped exactly: indentation and repeated spaces are content.
            for piece in wrap_exact(trimmed, width.saturating_sub(gutter.len() + 1).max(1)) {
                lines.push(Line::from(vec![
                    Span::styled(gutter.clone(), theme.faint()),
                    Span::styled(piece, theme.code()),
                ]));
            }
            continue;
        }
        if trimmed.trim().is_empty() {
            lines.push(Line::default());
            continue;
        }
        let body = trimmed.trim_start();
        if let Some(heading) = body.strip_prefix("### ").or(body.strip_prefix("## ")) {
            push_wrapped(&mut lines, heading, width, theme, theme.bold(), "");
            continue;
        }
        if let Some(heading) = body.strip_prefix("# ") {
            push_wrapped(&mut lines, heading, width, theme, theme.accent_bold(), "");
            continue;
        }
        if let Some(item) = body
            .strip_prefix("- ")
            .or(body.strip_prefix("* "))
            .or(body.strip_prefix("+ "))
        {
            let bullet = format!("{} ", markers.bullet);
            push_wrapped(&mut lines, item, width, theme, base, &bullet);
            continue;
        }
        push_wrapped(&mut lines, trimmed, width, theme, base, "");
    }
    if lines.is_empty() {
        lines.push(Line::default());
    }
    lines
}

fn push_wrapped(
    lines: &mut Vec<Line<'static>>,
    text: &str,
    width: usize,
    theme: &Theme,
    base: Style,
    prefix: &str,
) {
    let prefix_width = self::width(prefix);
    let room = width.saturating_sub(prefix_width).max(1);
    for (index, piece) in wrap(text, room).into_iter().enumerate() {
        let mut spans = Vec::new();
        if index == 0 && !prefix.is_empty() {
            spans.push(Span::styled(prefix.to_owned(), theme.accent()));
        } else if prefix_width > 0 {
            spans.push(Span::raw(" ".repeat(prefix_width)));
        }
        spans.extend(inline(&piece, base, theme));
        lines.push(Line::from(spans));
    }
}

/// Split a single line into styled spans for inline code and bold runs.
pub fn inline(text: &str, base: Style, theme: &Theme) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut buffer = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '`' => {
                flush(&mut spans, &mut buffer, base);
                let mut code = String::new();
                for next in chars.by_ref() {
                    if next == '`' {
                        break;
                    }
                    code.push(next);
                }
                if !code.is_empty() {
                    spans.push(Span::styled(code, theme.code()));
                }
            }
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                flush(&mut spans, &mut buffer, base);
                let mut strong = String::new();
                while let Some(next) = chars.next() {
                    if next == '*' && chars.peek() == Some(&'*') {
                        chars.next();
                        break;
                    }
                    strong.push(next);
                }
                if !strong.is_empty() {
                    spans.push(Span::styled(strong, base.add_modifier(Modifier::BOLD)));
                }
            }
            _ => buffer.push(ch),
        }
    }
    flush(&mut spans, &mut buffer, base);
    if spans.is_empty() {
        spans.push(Span::styled(String::new(), base));
    }
    spans
}

fn flush(spans: &mut Vec<Span<'static>>, buffer: &mut String, style: Style) {
    if !buffer.is_empty() {
        spans.push(Span::styled(std::mem::take(buffer), style));
    }
}

/// Compose a left-aligned and a right-aligned run of spans on one row of `width` cells.
/// The left side is truncated rather than pushing the right side off the row.
pub fn row(width: usize, left: Vec<Span<'static>>, right: Vec<Span<'static>>) -> Line<'static> {
    let right_width: usize = right.iter().map(|s| self::width(&s.content)).sum();
    let left_width: usize = left.iter().map(|s| self::width(&s.content)).sum();
    let mut spans: Vec<Span<'static>> = Vec::new();
    if left_width + right_width + 1 > width {
        let budget = width.saturating_sub(right_width + 2);
        let mut used = 0usize;
        for span in left {
            let piece = self::width(&span.content);
            if used + piece <= budget {
                used += piece;
                spans.push(span);
            } else {
                let room = budget.saturating_sub(used);
                if room > 1 {
                    spans.push(Span::styled(truncate(&span.content, room), span.style));
                    used += room;
                }
                break;
            }
        }
        spans.push(Span::raw(
            " ".repeat(width.saturating_sub(used + right_width)),
        ));
    } else {
        spans.extend(left);
        spans.push(Span::raw(
            " ".repeat(width.saturating_sub(left_width + right_width)),
        ));
    }
    spans.extend(right);
    Line::from(spans)
}

/// Compose the header of a page: its own name on the left, its summary on the right.
///
/// `row` spends the width on the right side first, which is right for a record row, where a
/// value matters more than a long key. A page header is the opposite. The title and the
/// command are how a reader knows which page they are reading, and the summary can carry a
/// captured count of any size, so here it is the right side that gives way.
pub fn header_row(
    width: usize,
    left: Vec<Span<'static>>,
    right: Vec<Span<'static>>,
) -> Line<'static> {
    let left_width: usize = left.iter().map(|s| self::width(&s.content)).sum();
    let right_width: usize = right.iter().map(|s| self::width(&s.content)).sum();
    if left_width + right_width < width {
        return row(width, left, right);
    }
    if left_width + 1 >= width {
        // Narrower than the name itself: the name is what is left to keep.
        return row(width, left, Vec::new());
    }
    let budget = width - left_width - 1;
    let mut spans = left;
    let mut kept: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    for span in right {
        let piece = self::width(&span.content);
        if used + piece <= budget {
            used += piece;
            kept.push(span);
        } else {
            let room = budget - used;
            if room > 1 {
                kept.push(Span::styled(truncate(&span.content, room), span.style));
                used += room;
            }
            break;
        }
    }
    spans.push(Span::raw(" ".repeat(width - left_width - used)));
    spans.extend(kept);
    Line::from(spans)
}

/// `HH:MM` extracted from an RFC 3339 timestamp, or an empty string.
pub fn clock(timestamp: &str) -> String {
    timestamp
        .split_once('T')
        .map(|(_, time)| time.chars().take(5).collect())
        .unwrap_or_default()
}

/// Lay out composer text exactly, character by character, and locate the cursor.
///
/// The composer hard-wraps rather than word-wraps: every character the user typed must be
/// visible in the position they typed it, including runs of spaces, and the cursor must
/// land on the cell the next character will occupy.
pub fn compose(text: &str, width: usize, cursor: usize) -> (Vec<String>, (usize, usize)) {
    let width = width.max(1);
    let mut lines = vec![String::new()];
    let mut row = 0usize;
    let mut col = 0usize;
    let mut byte = 0usize;
    let mut position = (0usize, 0usize);
    for ch in text.chars() {
        if ch == '\n' {
            if byte == cursor {
                position = (row, col);
            }
            lines.push(String::new());
            row += 1;
            col = 0;
            byte += 1;
            continue;
        }
        let cell = UnicodeWidthChar::width(ch).unwrap_or(0);
        if col + cell > width {
            lines.push(String::new());
            row += 1;
            col = 0;
        }
        if byte == cursor {
            position = (row, col);
        }
        if let Some(line) = lines.last_mut() {
            line.push(ch);
        }
        col += cell;
        byte += ch.len_utf8();
    }
    if byte == cursor {
        position = (row, col);
    }
    (lines, position)
}

/// The trailing `cells` display cells of `text`, marked when anything was dropped.
pub fn last_cells(text: &str, cells: usize) -> String {
    if width(text) <= cells {
        return text.to_owned();
    }
    let mut kept: Vec<char> = Vec::new();
    let mut used = 0usize;
    for ch in text.chars().rev() {
        let cell = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + cell > cells.saturating_sub(1) {
            break;
        }
        kept.push(ch);
        used += cell;
    }
    kept.reverse();
    format!("…{}", kept.into_iter().collect::<String>())
}
