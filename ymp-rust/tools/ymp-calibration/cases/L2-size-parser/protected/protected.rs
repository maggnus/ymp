use ymp_calibration_case::{ParseSizeError, parse_size};

#[test]
fn protected_grammar_and_boundary_matrix() {
    let max_gib = u64::MAX / (1024 * 1024 * 1024);
    for (input, expected) in [
        ("00", Ok(0)),
        ("0001B", Ok(1)),
        ("1GiB", Ok(1024 * 1024 * 1024)),
        (" 1", Err(ParseSizeError::InvalidNumber)),
        ("1\n", Err(ParseSizeError::InvalidUnit)),
        ("١B", Err(ParseSizeError::InvalidNumber)),
        ("1gib", Err(ParseSizeError::InvalidUnit)),
        ("1KiBextra", Err(ParseSizeError::InvalidUnit)),
        ("18446744073709551616", Err(ParseSizeError::Overflow)),
    ] {
        assert_eq!(parse_size(input), expected, "input={input:?}");
    }
    assert_eq!(
        parse_size(&format!("{max_gib}GiB")),
        Ok(max_gib * 1024 * 1024 * 1024)
    );
    assert_eq!(
        parse_size(&format!("{}GiB", max_gib + 1)),
        Err(ParseSizeError::Overflow)
    );
}

#[test]
fn protected_inputs_never_panic() {
    for input in [
        "",
        "🦀",
        "184467440737095516160000000000000000000GiB",
        "1_0MiB",
    ] {
        assert!(
            std::panic::catch_unwind(|| parse_size(input)).is_ok(),
            "input={input:?}"
        );
    }
}
