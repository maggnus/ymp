//! ymp-cli: command parsing, exit codes and subcommands; the interactive interface lives in ymp-tui.
//! Owner: W1-0015.
//! Created by owner decision on 2026-09-16; see ymp-docs/project-worktree.md.
//! A module gains behavior only through its owning task.
//!
//! The subcommands read the durable store and print plain text. They start no
//! session, call no model and never enter the terminal interface.

pub mod compare;
pub mod host;
pub mod store;

use host::{BackendChoice, Options};
use std::{io::IsTerminal, path::PathBuf};
use ymp_runtime::{
    domain::{Denial, Id},
    kernel::finalization::render_report,
};

pub const USAGE: &str = "\
Usage:
  ymp --store DIR --workspace DIR --scripted PROGRAM.json
  ymp --store DIR --workspace DIR --claude EXECUTABLE --model MODEL [--effort EFFORT]
  ymp sessions --store DIR
  ymp report --store DIR SESSION
  ymp --help | --version

The first two forms open the interactive interface. DIR for --store holds the
durable journal and must be outside the workspace. The workspace files are
changed directly. --scripted replays a recorded program and calls no model;
--claude runs the installed Claude Code executable with its own authentication.
`sessions` and `report` start nothing and record nothing about a session; they
refuse a directory that holds no journal.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Interactive(Options),
    Sessions { store: PathBuf },
    Report { store: PathBuf, session: String },
    Help,
    Version,
}
pub const SUCCESS: i32 = 0;
/// The request was understood and refused, or it failed.
pub const REFUSED: i32 = 1;
/// The command line is not a ymp command.
pub const MISUSED: i32 = 2;

