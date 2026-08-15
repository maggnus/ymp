//! The transcript model and its line layout.
//!
//! Structure follows the `TranscriptEntry` handoff in `ymp-docs/design/ymp_chat_tui.dc.html`:
//! entry forms sharing one frame, a fixed prefix column, and continuation lines indented under
//! the text rather than under the label.
//!
//! Origin is deliberately separate from body. A collaboration message differs from a run event
//! by its author and kind, not by its shape, so POC-2 board traffic lands in this transcript
//! without changing scrolling, wrapping or layout.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use crate::style;
use crate::text;
use crate::theme::{self, Markers};

/// Width of the label column used by human turns and application replies.
const LABEL_PREFIX: usize = 7;
/// Width of the time-and-plane column used by journal events.
const EVENT_PREFIX: usize = 19;

/// Trust plane of a journal event. Always rendered as a word; colour is redundant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Plane {
    /// Authoritative control state written only by the kernel.
    Control,
    /// Untrusted collaboration board: inert, attributed payload.
    Collaboration,
    /// Protected verification plane.
    Verification,
}

impl Plane {
    pub fn label(self) -> &'static str {
        match self {
            Self::Control => "ctrl",
            Self::Collaboration => "collab",
            Self::Verification => "verif",
        }
    }

    fn style(self) -> Style {
        Style::default().fg(match self {
            Self::Control => theme::PLANE_CTRL,
            Self::Collaboration => theme::PLANE_COLLAB,
            Self::Verification => theme::PLANE_VERIF,
        })
    }
}

/// Message kinds defined by `PROTOCOL.md`. Present so board traffic renders unchanged when the
/// collaboration plane exists; nothing produces these entries yet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageKind {
    Proposal,
    Question,
    Hypothesis,
    Observation,
    Constraint,
    DeadEnd,
    Challenge,
    Confirmation,
    Decision,
    HelpRequest,
}

impl MessageKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Proposal => "proposal",
            Self::Question => "question",
            Self::Hypothesis => "hypothesis",
            Self::Observation => "observation",
            Self::Constraint => "constraint",
            Self::DeadEnd => "dead_end",
            Self::Challenge => "challenge",
            Self::Confirmation => "confirmation",
            Self::Decision => "decision",
            Self::HelpRequest => "help_request",
        }
    }
}

/// One row of a structured entry step: a labelled field with its class, value and default.
#[derive(Clone, Debug)]
pub struct FieldRow {
    pub label: String,
    pub class: String,
    pub value: String,
    pub unit: String,
    pub note: String,
    pub focused: bool,
    pub invalid: Option<String>,
}

/// A titled, boxed group of field rows — the `FieldRow` block of the design handoff.
#[derive(Clone, Debug)]
pub struct FieldGroup {
    pub title: String,
    pub fields: Vec<FieldRow>,
}

/// The transcript entry forms.
#[derive(Clone, Debug)]
pub enum Entry {
    /// What the operator said.
    Human { text: String },
    /// An ordinary application reply. The first line carries emphasis; the rest is body.
    AppReply { text: String },
    /// What the runtime doing the work reported, attributed to the profile that reported it.
    ///
    /// It is not a journal fact and is never shown as one: the durable record of the same
    /// invocation is the run's runtime evidence, and the journal carries only what the controller
    /// committed. Nothing here is executable and nothing here decides anything.
    RuntimeNote { profile: String, text: String },
    /// A failure the application is reporting, with the exact cause and the way out.
    AppError { text: String },
    /// A durable journal event.
    RunEvent {
        time: String,
        plane: Plane,
        text: String,
    },
    /// An attributed, inert collaboration message. Nothing here is executable.
    BoardMessage {
        time: String,
        participant: String,
        kind: MessageKind,
        text: String,
    },
    /// A structured step rendered inside the conversation: a title, prose, boxed field groups.
    Structured {
        title: String,
        note: String,
        groups: Vec<FieldGroup>,
        hint: String,
    },
    /// The wordmark, drawn once on an empty transcript.
    Banner { version: String },
    /// A blank separator line.
    Blank,
}

