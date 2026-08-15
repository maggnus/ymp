use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;
use ymp_domain::{EVENT_SCHEMA_VERSION, EventEnvelope};

pub const MAX_EVENT_BYTES: u64 = 64 * 1024;
pub const MAX_JOURNAL_BYTES: u64 = 16 * 1024 * 1024;
pub const TERMINAL_EVENT_RESERVE_BYTES: u64 = MAX_EVENT_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct JournalLimits {
    pub max_event_bytes: u64,
    pub max_journal_bytes: u64,
    pub terminal_reserve_bytes: u64,
}

impl Default for JournalLimits {
    fn default() -> Self {
        Self {
            max_event_bytes: MAX_EVENT_BYTES,
            max_journal_bytes: MAX_JOURNAL_BYTES,
            terminal_reserve_bytes: TERMINAL_EVENT_RESERVE_BYTES,
        }
    }
}

#[derive(Debug, Error)]
pub enum JournalError {
    #[error("journal I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("journal line {line} is not valid JSON: {source}")]
    Json {
        line: usize,
        #[source]
        source: serde_json::Error,
    },
    #[error("journal has an incomplete final record")]
    IncompleteTail,
    #[error("unsupported event schema version {actual} at sequence {sequence}")]
    UnsupportedSchema { sequence: u64, actual: u32 },
    #[error("journal mixes run identifiers at sequence {sequence}")]
    RunMismatch { sequence: u64 },
    #[error("expected sequence {expected}, found {actual}")]
    Sequence { expected: u64, actual: u64 },
    #[error("predecessor digest mismatch at sequence {sequence}")]
    Predecessor { sequence: u64 },
    #[error("event digest mismatch at sequence {sequence}")]
    Digest { sequence: u64 },
    #[error("cannot append sequence {actual}; expected {expected}")]
    AppendSequence { expected: u64, actual: u64 },
    #[error("journal limits are invalid")]
    InvalidLimits,
    #[error("event record has {actual} bytes; the limit is {maximum}")]
    EventTooLarge { actual: u64, maximum: u64 },
    #[error("journal has {actual} bytes; the limit is {maximum}")]
    JournalTooLarge { actual: u64, maximum: u64 },
    #[error("journal capacity is exhausted at {maximum} bytes")]
    CapacityExhausted { maximum: u64 },
}

#[derive(Debug)]
pub struct Journal {
    path: PathBuf,
    last_sequence: u64,
    last_digest: Option<String>,
    bytes_written: u64,
    limits: JournalLimits,
}

impl Journal {
    pub fn open(path: impl AsRef<Path>) -> Result<(Self, Vec<EventEnvelope>), JournalError> {
        Self::open_with_limits(path, JournalLimits::default())
    }

