//! One native child process behind a bounded line channel, shared by the process
//! backends. It owns the process group, the technical directory and both pipes;
//! what a received line means stays with the provider.
use serde_json::Value;
use std::{
    io::{Read, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};
use ymp_domain::{Denial, Result};

/// Provider specifics of the channel.
#[derive(Clone, Copy)]
pub(crate) struct Wire {
    /// Builds the provider's refusal from a provider-neutral reason.
    pub refuse: fn(&str) -> Denial,
    /// Decodes one received line, newline included; `None` skips the line.
    pub parse: fn(&[u8]) -> Option<Result<Value>>,
    /// Largest line in either direction, in bytes.
    pub frame: usize,
}

fn nonblocking(descriptor: &dyn std::os::fd::AsFd, refuse: fn(&str) -> Denial) -> Result<()> {
    let flags = rustix::fs::fcntl_getfl(descriptor).map_err(|_| refuse("pipe"))?;
    rustix::fs::fcntl_setfl(descriptor, flags | rustix::fs::OFlags::NONBLOCK)
        .map_err(|_| refuse("pipe"))
}
fn end(child: &mut Child) {
    if let Some(group) = rustix::process::Pid::from_raw(child.id() as i32) {
        let _ = rustix::process::kill_process_group(group, rustix::process::Signal::KILL);
    }
    let _ = child.kill();
    let until = Instant::now() + Duration::from_millis(250);
    while child.try_wait().is_ok_and(|status| status.is_none()) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(2));
    }
}
/// Ends the process group unless the owner takes the child over.
struct Pending {
    child: Option<Child>,
    directory: Option<PathBuf>,
}
impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            end(child);
            if let Some(directory) = &self.directory {
                let _ = std::fs::remove_dir(directory);
            }
        }
    }
}

pub(crate) struct Process {
    child: Child,
    halt: Arc<AtomicBool>,
    drains: Vec<std::thread::JoinHandle<()>>,
    lines: Receiver<Result<Value>>,
    wire: Wire,
    pub directory: PathBuf,
    pub input: Arc<Mutex<ChildStdin>>,
}
impl Drop for Process {
    fn drop(&mut self) {
        self.halt.store(true, Ordering::SeqCst);
        end(&mut self.child);
        for drain in self.drains.drain(..) {
            let _ = drain.join();
        }
        let _ = std::fs::remove_dir(&self.directory);
    }
}

/// Writes one line within a fixed short deadline; a full pipe never blocks the host.
pub(crate) fn put(input: &Arc<Mutex<ChildStdin>>, value: &Value, wire: Wire) -> Result<()> {
    let refuse = wire.refuse;
    let mut line = serde_json::to_vec(value).map_err(|_| refuse("encode"))?;
    if line.len() > wire.frame {
        return Err(refuse("frame"));
    }
    line.push(b'\n');
    let until = Instant::now() + Duration::from_millis(250);
    let mut out = loop {
        match input.try_lock() {
            Ok(out) => break out,
            Err(std::sync::TryLockError::Poisoned(_)) => return Err(refuse("transport")),
            Err(_) if Instant::now() < until => std::thread::sleep(Duration::from_millis(2)),
            Err(_) => return Err(refuse("write_timeout")),
        }
    };
    let mut written = 0;
    while written < line.len() {
        if Instant::now() >= until {
            return Err(refuse("write_timeout"));
        }
        match out.write(&line[written..]) {
            Ok(0) => return Err(refuse("disconnect")),
            Ok(count) => written += count,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(2))
            }
            Err(_) => return Err(refuse("disconnect")),
        }
    }
    Ok(())
}

