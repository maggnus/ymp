//! Version-1 stream framing, naming, inspection and decoding for the durable
//! journal, exactly as fixed by the durable Journal contract.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::path::Path;

use ymp_domain::SessionId;
use ymp_kernel::{JournalEntry, JournalError, Revision, SessionEvent};

use crate::crc;
use crate::payload;

/// Magic bytes starting every stream header.
const MAGIC: [u8; 4] = *b"YMPJ";
/// The on-disk format version this adapter reads and writes.
pub(super) const FORMAT_VERSION: u16 = 1;
/// Length of the stream header (magic plus u16 version).
pub(super) const HEADER_LEN: usize = 6;
/// Record frame tag byte.
const TAG_RECORD: u8 = 0x01;
/// Batch completion frame tag byte.
const TAG_COMPLETION: u8 = 0x02;
/// A record payload is at most 2^20 bytes.
pub(super) const MAX_PAYLOAD_BYTES: usize = payload::MAX_PAYLOAD_BYTES;
/// A batch carries at most 256 records.
pub(super) const MAX_BATCH_RECORDS: usize = 256;
/// A stream holds at most 2^32 records.
pub(super) const MAX_STREAM_RECORDS: u64 = 1 << 32;
/// This adapter accepts session IDs of at most 100 UTF-8 bytes.
pub(super) const MAX_SESSION_ID_BYTES: usize = 100;

/// Length of a durable-prefix descriptor: u64 prefix length plus u32 digest.
const DESCRIPTOR_LEN: usize = 12;

pub(super) fn header_bytes() -> [u8; HEADER_LEN] {
    let mut header = [0u8; HEADER_LEN];
    header[..4].copy_from_slice(&MAGIC);
    header[4..].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
    header
}

/// Encodes one record frame: tag, u32 payload length, u64 revision, payload,
/// and a CRC-32 over all preceding bytes of this record.
pub(super) fn record_frame(revision: u64, payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(HEADER_LEN + payload.len() + 4 + 5);
    frame.push(TAG_RECORD);
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(&revision.to_le_bytes());
    frame.extend_from_slice(payload);
    let checksum = crc::crc32(&frame);
    frame.extend_from_slice(&checksum.to_le_bytes());
    frame
}

/// Encodes one batch completion frame: tag, u32 record count, and a CRC-32
/// over the full record frames of the batch followed by the completion tag
/// and count (the checksum field itself excluded). The returned frame covers
/// only the completion bytes; the record frames are written separately.
pub(super) fn completion_frame(record_frames: &[u8], count: u32) -> Vec<u8> {
    let mut frame = Vec::with_capacity(9);
    frame.push(TAG_COMPLETION);
    frame.extend_from_slice(&count.to_le_bytes());
    let checksum = crc::crc32_extend(crc::crc32(record_frames), &frame);
    frame.extend_from_slice(&checksum.to_le_bytes());
    frame
}

/// Assigns contiguous revisions to a batch starting after `expected` and
/// returns the last revision of the batch.
pub(super) fn assign_revisions(expected: Revision, count: usize) -> Result<Revision, JournalError> {
    let mut attempted = expected;
    for _ in 0..count {
        attempted = attempted
            .checked_next()
            .ok_or(JournalError::RevisionOverflow)?;
    }
    Ok(attempted)
}

/// The stream name of a session: the lowercase hexadecimal encoding of the
/// UTF-8 bytes of the session ID.
pub(super) fn stream_stem(session_id: &SessionId) -> String {
    encode_stem(session_id.as_str())
}

fn encode_stem(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut stem = String::with_capacity(value.len() * 2);
    for &byte in value.as_bytes() {
        stem.push(HEX[(byte >> 4) as usize] as char);
        stem.push(HEX[(byte & 0x0F) as usize] as char);
    }
    stem
}

