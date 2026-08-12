use std::io::{self, BufRead, Read};

use bstr::{ByteSlice, io::BufReadExt};

struct EmptyThenError {
    fill_calls: usize,
}

impl Read for EmptyThenError {
    fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("read after reported EOF"))
    }
}

impl BufRead for EmptyThenError {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.fill_calls += 1;
        if self.fill_calls == 1 {
            Ok(&[])
        } else {
            Err(io::Error::other("second fill_buf after reported EOF"))
        }
    }

    fn consume(&mut self, _amount: usize) {}
}

#[test]
fn protected_empty_fill_buffer_is_terminal_eof() {
    let mut reader = EmptyThenError { fill_calls: 0 };
    let mut records = 0;
    reader
        .for_byte_record_with_terminator(b'\n', |_| {
            records += 1;
            Ok(true)
        })
        .unwrap();
    assert_eq!(records, 0);
    assert_eq!(reader.fill_calls, 1);
}

#[test]
fn protected_invalid_utf8_debug_uses_lowercase_hex() {
    assert_eq!(format!("{:?}", b"\xed\xa0\x80\xff".as_bstr()), r#""\xed\xa0\x80\xff""#);
}
