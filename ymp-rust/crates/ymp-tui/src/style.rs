//! Token-level styling of body text, reproducing the artifact's colour discipline.
//!
//! The artifact styles the *words* of a fact, not whole lines: stable identifiers are bold,
//! positive state words are green, failure words are red, `/commands` are amber. This module
//! applies those rules to already-wrapped plain text so every fixture and every live journal
//! line gets the same treatment without hand-styling.

use ratatui::style::{Modifier, Style};
use ratatui::text::Span;

use crate::theme;

const GREEN_WORDS: &[&str] = &[
    "ready",
    "ok",
    "clean",
    "accepted",
    "passed",
    "authorized",
    "running",
    "owned",
    "yes",
    "durable",
    "working",
];

const RED_WORDS: &[&str] = &[
    "rejected",
    "failed",
    "blocked",
    "expired",
    "unusable",
    "error",
    "invalid",
    "BLOCKING",
    "infrastructure_error",
    "cancelled",
    "killed",
];

const AMBER_WORDS: &[&str] = &["exhausted", "abstained", "stale", "limiting", "draft"];

/// True for stable object identifiers like `rn-2209`, `tc-07`, `ev-0041`, `at-12`.
/// Profile names like `rp-claude-sonnet` stay ordinary text, matching the artifact.
fn is_object_id(token: &str) -> bool {
    let Some((prefix, tail)) = token.split_once('-') else {
        return false;
    };
    prefix.len() >= 2
        && prefix.len() <= 3
        && prefix.chars().all(|c| c.is_ascii_lowercase())
        && !tail.is_empty()
        && tail.chars().all(|c| c.is_ascii_digit())
}

fn classify(core: &str, base: Style) -> Style {
    if core.is_empty() {
        return base;
    }
    if is_object_id(core) {
        return base.add_modifier(Modifier::BOLD).fg(theme::TEXT);
    }
    if GREEN_WORDS.contains(&core) {
        return base.fg(theme::GREEN);
    }
    if RED_WORDS.contains(&core) {
        return base.fg(theme::RED);
    }
    if AMBER_WORDS.contains(&core) {
        return base.fg(theme::AMBER);
    }
    if core.starts_with(':') && core.len() > 1 {
        return base.fg(theme::ACCENT);
    }
    match core {
        "✓" => base.fg(theme::GREEN),
        "✗" => base.fg(theme::RED),
        "▲" | "⚠" => base.fg(theme::AMBER),
        _ => base,
    }
}

/// Style one line of already-wrapped text. Returns spans covering the line exactly.
pub fn spans(line: &str, base: Style) -> Vec<Span<'static>> {
    let mut out: Vec<Span<'static>> = Vec::new();
    let mut plain = String::new();

    let flush = |plain: &mut String, out: &mut Vec<Span<'static>>| {
        if !plain.is_empty() {
            out.push(Span::styled(std::mem::take(plain), base));
        }
    };

    for chunk in line.split_inclusive(' ') {
        let token = chunk.trim_end_matches(' ');
        let spaces = &chunk[token.len()..];

        // Classification looks at the token without surrounding punctuation.
        let core = token.trim_matches(|c: char| ",.;:()[]".contains(c) && c != ':');
        let core = if core.starts_with(':') && core.len() > 1 {
            core.trim_end_matches(|c: char| ",.;".contains(c))
        } else {
            core.trim_matches(|c: char| ",.;:()[]".contains(c))
        };

        let style = classify(core, base);
        if style == base {
            plain.push_str(chunk);
        } else {
            flush(&mut plain, &mut out);
            out.push(Span::styled(token.to_owned(), style));
            if !spaces.is_empty() {
                plain.push_str(spaces);
            }
        }
    }
    flush(&mut plain, &mut out);

    if out.is_empty() {
        out.push(Span::styled(String::new(), base));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(line: &str) -> Vec<(String, Style)> {
        spans(line, theme::dim())
            .into_iter()
            .map(|span| (span.content.into_owned(), span.style))
            .collect()
    }

    #[test]
    fn object_ids_become_bold() {
        let identifier = format!("seq-{:04}", 41);
        let parts = rendered(&format!("run demo-run started · {identifier}"));
        assert!(parts.iter().any(
            |(text, style)| *text == identifier && style.add_modifier.contains(Modifier::BOLD)
        ));
    }

    #[test]
    fn state_words_take_their_colour() {
        let parts = rendered("candidate rejected · candidate accepted · route ready");
        let colour_of = |needle: &str| {
            parts
                .iter()
                .find(|(text, _)| text == needle)
                .map(|(_, style)| style.fg)
                .unwrap()
        };
        assert_eq!(colour_of("rejected"), Some(theme::RED));
        assert_eq!(colour_of("accepted"), Some(theme::GREEN));
        assert_eq!(colour_of("ready"), Some(theme::GREEN));
    }

    #[test]
    fn runtime_profile_names_stay_plain() {
        let parts = rendered("route claude-code");
        assert!(
            parts
                .iter()
                .all(|(text, style)| !text.contains("claude-code")
                    || !style.add_modifier.contains(Modifier::BOLD))
        );
    }

    #[test]
    fn the_spans_reassemble_to_the_original_line() {
        let line = "verification started · candidate accepted · every enforced check passed";
        let joined: String = rendered(line).into_iter().map(|(text, _)| text).collect();
        assert_eq!(joined, line);
    }
}
