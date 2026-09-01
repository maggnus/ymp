//! Composition: projections become the specs the frame layer draws.
//!
//! Every full-screen surface is built here into a [`crate::frame::SurfaceSpec`], and every
//! floating surface by [`crate::overlay`] into a [`crate::frame::ModalSpec`]. Nothing in this
//! module writes to the terminal, and nothing in it invents a value: each span carries text
//! that came from the projection or names a key.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};

use crate::frame::{self, SurfaceSpec};
use crate::overlay;
use crate::projection::{self, Environment, RunFacts};
use crate::state::{App, COMMAND_PREFIX, Follow, Surface};
use crate::style;
use crate::text;
use crate::theme::{self, Markers};

pub use crate::frame::{MIN_HEIGHT, MIN_WIDTH};

/// Draw the interface. The single entry point the event loop calls.
pub fn render(target: &mut Frame, app: &App, markers: &Markers) {
    let area = target.area();
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        frame::render_size_guard(target, area, &size_guard_lines(area, markers));
        return;
    }

    let spec = surface_spec(app, area, markers);
    frame::render_surface(target, area, &spec);

    if let Some(modal) = overlay::modal_spec(app, area, markers) {
        frame::render_modal(target, area, &modal);
    }
}

/// The spec for whichever surface currently fills the frame.
pub fn surface_spec(app: &App, area: Rect, markers: &Markers) -> SurfaceSpec {
    let body_area = frame::rows(area).body;
    match app.surface {
        Surface::Transcript => transcript_spec(app, body_area, markers),
        Surface::Page(kind) => match app.page(kind) {
            Some(page) => page_spec(app, page, body_area, markers),
            None => missing_page_spec(kind),
        },
    }
}

// ---------------------------------------------------------------------------
// Transcript
// ---------------------------------------------------------------------------

fn transcript_spec(app: &App, body: Rect, markers: &Markers) -> SurfaceSpec {
    let (header_left, header_right) = context_header(app, body.width);
    SurfaceSpec {
        header_left,
        header_right,
        body: transcript_body(app, body, markers),
        input_left: input_left(app, markers),
        input_right: input_right(app, markers),
        status_left: style::spans(&app.data.status, theme::faint()),
        status_right: vec![
            Span::styled(COMMAND_PREFIX.to_string(), theme::accent()),
            Span::styled("commands".to_owned(), theme::muted()),
            Span::styled("  ".to_owned(), theme::faint()),
            Span::styled("?".to_owned(), theme::accent()),
            Span::styled("keys".to_owned(), theme::muted()),
        ],
    }
}

/// `ymp · <project>` left; contract, run and assurance right. A wide terminal also
/// carries the project path and the assurance profile; a narrow one drops them first.
fn context_header(app: &App, width: u16) -> (Vec<Span<'static>>, Vec<Span<'static>>) {
    let wide = width >= 120;
    let mut left: Vec<Span<'static>> = Vec::new();
    let mut right: Vec<Span<'static>> = Vec::new();

    let Some(environment) = &app.data.environment else {
        return (left, right);
    };

    left.push(Span::styled("ymp".to_owned(), theme::muted()));
    left.push(Span::styled(" · ".to_owned(), theme::faint()));
    left.push(Span::styled(environment.project.clone(), theme::dim()));
    if wide {
        left.push(Span::styled(
            format!(" ({})", environment.project_path.display()),
            theme::faint(),
        ));
    }

    match (app.data.contracts.first(), &app.data.run) {
        (None, None) => right.push(Span::styled(
            "no contract · no run".to_owned(),
            theme::faint(),
        )),
        (contract, run) => {
            if let Some(contract) = contract {
                right.extend(style::spans(&contract.contract_id, theme::muted()));
            }
            match run {
                Some(run) => {
                    if contract.is_some() {
                        right.push(Span::styled(" · ".to_owned(), theme::faint()));
                    }
                    right.extend(run_state_spans(run));
                }
                None => right.push(Span::styled(" · no run".to_owned(), theme::faint())),
            }
        }
    }
    if wide {
        right.push(Span::styled(" · assurance ".to_owned(), theme::faint()));
        right.push(Span::styled(
            format!("{} ▲", environment.assurance_profile),
            theme::amber(),
        ));
    }

    (left, right)
}

