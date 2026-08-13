//! Palette taken verbatim from `ymp-docs/design/ymp_chat_tui.dc.html`.
//!
//! Colour is always secondary: every state distinction the theme expresses in colour is also
//! carried by a word or a marker, so the interface survives a monochrome terminal.

use ratatui::style::{Color, Modifier, Style};

/// Screen background.
pub const BG: Color = Color::Rgb(0x14, 0x14, 0x14);
/// Panel background (input boxes, palette).
pub const PANEL: Color = Color::Rgb(0x26, 0x26, 0x26);
/// Hairlines and borders.
pub const RULE: Color = Color::Rgb(0x3a, 0x3a, 0x3a);
/// Bright foreground: identifiers, focused values.
pub const TEXT: Color = Color::Rgb(0xf2, 0xf2, 0xf2);
/// Ordinary body text.
pub const DIM: Color = Color::Rgb(0xd0, 0xd0, 0xd0);
/// Secondary text: labels, the control plane.
pub const MUTED: Color = Color::Rgb(0x8f, 0x8f, 0x8f);
/// Tertiary text: timestamps, hints.
pub const FAINT: Color = Color::Rgb(0x5f, 0x5f, 0x5f);
/// Bright amber accent: selection, page titles, decision frames, key hints.
pub const ACCENT: Color = Color::Rgb(0xd3, 0xa2, 0x4a);
/// Deep amber: the collaboration plane, the prompt marker, warnings.
pub const AMBER: Color = Color::Rgb(0xd3, 0x87, 0x2f);
/// Positive outcome; the verification plane.
pub const GREEN: Color = Color::Rgb(0x8f, 0xbf, 0x5f);
/// Failure that is a fact of the run, and interface errors.
pub const RED: Color = Color::Rgb(0xd1, 0x63, 0x5c);

pub fn text() -> Style {
    Style::default().fg(TEXT)
}

pub fn dim() -> Style {
    Style::default().fg(DIM)
}

pub fn muted() -> Style {
    Style::default().fg(MUTED)
}

pub fn faint() -> Style {
    Style::default().fg(FAINT)
}

pub fn accent() -> Style {
    Style::default().fg(ACCENT)
}

pub fn accent_bold() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn amber() -> Style {
    Style::default().fg(AMBER)
}

pub fn green() -> Style {
    Style::default().fg(GREEN)
}

pub fn red() -> Style {
    Style::default().fg(RED)
}

pub fn bold() -> Style {
    Style::default().fg(TEXT).add_modifier(Modifier::BOLD)
}

/// Selected row: amber background, dark bold text.
pub fn selected() -> Style {
    Style::default()
        .fg(BG)
        .bg(ACCENT)
        .add_modifier(Modifier::BOLD)
}

pub fn rule() -> Style {
    Style::default().fg(RULE)
}

/// Trust planes. The colour is redundant: the plane is always printed as a word.
pub const PLANE_CTRL: Color = MUTED;
pub const PLANE_COLLAB: Color = AMBER;
pub const PLANE_VERIF: Color = GREEN;

/// Markers carry meaning without colour. `ascii` mirrors each one for terminals that cannot
/// render the Unicode form.
#[derive(Clone, Copy, Debug)]
pub struct Markers {
    pub ok: &'static str,
    pub fail: &'static str,
    pub warn: &'static str,
    pub prompt: &'static str,
    pub cursor: &'static str,
    pub message_kind: &'static str,
    pub follow_live: &'static str,
    pub follow_paused: &'static str,
    pub more_below: &'static str,
    pub fix: &'static str,
    pub limiting: &'static str,
}

pub const UNICODE: Markers = Markers {
    ok: "✓",
    fail: "✗",
    warn: "▲",
    prompt: "›",
    cursor: "▁",
    message_kind: "▸",
    follow_live: "▸ live",
    follow_paused: "⏸ paused",
    more_below: "▼",
    fix: "↳",
    limiting: "◂ limiting",
};

pub const ASCII: Markers = Markers {
    ok: "+",
    fail: "x",
    warn: "!",
    prompt: ">",
    cursor: "_",
    message_kind: ">",
    follow_live: "> live",
    follow_paused: "|| paused",
    more_below: "v",
    fix: "->",
    limiting: "< limiting",
};

impl Markers {
    /// Unicode markers unless the environment says the terminal cannot show them.
    pub fn detect() -> Self {
        let ascii_only = ["LC_ALL", "LC_CTYPE", "LANG"]
            .iter()
            .filter_map(|key| std::env::var(key).ok())
            .any(|value| {
                let value = value.to_ascii_lowercase();
                value.contains("ascii") || value == "c" || value == "posix"
            });
        if ascii_only { ASCII } else { UNICODE }
    }
}
