//! Text measurement, sanitising, wrapping, and a small Markdown renderer.
//!
//! Every string that reaches the screen from a provider, the file system, or a stored
//! message passes through [`sanitize`] first, or, when it is code, through [`literal`], which
//! writes each control character out instead of removing it: payload is data, never
//! instructions for the terminal. Wrapping is computed here rather than delegated to
//! `Paragraph`, because the transcript needs to know how tall an entry is before it can be
//! scrolled or selected.

use crate::diff;
use crate::highlight;
use crate::theme::Theme;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use std::time::Duration;
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

/// Wrap styled pieces to `width` cells without losing a single character. Every row is returned,
/// including an empty one for empty input.
///
/// Code is wrapped this way. A word-wrapped line may drop the space it broke on, which is
/// harmless in prose and wrong in a command line or a literal string.
pub fn wrap_styled(pieces: &[(Style, String)], width: usize) -> Vec<Vec<Span<'static>>> {
    let width = width.max(1);
    let mut rows: Vec<Vec<Span<'static>>> = vec![Vec::new()];
    let mut used = 0usize;
    for (style, piece) in pieces {
        let mut run = String::new();
        for ch in piece.chars() {
            let cell = char_width(ch);
            if used + cell > width && used > 0 {
                if let Some(row) = rows.last_mut() {
                    if !run.is_empty() {
                        row.push(Span::styled(std::mem::take(&mut run), *style));
                    }
                }
                rows.push(Vec::new());
                used = 0;
            }
            run.push(ch);
            used += cell;
        }
        if let Some(row) = rows.last_mut() {
            if !run.is_empty() {
                row.push(Span::styled(run, *style));
            }
        }
    }
    rows
}

/// Code beyond this much source in one message is not highlighted.
pub const MESSAGE_CODE_BYTES: usize = 64 * 1024;

/// Tab stops in code are this many columns apart.
pub const TAB: usize = 4;

/// A part of a message as [`markdown`] reads it. Lines carry no line ending.
#[derive(Debug, PartialEq, Eq)]
pub enum Block<'a> {
    Prose(&'a str),
    /// A fenced code block. `closed` is false when the text ends inside it.
    Code {
        opening: &'a str,
        info: &'a str,
        lines: Vec<&'a str>,
        closed: bool,
    },
    /// A complete unified or Git patch that no fence declares.
    Patch(Vec<&'a str>),
}

/// The fence that opened a code block.
struct Fence<'a> {
    mark: char,
    count: usize,
    indent: usize,
    info: &'a str,
}

fn indentation(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// A line that opens a fenced code block: three or more backticks or tildes, then an info string,
/// which after backticks may not contain one, so a line of inline code opens nothing. Unlike
/// CommonMark, any indentation is accepted, because a fence inside a list item is indented with
/// the item.
fn opening(line: &str) -> Option<Fence<'_>> {
    let indent = indentation(line);
    let rest = &line[indent..];
    let mark = rest.chars().next().filter(|ch| matches!(ch, '`' | '~'))?;
    let count = rest.len() - rest.trim_start_matches(mark).len();
    let info = rest[count..].trim_matches([' ', '\t']);
    (count >= 3 && !(mark == '`' && info.contains('`'))).then_some(Fence {
        mark,
        count,
        indent,
        info,
    })
}

/// Whether `line` closes the block `fence` opened: the same character at least as many times,
/// nothing after it but spaces and tabs, indented no more than three columns past the fence.
fn closes(line: &str, fence: &Fence) -> bool {
    let indent = indentation(line);
    let rest = &line[indent..];
    let count = rest.len() - rest.trim_start_matches(fence.mark).len();
    indent <= fence.indent + 3
        && count >= fence.count
        && rest[count..].trim_matches([' ', '\t']).is_empty()
}