fn run_state_spans(run: &RunFacts) -> Vec<Span<'static>> {
    let mut spans = style::spans(&run.run_id, theme::muted());
    spans.push(Span::styled(" ".to_owned(), theme::faint()));
    spans.push(Span::styled(
        projection::outcome_marker(run.status).to_owned(),
        theme::muted(),
    ));
    spans.push(Span::styled(" ".to_owned(), theme::faint()));
    spans.extend(style::spans(
        projection::outcome(run.status),
        theme::muted(),
    ));
    spans
}

/// Top-aligned until the transcript fills the body, then anchored to the newest line. Scrolled
/// back, dashed markers state the position and the way to live.
fn transcript_body(app: &App, area: Rect, markers: &Markers) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    for entry in &app.data.entries {
        lines.extend(entry.layout(area.width, markers));
    }

    let height = area.height as usize;
    let total = lines.len();
    let width = area.width as usize;
    if total <= height {
        return lines;
    }

    let scrolled = app.scroll.min(total - height);
    let end = total - scrolled;
    let start = end.saturating_sub(height);
    let mut window: Vec<Line<'static>> = Vec::with_capacity(height);

    if scrolled > 0 && start > 0 {
        let label = app
            .viewing_around
            .clone()
            .unwrap_or_else(|| format!("{start} earlier lines"));
        window.push(dashed_marker(&format!(" viewing after {label} "), width));
        window.extend(lines[start + 1..end].iter().cloned());
    } else {
        window.extend(lines[start..end].iter().cloned());
    }

    if scrolled > 0 && !window.is_empty() {
        let last = window.len() - 1;
        window[last] = dashed_marker(
            &format!(
                " {} {scrolled} newer · End resume live ",
                markers.more_below
            ),
            width,
        );
    }
    window
}

fn dashed_marker(label: &str, width: usize) -> Line<'static> {
    let label_width = text::width(label);
    let side = width.saturating_sub(label_width) / 2;
    Line::from(vec![
        Span::styled("╌".repeat(side), theme::rule()),
        Span::styled(label.to_owned(), theme::faint()),
        Span::styled(
            "╌".repeat(width.saturating_sub(side + label_width)),
            theme::rule(),
        ),
    ])
}

/// The marks the input row cycles through while something runs away from this thread. They are
/// what makes a redraw visible: the row changes on every heartbeat, so a frozen interface and a
/// waiting one cannot look alike.
const WORKING_MARKS: [&str; 4] = ["·", "‥", "…", "‥"];

/// The row that states what runs away from the thread that draws, while something does.
///
/// It is the same row on every surface: work asked for on a page keeps running while the operator
/// stands on that page, and a page that stated nothing would leave them watching a screen that
/// never changes.
fn working_row(app: &App, markers: &Markers) -> Option<Vec<Span<'static>>> {
    let notice = app.data.working.as_ref()?;
    // The way out comes before the account of what is being waited for. The notice names a path
    // and is as long as that path; the row is as wide as the terminal, so whatever is last is what
    // a narrow row loses, and losing the way out is what must not happen. A wait Esc does not end
    // offers no key: the mark still advances on every heartbeat, which is what tells a wait from a
    // freeze, and no key is named that would change nothing.
    let mut spans = vec![
        Span::styled(format!("{} ", markers.prompt), theme::amber()),
        Span::styled(
            WORKING_MARKS[app.working_ticks % WORKING_MARKS.len()].to_owned(),
            theme::accent(),
        ),
    ];
    if app.data.working_ends_on_esc {
        spans.push(Span::styled(" Esc cancels".to_owned(), theme::accent()));
    }
    spans.push(Span::styled(format!(" · {notice}"), theme::muted()));
    Some(spans)
}

fn input_left(app: &App, markers: &Markers) -> Vec<Span<'static>> {
    // While nothing is being typed, the row states what is running away from this thread. A line
    // being typed outranks that notice: it supersedes the run when it is sent.
    if app.prompt.buffer.is_empty()
        && app.prompt.suspended.is_none()
        && let Some(spans) = working_row(app, markers)
    {
        return spans;
    }
    match &app.prompt.suspended {
        Some(reason) => vec![
            Span::styled(format!("{} ", markers.prompt), theme::faint()),
            Span::styled(reason.clone(), theme::faint()),
        ],
        None => {
            let mut spans = vec![
                Span::styled(format!("{} ", markers.prompt), theme::amber()),
                Span::styled(app.prompt.buffer.clone(), theme::text()),
                Span::styled(markers.cursor.to_owned(), theme::accent()),
            ];
            // While an answer is awaited, the row states which one, so pressing Enter on an
            // empty line is a stated choice rather than a guess.
            if app.prompt.buffer.is_empty()
                && let Some(awaiting) = &app.data.awaiting
            {
                spans.push(Span::styled(format!("  {awaiting}"), theme::faint()));
            }
            spans
        }
    }
}

