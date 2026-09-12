//! Turning stored messages into a readable conversation.
//!
//! The transcript is chat-first: the user's prompts and the team's final answers are the
//! content, and the machinery that produced them is one line of activity each. Routine
//! plan, bid and review payloads are structured JSON written for another agent, so they are
//! summarised rather than printed; internal conversation routing is not shown at all.
//! Every collapsed entry keeps its complete attributed text, which the inspector shows.

use crate::text;
use crate::theme::Theme;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use serde_json::Value;
use std::collections::BTreeMap;
use ymp_core::{parse_response, Config, Message};

/// How an entry is presented. The role decides the marker and the emphasis, never the
/// colour alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// A prompt written by the user.
    User,
    /// A final answer or run summary meant to be read.
    Answer,
    /// An execution report from an agent that changed files.
    Work,
    /// Routine coordination, collapsed to one line.
    Activity,
    /// Something ymp itself reports.
    Notice,
    /// An interface or run failure.
    Failure,
    /// Text a provider is still streaming.
    Stream,
}

/// A local, unsaved line from the interface itself.
#[derive(Clone, Debug)]
pub struct Notice {
    pub failure: bool,
    pub text: String,
    pub time: String,
}

/// One presentable unit of the conversation.
#[derive(Clone, Debug)]
pub struct Entry {
    pub role: Role,
    pub author: String,
    pub kind: String,
    pub time: String,
    pub seq: Option<i64>,
    /// The single line shown when the entry is collapsed.
    pub headline: String,
    /// The readable body, when the entry has one worth expanding.
    pub body: Option<String>,
    /// The complete attributed text, shown by the inspector.
    pub raw: String,
}

impl Entry {
    /// The heading the inspector shows above the full text.
    pub fn title(&self) -> String {
        format!("{} · {}", self.author, self.kind)
    }
}

/// How much of a partial provider response the preview shows. Source lines alone are not
/// a bound, because a structured response can arrive as a single line of any length.
const STREAM_PREVIEW_LINES: usize = 4;
const STREAM_PREVIEW_CELLS: usize = 400;
/// Rows the rendered preview may occupy, whatever the text wraps to.
const STREAM_PREVIEW_ROWS: usize = 4;

/// Message kinds that only route the conversation between agents. They carry no
/// information for the reader and are hidden unless detailed messages are requested.
const ROUTING: &[&str] = &["conversation", "context"];

/// Build the presentable conversation from stored messages, local notices and the text
/// providers are currently streaming.
pub fn build(
    messages: &[Message],
    notices: &[Notice],
    streams: &BTreeMap<String, String>,
    config: &Config,
    pool: &crate::provenance::Pool,
    records: &crate::provenance::Records,
    details: bool,
) -> Vec<Entry> {
    let mut entries = Vec::with_capacity(messages.len() + notices.len() + streams.len());
    for message in messages {
        if ROUTING.contains(&message.kind.as_str()) && !details {
            continue;
        }
        entries.push(entry(message, config, pool, records, details));
    }
    for notice in notices {
        entries.push(Entry {
            role: if notice.failure {
                Role::Failure
            } else {
                Role::Notice
            },
            author: "ymp".into(),
            kind: if notice.failure { "error" } else { "notice" }.into(),
            time: notice.time.clone(),
            seq: None,
            headline: text::one_line(&notice.text),
            body: notice.text.contains('\n').then(|| notice.text.clone()),
            raw: notice.text.clone(),
        });
    }
    for (agent, partial) in streams {
        if partial.trim().is_empty() {
            continue;
        }
        // A provider may stream one enormous line of JSON, so the preview is bounded by
        // characters as well as by lines. The rendered form is bounded again by rows.
        let tail = text::sanitize(partial);
        let tail: String = tail
            .lines()
            .rev()
            .take(STREAM_PREVIEW_LINES)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n");
        let tail = text::last_cells(&tail, STREAM_PREVIEW_CELLS);
        entries.push(Entry {
            role: Role::Stream,
            author: display_name(config, pool, records, agent),
            kind: "streaming".into(),
            time: String::new(),
            seq: None,
            headline: text::one_line(&tail),
            body: Some(tail.clone()),
            raw: partial.clone(),
        });
    }
    entries
}

/// Who wrote an entry. Agents are named by the shared rule, so the transcript, the sidebar and
/// every page say the same thing about the same actor.
fn display_name(
    config: &Config,
    pool: &crate::provenance::Pool,
    records: &crate::provenance::Records,
    id: &str,
) -> String {
    match id {
        "you" => "you".into(),
        "ymp" => "ymp".into(),
        other => crate::views::actor_name(config, pool, records, other),
    }
}

