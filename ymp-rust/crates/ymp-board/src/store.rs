//! The durable section this plane keeps its own records in, and the evidence file it exports.
//!
//! The board is not authoritative, so its records do not belong in the control journal: a board
//! section is a separate directory, with a separate opening record, a separate version and a
//! separate reader. Nothing here appends to the journal, reads it or shares a schema with it, and
//! the two are therefore versioned apart.
//!
//! What is recorded is the fact sequence the kernel committed, and nothing else. A board is
//! restored by opening it on the terms its opening record states and replaying those facts in
//! order, which is the same path [`BoardLedger::replay`] already served in memory. There is no
//! second representation of the board on disk that could disagree with the facts, so a restored
//! board is the facts and cannot drift from them.
//!
//! A payload never reaches this module for the same reason it never reaches the kernel: a fact
//! carries the identity and the length of a payload, so what is written out is a digest and a byte
//! count. This module also names no address and starts no process, and every path it builds is one
//! of the three file names it declares, under the directory the caller named — an identifier out
//! of a board record is never a path component, so nothing a participant chose can decide what is
//! written where.
//!
//! Each record is chained: the first fact names the digest of the opening record, and every later
//! one names the digest of its predecessor. A record file whose tail was cut short, whose sequence
//! skips, whose chain breaks or which is missing altogether is refused by name. Restoration never
//! degrades into an empty board, because an empty board is a statement that nothing was ever said
//! and a lost record file is not evidence of that. A fact record standing where the opening terms
//! are missing is likewise refused rather than created over, because what stands there is an audit
//! record and creating a section would write an empty file across it.
//!
//! One writer holds a section at a time, by the operating system's exclusive lock rather than by
//! agreement between callers. Two writers would each continue the recorded sequence from the
//! position they last saw, and the second append would take positions the first had already
//! written; the section refuses the second holder when it opens rather than producing a record
//! that no longer restores. What a board offers to be appended is checked against the recorded
//! chain for the same reason: a board of the right length is not thereby the board these records
//! were written from.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::budget::CommunicationAllowance;
use crate::ledger::BoardLedger;
use crate::protocol::{BoardError, BoardEvent};
use crate::records::{
    AssessmentRecord, DeliveryReceipt, InterventionRecord, MessageRecord, VerdictRecord,
};
use crate::{BOARD_SCHEMA_VERSION, digest_bytes};

/// The directory a store gives this plane for its own records. It stands beside the control
/// plane's records rather than inside them.
pub const BOARD_SECTION: &str = "board";

/// The file naming the terms one board was opened on.
pub const OPENING_RECORD: &str = "board.json";

/// The file holding the fact sequence, one record per line.
pub const FACT_RECORD: &str = "facts.jsonl";

/// The file one writer of the section holds exclusively while it holds the section.
pub const SECTION_LOCK: &str = "writer.lock";

/// What the opening record declares itself to be, so a directory holding some other product's
/// JSON is refused rather than read as a board.
pub const BOARD_RECORD_KIND: &str = "ymp-board-records";

/// What an exported evidence file declares itself to be.
pub const BOARD_EVIDENCE_KIND: &str = "ymp-board-evidence";

/// The largest one recorded fact may be, in bytes.
pub const MAX_FACT_BYTES: u64 = 64 * 1024;

/// The largest the whole fact record may grow, in bytes.
pub const MAX_RECORD_BYTES: u64 = 16 * 1024 * 1024;

/// The suffix a file carries while it is still being written. A completed file appears under its
/// own name by a rename, so a reader sees it whole or not at all.
const STAGING_SUFFIX: &str = "tmp";

