//! The ymp terminal interface.
//!
//! A chat-first layout with a right sidebar: the conversation fills the main column, and
//! the sidebar carries navigation, live team activity and the current session's context.
//! The event loop here does three things and nothing else. It applies runtime events to
//! [`state::App`], it executes the [`state::Action`]s the keyboard produced, and it decides
//! when a frame is worth drawing. Everything else lives in a module of its own.

use anyhow::{bail, Result};
use crossterm::event::Event;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use ymp_core::{Config, UiEvent};
use ymp_runtime::{Engine, RunOutcome};
use ymp_storage::Store;

mod commands;
mod frame;
mod prefs;
mod provenance;
mod sidebar;
mod state;
mod terminal;
mod text;
mod theme;
mod transcript;
mod ui;
mod usage;
mod views;

#[cfg(test)]
mod tests;

use state::{Action, App};

/// Shortest gap between frames. Redraws coalesce so a burst of streamed text cannot
/// out-run typing; pending key presses are always applied before a frame is drawn.
const FRAME_BUDGET: Duration = Duration::from_millis(16);
/// How often the loop wakes to advance animation and the elapsed clock.
const TICK: Duration = Duration::from_millis(40);

/// Run the interface until the user leaves.
pub async fn run(
    store: Store,
    config: Config,
    path: PathBuf,
    resume: Option<String>,
) -> Result<()> {
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        bail!("An interactive terminal is required. Use `ymp run` for headless execution.");
    }
    let mut app = App::new(store.clone(), config, path.clone());
    if let Some(id) = resume {
        match app.load_session(&id) {
            Ok(()) => app.notice(
                "Session loaded for reading. Send a message to continue the conversation, or use /resume to continue the run.",
            ),
            Err(error) => app.fail(format!("The session could not be read: {error:#}")),
        }
    }

    let (mut terminal, _guard) = terminal::enter()?;
    let mut input = terminal::input();
    let (events, mut runtime_events) = mpsc::unbounded_channel();

    let mut running: Option<JoinHandle<Result<RunOutcome>>> = None;
    let mut cancel = CancellationToken::new();
    let mut ticker = tokio::time::interval(TICK);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let epoch = Instant::now();
    let mut last_draw = Instant::now() - FRAME_BUDGET;
    let mut quit = false;

    loop {
        let mut actions: Vec<Action> = Vec::new();
        tokio::select! {
            Some(event) = runtime_events.recv() => app.event(event),
            event = input.events.recv() => match event {
                Some(event) => actions.extend(handle(&mut app, event)),
                // The terminal went away. Leave rather than spin on the tick.
                None => quit = true,
            },
            _ = ticker.tick() => {}
        }
        // Drain everything that has already arrived before drawing, so a backlog of
        // streamed deltas never delays the next key press.
        while let Ok(event) = runtime_events.try_recv() {
            app.event(event);
        }
        while let Ok(event) = input.events.try_recv() {
            actions.extend(handle(&mut app, event));
        }

        let spin = (epoch.elapsed().as_millis() / 140) as u64;
        if spin != app.tick {
            app.tick = spin;
            if app.active {
                app.dirty = true;
            }
        }

        if running.as_ref().is_some_and(JoinHandle::is_finished) {
            if let Some(handle) = running.take() {
                app.active = false;
                match handle.await {
                    Ok(Ok(outcome)) => {
                        app.session_status = outcome.session.status.clone();
                        app.status = format!(
                            "Run {}. Files are in {}.",
                            outcome.session.status,
                            outcome.workspace.display()
                        );
                    }
                    Ok(Err(error)) => {
                        app.status = "The run stopped".into();
                        app.fail(format!("{error:#}"));
                    }
                    Err(error) => {
                        app.status = "The run stopped".into();
                        app.fail(format!("The run task failed: {error}"));
                    }
                }
                app.dirty = true;
            }
        }

        for action in actions {
            match action {
                Action::Quit => quit = true,
                Action::Cancel => {
                    cancel.cancel();
                    app.status = "Stopping active turns".into();
                }
                Action::QueueMessage { session, text } => {
                    match store.message(&session, "you", None, "user", &text) {
                        Ok(message) => app.event(UiEvent::Message(message)),
                        Err(error) => app.fail(format!("The message was not saved: {error:#}")),
                    }
                }
                Action::StartRun { prompt } => {
                    if running.is_some() {
                        app.fail("A run is already active. Use /stop first.");
                        continue;
                    }
                    match start(&store, &app.config, &events, &mut cancel) {
                        Ok(engine) => {
                            let cwd = path.clone();
                            app.active = true;
                            app.started = Some(Instant::now());
                            app.status = "Starting the team".into();
                            running = Some(tokio::spawn(async move {
                                engine.run(&cwd, &prompt, None).await
                            }));
                        }
                        Err(error) => app.fail(format!("{error:#}")),
                    }
                }
                Action::FollowUp { session, prompt } => {
                    if running.is_some() {
                        app.fail("A run is already active. Use /stop first.");
                        continue;
                    }
                    match start(&store, &app.config, &events, &mut cancel) {
                        Ok(engine) => {
                            let cwd = path.clone();
                            app.active = true;
                            app.started = Some(Instant::now());
                            app.status = "Continuing the conversation".into();
                            running = Some(tokio::spawn(async move {
                                engine.follow_up(&cwd, &prompt, &session).await
                            }));
                        }
                        Err(error) => app.fail(format!("{error:#}")),
                    }
                }
                Action::Resume { session } => {
                    if running.is_some() {
                        app.fail("A run is already active. Use /stop first.");
                        continue;
                    }
                    if let Err(error) = app.load_session(&session) {
                        app.fail(format!("The session could not be read: {error:#}"));
                        continue;
                    }
                    app.set_view(views::View::Chat);
                    match start(&store, &app.config, &events, &mut cancel) {
                        Ok(engine) => {
                            let cwd = path.clone();
                            app.active = true;
                            app.started = Some(Instant::now());
                            app.status = "Continuing the interrupted run".into();
                            running = Some(tokio::spawn(async move {
                                engine.run(&cwd, "", Some(&session)).await
                            }));
                        }
                        Err(error) => app.fail(format!("{error:#}")),
                    }
                }
            }
        }

        if quit {
            cancel.cancel();
            if let Some(handle) = running.take() {
                let _ = tokio::time::timeout(Duration::from_secs(10), handle).await;
            }
            break;
        }

        if app.dirty && last_draw.elapsed() >= FRAME_BUDGET {
            terminal.draw(|frame| ui::render(frame, &mut app))?;
            app.dirty = false;
            last_draw = Instant::now();
        }
    }
    Ok(())
}

/// Build an engine for a fresh cancellation scope.
fn start(
    store: &Store,
    config: &Config,
    events: &mpsc::UnboundedSender<UiEvent>,
    cancel: &mut CancellationToken,
) -> Result<Engine> {
    *cancel = CancellationToken::new();
    Engine::new(
        store.clone(),
        config.clone(),
        events.clone(),
        cancel.clone(),
    )
}

/// Turn one terminal event into state changes and the actions they imply.
fn handle(app: &mut App, event: Event) -> Vec<Action> {
    match event {
        Event::Key(key) => {
            let width = app.viewport.width as u16;
            app.on_key(key, width)
        }
        Event::Paste(text) => {
            app.paste(&text);
            Vec::new()
        }
        Event::Resize(..) => {
            app.dirty = true;
            Vec::new()
        }
        _ => Vec::new(),
    }
}
