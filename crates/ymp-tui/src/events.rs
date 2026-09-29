//! Terminal event loop. Provider work runs on the session thread, so this loop
//! only reads what was published and never waits for a provider. The terminal is
//! restored on normal exit, on a returned error and on a panic of this loop. A
//! panic of the session thread is kept as a fault and shown by the interface; a
//! panic of any other thread is left to the runtime that owns that thread.
use crate::{
    app::{App, Host, Leaving},
    screens,
};
use ratatui::crossterm::{
    event::{self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEventKind},
    execute,
};
use std::{
    sync::{Mutex, Once},
    time::Duration,
};
use ymp_runtime::{
    domain::{Denial, Result},
    live_session::THREAD,
};

/// Time between frames while nothing is typed.
const FRAME: Duration = Duration::from_millis(50);

struct Restore;
impl Drop for Restore {
    fn drop(&mut self) {
        let _ = execute!(std::io::stdout(), DisableBracketedPaste);
        ratatui::restore();
    }
}
fn unavailable(_: std::io::Error) -> Denial {
    Denial::new("terminal", "The terminal is unavailable")
}
static FAULT: Mutex<Option<String>> = Mutex::new(None);
static HOOK: Once = Once::new();

/// Where the session thread panicked, taken once.
pub fn fault() -> Option<String> {
    FAULT.lock().ok().and_then(|mut fault| fault.take())
}
/// The hook installed by `init` restores the terminal and prints. That is right
/// for this loop only: after a panic of another thread the interface keeps
/// running and needs its terminal.
fn keep_session_faults() {
    HOOK.call_once(|| {
        let interface = std::thread::current().id();
        let restoring = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |panic| {
            if std::thread::current().id() == interface {
                restoring(panic);
            } else if std::thread::current().name() == Some(THREAD)
                && let Ok(mut fault) = FAULT.lock()
            {
                *fault = Some(match panic.location() {
                    Some(at) => format!("panic at {}:{}", at.file(), at.line()),
                    None => "panic".into(),
                });
            }
        }));
    });
}
/// Run the interface until the user leaves. Leaving itself records nothing.
pub fn run<H: Host>(host: H) -> Result<Leaving> {
    let mut app = App::new(host);
    // Restoring also covers an initialization that failed halfway.
    let _restore = Restore;
    // `init` also installs a panic hook that restores the terminal first.
    let mut terminal = ratatui::try_init().map_err(unavailable)?;
    keep_session_faults();
    execute!(std::io::stdout(), EnableBracketedPaste).map_err(unavailable)?;
    while !app.quit {
        app.refresh();
        terminal
            .draw(|frame| screens::draw(frame, &mut app))
            .map_err(unavailable)?;
        if !event::poll(FRAME).map_err(unavailable)? {
            continue;
        }
        // Everything already typed is handled before the next frame.
        loop {
            match event::read().map_err(unavailable)? {
                Event::Key(key) if key.kind != KeyEventKind::Release => app.key(key),
                Event::Paste(text) => app.paste(&text),
                _ => {}
            }
            if app.quit || !event::poll(Duration::ZERO).map_err(unavailable)? {
                break;
            }
        }
    }
    Ok(app.leaving())
}