/// The ymp wordmark as it appears in the design artifact.
const WORDMARK: [&str; 5] = [
    r"  _   _ _ __ ___  _ __",
    r" | | | | '_ ` _ \| '_ \",
    r" | |_| | | | | | | |_) |",
    r"  \__, |_| |_| |_| .__/",
    r"  |___/          |_|",
];

impl Entry {
    /// Lay the entry out into terminal lines for the given content width.
    pub fn layout(&self, width: u16, markers: &Markers) -> Vec<Line<'static>> {
        let width = width as usize;
        match self {
            Self::Blank => vec![Line::default()],

            Self::Banner { version } => {
                let mut lines: Vec<Line<'static>> = WORDMARK
                    .iter()
                    .map(|row| Line::from(Span::styled((*row).to_owned(), theme::accent())))
                    .collect();
                if let Some(last) = lines.last_mut() {
                    last.spans
                        .push(Span::styled(format!("     {version}"), theme::faint()));
                }
                lines
            }

            Self::Human { text } => {
                let mut lines = Vec::new();
                let body_width = width.saturating_sub(LABEL_PREFIX).max(1);
                for (index, piece) in text::wrap(&text::sanitize(text), body_width)
                    .into_iter()
                    .enumerate()
                {
                    let mut spans: Vec<Span<'static>> = Vec::new();
                    if index == 0 {
                        spans.push(Span::styled(format!("{} ", markers.prompt), theme::amber()));
                        spans.push(Span::styled("you  ".to_owned(), theme::amber()));
                    } else {
                        spans.push(Span::raw(" ".repeat(LABEL_PREFIX)));
                    }
                    spans.push(Span::styled(piece, theme::text()));
                    lines.push(Line::from(spans));
                }
                lines
            }

            Self::AppReply { text } => labelled_app(text, theme::dim(), width, false),

            Self::AppError { text } => {
                let body = format!("{} {}", markers.fail, text);
                labelled_app(&body, theme::red(), width, true)
            }

            Self::RuntimeNote { profile, text } => labelled_runtime(profile, text, width),

            Self::RunEvent { time, plane, text } => {
                event_lines(time, *plane, None, None, text, width, markers)
            }

            Self::BoardMessage {
                time,
                participant,
                kind,
                text,
            } => event_lines(
                time,
                Plane::Collaboration,
                Some(participant.as_str()),
                Some(*kind),
                text,
                width,
                markers,
            ),

            Self::Structured {
                title,
                note,
                groups,
                hint,
            } => structured_lines(title, note, groups, hint, width, markers),
        }
    }
}

/// An application turn: faint `ymp` label, first line bright, continuations aligned under text.
fn labelled_app(body: &str, body_style: Style, width: usize, plain: bool) -> Vec<Line<'static>> {
    let content_width = width.saturating_sub(LABEL_PREFIX).max(1);
    let wrapped = text::wrap(&text::sanitize(body), content_width);

    let mut lines = Vec::with_capacity(wrapped.len());
    for (index, piece) in wrapped.into_iter().enumerate() {
        let mut spans: Vec<Span<'static>> = Vec::new();
        if index == 0 {
            spans.push(Span::styled("  ymp  ".to_owned(), theme::faint()));
        } else {
            spans.push(Span::raw(" ".repeat(LABEL_PREFIX)));
        }
        if plain {
            spans.push(Span::styled(piece, body_style));
        } else {
            spans.extend(style::spans(&piece, body_style));
        }
        lines.push(Line::from(spans));
    }
    lines
}

/// What the runtime doing the work reported: the profile it came from, then the text.
///
/// The label is the profile's own name rather than a fixed column, so a longer name pushes the
/// first line right instead of being cut to something the operator would have to decode. Every
/// line still fits the terminal, because the wrap width follows the label.
fn labelled_runtime(profile: &str, body: &str, width: usize) -> Vec<Line<'static>> {
    let head = profile.chars().count() + 2;
    let indent = LABEL_PREFIX.max(head);
    let content_width = width.saturating_sub(indent).max(1);

    let mut lines = Vec::new();
    for (index, piece) in text::wrap(&text::sanitize(body), content_width)
        .into_iter()
        .enumerate()
    {
        let mut spans: Vec<Span<'static>> = Vec::new();
        if index == 0 {
            spans.push(Span::styled(format!(" {profile} "), theme::amber()));
            spans.push(Span::raw(" ".repeat(indent - head)));
        } else {
            spans.push(Span::raw(" ".repeat(indent)));
        }
        spans.push(Span::styled(piece, theme::muted()));
        lines.push(Line::from(spans));
    }
    lines
}