#[derive(Debug, Error)]
pub enum BoardRecordError {
    #[error("board record I/O error at {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "{} holds no {OPENING_RECORD}, so it is not a board section and no board was restored \
         from it",
        path.display()
    )]
    NoOpeningRecord { path: PathBuf },
    #[error(
        "the board section {} states its opening terms but holds no {FACT_RECORD}: the fact \
         sequence is missing rather than empty, and no board is restored from it",
        path.display()
    )]
    FactRecordMissing { path: PathBuf },
    #[error(
        "{} holds a {FACT_RECORD} and no {OPENING_RECORD}: the terms restoration begins from are \
         missing, and no section is created over a fact record that is still there",
        path.display()
    )]
    FactRecordWithoutOpening { path: PathBuf },
    #[error(
        "the board section {} is already held by another writer; a section is written by one",
        path.display()
    )]
    SectionHeld { path: PathBuf },
    #[error("the opening record at {} is not readable: {reason}", path.display())]
    UnreadableOpening { path: PathBuf, reason: String },
    #[error(
        "the board section states record version {found}; this build writes version {expected} \
         and migrates no board section"
    )]
    UnsupportedSchema { found: u32, expected: u32 },
    #[error(
        "the board section was opened with {field} {recorded} and is now addressed with {stated}"
    )]
    OpeningMismatch {
        field: &'static str,
        recorded: String,
        stated: String,
    },
    #[error("the fact record ends in an incomplete line and its final fact is not recovered")]
    IncompleteTail,
    #[error("fact record line {line} is not readable JSON: {source}")]
    Json {
        line: usize,
        #[source]
        source: serde_json::Error,
    },
    #[error("the fact record states sequence {actual} where sequence {expected} belongs")]
    Sequence { expected: u64, actual: u64 },
    #[error("the fact at sequence {sequence} does not name the record before it")]
    Predecessor { sequence: u64 },
    #[error("the fact at sequence {sequence} does not match the digest recorded for it")]
    Digest { sequence: u64 },
    #[error("a recorded fact of {actual} bytes exceeds the {maximum} one fact may occupy")]
    FactTooLarge { actual: u64, maximum: u64 },
    #[error("the fact record holds {actual} bytes; the limit is {maximum}")]
    RecordTooLarge { actual: u64, maximum: u64 },
    #[error(
        "the section already records {recorded} facts and the board offered holds {held}: the \
         board is not the one these records were written from"
    )]
    RecordAhead { recorded: u64, held: u64 },
    #[error(
        "the fact this board holds at position {sequence} is not the one recorded there: the \
         board is not the one these records were written from"
    )]
    RecordDivergence { sequence: u64 },
    #[error("a recorded fact was refused on replay: {0}")]
    Replay(#[from] BoardError),
    #[error("{} already exists and no export overwrites one", path.display())]
    ExportExists { path: PathBuf },
    #[error("the evidence file is not a {BOARD_EVIDENCE_KIND} record: {reason}")]
    UnreadableEvidence { reason: String },
}

/// The terms one board was opened on: what a restoration needs before the first fact means
/// anything.
///
/// The endowment belongs here rather than among the facts because no fact creates it. It is the
/// whole capacity this plane will ever hold, stated once when the board was opened.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BoardOpening {
    pub schema_version: u32,
    pub kind: String,
    pub controller: String,
    pub root_participant: String,
    pub endowment: CommunicationAllowance,
}

impl BoardOpening {
    fn new(controller: &str, root_participant: &str, endowment: CommunicationAllowance) -> Self {
        Self {
            schema_version: BOARD_SCHEMA_VERSION,
            kind: BOARD_RECORD_KIND.to_owned(),
            controller: controller.to_owned(),
            root_participant: root_participant.to_owned(),
            endowment,
        }
    }

    /// The identity of these terms. It is taken over the parsed record rather than over the file,
    /// so how the file was formatted never changes what the fact chain is anchored to.
    pub fn digest(&self) -> String {
        digest_bytes(&serde_json::to_vec(self).expect("opening terms serialize"))
    }

    /// The board these terms open, before any fact is replayed into it.
    fn board(&self) -> Result<BoardLedger, BoardError> {
        BoardLedger::new(&self.controller, &self.root_participant, self.endowment)
    }

    /// Why these terms are not a board record, if they are not. The kind is stated by the record
    /// itself, so a directory holding some other product's JSON is refused instead of read.
    fn wrong_kind(&self) -> Option<String> {
        (self.kind != BOARD_RECORD_KIND).then(|| {
            format!(
                "it names kind {} rather than {BOARD_RECORD_KIND}",
                self.kind
            )
        })
    }

