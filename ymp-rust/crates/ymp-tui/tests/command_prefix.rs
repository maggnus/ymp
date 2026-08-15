//! Acceptance: every command the interface offers is entered with a leading `/`.
//!
//! An operator who knows Claude Code types `/` for a command. The interface therefore opens its
//! command line on that key, prefixes every command it names with it, and no longer reads `:` as
//! anything but a character of the line being typed.
//!
//! The negative half is the build this card started from, where `:` opened the palette and the
//! hints printed `:authorize`: `slash_opens_the_command_line_and_colon_does_not` reports the
//! colon opening a modal, and `no_hint_names_a_command_with_a_colon` reports the printed hints.

mod support;

use crossterm::event::KeyCode;
use support::{SIZES, contract, press, screen};
use ymp_tui::journal::Model;
use ymp_tui::state::{App, Modal};

/// The commands the interface names in its own prose. Each one has to be printed with the
/// prefix an operator would type, and never with the retired one.
const NAMED_COMMANDS: [&str; 3] = ["runtimes", "events", "authorize"];

fn cold_app(contracts: Vec<ymp_tui::projection::ContractFacts>) -> App {
    App::new(Model::cold(support::environment(), contracts).projection(None))
}

#[test]
fn slash_opens_the_command_line_and_colon_does_not() {
    let mut app = cold_app(Vec::new());
    press(&mut app, KeyCode::Char('/'), 40);
    let Modal::Palette(palette) = &app.modal else {
        panic!("the command line did not open on the prefix an operator types");
    };
    assert_eq!(palette.input, "/");
    assert!(
        !palette.matches().is_empty(),
        "the command line opened over no commands"
    );

    let mut app = cold_app(Vec::new());
    press(&mut app, KeyCode::Char(':'), 40);
    assert!(
        matches!(app.modal, Modal::None),
        "the retired prefix still opens a surface"
    );
    assert_eq!(
        app.prompt.buffer, ":",
        "the retired prefix was neither a command nor a character of the line"
    );
}

/// The palette filters on what is typed after the prefix, so `/ru` reaches the runtimes page.
#[test]
fn the_command_line_filters_on_what_follows_the_prefix() {
    let mut app = cold_app(Vec::new());
    press(&mut app, KeyCode::Char('/'), 40);
    for character in "ru".chars() {
        press(&mut app, KeyCode::Char(character), 40);
    }
    let Modal::Palette(palette) = &app.modal else {
        panic!("the command line closed while a command was being typed");
    };
    assert_eq!(palette.input, "/ru");
    let names: Vec<&str> = palette
        .matches()
        .iter()
        .map(|item| item.name.as_str())
        .collect();
    assert_eq!(names, vec!["runtimes"]);
}

/// Every hint the interface prints names its commands with the prefix an operator would type.
#[test]
fn no_hint_names_a_command_with_a_colon() {
    for contracts in [Vec::new(), vec![contract(true)]] {
        let mut app = cold_app(contracts);
        let mut surfaces = vec![screen(&app, 120, 40)];
        app.modal = Modal::Keys;
        surfaces.push(screen(&app, 120, 40));
        app.modal = Modal::Palette(app.open_palette());
        surfaces.push(screen(&app, 120, 40));

        for rendered in surfaces {
            for command in NAMED_COMMANDS {
                assert!(
                    !rendered.contains(&format!(":{command}")),
                    "a hint still names {command} with the retired prefix:\n{rendered}"
                );
            }
        }
    }

    // The hints that name a command on the cold screen name it with the prefix. The transcript
    // carries only the one that changes what to do next; the palette itself is named by the
    // status line.
    let app = cold_app(vec![contract(true)]);
    for (width, height) in SIZES {
        let rendered = screen(&app, width, height);
        assert!(rendered.contains("/authorize"), "{rendered}");
        assert!(rendered.contains("/commands"), "{rendered}");
    }
}