/// A journal event: time, plane word, then the fact. Board messages add `participant ▸ kind`
/// on the first line and continue underneath in muted text.
fn event_lines(
    time: &str,
    plane: Plane,
    participant: Option<&str>,
    kind: Option<MessageKind>,
    body: &str,
    width: usize,
    markers: &Markers,
) -> Vec<Line<'static>> {
    let content_width = width.saturating_sub(EVENT_PREFIX).max(1);
    let body = text::sanitize(body);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let head: Vec<Span<'static>> = vec![
        Span::raw("  ".to_owned()),
        Span::styled(pad(time, 10), theme::faint()),
        Span::styled(pad(plane.label(), 7), plane.style()),
    ];

    match (participant, kind) {
        (Some(participant), Some(kind)) => {
            // Attribution and kind occupy the first line; the payload follows indented, which
            // keeps inert message text visually separate from the control facts above it.
            let mut first = head;
            first.push(Span::styled(participant.to_owned(), theme::bold()));
            first.push(Span::styled(
                format!(" {} ", markers.message_kind),
                theme::faint(),
            ));
            first.push(Span::styled(kind.label().to_owned(), plane.style()));
            lines.push(Line::from(first));

            let indent = EVENT_PREFIX + 2;
            let payload_width = width.saturating_sub(indent).max(1);
            for piece in text::wrap(&body, payload_width) {
                lines.push(Line::from(vec![
                    Span::raw(" ".repeat(indent)),
                    Span::styled(piece, theme::muted()),
                ]));
            }
        }
        _ => {
            let wrapped = text::wrap(&body, content_width);
            for (index, piece) in wrapped.into_iter().enumerate() {
                if index == 0 {
                    let mut first = head.clone();
                    first.extend(style::spans(&piece, theme::dim()));
                    lines.push(Line::from(first));
                } else {
                    lines.push(Line::from(vec![
                        Span::raw(" ".repeat(EVENT_PREFIX + 2)),
                        Span::styled(piece, theme::muted()),
                    ]));
                }
            }
        }
    }

    lines
}

/// A structured step: `ymp` turn with title and note, then boxed field groups, then the keys
/// hint — the composition of frame 1c.
fn structured_lines(
    title: &str,
    note: &str,
    groups: &[FieldGroup],
    hint: &str,
    width: usize,
    markers: &Markers,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Title as a bright app turn, note as muted continuation.
    let content_width = width.saturating_sub(LABEL_PREFIX).max(1);
    for (index, piece) in text::wrap(&text::sanitize(title), content_width)
        .into_iter()
        .enumerate()
    {
        let mut spans: Vec<Span<'static>> = Vec::new();
        if index == 0 {
            spans.push(Span::styled("  ymp  ".to_owned(), theme::faint()));
        } else {
            spans.push(Span::raw(" ".repeat(LABEL_PREFIX)));
        }
        spans.push(Span::styled(piece, theme::text()));
        lines.push(Line::from(spans));
    }
    for piece in text::wrap(&text::sanitize(note), content_width) {
        lines.push(Line::from(vec![
            Span::raw(" ".repeat(LABEL_PREFIX)),
            Span::styled(piece, theme::muted()),
        ]));
    }

    // Boxed groups, indented to the label column.
    let box_width = width.saturating_sub(LABEL_PREFIX + 1).max(20);
    for group in groups {
        lines.push(Line::default());
        lines.extend(field_group_lines(group, box_width, markers));
    }

    if !hint.is_empty() {
        lines.push(Line::default());
        let mut spans = vec![Span::raw(" ".repeat(LABEL_PREFIX))];
        spans.extend(hint_spans(hint));
        lines.push(Line::from(spans));
    }

    lines
}