fn entry(
    message: &Message,
    config: &Config,
    pool: &crate::provenance::Pool,
    records: &crate::provenance::Records,
    details: bool,
) -> Entry {
    let author = display_name(config, pool, records, &message.author);
    let time = text::clock(&message.created_at);
    let raw = message.text.clone();
    let (role, headline, body) = present(&message.kind, &message.text, details);
    Entry {
        role,
        author,
        kind: message.kind.clone(),
        time,
        seq: Some(message.seq),
        headline,
        body,
        raw,
    }
}

/// Decide how one stored message reads. Structured payloads become a sentence; prose stays
/// prose. Nothing here invents a value the message does not contain.
fn present(kind: &str, body: &str, details: bool) -> (Role, String, Option<String>) {
    if details {
        return (
            match kind {
                "user" => Role::User,
                "answer" | "summary" | "synthesis" => Role::Answer,
                "execute" => Role::Work,
                "notice" | "memory" | "plan_accepted" => Role::Notice,
                _ => Role::Activity,
            },
            text::first_line(body),
            Some(body.to_owned()),
        );
    }
    match kind {
        "user" => (Role::User, text::one_line(body), Some(body.to_owned())),
        "answer" | "summary" | "synthesis" => {
            (Role::Answer, text::first_line(body), Some(body.to_owned()))
        }
        "execute" => (Role::Work, text::first_line(body), Some(body.to_owned())),
        "plan" => (Role::Activity, plan_line(body), None),
        "bid" => (Role::Activity, bid_line(body), None),
        "review" | "review_plan" | "final_review" | "review_memory" => {
            (Role::Activity, review_line(body), None)
        }
        "learn" => (Role::Activity, learn_line(body), None),
        "plan_accepted" => (
            Role::Notice,
            format!("Plan accepted · {}", text::one_line(body)),
            None,
        ),
        "notice" | "memory" => (Role::Notice, text::one_line(body), None),
        _ => (
            Role::Activity,
            format!("{kind} · {}", text::one_line(body)),
            None,
        ),
    }
}

fn json(body: &str) -> Option<Value> {
    parse_response::<Value>(body).ok()
}

fn plan_line(body: &str) -> String {
    match json(body) {
        Some(value) => {
            let tasks = value["tasks"].as_array().map(Vec::len).unwrap_or(0);
            let summary = value["summary"].as_str().unwrap_or("").trim();
            let noun = if tasks == 1 { "task" } else { "tasks" };
            if summary.is_empty() {
                format!("proposed a plan · {tasks} {noun}")
            } else {
                format!("proposed a plan · {tasks} {noun} · {summary}")
            }
        }
        None => format!("proposed a plan · {}", text::one_line(body)),
    }
}

fn bid_line(body: &str) -> String {
    match json(body) {
        Some(value) => {
            let willing = value["willing"] == Value::Bool(true);
            let approach = value["approach"].as_str().unwrap_or("").trim();
            let verdict = if willing {
                "offered to take it"
            } else {
                "declined"
            };
            if approach.is_empty() {
                format!("bid · {verdict}")
            } else {
                format!("bid · {verdict} · {approach}")
            }
        }
        None => format!("bid · {}", text::one_line(body)),
    }
}

fn review_line(body: &str) -> String {
    match json(body) {
        Some(value) => {
            let approved = value["approved"] == Value::Bool(true);
            let reason = value["reason"].as_str().unwrap_or("").trim();
            let verdict = if approved { "accepted" } else { "rejected" };
            if reason.is_empty() {
                format!("review · {verdict}")
            } else {
                format!("review · {verdict} · {reason}")
            }
        }
        None => format!("review · {}", text::one_line(body)),
    }
}

fn learn_line(body: &str) -> String {
    match json(body) {
        Some(value) if value["useful"] == Value::Bool(true) => {
            let title = value["title"].as_str().unwrap_or("").trim();
            format!("proposed a reusable procedure · {title}")
        }
        Some(_) => "found nothing reusable to record".into(),
        None => format!("learning · {}", text::one_line(body)),
    }
}

