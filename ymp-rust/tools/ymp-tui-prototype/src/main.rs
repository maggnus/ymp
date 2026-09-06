#![forbid(unsafe_code)]

mod actions;
mod app;
mod entities;
mod forms;
mod management;
mod projection;
mod resources;
mod simulation;
mod snapshot;
mod storage;
mod ui;

use std::{io, path::PathBuf, time::Duration};

use anyhow::{Context, Result, bail};
use crossterm::{
    event::{
        self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, KeyEventKind, MouseButton, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

struct ScreenGuard;
impl Drop for ScreenGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            DisableBracketedPaste,
            DisableMouseCapture,
            LeaveAlternateScreen,
            crossterm::cursor::Show
        );
    }
}

fn main() -> Result<()> {
    let mut output = None;
    let mut state_path = None;
    let mut ephemeral = false;
    let mut width = 144;
    let mut height = 44;
    let mut count = None;
    let mut scene = None;
    let mut detail = false;
    let mut form_preview = None;
    let mut actions_preview = false;
    let mut expand_tools = false;
    let mut view = None;
    let mut query = String::new();
    let mut selected_row = 0;
    let mut empty = false;
    let mut commands = false;
    let mut context = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--state" => {
                state_path = Some(PathBuf::from(
                    args.next().context("--state needs a file path")?,
                ))
            }
            "--ephemeral" => ephemeral = true,
            "--snapshot" => {
                output = Some(PathBuf::from(
                    args.next().context("--snapshot needs an SVG path")?,
                ))
            }
            "--width" => {
                width = args
                    .next()
                    .context("--width needs a number")?
                    .parse::<u16>()?
            }
            "--height" => {
                height = args
                    .next()
                    .context("--height needs a number")?
                    .parse::<u16>()?
            }
            "--agent-count" | "--stress" => {
                count = Some(
                    args.next()
                        .context("--stress needs a number")?
                        .parse::<usize>()?,
                );
            }
            "--scenario" => scene = Some(args.next().context("--scenario needs a scene")?),
            "--detail" => detail = true,
            "--form" => {
                form_preview = Some(
                    args.next()
                        .context("--form needs provider, task or agent")?,
                )
            }
            "--actions" => actions_preview = true,
            "--expand-tools" => expand_tools = true,
            "--view" => {
                let name = args.next().context("--view needs a resource name")?;
                view =
                    Some(resources::Kind::parse(&name).context(
                        "expected agents, tasks, board, providers, activity or sessions",
                    )?);
            }
            "--query" => query = args.next().context("--query needs text")?,
            "--row" => {
                selected_row = args
                    .next()
                    .context("--row needs a zero-based index")?
                    .parse::<usize>()?
            }
            "--empty" => empty = true,
            "--commands" => commands = true,
            "--context" => context = true,
            "--knowledge" => view = Some(resources::Kind::Board),
            "--version" | "-V" => {
                println!("ymp-prototype {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "ymp-prototype — persistent lifecycle simulation\n\n--state FILE      Checkpoint location (default ~/.ymp-prototype/state.json)\n--ephemeral       Do not save this preview\n--scenario NAME   empty, provider, clarify, ready, starting, working, failed, checking, review, done\n--stress N        Explicit large-table fixture (1..10000); no agents are started\n--view NAME       agents, tasks, board, providers, tools, checks, files\n--query TEXT      Initial table filter\n--row N           Selected row\n--detail          Show selected row details\n--form NAME       Preview an entity form\n--actions         Preview entity actions\n--expand-tools    Expand transcript tool output\n--commands        Show slash commands\n--context         Show task context\n--empty           Empty task\n--snapshot FILE   Export actual terminal cells to SVG and text\n--width N         Snapshot columns\n--height N        Snapshot rows\n\n/ commands · Ctrl F filter · ? help · Ctrl Q quit\nNo model or external command is executed. Checkpoints and exported reports are real local files."
                );
                return Ok(());
            }
            _ => bail!("unknown option: {arg}; use --help"),
        }
    }
    if count.is_some_and(|n| !(1..=10_000).contains(&n)) {
        bail!("--stress must be between 1 and 10000");
    }
    let fixture = scene.is_some() || count.is_some() || output.is_some();
    let store = if ephemeral || (fixture && state_path.is_none()) {
        None
    } else {
        Some(storage::Store::open(
            state_path.unwrap_or(storage::Store::default_path()?),
        )?)
    };
    let existing = store.as_ref().map(|s| s.load()).transpose()?.flatten();
    if fixture && existing.is_some() {
        bail!(
            "A fixture cannot replace an existing checkpoint. Use a fresh --state path or --ephemeral."
        );
    }
    let mut app = if let Some((sim, ui)) = existing {
        app::App::from_checkpoint(sim, ui)
    } else if let Some(scene) = scene {
        app::App::scenario_scene(&scene).map_err(anyhow::Error::msg)?
    } else if let Some(count) = count {
        app::App::demo(count)
    } else {
        app::App::new()
    };
    if empty {
        app.execute_command("/new");
    }
    if let Some(kind) = view {
        app.open_table(kind, &query);
        if let Some(table) = &mut app.table {
            table.navigate(selected_row.min(10_000) as isize);
        }
    }
    if detail
        && let Some(table) = &app.table
        && let Some(&index) = table.matches.get(table.selected)
    {
        app.layers
            .push(app::Layer::Detail(table.data.records[index].id.clone()));
    }
    if let Some(form) = form_preview {
        let action = match form.as_str() {
            "provider" => forms::Action::AddProvider,
            "task" => forms::Action::AddTask,
            "agent" => forms::Action::AddAgent,
            _ => bail!("Unknown form"),
        };
        app.perform(action, false);
    }
    if actions_preview {
        app.open_actions();
    }
    if expand_tools && let Some(s) = &app.simulation {
        app.expanded_tools = s.tools.iter().map(|t| t.id.clone()).collect();
    }
    if commands {
        app.open_commands();
    }
    if context {
        app.layers.push(app::Layer::Services);
    }
    if let Some(path) = output {
        if !(32..=300).contains(&width) || !(12..=120).contains(&height) {
            bail!("snapshot size must be within 32..300 columns and 12..120 rows");
        }
        return snapshot::save(&mut app, &path, width, height);
    }

    enable_raw_mode()?;
    let _guard = ScreenGuard;
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        EnableBracketedPaste,
        EnableMouseCapture
    )?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    let started = std::time::Instant::now();
    let mut last_saved: Option<(u64, storage::UiState)> = None;
    let initial_time = app.simulation.as_ref().map_or(0, |s| s.now);
    while !app.quit {
        terminal.draw(|frame| ui::draw(frame, &mut app))?;
        if event::poll(Duration::from_millis(120))? {
            match event::read()? {
                Event::Key(key) if key.kind != KeyEventKind::Release => app.key(key),
                Event::Paste(text) => app.paste(&text),
                Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                    app.click_action(mouse.column, mouse.row)
                }
                _ => {}
            }
        }
        for effect in std::mem::take(&mut app.effects) {
            let result = if let Some(store) = &store {
                let sim = app.simulation.as_mut().context("No simulation to export")?;
                match effect {
                    app::Effect::Export { kind, id } => store
                        .export(sim, &kind, &id)
                        .map(|path| format!("Exported {}", path.display())),
                    app::Effect::RemoveExport(id) => store
                        .remove_export(sim, &id)
                        .map(|_| "Export removed".into()),
                }
            } else {
                Err(anyhow::anyhow!("This preview does not write reports."))
            };
            match result {
                Ok(message) => {
                    app.sync();
                    app.notice(&message);
                }
                Err(error) => app.notice(&error.to_string()),
            }
        }
        app.advance(
            initial_time.saturating_add(started.elapsed().as_millis().min(u64::MAX as u128) as u64),
        );
        if let Some(store) = &store
            && let Some(sim) = &app.simulation
        {
            let ui = app.ui_state();
            if app.quit
                || last_saved
                    .as_ref()
                    .is_none_or(|(generation, state)| *generation != sim.generation || state != &ui)
            {
                match store.save(sim, &ui) {
                    Ok(()) => {
                        last_saved = Some((sim.generation, ui));
                        app.persistence_error = None;
                    }
                    Err(error) => {
                        app.quit = false;
                        app.persistence_error = Some(format!("State not saved: {error}"));
                    }
                }
            }
        }
    }
    Ok(())
}