/// One bordered field group with its title on the top border.
fn field_group_lines(group: &FieldGroup, width: usize, markers: &Markers) -> Vec<Line<'static>> {
    let indent = " ".repeat(LABEL_PREFIX);
    let inner = width.saturating_sub(2);
    let mut lines = Vec::new();

    // ┌─ title ────┐
    let title = format!(" {} ", group.title);
    let dashes = inner.saturating_sub(text::width(&title) + 1);
    lines.push(Line::from(vec![
        Span::raw(indent.clone()),
        Span::styled("┌─".to_owned(), theme::rule()),
        Span::styled(title, theme::muted()),
        Span::styled("─".repeat(dashes), theme::rule()),
        Span::styled("┐".to_owned(), theme::rule()),
    ]));

    let label_width = group
        .fields
        .iter()
        .map(|field| text::width(&field.label))
        .max()
        .unwrap_or(0)
        .max(9);
    let class_width = group
        .fields
        .iter()
        .map(|field| text::width(&field.class))
        .max()
        .unwrap_or(0);
    let value_width = group
        .fields
        .iter()
        .map(|field| text::width(&field.value))
        .max()
        .unwrap_or(0)
        .max(6);

    let body_row = |spans: Vec<Span<'static>>| {
        let used: usize = spans.iter().map(|span| text::width(&span.content)).sum();
        let mut row = vec![
            Span::raw(indent.clone()),
            Span::styled("│ ".to_owned(), theme::rule()),
        ];
        row.extend(spans);
        row.push(Span::raw(" ".repeat(inner.saturating_sub(used + 1))));
        row.push(Span::styled("│".to_owned(), theme::rule()));
        Line::from(row)
    };

    for field in &group.fields {
        let mut spans: Vec<Span<'static>> = vec![Span::styled(
            pad(&field.label, label_width + 2),
            theme::dim(),
        )];
        if class_width > 0 {
            spans.push(Span::styled(
                pad(&field.class, class_width + 2),
                theme::faint(),
            ));
        }
        if field.value.is_empty() {
            spans.push(Span::styled(field.note.clone(), theme::faint()));
        } else {
            let value_style = if field.focused {
                theme::selected()
            } else {
                theme::text()
            };
            spans.push(Span::styled("[ ".to_owned(), theme::rule()));
            spans.push(Span::styled(pad(&field.value, value_width), value_style));
            spans.push(Span::styled(" ]".to_owned(), theme::rule()));
            if !field.unit.is_empty() {
                spans.push(Span::styled(format!(" {}", field.unit), theme::dim()));
            }
            if !field.note.is_empty() {
                spans.push(Span::raw("  ".to_owned()));
                spans.extend(style::spans(&field.note, theme::faint()));
            }
        }
        lines.push(body_row(spans));

        if let Some(reason) = &field.invalid {
            let offset =
                " ".repeat(label_width + 2 + if class_width > 0 { class_width + 2 } else { 0 });
            lines.push(body_row(vec![
                Span::raw(offset),
                Span::styled(format!("{} {}", markers.fail, reason), theme::red()),
            ]));
        }
    }

    lines.push(Line::from(vec![
        Span::raw(indent),
        Span::styled(format!("└{}┘", "─".repeat(inner)), theme::rule()),
    ]));

    lines
}

/// Key hints like "Tab next field · S-Tab prev · Enter accept step": key names amber.
pub fn hint_spans(hint: &str) -> Vec<Span<'static>> {
    const KEYS: &[&str] = &[
        "Tab",
        "S-Tab",
        "Enter",
        "Esc",
        "End",
        "PgUp",
        "PgDn",
        "PgUp/PgDn",
        "↑↓",
    ];
    let mut spans = Vec::new();
    let mut plain = String::new();
    for chunk in hint.split_inclusive(' ') {
        let token = chunk.trim_end_matches(' ');
        if KEYS.contains(&token) {
            if !plain.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut plain), theme::faint()));
            }
            spans.push(Span::styled(token.to_owned(), theme::accent()));
            plain.push_str(&chunk[token.len()..]);
        } else {
            plain.push_str(chunk);
        }
    }
    if !plain.is_empty() {
        spans.push(Span::styled(plain, theme::faint()));
    }
    spans
}

