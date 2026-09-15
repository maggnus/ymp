//! Durable `Journal` storage for ymp sessions (DEV-0005).
//!
//! [`FileJournal`] implements the kernel's [`Journal`] port as an append-only
//! per-session record log with fsync and an atomically renamed durable-prefix
//! descriptor, exactly as fixed by the durable Journal contract
//! (`ymp-docs/durable-journal-contract.md`). The kernel port, its typed errors
//! and the projection rules are unchanged; this adapter only adds the
//! `Corruption`, `UnsupportedFormat` and `IndeterminateCommit` failure
//! variants to the shared `JournalError`.
//!
//! # Storage ownership, location, lifetime and deletion
//!
//! See [`STORAGE_DOCUMENTATION`] for the user-facing statement. In short: the
//! journal root is selected explicitly by the caller through runtime assembly
//! or a CLI option; the adapter writes only inside that root and creates
//! nothing elsewhere; the files are plain, documented, user-owned files;
//! their lifetime ends only when the user deletes the root; there is no
//! migration and no compaction; and streams grow monotonically with every
//! committed batch.
//!
//! # Concurrency
//!
//! Within one process, a process-wide in-process lock serializes
//! compare-and-append per root. Across processes on one root, an advisory
//! exclusive OS lock on `<root>/journal.lock` covers a whole append and
//! open-time recovery; a crashed holder's lock is released by the OS. Reads
//! take no append lock: the descriptor replacement is atomic and the prefix
//! is durable before the descriptor changes.

#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use ymp_domain::SessionId;
use ymp_kernel::{Journal, JournalEntry, JournalError, Revision, SessionEvent};

mod crc;
mod payload;
mod stream;

use stream::Inspection;

/// User-facing documentation of the durable journal's storage behavior,
/// required by the durable Journal contract (acceptance criterion 10).
pub const STORAGE_DOCUMENTATION: &str = "\
Durable session storage

Location: the journal root is selected explicitly by the caller (runtime \
assembly or CLI option). All journal data lives under that root as plain \
files: <root>/sessions/<encoded-session-id>.log (append-only record stream), \
<root>/sessions/<encoded-session-id>.commit (durable-prefix descriptor) and \
<root>/journal.lock (append lock). The adapter writes only inside the root \
and creates nothing elsewhere; there are no hidden application-owned files.

Ownership: the files are plain documented files owned by the user running \
ymp. They record the exact user Task content - goal, acceptance criteria \
and constraints - and may contain sensitive text; protect them accordingly. \
The adapter imports no credentials or authentication state.

Lifetime and growth: journal data lives exactly as long as these files do. \
Streams grow monotonically with every committed batch. There is no format \
migration and no compaction; each would require its own explicit future \
task. Deletion is an explicit user action - remove the root - after which no \
hidden copies remain. Copying, moving, backing up or deleting journal data \
is a quiescent operation: close every process using the root first, unless \
you take an explicitly agreed coherent filesystem snapshot.

Recovery limits: an acknowledged batch survives process restart. \
Unacknowledged work lost to an interruption is discarded by design. Damaged \
storage is reported as an explicit error, never as an empty or shortened \
session. Power-loss durability rests on the filesystem honoring fsync on a \
POSIX local filesystem; network filesystems are unsupported.";

/// Verification-only fault injection used by the DEV-0005 acceptance tests
/// (contract criteria 3 and 9). A fault fires once, on the first append that
/// reaches its point in the persistence protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InjectedFault {
    /// Fails with `AdapterFailure` after the batch bytes are durable in the
    /// log but before the commit descriptor is written or renamed (before
    /// the commit point).
    AdapterFailureBeforeCommit,
    /// Fails with `IndeterminateCommit` after the commit descriptor rename
    /// succeeded, skipping the sessions-directory sync (after the commit
    /// point).
    IndeterminateAfterCommit,
}

/// A durable `Journal` adapter storing one append-only stream per session
/// under a caller-selected root.
#[derive(Debug)]
pub struct FileJournal {
    root: PathBuf,
    sessions_dir: PathBuf,
    lock_file: File,
    process_lock: Arc<Mutex<()>>,
    fault: Mutex<Option<InjectedFault>>,
    temp_counter: AtomicU64,
}

impl FileJournal {
    /// Opens (creating if needed) the journal at `root` and performs open-time
    /// recovery under the root lock.
    pub fn open(root: &Path) -> Result<Self, JournalError> {
        Self::open_inner(root, None)
    }