    fn check_version(&self) -> Result<(), BoardRecordError> {
        if self.schema_version == BOARD_SCHEMA_VERSION {
            Ok(())
        } else {
            Err(BoardRecordError::UnsupportedSchema {
                found: self.schema_version,
                expected: BOARD_SCHEMA_VERSION,
            })
        }
    }

    fn expect_same_terms(&self, stated: &Self) -> Result<(), BoardRecordError> {
        let mismatch = |field: &'static str, recorded: String, stated: String| {
            Err(BoardRecordError::OpeningMismatch {
                field,
                recorded,
                stated,
            })
        };
        if self.controller != stated.controller {
            return mismatch(
                "controller",
                self.controller.clone(),
                stated.controller.clone(),
            );
        }
        if self.root_participant != stated.root_participant {
            return mismatch(
                "root participant",
                self.root_participant.clone(),
                stated.root_participant.clone(),
            );
        }
        if self.endowment != stated.endowment {
            return mismatch(
                "endowment",
                format!("{:?}", self.endowment.amounts()),
                format!("{:?}", stated.endowment.amounts()),
            );
        }
        Ok(())
    }
}

/// One recorded fact, in the position and the chain it was recorded in.
///
/// The fact itself is exactly what the kernel committed. What surrounds it is what makes a lost,
/// reordered or edited record detectable rather than silently readable.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RecordedFact {
    pub schema_version: u32,
    /// Position in the recorded sequence, counted from one.
    pub sequence: u64,
    /// The digest of the record before this one; for the first fact, the digest of the opening
    /// terms. A fact sequence therefore cannot be moved onto another board's terms unnoticed.
    pub predecessor_digest: String,
    pub digest: String,
    pub fact: BoardEvent,
}

impl RecordedFact {
    fn new(sequence: u64, predecessor_digest: &str, fact: &BoardEvent) -> Self {
        let mut record = Self {
            schema_version: BOARD_SCHEMA_VERSION,
            sequence,
            predecessor_digest: predecessor_digest.to_owned(),
            digest: String::new(),
            fact: fact.clone(),
        };
        record.digest = record.own_digest();
        record
    }

    /// The digest of everything this record states apart from the digest itself.
    fn own_digest(&self) -> String {
        let staged = Self {
            digest: String::new(),
            ..self.clone()
        };
        digest_bytes(&serde_json::to_vec(&staged).expect("a recorded fact serializes"))
    }

    fn has_valid_digest(&self) -> bool {
        self.digest == self.own_digest()
    }
}

/// Exclusive ownership of one board section, held for as long as the store that took it.
///
/// The lock is the operating system's and not a file this crate interprets: it is released when
/// the process holding it ends, however it ends, so a crash leaves no stale claim for an operator
/// to clear by hand. Two holders of one section would each believe their own fact sequence
/// continued the recorded one, and the second append would be recorded at positions the first had
/// already taken; the refusal here is what makes that a named error at the moment of opening
/// rather than a record that no longer restores.
#[derive(Debug)]
struct SectionLock {
    file: File,
}

impl SectionLock {
    fn acquire(directory: &Path) -> Result<Self, BoardRecordError> {
        create_dir_all(directory)?;
        let path = directory.join(SECTION_LOCK);
        let io = |source| BoardRecordError::Io {
            path: path.clone(),
            source,
        };
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(io)?;
        file.try_lock_exclusive()
            .map_err(|_| BoardRecordError::SectionHeld {
                path: directory.to_path_buf(),
            })?;
        Ok(Self { file })
    }
}

impl Drop for SectionLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

/// The board's own durable section: the terms it was opened on, and the facts it committed.
#[derive(Debug)]
pub struct BoardStore {
    directory: PathBuf,
    opening: BoardOpening,
    sequence: u64,
    /// The digest of the record at `sequence`, or the digest of the opening terms when this
    /// section holds no fact yet.
    last_digest: String,
    /// The digest of the record before `sequence`, which is what makes the recorded position
    /// checkable against a board without rereading the file. It is unused while `sequence` is
    /// zero.
    previous_digest: String,
    /// Whether the recorded prefix has been established, fact by fact, to be the prefix of the
    /// board this section is written from. A section created here starts with an empty prefix and
    /// nothing to establish; a section restored from a file holds records this process did not
    /// write, and the first append checks the whole of them.
    prefix_verified: bool,
    bytes_written: u64,
    /// Exclusive ownership of the section. The field is never read; dropping it is what releases
    /// the claim, and holding it is what refuses a second writer.
    _lock: SectionLock,
}

