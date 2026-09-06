use std::{fmt::Write, path::Path};

use anyhow::Result;
use ratatui::{
    Terminal,
    backend::TestBackend,
    style::{Color, Modifier},
};
use unicode_width::UnicodeWidthStr;

use crate::{app::App, ui};

/// Export the exact cell buffer produced by the interactive renderer, not a second mockup.
pub fn save(app: &mut App, path: &Path, width: u16, height: u16) -> Result<()> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| ui::draw(frame, app))?;
    let buffer = terminal.backend().buffer();
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\"><rect width=\"100%\" height=\"100%\" fill=\"{}\"/>",
        width * 10,
        height * 20,
        width * 10,
        height * 20,
        color(ui::BASE)
    );
    let mut plain = String::new();
    for y in 0..height {
        for x in 0..width {
            let cell = &buffer[(x, y)];
            if cell.bg != Color::Reset && cell.bg != ui::BASE {
                write!(
                    svg,
                    "<rect x=\"{}\" y=\"{}\" width=\"10\" height=\"20\" fill=\"{}\"/>",
                    x * 10,
                    y * 20,
                    color(cell.bg)
                )?;
            }
        }
    }
    svg.push_str("<g font-family=\"Menlo,monospace\" font-size=\"16\" xml:space=\"preserve\">");
    for y in 0..height {
        for x in 0..width {
            let cell = &buffer[(x, y)];
            plain.push_str(cell.symbol());
            if cell.symbol().trim().is_empty() {
                continue;
            }
            write!(
                svg,
                "<text x=\"{}\" y=\"{}\" fill=\"{}\" font-weight=\"{}\" textLength=\"{}\" lengthAdjust=\"spacingAndGlyphs\">{}</text>",
                x * 10,
                y * 20 + 15,
                color(cell.fg),
                if cell.modifier.contains(Modifier::BOLD) {
                    "bold"
                } else {
                    "normal"
                },
                UnicodeWidthStr::width(cell.symbol()).max(1) * 10,
                escape(cell.symbol())
            )?;
        }
        plain.push('\n');
    }
    svg.push_str("</g></svg>\n");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, svg)?;
    std::fs::write(path.with_extension("txt"), plain)?;
    Ok(())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn color(value: Color) -> String {
    match value {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Black => "#000000".into(),
        _ => "#e0e9f1".into(),
    }
}
