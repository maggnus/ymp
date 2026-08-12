#![forbid(unsafe_code)]

mod parser;

pub use parser::parse_size;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParseSizeError {
    Empty,
    InvalidNumber,
    InvalidUnit,
    Overflow,
}

#[cfg(test)]
mod tests {
    use super::{ParseSizeError, parse_size};

    #[test]
    fn parses_public_examples() {
        assert_eq!(parse_size("0"), Ok(0));
        assert_eq!(parse_size("8B"), Ok(8));
        assert_eq!(parse_size("2KiB"), Ok(2 * 1024));
        assert_eq!(parse_size("3MiB"), Ok(3 * 1024 * 1024));
        assert_eq!(parse_size(""), Err(ParseSizeError::Empty));
        assert_eq!(parse_size("KiB"), Err(ParseSizeError::InvalidNumber));
        assert_eq!(parse_size("12KB"), Err(ParseSizeError::InvalidUnit));
    }
}
