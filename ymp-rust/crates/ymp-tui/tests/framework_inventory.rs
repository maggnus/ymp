//! Acceptance: the interface is one framework, not a set of per-screen implementations.
//!
//! Two halves. The structural half rejects a screen that owns a private drawing path: only
//! `frame.rs` may write into a ratatui frame. The behavioural half renders every declared
//! surface and modal and requires the shared chrome in each, so a surface cannot be added
//! without composing the shared layer.
//!
//! The check that must fail: give any other module a drawing call — for example add
//! `frame.render_widget(Clear, area);` to `src/ui.rs` — and
//! `only_the_frame_layer_draws` reports it with a non-zero exit.

mod support;

use support::{SIZES, app_for, buffer, contract, crate_sources};
use ymp_tui::frame;
use ymp_tui::scenario;
use ymp_tui::state::{Modal, PageKind, Surface};

/// The module allowed to write into a frame. Everything else composes specifications.
const DRAWING_MODULE: &str = "frame.rs";

/// Calls that put something on the screen. Any of them outside the frame layer is a private
/// drawing path.
const DRAWING_CALLS: [&str; 3] = ["render_widget", "buffer_mut", "render_stateful_widget"];

#[test]
fn only_the_frame_layer_draws() {
    let mut offenders = Vec::new();
    for (path, source) in crate_sources() {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name == DRAWING_MODULE {
            continue;
        }
        for (number, line) in source.lines().enumerate() {
            if line.trim_start().starts_with("//") || line.trim_start().starts_with("//!") {
                continue;
            }
            for call in DRAWING_CALLS {
                if line.contains(call) {
                    offenders.push(format!("{name}:{}: {}", number + 1, line.trim()));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "these screens draw outside the shared layer ({DRAWING_MODULE}):\n{}",
        offenders.join("\n")
    );
}

#[test]
fn every_declared_surface_composes_the_shared_frame() {
    let run = scenario::running();
    for (width, height) in SIZES {
        let mut surfaces: Vec<(String, Surface)> = vec![("transcript".into(), Surface::Transcript)];
        for kind in PageKind::ALL {
            surfaces.push((kind.command_name().to_owned(), Surface::Page(kind)));
        }

        for (name, surface) in surfaces {
            let mut app = app_for(Some(&run), vec![contract(true)]);
            if surface == Surface::Page(PageKind::Describe) {
                app.describe_index = Some(0);
                support::rebuild(&mut app, Some(&run), vec![contract(true)]);
            }
            app.surface = surface;
            let lines = buffer(&app, width, height);

            // The shared frame: a header row, two hairlines and a status row, always.
            let rule = "─".repeat(width as usize);
            assert_eq!(lines[1], rule, "{name}: missing the top hairline");
            assert_eq!(
                lines[height as usize - 2],
                rule,
                "{name}: missing the bottom hairline"
            );
            assert!(
                !lines[0].trim().is_empty(),
                "{name}: the header row is empty"
            );
            assert!(
                !lines[height as usize - 1].trim().is_empty(),
                "{name}: the status row is empty"
            );
        }
    }
}

#[test]
fn every_declared_modal_composes_the_shared_floating_surface() {
    let run = scenario::running();
    for (width, height) in SIZES {
        let mut app = app_for(Some(&run), vec![contract(true)]);

        let mut modals: Vec<(&str, Modal)> = Vec::new();
        modals.push(("palette", Modal::Palette(app.open_palette())));
        app.open_authorize();
        modals.push(("authorize", app.modal.clone()));
        app.open_cancel_confirm();
        modals.push(("confirm", app.modal.clone()));
        modals.push(("keys", Modal::Keys));

        for (name, modal) in modals {
            let mut app = app_for(Some(&run), vec![contract(true)]);
            app.modal = modal;
            let lines = buffer(&app, width, height);
            let rendered = lines.join("\n");
            // A floating surface is a bordered box: the frame layer draws the corners.
            assert!(
                rendered.contains('┌') && rendered.contains('┘'),
                "{name} at {width}x{height} is not a bordered box:\n{rendered}"
            );
            // and the surface underneath keeps the shared frame.
            assert!(
                !lines[height as usize - 1].trim().is_empty(),
                "{name}: the status row vanished under the modal"
            );
        }
    }
}

#[test]
fn the_size_guard_is_the_only_surface_without_the_shared_frame() {
    let app = app_for(Some(&scenario::running()), vec![contract(true)]);
    let lines = buffer(&app, frame::MIN_WIDTH - 1, frame::MIN_HEIGHT - 1);
    let rendered = lines.join("\n");
    assert!(rendered.contains("terminal too small"), "{rendered}");
    assert!(
        !rendered.contains('─'),
        "the guard drew the frame it replaces:\n{rendered}"
    );
}