/// Decodes a stream stem back into a session ID. Only even-length lowercase
/// hexadecimal encodings of valid session IDs are accepted.
pub(super) fn decode_stem(stem: &str) -> Option<SessionId> {
    if stem.is_empty() || stem.len() % 2 != 0 {
        return None;
    }
    let mut bytes = Vec::with_capacity(stem.len() / 2);
    let characters: Vec<char> = stem.chars().collect();
    for pair in characters.chunks(2) {
        let high = pair[0].to_digit(16)?;
        let low = pair[1].to_digit(16)?;
        if pair[0].is_ascii_uppercase() || pair[1].is_ascii_uppercase() {
            return None;
        }
        bytes.push((high * 16 + low) as u8);
    }
    let value = std::str::from_utf8(&bytes).ok()?;
    SessionId::new(value).ok()
}

/// Returns the stem if `name` is a temporary descriptor file name
/// (`<stem>.commit.tmp-<16 lowercase hex digits>`).
pub(super) fn temp_file_stem(name: &str) -> Option<String> {
    let (stem, suffix) = name.split_once(".commit.tmp-")?;
    if suffix.len() != 16
        || !suffix
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    if stem.is_empty() {
        return None;
    }
    Some(stem.to_owned())
}

/// The classification of one log/commit pair, as fixed by the contract's
/// open-time recovery matrix.
#[derive(Debug)]
pub(super) enum Inspection {
    /// Neither file exists: an unknown session.
    Absent,
    /// Nothing is committed; the stream rests at `Revision::INITIAL`.
    /// `committed_len` is 0 (no descriptor or empty prefix) or the header
    /// length; `digest` is set when a descriptor has been published.
    Initial {
        committed_len: u64,
        digest: Option<u32>,
        log_len: u64,
        log_has_valid_header: bool,
    },
    /// A consistent non-empty committed prefix; `bytes` covers the whole
    /// prefix including the header and the digest has been verified.
    Committed {
        committed_len: u64,
        digest: u32,
        log_len: u64,
        bytes: Vec<u8>,
    },
    /// A recognized stream header of an unknown format version.
    Unsupported(u16),
    /// A corrupted store for this stream.
    Corrupted(String),
}

pub(super) fn log_path(sessions_dir: &Path, stem: &str) -> std::path::PathBuf {
    sessions_dir.join(format!("{stem}.log"))
}

pub(super) fn commit_path(sessions_dir: &Path, stem: &str) -> std::path::PathBuf {
    sessions_dir.join(format!("{stem}.commit"))
}

fn adapter_io(context: &str, error: io::Error) -> JournalError {
    JournalError::AdapterFailure {
        message: format!("{context}: {error}"),
    }
}

fn read_exact_prefix(path: &Path, length: u64) -> io::Result<Vec<u8>> {
    let file = File::open(path)?;
    let mut buffer = vec![0u8; length as usize];
    let mut reader = file.take(length);
    reader.read_exact(&mut buffer)?;
    Ok(buffer)
}

