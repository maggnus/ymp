#![forbid(unsafe_code)]

/// Returns the index of the first byte after the first complete UTF-8 line.
///
/// A line ends with `\n`, `\r\n`, or a final lone `\r`. If there is no line
/// ending, the entire input is one line. The returned index must always be a
/// UTF-8 boundary.
pub fn first_line_end(input: &str) -> usize {
    input
        .bytes()
        .position(|byte| byte == b'\n' || byte == b'\r')
        .map_or(input.len(), |index| index + 1)
}

#[cfg(test)]
mod tests {
    use super::first_line_end;

    #[test]
    fn handles_public_examples() {
        assert_eq!(first_line_end("alpha\nbeta"), 6);
        assert_eq!(first_line_end("alpha"), 5);
        assert_eq!(first_line_end(""), 0);
    }
}
