//! CRC-32 (ISO-HDLC) checksums used by the durable journal format.
//!
//! The polynomial is the reflected `0xEDB88320` form of the standard CRC-32
//! (init `0xFFFFFFFF`, final XOR `0xFFFFFFFF`), matching the checksum named by
//! the durable Journal contract.

/// Precomputed table for the ISO-HDLC CRC-32 polynomial.
const CRC_TABLE: [u32; 256] = build_crc_table();

const fn build_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut index = 0usize;
    while index < 256 {
        let mut value = index as u32;
        let mut bit = 0;
        while bit < 8 {
            value = if value & 1 != 0 {
                0xEDB8_8320 ^ (value >> 1)
            } else {
                value >> 1
            };
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
}

/// Feeds `bytes` into the raw CRC register `state` (no init or final XOR).
fn crc_feed(mut state: u32, bytes: &[u8]) -> u32 {
    for &byte in bytes {
        let index = ((state ^ byte as u32) & 0xFF) as usize;
        state = CRC_TABLE[index] ^ (state >> 8);
    }
    state
}

/// CRC-32 (ISO-HDLC) of `bytes`.
pub(super) fn crc32(bytes: &[u8]) -> u32 {
    crc_feed(0xFFFF_FFFF, bytes) ^ 0xFFFF_FFFF
}

/// Extends a finished CRC-32 over `previous_final` with more bytes, so that
/// `crc32_extend(crc32(a), b) == crc32(a ++ b)`.
///
/// A finished ISO-HDLC CRC-32 is the raw register XOR `0xFFFFFFFF`, so the
/// register can be recovered by XOR-ing again before feeding the continuation.
pub(super) fn crc32_extend(previous_final: u32, bytes: &[u8]) -> u32 {
    crc_feed(previous_final ^ 0xFFFF_FFFF, bytes) ^ 0xFFFF_FFFF
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vectors_match_iso_hdlc() {
        assert_eq!(crc32(b""), 0x0000_0000);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(
            crc32(b"The quick brown fox jumps over the lazy dog"),
            0x414F_A339
        );
    }

    #[test]
    fn extend_equals_full_recomputation() {
        let head = b"YMPJ\x01\x00record bytes";
        let tail = [0u8, 1, 2, 3, 255, 128, 42];
        let mut whole = head.to_vec();
        whole.extend_from_slice(&tail);
        assert_eq!(crc32_extend(crc32(head), &tail), crc32(&whole));
        assert_eq!(crc32_extend(crc32(b""), &tail), crc32(&tail));
    }
}
