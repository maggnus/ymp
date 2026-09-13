//! Leaving the interface.
//!
//! A first Ctrl+C only asks for a second one, so a stray press can neither end a run nor lose
//! a draft. Leaving stops whatever is still working, waits a bounded time for it to record its
//! state, and, once the terminal is restored, prints the command that opens the same
//! conversation again.

use anyhow::{Context, Result};
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use ymp_core::UiEvent;
use ymp_storage::Store;

/// How long a first Ctrl+C waits for the second.
pub const CONFIRM_WINDOW: Duration = Duration::from_secs(2);

/// What the status row says while a second Ctrl+C would leave.
pub const CONFIRM_PROMPT: &str = "Press Ctrl-C again to exit";

/// The longest leaving waits for stopped work to record its state.
pub const SHUTDOWN_WAIT: Duration = Duration::from_secs(10);

/// The status painted once leaving is committed, before anything is waited on.
pub fn closing_status(run: bool, scan: bool) -> String {
    let wait = SHUTDOWN_WAIT.as_secs();
    match (run, scan) {
        (false, false) => "Leaving ymp".into(),
        (true, false) => format!(
            "Leaving ymp: stopping the active run and waiting up to {wait} s while it records its state"
        ),
        (false, true) => format!(
            "Leaving ymp: stopping the catalog reading and waiting up to {wait} s for it to finish"
        ),
        (true, true) => format!(
            "Leaving ymp: stopping the active run and the catalog reading, waiting up to {wait} s for both to finish"
        ),
    }
}

/// How leaving ended, as far as the command printed afterwards needs to know.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Departure {
    /// The conversation the window had open, including one a run opened while it stopped.
    pub session: Option<String>,
    /// Some work had not finished stopping when the wait ran out.
    pub unfinished: bool,
}

/// Stop the run and the catalog reading, and wait for both within one bound.
///
/// The events the run sent on its way out are read too. A run started just before leaving
/// may report the session it opened only there, and that is the session to resume.
pub async fn stop_and_wait<R, S>(
    cancel: &CancellationToken,
    scan_cancel: &CancellationToken,
    running: Option<JoinHandle<R>>,
    scanning: Option<JoinHandle<S>>,
    events: &mut mpsc::UnboundedReceiver<UiEvent>,
    session: Option<String>,
    wait: Duration,
) -> Departure {
    cancel.cancel();
    scan_cancel.cancel();
    let deadline = tokio::time::Instant::now() + wait;
    let mut departure = Departure {
        session,
        unfinished: false,
    };
    if let Some(handle) = running {
        departure.unfinished |= tokio::time::timeout_at(deadline, handle).await.is_err();
    }
    if let Some(handle) = scanning {
        departure.unfinished |= tokio::time::timeout_at(deadline, handle).await.is_err();
    }
    while let Ok(event) = events.try_recv() {
        match event {
            UiEvent::Message(message) => departure.session = Some(message.session_id),
            UiEvent::Finished { session_id, .. } => departure.session = Some(session_id),
            _ => {}
        }
    }
    departure
}

/// What is printed after the terminal is restored. A window that never had a conversation
/// prints no command, because there is nothing to resume.
pub fn farewell(store: &Store, departure: &Departure, standard_home: Option<&Path>) -> String {
    let mut text = String::new();
    if departure.unfinished {
        text.push_str(&format!(
            "ymp stopped waiting after {} s. Work that was still stopping may not have recorded its latest state.\n",
            SHUTDOWN_WAIT.as_secs()
        ));
    }
    let Some(session) = departure.session.as_deref() else {
        return text;
    };
    match resume_command(store, session, standard_home) {
        Ok(command) => text.push_str(&format!(
            "Resume this session with:\n{command}\nThis opens the saved conversation and starts no agents. Use /resume there to continue the run, or send a message to continue the conversation.\n"
        )),
        Err(error) => text.push_str(&format!(
            "Session {session} could not be read again, so no resume command is printed: {error:#}\n"
        )),
    }
    text
}

/// The command that opens `session` again, every word quoted for a POSIX shell.
///
/// The directory is the one the session's project is saved under, which is where a resumed
/// run works, not the directory this window started in. The metadata directory is named
/// whenever it is not `standard_home`, so the command does not depend on the environment of
/// the shell it is pasted into.
pub fn resume_command(
    store: &Store,
    session: &str,
    standard_home: Option<&Path>,
) -> Result<String> {
    let record = store.session(session)?;
    let project = store.get_project(&record.project_id)?;
    let home = std::path::absolute(&store.home)?;
    let mut words = vec!["ymp".to_owned()];
    if standard_home != Some(home.as_path()) {
        words.push("--home".into());
        words.push(shell_word(path_text(&home)?));
    }
    words.push("-C".into());
    words.push(shell_word(path_text(&project.path)?));
    words.push("resume".into());
    words.push(shell_word(&record.id));
    Ok(words.join(" "))
}

fn path_text(path: &Path) -> Result<&str> {
    path.to_str()
        .with_context(|| format!("{} is not valid UTF-8", path.display()))
}

/// One word a POSIX shell reads back as exactly `text`.
///
/// A plain word stays as it is and anything else is single-quoted. A control character is
/// written as an escape instead, so printing the command cannot also drive the terminal.
pub fn shell_word(text: &str) -> String {
    let plain = |byte: u8| byte.is_ascii_alphanumeric() || b"-_./:@+,".contains(&byte);
    if !text.is_empty() && text.bytes().all(plain) {
        return text.to_owned();
    }
    if !text.chars().any(char::is_control) {
        return format!("'{}'", text.replace('\'', r"'\''"));
    }
    let mut word = String::from("$'");
    for ch in text.chars() {
        match ch {
            '\\' | '\'' => {
                word.push('\\');
                word.push(ch);
            }
            ch if ch.is_control() => {
                for byte in ch.encode_utf8(&mut [0; 4]).bytes() {
                    word.push_str(&format!("\\x{byte:02x}"));
                }
            }
            ch => word.push(ch),
        }
    }
    word.push('\'');
    word
}