    pub fn open_with_limits(
        path: impl AsRef<Path>,
        limits: JournalLimits,
    ) -> Result<(Self, Vec<EventEnvelope>), JournalError> {
        validate_limits(limits)?;
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let events = Self::read_all_with_limits(&path, limits)?;
        let bytes_written = match path.metadata() {
            Ok(metadata) => metadata.len(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
            Err(error) => return Err(error.into()),
        };
        let last_sequence = events.last().map_or(0, |event| event.sequence);
        let last_digest = events.last().map(|event| event.digest.clone());
        Ok((
            Self {
                path,
                last_sequence,
                last_digest,
                bytes_written,
                limits,
            },
            events,
        ))
    }

    pub fn read_all(path: impl AsRef<Path>) -> Result<Vec<EventEnvelope>, JournalError> {
        Self::read_all_with_limits(path, JournalLimits::default())
    }

    pub fn read_all_with_limits(
        path: impl AsRef<Path>,
        limits: JournalLimits,
    ) -> Result<Vec<EventEnvelope>, JournalError> {
        validate_limits(limits)?;
        let path = path.as_ref();
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file_bytes = path.metadata()?.len();
        if file_bytes > limits.max_journal_bytes {
            return Err(JournalError::JournalTooLarge {
                actual: file_bytes,
                maximum: limits.max_journal_bytes,
            });
        }
        let mut bytes = Vec::with_capacity(file_bytes as usize);
        File::open(path)?.read_to_end(&mut bytes)?;
        if bytes.is_empty() {
            return Ok(Vec::new());
        }
        if !bytes.ends_with(b"\n") {
            return Err(JournalError::IncompleteTail);
        }

        let mut events = Vec::new();
        let mut run_id: Option<String> = None;
        let mut predecessor: Option<String> = None;

        for (index, line) in BufReader::new(bytes.as_slice()).lines().enumerate() {
            let line_number = index + 1;
            let line = line?;
            if line.len() as u64 + 1 > limits.max_event_bytes {
                return Err(JournalError::EventTooLarge {
                    actual: line.len() as u64 + 1,
                    maximum: limits.max_event_bytes,
                });
            }
            let event: EventEnvelope =
                serde_json::from_str(&line).map_err(|source| JournalError::Json {
                    line: line_number,
                    source,
                })?;
            let expected_sequence = line_number as u64;
            if event.schema_version != EVENT_SCHEMA_VERSION {
                return Err(JournalError::UnsupportedSchema {
                    sequence: event.sequence,
                    actual: event.schema_version,
                });
            }
            if event.sequence != expected_sequence {
                return Err(JournalError::Sequence {
                    expected: expected_sequence,
                    actual: event.sequence,
                });
            }
            match &run_id {
                Some(expected) if expected != &event.run_id => {
                    return Err(JournalError::RunMismatch {
                        sequence: event.sequence,
                    });
                }
                None => run_id = Some(event.run_id.clone()),
                _ => {}
            }
            if event.predecessor_digest != predecessor {
                return Err(JournalError::Predecessor {
                    sequence: event.sequence,
                });
            }
            if !event.has_valid_digest() {
                return Err(JournalError::Digest {
                    sequence: event.sequence,
                });
            }
            predecessor = Some(event.digest.clone());
            events.push(event);
        }
        Ok(events)
    }

    pub fn append(&mut self, event: &EventEnvelope) -> Result<(), JournalError> {
        let normal_limit = self
            .limits
            .max_journal_bytes
            .saturating_sub(self.limits.terminal_reserve_bytes);
        self.append_with_ceiling(event, normal_limit)
    }

    pub fn append_terminal(&mut self, event: &EventEnvelope) -> Result<(), JournalError> {
        self.append_with_ceiling(event, self.limits.max_journal_bytes)
    }

    fn append_with_ceiling(
        &mut self,
        event: &EventEnvelope,
        ceiling: u64,
    ) -> Result<(), JournalError> {
        let expected = self.last_sequence + 1;
        if event.sequence != expected {
            return Err(JournalError::AppendSequence {
                expected,
                actual: event.sequence,
            });
        }
        if event.predecessor_digest != self.last_digest || !event.has_valid_digest() {
            return Err(JournalError::Predecessor {
                sequence: event.sequence,
            });
        }

        let bytes = serde_json::to_vec(event).map_err(|source| JournalError::Json {
            line: event.sequence as usize,
            source,
        })?;
        let record_bytes = bytes.len() as u64 + 1;
        if record_bytes > self.limits.max_event_bytes {
            return Err(JournalError::EventTooLarge {
                actual: record_bytes,
                maximum: self.limits.max_event_bytes,
            });
        }
        if self.bytes_written.saturating_add(record_bytes) > ceiling {
            return Err(JournalError::CapacityExhausted { maximum: ceiling });
        }

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(&bytes)?;
        file.write_all(b"\n")?;
        file.flush()?;
        file.sync_data()?;

        self.last_sequence = event.sequence;
        self.last_digest = Some(event.digest.clone());
        self.bytes_written += record_bytes;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The sequence of the last record this journal holds.
    ///
    /// A reader that keeps its own projection of the journal compares it against this number to
    /// learn, without reading the file again, whether the record has moved past what it applied.
    pub fn last_sequence(&self) -> u64 {
        self.last_sequence
    }

    pub fn read_committed(&self) -> Result<Vec<EventEnvelope>, JournalError> {
        Self::read_all_with_limits(&self.path, self.limits)
    }
}

fn validate_limits(limits: JournalLimits) -> Result<(), JournalError> {
    if limits.max_event_bytes == 0
        || limits.max_journal_bytes == 0
        || limits.terminal_reserve_bytes < limits.max_event_bytes
        || limits.terminal_reserve_bytes >= limits.max_journal_bytes
    {
        Err(JournalError::InvalidLimits)
    } else {
        Ok(())
    }
}