    /// Opens the journal like [`FileJournal::open`] with a verification fault
    /// installed (see [`InjectedFault`]). Production code uses `open`.
    pub fn open_with_injected_fault(
        root: &Path,
        fault: InjectedFault,
    ) -> Result<Self, JournalError> {
        Self::open_inner(root, Some(fault))
    }

    fn open_inner(root: &Path, fault: Option<InjectedFault>) -> Result<Self, JournalError> {
        let adapter = |message: String| JournalError::AdapterFailure { message };

        let root_exists = root
            .try_exists()
            .map_err(|error| adapter(format!("cannot inspect journal root: {error}")))?;
        if root_exists && !root.is_dir() {
            return Err(adapter(
                "journal root exists and is not a directory".to_owned(),
            ));
        }
        if !root_exists {
            fs::create_dir_all(root)
                .map_err(|error| adapter(format!("cannot create journal root: {error}")))?;
            if let Some(parent) = root
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
            {
                stream::sync_dir(parent)
                    .map_err(|error| adapter(format!("cannot sync the root's parent: {error}")))?;
            }
        }

        let sessions_dir = root.join("sessions");
        if !sessions_dir
            .try_exists()
            .map_err(|error| adapter(format!("cannot inspect the sessions directory: {error}")))?
        {
            fs::create_dir_all(&sessions_dir).map_err(|error| {
                adapter(format!("cannot create the sessions directory: {error}"))
            })?;
            stream::sync_dir(root)
                .map_err(|error| adapter(format!("cannot sync the journal root: {error}")))?;
        }

        let lock_path = root.join("journal.lock");
        let lock_existed = lock_path
            .try_exists()
            .map_err(|error| adapter(format!("cannot inspect the lock file: {error}")))?;
        let lock_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|error| adapter(format!("cannot open the journal lock file: {error}")))?;
        if !lock_existed {
            stream::sync_dir(root)
                .map_err(|error| adapter(format!("cannot sync the journal root: {error}")))?;
        }

        let canonical_root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let journal = Self {
            root: canonical_root.clone(),
            sessions_dir,
            lock_file,
            process_lock: process_lock_for(&canonical_root),
            fault: Mutex::new(fault),
            temp_counter: AtomicU64::new(0),
        };

        let recovery = {
            let _guard = journal
                .process_lock
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            match journal.lock_root_file() {
                Ok(()) => {
                    let recovery = journal.recover();
                    let _ = journal.lock_file.unlock();
                    recovery
                }
                Err(error) => Err(error),
            }
        };
        recovery?;

