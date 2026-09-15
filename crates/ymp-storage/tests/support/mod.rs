//! Shared support for the ymp-storage acceptance tests.
//!
//! The framing, checksum and naming helpers below are deliberately
//! independent re-implementations of the durable Journal contract's version-1
//! format, so that byte-level assertions cross-check the adapter instead of
//! reusing its code.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ymp_domain::{
    AcceptanceContract, Constraints, Criterion, CriterionId, Goal, SessionId, Task, TaskId,
};
use ymp_kernel::Dispatcher;
use ymp_storage::FileJournal;

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

/// Version-1 stream header: magic `YMPJ` plus u16 little-endian version.
pub fn header() -> Vec<u8> {
    let mut bytes = b"YMPJ".to_vec();
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes
}

/// Record frame: tag 0x01, u32 payload length, u64 revision, payload, then a
/// CRC-32 over all preceding bytes of this record.
pub fn record_frame(revision: u64, payload: &[u8]) -> Vec<u8> {
    let mut frame = vec![0x01];
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&revision.to_le_bytes());
    frame.extend_from_slice(payload);
    let checksum = crc32(&frame);
    frame.extend_from_slice(&checksum.to_le_bytes());
    frame
}

/// Batch completion frame: tag 0x02, u32 record count, then a CRC-32 over the
/// full record frames of the batch followed by the completion tag and count.
/// Returns only the completion bytes.
pub fn completion_frame(record_frames: &[u8], count: u32) -> Vec<u8> {
    let mut scope = record_frames.to_vec();
    scope.push(0x02);
    scope.extend_from_slice(&count.to_le_bytes());
    let checksum = crc32(&scope);
    let mut frame = Vec::with_capacity(9);
    frame.push(0x02);
    frame.extend_from_slice(&count.to_le_bytes());
    frame.extend_from_slice(&checksum.to_le_bytes());
    frame
}

/// Builds a whole framed stream from batches of payloads.
pub fn framed_stream(batches: &[Vec<Vec<u8>>]) -> Vec<u8> {
    let mut bytes = header();
    for batch in batches {
        let mut record_frames = Vec::new();
        let mut revision = bytes_committed_records(&bytes) + 1;
        for payload in batch {
            record_frames.extend_from_slice(&record_frame(revision, payload));
            revision += 1;
        }
        bytes.extend_from_slice(&record_frames);
        bytes.extend_from_slice(&completion_frame(&record_frames, batch.len() as u32));
    }
    bytes
}

fn bytes_committed_records(bytes: &[u8]) -> u64 {
    // Only used to assign contiguous revisions in `framed_stream`.
    let mut count = 0u64;
    let mut position = 6usize;
    while position < bytes.len() {
        if bytes[position] == 0x01 {
            let payload_len = u32::from_le_bytes(
                bytes[position + 1..position + 5]
                    .try_into()
                    .expect("u32 slice"),
            ) as usize;
            position += 13 + payload_len + 4;
            count += 1;
        } else {
            position += 9;
        }
    }
    count
}

/// Durable-prefix descriptor: u64 prefix length plus u32 digest over the
/// committed log bytes.
pub fn descriptor(prefix_len: u64, digest: u32) -> Vec<u8> {
    let mut bytes = prefix_len.to_le_bytes().to_vec();
    bytes.extend_from_slice(&digest.to_le_bytes());
    bytes
}

/// Descriptor naming exactly `prefix` as the committed region.
pub fn committed_descriptor(prefix: &[u8]) -> Vec<u8> {
    descriptor(prefix.len() as u64, crc32(prefix))
}

/// Lowercase-hex stream stem of a session ID.
pub fn hex_stem(session_id: &str) -> String {
    session_id
        .as_bytes()
        .iter()
        .flat_map(|byte| [(byte >> 4), (byte & 0x0F)])
        .map(|nibble| char::from_digit(nibble as u32, 16).expect("hex digit"))
        .collect()
}

/// Writes a store fixture for one session.
pub fn write_store(root: &Path, session_id: &str, log: &[u8], commit: Option<&[u8]>) {
    let sessions = root.join("sessions");
    fs::create_dir_all(&sessions).expect("sessions directory creates");
    let stem = hex_stem(session_id);
    fs::write(sessions.join(format!("{stem}.log")), log).expect("log fixture writes");
    if let Some(commit) = commit {
        fs::write(sessions.join(format!("{stem}.commit")), commit).expect("commit fixture writes");
    }
}

pub fn log_path(root: &Path, session_id: &str) -> PathBuf {
    root.join("sessions")
        .join(format!("{}.log", hex_stem(session_id)))
}

pub fn commit_path(root: &Path, session_id: &str) -> PathBuf {
    root.join("sessions")
        .join(format!("{}.commit", hex_stem(session_id)))
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

pub fn open_journal(root: &Path) -> FileJournal {
    FileJournal::open(root).expect("journal opens")
}

pub fn open_dispatcher(root: &Path) -> Dispatcher<FileJournal> {
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