pub fn pad(value: &str, width: usize) -> String {
    let current = text::width(value);
    if current >= width {
        value.to_owned()
    } else {
        format!("{value}{}", " ".repeat(width - current))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(lines: &[Line<'static>]) -> Vec<String> {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect()
    }

    #[test]
    fn every_entry_form_fits_the_given_width() {
        let markers = theme::UNICODE;
        let entries = vec![
            Entry::Human {
                text: "make the replay path idempotent".into(),
            },
            Entry::AppReply {
                text: "reading the durable journal for this project …".into(),
            },
            Entry::AppError {
                text: "runtime probe failed: executable not found — fix: install the runtime"
                    .into(),
            },
            Entry::RunEvent {
                time: "#0018".into(),
                plane: Plane::Verification,
                text: "verification recorded · candidate 9f2a rejected · the replay check failed: \
                       a second request with the same key observed a conflict where the first \
                       response was expected"
                    .into(),
            },
            Entry::BoardMessage {
                time: "#0021".into(),
                participant: "participant-1".into(),
                kind: MessageKind::Observation,
                text: "the replayed response must be identical byte for byte, not merely equal \
                       once decoded."
                    .into(),
            },
        ];

        for width in [80u16, 120, 180] {
            for entry in &entries {
                for line in plain(&entry.layout(width, &markers)) {
                    assert!(
                        text::width(&line) <= width as usize,
                        "line {line:?} exceeds width {width}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_run_event_states_its_plane_as_a_word() {
        let entry = Entry::RunEvent {
            time: "#0001".into(),
            plane: Plane::Control,
            text: "run demo-run started".into(),
        };
        let rendered = plain(&entry.layout(120, &theme::UNICODE)).join("\n");
        assert!(rendered.contains("ctrl"), "{rendered}");
    }

    #[test]
    fn a_board_message_names_its_author_and_kind() {
        let entry = Entry::BoardMessage {
            time: "#0029".into(),
            participant: "participant-1".into(),
            kind: MessageKind::DeadEnd,
            text: "advisory row lock deadlocks under the test harness".into(),
        };
        let rendered = plain(&entry.layout(120, &theme::UNICODE));
        assert!(rendered[0].contains("participant-1"), "{rendered:?}");
        assert!(rendered[0].contains("dead_end"), "{rendered:?}");
        assert!(rendered[0].contains("collab"), "{rendered:?}");
    }

    #[test]
    fn a_structured_block_draws_its_boxes_within_width() {
        let entry = Entry::Structured {
            title: "budget dimensions of this run".into(),
            note: "Fields below are part of this conversation.".into(),
            groups: vec![FieldGroup {
                title: "budget · 2 dimensions".into(),
                fields: vec![
                    FieldRow {
                        label: "attempts".into(),
                        class: "enforced".into(),
                        value: "3".into(),
                        unit: String::new(),
                        note: "1".into(),
                        focused: true,
                        invalid: None,
                    },
                    FieldRow {
                        label: "verification_queries".into(),
                        class: "enforced".into(),
                        value: "x".into(),
                        unit: String::new(),
                        note: "1".into(),
                        focused: false,
                        invalid: Some("not a whole number".into()),
                    },
                ],
            }],
            hint: "Tab next field · S-Tab prev · Enter accept step · Esc restore default".into(),
        };

        let rendered = plain(&entry.layout(120, &theme::UNICODE));
        let joined = rendered.join("\n");
        assert!(joined.contains("budget · 2 dimensions"), "{joined}");
        assert!(joined.contains("not a whole number"), "{joined}");
        for line in &rendered {
            assert!(text::width(line) <= 120, "line {line:?} too wide");
        }
    }

    #[test]
    fn hostile_text_never_reaches_the_line_buffer() {
        let entry = Entry::AppReply {
            text: "\u{1b}[2Jwiped".into(),
        };
        let rendered = plain(&entry.layout(80, &theme::UNICODE)).join("");
        assert!(!rendered.contains('\u{1b}'));
        assert!(rendered.contains("wiped"));
    }
}