/// Classifies the log/commit pair for one stream under `sessions_dir`.
///
/// Ordinary I/O failures become `AdapterFailure`; store inconsistency and
/// committed-prefix damage become `Corruption`; a recognized header of an
/// unknown version becomes `UnsupportedFormat` (reported by the caller).
pub(super) fn inspect(sessions_dir: &Path, stem: &str) -> Result<Inspection, JournalError> {
    let log = log_path(sessions_dir, stem);
    let commit = commit_path(sessions_dir, stem);

    let log_len = match fs::metadata(&log) {
        Ok(metadata) if metadata.is_file() => Some(metadata.len()),
        Ok(_) => {
            return Ok(Inspection::Corrupted(format!(
                "stream {stem} log path is not a regular file"
            )));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(adapter_io("cannot stat the stream log", error)),
    };

    let descriptor = match fs::read(&commit) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(adapter_io("cannot read the commit descriptor", error)),
    };

    let Some(log_len) = log_len else {
        return match descriptor {
            None => Ok(Inspection::Absent),
            Some(_) => Ok(Inspection::Corrupted(format!(
                "commit descriptor for stream {stem} exists without its log"
            ))),
        };
    };

    // Header examination over the first bytes of the log whenever present.
    let mut header_ok = false;
    let mut unsupported: Option<u16> = None;
    let mut corrupted_header = false;
    if log_len >= HEADER_LEN as u64 {
        let header = read_exact_prefix(&log, HEADER_LEN as u64)
            .map_err(|error| adapter_io("cannot read the stream header", error))?;
        if header[..4] == MAGIC {
            let version = u16::from_le_bytes([header[4], header[5]]);
            if version == FORMAT_VERSION {
                header_ok = true;
            } else {
                unsupported = Some(version);
            }
        } else {
            corrupted_header = true;
        }
    }

    if let Some(version) = unsupported {
        return Ok(Inspection::Unsupported(version));
    }

    let Some(descriptor) = descriptor else {
        // No published descriptor: nothing is committed, so a short, partial
        // or complete header (with any uncommitted suffix) is Initial.
        return Ok(Inspection::Initial {
            committed_len: 0,
            digest: None,
            log_len,
            log_has_valid_header: header_ok,
        });
    };

    if descriptor.len() != DESCRIPTOR_LEN {
        return Ok(Inspection::Corrupted(format!(
            "commit descriptor for stream {stem} is malformed"
        )));
    }
    let committed_len = u64::from_le_bytes(descriptor[..8].try_into().expect("u64 slice"));
    let digest = u32::from_le_bytes(descriptor[8..].try_into().expect("u32 slice"));

    if corrupted_header && committed_len > 0 {
        return Ok(Inspection::Corrupted(format!(
            "committed prefix of stream {stem} does not begin with the stream header magic"
        )));
    }
    if committed_len > log_len {
        return Ok(Inspection::Corrupted(format!(
            "descriptor of stream {stem} names {committed_len} committed bytes but the log holds {log_len}"
        )));
    }
    if committed_len == 0 {
        let empty_digest = crc::crc32(&[]);
        if digest != empty_digest {
            return Ok(Inspection::Corrupted(format!(
                "descriptor of stream {stem} names an empty prefix with a non-empty digest"
            )));
        }
        return Ok(Inspection::Initial {
            committed_len: 0,
            digest: Some(digest),
            log_len,
            log_has_valid_header: header_ok,
        });
    }
    if committed_len < HEADER_LEN as u64 {
        return Ok(Inspection::Corrupted(format!(
            "committed prefix of stream {stem} is shorter than the stream header"
        )));
    }

    let bytes = read_exact_prefix(&log, committed_len)
        .map_err(|error| adapter_io("cannot read the committed prefix", error))?;
    if crc::crc32(&bytes) != digest {
        return Ok(Inspection::Corrupted(format!(
            "descriptor digest of stream {stem} does not match the committed log bytes"
        )));
    }
    if committed_len == HEADER_LEN as u64 {
        return Ok(Inspection::Initial {
            committed_len,
            digest: Some(digest),
            log_len,
            log_has_valid_header: header_ok,
        });
    }
    Ok(Inspection::Committed {
        committed_len,
        digest,
        log_len,
        bytes,
    })
}

