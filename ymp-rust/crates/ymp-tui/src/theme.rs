//! Theme tokens and the built-in palettes.
//!
//! Nothing in the interface names a colour directly. Every surface, label, state and
//! selection asks the active [`Theme`] for a semantic token, so adding a palette is a data
//! change rather than a rendering change. Colour is always redundant: each distinction the
//! palette expresses is also carried by a word or a marker, so the interface stays readable
//! on a monochrome terminal and for readers who cannot separate the hues.
//!
//! Ember and Slate are ymp's own palettes. The other sixteen come from the `ratatui-themes`
//! crate. Its palettes name fewer roles than a ymp theme has, so [`from_library`] fills the
//! roles a palette names with its own colours and derives the rest from them.

use ratatui::style::{Color, Modifier, Style};
use ratatui_themes::ThemeName;
use std::sync::LazyLock;

/// Whether a palette is meant for a dark or a light terminal, shown in the chooser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeKind {
    Dark,
    Light,
}

impl ThemeKind {
    pub fn label(self) -> &'static str {
        match self {
            ThemeKind::Dark => "dark",
            ThemeKind::Light => "light",
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
    /// A value nobody reported. It stands for an unknown count, never for a zero.
    pub unknown: &'static str,
    /// A figure that can still grow, because something it counts is not finished.
    pub growing: &'static str,
    /// Beside the title of a column sorted in ascending order.
    pub ascending: &'static str,
    /// Beside the title of a column sorted in descending order.
    pub descending: &'static str,
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
    unknown: "—",
    growing: "+",
    ascending: "↑",
    descending: "↓",
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
    unknown: "-",
    growing: "+",
    ascending: "^",
    descending: "v",
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

/// The library palettes the chooser offers after ymp's own, in presentation order, each with
/// the sentence the chooser shows for it.
const LIBRARY: [(ThemeName, &str); 16] = [
    (
        ThemeName::Dracula,
        "Purple and pink accents on a dark violet grey.",
    ),
    (
        ThemeName::OneDarkPro,
        "Atom's balanced dark palette with a blue accent.",
    ),
    (
        ThemeName::Nord,
        "Arctic, north-bluish colours. Clean and easy on the eyes.",
    ),
    (
        ThemeName::CatppuccinMocha,
        "Soothing warm pastels on the darkest Catppuccin base.",
    ),
    (
        ThemeName::CatppuccinLatte,
        "Catppuccin's warm pastels for bright rooms.",
    ),
    (
        ThemeName::GruvboxDark,
        "Retro groove colours on a warm dark background.",
    ),
    (
        ThemeName::GruvboxLight,
        "Retro groove colours on warm cream.",
    ),
    (
        ThemeName::TokyoNight,
        "Futuristic deep blue with bright accents.",
    ),
    (
        ThemeName::SolarizedDark,
        "Precision-tuned colours on the dark Solarized base.",
    ),
    (
        ThemeName::SolarizedLight,
        "Precision-tuned colours on the light Solarized base.",
    ),
    (
        ThemeName::MonokaiPro,
        "Classic syntax-highlighting colours on charcoal.",
    ),
    (
        ThemeName::RosePine,
        "Muted rose, iris and pine on a deep base.",
    ),
    (
        ThemeName::Kanagawa,
        "Ink and wave tones inspired by Katsushika Hokusai.",
    ),
    (ThemeName::Everforest, "Comfortable green forest tones."),
    (ThemeName::Cyberpunk, "Neon-soaked futuristic colours."),
    (
        ThemeName::MidnightCommander,
        "The classic file manager: white and cyan on dark blue.",
    ),
];

/// Every palette the chooser offers, in presentation order. Ember comes first and is the
/// default.
static CATALOG: LazyLock<Vec<Theme>> = LazyLock::new(|| {
    let mut themes = vec![EMBER, SLATE];
    themes.extend(
        LIBRARY
            .iter()
            .map(|(name, summary)| from_library(*name, summary)),
    );
    themes
});

/// Every palette the chooser offers, in presentation order.
pub fn catalog() -> &'static [Theme] {
    &CATALOG
}

/// The palette used when no preference has been stored.
pub const DEFAULT_THEME: &str = EMBER.id;

/// Look up a palette by identifier. An identifier the catalog does not offer, such as one
/// saved by a version that had a palette since removed, falls back to the default.
pub fn theme(id: &str) -> &'static Theme {
    let themes = catalog();
    themes.iter().find(|t| t.id == id).unwrap_or(&themes[0])
}

