//! Text measurement, wrapping and sanitising for the transcript.
//!
//! Wrapping is computed here rather than delegated to `Paragraph`: entry height must be known
//! before drawing, and `Paragraph::line_count` is behind an unstable feature in ratatui 0.29.
//! Width uses the same `unicode-width` version ratatui itself uses, so the layout computed here
//! matches the cells the backend fills.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Display width of a string in terminal cells.
pub fn width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

/// Strip anything that could move the cursor, repaint the screen, or open a hyperlink.
///
/// Every string that originates outside ymp — runtime output, failure reasons, and later
/// collaboration messages — passes through here. Untrusted payload is data, never instructions
/// to the terminal.
pub fn sanitize(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            // ESC introduces CSI/OSC/other sequences: drop the introducer and its payload.
            '\u{1b}' => {
                match chars.peek() {
                    // OSC runs until BEL or ST.
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
                    // CSI runs until a byte in 0x40..=0x7e.
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
                }
            }
            '\t' => out.push_str("    "),
            // Keep no other control characters; C1 is dropped with them.
            c if c.is_control() => {}
            c if ('\u{80}'..='\u{9f}').contains(&c) => {}
            c => out.push(c),
        }
    }
    out
}

/// Wrap `text` to `width` cells, breaking on word boundaries and splitting words that cannot
/// fit on a line of their own. Returns at least one line, possibly empty.
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }

    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        let mut line_width = 0usize;

        // Leading indentation is meaning (aligned "next" blocks, nested lists) — keep it.
        let indent = paragraph.len() - paragraph.trim_start_matches(' ').len();
        if indent > 0 && indent < width {
            line.push_str(&paragraph[..indent]);
            line_width = indent;
        }
        let paragraph = &paragraph[indent..];

        for word in paragraph.split_inclusive(' ') {
            let trimmed = word.trim_end_matches(' ');
            let spaces = word.len() - trimmed.len();
            let word_width = self::width(trimmed);

            if line_width > 0 && line_width + word_width > width {
                lines.push(std::mem::take(&mut line));
                line_width = 0;
            }

            if word_width > width {
                for piece in split_to_width(trimmed, width) {
                    let piece_width = self::width(&piece);
                    if line_width > 0 && line_width + piece_width > width {
                        lines.push(std::mem::take(&mut line));
                        line_width = 0;
                    }
                    line.push_str(&piece);
                    line_width += piece_width;
                }
            } else {
                line.push_str(trimmed);
                line_width += word_width;
            }

            if spaces > 0 && line_width > 0 && line_width < width {
                let room = (width - line_width).min(spaces);
                line.push_str(&" ".repeat(room));
                line_width += room;
            }
        }

        lines.push(std::mem::take(&mut line));
    }

    lines
        .into_iter()
        .map(|line| line.trim_end().to_owned())
        .collect()
}

/// Break one over-long token into chunks that each fit `width`, never splitting a grapheme.
fn split_to_width(word: &str, width: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut chunk = String::new();
    let mut chunk_width = 0usize;

    for grapheme in word.graphemes(true) {
        let grapheme_width = self::width(grapheme).max(1);
        if chunk_width + grapheme_width > width && !chunk.is_empty() {
            chunks.push(std::mem::take(&mut chunk));
            chunk_width = 0;
        }
        chunk.push_str(grapheme);
        chunk_width += grapheme_width;
    }

    if !chunk.is_empty() {
        chunks.push(chunk);
    }
    chunks
}

/// Truncate to `width` cells, replacing the tail with `…` when anything was dropped.
pub fn truncate(text: &str, width: usize) -> String {
    if self::width(text) <= width {
        return text.to_owned();
    }
    if width == 0 {
        return String::new();
    }

    let mut out = String::new();
    let mut used = 0usize;
    for grapheme in text.graphemes(true) {
        let grapheme_width = self::width(grapheme).max(1);
        if used + grapheme_width > width.saturating_sub(1) {
            break;
        }
        out.push_str(grapheme);
        used += grapheme_width;
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_never_exceeds_the_given_width() {
        let text = "concurrent same-key requests must serialize; second request observed 500 \
                    after lock timeout (expected 200 with first body)";
        for width in [20usize, 34, 58, 80] {
            for line in wrap(text, width) {
                assert!(
                    self::width(&line) <= width,
                    "line {line:?} exceeds width {width}"
                );
            }
        }
    }

    #[test]
    fn a_word_longer_than_the_line_is_split_rather_than_dropped() {
        let digest = "sha256:9c41f2ab55d0e7c3a1b2c3d4e5f60718293a4b5c6d7e8f90";
        let lines = wrap(digest, 16);
        assert!(lines.len() > 1);
        assert_eq!(lines.concat(), digest);
    }

    #[test]
    fn double_width_text_is_measured_in_cells() {
        let lines = wrap("残業 が 多い です ね", 8);
        for line in &lines {
            assert!(self::width(line) <= 8, "line {line:?} too wide");
        }
    }

    #[test]
    fn sanitize_removes_escape_sequences_and_keeps_the_text() {
        let hostile = "\u{1b}]8;;http://evil.example\u{7}click\u{1b}]8;;\u{7} \u{1b}[2Jplain";
        let clean = sanitize(hostile);
        assert!(!clean.contains('\u{1b}'));
        assert!(clean.contains("click"));
        assert!(clean.contains("plain"));
    }

    #[test]
    fn sanitize_drops_carriage_returns_and_bells() {
        let clean = sanitize("progress\r\u{7}done");
        assert_eq!(clean, "progressdone");
    }

    #[test]
    fn truncate_marks_that_something_was_dropped() {
        assert_eq!(truncate("abcdef", 10), "abcdef");
        assert_eq!(truncate("abcdef", 4), "abc…");
    }
}