impl BoardStore {
    /// Address the board section under `directory`, creating it on the stated terms when it holds
    /// none, and restore the board it records.
    ///
    /// A section that already exists is restored on the terms it records. Stated terms that differ
    /// from the recorded ones are refused by name rather than quietly reopened, because a board
    /// whose controller, root participant or total capacity changed underneath its facts is not
    /// the board those facts were committed to.
    pub fn open(
        directory: impl AsRef<Path>,
        controller: &str,
        root_participant: &str,
        endowment: CommunicationAllowance,
    ) -> Result<(Self, BoardLedger), BoardRecordError> {
        let directory = directory.as_ref();
        let stated = BoardOpening::new(controller, root_participant, endowment);
        if opening_path(directory).is_file() {
            let (store, board) = Self::restore(directory)?;
            store.opening.expect_same_terms(&stated)?;
            return Ok((store, board));
        }
        let lock = SectionLock::acquire(directory)?;
        // A fact record standing where the opening terms are missing is a section that lost the
        // one file restoration begins from. Creating the section over it would write an empty
        // fact record across the audit that is still there, so what is missing is named and
        // nothing is written. The check follows the lock, because a section only one holder may
        // write is the only one whose files are stable enough to decide this on.
        if fact_path(directory).exists() {
            return Err(BoardRecordError::FactRecordWithoutOpening {
                path: directory.to_path_buf(),
            });
        }
        write_atomically(
            &opening_path(directory),
            &serde_json::to_vec_pretty(&stated).expect("opening terms serialize"),
        )?;
        // The fact record exists from the moment the section does. A board that has committed
        // nothing holds an empty file; a board whose file is gone holds no evidence of that, and
        // the difference is what keeps a lost record from reading as a silent empty board.
        write_atomically(&fact_path(directory), b"")?;
        let board = stated.board()?;
        Ok((
            Self {
                directory: directory.to_path_buf(),
                last_digest: stated.digest(),
                previous_digest: String::new(),
                prefix_verified: true,
                opening: stated,
                sequence: 0,
                bytes_written: 0,
                _lock: lock,
            },
            board,
        ))
    }

