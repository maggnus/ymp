//! Terminal setup, restoration and the panic hook.
//!
//! Restoration must survive a panic: without a hook the process leaves the terminal in raw mode
//! and inside the alternate screen, and the panic message itself lands somewhere unreadable.

use std::io::{self, Stdout, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Context as _;
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{cursor, execute};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

static ENTERED: AtomicBool = AtomicBool::new(false);

/// Owns the terminal mode for as long as the application runs.
pub struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalGuard {
    pub fn enter() -> anyhow::Result<Self> {
        enable_raw_mode().context("enable raw mode")?;
        let mut stdout = io::stdout();
        execute!(
            stdout,
            EnterAlternateScreen,
            EnableBracketedPaste,
            cursor::Hide
        )
        .context("enter alternate screen")?;
        ENTERED.store(true, Ordering::SeqCst);

        install_panic_hook();

        let terminal =
            Terminal::new(CrosstermBackend::new(io::stdout())).context("create terminal")?;
        Ok(Self { terminal })
    }

    pub fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore();
    }
}

/// Put the terminal back the way it was. Safe to call more than once and from a panicking
/// thread: it never allocates a lock and never panics itself.
pub fn restore() {
    if !ENTERED.swap(false, Ordering::SeqCst) {
        return;
    }
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        LeaveAlternateScreen,
        DisableBracketedPaste,
        cursor::Show
    );
    let _ = disable_raw_mode();
    let _ = stdout.flush();
}

/// Restore first, then let the previous hook print — so the panic message is readable.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        previous(info);
    }));
}
