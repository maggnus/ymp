//! Token statistics: what the interface knows about a session's token accounting.
//!
//! The store owns the arithmetic, this module owns the presentation, and one rule governs
//! all of it: a count nobody reported is unknown, never zero. An unknown count is an em
//! dash, a figure that can still grow carries a `+`, and the coverage behind every number
//! is available in words next to it: how many invocations reported, how many were partial,
//! how many are open, and how many were recorded without an agent at all.
//!
//! Statistics are grouped by agent. Two agents that share a provider are two rows with two
//! counters; the provider is secondary metadata and never groups anything. A snapshot also
//! carries the session it describes, so statistics from a run this window is not showing
//! can be discarded instead of mixed in.

use crate::text;
use crate::theme::Theme;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ymp_core::{AgentProfile, Config, SessionUsage, UsageTotals};

/// Everything the interface presents about one session's token accounting.
#[derive(Clone, Debug, Default)]
pub struct Stats {
    session: Option<String>,
    usage: Option<SessionUsage>,
}

impl Stats {
    /// Do these statistics describe `session`?
    pub fn describes(&self, session: &str) -> bool {
        self.session.as_deref() == Some(session)
    }

    /// Replace everything known about a session.
    ///
    /// A snapshot is never merged into the one before it. The store reports the whole
    /// session every time, so adding them together would count every turn again.
    pub fn replace(&mut self, session: &str, usage: SessionUsage) {
        self.session = Some(session.to_owned());
        self.usage = Some(usage);
    }

    /// Record that a session is open but its statistics could not be read. This is not
    /// the same as a session that has spent nothing.
    pub fn unavailable(&mut self, session: &str) {
        self.session = Some(session.to_owned());
        self.usage = None;
    }

    /// Forget everything, for a conversation that was closed or never opened.
    pub fn clear(&mut self) {
        self.session = None;
        self.usage = None;
    }

    /// The session total, or nothing when no snapshot has been read.
    pub fn total(&self) -> Option<&UsageTotals> {
        self.usage.as_ref().map(|usage| &usage.total)
    }

    /// One agent's totals.
    ///
    /// An agent a recorded session never invoked has spent nothing, which is a known zero.
    /// Without a snapshot at all, nothing about it is known — and neither is anything known
    /// about an agent with no row while the session holds invocations it cannot attribute:
    /// one of those may well be this agent's.
    pub fn agent(&self, id: &str) -> Option<UsageTotals> {
        let usage = self.usage.as_ref()?;
        match usage.agents.get(id) {
            Some(totals) => Some(totals.clone()),
            None if self.unattributed() > 0 => None,
            None => Some(UsageTotals::default()),
        }
    }

    /// Invocations the session counted without recording which agent made them.
    ///
    /// A session can know how many turns it spent without knowing who spent them: older
    /// traces recorded the turn count but not every turn. Those invocations belong to
    /// somebody, so while there are any, no agent figure is complete and no missing agent
    /// row can be called zero.
    pub fn unattributed(&self) -> u64 {
        let Some(usage) = &self.usage else {
            return 0;
        };
        let attributed: u64 = usage.agents.values().map(|totals| totals.calls).sum();
        usage.total.calls.saturating_sub(attributed)
    }
}

/// Is there token accounting to show: a conversation is open, or a snapshot has already
/// arrived for the run that is opening one?
pub fn present(session: Option<&str>, stats: &Stats) -> bool {
    session.is_some() || stats.total().is_some()
}

/// One agent's line of statistics.
#[derive(Clone, Debug)]
pub struct AgentUsage {
    /// The stable actor ID. A row is named where it is drawn, by the model its turns ran.
    pub id: String,
    /// Secondary metadata. It never groups a row: two agents on one provider stay apart.
    pub provider: String,
    pub totals: Option<UsageTotals>,
    /// The agent spent tokens in this session without being in the team it captured.
    pub outside_team: bool,
    /// The session holds invocations it cannot attribute, so any of them could be this
    /// agent's. Its figure is a lower bound however complete its own counters look.
    pub incomplete: bool,
}

/// Every agent worth a row: the session's own team first, in the order it captured, then
/// anyone else the session actually invoked.
///
/// The team comes from the session rather than the configuration, because a profile that
/// was renamed or removed since the run must still be able to account for what it spent.
pub fn agent_rows(stats: &Stats, team: &[AgentProfile], config: &Config) -> Vec<AgentUsage> {
    let incomplete = stats.unattributed() > 0;
    let mut rows: Vec<AgentUsage> = team
        .iter()
        .map(|profile| AgentUsage {
            id: profile.id.clone(),
            provider: profile.provider.clone(),
            totals: stats.agent(&profile.id),
            outside_team: false,
            incomplete,
        })
        .collect();
    let recorded = stats.usage.as_ref().map(|usage| &usage.agents);
    for id in recorded.into_iter().flat_map(|agents| agents.keys()) {
        if rows.iter().any(|row| &row.id == id) {
            continue;
        }
        let profile = config.agents.iter().find(|agent| &agent.id == id);
        rows.push(AgentUsage {
            id: id.clone(),
            provider: profile
                .map(|agent| agent.provider.clone())
                .unwrap_or_default(),
            totals: stats.agent(id),
            outside_team: true,
            incomplete,
        });
    }
    rows
}

