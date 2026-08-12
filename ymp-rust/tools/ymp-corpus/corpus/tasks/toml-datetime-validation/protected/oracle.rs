use std::str::FromStr;

use toml_datetime::Datetime;

#[test]
fn protected_leap_second_boundary_is_supported() {
    assert!(Datetime::from_str("1979-05-27T07:32:60Z").is_ok());
    assert!(Datetime::from_str("1979-05-27T07:32:61Z").is_err());
}

#[test]
fn protected_gregorian_calendar_days_are_validated() {
    for value in [
        "2000-02-29",
        "2004-02-29T00:00:00",
        "2024-04-30T12:00:00Z",
    ] {
        assert!(Datetime::from_str(value).is_ok(), "value={value}");
    }
    for value in [
        "1900-02-29",
        "2001-02-29T00:00:00",
        "2024-04-31T12:00:00Z",
    ] {
        assert!(Datetime::from_str(value).is_err(), "value={value}");
    }
}