/// Split a message into prose lines, fenced code blocks and patches.
///
/// A line ends at `\n`, and one `\r` before it belongs to the ending. A code line keeps
/// everything else, trailing whitespace included, less the indentation of its fence. A fence left
/// open runs to the end of the text. A patch outside a fence must be complete by its own
/// structure; a list of lines that begin with `+` or `-` is prose.
pub fn blocks(text: &str) -> Vec<Block<'_>> {
    let lines: Vec<&str> = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .collect();
    let mut blocks = Vec::new();
    let mut index = 0;
    // Lines before this one have been read looking for a patch, and are not read again.
    let mut scanned = 0;
    while index < lines.len() {
        let line = lines[index];
        if let Some(fence) = opening(line) {
            let mut code = Vec::new();
            let mut closed = false;
            index += 1;
            while let Some(&next) = lines.get(index) {
                index += 1;
                if closes(next, &fence) {
                    closed = true;
                    break;
                }
                code.push(&next[indentation(next).min(fence.indent)..]);
            }
            blocks.push(Block::Code {
                opening: line,
                info: fence.info,
                lines: code,
                closed,
            });
            continue;
        }
        if index >= scanned && (line.starts_with("diff --git ") || line.starts_with("--- ")) {
            let found = diff::scan(&lines[index..]);
            scanned = index + found.read;
            if let Some(length) = found.patch {
                blocks.push(Block::Patch(lines[index..index + length].to_vec()));
                index += length;
                continue;
            }
        }
        blocks.push(Block::Prose(line));
        index += 1;
    }
    blocks
}

/// The line that opened the fenced code block `text` ends inside, if it ends inside one.
pub fn open_fence(text: &str) -> Option<&str> {
    match blocks(text).pop() {
        Some(Block::Code {
            opening,
            closed: false,
            ..
        }) => Some(opening),
        _ => None,
    }
}

/// Render a small, well-behaved subset of Markdown: headings, bullets, inline code, bold runs,
/// fenced code and patches. Anything else is shown verbatim.
///
/// Prose is sanitised line by line, and inline code in it stays literal. Code is drawn with
/// [`literal`] and wrapped without losing a character. A fence that names a language the
/// highlighter knows is drawn in the theme's syntax colours; a block declared `diff` or `patch`,
/// an unlabelled block that holds only a patch, and a complete patch in the prose are drawn by the
/// role of each line, which keeps its own marker. Highlighting one message is bounded by
/// [`MESSAGE_CODE_BYTES`] and [`highlight::BUDGET`]: code beyond either is plain, and a note under
/// the first block affected says so. No text is dropped, and the message itself is not changed.
pub fn markdown(text: &str, width: usize, theme: &Theme, base: Style) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut allowance = Allowance::default();
    for block in blocks(text) {
        match block {
            Block::Prose(line) => push_prose(&mut lines, line, width, theme, base),
            Block::Code {
                info, lines: code, ..
            } => push_code(&mut lines, info, &code, width, theme, &mut allowance),
            Block::Patch(patch) => push_patch(&mut lines, &patch, width, theme),
        }
    }
    if lines.is_empty() {
        lines.push(Line::default());
    }
    lines
}

fn push_prose(lines: &mut Vec<Line<'static>>, raw: &str, width: usize, theme: &Theme, base: Style) {
    let line = sanitize(raw);
    let trimmed = line.trim_end();
    if trimmed.is_empty() {
        lines.push(Line::default());
        return;
    }
    let body = trimmed.trim_start();
    if let Some(heading) = body.strip_prefix("### ").or(body.strip_prefix("## ")) {
        push_wrapped(lines, heading, width, theme, theme.bold(), "");
        return;
    }
    if let Some(heading) = body.strip_prefix("# ") {
        push_wrapped(lines, heading, width, theme, theme.accent_bold(), "");
        return;
    }
    if let Some(item) = body
        .strip_prefix("- ")
        .or(body.strip_prefix("* "))
        .or(body.strip_prefix("+ "))
    {
        let bullet = format!("{} ", theme.markers.bullet);
        push_wrapped(lines, item, width, theme, base, &bullet);
        return;
    }
    push_wrapped(lines, trimmed, width, theme, base, "");
}

/// What highlighting one message may still do. Both limits are checked as a block starts, so the
/// block that reaches one may overrun it, by at most [`highlight::BUDGET`].
#[derive(Default)]
struct Allowance {
    bytes: usize,
    spent: Duration,
    /// A limit was reached, and the rest of the message's code is plain.
    over: bool,
    /// A frame had no work left, and the rest of the message's code waits for a later frame.
    deferred: bool,
}

impl Allowance {
    /// Whether a block of `bytes` may be highlighted. When it may not, the sentence that says why,
    /// for the first such block only.
    fn admit(&mut self, bytes: usize) -> Result<(), Option<String>> {
        if self.over || self.deferred {
            return Err(None);
        }
        let limit = if self.bytes + bytes > MESSAGE_CODE_BYTES {
            format!("{} kB", MESSAGE_CODE_BYTES / 1024)
        } else if self.spent >= highlight::BUDGET {
            format!("{} ms", highlight::BUDGET.as_millis())
        } else {
            return Ok(());
        };
        self.over = true;
        Err(Some(format!(
            "Highlighting this message reached its {limit} limit, so this block and the code after it are plain text."
        )))
    }
}