/// The index of a palette in [`catalog`].
pub fn index_of(id: &str) -> usize {
    catalog().iter().position(|t| t.id == id).unwrap_or(0)
}

/// A palette with the markers the terminal can actually render.
pub fn resolved(id: &str, markers: Markers) -> Theme {
    let mut theme = *theme(id);
    theme.markers = markers;
    theme
}

/// A ymp theme from a `ratatui-themes` palette.
///
/// The roles the library names keep its colours: background, foreground (ymp's body text),
/// accent, the four states, and the muted colour, which ymp uses for its faintest text. The
/// roles it has no name for are derived from those colours:
///
/// - headings and identifiers are the foreground moved away from the background, so they
///   stand out from prose without lowering its contrast;
/// - labels sit halfway between the muted colour and the foreground, so they read above hints;
/// - panels and code are the background moved towards the library's selection colour, and
///   hairlines towards its muted colour;
/// - text on the accent is whichever of the background and the foreground contrasts more
///   with it, so a selected row stays readable on dark and light palettes alike.
fn from_library(name: ThemeName, summary: &'static str) -> Theme {
    let palette = name.palette();
    let accent_ink = if contrast(palette.bg, palette.accent) >= contrast(palette.fg, palette.accent)
    {
        palette.bg
    } else {
        palette.fg
    };
    let (kind, brightest) = if palette.is_light() {
        (ThemeKind::Light, Color::Rgb(0, 0, 0))
    } else {
        (ThemeKind::Dark, Color::Rgb(255, 255, 255))
    };
    Theme {
        id: name.slug(),
        name: name.display_name(),
        summary,
        kind,
        bg: palette.bg,
        surface: mix(palette.bg, palette.selection, 0.35),
        raised: mix(palette.bg, palette.selection, 0.6),
        rule: mix(palette.bg, palette.muted, 0.6),
        text: mix(palette.fg, brightest, 0.35),
        body: palette.fg,
        muted: mix(palette.muted, palette.fg, 0.5),
        faint: palette.muted,
        accent: palette.accent,
        accent_ink,
        info: palette.info,
        good: palette.success,
        warn: palette.warning,
        bad: palette.error,
        focus: palette.accent,
        markers: UNICODE,
    }
}

/// A colour's red, green and blue channels, or nothing for a colour the terminal defines.
fn channels(color: Color) -> Option<[f64; 3]> {
    match color {
        Color::Rgb(red, green, blue) => Some([red, green, blue].map(f64::from)),
        _ => None,
    }
}

/// `from` moved `amount` of the way towards `to`. A colour the terminal defines has no
/// channels to mix, so it is kept.
fn mix(from: Color, to: Color, amount: f64) -> Color {
    match (channels(from), channels(to)) {
        (Some(from), Some(to)) => {
            let [red, green, blue] =
                [0, 1, 2].map(|at| (from[at] + (to[at] - from[at]) * amount).round() as u8);
            Color::Rgb(red, green, blue)
        }
        _ => from,
    }
}

/// Relative luminance as WCAG 2 defines it, from 0 for black to 1 for white.
fn luminance(color: Color) -> f64 {
    let Some(channels) = channels(color) else {
        return 0.0;
    };
    let [red, green, blue] = channels.map(|channel| {
        let channel = channel / 255.0;
        if channel <= 0.040_45 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    });
    0.2126 * red + 0.7152 * green + 0.0722 * blue
}