/// Decodes the committed region of one stream (header included). Every
/// violation of the version-1 framing, checksum, revision or payload rules is
/// a typed `Corruption`.
pub(super) fn decode_committed(bytes: &[u8]) -> Result<Vec<JournalEntry>, JournalError> {
    let corruption = |message: String| JournalError::Corruption { message };
    if bytes.len() < HEADER_LEN {
        return Err(corruption(
            "committed prefix is shorter than the header".to_owned(),
        ));
    }
    if bytes[..4] != MAGIC {
        return Err(corruption(
            "committed prefix does not begin with the header magic".to_owned(),
        ));
    }
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != FORMAT_VERSION {
        return Err(JournalError::UnsupportedFormat { version });
    }

    let mut entries = Vec::new();
    let mut position = HEADER_LEN;
    let mut batch_start = HEADER_LEN;
    let mut records_in_batch = 0usize;
    let mut next_revision = 1u64;

    while position < bytes.len() {
        let tag = bytes[position];
        match tag {
            TAG_RECORD => {
                if bytes.len() < position + 13 {
                    return Err(corruption(
                        "record header crosses the committed boundary".to_owned(),
                    ));
                }
                let payload_len = u32::from_le_bytes(
                    bytes[position + 1..position + 5]
                        .try_into()
                        .expect("u32 slice"),
                ) as usize;
                if payload_len > MAX_PAYLOAD_BYTES {
                    return Err(corruption(format!(
                        "record payload length {payload_len} exceeds the {MAX_PAYLOAD_BYTES}-byte limit"
                    )));
                }
                let payload_start = position + 13;
                let checksum_start = payload_start + payload_len;
                if bytes.len() < checksum_start + 4 {
                    return Err(corruption(
                        "record frame crosses the committed boundary".to_owned(),
                    ));
                }
                let stored_checksum = u32::from_le_bytes(
                    bytes[checksum_start..checksum_start + 4]
                        .try_into()
                        .expect("u32 slice"),
                );
                if crc::crc32(&bytes[position..checksum_start]) != stored_checksum {
                    return Err(corruption("record checksum mismatch".to_owned()));
                }
                let revision_value = u64::from_le_bytes(
                    bytes[position + 5..position + 13]
                        .try_into()
                        .expect("u64 slice"),
                );
                if revision_value != next_revision {
                    return Err(corruption(format!(
                        "record revision {revision_value} is not the expected contiguous revision {next_revision}"
                    )));
                }
                if revision_value > MAX_STREAM_RECORDS {
                    return Err(corruption(format!(
                        "stream record count exceeds the {MAX_STREAM_RECORDS}-record limit"
                    )));
                }
                records_in_batch += 1;
                if records_in_batch > MAX_BATCH_RECORDS {
                    return Err(corruption(format!(
                        "batch exceeds the {MAX_BATCH_RECORDS}-record limit"
                    )));
                }
                let payload = &bytes[payload_start..checksum_start];
                let event = decode_event_payload(payload)?;
                entries.push(JournalEntry::new(Revision::new(revision_value), event));
                next_revision += 1;
                position = checksum_start + 4;
            }
            TAG_COMPLETION => {
                if bytes.len() < position + 9 {
                    return Err(corruption(
                        "batch completion frame crosses the committed boundary".to_owned(),
                    ));
                }
                let count = u32::from_le_bytes(
                    bytes[position + 1..position + 5]
                        .try_into()
                        .expect("u32 slice"),
                );
                let stored_checksum = u32::from_le_bytes(
                    bytes[position + 5..position + 9]
                        .try_into()
                        .expect("u32 slice"),
                );
                if crc::crc32(&bytes[batch_start..position + 5]) != stored_checksum {
                    return Err(corruption("batch completion checksum mismatch".to_owned()));
                }
                if count == 0 {
                    return Err(corruption("batch completion with zero records".to_owned()));
                }
                if count as usize != records_in_batch {
                    return Err(corruption(format!(
                        "batch completion count {count} does not match {records_in_batch} record frames"
                    )));
                }
                records_in_batch = 0;
                position += 9;
                batch_start = position;
            }
            other => {
                return Err(corruption(format!("unknown frame tag {other:#04x}")));
            }
        }
    }

    if records_in_batch != 0 {
        return Err(corruption(
            "committed prefix ends inside a batch".to_owned(),
        ));
    }
    Ok(entries)
}

fn decode_event_payload(payload: &[u8]) -> Result<SessionEvent, JournalError> {
    payload::decode_payload(payload).map_err(|reason| JournalError::Corruption {
        message: format!("malformed record payload: {reason}"),
    })
}

/// Synchronizes an open directory so that entry changes inside it are durable.
pub(super) fn sync_dir(path: &Path) -> io::Result<()> {
    File::open(path)?.sync_all()
}