/// A fenced block under the gutter.
fn push_code(
    lines: &mut Vec<Line<'static>>,
    info: &str,
    code: &[&str],
    width: usize,
    theme: &Theme,
    allowance: &mut Allowance,
) {
    if diff::declared(info) || (info.is_empty() && diff::scan(code).patch == Some(code.len())) {
        push_patch(lines, code, width, theme);
        return;
    }
    let (gutter, room) = code_gutter(width, theme);
    // The highlighter would take a second `\r` at the end of a line for part of its ending, and
    // it is content, so such a block stays plain.
    let language = if code.is_empty() || code.iter().any(|line| line.ends_with('\r')) {
        None
    } else {
        highlight::for_token(info)
    };
    let mut highlighted = None;
    let mut note = None;
    if let Some(language) = language {
        let mut source = code.join("\n");
        source.push('\n');
        match allowance.admit(source.len()) {
            Ok(()) => match highlight::highlight_cached(
                &source,
                language,
                theme,
                theme.raised,
                theme.code(),
            ) {
                Some(cached) => {
                    allowance.bytes += source.len();
                    allowance.spent += cached.cost;
                    highlighted = Some(cached.highlighted);
                }
                None => allowance.deferred = true,
            },
            Err(sentence) => note = sentence,
        }
    }
    match &highlighted {
        Some(result) => {
            for pieces in &result.lines {
                push_code_line(lines, pieces, &gutter, room, theme);
            }
            if let Some(stop) = &result.stopped {
                note = Some(stop.sentence());
            }
        }
        None => {
            for line in code {
                let pieces = [(theme.code(), (*line).to_owned())];
                push_code_line(lines, &pieces, &gutter, room, theme);
            }
        }
    }
    for piece in note.iter().flat_map(|note| wrap(note, width.max(1))) {
        lines.push(Line::from(Span::styled(piece, theme.faint())));
    }
}

/// The lines of a patch under the gutter, each in the style of its role and with its own marker.
/// An actual patch drawn as the transcript draws a declared one: every line styled by its role
/// and kept whole, under the code gutter. A carriage return stays content and is written out.
pub fn patch(text: &str, width: usize, theme: &Theme) -> Vec<Line<'static>> {
    let rows: Vec<&str> = text.split_terminator('\n').collect();
    let mut lines = Vec::new();
    push_patch(&mut lines, &rows, width, theme);
    lines
}

fn push_patch(lines: &mut Vec<Line<'static>>, patch: &[&str], width: usize, theme: &Theme) {
    let (gutter, room) = code_gutter(width, theme);
    for (line, role) in patch.iter().zip(diff::roles(patch)) {
        let pieces = [(role.style(theme), (*line).to_owned())];
        push_code_line(lines, &pieces, &gutter, room, theme);
    }
}

/// The gutter drawn before code, and the cells left for the code beside it.
fn code_gutter(width: usize, theme: &Theme) -> (String, usize) {
    let gutter = format!("{} ", theme.markers.gutter);
    let room = width.saturating_sub(self::width(&gutter) + 1).max(1);
    (gutter, room)
}