/// Lines an entry occupies, laid out for `width` cells.
///
/// The first two columns are a gutter that carries the selection marker, so selecting an
/// entry never reflows the text beside it.
pub fn render(
    entry: &Entry,
    width: usize,
    theme: &Theme,
    selected: bool,
    expanded: bool,
) -> Vec<Line<'static>> {
    let markers = theme.markers;
    let gutter = if selected {
        Span::styled(format!("{} ", markers.selection), theme.accent_bold())
    } else {
        Span::raw("  ".to_owned())
    };
    let inner = width.saturating_sub(4).max(8);
    let head_style = if selected {
        theme.selected()
    } else {
        Style::default()
    };
    let mut lines: Vec<Line<'static>> = Vec::new();

    match entry.role {
        Role::User => {
            lines.push(Line::default());
            let header = text::row(
                inner,
                vec![
                    Span::styled(format!("{} ", markers.user), theme.accent_bold()),
                    Span::styled(entry.author.clone(), theme.accent_bold().patch(head_style)),
                ],
                vec![Span::styled(entry.time.clone(), theme.faint())],
            );
            lines.push(prepend(gutter.clone(), header));
            let body = entry.body.clone().unwrap_or_else(|| entry.headline.clone());
            for piece in text::wrap(&text::sanitize(&body), inner.saturating_sub(2)) {
                lines.push(Line::from(vec![
                    Span::raw("  ".to_owned()),
                    Span::styled(format!("{} ", markers.user), theme.accent()),
                    Span::styled(piece, theme.bold()),
                ]));
            }
        }
        Role::Answer => {
            lines.push(Line::default());
            let label = match entry.kind.as_str() {
                "summary" => "run summary",
                "answer" => "answer",
                _ => "final result",
            };
            let header = text::row(
                inner,
                vec![
                    Span::styled(format!("{} ", markers.answer), theme.accent_bold()),
                    Span::styled(entry.author.clone(), theme.bold().patch(head_style)),
                    Span::styled(format!(" · {label}"), theme.muted()),
                ],
                vec![Span::styled(entry.time.clone(), theme.faint())],
            );
            lines.push(prepend(gutter.clone(), header));
            let body = entry.body.clone().unwrap_or_else(|| entry.headline.clone());
            for line in text::markdown(&body, inner.saturating_sub(2), theme, theme.body()) {
                lines.push(prepend(Span::raw("    ".to_owned()), line));
            }
        }
        Role::Work => {
            lines.push(Line::default());
            let header = text::row(
                inner,
                vec![
                    Span::styled(format!("{} ", markers.activity), theme.good()),
                    Span::styled(entry.author.clone(), theme.bold().patch(head_style)),
                    // An execution turn may legitimately conclude that nothing needs
                    // changing, so the label reports the turn, not an outcome.
                    Span::styled(" · execution report".to_owned(), theme.muted()),
                ],
                vec![Span::styled(entry.time.clone(), theme.faint())],
            );
            lines.push(prepend(gutter.clone(), header));
            let body = entry.body.clone().unwrap_or_default();
            let rendered = text::markdown(&body, inner.saturating_sub(2), theme, theme.body());
            let budget = if expanded { rendered.len() } else { 6 };
            let hidden = rendered.len().saturating_sub(budget);
            for line in rendered.into_iter().take(budget) {
                lines.push(prepend(Span::raw("    ".to_owned()), line));
            }
            if hidden > 0 {
                lines.push(Line::from(vec![
                    Span::raw("    ".to_owned()),
                    Span::styled(
                        format!("{hidden} more lines · Enter to read the full report"),
                        theme.faint(),
                    ),
                ]));
            }
        }
        Role::Activity => {
            let head = text::row(
                inner,
                vec![
                    Span::styled(format!("{} ", markers.activity), theme.faint()),
                    Span::styled(
                        format!("{} ", entry.author),
                        theme.muted().patch(head_style),
                    ),
                    Span::styled(entry.headline.clone(), theme.faint().patch(head_style)),
                ],
                vec![Span::styled(entry.time.clone(), theme.faint())],
            );
            lines.push(prepend(gutter.clone(), head));
            if expanded {
                for line in text::markdown(
                    entry.body.as_deref().unwrap_or(&entry.raw),
                    inner.saturating_sub(2),
                    theme,
                    theme.faint(),
                ) {
                    lines.push(prepend(Span::raw("    ".to_owned()), line));
                }
            }
        }
        Role::Notice | Role::Failure => {
            let (marker, style) = if entry.role == Role::Failure {
                (markers.fail, theme.bad())
            } else {
                (markers.notice, theme.info())
            };
            let head = text::row(
                inner,
                vec![
                    Span::styled(format!("{marker} "), style),
                    Span::styled(entry.headline.clone(), style.patch(head_style)),
                ],
                vec![Span::styled(entry.time.clone(), theme.faint())],
            );
            lines.push(prepend(gutter.clone(), head));
        }
        Role::Stream => {
            let head = text::row(
                inner,
                vec![
                    Span::styled(format!("{} ", markers.busy), theme.warn()),
                    Span::styled(entry.author.clone(), theme.bold()),
                    Span::styled(" is writing".to_owned(), theme.muted()),
                ],
                vec![],
            );
            lines.push(prepend(gutter.clone(), head));
            // Whatever shape the stream has, the preview occupies a fixed number of rows.
            let wrapped = text::wrap(
                entry.body.as_deref().unwrap_or_default(),
                inner.saturating_sub(2),
            );
            let first = wrapped.len().saturating_sub(STREAM_PREVIEW_ROWS);
            for piece in wrapped.into_iter().skip(first) {
                lines.push(Line::from(vec![
                    Span::raw("    ".to_owned()),
                    Span::styled(piece, theme.faint()),
                ]));
            }
        }
    }
    lines
}

fn prepend(span: Span<'static>, line: Line<'static>) -> Line<'static> {
    let mut spans = vec![span];
    spans.extend(line.spans);
    Line::from(spans)
}
