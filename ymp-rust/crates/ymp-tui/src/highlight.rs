//! Syntax highlighting in the colours of the active theme.
//!
//! The grammars are the Sublime Text syntax definitions the `bat` project collects, bundled by
//! `two-face` and run by `syntect` on its pure-Rust `fancy-regex` engine. A few definitions need
//! features that engine lacks, and `two-face` leaves them out of this bundle: ARM Assembly,
//! JavaScript (Babel), LiveScript, PowerShell, Sass and Salt State SLS. Such files are shown as
//! plain text, like any file whose language is not recognised. `tui-syntax-highlight` turns what
//! `syntect` reports for each line into Ratatui styles.
//!
//! No TextMate colour scheme is used. Each kind of scope takes one of the semantic roles every
//! ymp theme defines, so source follows the chosen palette, light or dark, and a role colour that
//! would not read on the surface it is painted on gives way to the theme's own text colour.
//!
//! The work is bounded. Lines are highlighted one at a time, and after a line longer than
//! [`LINE_BYTES`], or once [`BUDGET`] is spent, the remaining lines are returned as plain text
//! with the reason, so a long or hostile file costs at most one bounded line beyond the budget.

use crate::theme::{contrast, Theme};
use ratatui::style::{Color, Style};
use std::path::Path;
use std::str::FromStr;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use syntect::easy::HighlightLines;
use syntect::highlighting::{
    Color as SyntaxColor, FontStyle, ScopeSelectors, StyleModifier, Theme as SyntaxTheme,
    ThemeItem, ThemeSettings,
};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;
use tui_syntax_highlight::Highlighter as Adapter;

/// Loaded the first time something is highlighted, never while the window starts.
static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(two_face::syntax::extra_newlines);

/// Where the licences and notices of the bundled syntax definitions are listed, for exactly the
/// version that is built in.
pub const NOTICES: &str = two_face::acknowledgement::url();

/// Lines longer than this are not highlighted, and neither is anything after them.
pub const LINE_BYTES: usize = 4 * 1024;

/// Time highlighting may take for one text before the rest is left plain.
pub const BUDGET: Duration = Duration::from_millis(250);

/// The contrast a highlighted colour keeps with its background, the floor body text keeps.
const MIN_CONTRAST: f64 = 3.0;

/// The most of a first line that is matched against the grammars' first-line patterns.
const FIRST_LINE_BYTES: usize = 256;

/// A language the highlighter has a grammar for.
#[derive(Clone, Copy, Debug)]
pub struct Language(&'static SyntaxReference);

impl Language {
    pub fn name(self) -> &'static str {
        &self.0.name
    }
}

/// The language of a file, by its whole name, then its extension, then its first line.
pub fn for_file(name: &str, first_line: &str) -> Option<Language> {
    let syntaxes = &*SYNTAXES;
    let extension = Path::new(name)
        .extension()
        .and_then(|extension| extension.to_str());
    let mut first = first_line;
    if first.len() > FIRST_LINE_BYTES {
        let mut end = FIRST_LINE_BYTES;
        while !first.is_char_boundary(end) {
            end -= 1;
        }
        first = &first[..end];
    }
    syntaxes
        .find_syntax_by_extension(name)
        .or_else(|| extension.and_then(|extension| syntaxes.find_syntax_by_extension(extension)))
        .or_else(|| {
            extension.and_then(|extension| {
                syntaxes.find_syntax_by_extension(&extension.to_ascii_lowercase())
            })
        })
        .or_else(|| syntaxes.find_syntax_by_first_line(first))
        .filter(|syntax| !is_plain(syntax))
        .map(Language)
}

/// The language a fenced code block names, such as `rust`, `py` or `toml`.
pub fn for_token(info: &str) -> Option<Language> {
    let token = info
        .split(|ch: char| ch.is_whitespace() || ch == ',' || ch == '{')
        .next()
        .unwrap_or_default();
    if token.is_empty() {
        return None;
    }
    SYNTAXES
        .find_syntax_by_token(token)
        .filter(|syntax| !is_plain(syntax))
        .map(Language)
}

fn is_plain(syntax: &SyntaxReference) -> bool {
    syntax.name == "Plain Text"
}

/// Why highlighting stopped before the end of the text. Lines are counted from one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stop {
    LongLine { line: usize },
    Budget { line: usize },
    Failed { line: usize, reason: String },
}

impl Stop {
    pub fn sentence(&self) -> String {
        match self {
            Stop::LongLine { line } => format!(
                "Line {line} is longer than {} kB, so highlighting stopped there and the rest is plain text.",
                LINE_BYTES / 1024
            ),
            Stop::Budget { line } => format!(
                "Highlighting stopped at line {line} after {} ms, and the rest is plain text.",
                BUDGET.as_millis()
            ),
            Stop::Failed { line, reason } => format!(
                "The grammar failed at line {line} ({reason}), and the rest is plain text."
            ),
        }
    }
}