fn input_right(app: &App, markers: &Markers) -> Vec<Span<'static>> {
    match app.follow {
        Follow::Live => vec![
            Span::styled("follow ".to_owned(), theme::muted()),
            Span::styled(markers.follow_live.to_owned(), theme::green()),
        ],
        Follow::Paused => {
            let mut spans = vec![
                Span::styled("follow ".to_owned(), theme::muted()),
                Span::styled(markers.follow_paused.to_owned(), theme::amber()),
            ];
            if let Some(position) = &app.viewing_around {
                spans.push(Span::styled(format!(" · {position}"), theme::faint()));
            }
            spans
        }
    }
}

// ---------------------------------------------------------------------------
// Data pages
// ---------------------------------------------------------------------------

fn page_spec(app: &App, page: &crate::pages::Page, body: Rect, markers: &Markers) -> SurfaceSpec {
    let mut header_left: Vec<Span<'static>> = Vec::new();
    for (index, segment) in page.breadcrumb.iter().enumerate() {
        let last = index + 1 == page.breadcrumb.len();
        if index > 0 {
            header_left.push(Span::styled(" › ".to_owned(), theme::faint()));
        }
        header_left.push(Span::styled(
            segment.clone(),
            if last {
                theme::accent_bold()
            } else {
                theme::faint()
            },
        ));
    }
    header_left.extend(page.summary.iter().cloned());

    SurfaceSpec {
        header_left,
        header_right: frame::key_hints(&[("Esc", "back")]),
        body: page.layout(body.width, body.height),
        // A page is where the acts that start work are taken — enabling an account is one — so it
        // states what is running under it and advances the same mark the conversation does.
        input_left: working_row(app, markers).unwrap_or_default(),
        input_right: Vec::new(),
        status_left: page.footer.clone(),
        status_right: frame::key_hints(&page.keys),
    }
}

/// A page kind the current state does not populate. It says so rather than drawing nothing.
fn missing_page_spec(kind: crate::state::PageKind) -> SurfaceSpec {
    SurfaceSpec {
        header_left: vec![
            Span::styled("transcript".to_owned(), theme::faint()),
            Span::styled(" › ".to_owned(), theme::faint()),
            Span::styled(kind.command_name().to_owned(), theme::accent_bold()),
            Span::styled(" · unavailable".to_owned(), theme::amber()),
        ],
        header_right: frame::key_hints(&[("Esc", "back")]),
        body: vec![
            Line::default(),
            Line::from(Span::styled(
                "  no state populates this page yet — it fills in as the run records it".to_owned(),
                theme::muted(),
            )),
        ],
        input_left: Vec::new(),
        input_right: Vec::new(),
        status_left: vec![Span::styled("nothing to show".to_owned(), theme::faint())],
        status_right: frame::key_hints(&[("Esc", "back")]),
    }
}

// ---------------------------------------------------------------------------
// Size guard
// ---------------------------------------------------------------------------

fn size_guard_lines(area: Rect, markers: &Markers) -> Vec<Line<'static>> {
    vec![
        Line::from(Span::styled(
            "terminal too small".to_owned(),
            theme::accent_bold(),
        )),
        Line::default(),
        Line::from(vec![
            Span::styled("required  ".to_owned(), theme::muted()),
            Span::styled(format!("{MIN_WIDTH}x{MIN_HEIGHT}"), theme::text()),
        ]),
        Line::from(vec![
            Span::styled("observed  ".to_owned(), theme::muted()),
            Span::styled(
                format!("{}x{} {}", area.width, area.height, markers.warn),
                theme::amber(),
            ),
        ]),
        Line::default(),
        Line::from(Span::styled(
            "resize the window, or".to_owned(),
            theme::muted(),
        )),
        Line::from(vec![
            Span::styled("q".to_owned(), theme::accent()),
            Span::styled(
                " quit safely — the run state stays journaled".to_owned(),
                theme::muted(),
            ),
        ]),
    ]
}

/// The environment header, exposed so a test can assert the header without a rendered buffer.
pub fn header_of(environment: &Environment) -> String {
    format!("ymp · {}", environment.project)
}