    /// Restore the board an existing section records. A section that is not there, or whose fact
    /// record is not there, is refused with the reason named.
    pub fn restore(directory: impl AsRef<Path>) -> Result<(Self, BoardLedger), BoardRecordError> {
        let directory = directory.as_ref();
        let lock = SectionLock::acquire(directory)?;
        let opening = read_opening(directory)?;
        let facts = read_facts(directory, &opening)?;
        let mut board = opening.board()?;
        for record in &facts {
            board.replay(&record.fact)?;
        }
        let last_digest = facts
            .last()
            .map_or_else(|| opening.digest(), |record| record.digest.clone());
        let previous_digest = match facts.len() {
            0 | 1 => opening.digest(),
            length => facts[length - 2].digest.clone(),
        };
        let bytes_written = file_bytes(&fact_path(directory))?;
        Ok((
            Self {
                directory: directory.to_path_buf(),
                opening,
                sequence: facts.len() as u64,
                last_digest,
                previous_digest,
                prefix_verified: false,
                bytes_written,
                _lock: lock,
            },
            board,
        ))
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub const fn opening(&self) -> &BoardOpening {
        &self.opening
    }

    /// How many facts this section holds.
    pub const fn recorded_facts(&self) -> u64 {
        self.sequence
    }

    /// Append every fact the board has committed beyond what this section already holds.
    ///
    /// Synchronizing against the board's own fact sequence, rather than against what a caller
    /// remembers to hand over, is what keeps the two from drifting: whatever path committed a fact
    /// — a command, a delivery, a replay — the fact is on the board and is written from there.
    /// The appended group reaches the file in one synchronized write, so an interrupted append
    /// leaves either whole records or an incomplete final line that restoration refuses.
    pub fn record(&mut self, board: &BoardLedger) -> Result<u64, BoardRecordError> {
        let held = board.facts().len() as u64;
        if held < self.sequence {
            return Err(BoardRecordError::RecordAhead {
                recorded: self.sequence,
                held,
            });
        }
        // A board of the right length is not thereby the board these records were written from.
        // Without a check on what it holds, another board's facts append onto this section and the
        // audit becomes two conversations no reader can separate.
        //
        // A section restored from a file has a recorded prefix this process did not write, so the
        // whole of it is rebuilt from the board's own facts and the resulting chain compared: that
        // establishes fact by fact that the board offered is the one these records came from. It
        // is done once, because afterwards the recorded prefix is what this store appended from a
        // board it had already checked. Every later append is checked at the last recorded
        // position, where recomputing the record from the fact the board holds there must
        // reproduce the digest it was written under.
        if self.sequence > 0 {
            let recomputed = if self.prefix_verified {
                RecordedFact::new(
                    self.sequence,
                    &self.previous_digest,
                    &board.facts()[self.sequence as usize - 1],
                )
                .digest
            } else {
                chain_digest(
                    &self.opening.digest(),
                    &board.facts()[..self.sequence as usize],
                )
            };
            if recomputed != self.last_digest {
                return Err(BoardRecordError::RecordDivergence {
                    sequence: self.sequence,
                });
            }
            self.prefix_verified = true;
        }
        let pending = &board.facts()[self.sequence as usize..];
        if pending.is_empty() {
            return Ok(0);
        }
        let mut sequence = self.sequence;
        let mut predecessor = self.last_digest.clone();
        let mut penultimate = self.previous_digest.clone();
        let mut bytes = Vec::new();
        for fact in pending {
            sequence += 1;
            let record = RecordedFact::new(sequence, &predecessor, fact);
            let line = serde_json::to_vec(&record).map_err(|source| BoardRecordError::Json {
                line: sequence as usize,
                source,
            })?;
            let occupied = line.len() as u64 + 1;
            if occupied > MAX_FACT_BYTES {
                return Err(BoardRecordError::FactTooLarge {
                    actual: occupied,
                    maximum: MAX_FACT_BYTES,
                });
            }
            penultimate = predecessor;
            predecessor = record.digest;
            bytes.extend_from_slice(&line);
            bytes.push(b'\n');
        }
        let total = self.bytes_written.saturating_add(bytes.len() as u64);
        if total > MAX_RECORD_BYTES {
            return Err(BoardRecordError::RecordTooLarge {
                actual: total,
                maximum: MAX_RECORD_BYTES,
            });
        }

        let path = fact_path(&self.directory);
        let io = |source| BoardRecordError::Io {
            path: path.clone(),
            source,
        };
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(io)?;
        file.write_all(&bytes).map_err(io)?;
        file.flush().map_err(io)?;
        file.sync_data().map_err(io)?;

        let appended = sequence - self.sequence;
        self.sequence = sequence;
        self.last_digest = predecessor;
        self.previous_digest = penultimate;
        self.bytes_written = total;
        Ok(appended)
    }

    /// Write the board's evidence to `path`: the terms it was opened on, the chained fact
    /// sequence, and the projections a reader of that evidence checks the facts against.
    ///
    /// The file is a function of the board state alone. Nothing observed at the moment of writing
    /// enters it — no wall clock, no path, no ordinal — so exporting the same state twice produces
    /// the same bytes, and two exports that differ are evidence that the state differed.
    pub fn export_evidence(
        &self,
        board: &BoardLedger,
        path: impl AsRef<Path>,
    ) -> Result<BoardEvidence, BoardRecordError> {
        let path = path.as_ref();
        if path.exists() {
            return Err(BoardRecordError::ExportExists {
                path: path.to_path_buf(),
            });
        }
        let evidence = self.evidence(board);
        let mut bytes = serde_json::to_vec_pretty(&evidence).expect("board evidence serializes");
        bytes.push(b'\n');
        if let Some(parent) = path.parent() {
            create_dir_all(parent)?;
        }
        write_atomically(path, &bytes)?;
        Ok(evidence)
    }

    /// The evidence for this board, as bytes are exported from it.
    pub fn evidence(&self, board: &BoardLedger) -> BoardEvidence {
        let mut predecessor = self.opening.digest();
        let mut facts = Vec::with_capacity(board.facts().len());
        for (index, fact) in board.facts().iter().enumerate() {
            let record = RecordedFact::new(index as u64 + 1, &predecessor, fact);
            predecessor = record.digest.clone();
            facts.push(record);
        }
        BoardEvidence {
            schema_version: BOARD_SCHEMA_VERSION,
            kind: BOARD_EVIDENCE_KIND.to_owned(),
            opening: self.opening.clone(),
            opening_digest: self.opening.digest(),
            facts,
            audit: board.audit().to_vec(),
            receipts: board.receipts().to_vec(),
            assessments: board.assessments().to_vec(),
            verdicts: board.verdicts().cloned().collect(),
            interventions: board.interventions().cloned().collect(),
        }
    }
}

/// One board's exported evidence: what it was opened as, every fact it committed in a chain a
/// reader can check on its own, and the projections those facts produce.
///
/// The audit projection is exported in full and is not filtered by salience or by any grant: what
/// expires is the active projection a participant is delivered, and evidence of what was said is
/// exactly what must outlive it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BoardEvidence {
    pub schema_version: u32,
    pub kind: String,
    pub opening: BoardOpening,
    pub opening_digest: String,
    pub facts: Vec<RecordedFact>,
    pub audit: Vec<MessageRecord>,
    pub receipts: Vec<DeliveryReceipt>,
    pub assessments: Vec<AssessmentRecord>,
    pub verdicts: Vec<VerdictRecord>,
    pub interventions: Vec<InterventionRecord>,
}