impl Process {
    /// Starts `command` in its own process group and in a fresh empty technical
    /// directory that is not a workspace.
    pub fn launch(mut command: Command, label: &str, wire: Wire) -> Result<Self> {
        use std::os::unix::process::CommandExt;
        let refuse = wire.refuse;
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).map_err(|_| refuse("entropy"))?;
        let directory =
            std::env::temp_dir().join(format!("ymp-{label}-{}", ymp_domain::Digest::of(nonce)));
        std::fs::create_dir(&directory).map_err(|_| refuse("directory"))?;
        // A native process reports its resolved working directory.
        let directory = directory.canonicalize().map_err(|_| {
            let _ = std::fs::remove_dir(&directory);
            refuse("directory")
        })?;
        let child = command
            .process_group(0)
            .current_dir(&directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| {
                let _ = std::fs::remove_dir(&directory);
                refuse("spawn")
            })?;
        let mut pending = Pending {
            child: Some(child),
            directory: Some(directory.clone()),
        };
        let child = pending.child.as_mut().unwrap();
        let stdin = child.stdin.take().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let mut stderr = child.stderr.take().unwrap();
        nonblocking(&stdin, refuse)?;
        nonblocking(&stdout, refuse)?;
        nonblocking(&stderr, refuse)?;
        let halt = Arc::new(AtomicBool::new(false));
        let (sender, lines) = mpsc::sync_channel(64);
        let halted = halt.clone();
        let output = std::thread::spawn(move || {
            let mut line = Vec::new();
            let mut buffer = [0; 4096];
            let deliver = |mut message: Result<Value>| {
                while !halted.load(Ordering::SeqCst) {
                    match sender.try_send(message) {
                        Ok(()) => return true,
                        Err(mpsc::TrySendError::Full(returned)) => {
                            message = returned;
                            std::thread::sleep(Duration::from_millis(2));
                        }
                        Err(_) => return false,
                    }
                }
                false
            };
            while !halted.load(Ordering::SeqCst) {
                match stdout.read(&mut buffer) {
                    Ok(0) => {
                        deliver(Err(refuse("disconnect")));
                        return;
                    }
                    Ok(count) => {
                        for byte in &buffer[..count] {
                            line.push(*byte);
                            if line.len() > wire.frame {
                                deliver(Err(refuse("frame")));
                                return;
                            }
                            if *byte != b'\n' {
                                continue;
                            }
                            if let Some(message) = (wire.parse)(&line) {
                                let malformed = message.is_err();
                                if !deliver(message) || malformed {
                                    return;
                                }
                            }
                            line.clear();
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => {
                        deliver(Err(refuse("disconnect")));
                        return;
                    }
                }
            }
        });
        // Native diagnostics are drained so that the pipe never blocks, and never retained.
        let halted = halt.clone();
        let diagnostics = std::thread::spawn(move || {
            let mut buffer = [0; 4096];
            while !halted.load(Ordering::SeqCst) {
                match stderr.read(&mut buffer) {
                    Ok(0) => return,
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2))
                    }
                    Err(_) => return,
                }
            }
        });
        let child = pending.child.take().unwrap();
        Ok(Self {
            child,
            halt,
            drains: vec![output, diagnostics],
            lines,
            wire,
            directory,
            input: Arc::new(Mutex::new(stdin)),
        })
    }
    pub fn take(&self, until: Instant) -> Result<Value> {
        let wait = until
            .checked_duration_since(Instant::now())
            .ok_or_else(|| (self.wire.refuse)("timeout"))?;
        self.lines
            .recv_timeout(wait)
            .map_err(|_| (self.wire.refuse)("timeout"))?
    }
    pub fn put(&self, value: &Value) -> Result<()> {
        put(&self.input, value, self.wire)
    }
}

/// Runs a short metadata command that starts no session and returns whether it
/// succeeded together with at most 128 bytes of its output.
pub(crate) fn reported(
    mut command: Command,
    timeout: Duration,
    refuse: fn(&str) -> Denial,
) -> Result<(bool, String)> {
    use std::os::unix::process::CommandExt;
    let child = command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| refuse("version"))?;
    let mut pending = Pending {
        child: Some(child),
        directory: None,
    };
    let child = pending.child.as_mut().unwrap();
    let mut out = child.stdout.take().unwrap();
    nonblocking(&out, refuse)?;
    let until = Instant::now() + timeout;
    let mut text = vec![];
    let mut buffer = [0; 129];
    let mut closed = false;
    loop {
        if Instant::now() >= until {
            return Err(refuse("timeout"));
        }
        match out.read(&mut buffer) {
            Ok(0) => closed = true,
            Ok(count) => {
                text.extend_from_slice(&buffer[..count]);
                if text.len() > 128 {
                    return Err(refuse("version"));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => return Err(refuse("version")),
        }
        if closed && let Some(status) = child.try_wait().map_err(|_| refuse("version"))? {
            return Ok((
                status.success(),
                String::from_utf8_lossy(&text).trim().to_owned(),
            ));
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}
