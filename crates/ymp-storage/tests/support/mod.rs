//! Shared support for the ymp-storage acceptance tests.
//!
//! The CRC-32 and checksum constructions below are deliberately independent
//! re-implementations of the durable Journal contract's checksum input, so
//! byte-level assertions cross-check the adapter instead of reusing its code.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use ymp_domain::{
    AcceptanceContract, Constraints, Criterion, CriterionId, Goal, SessionId, Task, TaskId,
};
use ymp_kernel::Dispatcher;
use ymp_storage::SqliteJournal;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A unique temporary directory removed on drop.
pub struct TempRoot {
    path: PathBuf,
}

impl TempRoot {
    pub fn new(tag: &str) -> Self {
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|since| since.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!(
            "ymp-storage-{}-{}-{}-{}",
            tag,
            std::process::id(),
            unique,
            nanos
        ));
        fs::create_dir_all(&path).expect("temporary root creates");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Independent bitwise CRC-32 (ISO-HDLC) used to cross-check the adapter.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = u32::from(crc & 1 != 0) * 0xEDB8_8320;
            crc = (crc >> 1) ^ mask;
        }
    }
    crc ^ 0xFFFF_FFFF
}

/// The contract's per-row content checksum, built from the independent CRC-32
/// above: le32(payload_version) | le32(revision_hi) | le32(revision_lo) |
/// le32(len(session UTF-8)) | session UTF-8 | payload bytes.
pub fn row_checksum(
    payload_version: u32,
    revision_hi: u32,
    revision_lo: u32,
    session_id: &str,
    payload: &[u8],
) -> u32 {
    let mut input = Vec::new();
    input.extend_from_slice(&payload_version.to_le_bytes());
    input.extend_from_slice(&revision_hi.to_le_bytes());
    input.extend_from_slice(&revision_lo.to_le_bytes());
    input.extend_from_slice(
        &u32::try_from(session_id.len())
            .expect("session ID length fits u32")
            .to_le_bytes(),
    );
    input.extend_from_slice(session_id.as_bytes());
    input.extend_from_slice(payload);
    crc32(&input)
}

/// Opens a direct SQLite connection to the journal database, bypassing the
/// adapter, for deliberate row manipulation in fixtures.
pub fn direct_connection(root: &Path) -> Connection {
    Connection::open(root.join("journal.db")).expect("direct connection opens")
}

/// Captures every file under `root` (relative path and bytes), sorted.
pub fn snapshot_tree(root: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(base: &Path, prefix: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let entries = fs::read_dir(prefix).expect("directory lists");
        for entry in entries {
            let entry = entry.expect("directory entry reads");
            let path = entry.path();
            let name = path
                .strip_prefix(base)
                .expect("path is under base")
                .to_string_lossy()
                .to_string();
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                let bytes = fs::read(&path).expect("file reads");
                out.push((name, bytes));
            }
        }
    }
    let mut files = Vec::new();
    walk(root, root, &mut files);
    files.sort();
    files
}

pub fn session_id(value: &str) -> SessionId {
    SessionId::new(value).expect("valid session ID")
}

/// A task whose exact text (spaces, newlines, quotes, backslashes, non-ASCII)
/// must survive persistence without normalization.
pub fn sample_task(tag: &str) -> Task {
    Task::new(
        TaskId::new(format!("  task id {tag}  ")).expect("valid task ID"),
        Goal::new(format!(
            "Goal \"quoted\" \\ backslash\nnewline {tag} émoji 🚀"
        ))
        .expect("valid goal"),
        AcceptanceContract::new(vec![
            Criterion::new(
                CriterionId::new(format!("crit-{tag}")).expect("valid criterion ID"),
                format!("Criterion one for {tag} with  spaces.\tTab."),
            )
            .expect("valid criterion"),
            Criterion::new(
                CriterionId::new("crit-2").expect("valid criterion ID"),
                format!("Второй критерий {tag} ✓"),
            )
            .expect("valid criterion"),
        ])
        .expect("valid acceptance contract"),
        Constraints::new(vec![
            "Do not access the network.".to_owned(),
            "  Retain surrounding spaces.  ".to_owned(),
        ])
        .expect("valid constraints"),
    )
}

pub fn cancelled_event(session_id: &SessionId) -> ymp_kernel::SessionEvent {
    ymp_kernel::SessionEvent::SessionCancelled {
        session_id: session_id.clone(),
    }
}

pub fn opened_event(session_id: &SessionId, task: &Task) -> ymp_kernel::SessionEvent {
    ymp_kernel::SessionEvent::SessionOpened {
        session_id: session_id.clone(),
        task: task.clone(),
    }
}

pub fn open_journal(root: &Path) -> SqliteJournal {
    SqliteJournal::open(root).expect("journal opens")
}

pub fn open_dispatcher(root: &Path) -> Dispatcher<SqliteJournal> {
    Dispatcher::new(open_journal(root))
}

// ---------------------------------------------------------------------------
// Child-process support (the test binary re-executes itself with `--exact`).
// ---------------------------------------------------------------------------

/// Returns the scenario name when running as a child process.
pub fn child_mode() -> Option<String> {
    std::env::var("YMP_TEST_MODE")
        .ok()
        .filter(|value| !value.is_empty())
}

pub fn child_var(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("child environment {name} is set"))
}

pub fn write_outcome(text: &str) {
    fs::write(child_var("YMP_TEST_OUTCOME"), text).expect("outcome writes");
}

/// Runs one scenario of this test binary as a separate process and returns its
/// outcome line. Panics if the child fails or exceeds the deadline.
pub fn run_child(test_name: &str, mode: &str, envs: &[(&str, String)]) -> String {
    spawn_child(test_name, mode, envs)
        .join()
        .expect("child scenario thread completes")
}

/// Spawns one scenario of this test binary as a separate process without
/// waiting; the returned handle joins on the child's outcome line.
pub fn spawn_child(
    test_name: &str,
    mode: &str,
    envs: &[(&str, String)],
) -> std::thread::JoinHandle<String> {
    let temp = TempRoot::new("child-spawn");
    let outcome_path = temp.path().join("outcome");
    let executable = std::env::current_exe().expect("test binary path");
    let mut command = Command::new(executable);
    command
        .arg("--exact")
        .arg(test_name)
        .arg("--nocapture")
        .arg("--test-threads=1")
        .env("YMP_TEST_MODE", mode)
        .env("YMP_TEST_OUTCOME", &outcome_path)
        .envs(envs.iter().cloned());
    let mut child = command.spawn().expect("child process spawns");
    let mode = mode.to_owned();
    std::thread::spawn(move || {
        let _outcome_dir = temp; // keeps the outcome file alive until read
        let deadline = SystemTime::now() + Duration::from_secs(30);
        let status = loop {
            match child.try_wait().expect("child is waitable") {
                Some(status) => break status,
                None if SystemTime::now() > deadline => {
                    let _ = child.kill();
                    panic!("child scenario '{mode}' timed out");
                }
                None => std::thread::sleep(Duration::from_millis(2)),
            }
        };
        assert!(
            status.success(),
            "child scenario '{mode}' failed with {status}"
        );
        fs::read_to_string(&outcome_path)
            .unwrap_or_default()
            .trim()
            .to_owned()
    })
}

/// Waits until `path` exists (a start barrier between two processes).
pub fn wait_for_barrier(path: &Path) {
    let deadline = SystemTime::now() + Duration::from_secs(30);
    while !path.exists() {
        assert!(SystemTime::now() < deadline, "barrier never appeared");
        std::thread::sleep(Duration::from_millis(1));
    }
}