        Ok(journal)
    }

    /// The journal root this adapter was opened with.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn lock_root_file(&self) -> Result<(), JournalError> {
        loop {
            match self.lock_file.try_lock() {
                Ok(()) => return Ok(()),
                Err(fs::TryLockError::WouldBlock) => {
                    // Another process holds the advisory root lock; appends
                    // serialize, so wait for it.
                    thread::sleep(Duration::from_millis(1));
                }
                Err(fs::TryLockError::Error(error)) => {
                    return Err(JournalError::AdapterFailure {
                        message: format!("cannot acquire the journal root lock: {error}"),
                    });
                }
            }
        }
    }

    fn take_fault_if(&self, fault: InjectedFault) -> bool {
        let mut current = self
            .fault
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *current == Some(fault) {
            *current = None;
            true
        } else {
            false
        }
    }

    /// Open-time recovery: classifies every log/commit pair in the sessions
    /// directory and applies the contract's recovery matrix — truncating only
    /// uncommitted suffixes, never deleting a committed prefix, cleaning up
    /// temporary descriptors, and never touching streams of an unknown format
    /// version. Runs only under both the in-process and the advisory root
    /// lock.
    fn recover(&self) -> Result<(), JournalError> {
        let adapter = |message: String| JournalError::AdapterFailure { message };

        let mut stems: HashSet<String> = HashSet::new();
        let mut temp_files: Vec<String> = Vec::new();
        let entries = fs::read_dir(&self.sessions_dir)
            .map_err(|error| adapter(format!("cannot read the sessions directory: {error}")))?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                adapter(format!("cannot iterate the sessions directory: {error}"))
            })?;
            let name = entry.file_name().into_string().map_err(|name| {
                adapter(format!(
                    "sessions directory contains a non-UTF-8 file name: {name:?}"
                ))
            })?;
            if let Some(stem) = stream::temp_file_stem(&name) {
                stems.insert(stem);
                temp_files.push(name);
            } else if let Some(stem) = name
                .strip_suffix(".log")
                .or_else(|| name.strip_suffix(".commit"))
            {
                stems.insert(stem.to_owned());
            }
        }

        let mut unsupported: HashSet<String> = HashSet::new();
        let mut removed_any = false;
        for stem in &stems {
            if stream::decode_stem(stem).is_none() {
                // Not a stream this adapter ever wrote; leave it untouched.
                continue;
            }
            match stream::inspect(&self.sessions_dir, stem)? {
                Inspection::Absent | Inspection::Corrupted(_) => {}
                Inspection::Unsupported(_) => {
                    // Opening never migrates, rewrites or repairs format
                    // versions, including temporary artifacts.
                    unsupported.insert(stem.clone());
                }
                Inspection::Initial {
                    committed_len,
                    log_len,
                    ..
                } => {
                    if log_len > committed_len {
                        stream::truncate_log(&self.sessions_dir, stem, committed_len)?;
                        removed_any = true;
                    }
                }
                Inspection::Committed {
                    committed_len,
                    log_len,
                    ..
                } => {
                    if log_len > committed_len {
                        stream::truncate_log(&self.sessions_dir, stem, committed_len)?;
                    }
                }
            }
        }

        for name in temp_files {
            let stem = stream::temp_file_stem(&name).unwrap_or_default();
            if unsupported.contains(&stem) {
                continue;
            }
            fs::remove_file(self.sessions_dir.join(&name)).map_err(|error| {
                adapter(format!(
                    "cannot remove the temporary descriptor {name}: {error}"
                ))
            })?;
            removed_any = true;
        }
        if removed_any {
            stream::sync_dir(&self.sessions_dir).map_err(|error| {
                adapter(format!(
                    "cannot sync the sessions directory after recovery: {error}"
                ))
            })?;
        }
        Ok(())
    }

    fn append_locked(
        &self,
        session_id: &SessionId,
        expected_revision: Revision,
        payloads: &[Vec<u8>],
    ) -> Result<Revision, JournalError> {
        let adapter = |message: String| JournalError::AdapterFailure { message };
        let stem = stream::stream_stem(session_id);
        let log_path = stream::log_path(&self.sessions_dir, &stem);

        // Step 2: validate or recover the current durable prefix. No byte is
        // written until the append is admitted, so a stale append leaves the
        // filesystem untouched.
        let inspection = stream::inspect(&self.sessions_dir, &stem)?;
        let durable_revision = match &inspection {
            Inspection::Absent | Inspection::Initial { .. } => Revision::INITIAL,
            Inspection::Committed { bytes, .. } => {
                let entries = stream::decode_committed(bytes)?;
                entries
                    .last()
                    .map(JournalEntry::revision)
                    .unwrap_or(Revision::INITIAL)
            }
            Inspection::Unsupported(version) => {
                return Err(JournalError::UnsupportedFormat { version: *version });
            }
            Inspection::Corrupted(message) => {
                return Err(JournalError::Corruption {
                    message: message.clone(),
                });
            }
        };

        if durable_revision != expected_revision {
            return Err(JournalError::StaleRevision {
                expected: expected_revision,
                actual: durable_revision,
            });
        }

        let attempted_revision = stream::assign_revisions(expected_revision, payloads.len())?;
        if attempted_revision.value() > stream::MAX_STREAM_RECORDS {
            return Err(adapter(format!(
                "append would exceed the {}-record stream limit",
                stream::MAX_STREAM_RECORDS
            )));
        }

        let mut record_frames = Vec::new();
        let mut revision = expected_revision;
        for payload in payloads {
            revision = revision
                .checked_next()
                .ok_or(JournalError::RevisionOverflow)?;
            record_frames.extend_from_slice(&stream::record_frame(revision.value(), payload));
        }
        let completion = stream::completion_frame(&record_frames, payloads.len() as u32);

        // The append is admitted. Normalize the stream to a known state and
        // establish the committed prefix the descriptor will be extended from.
        let (mut current_len, mut current_digest) = match inspection {
            Inspection::Absent => {
                // First creation: create the log (the directory gains an
                // entry), write and sync the header, then install the
                // initial empty-prefix descriptor through steps 5-8.
                let mut log = OpenOptions::new()
                    .write(true)
                    .read(true)
                    .create_new(true)
                    .open(&log_path)
                    .map_err(|error| adapter(format!("cannot create the stream log: {error}")))?;
                stream::sync_dir(&self.sessions_dir).map_err(|error| {
                    adapter(format!("cannot sync the sessions directory: {error}"))
                })?;
                let header = stream::header_bytes();
                log.write_all(&header)
                    .and_then(|()| log.sync_all())
                    .map_err(|error| adapter(format!("cannot write the stream header: {error}")))?;
                drop(log);
                let header_digest = crc::crc32(&header);
                self.install_descriptor(&stem, stream::HEADER_LEN as u64, header_digest)?;
                // Step 8 for the initial descriptor: the replaced entry
                // becomes durable before the batch is appended.
                stream::sync_dir(&self.sessions_dir).map_err(|error| {
                    adapter(format!("cannot sync the sessions directory: {error}"))
                })?;
                (stream::HEADER_LEN as u64, header_digest)
            }
            Inspection::Initial {
                committed_len,
                digest,
                log_len,
                log_has_valid_header,
            } => {
                // Normalize to the initial empty stream: exactly the header
                // in the log, and the empty-prefix descriptor published. Only
                // uncommitted bytes are rewritten.
                let header = stream::header_bytes();
                if !log_has_valid_header || log_len != stream::HEADER_LEN as u64 {
                    let mut log = OpenOptions::new()
                        .write(true)
                        .read(true)
                        .truncate(false)
                        .open(&log_path)
                        .map_err(|error| adapter(format!("cannot open the stream log: {error}")))?;
                    log.set_len(0)
                        .and_then(|()| log.write_all(&header))
                        .and_then(|()| log.sync_all())
                        .map_err(|error| {
                            adapter(format!("cannot rewrite the stream header: {error}"))
                        })?;
                }
                let header_digest = crc::crc32(&header);
                if committed_len != stream::HEADER_LEN as u64 || digest != Some(header_digest) {
                    self.install_descriptor(&stem, stream::HEADER_LEN as u64, header_digest)?;
                    stream::sync_dir(&self.sessions_dir).map_err(|error| {
                        adapter(format!("cannot sync the sessions directory: {error}"))
                    })?;
                }
                (stream::HEADER_LEN as u64, header_digest)
            }
            Inspection::Committed {
                committed_len,
                digest,
                log_len,
                ..
            } => {
                if log_len > committed_len {
                    stream::truncate_log(&self.sessions_dir, &stem, committed_len)?;
                }
                (committed_len, digest)
            }
            Inspection::Unsupported(_) | Inspection::Corrupted(_) => {
                unreachable!("unsupported and corrupted streams fail before admission")
            }
        };

        // Steps 3-4: append the framed batch and make it durable.
        let mut log = OpenOptions::new()
            .write(true)
            .read(true)
            .open(&log_path)
            .map_err(|error| adapter(format!("cannot open the stream log: {error}")))?;
        log.seek(SeekFrom::End(0))
            .map_err(|error| adapter(format!("cannot seek the stream log: {error}")))?;
        log.write_all(&record_frames)
            .and_then(|()| log.write_all(&completion))
            .and_then(|()| log.sync_all())
            .map_err(|error| adapter(format!("cannot append the batch to the log: {error}")))?;

        if self.take_fault_if(InjectedFault::AdapterFailureBeforeCommit) {
            return Err(JournalError::AdapterFailure {
                message: "injected failure before the commit point: the durable stream is \
                          unchanged and the uncommitted suffix is invisible"
                    .to_owned(),
            });
        }

        // Steps 5-7: write, sync and rename the new durable-prefix descriptor.
        current_len += (record_frames.len() + completion.len()) as u64;
        current_digest = crc::crc32_extend(current_digest, &record_frames);
        current_digest = crc::crc32_extend(current_digest, &completion);
        self.install_descriptor(&stem, current_len, current_digest)?;

        if self.take_fault_if(InjectedFault::IndeterminateAfterCommit) {
            return Err(JournalError::IndeterminateCommit {
                expected: expected_revision,
                attempted: attempted_revision,
                message: "injected failure after the commit descriptor rename: the \
                          sessions-directory sync was not performed"
                    .to_owned(),
            });
        }

        // Step 8: make the replaced directory entry durable.
        stream::sync_dir(&self.sessions_dir).map_err(|error| {
            JournalError::IndeterminateCommit {
                expected: expected_revision,
                attempted: attempted_revision,
                message: format!("sessions directory sync failed after the commit rename: {error}"),
            }
        })?;

        // Step 9: the caller releases the locks and returns success.
        Ok(attempted_revision)
    }

    /// Steps 5-7 of the persistence protocol: write a unique temporary
    /// descriptor with `create_new` (re-selecting the suffix on collision),
    /// sync it, and rename it over the commit descriptor.
    fn install_descriptor(
        &self,
        stem: &str,
        prefix_len: u64,
        digest: u32,
    ) -> Result<(), JournalError> {
        let adapter = |message: String| JournalError::AdapterFailure { message };
        let mut descriptor = Vec::with_capacity(12);
        descriptor.extend_from_slice(&prefix_len.to_le_bytes());
        descriptor.extend_from_slice(&digest.to_le_bytes());
        let target = stream::commit_path(&self.sessions_dir, stem);

        for _ in 0..64 {
            let name = format!("{stem}.commit.tmp-{}", self.next_temp_suffix());
            let temporary = self.sessions_dir.join(&name);
            let mut file = match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)
            {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(adapter(format!(
                        "cannot create the temporary commit descriptor: {error}"
                    )));
                }
            };
            let write = file.write_all(&descriptor).and_then(|()| file.sync_all());
            drop(file);
            write.map_err(|error| {
                let _ = fs::remove_file(&temporary);
                adapter(format!(
                    "cannot write the temporary commit descriptor: {error}"
                ))
            })?;
            match fs::rename(&temporary, &target) {
                Ok(()) => return Ok(()),
                Err(error) => {
                    let _ = fs::remove_file(&temporary);
                    return Err(adapter(format!(
                        "cannot rename the commit descriptor into place: {error}"
                    )));
                }
            }
        }
        Err(adapter(
            "cannot find a free temporary commit descriptor name".to_owned(),
        ))
    }

    fn next_temp_suffix(&self) -> String {
        let counter = self
            .temp_counter
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|since| since.as_nanos() as u64)
            .unwrap_or(0);
        let pid = u64::from(std::process::id());
        let mixed = counter.wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ nanos.rotate_left(17)
            ^ pid.rotate_left(33);
        format!("{mixed:016x}")
    }
}

