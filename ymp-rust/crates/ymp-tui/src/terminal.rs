//! Terminal lifecycle: raw mode, the alternate screen, the panic hook, and input.
//!
//! Input is read on a dedicated thread and delivered over a channel, so a key press is
//! handled as soon as it arrives instead of waiting for the next frame. The thread polls
//! with a short timeout and stops when the guard is dropped, so returning from `run` leaves
//! the terminal to whatever runs next.

use anyhow::Result;
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyboardEnhancementFlags,
    PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, Stdout};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Restores the terminal however the interface ends, including on a panic.
pub struct Guard {
    enhanced: bool,
}

impl Drop for Guard {
    fn drop(&mut self) {
        restore(self.enhanced);
    }
}

fn restore(enhanced: bool) {
    let mut out = io::stdout();
    if enhanced {
        let _ = execute!(out, PopKeyboardEnhancementFlags);
    }
    let _ = execute!(out, DisableBracketedPaste, LeaveAlternateScreen);
    let _ = disable_raw_mode();
}

/// Enter the alternate screen and take over the keyboard.
pub fn enter() -> Result<(Terminal<CrosstermBackend<Stdout>>, Guard)> {
    enable_raw_mode()?;
    let mut out = io::stdout();
    execute!(out, EnterAlternateScreen, EnableBracketedPaste)?;
    // Disambiguated escapes let Esc act immediately instead of waiting for a sequence.
    let enhanced = execute!(
        out,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
    )
    .is_ok();
    let guard = Guard { enhanced };
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore(enhanced);
        previous(info);
    }));
    let terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;
    Ok((terminal, guard))
}

/// A terminal input stream delivered over a channel.
pub struct Input {
    pub events: mpsc::UnboundedReceiver<Event>,
    stop: Arc<AtomicBool>,
}

impl Drop for Input {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// Start reading terminal events on a dedicated thread.
pub fn input() -> Input {
    let (sender, events) = mpsc::unbounded_channel();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    std::thread::spawn(move || {
        while !flag.load(Ordering::SeqCst) {
            match event::poll(Duration::from_millis(80)) {
                Ok(true) => match event::read() {
                    Ok(event) => {
                        if sender.send(event).is_err() {
                            return;
                        }
                    }
                    Err(_) => return,
                },
                Ok(false) => {}
                Err(_) => return,
            }
        }
    });
    Input { events, stop }
}
