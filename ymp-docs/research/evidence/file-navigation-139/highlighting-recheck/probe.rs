use ratatui::style::{Color, Style};
use ratatui::text::Line;
use std::str::FromStr;
use std::time::Instant;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Color as SColor, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSettings};
use syntect::util::LinesWithEndings;
use tui_syntax_highlight::Highlighter;

fn color(r: u8, g: u8, b: u8) -> SColor { SColor { r, g, b, a: 255 } }

fn main() {
    let started = Instant::now();
    let syntaxes = two_face::syntax::extra_newlines();
    let source = "/* first\n second */\nfn café() {\n    let s = \"a  b\";\n\t// tab remains data\n}\n";
    let syntax = syntaxes.find_syntax_by_extension("rs").unwrap();
    let theme = Theme {
        settings: ThemeSettings { foreground: Some(color(210, 210, 210)), ..Default::default() },
        scopes: vec![
            ThemeItem { scope: ScopeSelectors::from_str("keyword, storage").unwrap(), style: StyleModifier { foreground: Some(color(200, 100, 0)), ..Default::default() } },
            ThemeItem { scope: ScopeSelectors::from_str("comment").unwrap(), style: StyleModifier { foreground: Some(color(100, 100, 100)), ..Default::default() } },
            ThemeItem { scope: ScopeSelectors::from_str("string").unwrap(), style: StyleModifier { foreground: Some(color(0, 160, 80)), ..Default::default() } },
        ],
        ..Default::default()
    };
    let adapter = Highlighter::new(theme.clone()).line_numbers(false).override_background(Color::Reset);
    let mut parser = HighlightLines::new(syntax, &theme);
    let rendered: Vec<Line<'static>> = LinesWithEndings::from(source)
        .enumerate()
        .map(|(i, line)| adapter.highlight_line(line, &mut parser, i, Style::default(), &syntaxes).unwrap())
        .collect();
    let text: Vec<String> = rendered.iter().map(|line| line.spans.iter().map(|s| s.content.as_ref()).collect()).collect();
    assert_eq!(text, source.lines().collect::<Vec<_>>());
    assert!(rendered[2].spans.iter().any(|s| s.content.contains("fn") && s.style.fg == Some(Color::Rgb(200, 100, 0))));
    assert!(rendered[1].spans.iter().any(|s| s.content.contains("second") && s.style.fg == Some(Color::Rgb(100, 100, 100))));
    assert!(rendered[3].spans.iter().any(|s| s.content.contains("a  b") && s.style.fg == Some(Color::Rgb(0, 160, 80))));
    for ext in ["rs", "py", "toml", "ts"] { assert!(syntaxes.find_syntax_by_extension(ext).is_some(), "{ext}"); }
    let mut crlf_parser = HighlightLines::new(syntax, &theme);
    let crlf = adapter.highlight_line("let n = 1;\r\n", &mut crlf_parser, 0, Style::default(), &syntaxes).unwrap();
    let crlf_text: String = crlf.spans.iter().map(|s| s.content.as_ref()).collect();
    println!("{}", serde_json::json!({
        "ratatui": "0.30.2", "adapter": "tui-syntax-highlight 0.2.0", "regex": "fancy", "native_inference": false,
        "checks": { "compiles_with_ratatui_line": true, "unicode_whitespace_and_tabs_preserved": true, "multiline_state_preserved": true, "semantic_colors_applied": true, "rust_python_toml_typescript_grammars": true },
        "lines": rendered.len(), "elapsed_ms": started.elapsed().as_millis(),
        "crlf_output": crlf_text, "crlf_requires_caller_normalization": crlf_text.ends_with('\r'),
        "remaining": "Application must impose per-line/time bounds and normalize display controls/line endings; full ymp integration is not tested here."
    }));
}