fn misuse(text: impl Into<String>) -> String {
    text.into()
}
pub fn parse(arguments: &[String]) -> Result<Command, String> {
    let mut names = std::collections::BTreeMap::new();
    let mut words = vec![];
    let mut rest = arguments.iter();
    while let Some(argument) = rest.next() {
        match argument.as_str() {
            "--help" | "-h" => return Ok(Command::Help),
            "--version" | "-V" => return Ok(Command::Version),
            "--store" | "--workspace" | "--scripted" | "--claude" | "--model" | "--effort" => {
                let value = rest
                    .next()
                    .ok_or_else(|| misuse(format!("{argument} requires a value")))?;
                if names.insert(argument.as_str(), value.clone()).is_some() {
                    return Err(misuse(format!("{argument} is stated twice")));
                }
            }
            other if other.starts_with('-') => {
                return Err(misuse(format!("Unknown option {other}")));
            }
            other => words.push(other),
        }
    }
    let mut take = |name: &str| names.remove(name);
    let store = take("--store").map(PathBuf::from);
    let command = match words.as_slice() {
        [] => {
            let workspace = take("--workspace")
                .map(PathBuf::from)
                .ok_or_else(|| misuse("--workspace is required"))?;
            let backend = match (take("--scripted"), take("--claude")) {
                (Some(program), None) => BackendChoice::Scripted {
                    program: program.into(),
                },
                (None, Some(executable)) => BackendChoice::Claude {
                    executable: executable.into(),
                    model: take("--model").ok_or_else(|| misuse("--claude requires --model"))?,
                    effort: take("--effort"),
                },
                _ => return Err(misuse("State exactly one of --scripted and --claude")),
            };
            Command::Interactive(Options {
                store: store.ok_or_else(|| misuse("--store is required"))?,
                workspace,
                backend,
            })
        }
        ["sessions"] => Command::Sessions {
            store: store.ok_or_else(|| misuse("--store is required"))?,
        },
        ["report", session] => Command::Report {
            store: store.ok_or_else(|| misuse("--store is required"))?,
            session: (*session).into(),
        },
        _ => return Err(misuse("Unknown command")),
    };
    match names.keys().next() {
        Some(name) => Err(misuse(format!("{name} does not apply to this command"))),
        None => Ok(command),
    }
}
/// Recorded text reaches the terminal as text, never as control sequences or
/// marks that reorder what surrounds them.
fn plain(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\n' => '\n',
            '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{202a}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}' => '\u{fffd}',
            c if c.is_control() => '\u{fffd}',
            c => c,
        })
        .collect()
}
fn sessions(store: &std::path::Path) -> Result<(), Denial> {
    let journal = store::journal(store, false)?;
    let found = journal.sessions()?;
    if found.is_empty() {
        println!("The store records no session.");
    }
    for (session, revision) in found {
        let view = store::view(&journal, &session)?;
        let goal = view
            .task()
            .map(|task| task.goal.request.clone())
            .unwrap_or_default();
        println!(
            "{}  revision {revision}  status {}  {}",
            session.as_str(),
            plain(&store::status(&view)),
            plain(&goal.replace('\n', " "))
        );
    }
    Ok(())
}
fn report(store: &std::path::Path, session: &str) -> Result<(), Denial> {
    let journal = store::journal(store, false)?;
    let view = store::view(&journal, &Id::new(session)?)?;
    if !view.opened() {
        return Err(Denial::new(
            "session_missing",
            "The store records no such session",
        ));
    }
    println!(
        "session {} · revision {} · recorded status {}",
        plain(session),
        view.revision(),
        plain(&store::status(&view))
    );
    match &view.finalization().delivered {
        Some(delivered) => {
            println!("{}", plain(&render_report(delivered)));
            Ok(())
        }
        None => Err(Denial::new(
            "report_missing",
            "No report is delivered for this session",
        )),
    }
}
fn interactive(options: Options) -> Result<(), Denial> {
    if !(std::io::stdin().is_terminal() && std::io::stdout().is_terminal()) {
        return Err(Denial::new(
            "terminal",
            "The interactive interface requires a terminal; use the sessions and report commands otherwise",
        ));
    }
    let composition = host::Composition::open(options)?;
    let store = composition.store().to_path_buf();
    let left = ymp_tui::run(composition)?;
    let Some(session) = left.session else {
        println!("No session was opened.");
        return Ok(());
    };
    match (left.revision, left.status) {
        (Some(revision), Some(status)) => println!(
            "Left session {} at revision {revision} with recorded status {}.",
            session.as_str(),
            plain(&status)
        ),
        _ => println!(
            "Left session {} before anything was shown.",
            session.as_str()
        ),
    }
    if let Some(denial) = left.unread {
        println!(
            "The record could not be read again ({}: {}); the revision above is the last one shown.",
            plain(&denial.code),
            plain(&denial.message)
        );
    }
    if left.driving {
        println!(
            "The session had work left and nothing drives it now. Open it again and choose /recover continue or /recover report."
        );
    }
    println!(
        "Read it again with: ymp report --store {} {}",
        plain(&store.display().to_string()),
        session.as_str()
    );
    Ok(())
}
/// Run one command line and return the process exit code.
pub fn run(arguments: &[String]) -> i32 {
    let command = match parse(arguments) {
        Ok(command) => command,
        Err(text) => {
            eprintln!("ymp: {text}\n\n{USAGE}");
            return MISUSED;
        }
    };
    let result = match command {
        Command::Help => {
            println!("{USAGE}");
            Ok(())
        }
        Command::Version => {
            println!("ymp {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::Sessions { store } => sessions(&store),
        Command::Report { store, session } => report(&store, &session),
        Command::Interactive(options) => interactive(options),
    };
    match result {
        Ok(()) => SUCCESS,
        Err(denial) => {
            eprintln!("ymp: {}: {}", plain(&denial.code), plain(&denial.message));
            REFUSED
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn line(words: &[&str]) -> Result<Command, String> {
        parse(
            &words
                .iter()
                .map(|word| word.to_string())
                .collect::<Vec<_>>(),
        )
    }
    #[test]
    fn a_command_line_names_one_command() {
        assert_eq!(
            line(&["--store", "s", "--workspace", "w", "--scripted", "p.json"]),
            Ok(Command::Interactive(Options {
                store: "s".into(),
                workspace: "w".into(),
                backend: BackendChoice::Scripted {
                    program: "p.json".into()
                },
            }))
        );
        assert_eq!(
            line(&["report", "--store", "s", "session-1"]),
            Ok(Command::Report {
                store: "s".into(),
                session: "session-1".into()
            })
        );
        for refused in [
            &["--store", "s", "--workspace", "w"][..],
            &[
                "--store",
                "s",
                "--workspace",
                "w",
                "--scripted",
                "p",
                "--claude",
                "c",
            ],
            &["--store", "s", "--workspace", "w", "--claude", "c"],
            &[
                "--store",
                "s",
                "--workspace",
                "w",
                "--scripted",
                "p",
                "--model",
                "m",
            ],
            &["sessions"],
            &["sessions", "--store", "s", "--workspace", "w"],
            &["report", "--store", "s"],
            &["--store", "s", "--store", "t", "sessions"],
            &["--unknown"],
        ] {
            assert!(line(refused).is_err(), "{refused:?}");
        }
    }
    #[test]
    fn recorded_text_is_printed_as_text() {
        assert_eq!(
            plain("a\u{1b}[31mb\u{202e}c\nd"),
            "a\u{fffd}[31mb\u{fffd}c\nd"
        );
    }
    #[test]
    fn a_store_inside_the_workspace_is_recognized_before_it_exists() {
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join(format!("ymp-cli-store-{}", std::process::id()));
        std::fs::create_dir_all(root.join("work")).unwrap();
        let inside = store::resolve(&root.join("work/a/b/store")).unwrap();
        assert!(inside.starts_with(root.join("work")));
        assert!(!root.join("work/a").exists());
        assert!(store::resolve(&root.join("work/../store")).is_err());
        assert!(store::journal(&root.join("work"), false).is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