/// The WCAG 2 contrast ratio of two colours, from 1 for equal colours to 21.
pub fn contrast(a: Color, b: Color) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colours(theme: &Theme) -> [Color; 15] {
        [
            theme.bg,
            theme.surface,
            theme.raised,
            theme.rule,
            theme.text,
            theme.body,
            theme.muted,
            theme.faint,
            theme.accent,
            theme.accent_ink,
            theme.info,
            theme.good,
            theme.warn,
            theme.bad,
            theme.focus,
        ]
    }

    #[test]
    fn ember_and_slate_lead_the_catalog_unchanged_and_ember_is_the_default() {
        let themes = catalog();
        assert_eq!(DEFAULT_THEME, "ember");
        assert_eq!(theme("no-such-theme").id, "ember");
        for (offered, own) in themes[..2].iter().zip([EMBER, SLATE]) {
            assert_eq!(
                (offered.id, offered.name, offered.summary, offered.kind),
                (own.id, own.name, own.summary, own.kind)
            );
            assert_eq!(colours(offered), colours(&own));
        }
        // The values ymp shipped before the library palettes were added.
        assert_eq!(
            colours(&EMBER),
            [
                0x000000, 0x000000, 0x000000, 0x3a352f, 0xf2eee6, 0xd8d2c6, 0x968f85, 0x6b655c,
                0xd8a34c, 0x17130c, 0x8ca9bc, 0x92b889, 0xdd8c3a, 0xd58673, 0xd8a34c,
            ]
            .map(rgb)
        );
        assert_eq!(
            colours(&SLATE),
            [
                0x0f1319, 0x161b23, 0x1f2732, 0x303a47, 0xe6edf3, 0xc4d0dc, 0x8b98a6, 0x5c6873,
                0x5cb3e6, 0x07121a, 0x7aa2f7, 0x7bc99a, 0xe0af68, 0xe06c75, 0x5cb3e6,
            ]
            .map(rgb)
        );
    }

    #[test]
    fn the_catalog_offers_the_requested_library_palettes_in_order() {
        let themes = catalog();
        let names: Vec<&str> = themes.iter().map(|theme| theme.name).collect();
        assert_eq!(
            names,
            [
                "Ember",
                "Slate",
                "Dracula",
                "One Dark Pro",
                "Nord",
                "Catppuccin Mocha",
                "Catppuccin Latte",
                "Gruvbox Dark",
                "Gruvbox Light",
                "Tokyo Night",
                "Solarized Dark",
                "Solarized Light",
                "Monokai Pro",
                "Rosé Pine",
                "Kanagawa",
                "Everforest",
                "Cyberpunk",
                "Midnight Commander",
            ]
        );
        let mut ids: Vec<&str> = themes.iter().map(|theme| theme.id).collect();
        assert!(
            !ids.iter()
                .any(|id| ["paper", "contrast", "terminal"].contains(id)),
            "a removed palette is still offered: {ids:?}"
        );
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), themes.len(), "theme identifiers must be unique");

        for (offered, (name, _)) in themes[2..].iter().zip(LIBRARY) {
            let palette = name.palette();
            assert_eq!(offered.id, name.slug());
            // The library's own colours, not a copy of them.
            assert_eq!(
                [offered.bg, offered.body, offered.accent, offered.faint],
                [palette.bg, palette.fg, palette.accent, palette.muted]
            );
            assert_eq!(
                [offered.info, offered.good, offered.warn, offered.bad],
                [
                    palette.info,
                    palette.success,
                    palette.warning,
                    palette.error
                ]
            );
            assert_eq!(
                offered.kind == ThemeKind::Light,
                palette.is_light(),
                "{} is classified against its library palette",
                offered.id
            );
        }
        let light: Vec<&str> = themes
            .iter()
            .filter(|theme| theme.kind == ThemeKind::Light)
            .map(|theme| theme.id)
            .collect();
        assert_eq!(
            light,
            ["catppuccin-latte", "gruvbox-light", "solarized-light"]
        );
    }

    /// Solarized keeps its deliberately soft foreground, about 4.1:1 on the light base, so
    /// body text is held to the 3:1 floor and headings, which ymp brightens, to 4.5:1.
    #[test]
    fn every_palette_keeps_its_text_labels_and_selected_row_readable() {
        let mut unreadable = Vec::new();
        for theme in catalog() {
            let body = contrast(theme.body, theme.bg);
            for (role, ratio, least) in [
                ("heading", contrast(theme.text, theme.bg), 4.5_f64.max(body)),
                ("body", body, 3.0),
                ("body on a panel", contrast(theme.body, theme.surface), 3.0),
                ("code", contrast(theme.text, theme.raised), 3.0),
                ("label", contrast(theme.muted, theme.bg), 3.0),
                (
                    "selected row",
                    contrast(theme.accent_ink, theme.accent),
                    3.0,
                ),
            ] {
                if ratio < least {
                    unreadable.push(format!("{} {role}: {ratio:.2} is below {least}", theme.id));
                }
            }
        }
        assert!(unreadable.is_empty(), "{}", unreadable.join("\n"));
    }

    #[test]
    fn the_ink_of_a_selected_row_is_the_side_that_contrasts_more() {
        // Midnight Commander's white foreground reads worse on its cyan accent than its dark
        // blue background does.
        let mc = theme("midnight-commander");
        let palette = ThemeName::MidnightCommander.palette();
        assert!(contrast(palette.bg, palette.accent) > contrast(palette.fg, palette.accent));
        assert_eq!(mc.accent_ink, palette.bg);
        let latte = theme("catppuccin-latte");
        assert!(contrast(latte.accent_ink, latte.accent) >= 3.0);
    }
}
