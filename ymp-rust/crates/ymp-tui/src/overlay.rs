//! Floating surfaces: the `:` palette, the two decision modals, the key map.
//!
//! Each one is composed into a [`ModalSpec`] and drawn by the frame layer, so a decision cannot
//! acquire its own drawing path. A decision carries the amber frame, an explicit action and
//! Esc; reference material carries the grey frame and can never commit anything.

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

use crate::frame::{self, ModalRole, ModalSpec};
use crate::state::{App, Authorize, Confirm, Modal, Palette, RequirementState};
use crate::style;
use crate::text;
use crate::theme::{self, Markers};
use crate::transcript::pad;

/// The spec for whatever floats above the current surface, if anything.
pub fn modal_spec(app: &App, area: Rect, markers: &Markers) -> Option<ModalSpec> {
    match &app.modal {
        Modal::None => None,
        Modal::Palette(palette) => Some(palette_spec(palette, area)),
        Modal::Authorize(authorize) => Some(authorize_spec(authorize, area, markers)),
        Modal::Confirm(confirm) => Some(confirm_spec(confirm, markers)),
        Modal::Keys => Some(keys_spec()),
    }
}

// ---------------------------------------------------------------------------
// Command palette
// ---------------------------------------------------------------------------

const PALETTE_WIDTH: u16 = 96;

fn palette_spec(palette: &Palette, area: Rect) -> ModalSpec {
    let width = PALETTE_WIDTH.min(area.width.saturating_sub(6));
    let inner = width.saturating_sub(2);
    let matches = palette.matches();

    let mut body = vec![
        Line::from(vec![
            Span::styled(palette.input.clone(), theme::text()),
            Span::styled("▁".to_owned(), theme::accent()),
        ]),
        frame::inner_rule(inner),
    ];

    let name_width = matches
        .iter()
        .map(|item| text::width(&item.name))
        .max()
        .unwrap_or(0)
        .max(12);

    for (index, item) in matches.iter().enumerate() {
        let selected = index == palette.selected;
        let (name_style, description_style) = if selected {
            (theme::selected(), theme::selected())
        } else {
            (theme::bold(), theme::muted())
        };
        let mut spans = vec![
            Span::styled(pad(&item.name, name_width + 2), name_style),
            Span::styled(item.description.clone(), description_style),
        ];
        if selected {
            let used = name_width + 2 + text::width(&item.description);
            spans.push(Span::styled(
                " ".repeat((inner as usize).saturating_sub(used)),
                theme::selected(),
            ));
        }
        body.push(Line::from(spans));
    }
    if matches.is_empty() {
        body.push(Line::from(Span::styled(
            "no matching command".to_owned(),
            theme::faint(),
        )));
    }

    body.push(frame::inner_rule(inner));
    body.push(Line::from(frame::key_hints(&[
        ("↑↓", "select"),
        ("Enter", "open"),
        ("Esc", "close"),
    ])));

    ModalSpec {
        title: "commands".into(),
        badge: String::new(),
        role: ModalRole::Reference,
        width,
        body,
    }
}

// ---------------------------------------------------------------------------
// Contract authorization
// ---------------------------------------------------------------------------

const AUTHORIZE_WIDTH: u16 = 100;
/// Below this width the negative-control column is dropped into the detail line, as the
/// artifact's narrow variants drop optional columns first and state what was dropped.
const AUTHORIZE_WIDE: u16 = 96;

fn authorize_spec(authorize: &Authorize, area: Rect, markers: &Markers) -> ModalSpec {
    let width = AUTHORIZE_WIDTH.min(area.width.saturating_sub(4));
    let inner = width.saturating_sub(2);
    let wide = width >= AUTHORIZE_WIDE;
    let mut body: Vec<Line<'static>> = Vec::new();

    for (label, value) in &authorize.facts {
        let mut spans = vec![Span::styled(pad(label, 11), theme::faint())];
        spans.extend(style::spans(
            &text::truncate(value, (inner as usize).saturating_sub(11)),
            theme::dim(),
        ));
        body.push(Line::from(spans));
    }

    body.push(frame::inner_rule(inner));

    let name_width = authorize
        .requirements
        .iter()
        .map(|requirement| text::width(&requirement.name))
        .max()
        .unwrap_or(0)
        .max(20)
        .min(inner as usize / 2);
    let mut header = vec![
        Span::styled(pad("requirement", name_width + 4), theme::muted()),
        Span::styled(pad("checked by", 20), theme::muted()),
    ];
    if wide {
        header.push(Span::styled("negative control".to_owned(), theme::muted()));
    }
    body.push(Line::from(header));

    for requirement in &authorize.requirements {
        let (marker, style) = match requirement.state {
            RequirementState::Covered => (markers.ok, theme::green()),
            RequirementState::Warning => (markers.warn, theme::amber()),
            RequirementState::Blocking => (markers.fail, theme::red()),
        };
        let mut row = vec![
            Span::styled(format!("{marker} "), style),
            Span::styled(
                pad(
                    &text::truncate(&requirement.name, name_width),
                    name_width + 2,
                ),
                theme::dim(),
            ),
            Span::styled(pad(&requirement.checked_by, 20), theme::muted()),
        ];
        if wide {
            row.push(Span::styled(requirement.negative_control.clone(), style));
        }
        body.push(Line::from(row));

        let mut detail = String::new();
        if !wide {
            detail.push_str(&format!(
                "negative control: {} · ",
                requirement.negative_control
            ));
        }
        if let Some(text) = &requirement.detail {
            detail.push_str(text);
        }
        if !detail.is_empty() {
            for (index, piece) in text::wrap(&detail, inner as usize - 4)
                .into_iter()
                .enumerate()
            {
                body.push(Line::from(vec![
                    Span::styled(
                        if index == 0 {
                            format!("  {} ", markers.fix)
                        } else {
                            "    ".to_owned()
                        },
                        theme::faint(),
                    ),
                    Span::styled(
                        piece,
                        match requirement.state {
                            RequirementState::Blocking => theme::red(),
                            _ => theme::muted(),
                        },
                    ),
                ]));
            }
        }
    }
    if !wide {
        body.push(Line::from(Span::styled(
            "  hidden: negative control — stated in the fix line above".to_owned(),
            theme::faint(),
        )));
    }

    body.push(frame::inner_rule(inner));
    body.push(Line::from(Span::styled(
        authorize.footer_note.clone(),
        theme::muted(),
    )));
    body.push(Line::default());

    // The action is drawn as enabled only when the projection carries one; otherwise it states
    // exactly why it is not available.
    let enabled = authorize.action.is_some();
    let action = vec![
        Span::styled(
            " [ authorize ] ".to_owned(),
            if enabled {
                theme::accent_bold().bg(theme::PANEL)
            } else {
                theme::faint().bg(theme::PANEL)
            },
        ),
        Span::styled(
            text::truncate(
                &format!("  {}", authorize.action_note),
                (inner as usize).saturating_sub(24),
            ),
            if enabled {
                theme::green()
            } else {
                theme::amber()
            },
        ),
    ];
    let hints = if enabled {
        frame::key_hints(&[("Enter", "confirm"), ("Esc", "back")])
    } else {
        frame::key_hints(&[("Esc", "back")])
    };
    body.push(frame::row_line(inner, &action, &hints));

    ModalSpec {
        title: format!("authorize contract {}", authorize.contract),
        badge: authorize.badge.clone(),
        role: ModalRole::Decision,
        width,
        body,
    }
}