/// Text as styled pieces, one list per line, without line endings.
#[derive(Clone, Debug, Default)]
pub struct Highlighted {
    pub lines: Vec<Vec<(Style, String)>>,
    pub stopped: Option<Stop>,
}

/// Every line of `text` in the `base` style.
pub fn plain(text: &str, base: Style) -> Highlighted {
    Highlighted {
        lines: LinesWithEndings::from(text)
            .map(|line| vec![(base, content(line).to_owned())])
            .collect(),
        stopped: None,
    }
}

/// Highlight `text` as `language`, for painting over `background`, within `budget`.
pub fn highlight(
    text: &str,
    language: Language,
    theme: &Theme,
    background: Color,
    base: Style,
    budget: Duration,
) -> Highlighted {
    let Some(palette) = syntax_theme(theme, background) else {
        return plain(text, base);
    };
    // The page draws its own line numbers.
    let adapter = Adapter::new(palette.clone()).line_numbers(false);
    let mut state = HighlightLines::new(language.0, &palette);
    let started = Instant::now();
    let mut highlighted = Highlighted::default();
    for (index, line) in LinesWithEndings::from(text).enumerate() {
        let content = content(line);
        if highlighted.stopped.is_none() {
            if line.len() > LINE_BYTES {
                highlighted.stopped = Some(Stop::LongLine { line: index + 1 });
            } else if index > 0 && started.elapsed() > budget {
                // Nothing is spent before the first line, so it is always highlighted.
                highlighted.stopped = Some(Stop::Budget { line: index + 1 });
            }
        }
        if highlighted.stopped.is_some() {
            highlighted.lines.push(vec![(base, content.to_owned())]);
            continue;
        }
        // The adapter ends the line with the `\n` the grammars expect and removes it again. It
        // would keep a `\r`, so the line is given to it without its ending.
        match adapter.highlight_line(content, &mut state, index, Style::new(), &SYNTAXES) {
            Ok(line) => highlighted.lines.push(
                line.spans
                    .into_iter()
                    .filter(|span| !span.content.is_empty())
                    .map(|span| {
                        // The text takes the theme's colours; the surface keeps its background.
                        let style = Style {
                            bg: None,
                            ..span.style
                        };
                        (base.patch(style), span.content.into_owned())
                    })
                    .collect(),
            ),
            Err(error) => {
                highlighted.stopped = Some(Stop::Failed {
                    line: index + 1,
                    reason: error.to_string(),
                });
                highlighted.lines.push(vec![(base, content.to_owned())]);
            }
        }
    }
    highlighted
}

/// A line without its `\n` or `\r\n` ending.
fn content(line: &str) -> &str {
    match line.strip_suffix('\n') {
        Some(line) => line.strip_suffix('\r').unwrap_or(line),
        None => line,
    }
}