impl BoardEvidence {
    /// Rebuild the board this evidence was exported from, checking the chain on the way.
    ///
    /// A reader that holds only the file establishes for itself that the facts are the ones the
    /// terms were opened with and that none was inserted, dropped or edited; what it then holds is
    /// a board, and the projections stated in the file are checked against the ones that board
    /// produces rather than believed.
    pub fn replay(&self) -> Result<BoardLedger, BoardRecordError> {
        if self.kind != BOARD_EVIDENCE_KIND {
            return Err(BoardRecordError::UnreadableEvidence {
                reason: format!(
                    "it names kind {} rather than {BOARD_EVIDENCE_KIND}",
                    self.kind
                ),
            });
        }
        if let Some(reason) = self.opening.wrong_kind() {
            return Err(BoardRecordError::UnreadableEvidence {
                reason: format!("the opening terms it states are not board terms: {reason}"),
            });
        }
        self.opening.check_version()?;
        if self.opening_digest != self.opening.digest() {
            return Err(BoardRecordError::UnreadableEvidence {
                reason: "the stated opening digest does not identify the stated opening terms"
                    .to_owned(),
            });
        }
        let mut board = self.opening.board()?;
        let mut predecessor = self.opening_digest.clone();
        for (index, record) in self.facts.iter().enumerate() {
            check_chain(record, index as u64 + 1, &predecessor)?;
            predecessor = record.digest.clone();
            board.replay(&record.fact)?;
        }
        Ok(board)
    }

    /// Read evidence from the exact bytes of an exported file.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, BoardRecordError> {
        serde_json::from_slice(bytes).map_err(|source| BoardRecordError::UnreadableEvidence {
            reason: source.to_string(),
        })
    }
}

fn opening_path(directory: &Path) -> PathBuf {
    directory.join(OPENING_RECORD)
}

fn fact_path(directory: &Path) -> PathBuf {
    directory.join(FACT_RECORD)
}

fn read_opening(directory: &Path) -> Result<BoardOpening, BoardRecordError> {
    let path = opening_path(directory);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(BoardRecordError::NoOpeningRecord {
                path: directory.to_path_buf(),
            });
        }
        Err(source) => return Err(BoardRecordError::Io { path, source }),
    };
    let opening: BoardOpening =
        serde_json::from_slice(&bytes).map_err(|error| BoardRecordError::UnreadableOpening {
            path: path.clone(),
            reason: error.to_string(),
        })?;
    if let Some(reason) = opening.wrong_kind() {
        return Err(BoardRecordError::UnreadableOpening { path, reason });
    }
    opening.check_version()?;
    Ok(opening)
}