// ---------------------------------------------------------------------------
// Typed confirmation
// ---------------------------------------------------------------------------

const CONFIRM_WIDTH: u16 = 76;

fn confirm_spec(confirm: &Confirm, markers: &Markers) -> ModalSpec {
    let inner = CONFIRM_WIDTH - 2;
    let mut body: Vec<Line<'static>> = confirm
        .consequences
        .iter()
        .flat_map(|consequence| {
            text::wrap(consequence, inner as usize)
                .into_iter()
                .map(|piece| Line::from(style::spans(&piece, theme::dim())))
        })
        .collect();

    body.push(Line::default());
    // The identifier lives in the body, not only in the border title: the operator has to read
    // it in full to type it, and a border title yields to the irreversibility badge.
    for piece in text::wrap(
        &format!("{} {}", confirm.prompt_label, confirm.required),
        inner as usize,
    ) {
        body.push(Line::from(style::spans(&piece, theme::dim())));
    }
    body.push(Line::from(vec![
        Span::styled(format!("{} ", markers.prompt), theme::amber()),
        Span::styled(confirm.typed.clone(), theme::text()),
        Span::styled("▁".to_owned(), theme::accent()),
    ]));

    let exact = confirm.is_exact();
    let left = vec![
        Span::styled("Enter".to_owned(), theme::accent()),
        Span::styled(
            format!(
                " {}",
                if exact {
                    "confirm"
                } else {
                    confirm.confirm_hint.as_str()
                }
            ),
            if exact {
                theme::green()
            } else {
                theme::faint()
            },
        ),
    ];
    let right = vec![
        Span::styled("Esc".to_owned(), theme::accent()),
        Span::styled(format!(" {}", confirm.cancel_hint), theme::muted()),
    ];
    body.push(frame::row_line(inner, &left, &right));

    ModalSpec {
        title: confirm.title.clone(),
        badge: confirm.badge.clone(),
        role: ModalRole::Decision,
        width: CONFIRM_WIDTH,
        body,
    }
}

// ---------------------------------------------------------------------------
// Key map
// ---------------------------------------------------------------------------

/// Every keyboard transition the interface implements. The overlay is the map of this table,
/// so a binding cannot exist without appearing here.
pub const KEY_GROUPS: &[(&str, &[(&str, &str)])] = &[
    (
        "global",
        &[
            (":", "command line"),
            ("?", "this key map"),
            ("Esc", "close / back / transcript"),
            ("q", "quit"),
        ],
    ),
    (
        "transcript",
        &[
            ("↑↓ PgUp PgDn", "scroll · pauses follow"),
            ("End", "resume live follow"),
            ("Enter", "send the input line"),
        ],
    ),
    (
        "data pages",
        &[
            ("↑↓ j k", "select row"),
            ("PgUp PgDn", "page the selection"),
            ("Enter", "describe the selection"),
        ],
    ),
    (
        "modals",
        &[
            ("Enter", "confirm — only when enabled"),
            ("Esc", "always the safe exit"),
        ],
    ),
];

fn keys_spec() -> ModalSpec {
    let mut body: Vec<Line<'static>> = Vec::new();
    for (index, (group, keys)) in KEY_GROUPS.iter().enumerate() {
        if index > 0 {
            body.push(Line::default());
        }
        body.push(Line::from(Span::styled(
            (*group).to_owned(),
            theme::muted(),
        )));
        for (key, label) in *keys {
            body.push(Line::from(vec![
                Span::styled(pad(key, 16), theme::accent()),
                Span::styled((*label).to_owned(), theme::dim()),
            ]));
        }
    }
    body.push(Line::default());
    body.push(Line::from(Span::styled(
        "no mouse — every action is reachable from this map".to_owned(),
        theme::faint(),
    )));

    ModalSpec {
        title: "keys".into(),
        badge: "? or Esc to close".into(),
        role: ModalRole::Reference,
        width: 64,
        body,
    }
}