/// One line of code under the gutter, drawn with [`literal`] and wrapped without losing a
/// character: indentation, repeated and trailing spaces are content.
fn push_code_line(
    lines: &mut Vec<Line<'static>>,
    pieces: &[(Style, String)],
    gutter: &str,
    room: usize,
    theme: &Theme,
) {
    let escapes = theme
        .code()
        .fg(highlight::legible(theme, theme.warn, theme.raised));
    let (shown, _) = literal(pieces, escapes, usize::MAX);
    for row in wrap_styled(&shown, room) {
        let mut spans = vec![Span::styled(gutter.to_owned(), theme.faint())];
        if row.is_empty() {
            spans.push(Span::styled(String::new(), theme.code()));
        }
        spans.extend(row);
        lines.push(Line::from(spans));
    }
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

/// The end of `text`, at most `chars` characters of it.
fn last_chars(text: &str, chars: usize) -> &str {
    if chars == 0 {
        return "";
    }
    match text.char_indices().rev().nth(chars - 1) {
        Some((start, _)) => &text[start..],
        None => text,
    }
}

/// Text still arriving, as its preview draws it: the last `count` lines, each cut to its last
/// `cells` characters, with prose sanitised and wrapped, code drawn with [`literal`] under the
/// gutter and a patch by the role of each line. Nothing is highlighted while it arrives. Fence
/// lines are not drawn, as in [`markdown`], and a block the text has not closed yet is shown as
/// far as it has arrived, with nothing added to end it.
pub fn stream_preview(
    text: &str,
    count: usize,
    cells: usize,
    width: usize,
    theme: &Theme,
) -> Vec<Line<'static>> {
    // A line ending that has arrived starts no line yet.
    let text = text.strip_suffix('\n').unwrap_or(text);
    let mut shown: Vec<(Option<Style>, &str)> = Vec::new();
    for block in blocks(text) {
        match block {
            Block::Prose(line) => shown.push((None, line)),
            Block::Code { info, lines, .. } if diff::declared(info) => shown.extend(
                lines
                    .iter()
                    .zip(diff::roles(&lines))
                    .map(|(line, role)| (Some(role.style(theme)), *line)),
            ),
            Block::Code { lines, .. } => {
                shown.extend(lines.iter().map(|line| (Some(theme.code()), *line)))
            }
            Block::Patch(patch) => shown.extend(
                patch
                    .iter()
                    .zip(diff::roles(&patch))
                    .map(|(line, role)| (Some(role.style(theme)), *line)),
            ),
        }
    }
    let (gutter, room) = code_gutter(width, theme);
    let mut rows = Vec::new();
    for (style, line) in &shown[shown.len().saturating_sub(count)..] {
        let kept = last_chars(line, cells);
        let cut = kept.len() < line.len();
        match style {
            None => {
                let prose = sanitize(kept);
                let prose = if cut { format!("…{prose}") } else { prose };
                for piece in wrap(&prose, width.max(1)) {
                    rows.push(Line::from(Span::styled(piece, theme.faint())));
                }
            }
            Some(style) => {
                let mut pieces = Vec::new();
                if cut {
                    pieces.push((theme.faint(), "…".to_owned()));
                }
                pieces.push((*style, kept.to_owned()));
                push_code_line(&mut rows, &pieces, &gutter, room, theme);
            }
        }
    }
    rows
}

/// Pieces of a line of code as they are drawn: tabs expanded to the next stop of every [`TAB`]
/// columns, and control and bidirectional formatting characters written as visible escapes in the
/// `escapes` style, so no character reaches the terminal as an instruction. The text itself is
/// untouched. At most `limit` characters are kept; the count of the rest is returned with them.
pub fn literal(
    pieces: &[(Style, String)],
    escapes: Style,
    limit: usize,
) -> (Vec<(Style, String)>, usize) {
    let mut shown = Vec::new();
    let mut column = 0usize;
    let mut kept = 0usize;
    let mut hidden = 0usize;
    for (style, piece) in pieces {
        let mut run = String::new();
        for ch in piece.chars() {
            if kept == limit {
                hidden += 1;
                continue;
            }
            kept += 1;
            if ch == '\t' {
                let spaces = TAB - column % TAB;
                run.extend(std::iter::repeat_n(' ', spaces));
                column += spaces;
            } else if let Some(escape) = escape(ch) {
                if !run.is_empty() {
                    shown.push((*style, std::mem::take(&mut run)));
                }
                column += escape.len();
                shown.push((escapes, escape));
            } else {
                run.push(ch);
                column += char_width(ch);
            }
        }
        if !run.is_empty() {
            shown.push((*style, run));
        }
    }
    (shown, hidden)
}

/// How a control character is written in code: caret notation for the C0 controls and DEL, and
/// the code point for the rest and for bidirectional formatting characters.
pub fn escape(ch: char) -> Option<String> {
    match ch {
        '\u{7f}' => Some("^?".into()),
        ch if (ch as u32) < 0x20 => Some(format!("^{}", char::from(ch as u8 + 0x40))),
        ch if ch.is_control() || is_format_control(ch) => Some(format!("<U+{:04X}>", ch as u32)),
        _ => None,
    }
}

/// Characters that reorder the text around them when a terminal applies bidirectional layout.
pub fn is_format_control(ch: char) -> bool {
    matches!(
        ch,
        '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
    )
}