/// Truncates a stream log to `length` and makes the truncation durable.
pub(super) fn truncate_log(
    sessions_dir: &Path,
    stem: &str,
    length: u64,
) -> Result<(), JournalError> {
    let file = OpenOptions::new()
        .write(true)
        .open(log_path(sessions_dir, stem))
        .map_err(|error| adapter_io("cannot open the stream log for truncation", error))?;
    file.set_len(length)
        .and_then(|()| file.sync_all())
        .map_err(|error| adapter_io("cannot truncate the stream log", error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_stems_are_lowercase_hex_and_reversible() {
        let id = SessionId::new("Aä🚀z").expect("valid session ID");
        let stem = stream_stem(&id);
        assert!(
            stem.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
        assert_eq!(stem.len(), id.as_str().len() * 2);
        assert_eq!(decode_stem(&stem).as_ref(), Some(&id));
        assert_eq!(decode_stem("0"), None);
        assert_eq!(decode_stem("ZZ"), None);
        assert_eq!(decode_stem("0A"), None);
    }

    #[test]
    fn temp_file_names_recognize_exactly_the_contract_shape() {
        assert_eq!(
            temp_file_stem("ab.commit.tmp-0123456789abcdef").as_deref(),
            Some("ab")
        );
        assert_eq!(temp_file_stem("ab.commit.tmp-0123456789ABCDEF"), None);
        assert_eq!(temp_file_stem("ab.commit.tmp-0123456789abcde"), None);
        assert_eq!(temp_file_stem("ab.commit.tmp-0123456789abcdefg"), None);
        assert_eq!(temp_file_stem("ab.log"), None);
    }

    #[test]
    fn assign_revisions_detects_overflow_before_any_write() {
        assert_eq!(assign_revisions(Revision::INITIAL, 3), Ok(Revision::new(3)));
        assert_eq!(
            assign_revisions(Revision::new(u64::MAX), 1),
            Err(JournalError::RevisionOverflow)
        );
        assert_eq!(
            assign_revisions(Revision::new(u64::MAX - 1), 2),
            Err(JournalError::RevisionOverflow)
        );
    }

    #[test]
    fn decode_rejects_framing_violations() {
        let payload = br#"{"type":"session_cancelled","session_id":"s"}"#.to_vec();
        let frame = record_frame(1, &payload);
        let completion = completion_frame(&frame, 1);

        let mut stream = header_bytes().to_vec();
        stream.extend_from_slice(&frame);
        stream.extend_from_slice(&completion);
        let entries = decode_committed(&stream).expect("valid stream decodes");
        assert_eq!(entries.len(), 1);

        let with_unknown_tag = {
            let mut bytes = stream.clone();
            let tag_position = bytes.len() - completion.len();
            bytes[tag_position] = 0x03;
            bytes
        };
        assert!(matches!(
            decode_committed(&with_unknown_tag),
            Err(JournalError::Corruption { .. })
        ));

        let with_bad_record_crc = {
            let mut bytes = stream.clone();
            let frame_end = bytes.len() - completion.len();
            bytes[frame_end - 1] ^= 0xFF;
            bytes
        };
        assert!(matches!(
            decode_committed(&with_bad_record_crc),
            Err(JournalError::Corruption { .. })
        ));

        let non_contiguous = {
            let mut bytes = header_bytes().to_vec();
            bytes.extend_from_slice(&record_frame(2, &payload));
            bytes.extend_from_slice(&completion_frame(&bytes[HEADER_LEN..], 1));
            bytes
        };
        assert!(matches!(
            decode_committed(&non_contiguous),
            Err(JournalError::Corruption { .. })
        ));

        let ends_inside_batch = {
            let mut bytes = stream.clone();
            bytes.truncate(bytes.len() - completion.len());
            bytes
        };
        assert!(matches!(
            decode_committed(&ends_inside_batch),
            Err(JournalError::Corruption { .. })
        ));

        let count_mismatch = {
            let mut bytes = stream.clone();
            let index = bytes.len() - 5;
            bytes[index] = 2;
            bytes
        };
        assert!(matches!(
            decode_committed(&count_mismatch),
            Err(JournalError::Corruption { .. })
        ));
    }

    #[test]
    fn decode_rejects_oversized_declared_lengths_without_trusting_them() {
        let mut bytes = header_bytes().to_vec();
        bytes.push(TAG_RECORD);
        bytes.extend_from_slice(&(u32::MAX).to_le_bytes());
        bytes.extend_from_slice(&1u64.to_le_bytes());
        assert!(matches!(
            decode_committed(&bytes),
            Err(JournalError::Corruption { .. })
        ));
    }

    #[test]
    fn decode_rejects_unknown_header_version() {
        let mut stream = b"YMPJ".to_vec();
        stream.extend_from_slice(&2u16.to_le_bytes());
        assert_eq!(
            decode_committed(&stream),
            Err(JournalError::UnsupportedFormat { version: 2 })
        );
    }
}
