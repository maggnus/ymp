//! Theme tokens and the built-in palettes.
//!
//! Nothing in the interface names a colour directly. Every surface, label, state and
//! selection asks the active [`Theme`] for a semantic token, so adding a palette is a data
//! change rather than a rendering change. Colour is always redundant: each distinction the
//! palette expresses is also carried by a word or a marker, so the interface stays readable
//! on a monochrome terminal and for readers who cannot separate the hues.

use ratatui::style::{Color, Modifier, Style};

/// How a palette is meant to be used, shown in the chooser so the choice is informed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeKind {
    Dark,
    Light,
    HighContrast,
    TerminalPalette,
}

impl ThemeKind {
    pub fn label(self) -> &'static str {
        match self {
            ThemeKind::Dark => "dark",
            ThemeKind::Light => "light",
            ThemeKind::HighContrast => "high contrast",
            ThemeKind::TerminalPalette => "terminal palette",
        }
    }
}

/// The complete token set. Views may only read these fields.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub id: &'static str,
    pub name: &'static str,
    pub summary: &'static str,
    pub kind: ThemeKind,
    /// Screen background.
    pub bg: Color,
    /// Panel background: sidebar, composer, list rows.
    pub surface: Color,
    /// Raised background: code, badges, inline emphasis.
    pub raised: Color,
    /// Hairlines and borders.
    pub rule: Color,
    /// Bright foreground: headings, identifiers, focused values.
    pub text: Color,
    /// Ordinary body text.
    pub body: Color,
    /// Secondary text: labels and section titles.
    pub muted: Color,
    /// Tertiary text: timestamps and hints.
    pub faint: Color,
    /// Selection, prompts, page titles.
    pub accent: Color,
    /// Text drawn on top of `accent`.
    pub accent_ink: Color,
    /// Informational state.
    pub info: Color,
    /// Positive outcome.
    pub good: Color,
    /// Attention without failure.
    pub warn: Color,
    /// Failure, and interface errors.
    pub bad: Color,
    /// Border of the region that owns the keyboard.
    pub focus: Color,
    /// Markers used alongside colour so state never depends on hue alone.
    pub markers: Markers,
}

impl Theme {
    pub fn base(&self) -> Style {
        Style::default().fg(self.body).bg(self.bg)
    }
    pub fn surface(&self) -> Style {
        Style::default().fg(self.body).bg(self.surface)
    }
    pub fn text(&self) -> Style {
        Style::default().fg(self.text)
    }
    pub fn body(&self) -> Style {
        Style::default().fg(self.body)
    }
    pub fn muted(&self) -> Style {
        Style::default().fg(self.muted)
    }
    pub fn faint(&self) -> Style {
        Style::default().fg(self.faint)
    }
    pub fn accent(&self) -> Style {
        Style::default().fg(self.accent)
    }
    pub fn info(&self) -> Style {
        Style::default().fg(self.info)
    }
    pub fn good(&self) -> Style {
        Style::default().fg(self.good)
    }
    pub fn warn(&self) -> Style {
        Style::default().fg(self.warn)
    }
    pub fn bad(&self) -> Style {
        Style::default().fg(self.bad)
    }
    pub fn rule(&self) -> Style {
        Style::default().fg(self.rule)
    }
    pub fn bold(&self) -> Style {
        self.text().add_modifier(Modifier::BOLD)
    }
    pub fn accent_bold(&self) -> Style {
        self.accent().add_modifier(Modifier::BOLD)
    }
    /// A selected row. The caller also prefixes the selection marker.
    pub fn selected(&self) -> Style {
        Style::default()
            .fg(self.accent_ink)
            .bg(self.accent)
            .add_modifier(Modifier::BOLD)
    }
    /// Inline or fenced code.
    pub fn code(&self) -> Style {
        Style::default().fg(self.text).bg(self.raised)
    }
    /// A border, brighter when the region owns the keyboard.
    pub fn border(&self, focused: bool) -> Style {
        if focused {
            Style::default().fg(self.focus).add_modifier(Modifier::BOLD)
        } else {
            self.rule()
        }
    }
}

