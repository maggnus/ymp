use ymp_calibration_case::first_line_end;

#[test]
fn protected_line_ending_matrix() {
    for (input, expected) in [
        ("\r", 1),
        ("\r\n", 2),
        ("a\rb\nrest", 4),
        ("a\rb", 3),
        ("é\r\nrest", 4),
        ("é\rrest", 7),
        ("é\r", 3),
        ("😀", 4),
    ] {
        let actual = first_line_end(input);
        assert_eq!(actual, expected, "input={input:?}");
        assert!(input.is_char_boundary(actual), "input={input:?}");
    }
}