/// A count for a narrow column: exact below ten thousand, then one decimal and a unit.
pub fn compact(value: Option<u64>, theme: &Theme) -> String {
    match value {
        None => theme.markers.unknown.to_owned(),
        Some(value) if value < 10_000 => value.to_string(),
        // A rounded 999 999 must not be printed as 1000.0k.
        Some(value) if value < 999_950 => format!("{:.1}k", value as f64 / 1e3),
        Some(value) => format!("{:.1}M", value as f64 / 1e6),
    }
}

/// A count in full, grouped in threes so a long figure can still be read.
pub fn exact(value: Option<u64>, theme: &Theme) -> String {
    let Some(value) = value else {
        return theme.markers.unknown.to_owned();
    };
    let digits = value.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.char_indices() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(' ');
        }
        out.push(digit);
    }
    out
}

/// The headline figure for a set of invocations, marked while it can still grow.
///
/// Nothing known at all is an em dash. A known figure that some invocation has not
/// finished contributing to is a lower bound, and says so.
pub fn headline(totals: Option<&UsageTotals>, theme: &Theme) -> String {
    figure(totals, false, theme)
}

/// Colour repeats what the figure already says, so neither carries meaning alone.
pub fn headline_style(totals: Option<&UsageTotals>, theme: &Theme) -> Style {
    figure_style(totals, false, theme)
}

/// The headline for one agent.
///
/// It is a lower bound not only when its own invocations are unfinished, but also when the
/// session holds invocations with no agent recorded, because one of those may be this
/// agent's. Nothing is added to the figure on that account: only the mark that says the
/// figure is incomplete.
pub fn agent_headline(row: &AgentUsage, theme: &Theme) -> String {
    figure(row.totals.as_ref(), row.incomplete, theme)
}

/// Colour for an agent figure, following the same rule as the figure itself.
pub fn agent_headline_style(row: &AgentUsage, theme: &Theme) -> Style {
    figure_style(row.totals.as_ref(), row.incomplete, theme)
}

/// Is this figure a lower bound rather than a settled amount? `incomplete` carries a
/// reason that the counters themselves cannot show, such as an invocation the session
/// could not attribute to anybody.
fn growing(totals: Option<&UsageTotals>, incomplete: bool) -> bool {
    totals
        .is_some_and(|totals| totals.known_total().is_some() && (totals.is_partial() || incomplete))
}

fn figure(totals: Option<&UsageTotals>, incomplete: bool, theme: &Theme) -> String {
    let Some(totals) = totals else {
        return theme.markers.unknown.to_owned();
    };
    let figure = compact(totals.known_total(), theme);
    if growing(Some(totals), incomplete) {
        return format!("{figure}{}", theme.markers.growing);
    }
    figure
}

fn figure_style(totals: Option<&UsageTotals>, incomplete: bool, theme: &Theme) -> Style {
    match totals {
        None => theme.faint(),
        Some(totals) if totals.known_total().is_none() => theme.faint(),
        Some(totals) if totals.is_partial() || incomplete => theme.info(),
        Some(_) => theme.body(),
    }
}

/// What a figure is based on, in words: nothing here is inferred from the figure itself.
pub fn coverage(totals: Option<&UsageTotals>, unattributed: u64) -> String {
    let Some(totals) = totals else {
        return "No statistics have been read for this session.".to_owned();
    };
    if totals.calls == 0 {
        return "No agent invocation has been recorded.".to_owned();
    }
    let mut text = format!(
        "{} of {} invocations reported",
        totals.reported, totals.calls
    );
    if totals.partial_calls > 0 {
        text.push_str(&format!(" · {} partial", totals.partial_calls));
    }
    if totals.open_calls > 0 {
        text.push_str(&format!(" · {} open", totals.open_calls));
    }
    if unattributed > 0 {
        text.push_str(&format!(" · {unattributed} without an agent"));
    }
    text
}

/// What the invocations with no agent mean for the figures underneath them.
///
/// A session that counted its turns without recording who took them cannot have its
/// spending divided between agents. Rather than dividing it anyway, the interface says how
/// many invocations are unaccounted for and stops calling any agent figure complete.
pub fn unattributed_note(unattributed: u64) -> Option<String> {
    if unattributed == 0 {
        return None;
    }
    let invocations = if unattributed == 1 {
        "1 invocation was".to_owned()
    } else {
        format!("{unattributed} invocations were")
    };
    Some(format!(
        "{invocations} recorded without the agent that made them, which older sessions do. \
         The agent figures below therefore need not add up to the session total, an agent \
         with no figure at all may still have spent tokens, and none of them is divided or \
         guessed here."
    ))
}