/// Markers carry meaning without colour. The ASCII set mirrors every Unicode marker for
/// terminals that cannot render the richer glyphs.
#[derive(Clone, Copy, Debug)]
pub struct Markers {
    pub selection: &'static str,
    pub user: &'static str,
    pub answer: &'static str,
    pub activity: &'static str,
    pub notice: &'static str,
    pub ok: &'static str,
    pub fail: &'static str,
    pub warn: &'static str,
    pub busy: &'static str,
    pub idle: &'static str,
    pub prompt: &'static str,
    pub bullet: &'static str,
    pub gutter: &'static str,
    pub hline: &'static str,
    pub vline: &'static str,
    pub more: &'static str,
    pub paused: &'static str,
    pub spinner: [&'static str; 4],
}

pub const UNICODE: Markers = Markers {
    selection: "›",
    user: "▌",
    answer: "◆",
    activity: "·",
    notice: "◇",
    ok: "✓",
    fail: "✗",
    warn: "▲",
    busy: "●",
    idle: "○",
    prompt: "›",
    bullet: "•",
    gutter: "│",
    hline: "─",
    vline: "│",
    more: "▼",
    paused: "⏸",
    spinner: ["⠋", "⠙", "⠹", "⠸"],
};

pub const ASCII: Markers = Markers {
    selection: ">",
    user: "|",
    answer: "*",
    activity: ".",
    notice: "-",
    ok: "+",
    fail: "x",
    warn: "!",
    busy: "*",
    idle: "o",
    prompt: ">",
    bullet: "-",
    gutter: "|",
    hline: "-",
    vline: "|",
    more: "v",
    paused: "||",
    spinner: ["|", "/", "-", "\\"],
};

/// Unicode markers unless the environment says the terminal cannot show them.
pub fn detect_markers() -> Markers {
    let ascii_only = ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .filter_map(|key| std::env::var(key).ok())
        .any(|value| {
            let value = value.to_ascii_lowercase();
            value.contains("ascii") || value == "c" || value == "posix"
        });
    if ascii_only {
        ASCII
    } else {
        UNICODE
    }
}

const fn rgb(value: u32) -> Color {
    Color::Rgb(
        ((value >> 16) & 0xff) as u8,
        ((value >> 8) & 0xff) as u8,
        (value & 0xff) as u8,
    )
}

/// Warm amber on black: the default ymp palette.
pub const EMBER: Theme = Theme {
    id: "ember",
    name: "Ember",
    summary: "Warm amber on black. The default ymp palette.",
    kind: ThemeKind::Dark,
    bg: rgb(0x000000),
    surface: rgb(0x000000),
    raised: rgb(0x000000),
    rule: rgb(0x3a352f),
    text: rgb(0xf2eee6),
    body: rgb(0xd8d2c6),
    muted: rgb(0x968f85),
    faint: rgb(0x6b655c),
    accent: rgb(0xd8a34c),
    accent_ink: rgb(0x17130c),
    info: rgb(0x8ca9bc),
    good: rgb(0x92b889),
    warn: rgb(0xdd8c3a),
    bad: rgb(0xd58673),
    focus: rgb(0xd8a34c),
    markers: UNICODE,
};

/// Cool blue on deep slate, for readers who find warm palettes tiring.
pub const SLATE: Theme = Theme {
    id: "slate",
    name: "Slate",
    summary: "Cool blue on deep slate. Calm, low-saturation dark theme.",
    kind: ThemeKind::Dark,
    bg: rgb(0x0f1319),
    surface: rgb(0x161b23),
    raised: rgb(0x1f2732),
    rule: rgb(0x303a47),
    text: rgb(0xe6edf3),
    body: rgb(0xc4d0dc),
    muted: rgb(0x8b98a6),
    faint: rgb(0x5c6873),
    accent: rgb(0x5cb3e6),
    accent_ink: rgb(0x07121a),
    info: rgb(0x7aa2f7),
    good: rgb(0x7bc99a),
    warn: rgb(0xe0af68),
    bad: rgb(0xe06c75),
    focus: rgb(0x5cb3e6),
    markers: UNICODE,
};

/// Dark ink on warm paper, for bright rooms and light terminal profiles.
pub const PAPER: Theme = Theme {
    id: "paper",
    name: "Paper",
    summary: "Dark ink on warm paper. For bright rooms and light terminals.",
    kind: ThemeKind::Light,
    bg: rgb(0xf4f1ea),
    surface: rgb(0xe9e4d9),
    raised: rgb(0xdcd5c6),
    rule: rgb(0xbdb5a5),
    text: rgb(0x201e1a),
    body: rgb(0x3b3730),
    muted: rgb(0x6b6558),
    faint: rgb(0x8a8375),
    accent: rgb(0x8a4f14),
    accent_ink: rgb(0xfdfbf6),
    info: rgb(0x265f80),
    good: rgb(0x2f6b36),
    warn: rgb(0x8a5a12),
    bad: rgb(0xa3342a),
    focus: rgb(0x8a4f14),
    markers: UNICODE,
};

/// Maximum separation between foreground and background.
pub const CONTRAST: Theme = Theme {
    id: "contrast",
    name: "Contrast",
    summary: "Maximum separation on pure black. Built for low-vision reading.",
    kind: ThemeKind::HighContrast,
    bg: rgb(0x000000),
    surface: rgb(0x0d0d0d),
    raised: rgb(0x1c1c1c),
    rule: rgb(0x767676),
    text: rgb(0xffffff),
    body: rgb(0xf2f2f2),
    muted: rgb(0xcccccc),
    faint: rgb(0xa0a0a0),
    accent: rgb(0xffd400),
    accent_ink: rgb(0x000000),
    info: rgb(0x63d9ff),
    good: rgb(0x5cff8f),
    warn: rgb(0xffb000),
    bad: rgb(0xff7a7a),
    focus: rgb(0xffd400),
    markers: UNICODE,
};

/// The sixteen colours the terminal profile already defines, so ymp inherits it.
pub const TERMINAL: Theme = Theme {
    id: "terminal",
    name: "Terminal",
    summary: "Inherits the sixteen colours your terminal profile already defines.",
    kind: ThemeKind::TerminalPalette,
    bg: Color::Reset,
    surface: Color::Reset,
    raised: Color::Black,
    rule: Color::DarkGray,
    text: Color::White,
    body: Color::Gray,
    muted: Color::DarkGray,
    faint: Color::DarkGray,
    accent: Color::Yellow,
    accent_ink: Color::Black,
    info: Color::Cyan,
    good: Color::Green,
    warn: Color::LightYellow,
    bad: Color::Red,
    focus: Color::Cyan,
    markers: UNICODE,
};

/// Every palette the chooser offers, in presentation order.
pub const THEMES: &[Theme] = &[EMBER, SLATE, PAPER, CONTRAST, TERMINAL];

/// The palette used when no preference has been stored.
pub const DEFAULT_THEME: &str = EMBER.id;

/// Look up a palette by identifier, falling back to the default.
pub fn theme(id: &str) -> &'static Theme {
    THEMES.iter().find(|t| t.id == id).unwrap_or(&THEMES[0])
}

/// The index of a palette in [`THEMES`].
pub fn index_of(id: &str) -> usize {
    THEMES.iter().position(|t| t.id == id).unwrap_or(0)
}

/// A palette with the markers the terminal can actually render.
pub fn resolved(id: &str, markers: Markers) -> Theme {
    let mut theme = *theme(id);
    theme.markers = markers;
    theme
}