impl Journal for FileJournal {
    fn read(&self, session_id: &SessionId) -> Result<Vec<JournalEntry>, JournalError> {
        // Reads take no append lock: the descriptor replacement is atomic and
        // the prefix is durable before the descriptor changes.
        let stem = stream::stream_stem(session_id);
        match stream::inspect(&self.sessions_dir, &stem)? {
            Inspection::Absent | Inspection::Initial { .. } => Ok(Vec::new()),
            Inspection::Committed { bytes, .. } => stream::decode_committed(&bytes),
            Inspection::Unsupported(version) => Err(JournalError::UnsupportedFormat { version }),
            Inspection::Corrupted(message) => Err(JournalError::Corruption { message }),
        }
    }

    fn append(
        &self,
        session_id: &SessionId,
        expected_revision: Revision,
        events: Vec<SessionEvent>,
    ) -> Result<Revision, JournalError> {
        // Rejected before any storage access; no byte is written.
        if events.is_empty() {
            return Err(JournalError::EmptyBatch);
        }
        // Input limits are validated before any change is made.
        if session_id.as_str().len() > stream::MAX_SESSION_ID_BYTES {
            return Err(JournalError::AdapterFailure {
                message: format!(
                    "session id exceeds the adapter limit of {} UTF-8 bytes",
                    stream::MAX_SESSION_ID_BYTES
                ),
            });
        }
        if events.len() > stream::MAX_BATCH_RECORDS {
            return Err(JournalError::AdapterFailure {
                message: format!(
                    "batch exceeds the adapter limit of {} records",
                    stream::MAX_BATCH_RECORDS
                ),
            });
        }
        let mut payloads = Vec::with_capacity(events.len());
        for event in &events {
            payloads.push(payload::encode_event(event)?);
        }

        // Step 1: the in-process lock, then the advisory root lock.
        let _process_guard = self
            .process_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.lock_root_file()?;
        let result = self.append_locked(session_id, expected_revision, &payloads);
        let _ = self.lock_file.unlock();
        result
    }
}

/// Returns the process-wide in-process lock for one journal root, so that all
/// [`FileJournal`] instances in this process serialize per root.
fn process_lock_for(root: &Path) -> Arc<Mutex<()>> {
    static REGISTRY: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> = OnceLock::new();
    let registry = REGISTRY.get_or_init(|| Mutex::new(HashMap::new()));
    let mut locks = registry
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    locks.entry(root.to_path_buf()).or_default().clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_documentation_states_the_required_facts() {
        for required in [
            "journal root",
            "sessions",
            ".log",
            ".commit",
            "journal.lock",
            "owned by the user",
            "no format migration",
            "no compaction",
            "remove the root",
            "no hidden copies",
        ] {
            assert!(
                STORAGE_DOCUMENTATION.contains(required),
                "documentation is missing '{required}'"
            );
        }
    }
}