/// The semantic role of each kind of scope. Where several selectors match a scope, the most
/// specific one applies, as in any TextMate theme.
pub fn roles(theme: &Theme) -> [(&'static str, Color, FontStyle); 12] {
    let none = FontStyle::empty();
    [
        (
            "comment, punctuation.definition.comment",
            theme.muted,
            FontStyle::ITALIC,
        ),
        (
            "string, punctuation.definition.string, constant.character.escape",
            theme.good,
            none,
        ),
        (
            "constant.numeric, constant.language, constant.character, constant.other, support.constant",
            theme.warn,
            none,
        ),
        ("keyword, storage", theme.accent, none),
        (
            "entity.name.type, entity.name.class, entity.name.struct, entity.name.enum, \
             entity.name.trait, entity.name.namespace, support.type, support.class, \
             entity.other.attribute-name",
            theme.info,
            none,
        ),
        (
            "entity.name.function, support.function, variable.function",
            theme.text,
            none,
        ),
        ("entity.name.tag", theme.accent, none),
        (
            "markup.heading, entity.name.section",
            theme.accent,
            FontStyle::BOLD,
        ),
        ("markup.inserted", theme.good, none),
        ("markup.deleted, invalid", theme.bad, none),
        ("markup.changed", theme.warn, none),
        ("markup.raw", theme.text, none),
    ]
}

/// `color` where it reads on `background`; otherwise whichever of the theme's body and text
/// colours reads better there.
pub fn legible(theme: &Theme, color: Color, background: Color) -> Color {
    if contrast(color, background) >= MIN_CONTRAST {
        return color;
    }
    if contrast(theme.body, background) >= contrast(theme.text, background) {
        theme.body
    } else {
        theme.text
    }
}

/// The roles as a syntect theme, or nothing when the palette is not given in RGB.
fn syntax_theme(theme: &Theme, background: Color) -> Option<SyntaxTheme> {
    let rgb = |color: Color| match legible(theme, color, background) {
        Color::Rgb(r, g, b) => Some(SyntaxColor { r, g, b, a: 0xFF }),
        _ => None,
    };
    let mut scopes = Vec::new();
    for (selectors, color, font_style) in roles(theme) {
        let Ok(scope) = ScopeSelectors::from_str(selectors) else {
            continue;
        };
        scopes.push(ThemeItem {
            scope,
            style: StyleModifier {
                foreground: Some(rgb(color)?),
                background: None,
                font_style: Some(font_style),
            },
        });
    }
    Some(SyntaxTheme {
        name: Some(theme.id.to_owned()),
        author: None,
        settings: ThemeSettings {
            foreground: Some(rgb(theme.body)?),
            ..ThemeSettings::default()
        },
        scopes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::catalog;

    #[test]
    fn a_language_is_found_by_name_extension_first_line_or_token() {
        assert_eq!(for_file("main.rs", "").unwrap().name(), "Rust");
        assert_eq!(for_file("Cargo.toml", "").unwrap().name(), "TOML");
        assert_eq!(for_file("Makefile", "").unwrap().name(), "Makefile");
        assert_eq!(for_file("SHOUT.PY", "").unwrap().name(), "Python");
        assert_eq!(
            for_file("run", "#!/usr/bin/env python3").unwrap().name(),
            "Python"
        );
        assert!(for_file("notes.unknown-extension", "hello").is_none());
        assert!(
            for_file("readme.txt", "").is_none(),
            "plain text is no language"
        );
        assert_eq!(for_token("rust").unwrap().name(), "Rust");
        assert_eq!(for_token("py title=\"x\"").unwrap().name(), "Python");
        assert!(for_token("").is_none());
        assert!(for_token("no-such-language").is_none());
        // Excluded from the fancy-regex bundle, so shown as plain text.
        assert!(for_file("script.ps1", "").is_none());
    }

    #[test]
    fn highlighting_keeps_the_text_and_colours_it_by_role() {
        let theme = crate::theme::theme("ember");
        let source = "// note\nfn main() {\n    let s = \"a  b\";\n}\n";
        let language = for_file("main.rs", "").unwrap();
        let result = highlight(
            source,
            language,
            theme,
            theme.surface,
            theme.body(),
            Duration::from_secs(10),
        );
        assert_eq!(result.stopped, None);
        let text: Vec<String> = result
            .lines
            .iter()
            .map(|pieces| pieces.iter().map(|(_, piece)| piece.as_str()).collect())
            .collect();
        assert_eq!(
            text,
            ["// note", "fn main() {", "    let s = \"a  b\";", "}"]
        );
        let colour_of = |line: usize, needle: &str| {
            result.lines[line]
                .iter()
                .find(|(_, piece)| piece.contains(needle))
                .and_then(|(style, _)| style.fg)
        };
        assert_eq!(colour_of(0, "note"), Some(theme.muted));
        assert_eq!(colour_of(2, "a  b"), Some(theme.good));
        assert_eq!(colour_of(1, "fn"), Some(theme.accent));
        assert_ne!(colour_of(1, "main"), colour_of(1, "fn"));
    }

    #[test]
    fn a_long_line_or_a_spent_budget_leaves_the_rest_plain() {
        let theme = crate::theme::theme("slate");
        let language = for_file("x.rs", "").unwrap();
        let long = format!("let a = 1;\n{}\nlet b = 2;\n", "x".repeat(LINE_BYTES + 1));
        let result = highlight(
            &long,
            language,
            theme,
            theme.surface,
            theme.body(),
            Duration::from_secs(10),
        );
        assert_eq!(result.stopped, Some(Stop::LongLine { line: 2 }));
        assert_eq!(result.lines.len(), 3);
        assert_eq!(
            result.lines[2],
            vec![(theme.body(), "let b = 2;".to_owned())]
        );

        let many = "let value = 1;\n".repeat(50);
        let result = highlight(
            &many,
            language,
            theme,
            theme.surface,
            theme.body(),
            Duration::ZERO,
        );
        assert!(
            matches!(result.stopped, Some(Stop::Budget { line: 2 })),
            "{:?}",
            result.stopped
        );
        assert_eq!(result.lines.len(), 50);
    }

    #[test]
    fn every_role_reads_on_panels_and_code_in_every_theme() {
        let mut unreadable = Vec::new();
        for theme in catalog() {
            for background in [theme.surface, theme.raised, theme.bg] {
                for (selectors, color, _) in roles(theme) {
                    let shown = legible(theme, color, background);
                    let ratio = contrast(shown, background);
                    if ratio < MIN_CONTRAST {
                        unreadable.push(format!("{} {selectors}: {ratio:.2}", theme.id));
                    }
                }
            }
            assert!(syntax_theme(theme, theme.surface).is_some(), "{}", theme.id);
        }
        assert!(unreadable.is_empty(), "{}", unreadable.join("\n"));
    }

    #[test]
    fn the_notice_names_the_bundled_version() {
        assert!(NOTICES.starts_with("https://codeberg.org/CosmicHarper/two-face/src/tag/v0.5.2"));
    }
}