/// What the open invocations mean, which depends on whether this window is running them.
///
/// An open invocation is one with no final status recorded. While a run is active in this
/// window that means a turn is in flight. In a stored session it means no final status was
/// ever written, which is what an interrupted or crashed run leaves behind: it is not
/// evidence that anything is still running.
pub fn open_note(open: u64, live: bool) -> Option<String> {
    if open == 0 {
        return None;
    }
    let invocations = if open == 1 {
        "1 invocation is".to_owned()
    } else {
        format!("{open} invocations are")
    };
    Some(if live {
        format!("{invocations} open: the active run has not finished reporting them, so the total can still grow.")
    } else {
        format!("{invocations} open, meaning no final status was recorded for them. A run that was interrupted leaves them this way; it does not mean work is still in progress.")
    })
}

/// The complete breakdown of one set of totals, as label and value rows.
///
/// Cache reads and writes are indented under the input they are part of, and reasoning
/// under the output, because that is the arithmetic: neither is added again.
pub fn breakdown(
    totals: Option<&UsageTotals>,
    incomplete: bool,
    theme: &Theme,
    width: usize,
) -> Vec<Line<'static>> {
    let width = width.max(16);
    let counts = totals.map(|totals| &totals.counts);
    let count = |amount: fn(&ymp_core::TokenCounts) -> Option<u64>| counts.and_then(amount);
    let mut lines = vec![
        amount(theme, "input", count(|c| c.input), width, false),
        amount(theme, "cache read", count(|c| c.cache_read), width, true),
        amount(theme, "cache write", count(|c| c.cache_write), width, true),
        amount(theme, "output", count(|c| c.output), width, false),
        amount(theme, "reasoning", count(|c| c.reasoning), width, true),
    ];
    let total = totals.and_then(UsageTotals::known_total);
    let mark = if growing(totals, incomplete) {
        theme.markers.growing
    } else {
        ""
    };
    lines.push(text::row(
        width,
        vec![Span::styled("total".to_owned(), theme.muted())],
        vec![Span::styled(
            format!("{}{mark}", exact(total, theme)),
            figure_style(totals, incomplete, theme),
        )],
    ));
    // Without a snapshot there is nothing to count; the caller says why in words.
    let Some(totals) = totals else {
        return lines;
    };
    lines.push(counter(
        theme,
        "invocations",
        totals.calls.to_string(),
        width,
        false,
    ));
    lines.push(counter(
        theme,
        "reported",
        format!("{} of {}", totals.reported, totals.calls),
        width,
        true,
    ));
    if totals.partial_calls > 0 {
        lines.push(counter(
            theme,
            "partial",
            totals.partial_calls.to_string(),
            width,
            true,
        ));
    }
    if totals.open_calls > 0 {
        lines.push(counter(
            theme,
            "open",
            totals.open_calls.to_string(),
            width,
            true,
        ));
    }
    lines
}

/// A token count: unknown counts read as a dash and are styled as absent, not as small.
fn amount(
    theme: &Theme,
    label: &str,
    value: Option<u64>,
    width: usize,
    nested: bool,
) -> Line<'static> {
    let style = match (value, nested) {
        (None, _) => theme.faint(),
        (Some(_), true) => theme.muted(),
        (Some(_), false) => theme.body(),
    };
    row(theme, label, exact(value, theme), style, width, nested)
}

/// A count of invocations, which the store always knows exactly.
fn counter(theme: &Theme, label: &str, value: String, width: usize, nested: bool) -> Line<'static> {
    let style = if nested { theme.muted() } else { theme.body() };
    row(theme, label, value, style, width, nested)
}

fn row(
    theme: &Theme,
    label: &str,
    value: String,
    style: Style,
    width: usize,
    nested: bool,
) -> Line<'static> {
    text::row(
        width,
        vec![Span::styled(
            format!("{}{label}", if nested { "  " } else { "" }),
            if nested { theme.faint() } else { theme.muted() },
        )],
        vec![Span::styled(value, style)],
    )
}

/// What the numbers mean. Every one of these is a property of the accounting, not a
/// reading of any particular figure.
pub const NOTES: &[&str] = &[
    "Input already includes cache reads and writes, and output already includes reasoning. Neither is counted towards the total a second time.",
    "A dash is a count the provider did not report. It is not a zero, and no figure here is estimated from the length of a message.",
    "A total marked + is a lower bound: an invocation is open, or a provider reported only part of the turn it ran.",
];