fn read_facts(
    directory: &Path,
    opening: &BoardOpening,
) -> Result<Vec<RecordedFact>, BoardRecordError> {
    let path = fact_path(directory);
    let io = |source| BoardRecordError::Io {
        path: path.clone(),
        source,
    };
    let occupied = match path.metadata() {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(BoardRecordError::FactRecordMissing {
                path: directory.to_path_buf(),
            });
        }
        Err(source) => return Err(io(source)),
    };
    if occupied > MAX_RECORD_BYTES {
        return Err(BoardRecordError::RecordTooLarge {
            actual: occupied,
            maximum: MAX_RECORD_BYTES,
        });
    }
    let mut bytes = Vec::with_capacity(occupied as usize);
    File::open(&path)
        .map_err(io)?
        .read_to_end(&mut bytes)
        .map_err(io)?;
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if !bytes.ends_with(b"\n") {
        return Err(BoardRecordError::IncompleteTail);
    }

    let mut facts = Vec::new();
    let mut predecessor = opening.digest();
    for (index, line) in BufReader::new(bytes.as_slice()).lines().enumerate() {
        let sequence = index as u64 + 1;
        let line = line.map_err(io)?;
        if line.len() as u64 + 1 > MAX_FACT_BYTES {
            return Err(BoardRecordError::FactTooLarge {
                actual: line.len() as u64 + 1,
                maximum: MAX_FACT_BYTES,
            });
        }
        let record: RecordedFact =
            serde_json::from_str(&line).map_err(|source| BoardRecordError::Json {
                line: sequence as usize,
                source,
            })?;
        check_chain(&record, sequence, &predecessor)?;
        predecessor = record.digest.clone();
        facts.push(record);
    }
    Ok(facts)
}

/// The digest one chain of facts arrives at, starting from `opening_digest`. It is what the last
/// record of that chain states, and it is therefore what a recorded position is compared against.
fn chain_digest(opening_digest: &str, facts: &[BoardEvent]) -> String {
    let mut predecessor = opening_digest.to_owned();
    for (index, fact) in facts.iter().enumerate() {
        predecessor = RecordedFact::new(index as u64 + 1, &predecessor, fact).digest;
    }
    predecessor
}

/// Whether one record stands where it claims to, after the record it claims to follow, stating the
/// digest of what it in fact holds.
fn check_chain(
    record: &RecordedFact,
    sequence: u64,
    predecessor: &str,
) -> Result<(), BoardRecordError> {
    if record.schema_version != BOARD_SCHEMA_VERSION {
        return Err(BoardRecordError::UnsupportedSchema {
            found: record.schema_version,
            expected: BOARD_SCHEMA_VERSION,
        });
    }
    if record.sequence != sequence {
        return Err(BoardRecordError::Sequence {
            expected: sequence,
            actual: record.sequence,
        });
    }
    if record.predecessor_digest != predecessor {
        return Err(BoardRecordError::Predecessor { sequence });
    }
    if !record.has_valid_digest() {
        return Err(BoardRecordError::Digest { sequence });
    }
    Ok(())
}

fn file_bytes(path: &Path) -> Result<u64, BoardRecordError> {
    match path.metadata() {
        Ok(metadata) => Ok(metadata.len()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(source) => Err(BoardRecordError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn create_dir_all(path: &Path) -> Result<(), BoardRecordError> {
    fs::create_dir_all(path).map_err(|source| BoardRecordError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Write a whole file, so that a reader arriving during the write sees the earlier file or the
/// later one and never half of either.
fn write_atomically(path: &Path, bytes: &[u8]) -> Result<(), BoardRecordError> {
    let mut staged = path.as_os_str().to_owned();
    staged.push(".");
    staged.push(STAGING_SUFFIX);
    let staged = PathBuf::from(staged);
    let io = |source| BoardRecordError::Io {
        path: path.to_path_buf(),
        source,
    };
    let mut file = File::create(&staged).map_err(io)?;
    file.write_all(bytes).map_err(io)?;
    file.flush().map_err(io)?;
    file.sync_data().map_err(io)?;
    drop(file);
    fs::rename(&staged, path).map_err(io)
}
