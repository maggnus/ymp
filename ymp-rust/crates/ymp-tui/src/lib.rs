//! The ymp terminal interface.
//!
//! A chat-first layout with a right sidebar: the conversation fills the main column, and
//! the sidebar carries the current session's context and live team activity.
//! The event loop here does three things and nothing else. It applies runtime events to
//! [`state::App`], it executes the [`state::Action`]s the keyboard produced, and it decides
//! when a frame is worth drawing. Everything else lives in a module of its own.

use anyhow::{bail, Result};
use crossterm::event::Event;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use ymp_core::{Config, UiEvent};
use ymp_runtime::{Engine, RunOutcome};
use ymp_storage::Store;
use ymp_workspace::git::GitError;

mod commands;
mod diff;
mod exit;
mod files;
mod frame;
mod git_view;
mod highlight;
pub mod label;
mod prefs;
mod provenance;
mod sidebar;
mod state;
mod table;
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

    let (mut terminal, guard) = terminal::enter()?;
    let mut input = terminal::input();
    let (events, mut runtime_events) = mpsc::unbounded_channel();

    let mut running: Option<JoinHandle<Result<RunOutcome>>> = None;
    // A scan is not a run: it asks each installation what it offers and sends no prompt. It is
    // spawned so the window keeps painting while a provider takes its time.
    type ScanResult = Result<(Config, ymp_providers::discovery::CatalogScanReport)>;
    let mut scanning: Option<JoinHandle<ScanResult>> = None;
    let mut scan_cancel = CancellationToken::new();
    // The Git page's one admitted request. Its library work cannot be stopped part way, so no
    // second request starts beside it, whatever the page does meanwhile.
    let mut git_job: Option<JoinHandle<git_view::Reply>> = None;
    let mut cancel = CancellationToken::new();
    let mut ticker = tokio::time::interval(TICK);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let epoch = Instant::now();
    let mut last_draw = Instant::now() - FRAME_BUDGET;
    let mut quit = false;

    let departure = loop {
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
        app.expire_exit_request(Instant::now());

        if scanning.as_ref().is_some_and(JoinHandle::is_finished) {
            if let Some(handle) = scanning.take() {
                match handle.await {
                    Ok(Ok((config, report))) => {
                        let read = report
                            .providers
                            .iter()
                            .filter(|provider| provider.status == "updated")
                            .count();
                        let offerings: usize = report.providers.iter().map(|p| p.model_count).sum();
                        app.adopt_config(config);
                        app.status = "Ready".into();
                        app.notice(format!(
                            "Catalog scan: {read} of {} providers updated; entries stored: {offerings}; agents added: {}; existing agents resolved to a native model: {}. Configured names unchanged; no model was invoked.",
                            report.providers.len(),
                            report.created_agents.len(),
                            report.migrated_agents.len()
                        ));
                    }
                    Ok(Err(error)) => {
                        app.status = "Ready".into();
                        app.fail(format!("Catalog scan failed: {error:#}"));
                    }
                    Err(error) => {
                        app.status = "Ready".into();
                        app.fail(format!("The scan task failed: {error}"));
                    }
                }
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

        if git_job.as_ref().is_some_and(JoinHandle::is_finished) {
            if let Some(handle) = git_job.take() {
                match handle.await {
                    Ok(reply) => app.git_reply(reply),
                    Err(error) => app.git_abandoned(format!("The Git task failed: {error}")),
                }
            }
        }
        if git_job.is_none() {
            if let Some(request) = app.git_request(Instant::now()) {
                git_job = Some(spawn_git(&store, request, running.is_some()));
            }
        }

        for action in actions {
            if matches!(
                action,
                Action::StartRun { .. } | Action::FollowUp { .. } | Action::Resume { .. }
            ) {
                if let Some(branch) = app.git.switching() {
                    let message = format!(
                        "A switch to {} is in progress. Runs can start once it has finished.",
                        text::sanitize(branch)
                    );
                    app.fail(message);
                    continue;
                }
            }
            match action {
                Action::Quit => {
                    // Nothing typed after leaving was committed is started.
                    quit = true;
                    break;
                }
                Action::Cancel => {
                    cancel.cancel();
                    scan_cancel.cancel();
                    app.status = "Stopping active turns".into();
                }
                Action::RefreshCatalog { provider } => {
                    if scanning.is_some() {
                        app.fail("A catalog reading is already running.");
                        continue;
                    }
                    if running.is_some() {
                        app.fail("A run is active. Stop it with /stop before reading catalogs.");
                        continue;
                    }
                    scan_cancel = CancellationToken::new();
                    let token = scan_cancel.clone();
                    let mut config = app.config.clone();
                    let home = store.home.clone();
                    let cwd = path.clone();
                    let store = store.clone();
                    let options = ymp_providers::discovery::ScanOptions {
                        provider: provider.clone(),
                        ..Default::default()
                    };
                    app.status = match &provider {
                        Some(id) => format!("Asking {id} what it offers"),
                        None => "Asking the installations what they offer".into(),
                    };
                    app.dirty = true;
                    scanning = Some(tokio::spawn(async move {
                        // The bridge path the runtime resolves, so a scan reaches an
                        // installation exactly the way a turn would.
                        let (events, _ignored) = mpsc::unbounded_channel();
                        let engine = Engine::new(store, config.clone(), events, token.clone())?;
                        let report = ymp_providers::discovery::refresh_catalog(
                            &mut config,
                            &home,
                            &cwd,
                            &engine.bridge,
                            options,
                            token,
                        )
                        .await?;
                        Ok((config, report))
                    }));
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
            // Stopping a run can take seconds, so the window says what it waits on first.
            app.exit_requested = None;
            app.status = exit::closing_status(running.is_some(), scanning.is_some());
            let _ = terminal.draw(|frame| ui::render(frame, &mut app));
            break exit::stop_and_wait(
                &cancel,
                &scan_cancel,
                running.take(),
                scanning.take(),
                &mut runtime_events,
                app.session.clone(),
                exit::SHUTDOWN_WAIT,
            )
            .await;
        }

        if app.dirty && last_draw.elapsed() >= FRAME_BUDGET {
            terminal.draw(|frame| ui::render(frame, &mut app))?;
            // A frame that left work for later is drawn again on a following pass.
            app.dirty = std::mem::take(&mut app.redraw);
            last_draw = Instant::now();
        }
    };
    // Printed once the terminal is restored, so the command stays in the shell's scrollback
    // instead of vanishing with the alternate screen.
    drop(input);
    drop(terminal);
    drop(guard);
    let mut out = std::io::stdout();
    let _ = out.write_all(exit::farewell(&store, &departure).as_bytes());
    let _ = out.flush();
    Ok(())
}

/// Hand one request to the Git backend. A branch switch is refused while a run is active, and
/// otherwise takes the lock a run of that directory takes and moves it into the backend, which
/// keeps it until the checkout has ended.
fn spawn_git(
    store: &Store,
    request: git_view::Request,
    run_active: bool,
) -> JoinHandle<git_view::Reply> {
    let mut lock = None;
    if let git_view::Request::Switch { root, branch, .. } = &request {
        let refused = |message: String| git_view::Reply::Switch {
            branch: branch.clone(),
            result: Err(GitError::Failed(message)),
        };
        if run_active {
            let reply =
                refused("A run is active. Stop it with /stop before switching branches.".into());
            return tokio::spawn(async move { reply });
        }
        match store
            .project(root)
            .and_then(|project| store.lock_project(&project.id))
        {
            Ok(held) => lock = Some(held),
            Err(error) => {
                let reply = refused(format!("{error:#}"));
                return tokio::spawn(async move { reply });
            }
        }
    }
    tokio::spawn(git_view::perform(request, lock))
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
